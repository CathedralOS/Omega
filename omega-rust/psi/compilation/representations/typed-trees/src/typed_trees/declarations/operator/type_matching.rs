//! Type reference matching under a policy, const binding and expected
//! type parameters.

use crate::TypedTrees;
use crate::data::TypeParameter;
use crate::typed_trees::declarations::operator::OperatorConstBinding;
use crate::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};
use arena::HandleSpan;
use language_semantics::const_value::{CanonicalConstIdentity, CanonicalConstValue};
use symbols::SymbolHandle;

pub(crate) fn type_reference_matches(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    expected: TypeReferenceHandle,
    bindable_owner: Option<SymbolHandle>,
    type_parameters: &[TypeParameter],
    bindings: &mut Vec<(SymbolHandle, TypeReferenceHandle)>,
    const_bindings: &mut Vec<OperatorConstBinding>,
) -> bool {
    type_reference_matches_with_policy(
        program,
        actual,
        expected,
        bindable_owner,
        type_parameters,
        bindings,
        const_bindings,
        true,
    )
}

pub(crate) fn type_reference_matches_with_policy(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    expected: TypeReferenceHandle,
    bindable_owner: Option<SymbolHandle>,
    type_parameters: &[TypeParameter],
    bindings: &mut Vec<(SymbolHandle, TypeReferenceHandle)>,
    const_bindings: &mut Vec<OperatorConstBinding>,
    allow_name_fallback: bool,
) -> bool {
    if !actual.is_valid() || !expected.is_valid() {
        return false;
    }
    if let Some(parameter) =
        expected_type_parameter(program, expected, type_parameters, allow_name_fallback)
        && let crate::data::TypeParameterKind::Const { type_reference } = parameter.kind
    {
        if let Some(value) =
            closed_const_identity_from_type_reference(program, actual, type_reference)
        {
            return bind_operator_const(const_bindings, parameter.symbol, value);
        }
        // Ordinary unresolved generic matching historically permits forwarded
        // symbolic arguments. Exact D29 inference disables name fallback and
        // therefore fails closed instead of treating such a demand as closed.
        if !allow_name_fallback {
            return false;
        }
    }
    if let Some(bindable_symbol) = expected_bindable_symbol(
        program,
        expected,
        bindable_owner,
        type_parameters,
        allow_name_fallback,
    ) {
        if let Some((_, bound_actual)) = bindings
            .iter()
            .find(|(symbol, _)| *symbol == bindable_symbol)
        {
            return type_reference_matches_with_policy(
                program,
                actual,
                *bound_actual,
                None,
                &[],
                &mut Vec::new(),
                &mut Vec::new(),
                allow_name_fallback,
            );
        }
        bindings.push((bindable_symbol, actual));
        return true;
    }

    match (
        program.type_reference_table.type_reference(actual),
        program.type_reference_table.type_reference(expected),
    ) {
        (
            TypeReferenceNode::Reference {
                referee: actual_referee,
                access: actual_access,
                // Lifetimes do not affect operator/conformance type matching.
                lifetime: _,
            },
            TypeReferenceNode::Reference {
                referee: expected_referee,
                access: expected_access,
                lifetime: _,
            },
        ) => {
            actual_access == expected_access
                && type_reference_matches_with_policy(
                    program,
                    *actual_referee,
                    *expected_referee,
                    bindable_owner,
                    type_parameters,
                    bindings,
                    const_bindings,
                    allow_name_fallback,
                )
        }
        (
            TypeReferenceNode::Constrained {
                base_type: actual_base,
                ..
            },
            _,
        ) => type_reference_matches_with_policy(
            program,
            *actual_base,
            expected,
            bindable_owner,
            type_parameters,
            bindings,
            const_bindings,
            allow_name_fallback,
        ),
        (
            _,
            TypeReferenceNode::Constrained {
                base_type: expected_base,
                ..
            },
        ) => type_reference_matches_with_policy(
            program,
            actual,
            *expected_base,
            bindable_owner,
            type_parameters,
            bindings,
            const_bindings,
            allow_name_fallback,
        ),
        (
            TypeReferenceNode::FixedArray {
                element_type: actual_element,
                length: actual_length,
            },
            TypeReferenceNode::FixedArray {
                element_type: expected_element,
                length: expected_length,
            },
        ) => {
            fixed_array_lengths_match(
                program,
                actual_length,
                expected_length,
                type_parameters,
                const_bindings,
                allow_name_fallback,
            ) && type_reference_matches_with_policy(
                program,
                *actual_element,
                *expected_element,
                bindable_owner,
                type_parameters,
                bindings,
                const_bindings,
                allow_name_fallback,
            )
        }
        (
            TypeReferenceNode::Slice {
                element_type: actual_element,
            },
            TypeReferenceNode::Slice {
                element_type: expected_element,
            },
        ) => type_reference_matches_with_policy(
            program,
            *actual_element,
            *expected_element,
            bindable_owner,
            type_parameters,
            bindings,
            const_bindings,
            allow_name_fallback,
        ),
        (
            TypeReferenceNode::Named {
                symbol: actual_symbol,
                ..
            },
            TypeReferenceNode::Generic { .. },
        ) if actual_symbol.is_valid() => program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == *actual_symbol)
            .and_then(|definition| definition.generic_instance)
            .is_some_and(|origin| {
                type_reference_matches_with_policy(
                    program,
                    origin,
                    expected,
                    bindable_owner,
                    type_parameters,
                    bindings,
                    const_bindings,
                    allow_name_fallback,
                )
            }),
        (
            TypeReferenceNode::Named {
                symbol: actual_symbol,
                name: actual_name,
            },
            TypeReferenceNode::Named {
                symbol: expected_symbol,
                name: expected_name,
            },
        ) => nominal_type_identity_matches(
            *actual_symbol,
            actual_name.as_str(),
            *expected_symbol,
            expected_name.as_str(),
            allow_name_fallback,
        ),
        (
            TypeReferenceNode::Generic {
                base_symbol: actual_symbol,
                base_name: actual_name,
                arguments: actual_arguments,
                ..
            },
            TypeReferenceNode::Generic {
                base_symbol: expected_symbol,
                base_name: expected_name,
                arguments: expected_arguments,
                ..
            },
        ) => {
            nominal_type_identity_matches(
                *actual_symbol,
                actual_name.as_str(),
                *expected_symbol,
                expected_name.as_str(),
                allow_name_fallback,
            ) && type_reference_spans_match(
                program,
                *actual_arguments,
                *expected_arguments,
                bindable_owner,
                type_parameters,
                bindings,
                const_bindings,
                allow_name_fallback,
            )
        }
        (TypeReferenceNode::Unit, TypeReferenceNode::Unit) => true,
        _ => false,
    }
}

