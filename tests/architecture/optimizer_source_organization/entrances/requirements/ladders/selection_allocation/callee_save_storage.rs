use super::SemanticLadder;

pub(super) const PRODUCTION: SemanticLadder = SemanticLadder {
    family: "non-authoritative callee-save storage planning",
    paths: &[
        "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/allocation/callee_save_storage/mod.rs",
        "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/allocation/callee_save_storage/model.rs",
        "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/allocation/callee_save_storage/error.rs",
        "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/allocation/callee_save_storage/identity.rs",
        "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/allocation/callee_save_storage/custody.rs",
        "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/allocation/callee_save_storage/validation.rs",
        "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/allocation/callee_save_storage/compute/mod.rs",
        "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/allocation/callee_save_storage/compute/groups.rs",
        "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/allocation/callee_save_storage/compute/work.rs",
        "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/allocation/callee_save_storage/replay/mod.rs",
        "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/allocation/callee_save_storage/replay/groups.rs",
        "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/allocation/callee_save_storage/replay/work.rs",
    ],
};

pub(super) const COVERAGE: SemanticLadder = SemanticLadder {
    family: "non-authoritative callee-save storage coverage",
    paths: &[
        "omega-rust/omega/pipeline/omega-optimization-pipeline/src/tests/stages/allocation/register_allocation/callee_save_storage/mod.rs",
        "omega-rust/omega/pipeline/omega-optimization-pipeline/src/tests/stages/allocation/register_allocation/callee_save_storage/fixture.rs",
        "omega-rust/omega/pipeline/omega-optimization-pipeline/src/tests/stages/allocation/register_allocation/callee_save_storage/positive.rs",
        "omega-rust/omega/pipeline/omega-optimization-pipeline/src/tests/stages/allocation/register_allocation/callee_save_storage/corruption.rs",
        "omega-rust/omega/pipeline/omega-optimization-pipeline/src/tests/stages/allocation/register_allocation/callee_save_storage/budget.rs",
    ],
};
