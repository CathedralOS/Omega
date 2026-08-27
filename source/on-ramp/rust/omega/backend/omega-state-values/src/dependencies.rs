use crate::planning::StateValuePlanningContext;
use crate::simplify::{resolve_call_target_machine, resolve_call_target_state};
use psi_checked_trees::CheckedTrees;
use psi_checked_trees::expression::{CallExpression, Expression, ExpressionHandle};
use psi_checked_trees::machine::Machine;
use psi_checked_trees::statement::{
    StatementNode, TransitionGuardNode, TransitionTargetHandle, TransitionTargetNode,
};
use psi_symbols::SymbolHandle;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct StateIdentity {
    machine: (u32, u32),
    state: (u32, u32),
}

impl StateIdentity {
    const fn new(machine: SymbolHandle, state: SymbolHandle) -> Self {
        Self {
            machine: symbol_identity(machine),
            state: symbol_identity(state),
        }
    }
}

const fn symbol_identity(symbol: SymbolHandle) -> (u32, u32) {
    (symbol.arena_index(), symbol.generation())
}

/// Exact checked-tree value-call reachability used only to bound state-value
/// planning. `StateValueUse::required` remains the downstream emission fact;
/// this index additionally retains off-flow helper states whose bodies the
/// recursive expression simplifier may inspect.
#[derive(Debug, Clone, Default)]
pub(crate) struct StateValueDependencyIndex {
    retained: BTreeSet<StateIdentity>,
    retain_all: bool,
}

impl StateValueDependencyIndex {
    pub(crate) fn build(program: &CheckedTrees, context: &StateValuePlanningContext) -> Self {
        let mut locations = BTreeMap::new();
        let mut state_symbols = BTreeSet::new();
        let mut complete = true;

        for (machine_index, machine) in program.machines().iter().enumerate() {
            if !machine.symbol.is_valid() {
                complete = false;
            }
            for (state_index, state) in program.machine_states(machine).iter().enumerate() {
                let identity = StateIdentity::new(machine.symbol, state.symbol);
                if !state.symbol.is_valid()
                    || locations
                        .insert(identity, (machine_index, state_index))
                        .is_some()
                {
                    complete = false;
                }
                state_symbols.insert(symbol_identity(state.symbol));
            }
        }

        let mut retained = context
            .runtime_flow
            .states
            .iter()
            .map(|(_, runtime)| StateIdentity::new(runtime.key.machine, runtime.key.state))
            .chain(
                context
                    .state_calls
                    .required_states
                    .iter()
                    .map(|(_, required)| StateIdentity::new(required.machine, required.state)),
            )
            .collect::<BTreeSet<_>>();
        let mut pending = retained.iter().copied().collect::<VecDeque<_>>();

        if retained
            .iter()
            .any(|identity| !locations.contains_key(identity))
        {
            complete = false;
        }

        // A nonempty checked program without a runtime/call seed has no exact
        // pruning authority. Preserve the previous full-plan behavior.
        if retained.is_empty() {
            complete = false;
        }

        while complete {
            let Some(identity) = pending.pop_front() else {
                break;
            };
            let Some(&(machine_index, state_index)) = locations.get(&identity) else {
                complete = false;
                break;
            };
            let machine = &program.machines()[machine_index];
            let state = &program.machine_states(machine)[state_index];
            complete = visit_state(
                program,
                machine,
                state,
                &locations,
                &state_symbols,
                &mut retained,
                &mut pending,
            );
        }

        Self {
            retained,
            retain_all: !complete,
        }
    }

    pub(crate) fn retains(&self, machine: SymbolHandle, state: SymbolHandle) -> bool {
        self.retain_all || self.retained.contains(&StateIdentity::new(machine, state))
    }
}

