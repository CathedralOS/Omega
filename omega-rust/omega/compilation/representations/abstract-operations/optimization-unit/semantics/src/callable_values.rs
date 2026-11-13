//! Project current Abstract/optimization operations into the shared code-value query.

use std::borrow::Cow;
use std::collections::BTreeMap;

use abstract_operations::{AbstractCallTarget, AbstractOperation as O, AbstractOperationPlan};
use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
use terminal_semantics::callable_values::{
    CallableDefinition, CallableInvocationSite, CallableInvocationTarget, CallableValueBody,
    CallableValueProgram, CallableValueResult, project_callable_invocation,
    project_nominal_cleanup_invocation, stored_callable_definition,
};

pub fn optimized_callable_invocations(
    unit: &PsiOptimizationUnit,
) -> CallableValueResult<Option<terminal_semantics::callable_values::ReachableCallableInvocations>>
{
    if !unit.block_nodes().any(|node| {
        matches!(
            node.operation,
            O::CallStructural {
                callee: AbstractCallTarget::Callable { .. },
                ..
            }
        )
    }) {
        return Ok(None);
    }
    let program = optimized_callable_values(unit)?;
    terminal_semantics::callable_values::reachable_callable_invocations(&program, [unit.entry])
        .map(Some)
}

pub fn abstract_callable_values(
    plan: &AbstractOperationPlan,
) -> CallableValueResult<CallableValueProgram<'_>> {
    let functions = plan
        .functions
        .iter()
        .map(optimization_unit::reconstruct_psi_optimization_function_seed)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "callable provenance cannot reconstruct current Abstract function graphs")?;
    project(
        &plan.callables,
        plan.structural_types.as_slice(),
        &plan.boundary_machines,
        &plan.dynamic_dispatch,
        &functions,
    )
}

pub fn optimized_callable_values(
    unit: &PsiOptimizationUnit,
) -> CallableValueResult<CallableValueProgram<'_>> {
    project(
        &unit.callables,
        unit.structural_types.as_slice(),
        &unit.boundary_machines,
        &unit.dynamic_dispatch,
        &unit.functions,
    )
}

