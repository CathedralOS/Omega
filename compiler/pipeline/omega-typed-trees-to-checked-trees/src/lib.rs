use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, NamePath};
use omega_checked_trees::name::ProgramName;
use omega_checked_trees::statement::{
    StatementNode, TableCall, TransitionGuardNode, TransitionTargetNode,
};
use omega_checked_trees::{
    BorrowAccessKind, BorrowArgumentAccessFact, BorrowCallFact, BorrowFacts, BorrowRootKind,
    BorrowWritableRootFact, CheckFacts, ContractProofFact, ContractProofFactKind,
    ContractProofFactOwner, InvariantFact, InvariantFacts, Program, ProofFactKind, ProofFacts,
    ProofObligationFact, ProofObligationOwner, StateBorrowFact,
};
use omega_core::arena::{Handle, HandleSpan};
use omega_core::symbols::SymbolHandle;

pub fn lower_typed_trees(
    program: omega_typed_trees::TypedTrees,
) -> Result<Program, Vec<omega_core::diagnostics::Diagnostic>> {
    omega_validation::validate_program(&program)?;

    let proof_plan = omega_proof::obligations::build_proof_plan(&program);
    omega_proof::checker::check_proof_plan(&proof_plan)?;
    let effects = omega_effects::infer_effects(&program);
    omega_validation::validate_effect_plan(&program, &effects)?;
    let facts = CheckFacts {
        borrow: build_borrow_facts(&program),
        proof: build_proof_facts(&program, &proof_plan),
        invariants: build_invariant_facts(&program),
        effects,
    };

    Ok(Program {
        typed: program,
        facts,
    })
}

pub fn lower_typed_program(
    program: omega_typed_trees::TypedTrees,
) -> Result<Program, Vec<omega_core::diagnostics::Diagnostic>> {
    lower_typed_trees(program)
}

fn build_proof_facts(
    program: &omega_typed_trees::TypedTrees,
    proof_plan: &omega_proof::obligations::ProofPlan,
) -> ProofFacts {
    let mut obligations = omega_core::arena::Arena::with_capacity(proof_plan.obligations.len());
    let mut contract_facts =
        omega_core::arena::Arena::with_capacity(estimated_contract_fact_capacity(program));

    for (_, obligation) in proof_plan.obligations.iter() {
        obligations.append(match obligation {
            omega_proof::obligations::ProofObligation::BoundedAssignment(obligation) => {
                ProofObligationFact {
                    kind: ProofFactKind::BoundedAssignment,
                    machine_symbol: obligation.machine_symbol,
                    state_symbol: obligation.state_symbol,
                    owner: state_owner(obligation.machine_symbol, obligation.state_symbol),
                }
            }
            omega_proof::obligations::ProofObligation::BoundedCallArgument(obligation) => {
                ProofObligationFact {
                    kind: ProofFactKind::BoundedCallArgument,
                    machine_symbol: obligation.machine_symbol,
                    state_symbol: obligation.state_symbol,
                    owner: ProofObligationOwner::CallParameter {
                        machine_symbol: obligation.machine_symbol,
                        state_symbol: obligation.state_symbol,
                        target_symbol: obligation.target_symbol,
                        parameter_symbol: obligation.parameter_symbol,
                    },
                }
            }
            omega_proof::obligations::ProofObligation::BoundedInitializer(obligation) => {
                ProofObligationFact {
                    kind: ProofFactKind::BoundedInitializer,
                    machine_symbol: omega_core::symbols::SymbolHandle::invalid(),
                    state_symbol: omega_core::symbols::SymbolHandle::invalid(),
                    owner: proof_owner(&obligation.owner),
                }
            }
            omega_proof::obligations::ProofObligation::BoundedStateReturn(obligation) => {
                ProofObligationFact {
                    kind: ProofFactKind::BoundedStateReturn,
                    machine_symbol: obligation.machine_symbol,
                    state_symbol: obligation.state_symbol,
                    owner: ProofObligationOwner::StateReturn {
                        machine_symbol: obligation.machine_symbol,
                        state_symbol: obligation.state_symbol,
                    },
                }
            }
            omega_proof::obligations::ProofObligation::BoundedValue(obligation) => {
                ProofObligationFact {
                    kind: ProofFactKind::BoundedValue,
                    machine_symbol: omega_core::symbols::SymbolHandle::invalid(),
                    state_symbol: omega_core::symbols::SymbolHandle::invalid(),
                    owner: proof_owner(&obligation.owner),
                }
            }
            omega_proof::obligations::ProofObligation::BoundedTransitionArgument(obligation) => {
                ProofObligationFact {
                    kind: ProofFactKind::BoundedTransitionArgument,
                    machine_symbol: obligation.machine_symbol,
                    state_symbol: obligation.state_symbol,
                    owner: ProofObligationOwner::TransitionParameter {
                        machine_symbol: obligation.machine_symbol,
                        state_symbol: obligation.state_symbol,
                        parameter_symbol: obligation.parameter_symbol,
                    },
                }
            }
            omega_proof::obligations::ProofObligation::GuardedTransition(obligation) => {
                ProofObligationFact {
                    kind: ProofFactKind::GuardedTransition,
                    machine_symbol: obligation.machine_symbol,
                    state_symbol: obligation.state_symbol,
                    owner: state_owner(obligation.machine_symbol, obligation.state_symbol),
                }
            }
        });
    }

    for machine in program.machines() {
        append_machine_contract_facts(program, machine, &mut contract_facts);
    }

    ProofFacts {
        obligations,
        contract_facts,
    }
}

