use super::*;
use terminal_psi::{OperationKind as O, TerminalModule, Terminator};

/// A checked provider body chosen for one exact boundary application. This
/// association describes call flow; it grants no provider or package authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CallableBoundaryImplementation {
    pub boundary: BoundaryMachineId,
    pub machine: MachineId,
}

/// Follow selected providers through the same actuals and returned code values
/// as ordinary calls. Making their bodies independent entry roots would lose
/// that substitution and could activate code the caller never invokes.
pub fn terminal_callable_values_with_boundary_implementations<'a>(
    module: &'a TerminalModule,
    implementations: &[CallableBoundaryImplementation],
) -> CallableValueResult<CallableValueProgram<'a>> {
    let mut bindings = implementations.to_vec();
    bindings.sort_unstable_by_key(|binding| binding.boundary);
    if bindings
        .windows(2)
        .any(|pair| pair[0].boundary == pair[1].boundary)
    {
        return Err("callable boundary application has competing selected bodies");
    }
    for binding in &bindings {
        if module.boundary_machine(binding.boundary).is_none()
            || module.machine(binding.machine).is_none()
        {
            return Err("callable boundary implementation has an absent requirement or body");
        }
    }
    let mut program = terminal_callable_values(module)?;
    for body in program.bodies.values_mut() {
        for invocation in body.invocations.values_mut() {
            let Some(boundary) = invocation.boundary else {
                continue;
            };
            let Ok(index) = bindings.binary_search_by_key(&boundary, |binding| binding.boundary)
            else {
                continue;
            };
            if invocation.target != CallableInvocationTarget::Opaque {
                return Err("callable boundary implementation replaces a non-boundary target");
            }
            invocation.target = CallableInvocationTarget::Machine(bindings[index].machine);
        }
    }
    Ok(program)
}

/// Find actual boundary invocations from an entry, including selected code,
/// terminal edges and cleanup calls. Retaining a boundary header or descriptor
/// alone does not make it a demand for executable supply.
pub fn reached_boundary_calls(
    module: &TerminalModule,
    entry: MachineId,
    implementations: &[CallableBoundaryImplementation],
) -> CallableValueResult<Vec<BoundaryMachineId>> {
    let program = terminal_callable_values_with_boundary_implementations(module, implementations)?;
    let reached = reachable_callable_invocations(&program, [entry])?;
    let mut boundaries = Vec::new();
    for row in reached.invocations.values() {
        if let Some(boundary) = row.invocation.boundary {
            module
                .boundary_machine(boundary)
                .ok_or("reached invocation has no exact boundary declaration")?;
            boundaries.push(boundary);
        }
        match &row.target {
            ReachedCallableTarget::Callable(values) => {
                for ceiling in &values.unresolved_ceilings {
                    let contract = module
                        .callables
                        .contract(*ceiling)
                        .ok_or("reached callable has no exact unresolved contract")?;
                    if !contract.envelope.invocations.is_empty() {
                        return Err(
                            "unresolved callable arrivals need their complete boundary demand representation",
                        );
                    }
                }
            }
            ReachedCallableTarget::Opaque if row.invocation.boundary.is_none() => {
                return Err("reached opaque invocation has no boundary or callable contract");
            }
            _ => {}
        }
    }
    boundaries.sort_unstable();
    boundaries.dedup();
    Ok(boundaries)
}

