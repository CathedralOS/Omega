use crate::phase_diagram::PhaseDiagramBuilder;
use crate::service_reach::{append_reach_and_operation_lines, service_names};
use omega_control_flow::{
    ControlFlowPlan, MachineFlow, Operation, OperationExpressionRefs, OperationKind,
    PlannedTransitionTarget, StateBorrowAccessKind, StateBorrowActivation,
    StateBorrowArgumentAccess, StateBorrowCall, StateBorrowEventSource, StateBorrowLoan,
    StateBorrowWeakening, StateBorrowWeakeningReason, StateFlow, StateKey, TransitionFlow,
};
use psi_symbols::SymbolHandle;
use psi_typed_trees::expression::ExpressionHandle;

pub fn control_flow_html(plan: &ControlFlowPlan) -> String {
    let mut diagram = PhaseDiagramBuilder::new("control_flow");
    let mut machine_nodes = Vec::new();
    let mut state_nodes = Vec::new();
    let mut state_scope_nodes = Vec::new();

    for (machine_index, machine) in plan.machines.iter() {
        let states = unique_machine_states(plan, machine);
        let root_keys = visual_root_keys(plan, &states);

        for root_key in &root_keys {
            let Some(root_state) = state_by_key_in_slice(&states, *root_key) else {
                continue;
            };
            let machine_id = diagram.node(
                format!(
                    "machine_{}_{}",
                    machine_index.arena_index(),
                    root_key.state.arena_index()
                ),
                machine_label(plan, machine, root_state, &states, &root_keys),
                "machine",
                machine_index.arena_index() as usize,
            );
            diagram.node_service_reaches(
                &machine_id,
                service_names(
                    &plan.semantics.service_reach.services,
                    &plan.semantics.service_reach.rows,
                    machine.service_reach.transitive,
                ),
            );
            machine_nodes.push((
                machine.symbol,
                machine.name.as_str().to_owned(),
                root_state.name.as_str().to_owned(),
                *root_key,
                machine_id,
            ));
        }

        for state in states.iter().copied() {
            let root_key = root_key_for_state(plan, &states, &root_keys, state.key);
            let Some(machine_id) = machine_id_for_root(&machine_nodes, machine.symbol, root_key)
            else {
                continue;
            };

            let state_id = diagram.node(
                format!(
                    "state_{}_{}_{}",
                    machine_index.arena_index(),
                    state.key.state.arena_index(),
                    state.key.segment_index
                ),
                state_label(plan, machine, state),
                "state_block",
                machine_index.arena_index() as usize,
            );
            diagram.node_service_reaches(
                &state_id,
                service_names(
                    &plan.semantics.service_reach.services,
                    &plan.semantics.service_reach.rows,
                    state.service_reach.transitive,
                ),
            );
            diagram.containment_edge(machine_id, &state_id);
            state_nodes.push((state.key, state_id));
            state_scope_nodes.push((state.key, machine_id.to_owned()));
        }
    }

    for (_, machine) in plan.machines.iter() {
        for state in plan.states.span_or_empty(machine.states) {
            let Some(source_id) = state_id_for_key(&state_nodes, state.key) else {
                continue;
            };
            let source_scope_id = scope_id_for_state(&state_scope_nodes, state.key);

            for operation in plan.operations.span_or_empty(state.operations) {
                if let Some(target_key) = operation_call_target_key(plan, operation) {
                    let Some(target_id) = state_id_for_key(&state_nodes, target_key) else {
                        continue;
                    };
                    let target_scope_id = scope_id_for_state(&state_scope_nodes, target_key);
                    if source_scope_id == target_scope_id {
                        diagram.edge(source_id, target_id, "call");
                    } else if let Some(scope_target_id) = target_scope_id {
                        append_external_call_node(
                            &mut diagram,
                            plan,
                            state.key,
                            source_id,
                            operation,
                            scope_target_id,
                        );
                    }
                } else if let Some(scope_target_id) = operation_external_call_scope_id(
                    plan,
                    &machine_nodes,
                    &state_scope_nodes,
                    machine,
                    operation,
                ) {
                    if Some(scope_target_id) != source_scope_id {
                        append_external_call_node(
                            &mut diagram,
                            plan,
                            state.key,
                            source_id,
                            operation,
                            scope_target_id,
                        );
                    }
                }
            }

            for transition in plan.transitions.span_or_empty(state.transitions) {
                append_transition_edges(&mut diagram, plan, &state_nodes, source_id, transition);
            }
        }
    }

    diagram.finish()
}

