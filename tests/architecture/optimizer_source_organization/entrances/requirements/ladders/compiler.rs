use super::super::SemanticLadder;

pub(crate) const LADDERS: &[SemanticLadder] = &[
    SemanticLadder {
        family: "build-authored optimization selection",
        paths: &[
            "omega-rust/omega/build/build-evaluation/src/optimization/mod.rs",
            "omega-rust/omega/build/build-evaluation/src/optimization/vocabulary.rs",
            "omega-rust/omega/build/build-evaluation/src/optimization/selection.rs",
        ],
    },
    SemanticLadder {
        family: "compiler optimization vocabulary and checked handoff",
        paths: &[
            "omega-rust/omega/compiler/src/checked/optimization/mod.rs",
            "omega-rust/omega/compiler/src/sources/source_assembly/build_vocabulary/mod.rs",
            "omega-rust/omega/compiler/src/sources/source_assembly/build_vocabulary/fragments.rs",
            "omega-rust/omega/compiler/src/checked/optimization/checked_handoff/mod.rs",
        ],
    },
    SemanticLadder {
        family: "compiler native product and optimization selection",
        paths: &[
            "omega-rust/omega/compiler/src/native/native_product.rs",
            "omega-rust/omega/compiler/src/native/native_product/prepared.rs",
            "omega-rust/omega/compiler/src/native/native_product/admission.rs",
            "omega-rust/omega/compiler/src/native/native_product/realization.rs",
            "omega-rust/omega/compiler/src/checked/optimization/rollback/mod.rs",
            "omega-rust/omega/compiler/src/checked/optimization/rollback/request.rs",
        ],
    },
];
