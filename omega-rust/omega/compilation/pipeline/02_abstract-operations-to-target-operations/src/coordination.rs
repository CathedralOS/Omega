/*
  Whole-plan target lowering coordination
  (`lower_to_target_operations_with_settlements_and_installation`), settlement
  roster admission, installed-provider call replay, and native callback argument
  binding.

  Admits boundary settlements and provider installation evidence once for the
  plan, derives each function's fixed scalar and mixed structural-scalar ABIs,
  constructs the borrowed-source and stored-effect reference catalogs, and
  lowers every function into `TargetOperationPlan`.
*/
use super::function::lower_function;
use super::scalar_abi::{
    derive_fixed_scalar_function_abi, derive_mixed_structural_scalar_function_abi,
};
use crate::LoweringError;
use crate::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractOperation, AbstractOperationPlan};
use installation_evidence::ProviderInstallationEvidence;
use installed_provider_calls::validate_installed_provider_calls;
use settlement_roster::validate_settlement_roster;
use std::collections::BTreeMap;
use target::NativeTarget;
use target_operations::{BoundarySettlementBinding, TargetFunction, TargetOperationPlan};

mod installed_provider_calls {
    use crate::LoweringError;
    use abstract_operations::{AbstractOperation, AbstractOperationPlan};
    use installation_evidence::{
        InstalledProviderCallEvidence, InstalledProviderCompletionClaimSource,
        ProviderInstallationEvidence,
    };
    use semantic_vocabulary::{BoundaryMachineId, InvocationSite, MachineId};
    use std::collections::BTreeMap;

    pub(crate) type BoundaryCallKey = (MachineId, InvocationSite, BoundaryMachineId);

    pub(crate) type InstalledCallsByCall = BTreeMap<BoundaryCallKey, InstalledProviderCallEvidence>;

    pub(crate) type BoundaryCallsByKey<'a> =
        BTreeMap<(MachineId, InvocationSite, BoundaryMachineId), &'a AbstractOperation>;

    pub(crate) fn index_installed_provider_calls(
        plan: &AbstractOperationPlan,
        installation: Option<&dyn ProviderInstallationEvidence>,
    ) -> Result<InstalledCallsByCall, LoweringError> {
        let installed_calls = installation
            .map(|installation| {
                if installation.psi() != plan.psi {
                    return Err(LoweringError::ProviderInstallationIdentityMismatch);
                }
                Ok(installation.installed_provider_calls())
            })
            .transpose()?
            .unwrap_or_default();
        let mut installed_by_call = InstalledCallsByCall::new();
        for installed in installed_calls {
            let key = (
                installed.caller,
                installed.invocation_site,
                installed.boundary,
            );
            if installed_by_call.insert(key, installed).is_some() {
                return Err(LoweringError::DuplicateSelectedBoundaryCall {
                    machine: key.0,
                    operation: key.1,
                    boundary: key.2,
                });
            }
        }
        Ok(installed_by_call)
    }

