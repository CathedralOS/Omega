//! Erase nonescaping reference carriers, not their captured referent or access.
use super::{
    CheckFacts, CheckedStructuralAccess, ExpressionNode, StatementNode, SymbolHandle,
    TypeReferenceHandle,
};
use crate::execution::terminal_unit::types::{
    lent_storage_identity, state_flow, structural_access_for_type_reference,
};

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
    let borrow = program
        .expression_table
        .borrow_expression(local.initial_value)?;
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
    .0;
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
    for (statement_index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
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

/// The erased parent alias and its one-, two-, or three-child roster named by
/// the checked post-restoration call join on the disposition event. These source bindings carry
/// borrow lifetimes, not independent Terminal runtime places, so the statement
/// sequence advances past their declarations; every other reference local
/// continues to reject through the ordinary local-data path.
pub(in crate::execution::terminal_unit) fn restored_call_alias_locals(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> Option<Vec<SymbolHandle>> {
    let statements = program.statement_table.statements(state.statement_nodes);
    let parent_local = statements.first()?.as_local_data()?;
    if parent_local.is_mutable || facts.borrow.reborrow_disposition_events.is_empty() {
        return None;
    }
    let ExpressionNode::Borrow(parent_borrow) = program
        .expression_table
        .expression(parent_local.initial_value)
    else {
        return None;
    };
    if parent_borrow.access != language_semantics::ReferenceAccess::Mutable {
        return None;
    }
    let child_count = facts
        .borrow
        .reborrow_disposition_events
        .iter()
        .filter_map(|(_, event)| {
            (event.machine_symbol == machine.symbol
                && event.state_symbol == state.symbol
                && event.restored_call.is_valid())
            .then(|| event.shared_cohort.len().max(1))
        })
        .find(|count| matches!(count, 1..=3))?;
    let child_locals = statements
        .get(1..=child_count)?
        .iter()
        .map(|statement| match statement {
            StatementNode::LocalData(local) if !local.is_mutable => Some(local),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    let child_borrows = child_locals
        .iter()
        .map(
            |local| match program.expression_table.expression(local.initial_value) {
                ExpressionNode::Borrow(borrow) => Some(borrow),
                _ => None,
            },
        )
        .collect::<Option<Vec<_>>>()?;

    let parent_source = validation::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        0,
        parent_borrow.target,
    )?;
    let child_sources = child_borrows
        .iter()
        .enumerate()
        .map(|(offset, borrow)| {
            validation::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                offset + 1,
                borrow.target,
            )
        })
        .collect::<Option<Vec<_>>>()?;
    let candidates = facts
        .borrow
        .reborrow_disposition_events
        .iter()
        .filter(|(_, event)| {
            if event.machine_symbol != machine.symbol
                || event.state_symbol != state.symbol
                || !event.restored_call.is_valid()
                || !facts
                    .borrow
                    .reborrow_loan_resources
                    .is_valid(event.child_resource)
                || !facts.flow.control.calls.is_valid(event.restored_call)
                || child_sources.iter().any(|source| {
                    source.root != facts::PlaceRoot::Symbol(parent_local.symbol)
                        || !source.segments.is_empty()
                })
            {
                return false;
            }
            let checked_trees::CheckedParentBorrowResource::DirectRoot {
                resource: parent_resource,
            } = event.parent_resource
            else {
                return false;
            };
            if !facts.borrow.direct_loan_resources.is_valid(parent_resource) {
                return false;
            }
            let parent = facts.borrow.direct_loan_resources.get(parent_resource);
            if parent.owner_symbol != parent_local.symbol
                || parent_source.root != parent.captured_place.root
                || parent_source.segments != parent.captured_place.segments
            {
                return false;
            }
            let child = facts
                .borrow
                .reborrow_loan_resources
                .get(event.child_resource);
            let call = facts.flow.control.calls.get(event.restored_call);
            let roster = if event.shared_cohort.is_empty() {
                vec![event.child_resource]
            } else {
                event.shared_cohort.clone()
            };
            roster.len() == child_count
                && child_locals
                    .iter()
                    .zip(&child_borrows)
                    .all(|(local, borrow)| {
                        roster.iter().any(|resource| {
                            if !facts.borrow.reborrow_loan_resources.is_valid(*resource) {
                                return false;
                            }
                            let member = facts.borrow.reborrow_loan_resources.get(*resource);
                            member.owner_symbol == local.symbol
                                && borrow.access
                                    == match member.access {
                                        checked_trees::BorrowAccessKind::Mutable => {
                                            language_semantics::ReferenceAccess::Mutable
                                        }
                                        checked_trees::BorrowAccessKind::WriteOnly => {
                                            language_semantics::ReferenceAccess::WriteOnly
                                        }
                                        checked_trees::BorrowAccessKind::Read => {
                                            language_semantics::ReferenceAccess::Shared
                                        }
                                    }
                        })
                    })
                && roster.contains(&event.child_resource)
                && child_locals
                    .iter()
                    .any(|local| local.symbol == child.owner_symbol)
                && call.statement_index == child_count + usize::from(child_count > 1) + 1
                && call.call_ordinal == 0
        })
        .count();
    (candidates == 1).then(|| {
        std::iter::once(parent_local.symbol)
            .chain(child_locals.iter().map(|local| local.symbol))
            .collect()
    })
}
