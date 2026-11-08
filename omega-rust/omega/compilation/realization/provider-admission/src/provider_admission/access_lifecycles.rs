// Native access lifecycle and empty-run release settlement.
//
// A profile's shape and bytes are not its provenance. The unchanged payload
// query identifies its producer; installation then supplies the exact selected
// call, and the profile call and lifecycle must observe the same unchanged
// extent occurrence. `require_image_contract` verifies the native whole-value
// codec contract (non-erased runtime fields, acyclic inline storage, and full
// integer storage widths on initialization).
//
// `settle_empty_releases` settles the source-authenticated `EmptyRunRelease`
// operation, whose checked precondition proves zero cells so the native
// operation transports the original storage descriptor unchanged without
// calling a provider.

use super::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement, AdmittedTerminalMechanism,
    NativeRealizationInput, SettledCompilerBuiltin, TerminalAuthorityPolicy,
};
use crate::provider_installation::{AdmittedInstalledProviderCall, AdmittedProviderInstallation};
use diagnostics::Diagnostic;
use effects::CompilerIntrinsicExecutionIdentity;
use layout_plans::LayoutPlacementReport;
use semantic_vocabulary::{InvocationSite, MachineId, OperationId, StructuralTypeId};
use terminal_psi::{
    CheckedBoundaryExecution, OperationKind, StructuralAccess, StructuralArgument,
    StructuralFieldDeclaration, StructuralTypeShape, TerminalAccessLifecycleExecution,
    TerminalAccessLifecycleOperation, TerminalAccessLifecyclePlan, TerminalModule,
};
use terminal_semantics::descriptor_lifetimes::DescriptorPosition;
use terminal_semantics::descriptor_results::{
    self, DescriptorAnalysis, DescriptorMachine, DescriptorPayloadTarget,
};

pub(super) fn settle_native_lifecycles<'execution>(
    input: &NativeRealizationInput,
    installation: Option<&AdmittedProviderInstallation>,
    policy: &TerminalAuthorityPolicy,
) -> Result<
    (
        Vec<AdmittedBoundarySettlement<'execution>>,
        Vec<AdmittedTerminalMechanism>,
    ),
    Vec<Diagnostic>,
