use semantic_vocabulary::IeeeFloatComparisonOperation;
use sha2::{Digest, Sha256};

use super::generator::{GENERATOR, SEED, next};

pub(super) const FORMAT: &str = "omega.optimizer-corpus.ieee-compare.v1";
pub(super) const CASE_COUNT: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CompareCase {
    pub(super) ordinal: usize,
    pub(super) comparison: IeeeFloatComparisonOperation,
    pub(super) left_bits: u64,
    pub(super) right_bits: u64,
    pub(super) expected: bool,
}

pub(super) fn cases() -> Vec<CompareCase> {
    let mut state = SEED;
    let cases = (0..CASE_COUNT)
        .map(|ordinal| {
            // Low LCG bits cycle with period 2^k, so kind selectors read the high word.
            let comparison = match (next(&mut state) >> 32) % 6 {
                0 => IeeeFloatComparisonOperation::Equal,
                1 => IeeeFloatComparisonOperation::NotEqual,
                2 => IeeeFloatComparisonOperation::Less,
                3 => IeeeFloatComparisonOperation::LessOrEqual,
                4 => IeeeFloatComparisonOperation::Greater,
                _ => IeeeFloatComparisonOperation::GreaterOrEqual,
            };
            let left_bits = operand(&mut state);
            let right_kind = (next(&mut state) >> 61) as u8;
            let right_bits = match right_kind {
                0..=3 => operand(&mut state),
                4 => left_bits,
                5 => left_bits ^ (1 << 63),
                6 => 0x7ff8_0000_0000_0000 | (next(&mut state) & 0x000f_ffff_ffff_ffff),
                7 => (left_bits ^ (1 << 63)) & (1 << 63),
                _ => unreachable!("right kind is selected from three high bits"),
            };
            let left = f64::from_bits(left_bits);
            let right = f64::from_bits(right_bits);
            let expected = match comparison {
                IeeeFloatComparisonOperation::Equal => left == right,
                IeeeFloatComparisonOperation::NotEqual => left != right,
                IeeeFloatComparisonOperation::Less => left < right,
                IeeeFloatComparisonOperation::LessOrEqual => left <= right,
                IeeeFloatComparisonOperation::Greater => left > right,
                IeeeFloatComparisonOperation::GreaterOrEqual => left >= right,
            };
            CompareCase {
                ordinal,
                comparison,
                left_bits,
                right_bits,
                expected,
            }
        })
        .collect::<Vec<_>>();
    assert_coverage(&cases);
    cases
}

fn operand(state: &mut u64) -> u64 {
    match next(state) >> 61 {
        0..=3 => next(state),
        4 => next(state) & (1 << 63),
        5 => 0x7ff0_0000_0000_0000 | (next(state) & (1 << 63)),
        6 => next(state) & 0x800f_ffff_ffff_ffff,
        _ => 0x7ff8_0000_0000_0000 | (next(state) & 0x800f_ffff_ffff_ffff),
    }
}

pub(super) fn validate_manifest(cases: &[CompareCase]) {
    assert_eq!(cases.len(), CASE_COUNT);
    let rendered = format!(
        "format={FORMAT}\ngenerator={GENERATOR}\nseed={SEED:#018x}\ncase_count={CASE_COUNT}\npsi_shape=boolean_conditional_identical_ieee_binary64_compare_leaves\nhost_native_shape=boolean_conditional_identical_ieee_binary64_compare_leaves_same_artifact\nhost_native_oracle=host_f64_equals_terminal_interpreter_equals_postallocation_optimized_native_bool\noperand_lane=raw_signed_zero_infinity_subnormal_nan_mixture\nrecords_sha256={}\n",
        records_digest(cases),
    );
    let checked_in =
        include_str!("../../corpora/optimizer/v2/conditional_ieee_binary64_compare_lanes.txt");
    assert_eq!(
        checked_in, rendered,
        "IEEE optimizer corpus manifest drifted"
    );
}

fn records_digest(cases: &[CompareCase]) -> String {
    let mut hasher = Sha256::new();
    for case in cases {
        hasher.update((case.ordinal as u64).to_le_bytes());
        hasher.update(comparison_code(case.comparison).to_le_bytes());
        hasher.update(case.left_bits.to_le_bytes());
        hasher.update(case.right_bits.to_le_bytes());
        hasher.update([case.expected as u8]);
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn comparison_code(comparison: IeeeFloatComparisonOperation) -> u8 {
    match comparison {
        IeeeFloatComparisonOperation::Equal => 0,
        IeeeFloatComparisonOperation::NotEqual => 1,
        IeeeFloatComparisonOperation::Less => 2,
        IeeeFloatComparisonOperation::LessOrEqual => 3,
        IeeeFloatComparisonOperation::Greater => 4,
        IeeeFloatComparisonOperation::GreaterOrEqual => 5,
    }
}

fn assert_coverage(cases: &[CompareCase]) {
    assert!(cases.iter().any(|case| {
        f64::from_bits(case.left_bits).is_nan() || f64::from_bits(case.right_bits).is_nan()
    }));
    assert!(cases.iter().any(|case| {
        let left_is_zero = case.left_bits & !(1 << 63) == 0;
        let right_is_zero = case.right_bits & !(1 << 63) == 0;
        left_is_zero && right_is_zero && (case.left_bits ^ case.right_bits) == (1 << 63)
    }));
    assert!(cases.iter().any(|case| {
        f64::from_bits(case.left_bits).is_infinite()
            || f64::from_bits(case.right_bits).is_infinite()
    }));
    assert!(cases.iter().any(|case| {
        f64::from_bits(case.left_bits) == 0.0 || f64::from_bits(case.right_bits) == 0.0
    }));
    for comparison in [
        IeeeFloatComparisonOperation::Equal,
        IeeeFloatComparisonOperation::NotEqual,
        IeeeFloatComparisonOperation::Less,
        IeeeFloatComparisonOperation::LessOrEqual,
        IeeeFloatComparisonOperation::Greater,
        IeeeFloatComparisonOperation::GreaterOrEqual,
    ] {
        assert!(cases.iter().any(|case| case.comparison == comparison));
    }
    assert!(cases.iter().any(|case| case.expected));
    assert!(cases.iter().any(|case| !case.expected));
}
