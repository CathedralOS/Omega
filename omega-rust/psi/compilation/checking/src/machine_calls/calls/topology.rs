//! Exact callable topology: machine heads, entry-only calls, named states and requirements.

use symbols::SymbolHandle;

pub fn find_state_in_machine(
    program: &dyn crate::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> Option<&typed_trees::state::State> {
    let machine = program.machine_by_symbol(machine_symbol)?;
    program.state_in_machine(machine, state_symbol)
}

/// The state `state_symbol` names on `machine` — the machine-object form of
/// `find_state_in_machine` for callers already holding the machine itself.
pub fn find_state_on_machine<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state_symbol: SymbolHandle,
) -> Option<&'program typed_trees::state::State> {
    find_state_in_machine(program, machine.symbol, state_symbol)
}

/// The state an exit fact sits on, resolved through the fact's machine.
pub fn find_state_for_exit<'program>(
    program: &'program dyn crate::CheckingInput,
    exit: &checked_trees::FlowExitFact,
) -> Option<&'program typed_trees::state::State> {
    find_state_in_machine(program, exit.machine_symbol, exit.state_symbol)
}

/// The actual storage owner and state, resolved by the typed program's owner query.
pub fn find_state(
    program: &dyn crate::CheckingInput,
    state_symbol: SymbolHandle,
) -> Option<(&typed_trees::machine::Machine, &typed_trees::state::State)> {
    let machine = program.machine_holding_state(state_symbol)?;
    let state = find_state_in_machine(program, machine.symbol, state_symbol)?;
    Some((machine, state))
}

