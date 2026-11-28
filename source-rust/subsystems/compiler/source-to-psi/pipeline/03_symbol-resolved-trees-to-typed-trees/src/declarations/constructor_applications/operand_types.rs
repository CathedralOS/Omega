//! Infer constructor and call arguments from the corresponding operand types.
//! Exposing a callable's parameter and result types does not grant its effects.
use super::{
    Diagnostic, FixedArrayLength, SymbolHandle, TypeReferenceHandle, TypeReferenceNode, TypedTrees,
    integer_argument,
};
use crate::source_state::signature::{StateParameter, StateSignature};

/// The same input and result positions determine type arguments whether the
/// operand is a machine declaration or an already-typed callable value.
pub(super) fn signature_operands(
    program: &TypedTrees,
    required: &StateSignature,
    actual: arena::RootRows<'_, StateParameter>,
    result: TypeReferenceHandle,
) -> Vec<(TypeReferenceHandle, TypeReferenceHandle)> {
    let inputs = program.state_signature_parameters(required);
    if inputs.len() != actual.len() {
        return Vec::new();
    }
    let mut pairs: Vec<_> = inputs
        .iter()
        .zip(actual.iter())
        .map(|(formal, actual)| (formal.type_reference, actual.type_reference))
        .collect();
    if required.return_type.is_valid() && result.is_valid() {
        pairs.push((required.return_type, result));
    }
    pairs
}

pub(super) fn unify(
    program: &mut TypedTrees,
    formal: TypeReferenceHandle,
    actual: TypeReferenceHandle,
    parameters: &[crate::source_state::data::TypeParameter],
    inferred: &mut [Option<TypeReferenceHandle>],
) -> Result<(), Diagnostic> {
    let mut pending = vec![(formal, actual)];
    let mut visited_calls = Vec::new();
    while let Some((formal, actual)) = pending.pop() {
        let types = &program.type_reference_table;
        match (types.type_reference(formal), types.type_reference(actual)) {
            (TypeReferenceNode::Named { symbol, .. }, _) => {
                bind(program, *symbol, actual, parameters, inferred)?;
            }
            (
                TypeReferenceNode::Generic {
                    base_symbol: formal,
                    arguments: left,
                    ..
                },
                TypeReferenceNode::Generic {
                    base_symbol: actual,
                    arguments: right,
                    ..
                },
            ) if formal == actual => {
                let left = types.type_reference_handles(*left);
                let right = types.type_reference_handles(*right);
                if left.len() == right.len() {
                    pending.extend(left.iter().copied().zip(right.iter().copied()).rev());
                }
            }
            (
                TypeReferenceNode::FixedArray {
                    element_type: left,
                    length,
                },
                TypeReferenceNode::FixedArray {
                    element_type: right,
                    ..
                },
            ) => {
                pending.push((*left, *right));
                if let FixedArrayLength::ConstParameter { symbol, .. } = length
                    && let Some(length) = program.closed_fixed_array_length(actual)
                {
                    let symbol = *symbol;
                    let argument = integer_argument(program, length);
                    bind(program, symbol, argument, parameters, inferred)?;
                }
            }
            (
                TypeReferenceNode::Reference { referee: left, .. },
                TypeReferenceNode::Reference { referee: right, .. },
            ) => pending.push((*left, *right)),
            (
                TypeReferenceNode::Callable { contract: formal },
                TypeReferenceNode::Callable { contract: actual },
            ) => {
                // A recursive callable type can refer to this pair again.
                let pair = (*formal, *actual);
                if visited_calls.contains(&pair) {
                    continue;
                }
                visited_calls.push(pair);
                let required = &program
                    .declared_callable_contract(*formal)
                    .map_err(Diagnostic::error)?
                    .signature;
                let actual = &program
                    .declared_callable_contract(*actual)
                    .map_err(Diagnostic::error)?
                    .signature;
                let pairs = signature_operands(
                    program,
                    required,
                    program.state_signature_parameters(actual),
                    actual.return_type,
                );
                pending.extend(pairs.into_iter().rev());
            }
            _ => {}
        }
    }
    Ok(())
}

fn bind(
    program: &TypedTrees,
    symbol: SymbolHandle,
    actual: TypeReferenceHandle,
    parameters: &[crate::source_state::data::TypeParameter],
    inferred: &mut [Option<TypeReferenceHandle>],
) -> Result<(), Diagnostic> {
    let Some(position) = parameters
        .iter()
        .position(|parameter| parameter.symbol == symbol)
    else {
        return Ok(());
    };
    if let Some(previous) = inferred[position]
        && program.normalized_type_identity(previous) != program.normalized_type_identity(actual)
    {
        return Err(Diagnostic::error(
            "constructor or call arguments determine conflicting generic arguments",
        ));
    }
    inferred[position] = Some(actual);
    Ok(())
}
