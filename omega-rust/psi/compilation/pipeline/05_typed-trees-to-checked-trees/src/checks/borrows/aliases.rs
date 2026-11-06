//! Local reference operands rejoin retained storage before overlap comparison.
//! A captured place does not grant ancestry: only receivers with independently
//! retained lineage may exempt their own parent loans from interference.
use crate::checks::borrows::loan_occurrences::LoanOccurrences;
use arena::{Handle, HandleSpan};
use checked_trees::{
    BorrowFacts, BorrowLoanFact, BorrowLoanLineage, BorrowLoanOwnerSegment, StateBorrowFact,
};
use checked_trees::{CapturedPlace, CheckFacts, FlowStateFact};
use typed_trees::statement::StatementNode;
use typed_trees::types::TypeReferenceNode;

mod lineage;
pub(in crate::checks) use lineage::transport_suppliers;
pub(in crate::checks::borrows) use lineage::{formation_lineage, retained_lineage};

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
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        handle: Handle<BorrowLoanFact>,
    ) -> bool {
        self.lineage.iter().any(|ancestor| {
            same_descriptor(facts, loan_occurrences, *ancestor, handle)
                || retains_lifetime_dependency(facts, loan_occurrences, *ancestor, handle)
        })
    }
}

/// Alternatives belong to an exact descriptor occurrence, never merely to
/// equal backing geometry. Wildcard paths may describe coexisting slots.
pub(in crate::checks::borrows) fn same_descriptor(
    facts: &CheckFacts,
    loan_occurrences: &LoanOccurrences<'_>,
    left: Handle<BorrowLoanFact>,
    right: Handle<BorrowLoanFact>,
) -> bool {
    let borrow = &facts.borrow;
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
    same_carrier_occurrence(facts, loan_occurrences, left, right)
}

/// One spatial descriptor retains its own lifetime cohort independently of
/// where it points. `lineage` contains only suppliers common to every route;
/// requiring this exact historical occurrence cannot revive a replaced slot or
/// turn an unselected input's address into a descriptor alternative.
fn retains_lifetime_dependency(
    facts: &CheckFacts,
    loan_occurrences: &LoanOccurrences<'_>,
    descriptor: Handle<BorrowLoanFact>,
    debt: Handle<BorrowLoanFact>,
) -> bool {
    let borrow = &facts.borrow;
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
        && same_carrier_occurrence(facts, loan_occurrences, descriptor, debt)
}

fn same_carrier_occurrence(
    facts: &CheckFacts,
    loan_occurrences: &LoanOccurrences<'_>,
    left: &BorrowLoanFact,
    right: &BorrowLoanFact,
) -> bool {
    let borrow = &facts.borrow;
    let path = borrow.loan_owner_path(right);
    (same_carrier_generation(left, right)
        || same_resolved_descriptor_load(facts, loan_occurrences, left, right))
        && borrow.loan_owner_path(left) == path
        && !path.contains(&checked_trees::BorrowLoanOwnerSegment::DynamicIndex)
}

/// Repeated reads of one unchanged input slot retain that descriptor's
/// occurrence. The completed source identifies the slot; expression-owned
/// temporary homes remain independent of the input's field path.
fn same_resolved_descriptor_load(
    facts: &CheckFacts,
    loan_occurrences: &LoanOccurrences<'_>,
    left: &BorrowLoanFact,
    right: &BorrowLoanFact,
) -> bool {
    if !is_descriptor_load(&facts.borrow, left)
        || !is_descriptor_load(&facts.borrow, right)
        || left.owner_symbol != right.owner_symbol
        || left.statement_index != right.statement_index
        || left.established_by_call != right.established_by_call
    {
        return false;
    }
    let slot = |loan| loan_occurrences.descriptor_slot(loan);
    matches!((slot(left), slot(right)), (Ok(left), Ok(right)) if left == right)
}

fn is_descriptor_load(borrow: &BorrowFacts, loan: &BorrowLoanFact) -> bool {
    borrow.incoming_references.is_valid(loan.incoming_reference)
        && matches!(
            borrow
                .incoming_references
                .get(loan.incoming_reference)
                .origin,
            checked_trees::IncomingReferenceOrigin::DescriptorLoad { .. }
        )
}

