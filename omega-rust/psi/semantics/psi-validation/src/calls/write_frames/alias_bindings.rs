//! Reference-binding replacement admission for write-frame inference.
//!
//! This leaf owns rebinding admission and the mutation of an already-known
//! alias slot, while untracked carrier replacements remain opaque.
//! Recursive origin inference remains in the parent and enters
//! through one callback over an immutable view of the established aliases.

use super::isolation::type_is_caller_isolated_local;
use super::local_aliases::expression_may_rebind_mutable_alias;
use super::place_paths::FramePlaceOrigin;
use super::type_capabilities::{type_may_carry_write, type_reference_is_reference};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::ExpressionHandle;
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;
use psi_typed_trees::statement::TableAssignment;

/// Whole-reference transport across a named edge requires the original input
/// binding, not merely a source expression with the same parameter symbol.
pub fn state_reference_parameter_binding_is_stable(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    parameter: psi_symbols::SymbolHandle,
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
            if let psi_typed_trees::statement::StatementNode::Assignment(assignment) = statement
                && is_binding(assignment.target)
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
    if super::coarse_place_path(program, assignment.target)
        .is_some_and(|target| aliases.iter().any(|(alias, _)| *alias == target))
    {
        return false;
    }
    let Some(reference) =
        crate::places::declared_place_type_raw(program, machine, Some(state), assignment.target)
    else {
        return false;
    };
    if type_reference_is_reference(program, reference) {
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
