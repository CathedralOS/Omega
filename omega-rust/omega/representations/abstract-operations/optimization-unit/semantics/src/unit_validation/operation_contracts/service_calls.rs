use crate::unit_validation::operation_contracts::affine_calls::exact_plain_affine_structural_call;
use crate::unit_validation::operation_contracts::boundaries::{
    boundary_completion_matches, boundary_requirements_match,
};
use crate::unit_validation::operation_contracts::claim_transfers::validate_internal_claim_transfers;
use crate::unit_validation::operation_contracts::payloadless_cases::{
    exact_payloadless_structural_call, payloadless_selected_evidence_surface_matches,
    plain_scalar_sum_call, scalar_case_establishment_matches, validate_structural_call_result,
};
use crate::unit_validation::operation_contracts::records::{
    plain_record_call, record_establishment_matches,
};
use crate::unit_validation::operation_contracts::scalar_arrays::{
    plain_scalar_array_call, scalar_array_establishment_matches,
};
use crate::unit_validation::operation_contracts::structural_access::{
    StructuralProjectionPolicy, structural_arguments_match,
};
use abstract_operations::AbstractOperation as O;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{
    BoundaryMachineId, MachineId, ServiceId, StructuralDomainId, StructuralTypeId,
};
use std::collections::BTreeMap;

pub(crate) fn operation_service_contract_matches(
    caller: &PsiOptimizationFunction,
    operation: &O,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundaries: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
    services: &BTreeMap<ServiceId, &terminal_psi::ServiceDeclaration>,
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
                        abstract_operations::AbstractDynamicDescriptorSource::Rebound {
                            application,
                            ..
                        }
                        | abstract_operations::AbstractDynamicDescriptorSource::Selection {
                            application,
                            ..
                        } => application.realization_callables.iter().all(|callable| {
                            functions.get(&callable.machine).is_some_and(|realization| {
                                reached_is_published(&realization.published_service_ceiling)
                            })
                        }),
                        abstract_operations::AbstractDynamicDescriptorSource::Parameter(_) => true,
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
        O::CallStoredDynamicScalar {
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
    boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    domains: &BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
) -> bool {
    match operation {
        O::EstablishScalarArray { .. } => {
            scalar_array_establishment_matches(caller, operation, types)
        }
        O::EstablishScalarCase { .. } => {
            scalar_case_establishment_matches(caller, operation, types)
        }
        O::EstablishRecord { .. } => record_establishment_matches(caller, operation, types),
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
                true,
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
                true,
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
        O::CallStoredDynamicScalar {
            dynamic_dispatch, ..
        } => functions
            .get(&dynamic_dispatch.dispatch.realization)
            .is_some_and(|callee| {
                structural_arguments_match(
                    caller,
                    std::slice::from_ref(&dynamic_dispatch.stored.selection.source),
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
            // A call returning reference custody has its own complete rule:
            // the caller instantiates the callee's exact source roster, and
            // claim traffic stays empty because carriers carry no claims.
            if callee.result.structural().is_some_and(|signature| {
                crate::unit_validation::references::contains_reference(
                    types,
                    signature.structural_type,
                )
            }) {
                return reference_call_matches(caller, operation, callee, types);
            }
            structural_arguments_match(
                caller,
                structural_arguments,
                &callee.structural_parameters,
                types,
                if plain_scalar_sum_call(operation, callee, types)
                    || plain_record_call(operation, callee, types)
                {
                    StructuralProjectionPolicy::Unit
                } else {
                    StructuralProjectionPolicy::EmptyOnly
                },
                plain_scalar_sum_call(operation, callee, types)
                    || plain_record_call(operation, callee, types),
            ) && validate_internal_claim_transfers(
                caller,
                callee,
                structural_arguments,
                claim_transfers,
            ) && validate_structural_call_result(
                result,
                callee,
                exact_payloadless_structural_call(operation, callee, types)
                    || plain_scalar_array_call(operation, callee, types)
                    || plain_scalar_sum_call(operation, callee, types)
                    || plain_record_call(operation, callee, types)
                    || exact_plain_affine_structural_call(operation, callee, types),
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

/// A call returning reference custody replays the verified rule exactly: the
/// result is a whole established carrier or constructible record matching the
/// callee signature, no claim traffic crosses, and every structural argument
/// is admitted under the ordinary internal-call contract (which itself admits
/// `.., Referent` projections through their exact primitive parameters).
fn reference_call_matches(
    caller: &PsiOptimizationFunction,
    operation: &O,
    callee: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> bool {
    let O::CallStructural {
        psi_operation,
        result,
        arguments,
        structural_arguments,
        claim_transfers,
        returned_claim_transfers,
        requirement_obligations,
        crash_continuations,
        ..
    } = operation
    else {
        return false;
    };
    let Some(signature) = callee.result.structural().filter(|signature| {
        crate::unit_validation::references::contains_reference(types, signature.structural_type)
    }) else {
        return false;
    };
    let Some(contract) = callee.verified_contract.as_ref() else {
        return false;
    };
    result.structural_type == signature.structural_type
        && result.multiplicity == signature.multiplicity
        && result.qualifications == signature.qualifications
        && result.projected_qualifications == signature.projected_qualifications
        && crate::unit_validation::references::contains_reference(types, result.structural_type)
        && (crate::unit_validation::references::referent(types, result.structural_type).is_some()
            || crate::unit_validation::operation_contracts::constructible(
                types,
                result.structural_type,
            ))
        && result.multiplicity == terminal_psi::StructuralMultiplicity::Affine
        && result.claims.is_empty()
        && caller.structural_places.iter().any(|place| {
            place.id == result.place
                && place.kind
                    == semantic_vocabulary::StructuralPlaceKind::OperationResult {
                        producer: *psi_operation,
                        structural_type: result.structural_type,
                    }
        })
        && claim_transfers.is_empty()
        && returned_claim_transfers.is_empty()
        && callee.entry_claims.is_empty()
        && callee.content_entry_claims.is_empty()
        && arguments.len() == callee.parameters.len()
        && requirement_obligations.len() == contract.requires.len()
        && *crash_continuations == contract.crash_routes
        && structural_arguments_match(
            caller,
            structural_arguments,
            &callee.structural_parameters,
            types,
            StructuralProjectionPolicy::Unit,
            true,
        )
}