fn machine_index_from_key(key: StateKey) -> usize {
    key.machine.arena_index() as usize
}

fn machine_label(
    plan: &ControlFlowPlan,
    machine: &MachineFlow,
    _root_state: &StateFlow,
    _states: &[&StateFlow],
    _root_keys: &[StateKey],
) -> String {
    let attached_data = machine
        .attached_data
        .as_ref()
        .map(|name| name.as_str())
        .unwrap_or("<none>");
    let mut label = format!(
        "machine {}\nattached data: {}\nowned data: {}",
        machine.name.as_str(),
        attached_data,
        plan.machine_owned_data(machine).len(),
    );
    append_reach_and_operation_lines(
        &mut label,
        &plan.semantics.service_reach.services,
        &plan.semantics.service_reach.rows,
        machine.service_reach,
        machine.suspension,
        machine.blocking,
    );
    label
}

fn state_label(plan: &ControlFlowPlan, machine: &MachineFlow, state: &StateFlow) -> String {
    let mut label = format!(
        "{}::{} [block {}]\nparams: {}  mutable params: {}\nborrow: roots {}",
        machine.name.as_str(),
        state.name.as_str(),
        state.key.segment_index,
        state.parameters.len(),
        state.borrow.mutable_parameter_count,
        state.borrow.writable_roots.len(),
    );
    append_reach_and_operation_lines(
        &mut label,
        &plan.semantics.service_reach.services,
        &plan.semantics.service_reach.rows,
        state.service_reach,
        state.suspension,
        state.blocking,
    );

    for call in plan
        .semantics
        .contracts
        .calls
        .span_or_empty(state.contracts.calls)
    {
        label.push('\n');
        label.push_str("  ");
        label.push_str(&format!(
            "contract call #{}.{} requires {} ensures {}",
            call.statement_index,
            call.call_ordinal,
            call.requires.len(),
            call.ensures.len()
        ));
    }

    for exit in plan
        .semantics
        .contracts
        .exits
        .span_or_empty(state.contracts.exits)
    {
        label.push('\n');
        label.push_str("  ");
        label.push_str(&format!(
            "contract exit #{} ensures {}",
            exit.statement_index,
            exit.ensures.len()
        ));
    }

    for borrow_call in plan
        .semantics
        .borrow
        .calls
        .span_or_empty(state.borrow.calls)
    {
        label.push('\n');
        label.push_str("  ");
        label.push_str(&borrow_call_label(plan, machine, state, borrow_call));
    }

    for loan in plan
        .semantics
        .borrow
        .loans
        .span_or_empty(state.borrow.active_loans)
    {
        label.push('\n');
        label.push_str("  ");
        label.push_str(&borrow_loan_label(plan, machine, state, loan));
    }

    for activation in plan
        .semantics
        .borrow
        .activations
        .span_or_empty(state.borrow.activations)
    {
        label.push('\n');
        label.push_str("  ");
        label.push_str(&borrow_activation_label(plan, machine, state, activation));
    }

    for weakening in plan
        .semantics
        .borrow
        .weakenings
        .span_or_empty(state.borrow.weakenings)
    {
        label.push('\n');
        label.push_str("  ");
        label.push_str(&borrow_weakening_label(plan, machine, state, weakening));
    }

    for operation in plan.operations.span_or_empty(state.operations) {
        label.push('\n');
        label.push_str("  ");
        label.push_str(&operation_label(plan, operation));
    }

    for transition in plan.transitions.span_or_empty(state.transitions) {
        label.push('\n');
        label.push_str("  ");
        label.push_str(&transition_label(plan, transition));
    }

    label
}

