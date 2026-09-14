use sha2::{Digest, Sha256};

use super::generator::{GENERATOR, next};

/// Lane-local seed distinct from `generator::SEED` so each corpus lane draws
/// an independent stream under the same LCG.
pub(super) const SEED: u64 = 0x4f4d_4547_415f_504d;
pub(super) const FORMAT: &str = "omega.optimizer-corpus.placed-memory.v1";
pub(super) const CASE_COUNT: usize = 64;
pub(super) const MAX_STORES_PER_ARM: u8 = 3;

/// Scalar-return leaf observing placed memory on both conditional arms. Each
/// arm establishes an unrestricted u64 primitive local from `left`, rewrites
/// it `*_stores` times — every intermediate write deposits a distinct value
/// and the final write restores `left` — then reads it back through
/// `PrimitiveScalarRead`. The arm also establishes an unrestricted
/// single-field u64 record from `right` and observes the field through
/// `IntegerStructuralField`, the admitted established-view read. The returned
/// value is `left.saturating_add(right)` on both arms, so a dropped,
/// reordered, or invented store, load, or field view must diverge from the
/// reference interpreter before the native result can agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PlacedMemoryCase {
    pub(super) ordinal: usize,
    pub(super) true_stores: u8,
    pub(super) false_stores: u8,
    pub(super) left: u64,
    pub(super) right: u64,
    pub(super) expected: u64,
}

pub(super) fn cases() -> Vec<PlacedMemoryCase> {
    let mut state = SEED;
    let cases = (0..CASE_COUNT)
        .map(|ordinal| {
            // Low LCG bits cycle with period 2^k, so count selectors read the
            // two high bits (exactly 0..=MAX_STORES_PER_ARM) and the operand
            // boundary selectors read the three high bits.
            let true_stores = (next(&mut state) >> 62) as u8;
            let false_stores = (next(&mut state) >> 62) as u8;
            let left = placed_operand(&mut state);
            let right = placed_operand(&mut state);
            PlacedMemoryCase {
                ordinal,
                true_stores,
                false_stores,
                left,
                right,
                expected: left.saturating_add(right),
            }
        })
        .collect::<Vec<_>>();
    assert_coverage(&cases);
    cases
}

/// Seeded operand spread biased toward representability boundaries: full-range
/// values, exact extremes, near-maximum sums, and small values.
fn placed_operand(state: &mut u64) -> u64 {
    match next(state) >> 61 {
        0..=3 => next(state),
        4 => u64::MAX,
        5 => u64::MAX - (next(state) & 0xffff),
        6 => 0,
        _ => next(state) & 0xff,
    }
}

pub(super) fn validate_manifest(cases: &[PlacedMemoryCase]) {
    assert_eq!(cases.len(), CASE_COUNT);
    let rendered = format!(
        "format={FORMAT}\ngenerator={GENERATOR}\nseed={SEED:#018x}\ncase_count={CASE_COUNT}\npsi_shape=boolean_conditional_primitive_local_stores_read_and_record_field_view_returns\nhost_native_shape=boolean_conditional_primitive_local_stores_read_and_record_field_view_returns_same_artifact\nhost_native_oracle=terminal_interpreter_equals_postallocation_optimized_native_u64\noperand_lane=unrestricted_primitive_local_restoring_stores_and_single_field_record_view\nrecords_sha256={}\n",
        records_digest(cases),
    );
    let checked_in = include_str!("../../corpora/optimizer/v2/conditional_placed_memory_lanes.txt");
    assert_eq!(
        checked_in, rendered,
        "placed-memory optimizer corpus manifest drifted"
    );
}

fn records_digest(cases: &[PlacedMemoryCase]) -> String {
    let mut hasher = Sha256::new();
    for case in cases {
        hasher.update((case.ordinal as u64).to_le_bytes());
        hasher.update([case.true_stores, case.false_stores]);
        hasher.update(case.left.to_le_bytes());
        hasher.update(case.right.to_le_bytes());
        hasher.update(case.expected.to_le_bytes());
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn assert_coverage(cases: &[PlacedMemoryCase]) {
    assert!(
        cases
            .iter()
            .any(|case| case.true_stores == 0 && case.false_stores == 0)
    );
    assert!(
        cases
            .iter()
            .any(|case| case.true_stores == 1 || case.false_stores == 1)
    );
    assert!(
        cases
            .iter()
            .any(|case| case.true_stores == MAX_STORES_PER_ARM)
    );
    assert!(
        cases
            .iter()
            .any(|case| case.false_stores == MAX_STORES_PER_ARM)
    );
    assert!(
        cases
            .iter()
            .any(|case| { case.true_stores >= 2 && case.false_stores >= 2 })
    );
    assert!(
        cases
            .iter()
            .any(|case| case.true_stores != case.false_stores)
    );
    assert!(
        cases
            .iter()
            .any(|case| case.true_stores == 0 && case.false_stores > 0)
    );
    assert!(
        cases
            .iter()
            .any(|case| case.false_stores == 0 && case.true_stores > 0)
    );
    assert!(
        cases
            .iter()
            .any(|case| case.left.checked_add(case.right).is_none())
    );
    assert!(cases.iter().any(|case| case.expected == 0));
    assert!(cases.iter().any(|case| case.expected == u64::MAX));
}