fn project<'a>(
    catalog: &'a terminal_psi::TerminalCallableCatalog,
    structural_types: &'a [terminal_psi::StructuralTypeDeclaration],
    boundaries: &[terminal_psi::BoundaryMachineDeclaration],
    dynamic: &terminal_psi::TerminalDynamicDispatchCatalog,
    functions: &[PsiOptimizationFunction],
) -> CallableValueResult<CallableValueProgram<'a>> {
    let graph = crate::descriptor_results::optimized_reference_catalog_graph(
        functions,
        boundaries,
        structural_types,
        dynamic,
        |_, node| Some(Cow::Borrowed(&node.operation)),
    )
    .ok_or("callable provenance cannot reconstruct current Abstract storage flow")?;
    let mut bodies = BTreeMap::new();
    let mut cleanups = Vec::new();
    for function in functions {
        let flow = graph
            .iter()
            .find(|row| row.id == function.machine)
            .ok_or("callable provenance function lost its current Abstract storage flow")?;
        let mut body = CallableValueBody::default();
        body.attachment = function.attachment;
        body.nominal_cleanup_subject = function.nominal_cleanup_subject;
        body.unit_result = matches!(
            function.result,
            abstract_operations::AbstractFunctionResult::Unit
        );
        body.structural_formals = function.structural_parameters.clone();
        body.scalar_result = function.result.scalar().map(|result| result.scalar_type);
        body.structural_result = function
            .result
            .structural()
            .map(|result| result.structural_type);
        body.structural_parameters = function
            .structural_parameters
            .iter()
            .map(|parameter| (parameter.structural_type, parameter.access))
            .collect();
        for (position, parameter) in function.parameters.iter().enumerate() {
            body.scalar_parameters
                .push((parameter.value, parameter.scalar_type));
            body.insert_value(
                parameter.value,
                parameter.scalar_type,
                CallableDefinition::Formal(position),
                false,
            )?;
        }
        for block in &function.blocks {
            for parameter in &block.parameters {
                body.insert_value(
                    parameter.value,
                    parameter.scalar_type,
                    CallableDefinition::Incoming(block.id),
                    block.id == function.entry,
                )?;
            }
            for (index, node) in block.nodes.iter().enumerate() {
                let position = (
                    block.id,
                    u32::try_from(index).map_err(|_| "callable Abstract position overflow")?,
                );
                if let Some((_, result)) = node.operation.structural_result() {
                    body.structural_results.insert(position, result.place);
                }
                if let Some((target, scalar, structural)) = invocation(&node.operation) {
                    let site = match node.operation {
                        O::InvokeNoNormal { psi_edge, .. } => {
                            semantic_vocabulary::InvocationSite::InvokeNoNormal(psi_edge)
                        }
                        O::ExternalCompletion { psi_edge, .. } => {
                            semantic_vocabulary::InvocationSite::ExternalCompletion(psi_edge)
                        }
                        _ => semantic_vocabulary::InvocationSite::Operation(
                            node.operation.psi_operation().ok_or(
                                "callable Abstract invocation lost its operation identity",
                            )?,
                        ),
                    };
                    let site = CallableInvocationSite::Invocation(site);
                    match project_callable_invocation(
                        structural_types,
                        flow,
                        site,
                        match node.operation {
                            O::BoundaryCall { boundary, .. }
                            | O::ExternalCompletion { boundary, .. } => Some(boundary),
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
                for result in &node.definitions {
                    let definition = match &node.operation {
                        O::SelectMachine { descriptor, .. } => {
                            CallableDefinition::Constructed(*descriptor)
                        }
                        _ if body.has_primary_invocation(position) => {
                            CallableDefinition::Invocation(position)
                        }
                        _ => stored_callable_definition(flow, position, result.value)
                            .unwrap_or(CallableDefinition::Unsupported),
                    };
                    body.insert_value(result.value, result.scalar_type, definition, false)?;
                }
                for (edge, actions) in node.operation.cleanup_rosters() {
                    if body
                        .cleanup_actions
                        .insert(edge, actions.to_vec())
                        .is_some()
                    {
                        return Err("callable cleanup has an ambiguous owning edge");
                    }
                    let successor = node
                        .successors
                        .iter()
                        .position(|successor| successor.psi_edge == edge);
                    for (ordinal, action) in actions.iter().enumerate() {
                        if let terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup) =
                            action
                        {
                            cleanups.push((
                                function.machine,
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
                match &node.operation {
                    O::Return { value, .. } => body.scalar_returns.push(*value),
                    O::ReturnStructural { source, .. } => {
                        body.structural_returns.push((position, *source))
                    }
                    _ => {}
                }
            }
        }
        if bodies.insert(function.machine, body).is_some() {
            return Err("callable provenance has duplicate Abstract machine identities");
        }
    }
    for (caller, position, edge, successor, ordinal, cleanup) in cleanups {
        let flow = graph
            .iter()
            .find(|row| row.id == caller)
            .ok_or("callable cleanup lost its current Abstract storage flow")?;
        let callee = bodies
            .get(&cleanup.cleanup_machine)
            .ok_or("callable cleanup has no exact selected body")?;
        let site = CallableInvocationSite::CleanupAction {
            edge,
            action_ordinal: ordinal,
        };
        let call = project_nominal_cleanup_invocation(
            structural_types,
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
        catalog,
        structural_types,
        graph,
        bodies,
    })
}

fn invocation(
    kind: &O,
) -> Option<(
    CallableInvocationTarget,
    &[semantic_vocabulary::ValueId],
    &[terminal_psi::StructuralArgument],
)> {
    match kind {
        O::Call {
            callee, arguments, ..
        } => Some((CallableInvocationTarget::Machine(*callee), arguments, &[])),
        O::CallStructural {
            callee,
            arguments,
            structural_arguments,
            ..
        } => {
            let target = match *callee {
                AbstractCallTarget::Machine(machine) => CallableInvocationTarget::Machine(machine),
                AbstractCallTarget::Callable { callee, contract } => {
                    CallableInvocationTarget::Value(callee, contract)
                }
                AbstractCallTarget::Dynamic { .. } => CallableInvocationTarget::Opaque,
            };
            Some((target, arguments, structural_arguments))
        }
        O::InvokeNoNormal {
            callee,
            arguments,
            structural_arguments,
            ..
        } => Some((
            CallableInvocationTarget::Machine(*callee),
            arguments,
            structural_arguments,
        )),
        O::BoundaryCall {
            arguments,
            structural_arguments,
            ..
        }
        | O::ExternalCompletion {
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
