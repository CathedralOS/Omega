//! Boundary settlement and admitted-provider Unit-call lowering.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_boundary_call(
    operation: &AbstractOperation,
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
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    mut shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    mut active: &mut BTreeSet<StructuralTypeId>,
    established_byte_sequences: &BTreeMap<
        PlaceId,
        (OperationId, StructuralTypeDeclaration, Vec<u8>),
    >,
    integer_constants: &BTreeMap<ValueId, (OperationId, IntegerType, IntegerValue)>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
    nonreturning_boundary: &mut bool,
) -> Result<(), LoweringError> {
    match operation {
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
                    || declaration.structural_parameters.len() != callee.structural_parameters.len()
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
                return Ok(());
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
                .cloned()
                .ok_or(LoweringError::MissingBoundarySettlement(*boundary))?;
            let declaration = boundary_machines
                .get(boundary)
                .copied()
                .ok_or(LoweringError::UnknownBoundarySettlement(*boundary))?;
            if let omega_target_operations::BoundarySettlementRealization::NormalizedForeignCall(
                foreign,
            ) = &binding.realization
            {
                let scalar_arguments = lower_normalized_foreign_scalar_arguments(
                    *boundary,
                    declaration,
                    arguments,
                    &foreign.boundary_entry_plan,
                    integer_constants,
                )?;
                if arguments.len() != declaration.scalar_parameters.len()
                    || !structural_arguments.is_empty()
                    || !completion_claim_sources.is_empty()
                    || !completion_receipts.is_empty()
                    || !declaration.structural_parameters.is_empty()
                    || declaration.result.is_some()
                    || target.object_format != ObjectFormat::Elf
                    || !matches!(
                        foreign.locator.locator(),
                        omega_target::ForeignLocatorCandidate::ElfVersioned { .. }
                    )
                    || foreign.boundary_entry_plan.call.policy
                        != omega_calling_conventions::CallingPolicy::native_for_target(target)
                    || foreign.boundary_entry_plan.call.entry_control
                        != omega_calling_conventions::EntryControl::CallReturn
                    || foreign.locator.target().native_target() != target
                {
                    return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                }
                operations.push(TargetUnitOperation::NormalizedForeignCall {
                    psi_operation: *psi_operation,
                    boundary: *boundary,
                    provider_execution: binding.provider_execution,
                    binding: foreign.clone(),
                    scalar_arguments,
                });
                provenance.operations.push(*psi_operation);
                return Ok(());
            }
            let omega_target_operations::BoundarySettlementRealization::Builtin(realization) =
                binding.realization
            else {
                unreachable!("normalized foreign settlement returns above")
            };
            let mut scalar_arguments = Vec::new();
            let mut byte_sequence_arguments = Vec::new();
            match realization {
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
                    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32 is valid");
                    let [argument] = arguments.as_slice() else {
                        return Err(LoweringError::InvalidLinuxExitGroupShape(function.machine));
                    };
                    let Some((_, actual_type, value)) = integer_constants.get(argument) else {
                        return Err(LoweringError::InvalidLinuxExitGroupShape(function.machine));
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
                        return Err(LoweringError::InvalidLinuxExitGroupShape(function.machine));
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
                    *nonreturning_boundary = true;
                }
                BoundaryRealization::DirectPortReadU8(_) => {
                    return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                }
            }
            operations.push(TargetUnitOperation::BoundarySettlement {
                psi_operation: *psi_operation,
                boundary: *boundary,
                provider_execution: binding.provider_execution,
                realization,
                scalar_arguments,
                arguments: structural_arguments.clone(),
                byte_sequence_arguments,
                completion_claim_sources: completion_claim_sources.clone(),
                completion_receipts: completion_receipts.clone(),
            });
            provenance.operations.push(*psi_operation);
        }
        _ => unreachable!("boundary-call routing admits only boundary calls"),
    }
    Ok(())
}

