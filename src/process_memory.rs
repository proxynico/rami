use crate::proc_list::list_all_pids;
use libc::{
    c_void, getpid, pid_t, proc_pid_rusage, proc_pidpath, rusage_info_t, rusage_info_v4,
    PROC_PIDPATHINFO_MAXSIZE, RUSAGE_INFO_V4,
};
use std::collections::HashMap;
use std::io;
use std::mem::MaybeUninit;

// `responsibility_get_pid_responsible_for_pid` is a private, undocumented libsystem SPI
// that maps a helper/agent process to the user-facing app responsible for it (e.g. rolling
// a "Google Chrome Helper" up to "Google Chrome"). Because it is not in the public SDK,
// resolve it at runtime with dlsym and degrade gracefully — no responsibility roll-up —
// if a future macOS removes it, rather than failing to launch on a missing symbol.
type ResponsibleForPidFn = unsafe extern "C" fn(pid_t) -> pid_t;

fn responsible_for_pid_fn() -> Option<ResponsibleForPidFn> {
    use std::sync::OnceLock;
    static FN: OnceLock<Option<ResponsibleForPidFn>> = OnceLock::new();
    *FN.get_or_init(|| {
        let name = c"responsibility_get_pid_responsible_for_pid";
        let sym = unsafe { libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr()) };
        if sym.is_null() {
            None
        } else {
            // SAFETY: the resolved symbol has the C signature `pid_t(pid_t)`.
            Some(unsafe { std::mem::transmute::<*mut c_void, ResponsibleForPidFn>(sym) })
        }
    })
}

