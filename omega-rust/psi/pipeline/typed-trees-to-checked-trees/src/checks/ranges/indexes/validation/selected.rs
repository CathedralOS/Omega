//! Bind the bounds judgment to the selected declaration, never a spelling-wide
//! search for any precondition. The structural clause reader accepts only facts
//! discharged by this occurrence's ordinary bounds judgment.

use checked_trees::{CheckedOperatorResolutionStatus, CheckedValueOrigin};
use language_core::operator_spelling::OperatorSpelling;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, TableIndexedExpression};
use typed_trees::machine::Machine;
use typed_trees::operator::{resolve_indexed_spelling_for_operands, resolve_spelling};
use typed_trees::state::State;

use super::{RangeFacts, lower_bounds};

mod clauses;

pub(super) fn obligation(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    facts: &RangeFacts<'_>,
    expression: ExpressionHandle,
    indexed: &TableIndexedExpression,
    spelling: OperatorSpelling,
) -> Result<Option<String>, String> {
    let failure = |reason: &str| format!("indexing spelling `{}` {reason}", spelling.symbol());
    let mut uses = facts.checked_operators.into_iter().flat_map(|operators| {
        operators.uses.iter().filter_map(move |(_, selected)| {
            (selected.expression == expression
                && matches!(selected.origin, CheckedValueOrigin::StateStatement {
                    machine_symbol, state_symbol, statement_index, ..
                } if machine_symbol == machine.symbol && state_symbol == state.symbol
                    && statement_index == facts.statement_index))
            .then_some(selected)
        })
    });
    let Some(selected) = uses.next() else {
        // Pure builtin indexing has no declaration to borrow authority from.
        // Once a declaration can govern the spelling, missing checked custody
        // is not permission to synthesize a selection from it here.
        return if resolve_spelling(program, spelling, None).is_empty() {
            Ok(None)
        } else {
            Err(failure(
                "has no exact checked operator use at this occurrence",
            ))
        };
    };
    let operators = facts
        .checked_operators
        .expect("selected use came from this table");
    if selected.spelling != spelling {
        return Err(failure("does not match its checked operator spelling"));
    }
    let operands = crate::operators::indexed_operand_types(program, indexed, selected.origin);
    let live = resolve_indexed_spelling_for_operands(program, spelling, &operands);
    if matches!(
        selected.status,
        CheckedOperatorResolutionStatus::Missing | CheckedOperatorResolutionStatus::BuiltinFallback
    ) && !selected.selected_operator_symbol.is_valid()
        && selected.candidate_count == 0
        && operators.candidates(selected).is_empty()
        && live.is_empty()
    {
        // An unrelated overload cannot replace this collection's builtin
        // indexing. The exact checked occurrence and freshly matched operands
        // must both have no candidate; a stale or conflicting selection fails.
        if uses.any(|other| other != selected) {
            return Err(failure("has inconsistent checked selection custody"));
        }
        return Ok(None);
    }
    if selected.status != CheckedOperatorResolutionStatus::Resolved {
        return Err(failure("has no uniquely selected checked declaration"));
    }
    let retained = operators
        .selected_candidate(selected)
        .ok_or_else(|| failure("has no candidate matching its selected declaration"))?;
    if selected.candidate_count != operators.candidates(selected).len()
        || uses.any(|other| {
            other.spelling != spelling
                || other.status != selected.status
                || other.selected_operator_symbol != selected.selected_operator_symbol
                || operators.selected_candidate(other) != Some(retained)
        })
    {
        return Err(failure("has inconsistent checked selection custody"));
    }
    let replayed = replay_selection(program, selected, &live)
        .ok_or_else(|| failure("has too many live operator candidates"))?;
    if replayed != (selected.status, selected.selected_operator_symbol) {
        return Err(failure(
            "selected disposition differs from exact binding-site selection",
        ));
    }
    let mut matching = live
        .iter()
        .filter(|candidate| candidate.operator.symbol == selected.selected_operator_symbol);
    let candidate = matching
        .next()
        .ok_or_else(|| failure("selected declaration does not match its actual operands"))?;
    if matching.next().is_some()
        || crate::operators::checked_candidate(program, candidate) != *retained
    {
        return Err(failure(
            "selected candidate no longer matches its exact declaration and contracts",
        ));
    }
    // A nominal user collection may select its own indexing operation without
    // having builtin array/slice geometry. Selection still rejoins exactly, but
    // this ranges checker has no storage-bound judgment to manufacture for it.
    // Preconditions on that custom operation cannot silently disappear here.
    if operands
        .first()
        .copied()
        .flatten()
        .is_some_and(|collection| !has_builtin_collection_geometry(program, collection))
    {
        return if program
            .operator_contracts(candidate.operator)
            .iter()
            .any(|contract| {
                contract.kind == typed_trees::signature::SignatureContractKind::Requires
            }) {
            Err(failure(
                "has unsupported selected `requires` on a non-array, non-slice collection",
            ))
        } else {
            Ok(None)
        };
    }
    let clauses = clauses::validate(program, candidate.operator, spelling).map_err(&failure)?;
    if !lower_bounds::prove(
        program,
        machine,
        state,
        facts,
        indexed,
        spelling,
        clauses.lower_bound_positions,
    ) {
        return Err(failure(
            "selected requires has an unproven non-negative operand",
        ));
    }
    let path = program
        .operator_path_members(candidate.operator.name)
        .iter()
        .map(|name| name.as_str())
        .collect::<Vec<_>>()
        .join("::");
    Ok(Some(format!(
        "cannot prove `{}` — the `requires` of `{path}` (spelled `{}`)",
        clauses.labels.join(" && "),
        spelling.symbol()
    )))
}

