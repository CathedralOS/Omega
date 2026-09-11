use super::super::RequiredCoordinationEntrance;

pub(crate) const ENTRANCES: &[RequiredCoordinationEntrance] = &[
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/src/artifact/mod.rs",
        coordination_marker: "pub fn lower_artifact_sections",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/src/optimization/mod.rs",
        coordination_marker: "pub fn build_verified_psi_optimization_unit",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/src/provider_installation/mod.rs",
        coordination_marker: "pub fn admit_provider_installation",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/src/lowering/mod.rs",
        coordination_marker: "pub(crate) fn lower_decoded_verified_module",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/src/lowering/machine.rs",
        coordination_marker: "pub(super) fn lower_machine",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/src/lowering/machine/operation/mod.rs",
        coordination_marker: "pub(super) fn lower_operation",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/publication/mod.rs",
        coordination_marker: "pub fn publish_optimization_run",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/publication/replay/mod.rs",
        coordination_marker: "pub(super) fn validate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/publication/replay/candidate_decisions/mod.rs",
        coordination_marker: "manifests::validate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/publication/replay/candidate_decisions/mod.rs",
        coordination_marker: "declarations::validate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/publication/replay/candidate_decisions/mod.rs",
        coordination_marker: "baseline::validate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/publication/source/mod.rs",
        coordination_marker: "pub(super) fn project_plan",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-target-operations/src/lowering/mod.rs",
        coordination_marker: "pub fn lower_to_target_operations_with_provider_executions_and_installation",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-target-operations/src/validation/whole_plan.rs",
        coordination_marker: "pub fn validate_abstract_to_target_translation",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-target-operations/src/lowering/coordination/projected_qualifications/mod.rs",
        coordination_marker: "pub(super) fn reject_unsupported",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-target-operations/src/validation/model/error/mod.rs",
        coordination_marker: "AbstractToTargetTranslationValidationError",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-target-operations/src/validation/model/receipt/mod.rs",
        coordination_marker: "AbstractToTargetFunctionRosterReceipt",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-target-operations/src/lowering/function/mod.rs",
        coordination_marker: "pub(super) fn lower_function",
    },
];
