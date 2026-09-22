use proof_admission::{AdmissionProfile, EvidenceRoute, ProofRule};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm, ScalarType,
};
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_section};
use terminal_fixed_fuel::{derive_fixed_entry_fuel, validate_fixed_entry_fuel};
use terminal_fuel::TerminalFuelSchedule;
use terminal_interpreter::{
    AcceptTerminalEffects, TerminalArtifactInterpretError, TerminalExecutionResult,
    TerminalInterpretError, TerminalScalarValue, TerminalStructuralBooleanFieldValue,
    TerminalStructuralValue, interpret_terminal_artifact_measured,
};
use terminal_psi::{
    OperationKind, OperationResult, StructuralFieldType, StructuralMultiplicity,
    StructuralTypeShape, TerminalAffineCleanupAction, TerminalMachineResult, Terminator,
};

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