    pub(crate) fn index_boundary_calls(plan: &AbstractOperationPlan) -> BoundaryCallsByKey<'_> {
        plan.functions
            .iter()
            .flat_map(|function| {
                function
                    .operations
                    .iter()
                    .filter_map(move |operation| match operation {
                        AbstractOperation::BoundaryCall {
                            psi_operation,
                            boundary,
                            ..
                        } => Some((
                            (function.machine, (*psi_operation).into(), *boundary),
                            operation,
                        )),
                        AbstractOperation::ExternalCompletion {
                            psi_edge, boundary, ..
                        } => Some((
                            (
                                function.machine,
                                InvocationSite::ExternalCompletion(*psi_edge),
                                *boundary,
                            ),
                            operation,
                        )),
                        _ => None,
                    })
            })
            .collect()
    }

    pub(crate) fn validate_installed_provider_calls(
        plan: &AbstractOperationPlan,
        installed_by_call: &InstalledCallsByCall,
        boundary_calls: &BoundaryCallsByKey<'_>,
    ) -> Result<(), LoweringError> {
        for (key, installed) in installed_by_call {
            if let Some(AbstractOperation::ExternalCompletion {
                arguments,
                structural_arguments,
                kind,
                ..
            }) = boundary_calls.get(key).copied()
            {
                if installed.result != terminal_psi::OperationResult::Unit
                    || installed.scalar_arguments != *arguments
                    || installed.structural_arguments != *structural_arguments
                    || !installed.completion_claim_sources.is_empty()
                    || !installed.completion_receipts.is_empty()
                    || installed.provider.boundary != key.2
                    || !plan.provider_candidates.contains(&installed.provider)
                    || !plan.boundary_machine(key.2).is_some_and(|declaration| {
                        declaration.result.is_unit()
                            && declaration.completion
                                == semantic_vocabulary::BoundaryCompletion::External(*kind)
                    })
                {
                    return Err(LoweringError::SelectedBoundaryCallEvidenceMismatch {
                        machine: key.0,
                        operation: key.1,
                        boundary: key.2,
                    });
                }
                continue;
            }
            let Some(AbstractOperation::BoundaryCall {
                result,
                arguments,
                structural_arguments,
                completion_claim_sources,
                completion_receipts,
                ..
            }) = boundary_calls.get(key).copied()
            else {
                return Err(LoweringError::UnknownSelectedBoundaryCall {
                    machine: key.0,
                    operation: key.1,
                    boundary: key.2,
                });
            };
            let exact_sources = completion_claim_sources
                .iter()
                .map(|source| InstalledProviderCompletionClaimSource {
                    claim: source.claim,
                    entry: source.entry.clone(),
                    content: source.content.clone(),
                })
                .collect::<Vec<_>>();
            let exact_result = match (result, &installed.result) {
                (
                    abstract_operations::AbstractBoundaryResult::Unit,
                    terminal_psi::OperationResult::Unit,
                ) => true,
                (
                    abstract_operations::AbstractBoundaryResult::Scalar(actual),
                    terminal_psi::OperationResult::Scalar(expected),
                ) => actual.value == expected.id && actual.scalar_type == expected.scalar_type,
                (
                    abstract_operations::AbstractBoundaryResult::Structural(actual),
                    terminal_psi::OperationResult::Structural(expected),
                ) => actual == expected,
                _ => false,
            };
            let declared_result_matches = plan.boundary_machine(key.2).is_some_and(|declaration| {
                match (result, &declaration.result) {
                    (
                        abstract_operations::AbstractBoundaryResult::Unit,
                        terminal_psi::BoundaryMachineResult::Unit,
                    ) => true,
                    (
                        abstract_operations::AbstractBoundaryResult::Scalar(actual),
                        terminal_psi::BoundaryMachineResult::Scalar(expected),
                    ) => actual.scalar_type == expected.scalar_type,
                    (
                        abstract_operations::AbstractBoundaryResult::Structural(actual),
                        terminal_psi::BoundaryMachineResult::Structural(expected),
                    ) => {
                        actual.structural_type == expected.structural_type
                            && actual.multiplicity == expected.multiplicity
                            && actual.qualifications == expected.qualifications
                            && actual.projected_qualifications == expected.projected_qualifications
                    }
                    _ => false,
                }
            });
            if !exact_result
                || !declared_result_matches
                || installed.scalar_arguments != *arguments
                || installed.structural_arguments != *structural_arguments
                || installed.completion_claim_sources != exact_sources
                || installed.completion_receipts != *completion_receipts
                || if let Some(assumption) = &installed.assumption {
                    !assumption.matches(key.2, &installed.provider)
                } else {
                    installed.provider.boundary != key.2
                }
                || !plan
                    .provider_candidates
                    .iter()
                    .any(|candidate| candidate == &installed.provider)
            {
                return Err(LoweringError::SelectedBoundaryCallEvidenceMismatch {
                    machine: key.0,
                    operation: key.1,
                    boundary: key.2,
                });
            }
        }
        Ok(())
    }
}
pub(crate) mod native_callbacks {
    use crate::LoweringError;
    use abstract_operations::{AbstractOperation, AbstractOperationPlan};
    use calling_conventions::validate_boundary_entry_plan_with_callback_materializations;
    use calling_conventions::{CallSignature, NativePlace, ValueShape};
    use semantic_vocabulary::OperationId;
    use std::collections::BTreeMap;
    use target::NativeTarget;

