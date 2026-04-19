use rami::lock::lock_file_path;
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