fn lower_normalized_foreign_scalar_arguments(
    boundary: BoundaryMachineId,
    declaration: &psi_terminal::BoundaryMachineDeclaration,
    arguments: &[ValueId],
    boundary_entry_plan: &omega_calling_conventions::BoundaryEntryPlan,
    integer_constants: &BTreeMap<ValueId, (OperationId, IntegerType, IntegerValue)>,
) -> Result<Vec<omega_target_operations::NormalizedForeignScalarArgument>, LoweringError> {
    if declaration.scalar_parameters.len() > 2 {
        return Err(LoweringError::BoundaryRealizationMismatch(boundary));
    }
    let scalar_parameter_shapes = declaration
        .scalar_parameters
        .iter()
        .map(|parameter| {
            let ScalarType::Integer(integer_type) = parameter else {
                return Err(LoweringError::BoundaryRealizationMismatch(boundary));
            };
            if integer_type.carrier() != psi_core::IntegerCarrier::Fixed
                || !matches!(integer_type.bits(), 8 | 16 | 32 | 64)
            {
                return Err(LoweringError::BoundaryRealizationMismatch(boundary));
            }
            let bytes = integer_type.bits().div_ceil(8);
            Ok(ValueShape::integer(bytes, bytes.next_power_of_two().min(8)))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let signature = CallSignature {
        parameters: scalar_parameter_shapes.clone(),
        result: None,
    };
    let validated = omega_calling_conventions::validate_boundary_entry_plan(
        boundary_entry_plan.clone(),
        &signature,
    )
    .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?;
    if arguments.len() != declaration.scalar_parameters.len()
        || validated.plan() != boundary_entry_plan
    {
        return Err(LoweringError::BoundaryRealizationMismatch(boundary));
    }
    arguments
        .iter()
        .zip(&declaration.scalar_parameters)
        .zip(&scalar_parameter_shapes)
        .zip(&boundary_entry_plan.call.parameters)
        .enumerate()
        .map(
            |(parameter_index, (((source_value, parameter), shape), placement))| {
                let ScalarType::Integer(integer_type) = parameter else {
                    return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                };
                let Some((_, actual_type, immediate)) = integer_constants.get(source_value) else {
                    return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                };
                let [
                    ValueLocation::Register {
                        value_byte_offset: 0,
                        byte_size,
                        ..
                    },
                ] = placement.locations.as_slice()
                else {
                    return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                };
                if actual_type != integer_type
                    || placement.shape != *shape
                    || u16::try_from(shape.byte_size) != Ok(*byte_size)
                    || psi_core::ScalarTerm::integer(*integer_type, *immediate).is_err()
                {
                    return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                }
                Ok(omega_target_operations::NormalizedForeignScalarArgument {
                    source_value: *source_value,
                    scalar_type: *integer_type,
                    immediate: *immediate,
                    parameter_index: u32::try_from(parameter_index)
                        .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?,
                    placement: placement.clone(),
                })
            },
        )
        .collect()
}

#[cfg(test)]
mod normalized_foreign_scalar_tests {
    use super::*;

    fn declaration(
        boundary: BoundaryMachineId,
        scalar_parameters: Vec<ScalarType>,
    ) -> psi_terminal::BoundaryMachineDeclaration {
        psi_terminal::BoundaryMachineDeclaration {
            id: boundary,
            identity: "Foreign::leaf".into(),
            attachment: None,
            scalar_parameters,
            structural_parameters: Vec::new(),
            result: None,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        }
    }

    fn entry_plan(
        target: NativeTarget,
        scalar_types: &[IntegerType],
    ) -> omega_calling_conventions::BoundaryEntryPlan {
        omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: scalar_types
                    .iter()
                    .map(|scalar_type| {
                        let bytes = scalar_type.bits().div_ceil(8);
                        ValueShape::integer(bytes, bytes.next_power_of_two().min(8))
                    })
                    .collect(),
                result: None,
            },
        )
        .expect("evaluated entry plan")
        .plan()
        .clone()
    }

    #[test]
    fn fixed_integer_literal_preserves_source_type_value_order_and_register_placement() {
        let boundary = BoundaryMachineId::new(41).expect("boundary");
        let source = ValueId::new(42).expect("source");
        let constant = OperationId::new(43).expect("constant");
        let integer_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
        let declaration = declaration(boundary, vec![ScalarType::Integer(integer_type)]);
        let constants =
            BTreeMap::from([(source, (constant, integer_type, IntegerValue::Signed(-17)))]);

        for (target, expected_register) in [
            (NativeTarget::linux_x64(), MachineRegister::X86Rdi),
            (NativeTarget::linux_arm64(), MachineRegister::Aarch64X(0)),
        ] {
            let plan = entry_plan(target, &[integer_type]);
            let arguments = lower_normalized_foreign_scalar_arguments(
                boundary,
                &declaration,
                &[source],
                &plan,
                &constants,
            )
            .expect("one evaluated literal argument");
            let [argument] = arguments.as_slice() else {
                panic!("one argument")
            };
            assert_eq!(argument.source_value, source);
            assert_eq!(argument.scalar_type, integer_type);
            assert_eq!(argument.immediate, IntegerValue::Signed(-17));
            assert_eq!(argument.parameter_index, 0);
            assert_eq!(argument.placement, plan.call.parameters[0]);
            assert!(matches!(
                argument.placement.locations.as_slice(),
                [ValueLocation::Register { register, .. }] if *register == expected_register
            ));
        }
    }

    #[test]
    fn two_fixed_integer_literals_preserve_ordered_occurrence_custody() {
        let boundary = BoundaryMachineId::new(45).expect("boundary");
        let first = ValueId::new(46).expect("first source");
        let second = ValueId::new(47).expect("second source");
        let i16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
        let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
        let declaration = declaration(
            boundary,
            vec![ScalarType::Integer(i16_type), ScalarType::Integer(i64_type)],
        );
        let constants = BTreeMap::from([
            (
                first,
                (
                    OperationId::new(48).expect("first constant"),
                    i16_type,
                    IntegerValue::Unsigned(513),
                ),
            ),
            (
                second,
                (
                    OperationId::new(49).expect("second constant"),
                    i64_type,
                    IntegerValue::Signed(-29),
                ),
            ),
        ]);

        for (target, expected_registers) in [
            (
                NativeTarget::linux_x64(),
                [MachineRegister::X86Rdi, MachineRegister::X86Rsi],
            ),
            (
                NativeTarget::linux_arm64(),
                [MachineRegister::Aarch64X(0), MachineRegister::Aarch64X(1)],
            ),
        ] {
            let plan = entry_plan(target, &[i16_type, i64_type]);
            let arguments = lower_normalized_foreign_scalar_arguments(
                boundary,
                &declaration,
                &[first, second],
                &plan,
                &constants,
            )
            .expect("two evaluated register literal arguments");
            assert_eq!(arguments.len(), 2);
            for (index, (argument, expected_register)) in
                arguments.iter().zip(expected_registers).enumerate()
            {
                assert_eq!(argument.source_value, [first, second][index]);
                assert_eq!(argument.scalar_type, [i16_type, i64_type][index]);
                assert_eq!(argument.parameter_index, index as u32);
                assert_eq!(argument.placement, plan.call.parameters[index]);
                assert!(matches!(
                    argument.placement.locations.as_slice(),
                    [ValueLocation::Register { register, .. }] if *register == expected_register
                ));
            }
            assert_eq!(arguments[0].immediate, IntegerValue::Unsigned(513));
            assert_eq!(arguments[1].immediate, IntegerValue::Signed(-29));

            let mut stack_plan = plan;
            stack_plan.call.parameters[1].locations = vec![ValueLocation::Stack {
                stack_byte_offset: 0,
                value_byte_offset: 0,
                byte_size: 8,
                alignment: 8,
            }];
            assert!(
                lower_normalized_foreign_scalar_arguments(
                    boundary,
                    &declaration,
                    &[first, second],
                    &stack_plan,
                    &constants,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn zero_argument_leaf_stays_valid_and_scalar_mutations_fail_closed() {
        let boundary = BoundaryMachineId::new(51).expect("boundary");
        let source = ValueId::new(52).expect("source");
        let constant = OperationId::new(53).expect("constant");
        let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
        let zero_plan = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::native_for_target(NativeTarget::linux_x64()),
            &CallSignature::default(),
        )
        .expect("zero-argument plan")
        .plan()
        .clone();
        assert_eq!(
            lower_normalized_foreign_scalar_arguments(
                boundary,
                &declaration(boundary, Vec::new()),
                &[],
                &zero_plan,
                &BTreeMap::new(),
            ),
            Ok(Vec::new())
        );

        let one_parameter_declaration = declaration(boundary, vec![ScalarType::Integer(i32_type)]);
        let plan = entry_plan(NativeTarget::linux_x64(), &[i32_type]);
        let constants = BTreeMap::from([(source, (constant, i32_type, IntegerValue::Signed(9)))]);
        for (arguments, constants) in [
            (Vec::new(), constants.clone()),
            (vec![source], BTreeMap::new()),
            (
                vec![source],
                BTreeMap::from([(source, (constant, i32_type, IntegerValue::Unsigned(9)))]),
            ),
        ] {
            assert!(matches!(
                lower_normalized_foreign_scalar_arguments(
                    boundary,
                    &one_parameter_declaration,
                    &arguments,
                    &plan,
                    &constants,
                ),
                Err(LoweringError::BoundaryRealizationMismatch(actual)) if actual == boundary
            ));
        }

        let mut stack_plan = plan.clone();
        stack_plan.call.parameters[0].locations = vec![ValueLocation::Stack {
            stack_byte_offset: 0,
            value_byte_offset: 0,
            byte_size: 4,
            alignment: 4,
        }];
        let mut result_plan = plan.clone();
        result_plan.call.result = Some(plan.call.parameters[0].clone());
        for invalid in [stack_plan, result_plan] {
            assert!(
                lower_normalized_foreign_scalar_arguments(
                    boundary,
                    &one_parameter_declaration,
                    &[source],
                    &invalid,
                    &constants,
                )
                .is_err()
            );
        }

        let three_parameter_declaration = declaration(
            boundary,
            vec![
                ScalarType::Integer(i32_type),
                ScalarType::Integer(i32_type),
                ScalarType::Integer(i32_type),
            ],
        );
        let three_plan = entry_plan(NativeTarget::linux_x64(), &[i32_type, i32_type, i32_type]);
        assert!(
            lower_normalized_foreign_scalar_arguments(
                boundary,
                &three_parameter_declaration,
                &[source, source, source],
                &three_plan,
                &constants,
            )
            .is_err()
        );
    }
}
