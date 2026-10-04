//! Owned transfers exclude outstanding loans over the storage they detach.
//! Statement extraction and assignment use the same captured-place compatibility
//! judgment; call transfers retain their occurrence-specific call certificates.

use checked_trees::{CheckFacts, FlowStateFact, FlowStatementFact};
use diagnostics::Diagnostic;

use super::aliases::ResolvedAlias;
use super::details::active_loan_detail;
use super::overlap::{StatedOrderingPremise, canonical_place_loan_compatibility};
use crate::flow::{CanonicalPlace, DiscoveredMoveEvent};
use crate::labels::symbol_name;

pub(super) fn check_storage_move<'program>(
    program: &'program typed_trees::TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement: &FlowStatementFact,
    event: &DiscoveredMoveEvent,
    path: &[facts::PlaceSegment],
    active_loans: impl Iterator<Item = arena::Handle<checked_trees::BorrowLoanFact>>,
    premises: &[StatedOrderingPremise],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    diagnostics: &mut Vec<Diagnostic>,
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'program>>,
) {
    if !crate::checks::multiplicity::transfers_owned_storage(
        program,
        state.state_symbol,
        statement.statement_index,
        event,
        path,
    ) {
        return;
    }
    let facts::PlaceRoot::Symbol(root_symbol) = event.root else {
        // A completed rvalue has no preexisting source storage to detach.
        return;
    };
    let resolved = if path.is_empty() {
        // Moving a whole owned carrier retains its own storage identity here.
        // Loans stored inside it do not turn that transfer into a move of
        // their referents. Reference values were excluded by classification.
        Some(ResolvedAlias {
            place: checked_trees::CapturedPlace {
                root_symbol,
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
        super::access_sources::resolve(
            program,
            facts,
            state,
            statement,
            event.expression,
            call_frames,
        )
    };
    let Some(resolved) = resolved else {
        diagnostics.push(Diagnostic::error(
            "owned transfer requires an exact storage source and available loan authority",
        ));
        return;
    };
    let place = CanonicalPlace {
        root: facts::PlaceRoot::Symbol(resolved.place.root_symbol),
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
