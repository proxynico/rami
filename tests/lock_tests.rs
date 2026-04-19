use rami::lock::{lock_file_path, AppLock};
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
