//! A saved reference keeps access to its original data after its source slot
//! changes. Follow that saved value to the checked borrow that permits its use.
//! Completed captures and projections identify every possible supplier; equal
//! geometry and lifetime roots cannot create that permission association.
//! A temporary call access may end while its stored descriptor retains it.

use super::retained_backing::retained_places;
use crate::borrow::{owner_path_from_place_segments, owner_place_segments};
use crate::published::places::place_at;
use arena::{Arena, Handle};
use checked_trees::CheckedCallBorrowLoanSource;
use checked_trees::{BorrowLoanFact, CheckFacts, FlowCallFact, FlowOperandFact, FlowStateFact};
use validation::completed_storage::{FrozenReferenceSelection, frozen_reference_leaf};

/// Permission selected by an exact completed descriptor. Its consumer owns
/// destination paths and invocation coordinates separately.
pub(super) struct PermissionSupplier {
    pub(super) source: CheckedCallBorrowLoanSource,
    pub(super) backing: checked_trees::CapturedPlace,
}

/// Find the permission lending a new view, separately from the complete call
/// input debt. Stored descriptor alternatives retain their own parents; the
/// call's conservative ancestry union cannot choose a parent for one leaf.
/// An explicit argument borrow supplies its checked access when it has no
/// stored descriptor occurrence. Both routes must establish complete coverage.
pub(super) fn lending_sources(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: Handle<FlowCallFact>,
    source: Handle<FlowOperandFact>,
    projection: &[facts::PlaceSegment],
    access: &checked_trees::BorrowAccessKind,
) -> Option<(Vec<PermissionSupplier>, Arena<Handle<BorrowLoanFact>>)> {
    use super::formation_captures::{AddressReceiver, address_receiver, formation_capture};
    let capture = facts.flow.control.operands.get(source);
    let mut lending = source;
    let mut lending_path = projection.to_vec();
    let mut actual_projection = Vec::new();
    let mut argument_grant = projection.is_empty() && has_captured_incoming_contents(facts, source);
    if projection.is_empty()
        && let Some(formation) = validation::reference_formation_source(program, capture.expression)
    {
        let target = formation_capture(facts, source, formation.source)?;
        match address_receiver(program, facts, target) {
            AddressReceiver::Reference {
                capture: receiver,
                projection,
            } => {
                let target_type = facts.flow.control.operands.get(target).type_reference;
                let result = validation::reference_shell(program, capture.type_reference)?;
                if formation.interpretation
                    == validation::ReferenceFormationInterpretation::Declared
                    && validation::storage_origin_types_match(program, result.referee, target_type)
                {
                    // This forms a borrow of descriptor storage, rather than
                    // forwarding the reference stored in that descriptor.
                    argument_grant = true;
                } else {
                    lending = receiver;
                    lending_path.clear();
                    actual_projection = projection;
                    argument_grant = has_captured_incoming_contents(facts, receiver);
                }
            }
            AddressReceiver::InlineValue => argument_grant = true,
            AddressReceiver::Unavailable => return None,
        }
    }
    // An explicit argument formation or incoming association can request the
    // checked CallAccess grant. Stored-source failure never switches routes.
    let (invocation, lending, lending_path) = if argument_grant {
        (call, source, projection)
    } else {
        (Handle::invalid(), lending, lending_path.as_slice())
    };
    let mut suppliers = Vec::new();
    let mut parents = Arena::new();
    let complete = collect(
        program,
        facts,
        state,
        invocation,
        lending,
        lending_path,
        access,
        false,
        &mut Vec::new(),
        &mut suppliers,
        &mut parents,
    );
    if !complete || suppliers.is_empty() {
        let value = facts.flow.control.operands.get(lending);
        let frozen =
            match frozen_reference_leaf(program, &facts.flow.control, lending, lending_path) {
                FrozenReferenceSelection::Unavailable => "Unavailable",
                FrozenReferenceSelection::NotFrozen => "NotFrozen",
                FrozenReferenceSelection::ThroughReference => "ThroughReference",
                FrozenReferenceSelection::Endpoint { .. } => "Endpoint",
            };
        let actuals = facts
            .flow
            .control
            .calls
            .get(call)
            .argument_operands()
            .handles()
            .filter_map(|handle| {
                let actual = facts.flow.control.operands.get(handle);
                (actual.expression == capture.expression).then_some((
                    handle,
                    *actual == *capture,
                    actual.state_symbol,
                    actual.statement_index,
                    actual.type_reference,
                    actual.completion,
                ))
            })
            .collect::<Vec<_>>();
        eprintln!(
            "lending diagnostic: source {:?} -> {:?}; source state {:?} statement {} expression {:?} type {:?} completion {:?} frozen {}; incoming {}; actuals {:?}",
            source,
            lending,
            value.state_symbol,
            value.statement_index,
            value.expression,
            value.type_reference,
            value.completion,
            frozen,
            has_captured_incoming_contents(facts, lending),
            actuals
        );
        return None;
    }
    if !argument_grant {
        for supplier in &mut suppliers {
            supplier
                .backing
                .segments
                .extend_from_slice(&actual_projection);
            crate::flow::compose_captured_view_segments(&mut supplier.backing.segments)?;
        }
    }
    Some((suppliers, parents))
}

