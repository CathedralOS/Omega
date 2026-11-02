//! Completed anonymous owners retain their exact source custody through calls,
//! borrowed observations and constructed children. A partial transfer keeps its
//! remaining partition until the actual normal consumer continuation; every
//! remaining linear claim needs its owner-authorized disposition. Borrowing
//! lends access without transferring that owner or shortening its lifetime.
use crate::checks::multiplicity::claim_outcomes::claim_paths_are_case_alternatives;
use crate::checks::multiplicity::linear_claim_frontier;
use crate::checks::multiplicity::linear_validation::{event_statement_index, permission_source};
use arena::HandleSpan;
use checked_trees::CheckFacts;
use checked_trees::FlowPermissionEventFact;
use diagnostics::Diagnostic;
use language_semantics::Multiplicity;
use language_semantics::PermissionAccess;
use language_semantics::PermissionClaimIdentity;
use language_semantics::PermissionEventKind;
use language_semantics::PermissionProvenance;
use symbols::SymbolHandle;

mod borrowed_owners;
mod owned_transfers;
mod owners;
mod projected;

pub(super) use owned_transfers::{append_owned_transfer, append_stored_transfer};
pub(crate) use owners::owned_temporary_type;
pub(super) use owners::{TemporaryOwner, projectable_owner, temporary_owners};
pub(super) use projected::{ProjectedConsumer, retire_owner};

/// One call completion disposes temporary owners in reverse establishment
/// order. Usage selects the exact whole or residual plan, not a separate
/// cleanup order for borrowed versus transferred arguments.
#[allow(clippy::too_many_arguments)]
pub(super) fn append_continuation_cleanup(
    program: &dyn validation::CheckingInput,
    facts: &mut CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    consumer: language_semantics::PermissionEventSource,
    completion: typed_trees::expression::ExpressionHandle,
    evaluation: checked_trees::FlowPermissionEvaluation,
    owners: &[TemporaryOwner],
    places: &mut [super::linear_obligations::LinearPlace],
    remaining: &super::cleanup_dispositions::RemainingCustody,
    permissions: &mut Vec<FlowPermissionEventFact>,
    dispositions: &mut checked_trees::ContextualCleanupFacts,
) -> Result<Vec<facts::PlaceRoot>, Diagnostic> {
    let mut retired = Vec::new();
    for owner in owners.iter().rev().filter(|owner| {
        owner.consumer == consumer
            && owner.completion == completion
            && owner.evaluation == evaluation
    }) {
        let mut diagnostics = Vec::new();
        match owner.consumption {
            owners::TemporaryConsumption::Borrow { .. } => {
                retired.extend(borrowed_owners::append_borrow(
                    program,
                    facts,
                    machine,
                    state,
                    consumer,
                    evaluation,
                    std::slice::from_ref(owner),
                    places,
                    permissions,
                    &mut diagnostics,
                    dispositions,
                ))
            }
            owners::TemporaryConsumption::OwnedProjection { .. } => {
                retired.extend(projected::append_cleanup(
                    program,
                    facts,
                    machine,
                    state,
                    consumer,
                    evaluation,
                    std::slice::from_ref(owner),
                    places,
                    remaining,
                    permissions,
                )?)
            }
            owners::TemporaryConsumption::OwnedTransfer => {}
        }
        if let Some(diagnostic) = diagnostics.into_iter().next() {
            return Err(diagnostic);
        }
    }
    Ok(retired)
}

/// A partial anonymous move needs its exact completing consumer before a
/// remaining claim can be considered for owner-authorized disposal. Eligibility
/// is checked only after the actual transfer, against the resulting custody.
pub(super) fn unselected_claim_diagnostic(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    event: &crate::flow::DiscoveredMoveEvent,
    path: &[facts::PlaceSegment],
    owners: &[TemporaryOwner],
) -> Option<Diagnostic> {
    let expression = event.root.as_expression()?;
    let consumer = permission_source(event.source);
    let mut continuations = owners.iter().filter(|owner| {
        owner.expression == expression
            && owner.consumer == consumer
            && owner.evaluation == event.evaluation
    });
    if matches!(
        consumer,
        language_semantics::PermissionEventSource::Call { .. }
    ) && let Some(owner) = continuations.next()
        && continuations.next().is_none()
        && matches!(
            &owner.consumption,
            owners::TemporaryConsumption::OwnedProjection { path: selected } if selected == path
        )
    {
        return None;
    }
    let root = validation::CanonicalPlace {
        root: event.root,
        segments: Vec::new(),
    };
    let reference = validation::canonical_place_type_reference(
        program,
        state_symbol,
        event_statement_index(event.source).unwrap_or(0),
        &root,
    )?;
    for claim in linear_claim_frontier(program, reference) {
        if claim.multiplicity != Multiplicity::Linear
            || claim.path.starts_with(path)
            || claim_paths_are_case_alternatives(&claim.path, path)
        {
            continue;
        }
        let machine_name = validation::find_state(program, state_symbol)
            .map(|(machine, _)| machine.name.as_str())
            .unwrap_or("<unknown machine>");
        let statement_index = event_statement_index(event.source).unwrap_or(0);
        return Some(Diagnostic::error(format!(
            "cannot partially move a temporary call result while an unselected linear claim remains in {machine_name}::{} at statement {statement_index}; bind the result to a local and transfer or consume every claim",
            program.symbols.name(state_symbol)
        )));
    }
    None
}