fn estimated_contract_fact_capacity(program: &omega_typed_trees::TypedTrees) -> usize {
    program
        .machines()
        .iter()
        .map(|machine| {
            program
                .machine_contracts(machine)
                .iter()
                .map(|contract| contract.facts.len())
                .sum::<usize>()
        })
        .sum()
}

fn append_machine_contract_facts(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    contract_facts: &mut omega_core::arena::Arena<ContractProofFact>,
) {
    for contract in program.machine_contracts(machine) {
        for fact in fact_handles(contract.facts) {
            contract_facts.append(ContractProofFact {
                kind: contract_fact_kind(contract.kind),
                owner: ContractProofFactOwner::Machine {
                    machine_symbol: machine.symbol,
                },
                fact,
            });
        }
    }
}

fn fact_handles(
    facts: HandleSpan<omega_typed_trees::domain::ProofFact>,
) -> impl Iterator<Item = Handle<omega_typed_trees::domain::ProofFact>> {
    (0..facts.count()).map(move |offset| {
        Handle::from_parts(
            facts
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("proof fact handle index overflow"),
            facts.start().generation(),
        )
    })
}

fn contract_fact_kind(
    kind: omega_typed_trees::signature::SignatureContractKind,
) -> ContractProofFactKind {
    match kind {
        omega_typed_trees::signature::SignatureContractKind::Requires => {
            ContractProofFactKind::Requires
        }
        omega_typed_trees::signature::SignatureContractKind::Ensures => {
            ContractProofFactKind::Ensures
        }
        omega_typed_trees::signature::SignatureContractKind::Trusted => {
            ContractProofFactKind::Trusted
        }
    }
}

fn state_owner(machine_symbol: SymbolHandle, state_symbol: SymbolHandle) -> ProofObligationOwner {
    ProofObligationOwner::MachineState {
        machine_symbol,
        state_symbol,
    }
}

fn proof_owner(owner: &omega_proof::obligations::ProofObligationOwner) -> ProofObligationOwner {
    match owner {
        omega_proof::obligations::ProofObligationOwner::Unknown => ProofObligationOwner::Unknown,
        omega_proof::obligations::ProofObligationOwner::MachineOwnedData {
            machine_symbol,
            machine: _,
            data_symbol,
            data: _,
        } => ProofObligationOwner::MachineOwnedData {
            machine_symbol: *machine_symbol,
            data_symbol: *data_symbol,
        },
        omega_proof::obligations::ProofObligationOwner::StateParameter {
            machine_symbol,
            machine: _,
            state_symbol,
            state: _,
            parameter_symbol,
            parameter: _,
        } => ProofObligationOwner::StateParameter {
            machine_symbol: *machine_symbol,
            state_symbol: *state_symbol,
            parameter_symbol: *parameter_symbol,
        },
        omega_proof::obligations::ProofObligationOwner::StateReturn {
            machine_symbol,
            machine: _,
            state_symbol,
            state: _,
        } => ProofObligationOwner::StateReturn {
            machine_symbol: *machine_symbol,
            state_symbol: *state_symbol,
        },
    }
}

fn build_invariant_facts(program: &omega_typed_trees::TypedTrees) -> InvariantFacts {
    let mut definitions =
        omega_core::arena::Arena::with_capacity(program.invariant_definitions().len());

    for definition in program.invariant_definitions() {
        definitions.append(InvariantFact {
            symbol: definition.symbol,
            name: definition.name.clone(),
            constraint_count: program
                .type_reference_table
                .constraints(definition.constraints)
                .len(),
        });
    }

    InvariantFacts { definitions }
}

