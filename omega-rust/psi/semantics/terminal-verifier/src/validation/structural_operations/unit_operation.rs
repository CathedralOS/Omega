//! Static validation of one Unit operation.

use crate::validation::structural_operations::boundary_requirements::{
    validate_boundary_completion_receipts, validate_boundary_requirements,
};
use crate::validation::structural_operations::claim_transfers::{
    validate_service_reach, validate_unit_call_claim_transfers,
};
use crate::validation::structural_operations::contract_places::{
    unit_call_contract_propositions, validate_unit_call_contract_places,
};
use crate::validation::structural_operations::crash_continuations::validate_unit_call_crash_continuations;
use crate::validation::structural_operations::payloadless_calls::is_exact_payloadless_structural_call;
use crate::validation::structural_operations::primitive_calls::validate_primitive_structural_call;
use crate::validation::structural_operations::structural_arguments::{
    StructuralArgumentSourcePolicy, is_admitted_unit_call_argument_path,
    is_literal_indexed_field_path, is_structural_call_result, is_unrestricted_mutable_subloan,
    is_unrestricted_shared_subloan, is_unrestricted_write_only_subloan,
    structural_paths_may_overlap, validate_structural_arguments,
};
use crate::validation::{
    BTreeMap, BTreeSet, BoundaryContentGuarantee, MachineId, ModuleError, OperationKind,
    ScalarType, StructuralAccess, StructuralMultiplicity, StructuralPathSegment,
    StructuralPlaceKind, StructuralTypeShape, TerminalMachine, TerminalMachineResult,
    TerminalModule, Terminator, is_partial_affine_path, partial_affine_root_type, propositions,
    resolve_structural_path,
};

