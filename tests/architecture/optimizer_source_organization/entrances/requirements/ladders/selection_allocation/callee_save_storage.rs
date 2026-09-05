use super::SemanticLadder;

pub(super) const PRODUCTION: SemanticLadder = SemanticLadder {
    family: "non-authoritative callee-save storage planning",
    paths: &[
        "omega-rust/omega/pipeline/omega-callee-saved-requirements-to-save-storage/src/callee_save_storage/mod.rs",
        "omega-rust/omega/pipeline/omega-callee-saved-requirements-to-save-storage/src/callee_save_storage/model.rs",
        "omega-rust/omega/pipeline/omega-callee-saved-requirements-to-save-storage/src/callee_save_storage/error.rs",
        "omega-rust/omega/pipeline/omega-callee-saved-requirements-to-save-storage/src/callee_save_storage/identity.rs",
        "omega-rust/omega/pipeline/omega-callee-saved-requirements-to-save-storage/src/callee_save_storage/custody.rs",
        "omega-rust/omega/pipeline/omega-callee-saved-requirements-to-save-storage/src/callee_save_storage/validation.rs",
        "omega-rust/omega/pipeline/omega-callee-saved-requirements-to-save-storage/src/callee_save_storage/compute/mod.rs",
        "omega-rust/omega/pipeline/omega-callee-saved-requirements-to-save-storage/src/callee_save_storage/compute/groups.rs",
        "omega-rust/omega/pipeline/omega-callee-saved-requirements-to-save-storage/src/callee_save_storage/compute/work.rs",
        "omega-rust/omega/pipeline/omega-callee-saved-requirements-to-save-storage/src/callee_save_storage/replay/mod.rs",
        "omega-rust/omega/pipeline/omega-callee-saved-requirements-to-save-storage/src/callee_save_storage/replay/groups.rs",
        "omega-rust/omega/pipeline/omega-callee-saved-requirements-to-save-storage/src/callee_save_storage/replay/work.rs",
    ],
};

pub(super) const COVERAGE: SemanticLadder = SemanticLadder {
    family: "non-authoritative callee-save storage coverage",
    paths: &[
        "tests/native-differential/tests/pipeline_ownership/stages/allocation/register_allocation/callee_save_storage/mod.rs",
        "tests/native-differential/tests/pipeline_ownership/stages/allocation/register_allocation/callee_save_storage/fixture.rs",
        "tests/native-differential/tests/pipeline_ownership/stages/allocation/register_allocation/callee_save_storage/positive.rs",
        "tests/native-differential/tests/pipeline_ownership/stages/allocation/register_allocation/callee_save_storage/corruption.rs",
        "tests/native-differential/tests/pipeline_ownership/stages/allocation/register_allocation/callee_save_storage/budget.rs",
    ],
};