fn visit_state(
    program: &CheckedTrees,
    machine: &Machine,
    state: &psi_checked_trees::state::State,
    locations: &BTreeMap<StateIdentity, (usize, usize)>,
    state_symbols: &BTreeSet<(u32, u32)>,
    retained: &mut BTreeSet<StateIdentity>,
    pending: &mut VecDeque<StateIdentity>,
) -> bool {
    for statement in program.statement_table.statements(state.statement_nodes) {
        let complete = match statement {
            StatementNode::AssemblyFact(_) => true,
            StatementNode::Assignment(assignment) => visit_expression_handle(
                program,
                machine,
                assignment.value,
                locations,
                state_symbols,
                retained,
                pending,
            ),
            StatementNode::Call(call) => program
                .statement_table
                .expression_handles(call.arguments)
                .iter()
                .all(|argument| {
                    visit_expression_handle(
                        program,
                        machine,
                        *argument,
                        locations,
                        state_symbols,
                        retained,
                        pending,
                    )
                }),
            StatementNode::Expression(expression) => visit_expression_handle(
                program,
                machine,
                *expression,
                locations,
                state_symbols,
                retained,
                pending,
            ),
            StatementNode::LocalData(local) => {
                !local.initial_value.is_valid()
                    || visit_expression_handle(
                        program,
                        machine,
                        local.initial_value,
                        locations,
                        state_symbols,
                        retained,
                        pending,
                    )
            }
            StatementNode::Transition(transition) => {
                let guard_complete = match transition.guard {
                    TransitionGuardNode::Always => true,
                    TransitionGuardNode::When(guard) => visit_expression_handle(
                        program,
                        machine,
                        guard,
                        locations,
                        state_symbols,
                        retained,
                        pending,
                    ),
                };
                guard_complete
                    && visit_transition_target(
                        program,
                        machine,
                        transition.target,
                        locations,
                        state_symbols,
                        retained,
                        pending,
                    )
                    && visit_transition_target(
                        program,
                        machine,
                        transition.continuation,
                        locations,
                        state_symbols,
                        retained,
                        pending,
                    )
            }
        };
        if !complete {
            return false;
        }
    }
    true
}

fn visit_transition_target(
    program: &CheckedTrees,
    machine: &Machine,
    target: TransitionTargetHandle,
    locations: &BTreeMap<StateIdentity, (usize, usize)>,
    state_symbols: &BTreeSet<(u32, u32)>,
    retained: &mut BTreeSet<StateIdentity>,
    pending: &mut VecDeque<StateIdentity>,
) -> bool {
    if !target.is_valid() {
        return true;
    }
    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named { arguments, .. } => program
            .statement_table
            .expression_handles(*arguments)
            .iter()
            .all(|argument| {
                visit_expression_handle(
                    program,
                    machine,
                    *argument,
                    locations,
                    state_symbols,
                    retained,
                    pending,
                )
            }),
        TransitionTargetNode::Value(value) => visit_expression_handle(
            program,
            machine,
            *value,
            locations,
            state_symbols,
            retained,
            pending,
        ),
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => true,
    }
}

fn visit_expression_handle(
    program: &CheckedTrees,
    machine: &Machine,
    expression: ExpressionHandle,
    locations: &BTreeMap<StateIdentity, (usize, usize)>,
    state_symbols: &BTreeSet<(u32, u32)>,
    retained: &mut BTreeSet<StateIdentity>,
    pending: &mut VecDeque<StateIdentity>,
) -> bool {
    if !expression.is_valid() {
        return true;
    }
    visit_expression(
        program,
        machine,
        &program.expression_table.to_tree(expression),
        locations,
        state_symbols,
        retained,
        pending,
    )
}

