//! Local reference operands rejoin retained storage before overlap comparison.
//! A captured place does not grant ancestry: only receivers with independently
//! retained lineage may exempt their own parent loans from interference.
use arena::{Handle, HandleSpan};
use checked_trees::{BorrowFacts, BorrowLoanFact, BorrowLoanLineage, StateBorrowFact};
use checked_trees::{CapturedPlace, CheckFacts, FlowStateFact};
use typed_trees::statement::StatementNode;
use typed_trees::types::TypeReferenceNode;

pub(in crate::checks::borrows) struct ResolvedAlias {
    pub(in crate::checks::borrows) place: CapturedPlace,
    pub(in crate::checks::borrows) lineage: Vec<Handle<BorrowLoanFact>>,
}

impl ResolvedAlias {
    /// Lineage names descriptor occurrences, each of which may retain several
    /// alternative origins. Comparing one origin with another origin of that
    /// same descriptor must not treat them as simultaneous exclusive loans.
    /// The producer groups a completed descriptor by storage, statement and
    /// leaf path; match arms may have distinct formation expressions. A later
    /// replacement, another leaf or a child descriptor is a different occurrence.
    pub(in crate::checks::borrows) fn retains_loan(
        &self,
        borrow: &BorrowFacts,
        handle: Handle<BorrowLoanFact>,
    ) -> bool {
        self.lineage.iter().any(|ancestor| {
            same_descriptor(borrow, *ancestor, handle)
                || retains_lifetime_dependency(borrow, *ancestor, handle)
        })
    }
}

/// Alternatives belong to an exact descriptor occurrence, never merely to
/// equal backing geometry. Wildcard paths may describe coexisting slots.
pub(in crate::checks::borrows) fn same_descriptor(
    borrow: &BorrowFacts,
    left: Handle<BorrowLoanFact>,
    right: Handle<BorrowLoanFact>,
) -> bool {
    if left == right {
        return left.is_valid();
    }
    if !borrow.loans.is_valid(left) || !borrow.loans.is_valid(right) {
        return false;
    }
    let left = borrow.loans.get(left);
    let right = borrow.loans.get(right);
    // Lifetime debts coexist with the descriptor and with one another; they
    // are not alternative spatial origins of that descriptor occurrence.
    if left.lineage == BorrowLoanLineage::LifetimeDependency
        || right.lineage == BorrowLoanLineage::LifetimeDependency
    {
        return false;
    }
    same_carrier_occurrence(borrow, left, right)
}

/// One spatial descriptor retains its own lifetime cohort independently of
/// where it points. `lineage` contains only suppliers common to every route;
/// requiring this exact historical occurrence cannot revive a replaced slot or
/// turn an unselected input's address into a descriptor alternative.
fn retains_lifetime_dependency(
    borrow: &BorrowFacts,
    descriptor: Handle<BorrowLoanFact>,
    debt: Handle<BorrowLoanFact>,
) -> bool {
    if !borrow.loans.is_valid(descriptor) || !borrow.loans.is_valid(debt) {
        return false;
    }
    if borrow.loans.get(descriptor).lineage == BorrowLoanLineage::SuppliedOutput
        && borrow.supplied_output_call(descriptor).is_none()
    {
        return false;
    }
    let descriptor = borrow.loans.get(descriptor);
    let debt = borrow.loans.get(debt);
    descriptor.lineage != BorrowLoanLineage::LifetimeDependency
        && descriptor.root.is_valid()
        && (!matches!(descriptor.root, facts::PlaceRoot::InvocationOutput(_))
            || descriptor.incoming_reference.is_valid())
        && debt.lineage == BorrowLoanLineage::LifetimeDependency
        && same_carrier_occurrence(borrow, descriptor, debt)
}

fn same_carrier_occurrence(
    borrow: &BorrowFacts,
    left: &BorrowLoanFact,
    right: &BorrowLoanFact,
) -> bool {
    let path = borrow.loan_owner_path(right);
    right.owner_symbol.is_valid()
        && right.storage_owner.is_valid()
        && left.owner_symbol == right.owner_symbol
        && left.storage_owner == right.storage_owner
        && left.statement_index == right.statement_index
        && left.established_by_call == right.established_by_call
        && borrow.loan_owner_path(left) == path
        && !path.contains(&checked_trees::BorrowLoanOwnerSegment::DynamicIndex)
}

