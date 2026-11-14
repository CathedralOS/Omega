//! A returned permission keeps the actual input storage its body returned.
//! Publish those existing canonical origin mappings from the shared descriptor
//! analysis; this derives a contract, never establishes its loan or authority.
//! The checker must independently reconstruct formation and completion.

use crate::{Unserved, refuse};
use semantic_vocabulary::MachineId;
use terminal_check_tables::{Checked, Construct};
use terminal_psi::TerminalModule;
use terminal_semantics::descriptor_results::{
    DescriptorAnalysis, terminal_borrowed_storage_graph_from,
};

pub(crate) fn publish(
    elaboration: &mut crate::Elaboration,
    selections: &[crate::Selected],
) -> Checked<()> {
    publish_results(&mut elaboration.module, selections)?;
    publish_parents(elaboration)
}

fn publish_results(module: &mut TerminalModule, selections: &[crate::Selected]) -> Checked<()> {
    let selected: Vec<_> = module
        .machines
        .iter()
        .filter_map(|machine| {
            machine
                .result
                .structural()
                .filter(|result| {
                    terminal_psi::structural_type_contains_reference_by(
                        |id| module.structural_type(id),
                        result.structural_type,
                    )
                })
                .map(|_| machine.id)
        })
        .collect();
    let Some(first) = selected.first().copied() else {
        return Ok(());
    };
    let unavailable =
        |machine: MachineId, reason| refuse(machine, Construct::Machine, Unserved::Call(reason));
    // A selected body can return a narrower permission than its public
    // requirement promises. Until calls retain that public result contract,
    // its body origins cannot authorize a reference escaping the call. Join
    // the frontier already prepared above without rescanning the machines
    // or their result types for each selected call.
    let mut selected_calls: Vec<_> = selections.iter().collect();
    selected_calls.sort_unstable_by_key(|selection| selection.implementation);
    for machine in &selected {
        if let Ok(position) =
            selected_calls.binary_search_by_key(machine, |selection| selection.implementation)
        {
            return Err(unavailable(
                selected_calls[position].caller,
                "selected reference result requires its public requirement contract",
            ));
        }
    }
    let graph = terminal_borrowed_storage_graph_from(module, selected.iter().copied(), &[])
        .map_err(|reason| unavailable(first, reason))?;
    let mut analysis = DescriptorAnalysis::with_reference_slots(&module.structural_types, &graph);
    let mut published = Vec::with_capacity(selected.len());
    for id in selected {
        let machine = module
            .machine(id)
            .ok_or_else(|| unavailable(id, "reference result lost its actual machine"))?;
        let result = machine
            .result
            .structural()
            .ok_or_else(|| unavailable(id, "reference result lost its structural declaration"))?;
        let slots = analysis.result_slots(id).ok_or_else(|| {
            unavailable(
                id,
                "reference result has no complete actual return-source correspondence",
            )
        })?;
        let mappings = terminal_semantics::reference_result_sources(
            &module.structural_types,
            &machine.structural_parameters,
            result,
            &slots,
        )
        .map_err(|reason| unavailable(id, reason))?;
        published.push((id, mappings));
    }
    // Commit only after every actual reference result is complete.
    for (id, mappings) in published {
        let machine = module
            .machines
            .iter_mut()
            .find(|machine| machine.id == id)
            .ok_or_else(|| unavailable(id, "reference result producer disappeared"))?;
        let terminal_psi::TerminalMachineResult::Structural(result) = &mut machine.contract.result
        else {
            return Err(unavailable(
                id,
                "reference result changed before publication",
            ));
        };
        result.reference_sources = mappings;
    }
    Ok(())
}

