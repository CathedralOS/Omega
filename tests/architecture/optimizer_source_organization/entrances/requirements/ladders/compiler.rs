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
            "omega-rust/omega/pipeline/assembled-syntax-to-checked-compilation/src/optimization/mod.rs",
            "omega-rust/omega/pipeline/source-files-to-assembled-syntax/src/source_assembly/build_vocabulary/mod.rs",
            "omega-rust/omega/pipeline/source-files-to-assembled-syntax/src/source_assembly/build_vocabulary/fragments.rs",
            "omega-rust/omega/pipeline/assembled-syntax-to-checked-compilation/src/optimization/checked_handoff/mod.rs",
        ],
    },
    SemanticLadder {
        family: "compiler native product and optimization selection",
        paths: &[
            "omega-rust/omega/compiler/native-realization/src/native_product.rs",
            "omega-rust/omega/compiler/native-realization/src/native_product/prepared.rs",
            "omega-rust/omega/compiler/native-realization/src/native_product/admission.rs",
            "omega-rust/omega/compiler/native-realization/src/native_product/realization.rs",
            "omega-rust/omega/pipeline/assembled-syntax-to-checked-compilation/src/optimization/rollback/mod.rs",
            "omega-rust/omega/pipeline/assembled-syntax-to-checked-compilation/src/optimization/rollback/request.rs",
            "omega-rust/omega/pipeline/assembled-syntax-to-checked-compilation/src/optimization/rollback/tests.rs",
        ],
    },
];
