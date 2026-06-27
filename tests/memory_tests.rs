use rami::memory::{snapshot_from_counts, validate_stats_count, MemoryCounts};

#[test]
fn snapshot_uses_active_wired_and_compressed_bytes() {
    let counts = MemoryCounts {
        total_bytes: 1000,
        page_size: 10,
        active_pages: 30,
        wired_pages: 20,
        compressed_pages: 10,
        free_pages: 0,
        inactive_pages: 0,
        speculative_pages: 0,
        purgeable_pages: 0,
    };

    let snapshot = snapshot_from_counts(counts, 0);

    assert_eq!(snapshot.used_bytes, 600);
    assert_eq!(snapshot.total_bytes, 1000);
    assert_eq!(snapshot.used_percent, 60);
    assert_eq!(snapshot.swap_used_bytes, 0);
}

#[test]
fn snapshot_sums_available_from_reclaimable_pools() {
    let counts = MemoryCounts {
        total_bytes: 1000,
        page_size: 10,
        active_pages: 30,
        wired_pages: 20,
        compressed_pages: 10,
        free_pages: 5,
        inactive_pages: 4,
        speculative_pages: 1,
        purgeable_pages: 2,
    };

    let snapshot = snapshot_from_counts(counts, 0);

    // (5 + 4 + 1 + 2) pages * 10 byte pages = 120 bytes reclaimable.
    assert_eq!(snapshot.available_bytes, 120);
}

#[test]
fn snapshot_rounds_to_nearest_whole_percent() {
    let counts = MemoryCounts {
        total_bytes: 1000,
        page_size: 1,
        active_pages: 524,
        wired_pages: 0,
        compressed_pages: 0,
        free_pages: 0,
        inactive_pages: 0,
        speculative_pages: 0,
        purgeable_pages: 0,
    };

    let snapshot = snapshot_from_counts(counts, 0);

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
        free_pages: 0,
        inactive_pages: 0,
        speculative_pages: 0,
        purgeable_pages: 0,
    };

    let snapshot = snapshot_from_counts(counts, 0);

    assert_eq!(snapshot.used_bytes, 130);
    assert_eq!(snapshot.used_percent, 100);
    assert_eq!(snapshot.swap_used_bytes, 0);
}

#[test]
fn snapshot_returns_zero_percent_when_total_bytes_is_zero() {
    let counts = MemoryCounts {
        total_bytes: 0,
        page_size: 10,
        active_pages: 8,
        wired_pages: 3,
        compressed_pages: 2,
        free_pages: 0,
        inactive_pages: 0,
        speculative_pages: 0,
        purgeable_pages: 0,
    };

    let snapshot = snapshot_from_counts(counts, 0);

    assert_eq!(snapshot.used_bytes, 130);
    assert_eq!(snapshot.total_bytes, 0);
    assert_eq!(snapshot.used_percent, 0);
    assert_eq!(snapshot.swap_used_bytes, 0);
}

#[test]
fn validate_stats_count_rejects_incomplete_host_stats() {
    let error = validate_stats_count(0).expect_err("count should be rejected");

    assert_eq!(error.kind(), std::io::ErrorKind::UnexpectedEof);
    assert!(error
        .to_string()
        .contains("insufficient host statistics count"));
}

#[test]
fn snapshot_carries_swap_usage() {
    let counts = MemoryCounts {
        total_bytes: 1000,
        page_size: 10,
        active_pages: 30,
        wired_pages: 20,
        compressed_pages: 10,
        free_pages: 0,
        inactive_pages: 0,
        speculative_pages: 0,
        purgeable_pages: 0,
    };

    let snapshot = snapshot_from_counts(counts, 2_000);

    assert_eq!(snapshot.used_bytes, 600);
    assert_eq!(snapshot.used_percent, 60);
    assert_eq!(snapshot.swap_used_bytes, 2_000);
}
