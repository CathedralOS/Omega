//! Preserve the input selected by a returned view so a later write does not
//! discard facts about an untouched input. Selection refines the complete
//! normal-result relation using captured Boolean inputs, before caller actuals
//! replace declaration coordinates. Unknown routes keep the full relation;
//! borrow permissions and lifetime obligations remain independently checked.

use super::super::call_targets::machine_state_by_symbol;
use super::super::inference::{BoolTruth, ParameterValues};
use super::super::type_capabilities::type_reference_is_reference;
use crate::frozen_program::expression_table;
use crate::value_custody::places::binding_observations::binding_is_exclusively_exposed;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::statement::{
    StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode,
};

/// Remember which input each completed reference result selected. Later changes
/// to its Boolean selector must not change that saved choice.
struct ResultSelection {
    values: ParameterValues,
    captured_results: Vec<(SymbolHandle, Option<Vec<SymbolHandle>>)>,
}

impl ResultSelection {
    fn new(program: &dyn crate::CheckingInput, state: &State, actual_values: &[BoolTruth]) -> Self {
        Self {
            values: ParameterValues::new(program, state, actual_values, &[]),
            captured_results: Vec::new(),
        }
    }

    fn finish_statement(
        &mut self,
        program: &dyn crate::CheckingInput,
        state: &State,
        statement: &StatementNode,
    ) {
        self.values.finish_statement(program, state, statement);
        let StatementNode::Assignment(assignment) = statement else {
            return;
        };
        let ExpressionNode::Name(target) = expression_table(program).expression(assignment.target)
        else {
            return;
        };
        if let Some((_, captured)) = self
            .captured_results
            .iter_mut()
            .find(|(symbol, _)| *symbol == target.symbol)
            && program
                .machine_holding_state(state.symbol)
                .and_then(|machine| {
                    super::super::reference_subjects::replaces_reference_binding(
                        program, machine, statement,
                    )
                })
                != Some(false)
        {
            // Descriptor replacement retires the choice. A scalar store through
            // the captured reference changes its referent, not that descriptor.
            *captured = None;
        }
    }
}

/// A transition block is sequential. The next arm runs only when the previous
/// ordinary guard falls through. Current captured Boolean values can refute a
/// guard and remove an unreachable referent from the caller-visible union.
/// Calls and named-state arrivals substitute the same captured current values,
/// not expressions replayed against a later caller or the outer entry. Finite
/// cycles settle in captured-input queries; unresolved routes retain the
/// complete result union.
pub(super) fn retain_reachable_sources(
    program: &dyn crate::CheckingInput,
    state: &State,
    actual_values: &[BoolTruth],
    origins: &mut super::AggregateOrigins,
) {
    let parameters = program.state_parameters(state);
    let mut reachability = ResultReachability::default();
    let reachable = loop {
        reachability.changed = false;
        for query in &mut reachability.queries {
            query.visited = false;
        }
        let roots = reachable_value_parameters(
            program,
            state,
            parameters,
            actual_values,
            &mut reachability,
        );
        if !reachability.changed {
            break roots;
        }
    };
    let Some(reachable) = reachable else { return };
    let retain = |leaf: &super::ReferenceLeaf| {
        if !matches!(
            leaf.kind,
            super::super::stored_origins::StoredOriginKind::Reference(_)
        ) {
            return true;
        }
        let Some(source) = &leaf.origin else {
            return true;
        };
        let place = &source.place().source;
        // Only modeled reference slots in declared input coordinates belong
        // to this selection. Hidden-pointer enclosures and lifetime debt stay
        // conservative even when the returned descriptor choice is known.
        // Images, invocation results, foreign captures and unknowns stay intact.
        place.is_static()
            || place.result_home().is_some()
            || !parameters
                .iter()
                .any(|parameter| parameter.symbol == place.root)
            || reachable.contains(&place.root)
    };
    // A missing selected spatial result cannot become an empty write footprint.
    // Retained lifetime debts alone are not physical destination evidence.
    if origins
        .references
        .iter()
        .any(|leaf| leaf.kind.may_indirect_storage())
        && !origins
            .references
            .iter()
            .any(|leaf| leaf.kind.may_indirect_storage() && retain(leaf))
    {
        return;
    }
    origins.references.retain(retain);
}

