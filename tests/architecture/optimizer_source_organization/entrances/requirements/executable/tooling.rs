use super::super::RequiredCoordinationEntrance;

pub(crate) const ENTRANCES: &[RequiredCoordinationEntrance] = &[
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/main.rs",
        coordination_marker: "fn run",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/training.rs",
        coordination_marker: "pub(super) fn train",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/bin/omega-optimization-policy-offline/evaluation.rs",
        coordination_marker: "pub(super) fn evaluate",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/corpus/mod.rs",
        coordination_marker: "pub fn admit_external_decision_logs",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/mod.rs",
        coordination_marker: "pub fn train_cost_threshold_v1",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/training/mod.rs",
        coordination_marker: "pub(super) fn train",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/tooling/omega-optimization-policy-offline/src/reference_policy/evaluation/mod.rs",
        coordination_marker: "pub(super) fn evaluate",
    },
];