fn visit_expression(
    program: &CheckedTrees,
    machine: &Machine,
    expression: &Expression,
    locations: &BTreeMap<StateIdentity, (usize, usize)>,
    state_symbols: &BTreeSet<(u32, u32)>,
    retained: &mut BTreeSet<StateIdentity>,
    pending: &mut VecDeque<StateIdentity>,
) -> bool {
    let mut visit = |expression: &Expression| {
        visit_expression(
            program,
            machine,
            expression,
            locations,
            state_symbols,
            retained,
            pending,
        )
    };

    match expression {
        // The simplifier treats compiler-authored atomics as opaque.
        Expression::Atomic(_) => true,
        Expression::ArrayLiteral(values) => values.iter().all(&mut visit),
        Expression::Binary(binary) => visit(&binary.left) && visit(&binary.right),
        Expression::Call(call) => {
            call.receiver.as_deref().is_none_or(&mut visit)
                && call.arguments.iter().all(&mut visit)
                && visit_call_target(
                    program,
                    machine,
                    call,
                    locations,
                    state_symbols,
                    retained,
                    pending,
                )
        }
        Expression::Cast(cast) => visit(&cast.value),
        Expression::Indexed(indexed) => visit(&indexed.collection) && visit(&indexed.index),
        Expression::Member(member) => visit(&member.receiver),
        Expression::Borrow(inner) => visit(&inner.target),
        Expression::Range(range) => {
            range.start.as_deref().is_none_or(&mut visit)
                && range.end.as_deref().is_none_or(&mut visit)
        }
        Expression::StructLiteral(literal) => {
            literal.fields.iter().all(|field| visit(&field.value))
        }
        Expression::Unary(unary) => visit(&unary.operand),
        Expression::Boolean(_)
        | Expression::Float(_)
        | Expression::Integer(_)
        | Expression::Name(_)
        | Expression::String(_)
        | Expression::ZeroValue(_) => true,
    }
}