fn build_borrow_facts(program: &omega_typed_trees::TypedTrees) -> BorrowFacts {
    let mut writable_roots =
        omega_core::arena::Arena::with_capacity(estimated_borrow_root_capacity(program));
    let mut argument_accesses =
        omega_core::arena::Arena::with_capacity(program.expression_table.expression_count());
    let mut calls =
        omega_core::arena::Arena::with_capacity(program.statement_table.statement_count());
    let mut states = omega_core::arena::Arena::with_capacity(machine_state_count(program));

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut writable_roots_span = omega_core::arena::HandleSpan::empty();
            for field in attached_data_fields(program, machine) {
                writable_roots.append_to_span(
                    &mut writable_roots_span,
                    BorrowWritableRootFact {
                        symbol: field.symbol,
                        kind: BorrowRootKind::OwnedData,
                    },
                );
            }

            for owned in program.machine_owned_data(machine) {
                writable_roots.append_to_span(
                    &mut writable_roots_span,
                    BorrowWritableRootFact {
                        symbol: owned.symbol,
                        kind: BorrowRootKind::OwnedData,
                    },
                );
            }

            for statement in program.statement_table.statements(state.statement_nodes) {
                let StatementNode::LocalData(local_data) = statement else {
                    continue;
                };
                writable_roots.append_to_span(
                    &mut writable_roots_span,
                    BorrowWritableRootFact {
                        symbol: local_data.symbol,
                        kind: BorrowRootKind::LocalData,
                    },
                );
            }

            for parameter in program
                .state_parameters(state)
                .iter()
                .filter(|parameter| parameter.is_mutable)
            {
                writable_roots.append_to_span(
                    &mut writable_roots_span,
                    BorrowWritableRootFact {
                        symbol: parameter.symbol,
                        kind: BorrowRootKind::MutableParameter,
                    },
                );
            }

            let mut calls_span = omega_core::arena::HandleSpan::empty();
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let mut call_ordinal = 0usize;
                collect_statement_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    statement,
                    &mut call_ordinal,
                    &mut argument_accesses,
                    &mut calls,
                    &mut calls_span,
                );
            }

            let mutable_parameter_count = program
                .state_parameters(state)
                .iter()
                .filter(|parameter| parameter.is_mutable)
                .count();

            states.append(StateBorrowFact {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
                writable_roots: writable_roots_span,
                mutable_parameter_count,
                calls: calls_span,
            });
        }
    }

    BorrowFacts {
        writable_roots,
        argument_accesses,
        calls,
        states,
    }
}

fn machine_state_count(program: &omega_typed_trees::TypedTrees) -> usize {
    program
        .machines()
        .iter()
        .map(|machine| program.machine_states(machine).len())
        .sum()
}

fn attached_data_fields<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
) -> impl Iterator<Item = &'program omega_typed_trees::data::DataField> {
    machine
        .attached_data
        .as_ref()
        .and_then(|attached_data| {
            program
                .data_definitions()
                .iter()
                .find(|definition| definition.name == *attached_data)
        })
        .into_iter()
        .flat_map(|definition| program.data_members(definition).iter())
        .filter_map(|member| match member {
            omega_typed_trees::data::DataMember::Field(field) => Some(field),
            omega_typed_trees::data::DataMember::Variant(_) => None,
        })
}

fn estimated_borrow_root_capacity(program: &omega_typed_trees::TypedTrees) -> usize {
    program
        .machines()
        .iter()
        .map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .map(|state| {
                    let local_data_count = program
                        .statement_table
                        .statements(state.statement_nodes)
                        .iter()
                        .filter(|statement| matches!(statement, StatementNode::LocalData(_)))
                        .count();
                    let mutable_parameter_count = program
                        .state_parameters(state)
                        .iter()
                        .filter(|parameter| parameter.is_mutable)
                        .count();

                    program.machine_owned_data(machine).len()
                        + attached_data_fields(program, machine).count()
                        + local_data_count
                        + mutable_parameter_count
                })
                .sum::<usize>()
        })
        .sum()
}

