use super::function::lower_function;
use super::scalar_abi::derive_fixed_integer_scalar_function_abi;
use super::shared::*;

pub(crate) fn lower_to_target_operations_with_settlements(
    plan: &AbstractOperationPlan,
    target: NativeTarget,
    settlement_bindings: &[BoundarySettlementBinding],
) -> Result<TargetOperationPlan, LoweringError> {
    lower_to_target_operations_with_settlements_and_installation(
        plan,
        target,
        settlement_bindings,
        None,
        &[],
    )
}

pub(super) fn lower_to_target_operations_with_settlements_and_installation(
    plan: &AbstractOperationPlan,
    target: NativeTarget,
    settlement_bindings: &[BoundarySettlementBinding],
    installation: Option<&dyn ProviderInstallationEvidence>,
    ieee_float_fma: &[crate::AdmittedIeeeFloatFmaSettlement<'_>],
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
            || !arguments.is_empty()
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
    Ok(TargetOperationPlan {
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
                )?;
                lowered.fixed_integer_scalar_abi =
                    fixed_integer_scalar_abis.get(&function.machine).cloned();
                Ok::<TargetFunction, LoweringError>(lowered)
            })
            .collect::<Result<Vec<_>, _>>()?,
    })
}
