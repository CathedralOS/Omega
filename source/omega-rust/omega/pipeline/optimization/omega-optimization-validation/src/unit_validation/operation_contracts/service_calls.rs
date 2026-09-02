use super::*;

pub(crate) fn operation_service_contract_matches(
    caller: &PsiOptimizationFunction,
    operation: &O,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundaries: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
    services: &BTreeMap<ServiceId, &psi_terminal::ServiceDeclaration>,
) -> bool {
    let reached_is_published = |reached: &[ServiceId]| {
        reached
            .iter()
            .all(|service| caller.published_service_ceiling.contains(service))
    };
    match operation {
        O::Call { callee, .. }
        | O::CallUnit { callee, .. }
        | O::CallStructuralScalar { callee, .. }
        | O::CallStructural { callee, .. } => functions
            .get(callee)
            .is_some_and(|callee| reached_is_published(&callee.published_service_ceiling)),
        O::CallStructuralScalarWithDynamicArguments {
            callee,
            dynamic_arguments,
            ..
        }
        | O::CallUnitWithDynamicArguments {
            callee,
            dynamic_arguments,
            ..
        } => {
            functions
                .get(callee)
                .is_some_and(|callee| reached_is_published(&callee.published_service_ceiling))
                && dynamic_arguments
                    .iter()
                    .all(|argument| match &argument.source {
                        omega_abstract_operations::AbstractDynamicDescriptorSource::Rebound {
                            application,
                            ..
                        } => application.realization_callables.iter().all(|callable| {
                            functions.get(&callable.machine).is_some_and(|realization| {
                                reached_is_published(&realization.published_service_ceiling)
                            })
                        }),
                        omega_abstract_operations::AbstractDynamicDescriptorSource::Parameter(
                            _,
                        ) => true,
                    })
        }
        O::CallDynamicScalar {
            dynamic_dispatch, ..
        }
        | O::CallDynamicUnit {
            dynamic_dispatch, ..
        } => functions
            .get(&dynamic_dispatch.dispatch.realization)
            .is_some_and(|callee| reached_is_published(&callee.published_service_ceiling)),
        O::BoundaryCall { boundary, .. } => boundaries
            .get(boundary)
            .is_some_and(|boundary| reached_is_published(&boundary.published_service_ceiling)),
        O::PortWrite { service, .. } => {
            services.contains_key(service) && caller.published_service_ceiling.contains(service)
        }
        _ => true,
    }
}

/// Independently reconstruct the structural half of every call contract from
/// verifier-owned module/function catalogs. Call-local source/receipt rows are
/// evidence to compare, never the authority from which the expected contract
/// is inferred.
pub(crate) fn operation_structural_call_contract_matches(
    caller: &PsiOptimizationFunction,
    operation: &O,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
    domains: &BTreeMap<StructuralDomainId, &psi_terminal::StructuralDomainDeclaration>,
) -> bool {
    match operation {
        O::EstablishPayloadlessCase { .. } => {
            payloadless_establishment_matches(caller, operation, types)
        }
        O::EstablishAffineScalarRecord { .. } => {
            affine_scalar_record_establishment_matches(caller, operation, types)
        }
        O::CallUnit {
            callee,
            structural_arguments,
            claim_transfers,
            ..
        } => functions.get(callee).is_some_and(|callee| {
            structural_arguments_match(
                caller,
                structural_arguments,
                &callee.structural_parameters,
                types,
                StructuralProjectionPolicy::Unit,
                false,
            ) && validate_internal_claim_transfers(
                caller,
                callee,
                structural_arguments,
                claim_transfers,
            )
        }),
        O::CallUnitWithDynamicArguments {
            callee,
            structural_arguments,
            claim_transfers,
            ..
        } => functions.get(callee).is_some_and(|callee| {
            structural_arguments_match(
                caller,
                structural_arguments,
                &callee.structural_parameters,
                types,
                StructuralProjectionPolicy::Unit,
                false,
            ) && validate_internal_claim_transfers(
                caller,
                callee,
                structural_arguments,
                claim_transfers,
            )
        }),
        O::CallStructuralScalar {
            callee,
            structural_arguments,
            claim_transfers,
            ..
        } => functions.get(callee).is_some_and(|callee| {
            structural_arguments_match(
                caller,
                structural_arguments,
                &callee.structural_parameters,
                types,
                StructuralProjectionPolicy::Projected,
                false,
            ) && validate_internal_claim_transfers(
                caller,
                callee,
                structural_arguments,
                claim_transfers,
            )
        }),
        O::CallStructuralScalarWithDynamicArguments {
            callee,
            structural_arguments,
            claim_transfers,
            ..
        } => functions.get(callee).is_some_and(|callee| {
            structural_arguments_match(
                caller,
                structural_arguments,
                &callee.structural_parameters,
                types,
                StructuralProjectionPolicy::Projected,
                false,
            ) && validate_internal_claim_transfers(
                caller,
                callee,
                structural_arguments,
                claim_transfers,
            )
        }),
        O::CallDynamicScalar {
            dynamic_dispatch, ..
        }
        | O::CallDynamicUnit {
            dynamic_dispatch, ..
        } => functions
            .get(&dynamic_dispatch.dispatch.realization)
            .is_some_and(|callee| {
                structural_arguments_match(
                    caller,
                    std::slice::from_ref(&dynamic_dispatch.initial.source),
                    &callee.structural_parameters,
                    types,
                    StructuralProjectionPolicy::Projected,
                    false,
                ) && structural_arguments_match(
                    caller,
                    std::slice::from_ref(&dynamic_dispatch.rebound.source),
                    &callee.structural_parameters,
                    types,
                    StructuralProjectionPolicy::Projected,
                    false,
                )
            }),
        O::CallStructural {
            result,
            callee,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            ..
        } => functions.get(callee).is_some_and(|callee| {
            structural_arguments_match(
                caller,
                structural_arguments,
                &callee.structural_parameters,
                types,
                StructuralProjectionPolicy::EmptyOnly,
                false,
            ) && validate_internal_claim_transfers(
                caller,
                callee,
                structural_arguments,
                claim_transfers,
            ) && validate_structural_call_result(
                result,
                callee,
                exact_payloadless_structural_call(operation, callee, types),
                claim_transfers,
                returned_claim_transfers,
                types,
            ) && payloadless_selected_evidence_surface_matches(operation, callee, types)
        }),
        O::BoundaryCall {
            boundary,
            structural_arguments,
            completion_claim_sources,
            completion_receipts,
            ..
        } => boundary_machines.get(boundary).is_some_and(|boundary| {
            structural_arguments_match(
                caller,
                structural_arguments,
                &boundary.structural_parameters,
                types,
                StructuralProjectionPolicy::Boundary,
                true,
            ) && boundary_requirements_match(caller, structural_arguments, boundary, domains)
                && boundary_completion_matches(
                    caller,
                    structural_arguments,
                    completion_claim_sources,
                    completion_receipts,
                )
        }),
        _ => true,
    }
}
