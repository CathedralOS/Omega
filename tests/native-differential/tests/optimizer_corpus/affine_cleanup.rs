use sha2::{Digest, Sha256};

use super::generator::{GENERATOR, SEED, next};

pub(super) const FORMAT: &str = "omega.optimizer-corpus.affine-cleanup.v1";
pub(super) const CASE_COUNT: usize = 64;
pub(super) const MAX_CLEANUPS_PER_ARM: u8 = 3;

/// Scalar-return leaf carrying an exact affine cleanup schedule. Each arm
/// establishes `*_cleanups` claim-free empty records and returns the same
/// saturating u64 sum, so the observation stays scalar while the verifier,
/// the interpreter, and every lowering stage must preserve the exact
/// reverse-producer `DiscardRoot` order on the selected return edge. The
/// scalar fold uses `SaturatingIntegerAdd` because the admitted native
/// instruction set does not legalize `WrappingIntegerAdd` results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CleanupCase {
    pub(super) ordinal: usize,
    pub(super) true_cleanups: u8,
    pub(super) false_cleanups: u8,
    pub(super) left: u64,
    pub(super) right: u64,
    pub(super) expected: u64,
}

pub(super) fn cases() -> Vec<CleanupCase> {
    let mut state = SEED;
    let cases = (0..CASE_COUNT)
        .map(|ordinal| {
            // Low LCG bits cycle with period 2^k, so count selectors read the
            // two high bits (exactly 0..=MAX_CLEANUPS_PER_ARM).
            let true_cleanups = (next(&mut state) >> 62) as u8;
            let false_cleanups = (next(&mut state) >> 62) as u8;
            let left = next(&mut state);
            let right = next(&mut state);
            CleanupCase {
                ordinal,
                true_cleanups,
                false_cleanups,
                left,
                right,
                expected: left.saturating_add(right),
            }
        })
        .collect::<Vec<_>>();
    assert_coverage(&cases);
    cases
}

pub(super) fn validate_manifest(cases: &[CleanupCase]) {
    assert_eq!(cases.len(), CASE_COUNT);
    let rendered = format!(
        "format={FORMAT}\ngenerator={GENERATOR}\nseed={SEED:#018x}\ncase_count={CASE_COUNT}\npsi_shape=boolean_conditional_saturating_add_leaves_affine_cleanup_returns\nhost_native_shape=boolean_conditional_saturating_add_leaves_affine_cleanup_returns_same_artifact\nhost_native_oracle=terminal_interpreter_equals_postallocation_optimized_native_u64\noperand_lane=empty_record_affine_results_discarded_in_reverse_producer_order\nrecords_sha256={}\n",
        records_digest(cases),
    );
    let checked_in =
        include_str!("../../corpora/optimizer/v2/conditional_affine_cleanup_lanes.txt");
    assert_eq!(
        checked_in, rendered,
        "affine cleanup optimizer corpus manifest drifted"
    );
}

fn records_digest(cases: &[CleanupCase]) -> String {
    let mut hasher = Sha256::new();
    for case in cases {
        hasher.update((case.ordinal as u64).to_le_bytes());
        hasher.update([case.true_cleanups, case.false_cleanups]);
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

fn assert_coverage(cases: &[CleanupCase]) {
    assert!(
        cases
            .iter()
            .any(|case| case.true_cleanups == 0 && case.false_cleanups == 0)
    );
    assert!(
        cases
            .iter()
            .any(|case| case.true_cleanups == MAX_CLEANUPS_PER_ARM)
    );
    assert!(
        cases
            .iter()
            .any(|case| case.false_cleanups == MAX_CLEANUPS_PER_ARM)
    );
    assert!(
        cases
            .iter()
            .any(|case| { case.true_cleanups >= 2 && case.false_cleanups >= 2 })
    );
    assert!(
        cases
            .iter()
            .any(|case| case.true_cleanups != case.false_cleanups)
    );
    assert!(
        cases
            .iter()
            .any(|case| case.true_cleanups == 0 && case.false_cleanups > 0)
    );
    assert!(
        cases
            .iter()
            .any(|case| case.false_cleanups == 0 && case.true_cleanups > 0)
    );
}