/// Complete the existing lender column after all descriptor transports are
/// available. These rows describe actual source occurrences; 06 reconstructs
/// them before deciding any loan and grants no authority from this publication.
fn publish_parents(elaboration: &mut crate::Elaboration) -> Checked<()> {
    use terminal_check_tables::ProgramPoint;
    use terminal_check_tables::obligations::{Lender, ObligationKind};
    use terminal_semantics::descriptor_lifetimes::{
        DescriptorLocation, descriptor_location, descriptor_location_lifetime,
    };
    let module = &elaboration.module;
    let unavailable = |machine, reason| refuse(machine, Construct::Machine, Unserved::Call(reason));
    let mut updates = Vec::new();
    for machine in &module.machines {
        let tables = elaboration.tables.machines.get(machine.id).ok_or_else(|| {
            unavailable(
                machine.id,
                "reference parent publication lost its machine tables",
            )
        })?;
        let demanded = tables
            .obligations
            .iter()
            .any(|(_, row)| matches!(row.kind, ObligationKind::Loan { .. }));
        if !demanded {
            continue;
        }
        let mut children = Vec::new();
        for (id, row) in tables.obligations.iter() {
            if let ObligationKind::Loan { captured, .. } = row.kind {
                let (root, segments) = captured_path(tables, captured)
                    .map_err(|reason| unavailable(machine.id, reason))?;
                if let Some(crossing) = segments
                    .iter()
                    .rposition(|step| *step == terminal_check_tables::bindings::Segment::Referent)
                {
                    let parent = descriptor_name(root, &segments[..crossing]).ok_or_else(|| {
                        unavailable(
                            machine.id,
                            "reference parent has no exact static descriptor path",
                        )
                    })?;
                    children.push((id, row.created_at, parent));
                }
            }
        }
        if children.is_empty() {
            continue;
        }
        let graph = terminal_borrowed_storage_graph_from(module, [machine.id], &[])
            .map_err(|reason| unavailable(machine.id, reason))?;
        let mut analysis =
            DescriptorAnalysis::with_reference_slots(&module.structural_types, &graph);
        let flow = analysis.flow(machine.id).ok_or_else(|| {
            unavailable(
                machine.id,
                "reference parent has no complete descriptor flow",
            )
        })?;
        let mut parents = Vec::new();
        for block in &machine.blocks {
            let mut before = ProgramPoint::BlockEntry(block.id);
            for (position, operation) in block.operations.iter().enumerate() {
                if let terminal_psi::OperationKind::EstablishReference { source, .. } =
                    &operation.kind
                {
                    let result = operation.result.as_structural().ok_or_else(|| {
                        unavailable(
                            machine.id,
                            "reference formation has no actual descriptor result",
                        )
                    })?;
                    let ty = machine.structural_place_type(source.place).ok_or_else(|| {
                        unavailable(machine.id, "reference formation has no source storage type")
                    })?;
                    let source_location = descriptor_location(
                        &module.structural_types,
                        source.place,
                        ty,
                        &source.path,
                        None,
                    )
                    .ok_or_else(|| {
                        unavailable(machine.id, "reference formation has no exact source place")
                    })?;
                    let mut matching = Vec::new();
                    for (id, row) in tables.obligations.iter() {
                        let ObligationKind::Loan {
                            captured, access, ..
                        } = row.kind
                        else {
                            continue;
                        };
                        if row.created_at != before
                            || access
                                != reference_access(source.access).ok_or_else(|| {
                                    unavailable(
                                        machine.id,
                                        "reference formation has no borrowed access",
                                    )
                                })?
                        {
                            continue;
                        }
                        let (root, path) = captured_path(tables, captured)
                            .map_err(|reason| unavailable(machine.id, reason))?;
                        if descriptor_name(root, &path).as_ref() == Some(&source_location) {
                            matching.push(id);
                        }
                    }
                    let [identity] = matching.as_slice() else {
                        return Err(unavailable(
                            machine.id,
                            "reference formation has no unique exact Loan row",
                        ));
                    };
                    let position = u32::try_from(position).map_err(|_| {
                        unavailable(machine.id, "reference formation position exceeds its graph")
                    })?;
                    let at = (block.id, position);
                    let lifetime = descriptor_location_lifetime(
                        &flow,
                        at,
                        &DescriptorLocation::whole(result.place),
                    );
                    if lifetime.live.contains(&at) {
                        return Err(unavailable(
                            machine.id,
                            "reference formation overlaps another activation of itself",
                        ));
                    }
                    parents.push((*identity, lifetime));
                }
                let retained = terminal_semantics::reference_call_lifetimes(
                    module,
                    machine,
                    operation,
                    &mut analysis,
                )
                .map_err(|reason| unavailable(machine.id, reason))?;
                if !retained.is_empty() {
                    let call = operation.kind.declared_invocation().ok_or_else(|| {
                        unavailable(machine.id, "retained reference lost its actual call")
                    })?;
                    let mut claimed = Vec::new();
                    let mut retained = retained.into_iter().peekable();
                    for (position, source) in call.structural_arguments.iter().enumerate() {
                        let Some(access) = reference_access(source.access) else {
                            continue;
                        };
                        let ty = machine.structural_place_type(source.place).ok_or_else(|| {
                            unavailable(machine.id, "retained call loan has no source type")
                        })?;
                        let location = descriptor_location(
                            &module.structural_types,
                            source.place,
                            ty,
                            &source.path,
                            None,
                        )
                        .ok_or_else(|| {
                            unavailable(machine.id, "retained call loan has no exact source")
                        })?;
                        let mut found = None;
                        for (id, row) in tables.obligations.iter() {
                            let ObligationKind::Loan {
                                captured,
                                access: actual,
                                ..
                            } = row.kind
                            else {
                                continue;
                            };
                            if row.created_at != before || actual != access || claimed.contains(&id)
                            {
                                continue;
                            }
                            let (root, path) = captured_path(tables, captured)
                                .map_err(|reason| unavailable(machine.id, reason))?;
                            if descriptor_name(root, &path).as_ref() == Some(&location) {
                                found = Some(id);
                                break;
                            }
                        }
                        let id = found.ok_or_else(|| {
                            unavailable(
                                machine.id,
                                "retained call loan has no exact occurrence row",
                            )
                        })?;
                        claimed.push(id);
                        if retained.peek().is_some_and(|(input, _)| *input == position) {
                            let (_, lifetime) = retained.next().ok_or_else(|| {
                                unavailable(machine.id, "retained call loan lifetime disappeared")
                            })?;
                            parents.push((id, lifetime));
                        }
                    }
                    if retained.next().is_some() {
                        return Err(unavailable(
                            machine.id,
                            "returned reference lifetime has no borrowed call occurrence",
                        ));
                    }
                }
                before = ProgramPoint::AfterOperation(operation.id);
            }
        }
        for (child, point, descriptor) in children {
            let at = formation_position(machine, point).ok_or_else(|| {
                unavailable(
                    machine.id,
                    "reference reborrow has no exact formation point",
                )
            })?;
            let parent = terminal_semantics::reference_loan_parent(
                module,
                machine,
                &mut analysis,
                at,
                &descriptor,
                parents
                    .iter()
                    .filter(|(id, _)| *id != child)
                    .map(|(id, life)| (*id, life)),
            )
            .map_err(|reason| unavailable(machine.id, reason))?;
            updates.push((
                machine.id,
                child,
                parent.map_or(Lender::Incoming, Lender::Loan),
            ));
        }
    }
    for (machine, child, lender) in updates {
        let row = elaboration
            .tables
            .machines
            .get_mut(machine)
            .and_then(|tables| tables.obligations.get_mut(child))
            .ok_or_else(|| {
                unavailable(
                    machine,
                    "reference parent publication lost its exact Loan row",
                )
            })?;
        let ObligationKind::Loan { lender: slot, .. } = &mut row.kind else {
            return Err(unavailable(
                machine,
                "reference parent publication changed obligation kind",
            ));
        };
        *slot = lender;
    }
    Ok(())
}

