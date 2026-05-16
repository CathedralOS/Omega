use omega_checked_trees::expression::{Expression, NamePath};
use omega_checked_trees::name::ProgramName;
use omega_checked_trees::statement::{Statement, TransitionGuard, TransitionTarget};
use omega_checked_trees::{
    BorrowAccessKind, BorrowArgumentAccessFact, BorrowCallFact, BorrowFacts, BorrowRootKind,
    BorrowWritableRootFact, CheckFacts, InvariantFact, InvariantFacts, Program, ProofFactKind,
    ProofFacts, ProofObligationFact, StateBorrowFact,
};
use omega_core::symbols::SymbolHandle;

pub fn lower_typed_trees(
    program: &omega_typed_trees::TypedTrees,
) -> Result<Program, Vec<omega_core::diagnostics::Diagnostic>> {
    omega_validation::validate_program(program)?;

    let proof_plan = omega_proof::obligations::build_proof_plan(program);
    omega_proof::checker::check_proof_plan(&proof_plan)?;

    Ok(Program {
        typed: program.clone(),
        facts: CheckFacts {
            borrow: build_borrow_facts(program),
            proof: build_proof_facts(program, &proof_plan),
            invariants: build_invariant_facts(program),
        },
    })
}

pub fn lower_typed_program(
    program: &omega_typed_trees::TypedTrees,
) -> Result<Program, Vec<omega_core::diagnostics::Diagnostic>> {
    lower_typed_trees(program)
}

fn build_proof_facts(
    _program: &omega_typed_trees::TypedTrees,
    proof_plan: &omega_proof::obligations::ProofPlan,
) -> ProofFacts {
    let mut obligations = omega_core::arena::Arena::new();

    for (_, obligation) in proof_plan.obligations.iter() {
        obligations.append(match obligation {
            omega_proof::obligations::ProofObligation::BoundedAssignment(obligation) => {
                ProofObligationFact {
                    kind: ProofFactKind::BoundedAssignment,
                    machine_symbol: obligation.machine_symbol,
                    state_symbol: obligation.state_symbol,
                    owner: format!(
                        "machine `{}` state `{}`",
                        obligation.machine, obligation.state
                    ),
                }
            }
            omega_proof::obligations::ProofObligation::BoundedCallArgument(obligation) => {
                ProofObligationFact {
                    kind: ProofFactKind::BoundedCallArgument,
                    machine_symbol: obligation.machine_symbol,
                    state_symbol: obligation.state_symbol,
                    owner: format!(
                        "machine `{}` state `{}` call `{}` parameter `{}`",
                        obligation.machine,
                        obligation.state,
                        obligation.target,
                        obligation.parameter
                    ),
                }
            }
            omega_proof::obligations::ProofObligation::BoundedInitializer(obligation) => {
                ProofObligationFact {
                    kind: ProofFactKind::BoundedInitializer,
                    machine_symbol: omega_core::symbols::SymbolHandle::invalid(),
                    state_symbol: omega_core::symbols::SymbolHandle::invalid(),
                    owner: obligation.owner.clone(),
                }
            }
            omega_proof::obligations::ProofObligation::BoundedStateReturn(obligation) => {
                ProofObligationFact {
                    kind: ProofFactKind::BoundedStateReturn,
                    machine_symbol: obligation.machine_symbol,
                    state_symbol: obligation.state_symbol,
                    owner: format!(
                        "machine `{}` state `{}` return",
                        obligation.machine, obligation.state
                    ),
                }
            }
            omega_proof::obligations::ProofObligation::BoundedValue(obligation) => {
                ProofObligationFact {
                    kind: ProofFactKind::BoundedValue,
                    machine_symbol: omega_core::symbols::SymbolHandle::invalid(),
                    state_symbol: omega_core::symbols::SymbolHandle::invalid(),
                    owner: obligation.owner.clone(),
                }
            }
            omega_proof::obligations::ProofObligation::BoundedTransitionArgument(obligation) => {
                ProofObligationFact {
                    kind: ProofFactKind::BoundedTransitionArgument,
                    machine_symbol: obligation.machine_symbol,
                    state_symbol: obligation.state_symbol,
                    owner: format!(
                        "machine `{}` state `{}` transition parameter `{}`",
                        obligation.machine, obligation.state, obligation.parameter
                    ),
                }
            }
            omega_proof::obligations::ProofObligation::GuardedTransition(obligation) => {
                ProofObligationFact {
                    kind: ProofFactKind::GuardedTransition,
                    machine_symbol: obligation.machine_symbol,
                    state_symbol: obligation.state_symbol,
                    owner: format!(
                        "machine `{}` state `{}` guard",
                        obligation.machine, obligation.state
                    ),
                }
            }
        });
    }

    ProofFacts { obligations }
}