fn same_carrier_generation(left: &BorrowLoanFact, right: &BorrowLoanFact) -> bool {
    right.owner_symbol.is_valid()
        && right.storage_owner.is_valid()
        && left.owner_symbol == right.owner_symbol
        && left.storage_owner == right.storage_owner
        && left.statement_index == right.statement_index
        && left.established_by_call == right.established_by_call
}

/// Formation may retain a descriptor together with its existing lifetime
/// obligations. Distinct cases of that same saved carrier cannot be present
/// together. Neither judgment supplies an address or permits a later read.
pub(in crate::checks::borrows) fn formation_preserves_loan(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    loan_occurrences: &LoanOccurrences<'_>,
    state: &StateBorrowFact,
    lineage: &[Handle<BorrowLoanFact>],
    active: Handle<BorrowLoanFact>,
) -> bool {
    lineage.iter().any(|ancestor| {
        same_descriptor(facts, loan_occurrences, *ancestor, active)
            || retains_lifetime_dependency(facts, loan_occurrences, *ancestor, active)
            || retains_lifetime_dependency(facts, loan_occurrences, active, *ancestor)
            || exclusive_carrier_cases(program, &facts.borrow, state, *ancestor, active)
    })
}

fn exclusive_carrier_cases(
    program: &dyn validation::CheckingInput,
    borrow: &BorrowFacts,
    state: &StateBorrowFact,
    left: Handle<BorrowLoanFact>,
    right: Handle<BorrowLoanFact>,
) -> bool {
    let exclusive = (|| {
        if !borrow.state_owns_loan(state, left) || !borrow.state_owns_loan(state, right) {
            return None;
        }
        let left = borrow.loans.get(left);
        let right = borrow.loans.get(right);
        if !same_carrier_generation(left, right) {
            return None;
        }
        let owner = left.storage_owner.as_symbol()?;
        let state = program.state_by_symbol(state.state_symbol)?;
        let reference = validation::state_root_type(program, state, owner)?;
        let left = crate::borrow::named_carrier_path(
            program,
            state,
            owner,
            reference,
            borrow.owner_segments.span(left.owner_path)?,
        )?;
        let right = crate::borrow::named_carrier_path(
            program,
            state,
            owner,
            reference,
            borrow.owner_segments.span(right.owner_path)?,
        )?;
        // The complete typed paths authenticate both cases under this one
        // applied carrier. A different field or index supplies no exclusion.
        for (left, right) in left.iter().zip(&right) {
            if left != right {
                return Some(matches!(
                    (left, right),
                    (
                        checked_trees::CheckedUnitStructuralPathSegment::Case(_),
                        checked_trees::CheckedUnitStructuralPathSegment::Case(_)
                    )
                ));
            }
        }
        None
    })();
    exclusive == Some(true)
}

pub(in crate::checks::borrows) struct ResolvedPlace {
    pub(in crate::checks::borrows) place: CapturedPlace,
    pub(in crate::checks::borrows) local_loan: Handle<BorrowLoanFact>,
    /// Prefix consumed at the descriptor's source slot. A loaded pointer's
    /// own home may be empty even when that slot lies inside an input record.
    pub(in crate::checks::borrows) consumed_segments: usize,
}

pub(in crate::checks::borrows) fn resolve_place(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    loan_occurrences: &LoanOccurrences<'_>,
    state_flow: &FlowStateFact,
    entry_constraints: HandleSpan<checked_trees::FlowConstraintRef>,
    place: CapturedPlace,
) -> Option<ResolvedPlace> {
    let mut places = resolve_places(
        program,
        facts,
        loan_occurrences,
        state_flow,
        entry_constraints,
        place,
    )?;
    (places.len() == 1).then(|| places.remove(0))
}

