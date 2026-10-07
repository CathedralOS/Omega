use crate::semantic::calls::CallSite;
use checked_trees::expression::ExpressionHandle;
use symbols::SymbolHandle;

pub(crate) fn call_site_argument_expressions<'program>(
    program: &'program typed_trees::TypedTrees,
    call_site: &CallSite<'program>,
) -> &'program [ExpressionHandle] {
    match call_site {
        CallSite::Statement(call) => program.statement_table.expression_handles(call.arguments),
        CallSite::Expression { call, .. } => {
            program.expression_table.expression_handles(call.arguments)
        }
        CallSite::TransitionNamed { arguments, .. } => {
            program.statement_table.expression_handles(*arguments)
        }
    }
}

pub(crate) fn find_state_in_machine(
    program: &typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> Option<&typed_trees::state::State> {
    let machine = crate::lookup::machine_by_symbol(program, machine_symbol)?;
    program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)
}

pub(crate) fn find_state(
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
) -> Option<&typed_trees::state::State> {
    find_state_with_machine(program, state_symbol).map(|(_, state)| state)
}

/// The machine whose state list actually stores `state_symbol`, together with
/// the state. The retained symbol parent names the owner directly when it
/// agrees with storage; the whole-program scan still runs when it does not,
/// so both tiers agree with `find_state`. Callers that need the owning
/// machine and the state — write-origin rebasing, reference-result
/// candidates, mutation summaries — take this pair instead of resolving the
/// state and then rescanning every machine for the container.
pub(crate) fn find_state_with_machine(
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
) -> Option<(&typed_trees::machine::Machine, &typed_trees::state::State)> {
    if !state_symbol.is_valid() {
        return None;
    }
    let symbol = program.symbols.get(state_symbol);
    if let Some(machine) = crate::lookup::machine_by_symbol(program, symbol.parent)
        && let Some(state) = program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == state_symbol)
    {
        return Some((machine, state));
    }
    // The whole-program scan is kept for a state whose retained parent
    // disagrees with its storage, and only for symbols that can name one.
    if !program.may_name_stored_state(symbol) {
        return None;
    }
    program.machines().iter().find_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == state_symbol)
            .map(|state| (machine, state))
    })
}

