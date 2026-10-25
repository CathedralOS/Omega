//! Resolve compatibility geometry from the loans at operand completion.
//! Recasts and placed results can establish exact local backing without a
//! descriptor supplier row. The shared access resolver follows those checked
//! origins and refuses unresolved reference endpoints. Lifetime dependencies
//! alone identify no pointer, and geometry never grants ancestry or access.
//! Incoming parameters retain their separately admitted joint namespace.
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
    if capture.state_symbol != state.state_symbol {
        return None;
    }
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
    );
    let resolved = resolved?;
    if resolved.is_empty() {
        return None;
    }
    resolved
        .into_iter()
        .map(|source| {
            if source.lineage.iter().any(|loan| {
                facts.borrow.loans.get(*loan).lineage
                    == checked_trees::BorrowLoanLineage::LifetimeDependency
            }) {
                return None;
            }
            if source.place.incoming_reference.is_valid() {
                let incoming = facts
                    .borrow
                    .incoming_references
                    .get(source.place.incoming_reference);
                // Invocation namespaces do not inherit joint entry admission.
                if incoming.machine_symbol != state.machine_symbol
                    || incoming.state_symbol != capture.state_symbol
                    || !matches!(
                        incoming.origin,
                        checked_trees::IncomingReferenceOrigin::Parameter { .. }
                    )
                {
                    return None;
                }
            }
            Some(source.place)
        })
        .collect()
}
