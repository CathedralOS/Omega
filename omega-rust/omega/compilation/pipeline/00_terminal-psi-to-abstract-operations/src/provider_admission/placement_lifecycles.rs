//! Bind lifecycle inputs to the selected provider's actual completed call.
//!
//! A profile's shape and bytes are not its provenance. The unchanged payload
//! query identifies its producer; installation then supplies the exact selected
//! call. The profile call and lifecycle must observe the same unchanged extent
//! occurrence. Native initialization consumes the already checked owned grant,
//! validates the bound profile at runtime, and publishes Resident only after
//! committing its complete image. The selected profile provider supplies the
//! admitted resource premise; an arbitrary constructed profile cannot do so.
//! No interpreter allocation registry or second numeric-address grant is made.

use super::{AdmittedBoundaryExecution, AdmittedBoundarySettlement, AdmittedTerminalMechanism};
use crate::provider_installation::{AdmittedInstalledProviderCall, AdmittedProviderInstallation};
use diagnostics::Diagnostic;
use semantic_vocabulary::{InvocationSite, MachineId, OperationId};
use terminal_psi::{
    OperationKind, StructuralAccess, StructuralArgument, TerminalModule,
    TerminalPlacementLifecycleExecution, TerminalPlacementLifecycleOperation,
    TerminalPlacementLifecyclePlan,
};
use terminal_semantics::descriptor_lifetimes::DescriptorPosition;
use terminal_semantics::descriptor_results::{
    self, DescriptorAnalysis, DescriptorMachine, DescriptorPayloadTarget,
};

mod image_contract;

pub(super) fn settle_native_lifecycles<'execution>(
    input: &super::NativeRealizationInput,
    installation: Option<&AdmittedProviderInstallation>,
    policy: &super::TerminalAuthorityPolicy,
) -> Result<
    (
        Vec<AdmittedBoundarySettlement<'execution>>,
        Vec<AdmittedTerminalMechanism>,
    ),
    Vec<Diagnostic>,
