use super::SemanticLadder;

pub(super) const PRODUCTION: SemanticLadder = SemanticLadder {
    family: "fixed/precolored split requirements",
    paths: &[
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/fixed_precolored_split_requirements/mod.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/fixed_precolored_split_requirements/model.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/fixed_precolored_split_requirements/error.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/fixed_precolored_split_requirements/identity.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/fixed_precolored_split_requirements/validation.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/fixed_precolored_split_requirements/compute/mod.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/fixed_precolored_split_requirements/compute/function.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/fixed_precolored_split_requirements/compute/topology.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/fixed_precolored_split_requirements/compute/cuts.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/fixed_precolored_split_requirements/compute/partition.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/fixed_precolored_split_requirements/compute/work.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/fixed_precolored_split_requirements/replay/mod.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/fixed_precolored_split_requirements/replay/indexes.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/fixed_precolored_split_requirements/replay/function.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/fixed_precolored_split_requirements/replay/topology.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/fixed_precolored_split_requirements/replay/cuts.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/fixed_precolored_split_requirements/replay/partition.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/fixed_precolored_split_requirements/replay/work.rs",
    ],
};

pub(super) const COVERAGE: SemanticLadder = SemanticLadder {
    family: "fixed/precolored split requirement coverage",
    paths: &[
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/allocation/register_allocation/fixed_precolored_split_requirements/mod.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/allocation/register_allocation/fixed_precolored_split_requirements/fixture.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/allocation/register_allocation/fixed_precolored_split_requirements/positive.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/allocation/register_allocation/fixed_precolored_split_requirements/corruption.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/allocation/register_allocation/fixed_precolored_split_requirements/budget.rs",
    ],
};