fn has_captured_incoming_contents(facts: &CheckFacts, source: Handle<FlowOperandFact>) -> bool {
    let mut source = source;
    let mut visited = Vec::new();
    while facts.flow.control.operands.is_valid(source) && !visited.contains(&source) {
        visited.push(source);
        let capture = facts.flow.control.operands.get(source);
        if let checked_trees::FlowOperandCompletion::IncomingContents { .. } = capture.completion {
            return true;
        }
        source = capture.value_source;
    }
    false
}

pub(super) fn collect(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: Handle<FlowCallFact>,
    source: Handle<FlowOperandFact>,
    projection: &[facts::PlaceSegment],
    output_access: &checked_trees::BorrowAccessKind,
    retained_by_store: bool,
    active: &mut Vec<(Handle<FlowOperandFact>, Vec<facts::PlaceSegment>)>,
    output: &mut Vec<PermissionSupplier>,
    parents: &mut Arena<Handle<BorrowLoanFact>>,
) -> bool {
    macro_rules! push_stored_loan {
        ($loan:expr, $row:expr) => {
            output.push(PermissionSupplier {
                source: CheckedCallBorrowLoanSource::StoredLoan { loan: $loan },
                backing: checked_trees::CapturedPlace {
                    incoming_reference: $row.incoming_reference,
                    incoming_owner_path: facts
                        .borrow
                        .access_segments
                        .span_or_empty($row.incoming_owner_path)
                        .to_vec(),
                    root: $row.root,
                    segments: facts.borrow.loan_segments($row).to_vec(),
                },
            });
        };
    }
    macro_rules! collect_supplier {
        ($source:expr, $projection:expr, $retained_by_store:expr $(,)?) => {
            collect(
                program,
                facts,
                state,
                call,
                $source,
                $projection,
                output_access,
                $retained_by_store,
                active,
                output,
                parents,
            )
        };
    }
    let Some((_, borrow_state)) = facts
        .borrow
        .first_state_borrow(state.machine_symbol, state.state_symbol)
    else {
        return false;
    };
    let query = (source, projection.to_vec());
    if active.contains(&query) {
        return false;
    }
    active.push(query);
    let before = output.len();
    if !facts.flow.control.operands.is_valid(source) {
        active.pop();
        return false;
    }
    let capture = facts.flow.control.operands.get(source);
    let retained_suppliers = validation::completed_storage::descriptor_suppliers(
        &facts.flow.control,
        capture.retention,
        &mut Vec::new(),
    );
    let frozen = frozen_reference_leaf(program, &facts.flow.control, source, projection);
    let resolved = if matches!(&frozen, FrozenReferenceSelection::Unavailable) {
        false
    } else if collect_completed_formations(
        program,
        facts,
        state,
        source,
        projection,
        output_access,
        output,
    ) {
        true
    } else if let checked_trees::FlowOperandCompletion::Selected(alternatives) = capture.completion
    {
        facts
            .flow
            .control
            .operand_alternatives
            .span(alternatives)
            .is_some_and(|rows| {
                !rows.is_empty()
                    && rows.iter().all(|alternative| {
                        alternative.source_arm.is_valid()
                            && collect_supplier!(alternative.value, projection, retained_by_store)
                    })
            })
    } else if let FrozenReferenceSelection::Endpoint {
        owner,
        leaf,
        path: leaf_path,
    } = frozen
    {
        if let Some(suppliers) = validation::completed_storage::descriptor_suppliers(
            &facts.flow.control,
            leaf.retention,
            &mut Vec::new(),
        ) {
            !suppliers.is_empty()
                && suppliers.iter().all(|supplier| {
                    facts
                        .flow
                        .control
                        .operand_retained_paths
                        .span(supplier.projection)
                        .is_some_and(|projection| {
                            collect_supplier!(supplier.operand, projection, true)
                        })
                })
        } else {
            let coverage = if projection.is_empty()
                && validation::reference_shell(program, capture.type_reference).is_some()
            {
                collect_invocation_accesses(
                    program,
                    facts,
                    state,
                    call,
                    capture,
                    output_access,
                    retained_by_store,
                    output,
                    parents,
                )
            } else {
                InvocationAccessCoverage::Absent
            };
            match coverage {
                InvocationAccessCoverage::Complete => true,
                InvocationAccessCoverage::Unavailable => false,
                InvocationAccessCoverage::Absent => {
                    if facts
                        .flow
                        .control
                        .operand_referents
                        .span(leaf.referents)
                        .is_some_and(|referents| {
                            !referents.is_empty()
                                && referents.iter().all(|referent| {
                                    facts
                                        .flow
                                        .control
                                        .referent_formations
                                        .is_valid(referent.formation)
                                        && facts
                                            .flow
                                            .control
                                            .referent_formations
                                            .get(referent.formation)
                                            .owner
                                            .is_valid()
                                })
                        })
                        && let Some(source_place) = place_at(
                            program,
                            owner.state_symbol,
                            owner.statement_index,
                            owner.expression,
                        )
                    {
                        // This is the private frozen inventory's exact original loan cut,
                        // not a recovery from the consumer's current source slot. Geometry
                        // and formation handles do not select or create a loan.
                        let mut source_path =
                            owner_path_from_place_segments(program, &source_place.segments);
                        source_path.extend(owner_path_from_place_segments(program, &leaf_path));
                        for loan in facts.flow.borrow_loan_constraints(leaf.constraints) {
                            let row = facts.borrow.loans.get(loan);
                            if matches!(
                                row.lineage,
                                checked_trees::BorrowLoanLineage::LifetimeDependency
                            ) || !facts.borrow.state_owns_loan(borrow_state, loan)
                                || row.storage_owner != source_place.root
                                || facts.borrow.loan_owner_path(row) != source_path
                            {
                                continue;
                            }
                            push_stored_loan!(loan, row);
                        }
                        output.len() > before
                    } else {
                        false
                    }
                }
            }
        }
    } else if !projection.is_empty() {
        let selected = validation::completed_storage::reference_captures_at_path(
            program,
            &facts.flow.control,
            source,
            projection,
            capture.storage_snapshot,
        )
        .and_then(|selected| selected.captures());
        selected.is_some_and(|selected| {
            !selected.is_empty()
                && selected.into_iter().all(|selected| {
                    // Opaque inline leaves can retain the exact completed occurrence
                    // without disclosing a descriptor supplier. Its checked formation
                    // loan is authority; selecting the same subtree again is a cycle.
                    if selected.capture == source && selected.projection == projection {
                        return collect_formed_projection(
                            program,
                            facts,
                            state,
                            source,
                            capture,
                            projection,
                            output_access,
                            output,
                        );
                    }
                    collect_supplier!(selected.capture, &selected.projection, retained_by_store)
                })
        })
    } else if let Some(suppliers) = retained_suppliers {
        !suppliers.is_empty()
            && suppliers.iter().all(|supplier| {
                collect_supplier!(
                    supplier.operand,
                    match facts
                        .flow
                        .control
                        .operand_retained_paths
                        .span(supplier.projection)
                    {
                        Some(projection) => projection,
                        None => return false,
                    },
                    true,
                )
            })
    } else {
        match collect_invocation_accesses(
            program,
            facts,
            state,
            call,
            capture,
            output_access,
            retained_by_store,
            output,
            parents,
        ) {
            InvocationAccessCoverage::Complete => true,
            InvocationAccessCoverage::Unavailable => false,
            InvocationAccessCoverage::Absent => {
                // A prior stored formation has no current invocation access. Its
                // exact occurrence, rather than the current slot path or backing
                // geometry, identifies the permission carried by this capture.
                for (loan, row) in facts.borrow.state_loans(borrow_state).filter(|(_, row)| {
                    !matches!(
                        row.lineage,
                        checked_trees::BorrowLoanLineage::LifetimeDependency
                    ) && capture.expression.is_valid()
                        && row.formation_expression == capture.expression
                        && row.statement_index == capture.statement_index
                }) {
                    push_stored_loan!(loan, row);
                }
                output.len() > before
            }
        }
    };
    active.pop();
    if !resolved {
        output.truncate(before);
    }
    resolved
}

