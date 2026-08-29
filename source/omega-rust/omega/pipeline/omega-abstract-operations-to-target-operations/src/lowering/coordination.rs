use super::function::lower_function;
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
    )
}

pub(super) fn lower_to_target_operations_with_settlements_and_installation(
    plan: &AbstractOperationPlan,
    target: NativeTarget,
    settlement_bindings: &[BoundarySettlementBinding],
    installation: Option<&dyn ProviderInstallationEvidence>,
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
            .insert(binding.boundary, *binding)
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
                lower_function(
                    function,
                    target,
                    &functions_by_machine,
                    &structural_types,
                    &boundary_machines,
                    &settlements_by_boundary,
                    &installed_by_call,
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
    })
}
