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
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::expressions::{
    LoweredDirectExpression, emit_byte_length, emit_direct_expression, emit_element_length,
};
use crate::emission::scalar_types::terminal_scalar_type;
use crate::lowering_error::{LoweringError, unsupported};
use crate::terminal_identities::obligation_id;
use checked_trees::types::PrimitiveType;
use semantic_vocabulary::{IntegerValue, PlaceId, StructuralPlaceKind, StructuralTypeId, ValueId};
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralMultiplicity, StructuralOperationResult,
    StructuralPlaceDeclaration, ValueDeclaration,
};

/// Which Terminal view family the range narrows: byte views count bytes,
/// element views count elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewFamily {
    Bytes,
    Elements,
}

/// Emit `source[start..end]` into `destination`. A missing start is zero and
/// a missing end is the source's own length; present endpoints must already
/// be branch-free `u64` expressions over `values`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit(
    family: ViewFamily,
    source: PlaceId,
    start: Option<&LoweredDirectExpression>,
    end: Option<&LoweredDirectExpression>,
    destination: PlaceId,
    structural_type: StructuralTypeId,
    values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let count_type = terminal_scalar_type(PrimitiveType::U64)?;
    let types = values
        .iter()
        .map(|value| value.scalar_type)
        .collect::<Vec<_>>();
    let mut endpoint = |expression: &LoweredDirectExpression| -> Result<ValueId, LoweringError> {
        if expression.scalar_type() != count_type
            || direct_expression_contains_short_circuit(expression)
        {
            return unsupported("view subslice endpoint needs a branch-free u64 value");
        }
        validate_direct_parameter_types(expression, &types)?;
        Ok(emit_direct_expression(
            expression, values, next_value, operations,
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
        ViewFamily::Bytes => emit_byte_length(source, next_value, operations),
        ViewFamily::Elements => emit_element_length(source, next_value, operations),
    };
    let end = end.unwrap_or(length);
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
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: producer,
        result: OperationResult::Structural(StructuralOperationResult {
            qualification_establishments: Vec::new(),
            place: destination,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind,
    });
    Ok(StructuralPlaceDeclaration {
        id: destination,
        kind: StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        },
    })
}
