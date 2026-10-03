use crate::unit_validation::operation_contracts::affine_calls::exact_plain_affine_structural_call;
use crate::unit_validation::operation_contracts::boundaries::{
    boundary_completion_matches, boundary_requirements_match,
};
use crate::unit_validation::operation_contracts::claim_transfers::validate_internal_claim_transfers;
use crate::unit_validation::operation_contracts::payloadless_cases::{
    case_establishment_matches, exact_payloadless_structural_call,
    payloadless_selected_evidence_surface_matches, plain_scalar_sum_call,
    validate_structural_call_result,
};
use crate::unit_validation::operation_contracts::records::{
    plain_record_call, record_establishment_matches, zeroed_establishment_matches,
};
use crate::unit_validation::operation_contracts::scalar_arrays::{
    plain_scalar_array_call, scalar_array_establishment_matches,
};
use crate::unit_validation::operation_contracts::structural_access::{
    StructuralProjectionPolicy, structural_arguments_match,
};
use abstract_operations::AbstractOperation as O;
use abstract_operations::AbstractStructuralCallResult;
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
        O::Call { callee, .. } => functions
            .get(callee)
            .is_some_and(|callee| reached_is_published(&callee.published_service_ceiling)),
        O::CallStructural {
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
        O::CallDynamic { dispatch, .. } => match dispatch.realization() {
            Some(realization) => functions
                .get(&realization)
                .is_some_and(|callee| reached_is_published(&callee.published_service_ceiling)),
            None => true,
        },
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
        O::EstablishStructuralArray { .. } => {
            super::scalar_arrays::structural_array_establishment_matches(caller, operation, types)
        }
        O::EstablishScalarArray { .. } => {
            scalar_array_establishment_matches(caller, operation, types)
        }
        O::EstablishCase { .. } => case_establishment_matches(caller, operation, types, domains),
        O::EstablishRecord { .. } => {
            record_establishment_matches(caller, operation, types, domains)
        }
        O::EstablishZeroedStructural { .. } => {
            zeroed_establishment_matches(caller, operation, types)
        }
        O::CallStructural {
            callee,
            structural_arguments,
            claim_transfers,
            result: AbstractStructuralCallResult::Unit,
            ..
        } => functions.get(callee).is_some_and(|callee| {
            structural_arguments_match(
                caller,
                operation.psi_operation(),
                structural_arguments,
                &callee.structural_parameters,
                types,
                domains,
                StructuralProjectionPolicy::Unit,
            ) && validate_internal_claim_transfers(
                caller,
                callee,
                structural_arguments,
                claim_transfers,
            )
        }),
        O::CallStructural {
            callee,
            structural_arguments,
            claim_transfers,
            result: AbstractStructuralCallResult::Scalar(_),
            ..
        } => functions.get(callee).is_some_and(|callee| {
            structural_arguments_match(
                caller,
                operation.psi_operation(),
                structural_arguments,
                &callee.structural_parameters,
                types,
                domains,
                StructuralProjectionPolicy::Projected,
            ) && validate_internal_claim_transfers(
                caller,
                callee,
                structural_arguments,
                claim_transfers,
            )
        }),
        O::CallDynamic { dispatch, .. } => match dispatch {
            abstract_operations::AbstractDynamicDispatch::Rebound(dynamic_dispatch) => functions
                .get(&dynamic_dispatch.dispatch.realization)
                .is_some_and(|callee| {
                    structural_arguments_match(
                        caller,
                        operation.psi_operation(),
                        std::slice::from_ref(&dynamic_dispatch.initial.source),
                        &callee.structural_parameters,
                        types,
                        domains,
                        StructuralProjectionPolicy::Projected,
                    ) && structural_arguments_match(
                        caller,
                        operation.psi_operation(),
                        std::slice::from_ref(&dynamic_dispatch.rebound.source),
                        &callee.structural_parameters,
                        types,
                        domains,
                        StructuralProjectionPolicy::Projected,
                    )
                }),
            abstract_operations::AbstractDynamicDispatch::Stored(dynamic_dispatch) => functions
                .get(&dynamic_dispatch.dispatch.realization)
                .is_some_and(|callee| {
                    structural_arguments_match(
                        caller,
                        operation.psi_operation(),
                        std::slice::from_ref(&dynamic_dispatch.stored.selection.source),
                        &callee.structural_parameters,
                        types,
                        domains,
                        StructuralProjectionPolicy::Projected,
                    )
                }),
            abstract_operations::AbstractDynamicDispatch::Parameter(_) => true,
        },
        // A structural result forwards no descriptors: stage 00 never
        // produces that shape and no lowering admits it.
        O::CallStructural {
            dynamic_arguments,
            result: AbstractStructuralCallResult::Structural { .. },
            ..
        } if !dynamic_arguments.is_empty() => false,
        O::CallStructural {
            result:
                AbstractStructuralCallResult::Structural {
                    result,
                    returned_claim_transfers,
                    ..
                },
            callee,
            structural_arguments,
            claim_transfers,
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
                return reference_call_matches(caller, operation, callee, types, domains);
            }
            structural_arguments_match(
                caller,
                operation.psi_operation(),
                structural_arguments,
                &callee.structural_parameters,
                types,
                domains,
                // Argument custody does not depend on the callee's result
                // producer or carrier family. Replay ordinary projections and
                // literal backing with the same source/access contract.
                StructuralProjectionPolicy::Projected,
            ) && validate_internal_claim_transfers(
                caller,
                callee,
                structural_arguments,
                claim_transfers,
            ) && validate_structural_call_result(
                result,
                callee,
                exact_payloadless_structural_call(operation, callee, types, domains)
                    || plain_scalar_array_call(operation, callee, types)
                    || plain_scalar_sum_call(operation, callee, types)
                    || plain_record_call(operation, callee, types)
                    || exact_plain_affine_structural_call(operation, callee, types),
                claim_transfers,
                returned_claim_transfers,
                types,
            ) && structural_call_contract_surface_matches(operation, callee)
                && payloadless_selected_evidence_surface_matches(operation, callee, types, domains)
                && (result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                    || !terminal_semantics::descriptor_results::is_shared_descriptor(
                        types.values().copied(),
                        result.structural_type,
                    )
                    || descriptor_call_origins_match(caller, result.place, functions, types))
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
                operation.psi_operation(),
                structural_arguments,
                &boundary.structural_parameters,
                types,
                domains,
                StructuralProjectionPolicy::Boundary,
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

/// The call's requirement-obligation and crash-route rows replay the callee's
/// verified contract: one obligation per authored `requires`, exact crash
/// routing. A callee without a verified contract admits only an empty surface.
fn structural_call_contract_surface_matches(
    operation: &O,
    callee: &PsiOptimizationFunction,
) -> bool {
    let O::CallStructural {
        requirement_obligations,
        crash_continuations,
        result: AbstractStructuralCallResult::Structural { .. },
        ..
    } = operation
    else {
        return false;
    };
    match &callee.verified_contract {
        Some(contract) => {
            requirement_obligations.len() == contract.requires.len()
                && *crash_continuations == contract.crash_routes
        }
        None => requirement_obligations.is_empty(),
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
    domains: &BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
) -> bool {
    let O::CallStructural {
        psi_operation,
        result:
            AbstractStructuralCallResult::Structural {
                result,
                returned_claim_transfers,
                ..
            },
        arguments,
        structural_arguments,
        claim_transfers,
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
    let checks = [
        (
            "sig_type",
            result.structural_type == signature.structural_type,
        ),
        ("sig_mult", result.multiplicity == signature.multiplicity),
        (
            "sig_qual",
            result.qualifications == signature.qualifications,
        ),
        (
            "sig_pqual",
            result.projected_qualifications == signature.projected_qualifications,
        ),
        (
            "contains_ref",
            crate::unit_validation::references::contains_reference(types, result.structural_type),
        ),
        (
            "ref_or_construct",
            crate::unit_validation::references::referent(types, result.structural_type)
                .is_some()
                || crate::unit_validation::operation_contracts::constructible(
                    types,
                    result.structural_type,
                )
                // A record referent is equally a whole established carrier:
                // `&Node` binds the constructible record its `Reference`
                // shell names, not a primitive referent.
                || match types.get(&result.structural_type).map(|declaration| {
                    &declaration.shape
                }) {
                    Some(terminal_psi::StructuralTypeShape::Reference { referent, .. }) => {
                        crate::unit_validation::operation_contracts::constructible(
                            types, *referent,
                        )
                    }
                    _ => false,
                },
        ),
        (
            "affine",
            result.multiplicity == terminal_psi::StructuralMultiplicity::Affine,
        ),
        ("claims_empty", result.claims.is_empty()),
        (
            "place_kind",
            caller.structural_places.iter().any(|place| {
                place.id == result.place
                    && place.kind
                        == semantic_vocabulary::StructuralPlaceKind::OperationResult {
                            producer: *psi_operation,
                            structural_type: result.structural_type,
                        }
            }),
        ),
        ("ct_empty", claim_transfers.is_empty()),
        ("rct_empty", returned_claim_transfers.is_empty()),
        ("ec_empty", callee.entry_claims.is_empty()),
        ("cec_empty", callee.content_entry_claims.is_empty()),
        ("args_len", arguments.len() == callee.parameters.len()),
        (
            "req_len",
            requirement_obligations.len() == contract.requires.len(),
        ),
        ("crash", *crash_continuations == contract.crash_routes),
        (
            "args_match",
            structural_arguments_match(
                caller,
                operation.psi_operation(),
                structural_arguments,
                &callee.structural_parameters,
                types,
                domains,
                StructuralProjectionPolicy::Unit,
            ),
        ),
    ];

    checks.iter().all(|(_, ok)| *ok)
}

/// Shared result custody retains its callee body's backing, not just the
/// signature's descriptor type. Reconstruct from current functions so an
/// optimizer cannot turn local storage into an escaping result or substitute
/// an actual argument while preserving only the old call result declaration.
fn descriptor_call_origins_match(
    caller: &PsiOptimizationFunction,
    result: semantic_vocabulary::PlaceId,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> bool {
    let types = types.values().copied().cloned().collect::<Vec<_>>();
    let Some(graph) =
        crate::descriptor_results::optimized_catalog_graph(functions.values().copied(), &types)
    else {
        return false;
    };
    terminal_semantics::descriptor_results::reconstruct_call_origins(
        &types,
        &graph,
        caller.machine,
        result,
    )
    .is_some()
}
