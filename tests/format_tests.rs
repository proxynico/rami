use rami::format::{gb_pair, gb_text, mem_text};

const ONE_GIB: u64 = 1_073_741_824;
const SIXTEEN_GIB: u64 = 16 * ONE_GIB;

#[test]
fn gb_text_rounds_to_one_decimal_place() {
    assert_eq!(gb_text(9 * ONE_GIB), "9.0 GB");
}

#[test]
fn mem_text_at_gib_boundary_uses_mb_then_gb() {
    assert_eq!(mem_text(ONE_GIB - 1), "1024 MB");
    assert_eq!(mem_text(ONE_GIB), "1.0 GB");
}

#[test]
fn gb_pair_renders_used_over_total() {
    assert_eq!(gb_pair(6_120_328_397, SIXTEEN_GIB), "5.7 / 16.0 GB");
}
