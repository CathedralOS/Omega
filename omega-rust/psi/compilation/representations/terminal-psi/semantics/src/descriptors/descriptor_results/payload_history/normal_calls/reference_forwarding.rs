//! Prove that a chosen helper forwards only its captured input references.
//!
//! Inspect every possible normal exit, including outputs the body leaves
//! untouched. The requested relation is disabled during this inspection so it
//! cannot justify itself. Pointer-value equality supplies no new permission,
//! lifetime, membership, or single write address.

use super::*;
use crate::descriptors::descriptor_lifetimes::descriptor_location;
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryReferenceDestination,
    BoundaryReferenceForwardingDemand, BoundaryReferenceSource, StructuralPathQualification,
    TerminalMachineResult,
};

mod entry_requirements;
#[cfg(test)]
mod tests;

struct ForwardingMemberships<'input> {
    history: PayloadMembershipHistory<'input>,
    normal: NormalPayloadContext<'input>,
}

#[cfg(test)]
fn require(
    module: &TerminalModule,
    boundary: semantic_vocabulary::BoundaryMachineId,
    implementation: Option<MachineId>,
) -> Result<(), &'static str> {
    super::boundary_preservation::BoundaryLenderPreservation::new(module)
        .require_forwarding(boundary, implementation)
}

pub(super) fn require_selected(
    module: &TerminalModule,
    boundary: semantic_vocabulary::BoundaryMachineId,
    declaration: &BoundaryMachineDeclaration,
    body: &TerminalMachine,
    requests: &[super::boundary_preservation::ForwardingRequest<'_>],
    call_contracts: Arc<super::call_contracts::CallContracts<'_>>,
) -> Result<(), &'static str> {
    let implementation = body.id;
    require_signature(declaration, body)?;
    entry_requirements::require(module, declaration, body)?;
    // The selected mapping authenticates graph coordinates, not the choice of
    // implementation. Selection admission remains with this entry's caller.
    let graph = super::super::super::terminal_graph_with_forwarding(
        module,
        DescriptorSlotKind::BorrowedStorage,
        &[(boundary, implementation)],
        &[],
    )
    .ok_or("reference forwarding has no complete independent value graph")?;
    // The caller owes the public entry duties even when the selected helper
    // assumes less. Replay those duties through the actual body; comparing
    // only its weaker private declarations would hide a broken promise.
    let mut parameters = body.structural_parameters.clone();
    let mut contracts = body
        .structural_places
        .iter()
        .filter_map(|place| terminal_payload_contract(body, place.id).map(|row| (place.id, row)))
        .collect::<Vec<_>>();
    for (index, parameter) in parameters.iter_mut().enumerate() {
        let public = public_entry_contract(declaration, index)?;
        let private = terminal_payload_contract(body, parameter.place)
            .ok_or("reference forwarding input has no private entry contract")?;
        if !covers_roster(&public, &private) {
            return Err("reference forwarding source assumes duties absent from its public input");
        }
        parameter.qualifications = public.qualifications.clone();
        parameter.projected_qualifications = public.projected_qualifications.clone();
        let entry = contracts
            .iter_mut()
            .find(|(place, _)| *place == parameter.place)
            .ok_or("reference forwarding input has no declared payload root")?;
        entry.1 = public;
    }
    let mut history = PayloadMembershipHistory::for_current_contracts(
        &module.structural_types,
        &module.structural_domains,
        &graph,
        body.id,
        &parameters,
        &contracts,
    )?;
    history.replay.memberships = PayloadMembershipKind::Preserved;
    let mut memberships = ForwardingMemberships {
        history,
        normal: NormalPayloadContext::with_call_contracts(&graph, call_contracts),
    };
    memberships.normal.executable_body(implementation)?;
    let selected = descriptor_machine(&graph, implementation)
        .ok_or("reference forwarding has no execution graph")?;
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
    let feasible = analysis.flow_feasibility(implementation);
    let mut exits = Vec::new();
    for position in &selected.normal_exits {
        match feasible.get(position) {
            Some(FlowFeasibility::ProvedImpossible) => {}
            Some(FlowFeasibility::Possible) => exits.push(*position),
            _ => return Err("reference forwarding has an unknown normal exit"),
        }
    }
    if exits.is_empty() {
        return Err("reference forwarding has no demonstrated normal return");
    }
    for request in requests {
        let demand = request.demand;
        require_output(
            module,
            declaration,
            body,
            &mut analysis,
            &mut memberships,
            &exits,
            demand,
        )?;
    }
    Ok(())
}