/// Resolve the installed descriptor generation before joining a loan. A later
/// read has a different expression from its initializer; the completed-storage
/// relation carries that exact earlier formation without rereading its slot.
fn collect_completed_formations(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    source: Handle<FlowOperandFact>,
    projection: &[facts::PlaceSegment],
    access: &checked_trees::BorrowAccessKind,
    output: &mut Vec<PermissionSupplier>,
) -> bool {
    let selected = validation::completed_storage::predecessors_in_control(
        program,
        &facts.flow.control,
        source,
        projection,
        validation::completed_storage::StoredObservation::Binding,
        Some((state.state_symbol, state.statements)),
    );
    let Some(selected) = selected.filter(|selected| !selected.is_empty()) else {
        eprintln!(
            "lending diagnostic: no binding predecessor for {:?} at path {:?}",
            source, projection
        );
        return false;
    };
    eprintln!(
        "lending diagnostic: binding predecessors for {:?}: {:?}",
        source, selected
    );
    let before = output.len();
    for (selected, projection) in selected {
        if !facts.flow.control.operands.is_valid(selected)
            || !collect_formed_projection(
                program,
                facts,
                state,
                selected,
                facts.flow.control.operands.get(selected),
                &projection,
                access,
                output,
            )
        {
            output.truncate(before);
            return false;
        }
    }
    true
}

