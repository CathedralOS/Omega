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
            "omega-rust/omega/compiler/compiler/src/pipeline/optimization/mod.rs",
            "omega-rust/omega/compiler/compiler/src/pipeline/optimization/build_vocabulary/mod.rs",
            "omega-rust/omega/compiler/compiler/src/pipeline/optimization/build_vocabulary/fragments.rs",
            "omega-rust/omega/compiler/compiler/src/pipeline/optimization/checked_handoff/mod.rs",
        ],
    },
    SemanticLadder {
        family: "compiler native optimization realization",
        paths: &[
            "omega-rust/omega/compiler/compiler/src/compiler/optimization/mod.rs",
            "omega-rust/omega/compiler/compiler/src/compiler/optimization/native_report/mod.rs",
            "omega-rust/omega/compiler/compiler/src/compiler/optimization/native_report/model.rs",
            "omega-rust/omega/compiler/compiler/src/compiler/optimization/admission.rs",
            "omega-rust/omega/compiler/compiler/src/compiler/optimization/native_realization.rs",
            "omega-rust/omega/compiler/compiler/src/compiler/optimization/rollback/mod.rs",
            "omega-rust/omega/compiler/compiler/src/compiler/optimization/rollback/request.rs",
            "omega-rust/omega/compiler/compiler/src/compiler/optimization/rollback/tests.rs",
        ],
    },
    SemanticLadder {
        family: "dormant external optimization policy execution",
        paths: &[
            "omega-rust/omega/compiler/compiler/src/compiler/optimization/external_policy/mod.rs",
            "omega-rust/omega/compiler/compiler/src/compiler/optimization/external_policy/capability.rs",
            "omega-rust/omega/compiler/compiler/src/compiler/optimization/external_policy/limits.rs",
            "omega-rust/omega/compiler/compiler/src/compiler/optimization/external_policy/model.rs",
            "omega-rust/omega/compiler/compiler/src/compiler/optimization/external_policy/response.rs",
            "omega-rust/omega/compiler/compiler/src/compiler/optimization/external_policy/settlement.rs",
            "omega-rust/omega/compiler/compiler/src/compiler/optimization/external_policy/tests.rs",
        ],
    },
];