> {
    let module = input.module();
    let primitive_plans: std::collections::BTreeMap<
        semantic_vocabulary::BoundaryMachineId,
        &TerminalAccessLifecyclePlan,
    > = input
        .plan()
        .structural_types
        .access_lifecycles()
        .iter()
        .filter(|plan| {
            matches!(
                plan.execution,
                TerminalAccessLifecycleExecution::CompilerPrimitive { .. }
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
                if !function
                    .operations
                    .iter()
                    .any(|lowered| lowered.psi_operation() == Some(operation.id))
                {
                    continue;
                }
                let fail = |reason| {
                    vec![Diagnostic::error(format!(
                        "native access lifecycle at machine {:?}, operation {:?}: {reason}",
                        machine.id, operation.id,
                    ))]
                };
                require_image_contract(module, plan).map_err(fail)?;
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
                if settlements
                    .iter()
                    .any(|row: &AdmittedBoundarySettlement<'_>| row.boundary == plan.boundary)
                {
                    continue;
                }
                let mechanism =
                    effects::CompilerIntrinsicExecutionIdentity::AccessLifecycle(plan.operation);
                policy.classify(mechanism).map_err(|error| vec![Diagnostic::error(format!(
                    "native lifecycle at machine {:?}, operation {:?} has no receiving classification for {:?}",
                    machine.id, operation.id, error.mechanism(),
                ))])?;
                let settled = SettledCompilerBuiltin::from_identity(mechanism, plan.boundary, None)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(format!(
                            "native lifecycle at machine {:?}, operation {:?} has no catalog entry",
                            machine.id, operation.id,
                        ))]
                    })?;
                settlements.push(AdmittedBoundarySettlement {
                    boundary: plan.boundary,
                    execution: AdmittedBoundaryExecution::CompilerBuiltin(settled),
                    realization: target_operations::BoundarySettlementRealization::Builtin(
                        target_operations::BoundaryRealization::AccessLifecycle {
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

pub(super) fn settle_empty_releases<'execution>(
    input: &NativeRealizationInput,
    policy: &TerminalAuthorityPolicy,
) -> Result<
    (
        Vec<AdmittedBoundarySettlement<'execution>>,
        Vec<AdmittedTerminalMechanism>,
    ),
    Vec<Diagnostic>,
> {
    let module = input.module();
    let mut settlements = Vec::new();
    let mut mechanisms = Vec::new();
    let mut reached: Vec<_> = input
        .plan()
        .functions
        .iter()
        .flat_map(|function| &function.operations)
        .filter_map(|operation| match operation {
            abstract_operations::AbstractOperation::BoundaryCall { boundary, .. } => {
                Some(*boundary)
            }
            _ => None,
        })
        .collect();
    reached.sort_unstable();
    reached.dedup();
    for boundary_id in reached {
        let Some(boundary) = module.boundary_machine(boundary_id) else {
            return Err(vec![Diagnostic::error(
                "reached boundary has no Terminal declaration",
            )]);
        };
        if boundary.declaration.as_ref().map(|row| row.execution)
            != Some(CheckedBoundaryExecution::EmptyRunRelease)
        {
            continue;
        }
        let fail = |reason: &str| {
            vec![Diagnostic::error(format!(
                "empty-run release {:?}: {reason}",
                boundary.id,
            ))]
        };
        terminal_semantics::empty_run_release_contract(&module.structural_types, boundary)
            .map_err(fail)?;
        let mechanism = CompilerIntrinsicExecutionIdentity::EmptyRunRelease;
        policy.classify(mechanism).map_err(|error| {
            fail(&format!(
                "missing execution classification for {:?}",
                error.mechanism(),
            ))
        })?;
        let settled = SettledCompilerBuiltin::from_identity(mechanism, boundary.id, None)
            .ok_or_else(|| fail("missing compiler execution catalog entry"))?;
        settlements.push(AdmittedBoundarySettlement {
            boundary: boundary.id,
            execution: AdmittedBoundaryExecution::CompilerBuiltin(settled),
            realization: target_operations::BoundarySettlementRealization::Builtin(
                target_operations::BoundaryRealization::EmptyRunRelease,
            ),
        });
        mechanisms.push(AdmittedTerminalMechanism {
            boundary: boundary.id,
            mechanism: mechanism.into(),
        });
    }
    Ok((settlements, mechanisms))
}

fn require_image_contract(
    module: &TerminalModule,
    plan: &TerminalAccessLifecyclePlan,
) -> Result<(), &'static str> {
    if plan.operation == terminal_psi::TerminalAccessLifecycleOperation::View {
        return terminal_semantics::validate_access_view_content(module, plan);
    }
    let declaration = module
        .structural_type(plan.schema)
        .ok_or("native lifecycle schema is absent")?;
    let StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
        return Err("native lifecycle has no complete record image");
    };
    if terminal_psi::structural_type_has_borrowed_storage_by(
        |identity| module.structural_type(identity),
        plan.schema,
    ) {
        return Err(
            "native resident content requires retained loan forwarding for reference fields",
        );
    }
    require_runtime_value(module, plan.schema, &mut Vec::new())?;
    if plan.operation != terminal_psi::TerminalAccessLifecycleOperation::Initialize {
        return Ok(());
    }
    for (field, access) in plan
        .image_fields
        .iter()
        .zip(plan.placement.access().plan().entries())
    {
        let field = fields
            .iter()
            .find(|candidate| candidate.id == *field)
            .ok_or("native lifecycle image field is absent")?;
        for entry in plan
            .placement
            .layout()
            .entries
            .iter()
            .filter(|entry| entry.field == access.field())
        {
            if let LayoutPlacementReport::IntegerAt { stored_width, .. } = entry.placement {
                let scalar = field
                    .field_type
                    .scalar_type()
                    .ok_or("stored integer image field has no scalar carrier")?;
                let bytes = terminal_semantics::reference_storage_byte_width(scalar)
                    .ok_or("stored integer image field has no fixed width")?;
                if stored_width != u64::from(bytes) * 8 {
                    return Err(
                        "native lifecycle narrowing requires checked concrete-value fitting evidence",
                    );
                }
            }
        }
    }
    Ok(())
}

fn require_runtime_value(
    module: &TerminalModule,
    root: StructuralTypeId,
    active: &mut Vec<StructuralTypeId>,
) -> Result<(), &'static str> {
    if active.contains(&root) {
        return Err("native lifecycle value has cyclic inline storage");
    }
    active.push(root);
    let declaration = module
        .structural_type(root)
        .ok_or("native lifecycle value declaration is absent")?;
    match &declaration.shape {
        StructuralTypeShape::Record { fields, .. } => require_fields(module, fields, active)?,
        StructuralTypeShape::Sum { cases } => {
            for case in cases {
                require_fields(module, &case.fields, active)?;
            }
        }
        StructuralTypeShape::Mixed { fields, cases } => {
            require_fields(module, fields, active)?;
            for case in cases {
                require_fields(module, &case.fields, active)?;
            }
        }
        StructuralTypeShape::FixedArray { element, length } if length.literal() != Some(0) => {
            require_runtime_value(module, *element, active)?
        }
        _ => {}
    }
    active.pop();
    Ok(())
}

fn require_fields(
    module: &TerminalModule,
    fields: &[StructuralFieldDeclaration],
    active: &mut Vec<StructuralTypeId>,
) -> Result<(), &'static str> {
    for field in fields {
        if field.relevance.is_erased() {
            return Err(
                "native lifecycle non-runtime fields require an authored custody agreement",
            );
        }
        if let Some(child) = field.field_type.as_structural() {
            if terminal_semantics::reference_storage_size(module.structural_types.iter(), child)
                == Some(0)
            {
                return Err(
                    "native lifecycle non-runtime fields require an authored custody agreement",
                );
            }
            require_runtime_value(module, child, active)?;
        }
    }
    Ok(())
}

fn bind_profile(
    module: &TerminalModule,
    graph: &[DescriptorMachine],
    installation: &AdmittedProviderInstallation,
    caller: MachineId,
    position: DescriptorPosition,
    plan: &TerminalAccessLifecyclePlan,
    arguments: &[StructuralArgument],
) -> Result<(), &'static str> {
    let TerminalAccessLifecycleExecution::CompilerPrimitive {
        profile_type: Some(profile_type),
        profile_requirements,
    } = &plan.execution
    else {
        return Err("lifecycle lost its exact core profile requirement");
    };
    let operands = plan.operation.profile_operands();
    if profile_requirements.len() != operands.len() {
        return Err("lifecycle lost its complete supply-role roster");
    }
    for (operand, requirement) in operands.iter().zip(profile_requirements) {
        let storage = arguments
            .get(operand.storage)
            .ok_or("lifecycle lost its storage input")?;
        let profile = arguments
            .get(operand.profile)
            .ok_or("lifecycle lost its profile input")?;
        let storage_access = if plan.operation == TerminalAccessLifecycleOperation::View {
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
        let mut analysis =
            DescriptorAnalysis::with_reference_slots(&module.structural_types, graph);
        let supplied = argument_storage(
            module,
            graph,
            &mut analysis,
            caller,
            original.producer,
            borrowed_storage,
        )?;
        let mut backing = storage.clone();
        if operand.occupied {
            backing.path.extend(
                plan.storage
                    .owner_storage_path(&module.structural_types)
                    .ok_or("lifecycle owner lost its exact backing projection")?,
            );
        }
        let consumed = argument_storage(module, graph, &mut analysis, caller, position, &backing)?;
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
    }
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
            provider: ProviderCandidateConformance {
                boundary,
                requirement_identity: identity,
                source_requirement_identity: identity,
                declaration_application: TerminalIdentityIndex(0),
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
