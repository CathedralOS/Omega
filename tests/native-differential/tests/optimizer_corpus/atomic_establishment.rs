use sha2::{Digest, Sha256};

use super::generator::{GENERATOR, SEED, next};

pub(super) const FORMAT: &str = "omega.optimizer-corpus.atomic-establishment.v1";
pub(super) const CASE_COUNT: usize = 64;
pub(super) const MAX_SUM_CASES: u8 = 4;
pub(super) const MAX_ARRAY_ELEMENTS: u8 = 3;

/// Atomic aggregate establishment leaf. Each conditional arm atomically
/// establishes one case of an unrestricted sum (`EstablishScalarCase`), then
/// observes its discriminator through `StructuralCaseMembership` for the
/// machine's Boolean result, and finally atomically establishes an unobserved
/// unrestricted fixed array (`EstablishScalarArray`) so a second atomic
/// establishment op must survive every lowering stage. The declared case at
/// index `i` carries `i % 3` u64 payload fields, so the corpus exercises both
/// payloadless and scalar-payload establishment. The per-arm `established`
/// selector lets the membership answer differ between arms; the reference
/// interpreter, both selected-machine replays, and the host-native oracle must
/// agree with each arm's exact answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AtomicCase {
    pub(super) ordinal: usize,
    pub(super) case_count: u8,
    pub(super) established_true: u8,
    pub(super) established_false: u8,
    pub(super) queried: u8,
    pub(super) payload: u64,
    pub(super) array_elements: u8,
}

impl AtomicCase {
    /// u64 payload field count declared on the case at `index`.
    pub(super) fn field_count(index: u8) -> u8 {
        index % 3
    }

    pub(super) fn expected_true(&self) -> bool {
        self.established_true == self.queried
    }

    pub(super) fn expected_false(&self) -> bool {
        self.established_false == self.queried
    }
}

pub(super) fn cases() -> Vec<AtomicCase> {
    let mut state = SEED;
    let cases = (0..CASE_COUNT)
        .map(|ordinal| {
            // Low LCG bits cycle with period 2^k, so selectors read high bits.
            let case_count = 2 + ((next(&mut state) >> 62) as u8 % (MAX_SUM_CASES - 1));
            let established_true = (next(&mut state) >> 61) as u8 % case_count;
            let established_false = (next(&mut state) >> 61) as u8 % case_count;
            let queried = (next(&mut state) >> 61) as u8 % case_count;
            let payload = next(&mut state);
            let array_elements = (next(&mut state) >> 62) as u8 % (MAX_ARRAY_ELEMENTS + 1);
            AtomicCase {
                ordinal,
                case_count,
                established_true,
                established_false,
                queried,
                payload,
                array_elements,
            }
        })
        .collect::<Vec<_>>();
    assert_coverage(&cases);
    cases
}

pub(super) fn validate_manifest(cases: &[AtomicCase]) {
    assert_eq!(cases.len(), CASE_COUNT);
    let rendered = format!(
        "format={FORMAT}\ngenerator={GENERATOR}\nseed={SEED:#018x}\ncase_count={CASE_COUNT}\npsi_shape=boolean_conditional_arms_establish_scalar_case_then_membership_query_then_unobserved_scalar_array\nhost_native_shape=boolean_conditional_atomic_establishment_arms_same_artifact\nhost_native_oracle=terminal_interpreter_equals_postallocation_optimized_native_bool_per_arm\noperand_lane=unrestricted_sum_scalar_case_establishment_membership_observation_with_unrestricted_fixed_array_establishment\nrecords_sha256={}\n",
        records_digest(cases),
    );
    let checked_in = include_str!("../../corpora/optimizer/v2/atomic_establishment_lanes.txt");
    assert_eq!(
        checked_in, rendered,
        "atomic establishment optimizer corpus manifest drifted"
    );
}

fn records_digest(cases: &[AtomicCase]) -> String {
    let mut hasher = Sha256::new();
    for case in cases {
        hasher.update((case.ordinal as u64).to_le_bytes());
        hasher.update([
            case.case_count,
            case.established_true,
            case.established_false,
            case.queried,
            case.array_elements,
        ]);
        hasher.update(case.payload.to_le_bytes());
        hasher.update([case.expected_true() as u8, case.expected_false() as u8]);
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn assert_coverage(cases: &[AtomicCase]) {
    assert!(cases.iter().any(|case| case.case_count == 2));
    assert!(cases.iter().any(|case| case.case_count == MAX_SUM_CASES));
    // Both membership answers must be witnessed on both arms.
    assert!(cases.iter().any(|case| case.expected_true()));
    assert!(cases.iter().any(|case| !case.expected_true()));
    assert!(cases.iter().any(|case| case.expected_false()));
    assert!(cases.iter().any(|case| !case.expected_false()));
    assert!(
        cases
            .iter()
            .any(|case| case.expected_true() != case.expected_false())
    );
    // Payloadless and multi-field establishment must both be witnessed.
    assert!(
        cases
            .iter()
            .any(|case| AtomicCase::field_count(case.established_true) == 0)
    );
    assert!(
        cases
            .iter()
            .any(|case| AtomicCase::field_count(case.established_false) == 0)
    );
    assert!(
        cases
            .iter()
            .any(|case| AtomicCase::field_count(case.established_true) == 2)
    );
    assert!(
        cases
            .iter()
            .any(|case| AtomicCase::field_count(case.established_false) == 2)
    );
    assert!(
        cases
            .iter()
            .any(|case| case.established_true != case.established_false)
    );
    // Every queryable case position must be queried at least once.
    for queried in 0..MAX_SUM_CASES {
        assert!(cases.iter().any(|case| case.queried == queried));
    }
    // The unobserved array establishment must visit empty and full rosters.
    assert!(cases.iter().any(|case| case.array_elements == 0));
    assert!(
        cases
            .iter()
            .any(|case| case.array_elements == MAX_ARRAY_ELEMENTS)
    );
}
