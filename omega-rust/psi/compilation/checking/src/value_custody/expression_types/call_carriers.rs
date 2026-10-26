//! Call-specific carrier correspondence before exact runtime-index checking.

use super::*;

/// Call admission may defer data Value-coordinate equality to the dedicated
/// occurrence index checker. Stores and declaration formation keep the strict
/// shared strict relation. Access, loan authority and all non-Value arguments remain
/// independent obligations.
pub(crate) fn argument_matches_call_carrier(
    program: &dyn crate::CheckingInput,
    scope: SymbolHandle,
    argument: ExpressionHandle,
    required: TypeReferenceHandle,
    required_substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> bool {
    if argument_matches_carrier(program, scope, argument, required, required_substitutions) {
        return true;
    }
    let raw_required = required;
    let (required, required_substitutions) =
        reference_values::substituted_required_reference(program, required, required_substitutions);
    // The independent index checker reads the declaration's telescope. A
    // Type substitution must not introduce an otherwise invisible indexed
    // application into this deferred relation.
    if required != raw_required {
        return false;
    }
    let TypeReferenceNode::Reference {
        referee, access, ..
    } = program.type_reference_table.type_reference(required)
    else {
        return false;
    };
    let borrow = program.expression_table.borrow_expression(argument);
    if borrow.is_some_and(|borrow| borrow.access != *access) {
        return false;
    }
    let source = borrow.map_or(argument, |borrow| borrow.target);
    let mut substitutions = Vec::new();
    let Some(mut actual) =
        reference_values::declared_value_type(program, source, &mut substitutions)
    else {
        return false;
    };
    if let TypeReferenceNode::Reference {
        referee,
        access: actual_access,
        ..
    } = program.type_reference_table.type_reference(actual)
    {
        if actual_access != access {
            return false;
        }
        actual = *referee;
    } else if borrow.is_none() {
        return false;
    }
    call_value_index_carriers_match(
        program,
        actual,
        &substitutions,
        *referee,
        required_substitutions,
        0,
    )
}

fn call_value_index_carriers_match(
    program: &dyn crate::CheckingInput,
    actual: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    required: TypeReferenceHandle,
    required_substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    depth: usize,
) -> bool {
    if depth >= 128 {
        return false;
    }
    if program.type_identity(TypeIdentityRequest {
        substitutions,
        ..TypeIdentityRequest::ordinary(actual)
    }) == program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
        substitutions: required_substitutions,
        ..TypeIdentityRequest::ordinary(required)
    }) {
        return true;
    }
    let Some(actual) = reference_values::substituted_reference(program, actual, substitutions)
    else {
        return false;
    };
    let raw_required = required;
    let (required, required_substitutions) =
        reference_values::substituted_required_reference(program, required, required_substitutions);
    // The independent index checker reads the declaration's telescope. A
    // Type substitution must not introduce an otherwise invisible indexed
    // application into this deferred relation.
    if required != raw_required {
        return false;
    }
    let actual = delivered_types::application_form(program, actual);
    let required = delivered_types::application_form(program, required);
    let (
        TypeReferenceNode::Generic {
            base_symbol: left_base,
            arguments: left,
            lifetime_arguments: left_lifetimes,
            ..
        },
        TypeReferenceNode::Generic {
            base_symbol: right_base,
            arguments: right,
            lifetime_arguments: right_lifetimes,
            ..
        },
    ) = (
        program.type_reference_table.type_reference(actual),
        program.type_reference_table.type_reference(required),
    )
    else {
        return false;
    };
    // Lifetime-bearing applications retain the existing exact correspondence.
    // This fallback grants no new region relation.
    if left_base != right_base || !left_lifetimes.is_empty() || !right_lifetimes.is_empty() {
        return false;
    }
    let Some(data) = program.data_definition_by_symbol(*left_base) else {
        return false;
    };
    let parameters = program.data_type_parameters(data);
    let left = program.type_reference_table.type_reference_handles(*left);
    let right = program.type_reference_table.type_reference_handles(*right);
    parameters.len() == left.len()
        && left.len() == right.len()
        && parameters
            .iter()
            .zip(left.iter().zip(right))
            .all(|(parameter, (left, right))| match parameter.kind {
                typed_trees::data::TypeParameterKind::Value { .. } => true,
                typed_trees::data::TypeParameterKind::Type => call_value_index_carriers_match(
                    program,
                    *left,
                    substitutions,
                    *right,
                    required_substitutions,
                    depth + 1,
                ),
                _ => {
                    program.type_identity(TypeIdentityRequest {
                        substitutions,
                        ..TypeIdentityRequest::ordinary(*left)
                    }) == program.type_identity_with_simultaneous_substitutions(
                        TypeIdentityRequest {
                            substitutions: required_substitutions,
                            ..TypeIdentityRequest::ordinary(*right)
                        },
                    )
                }
            })
}
