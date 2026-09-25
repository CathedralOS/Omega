use super::super::RequiredCoordinationEntrance;

pub(crate) const ENTRANCES: &[RequiredCoordinationEntrance] = &[
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/build/build-evaluation/src/optimization/mod.rs",
        coordination_marker: "impl BuildOptimizationAdmission",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/compiler/src/sources/source_assembly/build_vocabulary/mod.rs",
        coordination_marker: "fn install(base: &str)",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/compiler/src/checked/optimization/checked_handoff/mod.rs",
        coordination_marker: "fn retain(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/compiler/src/native/native_product.rs",
        coordination_marker: "pub fn prepare_native_product(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/compiler/src/checked/optimization/rollback/mod.rs",
        coordination_marker: "fn settle(",
    },
];
