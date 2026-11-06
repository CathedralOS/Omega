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
    /// Original evaluated reference when this is an invocation access.
    pub(super) actual: Handle<FlowOperandFact>,
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
) -> Result<(Vec<PermissionSupplier>, Arena<Handle<BorrowLoanFact>>), &'static str> {
    use super::super::formation_captures::{AddressReceiver, address_receiver, formation_capture};
    let capture = facts.flow.control.operands.get(source);
    let mut lending = source;
    let mut lending_path = projection.to_vec();
    let mut actual_projection = Vec::new();
    let mut argument_grant =
        projection.is_empty() && has_captured_entry_reference(program, facts, state, source);
    if projection.is_empty()
        && let Some(formation) = validation::reference_formation_source(program, capture.expression)
    {
        let target = formation_capture(facts, source, formation.source)
            .ok_or("the explicit reference source has no completed capture")?;
        match address_receiver(program, facts, target) {
            AddressReceiver::Reference {
                capture: receiver,
                projection,
            } => {
                let target_type = facts.flow.control.operands.get(target).type_reference;
                let result = validation::reference_shell(program, capture.type_reference)
                    .ok_or("the captured formation is not a reference")?;
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
                    argument_grant = has_captured_entry_reference(program, facts, state, receiver);
                }
            }
            AddressReceiver::InlineValue => argument_grant = true,
            AddressReceiver::Unavailable => {
                return Err("the formation receiver has no complete descriptor path");
            }
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
        return Err("the saved descriptor has no complete checked borrowing source");
    }
    if !argument_grant {
        for supplier in &mut suppliers {
            supplier
                .backing
                .segments
                .extend_from_slice(&actual_projection);
            crate::flow::compose_captured_view_segments(&mut supplier.backing.segments)
                .ok_or("the captured source projection cannot be composed")?;
        }
    }
    Ok((suppliers, parents))
}

