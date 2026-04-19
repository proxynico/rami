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

    let original_home = std::env::var_os("HOME");
    std::env::set_var("HOME", &home);

    let first = AppLock::acquire().unwrap();
    assert!(first.is_some());

    let second = AppLock::acquire().unwrap();
    assert!(second.is_none());

    match original_home {
        Some(value) => std::env::set_var("HOME", value),
        None => std::env::remove_var("HOME"),
    }
}
