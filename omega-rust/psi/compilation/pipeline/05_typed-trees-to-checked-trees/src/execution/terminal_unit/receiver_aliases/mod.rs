//! Erase nonescaping reference carriers, not their captured referent or access.
use super::{
    CheckFacts, CheckedStructuralAccess, ExpressionNode, SymbolHandle, TypeReferenceHandle,
};
use crate::execution::terminal_unit::types::{
    lent_storage_identity, state_flow, structural_access_for_type_reference,
};
use crate::flow::{expressions, state_statements};

use crate::execution::terminal_unit::calls;

use checked_trees::BorrowAccessKind;
use typed_trees::expression::ExpressionHandle;

mod projection;

pub(super) struct ReceiverAlias {
    pub(super) owner: SymbolHandle,
    /// Immediate captured storage, before chaining through an erased parent.
    captured_place: validation::CanonicalPlace,
    pub(super) root: SymbolHandle,
    pub(super) segments: Vec<facts::PlaceSegment>,
    /// The erased loan's exact access and terminal statement. A bare-argument
    /// move records only a carrier read; this evidence restores the authority
    /// that argument actually forwards.
    pub(super) kind: BorrowAccessKind,
    pub(super) last_use: usize,
}

/// Consume the checked occurrence of an immutable exclusive carrier. Its
/// source path chooses runtime storage, while its loan supplies the captured
/// access and last use. Direct roots and immediate-parent reborrows have the
/// same erasure operation; checking owns their different resource obligations.
fn formation(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    borrow_state: &checked_trees::StateBorrowFact,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    local: &typed_trees::statement::TableLocalData,
    aliases: &[ReceiverAlias],
) -> Option<ReceiverAlias> {
    if local.is_mutable
        || !local.symbol.is_valid()
        || aliases.iter().any(|alias| alias.owner == local.symbol)
    {
        return None;
    }
    let access = exclusive_access(program, local.type_reference)?;
    let borrow = expressions(program).borrow_expression(local.initial_value)?;
    if !matches!(
        (&access, borrow.access),
        (
            BorrowAccessKind::Mutable,
            language_semantics::ReferenceAccess::Mutable
        ) | (
            BorrowAccessKind::WriteOnly,
            language_semantics::ReferenceAccess::WriteOnly
        )
    ) {
        return None;
    }
    let (place, _) =
        projection::formation_place(program, machine, state, statement_index, borrow.target)?;
    // Erasure is optional: only static coordinates may be substituted later.
    // A runtime selector is frozen at formation, even if its source changes
    // while the child lives. Keep that descriptor on the general capture path
    // rather than re-evaluating its authored selector at each aliased use.
    if place
        .segments
        .iter()
        .any(|segment| matches!(segment, facts::PlaceSegment::Index { .. }))
    {
        return None;
    }
    let source_root = place.root.as_symbol()?;
    let projected_type = calls::projected_borrowed_path(
        program,
        machine.symbol,
        state.symbol,
        statement_index,
        &place,
    )?
    .storage_type;
    if lent_storage_identity(program, projected_type, &[])?
        != lent_storage_identity(program, local.type_reference, &[])?
    {
        return None;
    }
    let mut loans = facts
        .borrow
        .loan_group(borrow_state.loans)?
        .filter(|(_, loan)| {
            loan.owner_symbol == local.symbol
                && loan.statement_index == statement_index
                && facts.borrow.loan_owner_path(loan).is_empty()
        });
    let (loan_handle, loan) = loans.next()?;
    if loans.next().is_some() || loan.kind != access {
        return None;
    }
    let has_resource = facts
        .borrow
        .direct_loan_resources
        .iter()
        .any(|(_, resource)| resource.loan == loan_handle)
        || facts
            .borrow
            .reborrow_loan_resources
            .iter()
            .any(|(_, resource)| resource.loan == loan_handle);
    if !has_resource {
        return None;
    }
    let (root, segments) = match aliases.iter().find(|alias| alias.owner == source_root) {
        Some(parent) => {
            let mut segments = parent.segments.clone();
            segments.extend_from_slice(&place.segments);
            (parent.root, segments)
        }
        None => (source_root, place.segments.clone()),
    };
    Some(ReceiverAlias {
        owner: local.symbol,
        captured_place: place,
        root,
        segments,
        kind: access,
        last_use: loan.last_use_statement_index,
    })
}

/// Collect nonescaping exclusive carriers in declaration order. The operation
/// consumes checked resource identity; it imposes no sibling count, parent
/// retirement shape, or declaration-prefix requirement. Materialized descriptors
/// remain runtime values and stop substitution at their own storage.
pub(super) fn aliases(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> Option<Vec<ReceiverAlias>> {
    state_flow(facts, machine.symbol, state.symbol)?;
    let (_, borrow_state) = facts
        .borrow
        .first_state_borrow(machine.symbol, state.symbol)?;
    if borrow_state.loans.is_empty() {
        return Some(Vec::new());
    }
    let mut aliases = Vec::new();
    for (statement_index, statement) in state_statements(program, state.statement_nodes)
        .iter()
        .enumerate()
    {
        let Some(local) = statement.as_local_data() else {
            continue;
        };
        if let Some(alias) = formation(
            program,
            facts,
            borrow_state,
            machine,
            state,
            statement_index,
            local,
            &aliases,
        ) {
            aliases.push(alias);
        }
    }
    Some(aliases)
}

fn exclusive_access(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<BorrowAccessKind> {
    match structural_access_for_type_reference(program, reference)? {
        CheckedStructuralAccess::MutableBorrow => Some(BorrowAccessKind::Mutable),
        CheckedStructuralAccess::WriteOnlyBorrow => Some(BorrowAccessKind::WriteOnly),
        _ => None,
    }
}

/// Follow erased carriers only as far as the nearest materialized storage.
/// A captured descriptor is the authority for its referent; flattening through
/// that descriptor would instead lend its suspended backing a second time.
pub(super) fn resolve(
    aliases: &[ReceiverAlias],
    place: &validation::CanonicalPlace,
    materialized: impl Fn(facts::PlaceRoot) -> bool,
) -> Option<validation::CanonicalPlace> {
    let mut resolved = place.clone();
    for _ in 0..aliases.len() {
        if materialized(resolved.root) {
            break;
        }
        let Some(owner) = resolved.root.as_symbol() else {
            break;
        };
        let Some(alias) = aliases.iter().find(|alias| alias.owner == owner) else {
            break;
        };
        let mut segments = alias.captured_place.segments.clone();
        segments.extend(resolved.segments);
        resolved = validation::CanonicalPlace {
            root: alias.captured_place.root,
            segments,
        };
    }
    (resolved != *place).then_some(resolved)
}