fn collect_statement_borrow_calls(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    statement_index: usize,
    statement: &StatementNode,
    call_ordinal: &mut usize,
    argument_accesses: &mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    calls: &mut omega_core::arena::Arena<BorrowCallFact>,
    state_calls: &mut omega_core::arena::HandleSpan<BorrowCallFact>,
) {
    match statement {
        StatementNode::Assignment(assignment) => collect_expression_borrow_calls(
            program,
            machine,
            state,
            statement_index,
            call_ordinal,
            assignment.value,
            argument_accesses,
            calls,
            state_calls,
        ),
        StatementNode::Call(call) => {
            if statement_call_can_dispatch_to_machine(program, machine, state, call) {
                let receiver_path = statement_call_receiver_path(program, call);
                append_borrow_call(
                    calls,
                    state_calls,
                    statement_index,
                    *call_ordinal,
                    call.receiver_symbol,
                    call.target_symbol,
                    receiver_path.as_ref(),
                    collect_call_argument_accesses(
                        argument_accesses,
                        &program.expression_table,
                        program.statement_table.expression_handles(call.arguments),
                        machine.symbol,
                    ),
                );
                *call_ordinal += 1;
            }

            for argument in program.statement_table.expression_handles(call.arguments) {
                collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    *argument,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
        }
        StatementNode::Expression(expression) => collect_expression_borrow_calls(
            program,
            machine,
            state,
            statement_index,
            call_ordinal,
            *expression,
            argument_accesses,
            calls,
            state_calls,
        ),
        StatementNode::LocalData(local_data) => {
            if local_data.initial_value.is_valid() {
                collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    local_data.initial_value,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(expression) = transition.guard {
                collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    expression,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }

            collect_transition_target_borrow_calls(
                program,
                machine,
                state,
                statement_index,
                call_ordinal,
                transition.target,
                argument_accesses,
                calls,
                state_calls,
            );

            if transition.continuation.is_valid() {
                collect_transition_target_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    transition.continuation,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
        }
    }
}

fn collect_transition_target_borrow_calls(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    statement_index: usize,
    call_ordinal: &mut usize,
    target: omega_typed_trees::statement::TransitionTargetHandle,
    argument_accesses: &mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    calls: &mut omega_core::arena::Arena<BorrowCallFact>,
    state_calls: &mut omega_core::arena::HandleSpan<BorrowCallFact>,
) {
    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named { arguments, .. } => {
            for argument in program.statement_table.expression_handles(*arguments) {
                collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    *argument,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
        }
        TransitionTargetNode::Value(expression) => collect_expression_borrow_calls(
            program,
            machine,
            state,
            statement_index,
            call_ordinal,
            *expression,
            argument_accesses,
            calls,
            state_calls,
        ),
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
    }
}

fn append_borrow_call(
    calls: &mut omega_core::arena::Arena<BorrowCallFact>,
    state_calls: &mut omega_core::arena::HandleSpan<BorrowCallFact>,
    statement_index: usize,
    call_ordinal: usize,
    receiver_symbol: SymbolHandle,
    target_symbol: SymbolHandle,
    receiver_path: Option<&NamePath>,
    accesses: omega_core::arena::HandleSpan<BorrowArgumentAccessFact>,
) {
    calls.append_to_span(
        state_calls,
        BorrowCallFact {
            statement_index,
            call_ordinal,
            receiver_symbol,
            target_symbol,
            has_receiver: receiver_path.is_some(),
            accesses,
        },
    );
}

fn collect_expression_borrow_calls(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    statement_index: usize,
    call_ordinal: &mut usize,
    expression: ExpressionHandle,
    argument_accesses: &mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    calls: &mut omega_core::arena::Arena<BorrowCallFact>,
    state_calls: &mut omega_core::arena::HandleSpan<BorrowCallFact>,
) {
    match program.expression_table.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    *value,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_expression_borrow_calls(
                program,
                machine,
                state,
                statement_index,
                call_ordinal,
                binary.left,
                argument_accesses,
                calls,
                state_calls,
            );
            collect_expression_borrow_calls(
                program,
                machine,
                state,
                statement_index,
                call_ordinal,
                binary.right,
                argument_accesses,
                calls,
                state_calls,
            );
        }
        ExpressionNode::Call(call) => {
            let (receiver_symbol, receiver_path) = call_receiver_parts(program, call.receiver);
            let is_machine_call = resolve_state_call_target(
                program,
                machine,
                state,
                receiver_symbol,
                call.target_symbol,
                receiver_path.as_deref(),
                &call.target,
            )
            .is_valid()
                || receiver_can_dispatch_to_machine(
                    program,
                    machine,
                    state,
                    receiver_symbol,
                    receiver_path.as_deref(),
                );

            if is_machine_call {
                append_borrow_call(
                    calls,
                    state_calls,
                    statement_index,
                    *call_ordinal,
                    receiver_symbol,
                    call.target_symbol,
                    receiver_path.as_ref(),
                    collect_call_argument_accesses(
                        argument_accesses,
                        &program.expression_table,
                        program.expression_table.expression_handles(call.arguments),
                        machine.symbol,
                    ),
                );
                *call_ordinal += 1;
            }

            if call.receiver.is_valid() {
                collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    call.receiver,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    *argument,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
        }
        ExpressionNode::Cast(cast) => collect_expression_borrow_calls(
            program,
            machine,
            state,
            statement_index,
            call_ordinal,
            cast.value,
            argument_accesses,
            calls,
            state_calls,
        ),
        ExpressionNode::Indexed(indexed) => {
            collect_expression_borrow_calls(
                program,
                machine,
                state,
                statement_index,
                call_ordinal,
                indexed.collection,
                argument_accesses,
                calls,
                state_calls,
            );
            collect_expression_borrow_calls(
                program,
                machine,
                state,
                statement_index,
                call_ordinal,
                indexed.index,
                argument_accesses,
                calls,
                state_calls,
            );
        }
        ExpressionNode::Member(member) => collect_expression_borrow_calls(
            program,
            machine,
            state,
            statement_index,
            call_ordinal,
            member.receiver,
            argument_accesses,
            calls,
            state_calls,
        ),
        ExpressionNode::Mutable(inner_expression) => collect_expression_borrow_calls(
            program,
            machine,
            state,
            statement_index,
            call_ordinal,
            *inner_expression,
            argument_accesses,
            calls,
            state_calls,
        ),
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    field.value,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => {}
    }
}

fn statement_call_can_dispatch_to_machine(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    call: &TableCall,
) -> bool {
    resolve_state_call_target(
        program,
        machine,
        state,
        call.receiver_symbol,
        call.target_symbol,
        statement_call_receiver_members(program, call),
        &call.target,
    )
    .is_valid()
        || receiver_can_dispatch_to_machine(
            program,
            machine,
            state,
            call.receiver_symbol,
            statement_call_receiver_members(program, call),
        )
}

fn statement_call_receiver_members<'a>(
    program: &'a omega_typed_trees::TypedTrees,
    call: &TableCall,
) -> Option<&'a [ProgramName]> {
    (!call.receiver.is_empty()).then(|| program.statement_table.name_path_members(call.receiver))
}

