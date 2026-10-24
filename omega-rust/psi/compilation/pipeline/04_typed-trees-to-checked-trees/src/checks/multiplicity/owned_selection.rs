//! Selected owned values retain exact source and destination paths per arm.
//! Whole moves settle their original storage on every arm; partial moves keep
//! the typed residual until its checked death. Fresh literal producers retain
//! separate establishment evidence rather than masquerading as transfers.
//! Result names bind the obligations that actually arrive. Different arm
//! identities can join without gaining common lineage or new resource authority.
//! A shared-reference member owns descriptor storage, not its referent. Its
//! carrier therefore needs the same selected-source receipt as other owned
//! storage; loan checking independently retains its complete backing frontier.
//! The occurrence frontier ends only at an authenticated receiving write, not
//! at inherited provenance or a sibling edge's establishment.
use checked_trees::CheckFacts;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::statement::StatementNode;

pub(super) use availability::record_statement;
pub(in crate::checks) use operands::reachable_arms;
use operands::*;
mod availability;
mod leaves;
mod occurrences;
mod operands;
mod results;
pub(super) use occurrences::SelectionOccurrences;
pub(in crate::checks::multiplicity) use results::{
    selection_claim_origin, terminal_arm_route, terminal_arms,
};

/// Finite stored-value transport is independent of access, availability and
/// retirement. Loan checking retains every descriptor's exact backing frontier.
fn stored_contents(
    program: &dyn ::validation::CheckingInput,
    reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    validation::has_contents(
        program,
        reference,
        &[],
        validation::ContentsRequirement::StoredValue,
    )
}

fn source_multiplicity(
    program: &dyn ::validation::CheckingInput,
    reference: typed_trees::types::TypeReferenceHandle,
) -> language_semantics::Multiplicity {
    if validation::reference_shell(program, reference).is_some() {
        validation::reference_result_custody::binding_multiplicity(program, reference)
    } else {
        program.type_multiplicity(reference)
    }
}

fn owned_source(
    program: &dyn ::validation::CheckingInput,
    places: &[super::linear_obligations::LinearPlace],
    expression: typed_trees::expression::ExpressionHandle,
    reference: typed_trees::types::TypeReferenceHandle,
) -> Option<SymbolHandle> {
    validation::whole_owned_value_source(program, expression, reference).filter(|symbol| {
        source_multiplicity(program, reference)
            != language_semantics::Multiplicity::Unrestricted
            && stored_contents(program, reference)
            // A borrowed ingress denotes the referent. Only the permission
            // inventory's installed descriptor occurrence is a stored source;
            // otherwise ordinary reference capture forms a fresh arm carrier.
            && (validation::reference_shell(program, reference).is_none()
                || places.iter().any(|place| {
                    place.root == facts::PlaceRoot::Symbol(*symbol)
                        && place.path.is_empty()
                        && place.ever_established
                }))
    })
}

/// Two canonical paths overlap when one reaches inside the other or both
/// name the same place; a dynamic index cannot be statically excluded from a
/// fixed element claim, so it conflicts on the element axis.
pub(super) fn place_paths_overlap(
    used: &[facts::PlaceSegment],
    consumed: &[facts::PlaceSegment],
) -> bool {
    used.iter().zip(consumed.iter()).all(|(used, consumed)| {
        used == consumed
            || (matches!(used, facts::PlaceSegment::Index { .. })
                && matches!(
                    consumed,
                    facts::PlaceSegment::FixedIndex { .. }
                        | facts::PlaceSegment::Index { .. }
                        | facts::PlaceSegment::FixedRange { .. }
                ))
    })
}

/// Live loans exclude a move. Earlier borrow syntax alone does not: a
/// completed temporary invocation can retain no dependency on its argument.
/// Before trusting expiry, require every earlier receiving generation to
/// have complete attribution to its exact loan occurrence. Alias forwarding
/// can retain a source without spelling another borrow of its name.
fn source_needs_loan_closure(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    prior_statements: &[StatementNode],
    symbol: SymbolHandle,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let Some(state_flow) = facts.flow.control.states.iter().find_map(|(_, flow)| {
        (flow.machine_symbol == machine.symbol && flow.state_symbol == state.symbol).then_some(flow)
    }) else {
        return true;
    };
    for (index, statement) in prior_statements.iter().enumerate() {
        if !crate::checks::borrows::retained_outputs_are_attributed(
            program,
            facts,
            machine,
            state_flow,
            index,
            statement,
            symbol,
            call_frames,
        ) {
            return true;
        }
    }
    let Some(statement) = facts.flow.state_statement(state_flow, statement_index) else {
        return true;
    };
    facts
        .flow
        .borrow_loan_constraints(statement.entry_constraints)
        .any(|loan| facts.borrow.loans.get(loan).root == facts::PlaceRoot::Symbol(symbol))
}

fn unsupported() -> Diagnostic {
    Diagnostic::error(
        "owned match requires one existing whole plain-affine or linear-owned local per path and an immutable local or return destination; projections, calls and borrowed custody require additional ownership evidence",
    )
}
