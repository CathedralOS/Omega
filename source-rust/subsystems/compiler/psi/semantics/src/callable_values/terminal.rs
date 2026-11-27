use super::*;
use terminal_psi::{OperationKind as O, TerminalModule, Terminator};

/// Follow selected providers through the same actuals and returned code values
/// as ordinary calls. Making their bodies independent entry roots would lose
/// that substitution and could activate code the caller never invokes.
pub(crate) fn terminal_callable_values_with_boundary_implementations<'a>(
    module: &'a TerminalModule,
    implementations: &[CallableBoundaryImplementation],
) -> Result<CallableValueProgram<'a>, String> {
    check_declared_implementations(module, implementations)?;
    terminal_callable_values(module)?
        .with_boundary_implementations(implementations)
        .map_err(str::to_owned)
}

/// Apply the bindings to an already built program, refusing them with the
/// same reasons and in the same order as
/// `terminal_callable_values_with_boundary_implementations`.
pub(crate) fn bind_declared_implementations<'p>(
    module: &TerminalModule,
    program: Result<CallableValueView<'p>, &CallableValuePreparationError>,
    implementations: &[CallableBoundaryImplementation],
) -> Result<BoundCallableValues<'p>, String> {
    check_declared_implementations(module, implementations)?;
    let program = program.map_err(|error| error.clone().in_module(module))?;
    BoundCallableValues::new(program, implementations).map_err(str::to_owned)
}

fn check_declared_implementations(
    module: &TerminalModule,
    implementations: &[CallableBoundaryImplementation],
) -> Result<(), String> {
    if implementations.iter().any(|binding| {
        module.boundary_machine(binding.boundary).is_none()
            || module.machine(binding.machine).is_none()
    }) {
        return Err("callable boundary implementation has an absent requirement or body".into());
    }
    Ok(())
}

/// Boundary demand and the exact machines invoked by the same callable analysis.
/// A directly invoked selected body can be reached without its boundary ID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReachedBoundaryCalls {
    pub boundaries: Vec<BoundaryMachineId>,
    pub reached_machines: Vec<MachineId>,
}

/// Find actual boundary invocations from an entry, including selected code,
/// terminal edges and cleanup calls. Retaining a boundary header or descriptor
/// alone does not make it a demand for executable supply.
pub fn reached_boundary_calls(
    module: &TerminalModule,
    entry: MachineId,
    implementations: &[CallableBoundaryImplementation],
) -> Result<Vec<BoundaryMachineId>, String> {
    Ok(boundary_call_reach(module, entry, implementations)?.boundaries)
}

/// Keep machine reach and boundary demand paired to one actual-argument query.
pub fn boundary_call_reach(
    module: &TerminalModule,
    entry: MachineId,
    implementations: &[CallableBoundaryImplementation],
) -> Result<ReachedBoundaryCalls, String> {
    check_declared_implementations(module, implementations)?;
    let program = terminal_callable_values(module)?;
    let values = BoundCallableValues::new(&program, implementations).map_err(str::to_owned)?;
    boundary_call_reach_in(module, &values, entry)
}

/// `boundary_call_reach` over a program the caller already built and bound.
/// Callers that ask about many entries under one binding set build the
/// program once instead of once per entry.
pub fn boundary_call_reach_in<'p>(
    module: &TerminalModule,
    values: impl Into<CallableValueView<'p>>,
    entry: MachineId,
) -> Result<ReachedBoundaryCalls, String> {
    let reached = reachable_callable_invocations(values, [entry])?;
    let mut boundaries = Vec::new();
    for row in &reached.invocations {
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
                        .execution.callables
                        .contract(*ceiling)
                        .ok_or("reached callable has no exact unresolved contract")?;
                    if !contract.envelope.invocations.is_empty() {
                        return Err("unresolved callable arrivals need their complete boundary demand representation".into());
                    }
                }
            }
            ReachedCallableTarget::Opaque if row.invocation.boundary.is_none() => {
                return Err(
                    "reached opaque invocation has no boundary or callable contract".into(),
                );
            }
            _ => {}
        }
    }
    boundaries.sort_unstable();
    boundaries.dedup();
    Ok(ReachedBoundaryCalls {
        boundaries,
        reached_machines: reached.reached_machines,
    })
}

/// Keep the failed storage site until the caller chooses its diagnostic.
/// Projection failures still carry their existing text; neither variant grants
/// execution or skips the shared eager graph preparation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CallableValuePreparationError {
    StorageFlow(crate::descriptor_results::DescriptorGraphError),
    Projection(String),
}

