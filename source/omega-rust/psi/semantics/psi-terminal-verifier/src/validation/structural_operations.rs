//! Validates structural, boundary, and effect operation custody.

use super::*;

/// Recognize the bounded executable guarded-result carrier whose ordinary
/// exits each return an exact, claim-free payloadless case. This direct
/// producer classifier intentionally rejects calls and payload construction;
/// the separate exact caller-import rung consumes this closed leaf shape.
pub(crate) fn exact_payloadless_case_return_exits(
    machine: &TerminalMachine,
) -> Option<BTreeMap<BlockId, psi_terminal::OutcomeSpecificGuard>> {
    let result = machine.result.structural()?;
    if !super::structural_result_contracts::has_empty_qualification_rosters(
        &result.qualifications,
        &result.projected_qualifications,
    ) || result.multiplicity != StructuralMultiplicity::Unrestricted
        || machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::Call { .. }
                        | OperationKind::CallUnit { .. }
                        | OperationKind::CallStructuralScalar { .. }
                        | OperationKind::CallDynamicScalar { .. }
                        | OperationKind::CallDynamicParameterScalar { .. }
                        | OperationKind::CallStructural { .. }
                        | OperationKind::CallStructuralWithScalarArguments { .. }
                        | OperationKind::BoundaryCall { .. }
                )
            })
    {
        return None;
    }
    let mut exits = BTreeMap::new();
    for block in &machine.blocks {
        let Terminator::ReturnStructural {
            source,
            returned_claims,
            ..
        } = &block.terminator
        else {
            continue;
        };
        if !returned_claims.is_empty() {
            return None;
        }
        let producer = machine.structural_places.iter().find_map(|place| {
            (place.id == *source)
                .then_some(place.kind)
                .and_then(|kind| match kind {
                    StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type,
                    } if structural_type == result.structural_type => Some(producer),
                    _ => None,
                })
        })?;
        let operation = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|operation| operation.id == producer)?;
        let operation_result = operation.result.structural()?;
        let OperationKind::EstablishPayloadlessCase { result_case } = operation.kind else {
            return None;
        };
        if operation_result.place != *source
            || operation_result.structural_type != result.structural_type
            || operation_result.multiplicity != StructuralMultiplicity::Unrestricted
            || !operation_result.claims.is_empty()
            || !super::structural_result_contracts::has_empty_qualification_rosters(
                &operation_result.qualifications,
                &operation_result.projected_qualifications,
            )
        {
            return None;
        }
        exits.insert(
            block.id,
            psi_terminal::OutcomeSpecificGuard {
                result_type: result.structural_type,
                result_case,
            },
        );
    }
    (!exits.is_empty()).then_some(exits)
}

fn is_exact_payloadless_structural_call(
    module: &TerminalModule,
    operation: &psi_terminal::Operation,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
) -> bool {
    let OperationKind::CallStructural {
        callee,
        structural_arguments,
        claim_transfers,
        returned_claim_transfers,
        requirement_obligations,
        crash_continuations,
        selected_evidence: _,
    } = &operation.kind
    else {
        return false;
    };
    let Some(result) = operation.result.structural() else {
        return false;
    };
    let Some(callee) = machines.get(callee).copied() else {
        return false;
    };
    let Some(callee_result) = callee.result.structural() else {
        return false;
    };
    callee.parameters.is_empty()
        && callee.structural_parameters.is_empty()
        && callee.entry_claims.is_empty()
        && callee.content_entry_claims.is_empty()
        && callee.contract.requires.is_empty()
        && callee.contract.ensures.is_empty()
        && callee.contract.crash_routes.is_empty()
        && module
            .evidence_contract_lanes
            .iter()
            .all(|lane| lane.machine != callee.id)
        && structural_arguments.is_empty()
        && claim_transfers.is_empty()
        && returned_claim_transfers.is_empty()
        && requirement_obligations.is_empty()
        && crash_continuations.is_empty()
        && result.structural_type == callee_result.structural_type
        && result.multiplicity == StructuralMultiplicity::Unrestricted
        && result.multiplicity == callee_result.multiplicity
        && super::structural_result_contracts::has_empty_qualification_rosters(
            &result.qualifications,
            &result.projected_qualifications,
        )
        && super::structural_result_contracts::call_result_matches(result, callee_result)
        && result.claims.is_empty()
        && callee.contract.outcome_specific_ensures.iter().all(|row| {
            propositions::proposition_boolean_field_roots(&row.proposition)
                .into_iter()
                .chain(propositions::proposition_content_roots(&row.proposition))
                .all(|root| root == callee_result.place)
        })
        && exact_payloadless_case_return_exits(callee).is_some()
}

pub(super) fn exact_payloadless_structural_call(
    module: &TerminalModule,
    operation: &psi_terminal::Operation,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
) -> bool {
    is_exact_payloadless_structural_call(module, operation, machines)
}

