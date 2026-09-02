//! Recursive suspension/blocking inference plus normalized call topology.
//!
//! Service reach is deliberately absent. Calls retain stable
//! `(state, statement, ordinal)` identity in grouped arenas so service reach,
//! capability approval, carry checks, and reports can join the same topology
//! without rebuilding leaf `Vec`s or reintroducing a global service catalog.

use psi_arena::{Arena, HandleSpan};
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use psi_typed_trees::state::State;
use psi_typed_trees::statement::{StatementNode, TableCall};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OperationalPlan {
    pub root_machines: HandleSpan<MachineOperational>,
    pub machines: Arena<MachineOperational>,
    pub states: Arena<StateOperational>,
    pub calls: Arena<CallOperational>,
}

impl OperationalPlan {
    pub fn machines(&self) -> &[MachineOperational] {
        self.machines.span_or_empty(self.root_machines)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MachineOperational {
    pub symbol: SymbolHandle,
    /// Authored ceilings. For checked bodies these gate admission; for
    /// requirements and boundaries they are the pinned caller summary.
    pub published_may_suspend: bool,
    pub published_may_block: bool,
    /// Effective recursive summary consumed by callers.
    pub transitive_may_suspend: bool,
    pub transitive_may_block: bool,
    /// Declaration-free recursive body summary used to validate ceilings.
    pub body_may_suspend: bool,
    pub body_may_block: bool,
    pub states: HandleSpan<StateOperational>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateOperational {
    pub symbol: SymbolHandle,
    pub direct_may_suspend: bool,
    pub direct_may_block: bool,
    pub transitive_may_suspend: bool,
    pub transitive_may_block: bool,
    pub calls: HandleSpan<CallOperational>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CallOperational {
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub target_name: String,
    pub target_state_symbol: SymbolHandle,
    pub target_machine_symbol: SymbolHandle,
    pub direct_may_suspend: bool,
    pub direct_may_block: bool,
    pub transitive_may_suspend: bool,
    pub transitive_may_block: bool,
    pub acknowledgement: psi_language_semantics::CallOperationalAcknowledgement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MachineWork {
    symbol: SymbolHandle,
    uses_published_contract: bool,
    published_may_suspend: bool,
    published_may_block: bool,
    transitive_may_suspend: bool,
    transitive_may_block: bool,
    body_may_suspend: bool,
    body_may_block: bool,
    states: Vec<StateWork>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StateWork {
    symbol: SymbolHandle,
    direct_may_suspend: bool,
    direct_may_block: bool,
    transitive_may_suspend: bool,
    transitive_may_block: bool,
    calls: Vec<CallWork>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CallWork {
    statement_index: usize,
    call_ordinal: usize,
    target_name: String,
    target_state_symbol: SymbolHandle,
    target_machine_symbol: SymbolHandle,
    direct_may_suspend: bool,
    direct_may_block: bool,
    transitive_may_suspend: bool,
    transitive_may_block: bool,
    acknowledgement: psi_language_semantics::CallOperationalAcknowledgement,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct DirectCallOperational {
    may_suspend: bool,
    may_block: bool,
}

pub fn infer_operational_may(program: &TypedTrees) -> OperationalPlan {
    let mut machines = build_machine_work(program);
    propagate_operational_may(&mut machines);
    build_plan(machines)
}

fn build_machine_work(program: &TypedTrees) -> Vec<MachineWork> {
    let mut machines = Vec::with_capacity(program.machines().len());

    for machine in program.machines() {
        let uses_published_contract = machine.is_public
            || machine.supply_mode != psi_language_semantics::MachineSupplyMode::CheckedBody;
        let states = program
            .machine_states(machine)
            .iter()
            .map(|state| StateWork {
                symbol: state.symbol,
                direct_may_suspend: false,
                direct_may_block: false,
                transitive_may_suspend: false,
                transitive_may_block: false,
                calls: collect_state_calls(program, state),
            })
            .collect();

        machines.push(MachineWork {
            symbol: machine.symbol,
            uses_published_contract,
            published_may_suspend: machine.suspends,
            published_may_block: machine.blocks,
            transitive_may_suspend: uses_published_contract && machine.suspends,
            transitive_may_block: uses_published_contract && machine.blocks,
            body_may_suspend: false,
            body_may_block: false,
            states,
        });
    }

    machines
}

fn collect_state_calls(program: &TypedTrees, state: &State) -> Vec<CallWork> {
    let mut calls = Vec::new();

    for (statement_index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        // Call-site identity is (state, statement, ordinal-within-statement),
        // shared with borrow, flow, contracts, and diagnostics.
        let mut call_ordinal = 0usize;
        collect_statement_calls(
            program,
            statement,
            statement_index,
            &mut call_ordinal,
            &mut calls,
        );
    }

    calls
}

fn collect_statement_calls(
    program: &TypedTrees,
    statement: &StatementNode,
    statement_index: usize,
    call_ordinal: &mut usize,
    calls: &mut Vec<CallWork>,
) {
    match statement {
        // Assembly contract facts are proof obligations, never runtime calls.
        StatementNode::AssemblyFact(_) => {}
        StatementNode::Assignment(assignment) => {
            collect_expression_calls(
                program,
                assignment.target,
                statement_index,
                call_ordinal,
                calls,
            );
            collect_expression_calls(
                program,
                assignment.value,
                statement_index,
                call_ordinal,
                calls,
            );
        }
        StatementNode::Call(call) => {
            push_statement_call(program, call, statement_index, call_ordinal, calls)
        }
        StatementNode::Expression(expression) => {
            collect_expression_calls(program, *expression, statement_index, call_ordinal, calls)
        }
        StatementNode::LocalData(local_data) => {
            if local_data.initial_value.is_valid() {
                collect_expression_calls(
                    program,
                    local_data.initial_value,
                    statement_index,
                    call_ordinal,
                    calls,
                );
            }
        }
        StatementNode::Transition(transition) => {
            if let psi_typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard {
                collect_expression_calls(program, guard, statement_index, call_ordinal, calls);
            }
            collect_transition_target_expression_calls(
                program,
                transition.target,
                statement_index,
                call_ordinal,
                calls,
            );
            if transition.continuation.is_valid() {
                collect_transition_target_expression_calls(
                    program,
                    transition.continuation,
                    statement_index,
                    call_ordinal,
                    calls,
                );
            }
        }
    }
}

fn collect_transition_target_expression_calls(
    program: &TypedTrees,
    target: psi_typed_trees::statement::TransitionTargetHandle,
    statement_index: usize,
    call_ordinal: &mut usize,
    calls: &mut Vec<CallWork>,
) {
    if !target.is_valid() {
        return;
    }
    match program.statement_table.transition_target(target) {
        psi_typed_trees::statement::TransitionTargetNode::Named { arguments, .. } => {
            for argument in program.statement_table.expression_handles(*arguments) {
                collect_expression_calls(program, *argument, statement_index, call_ordinal, calls);
            }
        }
        psi_typed_trees::statement::TransitionTargetNode::Value(expression) => {
            collect_expression_calls(program, *expression, statement_index, call_ordinal, calls);
        }
        psi_typed_trees::statement::TransitionTargetNode::SelfTarget
        | psi_typed_trees::statement::TransitionTargetNode::Terminal => {}
    }
}

fn push_statement_call(
    program: &TypedTrees,
    call: &TableCall,
    statement_index: usize,
    call_ordinal: &mut usize,
    calls: &mut Vec<CallWork>,
) {
    push_call(
        program,
        call.target.as_str(),
        call.target_symbol,
        call.operational_acknowledgement,
        statement_index,
        call_ordinal,
        calls,
    );
    for argument in program.statement_table.expression_handles(call.arguments) {
        collect_expression_calls(program, *argument, statement_index, call_ordinal, calls);
    }
}

fn collect_expression_calls(
    program: &TypedTrees,
    expression: ExpressionHandle,
    statement_index: usize,
    call_ordinal: &mut usize,
    calls: &mut Vec<CallWork>,
) {
    if !expression.is_valid() {
        return;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            collect_expression_calls(program, atomic.value, statement_index, call_ordinal, calls);
        }
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                collect_expression_calls(program, *value, statement_index, call_ordinal, calls);
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_expression_calls(program, binary.left, statement_index, call_ordinal, calls);
            collect_expression_calls(program, binary.right, statement_index, call_ordinal, calls);
        }
        ExpressionNode::Cast(cast) => {
            collect_expression_calls(program, cast.value, statement_index, call_ordinal, calls);
        }
        ExpressionNode::Call(call) => {
            push_expression_call(program, call, statement_index, call_ordinal, calls);
            collect_expression_calls(program, call.receiver, statement_index, call_ordinal, calls);
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_expression_calls(program, *argument, statement_index, call_ordinal, calls);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            collect_expression_calls(
                program,
                indexed.collection,
                statement_index,
                call_ordinal,
                calls,
            );
            collect_expression_calls(program, indexed.index, statement_index, call_ordinal, calls);
        }
        ExpressionNode::Member(member) => {
            collect_expression_calls(
                program,
                member.receiver,
                statement_index,
                call_ordinal,
                calls,
            );
        }
        ExpressionNode::Borrow(inner) => {
            collect_expression_calls(program, inner.target, statement_index, call_ordinal, calls);
        }
        ExpressionNode::Unary(unary) => {
            collect_expression_calls(program, unary.operand, statement_index, call_ordinal, calls);
        }
        ExpressionNode::Range(range) => {
            collect_expression_calls(program, range.start, statement_index, call_ordinal, calls);
            collect_expression_calls(program, range.end, statement_index, call_ordinal, calls);
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                collect_expression_calls(
                    program,
                    field.value,
                    statement_index,
                    call_ordinal,
                    calls,
                );
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

fn push_expression_call(
    program: &TypedTrees,
    call: &TableCallExpression,
    statement_index: usize,
    call_ordinal: &mut usize,
    calls: &mut Vec<CallWork>,
) {
    push_call(
        program,
        call.target.as_str(),
        call.target_symbol,
        call.operational_acknowledgement,
        statement_index,
        call_ordinal,
        calls,
    );
}

fn push_call(
    program: &TypedTrees,
    target_name: &str,
    target_state_symbol: SymbolHandle,
    acknowledgement: psi_language_semantics::CallOperationalAcknowledgement,
    statement_index: usize,
    call_ordinal: &mut usize,
    calls: &mut Vec<CallWork>,
) {
    let target_machine_symbol = machine_symbol_for_state(program, target_state_symbol);
    let direct = direct_operational_for_signature_symbol(program, target_state_symbol);
    calls.push(CallWork {
        statement_index,
        call_ordinal: *call_ordinal,
        target_name: target_name.to_owned(),
        target_state_symbol,
        target_machine_symbol,
        direct_may_suspend: direct.may_suspend,
        direct_may_block: direct.may_block,
        transitive_may_suspend: false,
        transitive_may_block: false,
        acknowledgement,
    });
    *call_ordinal = call_ordinal.checked_add(1).expect("call ordinal overflow");
}

fn machine_symbol_for_state(program: &TypedTrees, state_symbol: SymbolHandle) -> SymbolHandle {
    if !state_symbol.is_valid() {
        return SymbolHandle::invalid();
    }
    program
        .machines()
        .iter()
        .find(|machine| {
            program
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == state_symbol)
        })
        .map(|machine| machine.symbol)
        .unwrap_or_else(SymbolHandle::invalid)
}

fn direct_operational_for_signature_symbol(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> DirectCallOperational {
    if !symbol.is_valid() {
        return DirectCallOperational::default();
    }

    if let Some((_, signature)) = program.machine_parameter_signature(symbol) {
        return signature_operational(program, signature);
    }

    for trait_definition in program.traits() {
        for signature in program.trait_machine_signatures(trait_definition) {
            if signature.symbol == symbol {
                return signature_operational(program, signature);
            }
        }
    }

    DirectCallOperational::default()
}

fn signature_operational(
    program: &TypedTrees,
    signature: &psi_typed_trees::signature::StateSignature,
) -> DirectCallOperational {
    let mut operational = DirectCallOperational {
        may_suspend: signature.suspends,
        may_block: signature.blocks,
    };
    let parameters = program
        .state_signature_parameters(signature)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    for target in crate::declared_signature_invocations(program, signature) {
        let service = match target {
            crate::InvocationTarget::Parameter(index) => {
                parameters.get(index as usize).map(|parameter| {
                    psi_typed_trees::service::exact_bound_service_requirement(
                        program,
                        parameter.type_reference,
                    )
                    .unwrap_or_else(|| {
                        program
                            .type_reference_table
                            .type_reference(parameter.type_reference)
                            .type_symbol(&program.type_reference_table)
                    })
                })
            }
            crate::InvocationTarget::Service(symbol) => Some(symbol),
        };
        let Some(service) = service else {
            continue;
        };
        let Some(trait_definition) = program
            .traits()
            .iter()
            .find(|definition| definition.is_boundary && definition.symbol == service)
        else {
            continue;
        };
        for invoked in program.trait_machine_signatures(trait_definition) {
            operational.may_suspend |= invoked.suspends;
            operational.may_block |= invoked.blocks;
        }
    }
    operational
}

fn propagate_operational_may(machines: &mut [MachineWork]) {
    loop {
        let previous = machines
            .iter()
            .map(|machine| {
                (
                    machine.transitive_may_suspend,
                    machine.transitive_may_block,
                    machine.body_may_suspend,
                    machine.body_may_block,
                )
            })
            .collect::<Vec<_>>();

        for machine_index in 0..machines.len() {
            let mut body_may_suspend = false;
            let mut body_may_block = false;
            for state in &machines[machine_index].states {
                body_may_suspend |= state.direct_may_suspend;
                body_may_block |= state.direct_may_block;
                for call in &state.calls {
                    body_may_suspend |= call.direct_may_suspend;
                    body_may_block |= call.direct_may_block;
                    if let Some(target) = machines
                        .iter()
                        .find(|machine| machine.symbol == call.target_machine_symbol)
                    {
                        if target.uses_published_contract {
                            body_may_suspend |= target.published_may_suspend;
                            body_may_block |= target.published_may_block;
                        } else {
                            body_may_suspend |= target.body_may_suspend;
                            body_may_block |= target.body_may_block;
                        }
                    }
                }
            }
            machines[machine_index].body_may_suspend = body_may_suspend;
            machines[machine_index].body_may_block = body_may_block;
            if machines[machine_index].uses_published_contract {
                machines[machine_index].transitive_may_suspend =
                    machines[machine_index].published_may_suspend;
                machines[machine_index].transitive_may_block =
                    machines[machine_index].published_may_block;
            } else {
                machines[machine_index].transitive_may_suspend = body_may_suspend;
                machines[machine_index].transitive_may_block = body_may_block;
            }
        }

        if machines
            .iter()
            .map(|machine| {
                (
                    machine.transitive_may_suspend,
                    machine.transitive_may_block,
                    machine.body_may_suspend,
                    machine.body_may_block,
                )
            })
            .eq(previous)
        {
            break;
        }
    }

    let summaries = machines
        .iter()
        .map(|machine| {
            (
                machine.symbol,
                machine.transitive_may_suspend,
                machine.transitive_may_block,
            )
        })
        .collect::<Vec<_>>();

    for machine in machines {
        for state in &mut machine.states {
            let mut transitive_may_suspend = state.direct_may_suspend;
            let mut transitive_may_block = state.direct_may_block;
            for call in &mut state.calls {
                call.transitive_may_suspend = call.direct_may_suspend;
                call.transitive_may_block = call.direct_may_block;
                if let Some((_, may_suspend, may_block)) = summaries
                    .iter()
                    .find(|(symbol, _, _)| *symbol == call.target_machine_symbol)
                {
                    call.transitive_may_suspend |= may_suspend;
                    call.transitive_may_block |= may_block;
                }
                transitive_may_suspend |= call.transitive_may_suspend;
                transitive_may_block |= call.transitive_may_block;
            }
            state.transitive_may_suspend = transitive_may_suspend;
            state.transitive_may_block = transitive_may_block;
        }
    }
}

fn build_plan(machines: Vec<MachineWork>) -> OperationalPlan {
    let mut plan = OperationalPlan::default();

    for machine in machines {
        let mut states = HandleSpan::empty();
        for state in machine.states {
            let mut calls = HandleSpan::empty();
            for call in state.calls {
                let acknowledgement = if call.acknowledgement.origin
                    == psi_language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized
                {
                    psi_language_semantics::CallOperationalAcknowledgement {
                        acknowledges_suspend: call.transitive_may_suspend,
                        acknowledges_block: call.transitive_may_block,
                        ..call.acknowledgement
                    }
                } else {
                    call.acknowledgement
                };
                plan.calls.append_to_span(
                    &mut calls,
                    CallOperational {
                        statement_index: call.statement_index,
                        call_ordinal: call.call_ordinal,
                        target_name: call.target_name,
                        target_state_symbol: call.target_state_symbol,
                        target_machine_symbol: call.target_machine_symbol,
                        direct_may_suspend: call.direct_may_suspend,
                        direct_may_block: call.direct_may_block,
                        transitive_may_suspend: call.transitive_may_suspend,
                        transitive_may_block: call.transitive_may_block,
                        acknowledgement,
                    },
                );
            }
            plan.states.append_to_span(
                &mut states,
                StateOperational {
                    symbol: state.symbol,
                    direct_may_suspend: state.direct_may_suspend,
                    direct_may_block: state.direct_may_block,
                    transitive_may_suspend: state.transitive_may_suspend,
                    transitive_may_block: state.transitive_may_block,
                    calls,
                },
            );
        }
        plan.machines.append_to_span(
            &mut plan.root_machines,
            MachineOperational {
                symbol: machine.symbol,
                published_may_suspend: machine.published_may_suspend,
                published_may_block: machine.published_may_block,
                transitive_may_suspend: machine.transitive_may_suspend,
                transitive_may_block: machine.transitive_may_block,
                body_may_suspend: machine.body_may_suspend,
                body_may_block: machine.body_may_block,
                states,
            },
        );
    }

    plan
}