fn require_signature(
    declaration: &BoundaryMachineDeclaration,
    body: &TerminalMachine,
) -> Result<(), &'static str> {
    let result_matches = match (&declaration.result, &body.result) {
        (BoundaryMachineResult::Unit, TerminalMachineResult::Unit) => true,
        (BoundaryMachineResult::Scalar(expected), TerminalMachineResult::Scalar(actual)) => {
            expected.scalar_type == actual.scalar_type
        }
        (
            BoundaryMachineResult::Structural(expected),
            TerminalMachineResult::Structural(actual),
        ) => {
            expected.structural_type == actual.structural_type
                && expected.multiplicity == actual.multiplicity
        }
        _ => false,
    };
    if declaration.role != terminal_psi::BoundaryDeclarationRole::Callable
        || declaration.completion != terminal_psi::BoundaryCompletion::NormalReturn
        || declaration.parameter_order != body.parameter_order
        || !body
            .parameters
            .iter()
            .map(|parameter| parameter.scalar_type)
            .eq(declaration.scalar_parameters.iter().copied())
        || declaration.structural_parameters.len() != body.structural_parameters.len()
        || declaration
            .structural_parameters
            .iter()
            .zip(&body.structural_parameters)
            .any(|(required, actual)| {
                required.structural_type != actual.structural_type
                    || required.access != actual.access
                    || required.multiplicity != actual.multiplicity
            })
        || !result_matches
    {
        return Err("reference forwarding execution signature differs from its public requirement");
    }
    Ok(())
}

