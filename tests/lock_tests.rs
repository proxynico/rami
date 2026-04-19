use rami::lock::{lock_file_path, AppLock};
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

static HOME_ENV_LOCK: Mutex<()> = Mutex::new(());

struct EnvVarGuard {
    key: &'static str,
    original: Option<OsString>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
        let original = std::env::var_os(key);
        std::env::set_var(key, value);
        Self { key, original }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match &self.original {
            Some(value) => std::env::set_var(self.key, value),
            None => std::env::remove_var(self.key),
        }
    }
}

fn lock_home_env() -> MutexGuard<'static, ()> {
    HOME_ENV_LOCK.lock().unwrap()
}

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
    let _env_lock = lock_home_env();
    let home = std::env::temp_dir().join(format!("rami-lock-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);

    let _home_guard = EnvVarGuard::set("HOME", &home);

    let first = AppLock::acquire().unwrap();
    assert!(first.is_some());

    let second = AppLock::acquire().unwrap();
    assert!(second.is_none());
}

#[test]
fn home_guard_restores_home_after_panic() {
    let _env_lock = lock_home_env();
    let original_home = std::env::var_os("HOME");

    let _ = std::panic::catch_unwind(|| {
        let _home_guard = EnvVarGuard::set("HOME", "/tmp/rami-home-guard-panic");
        panic!("force unwind");
    });

    assert_eq!(std::env::var_os("HOME"), original_home);
}