/// A bare machine call names the machine head: `symbol` selects either the
/// machine itself or its entry state, never a later state. The returned pair
/// is the machine and that entry state, so machine-head callers share the
/// entry parameters, contracts, and result type without a second lookup.
/// `find_state` alone is wider — it admits a target naming any
/// state inside the machine — so the state arm still verifies the resolved
/// state is stored first.
pub fn find_machine_head(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<(&typed_trees::machine::Machine, &typed_trees::state::State)> {
    if let Some(machine) = program.machine_by_symbol(symbol) {
        return program
            .machine_states(machine)
            .first()
            .map(|entry| (machine, entry));
    }
    let (machine, state) = find_state(program, symbol)?;
    program
        .machine_states(machine)
        .first()
        .is_some_and(|entry| entry.symbol == state.symbol)
        .then_some((machine, state))
}

/// The state a call or terminal transition target enters: a machine head
/// (the machine itself or its entry state) or an exact named state. A free
/// machine call names the machine, an attached one its entry state; both
/// enter the same body.
pub fn call_target_state(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<&typed_trees::state::State> {
    find_machine_head(program, symbol)
        .map(|(_, state)| state)
        .or_else(|| program.state_by_symbol(symbol))
}

/// Resolve one exact call destination and its result contract. A stored
/// body contributes its executable state; a requirement contributes its
/// declaration. States owned by this activation remain internal control edges,
/// including a machine-named re-entry of this activation's entry state.
pub fn terminal_callable_target(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    target: SymbolHandle,
) -> Option<(SymbolHandle, typed_trees::types::TypeReferenceHandle)> {
    if let Some(owner) = program.machine_by_symbol(target) {
        let entry = program.machine_states(owner).first()?;
        return (owner.symbol != machine.symbol).then_some((entry.symbol, entry.return_type));
    }
    if let Some((owner, state)) = find_state(program, target) {
        return (owner.symbol != machine.symbol).then_some((state.symbol, state.return_type));
    }
    call_target_return_type(program, target).map(|return_type| (target, return_type))
}

/// The machine whose entry state `state_symbol` names, with that state. The
/// scalar call-lowering gates spell a call's target as the entry state
/// itself: `find_machine_head` also admits the machine's own symbol, which
/// is wider than those sites resolve, so they keep this narrower lookup
/// rather than silently growing their admission.
pub fn find_machine_by_entry_state(
    program: &dyn crate::CheckingInput,
    state_symbol: SymbolHandle,
) -> Option<(&typed_trees::machine::Machine, &typed_trees::state::State)> {
    program.machine_by_entry_state_symbol(state_symbol)
}

/// The machine `symbol` names, either directly or through its retained
/// parent: a call target or member reference may spell the machine itself or
/// one of its member declarations, and the owning machine answers for both.
/// Unlike `find_state` this never descends into
/// `machine_states` — the symbol table's parent edge already names the
/// owner — and unlike `find_machine_head` every member resolves, not only
/// the entry state. In a well-formed program the two arms name one machine:
/// a symbol cannot be a machine and a machine's member at once.
pub fn find_machine(
    program: &dyn crate::CheckingInput,
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
pub fn call_target_parameters(
    program: &dyn crate::CheckingInput,
    target_state_symbol: SymbolHandle,
) -> Option<&[typed_trees::signature::StateParameter]> {
    // A selector call's slot pairs the selector operand and the
    // instantiated requirement formals with the call's operands.
    if let Some(slot) = program.dynamic_slot_by_symbol(target_state_symbol) {
        return Some(&slot.parameters);
    }
    if target_state_symbol.is_valid()
        && let Some(machine) = program.machine_by_symbol(target_state_symbol)
    {
        return program
            .machine_states(machine)
            .first()
            .map(|entry| program.state_parameters(entry));
    }
    if let Some(state) = program.state_by_symbol(target_state_symbol) {
        return Some(program.state_parameters(state));
    }

    if let Some((_, signature)) = program.machine_parameter_signature(target_state_symbol) {
        return Some(program.state_signature_parameters(signature));
    }

    if let Some(operator) = program
        .operators()
        .iter()
        .find(|operator| operator.symbol == target_state_symbol)
    {
        return Some(program.operator_parameters(operator));
    }

    super::selected_requirement_signature(program, target_state_symbol)
        .map(|(_, signature)| program.state_signature_parameters(signature))
}

/// The generic declaration context that owns a call target's formal
/// parameters. Carry and other structural properties must interpret `T`
/// against these bounds, not against an unrelated caller's same-spelled type
/// parameter and not as an invented concrete type.
pub fn call_target_type_parameters(
    program: &dyn crate::CheckingInput,
    target_state_symbol: SymbolHandle,
) -> &[typed_trees::data::TypeParameter] {
    if target_state_symbol.is_valid()
        && let Some(machine) = program.machine_by_own_or_state_symbol(target_state_symbol)
    {
        return program.machine_type_parameters(machine);
    }

    if let Some((machine, _)) = program.machine_parameter_signature(target_state_symbol) {
        return program.machine_type_parameters(machine);
    }

    if let Some(operator) = program
        .operators()
        .iter()
        .find(|operator| operator.symbol == target_state_symbol)
    {
        return program.operator_type_parameters(operator);
    }

    if let Some(trait_definition) = program.traits().iter().find(|trait_definition| {
        program
            .trait_machine_signature_by_symbol(trait_definition, target_state_symbol)
            .is_some()
    }) {
        return program.trait_type_parameters(trait_definition);
    }

    &[]
}

pub fn machine_state_count(program: &dyn crate::CheckingInput) -> usize {
    program
        .machines()
        .iter()
        .map(|machine| program.machine_states(machine).len())
        .sum()
}

pub fn machine_state_by_symbols(
    program: &dyn crate::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> Option<(&typed_trees::machine::Machine, &typed_trees::state::State)> {
    let machine = program.machine_by_symbol(machine_symbol)?;
    let state = find_state_in_machine(program, machine.symbol, state_symbol)?;
    Some((machine, state))
}

pub fn call_target_return_type(
    program: &dyn crate::CheckingInput,
    target_state_symbol: SymbolHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    if let Some(state) = program.state_by_symbol(target_state_symbol) {
        return Some(state.return_type);
    }
    if let Some(slot) = program.dynamic_slot_by_symbol(target_state_symbol) {
        return Some(slot.return_type);
    }
    if let Some((_, signature)) = program.machine_parameter_signature(target_state_symbol) {
        return Some(signature.return_type);
    }
    // Requirement calls retain the operator declaration, not a selected
    // provider body. Its result type belongs to Psi even before realization.
    if let Some(operator) =
        typed_trees::operator::declaration_by_symbol(program, target_state_symbol)
    {
        return Some(operator.return_type);
    }
    if let Some(reference) = asm_intrinsic_result_type(program, target_state_symbol) {
        return Some(reference);
    }
    program
        .trait_machine_signature(target_state_symbol)
        .map(|(_, signature)| signature.return_type)
}

/// The asm-block value intrinsics (`in`, `rdmsr`, `pushfq`, `read_crN`,
/// `read_<sysreg>`) carry
/// fixed result types declared by the instruction contract rather than an
/// authored signature: their desugared calls target compiler builtin symbols,
/// which own no state, machine-parameter, or trait row to rejoin above. The
/// atom lookup keeps this a read over seeded symbols — a package cannot supply
/// a substitute signature for an unnameable intrinsic.
fn asm_intrinsic_result_type(
    program: &dyn crate::CheckingInput,
    target: SymbolHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let atom = match program.symbols.builtin_function_for_symbol(target)? {
        symbols::BuiltinFunction::AsmPortIn => symbols::BuiltinTypeAtom::U8,
        symbols::BuiltinFunction::AsmSnapshotFlags
        | symbols::BuiltinFunction::AsmReadMsr
        | symbols::BuiltinFunction::AsmReadCr0
        | symbols::BuiltinFunction::AsmReadCr2
        | symbols::BuiltinFunction::AsmReadCr3
        | symbols::BuiltinFunction::AsmReadCr4
        | symbols::BuiltinFunction::AsmReadSctlrEl1
        | symbols::BuiltinFunction::AsmReadTcrEl1
        | symbols::BuiltinFunction::AsmReadTtbr0El1
        | symbols::BuiltinFunction::AsmReadTtbr1El1
        | symbols::BuiltinFunction::AsmReadMairEl1
        | symbols::BuiltinFunction::AsmReadVbarEl1
        | symbols::BuiltinFunction::AsmReadTpidrEl1
        | symbols::BuiltinFunction::AsmReadEsrEl1
        | symbols::BuiltinFunction::AsmReadFarEl1 => symbols::BuiltinTypeAtom::U64,
        _ => return None,
    };
    let symbol = program
        .symbols
        .child_handles(program.symbols.root())?
        .find(|candidate| program.symbols.builtin_type_atom(*candidate) == Some(atom))?;
    program
        .type_reference_table
        .find_named_type_reference(symbol)
}

/// The callee identity a typed call's flow occurrence keys on. A named
/// `Namespace::requirement(...)` call keeps its authored receiver path and an
/// invalid `target_symbol`; borrow discovery and flow rekey the occurrence by
/// the selected operator declaration's symbol (`resolve_named_expression_call`
/// returns the same declaration a spelled call would target). Any consumer
/// that compares or dispatches on the occurrence's `target_symbol` must
/// resolve the same identity — the raw leaf loses every named operator call
/// to an empty match. A call this cannot name keeps its authored leaf and
/// fails closed at whichever consumer reads it.
pub fn call_target_symbol(
    program: &dyn crate::CheckingInput,
    call: &typed_trees::expression::TableCallExpression,
) -> SymbolHandle {
    typed_trees::operator::resolve_named_expression_call(program, call)
        .map_or(call.target_symbol, |operator| operator.symbol)
}

/// The complete structural roster behind a generated portable interface.
/// Consumers apply their own body/contract eligibility after this exact join;
/// external members belong to the roster just as checked bodies do. This is
/// dependency topology, not an executable dispatch or a host-target selection.
pub(crate) fn target_family_members<'program>(
    program: &'program dyn crate::CheckingInput,
    interface: &typed_trees::machine::Machine,
) -> Option<&'program [typed_trees::machine::TargetMachineFamilyMember]> {
    if !interface.target_family_interface {
        return None;
    }
    let mut families = program
        .target_machine_families
        .iter()
        .filter(|family| family.canonical == interface.symbol);
    let family = families.next()?;
    if families.next().is_some() || family.members.is_empty() {
        return None;
    }
    for (index, member) in family.members.iter().enumerate() {
        let machine = program.machine(member.machine)?;
        if machine.target_family_interface
            || machine.authored_target.as_ref() != Some(&member.target)
            || program.machine_states(machine).is_empty()
            || family.members[..index]
                .iter()
                .any(|prior| prior.machine == member.machine || prior.target == member.target)
        {
            return None;
        }
    }
    Some(&family.members)
}
