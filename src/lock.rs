use libc::{flock, LOCK_EX, LOCK_NB};
use std::fs::{create_dir_all, File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};

pub struct AppLock {
    _file: File,
}

#[derive(Debug)]
enum FlockOutcome {
    Acquired,
    Contended,
}

pub fn lock_file_path(home: &Path) -> PathBuf {
    home.join("Library/Application Support/rami/rami.lock")
}

impl AppLock {
    pub fn acquire() -> io::Result<Option<Self>> {
        let home = std::env::var("HOME")
            .map(PathBuf::from)
            .map_err(|_| io::Error::new(io::ErrorKind::NotFound, "HOME not set"))?;
        Self::acquire_at_home(&home)
    }

    fn acquire_at_home(home: &Path) -> io::Result<Option<Self>> {
        let path = lock_file_path(home);
        let parent = path
            .parent()
            .expect("lock file should have a parent directory");
        create_dir_all(parent)?;

        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(path)?;

        match classify_flock_result(
            unsafe { flock(file.as_raw_fd(), LOCK_EX | LOCK_NB) },
            io::Error::last_os_error(),
        )? {
            FlockOutcome::Acquired => Ok(Some(Self { _file: file })),
            FlockOutcome::Contended => Ok(None),
        }
    }
}

fn classify_flock_result(rc: libc::c_int, err: io::Error) -> io::Result<FlockOutcome> {
    if rc == 0 {
        return Ok(FlockOutcome::Acquired);
    }

    match err.raw_os_error() {
        Some(code) if code == libc::EWOULDBLOCK || code == libc::EAGAIN => {
            Ok(FlockOutcome::Contended)
        }
        _ => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_flock_result, FlockOutcome};
    use std::io;

    #[test]
    fn classify_flock_result_maps_contention_to_contended() {
        let result =
            classify_flock_result(-1, io::Error::from_raw_os_error(libc::EWOULDBLOCK)).unwrap();

        assert!(matches!(result, FlockOutcome::Contended));
    }

    #[test]
    fn classify_flock_result_preserves_real_errors() {
        let err =
            classify_flock_result(-1, io::Error::from_raw_os_error(libc::ENOENT)).unwrap_err();

        assert_eq!(err.raw_os_error(), Some(libc::ENOENT));
    }
}
