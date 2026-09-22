//! Erase nonescaping reference carriers, not their captured referent or access.
use super::{
    CheckFacts, CheckedStructuralAccess, ExpressionNode, StatementNode, SymbolHandle,
    TypeReferenceHandle, TypeReferenceNode, TypedTrees,
};
use crate::execution::terminal_unit::base_type_identity;
use crate::execution::terminal_unit::calls;
use crate::execution::terminal_unit::state_flow;
use crate::execution::terminal_unit::structural_access_for_type_reference;
use checked_trees::{
    BorrowAccessKind, BorrowLoanLineage, FlowBorrowWeakeningReason, FlowInvalidationSource,
};
use typed_trees::expression::ExpressionHandle;

mod nested;
mod projection;
#[cfg(test)]
mod tests;

pub(super) struct ReceiverAlias {
    pub(super) owner: SymbolHandle,
    pub(super) root: SymbolHandle,
    pub(super) segments: Vec<facts::PlaceSegment>,
    /// The erased loan's exact access and terminal statement. A bare-argument
    /// move records only a carrier read; this evidence restores the authority
    /// that argument actually forwards.
    pub(super) kind: BorrowAccessKind,
    pub(super) last_use: usize,
}

/// One validated formation step: the local is an immutable exclusive borrow
/// whose checked loan, resource, activation, and weakening evidence exactly
/// reconstruct its captured referent. Nested carriers chain through an
/// already-erased parent alias.
#[allow(clippy::too_many_arguments)]
fn formation(
    program: &TypedTrees,
    facts: &CheckFacts,
    flow: &checked_trees::FlowStateFact,
    borrow_state: &checked_trees::StateBorrowFact,
    parameters: &[typed_trees::signature::StateParameter],
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    local: &typed_trees::statement::TableLocalData,
    statement_count: usize,
    aliases: &[ReceiverAlias],
    loans: &[(arena::Handle<checked_trees::BorrowLoanFact>, usize)],
    parents: &[Option<usize>],
) -> Option<(
    ReceiverAlias,
    (arena::Handle<checked_trees::BorrowLoanFact>, usize),
    Option<usize>,
)> {
    if local.is_mutable
        || !local.symbol.is_valid()
        || aliases
            .iter()
            .any(|alias: &ReceiverAlias| alias.owner == local.symbol)
    {
        return None;
    }
    let access = exclusive_access(program, local.type_reference)?;
    let ExpressionNode::Borrow(borrow) = program.expression_table.expression(local.initial_value)
    else {
        return None;
    };
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
    let (place, source) =
        projection::formation_place(program, machine, state, statement_index, borrow.target)?;
    let facts::PlaceRoot::Symbol(source_root) = place.root else {
        return None;
    };
    if let Some(parent_position) = aliases.iter().position(|alias| alias.owner == source_root) {
        let (projected_type, _) =
            calls::projected_argument_path(program, state.symbol, statement_index, &place)?;
        if parents.contains(&Some(parent_position))
            || source.root_symbol != source_root
            || source.segments != place.segments
            || base_type_identity(program, projected_type, &[])?
                != base_type_identity(program, local.type_reference, &[])?
        {
            return None;
        }
        let parent: &(arena::Handle<checked_trees::BorrowLoanFact>, usize) =
            loans.get(parent_position)?;
        let loan = nested::formation(
            facts,
            flow,
            borrow_state,
            statement_index,
            local.symbol,
            source_root,
            &source.segments,
            parent.0,
            &access,
            statement_count,
        )?;
        let mut segments = aliases[parent_position].segments.clone();
        segments.extend_from_slice(&place.segments);
        return Some((
            ReceiverAlias {
                owner: local.symbol,
                root: aliases[parent_position].root,
                segments,
                kind: access,
                last_use: loan.1,
            },
            loan,
            Some(parent_position),
        ));
    }
    let mut roots = parameters.iter().filter(|parameter| {
        parameter.symbol == source_root || (parameter.is_self && source_root == machine.symbol)
    });
    let root = roots.next()?;
    if roots.next().is_some()
        || root.is_const
        || exclusive_access(program, root.type_reference)?.direct_reborrow_effect(&access)
            != Some(checked_trees::CheckedReborrowAccessEffect::ExclusiveSuspension)
    {
        return None;
    }
    // Runtime self's reference names the machine/Self namespace. Its
    // referent is the exact attachment, as in the ordinary Unit signature
    // collector; no other parameter may borrow that substitution.
    let root_type = if root.is_self {
        let reference = validation::unwrapped_type_reference(program, root.type_reference)?;
        let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(reference)
        else {
            return None;
        };
        if *symbol != machine.symbol && *symbol != machine.attached_data_symbol {
            return None;
        }
        if !machine.attached_data_symbol.is_valid()
            || !program
                .data_definitions()
                .iter()
                .any(|data| data.symbol == machine.attached_data_symbol)
        {
            return None;
        }
        if place.segments.is_empty() {
            program
                .type_reference_table
                .find_named_type_reference(machine.attached_data_symbol)?
        } else {
            // Projected self reaches its type through the exact attachment
            // field below; no separately authored root type is required.
            root.type_reference
        }
    } else {
        root.type_reference
    };
    let projected_type = if place.segments.is_empty() {
        root_type
    } else {
        calls::projected_argument_path(program, state.symbol, statement_index, &place)?.0
    };
    if base_type_identity(program, projected_type, &[])?
        != base_type_identity(program, local.type_reference, &[])?
    {
        return None;
    }
    // Reconstruct the loan's capture with its original owner. Flow's
    // contextual place spelling normalizes runtime self to its formal,
    // while the borrowing owner retains the exact machine/Self root.
    let captured_root = source.root_symbol;
    let mut candidates = facts.borrow.loans.iter().filter(|(handle, loan)| {
        facts.borrow.state_owns_loan(borrow_state, *handle) && loan.owner_symbol == local.symbol
    });
    let (loan_handle, loan) = candidates.next()?;
    if candidates.next().is_some()
        || loan.statement_index != statement_index
        || loan.lineage != BorrowLoanLineage::DirectRoot
        || loan.source_owner_symbol.is_valid()
        || loan.kind != access
        || loan.root_symbol != captured_root
        || facts.borrow.loan_segments(loan) != source.segments
        || !facts.borrow.loan_owner_path(loan).is_empty()
    {
        return None;
    }
    let mut resources = facts
        .borrow
        .direct_loan_resources
        .iter()
        .filter(|(_, resource)| resource.loan == loan_handle);
    let (_, resource) = resources.next()?;
    let activation = FlowInvalidationSource::Statement { statement_index };
    if resources.next().is_some()
        || resource.machine_symbol != machine.symbol
        || resource.state_symbol != state.symbol
        || resource.owner_symbol != local.symbol
        || !resource.owner_path.is_empty()
        || resource.captured_place.root_symbol != captured_root
        || resource.captured_place != source
        || resource.access != access
        || resource.activation_source != activation
        || resource.parent_lifetime.machine_symbol != machine.symbol
        || resource.parent_lifetime.state_symbol != state.symbol
        || resource.parent_lifetime.root_symbol != captured_root
        || resource.restoration.parent != resource.parent_lifetime
        || resource.restoration.weakening_source != resource.weakening_source
        || resource.restoration.weakening_reason != resource.weakening_reason
    {
        return None;
    }
    let activations = facts
        .flow
        .borrow_lifetimes
        .activations
        .span_or_empty(flow.borrow_activations)
        .iter()
        .filter(|row| row.loan == loan_handle)
        .collect::<Vec<_>>();
    if !matches!(activations.as_slice(), [row] if row.source == activation) {
        return None;
    }
    let weakenings = facts
        .flow
        .borrow_lifetimes
        .weakenings
        .span_or_empty(flow.borrow_weakenings)
        .iter()
        .filter(|row| row.loan == loan_handle)
        .collect::<Vec<_>>();
    let [weakening] = weakenings.as_slice() else {
        return None;
    };
    let boundary = loan.last_use_statement_index.checked_add(1)?;
    let expiry_reason = if boundary == statement_count {
        FlowBorrowWeakeningReason::StateExit
    } else {
        FlowBorrowWeakeningReason::LastUseExpired
    };
    // Every non-store use retires the carrier loan one statement after its
    // last occurrence. A store through the alias is itself the terminal use:
    // the assignment overwrites the carrier's whole place, which records the
    // loan's LocalReassigned weakening at that statement instead.
    let expiry = boundary <= statement_count
        && weakening.source
            == (FlowInvalidationSource::Statement {
                statement_index: boundary,
            })
        && weakening.reason == expiry_reason;
    let reassigned = weakening.source
        == (FlowInvalidationSource::Statement {
            statement_index: loan.last_use_statement_index,
        })
        && weakening.reason == FlowBorrowWeakeningReason::LocalReassigned;
    if (!expiry && !reassigned)
        || resource.weakening_source != weakening.source
        || resource.weakening_reason != weakening.reason
    {
        return None;
    }
    Some((
        ReceiverAlias {
            owner: local.symbol,
            root: if root.is_self {
                machine.symbol
            } else {
                root.symbol
            },
            segments: place.segments,
            kind: access,
            last_use: loan.last_use_statement_index,
        },
        (loan_handle, loan.last_use_statement_index),
        None,
    ))
}