    pub(crate) fn bind_native_callback_arguments(
        plan: &AbstractOperationPlan,
        target: NativeTarget,
        admitted_rows: &[crate::AdmittedNativeCallbackArgument],
    ) -> Result<
        BTreeMap<OperationId, Vec<target_operations::TargetNativeCallbackArgument>>,
        LoweringError,
    > {
        let mut native_callbacks_by_operation: BTreeMap<
            OperationId,
            Vec<target_operations::TargetNativeCallbackArgument>,
        > = BTreeMap::new();
        for admitted in admitted_rows {
            if admitted_rows
                .iter()
                .filter(|row| {
                    row.terminal_operation == admitted.terminal_operation
                        && row.placement_index == admitted.placement_index
                })
                .count()
                != 1
            {
                return Err(LoweringError::DuplicateNativeCallbackArgument(
                    admitted.terminal_operation,
                ));
            }
            let matching_calls = plan
                .operations()
                .filter(|operation| {
                    matches!(operation,
                    AbstractOperation::BoundaryCall { psi_operation, .. }
                        if *psi_operation == admitted.terminal_operation)
                })
                .count();
            if matching_calls != 1 {
                return Err(LoweringError::UnknownNativeCallbackArgument(
                    admitted.terminal_operation,
                ));
            }
            if admitted.callback_function.callback_thunk_placement_index()
                != Some(admitted.placement_index)
                || !admitted.callback_function.is_valid()
                || admitted.registrar_application_commitment == [0; 32]
                || !native_callback_application_is_exact(admitted, target)
            {
                return Err(LoweringError::InvalidNativeCallbackArgument(
                    admitted.terminal_operation,
                ));
            }
            native_callbacks_by_operation
                .entry(admitted.terminal_operation)
                .or_default()
                .push(target_operations::TargetNativeCallbackArgument {
                    registrar_invokes_callback: admitted.registrar_invokes_callback,
                    terminal_operation: admitted.terminal_operation,
                    placement_index: admitted.placement_index,
                    callback_function: admitted.callback_function,
                    destination: admitted.destination.clone(),
                    field_placements: admitted.field_placements.clone(),
                    application: admitted.application.clone(),
                    registrar_boundary_entry_plan: admitted.registrar_boundary_entry_plan.clone(),
                    registrar_context: admitted.registrar_context.clone(),
                    registrar_application_commitment: admitted.registrar_application_commitment,
                });
        }
        for callbacks in native_callbacks_by_operation.values_mut() {
            callbacks.sort_by_key(|callback| callback.placement_index);
        }
        Ok(native_callbacks_by_operation)
    }

