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
    if !facts.borrow.incoming_references.is_valid(identity) {
        return Err("the loaded pointer has no checked incoming identity");
    }
    let incoming = facts.borrow.incoming_references.get(identity);
    let IncomingReferenceOrigin::DescriptorLoad { expression } = incoming.origin else {
        return Err("the incoming pointer has no completed descriptor-load source");
    };
    if incoming.machine_symbol != state.machine_symbol
        || incoming.state_symbol != state.state_symbol
    {
        return Err("the descriptor load belongs to another invocation");
    }
    let control = &facts.flow.control;
    let mut activations = facts
        .flow
        .borrow_lifetimes
        .activations
        .span_or_empty(state.borrow_activations)
        .iter()
        .filter(|activation| {
            if !facts.borrow.loans.is_valid(activation.loan) {
                return false;
            }
            let loan = facts.borrow.loans.get(activation.loan);
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
    let descriptor_loan = capture.descriptor_permission.loaded_loan();
    let descriptor_read = capture.descriptor_permission.loaded_read();
    if descriptor_loan != activation.loan
        || capture.expression != expression
        || capture.state_symbol != state.state_symbol
        || !control.value_observations.is_valid(descriptor_read)
        || !control.operands.is_valid(capture.backing)
    {
        return Err("the descriptor activation lost its completed source association");
    }
    let read = control.value_observations.get(descriptor_read);
    if read.expression != expression || read.receiver != capture.backing || read.places.is_empty() {
        return Err("the descriptor activation has no authenticated source read");
    }
    let receiver = control.operands.get(capture.backing);
    let source = crate::canonical_place_from_expression_in_state(
        program,
        state.state_symbol,
        capture.statement_index,
        expression,
    )
    .ok_or("the completed descriptor read has no exact source slot")?;
    let parent = crate::canonical_place_from_expression_in_state(
        program,
        state.state_symbol,
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
