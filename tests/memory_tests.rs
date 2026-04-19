use rami::memory::{
    pressure_from_raw, snapshot_from_counts, validate_stats_count, validate_sysctl_size,
    MemoryCounts, VM_PRESSURE_CRITICAL, VM_PRESSURE_NORMAL, VM_PRESSURE_WARN,
};
use rami::model::MemoryPressure;

#[test]
fn snapshot_uses_active_wired_and_compressed_bytes() {
    let counts = MemoryCounts {
        total_bytes: 1000,
        page_size: 10,
        active_pages: 30,
        wired_pages: 20,
        compressed_pages: 10,
    };

    let snapshot = snapshot_from_counts(counts, MemoryPressure::Normal, 0);

    assert_eq!(snapshot.used_bytes, 600);
    assert_eq!(snapshot.total_bytes, 1000);
    assert_eq!(snapshot.used_percent, 60);
    assert_eq!(snapshot.pressure, MemoryPressure::Normal);
    assert_eq!(snapshot.swap_used_bytes, 0);
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

    let snapshot = snapshot_from_counts(counts, MemoryPressure::Normal, 0);

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

    let snapshot = snapshot_from_counts(counts, MemoryPressure::Normal, 0);

    assert_eq!(snapshot.used_bytes, 130);
    assert_eq!(snapshot.used_percent, 100);
    assert_eq!(snapshot.pressure, MemoryPressure::Normal);
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
    };

    let snapshot = snapshot_from_counts(counts, MemoryPressure::Normal, 0);

    assert_eq!(snapshot.used_bytes, 130);
    assert_eq!(snapshot.total_bytes, 0);
    assert_eq!(snapshot.used_percent, 0);
    assert_eq!(snapshot.pressure, MemoryPressure::Normal);
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
fn snapshot_carries_pressure_and_swap_usage() {
    let counts = MemoryCounts {
        total_bytes: 1000,
        page_size: 10,
        active_pages: 30,
        wired_pages: 20,
        compressed_pages: 10,
    };

    let snapshot = snapshot_from_counts(counts, MemoryPressure::High, 2_000);

    assert_eq!(snapshot.used_bytes, 600);
    assert_eq!(snapshot.used_percent, 60);
    assert_eq!(snapshot.pressure, MemoryPressure::High);
    assert_eq!(snapshot.swap_used_bytes, 2_000);
}

#[test]
fn pressure_from_raw_maps_dispatch_compatible_levels() {
    assert_eq!(
        pressure_from_raw(VM_PRESSURE_NORMAL),
        MemoryPressure::Normal
    );
    assert_eq!(
        pressure_from_raw(VM_PRESSURE_WARN),
        MemoryPressure::Elevated
    );
    assert_eq!(
        pressure_from_raw(VM_PRESSURE_CRITICAL),
        MemoryPressure::High
    );
    assert_eq!(
        pressure_from_raw(VM_PRESSURE_WARN | VM_PRESSURE_CRITICAL),
        MemoryPressure::High
    );
}

#[test]
fn validate_sysctl_size_rejects_mismatched_byte_count() {
    let error =
        validate_sysctl_size(4, 8, "vm.swapusage").expect_err("size mismatch should be rejected");

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    assert!(error.to_string().contains("vm.swapusage"));
    assert!(error.to_string().contains("expected 8 bytes"));
    assert!(error.to_string().contains("got 4"));
}
