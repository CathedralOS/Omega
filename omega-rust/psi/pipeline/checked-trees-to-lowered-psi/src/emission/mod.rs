//! Operation and store emission shared by Unit and scalar bodies.
//!
//! Owns the ordered scalar-binding emitter, primitive and structural store
//! emission, byte-sequence writes, and the call-operand source custody those
//! emitters replay.

use checked_trees::types::PrimitiveType;
use checked_trees::{
    CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarExpressionRole, CheckedTrees,
    CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment,
};
use language_semantics::Multiplicity;
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
use crate::emission::operation_emission::emit_direct_expression;
use crate::emission::scalar_types::terminal_scalar_type;
use crate::expression_preparation::prepare_expression::lower_checked_scalar_expression;
use crate::lowering_error::{LoweringError, unsupported};
use crate::terminal_identities::{
    allocate_dense, obligation_id, place_id, structural_type_id, value_id,
};

pub(crate) mod byte_sequence_write;
pub(crate) mod call_source_custody;
pub(crate) mod operation_emission;
pub(crate) mod primitive_store;
pub(crate) mod structural_byte_sequence_index_store;
pub(crate) mod structural_byte_sequence_store;
pub(crate) mod structural_scalar_store;
pub(crate) mod structural_scalar_store_source;

pub(crate) mod expression_validation;
pub(crate) mod scalar_types;
pub(crate) mod store_destination;

pub(crate) mod boolean_control;
pub(crate) mod selected_comparison;

/// Resolve a byte-sequence store's scalar source against the dense scalar
/// namespace. A bound pure authored expression lowers through its
/// `AssignmentValue` row; the SSA result of the scalar call this same
/// statement performs binds no local and carries no `AssignmentValue` row, so
/// it is read back where the ordered call operation established it as the most
/// recent scalar value.
pub(crate) fn byte_store_scalar_value(
    bindings: &crate::expression_preparation::bindings::ScalarBindings,
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    statement_index: u32,
    value: &checked_trees::CheckedByteSequenceStoreValue,
    values: &[ValueDeclaration],
) -> Result<crate::emission::operation_emission::expressions::LoweredDirectExpression, LoweringError>
{
    match value {
        checked_trees::CheckedByteSequenceStoreValue::Pure(_) => bindings.expression_at(
            checked,
            state,
            statement_index,
            CheckedScalarExpressionRole::AssignmentValue,
        ),
        checked_trees::CheckedByteSequenceStoreValue::ScalarResult { position } => {
            let position = usize::try_from(*position).map_err(|_| {
                LoweringError::Unsupported("byte store call-result position exceeds usize")
            })?;
            if position.checked_add(1) != Some(values.len()) {
                return unsupported(
                    "byte store call result is not this statement's established scalar result",
                );
            }
            let scalar_type = terminal_scalar_type(PrimitiveType::U8)?;
            if values[position].scalar_type != scalar_type {
                return unsupported("byte store call result differs from its element type");
            }
            Ok(
                crate::emission::operation_emission::expressions::LoweredDirectExpression::Local {
                    position,
                    scalar_type,
                },
            )
        }
    }
}

/// Evaluate the authored integer before converting to Terminal's count
/// coordinate. Widening the expression itself would change narrow or wrapping
/// arithmetic. The ordinary exact-cast obligation proves fit (including signed
/// nonnegativity); the consuming store separately proves the current live bound.
pub(crate) fn emit_byte_index(
    expression: &operation_emission::expressions::LoweredDirectExpression,
    values: &[ValueDeclaration],
    next_value: &mut u64,
    next_obligation: &mut u64,
    operations: &mut operation_emission::buffer::OperationBuffer,
) -> Result<ValueId, LoweringError> {
    let ScalarType::Integer(source_type) = expression.scalar_type() else {
        return unsupported("byte index requires an integer carrier");
    };
    let operand = emit_direct_expression(expression, values, next_value, operations);
    let coordinate_type = terminal_scalar_type(PrimitiveType::U64)?;
    if expression.scalar_type() == coordinate_type {
        return Ok(operand);
    }
    let ScalarType::Integer(target_type) = coordinate_type else {
        return unsupported("byte coordinate requires an integer carrier");
    };
    let kind = if source_type.can_widen_to(target_type) {
        OperationKind::IntegerWiden { operand }
    } else {
        OperationKind::IntegerExactCast {
            operand,
            obligation: obligation_id(allocate_dense(next_obligation)?),
        }
    };
    let coordinate = value_id(allocate_dense(next_value)?);
    let id = operations.allocate();
    operations.push(Operation {
        static_reach_binding: None,
        id,
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: coordinate,
            scalar_type: coordinate_type,
        }),
        kind,
    });
    Ok(coordinate)
}