/// A bare machine call names the machine head: `symbol` selects either the
/// machine itself or its entry state, never a later state. The returned pair
/// is the machine and that entry state, so machine-head callers share the
/// entry parameters, contracts, and result type without a second lookup.
/// `find_state_with_machine` alone is wider — it admits a target naming any
/// state inside the machine — so the state arm still verifies the resolved
/// state is stored first.
pub(crate) fn find_machine_head(
    program: &typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> Option<(&typed_trees::machine::Machine, &typed_trees::state::State)> {
    if let Some(machine) = crate::lookup::machine_by_symbol(program, symbol) {
        return program
            .machine_states(machine)
            .first()
            .map(|entry| (machine, entry));
    }
    let (machine, state) = find_state_with_machine(program, symbol)?;
    program
        .machine_states(machine)
        .first()
        .is_some_and(|entry| entry.symbol == state.symbol)
        .then_some((machine, state))
}

/// Resolve one exact call destination and its result contract. A stored
/// body contributes its executable state; a requirement contributes its
/// declaration. States owned by this activation remain internal control edges,
/// including a machine-named re-entry of this activation's entry state.
pub(crate) fn terminal_callable_target(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    target: SymbolHandle,
) -> Option<(SymbolHandle, typed_trees::types::TypeReferenceHandle)> {
    if let Some(owner) = program.machine_by_symbol(target) {
        let entry = program.machine_states(owner).first()?;
        return (owner.symbol != machine.symbol).then_some((entry.symbol, entry.return_type));
    }
    if let Some((owner, state)) = crate::semantic::calls::find_state_with_machine(program, target) {
        return (owner.symbol != machine.symbol).then_some((state.symbol, state.return_type));
    }
    crate::flow::call_target_return_type(program, target).map(|return_type| (target, return_type))
}

/// The machine whose entry state `state_symbol` names, with that state. The
/// scalar call-lowering gates spell a call's target as the entry state
/// itself: `find_machine_head` also admits the machine's own symbol, which
/// is wider than those sites resolve, so they keep this narrower lookup
/// rather than silently growing their admission.
pub(crate) fn find_machine_by_entry_state(
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
) -> Option<(&typed_trees::machine::Machine, &typed_trees::state::State)> {
    program.machines().iter().find_map(|machine| {
        program
            .machine_states(machine)
            .first()
            .filter(|entry| entry.symbol == state_symbol)
            .map(|entry| (machine, entry))
    })
}

/// The machine `symbol` names, either directly or through its retained
/// parent: a call target or member reference may spell the machine itself or
/// one of its member declarations, and the owning machine answers for both.
/// Unlike `find_state_with_machine` this never descends into
/// `machine_states` — the symbol table's parent edge already names the
/// owner — and unlike `find_machine_head` every member resolves, not only
/// the entry state. In a well-formed program the two arms name one machine:
/// a symbol cannot be a machine and a machine's member at once.
pub(crate) fn find_machine(
    program: &typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> Option<&typed_trees::machine::Machine> {
    if !symbol.is_valid() {
        return None;
    }
    let parent = program.symbols.get(symbol).parent;
    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol || machine.symbol == parent)
}

/// The parameter list of a call target: a machine entry or selected state's
/// parameters, or --
/// for a call through a trait-typed receiver (boundary trait machines) or a
/// boundary-trait receiver -- the owning signature's parameters.
pub(crate) fn call_target_parameters(
    program: &typed_trees::TypedTrees,
    target_state_symbol: SymbolHandle,
) -> Option<&[typed_trees::signature::StateParameter]> {
    // A selector call's slot pairs the selector operand and the
    // instantiated requirement formals with the call's operands.
    if let Some(slot) = program.dynamic_slot_by_symbol(target_state_symbol) {
        return Some(&slot.parameters);
    }
    if target_state_symbol.is_valid()
        && let Some(machine) = crate::lookup::machine_by_symbol(program, target_state_symbol)
    {
        return program
            .machine_states(machine)
            .first()
            .map(|entry| program.state_parameters(entry));
    }
    if let Some(state) = find_state(program, target_state_symbol) {
        return Some(program.state_parameters(state));
    }

    if let Some((_, signature)) = program.machine_parameter_signature(target_state_symbol) {
        return Some(program.state_signature_parameters(signature));
    }

    if let Some(operator) = program
        .operators()
        .iter()
        .find(|operator| operator.is_boundary && operator.symbol == target_state_symbol)
    {
        return Some(program.operator_parameters(operator));
    }

    program
        .traits()
        .iter()
        .find_map(|trait_definition| {
            program
                .trait_machine_signatures(trait_definition)
                .iter()
                .find(|signature| signature.symbol == target_state_symbol)
        })
        .map(|signature| program.state_signature_parameters(signature))
}

/// The generic declaration context that owns a call target's formal
/// parameters. Carry and other structural properties must interpret `T`
/// against these bounds, not against an unrelated caller's same-spelled type
/// parameter and not as an invented concrete type.
pub(crate) fn call_target_type_parameters(
    program: &typed_trees::TypedTrees,
    target_state_symbol: SymbolHandle,
) -> &[typed_trees::data::TypeParameter] {
    if let Some(machine) = program.machines().iter().find(|machine| {
        target_state_symbol.is_valid()
            && (machine.symbol == target_state_symbol
                || program
                    .machine_states(machine)
                    .iter()
                    .any(|state| state.symbol == target_state_symbol))
    }) {
        return program.machine_type_parameters(machine);
    }

    if let Some((machine, _)) = program.machine_parameter_signature(target_state_symbol) {
        return program.machine_type_parameters(machine);
    }

    if let Some(operator) = program
        .operators()
        .iter()
        .find(|operator| operator.is_boundary && operator.symbol == target_state_symbol)
    {
        return program.operator_type_parameters(operator);
    }

    if let Some(trait_definition) = program.traits().iter().find(|trait_definition| {
        program
            .trait_machine_signatures(trait_definition)
            .iter()
            .any(|signature| signature.symbol == target_state_symbol)
    }) {
        return program.trait_type_parameters(trait_definition);
    }

    &[]
}