fn borrow_call_label(
    plan: &ControlFlowPlan,
    machine: &MachineFlow,
    state: &StateFlow,
    call: &StateBorrowCall,
) -> String {
    let accesses = plan
        .semantics
        .borrow
        .argument_accesses
        .span_or_empty(call.accesses)
        .iter()
        .map(|access| borrow_access_label(plan, machine, state, access))
        .collect::<Vec<_>>();
    let access_text = if accesses.is_empty() {
        "<none>".to_owned()
    } else {
        accesses.join(" | ")
    };
    format!(
        "borrow call #{}.{} -> {} [{}]",
        call.statement_index,
        call.call_ordinal,
        state_name_for_symbol(plan, call.target_symbol),
        access_text
    )
}

fn borrow_access_label(
    plan: &ControlFlowPlan,
    machine: &MachineFlow,
    state: &StateFlow,
    access: &StateBorrowArgumentAccess,
) -> String {
    let mut label = borrow_place_label(plan, machine, state, access.root_symbol, access.segments);
    label.push_str(": ");
    label.push_str(match access.kind {
        StateBorrowAccessKind::Read => "read",
        StateBorrowAccessKind::Mutable => "mutable",
        StateBorrowAccessKind::WriteOnly => "write-only",
    });
    label
}

fn borrow_loan_label(
    plan: &ControlFlowPlan,
    machine: &MachineFlow,
    state: &StateFlow,
    loan: &StateBorrowLoan,
) -> String {
    format!(
        "active loan {} -> {} [created {}, last use {}]",
        symbol_name_for_state(plan, machine, state, loan.owner_symbol),
        borrow_place_label(plan, machine, state, loan.root_symbol, loan.segments),
        loan.statement_index,
        loan.last_use_statement_index
    )
}

fn borrow_activation_label(
    plan: &ControlFlowPlan,
    machine: &MachineFlow,
    state: &StateFlow,
    activation: &StateBorrowActivation,
) -> String {
    let loan = plan.semantics.borrow.loans.get(activation.loan);
    format!(
        "activation {} -> {}",
        borrow_event_source_label(plan, activation.source),
        borrow_loan_label(plan, machine, state, loan)
    )
}

fn borrow_weakening_label(
    plan: &ControlFlowPlan,
    machine: &MachineFlow,
    state: &StateFlow,
    weakening: &StateBorrowWeakening,
) -> String {
    let loan = plan.semantics.borrow.loans.get(weakening.loan);
    format!(
        "weakening {} -> {} ({})",
        borrow_event_source_label(plan, weakening.source),
        borrow_loan_label(plan, machine, state, loan),
        borrow_weakening_reason_label(weakening.reason)
    )
}

fn borrow_place_label(
    plan: &ControlFlowPlan,
    machine: &MachineFlow,
    state: &StateFlow,
    root_symbol: SymbolHandle,
    segments: psi_arena::HandleSpan<psi_facts::PlaceSegment>,
) -> String {
    let mut label = symbol_name_for_state(plan, machine, state, root_symbol);
    for segment in plan
        .semantics
        .borrow
        .access_segments
        .span_or_empty(segments)
    {
        match segment {
            psi_facts::PlaceSegment::Field { symbol } => {
                label.push('.');
                label.push_str(&symbol_name_for_state(plan, machine, state, *symbol));
            }
            psi_facts::PlaceSegment::Case { variant } => {
                label.push_str("::");
                label.push_str(&symbol_name_for_state(plan, machine, state, *variant));
            }
            psi_facts::PlaceSegment::FixedIndex { index } => {
                label.push('[');
                label.push_str(&index.to_string());
                label.push(']');
            }
            psi_facts::PlaceSegment::Index { expression } => {
                label.push('[');
                label.push_str(&expression_label(plan, *expression));
                label.push(']');
            }
        }
    }
    label
}

fn borrow_event_source_label(plan: &ControlFlowPlan, source: StateBorrowEventSource) -> String {
    match source {
        StateBorrowEventSource::Statement { statement_index } => {
            format!("statement {statement_index}")
        }
        StateBorrowEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => format!(
            "call #{}.{} -> {}",
            statement_index,
            call_ordinal,
            state_name_for_symbol(plan, target_symbol)
        ),
    }
}

fn borrow_weakening_reason_label(reason: StateBorrowWeakeningReason) -> &'static str {
    match reason {
        StateBorrowWeakeningReason::LastUseExpired => "after last use",
        StateBorrowWeakeningReason::StateExit => "at state exit",
        StateBorrowWeakeningReason::LocalReassigned => "after local reassignment",
    }
}

