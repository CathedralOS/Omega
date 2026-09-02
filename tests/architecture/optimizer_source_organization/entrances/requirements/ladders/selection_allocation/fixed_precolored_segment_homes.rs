use super::SemanticLadder;

pub(super) const PRODUCTION: SemanticLadder = SemanticLadder {
    family: "fixed/precolored segmented homes",
    paths: &[
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/fixed_precolored_segment_homes/mod.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/fixed_precolored_segment_homes/model.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/fixed_precolored_segment_homes/error.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/fixed_precolored_segment_homes/identity.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/fixed_precolored_segment_homes/validation.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/fixed_precolored_segment_homes/compute/mod.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/fixed_precolored_segment_homes/compute/roots.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/fixed_precolored_segment_homes/compute/functions.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/fixed_precolored_segment_homes/compute/domains.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/fixed_precolored_segment_homes/compute/conflicts.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/fixed_precolored_segment_homes/compute/placement.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/fixed_precolored_segment_homes/compute/work.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/fixed_precolored_segment_homes/replay/mod.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/fixed_precolored_segment_homes/replay/indexes.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/fixed_precolored_segment_homes/replay/domains.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/fixed_precolored_segment_homes/replay/conflicts.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/fixed_precolored_segment_homes/replay/placement.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/fixed_precolored_segment_homes/replay/work.rs",
    ],
};

pub(super) const COVERAGE: SemanticLadder = SemanticLadder {
    family: "fixed/precolored segmented-home coverage",
    paths: &[
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/allocation/register_allocation/fixed_precolored_segment_homes/mod.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/allocation/register_allocation/fixed_precolored_segment_homes/fixture.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/allocation/register_allocation/fixed_precolored_segment_homes/positive.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/allocation/register_allocation/fixed_precolored_segment_homes/corruption.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/allocation/register_allocation/fixed_precolored_segment_homes/budget.rs",
    ],
};
