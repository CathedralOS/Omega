//! Reference-binding replacement admission for write-frame inference.
//!
//! This leaf owns rebinding admission and the mutation of an already-known
//! alias slot, while untracked carrier replacements remain opaque.
//! Recursive origin inference remains in the parent and enters
//! through one callback over an immutable view of the established aliases.

use super::isolation::type_is_caller_isolated_local;
use super::local_aliases::expression_may_rebind_mutable_alias;
use super::place_paths::{FramePlaceOrigin, coarse_place_path, split_place_root};
use super::type_capabilities::{type_may_carry_write, type_reference_is_reference};
use language_core::is_self_receiver;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableAssignment};

/// Whole-reference transport across a named edge requires the original input
/// binding, not merely a source expression with the same parameter symbol.
pub fn state_reference_parameter_binding_is_stable(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    parameter: symbols::SymbolHandle,
) -> bool {
    let Some(declaration) = program.state_parameters(state).iter().find(|candidate| {
        candidate.symbol == parameter
            && type_reference_is_reference(program, candidate.type_reference)
    }) else {
        return false;
    };
    // A scalar reborrow exposes the referent, not the descriptor slot.
    // Writes through it retire value facts but cannot change this invocation's
    // storage identity. Nested reference storage retains the independent
    // operand-exposure fence below.
    let scalar_referent = crate::reference_shell(program, declaration.type_reference)
        .and_then(|shell| program.primitive_type_reference(shell.referee));
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
            // Replacing the parameter's own descriptor (a `mut` binding given
            // another view) leaves no exact incoming origin at the exit. The
            // shared destination-ownership query decides it, so a write
            // through a non-`mut` borrowed parameter keeps its identity.
            if let Some(assignment) = statement.as_assignment()
                && is_binding(assignment.target)
                && super::reference_subjects::reference_value_replaces_binding(
                    program,
                    machine,
                    state,
                    declaration.type_reference,
                    assignment.target,
                    assignment.value,
                )
            {
                return false;
            }
            if let Some(call) = statement.as_call() {
                let contract = super::super::callee_entry_contract(program, call.target_symbol);
                return program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .enumerate()
                    .all(|(position, argument)| {
                        let expected = contract.as_ref().and_then(|contract| {
                            let position = position.checked_sub(contract.leading_operands)?;
                            Some(contract.parameters.get(position)?.type_reference)
                        });
                        !expression_exposes_reference_slot(
                            program,
                            *argument,
                            expected,
                            scalar_referent,
                            &is_binding,
                        )
                    });
            }
            !super::statement_value_expression_roots(program, statement)
                .into_iter()
                .any(|expression| {
                    expression_exposes_reference_slot(
                        program,
                        expression,
                        None,
                        scalar_referent,
                        &is_binding,
                    )
                })
        })
}

/// Distinguish lending a scalar referent from lending the reference slot that
/// points at it. Only a selected scalar-reference formal establishes the former;
/// an unknown consumer or a reference-to-reference formal keeps the exposure
/// fence. Nested operands are checked under their own receiving declarations.
fn expression_exposes_reference_slot(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    expected: Option<typed_trees::types::TypeReferenceHandle>,
    scalar_referent: Option<typed_trees::types::PrimitiveType>,
    is_binding: &impl Fn(ExpressionHandle) -> bool,
) -> bool {
    if !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(borrow)
            if borrow.access.is_exclusive() && is_binding(borrow.target) =>
        {
            let receiving_scalar = expected
                .and_then(|expected| crate::reference_shell(program, expected))
                .and_then(|shell| program.primitive_type_reference(shell.referee));
            scalar_referent.is_none() || scalar_referent != receiving_scalar
        }
        ExpressionNode::Call(call) => {
            if expression_exposes_reference_slot(
                program,
                call.receiver,
                None,
                scalar_referent,
                is_binding,
            ) {
                return true;
            }
            let contract = super::super::callee_entry_contract(program, call.target_symbol);
            program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .enumerate()
                .any(|(position, argument)| {
                    let expected = contract.as_ref().and_then(|contract| {
                        let position = position.checked_sub(contract.leading_operands)?;
                        Some(contract.parameters.get(position)?.type_reference)
                    });
                    expression_exposes_reference_slot(
                        program,
                        *argument,
                        expected,
                        scalar_referent,
                        is_binding,
                    )
                })
        }
        node => {
            let mut exposed = false;
            crate::expression_traversal::children(program, node, |child| {
                exposed |= expression_exposes_reference_slot(
                    program,
                    child,
                    None,
                    scalar_referent,
                    is_binding,
                );
            });
            exposed
        }
    }
}

/// A pure terminal reference expression transports its referent but cannot
/// replace a binding. Calls inside the result still need the ordinary exposure
/// fence, and stored/discarded expressions are not admitted by this rule.
pub(super) fn statement_returns_reference_without_effects(
    program: &dyn crate::CheckingInput,
    state: &State,
    statement: &typed_trees::statement::StatementNode,
) -> bool {
    let Some(expression) = statement.as_expression() else {
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
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    assignment: &TableAssignment,
    aliases: &[(String, FramePlaceOrigin)],
) -> bool {
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
        super::reference_subjects::reference_value_replaces_binding(
            program,
            machine,
            state,
            reference,
            assignment.target,
            assignment.value,
        )
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
    program: &dyn crate::CheckingInput,
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

/// The shared direct-place mutable-alias rebind used by the assignment
/// walks: `Some(Some(relative))` means the stable alias record replaced the
/// write and carries the rebound relative path (callers may still record
/// it); `Some(None)` leaves the ordinary target path to resolve; `None`
/// fails the walk closed.
pub(super) fn rebind_direct_assignment_target<ResolveOrigin>(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    assignment: &TableAssignment,
    aliases: &mut [(String, FramePlaceOrigin)],
    resolve_origin: ResolveOrigin,
) -> Option<Option<String>>
where
    ResolveOrigin: FnOnce(&[(String, FramePlaceOrigin)], &str) -> Option<FramePlaceOrigin>,
{
    let Some(relative) = coarse_place_path(program, assignment.target) else {
        return Some(None);
    };
    rebind_stable_local_mutable_alias_origin(
        program,
        machine,
        state,
        &relative,
        assignment.value,
        aliases,
        |established| resolve_origin(established, &relative),
    )
    .map(|rebound| rebound.then_some(relative))
}

/// Check whether a reborrow expression can replace an established local alias
/// without mutating it. The origin callback runs only after exact target and
/// reference-shaped replacement admission, preserving fail-closed ordering.
pub(super) fn stable_local_mutable_alias_rebinding_is_representable<ResolveOrigin>(
    program: &dyn crate::CheckingInput,
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

/// Whether `statement` is an assignment whose target a stable local mutable
/// alias may rebind to; `resolve_origin` supplies the alias's initializer
/// origin from the established alias table.
pub(super) fn representable_assignment_alias_rebinding<ResolveOrigin>(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement: &StatementNode,
    aliases: &[(String, FramePlaceOrigin)],
    resolve_origin: ResolveOrigin,
) -> bool
where
    ResolveOrigin:
        FnOnce(&[(String, FramePlaceOrigin)], ExpressionHandle) -> Option<FramePlaceOrigin>,
{
    match statement {
        StatementNode::Assignment(assignment) => coarse_place_path(program, assignment.target)
            .is_some_and(|target| {
                stable_local_mutable_alias_rebinding_is_representable(
                    program,
                    machine,
                    state,
                    &target,
                    assignment.value,
                    aliases,
                    |aliases| resolve_origin(aliases, assignment.value),
                )
            }),
        _ => false,
    }
}