fn statement_call_receiver_path(
    program: &omega_typed_trees::TypedTrees,
    call: &TableCall,
) -> Option<NamePath> {
    let members = statement_call_receiver_members(program, call)?;

    Some(NamePath::resolved_from_iter(
        members.iter().cloned(),
        call.receiver_symbol,
        call.receiver_symbol,
    ))
}

fn call_receiver_parts(
    program: &omega_typed_trees::TypedTrees,
    receiver: ExpressionHandle,
) -> (
    SymbolHandle,
    Option<omega_checked_trees::expression::NamePath>,
) {
    if !receiver.is_valid() {
        return (SymbolHandle::invalid(), None);
    }

    match program.expression_table.expression(receiver) {
        ExpressionNode::Mutable(inner) => call_receiver_parts(program, *inner),
        ExpressionNode::Name(path) => (
            path.symbol,
            Some(NamePath::resolved_from_iter(
                program
                    .expression_table
                    .name_path_members(path.members)
                    .iter()
                    .cloned(),
                path.head_symbol,
                path.symbol,
            )),
        ),
        ExpressionNode::Member(member) => {
            let (_, path) = call_receiver_parts(program, member.receiver);
            let mut path = path.unwrap_or_default();
            path.push_resolved(member.member.clone(), member.member_symbol);
            (member.member_symbol, Some(path))
        }
        _ => (SymbolHandle::invalid(), None),
    }
}

fn resolve_state_call_target(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    receiver_symbol: SymbolHandle,
    target_symbol: SymbolHandle,
    receiver: Option<&[ProgramName]>,
    _target_state: &ProgramName,
) -> SymbolHandle {
    if receiver.is_none() || receiver_symbol == machine.symbol {
        return resolve_state_symbol_in_machine(program, machine, target_symbol);
    }

    if !receiver_symbol.is_valid() {
        return SymbolHandle::invalid();
    }

    if let Some(contained) = program
        .machine_contained_objects(machine)
        .iter()
        .find(|contained| contained.symbol == receiver_symbol)
    {
        let Some(target_machine) = machine_by_symbol(program, contained.type_symbol) else {
            return SymbolHandle::invalid();
        };
        return resolve_state_symbol_in_machine(program, target_machine, target_symbol);
    }

    if let Some(target_machine) = machine_by_symbol(program, receiver_symbol) {
        return resolve_state_symbol_in_machine(program, target_machine, target_symbol);
    }

    let type_symbol = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == receiver_symbol)
        .map(|parameter| {
            machine_symbol_from_type_reference_handle(program, parameter.type_reference)
        })
        .unwrap_or_else(SymbolHandle::invalid);
    if type_symbol.is_valid()
        && let Some(target_machine) = machine_by_symbol(program, type_symbol)
    {
        return resolve_state_symbol_in_machine(program, target_machine, target_symbol);
    }

    if target_symbol.is_valid()
        && program
            .machines()
            .iter()
            .flat_map(|machine| program.machine_states(machine).iter())
            .any(|state| state.symbol == target_symbol)
    {
        return target_symbol;
    }

    SymbolHandle::invalid()
}

