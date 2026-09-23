use super::super::SemanticLadder;

pub(crate) const LADDERS: &[SemanticLadder] = &[
    SemanticLadder {
        family: "terminal native-artifact realization",
        paths: &[
            "omega-rust/omega/compiler/native-realization/src/native_realization.rs",
            "omega-rust/omega/compiler/native-realization/src/native_realization/boundary_applications.rs",
            "omega-rust/omega/compiler/native-realization/src/native_realization/input_preparation.rs",
            "omega-rust/omega/compiler/native-realization/src/native_realization/providers/mod.rs",
            "omega-rust/omega/compiler/native-realization/src/native_realization/object_emission.rs",
            "omega-rust/omega/compiler/native-realization/src/native_realization/artifact_assembly.rs",
        ],
    },
    SemanticLadder {
        family: "terminal authority closure review",
        paths: &[
            "omega-rust/omega/compiler/native-realization/src/native_realization/terminal_authority_review.rs",
            "omega-rust/omega/compiler/native-realization/src/native_realization/terminal_authority_review/context.rs",
            "omega-rust/omega/compiler/native-realization/src/native_realization/terminal_authority_review/reviewer.rs",
            "omega-rust/omega/compiler/native-realization/src/native_realization/terminal_authority_review/operations.rs",
            "omega-rust/omega/compiler/native-realization/src/native_realization/terminal_authority_review/tests.rs",
        ],
    },
    SemanticLadder {
        family: "terminal authority policy",
        paths: &[
            "omega-rust/omega/compiler/native-realization/src/native_realization/terminal_authority_policy/mod.rs",
            "omega-rust/omega/compiler/native-realization/src/native_realization/terminal_authority_policy/model.rs",
            "omega-rust/omega/compiler/native-realization/src/native_realization/terminal_authority_policy/normalized_foreign.rs",
            "omega-rust/omega/compiler/native-realization/src/native_realization/terminal_authority_policy/classification.rs",
            "omega-rust/omega/compiler/native-realization/src/native_realization/terminal_authority_policy/inventory.rs",
            "omega-rust/omega/compiler/native-realization/src/native_realization/terminal_authority_policy/commitment.rs",
            "omega-rust/omega/compiler/native-realization/src/native_realization/terminal_authority_policy/tests/mod.rs",
            "omega-rust/omega/compiler/native-realization/src/native_realization/terminal_authority_policy/tests/inventory.rs",
            "omega-rust/omega/compiler/native-realization/src/native_realization/terminal_authority_policy/tests/foreign_rows.rs",
        ],
    },
    SemanticLadder {
        family: "optimized semantic program entry",
        paths: &[
            "omega-rust/omega/backend/plans/program-entry-plan/src/optimized_semantic_entry/mod.rs",
            "omega-rust/omega/backend/plans/program-entry-plan/src/optimized_semantic_entry/validation.rs",
            "omega-rust/omega/backend/plans/program-entry-plan/src/optimized_semantic_entry/construction.rs",
        ],
    },
    SemanticLadder {
        family: "optimized semantic program wrapper",
        paths: &[
            "omega-rust/omega/backend/plans/program-entry-plan/src/optimized_semantic_wrapper/mod.rs",
            "omega-rust/omega/backend/plans/program-entry-plan/src/optimized_semantic_wrapper/recipe.rs",
            "omega-rust/omega/backend/plans/program-entry-plan/src/optimized_semantic_wrapper/validation.rs",
            "omega-rust/omega/backend/plans/program-entry-plan/src/optimized_semantic_wrapper/encoding.rs",
            "omega-rust/omega/backend/plans/program-entry-plan/src/optimized_semantic_wrapper/encoding/projection.rs",
        ],
    },
    SemanticLadder {
        family: "provider-execution settlement",
        paths: &[
            "omega-rust/omega/compiler/native-realization/src/native_realization/providers/settlements/mod.rs",
            "omega-rust/omega/compiler/native-realization/src/native_realization/providers/settlements/boundary.rs",
            "omega-rust/omega/compiler/native-realization/src/native_realization/providers/settlements/exact_plan.rs",
            "omega-rust/omega/compiler/native-realization/src/native_realization/providers/settlements/normalized_foreign_call.rs",
            "omega-rust/omega/compiler/native-realization/src/native_realization/providers/settlements/tests/mod.rs",
            "omega-rust/omega/compiler/native-realization/src/native_realization/providers/settlements/tests/exact_evidence.rs",
            "omega-rust/omega/compiler/native-realization/src/native_realization/providers/settlements/tests/fixtures.rs",
        ],
    },
    SemanticLadder {
        family: "function-relative realization codec",
        paths: &[
            "omega-rust/omega/backend/machine-emission/src/function_realization/codec/mod.rs",
            "omega-rust/omega/backend/machine-emission/src/function_realization/codec/encoding.rs",
            "omega-rust/omega/backend/machine-emission/src/function_realization/codec/decoding.rs",
            "omega-rust/omega/backend/machine-emission/src/function_realization/codec/post_allocation.rs",
            "omega-rust/omega/backend/machine-emission/src/function_realization/codec/target.rs",
            "omega-rust/omega/backend/machine-emission/src/function_realization/codec/rendering.rs",
            "omega-rust/omega/backend/machine-emission/src/function_realization/codec/cursor.rs",
            "omega-rust/omega/backend/machine-emission/src/function_realization/codec/error.rs",
        ],
    },
    SemanticLadder {
        family: "ProgramStorage wrapper manifest mutation tests",
        paths: &[
            "omega-rust/omega/backend/artifacts/native-artifact/src/semantic_wrapper_object/tests/manifest_mutation_matrix/mod.rs",
            "omega-rust/omega/backend/artifacts/native-artifact/src/semantic_wrapper_object/tests/manifest_mutation_matrix/fixture.rs",
            "omega-rust/omega/backend/artifacts/native-artifact/src/semantic_wrapper_object/tests/manifest_mutation_matrix/fields.rs",
            "omega-rust/omega/backend/artifacts/native-artifact/src/semantic_wrapper_object/tests/manifest_mutation_matrix/wire.rs",
            "omega-rust/omega/backend/artifacts/native-artifact/src/semantic_wrapper_object/tests/manifest_mutation_matrix/wire_offsets.rs",
        ],
    },
];
