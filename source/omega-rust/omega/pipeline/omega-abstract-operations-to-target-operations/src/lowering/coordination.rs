use super::function::lower_function;
use super::scalar_abi::{
    derive_fixed_integer_scalar_function_abi, derive_mixed_structural_scalar_function_abi,
};
use super::shared::*;

mod projected_qualifications;

pub(crate) fn lower_to_target_operations_with_settlements(
    plan: &AbstractOperationPlan,
    target: NativeTarget,
    settlement_bindings: &[BoundarySettlementBinding],
) -> Result<TargetOperationPlan, LoweringError> {
    Ok(
        lower_to_target_operations_with_settlements_and_installation(
            plan,
            target,
            settlement_bindings,
            None,
            &[],
            &[],
        )?
        .plan,
    )
}

pub(super) fn lower_to_target_operations_with_settlements_and_installation(
    plan: &AbstractOperationPlan,
    target: NativeTarget,
    settlement_bindings: &[BoundarySettlementBinding],
    installation: Option<&dyn ProviderInstallationEvidence>,
    ieee_float_fma: &[crate::AdmittedIeeeFloatFmaSettlement<'_>],
    native_callbacks: &[crate::AdmittedNativeCallbackArgument],
) -> Result<omega_target_operations::TargetOperationPlanWithNativeCallbacks, LoweringError> {
    projected_qualifications::reject_unsupported(plan)?;
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
    let mut fixed_integer_scalar_abis = BTreeMap::new();
    for function in &plan.functions {
        if let Some(abi) = derive_fixed_integer_scalar_function_abi(function, target)? {
            fixed_integer_scalar_abis.insert(function.machine, abi);
        }
    }
    let structural_types = plan
        .structural_types
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect::<BTreeMap<_, _>>();
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
    let mut settlements_by_boundary = BTreeMap::new();
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
    let abstract_fma = plan
        .functions
        .iter()
        .flat_map(|function| &function.operations)
        .filter_map(|operation| match operation {
            AbstractOperation::NearestIeeeFloatFusedMultiplyAdd {
                psi_operation,
                format,
                ..
            } => Some((*psi_operation, *format)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let mut ieee_float_fma_by_operation = BTreeMap::new();
    for settlement in ieee_float_fma {
        if ieee_float_fma_by_operation.contains_key(&settlement.terminal_operation) {
            return Err(LoweringError::DuplicateIeeeFloatFmaSettlement(
                settlement.terminal_operation,
            ));
        }
        let Some(format) = abstract_fma.get(&settlement.terminal_operation) else {
            return Err(LoweringError::UnknownIeeeFloatFmaSettlement(
                settlement.terminal_operation,
            ));
        };
        let expected_slot = match format {
            IeeeFloatFormat::Binary32 => omega_target::X86ScalarFmaSlot::Binary32,
            IeeeFloatFormat::Binary64 => omega_target::X86ScalarFmaSlot::Binary64,
        };
        let expected_selected_requirement = expected_slot.selected_plan_requirement_identity();
        let provider = settlement.provider;
        let plan = settlement.provider_plan;
        if settlement.format != *format
            || settlement.slot != expected_slot
            || target.architecture != Architecture::X86_64
            || !provider.has_canonical_identity()
            || provider.profile().native_target() != target
            || !provider.admits(provider.requirement(), settlement.slot)
            || plan.target != provider.profile().target_name()
            || !matches!(plan.rows.as_slice(), [row]
                if row.requirement_identity == expected_selected_requirement
                    && matches!(row.binding,
                        omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }))
        {
            return Err(LoweringError::InvalidIeeeFloatFmaSettlement(
                settlement.terminal_operation,
            ));
        }
        ieee_float_fma_by_operation.insert(
            settlement.terminal_operation,
            omega_target_operations::TargetX86ScalarFmaSettlement {
                terminal_operation: settlement.terminal_operation,
                provider_plan_report_identity: plan.report_fingerprint(),
                provider_plan_digest: *plan.identity_digest().as_bytes(),
                format: settlement.format,
                slot: settlement.slot,
                provider,
            },
        );
    }
    if let Some(missing) = abstract_fma
        .keys()
        .find(|operation| !ieee_float_fma_by_operation.contains_key(operation))
    {
        return Err(LoweringError::MissingIeeeFloatFmaSettlement(*missing));
    }
    let installed_calls = installation
        .map(|installation| {
            if installation.psi() != plan.psi {
                return Err(LoweringError::ProviderInstallationIdentityMismatch);
            }
            Ok(installation.installed_provider_unit_calls())
        })
        .transpose()?
        .unwrap_or_default();
    let mut installed_by_call = BTreeMap::new();
    for installed in installed_calls {
        let key = (
            installed.caller,
            installed.psi_operation,
            installed.boundary,
        );
        if installed_by_call.insert(key, installed).is_some() {
            return Err(LoweringError::DuplicateInstalledProviderCall {
                machine: key.0,
                operation: key.1,
                boundary: key.2,
            });
        }
    }
    let boundary_calls = plan
        .functions
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
                    } => Some(((function.machine, *psi_operation, *boundary), operation)),
                    _ => None,
                })
        })
        .collect::<BTreeMap<_, _>>();
    let native_callbacks_by_operation =
        bind_native_callback_arguments(plan, target, native_callbacks)?;
    for (key, installed) in &installed_by_call {
        let Some(AbstractOperation::BoundaryCall {
            result,
            arguments,
            structural_arguments,
            completion_claim_sources,
            completion_receipts,
            ..
        }) = boundary_calls.get(key).copied()
        else {
            return Err(LoweringError::UnknownInstalledProviderCall {
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
        if result.is_some()
            || installed.scalar_arguments != *arguments
            || installed.structural_arguments != *structural_arguments
            || installed.completion_claim_sources != exact_sources
            || installed.completion_receipts != *completion_receipts
            || installed.provider.boundary != key.2
            || !plan
                .provider_candidates
                .iter()
                .any(|candidate| candidate == &installed.provider)
        {
            return Err(LoweringError::InstalledProviderCallEvidenceMismatch {
                machine: key.0,
                operation: key.1,
                boundary: key.2,
            });
        }
    }
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
        .find(|key| installed_boundaries.contains(&key.2) && !installed_by_call.contains_key(key))
        .copied()
    {
        return Err(LoweringError::PartialInstalledProviderBoundary {
            machine,
            operation,
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
    let target_plan = TargetOperationPlan {
        psi: plan.psi,
        target,
        entry: plan.entry,
        functions: plan
            .functions
            .iter()
            .map(|function| {
                let mut lowered = lower_function(
                    function,
                    target,
                    &functions_by_machine,
                    &fixed_integer_scalar_abis,
                    &structural_types,
                    &boundary_machines,
                    &settlements_by_boundary,
                    &installed_by_call,
                    &ieee_float_fma_by_operation,
                    &native_callbacks_by_operation,
                )?;
                lowered.fixed_integer_scalar_abi =
                    fixed_integer_scalar_abis.get(&function.machine).cloned();
                lowered.mixed_structural_scalar_abi =
                    mixed_structural_scalar_abis.get(&function.machine).cloned();
                Ok::<TargetFunction, LoweringError>(lowered)
            })
            .collect::<Result<Vec<_>, _>>()?,
    };
    validate_native_callback_target_rows(&target_plan, &native_callbacks_by_operation)?;
    Ok(
        omega_target_operations::TargetOperationPlanWithNativeCallbacks {
            plan: target_plan,
            native_callback_arguments: native_callbacks_by_operation.into_values().collect(),
        },
    )
}

pub(crate) fn bind_native_callback_arguments(
    plan: &AbstractOperationPlan,
    target: NativeTarget,
    admitted_rows: &[crate::AdmittedNativeCallbackArgument],
) -> Result<
    BTreeMap<OperationId, omega_target_operations::TargetNativeCallbackArgument>,
    LoweringError,
> {
    if admitted_rows.len() > 1 {
        for (index, admitted) in admitted_rows.iter().enumerate() {
            if admitted_rows[..index]
                .iter()
                .any(|prior| prior.terminal_operation == admitted.terminal_operation)
            {
                return Err(LoweringError::DuplicateNativeCallbackArgument(
                    admitted.terminal_operation,
                ));
            }
        }
        return Err(LoweringError::MultipleNativeCallbackArguments);
    }
    let mut native_callbacks_by_operation = BTreeMap::new();
    for admitted in admitted_rows {
        if native_callbacks_by_operation.contains_key(&admitted.terminal_operation) {
            return Err(LoweringError::DuplicateNativeCallbackArgument(
                admitted.terminal_operation,
            ));
        }
        let matching_calls = plan
            .functions
            .iter()
            .flat_map(|function| &function.operations)
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
        native_callbacks_by_operation.insert(
            admitted.terminal_operation,
            omega_target_operations::TargetNativeCallbackArgument {
                terminal_operation: admitted.terminal_operation,
                placement_index: admitted.placement_index,
                callback_function: admitted.callback_function,
                application: admitted.application.clone(),
                registrar_boundary_entry_plan: admitted.registrar_boundary_entry_plan.clone(),
                registrar_context: admitted.registrar_context.clone(),
                registrar_application_commitment: admitted.registrar_application_commitment,
            },
        );
    }
    Ok(native_callbacks_by_operation)
}

fn native_callback_application_is_exact(
    admitted: &crate::AdmittedNativeCallbackArgument,
    target: NativeTarget,
) -> bool {
    let application = &admitted.application;
    let Ok(ordinal) = usize::try_from(application.native_ordinal) else {
        return false;
    };
    let expected_shape = u16::try_from(target.pointer_size)
        .ok()
        .zip(u16::try_from(target.pointer_alignment).ok())
        .map(|(size, alignment)| ValueShape::integer(size, alignment));
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
    let Ok(validated) =
        omega_calling_conventions::validate_boundary_entry_plan_with_callback_materializations(
            admitted.registrar_boundary_entry_plan.clone(),
            &signature,
            &admitted.registrar_context,
        )
    else {
        return false;
    };
    let ([binder], [demand], [materialization]) = (
        admitted.registrar_context.binders.as_slice(),
        admitted.registrar_context.demands.as_slice(),
        admitted
            .registrar_boundary_entry_plan
            .call
            .callback_materializations
            .as_slice(),
    ) else {
        return false;
    };
    validated.plan() == &admitted.registrar_boundary_entry_plan
        && application.shape == application.placement.shape
        && Some(application.shape) == expected_shape
        && admitted
            .registrar_boundary_entry_plan
            .call
            .parameters
            .get(ordinal)
            == Some(&application.placement)
        && demand.destination
            == omega_calling_conventions::NativePlace::Parameter(application.parameter)
        && materialization.destination == demand.destination
        && materialization.binder == binder.binder
        && binder.requirement == demand.requirement
}

pub(crate) fn validate_native_callback_target_rows(
    plan: &TargetOperationPlan,
    expected: &BTreeMap<OperationId, omega_target_operations::TargetNativeCallbackArgument>,
) -> Result<(), LoweringError> {
    for (operation, callback) in expected {
        let matches = plan
            .functions
            .iter()
            .filter_map(|function| match &function.operation {
                TargetOperation::UnitBody(body) => Some(body),
                _ => None,
            })
            .flat_map(|body| &body.operations)
            .filter(|candidate| {
                matches!(candidate,
                    TargetUnitOperation::NormalizedForeignCall { psi_operation, binding, .. }
                        if psi_operation == operation
                            && binding.boundary_entry_plan
                                == callback.registrar_boundary_entry_plan)
            })
            .count();
        if matches == 0 {
            return Err(LoweringError::UnusedNativeCallbackArgument(*operation));
        }
        if matches != 1 {
            return Err(LoweringError::InvalidNativeCallbackArgument(*operation));
        }
    }
    Ok(())
}