fn symbol_name_for_state(
    plan: &ControlFlowPlan,
    machine: &MachineFlow,
    state: &StateFlow,
    symbol: SymbolHandle,
) -> String {
    if symbol == machine.symbol {
        return "self".to_owned();
    }

    if let Some(parameter) = plan
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == symbol)
    {
        return parameter.name.as_str().to_owned();
    }

    if let Some(owned) = plan
        .machine_owned_data(machine)
        .iter()
        .find(|owned| owned.symbol == symbol)
    {
        return owned.name.as_str().to_owned();
    }

    if let Some(contained) = plan
        .machine_contains(machine)
        .iter()
        .find(|contained| contained.symbol == symbol)
    {
        return contained.name.as_str().to_owned();
    }

    for candidate_machine in plan.machines.iter().map(|(_, machine)| machine) {
        if candidate_machine.symbol == symbol {
            return candidate_machine.name.as_str().to_owned();
        }
        if let Some(state) = plan
            .states
            .span_or_empty(candidate_machine.states)
            .iter()
            .find(|state| state.key.state == symbol)
        {
            return state.name.as_str().to_owned();
        }
    }

    if symbol.is_valid() {
        format!("#{}", symbol.arena_index())
    } else {
        "invalid".to_owned()
    }
}

fn state_name_for_symbol(plan: &ControlFlowPlan, symbol: SymbolHandle) -> String {
    for machine in plan.machines.iter().map(|(_, machine)| machine) {
        if let Some(state) = plan
            .states
            .span_or_empty(machine.states)
            .iter()
            .find(|state| state.key.state == symbol)
        {
            return format!("{}::{}", machine.name.as_str(), state.name.as_str());
        }
    }

    if symbol.is_valid() {
        format!("#{}", symbol.arena_index())
    } else {
        "invalid".to_owned()
    }
}

fn operation_label(plan: &ControlFlowPlan, operation: &Operation) -> String {
    match &operation.kind {
        OperationKind::Assignment => {
            let OperationExpressionRefs::Assignment { target, value } = operation.expressions
            else {
                return format!("#{} assign", operation.statement_index);
            };
            format!(
                "#{} {} = {}",
                operation.statement_index,
                expression_label(plan, target),
                expression_label(plan, value)
            )
        }
        OperationKind::Call {
            has_receiver,
            receiver,
            target,
            ..
        } => {
            let arguments = match operation.expressions {
                OperationExpressionRefs::Call { arguments } => plan
                    .expressions
                    .expression_handles(arguments)
                    .iter()
                    .map(|argument| expression_label(plan, *argument))
                    .collect::<Vec<_>>()
                    .join(", "),
                _ => String::new(),
            };
            if *has_receiver {
                format!(
                    "#{} call {}.{}({})",
                    operation.statement_index,
                    receiver.as_str(),
                    target.as_str(),
                    arguments
                )
            } else {
                format!(
                    "#{} call {}({})",
                    operation.statement_index,
                    target.as_str(),
                    arguments
                )
            }
        }
        OperationKind::ConstantIntegerAssignment => {
            format!("#{} const-int assign", operation.statement_index)
        }
        OperationKind::Expression => match operation.expressions {
            OperationExpressionRefs::Expression(expression) => format!(
                "#{} expr {}",
                operation.statement_index,
                expression_label(plan, expression)
            ),
            _ => format!("#{} expr", operation.statement_index),
        },
        OperationKind::LocalData => format!("#{} local data", operation.statement_index),
        OperationKind::StaticAssignment => {
            format!("#{} static assign", operation.statement_index)
        }
    }
}

fn transition_label(plan: &ControlFlowPlan, transition: &TransitionFlow) -> String {
    let guard = expression_label_option(plan, transition.expressions.guard)
        .map(|guard| format!(" if {guard}"))
        .unwrap_or_default();
    format!(
        "#{} transition{} -> {}",
        transition.statement_index,
        guard,
        transition_target_label(&transition.target)
    )
}

fn transition_target_label(target: &PlannedTransitionTarget) -> String {
    match target {
        PlannedTransitionTarget::None => "none".to_owned(),
        PlannedTransitionTarget::State { name, .. } => name.to_string(),
        PlannedTransitionTarget::Nested {
            receiver, state, ..
        } => format!("{}.{}", receiver.as_str(), state.as_str()),
        PlannedTransitionTarget::SelfTarget => "self".to_owned(),
        PlannedTransitionTarget::Terminal => "terminal".to_owned(),
    }
}