pub(in crate::checks::borrows) struct ResolvedPlace {
    pub(in crate::checks::borrows) place: CapturedPlace,
    pub(in crate::checks::borrows) local_loan: Handle<BorrowLoanFact>,
}

pub(in crate::checks::borrows) fn resolve_place(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    entry_constraints: HandleSpan<checked_trees::FlowConstraintRef>,
    place: CapturedPlace,
) -> Option<ResolvedPlace> {
    let mut places = resolve_places(program, facts, state_flow, entry_constraints, place)?;
    (places.len() == 1).then(|| places.remove(0))
}

pub(in crate::checks::borrows) fn resolve_places(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    entry_constraints: HandleSpan<checked_trees::FlowConstraintRef>,
    place: CapturedPlace,
) -> Option<Vec<ResolvedPlace>> {
    resolve_places_with_loans(
        program,
        facts,
        state_flow,
        facts.flow.borrow_loan_constraints(entry_constraints),
        place,
    )
}

/// The caller supplies its exact current occurrence roster, either a retained
/// flow cut or the shared statement replay cursor. No geometry selects a loan.
pub(in crate::checks::borrows) fn resolve_places_with_loans(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    loans: impl Iterator<Item = Handle<BorrowLoanFact>>,
    place: CapturedPlace,
) -> Option<Vec<ResolvedPlace>> {
    let state = program.state_by_symbol(state_flow.state_symbol)?;
    let reference_local = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .any(|statement| {
            matches!(statement, StatementNode::LocalData(local)
                if facts::PlaceRoot::Symbol(local.symbol) == place.root
                    && matches!(program.type_reference_table.type_reference(local.type_reference),
                        TypeReferenceNode::Reference { .. }))
        });
    if !reference_local {
        return Some(vec![ResolvedPlace {
            place,
            local_loan: Handle::invalid(),
        }]);
    }
    resolve_descriptor_places_with_loans(
        program,
        facts,
        state_flow,
        loans,
        std::iter::empty(),
        place,
    )
}

/// Resolve the completed value of a reference descriptor, as distinct from
/// access to the owned storage holding it. Projected stored references use the
/// same captured loan occurrence and lineage as a whole reference binding.
pub(in crate::checks::borrows) fn resolve_descriptor_places(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    entry_constraints: HandleSpan<checked_trees::FlowConstraintRef>,
    place: CapturedPlace,
) -> Option<Vec<ResolvedPlace>> {
    resolve_descriptor_places_with_loans(
        program,
        facts,
        state_flow,
        facts.flow.borrow_loan_constraints(entry_constraints),
        facts.flow.incoming_reference_constraints(entry_constraints),
        place,
    )
}

