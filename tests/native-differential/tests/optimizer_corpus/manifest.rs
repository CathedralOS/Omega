use sha2::{Digest, Sha256};

use super::generator::{CorpusCase, CASE_COUNT, FORMAT, GENERATOR, SEED};

const CHECKED_IN: &str =
    include_str!("../../corpora/optimizer/v2/conditional_u64_optimizer_lanes.txt");

pub(super) fn validate(cases: &[CorpusCase]) {
    assert_eq!(cases.len(), CASE_COUNT);
    let rendered = format!(
        "format={FORMAT}\ngenerator={GENERATOR}\nseed={SEED:#018x}\ncase_count={CASE_COUNT}\npsi_shape=boolean_conditional_identical_wrapping_add_leaves\nselected_machine_shape=boolean_conditional_identical_immediate_leaves\nhost_native_shape=boolean_conditional_identical_immediate_leaves_same_artifact\nhost_native_oracle=terminal_interpreter_equals_postallocation_optimized_native_u64\nx86_lane=nonzero_zero_extended_u32\naarch64_lane=single_complemented_u16_chunk\nrecords_sha256={}\n",
        records_digest(cases),
    );
    assert_eq!(CHECKED_IN, rendered, "optimizer corpus manifest drifted");
}

fn records_digest(cases: &[CorpusCase]) -> String {
    let mut hasher = Sha256::new();
    for case in cases {
        hasher.update((case.ordinal as u64).to_le_bytes());
        for lane in [case.x86, case.aarch64] {
            hasher.update(lane.left.to_le_bytes());
            hasher.update(lane.right.to_le_bytes());
            hasher.update(lane.expected.to_le_bytes());
        }
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
