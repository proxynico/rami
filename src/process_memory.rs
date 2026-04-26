use libc::{
    c_int, c_void, getpid, pid_t, proc_listallpids, proc_name, proc_pid_rusage, proc_pidpath,
    rusage_info_t, rusage_info_v4, PROC_PIDPATHINFO_MAXSIZE, RUSAGE_INFO_V4,
};
use std::collections::HashMap;
use std::io;
use std::mem::MaybeUninit;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppMemoryUsage {
    pub name: String,
    pub footprint_bytes: u64,
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
        for pid in pids {
            if should_skip_pid(pid, self.self_pid) {
                continue;
            }
            if let Some(record) = sample_pid(pid) {
                records.push(record);
            }
        }

        let rows = aggregate(records, top_n);
        if rows.is_empty() {
            return Err(io::Error::other(
                "no per-process memory rows available",
            ));
        }
        Ok(rows)
    }
}

fn list_all_pids() -> io::Result<Vec<pid_t>> {
    let needed_bytes = unsafe { proc_listallpids(std::ptr::null_mut(), 0) };
    if needed_bytes <= 0 {
        return Err(io::Error::last_os_error());
    }

    let cap = (needed_bytes as usize / std::mem::size_of::<pid_t>()) + 32;
    let mut buf: Vec<pid_t> = vec![0; cap];
    let buf_bytes = (cap * std::mem::size_of::<pid_t>()) as c_int;

    let written = unsafe { proc_listallpids(buf.as_mut_ptr() as *mut c_void, buf_bytes) };
    if written <= 0 {
        return Err(io::Error::last_os_error());
    }

    let count = written as usize / std::mem::size_of::<pid_t>();
    buf.truncate(count);
    Ok(buf)
}

fn should_skip_pid(pid: pid_t, self_pid: pid_t) -> bool {
    pid <= 0 || pid == self_pid
}

fn sample_pid(pid: pid_t) -> Option<ProcessMemoryRecord> {
    let footprint = read_phys_footprint(pid)?;
    if footprint == 0 {
        return None;
    }

    let path = read_pid_path(pid).unwrap_or_default();
    let name = read_pid_name(pid).unwrap_or_default();
    if path.is_empty() && name.is_empty() {
        return None;
    }

    let (group_key, display_name) = group_key_and_name(&path, &name);
    Some(ProcessMemoryRecord {
        pid,
        group_key,
        display_name,
        footprint_bytes: footprint,
    })
}

fn read_phys_footprint(pid: pid_t) -> Option<u64> {
    let mut info = MaybeUninit::<rusage_info_v4>::zeroed();
    let rc = unsafe {
        proc_pid_rusage(
            pid,
            RUSAGE_INFO_V4,
            info.as_mut_ptr() as *mut rusage_info_t,
        )
    };
    if rc != 0 {
        return None;
    }
    let info = unsafe { info.assume_init() };
    Some(info.ri_phys_footprint)
}

fn read_pid_path(pid: pid_t) -> Option<String> {
    let mut buf = vec![0u8; PROC_PIDPATHINFO_MAXSIZE as usize];
    let len = unsafe {
        proc_pidpath(
            pid,
            buf.as_mut_ptr() as *mut c_void,
            buf.len() as u32,
        )
    };
    if len <= 0 {
        return None;
    }
    buf.truncate(len as usize);
    Some(String::from_utf8_lossy(&buf).into_owned())
}

fn read_pid_name(pid: pid_t) -> Option<String> {
    let mut buf = vec![0u8; 256];
    let len = unsafe {
        proc_name(
            pid,
            buf.as_mut_ptr() as *mut c_void,
            buf.len() as u32,
        )
    };
    if len <= 0 {
        return None;
    }
    buf.truncate(len as usize);
    Some(String::from_utf8_lossy(&buf).into_owned())
}

fn group_key_and_name(exec_path: &str, proc_name: &str) -> (String, String) {
    if let Some((bundle_path, app_segment)) = first_app_bundle(exec_path) {
        let display = app_segment
            .strip_suffix(".app")
            .unwrap_or(app_segment)
            .to_string();
        return (bundle_path, display);
    }

    if !proc_name.is_empty() {
        return (proc_name.to_string(), proc_name.to_string());
    }

    let basename = exec_path
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_string();
    (basename.clone(), basename)
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

    let mut by_group: HashMap<String, (String, u64)> = HashMap::new();
    for r in records {
        let entry = by_group
            .entry(r.group_key)
            .or_insert_with(|| (r.display_name.clone(), 0));
        entry.1 += r.footprint_bytes;
    }

    let mut rows: Vec<AppMemoryUsage> = by_group
        .into_iter()
        .map(|(_, (name, bytes))| AppMemoryUsage {
            name,
            footprint_bytes: bytes,
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

    fn record(pid: pid_t, group: &str, name: &str, bytes: u64) -> ProcessMemoryRecord {
        ProcessMemoryRecord {
            pid,
            group_key: group.to_string(),
            display_name: name.to_string(),
            footprint_bytes: bytes,
        }
    }

    #[test]
    fn groups_helper_under_outer_app() {
        let path = "/Applications/Google Chrome.app/Contents/Frameworks/Google Chrome Framework.framework/Versions/Current/Helpers/Google Chrome Helper.app/Contents/MacOS/Google Chrome Helper";
        let (key, name) = group_key_and_name(path, "Google Chrome Helper");
        assert_eq!(key, "/Applications/Google Chrome.app");
        assert_eq!(name, "Google Chrome");
    }

    #[test]
    fn falls_back_to_proc_name_for_non_app() {
        let (key, name) = group_key_and_name("/usr/sbin/cfprefsd", "cfprefsd");
        assert_eq!(key, "cfprefsd");
        assert_eq!(name, "cfprefsd");
    }

    #[test]
    fn empty_path_falls_back_to_proc_name() {
        let (key, name) = group_key_and_name("", "launchd");
        assert_eq!(key, "launchd");
        assert_eq!(name, "launchd");
    }

    #[test]
    fn empty_name_falls_back_to_path_basename() {
        let (key, name) = group_key_and_name("/usr/bin/odd-binary", "");
        assert_eq!(key, "odd-binary");
        assert_eq!(name, "odd-binary");
    }

    #[test]
    fn group_key_first_app_segment_wins() {
        let path = "/Applications/Outer.app/Contents/MacOS/Inner.app/Contents/MacOS/Bar";
        let (key, name) = group_key_and_name(path, "Bar");
        assert_eq!(key, "/Applications/Outer.app");
        assert_eq!(name, "Outer");
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
}
