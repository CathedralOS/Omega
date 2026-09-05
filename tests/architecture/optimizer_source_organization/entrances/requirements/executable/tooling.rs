use super::super::RequiredCoordinationEntrance;

pub(crate) const ENTRANCES: &[RequiredCoordinationEntrance] = &[
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/tooling/optimization-policy-offline/src/bin/optimization-policy-offline/main.rs",
        coordination_marker: "fn run",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/tooling/optimization-policy-offline/src/bin/optimization-policy-offline/training.rs",
        coordination_marker: "pub(super) fn train",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/tooling/optimization-policy-offline/src/bin/optimization-policy-offline/evaluation.rs",
        coordination_marker: "pub(super) fn evaluate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/tooling/optimization-policy-offline/src/bin/optimization-policy-offline/regression_manifest.rs",
        coordination_marker: "pub(super) fn create",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/tooling/optimization-policy-offline/src/corpus/mod.rs",
        coordination_marker: "pub fn admit_external_decision_logs",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/tooling/optimization-policy-offline/src/reference_policy/mod.rs",
        coordination_marker: "pub fn train_cost_threshold_v1",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/tooling/optimization-policy-offline/src/reference_policy/training/mod.rs",
        coordination_marker: "pub(super) fn train",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/tooling/optimization-policy-offline/src/reference_policy/evaluation/mod.rs",
        coordination_marker: "pub(super) fn evaluate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/tooling/optimization-policy-offline/src/reference_policy/regression_manifest/mod.rs",
        coordination_marker: "pub(super) fn create",
    },
];
