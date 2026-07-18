use rami::memory::{snapshot_from_counts, validate_stats_count, MemoryCounts};
use rami::model::PressureSource;

#[test]
fn snapshot_uses_active_wired_and_compressed_bytes() {
    let counts = MemoryCounts {
        total_bytes: 1000,
        page_size: 10,
        active_pages: 30,
        internal_pages: 25,
        wired_pages: 20,
        compressed_pages: 10,
        free_pages: 0,
        inactive_pages: 0,
        speculative_pages: 0,
        purgeable_pages: 0,
    };

    let snapshot = snapshot_from_counts(counts, 0, None);

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
        internal_pages: 25,
        wired_pages: 20,
        compressed_pages: 10,
        free_pages: 5,
        inactive_pages: 4,
        speculative_pages: 1,
        purgeable_pages: 2,
    };

    let snapshot = snapshot_from_counts(counts, 0, None);

    // (5 + 4 + 2) pages * 10 byte pages = 110 bytes reclaimable.
    //
    // Speculative pages are excluded: host_statistics64 reports free_count as raw
    // free plus speculative, so adding speculative_count again would double-count
    // them and overstate Available.
    assert_eq!(snapshot.available_bytes, 110);
}

#[test]
fn snapshot_rounds_to_nearest_whole_percent() {
    let counts = MemoryCounts {
        total_bytes: 1000,
        page_size: 1,
        active_pages: 524,
        internal_pages: 524,
        wired_pages: 0,
        compressed_pages: 0,
        free_pages: 0,
        inactive_pages: 0,
        speculative_pages: 0,
        purgeable_pages: 0,
    };

    let snapshot = snapshot_from_counts(counts, 0, None);

    assert_eq!(snapshot.used_percent, 52);
}

#[test]
fn snapshot_clamps_when_used_exceeds_total() {
    let counts = MemoryCounts {
        total_bytes: 100,
        page_size: 10,
        active_pages: 8,
        internal_pages: 8,
        wired_pages: 3,
        compressed_pages: 2,
        free_pages: 0,
        inactive_pages: 0,
        speculative_pages: 0,
        purgeable_pages: 0,
    };

    let snapshot = snapshot_from_counts(counts, 0, None);

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
        internal_pages: 8,
        wired_pages: 3,
        compressed_pages: 2,
        free_pages: 0,
        inactive_pages: 0,
        speculative_pages: 0,
        purgeable_pages: 0,
    };

    let snapshot = snapshot_from_counts(counts, 0, None);

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
        internal_pages: 25,
        wired_pages: 20,
        compressed_pages: 10,
        free_pages: 0,
        inactive_pages: 0,
        speculative_pages: 0,
        purgeable_pages: 0,
    };

    let snapshot = snapshot_from_counts(counts, 2_000, None);

    assert_eq!(snapshot.used_bytes, 600);
    assert_eq!(snapshot.used_percent, 60);
    assert_eq!(snapshot.swap_used_bytes, 2_000);
}

#[test]
fn snapshot_exposes_breakdown_and_kernel_pressure() {
    let counts = MemoryCounts {
        total_bytes: 1_000,
        page_size: 10,
        active_pages: 30,
        internal_pages: 25,
        wired_pages: 20,
        compressed_pages: 10,
        free_pages: 5,
        inactive_pages: 4,
        speculative_pages: 1,
        purgeable_pages: 2,
    };

    let snapshot = snapshot_from_counts(counts, 0, Some(42));

    assert_eq!(snapshot.app_memory_bytes, 230);
    assert_eq!(snapshot.wired_bytes, 200);
    assert_eq!(snapshot.compressed_bytes, 100);
    // free_count bundles speculative pages in, so the Free row strips them:
    // (5 - 1) pages * 10 = 40.
    assert_eq!(snapshot.free_bytes, 40);
    assert_eq!(snapshot.pressure_percent, 58);
    assert_eq!(snapshot.pressure_source, PressureSource::Kernel);
}

/// Pins the Darwin `free_count` semantics this arithmetic depends on, using real
/// values captured from `host_statistics64` on Darwin 25:
///
/// ```text
/// free_count        = 7368
/// speculative_count = 2838
/// vm.page_free_count = 4530     <- 7368 - 2838, exactly
/// ```
///
/// `free_count` bundles speculative pages in. If a future macOS changes that, the
/// Free row and Available both silently drift, so the relationship is asserted
/// here rather than left as a comment.
#[test]
fn free_row_strips_the_speculative_pages_that_free_count_bundles_in() {
    let counts = MemoryCounts {
        total_bytes: 16 * 1024 * 1024 * 1024,
        page_size: 16_384,
        active_pages: 267_651,
        internal_pages: 200_000,
        wired_pages: 100_000,
        compressed_pages: 50_000,
        free_pages: 7_368,
        inactive_pages: 252_619,
        speculative_pages: 2_838,
        purgeable_pages: 10_000,
    };

    let snapshot = snapshot_from_counts(counts, 0, None);

    // Free shows the 4_530 genuinely free pages, not the 7_368 free_count reports.
    assert_eq!(snapshot.free_bytes, 4_530 * 16_384);

    // Available counts free (which already includes speculative) + inactive +
    // purgeable. Adding speculative again would overstate it by 2_838 pages.
    assert_eq!(
        snapshot.available_bytes,
        (7_368 + 252_619 + 10_000) * 16_384
    );
}

#[test]
fn snapshot_falls_back_to_available_share_when_kernel_pressure_is_unavailable() {
    let counts = MemoryCounts {
        total_bytes: 1_000,
        page_size: 10,
        active_pages: 30,
        internal_pages: 25,
        wired_pages: 20,
        compressed_pages: 10,
        free_pages: 5,
        inactive_pages: 4,
        speculative_pages: 1,
        purgeable_pages: 2,
    };

    let snapshot = snapshot_from_counts(counts, 0, None);

    // (5 + 4 + 2) pages * 10 = 110 available, so 11% of 1_000 total.
    assert_eq!(snapshot.available_bytes, 110);
    assert_eq!(snapshot.pressure_percent, 89);
    assert_eq!(snapshot.pressure_source, PressureSource::AvailableFallback);
}

#[test]
fn forced_pressure_names_map_into_their_bands() {
    use rami::memory::parse_forced_pressure;
    use rami::model::{classify_pressure, MemoryPressure};

    let normal = parse_forced_pressure("normal").expect("normal is a valid state");
    let warning = parse_forced_pressure("warning").expect("warning is a valid state");
    let critical = parse_forced_pressure("critical").expect("critical is a valid state");

    // The point of the hook is the accent, so assert the classification rather
    // than the percent it happens to pick.
    assert_eq!(classify_pressure(normal), MemoryPressure::Normal);
    assert_eq!(classify_pressure(warning), MemoryPressure::Warning);
    assert_eq!(classify_pressure(critical), MemoryPressure::Critical);
}

#[test]
fn forced_pressure_is_case_and_whitespace_insensitive() {
    use rami::memory::parse_forced_pressure;

    assert_eq!(
        parse_forced_pressure("  CRITICAL \n"),
        parse_forced_pressure("critical")
    );
    assert_eq!(
        parse_forced_pressure("Warning"),
        parse_forced_pressure("warning")
    );
}

#[test]
fn forced_pressure_rejects_unknown_values() {
    use rami::memory::parse_forced_pressure;

    assert_eq!(parse_forced_pressure(""), None);
    assert_eq!(parse_forced_pressure("high"), None);
    // Numeric values are not accepted; the hook takes state names only.
    assert_eq!(parse_forced_pressure("95"), None);
}