pub(in crate::checks::borrows) fn resolve_places(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    loan_occurrences: &LoanOccurrences<'_>,
    state_flow: &FlowStateFact,
    entry_constraints: HandleSpan<checked_trees::FlowConstraintRef>,
    place: CapturedPlace,
) -> Option<Vec<ResolvedPlace>> {
    resolve_places_with_loans(
        program,
        facts,
        loan_occurrences,
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
    loan_occurrences: &LoanOccurrences<'_>,
    state_flow: &FlowStateFact,
    loans: impl Iterator<Item = Handle<BorrowLoanFact>>,
    place: CapturedPlace,
) -> Option<Vec<ResolvedPlace>> {
    let state = program.state_by_symbol(state_flow.state_symbol)?;
    let reference_local = crate::flow::state_statements(program, state.statement_nodes)
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
            consumed_segments: 0,
        }]);
    }
    resolve_descriptor_places_with_loans(
        program,
        facts,
        loan_occurrences,
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
    loan_occurrences: &LoanOccurrences<'_>,
    state_flow: &FlowStateFact,
    entry_constraints: HandleSpan<checked_trees::FlowConstraintRef>,
    place: CapturedPlace,
) -> Option<Vec<ResolvedPlace>> {
    resolve_descriptor_places_with_loans(
        program,
        facts,
        loan_occurrences,
        state_flow,
        facts.flow.borrow_loan_constraints(entry_constraints),
        facts.flow.incoming_reference_constraints(entry_constraints),
        place,
    )
}

fn resolve_descriptor_places_with_loans(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    loan_occurrences: &LoanOccurrences<'_>,
    state_flow: &FlowStateFact,
    loans: impl Iterator<Item = Handle<BorrowLoanFact>>,
    incoming: impl Iterator<Item = Handle<checked_trees::IncomingReferenceIdentity>>,
    mut place: CapturedPlace,
) -> Option<Vec<ResolvedPlace>> {
    let borrow = &facts.borrow;
    let (_, state_borrow) =
        borrow.first_state_borrow(state_flow.machine_symbol, state_flow.state_symbol)?;
    let carrier = place.root.as_symbol().and_then(|root| {
        validation::state_root_type(
            program,
            program.state_by_symbol(state_flow.state_symbol)?,
            root,
        )
    });
    if let Some(carrier) = carrier {
        place.segments =
            validation::corresponding_place_projection(program, carrier, &place.segments)?;
    }
    let matches_owner = |owner: &[BorrowLoanOwnerSegment]| {
        let selected;
        let owner = if let Some(carrier) = carrier {
            let Some(path) = validation::corresponding_loan_owner_path(program, carrier, owner)
            else {
                return false;
            };
            selected = path;
            selected.as_slice()
        } else {
            owner
        };
        owner.len() <= place.segments.len()
            && owner.iter().zip(&place.segments).all(|(owner, selected)| {
                crate::borrow::owner_segment_matches_place_segment(program, owner, selected)
                    == Some(true)
            })
    };
    // A stored descriptor can be a whole reference binding or a reference
    // leaf inside a completed carrier. The live loan's exact owner path names
    // that descriptor; an access to its enclosing owned record is independent.
    let mut candidates = loans
        .filter_map(|handle| {
            let loan = borrow.loans.get(handle);
            if !borrow.state_owns_loan(state_borrow, handle)
                // The dependency remains active for lifetime/interference
                // checking, but supplies no descriptor address or ancestry.
                || loan.lineage == BorrowLoanLineage::LifetimeDependency
            {
                return None;
            }
            if loan.storage_owner == place.root {
                let owner = borrow.loan_owner_path(loan);
                return matches_owner(owner).then_some((handle, owner.len()));
            }
            // The captured read names the original input slot even after
            // replacement. Never project that slot's path through the loaded
            // pointer's temporary home or use it as the pointee namespace.
            let slot = loan_occurrences.descriptor_slot(loan).ok()?;
            (slot.root == place.root
                && slot.segments.len() <= place.segments.len()
                && slot
                    .segments
                    .iter()
                    .zip(&place.segments)
                    .all(|(left, right)| {
                        validation::storage_path_relations::canonical_place_segments_equal(
                            program, *left, *right,
                        )
                    }))
            .then_some((handle, slot.segments.len()))
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
        incoming.place_root() == place.root && matches_owner(owner)
    };
    let incoming = incoming
        .into_iter()
        .filter(|identity| matches_incoming(*identity))
        .collect::<Vec<_>>();
    let depth =
        candidates
            .iter()
            .map(|(_, depth)| *depth)
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
            consumed_segments: 0,
        }]);
    };
    candidates.retain(|(_, candidate_depth)| *candidate_depth == depth);
    let mut places: Vec<_> = candidates
        .into_iter()
        .map(|(handle, _)| {
            let loan = borrow.loans.get(handle);
            if !loan.root.is_valid() {
                return None;
            }
            let mut segments = borrow.loan_segments(loan).to_vec();
            segments.extend_from_slice(&place.segments[depth..]);
            Some(ResolvedPlace {
                place: CapturedPlace {
                    incoming_reference: loan.incoming_reference,
                    incoming_owner_path: facts
                        .borrow
                        .access_segments
                        .span_or_empty(loan.incoming_owner_path)
                        .to_vec(),
                    root: loan.root,
                    segments,
                },
                local_loan: handle,
                consumed_segments: depth,
            })
        })
        .collect::<Option<_>>()?;
    for identity in incoming {
        let incoming = borrow.incoming_references.get(identity);
        if incoming.owner_path.count() as usize == depth {
            places.push(ResolvedPlace {
                place: CapturedPlace {
                    incoming_reference: identity,
                    incoming_owner_path: Vec::new(),
                    root: incoming.place_root(),
                    segments: place.segments[depth..].to_vec(),
                },
                local_loan: Handle::invalid(),
                consumed_segments: depth,
            });
        }
    }
    (!places.is_empty()).then_some(places)
}