/// A helper can retain either a borrowed input or a reference field read from
/// it. The unchanged storage snapshot and current entry roster identify that
/// descriptor. Its complete owner path selects the pointer's own permission;
/// an enclosing record's borrow cannot stand in for an interior reference.
fn has_captured_entry_reference(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    source: Handle<FlowOperandFact>,
) -> bool {
    let capture = facts.flow.control.operands.get(source);
    let Some(actual) = validation::reference_shell(program, capture.type_reference) else {
        return false;
    };
    if capture.state_symbol != state.state_symbol {
        return false;
    }
    let Some(place) = place_at(
        program,
        capture.state_symbol,
        capture.statement_index,
        capture.expression,
    ) else {
        return false;
    };
    if validation::completed_storage::at_snapshot(
        program,
        &facts.flow.control,
        capture.storage_snapshot,
        &place,
        validation::completed_storage::StoredObservation::Binding,
    )
    .is_some()
    {
        return false;
    }
    super::super::aliases::resolve_descriptor_places(
        program,
        facts,
        state,
        capture.constraints,
        checked_trees::CapturedPlace {
            root: place.root,
            segments: place.segments,
            ..Default::default()
        },
    )
    .is_some_and(|sources| {
        !sources.is_empty()
            && sources.iter().all(|source| {
                if source.local_loan.is_valid()
                    || !source.place.segments.is_empty()
                    || !facts
                        .borrow
                        .incoming_references
                        .is_valid(source.place.incoming_reference)
                {
                    return false;
                }
                let incoming = facts
                    .borrow
                    .incoming_references
                    .get(source.place.incoming_reference);
                if !matches!(
                    incoming.origin,
                    checked_trees::IncomingReferenceOrigin::ParameterReferent { .. }
                        | checked_trees::IncomingReferenceOrigin::Parameter { .. }
                ) {
                    return false;
                }
                let Some(owner) = facts.borrow.owner_segments.span(incoming.owner_path) else {
                    return false;
                };
                let Some(selected) = validation::owned_projection_type_reference(
                    program,
                    incoming.carrier_type,
                    &owner_place_segments(owner),
                ) else {
                    return false;
                };
                validation::reference_shell(program, selected).is_some_and(|declared| {
                    checked_trees::BorrowAccessKind::from(declared.access)
                        .direct_reborrow_effect(&checked_trees::BorrowAccessKind::from(
                            actual.access,
                        ))
                        .is_some()
                        && validation::storage_origin_types_match(
                            program,
                            actual.referee,
                            declared.referee,
                        )
                })
            })
    })
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
                actual: Handle::invalid(),
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
    let plain_observation = matches!(&frozen, FrozenReferenceSelection::NotFrozen);
    let frozen_kind = match &frozen {
        FrozenReferenceSelection::NotFrozen => "not-frozen",
        FrozenReferenceSelection::Endpoint { .. } => "endpoint",
        FrozenReferenceSelection::ThroughReference => "through-reference",
        FrozenReferenceSelection::Unavailable => "unavailable",
    };
    let mut resolved = if let Some(complete) = collect_completed_output(
        program,
        facts,
        state,
        capture,
        projection,
        output_access,
        output,
    ) {
        complete
    } else if matches!(&frozen, FrozenReferenceSelection::Unavailable) {
        false
    } else if collect_formed_projection(
        program,
        facts,
        state,
        capture,
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
    } else if let Some(complete) = collect_completed_formations(
        program,
        facts,
        state,
        call,
        source,
        projection,
        output_access,
        retained_by_store,
        active,
        output,
        parents,
    ) {
        complete
    } else if let Some(complete) = collect_descriptor_captures(
        program,
        facts,
        state,
        call,
        source,
        projection,
        output_access,
        retained_by_store,
        active,
        output,
        parents,
    ) {
        complete
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
                    source,
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
    } else if let Some(complete) = collect_reborrow_sources(
        program,
        facts,
        state,
        call,
        source,
        projection,
        output_access,
        active,
        output,
        parents,
    ) {
        complete
    } else if collect_binding_observation(
        program,
        facts,
        state,
        capture,
        projection,
        output_access,
        output,
    ) {
        true
    } else {
        match collect_invocation_accesses(
            program,
            facts,
            state,
            call,
            source,
            capture,
            output_access,
            retained_by_store,
            output,
            parents,
        ) {
            InvocationAccessCoverage::Complete => true,
            InvocationAccessCoverage::Unavailable => false,
            InvocationAccessCoverage::Absent => false,
        }
    };
    active.pop();
    if !resolved {
        if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
            eprintln!(
                "STRUCT descriptor-permission-unavailable source={source:?} expression={:?} completion={:?} permission={:?} path={projection:?} frozen={frozen_kind}",
                capture.expression, capture.completion, capture.descriptor_permission
            );
        }
        output.truncate(before);
        // An earlier capture strategy can refuse without consulting the
        // binding. A plain observation of that binding still has its loan.
        if plain_observation
            && collect_binding_observation(
                program,
                facts,
                state,
                capture,
                projection,
                output_access,
                output,
            )
        {
            resolved = true;
        }
    }
    resolved
}