fn type_reference_spans_match(
    program: &TypedTrees,
    actual: HandleSpan<TypeReferenceHandle>,
    expected: HandleSpan<TypeReferenceHandle>,
    bindable_owner: Option<SymbolHandle>,
    type_parameters: &[TypeParameter],
    bindings: &mut Vec<(SymbolHandle, TypeReferenceHandle)>,
    const_bindings: &mut Vec<OperatorConstBinding>,
    allow_name_fallback: bool,
) -> bool {
    let actual = program.type_reference_table.type_reference_handles(actual);
    let expected = program
        .type_reference_table
        .type_reference_handles(expected);
    actual.len() == expected.len()
        && actual.iter().zip(expected).all(|(actual, expected)| {
            type_reference_matches_with_policy(
                program,
                *actual,
                *expected,
                bindable_owner,
                type_parameters,
                bindings,
                const_bindings,
                allow_name_fallback,
            )
        })
}

fn fixed_array_lengths_match(
    program: &TypedTrees,
    actual: &FixedArrayLength,
    expected: &FixedArrayLength,
    type_parameters: &[TypeParameter],
    const_bindings: &mut Vec<OperatorConstBinding>,
    allow_name_fallback: bool,
) -> bool {
    let FixedArrayLength::ConstParameter { symbol, name } = expected else {
        return actual == expected;
    };
    let Some(parameter) = type_parameters.iter().find(|parameter| {
        (symbol.is_valid() && parameter.symbol == *symbol)
            || (allow_name_fallback && parameter.name == *name)
    }) else {
        return actual == expected;
    };
    let crate::data::TypeParameterKind::Const { type_reference } = parameter.kind else {
        return false;
    };
    let FixedArrayLength::Literal(value) = actual else {
        return allow_name_fallback && actual == expected;
    };
    let Ok(value) = i128::try_from(*value) else {
        return false;
    };
    let Some(primitive) = program.type_reference_table.primitive_type(type_reference) else {
        return false;
    };
    bind_operator_const(
        const_bindings,
        parameter.symbol,
        CanonicalConstIdentity::integer(primitive.name(), value),
    )
}

