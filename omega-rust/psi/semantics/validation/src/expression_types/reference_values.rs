//! Declared projected reference values share exact root and field selection.
//!
//! Type matching does not manufacture a loan. Forwarding additionally anchors
//! the current-state binding and checks every enclosing reference permission;
//! a mutable leaf behind shared storage cannot authorize an exclusive call.

use super::*;
use language_semantics::ReferenceAccess;

/// Reference correspondence does not form a loan. Array-to-slice adaptation
/// retains element identity and readable access; callers still establish the
/// original backing, extent and permission. Write-only attenuation is explicit.
pub(super) fn reference_type_matches(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
) -> bool {
    let TypeReferenceNode::Reference {
        access: actual_access,
        referee: actual_referee,
        ..
    } = program.type_reference_table.type_reference(actual)
    else {
        return false;
    };
    let TypeReferenceNode::Reference {
        access: required_access,
        referee: required_referee,
        ..
    } = program.type_reference_table.type_reference(required)
    else {
        return false;
    };
    let actual_identity = |reference| {
        program.normalized_type_identity_with_binders_and_substitutions(
            reference,
            &[],
            substitutions,
        )
    };
    if actual_identity(actual) == program.normalized_type_identity(required) {
        return true;
    }
    if *actual_access == ReferenceAccess::WriteOnly
        || *required_access == ReferenceAccess::WriteOnly
        || (*actual_access != *required_access
            && !(*actual_access == ReferenceAccess::Mutable
                && *required_access == ReferenceAccess::Shared))
    {
        return false;
    }
    actual_identity(*actual_referee) == program.normalized_type_identity(*required_referee)
        // A shared view may forget a carrier predicate without changing its
        // contents. A raw mutable view could violate that predicate; retain
        // rejection until its invariant-window obligations are established.
        || ((*required_access == ReferenceAccess::Shared
            || matches!(
                program.type_reference_table.type_reference(*actual_referee),
                TypeReferenceNode::FixedArray { .. }
            ))
            && owned_array_projects_to_slice(
                program,
                *actual_referee,
                *required_referee,
                substitutions,
            ))
}

pub(super) fn projected_matches_reference(
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
    if matches!(
        program.type_reference_table.type_reference(actual),
        TypeReferenceNode::Reference { .. }
    ) {
        return reference_type_matches(program, actual, required, &substitutions);
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
    selected_value_type(program, expression, substitutions, None)
}

/// A known current-state root supplies identity, not a new loan. Intermediate
/// reference permissions constrain forwarding independently of the leaf type.
pub(crate) fn place_forwards_mutable_reference(
    program: &TypedTrees,
    expression: ExpressionHandle,
    root: symbols::SymbolHandle,
    root_type: TypeReferenceHandle,
) -> bool {
    let mut substitutions = Vec::new();
    let Some(selected) = selected_value_type(
        program,
        expression,
        &mut substitutions,
        Some((root, root_type)),
    ) else {
        return false;
    };
    let Some(mut selected) = substituted_reference(program, selected, &substitutions) else {
        return false;
    };
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(selected)
    {
        selected = *base_type;
    }
    matches!(
        program.type_reference_table.type_reference(selected),
        TypeReferenceNode::Reference {
            access: ReferenceAccess::Mutable,
            ..
        }
    )
}

fn projection_receiver_type(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
    substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
    require_mutable_access: bool,
) -> Option<TypeReferenceHandle> {
    for _ in 0..128 {
        reference = substituted_reference(program, reference, substitutions)?;
        reference = match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => *base_type,
            TypeReferenceNode::Reference {
                referee, access, ..
            } => {
                if require_mutable_access && *access != ReferenceAccess::Mutable {
                    return None;
                }
                *referee
            }
            _ => return Some(reference),
        };
    }
    None
}