fn append_transition_edges(
    diagram: &mut PhaseDiagramBuilder,
    plan: &ControlFlowPlan,
    state_nodes: &[(StateKey, String)],
    source_id: &str,
    transition: &TransitionFlow,
) {
    if let Some(target_id) = transition_target_id(plan, state_nodes, &transition.target) {
        diagram.edge(source_id, target_id, "transition_target");
    }

    if let Some(target_id) = transition_target_id(plan, state_nodes, &transition.continuation) {
        diagram.edge(source_id, target_id, "transition_continuation");
    }
}

fn unique_machine_states<'plan>(
    plan: &'plan ControlFlowPlan,
    machine: &MachineFlow,
) -> Vec<&'plan StateFlow> {
    let mut states = Vec::new();
    for state in plan.states.span_or_empty(machine.states) {
        if states
            .iter()
            .any(|existing: &&StateFlow| existing.key == state.key)
        {
            continue;
        }
        states.push(state);
    }
    states
}

fn visual_root_keys(plan: &ControlFlowPlan, states: &[&StateFlow]) -> Vec<StateKey> {
    let mut incoming = Vec::new();

    for state in states {
        for transition in plan.transitions.span_or_empty(state.transitions) {
            for target in [&transition.target, &transition.continuation] {
                if let Some(target_key) = transition_target_key_in_states(states, target) {
                    if target_key != state.key && !incoming.contains(&target_key) {
                        incoming.push(target_key);
                    }
                }
            }
        }
    }

    let mut roots = states
        .iter()
        .filter(|state| !incoming.contains(&state.key))
        .map(|state| state.key)
        .collect::<Vec<_>>();
    if roots.is_empty() {
        if let Some(first) = states.first() {
            roots.push(first.key);
        }
    }
    roots
}

fn root_key_for_state(
    plan: &ControlFlowPlan,
    states: &[&StateFlow],
    root_keys: &[StateKey],
    state_key: StateKey,
) -> StateKey {
    if root_keys.contains(&state_key) {
        return state_key;
    }

    for root_key in root_keys {
        if reaches_state(plan, states, *root_key, state_key) {
            return *root_key;
        }
    }

    root_keys.first().copied().unwrap_or(state_key)
}

fn reaches_state(
    plan: &ControlFlowPlan,
    states: &[&StateFlow],
    root_key: StateKey,
    target_key: StateKey,
) -> bool {
    let mut stack = vec![root_key];
    let mut visited = Vec::new();

    while let Some(key) = stack.pop() {
        if key == target_key {
            return true;
        }
        if visited.contains(&key) {
            continue;
        }
        visited.push(key);

        let Some(state) = state_by_key_in_slice(states, key) else {
            continue;
        };
        for transition in plan.transitions.span_or_empty(state.transitions) {
            for target in [&transition.target, &transition.continuation] {
                if let Some(next_key) = transition_target_key_in_states(states, target) {
                    stack.push(next_key);
                }
            }
        }
    }

    false
}

fn transition_target_key_in_states(
    states: &[&StateFlow],
    target: &PlannedTransitionTarget,
) -> Option<StateKey> {
    match target {
        PlannedTransitionTarget::State { key, .. } => {
            states.iter().any(|state| state.key == *key).then_some(*key)
        }
        PlannedTransitionTarget::Nested { state_symbol, .. } => states
            .iter()
            .find(|state| state.key.state == *state_symbol)
            .map(|state| state.key),
        PlannedTransitionTarget::None
        | PlannedTransitionTarget::SelfTarget
        | PlannedTransitionTarget::Terminal => None,
    }
}

fn state_by_key_in_slice<'states>(
    states: &'states [&StateFlow],
    key: StateKey,
) -> Option<&'states StateFlow> {
    states.iter().copied().find(|state| state.key == key)
}

