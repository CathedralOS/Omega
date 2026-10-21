//! Route discharge for named `Namespace::requirement(...)` operator calls.
//!
//! A named use selects the same crash contract as a spelled use. The flow
//! pass captures its operand-time evidence under a `FlowOperatorInvocationFact`
//! keyed by the `named_use` handle rather than a `uses` row, so the
//! operand-time `InvocationContexts` discharge in checks/operators applies
//! unchanged: short-circuit premises, per-operand snapshots for copied scalars
//! and stable by-value carriers, live-intersection for view carriers such as
//! references and slices, and multi-operand per-context intersection all
//! behave exactly as they do for `1 == value`.
//!
//! A named use whose position emitted no capture row — one inside a statement
//! the expression scheduler never evaluates, or a row the operator-resolution
//! stage never minted — has no operand-time custody to consult. The
//! containing statement still records entry semantic contexts: facts the
//! flow pass proved on arrival at that statement. Those facts describe
//! storage as it stands at statement entry, so they falsify a route guard
//! only while every place a guard leaf reads keeps its statement-entry value
//! through the invocation. `operand_entry_provenance` supplies exactly that
//! boundary per occurrence: its pristine-storage window includes the
//! containing statement, so an earlier sibling effect — a receiver-mutating
//! call, an exclusive borrow, an atomic — already voids provenance at the
//! place the leaf reads, while writes confined to disjoint sibling fields
//! leave it intact, and immutable bindings cannot be overwritten at all. A
//! leaf occurrence that does not resolve to an entry-proven operand
//! projection — a `self` field with unknown receiver provenance, a constant,
//! an unproven binding — keeps the route, as does a leaf that would evaluate
//! a nested call or atomic afresh rather than re-reading captured storage.
//! A shared-borrow operand is the referent: the referent's provenance
//! transports through the borrow and the instantiated label names the
//! referent, mirroring the call-`requires` rule that a reference formal used
//! as a predicate value reads its referent.

use checked_trees::FlowFacts;
use facts::{FactContextHandle, FactPayload, FactPlan};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;

use crate::labels::{context_proves_boolean_label, instantiate_operator_contract_expression_label};

/// Whether the selected operator's published route guard is provably false at
/// one named call. When the call's evaluation emitted an operand-time
/// capture row, discharge goes through `checks::named_operator_route_is_false`
/// with exactly the spelled use's selection rules — operand expressions must
/// match, detached-copy carriers (copied scalars and stable by-value formals)
/// keep operand-time snapshots, view carriers intersect capture with
/// invocation-live facts, and several referenced
/// operands intersect by exact context identity. Present-but-ambiguous or
/// substituted custody fails closed: duplicated or mismatched rows select no
/// contexts. A call with no capture row at all falls back to the containing
/// statement's entry contexts, mirroring
/// `checks::operator_route_is_false`'s Boolean polarity structure over rows
/// every incoming path must satisfy.
#[allow(clippy::too_many_arguments)]
pub(super) fn named_route_is_false(
    program: &dyn ::validation::CheckingInput,
    flow: &FlowFacts,
    semantic: &FactPlan,
    named_use: arena::Handle<checked_trees::CheckedNamedOperatorUseFact>,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    expression: ExpressionHandle,
) -> bool {
    if flow
        .control
        .operator_invocations
        .iter()
        .any(|(_, invocation)| invocation.named_use == named_use)
    {
        let operand_labels = super::named_call_operand_labels(program, parameters, operands);
        return crate::checks::named_operator_route_is_false(
            program,
            facts::ProgramPoint::Statement {
                machine_symbol,
                state_symbol,
                statement_index,
            },
            flow,
            semantic,
            named_use,
            parameters,
            operands,
            &operand_labels,
            expression,
        );
    }
    statement_entry_route_is_false(
        program,
        flow,
        semantic,
        machine_symbol,
        state_symbol,
        statement_index,
        parameters,
        operands,
        expression,
    )
}

/// Shared entry-fact discharge for calls without operator operand captures.
/// Ordinary machine calls use this same pristine-storage check after their
/// selected contract's builtin meaning has been established. The containing
/// statement remains inside the provenance window: sibling argument effects
/// cannot make an old entry premise describe a new value.
#[allow(clippy::too_many_arguments)]
pub(crate) fn statement_entry_route_is_false(
    program: &dyn ::validation::CheckingInput,
    flow: &FlowFacts,
    semantic: &FactPlan,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    expression: ExpressionHandle,
) -> bool {
    let operand_labels = super::named_call_operand_labels(program, parameters, operands);
    let context_rows =
        statement_entry_context_rows(flow, machine_symbol, state_symbol, statement_index);
    if context_rows.is_empty() {
        return false;
    }
    expression_has_polarity(
        program,
        semantic,
        &context_rows,
        machine_symbol,
        state_symbol,
        statement_index,
        parameters,
        operands,
        &operand_labels,
        expression,
        false,
    )
}