> {
    let module = input.context().module();
    let primitive_plans: std::collections::BTreeMap<
        semantic_vocabulary::BoundaryMachineId,
        &TerminalPlacementLifecyclePlan,
    > = module
        .placement_lifecycle_plans
        .iter()
        .filter(|plan| {
            matches!(
                plan.execution,
                TerminalPlacementLifecycleExecution::CompilerPrimitive { .. }
            )
        })
        .map(|plan| (plan.boundary, plan))
        .collect();
    if primitive_plans.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let reached_functions: std::collections::BTreeMap<
        MachineId,
        &abstract_operations::AbstractFunction,
    > = input
        .plan()
        .functions
        .iter()
        .map(|function| (function.machine, function))
        .collect();
    let mut graph = None;
    let mut settlements = Vec::new();
    let mut mechanisms = Vec::new();
    for machine in &module.machines {
        let Some(function) = reached_functions.get(&machine.id) else {
            continue;
        };
        for block in &machine.blocks {
            for (ordinal, operation) in block.operations.iter().enumerate() {
                let OperationKind::BoundaryCall {
                    boundary,
                    structural_arguments,
                    ..
                } = &operation.kind
                else {
                    continue;
                };
                let Some(&plan) = primitive_plans.get(boundary) else {
                    continue;
                };
                // The canonical module also retains operations outside this
                // native product. A dead call or another target's body must not
                // demand execution merely because its declaration is present.
                if !function
                    .operations
                    .iter()
                    .any(|lowered| lowered.psi_operation() == Some(operation.id))
                {
                    continue;
                }
                let fail = |reason| {
                    vec![Diagnostic::error(format!(
                        "native placement lifecycle at machine {:?}, operation {:?}: {reason}",
                        machine.id, operation.id,
                    ))]
                };
                image_contract::require(module, plan).map_err(fail)?;
                {
                    let graph = graph.get_or_insert_with(|| {
                        descriptor_results::terminal_reference_graph(module)
                    });
                    let position = (
                        block.id,
                        u32::try_from(ordinal)
                            .map_err(|_| fail("operation position exceeds the descriptor graph"))?,
                    );
                    let installation = installation.ok_or_else(|| {
                        fail("profile supply has no admitted checked-provider installation")
                    })?;
                    bind_profile(
                        module,
                        graph,
                        installation,
                        machine.id,
                        position,
                        plan,
                        structural_arguments,
                    )
                    .map_err(fail)?;
                }
                // Admission is per actual occurrence; realization is one row per
                // declaration. A profile re-observation establishes neither
                // backing nor content: MoveOut still consumes the original
                // checked Resident contract, independently of its read supply.
                if settlements
                    .iter()
                    .any(|row: &AdmittedBoundarySettlement<'_>| row.boundary == plan.boundary)
                {
                    continue;
                }
                let mechanism =
                    effects::CompilerIntrinsicExecutionIdentity::PlacementLifecycle(plan.operation);
                policy.classify(mechanism).map_err(|error| vec![Diagnostic::error(format!(
                    "native lifecycle at machine {:?}, operation {:?} has no receiving classification for {:?}",
                    machine.id, operation.id, error.mechanism(),
                ))])?;
                settlements.push(AdmittedBoundarySettlement {
                    boundary: plan.boundary,
                    execution: AdmittedBoundaryExecution::CompilerBuiltin(
                        target_operations::CompilerBuiltinExecution::PlacementLifecycle {
                            boundary: plan.boundary,
                            operation: plan.operation,
                        },
                    ),
                    realization: target_operations::BoundarySettlementRealization::Builtin(
                        target_operations::BoundaryRealization::PlacementLifecycle {
                            boundary: plan.boundary,
                        },
                    ),
                });
                mechanisms.push(AdmittedTerminalMechanism {
                    boundary: plan.boundary,
                    mechanism: mechanism.into(),
                });
            }
        }
    }
    Ok((settlements, mechanisms))
}

fn bind_profile(
    module: &TerminalModule,
    graph: &[DescriptorMachine],
    installation: &AdmittedProviderInstallation,
    caller: MachineId,
    position: DescriptorPosition,
    plan: &TerminalPlacementLifecyclePlan,
    arguments: &[StructuralArgument],
) -> Result<(), &'static str> {
    let TerminalPlacementLifecycleExecution::CompilerPrimitive {
        profile_type: Some(profile_type),
        profile_requirement: Some(requirement),
    } = &plan.execution
    else {
        return Err("lifecycle lost its exact core profile requirement");
    };
    let (storage, profile) = match (plan.operation, arguments) {
        (TerminalPlacementLifecycleOperation::Initialize, [storage, _, profile])
        | (
            TerminalPlacementLifecycleOperation::MoveOut
            | TerminalPlacementLifecycleOperation::View,
            [storage, profile],
        ) => (storage, profile),
        _ => return Err("lifecycle lost its storage, value or profile input"),
    };
    let storage_access = if plan.operation == TerminalPlacementLifecycleOperation::View {
        module
            .boundary_machine(plan.boundary)
            .and_then(|boundary| boundary.structural_parameters.first())
            .filter(|parameter| {
                matches!(
                    parameter.access,
                    terminal_psi::reading_borrow_pair_pattern!()
                )
            })
            .ok_or("borrowed placement lost its declared storage loan")?
            .access
    } else {
        StructuralAccess::Owned
    };
    if storage.access != storage_access || profile.access != StructuralAccess::SharedBorrow {
        return Err("placement operation lost its storage access or original profile borrow");
    }
    let query = argument_query(module, graph, caller, profile)?;
    let original = descriptor_results::unchanged_payload_producer(
        &module.structural_types,
        graph,
        caller,
        position,
        &query,
    )?;
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == caller)
        .ok_or("profile consumer machine is absent")?;
    let producer = machine
        .blocks
        .iter()
        .find(|block| block.id == original.producer.0)
        .and_then(|block| block.operations.get(original.producer.1 as usize))
        .ok_or("profile producer operation is absent")?;
    let call = selected_profile_call(
        installation.installed_calls(),
        caller,
        producer.id,
        requirement.boundary,
    )?;
    if call.result() != &producer.result
        || call.result().as_structural().is_none_or(|result| {
            result.place != original.payload.root || result.structural_type != *profile_type
        })
    {
        return Err("profile is not the complete original selected-call result");
    }
    let [borrowed_storage] = call.structural_arguments() else {
        return Err("profile call has no exact borrowed extent input");
    };
    if borrowed_storage.access != StructuralAccess::SharedBorrow
        || !call.scalar_arguments().is_empty()
    {
        return Err("profile call differs from its shared extent contract");
    }
    let mut analysis = DescriptorAnalysis::with_reference_slots(&module.structural_types, graph);
    let supplied = argument_storage(
        module,
        graph,
        &mut analysis,
        caller,
        original.producer,
        borrowed_storage,
    )?;
    let consumed = argument_storage(module, graph, &mut analysis, caller, position, storage)?;
    if supplied != consumed {
        return Err("profile call and lifecycle refer to different extent occurrences");
    }
    descriptor_results::require_unchanged_observed_storage(
        &module.structural_types,
        graph,
        caller,
        original.producer,
        position,
        &consumed,
    )?;
    Ok(())
}