fn resolve_descriptor_places_with_loans(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    loans: impl Iterator<Item = Handle<BorrowLoanFact>>,
    incoming: impl Iterator<Item = Handle<checked_trees::IncomingReferenceIdentity>>,
    place: CapturedPlace,
) -> Option<Vec<ResolvedPlace>> {
    let borrow = &facts.borrow;
    let (_, state_borrow) =
        borrow.first_state_borrow(state_flow.machine_symbol, state_flow.state_symbol)?;
    // A stored descriptor can be a whole reference binding or a reference
    // leaf inside a completed carrier. The live loan's exact owner path names
    // that descriptor; an access to its enclosing owned record is independent.
    let mut candidates = loans
        .filter(|handle| {
            let loan = borrow.loans.get(*handle);
            let owner = borrow.loan_owner_path(loan);
            borrow.state_owns_loan(state_borrow, *handle)
                // The dependency remains active for lifetime/interference
                // checking, but supplies no descriptor address or ancestry.
                && loan.lineage != BorrowLoanLineage::LifetimeDependency
                && loan.storage_owner == place.root
                && owner.len() <= place.segments.len()
                && owner.iter().zip(&place.segments).all(|(owner, selected)| {
                    crate::borrow::owner_segment_matches_place_segment(program, owner, selected)
                        == Some(true)
                })
        })
        .collect::<Vec<_>>();
    let incoming = incoming.collect::<Vec<_>>();
    // A flow cut can retain only this invocation's actual entry identities.
    // Invalid or cross-state references are unknown, never a request to fall
    // back to the current parameter's address.
    if incoming
        .iter()
        .any(|identity| !borrow.state_owns_incoming_reference(state_borrow, *identity))
    {
        return None;
    }
    let matches_incoming = |identity| {
        let incoming = borrow.incoming_references.get(identity);
        let owner = borrow.owner_segments.span_or_empty(incoming.owner_path);
        incoming.place_root() == place.root
            && owner.len() <= place.segments.len()
            && owner.iter().zip(&place.segments).all(|(owner, selected)| {
                crate::borrow::owner_segment_matches_place_segment(program, owner, selected)
                    == Some(true)
            })
    };
    let incoming = incoming
        .into_iter()
        .filter(|identity| matches_incoming(*identity))
        .collect::<Vec<_>>();
    let depth =
        candidates
            .iter()
            .map(|handle| borrow.loan_owner_path(borrow.loans.get(*handle)).len())
            .chain(incoming.iter().map(|identity| {
                borrow.incoming_references.get(*identity).owner_path.count() as usize
            }))
            .max();
    let Some(depth) = depth else {
        // A borrowed parameter supplies its root referent directly. Interior
        // descriptors instead require their exact retained ingress resource;
        // a retired leaf cannot recover authority from its parameter's type.
        let state = program.state_by_symbol(state_flow.state_symbol)?;
        let root_parameter = program.state_parameters(state).iter().any(|parameter| {
            if place.root != facts::PlaceRoot::Symbol(parameter.symbol)
                || parameter.is_const
                || parameter.relevance.is_erased()
            {
                return false;
            }
            let Some(reference) = validation::reference_shell(program, parameter.type_reference)
            else {
                return false;
            };
            // The parameter's grant covers owned projections of its root
            // referent. Every further descriptor crossing requires a retained
            // resource, including the final reference leaf itself.
            validation::owned_projection_type_reference(program, reference.referee, &place.segments)
                .is_some_and(|selected| validation::reference_shell(program, selected).is_none())
        });
        return root_parameter.then_some(vec![ResolvedPlace {
            place,
            local_loan: Handle::invalid(),
        }]);
    };
    candidates.retain(|handle| borrow.loan_owner_path(borrow.loans.get(*handle)).len() == depth);
    let mut places: Vec<_> = candidates
        .into_iter()
        .map(|handle| {
            let loan = borrow.loans.get(handle);
            if !loan.root.is_valid() {
                return None;
            }
            let mut segments = borrow.loan_segments(loan).to_vec();
            segments.extend_from_slice(&place.segments[depth..]);
            Some(ResolvedPlace {
                place: CapturedPlace {
                    incoming_reference: loan.incoming_reference,
                    root: loan.root,
                    segments,
                },
                local_loan: handle,
            })
        })
        .collect::<Option<_>>()?;
    for identity in incoming {
        let incoming = borrow.incoming_references.get(identity);
        if incoming.owner_path.count() as usize == depth {
            places.push(ResolvedPlace {
                place: CapturedPlace {
                    incoming_reference: identity,
                    root: incoming.place_root(),
                    segments: place.segments[depth..].to_vec(),
                },
                local_loan: Handle::invalid(),
            });
        }
    }
    (!places.is_empty()).then_some(places)
}

/// Forming a lifetime debt retains an already supplied input; it does not
/// borrow that input again. Authenticate every supplier before admitting its
/// formation alongside a common ancestor. This query is only for formation:
/// ordinary descriptor access still rejects LifetimeDependency as a source.
pub(in crate::checks::borrows) fn formation_lineage(
    borrow: &BorrowFacts,
    state: &StateBorrowFact,
    handle: Handle<BorrowLoanFact>,
) -> Option<Vec<Handle<BorrowLoanFact>>> {
    if !borrow.state_owns_loan(state, handle) {
        return None;
    }
    let loan = borrow.loans.get(handle);
    if loan.lineage != BorrowLoanLineage::LifetimeDependency {
        return retained_lineage(borrow, state, handle);
    }
    let suppliers = borrow.loan_suppliers.span(loan.supplying_loans)?;
    if suppliers.is_empty() || !loan.source_owner_symbol.is_valid() {
        return None;
    }
    let mut common = Vec::new();
    for (position, supplier) in suppliers.iter().enumerate() {
        if !borrow.state_owns_loan(state, *supplier) {
            return None;
        }
        let source = borrow.loans.get(*supplier);
        if source.owner_symbol != loan.source_owner_symbol
            || source.kind.direct_reborrow_effect(&loan.kind).is_none()
        {
            return None;
        }
        let lineage = retained_lineage(borrow, state, *supplier)?;
        if position == 0 {
            common = lineage;
        } else {
            common.retain(|ancestor| {
                lineage
                    .iter()
                    .any(|candidate| same_descriptor(borrow, *ancestor, *candidate))
            });
        }
    }
    Some(common)
}

