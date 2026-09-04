use super::super::SemanticLadder;

pub(crate) const LADDERS: &[SemanticLadder] = &[
    SemanticLadder {
        family: "terminal native-artifact realization",
        paths: &[
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/mod.rs",
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/native_artifact.rs",
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/boundary_applications.rs",
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/input.rs",
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/mod.rs",
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/machine_code.rs",
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/output.rs",
        ],
    },
    SemanticLadder {
        family: "terminal authority closure review",
        paths: &[
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/terminal_authority_review.rs",
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/terminal_authority_review/context.rs",
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/terminal_authority_review/reviewer.rs",
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/terminal_authority_review/operations.rs",
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/terminal_authority_review/tests.rs",
        ],
    },
    SemanticLadder {
        family: "terminal authority policy",
        paths: &[
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/terminal_authority_policy/mod.rs",
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/terminal_authority_policy/model.rs",
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/terminal_authority_policy/normalized_foreign.rs",
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/terminal_authority_policy/classification.rs",
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/terminal_authority_policy/inventory.rs",
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/terminal_authority_policy/commitment.rs",
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/terminal_authority_policy/tests/mod.rs",
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/terminal_authority_policy/tests/inventory.rs",
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/terminal_authority_policy/tests/foreign_rows.rs",
        ],
    },
    SemanticLadder {
        family: "optimized semantic program entry",
        paths: &[
            "omega-rust/omega/backend/plans/omega-program-entry-plan/src/optimized_semantic_entry/mod.rs",
            "omega-rust/omega/backend/plans/omega-program-entry-plan/src/optimized_semantic_entry/model.rs",
            "omega-rust/omega/backend/plans/omega-program-entry-plan/src/optimized_semantic_entry/validation.rs",
            "omega-rust/omega/backend/plans/omega-program-entry-plan/src/optimized_semantic_entry/construction.rs",
        ],
    },
    SemanticLadder {
        family: "optimized semantic program wrapper",
        paths: &[
            "omega-rust/omega/backend/plans/omega-program-entry-plan/src/optimized_semantic_wrapper/mod.rs",
            "omega-rust/omega/backend/plans/omega-program-entry-plan/src/optimized_semantic_wrapper/model.rs",
            "omega-rust/omega/backend/plans/omega-program-entry-plan/src/optimized_semantic_wrapper/recipe.rs",
            "omega-rust/omega/backend/plans/omega-program-entry-plan/src/optimized_semantic_wrapper/validation.rs",
        ],
    },
    SemanticLadder {
        family: "provider-execution settlement",
        paths: &[
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/settlements/mod.rs",
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/settlements/boundary.rs",
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/settlements/exact_plan.rs",
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/settlements/normalized_foreign_call.rs",
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/settlements/tests/mod.rs",
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/settlements/tests/exact_evidence.rs",
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/settlements/tests/fixtures.rs",
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/settlements/tests/literal_arguments.rs",
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/settlements/tests/zero_argument_import.rs",
        ],
    },
    SemanticLadder {
        family: "function-relative realization codec",
        paths: &[
            "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/function_relative_realization/codec/mod.rs",
            "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/function_relative_realization/codec/encoding.rs",
            "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/function_relative_realization/codec/decoding.rs",
            "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/function_relative_realization/codec/post_allocation.rs",
            "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/function_relative_realization/codec/target.rs",
            "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/function_relative_realization/codec/rendering.rs",
            "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/function_relative_realization/codec/cursor.rs",
            "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/function_relative_realization/codec/error.rs",
        ],
    },
    SemanticLadder {
        family: "function-relative realization mutation tests",
        paths: &[
            "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/realization/function_relative_manifest_mutation_matrix/mod.rs",
            "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/realization/function_relative_manifest_mutation_matrix/fixture.rs",
            "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/realization/function_relative_manifest_mutation_matrix/manifest_fields.rs",
            "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/realization/function_relative_manifest_mutation_matrix/manifest_wire.rs",
            "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/realization/function_relative_manifest_mutation_matrix/wire_offsets.rs",
            "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/realization/function_relative_manifest_mutation_matrix/custody.rs",
        ],
    },
    SemanticLadder {
        family: "hosted post-allocation publication matrix",
        paths: &[
            "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/artifacts/output_artifacts/function_fragment_emission/post_allocation_machine/mod.rs",
            "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/artifacts/output_artifacts/function_fragment_emission/post_allocation_machine/cases.rs",
            "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/artifacts/output_artifacts/function_fragment_emission/post_allocation_machine/realization.rs",
            "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/artifacts/output_artifacts/function_fragment_emission/post_allocation_machine/artifacts.rs",
            "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/artifacts/output_artifacts/function_fragment_emission/post_allocation_machine/refusals.rs",
        ],
    },
    SemanticLadder {
        family: "ProgramStorage wrapper manifest mutation tests",
        paths: &[
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/optimized_semantic_wrapper_object/tests/manifest_mutation_matrix/mod.rs",
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/optimized_semantic_wrapper_object/tests/manifest_mutation_matrix/fixture.rs",
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/optimized_semantic_wrapper_object/tests/manifest_mutation_matrix/fields.rs",
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/optimized_semantic_wrapper_object/tests/manifest_mutation_matrix/wire.rs",
            "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/optimized_semantic_wrapper_object/tests/manifest_mutation_matrix/wire_offsets.rs",
        ],
    },
];