    fn native_callback_application_is_exact(
        admitted: &crate::AdmittedNativeCallbackArgument,
        target: NativeTarget,
    ) -> bool {
        if admitted.registrar_application.target != target {
            return false;
        }
        let application = &admitted.application;
        let Ok(ordinal) = usize::try_from(application.native_ordinal) else {
            return false;
        };
        let expected_shape = u16::try_from(target.pointer_size)
            .ok()
            .zip(u16::try_from(target.pointer_alignment).ok())
            .map(|(size, alignment)| ValueShape::integer(u32::from(size), alignment));
        let signature = CallSignature {
            parameters: admitted
                .registrar_boundary_entry_plan
                .call
                .parameters
                .iter()
                .map(|placement| placement.shape)
                .collect(),
            result: admitted
                .registrar_boundary_entry_plan
                .call
                .result
                .as_ref()
                .map(|result| result.shape),
        };
        let Ok(validated) = validate_boundary_entry_plan_with_callback_materializations(
            admitted.registrar_boundary_entry_plan.clone(),
            &signature,
            &admitted.registrar_context,
        ) else {
            return false;
        };
        if validated.plan() != &admitted.registrar_boundary_entry_plan
            || application.shape != application.placement.shape
            || Some(application.shape) != expected_shape
            || admitted
                .registrar_boundary_entry_plan
                .call
                .parameters
                .get(ordinal)
                != Some(&application.placement)
        {
            return false;
        }
        let materializations = admitted
            .registrar_boundary_entry_plan
            .call
            .callback_materializations
            .iter()
            .filter(|row| row.destination == admitted.destination)
            .collect::<Vec<_>>();
        let [materialization] = materializations.as_slice() else {
            return false;
        };
        let binders = admitted
            .registrar_context
            .binders
            .iter()
            .filter(|binder| binder.binder == materialization.binder)
            .collect::<Vec<_>>();
        let [binder] = binders.as_slice() else {
            return false;
        };
        let demands = admitted
            .registrar_context
            .demands
            .iter()
            .filter(|demand| {
                demand.destination == materialization.destination
                    && demand.requirement == binder.requirement
            })
            .collect::<Vec<_>>();
        if demands.len() != 1 {
            return false;
        }
        match &admitted.destination {
            NativePlace::Parameter(parameter) => {
                admitted.field_placements.is_none() && *parameter == application.parameter
            }
            NativePlace::Field { parameter, .. } => {
                admitted.field_placements.is_some() && *parameter == application.parameter
            }
        }
    }
}
mod settlement_roster {
    use super::installed_provider_calls::BoundaryCallsByKey;
    use crate::LoweringError;
    use crate::selected_boundary_calls::SelectedCallsByCall;
    use abstract_operations::AbstractOperationPlan;
    use semantic_vocabulary::BoundaryMachineId;
    use std::collections::{BTreeMap, BTreeSet};
    use target_operations::BoundarySettlementBinding;

    pub(crate) type SettlementsByBoundary = BTreeMap<BoundaryMachineId, BoundarySettlementBinding>;

    pub(crate) fn index_settlement_bindings(
        plan: &AbstractOperationPlan,
        settlement_bindings: &[BoundarySettlementBinding],
    ) -> Result<SettlementsByBoundary, LoweringError> {
        let mut settlements_by_boundary = SettlementsByBoundary::new();
        for binding in settlement_bindings {
            if settlements_by_boundary
                .insert(binding.boundary, binding.clone())
                .is_some()
            {
                return Err(LoweringError::DuplicateBoundarySettlement(binding.boundary));
            }
            if !plan
                .boundary_machines
                .iter()
                .any(|boundary| boundary.id == binding.boundary)
            {
                return Err(LoweringError::UnknownBoundarySettlement(binding.boundary));
            }
        }
        Ok(settlements_by_boundary)
    }