/// The entry semantic contexts of every flow statement row at this source
/// coordinate. A statement reached by several paths may carry several rows;
/// every row must prove a leaf before it is discharged, so a fact live on
/// only one incoming path can never narrow the surviving routes.
fn statement_entry_context_rows(
    flow: &FlowFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
) -> Vec<Vec<FactContextHandle>> {
    flow.control
        .states
        .iter()
        .filter(|(_, state)| {
            state.machine_symbol == machine_symbol && state.state_symbol == state_symbol
        })
        .flat_map(|(_, state)| {
            flow.control
                .statements
                .span_or_empty(state.statements)
                .iter()
        })
        .filter(|statement| statement.statement_index == statement_index)
        .map(|statement| {
            flow.contexts
                .semantic_context_refs
                .span_or_empty(statement.entry_semantic_contexts)
                .iter()
                .map(|reference| reference.context)
                .chain(flow.semantic_constraint_contexts(statement.entry_constraints))
                .collect()
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn expression_has_polarity(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    context_rows: &[Vec<FactContextHandle>],
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    before_statement: usize,
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    operand_labels: &[String],
    expression: ExpressionHandle,
    polarity: bool,
) -> bool {
    crate::labels::boolean_expression_has_polarity(
        program,
        expression,
        polarity,
        &mut |expression, polarity| {
            leaf_has_polarity(
                program,
                semantic,
                context_rows,
                machine_symbol,
                state_symbol,
                before_statement,
                parameters,
                operands,
                operand_labels,
                expression,
                polarity,
            )
        },
    )
}

fn leaf_has_polarity(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    context_rows: &[Vec<FactContextHandle>],
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    before_statement: usize,
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    operand_labels: &[String],
    expression: ExpressionHandle,
    polarity: bool,
) -> bool {
    if !leaf_reads_only_entry_operands(
        program,
        machine_symbol,
        state_symbol,
        before_statement,
        parameters,
        operands,
        expression,
    ) || expression_evaluates_effects(program, expression)
    {
        return false;
    }
    let required = instantiate_operator_contract_expression_label(
        program,
        parameters,
        operand_labels,
        expression,
    );
    if polarity {
        return context_rows.iter().all(|row| {
            row.iter().any(|context| {
                context_proves_boolean_label(
                    program,
                    semantic,
                    *context,
                    row,
                    facts::ProgramPoint::Statement {
                        machine_symbol,
                        state_symbol,
                        statement_index: before_statement,
                    },
                    &required,
                )
            })
        });
    }
    context_rows.iter().all(|row| {
        row.iter().any(|context| {
            semantic
                .context_view(semantic.contexts.get(*context))
                .facts()
                .any(|fact| {
                    matches!(fact.payload, FactPayload::BooleanValue { expression, value: false }
                        if program.expression_table.display_name(expression) == required)
                })
        })
    })
}

/// Every place occurrence the leaf reads must resolve to an operator
/// parameter whose operand carries entry provenance *at the projected place*:
/// only then does a statement-entry fact about that place still describe the
/// value the invocation evaluates. The provenance is per occurrence rather
/// than per operand — a mutable record whose `other` field was written still
/// carries `count` provenance, so a guard leaf reading only `cell.count`
/// through `&rec` discharges where whole-operand provenance would retain. A
/// constant, a `self` field with unknown receiver provenance, or a binding
/// whose projection was disturbed fails this gate and retains the guard.
fn leaf_reads_only_entry_operands(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    before_statement: usize,
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    expression: ExpressionHandle,
) -> bool {
    let mut occurrences = Vec::new();
    super::super::contract_occurrences::append_expression_occurrences(
        program,
        expression,
        &mut occurrences,
    );
    occurrences.iter().all(|occurrence| {
        let root = occurrence_root_symbol(program, *occurrence);
        parameters.iter().enumerate().any(|(index, parameter)| {
            parameter.symbol == root
                && operands.get(index).is_some_and(|operand| {
                    super::super::crash_entry_values::operand_entry_provenance(
                        program,
                        machine_symbol,
                        state_symbol,
                        before_statement,
                        *operand,
                        *occurrence,
                    )
                })
        })
    })
}

/// A leaf proven by a statement-entry fact is re-evaluated at the invocation,
/// so it must be a pure read of the proven storages. A nested call or atomic
/// could observe storage beyond the operand occurrences — state a matching
/// fact snapshot does not transport — and is kept conservative instead.
fn expression_evaluates_effects(
    program: &dyn ::validation::CheckingInput,
    root: ExpressionHandle,
) -> bool {
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        if !program.expression_table.expression_is_valid(expression) {
            return true;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Call(_) | ExpressionNode::Atomic(_) => return true,
            ExpressionNode::Match(dispatch) => {
                crate::facts::push_match_children(program, dispatch, &mut pending)
            }
            ExpressionNode::Borrow(borrow) => pending.push(borrow.target),
            _ => crate::facts::push_expression_children(program, expression, &mut pending),
        }
    }
    false
}

/// Port of the requires prover's occurrence rooting: a place occurrence roots
/// at the symbol its member/index/borrow selectors descend to.
fn occurrence_root_symbol(
    program: &dyn ::validation::CheckingInput,
    mut expression: ExpressionHandle,
) -> SymbolHandle {
    loop {
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(path) => return path.symbol,
            ExpressionNode::Member(member) => expression = member.receiver,
            ExpressionNode::Indexed(indexed) => expression = indexed.collection,
            ExpressionNode::Borrow(borrow) => expression = borrow.target,
            _ => return SymbolHandle::invalid(),
        }
    }
}
