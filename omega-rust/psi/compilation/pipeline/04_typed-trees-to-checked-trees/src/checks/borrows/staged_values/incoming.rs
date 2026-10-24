//! Unchanged incoming descriptors keep their admitted permission namespace.
//! This is compatibility supplied at entry, not an inferred backing address.
//! Call only after the frozen storage query reports an unchanged incoming slot;
//! unavailable or replaced slots must never recover an older entry permission.
use arena::Handle;
use checked_trees::{CapturedPlace, CheckFacts, FlowOperandFact, FlowStateFact};

pub(super) fn places(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    operand: Handle<FlowOperandFact>,
    path: &[facts::PlaceSegment],
) -> Option<Vec<CapturedPlace>> {
    let capture = facts.flow.control.operands.get(operand);
    let mut source = validation::canonical_place_from_expression_in_state(
        program,
        capture.state_symbol,
        capture.statement_index,
        capture.expression,
    )?;
    source.segments.extend_from_slice(path);
    let resolved = super::super::resources::call_operands::retained_places(
        program,
        facts,
        state,
        capture.statement_index,
        capture.constraints,
        CapturedPlace {
            incoming_reference: Handle::invalid(),
            root: source.root,
            segments: source.segments,
        },
        &mut Vec::new(),
    )?;
    if resolved.is_empty() {
        return None;
    }
    resolved
        .into_iter()
        .map(|source| {
            if !facts
                .borrow
                .incoming_references
                .is_valid(source.place.incoming_reference)
            {
                return None;
            }
            let incoming = facts
                .borrow
                .incoming_references
                .get(source.place.incoming_reference);
            // A dependency can keep an input alive without identifying its pointer.
            // Invocation namespaces likewise do not inherit joint entry admission.
            if incoming.machine_symbol != state.machine_symbol
                || incoming.state_symbol != capture.state_symbol
                || capture.state_symbol != state.state_symbol
                || !matches!(
                    incoming.origin,
                    checked_trees::IncomingReferenceOrigin::Parameter { .. }
                )
                || source.lineage.iter().any(|loan| {
                    facts.borrow.loans.get(*loan).lineage
                        == checked_trees::BorrowLoanLineage::LifetimeDependency
                })
            {
                return None;
            }
            Some(source.place)
        })
        .collect()
}
