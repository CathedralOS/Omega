use super::super::SemanticLadder;

pub(crate) const LADDERS: &[SemanticLadder] = &[
    SemanticLadder {
        family: "build-authored optimization selection",
        paths: &[
            "source/omega-rust/omega/build/omega-build-evaluation/src/optimization/mod.rs",
            "source/omega-rust/omega/build/omega-build-evaluation/src/optimization/vocabulary.rs",
            "source/omega-rust/omega/build/omega-build-evaluation/src/optimization/selection.rs",
        ],
    },
    SemanticLadder {
        family: "compiler optimization vocabulary and checked handoff",
        paths: &[
            "source/omega-rust/omega/compiler/omega-compiler/src/pipeline/optimization/mod.rs",
            "source/omega-rust/omega/compiler/omega-compiler/src/pipeline/optimization/build_vocabulary/mod.rs",
            "source/omega-rust/omega/compiler/omega-compiler/src/pipeline/optimization/build_vocabulary/fragments.rs",
            "source/omega-rust/omega/compiler/omega-compiler/src/pipeline/optimization/checked_handoff/mod.rs",
        ],
    },
    SemanticLadder {
        family: "compiler native optimization realization",
        paths: &[
            "source/omega-rust/omega/compiler/omega-compiler/src/compiler/optimization/mod.rs",
            "source/omega-rust/omega/compiler/omega-compiler/src/compiler/optimization/admission.rs",
            "source/omega-rust/omega/compiler/omega-compiler/src/compiler/optimization/native_realization.rs",
            "source/omega-rust/omega/compiler/omega-compiler/src/compiler/optimization/rollback/mod.rs",
            "source/omega-rust/omega/compiler/omega-compiler/src/compiler/optimization/rollback/request.rs",
            "source/omega-rust/omega/compiler/omega-compiler/src/compiler/optimization/rollback/tests.rs",
        ],
    },
    SemanticLadder {
        family: "dormant external optimization policy execution",
        paths: &[
            "source/omega-rust/omega/compiler/omega-compiler/src/compiler/optimization/external_policy/mod.rs",
            "source/omega-rust/omega/compiler/omega-compiler/src/compiler/optimization/external_policy/capability.rs",
            "source/omega-rust/omega/compiler/omega-compiler/src/compiler/optimization/external_policy/limits.rs",
            "source/omega-rust/omega/compiler/omega-compiler/src/compiler/optimization/external_policy/model.rs",
            "source/omega-rust/omega/compiler/omega-compiler/src/compiler/optimization/external_policy/response.rs",
            "source/omega-rust/omega/compiler/omega-compiler/src/compiler/optimization/external_policy/settlement.rs",
            "source/omega-rust/omega/compiler/omega-compiler/src/compiler/optimization/external_policy/tests.rs",
        ],
    },
];
