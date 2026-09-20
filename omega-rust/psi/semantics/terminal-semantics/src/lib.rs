#![forbid(unsafe_code)]

//! Closed, target-neutral semantic tables for terminal Psi.
//!
//! This crate owns declarative operation-row identity and the local semantics
//! that can be interpreted without control-flow, call-composition, or proof
//! reduction policy. It deliberately does not own traversal, evidence
//! availability, sufficient-form reduction, or provider realization.
//!
//! `semantic_rows.rs` is the root: the row every terminal
//! operation kind resolves to. `scalar_leaf_schema.rs` describes a goal-free
//! scalar leaf and `scalar_leaf_semantics.rs` interprets one; the other
//! modules cover call composition, proof-bearing scalars, record fields,
//! primitive places, scalar arrays and structural effects.

mod call_composition;
mod primitive_place;
mod proof_bearing_scalar;
mod record_field;
mod scalar_array;
mod scalar_leaf_schema;
mod scalar_leaf_semantics;
mod semantic_rows;
mod static_path;
mod structural_effect;
#[cfg(test)]
mod tests;

pub use call_composition::{
    CallArgumentRule, CallCompositionSchema, CallCompositionSemanticRow, CallCrashRule,
    CallEvidenceRule, CallFrontierRule, CallFuelPolicy, CallOutcomeRule, CallRequirementRule,
    CallResultRule, CallTargetRule, CallTransferRule, boundary_buffer_capacity,
    call_composition_semantic_row, exact_call_composition_semantic_row_in,
    mutable_fixed_byte_array_extent, shared_boundary_buffer_capacity,
    validate_call_composition_semantic_rows,
};
pub use primitive_place::{fixed_array_place_shape, primitive_place_type};
pub use proof_bearing_scalar::{
    CanonicalScalarGoal, ProofBearingIntegerPolicyBinding, ProofBearingScalarLeafElision,
    ProofBearingScalarLeafSchema, ProofBearingScalarLeafSemantics, ProofBearingScalarSemanticRow,
    elidable_proof_bearing_scalar_leaf, exact_proof_bearing_scalar_semantic_row_in,
    proof_bearing_integer_policy_binding, proof_bearing_scalar_leaf_semantics,
    proof_bearing_scalar_semantic_row, validate_proof_bearing_scalar_semantic_rows,
};
pub use record_field::{RecordFieldCarrier, record_field_carrier};
pub use scalar_array::scalar_array_leaf_shape;
pub use scalar_leaf_schema::{
    GoalFreeScalarLeafSchema, ScalarLeafCrashPolicy, ScalarLeafDenotation, ScalarLeafFactShape,
    ScalarLeafFrontierPolicy, ScalarLeafFuelPolicy, ScalarLeafGoalShape, ScalarLeafOperandShape,
    ScalarLeafResultShape,
};
pub use scalar_leaf_semantics::{
    GoalFreeScalarLeafSemantics, ScalarLeafLiteral, constant_goal_free_scalar_leaf,
    goal_free_scalar_leaf_semantics,
};
pub use semantic_rows::{
    OperationSemanticCustody, OperationSemanticError, OperationSemanticRow, OperationSemanticTag,
    exact_operation_semantic_row_in, is_unconditionally_total_scalar, operation_semantic_row,
};
pub use static_path::{canonical_structural_path_tip, runtime_structural_path_tip};
pub use structural_effect::{
    StructuralEffectAction, StructuralEffectCustody, StructuralEffectExternalEffect,
    StructuralEffectFrontierPolicy, StructuralEffectFuelPolicy, StructuralEffectGoalShape,
    StructuralEffectLeafSchema, StructuralEffectObservation, StructuralEffectResultShape,
    StructuralEffectSemanticRow, exact_structural_effect_semantic_row_in, literal_length_equation,
    structural_effect_leaf_observation, structural_effect_leaf_observation_in,
    structural_effect_semantic_row, subslice_length_equation,
    validate_structural_effect_semantic_rows,
};