fn has_builtin_collection_geometry(
    program: &TypedTrees,
    mut collection: typed_trees::types::TypeReferenceHandle,
) -> bool {
    for _ in 0..128 {
        if !program
            .type_reference_table
            .contains_type_reference(collection)
        {
            return true;
        }
        match program.type_reference_table.type_reference(collection) {
            typed_trees::types::TypeReferenceNode::Reference { referee, .. }
            | typed_trees::types::TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => collection = *referee,
            typed_trees::types::TypeReferenceNode::FixedArray { .. }
            | typed_trees::types::TypeReferenceNode::Slice { .. } => return true,
            _ => return false,
        }
    }
    // An unresolved/deep structural shape never earns the custom-carrier exit.
    true
}

/// Replay the existing static binding-site owner on freshly derived candidates.
/// The retained choice is an expectation, never an input to domain activation.
/// This scratch use is not published and carries no additional proof authority.
fn replay_selection(
    program: &TypedTrees,
    selected: &checked_trees::CheckedOperatorUseFact,
    live: &[typed_trees::operator::SpelledOperator<'_>],
) -> Option<(CheckedOperatorResolutionStatus, symbols::SymbolHandle)> {
    let count = u32::try_from(live.len()).ok()?;
    let mut candidates = arena::Arena::with_capacity(live.len());
    let mut first = arena::Handle::invalid();
    for candidate in live {
        let handle = candidates.append(crate::operators::checked_candidate(program, candidate));
        if !first.is_valid() {
            first = handle;
        }
    }
    let mut use_fact = *selected;
    use_fact.candidates = arena::HandleSpan::from_parts(first, count);
    use_fact.candidate_count = live.len();
    use_fact.selected_operator_symbol = symbols::SymbolHandle::invalid();
    use_fact.status = CheckedOperatorResolutionStatus::DomainPending;
    let mut uses = arena::Arena::with_capacity(1);
    let handle = uses.append(use_fact);
    let mut replay =
        checked_trees::CheckedOperatorFacts::with_roots(uses, arena::Arena::default(), candidates);
    crate::operators::select_pending_domain_operator_meanings(program, &mut replay);
    let selected = replay.uses.get(handle);
    Some((selected.status, selected.selected_operator_symbol))
}