fn closed_const_identity_from_type_reference(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    declared_carrier: TypeReferenceHandle,
) -> Option<CanonicalConstIdentity> {
    let TypeReferenceNode::Named { name, .. } = program.type_reference_table.type_reference(actual)
    else {
        return None;
    };
    if let Some(value) = CanonicalConstValue::from_atom(name.as_str()) {
        return Some(value.identity());
    }
    let value = name.as_str().parse::<i128>().ok()?;
    let primitive = program
        .type_reference_table
        .primitive_type(declared_carrier)?;
    Some(CanonicalConstIdentity::integer(primitive.name(), value))
}

fn bind_operator_const(
    bindings: &mut Vec<OperatorConstBinding>,
    symbol: SymbolHandle,
    value: CanonicalConstIdentity,
) -> bool {
    if let Some(existing) = bindings.iter().find(|binding| binding.symbol == symbol) {
        return existing.value == value;
    }
    bindings.push(OperatorConstBinding { symbol, value });
    true
}

fn nominal_type_identity_matches(
    actual_symbol: SymbolHandle,
    actual_name: &str,
    expected_symbol: SymbolHandle,
    expected_name: &str,
    allow_name_fallback: bool,
) -> bool {
    if allow_name_fallback {
        return (actual_symbol.is_valid() && actual_symbol == expected_symbol)
            || actual_name == expected_name;
    }
    if actual_symbol.is_valid() || expected_symbol.is_valid() {
        return actual_symbol.is_valid()
            && expected_symbol.is_valid()
            && actual_symbol == expected_symbol;
    }
    matches!(
        (
            crate::types::PrimitiveType::from_name(actual_name),
            crate::types::PrimitiveType::from_name(expected_name),
        ),
        (Some(actual), Some(expected)) if actual == expected
    )
}

fn expected_bindable_symbol(
    program: &TypedTrees,
    expected: TypeReferenceHandle,
    bindable_owner: Option<SymbolHandle>,
    type_parameters: &[TypeParameter],
    allow_name_fallback: bool,
) -> Option<SymbolHandle> {
    let (symbol, _) = match program.type_reference_table.type_reference(expected) {
        TypeReferenceNode::Named { symbol, name }
        | TypeReferenceNode::Generic {
            base_symbol: symbol,
            base_name: name,
            ..
        } => (*symbol, name),
        _ => return None,
    };
    if symbol.is_valid() && Some(symbol) == bindable_owner {
        return Some(symbol);
    }
    expected_type_parameter(program, expected, type_parameters, allow_name_fallback)
        .map(|parameter| parameter.symbol)
}

fn expected_type_parameter<'a>(
    program: &TypedTrees,
    expected: TypeReferenceHandle,
    type_parameters: &'a [TypeParameter],
    allow_name_fallback: bool,
) -> Option<&'a TypeParameter> {
    match program.type_reference_table.type_reference(expected) {
        TypeReferenceNode::Named { symbol, name }
        | TypeReferenceNode::Generic {
            base_symbol: symbol,
            base_name: name,
            ..
        } => type_parameters.iter().find(|parameter| {
            (symbol.is_valid() && parameter.symbol == *symbol)
                || (allow_name_fallback && parameter.name == *name)
        }),
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Constrained { .. }
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Unit => None,
    }
}