fn build_invariant_facts(program: &omega_typed_trees::TypedTrees) -> InvariantFacts {
    let mut definitions = omega_core::arena::Arena::new();

    for definition in program.invariant_definitions() {
        definitions.append(InvariantFact {
            symbol: definition.symbol,
            name: definition.name.clone(),
            constraint_count: program
                .type_constraints
                .span_or_empty(definition.constraints)
                .len(),
        });
    }

    InvariantFacts { definitions }
}

fn build_borrow_facts(program: &omega_typed_trees::TypedTrees) -> BorrowFacts {
    let mut writable_roots = omega_core::arena::Arena::new();
    let mut argument_accesses = omega_core::arena::Arena::new();
    let mut calls = omega_core::arena::Arena::new();
    let mut states = omega_core::arena::Arena::new();

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut writable_roots_span = omega_core::arena::HandleSpan::empty();
            for owned in program.machine_owned_data(machine) {
                writable_roots.append_to_span(
                    &mut writable_roots_span,
                    BorrowWritableRootFact {
                        symbol: owned.symbol,
                        name: owned.name.clone(),
                        kind: BorrowRootKind::OwnedData,
                    },
                );
            }

            for statement in program.state_statements(state) {
                let Statement::LocalData(local_data) = statement else {
                    continue;
                };
                writable_roots.append_to_span(
                    &mut writable_roots_span,
                    BorrowWritableRootFact {
                        symbol: local_data.symbol,
                        name: local_data.name.clone(),
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
                        name: parameter.name.clone(),
                        kind: BorrowRootKind::MutableParameter,
                    },
                );
            }

            let mut calls_span = omega_core::arena::HandleSpan::empty();
            for (statement_index, statement) in program.state_statements(state).iter().enumerate() {
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
                machine_name: machine.name.clone(),
                state_symbol: state.symbol,
                state_name: state.name.clone(),
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

fn collect_statement_borrow_calls(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    statement_index: usize,
    statement: &Statement,
    call_ordinal: &mut usize,
    argument_accesses: &mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    calls: &mut omega_core::arena::Arena<BorrowCallFact>,
    state_calls: &mut omega_core::arena::HandleSpan<BorrowCallFact>,
) {
    match statement {
        Statement::Assignment(assignment) => collect_expression_borrow_calls(
            program,
            machine,
            state,
            statement_index,
            call_ordinal,
            &assignment.value,
            argument_accesses,
            calls,
            state_calls,
        ),
        Statement::Call(call) => {
            if statement_call_can_dispatch_to_machine(program, machine, state, call) {
                append_borrow_call(
                    argument_accesses,
                    calls,
                    state_calls,
                    statement_index,
                    *call_ordinal,
                    call.receiver_symbol,
                    call.target_symbol,
                    statement_call_receiver_path(program, call),
                    call.target.clone(),
                    collect_call_argument_accesses(program.call_arguments(call)),
                );
                *call_ordinal += 1;
            }

            for argument in program.call_arguments(call) {
                collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    argument,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
        }
        Statement::Expression(expression) => collect_expression_borrow_calls(
            program,
            machine,
            state,
            statement_index,
            call_ordinal,
            expression,
            argument_accesses,
            calls,
            state_calls,
        ),
        Statement::LocalData(local_data) => {
            if let Some(initial_value) = &local_data.initial_value {
                collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    initial_value,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
        }
        Statement::Transition(transition) => {
            if let TransitionGuard::When(expression) = &transition.guard {
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
                &transition.target,
                argument_accesses,
                calls,
                state_calls,
            );

            if let Some(continuation) = &transition.continuation {
                collect_transition_target_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    continuation,
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
    target: &TransitionTarget,
    argument_accesses: &mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    calls: &mut omega_core::arena::Arena<BorrowCallFact>,
    state_calls: &mut omega_core::arena::HandleSpan<BorrowCallFact>,
) {
    match target {
        TransitionTarget::Named { .. } => {
            for argument in program.transition_target_arguments(target) {
                collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    argument,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
        }
        TransitionTarget::Value(expression) => collect_expression_borrow_calls(
            program,
            machine,
            state,
            statement_index,
            call_ordinal,
            expression,
            argument_accesses,
            calls,
            state_calls,
        ),
        TransitionTarget::SelfTarget | TransitionTarget::Terminal => {}
    }
}

fn append_borrow_call(
    argument_accesses: &mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    calls: &mut omega_core::arena::Arena<BorrowCallFact>,
    state_calls: &mut omega_core::arena::HandleSpan<BorrowCallFact>,
    statement_index: usize,
    call_ordinal: usize,
    receiver_symbol: SymbolHandle,
    target_symbol: SymbolHandle,
    receiver: Option<NamePath>,
    target: ProgramName,
    accesses: Vec<BorrowArgumentAccessFact>,
) {
    let accesses = argument_accesses.insert_many(accesses);
    calls.append_to_span(
        state_calls,
        BorrowCallFact {
            statement_index,
            call_ordinal,
            receiver_symbol,
            target_symbol,
            receiver,
            target,
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
    expression: &Expression,
    argument_accesses: &mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    calls: &mut omega_core::arena::Arena<BorrowCallFact>,
    state_calls: &mut omega_core::arena::HandleSpan<BorrowCallFact>,
) {
    match expression {
        Expression::ArrayLiteral(values) => {
            for value in values {
                collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    value,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
        }
        Expression::Binary(binary) => {
            collect_expression_borrow_calls(
                program,
                machine,
                state,
                statement_index,
                call_ordinal,
                &binary.left,
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
                &binary.right,
                argument_accesses,
                calls,
                state_calls,
            );
        }
        Expression::Call(call) => {
            let (receiver_symbol, receiver_path) = call_receiver_parts(call.receiver.as_deref());
            let is_machine_call = resolve_state_call_target(
                program,
                machine,
                state,
                receiver_symbol,
                call.target_symbol,
                receiver_path.as_deref(),
                &call.target,
            )
            .is_some()
                || receiver_can_dispatch_to_machine(
                    program,
                    machine,
                    state,
                    receiver_symbol,
                    receiver_path.as_deref(),
                );

            if is_machine_call {
                append_borrow_call(
                    argument_accesses,
                    calls,
                    state_calls,
                    statement_index,
                    *call_ordinal,
                    receiver_symbol,
                    call.target_symbol,
                    receiver_path,
                    call.target.clone(),
                    collect_call_argument_accesses(&call.arguments),
                );
                *call_ordinal += 1;
            }

            if let Some(receiver) = &call.receiver {
                collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    receiver,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
            for argument in &call.arguments {
                collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    argument,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
        }
        Expression::Cast(cast) => collect_expression_borrow_calls(
            program,
            machine,
            state,
            statement_index,
            call_ordinal,
            &cast.value,
            argument_accesses,
            calls,
            state_calls,
        ),
        Expression::Indexed(indexed) => {
            collect_expression_borrow_calls(
                program,
                machine,
                state,
                statement_index,
                call_ordinal,
                &indexed.collection,
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
                &indexed.index,
                argument_accesses,
                calls,
                state_calls,
            );
        }
        Expression::Member(member) => collect_expression_borrow_calls(
            program,
            machine,
            state,
            statement_index,
            call_ordinal,
            &member.receiver,
            argument_accesses,
            calls,
            state_calls,
        ),
        Expression::Mutable(inner_expression) => collect_expression_borrow_calls(
            program,
            machine,
            state,
            statement_index,
            call_ordinal,
            inner_expression,
            argument_accesses,
            calls,
            state_calls,
        ),
        Expression::StructLiteral(struct_literal) => {
            for field in &struct_literal.fields {
                collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    &field.value,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
        }
        Expression::Boolean(_)
        | Expression::Float(_)
        | Expression::Integer(_)
        | Expression::Name(_)
        | Expression::String(_) => {}
    }
}

fn statement_call_can_dispatch_to_machine(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    call: &omega_checked_trees::statement::Call,
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
    .is_some()
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
    call: &omega_checked_trees::statement::Call,
) -> Option<&'a [ProgramName]> {
    (!call.receiver.is_empty()).then(|| program.statement_path_members(call.receiver))
}

fn statement_call_receiver_path(
    program: &omega_typed_trees::TypedTrees,
    call: &omega_checked_trees::statement::Call,
) -> Option<NamePath> {
    let members = statement_call_receiver_members(program, call)?;

    Some(NamePath::resolved(
        members.iter().cloned().collect(),
        call.receiver_symbol,
        call.receiver_symbol,
    ))
}

fn call_receiver_parts(
    receiver: Option<&Expression>,
) -> (
    SymbolHandle,
    Option<omega_checked_trees::expression::NamePath>,
) {
    let Some(receiver) = receiver else {
        return (SymbolHandle::invalid(), None);
    };

    match receiver {
        Expression::Mutable(inner) => call_receiver_parts(Some(inner)),
        Expression::Name(path) => (path.symbol(), Some(path.clone())),
        Expression::Member(member) => {
            let (_, path) = call_receiver_parts(Some(&member.receiver));
            let mut path = path.unwrap_or_default();
            path.push(member.member.clone());
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
) -> Option<SymbolHandle> {
    if receiver.is_none() || receiver.is_some_and(|receiver| receiver == ["self"]) {
        return resolve_state_symbol_in_machine(program, machine, target_symbol);
    }

    if !receiver_symbol.is_valid() {
        return None;
    }

    if let Some(contained) = program
        .machine_contained_objects(machine)
        .iter()
        .find(|contained| contained.symbol == receiver_symbol)
    {
        return machine_by_symbol(program, contained.type_symbol).and_then(|target_machine| {
            resolve_state_symbol_in_machine(program, target_machine, target_symbol)
        });
    }

    if let Some(target_machine) = machine_by_symbol(program, receiver_symbol) {
        return resolve_state_symbol_in_machine(program, target_machine, target_symbol);
    }

    if let Some(type_symbol) = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == receiver_symbol)
        .and_then(|parameter| machine_symbol_from_type_reference(&parameter.type_reference))
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
        return Some(target_symbol);
    }

    None
}

fn receiver_can_dispatch_to_machine(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    receiver_symbol: SymbolHandle,
    receiver: Option<&[ProgramName]>,
) -> bool {
    if receiver.is_none() || receiver.is_some_and(|receiver| receiver == ["self"]) {
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

    machine_by_symbol(program, receiver_symbol).is_some()
        || program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.symbol == receiver_symbol)
            .and_then(|parameter| machine_symbol_from_type_reference(&parameter.type_reference))
            .and_then(|type_symbol| machine_by_symbol(program, type_symbol))
            .is_some()
}

fn resolve_state_symbol_in_machine(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state_symbol: SymbolHandle,
) -> Option<SymbolHandle> {
    if !state_symbol.is_valid() {
        return None;
    }

    program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)
        .map(|state| state.symbol)
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

fn machine_symbol_from_type_reference(
    type_reference: &omega_typed_trees::types::TypeReference,
) -> Option<SymbolHandle> {
    match type_reference {
        omega_typed_trees::types::TypeReference::Reference { referee, .. } => {
            machine_symbol_from_type_reference(referee)
        }
        omega_typed_trees::types::TypeReference::Constrained { base_type, .. } => {
            machine_symbol_from_type_reference(base_type)
        }
        omega_typed_trees::types::TypeReference::Generic { base_symbol, .. }
        | omega_typed_trees::types::TypeReference::Named {
            symbol: base_symbol,
            ..
        } => Some(*base_symbol),
        omega_typed_trees::types::TypeReference::FixedArray { .. }
        | omega_typed_trees::types::TypeReference::Slice { .. }
        | omega_typed_trees::types::TypeReference::Unit => None,
    }
}

fn collect_call_argument_accesses(arguments: &[Expression]) -> Vec<BorrowArgumentAccessFact> {
    let mut accesses = Vec::new();

    for argument in arguments {
        collect_argument_accesses(argument, &mut accesses);
    }

    accesses
}

fn collect_argument_accesses(
    expression: &Expression,
    accesses: &mut Vec<BorrowArgumentAccessFact>,
) {
    match expression {
        Expression::Mutable(inner_expression) => {
            if let Some(root_name) = expression_root_name(inner_expression) {
                accesses.push(BorrowArgumentAccessFact {
                    root_name,
                    kind: BorrowAccessKind::Mutable,
                });
            }
        }
        other_expression => collect_read_accesses(other_expression, accesses),
    }
}

fn collect_read_accesses(expression: &Expression, accesses: &mut Vec<BorrowArgumentAccessFact>) {
    match expression {
        Expression::ArrayLiteral(values) => {
            for value in values {
                collect_read_accesses(value, accesses);
            }
        }
        Expression::Binary(binary) => {
            collect_read_accesses(&binary.left, accesses);
            collect_read_accesses(&binary.right, accesses);
        }
        Expression::Call(call) => {
            if let Some(receiver) = &call.receiver {
                collect_read_accesses(receiver, accesses);
            }

            for argument in &call.arguments {
                collect_read_accesses(argument, accesses);
            }
        }
        Expression::Cast(cast) => collect_read_accesses(&cast.value, accesses),
        Expression::Indexed(indexed) => {
            if let Some(root_name) = expression_root_name(&indexed.collection) {
                accesses.push(BorrowArgumentAccessFact {
                    root_name,
                    kind: BorrowAccessKind::Read,
                });
            }

            collect_read_accesses(&indexed.index, accesses);
        }
        Expression::Member(member) => collect_read_accesses(&member.receiver, accesses),
        Expression::Name(path) => {
            if let Some(root_name) = path.first() {
                accesses.push(BorrowArgumentAccessFact {
                    root_name: root_name.clone(),
                    kind: BorrowAccessKind::Read,
                });
            }
        }
        Expression::Mutable(inner_expression) => collect_read_accesses(inner_expression, accesses),
        Expression::StructLiteral(struct_literal) => {
            for field in &struct_literal.fields {
                collect_read_accesses(&field.value, accesses);
            }
        }
        Expression::Boolean(_)
        | Expression::Float(_)
        | Expression::Integer(_)
        | Expression::String(_) => {}
    }
}

fn expression_root_name(expression: &Expression) -> Option<ProgramName> {
    match expression {
        Expression::Indexed(indexed) => expression_root_name(&indexed.collection),
        Expression::Member(member) => match &member.receiver {
            Expression::Name(path)
                if path.len() == 1 && path.first().is_some_and(|name| name.as_str() == "self") =>
            {
                Some(member.member.clone())
            }
            other => expression_root_name(other),
        },
        Expression::Name(path) => path.first().cloned(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::build_borrow_facts;
    use omega_checked_trees::expression::{CallExpression, Expression, NamePath};
    use omega_checked_trees::machine::Machine;
    use omega_checked_trees::name::ProgramName;
    use omega_checked_trees::signature::StateParameter;
    use omega_checked_trees::state::State;
    use omega_checked_trees::statement::{Call, Statement};
    use omega_checked_trees::types::TypeReference;
    use omega_core::symbols::SymbolHandle;

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
            arguments: vec![item_argument],
        }));

        let mut program = omega_typed_trees::TypedTrees::default();
        let mut outer_arguments = Default::default();
        program.push_statement_expression(&mut outer_arguments, nested_call);
        let mut machine = Machine {
            symbol: machine_symbol,
            name: ProgramName::generated("Game"),
            contains: Default::default(),
            owned_data: Default::default(),
            states: Default::default(),
        };
        let mut entry_state = State {
            symbol: entry_symbol,
            name: ProgramName::generated("entry"),
            parameters: Default::default(),
            return_type: None,
            statements: Default::default(),
            statement_nodes: Default::default(),
        };
        program.push_state_statement(
            &mut entry_state,
            Statement::Call(Call {
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
                type_reference: TypeReference::Unit,
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
                return_type: None,
                statements: Default::default(),
                statement_nodes: Default::default(),
            },
        );
        program.push_machine_state(
            &mut machine,
            State {
                symbol: inner_symbol,
                name: ProgramName::generated("inner"),
                parameters: Default::default(),
                return_type: None,
                statements: Default::default(),
                statement_nodes: Default::default(),
            },
        );
        program.push_machine(machine);
        program.rebuild_tables();

        let facts = build_borrow_facts(&program);
        let state = facts.states.iter().next().map(|(_, state)| state).unwrap();
        let calls = facts.calls.span(state.calls).unwrap();

        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].statement_index, 0);
        assert_eq!(calls[0].call_ordinal, 0);
        assert_eq!(calls[0].target, ProgramName::generated("outer"));
        assert_eq!(calls[1].statement_index, 0);
        assert_eq!(calls[1].call_ordinal, 1);
        assert_eq!(calls[1].target, ProgramName::generated("inner"));
    }
}
