//! Membership and sum-pattern relabeling.
use super::super::{
    ExpressionNode, HandleSpan, HashSet, Identifier, SyntaxTrees, TypeReferenceHandle,
    TypeReferenceNode,
};
use crate::preparation::generic_data::uses::{
    closed_constructor_carrier, collect_statement_expression_handles, concrete_state_local_types,
    constructor_carrier_span, constructor_path_name, expected_instance, selected_constructor,
};

use crate::preparation::generic_data::Instantiation;

use crate::preparation::declaration_selection;

/// Destructure syntax lowers to `subject in Base::Case` before this pass. When
/// the subject is a state parameter or local with an exact synthesized type,
/// that annotation selects the corresponding closed case identity even when
/// another closed instance of the same generic sum exists in the program.
pub(in crate::preparation::generic_data) fn relabel_closed_sum_memberships_from_local_types(
    syntax: &mut SyntaxTrees,
    instances: &[Instantiation],
    selection: Option<&declaration_selection::ConstantSelection>,
    declarations: &[syntax_trees::item::ItemHandle],
) {
    for (_state, local_types, statements) in concrete_state_local_types(syntax, declarations) {
        let mut reachable = HashSet::new();
        for statement in statements {
            collect_statement_expression_handles(syntax, statement, &mut reachable);
        }
        // Collection already identifies this state's work. Scanning the whole
        // expression arena again makes each unrelated state multiply preparation
        // cost. Retain arena order so synthesized paths stay deterministic.
        let mut reachable = reachable.into_iter().collect::<Vec<_>>();
        reachable.sort_unstable_by_key(|handle| (handle.generation(), handle.arena_index()));
        let replacements = reachable
            .into_iter()
            .filter_map(|handle| {
                let expression = syntax.expressions.expression(handle);
                let ExpressionNode::Membership(membership) = expression else {
                    return None;
                };
                let closed = match syntax.expressions.expression(membership.value) {
                    ExpressionNode::Name(subject_path) => {
                        let [subject] = syntax.expressions.identifier_path_members(*subject_path)
                        else {
                            return None;
                        };
                        local_types.get(subject.as_str(syntax))?
                    }
                    _ => return None,
                };
                // Membership observes the referent's case, not the reference
                // carrier. Keep constructor expected types strict; only this
                // observation peels access and qualification shells before
                // selecting the retained generic application. Counting other
                // instances cannot supply this subject's identity.
                let closed = membership_subject_type(syntax, *closed);
                if !closed.is_valid() {
                    return None;
                }
                let instance = expected_instance(syntax, instances, selection, closed)?;
                // Destructure lowering currently retains the two-part case path.
                let [_, _] = syntax
                    .expressions
                    .identifier_path_members(membership.domain)
                else {
                    return None;
                };
                let name = constructor_path_name(syntax, membership.domain)?;
                let (owner, case) = selected_constructor(syntax, selection, &name)?;
                if owner != instance.template {
                    return None;
                }
                let case = case?;
                let closed = closed_constructor_carrier(syntax, selection, instance, &name, true)?;
                Some((
                    handle,
                    membership.value,
                    closed,
                    constructor_carrier_span(syntax, membership.domain)?,
                    case,
                ))
            })
            .collect::<Vec<_>>();
        for (handle, value, closed, carrier_span, case) in replacements {
            let domain = closed_sum_path(syntax, closed, carrier_span, case);
            syntax.expressions.replace_expression(
                handle,
                ExpressionNode::Membership(syntax_trees::expression::TableMembershipExpression {
                    value,
                    domain,
                }),
            );
        }
    }
}

fn membership_subject_type(
    syntax: &SyntaxTrees,
    mut reference: TypeReferenceHandle,
) -> TypeReferenceHandle {
    for _ in 0..syntax.type_references.type_reference_count() {
        if !reference.is_valid() {
            return reference;
        }
        reference = match syntax.type_references.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. } => *referee,
            TypeReferenceNode::Constrained { base_type, .. } => *base_type,
            _ => return reference,
        };
    }
    TypeReferenceHandle::default()
}

pub(in crate::preparation::generic_data) fn named_type_name(
    syntax: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
) -> Option<String> {
    let type_reference = match syntax.tables.type_references.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => *base_type,
        TypeReferenceNode::Named(_) => type_reference,
        _ => return None,
    };
    match syntax.tables.type_references.type_reference(type_reference) {
        TypeReferenceNode::Named(name) => Some(name.as_str(syntax).to_owned()),
        _ => None,
    }
}

pub(in crate::preparation::generic_data) fn closed_sum_path(
    syntax: &mut SyntaxTrees,
    closed: Identifier,
    carrier_span: source::SourceSpan,
    case: Identifier,
) -> HandleSpan<Identifier> {
    let mut path = HandleSpan::empty();
    // Specialization changes the selected carrier, not its authored occurrence.
    // Membership checking rejoins this carrier receipt with the case receipt.
    let closed = Identifier::new(closed.as_str(syntax), carrier_span);
    syntax
        .expressions
        .append_identifier_path_member_to_span(&mut path, closed);
    syntax
        .expressions
        .append_identifier_path_member_to_span(&mut path, case);
    path
}
