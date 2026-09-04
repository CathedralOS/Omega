use psi_checked_trees::{
    CheckedTerminalDebugPlans, CheckedTerminalMachineDebugPlan, CheckedTerminalStateDebugPlan,
};
use psi_typed_trees::{
    TypedTrees,
    domain::ProofFact,
    expression::{ExpressionHandle, ExpressionNode},
    signature::SignatureContractKind,
    statement::{StatementNode, TransitionGuardNode, TransitionTargetNode},
};

pub(crate) fn build_checked_terminal_debug_plans(
    program: &TypedTrees,
) -> CheckedTerminalDebugPlans {
    CheckedTerminalDebugPlans {
        machines: program
            .machines()
            .iter()
            .map(|machine| build_machine_debug_plan(program, machine))
            .collect(),
    }
}

fn build_machine_debug_plan(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
) -> CheckedTerminalMachineDebugPlan {
    let machine_span = program.symbols.symbol_source_span(machine.symbol);
    let contract_span = source_ensures_span(program, machine)
        .filter(|span| *span != psi_source::SourceSpan::default())
        .filter(|span| program.symbols.source_file(*span).is_some())
        .or(machine_span);
    let states = program
        .machine_states(machine)
        .iter()
        .map(|state| CheckedTerminalStateDebugPlan {
            state: state.symbol,
            state_span: program.symbols.symbol_source_span(state.symbol),
            parameter_spans: program
                .state_parameters(state)
                .iter()
                .filter(|parameter| !parameter.is_self)
                .map(|parameter| program.symbols.symbol_source_span(parameter.symbol))
                .collect(),
            transition_spans: source_transition_spans(program, state),
            operation_spans: source_operation_spans(program, state),
        })
        .collect::<Vec<_>>();
    let mut source_ids = machine_span
        .into_iter()
        .chain(contract_span)
        .chain(states.iter().flat_map(|state| {
            state
                .state_span
                .into_iter()
                .chain(state.parameter_spans.iter().flatten().copied())
                .chain(state.transition_spans.iter().copied())
                .chain(state.operation_spans.iter().copied())
        }))
        .map(|span| span.source_id.0)
        .collect::<Vec<_>>();
    source_ids.sort_unstable();
    source_ids.dedup();
    let source_files = source_ids
        .into_iter()
        .filter_map(|source_id| {
            program.symbols.source_file(psi_source::SourceSpan::new(
                psi_source::SourceId(source_id),
                psi_source::Span::default(),
            ))
        })
        .cloned()
        .collect();
    CheckedTerminalMachineDebugPlan {
        machine: machine.symbol,
        machine_span,
        contract_span,
        states,
        source_files,
    }
}

fn source_ensures_span(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
) -> Option<psi_source::SourceSpan> {
    let contract = program
        .machine_contracts(machine)
        .iter()
        .find(|contract| contract.kind == SignatureContractKind::Ensures)?;
    let [ProofFact::Expression(expression)] = program.proof_facts.span_or_empty(contract.facts)
    else {
        return None;
    };
    Some(program.expression_table.source_span(*expression))
}

fn source_transition_spans(
    program: &TypedTrees,
    state: &psi_typed_trees::state::State,
) -> Vec<psi_source::SourceSpan> {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| match statement {
            StatementNode::Transition(transition) => Some(transition.source_span),
            StatementNode::Expression(expression) => {
                Some(program.expression_table.source_span(*expression))
            }
            _ => None,
        })
        .collect()
}

fn source_operation_spans(
    program: &TypedTrees,
    state: &psi_typed_trees::state::State,
) -> Vec<psi_source::SourceSpan> {
    let mut spans = Vec::new();
    for statement in program.statement_table.statements(state.statement_nodes) {
        match statement {
            StatementNode::Expression(expression) => {
                collect_source_operation_spans(program, *expression, &mut spans);
            }
            StatementNode::LocalData(local) if local.initial_value.is_valid() => {
                collect_source_operation_spans(program, local.initial_value, &mut spans);
            }
            StatementNode::Transition(transition) => {
                if let TransitionGuardNode::When(guard) = transition.guard {
                    collect_source_operation_spans(program, guard, &mut spans);
                }
                if let TransitionTargetNode::Named { arguments, .. } =
                    program.statement_table.transition_target(transition.target)
                {
                    for expression in program.statement_table.expression_handles(*arguments) {
                        collect_source_operation_spans(program, *expression, &mut spans);
                    }
                }
            }
            _ => {}
        }
    }
    spans
}

fn collect_source_operation_spans(
    program: &TypedTrees,
    expression: ExpressionHandle,
    spans: &mut Vec<psi_source::SourceSpan>,
) {
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => {
            spans.push(program.expression_table.source_span(expression));
        }
        ExpressionNode::Binary(binary) => {
            collect_source_operation_spans(program, binary.left, spans);
            collect_source_operation_spans(program, binary.right, spans);
            spans.push(program.expression_table.source_span(expression));
        }
        ExpressionNode::Unary(unary) => {
            collect_source_operation_spans(program, unary.operand, spans);
            spans.push(program.expression_table.source_span(expression));
        }
        _ => {}
    }
}