fn append_external_call_node(
    diagram: &mut PhaseDiagramBuilder,
    plan: &ControlFlowPlan,
    source_key: StateKey,
    source_id: &str,
    operation: &Operation,
    scope_target_id: &str,
) {
    let call_id = diagram.scoped_node(
        format!(
            "external_call_{}_{}_{}_{}",
            source_key.machine.arena_index(),
            source_key.state.arena_index(),
            source_key.segment_index,
            operation.statement_index
        ),
        format!(
            "external call\n{}\n\ndouble-click to scope target",
            operation_label(plan, operation)
        ),
        "external_call",
        machine_index_from_key(source_key),
        scope_target_id,
    );
    diagram.edge(source_id, &call_id, "call");
    diagram.containment_edge(source_id, &call_id);
}

fn operation_call_target_key(plan: &ControlFlowPlan, operation: &Operation) -> Option<StateKey> {
    let OperationKind::Call {
        receiver_symbol,
        target_symbol,
        has_receiver,
        ..
    } = operation.kind
    else {
        return None;
    };

    if has_receiver {
        return plan.state_key_by_symbols(receiver_symbol, target_symbol);
    }

    state_key_for_state_symbol(plan, target_symbol)
}

fn operation_external_call_scope_id<'nodes>(
    plan: &ControlFlowPlan,
    machine_nodes: &'nodes [(SymbolHandle, String, String, StateKey, String)],
    state_scope_nodes: &'nodes [(StateKey, String)],
    source_machine: &MachineFlow,
    operation: &Operation,
) -> Option<&'nodes str> {
    let OperationKind::Call {
        receiver_symbol,
        target_symbol,
        has_receiver,
        receiver,
        target,
    } = &operation.kind
    else {
        return None;
    };

    if *has_receiver {
        return receiver_machine_scope_id(
            plan,
            machine_nodes,
            state_scope_nodes,
            source_machine,
            *receiver_symbol,
            receiver.as_str(),
            target.as_str(),
        );
    }

    if target_symbol.is_valid() {
        if let Some(key) = state_key_for_state_symbol(plan, *target_symbol) {
            return scope_id_for_state(state_scope_nodes, key);
        }
    }

    unique_machine_id_for_state_name(plan, state_scope_nodes, target.as_str())
}

fn receiver_machine_scope_id<'nodes>(
    plan: &ControlFlowPlan,
    machine_nodes: &'nodes [(SymbolHandle, String, String, StateKey, String)],
    state_scope_nodes: &'nodes [(StateKey, String)],
    source_machine: &MachineFlow,
    receiver_symbol: SymbolHandle,
    receiver_name: &str,
    target_name: &str,
) -> Option<&'nodes str> {
    if receiver_symbol == source_machine.symbol
        || names_match(source_machine.name.as_str(), receiver_name)
    {
        return machine_id_for_symbol_and_state_name(
            machine_nodes,
            source_machine.symbol,
            target_name,
        )
        .or_else(|| unique_machine_id_for_state_name(plan, state_scope_nodes, target_name));
    }

    for contained in plan.machine_contains(source_machine) {
        if contained.symbol == receiver_symbol
            || names_match(contained.name.as_str(), receiver_name)
        {
            return machine_id_for_name_and_state_name(
                machine_nodes,
                contained.type_name.as_str(),
                target_name,
            )
            .or_else(|| {
                machine_id_for_symbol_and_state_name(
                    machine_nodes,
                    contained.type_symbol,
                    target_name,
                )
            })
            .or_else(|| unique_machine_id_for_state_name(plan, state_scope_nodes, target_name));
        }
    }

    for owned_data in plan.machine_owned_data(source_machine) {
        if owned_data.symbol == receiver_symbol
            || names_match(owned_data.name.as_str(), receiver_name)
        {
            return unique_machine_id_for_state_name(plan, state_scope_nodes, target_name);
        }
    }

    unique_machine_id_for_state_name(plan, state_scope_nodes, target_name)
}

fn unique_machine_id_for_state_name<'nodes>(
    plan: &ControlFlowPlan,
    state_scope_nodes: &'nodes [(StateKey, String)],
    state_name: &str,
) -> Option<&'nodes str> {
    let mut matches = plan
        .states
        .iter()
        .map(|(_, state)| state)
        .filter(|state| names_match(state.name.as_str(), state_name))
        .filter_map(|state| scope_id_for_state(state_scope_nodes, state.key));
    let first = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(first)
}