/// A helper may save a narrower view of an evaluated argument. Its source
/// path selects the original descriptor; only the separate suffix projects
/// into that descriptor's data. Authenticate the original permission before
/// composing this checked projection, and preserve every source alternative.
#[allow(clippy::too_many_arguments)]
fn collect_reborrow_sources(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: Handle<FlowCallFact>,
    source: Handle<FlowOperandFact>,
    projection: &[facts::PlaceSegment],
    access: &checked_trees::BorrowAccessKind,
    active: &mut Vec<(Handle<FlowOperandFact>, Vec<facts::PlaceSegment>)>,
    output: &mut Vec<PermissionSupplier>,
    parents: &mut Arena<Handle<BorrowLoanFact>>,
) -> Option<bool> {
    let control = &facts.flow.control;
    let capture = control.operands.get(source);
    if !control.operand_retentions.is_valid(capture.retention) {
        return None;
    }
    let profile = control.operand_retentions.get(capture.retention);
    if profile.reborrow_sources.is_empty() {
        return None;
    }
    let resolve = || -> Option<Vec<(Handle<FlowOperandFact>, Vec<facts::PlaceSegment>, Vec<facts::PlaceSegment>)>> {
        if !projection.is_empty() {
            return None;
        }
        let result = validation::reference_shell(program, capture.type_reference)?;
        checked_trees::BorrowAccessKind::from(result.access).direct_reborrow_effect(access)?;
        let sources = control.operand_reborrow_sources.span(profile.reborrow_sources)?;
        let mut selected = Vec::new();
        for source in sources {
            if !control.operands.is_valid(source.operand) { return None; }
            let value = control.operands.get(source.operand);
            let path = control.operand_retained_paths.span(source.source_path)?;
            let reference = validation::owned_projection_type_reference(program, value.type_reference, path)?;
            let reference = validation::reference_shell(program, reference)?;
            checked_trees::BorrowAccessKind::from(reference.access).direct_reborrow_effect(access)?;
            let suffix = validation::corresponding_place_projection(
                program, reference.referee, control.operand_retained_paths.span(source.projection)?,
            )?;
            // The producer records the last crossed descriptor, so this suffix
            // contains only owned projections. A further descriptor crossing
            // must supply its own captured source and permission instead.
            let referee = validation::owned_projection_type_reference(program, reference.referee, &suffix)?;
            if !validation::storage_origin_types_match(program, referee, result.referee) {
                return None;
            }
            selected.push((source.operand, path.to_vec(), suffix));
        }
        (!selected.is_empty()).then_some(selected)
    };
    let Some(selected) = resolve() else {
        return Some(false);
    };
    let before = output.len();
    for (source, path, suffix) in selected {
        let start = output.len();
        if !collect(
            program, facts, state, call, source, &path, access, true, active, output, parents,
        ) {
            output.truncate(before);
            return Some(false);
        }
        for supplier in &mut output[start..] {
            supplier.backing.segments.extend_from_slice(&suffix);
            if crate::flow::compose_captured_view_segments(&mut supplier.backing.segments).is_none()
            {
                output.truncate(before);
                return Some(false);
            }
        }
    }
    Some(output.len() > before)
}

/// A pointer installed by a helper carries its completed output permission.
/// This link belongs to the produced value, so replacing its former variable
/// cannot retarget it. Input lifetime debts never enter this permission route.
fn collect_completed_output(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    capture: &FlowOperandFact,
    projection: &[facts::PlaceSegment],
    access: &checked_trees::BorrowAccessKind,
    output: &mut Vec<PermissionSupplier>,
) -> Option<bool> {
    if !matches!(
        capture.descriptor_permission,
        checked_trees::FlowDescriptorPermission::SuppliedOutput { .. }
    ) {
        return None;
    }
    let complete = (|| -> Result<PermissionSupplier, &'static str> {
        if !projection.is_empty() || capture.state_symbol != state.state_symbol {
            return Err("the output permission does not describe this whole captured leaf");
        }
        let loan = crate::borrow::supplied_outputs::captured_permission(
            program,
            &facts.borrow,
            &facts.flow,
            capture,
        )?;
        let row = facts.borrow.loans.get(loan);
        let incoming = facts.borrow.incoming_references.get(row.incoming_reference);
        if incoming.machine_symbol != state.machine_symbol
            || row.kind.direct_reborrow_effect(access).is_none()
        {
            return Err("the output permission cannot lend this state or access");
        }
        Ok(PermissionSupplier {
            source: CheckedCallBorrowLoanSource::StoredLoan { loan },
            actual: Handle::invalid(),
            backing: checked_trees::CapturedPlace {
                incoming_reference: row.incoming_reference,
                incoming_owner_path: facts
                    .borrow
                    .access_segments
                    .span(row.incoming_owner_path)
                    .ok_or("the output permission lost its incoming path")?
                    .to_vec(),
                root: row.root,
                segments: facts.borrow.loan_segments(row).to_vec(),
            },
        })
    })();
    Some(match complete {
        Ok(supplier) => {
            output.push(supplier);
            true
        }
        Err(reason) => {
            if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
                eprintln!("STRUCT supplied-descriptor-permission-refused {reason}");
            }
            false
        }
    })
}

