//! Keep the caller requirements with the checked source. When Build selects
//! an external entry, check them again: that entry has no ordinary caller whose
//! arguments can prove its outstanding requirements.
use checked_trees::CheckFacts;
use facts::{PlaceRoot, PlaceSegment};
use symbols::SymbolHandle;
use validation::CanonicalPlace;

use super::sources::state_flow;
use super::{IncomingBacking, IncomingBackingCalls, PendingRequirement, Requirement};

/// Build can select different entries while reusing the same checked source.
/// Check those entries' saved requirements for each request. Their statements
/// and other borrow conflicts have already been checked.
pub(crate) fn check_external_roots(
    checked: &checked_trees::CheckedTrees,
    roots: &[SymbolHandle],
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    if roots.is_empty() || checked.facts.borrow.incoming_compatibility.is_empty() {
        return Ok(());
    }
    let program: &dyn validation::CheckingInput = checked;
    let facts = &checked.facts;
    let calls = IncomingBackingCalls::new(program, facts, roots);
    let mut diagnostics = Vec::new();
    for (_, row) in facts.borrow.incoming_compatibility.iter() {
        let result = retained_requirement(facts, row)
            .and_then(|requirement| calls.discharge(program, facts, vec![requirement]));
        if let Err(reason) = result {
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "external root admission cannot establish captured backing compatibility at statement {}: {reason}",
                row.statement_index,
            )));
        }
    }
    crate::checks::finish(diagnostics)
}

pub(in crate::checks::borrows) fn publish(
    facts: &mut checked_trees::BorrowFacts,
    pending: Vec<PendingRequirement>,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    for pending in pending {
        let requirement = pending.requirement;
        let input_error = || {
            vec![diagnostics::Diagnostic::error(
                "incoming backing compatibility publication lost an exact input parameter identity",
            )]
        };
        let backing_input = requirement
            .backing
            .slot
            .root
            .as_symbol()
            .filter(|symbol| symbol.is_valid())
            .ok_or_else(input_error)?;
        let storage_input = requirement
            .storage
            .root
            .as_symbol()
            .filter(|symbol| symbol.is_valid())
            .ok_or_else(input_error)?;
        let backing_path = facts
            .access_segments
            .insert_many(requirement.backing.slot.segments);
        let backing_suffix = facts
            .access_segments
            .insert_many(requirement.backing.suffix);
        let storage_path = facts
            .access_segments
            .insert_many(requirement.storage.segments);
        facts.incoming_compatibility.append(
            checked_trees::BorrowIncomingCompatibilityRequirement {
                machine_symbol: pending.machine,
                state_symbol: requirement.state,
                statement_index: pending.statement,
                loan: pending.loan,
                load: pending.load,
                read: pending.read,
                store_target: pending.store_target,
                // Both roots were established as exact input parameters before
                // the pending row was admitted, rather than guessed at publication.
                backing_input,
                backing_path,
                backing_suffix,
                storage_input,
                storage_path,
            },
        );
    }
    Ok(())
}

fn retained_requirement(
    facts: &CheckFacts,
    row: &checked_trees::BorrowIncomingCompatibilityRequirement,
) -> Result<Requirement, &'static str> {
    let state = state_flow(facts, row.state_symbol)?;
    if state.machine_symbol != row.machine_symbol {
        return Err("the retained compatibility requirement has a different machine/state owner");
    }
    let control = &facts.flow.control;
    if !facts.borrow.loans.is_valid(row.loan)
        || !control.operands.is_valid(row.load)
        || !control.value_observations.is_valid(row.read)
        || !control.operands.is_valid(row.store_target)
    {
        return Err(
            "the retained incoming compatibility requirement lost its occurrence identities",
        );
    }
    let loan = facts.borrow.loans.get(row.loan);
    let load = control.operands.get(row.load);
    let store = control.operands.get(row.store_target);
    let descriptor_read = load.descriptor_permission.loaded_read();
    let descriptor_loan = load.descriptor_permission.loaded_loan();
    if descriptor_read != row.read
        || load.state_symbol != row.state_symbol
        || store.state_symbol != row.state_symbol
        || store.statement_index != row.statement_index
        || !facts.borrow.loans.is_valid(descriptor_loan)
        || facts.borrow.loans.get(descriptor_loan).incoming_reference != loan.incoming_reference
    {
        return Err(
            "the retained incoming compatibility requirement has mismatched load/store owners",
        );
    }
    let segments =
        |span| {
            facts.borrow.access_segments.span(span).map(<[PlaceSegment]>::to_vec)
        .ok_or("the retained incoming compatibility requirement lost its exact structural path")
        };
    Ok(Requirement {
        state: row.state_symbol,
        backing: IncomingBacking {
            slot: CanonicalPlace {
                root: PlaceRoot::Symbol(row.backing_input),
                segments: segments(row.backing_path)?,
            },
            suffix: segments(row.backing_suffix)?,
        },
        storage: CanonicalPlace {
            root: PlaceRoot::Symbol(row.storage_input),
            segments: segments(row.storage_path)?,
        },
    })
}