/// Project current executable Terminal operations into the shared value query.
pub fn terminal_callable_values(
    module: &TerminalModule,
) -> CallableValueResult<CallableValueProgram<'_>> {
    let graph = crate::descriptor_results::terminal_reference_graph_from(
        module,
        module.machines.iter().map(|machine| machine.id),
    )
    .map_err(|_| "callable provenance cannot reconstruct current Terminal storage flow")?;
    let mut bodies = BTreeMap::new();
    let mut cleanups = Vec::new();
    for machine in &module.machines {
        let flow = graph
            .iter()
            .find(|row| row.id == machine.id)
            .ok_or("callable provenance machine lost its current Terminal storage flow")?;
        let mut body = CallableValueBody {
            attachment: machine.attachment,
            nominal_cleanup_subject: machine.nominal_cleanup_subject,
            unit_result: matches!(machine.result, terminal_psi::TerminalMachineResult::Unit),
            structural_formals: machine.structural_parameters.clone(),
            scalar_result: machine.result.scalar().map(|result| result.scalar_type),
            structural_result: machine
                .result
                .structural()
                .map(|result| result.structural_type),
            structural_parameters: machine
                .structural_parameters
                .iter()
                .map(|parameter| (parameter.structural_type, parameter.access))
                .collect(),
            ..Default::default()
        };
        for (position, parameter) in machine.parameters.iter().enumerate() {
            body.scalar_parameters
                .push((parameter.id, parameter.scalar_type));
            body.insert_value(
                parameter.id,
                parameter.scalar_type,
                CallableDefinition::Formal(position),
                false,
            )?;
        }
        for block in &machine.blocks {
            for parameter in &block.parameters {
                body.insert_value(
                    parameter.id,
                    parameter.scalar_type,
                    CallableDefinition::Incoming(block.id),
                    block.id == machine.entry,
                )?;
            }
            for (index, operation) in block.operations.iter().enumerate() {
                let position = (
                    block.id,
                    u32::try_from(index).map_err(|_| "callable Terminal position overflow")?,
                );
                if let Some(result) = operation.result.as_structural() {
                    body.structural_results.insert(position, result.place);
                }
                if let Some((target, scalar, structural)) = invocation(&operation.kind) {
                    let site =
                        CallableInvocationSite::Invocation(InvocationSite::Operation(operation.id));
                    match project_callable_invocation(
                        &module.structural_types,
                        flow,
                        site,
                        match operation.kind {
                            O::BoundaryCall { boundary, .. } => Some(boundary),
                            _ => None,
                        },
                        position,
                        target,
                        scalar,
                        structural,
                    ) {
                        Ok(call) => {
                            body.retain_invocation(site, position, Ok(call))?;
                        }
                        Err(reason) => {
                            body.retain_invocation(site, position, Err(reason))?;
                        }
                    }
                }
                let Some(result) = operation.result.as_scalar() else {
                    continue;
                };
                let definition = match &operation.kind {
                    O::SelectMachine { descriptor } => CallableDefinition::Constructed(*descriptor),
                    _ if body.has_primary_invocation(position) => {
                        CallableDefinition::Invocation(position)
                    }
                    O::BoundaryCall { .. } => CallableDefinition::Opaque,
                    _ => stored_callable_definition(flow, position, result.id)
                        .unwrap_or(CallableDefinition::Unsupported),
                };
                body.insert_value(result.id, result.scalar_type, definition, false)?;
            }
            let position = (
                block.id,
                u32::try_from(block.operations.len())
                    .map_err(|_| "callable Terminal return position overflow")?,
            );
            let terminal_call = match &block.terminator {
                Terminator::InvokeNoNormal {
                    edge,
                    callee,
                    arguments,
                    structural_arguments,
                    ..
                } => Some((
                    CallableInvocationSite::Invocation(InvocationSite::InvokeNoNormal(*edge)),
                    CallableInvocationTarget::Machine(*callee),
                    None,
                    arguments.as_slice(),
                    structural_arguments.as_slice(),
                )),
                Terminator::ExternalCompletion {
                    edge,
                    boundary,
                    arguments,
                    structural_arguments,
                    ..
                } => Some((
                    CallableInvocationSite::Invocation(InvocationSite::ExternalCompletion(*edge)),
                    CallableInvocationTarget::Opaque,
                    Some(*boundary),
                    arguments.as_slice(),
                    structural_arguments.as_slice(),
                )),
                _ => None,
            };
            if let Some((site, target, boundary, scalar, structural)) = terminal_call {
                match project_callable_invocation(
                    &module.structural_types,
                    flow,
                    site,
                    boundary,
                    position,
                    target,
                    scalar,
                    structural,
                ) {
                    Ok(call) => {
                        body.retain_invocation(site, position, Ok(call))?;
                    }
                    Err(reason) => {
                        body.retain_invocation(site, position, Err(reason))?;
                    }
                }
            }
            for (edge, actions) in block.terminator.cleanup_rosters() {
                if body
                    .cleanup_actions
                    .insert(edge, actions.to_vec())
                    .is_some()
                {
                    return Err("callable cleanup has an ambiguous owning edge");
                }
                let successor = block
                    .terminator
                    .successor_edges()
                    .iter()
                    .position(|(id, _)| *id == edge);
                for (ordinal, action) in actions.iter().enumerate() {
                    if let terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup) =
                        action
                    {
                        cleanups.push((
                            machine.id,
                            position,
                            edge,
                            successor,
                            u32::try_from(ordinal)
                                .map_err(|_| "callable cleanup ordinal overflow")?,
                            cleanup.clone(),
                        ));
                    }
                }
            }
            match block.terminator {
                Terminator::Return { value, .. } => body.scalar_returns.push(value),
                Terminator::ReturnStructural { source, .. } => {
                    body.structural_returns.push((position, source))
                }
                _ => {}
            }
        }
        if bodies.insert(machine.id, body).is_some() {
            return Err("callable provenance has duplicate Terminal machine identities");
        }
    }
    for (caller, position, edge, successor, ordinal, cleanup) in cleanups {
        let flow = graph
            .iter()
            .find(|row| row.id == caller)
            .ok_or("callable cleanup lost its current Terminal storage flow")?;
        let callee = bodies
            .get(&cleanup.cleanup_machine)
            .ok_or("callable cleanup has no exact selected body")?;
        let site = CallableInvocationSite::CleanupAction {
            edge,
            action_ordinal: ordinal,
        };
        let call = project_nominal_cleanup_invocation(
            &module.structural_types,
            flow,
            position,
            edge,
            successor,
            ordinal,
            &cleanup,
            callee,
        );
        let body = bodies
            .get_mut(&caller)
            .ok_or("callable cleanup lost its caller")?;
        match call {
            Ok(call) => {
                body.retain_invocation(site, position, Ok(call))?;
            }
            Err(reason) => {
                body.retain_invocation(site, position, Err(reason))?;
            }
        }
    }
    Ok(CallableValueProgram {
        catalog: &module.callables,
        structural_types: &module.structural_types,
        graph,
        bodies,
    })
}