pub(super) fn validate_unit_operation_static(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    operation: &psi_terminal::Operation,
) -> Result<(), ModuleError> {
    match &operation.kind {
        OperationKind::WriteOnlyPrimitiveStore { destination, .. } => {
            let invalid = || ModuleError::WriteOnlyPrimitiveStoreDestinationMismatch {
                operation: operation.id,
                place: *destination,
            };
            let parameter = machine
                .structural_parameters
                .iter()
                .find(|parameter| parameter.place == *destination)
                .ok_or_else(invalid)?;
            let place = machine
                .structural_places
                .iter()
                .find(|place| place.id == *destination)
                .ok_or_else(invalid)?;
            if !matches!(
                place.kind,
                StructuralPlaceKind::Parameter { position, is_self }
                    if position == parameter.position && is_self == parameter.is_self
            ) || !matches!(
                parameter.access,
                StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
            ) || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || !parameter.qualifications.is_empty()
                || machine
                    .entry_claims
                    .iter()
                    .any(|claim| claim.input == *destination)
                || machine
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == *destination)
            {
                return Err(invalid());
            }
            let declaration = module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == parameter.structural_type)
                .ok_or(ModuleError::UnknownStructuralType(
                    parameter.structural_type,
                ))?;
            if !matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(_)) {
                return Err(
                    ModuleError::WriteOnlyPrimitiveStoreRequiresPrimitiveScalar {
                        operation: operation.id,
                        structural_type: parameter.structural_type,
                    },
                );
            }
        }
        OperationKind::StructuralScalarFieldStore {
            destination,
            path,
            field,
            ..
        } => {
            super::structural_scalar_fields::structural_scalar_field_store_type(
                module,
                machine,
                operation.id,
                *destination,
                path,
                *field,
            )?;
        }
        OperationKind::EstablishPayloadlessCase { result_case } => {
            let Some(result) = operation.result.structural() else {
                return Err(ModuleError::PayloadlessCaseResultMismatch(operation.id));
            };
            let Some(place) = machine
                .structural_places
                .iter()
                .find(|place| place.id == result.place)
            else {
                return Err(ModuleError::PayloadlessCaseResultMismatch(operation.id));
            };
            if !matches!(
                place.kind,
                StructuralPlaceKind::OperationResult { producer, structural_type }
                    if producer == operation.id && structural_type == result.structural_type
            ) || result.multiplicity != StructuralMultiplicity::Unrestricted
                || !super::structural_result_contracts::has_empty_qualification_rosters(
                    &result.qualifications,
                    &result.projected_qualifications,
                )
                || !result.claims.is_empty()
            {
                return Err(ModuleError::PayloadlessCaseResultMismatch(operation.id));
            }
            let Some(declaration) = module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == result.structural_type)
            else {
                return Err(ModuleError::UnknownStructuralType(result.structural_type));
            };
            let StructuralTypeShape::Sum { cases } = &declaration.shape else {
                return Err(ModuleError::PayloadlessCaseRequiresSum {
                    operation: operation.id,
                    structural_type: result.structural_type,
                    result_case: *result_case,
                });
            };
            if !cases
                .iter()
                .any(|case| case.id == *result_case && case.fields.is_empty())
            {
                return Err(ModuleError::PayloadlessCaseRequiresPayloadlessMember {
                    operation: operation.id,
                    structural_type: result.structural_type,
                    result_case: *result_case,
                });
            }
        }
        OperationKind::EstablishAffineScalarRecord { field, value } => {
            let Some(result) = operation.result.structural() else {
                return Err(ModuleError::AffineScalarRecordResultMismatch(operation.id));
            };
            let Some(place) = machine
                .structural_places
                .iter()
                .find(|place| place.id == result.place)
            else {
                return Err(ModuleError::AffineScalarRecordResultMismatch(operation.id));
            };
            if !matches!(
                place.kind,
                StructuralPlaceKind::OperationResult { producer, structural_type }
                    if producer == operation.id && structural_type == result.structural_type
            ) || result.multiplicity != StructuralMultiplicity::Affine
                || !super::structural_result_contracts::has_empty_qualification_rosters(
                    &result.qualifications,
                    &result.projected_qualifications,
                )
                || !result.claims.is_empty()
            {
                return Err(ModuleError::AffineScalarRecordResultMismatch(operation.id));
            }
            let declaration = module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == result.structural_type)
                .ok_or(ModuleError::UnknownStructuralType(result.structural_type))?;
            let exact_i64_field = matches!(
                &declaration.shape,
                StructuralTypeShape::Record { fields }
                    if matches!(fields.as_slice(), [candidate]
                        if candidate.id == *field
                            && candidate.relevance == psi_terminal::BindingRelevance::Relevant
                            && matches!(candidate.field_type,
                                StructuralFieldType::Scalar(ScalarType::Integer(integer_type))
                                    if integer_type.sign() == IntegerSign::Signed
                                        && integer_type.bits() == 64))
            );
            if !exact_i64_field {
                return Err(ModuleError::AffineScalarRecordRequiresSingleI64Field {
                    operation: operation.id,
                    structural_type: result.structural_type,
                    field: *field,
                });
            }
            let i64_type = IntegerType::new(IntegerSign::Signed, 64)
                .expect("signed i64 is a valid fixed integer type");
            if !i64_type.admits(*value) {
                return Err(ModuleError::AffineScalarRecordValueOutsideI64(operation.id));
            }
        }
        OperationKind::CallUnit {
            callee,
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
            if callee.result != TerminalMachineResult::Unit || !callee.parameters.is_empty() {
                return Err(ModuleError::UnitCallTargetHasScalarSignature {
                    operation: operation.id,
                    callee: callee.id,
                });
            }
            if structural_arguments.iter().any(|argument| {
                !argument.path.is_empty()
                    && !matches!(
                        argument.path.as_slice(),
                        [StructuralPathSegment::FixedIndex(_)]
                            | [
                                StructuralPathSegment::FixedIndex(_),
                                StructuralPathSegment::FixedIndex(_),
                            ]
                    )
                    && !is_write_only_subloan_path(&argument.path)
            }) {
                return Err(ModuleError::InvalidStructuralArgumentPath {
                    operation: operation.id,
                    argument_index: structural_arguments
                        .iter()
                        .position(|argument| {
                            !argument.path.is_empty()
                                && !matches!(
                                    argument.path.as_slice(),
                                    [StructuralPathSegment::FixedIndex(_)]
                                        | [
                                            StructuralPathSegment::FixedIndex(_),
                                            StructuralPathSegment::FixedIndex(_),
                                        ]
                                )
                                && !is_write_only_subloan_path(&argument.path)
                        })
                        .unwrap_or_default() as u32,
                });
            }
            let projected = structural_arguments
                .iter()
                .any(|argument| !argument.path.is_empty());
            if projected
                && (machine.result != TerminalMachineResult::Unit
                    || !machine.parameters.is_empty()
                    || machine.structural_parameters.len() != 1
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
                StructuralArgumentSourcePolicy::ParametersAndAffineLocals,
            )?;
            if let Some(argument_index) = structural_arguments
                .iter()
                .zip(&callee.structural_parameters)
                .position(|(argument, expected)| {
                    (is_literal_indexed_field_path(&argument.path)
                        || is_double_literal_indexed_field_path(&argument.path)
                        || ((is_single_literal_index_path(&argument.path)
                            || is_double_literal_index_path(&argument.path))
                            && argument.access == StructuralAccess::WriteOnlyBorrow))
                        && !is_unrestricted_write_only_subloan(module, machine, expected, argument)
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
                let projected_parameter = callee.structural_parameters[0].place;
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
                structural_arguments,
                crash_continuations,
                operation.id,
            )?;
        }
        OperationKind::CallStructuralScalar {
            callee,
            arguments: _,
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
            validate_structural_arguments(
                module,
                machine,
                structural_arguments,
                &callee.structural_parameters,
                operation.id,
                true,
                StructuralArgumentSourcePolicy::ParametersOnly,
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
        } => {
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
            let exact_record = module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == callee_result.structural_type)
                .is_some_and(|declaration| {
                    matches!(
                        &declaration.shape,
                        StructuralTypeShape::Record { fields }
                            if matches!(
                                fields.as_slice(),
                                [field]
                                    if matches!(
                                        field.field_type,
                                        StructuralFieldType::Scalar(ScalarType::Integer(integer))
                                            if integer.carrier()
                                                == psi_core::IntegerCarrier::Fixed
                                                && integer.bits() == 64
                                    )
                            )
                    )
                });
            let exact_return = matches!(callee.blocks.as_slice(), [block]
            if block.operations.is_empty()
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
            if callee.parameters.len() != 1
                || !matches!(
                    callee.parameters[0].scalar_type,
                    ScalarType::Integer(integer)
                        if integer.carrier() == psi_core::IntegerCarrier::Fixed
                            && matches!(integer.bits(), 8 | 16 | 32 | 64)
                )
                || callee_parameter.position != 0
                || callee_parameter.is_self
                || callee_parameter.multiplicity != StructuralMultiplicity::Affine
                || callee_parameter.access != StructuralAccess::Owned
                || !callee_parameter.qualifications.is_empty()
                || !callee_parameter.projected_qualifications.is_empty()
                || argument.path.len() != 0
                || argument.access != StructuralAccess::Owned
                || result.structural_type != callee_result.structural_type
                || result.multiplicity != StructuralMultiplicity::Affine
                || result.multiplicity != callee_result.multiplicity
                || !super::structural_result_contracts::call_result_matches(result, callee_result)
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
                || !exact_record
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
                true,
                StructuralArgumentSourcePolicy::ParametersOnly,
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
            selected_evidence,
        } => {
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
            if !callee.parameters.is_empty()
                || structural_arguments.len() != 1
                || structural_arguments[0].path.len() != 0
                || callee.structural_parameters.len() != 1
                || result.structural_type != callee_result.structural_type
                || result.multiplicity != callee_result.multiplicity
                || !super::structural_result_contracts::call_result_matches(result, callee_result)
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
                StructuralArgumentSourcePolicy::ParametersOnly,
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
                StructuralArgumentSourcePolicy::ParametersAndByteSequenceLiterals,
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
                StructuralTypeShape::ByteSequence(psi_terminal::ByteSequenceCarrier::BorrowedView)
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

/// Validate the complete bounded representation for a nonempty run of
/// pairwise-disjoint field transfers, followed by disposal of every maximal
/// residual sibling subtree in recursive reverse declaration order. This
/// partition is checked independently of producer facts before the ownership
/// walk relies on the path-sensitive terminator.

fn validate_unit_call_contract_places(
    callee: &TerminalMachine,
    operation: OperationId,
) -> Result<(), ModuleError> {
    let parameters = callee
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .collect::<BTreeSet<_>>();
    for proposition in unit_call_contract_propositions(callee) {
        if let Some(place) = propositions::proposition_content_roots(proposition)
            .into_iter()
            .find(|place| !parameters.contains(place))
        {
            return Err(ModuleError::UnitCallContractPlaceHasNoArgument {
                operation,
                callee: callee.id,
                place,
            });
        }
    }
    Ok(())
}

fn unit_call_contract_propositions(callee: &TerminalMachine) -> impl Iterator<Item = &Proposition> {
    callee
        .contract
        .requires
        .iter()
        .chain(
            callee
                .contract
                .ensures
                .iter()
                .map(|clause| &clause.proposition),
        )
        .chain(
            callee
                .contract
                .crash_routes
                .iter()
                .flat_map(|bucket| &bucket.alternatives)
                .filter_map(|guard| match guard {
                    CrashRouteGuard::Truth => None,
                    CrashRouteGuard::Predicate(predicate) => Some(predicate.proposition()),
                }),
        )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StructuralArgumentSourcePolicy {
    ParametersOnly,
    ParametersAndByteSequenceLiterals,
    ParametersAndAffineLocals,
}

pub(super) fn validate_structural_arguments(
    module: &TerminalModule,
    caller: &TerminalMachine,
    arguments: &[StructuralArgument],
    expected: &[StructuralParameterDeclaration],
    operation: OperationId,
    allow_projected: bool,
    source_policy: StructuralArgumentSourcePolicy,
) -> Result<(), ModuleError> {
    if arguments.len() != expected.len() {
        return Err(ModuleError::StructuralArgumentArityMismatch {
            operation,
            expected: expected.len(),
            actual: arguments.len(),
        });
    }
    for (index, (argument, expected)) in arguments.iter().zip(expected).enumerate() {
        let Some((
            actual_type,
            actual_multiplicity,
            actual_access,
            actual_qualifications,
            actual_projected_qualifications,
        )) = caller
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == argument.place)
            .map(|parameter| {
                (
                    parameter.structural_type,
                    parameter.multiplicity,
                    parameter.access,
                    parameter.qualifications.as_slice(),
                    parameter.projected_qualifications.as_slice(),
                )
            })
            .or_else(|| {
                caller.structural_places.iter().find_map(|place| {
                    if place.id != argument.place {
                        return None;
                    }
                    match place.kind {
                        StructuralPlaceKind::ByteSequenceLiteral {
                            structural_type, ..
                        } if source_policy
                            == StructuralArgumentSourcePolicy::ParametersAndByteSequenceLiterals => Some((
                            structural_type,
                            StructuralMultiplicity::Unrestricted,
                            StructuralAccess::Owned,
                            &[][..],
                            &[][..],
                        )),
                        StructuralPlaceKind::TrivialAffineLocal {
                            structural_type, ..
                        } if source_policy
                            == StructuralArgumentSourcePolicy::ParametersAndAffineLocals
                            && argument.path.is_empty() => Some((
                            structural_type,
                            StructuralMultiplicity::Affine,
                            StructuralAccess::Owned,
                            &[][..],
                            &[][..],
                        )),
                        StructuralPlaceKind::OperationResult {
                            producer,
                            structural_type,
                        } if source_policy == StructuralArgumentSourcePolicy::ParametersAndAffineLocals
                            && argument.path.is_empty()
                            && caller.blocks.iter().flat_map(|block| &block.operations).any(
                                |operation| {
                                    operation.id == producer
                                        && matches!(
                                            operation.kind,
                                            OperationKind::EstablishAffineScalarRecord { .. }
                                        )
                                        && operation.result.structural().is_some_and(|result| {
                                            result.place == argument.place
                                                && result.structural_type == structural_type
                                                && result.multiplicity
                                                    == StructuralMultiplicity::Affine
                                                && result.qualifications.is_empty()
                                                && result.projected_qualifications.is_empty()
                                                && result.claims.is_empty()
                                        })
                                },
                            ) => Some((
                            structural_type,
                            StructuralMultiplicity::Affine,
                            StructuralAccess::Owned,
                            &[][..],
                            &[][..],
                        )),
                        _ => None,
                    }
                })
            })
        else {
            return Err(ModuleError::UnknownStructuralArgument {
                operation,
                argument_index: index as u32,
                place: argument.place,
            });
        };
        if !allow_projected && !argument.path.is_empty() {
            return Err(ModuleError::InvalidStructuralArgumentPath {
                operation,
                argument_index: index as u32,
            });
        }
        let root_type = actual_type;
        let Some(actual_type) = resolve_structural_path(module, actual_type, &argument.path) else {
            return Err(ModuleError::InvalidStructuralArgumentPath {
                operation,
                argument_index: index as u32,
            });
        };
        if actual_type != expected.structural_type {
            return Err(ModuleError::StructuralArgumentTypeMismatch {
                operation,
                argument_index: index as u32,
                expected: expected.structural_type,
                actual: actual_type,
            });
        }
        if argument.access != expected.access {
            return Err(ModuleError::StructuralArgumentAccessMismatch {
                operation,
                argument_index: index as u32,
                expected: expected.access,
                actual: argument.access,
            });
        }
        if !structural_access_can_supply(actual_access, argument.access) {
            return Err(ModuleError::StructuralArgumentAccessExceedsSource {
                operation,
                argument_index: index as u32,
                source: actual_access,
                presented: argument.access,
            });
        }
        let unrestricted_write_only_field_subloan =
            is_unrestricted_write_only_subloan(module, caller, expected, argument);
        let unrestricted_shared_field_subloan =
            is_unrestricted_shared_subloan(caller, expected, argument);
        let unrestricted_mutable_field_subloan =
            is_unrestricted_mutable_subloan(caller, expected, argument);
        let actual_multiplicity = if argument.path.is_empty() {
            actual_multiplicity
        } else if unrestricted_write_only_field_subloan
            || unrestricted_shared_field_subloan
            || unrestricted_mutable_field_subloan
        {
            StructuralMultiplicity::Unrestricted
        } else if expected.multiplicity == StructuralMultiplicity::Affine
            && is_bounded_partial_affine_path(module, root_type, &argument.path)
            && actual_multiplicity == StructuralMultiplicity::Affine
        {
            StructuralMultiplicity::Affine
        } else {
            StructuralMultiplicity::Linear
        };
        if actual_multiplicity != expected.multiplicity {
            return Err(ModuleError::StructuralArgumentMultiplicityMismatch {
                operation,
                argument_index: index as u32,
                expected: expected.multiplicity,
                actual: actual_multiplicity,
            });
        }
        for qualification in &expected.qualifications {
            if !structural_occurrence_carries_qualification(
                actual_qualifications,
                actual_projected_qualifications,
                &argument.path,
                *qualification,
            ) {
                return Err(ModuleError::StructuralArgumentMissingQualification {
                    operation,
                    argument_index: index as u32,
                    domain: *qualification,
                });
            }
        }
        for qualification in &expected.projected_qualifications {
            let mut source_path = argument.path.clone();
            source_path.extend(qualification.path.iter().cloned());
            if !structural_occurrence_carries_qualification(
                actual_qualifications,
                actual_projected_qualifications,
                &source_path,
                qualification.domain,
            ) {
                return Err(ModuleError::StructuralArgumentMissingQualification {
                    operation,
                    argument_index: index as u32,
                    domain: qualification.domain,
                });
            }
        }
    }
    for first in 0..arguments.len() {
        for second in first + 1..arguments.len() {
            let left = &arguments[first];
            let right = &arguments[second];
            if left.place == right.place
                && structural_paths_may_overlap(&left.path, &right.path)
                && (structural_access_is_exclusive(left.access)
                    || structural_access_is_exclusive(right.access))
            {
                return Err(ModuleError::OverlappingExclusiveStructuralArguments {
                    operation,
                    first_argument: first as u32,
                    second_argument: second as u32,
                });
            }
        }
    }
    Ok(())
}

fn structural_occurrence_carries_qualification(
    root: &[StructuralDomainId],
    projected: &[psi_terminal::StructuralPathQualification],
    path: &[StructuralPathSegment],
    domain: StructuralDomainId,
) -> bool {
    if path.is_empty() {
        root.contains(&domain)
    } else {
        projected
            .iter()
            .any(|qualification| qualification.path == path && qualification.domain == domain)
    }
}

fn structural_access_can_supply(source: StructuralAccess, presented: StructuralAccess) -> bool {
    match source {
        StructuralAccess::Owned => true,
        StructuralAccess::SharedBorrow => presented == StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow => matches!(
            presented,
            StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow
        ),
        StructuralAccess::WriteOnlyBorrow => presented == StructuralAccess::WriteOnlyBorrow,
    }
}

fn structural_access_is_exclusive(access: StructuralAccess) -> bool {
    matches!(
        access,
        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
    )
}

fn structural_paths_may_overlap(
    left: &[StructuralPathSegment],
    right: &[StructuralPathSegment],
) -> bool {
    left.iter().zip(right).all(|(left, right)| left == right)
}

fn is_literal_indexed_field_path(path: &[StructuralPathSegment]) -> bool {
    let Some((StructuralPathSegment::FixedIndex(_), fields)) = path.split_last() else {
        return false;
    };
    is_nonempty_field_path(fields)
}

fn is_single_literal_index_path(path: &[StructuralPathSegment]) -> bool {
    matches!(path, [StructuralPathSegment::FixedIndex(_)])
}

fn is_double_literal_index_path(path: &[StructuralPathSegment]) -> bool {
    matches!(
        path,
        [
            StructuralPathSegment::FixedIndex(_),
            StructuralPathSegment::FixedIndex(_),
        ]
    )
}

fn is_double_literal_indexed_field_path(path: &[StructuralPathSegment]) -> bool {
    let [
        fields @ ..,
        StructuralPathSegment::FixedIndex(_),
        StructuralPathSegment::FixedIndex(_),
    ] = path
    else {
        return false;
    };
    is_nonempty_field_path(fields)
}

fn is_literal_indexed_write_only_path(path: &[StructuralPathSegment]) -> bool {
    is_literal_indexed_field_path(path)
        || is_double_literal_indexed_field_path(path)
        || is_single_literal_index_path(path)
        || is_double_literal_index_path(path)
}

fn is_write_only_subloan_path(path: &[StructuralPathSegment]) -> bool {
    is_nonempty_field_path(path) || is_literal_indexed_write_only_path(path)
}

fn is_unrestricted_write_only_subloan(
    module: &TerminalModule,
    caller: &TerminalMachine,
    expected: &StructuralParameterDeclaration,
    argument: &StructuralArgument,
) -> bool {
    let Some(actual) = caller
        .structural_parameters
        .iter()
        .find(|actual| actual.place == argument.place)
    else {
        return false;
    };
    let indexed_leaf_is_primitive =
        !matches!(
            argument.path.last(),
            Some(StructuralPathSegment::FixedIndex(_))
        ) || resolve_structural_path(module, actual.structural_type, &argument.path).is_some_and(
            |leaf| {
                module.structural_types.iter().any(|declaration| {
                    declaration.id == leaf
                        && matches!(&declaration.shape, StructuralTypeShape::PrimitiveScalar(_))
                })
            },
        );

    is_write_only_subloan_path(&argument.path)
        && argument.access == StructuralAccess::WriteOnlyBorrow
        && expected.access == StructuralAccess::WriteOnlyBorrow
        && expected.multiplicity == StructuralMultiplicity::Unrestricted
        && actual.access == StructuralAccess::WriteOnlyBorrow
        && actual.multiplicity == StructuralMultiplicity::Unrestricted
        && indexed_leaf_is_primitive
}

fn is_unrestricted_shared_subloan(
    caller: &TerminalMachine,
    expected: &StructuralParameterDeclaration,
    argument: &StructuralArgument,
) -> bool {
    let Some(actual) = caller
        .structural_parameters
        .iter()
        .find(|actual| actual.place == argument.place)
    else {
        return false;
    };
    is_nonempty_field_path(&argument.path)
        && argument.access == StructuralAccess::SharedBorrow
        && expected.access == StructuralAccess::SharedBorrow
        && expected.multiplicity == StructuralMultiplicity::Unrestricted
        && actual.multiplicity == StructuralMultiplicity::Unrestricted
}

fn is_unrestricted_mutable_subloan(
    caller: &TerminalMachine,
    expected: &StructuralParameterDeclaration,
    argument: &StructuralArgument,
) -> bool {
    let Some(actual) = caller
        .structural_parameters
        .iter()
        .find(|actual| actual.place == argument.place)
    else {
        return false;
    };
    is_nonempty_field_path(&argument.path)
        && argument.access == StructuralAccess::MutableBorrow
        && expected.access == StructuralAccess::MutableBorrow
        && expected.multiplicity == StructuralMultiplicity::Unrestricted
        && actual.access == StructuralAccess::MutableBorrow
        && actual.multiplicity == StructuralMultiplicity::Unrestricted
}

pub(super) fn validate_service_reach(
    operation: OperationId,
    caller: &[ServiceId],
    reached: &[ServiceId],
) -> Result<(), ModuleError> {
    if let Some(service) = reached.iter().find(|service| !caller.contains(service)) {
        return Err(ModuleError::OperationServiceOutsidePublishedCeiling {
            operation,
            service: *service,
        });
    }
    Ok(())
}

fn validate_unit_call_claim_transfers(
    module: &TerminalModule,
    caller: &TerminalMachine,
    callee: &TerminalMachine,
    arguments: &[StructuralArgument],
    transfers: &[ClaimTransfer],
    operation: OperationId,
) -> Result<(), ModuleError> {
    for (argument_index, (argument, parameter)) in arguments
        .iter()
        .zip(&callee.structural_parameters)
        .enumerate()
    {
        if !argument.path.is_empty() {
            let callee_claims = callee
                .entry_claims
                .iter()
                .filter(|claim| claim.input == parameter.place)
                .collect::<Vec<_>>();
            let claim_free_unrestricted_write_only_field =
                is_unrestricted_write_only_subloan(module, caller, parameter, argument)
                    && callee_claims.is_empty()
                    && caller
                        .entry_claims
                        .iter()
                        .all(|claim| claim.input != argument.place);
            let claim_free_unrestricted_shared_field =
                is_unrestricted_shared_subloan(caller, parameter, argument)
                    && callee_claims.is_empty()
                    && caller
                        .entry_claims
                        .iter()
                        .all(|claim| claim.input != argument.place);
            let claim_free_unrestricted_mutable_field =
                is_unrestricted_mutable_subloan(caller, parameter, argument)
                    && callee_claims.is_empty()
                    && caller
                        .entry_claims
                        .iter()
                        .all(|claim| claim.input != argument.place);
            let claim_free_direct_affine = caller
                .structural_parameters
                .iter()
                .find(|actual| actual.place == argument.place)
                .is_some_and(|actual| {
                    is_bounded_partial_affine_path(module, actual.structural_type, &argument.path)
                })
                && parameter.multiplicity == StructuralMultiplicity::Affine
                && callee_claims.is_empty()
                && caller
                    .entry_claims
                    .iter()
                    .all(|claim| claim.input != argument.place);
            if !claim_free_unrestricted_write_only_field
                && !claim_free_unrestricted_shared_field
                && !claim_free_unrestricted_mutable_field
                && !claim_free_direct_affine
                && !matches!(callee_claims.as_slice(), [claim] if claim.path.is_empty())
            {
                return Err(ModuleError::UnitCallClaimPresenceMismatch {
                    operation,
                    argument_index: argument_index as u32,
                });
            }
            if caller
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == argument.place)
                || callee
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == parameter.place)
            {
                return Err(ModuleError::UnitCallContentClaimMismatch {
                    operation,
                    argument_index: argument_index as u32,
                });
            }
        }
        let mut caller_claim_paths = caller
            .entry_claims
            .iter()
            .filter(|claim| claim.input == argument.place && claim.path.starts_with(&argument.path))
            .map(|claim| &claim.path[argument.path.len()..])
            .collect::<Vec<_>>();
        let mut callee_claim_paths = callee
            .entry_claims
            .iter()
            .filter(|claim| claim.input == parameter.place)
            .map(|claim| claim.path.as_slice())
            .collect::<Vec<_>>();
        caller_claim_paths.sort();
        callee_claim_paths.sort();
        if caller_claim_paths != callee_claim_paths {
            return Err(ModuleError::UnitCallClaimPresenceMismatch {
                operation,
                argument_index: argument_index as u32,
            });
        }
        let mut caller_content = caller
            .content_entry_claims
            .iter()
            .filter(|binding| binding.input.root == argument.place)
            .map(|binding| (&binding.input.segments, &binding.projections))
            .collect::<Vec<_>>();
        let mut callee_content = callee
            .content_entry_claims
            .iter()
            .filter(|binding| binding.input.root == parameter.place)
            .map(|binding| (&binding.input.segments, &binding.projections))
            .collect::<Vec<_>>();
        caller_content.sort();
        callee_content.sort();
        if caller_content != callee_content {
            return Err(ModuleError::UnitCallContentClaimMismatch {
                operation,
                argument_index: argument_index as u32,
            });
        }
    }
    let callee_claims = callee
        .entry_claims
        .iter()
        .map(|claim| (claim.claim, claim.input))
        .chain(
            callee
                .content_entry_claims
                .iter()
                .map(|claim| (claim.claim, claim.input.root)),
        )
        .collect::<BTreeMap<_, _>>();
    for (claim, input) in &callee_claims {
        if !callee
            .structural_parameters
            .iter()
            .any(|parameter| parameter.place == *input)
        {
            return Err(ModuleError::UnitCallClaimHasNoStructuralArgument {
                operation,
                claim: *claim,
            });
        }
    }
    if transfers.len() != callee_claims.len() {
        return Err(ModuleError::UnitCallClaimTransferCountMismatch {
            operation,
            expected: callee_claims.len(),
            actual: transfers.len(),
        });
    }
    let mut caller_claims = BTreeSet::new();
    for transfer in transfers {
        if !caller_claims.insert(transfer.claim) {
            return Err(ModuleError::DuplicateUnitCallClaimTransfer(operation));
        }
        let Some(argument) = arguments.get(transfer.argument_index as usize) else {
            return Err(ModuleError::ClaimActionArgumentOutOfRange {
                operation,
                argument_index: transfer.argument_index,
            });
        };
        let Some((claim_input, claim_path)) = claim_input(caller, transfer.claim) else {
            return Err(ModuleError::UnknownClaimAtOperation {
                operation,
                claim: transfer.claim,
            });
        };
        let target_place = callee
            .structural_parameters
            .get(transfer.argument_index as usize)
            .map(|parameter| parameter.place);
        let structural_path_matches = claim_path.starts_with(&argument.path)
            && callee.entry_claims.iter().any(|claim| {
                Some(claim.input) == target_place && claim.path == claim_path[argument.path.len()..]
            });
        let content_matches = argument.path.is_empty()
            && caller
                .content_entry_claims
                .iter()
                .any(|claim| claim.claim == transfer.claim && claim.input.root == argument.place)
            && callee
                .content_entry_claims
                .iter()
                .any(|claim| Some(claim.input.root) == target_place);
        if claim_input != argument.place || (!structural_path_matches && !content_matches) {
            return Err(ModuleError::ClaimActionPlaceMismatch {
                operation,
                claim: transfer.claim,
                argument_index: transfer.argument_index,
            });
        }
    }
    for input in callee_claims.into_values() {
        let argument_index = callee
            .structural_parameters
            .iter()
            .position(|parameter| parameter.place == input)
            .expect("callee entry claims were validated against its signature")
            as u32;
        if !transfers
            .iter()
            .any(|transfer| transfer.argument_index == argument_index)
        {
            return Err(ModuleError::MissingUnitCallClaimTransfer {
                operation,
                argument_index,
            });
        }
    }
    if transfers.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ModuleError::NonCanonicalUnitCallClaimTransfers(operation));
    }
    Ok(())
}