/// A helper can return a saved reference without making a new borrow. Follow
/// that exact descriptor even when the whole reference, rather than a field,
/// is selected. Its lifetime sources alone cannot authorize the next view.
#[allow(clippy::too_many_arguments)]
fn collect_descriptor_captures(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: Handle<FlowCallFact>,
    source: Handle<FlowOperandFact>,
    projection: &[facts::PlaceSegment],
    access: &checked_trees::BorrowAccessKind,
    retained_by_store: bool,
    active: &mut Vec<(Handle<FlowOperandFact>, Vec<facts::PlaceSegment>)>,
    output: &mut Vec<PermissionSupplier>,
    parents: &mut Arena<Handle<BorrowLoanFact>>,
) -> Option<bool> {
    let capture = facts.flow.control.operands.get(source);
    let selected = validation::completed_storage::reference_captures_at_path(
        program,
        &facts.flow.control,
        source,
        projection,
        capture.storage_snapshot,
    )?;
    let selection_kind = match &selected {
        validation::completed_storage::StoredOperand::Unavailable => "unavailable",
        validation::completed_storage::StoredOperand::Incoming(_) => "incoming",
        validation::completed_storage::StoredOperand::Alternatives(_) => "alternatives",
        validation::completed_storage::StoredOperand::Value { .. } => "value",
    };
    let Some(selected) = selected.captures() else {
        if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
            eprintln!(
                "STRUCT descriptor-capture-frontier-unavailable source={source:?} path={projection:?} kind={selection_kind} capture={capture:?}"
            );
            trace_descriptor_storage(program, facts, capture, projection);
        }
        return Some(false);
    };
    // The resolver also represents an unchanged opaque descriptor by itself.
    // That identity edge proves no new association; the existing exact frozen
    // inventory or invocation access must still supply its permission.
    if projection.is_empty()
        && selected.len() == 1
        && selected[0].capture == source
        && selected[0].projection.is_empty()
    {
        return None;
    }
    Some(
        !selected.is_empty()
            && selected.into_iter().all(|selected| {
                if selected.capture == source && selected.projection == projection {
                    return collect_formed_projection(
                        program, facts, state, capture, projection, access, output,
                    );
                }
                collect(
                    program,
                    facts,
                    state,
                    call,
                    selected.capture,
                    &selected.projection,
                    access,
                    retained_by_store,
                    active,
                    output,
                    parents,
                )
            }),
    )
}

fn trace_descriptor_storage(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    capture: &FlowOperandFact,
    projection: &[facts::PlaceSegment],
) {
    let Some(mut place) = place_at(
        program,
        capture.state_symbol,
        capture.statement_index,
        capture.expression,
    ) else {
        return;
    };
    place.segments.extend_from_slice(projection);
    let root_type = validation::canonical_place_type_reference(
        program,
        capture.state_symbol,
        capture.statement_index,
        &validation::CanonicalPlace {
            root: place.root,
            segments: Vec::new(),
        },
    );
    eprintln!("STRUCT descriptor-storage-query place={place:?} root_type={root_type:?}");
    let mut pending = vec![capture.storage_snapshot];
    let mut visited = Vec::new();
    while let Some(snapshot) = pending.pop() {
        if visited.contains(&snapshot) || !facts.flow.control.storage_snapshots.is_valid(snapshot) {
            continue;
        }
        visited.push(snapshot);
        let event = facts.flow.control.storage_snapshots.get(snapshot);
        match event.operation {
            checked_trees::FlowStorageOperation::Store {
                destination,
                value,
                projection,
            } if destination.root == place.root => {
                eprintln!(
                    "STRUCT descriptor-storage-store at={snapshot:?} root={:?} path={:?} value={value:?} value_type={:?} permission={:?} projection={:?}",
                    destination.root,
                    facts
                        .flow
                        .control
                        .operand_referent_segments
                        .span(destination.segments),
                    facts.flow.control.operands.is_valid(value).then(|| facts
                        .flow
                        .control
                        .operands
                        .get(value)
                        .type_reference),
                    facts.flow.control.operands.is_valid(value).then(|| facts
                        .flow
                        .control
                        .operands
                        .get(value)
                        .descriptor_permission),
                    facts.flow.control.operand_retained_paths.span(projection)
                );
            }
            checked_trees::FlowStorageOperation::Writes {
                bindings,
                contents,
                bindings_complete,
                contents_complete,
            } => {
                for (kind, rows, complete) in [
                    ("binding", bindings, bindings_complete),
                    ("contents", contents, contents_complete),
                ] {
                    let writes = facts
                        .flow
                        .control
                        .operand_origin_writes
                        .span_or_empty(rows)
                        .iter()
                        .filter(|write| write.root == place.root)
                        .map(|write| {
                            facts
                                .flow
                                .control
                                .operand_referent_segments
                                .span(write.segments)
                        })
                        .collect::<Vec<_>>();
                    if !writes.is_empty() || !complete {
                        eprintln!(
                            "STRUCT descriptor-storage-writes at={snapshot:?} kind={kind} complete={complete} paths={writes:?}"
                        );
                    }
                }
            }
            checked_trees::FlowStorageOperation::Join { predecessors } => pending
                .extend_from_slice(
                    facts
                        .flow
                        .control
                        .storage_predecessors
                        .span_or_empty(predecessors),
                ),
            _ => {}
        }
        pending.push(event.previous);
    }
}