fn receiver_can_dispatch_to_machine(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    receiver_symbol: SymbolHandle,
    receiver: Option<&[ProgramName]>,
) -> bool {
    if receiver.is_none() || receiver_symbol == machine.symbol {
        return true;
    }

    if !receiver_symbol.is_valid() {
        return false;
    }

    if program
        .machine_contained_objects(machine)
        .iter()
        .any(|contained| contained.symbol == receiver_symbol)
    {
        return true;
    }

    let type_symbol = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == receiver_symbol)
        .map(|parameter| {
            machine_symbol_from_type_reference_handle(program, parameter.type_reference)
        })
        .unwrap_or_else(SymbolHandle::invalid);

    machine_by_symbol(program, receiver_symbol).is_some()
        || (type_symbol.is_valid() && machine_by_symbol(program, type_symbol).is_some())
}

fn resolve_state_symbol_in_machine(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state_symbol: SymbolHandle,
) -> SymbolHandle {
    if !state_symbol.is_valid() {
        return SymbolHandle::invalid();
    }

    program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)
        .map(|state| state.symbol)
        .unwrap_or_else(SymbolHandle::invalid)
}

fn machine_by_symbol(
    program: &omega_typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> Option<&omega_typed_trees::machine::Machine> {
    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol)
}

fn machine_symbol_from_type_reference_handle(
    program: &omega_typed_trees::TypedTrees,
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
) -> SymbolHandle {
    match program.type_reference_table.type_reference(type_reference) {
        omega_typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            machine_symbol_from_type_reference_handle(program, *referee)
        }
        omega_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            machine_symbol_from_type_reference_handle(program, *base_type)
        }
        omega_typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. }
        | omega_typed_trees::types::TypeReferenceNode::Named {
            symbol: base_symbol,
            ..
        } => *base_symbol,
        omega_typed_trees::types::TypeReferenceNode::FixedArray { .. }
        | omega_typed_trees::types::TypeReferenceNode::Slice { .. }
        | omega_typed_trees::types::TypeReferenceNode::Unit => SymbolHandle::invalid(),
    }
}

fn collect_call_argument_accesses(
    argument_accesses: &mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    expressions: &omega_typed_trees::expression::ExpressionTable,
    arguments: &[ExpressionHandle],
    machine_symbol: SymbolHandle,
) -> omega_core::arena::HandleSpan<BorrowArgumentAccessFact> {
    let mut accesses = omega_core::arena::HandleSpan::empty();

    for argument in arguments {
        collect_argument_accesses(
            *argument,
            expressions,
            argument_accesses,
            &mut accesses,
            machine_symbol,
        );
    }

    accesses
}

fn collect_argument_accesses(
    expression: ExpressionHandle,
    expressions: &omega_typed_trees::expression::ExpressionTable,
    argument_accesses: &mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    accesses: &mut omega_core::arena::HandleSpan<BorrowArgumentAccessFact>,
    machine_symbol: SymbolHandle,
) {
    match expressions.expression(expression) {
        ExpressionNode::Mutable(inner_expression) => {
            if let Some(root_symbol) =
                expression_root_symbol(*inner_expression, expressions, machine_symbol)
            {
                argument_accesses.append_to_span(
                    accesses,
                    BorrowArgumentAccessFact {
                        root_symbol,
                        kind: BorrowAccessKind::Mutable,
                    },
                );
            }
        }
        _ => collect_read_accesses(
            expression,
            expressions,
            argument_accesses,
            accesses,
            machine_symbol,
        ),
    }
}

