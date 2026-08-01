use libc::{c_int, pid_t, proc_listallpids};
use std::io;

/// Enumerate every pid on the system, shared by the app-memory and
/// process-CPU scans.
///
/// `proc_listallpids` returns a *count of pids* (it divides the underlying
/// byte count by sizeof(pid_t) itself); only the buffer size argument is in
/// bytes. Treating the return value as bytes silently truncates the scan to a
/// quarter of the process list.
pub(crate) fn list_all_pids() -> io::Result<Vec<pid_t>> {
    let pid_count = unsafe { proc_listallpids(std::ptr::null_mut(), 0) };
    if pid_count <= 0 {
        return Err(io::Error::last_os_error());
    }

    // Headroom for processes spawned between the two calls.
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