impl CallableValuePreparationError {
    /// Classify only an absent declaration at the exact failed operation.
    /// The public graph-error guard retains its unique machine/block rules.
    /// Index the retained point rather than searching by a possibly duplicate
    /// operation ID, and reuse the operation's ordinary invocation decoder.
    pub fn missing_boundary(&self, module: &TerminalModule) -> Option<BoundaryMachineId> {
        let Self::StorageFlow(error) = self else {
            return None;
        };
        let _ = error.operation(module)?;
        let crate::descriptor_results::DescriptorGraphError::IncompleteMachine {
            position: Some((block, index)),
            ..
        } = *error
        else {
            return None;
        };
        let operation = module
            .machine(error.machine())?
            .block(&block)?
            .operations
            .get(usize::try_from(index).ok()?)?;
        let terminal_psi::TargetMachineCallable::Boundary(boundary) =
            operation.declared_invocation()?.callee
        else {
            return None;
        };
        module
            .boundary_machine(boundary)
            .is_none()
            .then_some(boundary)
    }

    /// Preserve the established String API's exact failure description.
    pub fn in_module(self, module: &TerminalModule) -> String {
        match self {
            Self::StorageFlow(error) => format!(
                "callable provenance cannot reconstruct current Terminal storage flow: {}",
                error.in_module(module),
            ),
            Self::Projection(message) => message,
        }
    }
}

impl From<String> for CallableValuePreparationError {
    fn from(message: String) -> Self {
        Self::Projection(message)
    }
}

impl From<&str> for CallableValuePreparationError {
    fn from(message: &str) -> Self {
        Self::Projection(message.to_owned())
    }
}

/// Project current executable Terminal operations into the shared value query.
pub fn terminal_callable_values(
    module: &TerminalModule,
) -> Result<CallableValueProgram<'_>, String> {
    terminal_callable_values_with_error_context(module).map_err(|error| error.in_module(module))
}

/// The same projection with its structured preparation failure retained.
fn terminal_callable_values_with_error_context(
    module: &TerminalModule,
) -> Result<CallableValueProgram<'_>, CallableValuePreparationError> {
    let graph = crate::descriptor_results::terminal_reference_graph_from(
        module,
        module.machines.iter().map(|machine| machine.id),
    )
    .map_err(CallableValuePreparationError::StorageFlow)?;
    project_callable_values(module, graph)
}

/// The same projection, adapting the reference rows with the shared
/// analyses' sorted machines and zeroed-case table instead of preparing them
/// again.
///
/// The program owns its rows because the native compiler builds the same
/// type from rows it owns, and a row cannot be copied out of the shared
/// family. The rows are the ones `terminal_reference_graph_from` returns for
/// every machine.
pub(crate) fn terminal_callable_values_in<'m>(
    analyses: &crate::module_analyses::ModuleAnalyses<'m>,
) -> Result<CallableValueProgram<'m>, CallableValuePreparationError> {
    let module = analyses.module();
    let graph = analyses
        .closure_rows(
            crate::descriptor_results::DescriptorSlotKind::References,
            module.machines.iter().map(|machine| machine.id),
            &[],
            crate::descriptors::descriptor_results::GenericCallInputs::default(),
        )
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(CallableValuePreparationError::StorageFlow)?;
    project_callable_values(module, graph)
}

