use libc::{
    c_int, c_void, getpid, pid_t, proc_listallpids, proc_pid_rusage, proc_pidpath, rusage_info_t,
    rusage_info_v4, PROC_PIDPATHINFO_MAXSIZE, RUSAGE_INFO_V4,
};
use std::collections::HashMap;
use std::io;
use std::mem::MaybeUninit;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

const SAMPLE_INTERVAL: Duration = Duration::from_millis(200);
pub(crate) const PROCESS_CPU_ROW_LIMIT: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProcessCpuUsage {
    pub(crate) name: String,
    pub(crate) utilization_percent: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProcessCpuSnapshot {
    Hidden,
    Loading,
    Loaded(Vec<ProcessCpuUsage>),
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProcessCpuRecord {
    pid: pid_t,
    start_time: u64,
    name: String,
    cpu_time_ns: u64,
}

pub(crate) struct ProcessCpuSampler {
    self_pid: pid_t,
    interval: Duration,
}

impl ProcessCpuSampler {
    pub(crate) fn new() -> Self {
        Self {
            self_pid: unsafe { getpid() },
            interval: SAMPLE_INTERVAL,
        }
    }

    pub(crate) fn sample(&self, top_n: usize) -> io::Result<Vec<ProcessCpuUsage>> {
        let previous = read_process_records(self.self_pid)?;
        let started = Instant::now();
        thread::sleep(self.interval);
        let current = read_process_records(self.self_pid)?;
        Ok(process_usage_between(
            &previous,
            &current,
            started.elapsed(),
            top_n,
        ))
    }
}

impl Default for ProcessCpuSampler {
    fn default() -> Self {
        Self::new()
    }
}

fn process_usage_between(
    previous: &[ProcessCpuRecord],
    current: &[ProcessCpuRecord],
    elapsed: Duration,
    top_n: usize,
) -> Vec<ProcessCpuUsage> {
    let elapsed_ns = elapsed.as_nanos();
    if elapsed_ns == 0 || top_n == 0 {
        return Vec::new();
    }

    let previous_times: HashMap<_, _> = previous
        .iter()
        .map(|record| ((record.pid, record.start_time), record.cpu_time_ns))
        .collect();
    let mut ranked: Vec<_> = current
        .iter()
        .filter_map(|record| {
            let previous_time = previous_times.get(&(record.pid, record.start_time))?;
            let delta_ns = record.cpu_time_ns.checked_sub(*previous_time)?;
            let rounded_percent = (u128::from(delta_ns) * 100 + elapsed_ns / 2) / elapsed_ns;
            let utilization_percent = u16::try_from(rounded_percent).unwrap_or(u16::MAX);
            (utilization_percent > 0).then_some((
                record.pid,
                ProcessCpuUsage {
                    name: record.name.clone(),
                    utilization_percent,
                },
            ))
        })
        .collect();
    ranked.sort_by(|(left_pid, left), (right_pid, right)| {
        right
            .utilization_percent
            .cmp(&left.utilization_percent)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left_pid.cmp(right_pid))
    });
    ranked.truncate(top_n);
    ranked.into_iter().map(|(_, usage)| usage).collect()
}

fn read_process_records(self_pid: pid_t) -> io::Result<Vec<ProcessCpuRecord>> {
    let pids = list_all_pids()?;
    let records: Vec<_> = pids
        .into_iter()
        .filter(|pid| *pid > 0 && *pid != self_pid)
        .filter_map(read_process_record)
        .collect();
    if records.is_empty() {
        Err(io::Error::other("no readable process CPU records"))
    } else {
        Ok(records)
    }
}

fn list_all_pids() -> io::Result<Vec<pid_t>> {
    let pid_count = unsafe { proc_listallpids(std::ptr::null_mut(), 0) };
    if pid_count <= 0 {
        return Err(io::Error::last_os_error());
    }

    let capacity = pid_count as usize + 32;
    let mut pids = vec![0; capacity];
    let buffer_bytes = capacity
        .checked_mul(std::mem::size_of::<pid_t>())
        .and_then(|bytes| c_int::try_from(bytes).ok())
        .ok_or_else(|| io::Error::other("process ID buffer is too large"))?;
    let written = unsafe { proc_listallpids(pids.as_mut_ptr().cast(), buffer_bytes) };
    if written <= 0 {
        return Err(io::Error::last_os_error());
    }
    pids.truncate(written as usize);
    Ok(pids)
}

fn read_process_record(pid: pid_t) -> Option<ProcessCpuRecord> {
    let mut usage = MaybeUninit::<rusage_info_v4>::zeroed();
    let result = unsafe {
        proc_pid_rusage(
            pid,
            RUSAGE_INFO_V4,
            usage.as_mut_ptr().cast::<rusage_info_t>(),
        )
    };
    if result != 0 {
        return None;
    }
    let usage = unsafe { usage.assume_init() };
    if usage.ri_proc_exit_abstime != 0 {
        return None;
    }
    let cpu_time_ns = usage.ri_user_time.checked_add(usage.ri_system_time)?;

    let mut path_buffer = vec![0_u8; PROC_PIDPATHINFO_MAXSIZE as usize];
    let path_length = unsafe {
        proc_pidpath(
            pid,
            path_buffer.as_mut_ptr().cast::<c_void>(),
            u32::try_from(path_buffer.len()).ok()?,
        )
    };
    if path_length <= 0 {
        return None;
    }
    path_buffer.truncate(path_length as usize);
    let path = std::str::from_utf8(&path_buffer).ok()?;
    Some(ProcessCpuRecord {
        pid,
        start_time: usage.ri_proc_start_abstime,
        name: process_name_from_path(path)?,
        cpu_time_ns,
    })
}

fn process_name_from_path(path: &str) -> Option<String> {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(pid: pid_t, start_time: u64, name: &str, cpu_time_ns: u64) -> ProcessCpuRecord {
        ProcessCpuRecord {
            pid,
            start_time,
            name: name.to_string(),
            cpu_time_ns,
        }
    }

    #[test]
    fn process_cpu_rows_rank_delta_percentages_and_allow_multiple_cores() {
        let previous = vec![
            record(10, 1, "Editor", 1_000_000_000),
            record(20, 2, "Browser", 2_000_000_000),
            record(30, 3, "Music", 3_000_000_000),
        ];
        let current = vec![
            record(10, 1, "Editor", 1_300_000_000),
            record(20, 2, "Browser", 2_050_000_000),
            record(30, 3, "Music", 3_001_000_000),
        ];

        let rows = process_usage_between(&previous, &current, Duration::from_millis(100), 2);

        assert_eq!(
            rows,
            vec![
                ProcessCpuUsage {
                    name: "Editor".to_string(),
                    utilization_percent: 300,
                },
                ProcessCpuUsage {
                    name: "Browser".to_string(),
                    utilization_percent: 50,
                },
            ]
        );
    }

    #[test]
    fn exited_unreadable_and_reused_processes_are_skipped() {
        let previous = vec![
            record(10, 1, "Exited", 10),
            record(20, 2, "Unreadable", 20),
            record(30, 3, "Old Process", 30),
            record(40, 4, "Still Running", 40),
        ];
        let current = vec![
            // PIDs 10 and 20 are absent because exit/read failures are omitted by sampling.
            record(30, 99, "Reused PID", 1_000),
            record(40, 4, "Still Running", 50_000_040),
        ];

        assert_eq!(
            process_usage_between(&previous, &current, Duration::from_millis(100), 5,),
            vec![ProcessCpuUsage {
                name: "Still Running".to_string(),
                utilization_percent: 50,
            }]
        );
    }

    #[test]
    fn process_name_uses_the_executable_name() {
        assert_eq!(
            process_name_from_path("/Applications/Zen.app/Contents/MacOS/Zen"),
            Some("Zen".to_string())
        );
        assert_eq!(process_name_from_path(""), None);
        assert_eq!(process_name_from_path("/"), None);
    }

    #[test]
    #[ignore = "requires live macOS processes"]
    fn smoke_samples_live_process_cpu_activity() {
        let rows = ProcessCpuSampler::new()
            .sample(5)
            .expect("live process CPU sample should succeed");

        assert!(rows.len() <= 5);
        assert!(rows.iter().all(|row| !row.name.is_empty()));
    }
}