fn selected_profile_call(
    calls: &[AdmittedInstalledProviderCall],
    caller: MachineId,
    producer: OperationId,
    requirement: semantic_vocabulary::BoundaryMachineId,
) -> Result<&AdmittedInstalledProviderCall, &'static str> {
    let mut matching = calls.iter().filter(|call| {
        call.caller() == caller && call.invocation_site() == InvocationSite::Operation(producer)
    });
    let call = matching
        .next()
        .ok_or("profile was not produced by an installed selected-provider call")?;
    if matching.next().is_some() || call.boundary() != requirement {
        return Err("profile producer does not name the exact selected supply requirement");
    }
    Ok(call)
}

fn argument_query(
    module: &TerminalModule,
    graph: &[DescriptorMachine],
    caller: MachineId,
    argument: &StructuralArgument,
) -> Result<descriptor_results::DescriptorContentsQuery, &'static str> {
    let root = graph
        .iter()
        .find(|machine| machine.id == caller)
        .and_then(|machine| machine.place_types.get(&argument.place))
        .ok_or("profile/extent argument has no retained root type")?;
    descriptor_results::descriptor_contents_query(
        &module.structural_types,
        argument.place,
        *root,
        &argument.path,
        None,
    )
    .ok_or("profile/extent argument has no complete typed projection")
}

fn argument_storage(
    module: &TerminalModule,
    graph: &[DescriptorMachine],
    analysis: &mut DescriptorAnalysis<'_>,
    caller: MachineId,
    position: DescriptorPosition,
    argument: &StructuralArgument,
) -> Result<DescriptorPayloadTarget, &'static str> {
    let targets = analysis
        .payload_targets(
            caller,
            position,
            &argument_query(module, graph, caller, argument)?,
        )
        .ok_or("profile extent input has unresolved backing")?;
    let [storage] = targets.as_slice() else {
        return Err("profile extent input has ambiguous descriptor backing");
    };
    Ok(storage.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{BoundaryMachineId, EdgeId};
    use terminal_psi::{
        NominalDeclarationIdentity, OperationResult, ProviderCandidateConformance,
        ProviderRefinement, ProviderSignature, TerminalIdentityIndex,
    };

    fn call() -> AdmittedInstalledProviderCall {
        let identity = TerminalIdentityIndex(1);
        let boundary = BoundaryMachineId::new(7).unwrap();
        AdmittedInstalledProviderCall {
            caller: MachineId::new(3).unwrap(),
            invocation_site: InvocationSite::Operation(OperationId::new(11).unwrap()),
            boundary,
            // This is a coordinate-join unit test, not an installation claim.
            // The public native path separately supplies admitted installation.
            provider: ProviderCandidateConformance {
                boundary,
                requirement_identity: identity,
                source_requirement_identity: identity,
                receiving_application: TerminalIdentityIndex(0),
                receiving_region_arguments: vec![],
                provider_identity: identity,
                provider_declaration: NominalDeclarationIdentity::new(identity),
                candidate_identity: identity,
                source_candidate_identity: identity,
                source_candidate_package_identity: None,
                candidate: MachineId::new(4).unwrap(),
                signature: ProviderSignature::empty(),
                refinement: ProviderRefinement::empty(),
            },
            assumption: None,
            result: OperationResult::Unit,
            scalar_arguments: vec![],
            structural_arguments: vec![],
            completion_claim_sources: vec![],
            completion_receipts: vec![],
        }
    }

    #[test]
    fn profile_binding_requires_the_exact_selected_call_occurrence() {
        let call = call();
        let caller = call.caller();
        let operation = call.invocation_site().operation().unwrap();
        let boundary = call.boundary();
        assert!(
            selected_profile_call(std::slice::from_ref(&call), caller, operation, boundary).is_ok()
        );
        assert!(selected_profile_call(&[], caller, operation, boundary).is_err());
        assert!(
            selected_profile_call(&[call.clone(), call.clone()], caller, operation, boundary)
                .is_err()
        );
        let mut changed = call.clone();
        changed.caller = MachineId::new(5).unwrap();
        assert!(selected_profile_call(&[changed], caller, operation, boundary).is_err());
        let mut changed = call.clone();
        changed.boundary = BoundaryMachineId::new(8).unwrap();
        assert!(selected_profile_call(&[changed], caller, operation, boundary).is_err());
        let mut changed = call.clone();
        changed.invocation_site = InvocationSite::Operation(OperationId::new(12).unwrap());
        assert!(selected_profile_call(&[changed], caller, operation, boundary).is_err());
        let mut changed = call;
        changed.invocation_site = InvocationSite::ExternalCompletion(EdgeId::new(11).unwrap());
        assert!(selected_profile_call(&[changed], caller, operation, boundary).is_err());
    }
}