// Selection only narrows the independently completed origin relation. Each
// query is one state with captured Boolean inputs; its result is expressed in
// that state's own formal roots, so every edge substitutes a permutation
// simultaneously. There are finitely many three-valued input vectors and roots.
// Real returns seed the least fixed point, while a backedge reads only the
// provisional set. Unsupported routes are absorbing unknowns and disable
// narrowing. No provisional selection escapes before the whole solve settles.
#[derive(Default)]
struct ResultReachability {
    queries: Vec<ResultQuery>,
    changed: bool,
}

struct ResultQuery {
    state: SymbolHandle,
    values: Vec<BoolTruth>,
    roots: Option<Vec<SymbolHandle>>,
    active: bool,
    visited: bool,
}

fn reachable_value_parameters(
    program: &dyn crate::CheckingInput,
    state: &State,
    parameters: &[StateParameter],
    actual_values: &[BoolTruth],
    reachability: &mut ResultReachability,
) -> Option<Vec<SymbolHandle>> {
    if parameters.len() != actual_values.len() {
        return None;
    }
    let index = if let Some(index) = reachability
        .queries
        .iter()
        .position(|query| query.state == state.symbol && query.values == actual_values)
    {
        index
    } else {
        reachability.queries.push(ResultQuery {
            state: state.symbol,
            values: actual_values.to_vec(),
            roots: Some(Vec::new()),
            active: false,
            visited: false,
        });
        reachability.changed = true;
        reachability.queries.len() - 1
    };
    if reachability.queries[index].active || reachability.queries[index].visited {
        return reachability.queries[index].roots.clone();
    }
    reachability.queries[index].active = true;
    reachability.queries[index].visited = true;
    let incoming = reachable_value_parameters_body(program, state, actual_values, reachability);
    let query = &mut reachability.queries[index];
    query.active = false;
    match (&mut query.roots, incoming) {
        (Some(held), Some(incoming)) => {
            for root in incoming {
                if !held.contains(&root) {
                    held.push(root);
                    reachability.changed = true;
                }
            }
        }
        (Some(_), None) => {
            query.roots = None;
            reachability.changed = true;
        }
        (None, _) => {}
    }
    query.roots.clone()
}

fn reachable_value_parameters_body(
    program: &dyn crate::CheckingInput,
    state: &State,
    actual_values: &[BoolTruth],
    reachability: &mut ResultReachability,
) -> Option<Vec<SymbolHandle>> {
    let mut current = ResultSelection::new(program, state, actual_values);
    let mut fall_through = true;
    let mut reachable = Vec::new();
    let mut saw_value = false;
    let statements = program.statement_table.statements(state.statement_nodes);
    let last = statements.len().saturating_sub(1);
    for (statement_index, statement) in program
        .statement_table
        .admitted_execution_statements(state.statement_nodes)
    {
        match statement {
            StatementNode::Assignment(_) => current.finish_statement(program, state, statement),
            StatementNode::LocalData(local)
                if type_reference_is_reference(program, local.type_reference) =>
            {
                // Preserve a completed choice before a later selector store.
                // Unstable bindings keep the unspecialized result relation.
                let stable = !binding_is_exclusively_exposed(program, state, local.symbol);
                let result = stable
                    .then(|| {
                        contributed_parameters(
                            program,
                            state,
                            &current,
                            reachability,
                            local.initial_value,
                        )
                    })
                    .flatten();
                current.captured_results.push((local.symbol, result));
            }
            StatementNode::LocalData(_) => current.finish_statement(program, state, statement),
            StatementNode::Transition(transition) => {
                let truth = match transition.guard {
                    TransitionGuardNode::Always => BoolTruth::True,
                    TransitionGuardNode::When(condition) => {
                        current.values.truth(program, state, condition)
                    }
                };
                if transition.exit == TransitionExit::Ordinary
                    && fall_through
                    && truth != BoolTruth::False
                {
                    for edge in transition.targets() {
                        match program.statement_table.transition_target(edge) {
                            TransitionTargetNode::Value(result) => {
                                saw_value = true;
                                reachable.extend(contributed_parameters(
                                    program,
                                    state,
                                    &current,
                                    reachability,
                                    *result,
                                )?);
                            }
                            TransitionTargetNode::Named { .. } => {
                                saw_value = true;
                                reachable.extend(contributed_named_target(
                                    program,
                                    state,
                                    &current,
                                    reachability,
                                    edge,
                                )?);
                            }
                            TransitionTargetNode::Terminal => {}
                        }
                    }
                } else if transition.exit == TransitionExit::Ordinary && fall_through {
                    for edge in transition.targets() {
                        if matches!(
                            program.statement_table.transition_target(edge),
                            TransitionTargetNode::Value(_) | TransitionTargetNode::Named { .. }
                        ) {
                            saw_value = true;
                        }
                    }
                }
                if truth == BoolTruth::True {
                    fall_through = false;
                }
            }
            StatementNode::Expression(result) if statement_index == last && fall_through => {
                saw_value = true;
                reachable.extend(contributed_parameters(
                    program,
                    state,
                    &current,
                    reachability,
                    *result,
                )?);
            }
            _ => {}
        }
    }
    saw_value.then_some(reachable)
}