fn reference_access(
    access: terminal_psi::StructuralAccess,
) -> Option<language_core::ReferenceAccess> {
    Some(match access {
        terminal_psi::StructuralAccess::SharedBorrow => language_core::ReferenceAccess::Shared,
        terminal_psi::StructuralAccess::MutableBorrow => language_core::ReferenceAccess::Mutable,
        terminal_psi::StructuralAccess::WriteOnlyBorrow => {
            language_core::ReferenceAccess::WriteOnly
        }
        terminal_psi::StructuralAccess::Owned => return None,
    })
}

fn captured_path(
    tables: &terminal_check_tables::MachineTables,
    mut place: terminal_check_tables::ExactPlaceId,
) -> Result<
    (
        semantic_vocabulary::PlaceId,
        Vec<terminal_check_tables::bindings::Segment>,
    ),
    &'static str,
> {
    use terminal_check_tables::bindings::PlaceOrigin;
    let mut path = Vec::new();
    for _ in 0..tables.places.len() {
        let row = tables
            .places
            .get(place)
            .ok_or("reference capture has no live place row")?;
        match row.origin {
            PlaceOrigin::Root(root) => {
                path.reverse();
                return Ok((root, path));
            }
            PlaceOrigin::Projection { parent, segment } => {
                path.push(segment);
                place = parent;
            }
        }
    }
    Err("reference capture does not reach its exact storage root")
}

fn descriptor_name(
    root: semantic_vocabulary::PlaceId,
    path: &[terminal_check_tables::bindings::Segment],
) -> Option<terminal_semantics::descriptor_lifetimes::DescriptorLocation> {
    use terminal_check_tables::bindings::Segment;
    use terminal_semantics::descriptor_lifetimes::{DescriptorLocation, DescriptorPathSegment};
    Some(DescriptorLocation {
        root,
        path: path
            .iter()
            .map(|step| {
                Some(match step {
                    Segment::Field(field) => DescriptorPathSegment::Field(*field),
                    Segment::Case(case) => DescriptorPathSegment::Case(*case),
                    Segment::FixedIndex(index) => DescriptorPathSegment::FixedIndex(*index),
                    Segment::Referent => DescriptorPathSegment::Referent,
                    Segment::RuntimeIndex { .. } | Segment::Extent => return None,
                })
            })
            .collect::<Option<Vec<_>>>()?,
    })
}

fn formation_position(
    machine: &terminal_psi::TerminalMachine,
    point: terminal_check_tables::ProgramPoint,
) -> Option<terminal_semantics::descriptor_lifetimes::DescriptorPosition> {
    match point {
        terminal_check_tables::ProgramPoint::BlockEntry(block) => Some((block, 0)),
        terminal_check_tables::ProgramPoint::AfterOperation(operation) => {
            machine.blocks.iter().find_map(|block| {
                block
                    .operations
                    .iter()
                    .position(|row| row.id == operation)
                    .and_then(|position| u32::try_from(position + 1).ok())
                    .map(|position| (block.id, position))
            })
        }
        terminal_check_tables::ProgramPoint::Edge(_) => None,
    }
}