/// Exact checked access occurrences are permission evidence for a whole
/// evaluated descriptor, including one projected from an earlier owned copy.
/// A failed present roster cannot recover an older source-slot formation.
enum InvocationAccessCoverage {
    Absent,
    Complete,
    Unavailable,
}

fn collect_invocation_accesses(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: Handle<FlowCallFact>,
    capture: &FlowOperandFact,
    _output_access: &checked_trees::BorrowAccessKind,
    retained_by_store: bool,
    output: &mut Vec<PermissionSupplier>,
    parents: &mut Arena<Handle<BorrowLoanFact>>,
) -> InvocationAccessCoverage {
    if !facts.flow.control.calls.is_valid(call) {
        return InvocationAccessCoverage::Absent;
    }
    let invocation = facts.flow.control.calls.get(call);
    let accesses = invocation
        .accesses
        .handles()
        .filter(|access| {
            let access = facts.borrow.argument_accesses.get(*access);
            access.expression == capture.expression
                && (retained_by_store || access.end_before_access.is_none())
        })
        .collect::<Vec<_>>();
    if accesses.is_empty() {
        return InvocationAccessCoverage::Absent;
    }
    let before = output.len();
    for access in accesses {
        let row = facts.borrow.argument_accesses.get(access);
        let Some(backings) = retained_places(
            program,
            facts,
            state,
            capture.statement_index,
            capture.constraints,
            checked_trees::CapturedPlace {
                root: row.root,
                segments: facts.borrow.access_segments(row).to_vec(),
                ..Default::default()
            },
            &mut Vec::new(),
        ) else {
            output.truncate(before);
            return InvocationAccessCoverage::Unavailable;
        };
        if backings.is_empty() {
            output.truncate(before);
            return InvocationAccessCoverage::Unavailable;
        }
        for backing in backings {
            output.push(PermissionSupplier {
                source: CheckedCallBorrowLoanSource::CallAccess {
                    access,
                    parents: parents.insert_many(backing.lineage),
                },
                backing: backing.place,
            });
        }
    }
    InvocationAccessCoverage::Complete
}

