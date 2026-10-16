//! Operation and store emission shared by Unit and scalar bodies.
//!
//! Owns the ordered scalar-binding emitter, primitive and structural store
//! emission, runtime-indexed byte and element writes, and the call-operand
//! source custody those emitters replay.

use checked_trees::types::PrimitiveType;
use checked_trees::{
    CheckedScalarExpression, CheckedScalarExpressionRole, CheckedTrees,
    CheckedUnitStructuralPathSegment,
};
use semantic_vocabulary::{
    PlaceId, ScalarType, StructuralFieldId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralAccess, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape, ValueDeclaration,
};

use crate::emission::expression_validation::{
    direct_expression_contains_short_circuit, validate_direct_parameter_types,
};
use crate::emission::operation_emission::buffer::scalar_result_declaration;
use crate::emission::operation_emission::emit_direct_expression;
use crate::emission::scalar_types::terminal_scalar_type;
use crate::lowering_error::{LoweringError, unsupported};
use crate::terminal_identities::{
    allocate_dense, obligation_id, place_id, structural_type_id, value_id,
};

pub(crate) mod affine_cleanup;
mod assignment_destination;
pub(crate) mod claim_bindings;
pub(crate) mod continuation_identities;
pub(crate) mod continuation_specialization;
pub(crate) mod control_flow;
pub(crate) mod formation_memberships;
pub(crate) mod indexed_write;
pub(crate) mod loan_completions;
pub(crate) mod operation_emission;
pub(crate) mod primitive_store;
pub(crate) mod private_borrow_transport;
pub(crate) mod representation_view_store;
pub(crate) mod runtime_elements;
pub(crate) mod selected_continuations;
pub(crate) mod structural_byte_sequence_store;
pub(crate) mod structural_scalar_store;
pub(crate) mod structural_scalar_store_source;

/// Shared move-out/restore emission for owned and exclusive structural storage.
/// Operation frames drive its ledger through each straight-line sequence.
pub(crate) mod borrowed_window;

pub(crate) mod expression_validation;
pub(crate) mod scalar_types;
pub(crate) mod store_destination;

pub(crate) mod boolean_control;
pub(crate) mod case_payload_dispatch;
pub(crate) mod selected_comparison;

/// The next dense byte-sequence literal declaration ordinal in `places`.
/// Terminal verification requires a machine's literal ordinals to be exactly
/// `0..n`; a roster that also holds other private places (a composed body's
/// constructor and join temporaries) must count only its literals.
pub(crate) fn next_byte_sequence_literal_ordinal(
    places: &[StructuralPlaceDeclaration],
) -> Result<u32, LoweringError> {
    let literals = places
        .iter()
        .filter(|place| matches!(place.kind, StructuralPlaceKind::ByteSequenceLiteral { .. }))
        .count();
    u32::try_from(literals)
        .map_err(|_| LoweringError::Unsupported("literal declaration ordinal exceeds u32"))
}

/// Convert an already-evaluated integer to Terminal's `u64` count
/// coordinate: a `u64` is used as is, a narrower unsigned carrier widens, and
/// any other carrier takes the ordinary exact cast. A checked selector bound
/// proves that conversion fits too, including signed nonnegativity. Generated
/// coordinates without that source decision retain their conversion obligation.
pub(crate) fn emit_u64_coordinate(
    operand: ValueId,
    source: ScalarType,
    bound_decided: bool,
    next_value: &mut u64,
    next_obligation: &mut u64,
    operations: &mut operation_emission::buffer::OperationBuffer,
) -> Result<ValueId, LoweringError> {
    let Some(source_type) = source.as_integer() else {
        return unsupported("byte index requires an integer carrier");
    };
    let coordinate_type = terminal_scalar_type(PrimitiveType::U64)?;
    if source == coordinate_type {
        return Ok(operand);
    }
    let Some(target_type) = coordinate_type.as_integer() else {
        return unsupported("byte coordinate requires an integer carrier");
    };
    let kind = if source_type.can_widen_to(target_type) {
        OperationKind::IntegerWiden { operand }
    } else {
        let obligation = obligation_id(allocate_dense(next_obligation)?);
        if bound_decided {
            operations.source_checked_obligations.push(obligation);
        }
        OperationKind::IntegerExactCast {
            operand,
            obligation,
        }
    };
    let coordinate = value_id(allocate_dense(next_value)?);
    operations.push_leaf(scalar_result_declaration(coordinate, coordinate_type), kind);
    Ok(coordinate)
}

pub(crate) mod quotient_values;