fn selected_value_type(
    program: &TypedTrees,
    expression: ExpressionHandle,
    substitutions: &mut Vec<(symbols::SymbolHandle, TypeReferenceHandle)>,
    forwarding_root: Option<(symbols::SymbolHandle, TypeReferenceHandle)>,
) -> Option<TypeReferenceHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(name) => {
            let [spelling] = program.expression_table.name_path_members(name.members) else {
                return None;
            };
            if !(name.symbol.is_valid() && name.symbol == name.head_symbol) {
                return None;
            }
            if let Some((root, reference)) = forwarding_root {
                // The contextual owner already checked this root, including
                // `self`, whose machine symbol need not be spelled `self`.
                return (root == name.symbol).then_some(reference);
            }
            (program.symbols.name(name.symbol) == spelling.as_str())
                .then(|| named_value_type_reference(program, name))?
        }
        ExpressionNode::Call(call) if forwarding_root.is_none() => {
            crate::calls::resolved_call_result_type(program, call)
        }
        ExpressionNode::Member(member) => {
            if forwarding_root.is_none()
                && let ExpressionNode::Name(root) =
                    program.expression_table.expression(member.receiver)
                && let Some(machine) = program.machines().iter().find(|machine| {
                    root.head_symbol.is_valid() && machine.symbol == root.head_symbol
                })
            {
                return crate::places::exact_self_field(program, machine, expression)
                    .map(|field| field.type_reference);
            }
            let receiver =
                selected_value_type(program, member.receiver, substitutions, forwarding_root)?;
            let receiver = projection_receiver_type(
                program,
                receiver,
                substitutions,
                forwarding_root.is_some(),
            )?;
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
        ExpressionNode::Indexed(indexed) => {
            if matches!(
                program.expression_table.expression(indexed.index),
                ExpressionNode::Range(_)
            ) {
                return None;
            }
            let collection =
                selected_value_type(program, indexed.collection, substitutions, forwarding_root)?;
            // Selection needs the applied collection, not a raw field
            // telescope whose substituted elements could match another
            // authored operator. This query does not materialize types.
            if !substitutions.is_empty()
                && program.normalized_type_identity(collection)
                    != program.normalized_type_identity_with_binders_and_substitutions(
                        collection,
                        &[],
                        substitutions,
                    )
            {
                return None;
            }
            let index_type = declared_value_type(program, indexed.index, &mut Vec::new());
            let meaning_is_builtin =
                if let Some(machine) = place_machine_symbol(program, indexed.collection) {
                    typed_trees::operator::has_builtin_spelled_expression_meaning(
                        program,
                        machine,
                        expression,
                        language_core::OperatorSpelling::Index,
                        &[Some(collection), index_type],
                    )
                } else {
                    crate::places::has_retained_builtin_index_meaning(program, expression)
                };
            if !meaning_is_builtin
                || !typed_trees::operator::resolve_indexed_spelling_for_operands(
                    program,
                    language_core::OperatorSpelling::Index,
                    &[Some(collection), index_type],
                )
                .is_empty()
            {
                return None;
            }
            let collection = projection_receiver_type(
                program,
                collection,
                substitutions,
                forwarding_root.is_some(),
            )?;
            match program.type_reference_table.type_reference(collection) {
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => Some(*element_type),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Statement projections can precede final operator receipts. Their exact
/// named root still identifies the selection telescope for ordinary builtin
/// meaning reconstruction; no spelling lookup or default machine is allowed.
fn place_machine_symbol(
    program: &TypedTrees,
    mut expression: ExpressionHandle,
) -> Option<symbols::SymbolHandle> {
    for _ in 0..128 {
        match program.expression_table.expression(expression) {
            ExpressionNode::Member(member) => expression = member.receiver,
            ExpressionNode::Indexed(indexed) => expression = indexed.collection,
            ExpressionNode::Name(path)
                if path.symbol.is_valid() && path.symbol == path.head_symbol =>
            {
                let mut symbol = path.symbol;
                for _ in 0..128 {
                    if program
                        .machines()
                        .iter()
                        .any(|machine| machine.symbol == symbol)
                    {
                        return Some(symbol);
                    }
                    let parent = program.symbols.get(symbol).parent;
                    if !parent.is_valid() || parent == symbol {
                        return None;
                    }
                    symbol = parent;
                }
                return None;
            }
            _ => return None,
        }
    }
    None
}

#[cfg(test)]
mod tests;
