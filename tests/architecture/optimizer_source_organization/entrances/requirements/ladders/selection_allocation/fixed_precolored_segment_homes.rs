use super::SemanticLadder;

pub(super) const PRODUCTION: SemanticLadder = SemanticLadder {
    family: "fixed/precolored segmented homes",
    paths: &[
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/analyses/fixed_precolored_segment_homes/mod.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/analyses/fixed_precolored_segment_homes/model.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/analyses/fixed_precolored_segment_homes/error.rs",
        "omega-rust/omega/representations/register-homes/src/register_homes/storage/fixed_precolored_segment_homes.rs",
        "omega-rust/omega/representations/register-homes/src/register_homes/storage/fixed_precolored_segment_homes/identity.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/analyses/fixed_precolored_segment_homes/validation.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/analyses/fixed_precolored_segment_homes/compute/mod.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/analyses/fixed_precolored_segment_homes/compute/roots.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/analyses/fixed_precolored_segment_homes/compute/functions.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/analyses/fixed_precolored_segment_homes/compute/domains.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/analyses/fixed_precolored_segment_homes/compute/conflicts.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/analyses/fixed_precolored_segment_homes/compute/placement.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/analyses/fixed_precolored_segment_homes/compute/work.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/analyses/fixed_precolored_segment_homes/replay/mod.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/analyses/fixed_precolored_segment_homes/replay/indexes.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/analyses/fixed_precolored_segment_homes/replay/domains.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/analyses/fixed_precolored_segment_homes/replay/conflicts.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/analyses/fixed_precolored_segment_homes/replay/placement.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/analyses/fixed_precolored_segment_homes/replay/work.rs",
    ],
};

pub(super) const COVERAGE: SemanticLadder = SemanticLadder {
    family: "fixed/precolored segmented-home coverage",
    paths: &[
        "tests/native-differential/tests/pipeline_ownership/stages/allocation/register_allocation/fixed_precolored_segment_homes/mod.rs",
        "tests/native-differential/tests/pipeline_ownership/stages/allocation/register_allocation/fixed_precolored_segment_homes/fixture.rs",
        "tests/native-differential/tests/pipeline_ownership/stages/allocation/register_allocation/fixed_precolored_segment_homes/positive.rs",
        "tests/native-differential/tests/pipeline_ownership/stages/allocation/register_allocation/fixed_precolored_segment_homes/corruption.rs",
        "tests/native-differential/tests/pipeline_ownership/stages/allocation/register_allocation/fixed_precolored_segment_homes/budget.rs",
    ],
};