    pub(crate) fn validate_settlement_roster(
        settlements_by_boundary: &SettlementsByBoundary,
        installed_by_call: &SelectedCallsByCall,
        boundary_calls: &BoundaryCallsByKey<'_>,
    ) -> Result<(), LoweringError> {
        let installed_boundaries = installed_by_call
            .keys()
            .map(|(_, _, boundary)| *boundary)
            .collect::<BTreeSet<_>>();
        if let Some(boundary) = settlements_by_boundary
            .keys()
            .find(|boundary| installed_boundaries.contains(boundary))
        {
            return Err(LoweringError::BoundarySettlementOverlapsInstalledProvider(
                *boundary,
            ));
        }
        if let Some((machine, operation, boundary)) = boundary_calls
            .keys()
            .find(|key| {
                installed_boundaries.contains(&key.2) && !installed_by_call.contains_key(key)
            })
            .copied()
        {
            return Err(LoweringError::PartialInstalledProviderBoundary {
                machine,
                invocation_site: operation,
                boundary,
            });
        }
        let required_settlements = boundary_calls
            .keys()
            .filter_map(|key| (!installed_by_call.contains_key(key)).then_some(key.2))
            .collect::<BTreeSet<_>>();
        for boundary in &required_settlements {
            if !settlements_by_boundary.contains_key(boundary) {
                return Err(LoweringError::MissingBoundarySettlement(*boundary));
            }
        }
        if let Some(extra) = settlements_by_boundary
            .keys()
            .find(|boundary| !required_settlements.contains(boundary))
        {
            return Err(LoweringError::UnusedBoundarySettlement(*extra));
        }
        Ok(())
    }
}