fn invocation(
    kind: &O,
) -> Option<(
    CallableInvocationTarget,
    &[ValueId],
    &[terminal_psi::StructuralArgument],
)> {
    match kind {
        O::Call {
            callee, arguments, ..
        } => Some((CallableInvocationTarget::Machine(*callee), arguments, &[])),
        O::CallUnit {
            callee,
            arguments,
            structural_arguments,
            ..
        }
        | O::CallStructuralScalar {
            callee,
            arguments,
            structural_arguments,
            ..
        }
        | O::CallStructuralWithScalarArguments {
            callee,
            arguments,
            structural_arguments,
            ..
        } => Some((
            CallableInvocationTarget::Machine(*callee),
            arguments,
            structural_arguments,
        )),
        O::CallStructural {
            callee,
            structural_arguments,
            ..
        } => Some((
            CallableInvocationTarget::Machine(*callee),
            &[],
            structural_arguments,
        )),
        O::CallMachine {
            callee,
            contract,
            arguments,
            structural_arguments,
            ..
        } => Some((
            CallableInvocationTarget::Value(*callee, *contract),
            arguments,
            structural_arguments,
        )),
        O::CallDynamic {
            arguments,
            structural_arguments,
            ..
        }
        | O::BoundaryCall {
            arguments,
            structural_arguments,
            ..
        } => Some((
            CallableInvocationTarget::Opaque,
            arguments,
            structural_arguments,
        )),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn project_callable_invocation(
    types: &[StructuralTypeDeclaration],
    flow: &DescriptorMachine,
    site: CallableInvocationSite,
    boundary: Option<BoundaryMachineId>,
    position: DescriptorPosition,
    target: CallableInvocationTarget,
    scalar_arguments: &[ValueId],
    structural_arguments: &[terminal_psi::StructuralArgument],
) -> CallableValueResult<CallableInvocation> {
    let structural_actual_types = structural_arguments
        .iter()
        .map(|actual| {
            let root = *flow
                .place_types
                .get(&actual.place)
                .ok_or("callable actual lost its exact structural declaration")?;
            let projected = crate::runtime_structural_path_tip(types.iter(), root, &actual.path)
                .ok_or("callable actual has no exact structural type")?;
            Ok((projected.id, actual.access))
        })
        .collect::<CallableValueResult<Vec<_>>>()?;
    let structural_arguments = structural_arguments
        .iter()
        .map(|actual| {
            crate::descriptor_lifetimes::descriptor_location(
                types,
                actual.place,
                *flow
                    .place_types
                    .get(&actual.place)
                    .ok_or("callable actual lost its exact structural declaration")?,
                &actual.path,
                None,
            )
            .ok_or("callable actual has no exact descriptor coordinate")
        })
        .collect::<CallableValueResult<Vec<_>>>()?;
    Ok(CallableInvocation {
        operation: site.operation(),
        site,
        boundary,
        position,
        successor: None,
        target,
        scalar_arguments: scalar_arguments.to_vec(),
        structural_arguments,
        structural_actual_types,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn project_nominal_cleanup_invocation(
    types: &[StructuralTypeDeclaration],
    flow: &DescriptorMachine,
    position: DescriptorPosition,
    edge: EdgeId,
    successor: Option<usize>,
    action_ordinal: u32,
    cleanup: &terminal_psi::NominalAffineCleanup,
    callee: &CallableValueBody,
) -> CallableValueResult<CallableInvocation> {
    if callee.attachment != Some(cleanup.structural_type)
        || callee.nominal_cleanup_subject != Some(cleanup.structural_type)
        || !callee.unit_result
        || !callee.scalar_parameters.is_empty()
        || callee.structural_formals.len() > 1
        || (callee.structural_formals.is_empty() && cleanup.cleanup_receiver.is_some())
    {
        return Err("callable cleanup does not match its exact nominal body signature");
    }
    let mut arguments = Vec::new();
    for formal in &callee.structural_formals {
        if !formal.is_cleanup_receiver(cleanup) {
            return Err("callable cleanup lost its exact receiver formal");
        }
        arguments.push(terminal_psi::StructuralArgument {
            place: cleanup.place,
            path: cleanup.path.clone(),
            access: formal.access,
        });
    }
    let mut invocation = project_callable_invocation(
        types,
        flow,
        CallableInvocationSite::CleanupAction {
            edge,
            action_ordinal,
        },
        None,
        position,
        CallableInvocationTarget::Machine(cleanup.cleanup_machine),
        &[],
        &arguments,
    )?;
    invocation.successor = successor;
    Ok(invocation)
}

pub fn stored_callable_definition(
    flow: &DescriptorMachine,
    position: DescriptorPosition,
    value: ValueId,
) -> Option<CallableDefinition> {
    let mut reads =
        flow.scalar_payload_reads
            .get(&position)?
            .iter()
            .filter_map(|(location, term)| {
                matches!(term, ScalarTerm::Value { id, .. } if *id == value).then_some(location)
            });
    let location = reads.next()?.clone();
    reads
        .next()
        .is_none()
        .then_some(CallableDefinition::Stored { position, location })
}