const RESPONSIBILITY_MAX_HOPS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppMemoryUsage {
    pub name: String,
    pub group_key: String,
    pub footprint_bytes: u64,
    pub pids: Vec<pid_t>,
    pub delta_bytes: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppMemorySnapshot {
    Hidden,
    Loading,
    Loaded(Vec<AppMemoryUsage>),
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProcessMemoryRecord {
    pid: pid_t,
    group_key: String,
    display_name: String,
    footprint_bytes: u64,
}

pub struct ProcessMemorySampler {
    self_pid: pid_t,
}

impl Default for ProcessMemorySampler {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessMemorySampler {
    pub fn new() -> Self {
        Self {
            self_pid: unsafe { getpid() },
        }
    }

    pub fn sample(&self, top_n: usize) -> io::Result<Vec<AppMemoryUsage>> {
        let pids = list_all_pids()?;
        if pids.is_empty() {
            return Err(io::Error::other("proc_listallpids returned no pids"));
        }

        let mut records = Vec::with_capacity(pids.len());
        let mut lookup = LiveProcLookup::new();
        let mut bundle_memo = HashMap::new();
        for pid in pids {
            if should_skip_pid(pid, self.self_pid) {
                continue;
            }
            if let Some(record) = sample_pid(pid, &mut lookup, &mut bundle_memo) {
                records.push(record);
            }
        }

        let rows = aggregate(records, top_n);
        if rows.is_empty() {
            return Err(io::Error::other("no per-process memory rows available"));
        }
        Ok(rows)
    }
}

fn should_skip_pid(pid: pid_t, self_pid: pid_t) -> bool {
    pid <= 0 || pid == self_pid
}

fn sample_pid<L: ProcLookup>(
    pid: pid_t,
    lookup: &mut L,
    bundle_memo: &mut HashMap<pid_t, Option<(String, String)>>,
) -> Option<ProcessMemoryRecord> {
    let footprint = read_phys_footprint(pid)?;
    if footprint == 0 {
        return None;
    }

    let (group_key, display_name) = owning_app_bundle(pid, lookup, bundle_memo)?;
    Some(ProcessMemoryRecord {
        pid,
        group_key,
        display_name,
        footprint_bytes: footprint,
    })
}

trait ProcLookup {
    fn exec_path(&mut self, pid: pid_t) -> Option<String>;
    fn responsible_pid(&mut self, pid: pid_t) -> pid_t;
}

struct LiveProcLookup {
    pid_path_buf: Vec<u8>,
}

impl LiveProcLookup {
    fn new() -> Self {
        Self {
            pid_path_buf: vec![0; PROC_PIDPATHINFO_MAXSIZE as usize],
        }
    }
}

impl ProcLookup for LiveProcLookup {
    fn exec_path(&mut self, pid: pid_t) -> Option<String> {
        read_pid_path(pid, &mut self.pid_path_buf)
    }

    fn responsible_pid(&mut self, pid: pid_t) -> pid_t {
        match responsible_for_pid_fn() {
            // SAFETY: resolved symbol has the C signature `pid_t(pid_t)`.
            Some(responsible_for_pid) => unsafe { responsible_for_pid(pid) },
            None => 0, // SPI unavailable: terminate the walk, keeping the exec-path grouping.
        }
    }
}

fn owning_app_bundle<L: ProcLookup>(
    pid: pid_t,
    lookup: &mut L,
    bundle_memo: &mut HashMap<pid_t, Option<(String, String)>>,
) -> Option<(String, String)> {
    let mut current = pid;
    let mut last = 0;
    // The walk is bounded, so the visited set lives on the stack: no per-pid
    // heap allocation.
    let mut visited = [0 as pid_t; RESPONSIBILITY_MAX_HOPS];
    let mut visited_len = 0;
    for _ in 0..RESPONSIBILITY_MAX_HOPS {
        if current <= 0 {
            return cache_bundle_resolution(bundle_memo, &visited[..visited_len], None);
        }
        if let Some(resolution) = bundle_memo.get(&current).cloned() {
            return cache_bundle_resolution(bundle_memo, &visited[..visited_len], resolution);
        }
        visited[visited_len] = current;
        visited_len += 1;
        if let Some(path) = lookup.exec_path(current) {
            if let Some((bundle_path, app_segment)) = first_app_bundle(&path) {
                if is_user_facing_app_bundle(&bundle_path) {
                    let display = app_segment
                        .strip_suffix(".app")
                        .unwrap_or(app_segment)
                        .to_string();
                    return cache_bundle_resolution(
                        bundle_memo,
                        &visited[..visited_len],
                        Some((bundle_path, display)),
                    );
                }
                // System agent .app — keep walking the responsibility chain in case
                // the agent was spawned on behalf of a real user-facing app.
            }
        }
        let responsible = lookup.responsible_pid(current);
        if responsible <= 0 || responsible == current || responsible == last {
            return cache_bundle_resolution(bundle_memo, &visited[..visited_len], None);
        }
        last = current;
        current = responsible;
    }
    cache_bundle_resolution(bundle_memo, &visited[..visited_len], None)
}

fn cache_bundle_resolution(
    bundle_memo: &mut HashMap<pid_t, Option<(String, String)>>,
    visited: &[pid_t],
    resolution: Option<(String, String)>,
) -> Option<(String, String)> {
    for &visited_pid in visited {
        bundle_memo.insert(visited_pid, resolution.clone());
    }
    resolution
}

fn is_user_facing_app_bundle(bundle_path: &str) -> bool {
    !bundle_path.starts_with("/System/Library/")
        && !bundle_path.starts_with("/Library/")
        && !bundle_path.starts_with("/usr/")
        && !bundle_path.starts_with("/private/")
}

fn read_phys_footprint(pid: pid_t) -> Option<u64> {
    let mut info = MaybeUninit::<rusage_info_v4>::zeroed();
    let rc =
        unsafe { proc_pid_rusage(pid, RUSAGE_INFO_V4, info.as_mut_ptr() as *mut rusage_info_t) };
    if rc != 0 {
        return None;
    }
    let info = unsafe { info.assume_init() };
    Some(info.ri_phys_footprint)
}

fn read_pid_path(pid: pid_t, buf: &mut [u8]) -> Option<String> {
    let len = unsafe { proc_pidpath(pid, buf.as_mut_ptr() as *mut c_void, buf.len() as u32) };
    if len <= 0 {
        return None;
    }
    Some(String::from_utf8_lossy(&buf[..len as usize]).into_owned())
}

fn first_app_bundle(exec_path: &str) -> Option<(String, &str)> {
    let needle = ".app/Contents/";
    let idx = exec_path.find(needle)?;
    let bundle_end = idx + ".app".len();
    let bundle_path = &exec_path[..bundle_end];
    let app_segment = bundle_path.rsplit('/').next()?;
    Some((bundle_path.to_string(), app_segment))
}

fn aggregate(records: Vec<ProcessMemoryRecord>, top_n: usize) -> Vec<AppMemoryUsage> {
    if records.is_empty() {
        return Vec::new();
    }

    let mut by_group: HashMap<String, (String, u64, Vec<pid_t>)> = HashMap::new();
    for r in records {
        let entry = by_group
            .entry(r.group_key)
            .or_insert_with(|| (r.display_name.clone(), 0, Vec::new()));
        entry.1 = entry.1.saturating_add(r.footprint_bytes);
        entry.2.push(r.pid);
    }

    let mut rows: Vec<AppMemoryUsage> = by_group
        .into_iter()
        .map(|(group_key, (name, bytes, mut pids))| {
            pids.sort_unstable();
            AppMemoryUsage {
                name,
                group_key,
                footprint_bytes: bytes,
                pids,
                delta_bytes: None,
            }
        })
        .collect();

    rows.sort_by(|a, b| {
        b.footprint_bytes
            .cmp(&a.footprint_bytes)
            .then_with(|| a.name.cmp(&b.name))
    });
    rows.truncate(top_n);
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn record(pid: pid_t, group: &str, name: &str, bytes: u64) -> ProcessMemoryRecord {
        ProcessMemoryRecord {
            pid,
            group_key: group.to_string(),
            display_name: name.to_string(),
            footprint_bytes: bytes,
        }
    }

    struct FakeProcLookup {
        paths: HashMap<pid_t, String>,
        responsible: HashMap<pid_t, pid_t>,
        exec_path_calls: HashMap<pid_t, usize>,
    }

    impl FakeProcLookup {
        fn new() -> Self {
            Self {
                paths: HashMap::new(),
                responsible: HashMap::new(),
                exec_path_calls: HashMap::new(),
            }
        }

        fn with_path(mut self, pid: pid_t, path: &str) -> Self {
            self.paths.insert(pid, path.to_string());
            self
        }

        fn responsible(mut self, pid: pid_t, parent: pid_t) -> Self {
            self.responsible.insert(pid, parent);
            self
        }

        fn exec_path_call_count(&self, pid: pid_t) -> usize {
            *self.exec_path_calls.get(&pid).unwrap_or(&0)
        }
    }

    impl ProcLookup for FakeProcLookup {
        fn exec_path(&mut self, pid: pid_t) -> Option<String> {
            *self.exec_path_calls.entry(pid).or_default() += 1;
            self.paths.get(&pid).cloned()
        }

        fn responsible_pid(&mut self, pid: pid_t) -> pid_t {
            *self.responsible.get(&pid).unwrap_or(&pid)
        }
    }

    fn owning_bundle_with_fresh_memo<L: ProcLookup>(
        pid: pid_t,
        lookup: &mut L,
    ) -> Option<(String, String)> {
        owning_app_bundle(pid, lookup, &mut HashMap::new())
    }

    #[test]
    fn owning_bundle_uses_pid_path_when_inside_app() {
        let mut lookup =
            FakeProcLookup::new().with_path(42, "/Applications/Cursor.app/Contents/MacOS/Cursor");
        let (key, name) = owning_bundle_with_fresh_memo(42, &mut lookup).expect("bundle");
        assert_eq!(key, "/Applications/Cursor.app");
        assert_eq!(name, "Cursor");
    }

    #[test]
    fn owning_bundle_walks_responsibility_chain_to_app() {
        let mut lookup = FakeProcLookup::new()
            .with_path(
                100,
                "/System/Library/Frameworks/WebKit.framework/.../com.apple.WebKit.WebContent",
            )
            .with_path(7, "/Applications/Safari.app/Contents/MacOS/Safari")
            .responsible(100, 7);
        let (key, name) = owning_bundle_with_fresh_memo(100, &mut lookup).expect("rolled up");
        assert_eq!(key, "/Applications/Safari.app");
        assert_eq!(name, "Safari");
    }

    #[test]
    fn owning_bundle_drops_pid_with_no_responsible_app() {
        let mut lookup = FakeProcLookup::new().with_path(55, "/usr/sbin/cfprefsd");
        // cfprefsd is its own responsible pid (chain terminates without an .app)
        assert!(owning_bundle_with_fresh_memo(55, &mut lookup).is_none());
    }

    #[test]
    fn owning_bundle_skips_system_agent_app_and_walks_to_real_app() {
        let mut lookup = FakeProcLookup::new()
            .with_path(
                200,
                "/System/Library/PrivateFrameworks/Foo.framework/sociallayerd.app/Contents/MacOS/sociallayerd",
            )
            .with_path(7, "/Applications/Messages.app/Contents/MacOS/Messages")
            .responsible(200, 7);
        let (key, name) = owning_bundle_with_fresh_memo(200, &mut lookup).expect("rolled up");
        assert_eq!(key, "/Applications/Messages.app");
        assert_eq!(name, "Messages");
    }

    #[test]
    fn owning_bundle_drops_system_agent_with_no_real_responsible_app() {
        let mut lookup = FakeProcLookup::new().with_path(
            201,
            "/System/Library/PrivateFrameworks/Foo.framework/privatecloudcomputed.app/Contents/MacOS/privatecloudcomputed",
        );
        assert!(owning_bundle_with_fresh_memo(201, &mut lookup).is_none());
    }

    #[test]
    fn owning_bundle_short_circuits_on_self_responsible_loop() {
        let mut lookup = FakeProcLookup::new()
            .with_path(9, "/usr/libexec/some-helper")
            .responsible(9, 9);
        assert!(owning_bundle_with_fresh_memo(9, &mut lookup).is_none());
    }

    #[test]
    fn owning_bundle_outer_app_wins_when_helper_is_nested_app() {
        let mut lookup = FakeProcLookup::new().with_path(
            33,
            "/Applications/Google Chrome.app/Contents/Frameworks/Google Chrome Framework.framework/Versions/Current/Helpers/Google Chrome Helper.app/Contents/MacOS/Google Chrome Helper",
        );
        let (key, name) = owning_bundle_with_fresh_memo(33, &mut lookup).expect("bundle");
        assert_eq!(key, "/Applications/Google Chrome.app");
        assert_eq!(name, "Google Chrome");
    }

    #[test]
    fn owning_bundle_memoizes_shared_responsible_parent() {
        let mut lookup = FakeProcLookup::new()
            .with_path(100, "/usr/libexec/helper-one")
            .with_path(101, "/usr/libexec/helper-two")
            .with_path(7, "/Applications/Safari.app/Contents/MacOS/Safari")
            .responsible(100, 7)
            .responsible(101, 7);
        let mut bundle_memo = HashMap::new();

        assert!(owning_app_bundle(100, &mut lookup, &mut bundle_memo).is_some());
        assert!(owning_app_bundle(101, &mut lookup, &mut bundle_memo).is_some());

        assert_eq!(lookup.exec_path_call_count(7), 1);
    }

    #[test]
    fn aggregate_sums_helpers() {
        let records = vec![
            record(1, "/Applications/Cursor.app", "Cursor", 100),
            record(2, "/Applications/Cursor.app", "Cursor", 200),
            record(3, "/Applications/Cursor.app", "Cursor", 300),
        ];
        let rows = aggregate(records, 5);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Cursor");
        assert_eq!(rows[0].footprint_bytes, 600);
        assert_eq!(rows[0].pids, vec![1, 2, 3]);
    }

    #[test]
    fn aggregate_sorts_desc_then_name_asc() {
        let records = vec![
            record(1, "B", "B", 100),
            record(2, "A", "A", 100),
            record(3, "C", "C", 200),
        ];
        let rows = aggregate(records, 5);
        assert_eq!(rows[0].name, "C");
        assert_eq!(rows[1].name, "A");
        assert_eq!(rows[2].name, "B");
    }

    #[test]
    fn aggregate_truncates_to_top_n() {
        let records: Vec<_> = (0..7)
            .map(|i| record(i + 1, &format!("g{i}"), &format!("g{i}"), 100 + i as u64))
            .collect();
        let rows = aggregate(records, 5);
        assert_eq!(rows.len(), 5);
    }

    #[test]
    fn aggregate_empty_input_returns_empty() {
        let rows = aggregate(vec![], 5);
        assert!(rows.is_empty());
    }

    #[test]
    fn self_pid_is_filtered() {
        assert!(should_skip_pid(42, 42));
        assert!(!should_skip_pid(99, 42));
    }

    #[test]
    fn nonpositive_pids_are_skipped() {
        assert!(should_skip_pid(0, 42));
        assert!(should_skip_pid(-1, 42));
    }

    #[test]
    #[ignore]
    fn smoke_sample_against_real_processes() {
        let sampler = ProcessMemorySampler::new();
        let started = std::time::Instant::now();
        let rows = sampler.sample(5).expect("sample");
        let elapsed = started.elapsed();
        eprintln!("scan took {elapsed:?}, returned {} rows:", rows.len());
        for row in &rows {
            eprintln!(
                "  {:30} {:>8} MB",
                row.name,
                row.footprint_bytes / 1_000_000
            );
        }
        assert!(!rows.is_empty());
        assert!(rows.len() <= 5);
    }
}