/// Join an unchanged selected slot from the completed-storage owner to its
/// actual stored entry descriptor. This includes the inner `&Cell` of an
/// `&mut &Cell` input: the outer borrowed grant has a different origin even
/// though both owner paths are empty. Only a completed-storage Incoming
/// selection may use this join; a declared path alone is no current value.
pub(in crate::checks::borrows) fn unchanged_incoming_leaf(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    constraints: arena::HandleSpan<checked_trees::FlowConstraintRef>,
    source: &validation::CanonicalPlace,
) -> Result<Option<Vec<ResolvedAlias>>, &'static str> {
    let borrow = &facts.borrow;
    let (_, state_borrow) = borrow
        .first_state_borrow(state.machine_symbol, state.state_symbol)
        .ok_or("the incoming descriptor has no owning borrow state")?;
    let mut resolved = Vec::new();
    for identity in facts.flow.incoming_reference_constraints(constraints) {
        if !borrow.state_owns_incoming_reference(state_borrow, identity) {
            return Err("the current descriptor roster contains a foreign incoming identity");
        }
        let incoming = borrow.incoming_references.get(identity);
        let owner = borrow
            .owner_segments
            .span(incoming.owner_path)
            .ok_or("the incoming descriptor has an incomplete owner path")?;
        if incoming.place_root() != source.root
            || !matches!(
                incoming.origin,
                checked_trees::IncomingReferenceOrigin::Parameter { .. }
            )
        {
            continue;
        }
        let Some(segments) = validation::corresponding_place_projection(
            program,
            incoming.carrier_type,
            &source.segments,
        ) else {
            continue;
        };
        if !crate::borrow::owner_path_selects_leaf(program, owner, &segments) {
            continue;
        }
        let selected =
            validation::owned_projection_type_reference(program, incoming.carrier_type, &segments)
                .ok_or("the selected incoming descriptor has no typed owner projection")?;
        validation::reference_shell(program, selected)
            .ok_or("the incoming selection is not a descriptor")?;
        resolved.push(ResolvedAlias {
            place: CapturedPlace {
                incoming_reference: identity,
                incoming_owner_path: if owner.contains(&BorrowLoanOwnerSegment::DynamicIndex) {
                    segments
                } else {
                    Vec::new()
                },
                root: incoming.place_root(),
                segments: Vec::new(),
            },
            lineage: Vec::new(),
        });
    }
    Ok((!resolved.is_empty()).then_some(resolved))
}