/// Resolve the installed descriptor generation before joining a loan. A later
/// read has a different expression from its initializer; the completed-storage
/// relation carries that exact earlier formation without rereading its slot.
#[allow(clippy::too_many_arguments)]
fn collect_completed_formations(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: Handle<FlowCallFact>,
    source: Handle<FlowOperandFact>,
    projection: &[facts::PlaceSegment],
    access: &checked_trees::BorrowAccessKind,
    retained_by_store: bool,
    active: &mut Vec<(Handle<FlowOperandFact>, Vec<facts::PlaceSegment>)>,
    output: &mut Vec<PermissionSupplier>,
    parents: &mut Arena<Handle<BorrowLoanFact>>,
) -> Option<bool> {
    let selected = validation::completed_storage::predecessors_in_control(
        program,
        &facts.flow.control,
        source,
        projection,
        validation::completed_storage::StoredObservation::Binding,
        Some((state.state_symbol, state.statements)),
    )?;
    if selected.is_empty() {
        return Some(false);
    }
    // A descriptor transport must end at reference values on every path.
    // A new borrow's owned operand has a different type and is not a prior
    // descriptor: only that borrow's own checked formation can authorize it.
    let descriptors = selected
        .iter()
        .map(|(capture, projection)| {
            facts.flow.control.operands.is_valid(*capture)
                && validation::owned_projection_type_reference(
                    program,
                    facts.flow.control.operands.get(*capture).type_reference,
                    projection,
                )
                .is_some_and(|selected| validation::reference_shell(program, selected).is_some())
        })
        .collect::<Vec<_>>();
    if descriptors.iter().all(|descriptor| !descriptor) {
        return None;
    }
    if descriptors.iter().any(|descriptor| !descriptor) {
        return Some(false);
    }
    let before = output.len();
    for (selected, projection) in selected {
        if !collect(
            program,
            facts,
            state,
            call,
            selected,
            &projection,
            access,
            retained_by_store,
            active,
            output,
            parents,
        ) {
            if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
                eprintln!(
                    "STRUCT descriptor-predecessor-permission-unavailable source={source:?} predecessor={selected:?} path={projection:?}"
                );
            }
            output.truncate(before);
            return Some(false);
        }
    }
    Some(true)
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
    source: Handle<FlowOperandFact>,
    capture: &FlowOperandFact,
    output_access: &checked_trees::BorrowAccessKind,
    retained_by_store: bool,
    output: &mut Vec<PermissionSupplier>,
    parents: &mut Arena<Handle<BorrowLoanFact>>,
) -> InvocationAccessCoverage {
    let unavailable = |reason| {
        if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
            eprintln!(
                "STRUCT captured-call-access-refused source={source:?} call={call:?} expression={:?} reason={reason}",
                capture.expression
            );
        }
        InvocationAccessCoverage::Unavailable
    };
    let call = if retained_by_store {
        match super::super::formation_captures::actual_invocation(program, facts, state, source) {
            Ok(Some(original)) => original,
            Ok(None) => call,
            Err(reason) => return unavailable(reason),
        }
    } else {
        call
    };
    if !facts.flow.control.calls.is_valid(call) {
        return InvocationAccessCoverage::Absent;
    }
    if !capture.expression.is_valid() {
        return InvocationAccessCoverage::Absent;
    }
    match super::super::formation_captures::actual_invocation(program, facts, state, source) {
        Ok(Some(owner)) if owner == call => {}
        Ok(_) => return InvocationAccessCoverage::Absent,
        Err(reason) => return unavailable(reason),
    }
    let Some(presentation) = validation::reference_shell(program, capture.type_reference) else {
        return InvocationAccessCoverage::Absent;
    };
    if checked_trees::BorrowAccessKind::from(presentation.access)
        .direct_reborrow_effect(output_access)
        .is_none()
    {
        return unavailable(
            "the captured descriptor's presented access cannot supply this request",
        );
    }
    let formation =
        match super::super::formation_captures::reference_formation(program, facts, source) {
            Ok(formation) => formation,
            Err(reason) => return unavailable(reason),
        };
    match super::super::formation_captures::argument_formation(program, facts, state, source) {
        Ok(Some(owner)) if owner.storage && owner.value != source => {
            return InvocationAccessCoverage::Absent;
        }
        Ok(_) => {}
        Err(reason) => return unavailable(reason),
    }
    let target_expression = formation
        .as_ref()
        .map(|formation| facts.flow.control.operands.get(formation.target).expression);
    let invocation = facts.flow.control.calls.get(call);
    let accesses = invocation
        .accesses
        .handles()
        .filter(|access| {
            let access = facts.borrow.argument_accesses.get(*access);
            (access.expression == capture.expression
                || Some(access.expression) == target_expression)
                && access.kind.direct_reborrow_effect(output_access).is_some()
                && (retained_by_store || access.end_before_access.is_none())
        })
        .collect::<Vec<_>>();
    if accesses.is_empty() {
        return InvocationAccessCoverage::Absent;
    }
    let before = output.len();
    for access in accesses {
        let row = facts.borrow.argument_accesses.get(access);
        let backings = if let Some(formation) = &formation
            && formation.storage
        {
            match super::super::access_sources::stored_address(
                program,
                facts,
                state,
                formation.target,
                true,
            ) {
                Ok(backings) => Some(backings),
                Err(reason) => {
                    output.truncate(before);
                    return unavailable(reason);
                }
            }
        } else {
            retained_places(
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
            )
        };
        let Some(backings) = backings else {
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
                actual: source,
                backing: backing.place,
            });
        }
    }
    InvocationAccessCoverage::Complete
}

