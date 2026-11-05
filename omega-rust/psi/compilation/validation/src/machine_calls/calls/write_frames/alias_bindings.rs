//! Reference-binding reaccess admission for write-frame inference.
//!
//! This leaf checks which reference bindings still name their original input.
//! Replacement values are captured and installed by stored-origin transfer.

use super::isolation::type_is_caller_isolated_local;
use super::place_paths::{FramePlaceOrigin, split_place_root};
use super::type_capabilities::{type_may_carry_write, type_reference_is_reference};
use crate::frozen_program::expression_table;
use language_core::is_self_receiver;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::TableAssignment;

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
    // Reborrowing the selected referent does not expose the descriptor slot.
    // Writes through it retire value facts but cannot change this invocation's
    // storage identity. Nested reference storage retains the independent
    // operand-exposure fence below.
    let source_referee =
        crate::reference_shell(program, declaration.type_reference).map(|shell| shell.referee);
    let is_binding = |expression| {
        let source = super::FrameSourcePlace::from_expression(program, expression);
        (source.root == parameter
            || (source.root.is_valid()
                && program.symbols.authored_origin(source.root) == parameter))
            && source.segments.is_empty()
    };
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
        .all(|(statement_index, statement)| {
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
            // Named transition arguments have the same receiving contract as
            // other calls. Keep that contract when examining their borrows;
            // a generic expression walk would discard it at the arm boundary.
            if crate::collect_call_sites_in_statement(program, machine, state, statement_index)
                .into_iter()
                .filter(|site| !matches!(site, crate::CallSite::Expression { .. }))
                .any(|site| {
                    call_arguments_expose_reference_slot(
                        program,
                        machine.symbol,
                        site,
                        source_referee,
                        &is_binding,
                    )
                })
            {
                return false;
            }
            !program
                .tables
                .expression_roots(statement)
                .into_iter()
                .filter(|(role, _)| {
                    !matches!(
                        role,
                        typed_trees::statement::StatementRootRole::CallArgument
                            | typed_trees::statement::StatementRootRole::TransitionArgument
                    )
                })
                .any(|(_, expression)| {
                    expression_exposes_reference_slot(
                        program,
                        machine.symbol,
                        expression,
                        None,
                        source_referee,
                        &is_binding,
                    )
                })
        })
}

fn call_arguments_expose_reference_slot<'program>(
    program: &'program dyn crate::CheckingInput,
    lexical_owner: symbols::SymbolHandle,
    site: crate::CallSite<'program>,
    source_referee: Option<typed_trees::types::TypeReferenceHandle>,
    is_binding: &impl Fn(ExpressionHandle) -> bool,
) -> bool {
    let parameters = site.parameters(program, lexical_owner);
    let leading = super::super::callee_entry_contract(program, site.target_symbol(program))
        .map_or(0, |contract| contract.leading_operands);
    crate::call_site_argument_expressions(program, &site)
        .iter()
        .enumerate()
        .any(|(position, argument)| {
            let expected = position
                .checked_sub(leading)
                .and_then(|position| parameters?.get(position))
                .map(|parameter| parameter.type_reference);
            expression_exposes_reference_slot(
                program,
                lexical_owner,
                *argument,
                expected,
                source_referee,
                is_binding,
            )
        })
}

/// Distinguish lending a referent from lending the reference slot that
/// points at it. The selected formal must name that same referent;
/// an unknown consumer or a reference-to-reference formal keeps the exposure
/// fence. Nested operands are checked under their own receiving declarations.
fn expression_exposes_reference_slot(
    program: &dyn crate::CheckingInput,
    lexical_owner: symbols::SymbolHandle,
    expression: ExpressionHandle,
    expected: Option<typed_trees::types::TypeReferenceHandle>,
    source_referee: Option<typed_trees::types::TypeReferenceHandle>,
    is_binding: &impl Fn(ExpressionHandle) -> bool,
) -> bool {
    if !expression_table(program).expression_is_valid(expression) {
        return false;
    }
    match expression_table(program).expression(expression) {
        ExpressionNode::Borrow(borrow)
            if borrow.access.is_exclusive() && is_binding(borrow.target) =>
        {
            let receiving = expected
                .and_then(|expected| crate::reference_shell(program, expected))
                .map(|shell| shell.referee);
            !source_referee
                .zip(receiving)
                .is_some_and(|(source, receiving)| {
                    let source = program.normalized_type_identity(source);
                    source.is_valid() && source == program.normalized_type_identity(receiving)
                })
        }
        ExpressionNode::Call(call) => {
            if expression_exposes_reference_slot(
                program,
                lexical_owner,
                call.receiver,
                None,
                source_referee,
                is_binding,
            ) {
                return true;
            }
            call_arguments_expose_reference_slot(
                program,
                lexical_owner,
                crate::CallSite::Expression { expression, call },
                source_referee,
                is_binding,
            )
        }
        node => {
            let mut exposed = false;
            crate::queries::expression_traversal::children(program, node, |child| {
                exposed |= expression_exposes_reference_slot(
                    program,
                    lexical_owner,
                    child,
                    None,
                    source_referee,
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

/// Identify stored reference or carrier contents that an assignment replaces.
/// The stored-origin transfer captures their new contents before RHS effects.
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
