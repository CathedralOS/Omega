use super::SemanticLadder;

pub(super) const PRODUCTION: SemanticLadder = SemanticLadder {
    family: "fixed/precolored split requirements",
    paths: &[
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/analyses/fixed_precolored_split_requirements/mod.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/analyses/fixed_precolored_split_requirements/model.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/analyses/fixed_precolored_split_requirements/error.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/analyses/fixed_precolored_split_requirements/identity.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/analyses/fixed_precolored_split_requirements/validation.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/analyses/fixed_precolored_split_requirements/compute/mod.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/analyses/fixed_precolored_split_requirements/compute/function.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/analyses/fixed_precolored_split_requirements/compute/topology.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/analyses/fixed_precolored_split_requirements/compute/cuts.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/analyses/fixed_precolored_split_requirements/compute/partition.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/analyses/fixed_precolored_split_requirements/compute/work.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/analyses/fixed_precolored_split_requirements/replay/mod.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/analyses/fixed_precolored_split_requirements/replay/indexes.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/analyses/fixed_precolored_split_requirements/replay/function.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/analyses/fixed_precolored_split_requirements/replay/topology.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/analyses/fixed_precolored_split_requirements/replay/cuts.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/analyses/fixed_precolored_split_requirements/replay/partition.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/analyses/fixed_precolored_split_requirements/replay/work.rs",
    ],
};

pub(super) const COVERAGE: SemanticLadder = SemanticLadder {
    family: "fixed/precolored split requirement coverage",
    paths: &[
        "tests/native-differential/tests/pipeline_ownership/stages/allocation/register_allocation/fixed_precolored_split_requirements/mod.rs",
        "tests/native-differential/tests/pipeline_ownership/stages/allocation/register_allocation/fixed_precolored_split_requirements/fixture.rs",
        "tests/native-differential/tests/pipeline_ownership/stages/allocation/register_allocation/fixed_precolored_split_requirements/positive.rs",
        "tests/native-differential/tests/pipeline_ownership/stages/allocation/register_allocation/fixed_precolored_split_requirements/corruption.rs",
        "tests/native-differential/tests/pipeline_ownership/stages/allocation/register_allocation/fixed_precolored_split_requirements/budget.rs",
    ],
};
