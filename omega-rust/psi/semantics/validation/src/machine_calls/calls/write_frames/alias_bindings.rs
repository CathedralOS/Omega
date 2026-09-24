//! Reference-binding replacement admission for write-frame inference.
//!
//! This leaf owns rebinding admission and the mutation of an already-known
//! alias slot, while untracked carrier replacements remain opaque.
//! Recursive origin inference remains in the parent and enters
//! through one callback over an immutable view of the established aliases.

use super::isolation::type_is_caller_isolated_local;
use super::local_aliases::expression_may_rebind_mutable_alias;
use super::place_paths::{FramePlaceOrigin, split_place_root};
use super::type_capabilities::{type_may_carry_write, type_reference_is_reference};
use language_core::is_self_receiver;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::TableAssignment;
use typed_trees::types::TypeReferenceNode;

#[cfg(test)]
mod tests;

/// A primitive parameter assignment observes the RHS referent as a value;
/// it cannot replace the destination's reference carrier. Explicit reference
/// construction and reference-valued calls retain their separate opacity rules.
fn assignment_copies_primitive_referent(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    assignment: &TableAssignment,
) -> bool {
    let ExpressionNode::Name(target) = program.expression_table.expression(assignment.target)
    else {
        return false;
    };
    if target.symbol != target.head_symbol
        || program
            .expression_table
            .name_path_members(target.members)
            .len()
            != 1
        || !matches!(
            program.expression_table.expression(assignment.value),
            ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
        )
    {
        return false;
    }
    let Some(parameter) = program.state_parameters(state).iter().find(|parameter| {
        parameter.symbol == target.symbol && !parameter.is_self && !parameter.is_const
    }) else {
        return false;
    };
    let TypeReferenceNode::Reference {
        access, referee, ..
    } = program
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        return false;
    };
    if !matches!(
        access,
        language_semantics::ReferenceAccess::Mutable
            | language_semantics::ReferenceAccess::WriteOnly
    ) {
        return false;
    }
    let Some(primitive) = program.primitive_type_reference(*referee) else {
        return false;
    };
    let Some(source) = crate::value_custody::places::declared_place_type_raw(
        program,
        machine,
        Some(state),
        assignment.value,
    ) else {
        return false;
    };
    matches!(program.type_reference_table.type_reference(source),
        TypeReferenceNode::Reference { access, referee, .. }
            if access.is_readable() && program.primitive_type_reference(*referee) == Some(primitive))
}

/// Whole-reference transport across a named edge requires the original input
/// binding, not merely a source expression with the same parameter symbol.
pub fn state_reference_parameter_binding_is_stable(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    parameter: symbols::SymbolHandle,
) -> bool {
    if !program.state_parameters(state).iter().any(|candidate| {
        candidate.symbol == parameter
            && type_reference_is_reference(program, candidate.type_reference)
    }) {
        return false;
    }
    let is_binding = |expression| {
        let source = super::FrameSourcePlace::from_expression(program, expression);
        source.root == parameter && source.segments.is_empty()
    };
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .all(|statement| {
            if statement_returns_reference_without_effects(program, state, statement) {
                return true;
            }
            if let typed_trees::statement::StatementNode::Assignment(assignment) = statement
                && is_binding(assignment.target)
                && !assignment_copies_primitive_referent(program, machine, state, assignment)
                && expression_may_rebind_mutable_alias(program, machine, state, assignment.value)
            {
                return false;
            }
            !super::statement_value_expression_roots(program, statement)
                .into_iter()
                .any(|expression| {
                    super::local_aliases::expression_has_exclusive_borrow(
                        program,
                        expression,
                        &is_binding,
                    )
                })
        })
}

/// A pure terminal reference expression transports its referent but cannot
/// replace a binding. Calls inside the result still need the ordinary exposure
/// fence, and stored/discarded expressions are not admitted by this rule.
pub(super) fn statement_returns_reference_without_effects(
    program: &TypedTrees,
    state: &State,
    statement: &typed_trees::statement::StatementNode,
) -> bool {
    let typed_trees::statement::StatementNode::Expression(expression) = statement else {
        return false;
    };
    type_reference_is_reference(program, state.return_type)
        && program
            .statement_table
            .statements(state.statement_nodes)
            .last()
            .is_some_and(|last| std::ptr::eq(last, statement))
        && !super::expression_is_effectful_for_transparent_result(program, *expression)
        && !super::caller_aliases::expression_has_calls(program, *expression)
}