fn collect_read_accesses(
    expression: ExpressionHandle,
    expressions: &omega_typed_trees::expression::ExpressionTable,
    argument_accesses: &mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    accesses: &mut omega_core::arena::HandleSpan<BorrowArgumentAccessFact>,
    machine_symbol: SymbolHandle,
) {
    match expressions.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => {
            for value in expressions.expression_handles(*values) {
                collect_read_accesses(
                    *value,
                    expressions,
                    argument_accesses,
                    accesses,
                    machine_symbol,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_read_accesses(
                binary.left,
                expressions,
                argument_accesses,
                accesses,
                machine_symbol,
            );
            collect_read_accesses(
                binary.right,
                expressions,
                argument_accesses,
                accesses,
                machine_symbol,
            );
        }
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                collect_read_accesses(
                    call.receiver,
                    expressions,
                    argument_accesses,
                    accesses,
                    machine_symbol,
                );
            }

            for argument in expressions.expression_handles(call.arguments) {
                collect_read_accesses(
                    *argument,
                    expressions,
                    argument_accesses,
                    accesses,
                    machine_symbol,
                );
            }
        }
        ExpressionNode::Cast(cast) => collect_read_accesses(
            cast.value,
            expressions,
            argument_accesses,
            accesses,
            machine_symbol,
        ),
        ExpressionNode::Indexed(indexed) => {
            if let Some(root_symbol) =
                expression_root_symbol(indexed.collection, expressions, machine_symbol)
            {
                argument_accesses.append_to_span(
                    accesses,
                    BorrowArgumentAccessFact {
                        root_symbol,
                        kind: BorrowAccessKind::Read,
                    },
                );
            }

            collect_read_accesses(
                indexed.index,
                expressions,
                argument_accesses,
                accesses,
                machine_symbol,
            );
        }
        ExpressionNode::Member(member) => collect_read_accesses(
            member.receiver,
            expressions,
            argument_accesses,
            accesses,
            machine_symbol,
        ),
        ExpressionNode::Name(path) => {
            if let Some(root_symbol) = first_valid_name_path_symbol(path, expressions) {
                argument_accesses.append_to_span(
                    accesses,
                    BorrowArgumentAccessFact {
                        root_symbol,
                        kind: BorrowAccessKind::Read,
                    },
                );
            }
        }
        ExpressionNode::Mutable(inner_expression) => collect_read_accesses(
            *inner_expression,
            expressions,
            argument_accesses,
            accesses,
            machine_symbol,
        ),
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in expressions.struct_fields(struct_literal.fields) {
                collect_read_accesses(
                    field.value,
                    expressions,
                    argument_accesses,
                    accesses,
                    machine_symbol,
                );
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => {}
    }
}

fn expression_root_symbol(
    expression: ExpressionHandle,
    expressions: &omega_typed_trees::expression::ExpressionTable,
    machine_symbol: SymbolHandle,
) -> Option<SymbolHandle> {
    match expressions.expression(expression) {
        ExpressionNode::Indexed(indexed) => {
            expression_root_symbol(indexed.collection, expressions, machine_symbol)
        }
        ExpressionNode::Member(member) => match expressions.expression(member.receiver) {
            ExpressionNode::Name(path)
                if path.members.count() == 1
                    && path.symbol.is_valid()
                    && path.symbol == machine_symbol =>
            {
                member
                    .member_symbol
                    .is_valid()
                    .then_some(member.member_symbol)
            }
            _ => expression_root_symbol(member.receiver, expressions, machine_symbol),
        },
        ExpressionNode::Name(path) => first_valid_name_path_symbol(path, expressions),
        _ => None,
    }
}

fn first_valid_name_path_symbol(
    path: &omega_typed_trees::expression::TableNamePath,
    expressions: &omega_typed_trees::expression::ExpressionTable,
) -> Option<SymbolHandle> {
    expressions
        .name_path_member_symbols(path.member_symbols)
        .first()
        .copied()
        .filter(|symbol| symbol.is_valid())
        .or_else(|| path.head_symbol.is_valid().then_some(path.head_symbol))
        .or_else(|| path.symbol.is_valid().then_some(path.symbol))
}

#[cfg(test)]
mod tests {
    use super::{build_borrow_facts, build_proof_facts};
    use omega_checked_trees::expression::{CallExpression, Expression, NamePath};
    use omega_checked_trees::machine::Machine;
    use omega_checked_trees::name::ProgramName;
    use omega_checked_trees::signature::{
        SignatureContract, SignatureContractKind, StateParameter,
    };
    use omega_checked_trees::state::State;
    use omega_checked_trees::statement::{StatementNode, TableCall};
    use omega_checked_trees::types::TypeReferenceNode;
    use omega_checked_trees::{ContractProofFactKind, ContractProofFactOwner};
    use omega_core::arena::HandleSpan;
    use omega_core::symbols::SymbolHandle;
    use std::sync::Arc;