fn require_output(
    module: &TerminalModule,
    declaration: &BoundaryMachineDeclaration,
    body: &TerminalMachine,
    analysis: &mut DescriptorAnalysis<'_>,
    memberships: &mut ForwardingMemberships<'_>,
    exits: &[DescriptorPosition],
    demand: &BoundaryReferenceForwardingDemand,
) -> Result<(), &'static str> {
    let output = declaration
        .reference_outputs
        .get(demand.output as usize)
        .ok_or("reference forwarding names an absent output")?;
    let BoundaryReferenceDestination::Parameter(index) = output.destination;
    let destination = body
        .structural_parameters
        .get(index as usize)
        .ok_or("reference forwarding output has no execution formal")?;
    if output.path.iter().any(|step| {
        !matches!(
            step,
            StructuralPathSegment::Field(_)
                | StructuralPathSegment::Case(_)
                | StructuralPathSegment::FixedIndex(_)
        )
    }) {
        return Err("reference forwarding output is not an inline stored slot");
    }
    let output_type = crate::runtime_structural_path_tip(
        module.structural_types.iter(),
        destination.structural_type,
        &output.path,
    )
    .ok_or("reference forwarding output has no typed leaf")?;
    let StructuralTypeShape::Reference { referent, access } = output_type.shape else {
        return Err("reference forwarding output is not a reference value");
    };
    if access != output.access {
        return Err("reference forwarding output access differs from its declared value");
    }
    let output_slot = descriptor_location(
        &module.structural_types,
        destination.place,
        destination.structural_type,
        &output.path,
        None,
    )
    .ok_or("reference forwarding output has no exact stored slot")?;
    let writable = DescriptorSlotKind::BorrowedStorage
        .writable_slots(
            &module.structural_types,
            destination.place,
            destination.structural_type,
            destination.access,
        )
        .ok_or("reference forwarding output has an incomplete writable roster")?;
    if !writable.contains(&output_slot) {
        return Err("reference forwarding output is not writable through its formal");
    }
    if demand.sources.is_empty() || demand.sources.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err("reference forwarding needs a nonempty canonical captured choice set");
    }
    let mut allowed = BTreeMap::new();
    for source in &demand.sources {
        let (index, path) = match source {
            BoundaryReferenceSource::Reference { argument_index } => (*argument_index, None),
            BoundaryReferenceSource::Stored {
                argument_index,
                path,
            } => (*argument_index, Some(path.as_slice())),
        };
        let parameter = body
            .structural_parameters
            .get(index as usize)
            .ok_or("reference forwarding source has no execution formal")?;
        let entry = memberships.history.current_payload_contract_in(
            analysis,
            (body.entry, 0),
            parameter.place,
        )?;
        let (origins, stored_path) = match path {
            None => {
                if parameter.access == StructuralAccess::Owned
                    || parameter.structural_type != referent
                    || !access.admits_referent(parameter.access)
                {
                    return Err(
                        "reference forwarding outer source has incompatible carrier or access",
                    );
                }
                (
                    vec![DescriptorSourceOrigin::Backing(
                        DescriptorCallOrigin::Argument {
                            source: parameter.place,
                            projection: Projection::new(&[]),
                        },
                    )],
                    None,
                )
            }
            Some(path) => {
                require_stored_source(module, parameter, path, referent, access)?;
                let source = descriptor_location(
                    &module.structural_types,
                    parameter.place,
                    parameter.structural_type,
                    path,
                    None,
                )
                .ok_or("reference forwarding source has no typed stored slot")?;
                (
                    analysis
                        .symbolic_entry_source_origins(body.id, &source)
                        .ok_or("reference forwarding source has no captured entry value")?,
                    Some(source.path),
                )
            }
        };
        if origins.is_empty() {
            return Err("reference forwarding source has no captured entry alternative");
        }
        let mut contract = captured_roster(module, &entry, stored_path.as_deref(), output_type.id)?;
        let private = terminal_payload_contract(body, parameter.place)
            .ok_or("reference forwarding source has no private entry roster")?;
        let private = captured_roster(module, &private, stored_path.as_deref(), output_type.id)?;
        let public = public_entry_contract(declaration, index as usize)?;
        let public = captured_roster(module, &public, stored_path.as_deref(), output_type.id)?;
        // A helper's private assumptions must not create hidden caller duties.
        // Weaker entry assumptions are fine; every duty used in this proof
        // must already be present on the public input at the same source path.
        if !covers_roster(&public, &private) {
            return Err("reference forwarding source assumes duties absent from its public input");
        }
        canonicalize_roster(&mut contract);
        for origin in origins {
            let origin = unchanged_origin(origin)?;
            if let Some(previous) = allowed.insert(origin, contract.clone())
                && !same_roster(&previous, &contract)
            {
                return Err("reference forwarding source has inconsistent captured duty rosters");
            }
        }
    }
    let mut forwarded_exits = Vec::with_capacity(exits.len());
    for position in exits {
        let origins = analysis
            .symbolic_source_origins(body.id, *position, &output_slot)
            .ok_or("reference forwarding output has unavailable normal-return contents")?;
        if origins.is_empty() {
            return Err("reference forwarding output has no normal-return alternative");
        }
        let mut forwarded = Vec::with_capacity(origins.len());
        for origin in origins {
            let origin = unchanged_origin(origin)?;
            if !allowed.contains_key(&origin) {
                return Err("a normal output reference is not one of the captured sources");
            }
            forwarded.push(origin);
        }
        forwarded_exits.push((*position, forwarded));
    }
    // Returning the same address must not add or lose the value's duties.
    // Reconstruct each actual exit independently; the forwarding request is
    // disabled in this graph and supplies neither a roster nor its own proof.
    let normal_exits = memberships.history.preserved_normal_exit_rosters_in(
        analysis,
        &[destination.place],
        Some(&mut memberships.normal),
    )?;
    if normal_exits.len() != forwarded_exits.len() {
        return Err("reference forwarding has an incomplete normal-exit roster");
    }
    for exit in normal_exits {
        let (_, origins) = forwarded_exits
            .iter()
            .find(|(position, _)| *position == exit.position)
            .ok_or("reference forwarding has a different preserved normal exit")?;
        let (_, contract) = exit
            .contracts
            .into_iter()
            .find(|(root, _)| *root == destination.place)
            .ok_or("reference forwarding output has no complete normal duty roster")?;
        let mut contract = select_payload_memberships_with_kind(
            &module.structural_types,
            &module.structural_domains,
            &contract,
            &output_slot.path,
            PayloadMembershipKind::Preserved,
        )
        .ok_or("reference forwarding output has no complete stored duty roster")?;
        canonicalize_roster(&mut contract);
        for origin in origins {
            let source = allowed
                .get(origin)
                .ok_or("reference forwarding lost its captured source roster")?;
            if !same_roster(source, &contract) {
                return Err("a normal output reference changes its captured source duties");
            }
        }
    }
    Ok(())
}

fn public_entry_contract(
    declaration: &BoundaryMachineDeclaration,
    index: usize,
) -> Result<PayloadMembershipContract, &'static str> {
    let public = declaration
        .structural_parameters
        .get(index)
        .ok_or("reference forwarding source has no public formal")?;
    let mut contract = PayloadMembershipContract {
        carrier: public.structural_type,
        qualifications: public.qualifications.clone(),
        projected_qualifications: public.projected_qualifications.clone(),
        scalar_observations: Vec::new(),
        scalar_requirements: Vec::new(),
    };
    for requirement in declaration
        .requires
        .iter()
        .filter(|requirement| requirement.argument_index as usize == index)
    {
        if requirement.path.is_empty() {
            contract.qualifications.push(requirement.domain);
        } else {
            contract
                .projected_qualifications
                .push(StructuralPathQualification {
                    path: requirement.path.clone(),
                    domain: requirement.domain,
                });
        }
    }
    canonicalize_roster(&mut contract);
    Ok(contract)
}

