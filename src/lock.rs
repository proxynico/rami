use libc::{flock, LOCK_EX, LOCK_NB};
use std::fs::{create_dir_all, File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};

pub struct AppLock {
    _file: File,
}

fn lock_file_path(home: &Path) -> PathBuf {
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
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)?;

        if unsafe { flock(file.as_raw_fd(), LOCK_EX | LOCK_NB) } == 0 {
            return Ok(Some(Self { _file: file }));
        }
        let err = io::Error::last_os_error();
        match err.raw_os_error() {
            Some(libc::EWOULDBLOCK) => Ok(None),
            _ => Err(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{lock_file_path, AppLock};
    use std::path::PathBuf;

    #[test]
    fn lock_file_path_lives_in_application_support() {
        let home = PathBuf::from("/tmp/rami-home");
        let path = lock_file_path(&home);

        assert_eq!(
            path,
            PathBuf::from("/tmp/rami-home/Library/Application Support/rami/rami.lock")
        );
    }

    #[test]
    fn second_acquire_on_same_path_reports_contention() {
        let home = std::env::temp_dir().join(format!("rami-lock-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);

        let first = AppLock::acquire_at_home(&home).unwrap();
        assert!(first.is_some());

        let second = AppLock::acquire_at_home(&home).unwrap();
        assert!(second.is_none());
    }
}
