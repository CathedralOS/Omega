use super::super::SemanticLadder;

pub(crate) const LADDERS: &[SemanticLadder] = &[
    SemanticLadder {
        family: "optimized semantic program entry",
        paths: &[
            "source/omega-rust/omega/backend/plans/omega-program-entry-plan/src/optimized_semantic_entry/mod.rs",
            "source/omega-rust/omega/backend/plans/omega-program-entry-plan/src/optimized_semantic_entry/model.rs",
            "source/omega-rust/omega/backend/plans/omega-program-entry-plan/src/optimized_semantic_entry/validation.rs",
            "source/omega-rust/omega/backend/plans/omega-program-entry-plan/src/optimized_semantic_entry/construction.rs",
        ],
    },
    SemanticLadder {
        family: "optimized semantic program wrapper",
        paths: &[
            "source/omega-rust/omega/backend/plans/omega-program-entry-plan/src/optimized_semantic_wrapper/mod.rs",
            "source/omega-rust/omega/backend/plans/omega-program-entry-plan/src/optimized_semantic_wrapper/model.rs",
            "source/omega-rust/omega/backend/plans/omega-program-entry-plan/src/optimized_semantic_wrapper/recipe.rs",
            "source/omega-rust/omega/backend/plans/omega-program-entry-plan/src/optimized_semantic_wrapper/validation.rs",
        ],
    },
    SemanticLadder {
        family: "provider-execution settlement",
        paths: &[
            "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/settlements/mod.rs",
            "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/settlements/boundary.rs",
            "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/settlements/exact_plan.rs",
            "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/settlements/normalized_foreign_call.rs",
            "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/settlements/tests/mod.rs",
            "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/settlements/tests/exact_evidence.rs",
            "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/settlements/tests/fixtures.rs",
            "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/settlements/tests/literal_arguments.rs",
            "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/settlements/tests/zero_argument_import.rs",
        ],
    },
    SemanticLadder {
        family: "function-relative realization codec",
        paths: &[
            "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/function_relative_realization/codec/mod.rs",
            "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/function_relative_realization/codec/encoding.rs",
            "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/function_relative_realization/codec/decoding.rs",
            "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/function_relative_realization/codec/post_allocation.rs",
            "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/function_relative_realization/codec/target.rs",
            "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/function_relative_realization/codec/rendering.rs",
            "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/function_relative_realization/codec/cursor.rs",
            "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/function_relative_realization/codec/error.rs",
        ],
    },
];
