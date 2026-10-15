//! Recursive suspension/blocking inference plus normalized call topology.
//!
//! Service reach is deliberately absent. Calls retain stable
//! `(state, statement, ordinal)` identity in grouped arenas so service reach,
//! capability approval, carry checks, and reports can join the same topology
//! without rebuilding leaf `Vec`s or reintroducing a global service catalog.
//! Exact typed named-operator targets remain distinct from machine targets so
//! early execution can validate their declaration authority without inventing
//! a call-closure machine edge.
//! Static argument bindings belong to the same call topology: retaining their
//! exact parameter-to-selection join lets generic summaries substitute rows
//! without recovering arguments from source spelling or a second body walk.

//! Operational summaries share borrow/semantic call-site coordinates. Every
//! named transfer reserves its parent ordinal before argument calls, including
//! internal state transfers that add no machine-call edge. Exact machine-entry
//! and requirement targets contribute to the ordinary recursive summaries.

use arena::HandleSpan;
use symbols::JoinedPath;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableCall};

use flow_effects::{CallOperational, MachineOperational, OperationalPlan, StateOperational};

#[derive(Debug, Clone, PartialEq, Eq)]
struct MachineWork {
    symbol: SymbolHandle,
    family_members: Vec<SymbolHandle>,
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
    static_machine_parameter: SymbolHandle,
    static_machine_arguments: Box<[typed_trees::expression::StaticMachineArgument]>,
    target_machine_symbol: SymbolHandle,
    target_operator_symbol: SymbolHandle,
    direct_may_suspend: bool,
    direct_may_block: bool,
    transitive_may_suspend: bool,
    transitive_may_block: bool,
    acknowledgement: language_semantics::CallOperationalAcknowledgement,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct DirectCallOperational {
    pub(crate) may_suspend: bool,
    pub(crate) may_block: bool,
}

pub fn infer_operational_may(program: &dyn crate::CheckingInput) -> OperationalPlan {
    super::prepared_effects::operational_plan(program)
}

pub(crate) fn infer_operational_may_uncached(
    program: &dyn crate::CheckingInput,
) -> OperationalPlan {
    infer_operational_may_for_cleanup_dependencies(program, &arena::Arena::default())
}

/// Settle the same operational graph with ownership-selected edge invocations.
/// The caller supplies exact custody selection, never inferred source calls.
pub fn infer_operational_may_for_cleanup_dependencies(
    program: &dyn crate::CheckingInput,
    dependencies: &arena::Arena<flow_effects::CleanupDependency>,
) -> OperationalPlan {
    let signature_effects = DirectSignatureEffects::of(program);
    let mut machines = build_machine_work(&signature_effects, program);
    propagate_operational_may(&mut machines, dependencies);
    let mut plan = build_plan(program, machines);
    plan.cleanup_dependencies = dependencies.clone();
    plan
}

fn build_machine_work(
    signature_effects: &DirectSignatureEffects,
    program: &dyn crate::CheckingInput,
) -> Vec<MachineWork> {
    let mut machines = Vec::with_capacity(program.machines().len());

    for machine in program.machines() {
        let family_members = super::family_interface_members(program, machine)
            .iter()
            .map(|member| member.machine)
            .collect::<Vec<_>>();
        let uses_published_contract = family_members.is_empty()
            && (machine.is_public
                || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody);
        let states = program
            .machine_states(machine)
            .iter()
            .map(|state| StateWork {
                symbol: state.symbol,
                direct_may_suspend: false,
                direct_may_block: false,
                transitive_may_suspend: false,
                transitive_may_block: false,
                calls: collect_state_calls(signature_effects, program, machine.symbol, state),
            })
            .collect();

        let published = machine_operational(program, machine);
        machines.push(MachineWork {
            symbol: machine.symbol,
            family_members,
            uses_published_contract,
            published_may_suspend: published.may_suspend,
            published_may_block: published.may_block,
            transitive_may_suspend: uses_published_contract && published.may_suspend,
            transitive_may_block: uses_published_contract && published.may_block,
            body_may_suspend: false,
            body_may_block: false,
            states,
        });
    }

    machines
}

fn collect_state_calls(
    signature_effects: &DirectSignatureEffects,
    program: &dyn crate::CheckingInput,
    machine: SymbolHandle,
    state: &State,
) -> Vec<CallWork> {
    let mut calls = Vec::new();

    for (statement_index, statement) in program
        .statement_table
        .indexed_statements(state.statement_nodes)
    {
        // Call-site identity is (state, statement, ordinal-within-statement),
        // shared with borrow, flow, contracts, and diagnostics.
        let mut call_ordinal = 0usize;
        collect_statement_calls(
            signature_effects,
            program,
            machine,
            statement,
            statement_index,
            &mut call_ordinal,
            &mut calls,
        );
    }

    // Evidence-child targets are lexical coordinates for public requirement
    // signatures, not executable states. Borrow/flow already retain that
    // public identity. Resolve it here while its declaring machine is known,
    // so effect attachment joins the same call rather than losing both its
    // acknowledgement and the requirement's suspension/blocking envelope.
    // Invalid evidence applications remain for ordinary source diagnostics;
    // this association grants no implementation or conformance authority.
    if let Some(owner) = program.machine_by_symbol(machine) {
        for call in &mut calls {
            if let Ok(Some(requirement)) = crate::named_conformance_target_requirement(
                program,
                owner,
                call.target_state_symbol,
            ) {
                call.target_state_symbol = requirement.signature.symbol;
                let direct = signature_effects.for_symbol(requirement.signature.symbol);
                call.direct_may_suspend = direct.may_suspend;
                call.direct_may_block = direct.may_block;
            }
        }
    }

    calls
}

fn collect_statement_calls(
    signature_effects: &DirectSignatureEffects,
    program: &dyn crate::CheckingInput,
    machine: SymbolHandle,
    statement: &StatementNode,
    statement_index: usize,
    call_ordinal: &mut usize,
    calls: &mut Vec<CallWork>,
) {
    match statement {
        // Assembly contract facts are proof obligations, never runtime calls.
        StatementNode::RootBinding(binding) => {
            for expression in [binding.receiver, binding.implementation_operand] {
                if expression.is_valid() {
                    collect_expression_calls(
                        signature_effects,
                        program,
                        expression,
                        statement_index,
                        call_ordinal,
                        calls,
                    );
                }
            }
        }
        StatementNode::AssemblyFact(_) => {}
        StatementNode::Assignment(assignment) => {
            collect_expression_calls(
                signature_effects,
                program,
                assignment.target,
                statement_index,
                call_ordinal,
                calls,
            );
            collect_expression_calls(
                signature_effects,
                program,
                assignment.value,
                statement_index,
                call_ordinal,
                calls,
            );
        }
        StatementNode::Call(call) => push_statement_call(
            signature_effects,
            program,
            call,
            statement_index,
            call_ordinal,
            calls,
        ),
        StatementNode::Expression(expression) => collect_expression_calls(
            signature_effects,
            program,
            *expression,
            statement_index,
            call_ordinal,
            calls,
        ),
        StatementNode::LocalData(local_data) => {
            if local_data.initial_value.is_valid() {
                collect_expression_calls(
                    signature_effects,
                    program,
                    local_data.initial_value,
                    statement_index,
                    call_ordinal,
                    calls,
                );
            }
        }
        StatementNode::Transition(transition) => {
            if let typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard {
                collect_expression_calls(
                    signature_effects,
                    program,
                    guard,
                    statement_index,
                    call_ordinal,
                    calls,
                );
            }
            collect_transition_target_expression_calls(
                signature_effects,
                program,
                machine,
                transition.target,
                statement_index,
                call_ordinal,
                calls,
            );
            if transition.continuation.is_valid() {
                collect_transition_target_expression_calls(
                    signature_effects,
                    program,
                    machine,
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
    signature_effects: &DirectSignatureEffects,
    program: &dyn crate::CheckingInput,
    machine: SymbolHandle,
    target: typed_trees::statement::TransitionTargetHandle,
    statement_index: usize,
    call_ordinal: &mut usize,
    calls: &mut Vec<CallWork>,
) {
    if !target.is_valid() {
        return;
    }
    match program.statement_table.transition_target(target) {
        typed_trees::statement::TransitionTargetNode::Named {
            path,
            arguments,
            static_machine_parameter,
            operational_acknowledgement,
            ..
        } => {
            let target = crate::declarations::transitions::named_transition_call_symbol(
                program,
                machine,
                path.symbol,
            );
            if target.is_valid() {
                let target_name = program
                    .statement_table
                    .name_path_members(path.members)
                    .joined_path();
                push_call(
                    signature_effects,
                    program,
                    &target_name,
                    target,
                    SymbolHandle::invalid(),
                    *static_machine_parameter,
                    &[],
                    SymbolHandle::invalid(),
                    *operational_acknowledgement,
                    statement_index,
                    call_ordinal,
                    calls,
                );
            } else {
                // Borrow and semantic call-site traversal reserve a parent
                // coordinate even for an internal named state transfer.
                *call_ordinal = call_ordinal.checked_add(1).expect("call ordinal overflow");
            }
            for argument in program.statement_table.expression_handles(*arguments) {
                collect_expression_calls(
                    signature_effects,
                    program,
                    *argument,
                    statement_index,
                    call_ordinal,
                    calls,
                );
            }
        }
        typed_trees::statement::TransitionTargetNode::Value(expression) => {
            collect_expression_calls(
                signature_effects,
                program,
                *expression,
                statement_index,
                call_ordinal,
                calls,
            );
        }
        typed_trees::statement::TransitionTargetNode::Terminal => {}
    }
}

fn push_statement_call(
    signature_effects: &DirectSignatureEffects,
    program: &dyn crate::CheckingInput,
    call: &TableCall,
    statement_index: usize,
    call_ordinal: &mut usize,
    calls: &mut Vec<CallWork>,
) {
    let target_operator_symbol = {
        let candidates = typed_trees::operator::named_statement_call_candidates(program, call);
        match candidates.as_slice() {
            [operator] => operator.symbol,
            _ => SymbolHandle::invalid(),
        }
    };
    push_call(
        signature_effects,
        program,
        call.target.as_str(),
        call.target_symbol,
        call.static_requirement_dispatch
            .as_ref()
            .map_or(SymbolHandle::invalid(), |dispatch| {
                dispatch.contract_symbol()
            }),
        call.static_machine_parameter,
        &call.machine_arguments,
        target_operator_symbol,
        call.operational_acknowledgement,
        statement_index,
        call_ordinal,
        calls,
    );
    for argument in program.statement_table.expression_handles(call.arguments) {
        collect_expression_calls(
            signature_effects,
            program,
            *argument,
            statement_index,
            call_ordinal,
            calls,
        );
    }
}

fn collect_expression_calls(
    signature_effects: &DirectSignatureEffects,
    program: &dyn crate::CheckingInput,
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
            collect_expression_calls(
                signature_effects,
                program,
                atomic.value,
                statement_index,
                call_ordinal,
                calls,
            );
        }
        ExpressionNode::Call(call) => {
            push_expression_call(
                signature_effects,
                program,
                call,
                statement_index,
                call_ordinal,
                calls,
            );
            collect_expression_calls(
                signature_effects,
                program,
                call.receiver,
                statement_index,
                call_ordinal,
                calls,
            );
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_expression_calls(
                    signature_effects,
                    program,
                    *argument,
                    statement_index,
                    call_ordinal,
                    calls,
                );
            }
        }
        other => crate::expression_traversal::children(program, other, |child| {
            collect_expression_calls(
                signature_effects,
                program,
                child,
                statement_index,
                call_ordinal,
                calls,
            );
        }),
    }
}

fn push_expression_call(
    signature_effects: &DirectSignatureEffects,
    program: &dyn crate::CheckingInput,
    call: &TableCallExpression,
    statement_index: usize,
    call_ordinal: &mut usize,
    calls: &mut Vec<CallWork>,
) {
    if crate::proof_contracts::proof_embeddings::is_exact_embed_call(program, call) {
        // The proof-embedding owner validates this exact builtin's carrier
        // and proof-only occurrence. It forms a value term, not an unresolved
        // operational edge. Reserve its lexical coordinate so nested calls
        // still join the borrow/semantic walk; the caller traverses every
        // operand, so an effectful operand cannot disappear with the term.
        *call_ordinal = call_ordinal.checked_add(1).expect("call ordinal overflow");
        return;
    }
    let named_operator = typed_trees::operator::resolve_named_expression_call(program, call);
    let target_operator_symbol = named_operator
        .map(|operator| operator.symbol)
        .unwrap_or_else(SymbolHandle::invalid);
    // A named boundary use carries no leaf target symbol; borrow and flow
    // record the same resolved operator identity, so the call summary must
    // key on it for the reach/operational attach to join the occurrence.
    let target_state_symbol = named_operator
        .filter(|operator| operator.is_boundary)
        .map_or(call.target_symbol, |operator| operator.symbol);
    push_call(
        signature_effects,
        program,
        call.target.as_str(),
        target_state_symbol,
        call.static_requirement_dispatch
            .as_ref()
            .map_or(SymbolHandle::invalid(), |dispatch| {
                dispatch.contract_symbol()
            }),
        call.static_machine_parameter,
        &call.machine_arguments,
        target_operator_symbol,
        call.operational_acknowledgement,
        statement_index,
        call_ordinal,
        calls,
    );
}

fn push_call(
    signature_effects: &DirectSignatureEffects,
    program: &dyn crate::CheckingInput,
    target_name: &str,
    target_state_symbol: SymbolHandle,
    requirement_symbol: SymbolHandle,
    static_machine_parameter: SymbolHandle,
    static_machine_arguments: &[typed_trees::expression::StaticMachineArgument],
    target_operator_symbol: SymbolHandle,
    acknowledgement: language_semantics::CallOperationalAcknowledgement,
    statement_index: usize,
    call_ordinal: &mut usize,
    calls: &mut Vec<CallWork>,
) {
    let target_machine_symbol = machine_symbol_for_state(program, target_state_symbol);
    // Static selection changes execution, not the requirement's independent
    // acknowledgement envelope. Both nominal and structural binders retain it.
    let direct = program
        .retained_static_machine_contract(static_machine_parameter)
        .map(|contract| signature_operational(program, contract.signature()))
        .unwrap_or_else(|| {
            signature_effects.for_symbol(if requirement_symbol.is_valid() {
                requirement_symbol
            } else {
                target_state_symbol
            })
        });
    calls.push(CallWork {
        statement_index,
        call_ordinal: *call_ordinal,
        target_name: target_name.to_owned(),
        target_state_symbol,
        static_machine_parameter,
        static_machine_arguments: static_machine_arguments.into(),
        target_machine_symbol,
        target_operator_symbol,
        direct_may_suspend: direct.may_suspend,
        direct_may_block: direct.may_block,
        transitive_may_suspend: false,
        transitive_may_block: false,
        acknowledgement,
    });
    *call_ordinal = call_ordinal.checked_add(1).expect("call ordinal overflow");
}

fn machine_symbol_for_state(
    program: &dyn crate::CheckingInput,
    state_symbol: SymbolHandle,
) -> SymbolHandle {
    program
        .machine_holding_state(state_symbol)
        .map(|machine| machine.symbol)
        .unwrap_or_else(SymbolHandle::invalid)
}

/// Signature effects are declaration data for this inference pass. Collection
/// borrows these rows explicitly; nested inference owns another table and
/// cannot clear or replace its caller's answers.
struct DirectSignatureEffects {
    rows: Vec<(SymbolHandle, DirectCallOperational)>,
}

impl DirectSignatureEffects {
    fn of(program: &dyn crate::CheckingInput) -> Self {
        let mut symbols: Vec<_> = program
            .data_type_parameters
            .iter()
            .map(|(_, parameter)| parameter.symbol)
            .chain(
                program
                    .trait_machine_signatures
                    .iter()
                    .map(|(_, signature)| signature.symbol),
            )
            .chain(
                program
                    .closed_requirement_applications()
                    .map(|application| application.signature.symbol),
            )
            .filter(|symbol| symbol.is_valid())
            .collect();
        symbols.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
        symbols.dedup();
        let rows = symbols
            .into_iter()
            .map(|symbol| {
                (
                    symbol,
                    direct_operational_for_signature_symbol_uncached(program, symbol),
                )
            })
            .collect();
        Self { rows }
    }

    fn for_symbol(&self, symbol: SymbolHandle) -> DirectCallOperational {
        self.rows
            .binary_search_by_key(
                &(symbol.arena_index(), symbol.generation()),
                |(symbol, _)| (symbol.arena_index(), symbol.generation()),
            )
            .ok()
            .map(|position| self.rows[position].1)
            .unwrap_or_default()
    }
}

fn direct_operational_for_signature_symbol_uncached(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> DirectCallOperational {
    if let Some((_, signature)) = program.machine_parameter_signature(symbol) {
        return signature_operational(program, signature);
    }

    if let Some((_, signature)) = program.trait_machine_signature(symbol) {
        return signature_operational(program, signature);
    }

    DirectCallOperational::default()
}

pub(crate) fn signature_operational(
    program: &dyn crate::CheckingInput,
    signature: &typed_trees::signature::StateSignature,
) -> DirectCallOperational {
    let mut operational = DirectCallOperational {
        may_suspend: signature.suspends,
        may_block: signature.blocks,
    };
    super::callback_requirements::visit(
        program,
        super::callback_requirements::opaque_signature_parameters(program, signature),
        &super::declared_signature_invocations(program, signature),
        |invoked| {
            operational.may_suspend |= invoked.suspends;
            operational.may_block |= invoked.blocks;
        },
    );
    operational
}

/// Opaque top-level requirements publish the same exact callback envelope as
/// trait requirements. Checked machines derive calls from their body instead;
/// an invocation permission alone must not manufacture a body effect.
pub(crate) fn machine_operational(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> DirectCallOperational {
    let mut operational = DirectCallOperational {
        may_suspend: machine.suspends,
        may_block: machine.blocks,
    };
    if machine.supply_mode.is_boundary_declaration() && machine.has_bodyless_signature() {
        super::callback_requirements::visit(
            program,
            super::callback_requirements::opaque_machine_parameters(program, machine),
            &super::declared_machine_invocations(program, machine),
            |invoked| {
                operational.may_suspend |= invoked.suspends;
                operational.may_block |= invoked.blocks;
            },
        );
    }
    operational
}

fn propagate_operational_may(
    machines: &mut [MachineWork],
    dependencies: &arena::Arena<flow_effects::CleanupDependency>,
) {
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
                        body_may_suspend |= target.transitive_may_suspend;
                        body_may_block |= target.transitive_may_block;
                    }
                }
            }
            for (_, dependency) in dependencies.iter().filter(|(_, dependency)| {
                dependency.caller_machine == machines[machine_index].symbol
            }) {
                if let Some(target) = machines
                    .iter()
                    .find(|machine| machine.symbol == dependency.target_machine)
                {
                    body_may_suspend |= target.transitive_may_suspend;
                    body_may_block |= target.transitive_may_block;
                }
            }
            // A family has no body to validate. Its callable envelope joins
            // member summaries inside this same fixed point, including cycles
            // from members through portable calls back into other families.
            let mut family_may_suspend = false;
            let mut family_may_block = false;
            for member in &machines[machine_index].family_members {
                if let Some(target) = machines.iter().find(|machine| machine.symbol == *member) {
                    family_may_suspend |= target.transitive_may_suspend;
                    family_may_block |= target.transitive_may_block;
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
                machines[machine_index].transitive_may_suspend =
                    body_may_suspend || family_may_suspend;
                machines[machine_index].transitive_may_block = body_may_block || family_may_block;
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
            for (_, dependency) in dependencies.iter().filter(|(_, dependency)| {
                dependency.caller_machine == machine.symbol
                    && dependency.caller_state == state.symbol
            }) {
                if let Some((_, may_suspend, may_block)) = summaries
                    .iter()
                    .find(|(symbol, _, _)| *symbol == dependency.target_machine)
                {
                    transitive_may_suspend |= may_suspend;
                    transitive_may_block |= may_block;
                }
            }
            state.transitive_may_suspend = transitive_may_suspend;
            state.transitive_may_block = transitive_may_block;
        }
    }
}

fn retain_static_machine_bindings(
    program: &dyn crate::CheckingInput,
    target: SymbolHandle,
    arguments: &[typed_trees::expression::StaticMachineArgument],
    bindings: &mut arena::Arena<flow_effects::StaticMachineCallBinding>,
) -> HandleSpan<flow_effects::StaticMachineCallBinding> {
    let mut result = HandleSpan::empty();
    let Some((machine, _)) =
        crate::declarations::transitions::resolved_transition_target_state(program, target)
    else {
        return result;
    };
    let parameters = program.machine_kind_type_parameters(machine);
    // Static arguments occupy category-specific slots, matching specialization:
    // type and const arguments do not shift a machine parameter's ordinal.
    let selected = arguments.iter().filter(|argument| {
        argument.symbol.is_valid()
            && matches!(
                program.symbols.get(argument.symbol).kind,
                symbols::SymbolKind::State | symbols::SymbolKind::MachineParameter
            )
    });
    for (parameter, selected) in parameters.zip(selected) {
        let nested = selected
            .application
            .as_ref()
            .map_or(&[][..], |application| application.arguments.as_ref());
        let arguments = retain_static_machine_bindings(program, selected.symbol, nested, bindings);
        bindings.append_to_span(
            &mut result,
            flow_effects::StaticMachineCallBinding {
                parameter: parameter.symbol,
                selected: selected.symbol,
                arguments,
            },
        );
    }
    result
}

fn build_plan(program: &dyn crate::CheckingInput, machines: Vec<MachineWork>) -> OperationalPlan {
    let mut plan = OperationalPlan::default();

    for machine in machines {
        let mut states = HandleSpan::empty();
        for state in machine.states {
            let mut calls = HandleSpan::empty();
            for call in state.calls {
                let static_machine_bindings = retain_static_machine_bindings(
                    program,
                    call.target_state_symbol,
                    &call.static_machine_arguments,
                    &mut plan.static_machine_bindings,
                );
                let acknowledgement = if call.acknowledgement.origin
                    == language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized
                {
                    language_semantics::CallOperationalAcknowledgement {
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
                        static_machine_parameter: call.static_machine_parameter,
                        static_machine_bindings,
                        target_machine_symbol: call.target_machine_symbol,
                        target_operator_symbol: call.target_operator_symbol,
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