pub(in crate::checks::borrows) fn retained_lineage(
    borrow: &BorrowFacts,
    state: &StateBorrowFact,
    handle: Handle<BorrowLoanFact>,
) -> Option<Vec<Handle<BorrowLoanFact>>> {
    // A descriptor transfer transports its suppliers' established lineage,
    // without turning the copied carrier into a new reborrow parent. Traverse
    // exact historical identities only: backing coincidence grants nothing.
    // Every alternative must be complete; a partial supplier graph is unknown.
    // Equal-place alternatives may have different histories. Only ancestors
    // common to every supplier justify an access exemption after their join.
    let mut completed: Vec<(Handle<BorrowLoanFact>, Vec<Handle<BorrowLoanFact>>)> = Vec::new();
    let mut active = Vec::new();
    let mut pending = vec![(handle, false)];
    while let Some((handle, closing)) = pending.pop() {
        if closing {
            if active.pop() != Some(handle) {
                return None;
            }
            let loan = borrow.loans.get(handle);
            let parents = match &loan.lineage {
                BorrowLoanLineage::DirectRoot => &[][..],
                BorrowLoanLineage::SuppliedOutput => {
                    borrow.supplied_output_call(handle)?;
                    &[][..]
                }
                BorrowLoanLineage::Reborrow { parent_loan } => std::slice::from_ref(parent_loan),
                BorrowLoanLineage::UnretainedDerived => {
                    borrow.loan_suppliers.span(loan.supplying_loans)?
                }
                BorrowLoanLineage::LifetimeDependency => return None,
            };
            let mut common = Vec::new();
            for (position, parent) in parents.iter().enumerate() {
                let (_, ancestors) = completed
                    .iter()
                    .find(|(candidate, _)| candidate == parent)?;
                if position == 0 {
                    common.clone_from(ancestors);
                } else {
                    // Alternative origins have distinct loan rows but may
                    // belong to the same exact descriptor occurrence. Keep
                    // only occurrences present on every supplier path, not
                    // merely identical row handles or equal backing places.
                    common.retain(|ancestor| {
                        ancestors
                            .iter()
                            .any(|candidate| same_descriptor(borrow, *ancestor, *candidate))
                    });
                }
            }
            common.push(handle);
            completed.push((handle, common));
            continue;
        }
        if completed.iter().any(|(candidate, _)| *candidate == handle) {
            continue;
        }
        if !borrow.state_owns_loan(state, handle) || active.contains(&handle) {
            return None;
        }
        active.push(handle);
        pending.push((handle, true));
        let loan = borrow.loans.get(handle);
        match loan.lineage {
            BorrowLoanLineage::DirectRoot => {}
            BorrowLoanLineage::SuppliedOutput => {
                borrow.supplied_output_call(handle)?;
            }
            BorrowLoanLineage::Reborrow { parent_loan } => {
                if !borrow.state_owns_loan(state, parent_loan) {
                    return None;
                }
                let parent = borrow.loans.get(parent_loan);
                if loan.source_owner_symbol != parent.owner_symbol
                    || parent.kind.direct_reborrow_effect(&loan.kind).is_none()
                {
                    return None;
                }
                pending.push((parent_loan, false));
            }
            BorrowLoanLineage::UnretainedDerived => {
                let suppliers = borrow.loan_suppliers.span(loan.supplying_loans)?;
                if suppliers.is_empty() {
                    return None;
                }
                for supplier in suppliers {
                    if !borrow.state_owns_loan(state, *supplier) {
                        return None;
                    }
                    let source = borrow.loans.get(*supplier);
                    if loan.source_owner_symbol != source.owner_symbol
                        || source.kind.direct_reborrow_effect(&loan.kind).is_none()
                    {
                        return None;
                    }
                    pending.push((*supplier, false));
                }
            }
            BorrowLoanLineage::LifetimeDependency => return None,
        }
    }
    completed
        .into_iter()
        .find_map(|(candidate, lineage)| (candidate == handle).then_some(lineage))
}