/// Every exactly captured exclusive borrow carrier in the state, in
/// declaration order. `prefix` is the stricter head-of-body receiver-only
/// layout; this roster carries the same checked formation evidence so call,
/// store, and receiver planners can each substitute the captured referent
/// at their own use sites. A carrier whose evidence does not reconstruct is
/// simply absent from the roster and its uses stay unsupported.
pub(super) fn aliases(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> Option<Vec<ReceiverAlias>> {
    let statements = program.statement_table.statements(state.statement_nodes);
    let parameters = program.state_parameters(state);
    let flow = state_flow(facts, machine.symbol, state.symbol)?;
    let mut borrow_states = facts.borrow.states.iter().filter(|(_, candidate)| {
        candidate.machine_symbol == machine.symbol && candidate.state_symbol == state.symbol
    });
    let (_, borrow_state) = borrow_states.next()?;
    if borrow_states.next().is_some() {
        return None;
    }
    let mut aliases = Vec::new();
    let mut loans = Vec::new();
    let mut parents = Vec::new();
    for (statement_index, statement) in statements.iter().enumerate() {
        let StatementNode::LocalData(local) = statement else {
            continue;
        };
        if let Some((alias, loan, parent)) = formation(
            program,
            facts,
            flow,
            borrow_state,
            parameters,
            machine,
            state,
            statement_index,
            local,
            statements.len(),
            &aliases,
            &loans,
            &parents,
        ) {
            aliases.push(alias);
            loans.push(loan);
            parents.push(parent);
        }
    }
    nested::closures(facts, flow, &loans, &parents)?;
    Some(aliases)
}

fn exclusive_access(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<BorrowAccessKind> {
    match structural_access_for_type_reference(program, reference)? {
        CheckedStructuralAccess::MutableBorrow => Some(BorrowAccessKind::Mutable),
        CheckedStructuralAccess::WriteOnlyBorrow => Some(BorrowAccessKind::WriteOnly),
        _ => None,
    }
}

/// Substitute an erased carrier's authored root for its captured referent:
/// the referent's root plus the captured projection ahead of the authored
/// suffix. Roots with no validated formation stay as they were spelled.
pub(super) fn resolve(
    aliases: &[ReceiverAlias],
    place: &crate::flow::CanonicalPlace,
) -> Option<crate::flow::CanonicalPlace> {
    let facts::PlaceRoot::Symbol(owner) = place.root else {
        return None;
    };
    let alias = aliases.iter().find(|alias| alias.owner == owner)?;
    let mut segments = alias.segments.clone();
    segments.extend_from_slice(&place.segments);
    Some(crate::flow::CanonicalPlace {
        root: facts::PlaceRoot::Symbol(alias.root),
        segments,
    })
}

/// The erased parent alias and its one-, two-, or three-child roster named by
/// the checked post-reactivation certificate. These source bindings carry
/// borrow lifetimes, not independent Terminal runtime places, so the statement
/// sequence advances past their declarations; every other reference local
/// continues to reject through the ordinary local-data path.
pub(in crate::execution::terminal_unit) fn restored_call_alias_locals(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> Option<Vec<SymbolHandle>> {
    let statements = program.statement_table.statements(state.statement_nodes);
    let StatementNode::LocalData(parent_local) = statements.first()? else {
        return None;
    };
    let child_count = facts
        .borrow
        .reborrow_restored_call_use_certificates
        .iter()
        .filter_map(|(_, certificate)| {
            (certificate.machine_symbol == machine.symbol
                && certificate.state_symbol == state.symbol)
                .then_some(())?;
            facts
                .borrow
                .reborrow_disposition_events
                .is_valid(certificate.disposition)
                .then(|| {
                    facts
                        .borrow
                        .reborrow_disposition_events
                        .get(certificate.disposition)
                        .shared_cohort
                        .len()
                        .max(1)
                })
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
    if parent_local.is_mutable {
        return None;
    }
    let ExpressionNode::Borrow(parent_borrow) = program
        .expression_table
        .expression(parent_local.initial_value)
    else {
        return None;
    };
    let child_borrows = child_locals
        .iter()
        .map(
            |local| match program.expression_table.expression(local.initial_value) {
                ExpressionNode::Borrow(borrow) => Some(borrow),
                _ => None,
            },
        )
        .collect::<Option<Vec<_>>>()?;
    if parent_borrow.access != language_semantics::ReferenceAccess::Mutable {
        return None;
    }

    let parent_source = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        0,
        parent_borrow.target,
    )?;
    let child_sources = child_borrows
        .iter()
        .enumerate()
        .map(|(offset, borrow)| {
            crate::flow::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                offset + 1,
                borrow.target,
            )
        })
        .collect::<Option<Vec<_>>>()?;
    let candidates = facts
        .borrow
        .reborrow_restored_call_use_certificates
        .iter()
        .filter(|(_, certificate)| {
            if certificate.machine_symbol != machine.symbol
                || certificate.state_symbol != state.symbol
                || certificate.carrier_place.root_symbol != parent_local.symbol
                || !certificate.carrier_place.segments.is_empty()
                || parent_source.root
                    != facts::PlaceRoot::Symbol(certificate.restored_place.root_symbol)
                || parent_source.segments != certificate.restored_place.segments
                || child_sources.iter().any(|source| {
                    source.root != facts::PlaceRoot::Symbol(parent_local.symbol)
                        || !source.segments.is_empty()
                })
                || !facts
                    .borrow
                    .reborrow_loan_resources
                    .is_valid(certificate.child_resource)
                || !facts.flow.control.calls.is_valid(certificate.call)
            {
                return false;
            }
            let child = facts
                .borrow
                .reborrow_loan_resources
                .get(certificate.child_resource);
            let call = facts.flow.control.calls.get(certificate.call);
            let disposition = facts
                .borrow
                .reborrow_disposition_events
                .get(certificate.disposition);
            let roster = if disposition.shared_cohort.is_empty() {
                vec![certificate.child_resource]
            } else {
                disposition.shared_cohort.clone()
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
                && roster.contains(&certificate.child_resource)
                && child_locals
                    .iter()
                    .any(|local| local.symbol == child.owner_symbol)
                && call.statement_index == child_count + usize::from(child_count > 1) + 1
                && call.call_ordinal == 0
                && call.target_symbol == certificate.target_symbol
        })
        .count();
    (candidates == 1).then(|| {
        std::iter::once(parent_local.symbol)
            .chain(child_locals.iter().map(|local| local.symbol))
            .collect()
    })
}