fn visit_call_target(
    program: &CheckedTrees,
    machine: &Machine,
    call: &CallExpression,
    locations: &BTreeMap<StateIdentity, (usize, usize)>,
    state_symbols: &BTreeSet<(u32, u32)>,
    retained: &mut BTreeSet<StateIdentity>,
    pending: &mut VecDeque<StateIdentity>,
) -> bool {
    let Some(target_machine) = resolve_call_target_machine(
        program,
        machine,
        call.receiver.as_deref(),
        call.target_symbol,
    ) else {
        return !state_symbols.contains(&symbol_identity(call.target_symbol));
    };
    let Some(target_state) = resolve_call_target_state(program, target_machine, call) else {
        return !state_symbols.contains(&symbol_identity(call.target_symbol));
    };
    let identity = StateIdentity::new(target_machine.symbol, target_state.symbol);
    if !locations.contains_key(&identity) {
        return false;
    }
    if retained.insert(identity) {
        pending.push_back(identity);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::StateValueDependencyIndex;
    use crate::planning::{StateValuePlanningContext, build_state_value_plan};
    use omega_control_flow::StateKey;
    use omega_state_calls::StateCallPlan;
    use omega_state_graph::{RuntimeFlowPlan, RuntimeState};
    use psi_checked_trees::CheckedTrees;
    use psi_checked_trees::expression::{CallExpression, Expression};
    use psi_checked_trees::machine::Machine;
    use psi_checked_trees::state::State;
    use psi_checked_trees::statement::{
        StatementNode, TableLocalData, TableTransition, TransitionGuardNode, TransitionTargetNode,
    };
    use psi_symbols::SymbolHandle;
    use std::sync::Arc;

    fn symbol(index: u32) -> SymbolHandle {
        SymbolHandle::from_arena_index(index)
    }

    fn call(
        target_symbol: SymbolHandle,
        receiver: Option<Expression>,
        arguments: Vec<Expression>,
    ) -> Expression {
        Expression::Call(Box::new(CallExpression {
            receiver: receiver.map(Box::new),
            target_symbol,
            target: "state".into(),
            arguments: Arc::from(arguments),
            evidence_arguments: Arc::default(),
            operational_acknowledgement: Default::default(),
        }))
    }

    fn push_expression_state(
        program: &mut CheckedTrees,
        machine: &mut Machine,
        state_symbol: SymbolHandle,
        expression: Expression,
    ) {
        let expression = program.typed.expression_table.insert_tree(&expression);
        let mut state = State {
            symbol: state_symbol,
            name: "state".into(),
            ..State::default()
        };
        program.typed.statement_table.push_statement(
            &mut state.statement_nodes,
            StatementNode::Expression(expression),
        );
        program.typed.push_machine_state(machine, state);
    }

    fn context_with_runtime_root(
        machine: SymbolHandle,
        state: SymbolHandle,
    ) -> StateValuePlanningContext {
        let mut runtime_flow = RuntimeFlowPlan::default();
        runtime_flow.states.insert(RuntimeState {
            key: StateKey {
                machine,
                state,
                segment_index: 0,
            },
            ..RuntimeState::default()
        });
        StateValuePlanningContext {
            runtime_flow: Arc::new(runtime_flow),
            state_calls: Arc::new(StateCallPlan::default()),
        }
    }

    #[test]
    fn retains_transitive_helpers_from_every_simplifier_expression_position() {
        let machine_symbol = symbol(1);
        let root_symbol = symbol(2);
        let local_symbol = symbol(3);
        let deep_symbol = symbol(4);
        let guard_symbol = symbol(5);
        let value_symbol = symbol(6);
        let receiver_symbol = symbol(7);
        let argument_symbol = symbol(8);
        let dead_symbol = symbol(9);
        let mut program = CheckedTrees::default();
        let mut machine = Machine {
            symbol: machine_symbol,
            name: "Fixture".into(),
            ..Machine::default()
        };

        let local_call =
            program
                .typed
                .expression_table
                .insert_tree(&call(local_symbol, None, vec![]));
        let guard_call =
            program
                .typed
                .expression_table
                .insert_tree(&call(guard_symbol, None, vec![]));
        let value_call =
            program
                .typed
                .expression_table
                .insert_tree(&call(value_symbol, None, vec![]));
        let nested_calls = program.typed.expression_table.insert_tree(&call(
            SymbolHandle::invalid(),
            Some(call(receiver_symbol, None, vec![])),
            vec![call(argument_symbol, None, vec![])],
        ));
        let target = program
            .typed
            .statement_table
            .insert_transition_target(TransitionTargetNode::Value(value_call));
        let mut root = State {
            symbol: root_symbol,
            name: "root".into(),
            ..State::default()
        };
        program.typed.statement_table.push_statement(
            &mut root.statement_nodes,
            StatementNode::LocalData(TableLocalData {
                symbol: symbol(20),
                name: "local".into(),
                initial_value: local_call,
                ..TableLocalData::default()
            }),
        );
        program.typed.statement_table.push_statement(
            &mut root.statement_nodes,
            StatementNode::Transition(TableTransition {
                target,
                guard: TransitionGuardNode::When(guard_call),
                ..TableTransition::default()
            }),
        );
        program.typed.statement_table.push_statement(
            &mut root.statement_nodes,
            StatementNode::Expression(nested_calls),
        );
        program.typed.push_machine_state(&mut machine, root);

        push_expression_state(
            &mut program,
            &mut machine,
            local_symbol,
            call(deep_symbol, None, vec![]),
        );
        for state_symbol in [
            deep_symbol,
            guard_symbol,
            value_symbol,
            receiver_symbol,
            argument_symbol,
            dead_symbol,
        ] {
            push_expression_state(
                &mut program,
                &mut machine,
                state_symbol,
                Expression::Boolean(true),
            );
        }
        program.typed.push_machine(machine);

        let index = StateValueDependencyIndex::build(
            &program,
            &context_with_runtime_root(machine_symbol, root_symbol),
        );
        for retained in [
            root_symbol,
            local_symbol,
            deep_symbol,
            guard_symbol,
            value_symbol,
            receiver_symbol,
            argument_symbol,
        ] {
            assert!(index.retains(machine_symbol, retained));
        }
        assert!(!index.retains(machine_symbol, dead_symbol));
    }

    #[test]
    fn required_state_call_is_an_independent_dependency_seed() {
        let machine_symbol = symbol(30);
        let required_symbol = symbol(31);
        let dead_symbol = symbol(32);
        let mut program = CheckedTrees::default();
        let mut machine = Machine {
            symbol: machine_symbol,
            name: "RequiredFixture".into(),
            ..Machine::default()
        };
        for state_symbol in [required_symbol, dead_symbol] {
            push_expression_state(
                &mut program,
                &mut machine,
                state_symbol,
                Expression::Boolean(true),
            );
        }
        program.typed.push_machine(machine);

        let mut state_calls = StateCallPlan::default();
        state_calls.required_states.insert(StateKey {
            machine: machine_symbol,
            state: required_symbol,
            segment_index: 7,
        });
        let context = StateValuePlanningContext {
            runtime_flow: Arc::new(RuntimeFlowPlan::default()),
            state_calls: Arc::new(state_calls),
        };
        let index = StateValueDependencyIndex::build(&program, &context);
        assert!(index.retains(machine_symbol, required_symbol));
        assert!(!index.retains(machine_symbol, dead_symbol));
    }

    #[test]
    fn planning_retains_off_flow_helpers_without_marking_them_emission_required() {
        let machine_symbol = symbol(35);
        let root_symbol = symbol(36);
        let helper_symbol = symbol(37);
        let dead_symbol = symbol(38);
        let mut program = CheckedTrees::default();
        let mut machine = Machine {
            symbol: machine_symbol,
            name: "PlanningFixture".into(),
            ..Machine::default()
        };
        push_expression_state(
            &mut program,
            &mut machine,
            root_symbol,
            call(helper_symbol, None, vec![]),
        );
        for state_symbol in [helper_symbol, dead_symbol] {
            push_expression_state(
                &mut program,
                &mut machine,
                state_symbol,
                Expression::Boolean(true),
            );
        }
        program.typed.push_machine(machine);

        let plan = build_state_value_plan(
            &program,
            context_with_runtime_root(machine_symbol, root_symbol),
        );
        let values = plan
            .values
            .iter()
            .map(|(_, value)| value)
            .collect::<Vec<_>>();
        assert!(
            values
                .iter()
                .any(|value| { value.source_key.state == root_symbol && value.required })
        );
        assert!(
            values
                .iter()
                .any(|value| { value.source_key.state == helper_symbol && !value.required })
        );
        assert!(
            values
                .iter()
                .all(|value| value.source_key.state != dead_symbol)
        );
    }

    #[test]
    fn unresolved_known_state_call_conservatively_retains_every_state() {
        let machine_symbol = symbol(40);
        let root_symbol = symbol(41);
        let target_symbol = symbol(42);
        let dead_symbol = symbol(43);
        let mut program = CheckedTrees::default();
        let mut machine = Machine {
            symbol: machine_symbol,
            name: "FallbackFixture".into(),
            ..Machine::default()
        };
        push_expression_state(
            &mut program,
            &mut machine,
            root_symbol,
            call(
                target_symbol,
                Some(Expression::Integer(
                    psi_numerics::literals::IntegerLiteral::from_value(0),
                )),
                vec![],
            ),
        );
        for state_symbol in [target_symbol, dead_symbol] {
            push_expression_state(
                &mut program,
                &mut machine,
                state_symbol,
                Expression::Boolean(true),
            );
        }
        program.typed.push_machine(machine);

        let index = StateValueDependencyIndex::build(
            &program,
            &context_with_runtime_root(machine_symbol, root_symbol),
        );
        assert!(index.retains(machine_symbol, root_symbol));
        assert!(index.retains(machine_symbol, target_symbol));
        assert!(index.retains(machine_symbol, dead_symbol));
    }
}
