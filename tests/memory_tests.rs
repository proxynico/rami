use rami::memory::{snapshot_from_counts, MemoryCounts};

#[test]
fn snapshot_uses_active_wired_and_compressed_bytes() {
    let counts = MemoryCounts {
        total_bytes: 1000,
        page_size: 10,
        active_pages: 30,
        wired_pages: 20,
        compressed_pages: 10,
    };

    let snapshot = snapshot_from_counts(counts);

    assert_eq!(snapshot.used_bytes, 600);
    assert_eq!(snapshot.total_bytes, 1000);
    assert_eq!(snapshot.used_percent, 60);
}

#[test]
fn snapshot_rounds_to_nearest_whole_percent() {
    let counts = MemoryCounts {
        total_bytes: 1000,
        page_size: 1,
        active_pages: 524,
        wired_pages: 0,
        compressed_pages: 0,
    };

    let snapshot = snapshot_from_counts(counts);

    assert_eq!(snapshot.used_percent, 52);
}

#[test]
fn snapshot_clamps_when_used_exceeds_total() {
    let counts = MemoryCounts {
        total_bytes: 100,
        page_size: 10,
        active_pages: 8,
        wired_pages: 3,
        compressed_pages: 2,
    };

    let snapshot = snapshot_from_counts(counts);

    assert_eq!(snapshot.used_bytes, 130);
    assert_eq!(snapshot.used_percent, 100);
}