/// Stop at the checked borrow that created this exact reference. Copies and
/// selections create no permission and must follow every saved predecessor.
/// Public backing regions and enclosing storage cannot authorize this event.
#[allow(clippy::too_many_arguments)]
fn collect_formed_projection(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
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
    // The capture already contains the checked contextual type. Re-inferring
    // its syntax separately loses contextual substitutions and selected joins.
    let Some(selected) =
        validation::owned_projection_type_reference(program, capture.type_reference, projection)
            .and_then(|selected| validation::reference_shell(program, selected))
    else {
        return false;
    };
    if !matches!(
        (selected.access, output_access),
        (language_semantics::ReferenceAccess::Mutable, _)
            | (
                language_semantics::ReferenceAccess::Shared,
                checked_trees::BorrowAccessKind::Read
            )
            | (
                language_semantics::ReferenceAccess::WriteOnly,
                checked_trees::BorrowAccessKind::WriteOnly
            )
    ) {
        return false;
    }
    let Some((_, borrow_state)) = facts
        .borrow
        .first_state_borrow(state.machine_symbol, state.state_symbol)
    else {
        return false;
    };
    let before = output.len();
    for (loan, row) in facts.borrow.state_loans(borrow_state) {
        let Some(formed) = row.formation_owner_path else {
            continue;
        };
        // The formation cut can retain lifetime debts alongside descriptor loans.
        // A keepalive row never supplies a physical address or reborrow permission.
        if !matches!(
            row.lineage,
            checked_trees::BorrowLoanLineage::DirectRoot
                | checked_trees::BorrowLoanLineage::Reborrow { .. }
        ) || (row.formation == checked_trees::BorrowLoanFormation::ReturnedDescriptor
            && !is_independent_return(program, facts, state, loan, row, capture))
            || row.kind.direct_reborrow_effect(output_access).is_none()
            || row.formation_expression != capture.expression
            || row.statement_index != capture.statement_index
            || owner_place_segments(facts.borrow.owner_segments.span_or_empty(formed)) != projection
        {
            continue;
        }
        output.push(PermissionSupplier {
            source: CheckedCallBorrowLoanSource::StoredLoan { loan },
            actual: Handle::invalid(),
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
    output.len() > before
}

/// Reading a reference binding uses the loan that binding owns. The descriptor
/// may be stored in the referent, so a storage-owner match on the binding
/// place does not see it. The lexical owner is the association the checker
/// already recorded.
fn collect_binding_observation(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    capture: &FlowOperandFact,
    projection: &[facts::PlaceSegment],
    output_access: &checked_trees::BorrowAccessKind,
    output: &mut Vec<PermissionSupplier>,
) -> bool {
    if !projection.is_empty()
        || !matches!(
            capture.completion,
            checked_trees::FlowOperandCompletion::Observation
        )
    {
        return false;
    }
    let Some(place) = place_at(
        program,
        state.state_symbol,
        capture.statement_index,
        capture.expression,
    ) else {
        return false;
    };
    if !place.segments.is_empty() {
        return false;
    }
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return false;
    };
    let Some((_, borrow_state)) = facts
        .borrow
        .first_state_borrow(state.machine_symbol, state.state_symbol)
    else {
        return false;
    };
    let before = output.len();
    for (loan, row) in facts.borrow.state_loans(borrow_state) {
        if row.owner_symbol != symbol
            || row.statement_index > capture.statement_index
            || (row.last_use_statement_index != 0
                && row.last_use_statement_index < capture.statement_index)
            || matches!(
                row.lineage,
                checked_trees::BorrowLoanLineage::LifetimeDependency
            )
            || row.kind.direct_reborrow_effect(output_access).is_none()
        {
            continue;
        }
        output.push(PermissionSupplier {
            source: CheckedCallBorrowLoanSource::StoredLoan { loan },
            actual: Handle::invalid(),
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
    output.len() > before
}

/// A returned reference with no borrowed input owns its invocation's permission,
/// rather than forwarding an earlier caller loan. Require that exact checked
/// incoming identity and performed call; an empty lifetime roster proves neither.
fn is_independent_return(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    loan: Handle<BorrowLoanFact>,
    row: &BorrowLoanFact,
    capture: &FlowOperandFact,
) -> bool {
    let Some((_, borrow_state)) = facts
        .borrow
        .first_state_borrow(state.machine_symbol, state.state_symbol)
    else {
        return false;
    };
    if row.lineage != checked_trees::BorrowLoanLineage::DirectRoot
        || !facts.borrow.state_owns_loan(borrow_state, loan)
        || !facts
            .borrow
            .incoming_references
            .is_valid(row.incoming_reference)
    {
        return false;
    }
    let identity = facts.borrow.incoming_references.get(row.incoming_reference);
    let checked_trees::IncomingReferenceOrigin::Invocation { expression, target } = identity.origin
    else {
        return false;
    };
    identity.machine_symbol == state.machine_symbol
        && identity.state_symbol == state.state_symbol
        && target.is_valid()
        && row.formation_owner_path.is_some_and(|path| {
            facts
                .borrow
                .owner_segments
                .span_or_empty(identity.owner_path)
                == facts.borrow.owner_segments.span_or_empty(path)
        })
        && expression == capture.expression
        && row.root == facts::PlaceRoot::Expression(expression)
        && state.calls.handles().any(|invocation| {
            let call = facts.flow.control.calls.get(invocation);
            call.authored_expression == expression
                && call.statement_index == capture.statement_index
                && validation::call_target_entry_symbol(program, call.target_symbol)
                    == validation::call_target_entry_symbol(program, target)
        })
}