fn project_callable_values(
    module: &TerminalModule,
    graph: Vec<DescriptorMachine>,
) -> Result<CallableValueProgram<'_>, CallableValuePreparationError> {
    // Each machine and each cleanup caller looks up its row by identity.
    // Sorting (identity, position) pairs keeps the first row for an identity
    // first, which is the row a front-to-back search would return.
    let mut flow_positions = graph
        .iter()
        .enumerate()
        .map(|(position, row)| (row.id, position))
        .collect::<Vec<_>>();
    flow_positions.sort_unstable();
    let flow_of = |machine: semantic_vocabulary::MachineId| {
        let first = flow_positions.partition_point(|(id, _)| *id < machine);
        flow_positions
            .get(first)
            .filter(|(id, _)| *id == machine)
            .map(|(_, position)| &graph[*position])
    };
    let mut bodies: Vec<(semantic_vocabulary::MachineId, CallableValueBody)> =
        Vec::with_capacity(module.machines.len());
    let mut cleanups = Vec::new();
    for machine in &module.machines {
        let flow = flow_of(machine.id)
            .ok_or("callable provenance machine lost its current Terminal storage flow")?;
        let mut body = CallableValueBody {
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
                    body.retain_structural_result(position, result.place);
                }
                if let Some((target, scalar, structural)) = invocation(&operation.kind) {
                    let site =
                        CallableInvocationSite::Invocation(InvocationSite::Operation(operation.id));
                    match project_callable_invocation(
                        &module.types.structural_types,
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
                    &module.types.structural_types,
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
                    .retain_cleanup_actions(edge, actions.to_vec())
                    .is_some()
                {
                    return Err("callable cleanup has an ambiguous owning edge".into());
                }
                let successor = block
                    .terminator
                    .successor_edges()
                    .iter()
                    .position(|(id, _)| *id == edge);
                for (ordinal, action) in actions.iter().enumerate() {
                    if let terminal_psi::TerminalAffineCleanupAction::InvokeOwned(cleanup) = action
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
        let Err(position) = bodies.binary_search_by_key(&machine.id, |(id, _)| *id) else {
            return Err("callable provenance has duplicate Terminal machine identities".into());
        };
        bodies.insert(position, (machine.id, body));
    }
    for (caller, position, edge, successor, ordinal, cleanup) in cleanups {
        let flow =
            flow_of(caller).ok_or("callable cleanup lost its current Terminal storage flow")?;
        let site = CallableInvocationSite::CleanupAction {
            edge,
            action_ordinal: ordinal,
        };
        let call = project_owned_cleanup_invocation(
            &module.types.structural_types,
            flow,
            position,
            edge,
            successor,
            ordinal,
            &cleanup,
        );
        let body_position = bodies
            .binary_search_by_key(&caller, |(id, _)| *id)
            .map_err(|_| "callable cleanup lost its caller")?;
        let body = &mut bodies[body_position].1;
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
        catalog: &module.execution.callables,
        structural_types: &module.types.structural_types,
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
    let mut locations = Vec::with_capacity(structural_arguments.len());
    let mut structural_actual_types = Vec::with_capacity(structural_arguments.len());
    for actual in structural_arguments {
        let root = *flow
            .place_types
            .get(&actual.place)
            .ok_or("callable actual lost its exact structural declaration")?;
        // A lent element of a byte sequence has no declaration of its own; its
        // actual type is the primitive carrier the borrowed formal declares.
        let projected = crate::borrowed_projection_type(types.iter(), root, &actual.path, None)
            .ok_or("callable actual has no exact structural type")?;
        // The invocation records its actual at this evaluation point. A
        // query that substitutes a stored callable must separately resolve
        // the descriptor contents at that point.
        locations.push(DescriptorLocation {
            root: actual.place,
            path: crate::resolved_structural_path(types.iter(), root, &actual.path)?,
        });
        structural_actual_types.push((projected, actual.access));
    }
    Ok(CallableInvocation {
        operation: site.operation(),
        site,
        boundary,
        position,
        successor: None,
        target,
        scalar_arguments: scalar_arguments.to_vec(),
        structural_arguments: locations,
        structural_actual_types,
    })
}

/// Project the exact ordinary owned call already retained by the descriptor
/// graph. Its producer validates carrier selection, formal ownership and the
/// source actual; this query does not manufacture a special cleanup signature.
#[allow(clippy::too_many_arguments)]
pub fn project_owned_cleanup_invocation(
    types: &[StructuralTypeDeclaration],
    flow: &DescriptorMachine,
    position: DescriptorPosition,
    edge: EdgeId,
    successor: Option<usize>,
    action_ordinal: u32,
    cleanup: &terminal_psi::OwnedCleanupInvocation,
) -> CallableValueResult<CallableInvocation> {
    let retained = flow
        .invocation(
            position,
            crate::descriptor_results::DescriptorInvocationOwner::Cleanup {
                edge,
                action_ordinal,
            },
        )
        .ok_or("callable cleanup lost its exact retained owned call")?;
    let actuals = cleanup.module_invocation();
    let mut invocation = project_callable_invocation(
        types,
        flow,
        CallableInvocationSite::CleanupAction {
            edge,
            action_ordinal,
        },
        None,
        position,
        CallableInvocationTarget::Machine(actuals.callee),
        actuals.arguments,
        actuals.structural_arguments,
    )?;
    if retained.callee() != Some(actuals.callee)
        || !retained.scalar_arguments.is_empty()
        || retained.arguments.len() != invocation.structural_arguments.len()
        || !retained
            .arguments
            .iter()
            .zip(&invocation.structural_arguments)
            .all(|(retained, actual)| retained.as_ref() == Some(actual))
        || invocation.structural_actual_types
            != [(
                cleanup.structural_type,
                terminal_psi::StructuralAccess::Owned,
            )]
    {
        return Err("callable cleanup differs from its retained owned actual call");
    }
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