/// Retain the existing loan of one completed result leaf. Public backing
/// regions are deliberately absent from this lookup: they cannot identify a
/// supplier or create permission. Destination storage prefixes do not identify
/// the formation leaf either, so the checker records that relative path once.
#[allow(clippy::too_many_arguments)]
fn collect_formed_projection(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    source: Handle<FlowOperandFact>,
    capture: &FlowOperandFact,
    projection: &[facts::PlaceSegment],
    output_access: &checked_trees::BorrowAccessKind,
    output: &mut Vec<PermissionSupplier>,
) -> bool {
    if capture.state_symbol != state.state_symbol
        || !capture.expression.is_valid()
        || projection.iter().any(|segment| {
            !matches!(
                segment,
                facts::PlaceSegment::Field { .. }
                    | facts::PlaceSegment::Case { .. }
                    | facts::PlaceSegment::FixedIndex { .. }
            )
        })
    {
        return false;
    }
    let Some(declared) = validation::expression_type_reference_in_state(
        program,
        capture.state_symbol,
        capture.statement_index,
        capture.expression,
    ) else {
        eprintln!(
            "lending diagnostic: formation {:?} has no declared expression type at state {:?} statement {} expression {:?}",
            source, capture.state_symbol, capture.statement_index, capture.expression
        );
        return false;
    };
    if !validation::storage_origin_types_match(program, declared, capture.type_reference) {
        eprintln!(
            "lending diagnostic: formation {:?} declared type {:?} disagrees with captured {:?}",
            source, declared, capture.type_reference
        );
        return false;
    }
    let Some((_, borrow_state)) = facts
        .borrow
        .first_state_borrow(state.machine_symbol, state.state_symbol)
    else {
        return false;
    };
    let Some(sources) = validation::completed_storage::descriptor_formation_sources(
        program,
        &facts.flow.control,
        source,
        projection,
        state.statements,
    ) else {
        return false;
    };
    let before = output.len();
    for (loan, row) in facts.borrow.state_loans(borrow_state) {
        let Some(formed) = row.formation_owner_path else {
            continue;
        };
        // The formation cut can retain lifetime debts alongside descriptor loans.
        // A keepalive row never supplies a physical address or reborrow permission.
        if matches!(
            row.lineage,
            checked_trees::BorrowLoanLineage::LifetimeDependency
        ) || row.kind.direct_reborrow_effect(output_access).is_none()
            || !sources.iter().any(|source| {
                let capture = facts.flow.control.operands.get(source.capture);
                row.formation_expression == capture.expression
                    && row.statement_index == capture.statement_index
                    && owner_place_segments(facts.borrow.owner_segments.span_or_empty(formed))
                        == source.projection
            })
        {
            continue;
        }
        output.push(PermissionSupplier {
            source: CheckedCallBorrowLoanSource::StoredLoan { loan },
            backing: checked_trees::CapturedPlace {
                incoming_reference: row.incoming_reference,
                incoming_owner_path: facts
                    .borrow
                    .access_segments
                    .span_or_empty(row.incoming_owner_path)
                    .to_vec(),
                root: row.root,
                segments: facts.borrow.loan_segments(row).to_vec(),
            },
        });
    }
    if output.len() == before {
        eprintln!(
            "lending diagnostic: formation {:?} sources {:?}; loans {:?}",
            source,
            sources
                .iter()
                .map(|source| {
                    let row = facts.flow.control.operands.get(source.capture);
                    (
                        source.capture,
                        row.expression,
                        row.statement_index,
                        source.projection.clone(),
                    )
                })
                .collect::<Vec<_>>(),
            facts
                .borrow
                .state_loans(borrow_state)
                .map(|(loan, row)| (
                    loan,
                    row.formation_expression,
                    row.statement_index,
                    row.formation_owner_path.map(|path| owner_place_segments(
                        facts.borrow.owner_segments.span_or_empty(path)
                    ))
                ))
                .collect::<Vec<_>>()
        );
    }
    output.len() > before
}