pub(super) fn captured_roster(
    module: &TerminalModule,
    entry: &PayloadMembershipContract,
    stored_path: Option<&[DescriptorPathSegment]>,
    output: StructuralTypeId,
) -> Result<PayloadMembershipContract, &'static str> {
    if let Some(path) = stored_path {
        return select_payload_memberships_with_kind(
            &module.structural_types,
            &module.structural_domains,
            entry,
            path,
            PayloadMembershipKind::Preserved,
        )
        .and_then(|contract| {
            contract.presented(&module.structural_types, &module.structural_domains, output)
        })
        .ok_or("reference forwarding stored source has no complete entry roster");
    }
    let mut projected_qualifications = Vec::new();
    StructuralPathQualification::append_under(
        &entry.qualifications,
        &entry.projected_qualifications,
        &[StructuralPathSegment::Referent],
        &mut projected_qualifications,
    );
    Ok(PayloadMembershipContract {
        carrier: output,
        qualifications: Vec::new(),
        projected_qualifications,
        scalar_observations: Vec::new(),
        scalar_requirements: Vec::new(),
    })
}

pub(super) fn covers_roster(
    available: &PayloadMembershipContract,
    required: &PayloadMembershipContract,
) -> bool {
    available.carrier == required.carrier
        && required
            .qualifications
            .iter()
            .all(|domain| available.qualifications.contains(domain))
        && required
            .projected_qualifications
            .iter()
            .all(|row| available.projected_qualifications.contains(row))
}

pub(super) fn canonicalize_roster(contract: &mut PayloadMembershipContract) {
    contract.qualifications.sort();
    contract.qualifications.dedup();
    contract.projected_qualifications.sort();
    contract.projected_qualifications.dedup();
}

pub(super) fn same_roster(
    left: &PayloadMembershipContract,
    right: &PayloadMembershipContract,
) -> bool {
    left.carrier == right.carrier
        && left.qualifications == right.qualifications
        && left.projected_qualifications == right.projected_qualifications
}

fn require_stored_source(
    module: &TerminalModule,
    parameter: &terminal_psi::StructuralParameterDeclaration,
    path: &[StructuralPathSegment],
    expected_referent: StructuralTypeId,
    output_access: StructuralAccess,
) -> Result<(), &'static str> {
    let unavailable = "reference forwarding stored source has incompatible carrier or access";
    if parameter.access == StructuralAccess::WriteOnlyBorrow {
        return Err(unavailable);
    }
    let mut shared = parameter.access == StructuralAccess::SharedBorrow;
    for (position, step) in path.iter().enumerate() {
        match step {
            StructuralPathSegment::Field(_)
            | StructuralPathSegment::Case(_)
            | StructuralPathSegment::FixedIndex(_) => {}
            StructuralPathSegment::Referent => {
                let parent = crate::runtime_structural_path_tip(
                    module.structural_types.iter(),
                    parameter.structural_type,
                    &path[..position],
                )
                .ok_or(unavailable)?;
                match parent.shape {
                    StructuralTypeShape::Reference {
                        access: StructuralAccess::MutableBorrow,
                        ..
                    } => {}
                    StructuralTypeShape::Reference {
                        access: StructuralAccess::SharedBorrow,
                        ..
                    } => shared = true,
                    _ => return Err(unavailable),
                }
            }
            _ => return Err(unavailable),
        }
    }
    let leaf = crate::runtime_structural_path_tip(
        module.structural_types.iter(),
        parameter.structural_type,
        path,
    )
    .ok_or(unavailable)?;
    let StructuralTypeShape::Reference {
        referent,
        mut access,
    } = leaf.shape
    else {
        return Err(unavailable);
    };
    if shared {
        if !StructuralAccess::SharedBorrow.admits_referent(access) {
            return Err(unavailable);
        }
        access = StructuralAccess::SharedBorrow;
    }
    if referent != expected_referent || !output_access.admits_referent(access) {
        return Err(unavailable);
    }
    Ok(())
}

fn unchanged_origin(
    origin: DescriptorSourceOrigin,
) -> Result<DescriptorSourceOrigin, &'static str> {
    if let Some(slot) = origin.captured_entry_slot() {
        return Ok(DescriptorSourceOrigin::IncomingSlot(slot.into_owned()));
    }
    match origin {
        DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument { source, projection })
            if projection == Projection::new(&[]) =>
        {
            Ok(DescriptorSourceOrigin::Backing(
                DescriptorCallOrigin::Argument { source, projection },
            ))
        }
        _ => Err(
            "reference forwarding needs an unchanged input value, not fresh or projected backing",
        ),
    }
}
