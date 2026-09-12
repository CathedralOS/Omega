//! Declared member value types, without manufacturing a loan or storage origin.

use super::*;
use language_semantics::ReferenceAccess;

pub(super) fn member_matches_reference(
    program: &TypedTrees,
    expression: ExpressionHandle,
    required: TypeReferenceHandle,
) -> bool {
    let mut substitutions = Vec::new();
    let Some(actual) = declared_value_type(program, expression, &mut substitutions) else {
        return false;
    };
    let Some(actual) = substituted_reference(program, actual, &substitutions) else {
        return false;
    };
    let actual_identity = |reference| {
        program.normalized_type_identity_with_binders_and_substitutions(
            reference,
            &[],
            &substitutions,
        )
    };
    let TypeReferenceNode::Reference {
        access: required_access,
        referee: required_referee,
        ..
    } = program.type_reference_table.type_reference(required)
    else {
        return false;
    };
    if let TypeReferenceNode::Reference {
        access: actual_access,
        referee: actual_referee,
        ..
    } = program.type_reference_table.type_reference(actual)
    {
        return actual_identity(actual) == program.normalized_type_identity(required)
            || (*actual_access == ReferenceAccess::Mutable
                && *required_access == ReferenceAccess::Shared
                && actual_identity(*actual_referee)
                    == program.normalized_type_identity(*required_referee));
    }
    // The existing implicit shared borrow of an owned field still needs that
    // field's actual type. Member syntax alone cannot match an arbitrary referee.
    *required_access == ReferenceAccess::Shared
        && (actual_identity(actual) == program.normalized_type_identity(*required_referee)
            || owned_array_projects_to_slice(program, actual, *required_referee, &substitutions))
}

/// Shape correspondence for a shared view, not a loan or a domain proof.
/// Carrier qualifications may be forgotten by a plain view; element identities
/// and qualifications are unchanged, and no required view constraint is inferred.
fn owned_array_projects_to_slice(
    program: &TypedTrees,
    mut actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
) -> bool {
    let TypeReferenceNode::Slice {
        element_type: required_element,
    } = program.type_reference_table.type_reference(required)
    else {
        return false;
    };
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(actual)
    {
        actual = *base_type;
    }
    let TypeReferenceNode::FixedArray {
        element_type: actual_element,
        ..
    } = program.type_reference_table.type_reference(actual)
    else {
        return false;
    };
    program
        .type_reference_table
        .contains_type_reference(*actual_element)
        && program
            .type_reference_table
            .contains_type_reference(*required_element)
        && program.normalized_type_identity_with_binders_and_substitutions(
            *actual_element,
            &[],
            substitutions,
        ) == program.normalized_type_identity(*required_element)
}

fn substituted_reference(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
    substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
) -> Option<TypeReferenceHandle> {
    for _ in 0..=substitutions.len() {
        let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(reference)
        else {
            return Some(reference);
        };
        let Some((_, replacement)) = substitutions
            .iter()
            .find(|(parameter, _)| parameter == symbol)
        else {
            return Some(reference);
        };
        reference = *replacement;
    }
    None
}

fn declared_value_type(
    program: &TypedTrees,
    expression: ExpressionHandle,
    substitutions: &mut Vec<(symbols::SymbolHandle, TypeReferenceHandle)>,
) -> Option<TypeReferenceHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(name) => {
            let [spelling] = program.expression_table.name_path_members(name.members) else {
                return None;
            };
            (name.symbol.is_valid()
                && name.symbol == name.head_symbol
                && program.symbols.name(name.symbol) == spelling.as_str())
            .then(|| named_value_type_reference(program, name))?
        }
        ExpressionNode::Call(call) => crate::calls::resolved_call_result_type(program, call),
        ExpressionNode::Member(member) => {
            if let ExpressionNode::Name(root) = program.expression_table.expression(member.receiver)
                && let Some(machine) = program.machines().iter().find(|machine| {
                    root.head_symbol.is_valid() && machine.symbol == root.head_symbol
                })
            {
                return crate::places::exact_self_field(program, machine, expression)
                    .map(|field| field.type_reference);
            }
            let mut receiver = declared_value_type(program, member.receiver, substitutions)?;
            for _ in 0..=substitutions.len() {
                receiver = crate::places::unwrapped_type_reference(program, receiver)?;
                let TypeReferenceNode::Named { symbol, .. } =
                    program.type_reference_table.type_reference(receiver)
                else {
                    break;
                };
                let Some((_, replacement)) = substitutions
                    .iter()
                    .find(|(parameter, _)| parameter == symbol)
                else {
                    break;
                };
                if *replacement == receiver {
                    return None;
                }
                receiver = *replacement;
            }
            let (symbol, arguments) = match program.type_reference_table.type_reference(receiver) {
                TypeReferenceNode::Named { symbol, .. } => (*symbol, &[][..]),
                TypeReferenceNode::Generic {
                    base_symbol,
                    arguments,
                    ..
                } => (
                    *base_symbol,
                    program
                        .type_reference_table
                        .type_reference_handles(*arguments),
                ),
                _ => return None,
            };
            let mut definitions = program
                .data_definitions()
                .iter()
                .filter(|definition| symbol.is_valid() && definition.symbol == symbol);
            let definition = definitions.next()?;
            if definitions.next().is_some()
                || program.data_type_parameters(definition).len() != arguments.len()
            {
                return None;
            }
            // A payload's authored field type belongs to the data telescope,
            // not its caller's same-spelled binder. Compare it under the exact
            // receiver application without inventing a reference or a loan.
            for (parameter, argument) in program
                .data_type_parameters(definition)
                .iter()
                .zip(arguments)
            {
                if let Some((_, existing)) = substitutions
                    .iter()
                    .find(|(symbol, _)| *symbol == parameter.symbol)
                {
                    // Re-entering this telescope with different arguments needs
                    // scoped type views. This flat comparison cannot safely
                    // rebind earlier argument payloads, so retain rejection.
                    if program.normalized_type_identity_with_binders_and_substitutions(
                        *existing,
                        &[],
                        substitutions,
                    ) != program.normalized_type_identity_with_binders_and_substitutions(
                        *argument,
                        &[],
                        substitutions,
                    ) {
                        return None;
                    }
                } else {
                    substitutions.push((parameter.symbol, *argument));
                }
            }
            crate::places::exact_data_member_field(
                program,
                definition,
                member.member_symbol,
                member.member.as_str(),
                member.case_variant.as_ref().map(|variant| variant.as_str()),
            )
            .map(|field| field.type_reference)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
