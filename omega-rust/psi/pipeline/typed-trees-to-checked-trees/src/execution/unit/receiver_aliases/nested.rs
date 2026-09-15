//! Exact checked custody for linear whole-referent alias chains.

use arena::{Handle, HandleSpan};
use checked_trees::{
    BorrowAccessKind, BorrowLoanFact, BorrowLoanLineage, CheckFacts,
    CheckedBorrowResourceDispositionTarget, CheckedBorrowResourceLifecyclePhase,
    CheckedParentBorrowResource, CheckedReborrowAccessEffect,
    CheckedReborrowContainmentCertificate, CheckedReborrowContainmentKind,
    CheckedReborrowParentEndStatus, CheckedReborrowParentSuspensionBoundary,
    CheckedReborrowResourceDisposition, CheckedReborrowResourceDispositionEvent,
    CheckedReborrowRestorationObligation, CheckedRetiredParentResourceDispositionStep,
    FlowBorrowActivationFact, FlowBorrowWeakeningFact, FlowBorrowWeakeningReason,
    FlowConstraintKind, FlowInvalidationSource, FlowStateFact, ParentLexicalStatusAtChildEnd,
    StateBorrowFact,
};
use symbols::SymbolHandle;

fn span_handle<T>(span: HandleSpan<T>, offset: usize) -> Option<Handle<T>> {
    Some(Handle::from_parts(
        span.start()
            .arena_index()
            .checked_add(u32::try_from(offset).ok()?)?,
        span.start().generation(),
    ))
}

fn lifecycle(
    facts: &CheckFacts,
    flow: &FlowStateFact,
    loan: Handle<BorrowLoanFact>,
) -> Option<(
    Handle<FlowBorrowActivationFact>,
    Handle<FlowBorrowWeakeningFact>,
)> {
    let mut activations = facts
        .flow
        .borrow_lifetimes
        .activations
        .span_or_empty(flow.borrow_activations)
        .iter()
        .enumerate()
        .filter(|(_, row)| row.loan == loan);
    let (activation_offset, _) = activations.next()?;
    if activations.next().is_some() {
        return None;
    }
    let mut weakenings = facts
        .flow
        .borrow_lifetimes
        .weakenings
        .span_or_empty(flow.borrow_weakenings)
        .iter()
        .enumerate()
        .filter(|(_, row)| row.loan == loan);
    let (weakening_offset, _) = weakenings.next()?;
    if weakenings.next().is_some() {
        return None;
    }
    Some((
        span_handle(flow.borrow_activations, activation_offset)?,
        span_handle(flow.borrow_weakenings, weakening_offset)?,
    ))
}

