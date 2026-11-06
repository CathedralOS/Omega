//! Locate the completed read which supplied an incoming pointer. Copies keep
//! this occurrence after its former slot changes. This query selects existing
//! source evidence; it grants no permission or new entry assumption.
use arena::Handle;
use checked_trees::{
    CheckFacts, FlowOperandFact, FlowStateFact, IncomingReferenceIdentity, IncomingReferenceOrigin,
};
use facts::PlaceRoot;

pub struct CompletedDescriptorLoad {
    pub capture: Handle<FlowOperandFact>,
    pub sources: crate::completed_storage::StoredOperand,
}

pub fn completed_descriptor_load(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    identity: Handle<IncomingReferenceIdentity>,
) -> Result<CompletedDescriptorLoad, &'static str> {
    completed_descriptor_load_at_activations(
        program,
        &facts.borrow,
        &facts.flow.control,
        state.machine_symbol,
        state.state_symbol,
        facts
            .flow
            .borrow_lifetimes
            .activations
            .span_or_empty(state.borrow_activations)
            .iter(),
        identity,
    )
}

/// A pointer read keeps its original source after the variable changes. Find
/// that read in the caller's owned activation ledger while its state is still
/// being checked; exact loan and identity membership bind the same event.
#[allow(clippy::too_many_arguments)]
pub fn completed_descriptor_load_at_activations<'a>(
    program: &dyn crate::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    control: &checked_trees::FlowControlFacts,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    activations: impl Iterator<Item = &'a checked_trees::FlowBorrowActivationFact>,
    identity: Handle<IncomingReferenceIdentity>,
) -> Result<CompletedDescriptorLoad, &'static str> {
    if !borrow.incoming_references.is_valid(identity) {
        return Err("the loaded pointer has no checked incoming identity");
    }
    let incoming = borrow.incoming_references.get(identity);
    let IncomingReferenceOrigin::DescriptorLoad { expression } = incoming.origin else {
        return Err("the incoming pointer has no completed descriptor-load source");
    };
    if incoming.machine_symbol != machine || incoming.state_symbol != state {
        return Err("the descriptor load belongs to another invocation");
    }
    let owner = borrow
        .unique_state_borrow(machine, state)
        .ok_or("the descriptor load has no unique borrowing state")?;
    let mut activations = activations.filter(|activation| {
        if !borrow.state_owns_loan(owner, activation.loan) {
            return false;
        }
        let loan = borrow.loans.get(activation.loan);
        loan.incoming_reference == identity
            && loan.storage_owner == PlaceRoot::Expression(expression)
            && loan.formation_expression == expression
    });
    let activation = activations
        .next()
        .ok_or("the incoming pointer has no actual load activation")?;
    if activations.next().is_some() || !control.operands.is_valid(activation.formation_value) {
        return Err("the incoming pointer has no unique completed load");
    }
    let capture = control.operands.get(activation.formation_value);
    let checked_trees::FlowDescriptorPermission::Loaded { loan, read } =
        capture.descriptor_permission
    else {
        return Err("the descriptor activation lost its completed source association");
    };
    if loan != activation.loan
        || capture.expression != expression
        || capture.state_symbol != state
        || !control.value_observations.is_valid(read)
        || !control.operands.is_valid(capture.backing)
    {
        return Err("the descriptor activation lost its completed source association");
    }
    let read = control.value_observations.get(read);
    if read.expression != expression || read.receiver != capture.backing || read.places.is_empty() {
        return Err("the descriptor activation has no authenticated source read");
    }
    let receiver = control.operands.get(capture.backing);
    let source = crate::canonical_place_from_expression_in_state(
        program,
        state,
        capture.statement_index,
        expression,
    )
    .ok_or("the completed descriptor read has no exact source slot")?;
    let parent = crate::canonical_place_from_expression_in_state(
        program,
        state,
        receiver.statement_index,
        receiver.expression,
    )
    .ok_or("the completed descriptor read has no exact receiver slot")?;
    if source.root != parent.root || !source.segments.starts_with(&parent.segments) {
        return Err("the completed descriptor read does not project its actual receiver");
    }
    let sources = crate::completed_storage::reference_sources_at_path(
        program,
        control,
        capture.backing,
        &source.segments[parent.segments.len()..],
        capture.storage_snapshot,
    );
    Ok(CompletedDescriptorLoad {
        capture: activation.formation_value,
        sources,
    })
}