pub(crate) fn validate_unit_operation_static(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    operation: &terminal_psi::Operation,
) -> Result<(), ModuleError> {
    if crate::validation::references::validate_call(module, machine, machines, operation)? {
        return Ok(());
    }
    if validate_primitive_structural_call(module, machine, machines, operation)? {
        return Ok(());
    }
    match &operation.kind {
        OperationKind::EstablishReference { source } => {
            crate::validation::references::validate_establishment(
                module, machine, operation, source,
            )?;
        }
        OperationKind::ReleaseReference { source } => {
            if crate::validation::references::carrier_type(module, machine, *source).is_none() {
                return Err(crate::validation::references::invalid(
                    machine,
                    "release source is not a reference carrier",
                ));
            }
        }
        OperationKind::EstablishScalarArray { .. } => {
            crate::validation::scalar_array::shape(module, machine, operation)?;
        }
        OperationKind::WriteOnlyPrimitiveStore {
            destination, path, ..
        } => {
            crate::validation::primitive_storage::store_type(
                module,
                machine,
                operation.id,
                *destination,
                path,
            )?;
        }
        OperationKind::EstablishPrimitiveLocal { .. } => {
            crate::validation::primitive_storage::validate_establishment(
                module, machine, operation,
            )?;
        }
        OperationKind::StructuralScalarFieldStore {
            destination,
            path,
            field,
            ..
        } => {
            crate::validation::structural_scalar_fields::structural_scalar_field_store_type(
                module,
                machine,
                operation.id,
                *destination,
                path,
                *field,
            )?;
        }
        OperationKind::ByteSequenceWrite { .. } => {
            crate::validation::byte_sequence_write::validate(module, machine, operation)?;
        }
        OperationKind::StructuralByteSequenceFieldByteStore { .. } => {
            crate::validation::structural_byte_sequence_fields::validate(
                module, machine, operation,
            )?;
        }
        OperationKind::StructuralByteSequenceFieldStore { .. } => {
            crate::validation::structural_byte_sequence_store::capacity(
                module, machine, operation,
            )?;
        }
        OperationKind::EstablishScalarCase { .. } => {
            crate::validation::scalar_case::fields(module, machine, operation)?;
        }
        OperationKind::EstablishRecord { .. } => {
            crate::validation::record::fields(module, machine, operation)?;
        }
        OperationKind::CallUnit {
            callee,
            arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => {
            let callee = machines
                .get(callee)
                .copied()
                .ok_or(ModuleError::UnknownCallTarget {
                    operation: operation.id,
                    callee: *callee,
                })?;
            if callee.result != TerminalMachineResult::Unit {
                return Err(ModuleError::UnitCallTargetHasScalarSignature {
                    operation: operation.id,
                    callee: callee.id,
                });
            }
            if structural_arguments
                .iter()
                .any(|argument| !is_admitted_unit_call_argument_path(module, machine, argument))
            {
                return Err(ModuleError::InvalidStructuralArgumentPath {
                    operation: operation.id,
                    argument_index: structural_arguments
                        .iter()
                        .position(|argument| {
                            !is_admitted_unit_call_argument_path(module, machine, argument)
                        })
                        .unwrap_or_default() as u32,
                });
            }
            let projected = structural_arguments.iter().any(|argument| {
                !argument.path.is_empty()
                    && !crate::validation::references::is_reference_projection(
                        module, machine, argument,
                    )
            });
            // Independent borrowed projections retain each root's authority;
            // sibling argument counts do not create residual owned custody.
            let ordinary_borrowed_projections = structural_arguments.len()
                == callee.structural_parameters.len()
                && structural_arguments
                    .iter()
                    .zip(&callee.structural_parameters)
                    .all(|(argument, parameter)| {
                        argument.path.is_empty()
                            || crate::validation::references::is_reference_projection(
                                module, machine, argument,
                            )
                            || is_unrestricted_write_only_subloan(
                                module, machine, parameter, argument,
                            )
                            || is_unrestricted_shared_subloan(machine, parameter, argument)
                            || is_unrestricted_mutable_subloan(machine, parameter, argument)
                    });
            let result_projection = structural_arguments.iter().any(|argument| {
                argument.access == StructuralAccess::Owned
                    && !argument.path.is_empty()
                    && is_structural_call_result(machine, argument.place)
                    && partial_affine_root_type(machine, argument.place).is_some()
            });
            // Scalar inputs are independent of this result's residual custody.
            // Only the Jump route validates their transport across cleanup;
            // retain the separate final-return and parameter-root limits.
            let scalar_result_continuation = !machine.parameters.is_empty()
                && result_projection
                && machine.blocks.iter().any(|block| {
                    matches!(block.terminator, Terminator::Jump { .. })
                        && block
                            .operations
                            .iter()
                            .any(|candidate| candidate.id == operation.id)
                });
            if projected
                && !ordinary_borrowed_projections
                && ((machine.result != TerminalMachineResult::Unit)
                    || (!machine.parameters.is_empty() && !scalar_result_continuation)
                    || (!result_projection && machine.structural_parameters.len() != 1)
                    || structural_arguments.len() != 1
                    || callee.structural_parameters.len() != 1)
            {
                return Err(ModuleError::ProjectedUnitCallOutsideBoundedSlice {
                    operation: operation.id,
                });
            }
            validate_structural_arguments(
                module,
                machine,
                structural_arguments,
                &callee.structural_parameters,
                operation.id,
                true,
                StructuralArgumentSourcePolicy::ParametersOrAffineLocalsAndCallResults,
            )?;
            if let Some(argument_index) = structural_arguments
                .iter()
                .zip(&callee.structural_parameters)
                .position(|(argument, expected)| {
                    (is_literal_indexed_field_path(&argument.path)
                        || (argument.path.iter().any(|segment| {
                            matches!(segment, StructuralPathSegment::FixedIndex(_))
                        }) && argument.access == StructuralAccess::WriteOnlyBorrow))
                        && !is_unrestricted_write_only_subloan(module, machine, expected, argument)
                        && !is_unrestricted_shared_subloan(machine, expected, argument)
                        && !is_unrestricted_mutable_subloan(machine, expected, argument)
                        && !(argument.access == StructuralAccess::Owned
                            && expected.multiplicity == StructuralMultiplicity::Affine
                            && partial_affine_root_type(machine, argument.place).is_some_and(
                                |root_type| {
                                    is_partial_affine_path(module, root_type, &argument.path)
                                },
                            ))
                })
            {
                return Err(ModuleError::InvalidStructuralArgumentPath {
                    operation: operation.id,
                    argument_index: argument_index as u32,
                });
            }
            if let Some((argument_index, _)) = structural_arguments
                .iter()
                .zip(&callee.structural_parameters)
                .enumerate()
                .find(|(_, (argument, expected))| {
                    !argument.path.is_empty()
                        && (!expected.qualifications.is_empty()
                            || machine
                                .structural_parameters
                                .iter()
                                .find(|actual| actual.place == argument.place)
                                .is_some_and(|actual| !actual.qualifications.is_empty()))
                })
            {
                return Err(ModuleError::InvalidStructuralArgumentPath {
                    operation: operation.id,
                    argument_index: argument_index as u32,
                });
            }
            validate_unit_call_contract_places(callee, operation.id)?;
            if projected {
                for (argument, parameter) in structural_arguments
                    .iter()
                    .zip(&callee.structural_parameters)
                {
                    if argument.path.is_empty()
                        || crate::validation::references::is_reference_projection(
                            module, machine, argument,
                        )
                    {
                        continue;
                    }
                    let projected_parameter = parameter.place;
                    if unit_call_contract_propositions(callee).any(|proposition| {
                        propositions::proposition_content_roots(proposition)
                            .contains(&projected_parameter)
                    }) {
                        return Err(
                            ModuleError::ProjectedUnitCallContractUsesStructuralParameter {
                                operation: operation.id,
                                callee: callee.id,
                                place: projected_parameter,
                            },
                        );
                    }
                }
            }
            validate_service_reach(
                operation.id,
                &machine.published_service_ceiling,
                &callee.published_service_ceiling,
            )?;
            if requirement_obligations.len() != callee.contract.requires.len() {
                return Err(ModuleError::CallRequirementArityMismatch {
                    operation: operation.id,
                    expected: callee.contract.requires.len(),
                    actual: requirement_obligations.len(),
                });
            }
            validate_unit_call_claim_transfers(
                module,
                machine,
                callee,
                structural_arguments,
                claim_transfers,
                operation.id,
            )?;
            validate_unit_call_crash_continuations(
                module,
                machine,
                callee,
                arguments,
                structural_arguments,
                crash_continuations,
                operation.id,
            )?;
        }
        OperationKind::CallStructuralScalar {
            callee,
            arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => {
            let callee = machines
                .get(callee)
                .copied()
                .ok_or(ModuleError::UnknownCallTarget {
                    operation: operation.id,
                    callee: *callee,
                })?;
            let expected = callee.result.scalar().map(|result| result.scalar_type);
            let actual = operation.result.scalar().map(|result| result.scalar_type);
            if expected.is_none() || actual != expected {
                return Err(ModuleError::StructuralScalarCallTargetMismatch {
                    operation: operation.id,
                    callee: callee.id,
                    expected,
                    actual,
                });
            }
            // Exact completed-record receiver custody is independent of scalar return shape.
            validate_structural_arguments(
                module,
                machine,
                structural_arguments,
                &callee.structural_parameters,
                operation.id,
                true,
                StructuralArgumentSourcePolicy::ParametersOrAffineLocalsAndCallResults,
            )?;
            validate_unit_call_contract_places(callee, operation.id)?;
            validate_service_reach(
                operation.id,
                &machine.published_service_ceiling,
                &callee.published_service_ceiling,
            )?;
            if requirement_obligations.len() != callee.contract.requires.len() {
                return Err(ModuleError::CallRequirementArityMismatch {
                    operation: operation.id,
                    expected: callee.contract.requires.len(),
                    actual: requirement_obligations.len(),
                });
            }
            validate_unit_call_claim_transfers(
                module,
                machine,
                callee,
                structural_arguments,
                claim_transfers,
                operation.id,
            )?;
            validate_unit_call_crash_continuations(
                module,
                machine,
                callee,
                arguments,
                structural_arguments,
                crash_continuations,
                operation.id,
            )?;
        }
        OperationKind::CallStructuralWithScalarArguments {
            callee,
            arguments: _,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
        } if operation
            .result
            .structural()
            .is_none_or(|result| result.multiplicity != StructuralMultiplicity::Linear) =>
        {
            let callee = machines
                .get(callee)
                .copied()
                .ok_or(ModuleError::UnknownCallTarget {
                    operation: operation.id,
                    callee: *callee,
                })?;
            let Some(callee_result) = callee.result.structural() else {
                return Err(ModuleError::StructuralCallTargetMismatch {
                    operation: operation.id,
                    callee: callee.id,
                });
            };
            let Some(result) = operation.result.structural() else {
                return Err(ModuleError::StructuralCallResultMismatch(operation.id));
            };
            let [callee_parameter] = callee.structural_parameters.as_slice() else {
                return Err(ModuleError::StructuralCallTargetMismatch {
                    operation: operation.id,
                    callee: callee.id,
                });
            };
            let [argument] = structural_arguments.as_slice() else {
                return Err(ModuleError::StructuralCallTargetMismatch {
                    operation: operation.id,
                    callee: callee.id,
                });
            };
            let exact_return = matches!(callee.blocks.as_slice(), [block]
            if block.id == callee.entry
                && block.parameters.is_empty()
                && block.operations.is_empty()
                && matches!(
                    &block.terminator,
                    Terminator::ReturnStructural {
                        source,
                        returned_claims,
                        trivial_affine_discards,
                        ..
                    } if *source == callee_parameter.place
                        && returned_claims.is_empty()
                        && trivial_affine_discards.is_empty()
                ));
            if !callee.parameters.iter().all(|parameter| {
                matches!(
                    parameter.scalar_type,
                    ScalarType::Integer(integer)
                        if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                            && matches!(integer.bits(), 8 | 16 | 32 | 64)
                )
            }) || callee_parameter.position != 0
                || callee_parameter.is_self
                || callee_parameter.structural_type != callee_result.structural_type
                || callee_parameter.multiplicity != StructuralMultiplicity::Affine
                || callee_parameter.access != StructuralAccess::Owned
                || !callee_parameter.qualifications.is_empty()
                || !callee_parameter.projected_qualifications.is_empty()
                || !argument.path.is_empty()
                || argument.access != StructuralAccess::Owned
                || result.structural_type != callee_result.structural_type
                || result.multiplicity != StructuralMultiplicity::Affine
                || result.multiplicity != callee_result.multiplicity
                || !crate::validation::structural_result_contracts::call_result_matches(
                    result,
                    callee_result,
                )
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || !result.claims.is_empty()
                || !claim_transfers.is_empty()
                || !returned_claim_transfers.is_empty()
                || !requirement_obligations.is_empty()
                || !crash_continuations.is_empty()
                || !callee.entry_claims.is_empty()
                || !callee.content_entry_claims.is_empty()
                || !callee.content_identity_reshuffles.is_empty()
                || !callee.content_partition_compositions.is_empty()
                || !callee.published_service_ceiling.is_empty()
                || !callee.contract.requires.is_empty()
                || !callee.contract.ensures.is_empty()
                || !callee.contract.outcome_specific_ensures.is_empty()
                || !callee.contract.crash_routes.is_empty()
                || !crate::validation::structural_result_contracts::has_plain_owned_shape(
                    module,
                    callee_result.structural_type,
                )
                || !exact_return
            {
                return Err(ModuleError::StructuralCallTargetMismatch {
                    operation: operation.id,
                    callee: callee.id,
                });
            }
            let result_place = machine
                .structural_places
                .iter()
                .find(|place| place.id == result.place);
            if !matches!(
                result_place.map(|place| place.kind),
                Some(StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                }) if producer == operation.id && structural_type == result.structural_type
            ) {
                return Err(ModuleError::StructuralCallResultPlaceMismatch(operation.id));
            }
            validate_structural_arguments(
                module,
                machine,
                structural_arguments,
                &callee.structural_parameters,
                operation.id,
                false,
                StructuralArgumentSourcePolicy::ParametersOrAffineOperationResults,
            )?;
            validate_unit_call_contract_places(callee, operation.id)?;
            validate_service_reach(
                operation.id,
                &machine.published_service_ceiling,
                &callee.published_service_ceiling,
            )?;
        }
        OperationKind::CallStructural {
            callee,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
            ..
        }
        | OperationKind::CallStructuralWithScalarArguments {
            callee,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
            ..
        } => {
            let arguments = match &operation.kind {
                OperationKind::CallStructuralWithScalarArguments { arguments, .. } => {
                    arguments.as_slice()
                }
                _ => &[],
            };
            let selected_evidence = match &operation.kind {
                OperationKind::CallStructural {
                    selected_evidence, ..
                } => selected_evidence.as_slice(),
                _ => &[],
            };
            let callee = machines
                .get(callee)
                .copied()
                .ok_or(ModuleError::UnknownCallTarget {
                    operation: operation.id,
                    callee: *callee,
                })?;
            let Some(callee_result) = callee.result.structural() else {
                return Err(ModuleError::StructuralCallTargetMismatch {
                    operation: operation.id,
                    callee: callee.id,
                });
            };
            let Some(result) = operation.result.structural() else {
                return Err(ModuleError::StructuralCallResultMismatch(operation.id));
            };
            if is_exact_payloadless_structural_call(module, operation, machines) {
                let result_place = machine
                    .structural_places
                    .iter()
                    .find(|place| place.id == result.place);
                if !matches!(
                    result_place.map(|place| place.kind),
                    Some(StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type,
                    }) if producer == operation.id && structural_type == result.structural_type
                ) {
                    return Err(ModuleError::StructuralCallResultPlaceMismatch(operation.id));
                }
                validate_service_reach(
                    operation.id,
                    &machine.published_service_ceiling,
                    &callee.published_service_ceiling,
                )?;
                if !selected_evidence.is_empty()
                    && callee.contract.outcome_specific_ensures.is_empty()
                {
                    return Err(ModuleError::InvalidOutcomeSpecificCallEvidence {
                        caller: machine.id,
                        operation: operation.id,
                    });
                }
                return Ok(());
            }
            if callee.parameters.len() != arguments.len()
                || structural_arguments.len() != 1
                || !structural_arguments[0].path.is_empty()
                || callee.structural_parameters.len() != 1
                || result.structural_type != callee_result.structural_type
                || result.multiplicity != callee_result.multiplicity
                || !crate::validation::structural_result_contracts::call_result_matches(
                    result,
                    callee_result,
                )
                || result.multiplicity != StructuralMultiplicity::Linear
            {
                return Err(ModuleError::StructuralCallTargetMismatch {
                    operation: operation.id,
                    callee: callee.id,
                });
            }
            let result_place = machine
                .structural_places
                .iter()
                .find(|place| place.id == result.place);
            if !matches!(
                result_place.map(|place| place.kind),
                Some(StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                }) if producer == operation.id && structural_type == result.structural_type
            ) {
                return Err(ModuleError::StructuralCallResultPlaceMismatch(operation.id));
            }
            if result
                .qualifications
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
                || result.claims.windows(2).any(|pair| pair[0] >= pair[1])
                || result.claims.is_empty()
                || claim_transfers.is_empty()
                || claim_transfers
                    .iter()
                    .any(|transfer| transfer.argument_index != 0)
                || returned_claim_transfers.is_empty()
                || returned_claim_transfers
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
                || result.claims.iter().any(|binding| {
                    resolve_structural_path(module, result.structural_type, &binding.path).is_none()
                })
                || result.claims.iter().enumerate().any(|(index, binding)| {
                    result.claims[index + 1..]
                        .iter()
                        .any(|other| structural_paths_may_overlap(&binding.path, &other.path))
                })
            {
                return Err(ModuleError::NonCanonicalStructuralOperationResult(
                    operation.id,
                ));
            }
            let callee_claims = callee
                .entry_claims
                .iter()
                .map(|claim| (claim.claim, claim.path.as_slice()))
                .collect::<BTreeMap<_, _>>();
            let result_claims = result
                .claims
                .iter()
                .map(|binding| (binding.claim, binding.path.as_slice()))
                .collect::<BTreeMap<_, _>>();
            let transferred_caller_claims = claim_transfers
                .iter()
                .map(|transfer| transfer.claim)
                .collect::<BTreeSet<_>>();
            let returned_callee_claims = returned_claim_transfers
                .iter()
                .map(|transfer| transfer.callee_claim)
                .collect::<BTreeSet<_>>();
            let returned_caller_claims = returned_claim_transfers
                .iter()
                .map(|transfer| transfer.caller_claim)
                .collect::<BTreeSet<_>>();
            if callee_claims.is_empty()
                || callee_claims.len() != callee.entry_claims.len()
                || result_claims.len() != result.claims.len()
                || returned_callee_claims.len() != returned_claim_transfers.len()
                || returned_caller_claims.len() != returned_claim_transfers.len()
                || returned_callee_claims != callee_claims.keys().copied().collect()
                || returned_caller_claims != result_claims.keys().copied().collect()
                || transferred_caller_claims != result_claims.keys().copied().collect()
                || returned_claim_transfers.iter().any(|transfer| {
                    callee_claims.get(&transfer.callee_claim)
                        != result_claims.get(&transfer.caller_claim)
                })
            {
                return Err(ModuleError::StructuralCallClaimInterfaceMismatch(
                    operation.id,
                ));
            }
            let expected_callee_returns = callee
                .entry_claims
                .iter()
                .map(|claim| claim.claim)
                .collect::<Vec<_>>();
            if callee.blocks.iter().any(|block| {
                matches!(
                    &block.terminator,
                    Terminator::ReturnStructural {
                        returned_claims,
                        ..
                    } if returned_claims != &expected_callee_returns
                )
            }) {
                return Err(ModuleError::StructuralCallClaimInterfaceMismatch(
                    operation.id,
                ));
            }
            validate_structural_arguments(
                module,
                machine,
                structural_arguments,
                &callee.structural_parameters,
                operation.id,
                true,
                StructuralArgumentSourcePolicy::ParametersOrLinearCallResults,
            )?;
            validate_unit_call_contract_places(callee, operation.id)?;
            validate_service_reach(
                operation.id,
                &machine.published_service_ceiling,
                &callee.published_service_ceiling,
            )?;
            if requirement_obligations.len() != callee.contract.requires.len() {
                return Err(ModuleError::CallRequirementArityMismatch {
                    operation: operation.id,
                    expected: callee.contract.requires.len(),
                    actual: requirement_obligations.len(),
                });
            }
            validate_unit_call_claim_transfers(
                module,
                machine,
                callee,
                structural_arguments,
                claim_transfers,
                operation.id,
            )?;
            validate_unit_call_crash_continuations(
                module,
                machine,
                callee,
                arguments,
                structural_arguments,
                crash_continuations,
                operation.id,
            )?;
        }
        OperationKind::BoundaryCall {
            boundary,
            arguments: _,
            structural_arguments,
            completion_receipts,
        } => {
            let boundary = module
                .boundary_machines
                .iter()
                .find(|candidate| candidate.id == *boundary)
                .ok_or(ModuleError::UnknownBoundaryCallTarget {
                    operation: operation.id,
                    boundary: *boundary,
                })?;
            if boundary
                .content_guarantees
                .iter()
                .any(|guarantee| matches!(guarantee, BoundaryContentGuarantee::RetainedBorrow(_)))
            {
                return Err(ModuleError::RetainedBorrowBoundaryIsNotExecutable {
                    operation: operation.id,
                    boundary: boundary.id,
                });
            }
            validate_structural_arguments(
                module,
                machine,
                structural_arguments,
                &boundary.structural_parameters,
                operation.id,
                true,
                StructuralArgumentSourcePolicy::ParametersOrBoundaryActuals,
            )?;
            validate_service_reach(
                operation.id,
                &machine.published_service_ceiling,
                &boundary.published_service_ceiling,
            )?;
            validate_boundary_requirements(machine, boundary, structural_arguments, operation.id)?;
            validate_boundary_completion_receipts(
                machine,
                structural_arguments,
                completion_receipts,
                operation.id,
            )?;
        }
        OperationKind::PortWrite { service, .. } => {
            if !module
                .services
                .iter()
                .any(|candidate| candidate.id == *service)
            {
                return Err(ModuleError::UnknownOperationService {
                    operation: operation.id,
                    service: *service,
                });
            }
            if !machine.published_service_ceiling.contains(service) {
                return Err(ModuleError::OperationServiceOutsidePublishedCeiling {
                    operation: operation.id,
                    service: *service,
                });
            }
        }
        OperationKind::EstablishByteSequenceLiteral { destination, .. } => {
            let Some(place) = machine
                .structural_places
                .iter()
                .find(|place| place.id == *destination)
            else {
                return Err(ModuleError::UnknownByteSequenceLiteral {
                    operation: operation.id,
                    place: *destination,
                });
            };
            let StructuralPlaceKind::ByteSequenceLiteral {
                structural_type, ..
            } = place.kind
            else {
                return Err(ModuleError::UnknownByteSequenceLiteral {
                    operation: operation.id,
                    place: *destination,
                });
            };
            let Some(declaration) = module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == structural_type)
            else {
                return Err(ModuleError::UnknownStructuralType(structural_type));
            };
            if !matches!(
                declaration.shape,
                StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView)
            ) {
                return Err(ModuleError::ByteSequenceLiteralRequiresBorrowedView {
                    operation: operation.id,
                    place: *destination,
                });
            }
        }
        OperationKind::EstablishTrivialAffineLocal { destination } => {
            let Some(place) = machine
                .structural_places
                .iter()
                .find(|place| place.id == *destination)
            else {
                return Err(ModuleError::UnknownTrivialAffineLocal {
                    operation: operation.id,
                    place: *destination,
                });
            };
            let StructuralPlaceKind::TrivialAffineLocal {
                structural_type, ..
            } = place.kind
            else {
                return Err(ModuleError::UnknownTrivialAffineLocal {
                    operation: operation.id,
                    place: *destination,
                });
            };
            let Some(declaration) = module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == structural_type)
            else {
                return Err(ModuleError::UnknownStructuralType(structural_type));
            };
            if !matches!(declaration.shape, StructuralTypeShape::Record { ref fields } if fields.is_empty())
            {
                return Err(ModuleError::TrivialAffineLocalRequiresEmptyRecord {
                    operation: operation.id,
                    place: *destination,
                });
            }
        }
        OperationKind::StoreDynamicDescriptor { .. } => {
            // The dynamic-dispatch validator owns the exact descriptor,
            // selection, aggregate identity, field identity, and ordering.
        }
        _ => unreachable!("caller selects only structural/effect operations"),
    }
    Ok(())
}