/// Stable local aliases have an explicit replacement transfer. A reference
/// field or carrier does not yet: changing it before a later write invalidates
/// every original argument-leaf substitution, so the body must stay opaque.
pub(super) fn assignment_replaces_untracked_reference(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    assignment: &TableAssignment,
    aliases: &[(String, FramePlaceOrigin)],
) -> bool {
    if assignment_copies_primitive_referent(program, machine, state, assignment) {
        return false;
    }
    // A reference-typed or write-carrying receiver field writes under its own
    // path: every later write through the stored borrow already composes
    // beneath `self.<field>`, so no local-binding splice is needed for the
    // frame to stay finite.
    if super::coarse_place_path(program, assignment.target)
        .is_some_and(|target| is_self_receiver(split_place_root(&target).0))
    {
        return false;
    }
    if super::coarse_place_path(program, assignment.target)
        .is_some_and(|target| aliases.iter().any(|(alias, _)| *alias == target))
    {
        return false;
    }
    let Some(reference) = crate::value_custody::places::declared_place_type_raw(
        program,
        machine,
        Some(state),
        assignment.target,
    ) else {
        return false;
    };
    if type_reference_is_reference(program, reference) {
        // Replacing an untracked read-only local binding cannot redirect a
        // caller write. It remains a private slot write in ordinary summaries;
        // exact-reference demand separately tracks and updates its identity.
        let source = super::FrameSourcePlace::from_expression(program, assignment.target);
        if !type_may_carry_write(program, reference)
            && source.segments.is_empty()
            && program.symbols.get(source.root).kind == symbols::SymbolKind::Local
            && program.symbols.get(source.root).parent == state.symbol
            && super::caller_aliases::caller_binding_type(program, machine, assignment.target)
                .is_some_and(|declared| {
                    type_reference_is_reference(program, declared)
                        && !type_may_carry_write(program, declared)
                })
        {
            return false;
        }
        expression_may_rebind_mutable_alias(program, machine, state, assignment.value)
    } else {
        type_may_carry_write(program, reference)
            && !type_is_caller_isolated_local(program, reference)
    }
}

/// Update one local mutable-reference binding when its replacement has another
/// directly representable origin. Existing aliases retain their already-
/// canonicalized origins, so rebinding an upstream local never redirects a
/// previously established reborrow.
pub(super) fn rebind_stable_local_mutable_alias_origin<ResolveOrigin>(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    target: &str,
    value: ExpressionHandle,
    aliases: &mut [(String, FramePlaceOrigin)],
    resolve_origin: ResolveOrigin,
) -> Option<bool>
where
    ResolveOrigin: FnOnce(&[(String, FramePlaceOrigin)]) -> Option<FramePlaceOrigin>,
{
    let Some(position) = aliases.iter().position(|(alias, _)| alias == target) else {
        return Some(false);
    };
    if !expression_may_rebind_mutable_alias(program, machine, state, value) {
        return Some(false);
    }
    let origin = resolve_origin(aliases)?;
    aliases[position].1 = origin;
    Some(true)
}

/// Check whether a reborrow expression can replace an established local alias
/// without mutating it. The origin callback runs only after exact target and
/// reference-shaped replacement admission, preserving fail-closed ordering.
pub(super) fn stable_local_mutable_alias_rebinding_is_representable<ResolveOrigin>(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    target: &str,
    value: ExpressionHandle,
    aliases: &[(String, FramePlaceOrigin)],
    resolve_origin: ResolveOrigin,
) -> bool
where
    ResolveOrigin: FnOnce(&[(String, FramePlaceOrigin)]) -> Option<FramePlaceOrigin>,
{
    aliases.iter().any(|(alias, _)| alias == target)
        && expression_may_rebind_mutable_alias(program, machine, state, value)
        && resolve_origin(aliases).is_some()
}
