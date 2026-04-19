use libc::{flock, LOCK_EX, LOCK_NB};
use std::fs::{create_dir_all, File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};

pub struct AppLock {
    _file: File,
}

pub fn lock_file_path(home: &Path) -> PathBuf {
    home.join("Library/Application Support/rami/rami.lock")
}

impl AppLock {
    pub fn acquire() -> io::Result<Option<Self>> {
        let home = std::env::var("HOME")
            .map(PathBuf::from)
            .map_err(|_| io::Error::new(io::ErrorKind::NotFound, "HOME not set"))?;
        let path = lock_file_path(&home);
        let parent = path
            .parent()
            .expect("lock file should have a parent directory");
        create_dir_all(parent)?;

        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(path)?;

        let rc = unsafe { flock(file.as_raw_fd(), LOCK_EX | LOCK_NB) };
        if rc == 0 {
            Ok(Some(Self { _file: file }))
        } else {
            Ok(None)
        }
    }
}
