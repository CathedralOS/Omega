//! One emitter for every borrowed view range `[start, end)`.
//!
//! A call argument, a Unit-graph edge transfer, a scalar-graph successor and a
//! view local's `let` all narrow an established view with the same Terminal
//! operation. Their sites differ only in how they reach the source place and
//! lower the endpoints; the operation, its dominating length observation and
//! its obligation identity are emitted here once. The verifier independently
//! checks that the length observes the exact source and that the source is an
//! established view dominating this use; nothing here relaxes either check.
use crate::emission::expression_validation::{
    direct_expression_contains_short_circuit, validate_direct_parameter_types,
};
use crate::emission::operation_emission::buffer::{OperationBuffer, structural_result_declaration};
use crate::emission::operation_emission::expressions::{
    LoweredDirectExpression, emit_byte_length, emit_direct_expression, emit_element_length,
};
use crate::emission::scalar_types::terminal_scalar_type;
use crate::lowering_error::{LoweringError, unsupported};
use crate::terminal_identities::obligation_id;
use checked_trees::types::PrimitiveType;
use semantic_vocabulary::{IntegerValue, PlaceId, StructuralPlaceKind, StructuralTypeId, ValueId};
use terminal_psi::{
    Operation, OperationKind, StructuralMultiplicity, StructuralPlaceDeclaration, ValueDeclaration,
};

/// Which Terminal view family the range narrows: byte views count bytes,
/// element views count elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewFamily {
    Bytes,
    Elements,
}

/// Normalize a checked range endpoint after its authored integer arithmetic.
/// Source range admission establishes 0 <= endpoint <= extent <= u64::MAX,
/// so this exact coordinate conversion carries that same admission decision.
/// It never changes the endpoint's arithmetic carrier or policy during evaluation.
pub(crate) fn coordinate(
    expression: LoweredDirectExpression,
) -> Result<LoweredDirectExpression, LoweringError> {
    let source = expression
        .scalar_type()
        .as_integer()
        .filter(|integer| !integer.is_address())
        .ok_or(LoweringError::Unsupported(
            "range endpoint requires an integer carrier",
        ))?;
    let scalar_type = terminal_scalar_type(PrimitiveType::U64)?;
    if expression.scalar_type() == scalar_type {
        return Ok(expression);
    }
    if source.can_widen_to(
        scalar_type
            .as_integer()
            .ok_or(LoweringError::InvalidPsiIntegerType)?,
    ) {
        Ok(LoweredDirectExpression::IntegerWiden {
            scalar_type,
            operand: Box::new(expression),
        })
    } else {
        Ok(LoweredDirectExpression::IntegerExactCast {
            scalar_type,
            operand: Box::new(expression),
            formation_decided: true,
        })
    }
}

/// Emit `source[start..end]` into `destination`. A missing start is zero and
/// a missing end is the source's own length. Present endpoints keep their
/// authored integer carrier and become u64 coordinates only after evaluation.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit(
    family: ViewFamily,
    source: terminal_psi::StructuralArgument,
    start: Option<&LoweredDirectExpression>,
    end: Option<&LoweredDirectExpression>,
    destination: PlaceId,
    structural_type: StructuralTypeId,
    values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let multiplicity = match source.access {
        terminal_psi::StructuralAccess::SharedBorrow => StructuralMultiplicity::Unrestricted,
        terminal_psi::StructuralAccess::MutableBorrow => StructuralMultiplicity::Affine,
        _ => return unsupported("view subslice requires readable borrowed access"),
    };
    let count_type = terminal_scalar_type(PrimitiveType::U64)?;
    let types = values
        .iter()
        .map(|value| value.scalar_type)
        .collect::<Vec<_>>();
    let mut endpoint = |expression: &LoweredDirectExpression| -> Result<ValueId, LoweringError> {
        let expression = coordinate(expression.clone())?;
        if direct_expression_contains_short_circuit(&expression) {
            return unsupported("view subslice endpoint needs a branch-free u64 value");
        }
        validate_direct_parameter_types(&expression, &types)?;
        Ok(emit_direct_expression(
            &expression,
            values,
            next_value,
            operations,
        ))
    };
    let start = match start {
        Some(start) => endpoint(start)?,
        None => endpoint(&LoweredDirectExpression::IntegerLiteral {
            value: IntegerValue::Unsigned(0),
            scalar_type: count_type,
        })?,
    };
    let end = end.map(&mut endpoint).transpose()?;
    // The length names the exact source; the length emitters reuse the
    // dominating observation already on this emission path.
    let length = match family {
        ViewFamily::Bytes => emit_byte_length(source.clone(), next_value, operations),
        ViewFamily::Elements => emit_element_length(source.clone(), next_value, operations),
    };
    let end = end.unwrap_or(length);
    let source = terminal_psi::StructuralArgument {
        path: crate::emission::operation_emission::expressions::path_with_current_bounds(
            &source.path,
            operations.next_identity,
        ),
        ..source
    };
    let producer = operations.allocate();
    let obligation = obligation_id(producer.get().checked_add(1).ok_or(
        LoweringError::Unsupported("view subslice obligation identity overflows"),
    )?);
    let kind = match family {
        ViewFamily::Bytes => OperationKind::ByteSequenceSubslice {
            source,
            start,
            end,
            length,
            obligation,
        },
        ViewFamily::Elements => OperationKind::ElementViewSubslice {
            source,
            start,
            end,
            length,
            obligation,
        },
    };
    operations.push(Operation::unsuspended(
        producer,
        structural_result_declaration(destination, structural_type, multiplicity),
        kind,
    ));
    Ok(StructuralPlaceDeclaration {
        id: destination,
        kind: StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        },
    })
}