    #[test]
    fn carries_machine_contract_facts_into_checked_proof_facts() {
        let machine_symbol = SymbolHandle::from_arena_index(5);

        let mut program = omega_typed_trees::TypedTrees::default();
        let expression = program
            .expression_table
            .insert(omega_typed_trees::expression::ExpressionNode::Boolean(true));
        let fact = program
            .proof_facts
            .append(omega_typed_trees::domain::ProofFact::Expression(expression));
        let mut machine = Machine {
            symbol: machine_symbol,
            name: ProgramName::generated("Main::main"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        program.push_machine_contract(
            &mut machine,
            SignatureContract {
                kind: SignatureContractKind::Requires,
                facts: HandleSpan::from_parts(fact, 1),
                token_count: 1,
            },
        );
        program.push_machine(machine);

        let proof_plan = omega_proof::obligations::build_proof_plan(&program);
        let facts = build_proof_facts(&program, &proof_plan);
        let contract_fact = facts
            .contract_facts
            .iter()
            .next()
            .map(|(_, fact)| fact)
            .expect("checked proof facts should include the machine contract");

        assert_eq!(facts.contract_facts.len(), 1);
        assert_eq!(contract_fact.kind, ContractProofFactKind::Requires);
        assert_eq!(contract_fact.fact, fact);
        assert_eq!(
            contract_fact.owner,
            ContractProofFactOwner::Machine { machine_symbol }
        );
    }

    #[test]
    fn collects_nested_state_call_ordinals_for_checked_borrow_facts() {
        let entry_symbol = SymbolHandle::from_arena_index(1);
        let outer_symbol = SymbolHandle::from_arena_index(2);
        let inner_symbol = SymbolHandle::from_arena_index(3);
        let item_symbol = SymbolHandle::from_arena_index(4);
        let machine_symbol = SymbolHandle::from_arena_index(5);

        let item_argument = Expression::Mutable(Box::new(Expression::Name(NamePath::resolved(
            vec![ProgramName::generated("item")],
            item_symbol,
            item_symbol,
        ))));

        let nested_call = Expression::Call(Box::new(CallExpression {
            receiver: None,
            target_symbol: inner_symbol,
            target: ProgramName::generated("inner"),
            arguments: Arc::from(vec![item_argument].into_boxed_slice()),
        }));

        let mut program = omega_typed_trees::TypedTrees::default();
        let unit_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let nested_call = program.expression_table.insert_tree(&nested_call);
        let mut outer_arguments = Default::default();
        program
            .statement_table
            .push_expression_handle(&mut outer_arguments, nested_call);
        let mut machine = Machine {
            symbol: machine_symbol,
            name: ProgramName::generated("Game"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        let mut entry_state = State {
            symbol: entry_symbol,
            name: ProgramName::generated("entry"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        };
        program.statement_table.push_statement(
            &mut entry_state.statement_nodes,
            StatementNode::Call(TableCall {
                receiver_symbol: SymbolHandle::invalid(),
                target_symbol: outer_symbol,
                receiver: Default::default(),
                target: ProgramName::generated("outer"),
                arguments: outer_arguments,
            }),
        );
        program.push_state_parameter(
            &mut entry_state,
            StateParameter {
                symbol: item_symbol,
                name: ProgramName::generated("item"),
                type_reference: unit_type,
                is_const: false,
                is_mutable: true,
                is_self: false,
            },
        );
        program.push_machine_state(&mut machine, entry_state);
        program.push_machine_state(
            &mut machine,
            State {
                symbol: outer_symbol,
                name: ProgramName::generated("outer"),
                parameters: Default::default(),
                return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
                statement_nodes: Default::default(),
            },
        );
        program.push_machine_state(
            &mut machine,
            State {
                symbol: inner_symbol,
                name: ProgramName::generated("inner"),
                parameters: Default::default(),
                return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
                statement_nodes: Default::default(),
            },
        );
        program.push_machine(machine);

        let facts = build_borrow_facts(&program);
        let state = facts.states.iter().next().map(|(_, state)| state).unwrap();
        let calls = facts.calls.span(state.calls).unwrap();

        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].statement_index, 0);
        assert_eq!(calls[0].call_ordinal, 0);
        assert_eq!(calls[0].target_symbol, outer_symbol);
        assert_eq!(calls[1].statement_index, 0);
        assert_eq!(calls[1].call_ordinal, 1);
        assert_eq!(calls[1].target_symbol, inner_symbol);
    }
}