fn contributed_parameters(
    program: &dyn crate::CheckingInput,
    state: &State,
    current: &ResultSelection,
    reachability: &mut ResultReachability,
    result: ExpressionHandle,
) -> Option<Vec<SymbolHandle>> {
    match expression_table(program).expression(result) {
        ExpressionNode::Borrow(borrow) => {
            contributed_parameters(program, state, current, reachability, borrow.target)
        }
        ExpressionNode::Name(path) => {
            if let Some(parameter) = program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.symbol == path.symbol)
            {
                // This relation names the entry descriptor. A reseated or
                // exposed reference may name another input at this return.
                let stable = if type_reference_is_reference(program, parameter.type_reference) {
                    program
                        .machine_holding_state(state.symbol)
                        .is_some_and(|machine| {
                            super::super::alias_bindings::state_reference_parameter_binding_is_stable(
                                program,
                                machine,
                                state,
                                parameter.symbol,
                            )
                        })
                } else {
                    !parameter.is_mutable
                };
                if !stable {
                    return None;
                }
                return Some(vec![parameter.symbol]);
            }
            current
                .captured_results
                .iter()
                .find(|(symbol, _)| *symbol == path.symbol)?
                .1
                .clone()
        }
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                return None;
            }
            let (_, callee_state) =
                super::super::call_targets::machine_state_by_symbol(program, call.target_symbol)?;
            let callee_arguments = expression_table(program).expression_handles(call.arguments);
            parameters_reached_through(
                program,
                state,
                current,
                reachability,
                callee_state,
                callee_arguments,
            )
        }
        _ => None,
    }
}

fn contributed_named_target(
    program: &dyn crate::CheckingInput,
    state: &State,
    current: &ResultSelection,
    reachability: &mut ResultReachability,
    edge: typed_trees::statement::TransitionTargetHandle,
) -> Option<Vec<SymbolHandle>> {
    let TransitionTargetNode::Named { arguments, .. } =
        program.statement_table.transition_target(edge)
    else {
        return None;
    };
    let machine = program.machine_holding_state(state.symbol)?;
    let call_symbol = super::super::terminal_calls::target(program, machine, edge);
    let target_state = if call_symbol.is_valid() {
        machine_state_by_symbol(program, call_symbol)?.1
    } else {
        super::super::transition_topology::named_transition_target_state(
            program, machine, state, edge,
        )?
    };
    let arguments = program.statement_table.expression_handles(*arguments);
    parameters_reached_through(
        program,
        state,
        current,
        reachability,
        target_state,
        arguments,
    )
}

/// Follow a returned reference through a call or state transition. Map the
/// callee's selected inputs back to the caller's saved choices, using the
/// Boolean values captured at this call.
fn parameters_reached_through(
    program: &dyn crate::CheckingInput,
    host_state: &State,
    host: &ResultSelection,
    reachability: &mut ResultReachability,
    target_state: &State,
    target_arguments: &[ExpressionHandle],
) -> Option<Vec<SymbolHandle>> {
    let parameters = program.state_parameters(target_state);
    let actual_values = host.values.argument_values(program, target_arguments);
    let inner = reachable_value_parameters(
        program,
        target_state,
        parameters,
        &actual_values,
        reachability,
    )?;
    let mut mapped = Vec::new();
    for symbol in inner {
        let position = parameters
            .iter()
            .position(|parameter| parameter.symbol == symbol)?;
        let actual = *target_arguments.get(position)?;
        for parameter in contributed_parameters(program, host_state, host, reachability, actual)? {
            if !mapped.contains(&parameter) {
                mapped.push(parameter);
            }
        }
    }
    Some(mapped)
}
