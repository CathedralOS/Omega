use super::boundary_settlements::claim_completion_only_boundary_is_exact;
use super::cleanup::validate_bounded_nominal_cleanup_body;
use super::shared::*;
use super::structural::exact_fully_consumed_affine_pair_root;
use super::structural_layout::{
    checked_align_up_u32, expected_maximal_residual_subtrees, is_partial_cleanup_path,
    resolve_structural_field_path, structural_shape,
};

pub(super) fn lower_unit_function(
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    installed_calls: &BTreeMap<
        (MachineId, OperationId, BoundaryMachineId),
        InstalledProviderUnitCallEvidence,
    >,
) -> Result<TargetFunction, LoweringError> {
    if !function.parameters.is_empty() {
        return Err(LoweringError::UnitFunctionHasScalarParameters(
            function.machine,
        ));
    }
    if function.block_entries.len() != 1
        || function.block_entries[0].block != function.entry
        || !function.block_entries[0].parameters.is_empty()
    {
        return Err(LoweringError::UnitFunctionNotStraightLine(function.machine));
    }

    let mut shape_cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let parameter_shapes = function
        .structural_parameters
        .iter()
        .map(|parameter| {
            structural_shape(
                parameter.structural_type,
                structural_types,
                &mut shape_cache,
                &mut active,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let signature = CallSignature {
        parameters: parameter_shapes.clone(),
        result: None,
    };
    let call_plan = evaluate_call_plan(CallingPolicy::native_for_target(target), &signature)
        .map_err(LoweringError::AbiPlan)?;
    if call_plan.parameters.len() != function.structural_parameters.len() {
        return Err(LoweringError::AbiParameterCountMismatch {
            expected: function.structural_parameters.len(),
            actual: call_plan.parameters.len(),
        });
    }
    let parameters = function
        .structural_parameters
        .iter()
        .zip(parameter_shapes)
        .zip(&call_plan.parameters)
        .map(
            |((parameter, shape), placement)| TargetStructuralParameter {
                place: parameter.place,
                structural_type: parameter.structural_type,
                multiplicity: parameter.multiplicity,
                access: parameter.access,
                shape,
                placement: placement.clone(),
            },
        )
        .collect::<Vec<_>>();
    let parameters_by_place = parameters
        .iter()
        .map(|parameter| (parameter.place, parameter))
        .collect::<BTreeMap<_, _>>();

    let mut operations = Vec::with_capacity(function.operations.len());
    let mut provenance = TerminalPsiProvenance::default();
    let mut returned = false;
    let mut established_byte_sequences =
        BTreeMap::<PlaceId, (OperationId, StructuralTypeDeclaration, Vec<u8>)>::new();
    let mut integer_constants =
        BTreeMap::<ValueId, (OperationId, IntegerType, IntegerValue)>::new();
    let mut nonreturning_boundary = false;
    for operation in &function.operations {
        if returned {
            return Err(LoweringError::OperationAfterReturn(function.machine));
        }
        match operation {
            AbstractOperation::EstablishPayloadlessCase { .. } => {
                return Err(LoweringError::UnsupportedStructuralReturn(function.machine));
            }
            AbstractOperation::EstablishByteSequenceLiteral {
                psi_operation,
                place,
                structural_type,
                bytes,
            } => {
                if nonreturning_boundary
                    || !matches!(
                        (&place.kind, &structural_type.shape),
                        (
                            psi_core::StructuralPlaceKind::ByteSequenceLiteral {
                                structural_type: place_type,
                                ..
                            },
                            StructuralTypeShape::ByteSequence(
                                psi_terminal::ByteSequenceCarrier::BorrowedView
                            )
                        ) if *place_type == structural_type.id
                    )
                    || established_byte_sequences
                        .insert(
                            place.id,
                            (*psi_operation, structural_type.clone(), bytes.clone()),
                        )
                        .is_some()
                {
                    return Err(LoweringError::UnsupportedOperationInUnitFunction(
                        function.machine,
                    ));
                }
                operations.push(TargetUnitOperation::EstablishByteSequenceLiteral {
                    psi_operation: *psi_operation,
                    place: place.clone(),
                    structural_type: structural_type.clone(),
                    bytes: bytes.clone(),
                });
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::EstablishTrivialAffineLocal {
                psi_operation,
                place,
                structural_type,
            } => {
                operations.push(TargetUnitOperation::EstablishTrivialAffineLocal {
                    psi_operation: *psi_operation,
                    place: place.clone(),
                    structural_type: structural_type.clone(),
                });
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::CallUnit {
                psi_operation,
                callee,
                structural_arguments,
                claim_transfers,
            } => {
                let callee_function = functions
                    .get(callee)
                    .copied()
                    .ok_or(LoweringError::UnknownCallTarget(*callee))?;
                if callee_function.result != AbstractFunctionResult::Unit
                    || !callee_function.parameters.is_empty()
                {
                    return Err(LoweringError::UnitCallTargetKindMismatch(*callee));
                }
                if structural_arguments.len() != callee_function.structural_parameters.len() {
                    return Err(LoweringError::StructuralCallArgumentCountMismatch {
                        callee: *callee,
                        expected: callee_function.structural_parameters.len(),
                        actual: structural_arguments.len(),
                    });
                }
                let callee_shapes = callee_function
                    .structural_parameters
                    .iter()
                    .map(|parameter| {
                        structural_shape(
                            parameter.structural_type,
                            structural_types,
                            &mut shape_cache,
                            &mut active,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let callee_plan = evaluate_call_plan(
                    CallingPolicy::native_for_target(target),
                    &CallSignature {
                        parameters: callee_shapes.clone(),
                        result: None,
                    },
                )
                .map_err(LoweringError::AbiPlan)?;
                let arguments = structural_arguments
                    .iter()
                    .zip(&callee_function.structural_parameters)
                    .zip(callee_shapes)
                    .zip(&callee_plan.parameters)
                    .map(|(((argument, callee_parameter), shape), destination)| {
                        let source = parameters_by_place.get(&argument.place).copied().ok_or(
                            LoweringError::UnknownStructuralArgumentPlace {
                                machine: function.machine,
                                place: argument.place,
                            },
                        )?;
                        let (
                            projected_type,
                            projected_shape,
                            source_byte_offset,
                            fixed_array_length,
                            element_stride,
                        ) = match argument.path.as_slice() {
                            [] => (source.structural_type, source.shape, 0, None, None),
                            [StructuralPathSegment::FixedIndex(index)] => {
                                let declaration = structural_types
                                    .get(&source.structural_type)
                                    .copied()
                                    .ok_or(LoweringError::UnknownStructuralType(
                                        source.structural_type,
                                    ))?;
                                let StructuralTypeShape::FixedArray { element, length } =
                                    declaration.shape
                                else {
                                    return Err(
                                        LoweringError::StructuralCallArgumentTypeMismatch {
                                            callee: *callee,
                                            place: argument.place,
                                        },
                                    );
                                };
                                if *index >= length {
                                    return Err(
                                        LoweringError::StructuralCallArgumentTypeMismatch {
                                            callee: *callee,
                                            place: argument.place,
                                        },
                                    );
                                }
                                let element_shape = structural_shape(
                                    element,
                                    structural_types,
                                    &mut shape_cache,
                                    &mut active,
                                )?;
                                let stride = checked_align_up_u32(
                                    u32::from(element_shape.byte_size),
                                    u32::from(element_shape.alignment),
                                )
                                .ok_or(
                                    LoweringError::StructuralTypeTooLarge(source.structural_type),
                                )?;
                                let offset = u64::from(stride)
                                    .checked_mul(*index)
                                    .and_then(|offset| u32::try_from(offset).ok())
                                    .ok_or(LoweringError::StructuralTypeTooLarge(
                                        source.structural_type,
                                    ))?;
                                (element, element_shape, offset, Some(length), Some(stride))
                            }
                            path @ [StructuralPathSegment::Field(_), ..]
                                if path.iter().all(|segment| {
                                    matches!(segment, StructuralPathSegment::Field(_))
                                }) =>
                            {
                                let (field_type, field_shape, offset) =
                                    resolve_structural_field_path(
                                        source.structural_type,
                                        path,
                                        structural_types,
                                        &mut shape_cache,
                                        &mut active,
                                    )
                                    .map_err(|_| {
                                        LoweringError::StructuralCallArgumentTypeMismatch {
                                            callee: *callee,
                                            place: argument.place,
                                        }
                                    })?;
                                (field_type, field_shape, offset, None, None)
                            }
                            _ => {
                                return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                                    callee: *callee,
                                    place: argument.place,
                                });
                            }
                        };
                        if projected_type != callee_parameter.structural_type
                            || projected_shape != shape
                            || u32::from(shape.byte_size)
                                .checked_add(source_byte_offset)
                                .is_none_or(|end| end > u32::from(source.shape.byte_size))
                        {
                            return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                                callee: *callee,
                                place: argument.place,
                            });
                        }
                        Ok(TargetStructuralArgument {
                            place: argument.place,
                            access: argument.access,
                            path: argument.path.clone(),
                            root_structural_type: source.structural_type,
                            structural_type: projected_type,
                            shape,
                            source_byte_offset,
                            fixed_array_length,
                            element_stride,
                            source: source.placement.clone(),
                            destination: destination.clone(),
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                operations.push(TargetUnitOperation::Call {
                    psi_operation: *psi_operation,
                    callee: *callee,
                    arguments,
                    claim_transfers: claim_transfers.clone(),
                });
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::PortWrite {
                psi_operation,
                service,
                port,
                value,
            } => {
                operations.push(TargetUnitOperation::PortWrite {
                    psi_operation: *psi_operation,
                    service: *service,
                    port: *port,
                    value: *value,
                });
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::BoundaryCall {
                psi_operation,
                result,
                boundary,
                arguments,
                structural_arguments,
                completion_claim_sources,
                completion_receipts,
            } => {
                if let Some(installed) =
                    installed_calls.get(&(function.machine, *psi_operation, *boundary))
                {
                    let callee = functions
                        .get(&installed.provider.candidate)
                        .copied()
                        .ok_or(LoweringError::UnknownCallTarget(
                            installed.provider.candidate,
                        ))?;
                    let declaration = boundary_machines
                        .get(boundary)
                        .copied()
                        .ok_or(LoweringError::UnknownBoundarySettlement(*boundary))?;
                    if result.is_some()
                        || !arguments.is_empty()
                        || callee.result != AbstractFunctionResult::Unit
                        || !callee.parameters.is_empty()
                        || structural_arguments.len() != callee.structural_parameters.len()
                        || declaration.structural_parameters.len()
                            != callee.structural_parameters.len()
                        || installed.provider.signature.parameters.len()
                            != callee.structural_parameters.len()
                    {
                        return Err(LoweringError::InstalledProviderCallShapeMismatch {
                            machine: function.machine,
                            operation: *psi_operation,
                            boundary: *boundary,
                        });
                    }
                    let callee_shapes = callee
                        .structural_parameters
                        .iter()
                        .map(|parameter| {
                            structural_shape(
                                parameter.structural_type,
                                structural_types,
                                &mut shape_cache,
                                &mut active,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let callee_plan = evaluate_call_plan(
                        CallingPolicy::native_for_target(target),
                        &CallSignature {
                            parameters: callee_shapes.clone(),
                            result: None,
                        },
                    )
                    .map_err(LoweringError::AbiPlan)?;
                    let mut target_arguments = Vec::with_capacity(structural_arguments.len());
                    for (index, (((argument, boundary_parameter), callee_parameter), signature)) in
                        structural_arguments
                            .iter()
                            .zip(&declaration.structural_parameters)
                            .zip(&callee.structural_parameters)
                            .zip(&installed.provider.signature.parameters)
                            .enumerate()
                    {
                        let source = parameters_by_place.get(&argument.place).copied().ok_or(
                            LoweringError::UnknownStructuralArgumentPlace {
                                machine: function.machine,
                                place: argument.place,
                            },
                        )?;
                        let caller_parameter = function
                            .structural_parameters
                            .iter()
                            .find(|parameter| parameter.place == argument.place)
                            .ok_or(LoweringError::UnknownStructuralArgumentPlace {
                                machine: function.machine,
                                place: argument.place,
                            })?;
                        if !argument.path.is_empty()
                            || argument.access != psi_terminal::StructuralAccess::Owned
                            || source.access != psi_terminal::StructuralAccess::Owned
                            || boundary_parameter.access != psi_terminal::StructuralAccess::Owned
                            || callee_parameter.access != psi_terminal::StructuralAccess::Owned
                            || boundary_parameter.position != index as u32
                            || callee_parameter.position != index as u32
                            || signature.position != index as u32
                            || boundary_parameter.is_self
                            || callee_parameter.is_self
                            || signature.is_self
                            || source.structural_type != boundary_parameter.structural_type
                            || source.structural_type != callee_parameter.structural_type
                            || source.structural_type != signature.structural_type
                            || source.multiplicity != boundary_parameter.multiplicity
                            || source.multiplicity != callee_parameter.multiplicity
                            || source.multiplicity != signature.multiplicity
                            || source.access != signature.access
                            || source.structural_type
                                != callee.structural_parameters[index].structural_type
                            || source.placement.shape != callee_shapes[index]
                            || source.placement.shape != callee_plan.parameters[index].shape
                            || caller_parameter.qualifications.iter().any(|qualification| {
                                !boundary_parameter.qualifications.contains(qualification)
                                    || !callee_parameter.qualifications.contains(qualification)
                                    || !signature.qualifications.contains(qualification)
                            })
                            || caller_parameter.qualifications != boundary_parameter.qualifications
                            || caller_parameter.qualifications != callee_parameter.qualifications
                            || caller_parameter.qualifications != signature.qualifications
                        {
                            return Err(LoweringError::InstalledProviderCallShapeMismatch {
                                machine: function.machine,
                                operation: *psi_operation,
                                boundary: *boundary,
                            });
                        }
                        target_arguments.push(TargetStructuralArgument {
                            place: argument.place,
                            access: argument.access,
                            path: Vec::new(),
                            root_structural_type: source.structural_type,
                            structural_type: source.structural_type,
                            shape: source.shape,
                            source_byte_offset: 0,
                            fixed_array_length: None,
                            element_stride: None,
                            source: source.placement.clone(),
                            destination: callee_plan.parameters[index].clone(),
                        });
                    }
                    let claim_transfers = completion_receipts
                        .iter()
                        .map(|receipt| psi_terminal::ClaimTransfer {
                            claim: receipt.claim,
                            argument_index: receipt.argument_index,
                        })
                        .collect::<Vec<_>>();
                    if completion_receipts.len() != callee.entry_claims.len()
                        || callee.entry_claims.iter().any(|entry| {
                            let Some(index) = callee
                                .structural_parameters
                                .iter()
                                .position(|parameter| parameter.place == entry.input)
                            else {
                                return true;
                            };
                            let Some(receipt) = completion_receipts
                                .iter()
                                .find(|receipt| receipt.argument_index as usize == index)
                            else {
                                return true;
                            };
                            let Some(source) = completion_claim_sources
                                .iter()
                                .find(|source| source.claim == receipt.claim)
                            else {
                                return true;
                            };
                            source.entry.as_ref().is_none_or(|source| {
                                source.input != structural_arguments[index].place
                                    || source.path != entry.path
                            })
                        })
                    {
                        return Err(LoweringError::InstalledProviderClaimTransferMismatch {
                            machine: function.machine,
                            operation: *psi_operation,
                            boundary: *boundary,
                        });
                    }
                    operations.push(TargetUnitOperation::InstalledProviderCall {
                        psi_operation: *psi_operation,
                        boundary: *boundary,
                        provider: installed.provider.clone(),
                        source_arguments: structural_arguments.clone(),
                        arguments: target_arguments,
                        claim_transfers,
                        completion_claim_sources: completion_claim_sources.clone(),
                        completion_receipts: completion_receipts.clone(),
                    });
                    provenance.operations.push(*psi_operation);
                    continue;
                }
                if result.is_some() {
                    return Err(
                        LoweringError::ResultBearingBoundarySettlementRequiresNativeRealization {
                            machine: function.machine,
                            operation: *psi_operation,
                            boundary: *boundary,
                        },
                    );
                }
                let binding = settlements
                    .get(boundary)
                    .copied()
                    .ok_or(LoweringError::MissingBoundarySettlement(*boundary))?;
                let declaration = boundary_machines
                    .get(boundary)
                    .copied()
                    .ok_or(LoweringError::UnknownBoundarySettlement(*boundary))?;
                let mut scalar_arguments = Vec::new();
                let mut byte_sequence_arguments = Vec::new();
                match binding.realization {
                    BoundaryRealization::MetadataOnlyPort(realization) => {
                        if !arguments.is_empty()
                            || !matches!(
                                operations.last(),
                                Some(TargetUnitOperation::PortWrite {
                                    psi_operation,
                                    service,
                                    port,
                                    value,
                                }) if *psi_operation == realization.effect_operation
                                    && *service == realization.service
                                    && *port == realization.port
                                    && *value == realization.value
                            )
                        {
                            return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                        }
                        for argument in structural_arguments {
                            if !parameters_by_place.contains_key(&argument.place) {
                                return Err(LoweringError::UnknownStructuralArgumentPlace {
                                    machine: function.machine,
                                    place: argument.place,
                                });
                            }
                        }
                    }
                    BoundaryRealization::ClaimCompletionOnly(_) => {
                        if !claim_completion_only_boundary_is_exact(
                            function,
                            declaration,
                            arguments,
                            structural_arguments,
                            completion_claim_sources,
                            completion_receipts,
                            &parameters_by_place,
                        ) {
                            return Err(LoweringError::InvalidClaimCompletionOnlyShape {
                                machine: function.machine,
                                operation: *psi_operation,
                                boundary: *boundary,
                            });
                        }
                    }
                    BoundaryRealization::LinuxWriteLine(_) => {
                        if target.object_format != ObjectFormat::Elf
                            || !matches!(
                                target.architecture,
                                Architecture::X86_64 | Architecture::Aarch64
                            )
                            || !arguments.is_empty()
                            || declaration.result.is_some()
                            || !declaration.scalar_parameters.is_empty()
                            || structural_arguments.len() != 1
                            || declaration.structural_parameters.len() != 1
                        {
                            return Err(LoweringError::LinuxWriteLineUnsupportedOrInvalid {
                                machine: function.machine,
                                boundary: *boundary,
                                target,
                            });
                        }
                        let argument = &structural_arguments[0];
                        let parameter = &declaration.structural_parameters[0];
                        let Some((literal_operation, structural_type, bytes)) =
                            established_byte_sequences.get(&argument.place)
                        else {
                            return Err(LoweringError::LinuxWriteLineUnsupportedOrInvalid {
                                machine: function.machine,
                                boundary: *boundary,
                                target,
                            });
                        };
                        if !argument.path.is_empty()
                            || parameter.position != 0
                            || parameter.is_self
                            || parameter.structural_type != structural_type.id
                            || parameter.multiplicity
                                != psi_terminal::StructuralMultiplicity::Unrestricted
                            || !parameter.qualifications.is_empty()
                        {
                            return Err(LoweringError::LinuxWriteLineUnsupportedOrInvalid {
                                machine: function.machine,
                                boundary: *boundary,
                                target,
                            });
                        }
                        byte_sequence_arguments.push(BoundaryByteSequenceArgument {
                            argument: argument.clone(),
                            literal_operation: *literal_operation,
                            structural_type: structural_type.clone(),
                            bytes: bytes.clone(),
                        });
                    }
                    BoundaryRealization::LinuxExitGroupI32(_) => {
                        let i32_type =
                            IntegerType::new(IntegerSign::Signed, 32).expect("i32 is valid");
                        let [argument] = arguments.as_slice() else {
                            return Err(LoweringError::InvalidLinuxExitGroupShape(
                                function.machine,
                            ));
                        };
                        let Some((_, actual_type, value)) = integer_constants.get(argument) else {
                            return Err(LoweringError::InvalidLinuxExitGroupShape(
                                function.machine,
                            ));
                        };
                        if target.object_format != ObjectFormat::Elf
                            || !matches!(
                                target.architecture,
                                Architecture::X86_64 | Architecture::Aarch64
                            )
                            || declaration.scalar_parameters.as_slice()
                                != [ScalarType::Integer(i32_type)]
                            || !declaration.structural_parameters.is_empty()
                            || declaration.result.is_some()
                            || *actual_type != i32_type
                            || !i32_type.admits(*value)
                            || !structural_arguments.is_empty()
                        {
                            return Err(LoweringError::InvalidLinuxExitGroupShape(
                                function.machine,
                            ));
                        }
                        scalar_arguments.push(BoundaryScalarArgument {
                            source_value: *argument,
                            scalar_type: ScalarType::Integer(*actual_type),
                            immediate: *value,
                            destination: match target.architecture {
                                Architecture::X86_64 => MachineRegister::X86Rdi,
                                Architecture::Aarch64 => MachineRegister::Aarch64X(0),
                            },
                        });
                        nonreturning_boundary = true;
                    }
                    BoundaryRealization::DirectPortReadU8(_) => {
                        return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                    }
                }
                operations.push(TargetUnitOperation::BoundarySettlement {
                    psi_operation: *psi_operation,
                    boundary: *boundary,
                    provider_execution: binding.provider_execution,
                    realization: binding.realization,
                    scalar_arguments,
                    arguments: structural_arguments.clone(),
                    byte_sequence_arguments,
                    completion_claim_sources: completion_claim_sources.clone(),
                    completion_receipts: completion_receipts.clone(),
                });
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::ReturnUnit {
                psi_edge,
                cleanup_actions,
            } => {
                if nonreturning_boundary && !cleanup_actions.is_empty() {
                    return Err(LoweringError::InvalidLinuxExitGroupShape(function.machine));
                }
                let local_places = operations
                    .iter()
                    .filter_map(|operation| match operation {
                        TargetUnitOperation::EstablishTrivialAffineLocal { place, .. } => {
                            Some(place.id)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let fully_consumed_affine_pair = exact_fully_consumed_affine_pair_root(
                    function,
                    &parameters,
                    &operations,
                    structural_types,
                    functions,
                );
                let expected_roots = local_places
                    .iter()
                    .rev()
                    .copied()
                    .chain(
                        function
                            .structural_parameters
                            .iter()
                            .rev()
                            .filter(|parameter| {
                                parameter.multiplicity
                                    == psi_terminal::StructuralMultiplicity::Affine
                                    && Some(parameter.place) != fully_consumed_affine_pair
                            })
                            .map(|parameter| parameter.place),
                    )
                    .collect::<Vec<_>>();
                let root_discards = cleanup_actions
                    .iter()
                    .filter_map(|action| match action {
                        psi_terminal::TerminalAffineCleanupAction::DiscardRoot(place) => {
                            Some(*place)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let residual_discards = cleanup_actions
                    .iter()
                    .filter_map(|action| match action {
                        psi_terminal::TerminalAffineCleanupAction::DiscardResidual(discard) => {
                            Some(discard)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let nominal_cleanups = cleanup_actions
                    .iter()
                    .filter_map(|action| match action {
                        psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                            Some(cleanup.clone())
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if root_discards.len() + residual_discards.len() + nominal_cleanups.len()
                    != cleanup_actions.len()
                {
                    unreachable!("every cleanup action has one exact kind")
                }
                if residual_discards.is_empty()
                    && nominal_cleanups.is_empty()
                    && (root_discards != expected_roots
                        || operations.iter().any(|operation| {
                            matches!(operation,
                            TargetUnitOperation::Call { arguments, .. }
                                if arguments.iter().any(|argument| {
                                    !argument.path.is_empty()
                                        && root_discards.contains(&argument.place)
                                }))
                        }))
                {
                    return Err(LoweringError::UnsupportedOperationInUnitFunction(
                        function.machine,
                    ));
                }
                if !residual_discards.is_empty() {
                    let Some(residual_root) =
                        residual_discards.first().map(|discard| discard.place)
                    else {
                        unreachable!("nonempty residual cleanup has a root")
                    };
                    let Some(parameter) = function
                        .structural_parameters
                        .iter()
                        .find(|parameter| parameter.place == residual_root)
                    else {
                        return Err(LoweringError::UnsupportedOperationInUnitFunction(
                            function.machine,
                        ));
                    };
                    let moved_arguments = operations
                        .iter()
                        .filter_map(|operation| match operation {
                            TargetUnitOperation::Call { arguments, .. } => Some(arguments),
                            _ => None,
                        })
                        .flatten()
                        .filter(|argument| argument.place == residual_root)
                        .collect::<Vec<_>>();
                    let mut moved_subtrees = Vec::with_capacity(moved_arguments.len());
                    if moved_arguments.is_empty()
                        || moved_arguments.iter().any(|argument| {
                            argument.root_structural_type != parameter.structural_type
                                || !is_partial_cleanup_path(&argument.path)
                                || moved_subtrees
                                    .iter()
                                    .any(|(path, _)| path == &argument.path)
                                || {
                                    moved_subtrees
                                        .push((argument.path.clone(), argument.structural_type));
                                    false
                                }
                        })
                    {
                        return Err(LoweringError::UnsupportedOperationInUnitFunction(
                            function.machine,
                        ));
                    }
                    let Some(expected_residuals) = expected_maximal_residual_subtrees(
                        parameter.structural_type,
                        &moved_subtrees,
                        structural_types,
                    ) else {
                        return Err(LoweringError::UnsupportedOperationInUnitFunction(
                            function.machine,
                        ));
                    };
                    let fixed_array_call_count = structural_types
                        .get(&parameter.structural_type)
                        .and_then(|declaration| match declaration.shape {
                            StructuralTypeShape::FixedArray { length: 2, .. } => Some(1),
                            StructuralTypeShape::FixedArray { length: 3, .. } => Some(2),
                            _ => None,
                        });
                    if parameter.multiplicity != psi_terminal::StructuralMultiplicity::Affine
                        || fixed_array_call_count.is_some_and(|expected_calls| {
                            function.structural_parameters.len() != 1
                                || !function.entry_claims.is_empty()
                                || !function.published_service_ceiling.is_empty()
                                || parameter.position != 0
                                || parameter.is_self
                                || parameter.access != psi_terminal::StructuralAccess::Owned
                                || !parameter.qualifications.is_empty()
                                || !local_places.is_empty()
                                || operations.len() != expected_calls
                                || operations.iter().any(|operation| {
                                    !matches!(operation, TargetUnitOperation::Call { .. })
                                })
                        })
                        || root_discards != local_places.iter().rev().copied().collect::<Vec<_>>()
                        || expected_roots.get(local_places.len()..) != Some(&[residual_root][..])
                        || expected_residuals.len() != residual_discards.len()
                        || cleanup_actions.get(..root_discards.len()).is_none_or(|prefix| {
                            !prefix.iter().zip(&root_discards).all(|(action, place)| {
                                matches!(action,
                                    psi_terminal::TerminalAffineCleanupAction::DiscardRoot(actual)
                                        if actual == place)
                            })
                        })
                        || cleanup_actions.get(root_discards.len()..).is_none_or(|suffix| {
                            suffix.iter().zip(&expected_residuals).any(
                                |(action, (path, structural_type))| {
                                    !matches!(action,
                                        psi_terminal::TerminalAffineCleanupAction::DiscardResidual(discard)
                                            if discard.place == residual_root
                                                && discard.path == *path
                                                && discard.structural_type == *structural_type)
                                },
                            )
                        })
                    {
                        return Err(LoweringError::UnsupportedOperationInUnitFunction(
                            function.machine,
                        ));
                    }
                }
                if !nominal_cleanups.is_empty() {
                    if !local_places.is_empty()
                        || !root_discards.is_empty()
                        || !residual_discards.is_empty()
                        || nominal_cleanups.is_empty()
                        || function.structural_parameters.len() != nominal_cleanups.len()
                        || function
                            .structural_parameters
                            .iter()
                            .rev()
                            .zip(&nominal_cleanups)
                            .any(|(parameter, cleanup)| {
                                parameter.place != cleanup.place
                                    || parameter.structural_type != cleanup.structural_type
                                    || parameter.multiplicity
                                        != psi_terminal::StructuralMultiplicity::Affine
                            })
                    {
                        return Err(LoweringError::UnsupportedOperationInUnitFunction(
                            function.machine,
                        ));
                    }
                    for cleanup in &nominal_cleanups {
                        let Some(cleanup_function) =
                            functions.get(&cleanup.cleanup_machine).copied()
                        else {
                            return Err(LoweringError::UnsupportedOperationInUnitFunction(
                                function.machine,
                            ));
                        };
                        if cleanup_function.attachment != Some(cleanup.structural_type)
                            || cleanup_function.result != AbstractFunctionResult::Unit
                            || !cleanup_function.parameters.is_empty()
                            || !cleanup_function.structural_parameters.is_empty()
                            || !cleanup_function.entry_claims.is_empty()
                            || !cleanup_function.published_service_ceiling.is_empty()
                            || cleanup_function.block_entries.as_slice()
                                != [omega_abstract_operations::AbstractBlockEntry {
                                    block: cleanup_function.entry,
                                    parameters: Vec::new(),
                                    operation_offset: 0,
                                }]
                        {
                            return Err(LoweringError::UnsupportedOperationInUnitFunction(
                                function.machine,
                            ));
                        }
                        validate_bounded_nominal_cleanup_body(
                            function.machine,
                            cleanup,
                            cleanup_function,
                            functions,
                            structural_types,
                        )?;
                    }
                }
                if !nominal_cleanups.is_empty()
                    && nominal_cleanups.len() + root_discards.len() + residual_discards.len()
                        != cleanup_actions.len()
                {
                    return Err(LoweringError::UnsupportedOperationInUnitFunction(
                        function.machine,
                    ));
                }
                operations.push(TargetUnitOperation::Return {
                    psi_edge: *psi_edge,
                    cleanup_actions: cleanup_actions.clone(),
                });
                provenance.edges.push(*psi_edge);
                returned = true;
            }
            AbstractOperation::IntegerConstant {
                psi_operation,
                result,
                scalar_type: ScalarType::Integer(scalar_type),
                value,
            } => {
                if nonreturning_boundary
                    || integer_constants
                        .insert(*result, (*psi_operation, *scalar_type, *value))
                        .is_some()
                {
                    return Err(LoweringError::UnsupportedOperationInUnitFunction(
                        function.machine,
                    ));
                }
                operations.push(TargetUnitOperation::IntegerConstant {
                    psi_operation: *psi_operation,
                    result: *result,
                    scalar_type: *scalar_type,
                    value: *value,
                });
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::Crash { .. }
            | AbstractOperation::Call { .. }
            | AbstractOperation::CallStructuralScalar { .. }
            | AbstractOperation::CallStructural { .. }
            | AbstractOperation::IntegerConstant { .. }
            | AbstractOperation::BooleanConstant { .. }
            | AbstractOperation::BooleanStructuralField { .. }
            | AbstractOperation::BooleanNot { .. }
            | AbstractOperation::BooleanEqual { .. }
            | AbstractOperation::IntegerEqual { .. }
            | AbstractOperation::IntegerLessThan { .. }
            | AbstractOperation::IntegerLessOrEqual { .. }
            | AbstractOperation::IntegerBitwiseNot { .. }
            | AbstractOperation::IntegerWiden { .. }
            | AbstractOperation::IntegerExactCast { .. }
            | AbstractOperation::IntegerBitwiseAnd { .. }
            | AbstractOperation::IntegerBitwiseOr { .. }
            | AbstractOperation::IntegerBitwiseXor { .. }
            | AbstractOperation::WrappingIntegerShiftLeft { .. }
            | AbstractOperation::WrappingIntegerShiftRight { .. }
            | AbstractOperation::ExactIntegerShiftLeft { .. }
            | AbstractOperation::ExactIntegerShiftRight { .. }
            | AbstractOperation::WrappingIntegerAdd { .. }
            | AbstractOperation::ExactIntegerAdd { .. }
            | AbstractOperation::SaturatingIntegerAdd { .. }
            | AbstractOperation::WrappingIntegerSubtract { .. }
            | AbstractOperation::ExactIntegerSubtract { .. }
            | AbstractOperation::SaturatingIntegerSubtract { .. }
            | AbstractOperation::WrappingIntegerMultiply { .. }
            | AbstractOperation::ExactIntegerMultiply { .. }
            | AbstractOperation::SaturatingIntegerMultiply { .. }
            | AbstractOperation::ExactIntegerDivide { .. }
            | AbstractOperation::ExactIntegerRemainder { .. }
            | AbstractOperation::WrappingIntegerDivide { .. }
            | AbstractOperation::WrappingIntegerRemainder { .. }
            | AbstractOperation::SaturatingIntegerDivide { .. }
            | AbstractOperation::SaturatingIntegerRemainder { .. }
            | AbstractOperation::Jump { .. }
            | AbstractOperation::Conditional { .. }
            | AbstractOperation::Return { .. }
            | AbstractOperation::ReturnStructural { .. } => {
                return Err(LoweringError::UnsupportedOperationInUnitFunction(
                    function.machine,
                ));
            }
        }
    }
    if !returned {
        return Err(LoweringError::FunctionHasNoReturn(function.machine));
    }
    Ok(TargetFunction {
        machine: function.machine,
        attachment: function.attachment,
        provenance,
        operation: TargetOperation::UnitBody(TargetUnitBody {
            structural_types: structural_types
                .values()
                .map(|declaration| (*declaration).clone())
                .collect(),
            call_plan,
            parameters,
            operations,
        }),
    })
}