fn resource(
    facts: &CheckFacts,
    loan: Handle<BorrowLoanFact>,
) -> Option<CheckedParentBorrowResource> {
    let mut matches = facts
        .borrow
        .direct_loan_resources
        .iter()
        .filter(|(_, row)| row.loan == loan)
        .map(|(resource, _)| CheckedParentBorrowResource::DirectRoot { resource })
        .chain(
            facts
                .borrow
                .reborrow_loan_resources
                .iter()
                .filter(|(_, row)| row.loan == loan)
                .map(|(resource, _)| CheckedParentBorrowResource::Reborrow { resource }),
        );
    let result = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(result)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn formation(
    facts: &CheckFacts,
    flow: &FlowStateFact,
    state: &StateBorrowFact,
    statement_index: usize,
    owner: SymbolHandle,
    source_owner: SymbolHandle,
    projection: &[facts::PlaceSegment],
    parent_loan: Handle<BorrowLoanFact>,
    access: &BorrowAccessKind,
    statement_count: usize,
) -> Option<(Handle<BorrowLoanFact>, usize)> {
    let parent = facts.borrow.loans.get(parent_loan);
    let parent_place = checked_trees::CapturedPlace {
        root_symbol: parent.root_symbol,
        segments: facts.borrow.loan_segments(parent).to_vec(),
    };
    let mut place = parent_place.clone();
    place.segments.extend_from_slice(projection);
    let mut loans = facts.borrow.loans.iter().filter(|(handle, loan)| {
        facts.borrow.state_owns_loan(state, *handle) && loan.owner_symbol == owner
    });
    let (loan_handle, loan) = loans.next()?;
    if loans.next().is_some()
        || loan.statement_index != statement_index
        || loan.lineage != (BorrowLoanLineage::Reborrow { parent_loan })
        || loan.source_owner_symbol != source_owner
        || &loan.kind != access
        || loan.root_symbol != place.root_symbol
        || facts.borrow.loan_segments(loan) != place.segments
        || !facts.borrow.loan_owner_path(loan).is_empty()
        || parent.owner_symbol != source_owner
        || parent.kind.direct_reborrow_effect(access)
            != Some(CheckedReborrowAccessEffect::ExclusiveSuspension)
        || parent.statement_index >= statement_index
        || parent.last_use_statement_index != statement_index
        || loan.last_use_statement_index <= statement_index
    {
        return None;
    }
    let CheckedParentBorrowResource::Reborrow {
        resource: child_resource,
    } = resource(facts, loan_handle)?
    else {
        return None;
    };
    let parent_resource = resource(facts, parent_loan)?;
    let child = facts.borrow.reborrow_loan_resources.get(child_resource);
    let (child_activation, child_weakening) = lifecycle(facts, flow, loan_handle)?;
    let (_, parent_weakening) = lifecycle(facts, flow, parent_loan)?;
    let activation_source = FlowInvalidationSource::Statement { statement_index };
    let boundary = loan.last_use_statement_index.checked_add(1)?;
    let weakening_source = FlowInvalidationSource::Statement {
        statement_index: boundary,
    };
    let weakening_reason = if boundary == statement_count {
        FlowBorrowWeakeningReason::StateExit
    } else {
        FlowBorrowWeakeningReason::LastUseExpired
    };
    if boundary > statement_count
        || facts
            .flow
            .borrow_lifetimes
            .activations
            .get(child_activation)
            .source
            != activation_source
        || facts
            .flow
            .borrow_lifetimes
            .weakenings
            .get(child_weakening)
            .source
            != weakening_source
        || facts
            .flow
            .borrow_lifetimes
            .weakenings
            .get(child_weakening)
            .reason
            != weakening_reason
    {
        return None;
    }
    let mut statements = facts
        .flow
        .control
        .statements
        .span_or_empty(flow.statements)
        .iter()
        .filter(|row| row.statement_index == statement_index);
    let statement = statements.next()?;
    if statements.next().is_some() {
        return None;
    }
    let mut entries = facts
        .flow
        .contexts
        .constraint_refs
        .span_or_empty(statement.entry_constraints)
        .iter()
        .enumerate()
        .filter(|(_, row)| row.kind == (FlowConstraintKind::BorrowLoan { loan: parent_loan }));
    let (entry_offset, _) = entries.next()?;
    if entries.next().is_some() {
        return None;
    }
    let parent_entry_constraint = span_handle(statement.entry_constraints, entry_offset)?;
    if child.machine_symbol != flow.machine_symbol
        || child.state_symbol != flow.state_symbol
        || child.owner_symbol != owner
        || !child.owner_path.is_empty()
        || child.captured_place != place
        || &child.access != access
        || child.parent_access != parent.kind
        || child.access_effect != CheckedReborrowAccessEffect::ExclusiveSuspension
        || child.activation_source != activation_source
        || child.weakening_source != weakening_source
        || child.weakening_reason != weakening_reason
        || child.parent_loan != parent_loan
        || child.parent_resource != parent_resource
        || child.parent_suspension
            != (CheckedReborrowParentSuspensionBoundary {
                child_loan: loan_handle,
                parent_loan,
                parent_resource: parent_resource.clone(),
                child_activation,
                parent_entry_constraint,
                source: activation_source,
            })
        || child.parent_end_status
            != (CheckedReborrowParentEndStatus {
                child_loan: loan_handle,
                parent_loan,
                parent_resource: parent_resource.clone(),
                child_weakening,
                parent_weakening,
                status: ParentLexicalStatusAtChildEnd::RetiredBeforeChild,
            })
        || child.restoration
            != (CheckedReborrowRestorationObligation {
                child_loan: loan_handle,
                parent_loan,
                parent_resource: parent_resource.clone(),
                child_weakening_source: weakening_source,
                child_weakening_reason: weakening_reason,
            })
    {
        return None;
    }
    let expected = CheckedReborrowContainmentCertificate {
        machine_symbol: flow.machine_symbol,
        state_symbol: flow.state_symbol,
        child_loan: loan_handle,
        child_resource,
        parent_loan,
        parent_resource,
        parent_access: parent.kind.clone(),
        child_access: access.clone(),
        access_effect: CheckedReborrowAccessEffect::ExclusiveSuspension,
        child_activation,
        parent_entry_constraint,
        formation_source: activation_source,
        child_weakening,
        parent_weakening,
        child_weakening_source: weakening_source,
        child_weakening_reason: weakening_reason,
        parent_place,
        child_place: place,
        projection_remainder: projection.to_vec(),
        containment: CheckedReborrowContainmentKind::ExclusiveSuspension,
    };
    let mut certificates = facts
        .borrow
        .reborrow_containment_certificates
        .iter()
        .filter(|(_, row)| row.child_loan == loan_handle);
    if certificates.next()?.1 != &expected || certificates.next().is_some() {
        return None;
    }
    Some((loan_handle, loan.last_use_statement_index))
}

/// Retired ancestors have no premature disposition. The leaf's one event
/// names every ancestor, in immediate-parent order, through the original root.
pub(super) fn closures(
    facts: &CheckFacts,
    flow: &FlowStateFact,
    loans: &[(Handle<BorrowLoanFact>, usize)],
    parents: &[Option<usize>],
) -> Option<()> {
    for (position, parent) in parents.iter().enumerate() {
        let Some(parent_position) = parent else {
            continue;
        };
        let loan = loans[position].0;
        let mut events = facts
            .borrow
            .reborrow_disposition_events
            .iter()
            .filter(|(_, row)| row.child_loan == loan);
        if parents.contains(&Some(position)) {
            if events.next().is_some() {
                return None;
            }
            continue;
        }
        let event = events.next()?.1;
        if events.next().is_some() {
            return None;
        }
        let CheckedParentBorrowResource::Reborrow {
            resource: child_resource,
        } = resource(facts, loan)?
        else {
            return None;
        };
        let (child_activation, child_weakening) = lifecycle(facts, flow, loan)?;
        let weakening = facts.flow.borrow_lifetimes.weakenings.get(child_weakening);
        let mut retired_parent_path = Vec::new();
        let mut ancestor = *parent_position;
        let root_lifetime = loop {
            let parent_resource = resource(facts, loans[ancestor].0)?;
            let (_, weakening) = lifecycle(facts, flow, loans[ancestor].0)?;
            retired_parent_path.push(CheckedRetiredParentResourceDispositionStep {
                resource: parent_resource.clone(),
                weakening,
            });
            if let Some(parent) = parents[ancestor] {
                if parent >= ancestor {
                    return None;
                }
                ancestor = parent;
            } else {
                let CheckedParentBorrowResource::DirectRoot { resource } = parent_resource else {
                    return None;
                };
                break facts
                    .borrow
                    .direct_loan_resources
                    .get(resource)
                    .parent_lifetime
                    .clone();
            }
        };
        let (boundary_phase, disposition) = match weakening.reason {
            FlowBorrowWeakeningReason::StateExit => (
                CheckedBorrowResourceLifecyclePhase::StateExit,
                CheckedReborrowResourceDisposition::StateExitDirectRootHandoff,
            ),
            _ => return None,
        };
        let expected = CheckedReborrowResourceDispositionEvent {
            machine_symbol: flow.machine_symbol,
            state_symbol: flow.state_symbol,
            child_loan: loan,
            child_resource,
            child_activation,
            child_weakening,
            parent_loan: loans[*parent_position].0,
            parent_resource: resource(facts, loans[*parent_position].0)?,
            boundary_source: weakening.source,
            boundary_phase,
            shared_cohort: Vec::new(),
            retired_parent_path,
            final_target: CheckedBorrowResourceDispositionTarget::DirectRootLifetime(root_lifetime),
            disposition,
        };
        if event != &expected {
            return None;
        }
    }
    Some(())
}
