use psi_core::{IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm, ScalarType};
use psi_proof_admission::{AdmissionProfile, EvidenceRoute, ProofRule};
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::{
    OperationKind, OperationResult, StructuralFieldType, StructuralMultiplicity,
    StructuralTypeShape, TerminalAffineCleanupAction, TerminalMachineResult, Terminator,
};
use psi_terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_bundle};
use psi_terminal_fixed_fuel::{derive_fixed_entry_fuel, validate_fixed_entry_fuel};
use psi_terminal_fuel::TerminalFuelSchedule;
use psi_terminal_interpreter::{
    AcceptTerminalEffects, TerminalArtifactInterpretError, TerminalExecutionResult,
    TerminalInterpretError, TerminalScalarValue, TerminalStructuralBooleanFieldValue,
    TerminalStructuralValue, interpret_terminal_artifact_with_effect_handler_measured,
    interpret_terminal_artifact_with_structural_boolean_fields_measured,
};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

#[path = "nominal_affine_source/affine_cast.rs"]
mod affine_cast;
#[path = "nominal_affine_source/integer_comparison.rs"]
mod integer_comparison;
#[path = "nominal_affine_source/scalar_returns.rs"]
mod scalar_returns;
#[path = "nominal_affine_source/short_circuit.rs"]
mod short_circuit;
#[path = "nominal_affine_source/unit_cleanup.rs"]
mod unit_cleanup;
