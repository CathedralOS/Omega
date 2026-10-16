//! Owned transfers exclude outstanding loans over the storage they detach.
//! Statement extraction and assignment use the same captured-place compatibility
//! judgment; call transfers retain their occurrence-specific call certificates.

use validation::CanonicalPlace;

use checked_trees::{CheckFacts, FlowStateFact, FlowStatementFact};
use diagnostics::Diagnostic;

use super::aliases::ResolvedAlias;
use super::details::active_loan_detail;
use super::overlap::{StatedOrderingPremise, canonical_place_loan_compatibility};
use crate::flow::DiscoveredMoveEvent;
use crate::labels::symbol_name;

pub(super) fn check_storage_move<'program>(
    program: &'program dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement: &FlowStatementFact,
    event: &DiscoveredMoveEvent,
    path: &[facts::PlaceSegment],
    active_loans: impl Iterator<Item = arena::Handle<checked_trees::BorrowLoanFact>>,
    premises: &[StatedOrderingPremise],
    diagnostics: &mut Vec<Diagnostic>,
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'program>>,
) {
    let resolved = match owned_storage_source(program, facts, state, statement, event, path) {
        Ok(Some(resolved)) => resolved,
        Ok(None) => return,
        Err(diagnostic) => {
            diagnostics.push(diagnostic);
            return;
        }
    };
    let place = CanonicalPlace {
        root: resolved.place.root,
        segments: resolved.place.segments,
    };
    for loan_handle in active_loans {
        let loan = facts.borrow.loans.get(loan_handle);
        // Only the retained exclusive access chain supplies authority for
        // moving through a borrow. Its children and unrelated overlapping
        // loans still exclude the move, including aggregate-carried loans.
        if loan.kind.is_exclusive() && resolved.lineage.contains(&loan_handle) {
            continue;
        }
        if canonical_place_loan_compatibility(
            program,
            &place,
            loan,
            &facts.borrow,
            premises,
            bound_lookup,
        )
        .non_interfering
        {
            continue;
        }
        diagnostics.push(Diagnostic::error(format!(
            "statement {} moves `{}` while local borrow `{}` is still active ({})",
            statement.statement_index,
            place.label(program),
            symbol_name(program, loan.owner_symbol),
            active_loan_detail(state, facts, loan_handle, statement.statement_index)
                .unwrap_or_else(|| format!("borrowed at statement {}", loan.statement_index)),
        )));
    }
}

/// Resolve the storage being detached without following loans merely stored
/// inside a whole owned carrier. Both live-local and staged-call loan checks
/// consume this source; reference forwarding never becomes a referent move.
pub(super) fn owned_storage_source(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement: &FlowStatementFact,
    event: &DiscoveredMoveEvent,
    path: &[facts::PlaceSegment],
) -> Result<Option<ResolvedAlias>, Diagnostic> {
    if !crate::checks::multiplicity::transfers_owned_storage(
        program,
        state.state_symbol,
        statement.statement_index,
        event,
        path,
    ) {
        return Ok(None);
    }
    let Some(root) = event.root.as_symbol() else {
        // A completed rvalue has no preexisting source storage to detach.
        return Ok(None);
    };
    let endpoint_is_reference = validation::canonical_place_type_reference(
        program,
        state.state_symbol,
        statement.statement_index,
        &CanonicalPlace {
            root: event.root,
            segments: path.to_vec(),
        },
    )
    .is_some_and(|reference| crate::checks::type_reference_is_reference(program, reference));
    let resolved = if path.is_empty() && !endpoint_is_reference {
        // Moving a whole owned carrier retains its own storage identity here.
        // Loans stored inside it do not turn that transfer into a move of
        // their referents. Consuming a reference's whole referent instead
        // follows the same captured route as a projected extraction.
        Some(ResolvedAlias {
            place: checked_trees::CapturedPlace {
                incoming_reference: Default::default(),
                root: facts::PlaceRoot::Symbol(root),
                segments: Vec::new(),
            },
            lineage: Vec::new(),
        })
    } else {
        // This resolver rejoins reference-typed local roots and retained
        // call-returned root aliases. A stored-reference crossing inside an
        // owned aggregate path is not yet rebased to its backing here; this
        // exclusion does not certify that extraction's source authority.
        // Do not infer such authority from matching storage candidates or a
        // fallback authored path. Independent Terminal custody still checks it.
        super::access_sources::resolve(program, facts, state, statement, event.expression)
    };
    resolved.map(Some).ok_or_else(|| {
        Diagnostic::error(
            "owned transfer requires an exact storage source and available loan authority",
        )
    })
}
