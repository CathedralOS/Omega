use super::SemanticLadder;

pub(super) const PRODUCTION: SemanticLadder = SemanticLadder {
    family: "non-authoritative callee-save storage planning",
    paths: &[
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/allocation/callee_save_storage/mod.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/allocation/callee_save_storage/model.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/allocation/callee_save_storage/error.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/allocation/callee_save_storage/identity.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/allocation/callee_save_storage/custody.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/allocation/callee_save_storage/validation.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/allocation/callee_save_storage/compute/mod.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/allocation/callee_save_storage/compute/groups.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/allocation/callee_save_storage/compute/work.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/allocation/callee_save_storage/replay/mod.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/allocation/callee_save_storage/replay/groups.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/allocation/callee_save_storage/replay/work.rs",
    ],
};

pub(super) const COVERAGE: SemanticLadder = SemanticLadder {
    family: "non-authoritative callee-save storage coverage",
    paths: &[
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/allocation/register_allocation/callee_save_storage/mod.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/allocation/register_allocation/callee_save_storage/fixture.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/allocation/register_allocation/callee_save_storage/positive.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/allocation/register_allocation/callee_save_storage/corruption.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/allocation/register_allocation/callee_save_storage/budget.rs",
    ],
};