pub(crate) fn lower_to_target_operations_with_settlements_and_installation(
    plan: &AbstractOperationPlan,
    target: NativeTarget,
    settlement_bindings: &[BoundarySettlementBinding],
    installation: Option<&dyn ProviderInstallationEvidence>,
    native_callbacks: &[crate::AdmittedNativeCallbackArgument],
    verified_input: Option<&verified_optimization_input::AdmittedLoweringFacts>,
) -> Result<TargetOperationPlan, LoweringError> {
    if !plan
        .functions
        .iter()
        .any(|function| function.machine == plan.entry)
    {
        return Err(LoweringError::EntryFunctionMissing(plan.entry));
    }
    let functions_by_machine = plan
        .functions
        .iter()
        .map(|function| (function.machine, function))
        .collect::<BTreeMap<_, _>>();
    let mut scalar_abis = BTreeMap::new();
    for function in &plan.functions {
        if let Some(abi) = derive_fixed_scalar_function_abi(function, target)? {
            scalar_abis.insert(function.machine, abi);
        }
    }
    let structural_types = StructuralTypeLookup::new(&plan.structural_types)
        .map_err(LoweringError::NonDenseStructuralTypes)?;
    let mut mixed_structural_scalar_abis = BTreeMap::new();
    for function in &plan.functions {
        if let Some(abi) =
            derive_mixed_structural_scalar_function_abi(function, target, &structural_types)?
        {
            mixed_structural_scalar_abis.insert(function.machine, abi);
        }
    }
    let boundary_machines = plan
        .boundary_machines
        .iter()
        .map(|boundary| (boundary.id, boundary))
        .collect::<BTreeMap<_, _>>();
    let settlements_by_boundary =
        settlement_roster::index_settlement_bindings(plan, settlement_bindings)?;
    if let Some(operation) = plan
        .operations()
        .filter_map(|operation| match operation {
            AbstractOperation::NearestIeeeFloatFusedMultiplyAdd { psi_operation, .. } => {
                Some(*psi_operation)
            }
            _ => None,
        })
        .min()
    {
        return Err(LoweringError::UnsupportedIeeeFloatFmaRealization(operation));
    }
    let installed_by_call =
        installed_provider_calls::index_installed_provider_calls(plan, installation)?;
    let boundary_calls = installed_provider_calls::index_boundary_calls(plan);
    let native_callbacks_by_operation =
        native_callbacks::bind_native_callback_arguments(plan, target, native_callbacks)?;
    validate_installed_provider_calls(plan, &installed_by_call, &boundary_calls)?;
    let installed_by_call =
        super::selected_boundary_calls::index(plan, installed_by_call, verified_input)?;
    validate_settlement_roster(
        &settlements_by_boundary,
        &installed_by_call,
        &boundary_calls,
    )
    .inspect_err(|error| {
        if std::env::var_os("OMEGA_TRACE_NATIVE_BOUNDARY").is_some()
            && let LoweringError::MissingBoundarySettlement(boundary) = error
            && let Some(declaration) = boundary_machines.get(boundary)
        {
            let installations = verified_input.map(|input| input.module().installed_interrupt_registrations.iter()
                .filter(|registration| registration.installer == *boundary)
                .map(|registration| (registration.operation, registration.registrar_application.as_str(), registration.callbacks.len()))
                .collect::<Vec<_>>());
            eprintln!("checked interrupt installation records: {installations:?}");
            if let Some(input) = verified_input {
                let module = input.module();
                for registration in module.installed_interrupt_registrations.iter().filter(|registration| registration.installer == *boundary) {
                    let helpers = module.callback_applications.iter().filter(|application| application.registrar == registration.operation)
                        .map(|application| (application.static_machine_ordinal, application.entry_admitted, application.requirement.as_str(), &application.body)).collect::<Vec<_>>();
                    eprintln!("installed operation helper selections: {helpers:?}");
                    eprintln!("installed registrar ABI: {:?}", module.calling_abi_identity(registration.registrar_boundary));
                }
            }

            eprintln!(
                "unsettled native boundary: id={boundary:?} identity={:?} execution={:?} calls={:?}",
                plan.identities.text(declaration.identity),
                declaration.declaration.as_ref().map(|callable| &callable.execution),
                boundary_calls.keys().filter(|key| key.2 == *boundary).collect::<Vec<_>>(),
            );
        }
    })?;
    let borrowed_reference_sources = super::control_flow::references::borrowed_source_catalog(
        &functions_by_machine,
        &structural_types,
    );
    let stored_reference_effects = super::control_flow::references::stored_effect_catalog(
        &functions_by_machine,
        &plan.boundary_machines,
        &structural_types,
        &plan.dynamic_dispatch,
        &installed_by_call,
        &settlements_by_boundary,
    );
    let target_plan = TargetOperationPlan {
        borrowed_storage_layouts: super::borrowed_storage::capture(plan, &structural_types),
        psi: plan.psi,
        target,
        entry: plan.entry,
        native_callback_arguments: native_callbacks_by_operation
            .values()
            .flat_map(|callbacks| callbacks.iter().cloned())
            .collect(),
        functions: plan
            .functions
            .iter()
            .map(|function| {
                let mut lowered = lower_function(
                    function,
                    target,
                    &functions_by_machine,
                    &plan.dynamic_dispatch,
                    borrowed_reference_sources
                        .get(&function.machine)
                        .cloned()
                        .unwrap_or_default(),
                    stored_reference_effects.clone(),
                    &scalar_abis,
                    &structural_types,
                    &boundary_machines,
                    &settlements_by_boundary,
                    &installed_by_call,
                    &native_callbacks_by_operation,
                    verified_input.map(|input| input.installed_interrupts()),
                ).inspect_err(|error| {
                    if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some()
                        && matches!(error, LoweringError::InvalidStructuralValueInput { reason, .. }
                            if *reason == "view discard has no affine descriptor carrier")
                    {
                        let declaration = verified_input.and_then(|input| {
                            let module = input.module();
                            let declaration = module.machine(function.machine)?.declaration.as_ref()?;
                            Some((
                                module.identities.text(declaration.machine.identity()),
                                module.identities.text(declaration.state.identity()),
                                module.identities.text(declaration.callable),
                            ))
                        });
                        eprintln!(
                            "descriptor-discard catalog machine={:?} entry={:?} declaration={declaration:?} result={:?}",
                            function.machine, function.entry, function.result,
                        );
                    }
                })?;
                lowered.scalar_abi = scalar_abis.get(&function.machine).cloned();
                lowered.mixed_structural_scalar_abi =
                    mixed_structural_scalar_abis.get(&function.machine).cloned();
                Ok::<TargetFunction, LoweringError>(lowered)
            })
            .collect::<Result<Vec<_>, _>>()?,
    };
    Ok(target_plan)
}