fn claim_input(
    machine: &TerminalMachine,
    claim: ClaimId,
) -> Option<(PlaceId, &[StructuralPathSegment])> {
    machine
        .entry_claims
        .iter()
        .find_map(|candidate| {
            (candidate.claim == claim).then_some((candidate.input, candidate.path.as_slice()))
        })
        .or_else(|| {
            machine.content_entry_claims.iter().find_map(|candidate| {
                (candidate.claim == claim)
                    .then_some((candidate.input.root, &[] as &[StructuralPathSegment]))
            })
        })
}

fn validate_unit_call_crash_continuations(
    module: &TerminalModule,
    caller: &TerminalMachine,
    callee: &TerminalMachine,
    arguments: &[StructuralArgument],
    continuations: &[CrashRouteBucket],
    operation: OperationId,
) -> Result<(), ModuleError> {
    let boolean_roots = callee
        .contract
        .crash_routes
        .iter()
        .flat_map(|bucket| &bucket.alternatives)
        .filter_map(|guard| match guard {
            CrashRouteGuard::Truth => None,
            CrashRouteGuard::Predicate(predicate) => Some(predicate.proposition()),
        })
        .flat_map(propositions::proposition_boolean_field_roots)
        .collect::<BTreeSet<_>>();
    let substitutions = callee
        .structural_parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| {
            let prefix = structural_argument_canonical_prefix(module, caller, argument);
            if prefix.is_none() && boolean_roots.contains(&parameter.place) {
                return Err(
                    ModuleError::ProjectedUnitCallContractUsesStructuralParameter {
                        operation,
                        callee: callee.id,
                        place: parameter.place,
                    },
                );
            }
            Ok((
                parameter.place,
                (argument.place, prefix.unwrap_or_default()),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let expected = substitute_crash_route_places(&callee.contract.crash_routes, &substitutions);
    if continuations != expected {
        return Err(ModuleError::CallCrashContinuationsMismatch {
            operation,
            callee: callee.id,
        });
    }
    for continuation in continuations {
        let covered = caller.contract.crash_routes.iter().any(|published| {
            published.cause == continuation.cause
                && (published.alternatives == [CrashRouteGuard::Truth]
                    || continuation
                        .alternatives
                        .iter()
                        .all(|route| published.alternatives.contains(route)))
        });
        if !covered {
            return Err(ModuleError::CallCrashContinuationUncovered {
                operation,
                cause: continuation.cause,
            });
        }
    }
    Ok(())
}

fn substitute_crash_route_places(
    routes: &[CrashRouteBucket],
    substitutions: &BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
) -> Vec<CrashRouteBucket> {
    routes
        .iter()
        .map(|bucket| {
            let mut alternatives = bucket
                .alternatives
                .iter()
                .map(|guard| match guard {
                    CrashRouteGuard::Truth => CrashRouteGuard::Truth,
                    CrashRouteGuard::Predicate(predicate) => CrashRouteGuard::Predicate(
                        CrashPredicateTerm::new(substitute_proposition_structural_places(
                            predicate.proposition(),
                            substitutions,
                        )),
                    ),
                })
                .collect::<Vec<_>>();
            alternatives.sort();
            alternatives.dedup();
            if alternatives.contains(&CrashRouteGuard::Truth) {
                alternatives = vec![CrashRouteGuard::Truth];
            }
            CrashRouteBucket {
                cause: bucket.cause,
                alternatives,
            }
        })
        .collect()
}

pub(crate) fn structural_argument_canonical_prefix(
    module: &TerminalModule,
    caller: &TerminalMachine,
    argument: &StructuralArgument,
) -> Option<Vec<CanonicalStructuralPathSegment>> {
    let mut structural_type = caller
        .structural_parameters
        .iter()
        .find_map(|parameter| {
            (parameter.place == argument.place).then_some(parameter.structural_type)
        })
        .or_else(|| {
            caller
                .structural_places
                .iter()
                .find_map(|place| match place.kind {
                    StructuralPlaceKind::ByteSequenceLiteral {
                        structural_type, ..
                    }
                    | StructuralPlaceKind::TrivialAffineLocal {
                        structural_type, ..
                    }
                    | StructuralPlaceKind::OperationResult {
                        structural_type, ..
                    } if place.id == argument.place => Some(structural_type),
                    _ => None,
                })
        })?;
    let mut prefix = Vec::with_capacity(argument.path.len());
    for segment in &argument.path {
        match segment {
            StructuralPathSegment::Field(identity) => {
                let field = module
                    .structural_types
                    .iter()
                    .find(|declaration| declaration.id == structural_type)
                    .and_then(|declaration| match &declaration.shape {
                        StructuralTypeShape::Record { fields }
                        | StructuralTypeShape::Mixed { fields, .. } => {
                            fields.iter().find(|field| {
                                field.identity == *identity && !field.relevance.is_erased()
                            })
                        }
                        StructuralTypeShape::PrimitiveScalar(_)
                        | StructuralTypeShape::ByteSequence(_)
                        | StructuralTypeShape::FixedArray { .. }
                        | StructuralTypeShape::Sum { .. } => None,
                    })?;
                let StructuralFieldType::Structural(next) = field.field_type else {
                    return None;
                };
                prefix.push(CanonicalStructuralPathSegment::Field(field.id));
                structural_type = next;
            }
            StructuralPathSegment::FixedIndex(index) => {
                let element = module
                    .structural_types
                    .iter()
                    .find(|declaration| declaration.id == structural_type)
                    .and_then(|declaration| match declaration.shape {
                        StructuralTypeShape::FixedArray { element, length } if *index < length => {
                            Some(element)
                        }
                        _ => None,
                    })?;
                prefix.push(CanonicalStructuralPathSegment::FixedIndex(*index));
                structural_type = element;
            }
        }
    }
    Some(prefix)
}

fn validate_boundary_requirements(
    caller: &TerminalMachine,
    boundary: &BoundaryMachineDeclaration,
    arguments: &[StructuralArgument],
    operation: OperationId,
) -> Result<(), ModuleError> {
    for requirement in &boundary.requires {
        let argument = &arguments[requirement.argument_index as usize];
        let actual = caller
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == argument.place)
            .expect("structural arguments were validated before requirements");
        if !structural_occurrence_carries_qualification(
            &actual.qualifications,
            &actual.projected_qualifications,
            &argument.path,
            requirement.domain,
        ) {
            return Err(ModuleError::BoundaryArgumentMissingQualification {
                operation,
                argument_index: requirement.argument_index,
                domain: requirement.domain,
            });
        }
    }
    Ok(())
}

fn validate_boundary_completion_receipts(
    caller: &TerminalMachine,
    arguments: &[StructuralArgument],
    receipts: &[CompletionReceipt],
    operation: OperationId,
) -> Result<(), ModuleError> {
    let expected = arguments
        .iter()
        .enumerate()
        .flat_map(|(index, argument)| {
            caller
                .entry_claims
                .iter()
                .filter_map(move |claim| {
                    (claim.input == argument.place
                        && (argument.path.is_empty() || claim.path == argument.path))
                        .then_some((index as u32, claim.claim))
                })
                .chain(caller.content_entry_claims.iter().filter_map(move |claim| {
                    (claim.input.root == argument.place).then_some((index as u32, claim.claim))
                }))
        })
        .collect::<BTreeSet<_>>();
    let mut actual = BTreeSet::new();
    let mut claims = BTreeSet::new();
    for receipt in receipts {
        if !actual.insert((receipt.argument_index, receipt.claim)) || !claims.insert(receipt.claim)
        {
            return Err(ModuleError::DuplicateBoundaryCompletionReceipt(operation));
        }
        let Some(argument) = arguments.get(receipt.argument_index as usize) else {
            return Err(ModuleError::ClaimActionArgumentOutOfRange {
                operation,
                argument_index: receipt.argument_index,
            });
        };
        let Some((claim_input, claim_path)) = claim_input(caller, receipt.claim) else {
            return Err(ModuleError::UnknownClaimAtOperation {
                operation,
                claim: receipt.claim,
            });
        };
        if claim_input != argument.place
            || (!argument.path.is_empty() && claim_path != argument.path.as_slice())
        {
            return Err(ModuleError::ClaimActionPlaceMismatch {
                operation,
                claim: receipt.claim,
                argument_index: receipt.argument_index,
            });
        }
    }
    if actual != expected {
        return Err(ModuleError::BoundaryCompletionReceiptMismatch(operation));
    }
    if receipts.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ModuleError::NonCanonicalBoundaryCompletionReceipts(
            operation,
        ));
    }
    Ok(())
}
