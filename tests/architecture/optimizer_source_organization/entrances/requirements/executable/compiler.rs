use super::super::RequiredCoordinationEntrance;

pub(crate) const ENTRANCES: &[RequiredCoordinationEntrance] = &[
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/build/omega-build-evaluation/src/optimization/mod.rs",
        coordination_marker: "impl BuildOptimizationAdmission",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/compiler/omega-compiler/src/pipeline/optimization/build_vocabulary/mod.rs",
        coordination_marker: "fn install(base: &str)",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/compiler/omega-compiler/src/pipeline/optimization/checked_handoff/mod.rs",
        coordination_marker: "fn retain(",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/compiler/omega-compiler/src/compiler/optimization/mod.rs",
        coordination_marker: "fn native_report(",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/compiler/omega-compiler/src/compiler/optimization/external_policy/mod.rs",
        coordination_marker: "pub(crate) fn execute(",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/compiler/omega-compiler/src/compiler/optimization/rollback/mod.rs",
        coordination_marker: "fn settle(",
    },
];
