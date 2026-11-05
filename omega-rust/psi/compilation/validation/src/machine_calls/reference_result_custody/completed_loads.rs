//! Locate the completed read which supplied an incoming pointer. Copies keep
//! this occurrence after its former slot changes. This query selects existing
//! source evidence; it grants no permission or new entry assumption.
use arena::Handle;
use checked_trees::{
    CheckFacts, FlowOperandFact, FlowStateFact, IncomingReferenceIdentity, IncomingReferenceOrigin,
};
use facts::PlaceRoot;

mod prepared_slots;
pub use prepared_slots::PreparedDescriptorLoadSlots;

/// Repeated reads can share a descriptor only when their completed captures
/// identify the same unchanged input slot. The pointer's temporary home and
/// opaque referent are different coordinates and cannot stand in for that slot.
pub fn completed_descriptor_load_slot(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    loan: &checked_trees::BorrowLoanFact,
) -> Result<crate::CanonicalPlace, &'static str> {
    if !facts
        .borrow
        .incoming_references
        .is_valid(loan.incoming_reference)
    {
        return Err("the descriptor has no incoming load identity");
    }
    let incoming = facts
        .borrow
        .incoming_references
        .get(loan.incoming_reference);
    let IncomingReferenceOrigin::DescriptorLoad { expression } = incoming.origin else {
        return Err("the descriptor does not originate at a completed load");
    };
    if loan.storage_owner != PlaceRoot::Expression(expression)
        || loan.formation_expression != expression
    {
        return Err("the loan does not own the completed load's descriptor home");
    }
    let state = facts
        .flow
        .control
        .unique_state(incoming.machine_symbol, incoming.state_symbol)
        .ok_or("the descriptor load has no unique completed state")?;
    let loaded = completed_descriptor_load(program, facts, state, loan.incoming_reference)?;
    let capture = facts.flow.control.operands.get(loaded.capture);
    if capture.statement_index != loan.statement_index {
        return Err("the descriptor loan differs from its completed read statement");
    }
    unchanged_input_slot(loaded.sources)
}

fn unchanged_input_slot(
    source: crate::completed_storage::StoredOperand,
) -> Result<crate::CanonicalPlace, &'static str> {
    let mut pending = vec![source];
    let mut selected = None;
    while let Some(source) = pending.pop() {
        match source {
            crate::completed_storage::StoredOperand::Incoming(slot) => {
                if !slot.root.is_valid()
                    || !slot
                        .segments
                        .iter()
                        .all(crate::storage_path_relations::place_segment_has_static_identity)
                {
                    return Err("the input slot has an unresolved captured selector");
                }
                if selected.as_ref().is_some_and(|previous| *previous != slot) {
                    return Err("the completed load can read different input slots");
                }
                selected = Some(slot);
            }
            crate::completed_storage::StoredOperand::Alternatives(alternatives)
                if !alternatives.is_empty() =>
            {
                pending.extend(alternatives);
            }
            _ => return Err("the completed load has no unchanged input-slot source"),
        }
    }
    selected.ok_or("the completed load has no source slot")
}

pub struct CompletedDescriptorLoad {
    pub capture: Handle<FlowOperandFact>,
    pub sources: crate::completed_storage::StoredOperand,
}

fn activation_matches_load(
    facts: &CheckFacts,
    identity: Handle<IncomingReferenceIdentity>,
    expression: typed_trees::expression::ExpressionHandle,
    activation: &checked_trees::FlowBorrowActivationFact,
) -> bool {
    if !facts.borrow.loans.is_valid(activation.loan) {
        return false;
    }
    let loan = facts.borrow.loans.get(activation.loan);
    loan.incoming_reference == identity
        && loan.storage_owner == PlaceRoot::Expression(expression)
        && loan.formation_expression == expression
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
    let mut activations = facts
        .flow
        .borrow_lifetimes
        .activations
        .span_or_empty(state.borrow_activations)
        .iter()
        .filter(|activation| activation_matches_load(facts, identity, expression, activation));
    let activation = activations
        .next()
        .ok_or("the incoming pointer has no actual load activation")?;
    if activations.next().is_some() {
        return Err("the incoming pointer has no unique completed load");
    }
    complete_descriptor_load(program, facts, state, expression, activation)
}

// Selection establishes one matching activation. Both the ordinary query and
// the immutable batch use this same completed-read authentication and source cut.
fn complete_descriptor_load(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    expression: typed_trees::expression::ExpressionHandle,
    activation: &checked_trees::FlowBorrowActivationFact,
) -> Result<CompletedDescriptorLoad, &'static str> {
    let control = &facts.flow.control;
    if !control.operands.is_valid(activation.formation_value) {
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