fn machine_id_for_root(
    machine_nodes: &[(SymbolHandle, String, String, StateKey, String)],
    symbol: SymbolHandle,
    root_key: StateKey,
) -> Option<&str> {
    machine_nodes
        .iter()
        .find(|(machine_symbol, _, _, candidate_root_key, _)| {
            *machine_symbol == symbol && *candidate_root_key == root_key
        })
        .map(|(_, _, _, _, id)| id.as_str())
}

fn machine_id_for_symbol_and_state_name<'nodes>(
    machine_nodes: &'nodes [(SymbolHandle, String, String, StateKey, String)],
    symbol: SymbolHandle,
    state_name: &str,
) -> Option<&'nodes str> {
    let mut matches = machine_nodes
        .iter()
        .filter(|(machine_symbol, _, root_state_name, _, _)| {
            *machine_symbol == symbol && names_match(root_state_name, state_name)
        })
        .map(|(_, _, _, _, id)| id.as_str());
    let first = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(first)
}

fn machine_id_for_name_and_state_name<'nodes>(
    machine_nodes: &'nodes [(SymbolHandle, String, String, StateKey, String)],
    machine_name: &str,
    state_name: &str,
) -> Option<&'nodes str> {
    let mut matches = machine_nodes
        .iter()
        .filter(|(_, candidate_machine_name, root_state_name, _, _)| {
            names_match(candidate_machine_name, machine_name)
                && names_match(root_state_name, state_name)
        })
        .map(|(_, _, _, _, id)| id.as_str());
    let first = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(first)
}

fn scope_id_for_state(state_scope_nodes: &[(StateKey, String)], key: StateKey) -> Option<&str> {
    state_scope_nodes
        .iter()
        .find(|(candidate, _)| *candidate == key)
        .map(|(_, id)| id.as_str())
}

fn names_match(left: &str, right: &str) -> bool {
    normalized_name(left) == normalized_name(right)
}

fn normalized_name(value: &str) -> String {
    value
        .chars()
        .filter(|ch| *ch != '_' && *ch != ':' && *ch != '.')
        .flat_map(char::to_lowercase)
        .collect()
}

fn transition_target_id<'nodes>(
    plan: &ControlFlowPlan,
    state_nodes: &'nodes [(StateKey, String)],
    target: &PlannedTransitionTarget,
) -> Option<&'nodes str> {
    match target {
        PlannedTransitionTarget::State { key, .. } => state_id_for_key(state_nodes, *key),
        PlannedTransitionTarget::Nested { state_symbol, .. } => {
            state_id_for_state_symbol(plan, state_nodes, *state_symbol)
        }
        PlannedTransitionTarget::None
        | PlannedTransitionTarget::SelfTarget
        | PlannedTransitionTarget::Terminal => None,
    }
}

fn state_id_for_key(state_nodes: &[(StateKey, String)], key: StateKey) -> Option<&str> {
    state_nodes
        .iter()
        .find(|(candidate, _)| *candidate == key)
        .map(|(_, id)| id.as_str())
}

fn state_id_for_state_symbol<'nodes>(
    plan: &ControlFlowPlan,
    state_nodes: &'nodes [(StateKey, String)],
    state_symbol: SymbolHandle,
) -> Option<&'nodes str> {
    let mut matches = state_nodes
        .iter()
        .filter(|(key, _)| key.state == state_symbol && plan.state_by_key(*key).is_some())
        .map(|(_, id)| id.as_str());
    let first = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(first)
}

fn state_key_for_state_symbol(
    plan: &ControlFlowPlan,
    state_symbol: SymbolHandle,
) -> Option<StateKey> {
    let mut matches = plan
        .states
        .iter()
        .map(|(_, state)| state)
        .filter(|state| state.key.state == state_symbol)
        .map(|state| state.key);
    let first = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(first)
}

fn expression_label_option(plan: &ControlFlowPlan, expression: ExpressionHandle) -> Option<String> {
    if expression.is_valid() {
        Some(plan.expressions.display_name(expression))
    } else {
        None
    }
}

fn expression_label(plan: &ControlFlowPlan, expression: ExpressionHandle) -> String {
    expression_label_option(plan, expression).unwrap_or_else(|| "_".to_owned())
}
