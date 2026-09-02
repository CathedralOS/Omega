use super::*;
use omega_calling_conventions::{CallSignature, CallingPolicy, evaluate_call_plan};
use omega_target::NativeTarget;
use omega_target_operations::{
    BoundaryByteSequenceArgument, BoundaryScalarArgument, LinuxExitGroupI32Realization,
    LinuxWriteLineRealization, MetadataOnlyPortRealization, NormalizedForeignCallBinding,
    NormalizedForeignScalarArgument, ProviderExecutionBinding, ProviderPlanReportIdentity,
    ScalarParameterLocation, TargetBooleanControl, TargetBooleanExpression, TargetCallArgument,
    TargetConditionalBooleanArm, TargetConditionalIntegerArm, TargetFunction,
    TargetIeeeFloatFmaOperand, TargetIntegerControl, TargetIntegerExpression, TargetOperation,
    TargetOperationPlan, TargetScalarExpression, TargetStructuralArgument,
    TargetStructuralParameter, TargetUnitBody, TargetUnitOperation, TargetX86ScalarFmaSettlement,
    TerminalPsiProvenance,
};
use omega_target_operations_to_assigned_target_operations::assign_registers;
use psi_core::{
    BoundaryMachineId, EdgeId, IeeeFloatFormat, IeeeFloatValue, MachineId, ObligationId,
    OperationId, PlaceId, ServiceId, StructuralFieldId, StructuralTypeId, ValueId,
};

fn proof_obligation() -> ObligationId {
    ObligationId::new(1).expect("proof obligation")
}
use psi_terminal::{
    ByteSequenceCarrier, NominalAffineCleanup, SemanticFingerprint, StructuralAccess,
    StructuralArgument, StructuralMultiplicity, StructuralPathSegment, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalPsiIdentity, VocabularyMarker,
};

#[path = "tests/forwarded_dynamic_descriptor.rs"]
mod forwarded_dynamic_descriptor;
#[path = "tests/installed_provider_scalar.rs"]
mod installed_provider_scalar;

fn emit_machine_code(plan: &TargetOperationPlan) -> Result<MachineCodePlan, EmissionError> {
    let assigned = assign_registers(plan).expect("test target operations must assign");
    super::emit_machine_code(&assigned)
}

#[test]
fn assigned_x86_fma_emits_raw_bits_exact_plan_custody_and_canonical_mxcsr() {
    let target = NativeTarget::linux_x64();
    let profile = omega_target::TargetProfile::LinuxX64;
    let provider = omega_target::AdmittedX86ScalarFmaProvider::from_deployment_claim(
        profile,
        &omega_target::X86_SCALAR_FMA_REQUIRED_FEATURES,
    )
    .unwrap();
    for (format, values) in [
        (
            IeeeFloatFormat::Binary32,
            [
                IeeeFloatValue::Binary32(0x3f80_0001),
                IeeeFloatValue::Binary32(0x3f7f_fffe),
                IeeeFloatValue::Binary32(0xbf80_0000),
            ],
        ),
        (
            IeeeFloatFormat::Binary64,
            [
                IeeeFloatValue::Binary64(0x3ff0_0000_0000_0001),
                IeeeFloatValue::Binary64(0x3fef_ffff_ffff_fffe),
                IeeeFloatValue::Binary64(0xbff0_0000_0000_0000),
            ],
        ),
    ] {
        let machine = MachineId::new(880).unwrap();
        let operations = [881, 882, 883, 884].map(|id| OperationId::new(id).unwrap());
        let values_id = [881, 882, 883, 884].map(|id| ValueId::new(id).unwrap());
        let edge = EdgeId::new(885).unwrap();
        let slot = match format {
            IeeeFloatFormat::Binary32 => omega_target::X86ScalarFmaSlot::Binary32,
            IeeeFloatFormat::Binary64 => omega_target::X86ScalarFmaSlot::Binary64,
        };
        let operand = |index: usize| TargetIeeeFloatFmaOperand {
            defining_operation: operations[index],
            source_value: values_id[index],
            value: values[index],
        };
        let settlement = TargetX86ScalarFmaSettlement {
            terminal_operation: operations[3],
            provider_plan_report_identity: 0xfeed,
            provider_plan_digest: [0xa5; 32],
            format,
            slot,
            provider,
        };
        let plan = TargetOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([0x88; 32]),
            },
            target,
            entry: machine,
            functions: vec![TargetFunction {
                machine,
                attachment: None,
                fixed_integer_scalar_abi: None,
                mixed_structural_scalar_abi: None,
                provenance: TerminalPsiProvenance {
                    operations: operations.to_vec(),
                    edges: vec![edge],
                },
                operation: TargetOperation::UnitBody(TargetUnitBody {
                    structural_types: Vec::new(),
                    call_plan: evaluate_call_plan(
                        CallingPolicy::native_for_target(target),
                        &CallSignature {
                            parameters: Vec::new(),
                            result: None,
                        },
                    )
                    .unwrap(),
                    scalar_parameters: Vec::new(),
                    parameters: Vec::new(),
                    operations: vec![
                        TargetUnitOperation::IeeeFloatConstant {
                            psi_operation: operations[0],
                            result: values_id[0],
                            value: values[0],
                        },
                        TargetUnitOperation::IeeeFloatConstant {
                            psi_operation: operations[1],
                            result: values_id[1],
                            value: values[1],
                        },
                        TargetUnitOperation::IeeeFloatConstant {
                            psi_operation: operations[2],
                            result: values_id[2],
                            value: values[2],
                        },
                        TargetUnitOperation::NearestIeeeFloatFusedMultiplyAdd {
                            psi_operation: operations[3],
                            result: values_id[3],
                            format,
                            left: operand(0),
                            right: operand(1),
                            addend: operand(2),
                            settlement,
                        },
                        TargetUnitOperation::Return {
                            psi_edge: edge,
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }),
            }],
        };
        let assigned = assign_registers(&plan).unwrap();
        let emitted = super::emit_machine_code(&assigned).unwrap();
        let function = &emitted.functions[0];
        let [fragment] = function.x86_scalar_fma.as_slice() else {
            panic!("one mechanics fragment")
        };
        let [occurrence] = function.x86_scalar_fma_occurrences.as_slice() else {
            panic!("one semantic occurrence")
        };
        let control = function.x86_floating_control.expect("MXCSR custody");
        assert_eq!(control.canonical_mxcsr, 0x1f80);
        assert_eq!(occurrence.fragment_identity, fragment.identity);
        assert_eq!(occurrence.provider_plan_digest, [0xa5; 32]);
        assert_eq!(occurrence.left.value, values[0]);
        assert_eq!(occurrence.right.value, values[1]);
        assert_eq!(occurrence.addend.value, values[2]);
        assert!(control.install_offset < occurrence.left.code_offset);
        assert_eq!(
            fragment.code_offset + fragment.byte_count,
            control.restore_offset
        );

        let mut corrupted = assigned.clone();
        let omega_assigned_target_operations::AssignedOperation::UnitBody(body) =
            &mut corrupted.functions[0].operation
        else {
            unreachable!()
        };
        let omega_assigned_target_operations::AssignedUnitOperation::NearestIeeeFloatFusedMultiplyAdd {
            addend,
            ..
        } = &mut body.operations[3]
        else {
            unreachable!()
        };
        addend.source_value = values_id[0];
        assert!(matches!(
            super::emit_machine_code(&corrupted),
            Err(EmissionError::InvalidIeeeFloatFmaCustody(operation))
                if operation == operations[3]
        ));
    }
}

fn admitted_foreign_stack() -> omega_task_plans::AdmittedSameStackContribution {
    let provider_plan_commitment =
        omega_task_plans::SameStackProviderPlanCommitment::from_digest([0x73; 32]);
    omega_task_plans::admit_same_stack_contribution(
        omega_task_plans::SameStackContributionAdmissionCandidate {
            provider_plan_report_identity: 701,
            provider_plan_commitment,
            requirement_identity: "omega::test::foreign_leaf()".into(),
            receipt:
                omega_task_plans::SameStackContributionAdmissionReceiptId::from_normalized_identity(
                    702,
                )
                .unwrap(),
            bytes: 64,
            alignment: 16,
        },
        701,
        provider_plan_commitment,
        "omega::test::foreign_leaf()",
    )
    .unwrap()
}

#[test]
fn normalized_foreign_unit_leaf_emits_placeholder_and_stack_custody_on_both_linux_targets() {
    let machine = MachineId::new(700).unwrap();
    let boundary = BoundaryMachineId::new(700).unwrap();
    let operation = OperationId::new(700).unwrap();
    let return_edge = EdgeId::new(700).unwrap();
    let provider_execution = ProviderExecutionBinding::from_execution_record(
        ProviderPlanReportIdentity::new(701).unwrap(),
        702,
        703,
        704,
        705,
    )
    .unwrap();
    for profile in [
        omega_target::TargetProfile::LinuxX64,
        omega_target::TargetProfile::LinuxArm64,
    ] {
        let target = profile.native_target();
        let signature = CallSignature {
            parameters: Vec::new(),
            result: None,
        };
        let boundary_entry_plan = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::native_for_target(target),
            &signature,
        )
        .unwrap()
        .plan()
        .clone();
        let locator = omega_target::normalize_foreign_locator(
            omega_target::ForeignLocatorCandidate::ElfVersioned {
                object: b"libomega_foreign_test.so".to_vec(),
                symbol: b"omega_foreign_leaf".to_vec(),
                version: b"OMEGA_TEST_1".to_vec(),
            },
            profile,
        )
        .unwrap();
        let plan = TargetOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([0x70; 32]),
            },
            target,
            entry: machine,
            functions: vec![TargetFunction {
                fixed_integer_scalar_abi: None,
                mixed_structural_scalar_abi: None,
                machine,
                attachment: None,
                provenance: TerminalPsiProvenance {
                    operations: vec![operation],
                    edges: vec![return_edge],
                },
                operation: TargetOperation::UnitBody(TargetUnitBody {
                    structural_types: Vec::new(),
                    call_plan: evaluate_call_plan(
                        CallingPolicy::native_for_target(target),
                        &signature,
                    )
                    .unwrap(),
                    scalar_parameters: Vec::new(),
                    parameters: Vec::new(),
                    operations: vec![
                        TargetUnitOperation::NormalizedForeignCall {
                            psi_operation: operation,
                            boundary,
                            provider_execution,
                            binding: NormalizedForeignCallBinding {
                                locator: locator.clone(),
                                boundary_entry_plan: boundary_entry_plan.clone(),
                                same_stack_contribution: admitted_foreign_stack(),
                            },
                            scalar_arguments: Vec::new(),
                            result_home: None,
                        },
                        TargetUnitOperation::Return {
                            psi_edge: return_edge,
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }),
            }],
        };
        let emitted = emit_machine_code(&plan).expect("emit normalized foreign leaf");
        let function = &emitted.functions[0];
        let [call] = function.foreign_calls.as_slice() else {
            panic!("one foreign relocation")
        };
        assert_eq!(
            call.owner,
            omega_target_operations::CallSiteOwner::Operation(operation)
        );
        assert_eq!(call.locator, locator);
        assert_eq!(call.call_plan, boundary_entry_plan.call);
        assert_eq!(call.same_stack_contribution, admitted_foreign_stack());
        match target.architecture {
            omega_target::Architecture::X86_64 => {
                assert_eq!(call.aarch64_floating_control, None);
                assert_eq!(function.bytes[call.offset - 1], 0xe8);
                assert_eq!(&function.bytes[call.offset..call.offset + 4], &[0; 4]);
                let outbound = call.unit_stack.outbound.unwrap();
                assert_eq!(outbound.byte_size, 8);
                let control = call
                    .x86_floating_control
                    .expect("returning x86 foreign call preserves MXCSR");
                assert_eq!(control.target, target);
                assert_eq!(control.saved_slot_byte_offset, 0);
                assert_eq!(
                    control.save_offset + control.save_byte_count,
                    outbound.allocation_offset
                );
                assert_eq!(
                    outbound.release_offset + outbound.release_byte_count,
                    control.restore_offset
                );
                assert_eq!(
                    &function.bytes
                        [control.save_offset..control.save_offset + control.save_byte_count],
                    omega_isa_x86_64::encode_stmxcsr_rsp_displacement(0)
                        .unwrap()
                        .as_slice()
                );
                assert_eq!(
                    &function.bytes[control.restore_offset
                        ..control.restore_offset + control.restore_byte_count],
                    omega_isa_x86_64::encode_ldmxcsr_rsp_displacement(0)
                        .unwrap()
                        .as_slice()
                );
            }
            omega_target::Architecture::Aarch64 => {
                assert_eq!(
                    &function.bytes[call.offset..call.offset + 4],
                    &0x9400_0000_u32.to_le_bytes()
                );
                assert_eq!(call.unit_stack.outbound, None);
                assert_eq!(call.x86_floating_control, None);
                let control = call
                    .aarch64_floating_control
                    .expect("returning AArch64 foreign call preserves FPCR");
                assert_eq!(control.target, target);
                assert_eq!(control.saved_slot_byte_offset, 0);
                assert_eq!(control.save_offset + control.save_byte_count, call.offset);
                assert_eq!(call.offset + 4, control.restore_offset);
                assert_eq!(
                    &function.bytes
                        [control.save_offset..control.save_offset + control.save_byte_count],
                    &omega_isa_aarch64::encode_save_fpcr_to_sp_displacement(0).unwrap()
                );
                assert_eq!(
                    &function.bytes[control.restore_offset
                        ..control.restore_offset + control.restore_byte_count],
                    &omega_isa_aarch64::encode_restore_fpcr_from_sp_displacement(0).unwrap()
                );
            }
        }

        let mut assigned = assign_registers(&plan).unwrap();
        let omega_assigned_target_operations::AssignedOperation::UnitBody(body) =
            &mut assigned.functions[0].operation
        else {
            unreachable!()
        };
        let omega_assigned_target_operations::AssignedUnitOperation::NormalizedForeignCall {
            binding,
            ..
        } = &mut body.operations[0]
        else {
            unreachable!()
        };
        binding.boundary_entry_plan.call.stack_alignment = 8;
        assert_eq!(
            super::emit_machine_code(&assigned),
            Err(EmissionError::InvalidNormalizedForeignCallCustody)
        );
    }
}

#[test]
fn normalized_foreign_integer_literal_uses_only_the_evaluated_register_before_the_call() {
    let machine = MachineId::new(710).unwrap();
    let boundary = BoundaryMachineId::new(710).unwrap();
    let constant_operation = OperationId::new(710).unwrap();
    let call_operation = OperationId::new(711).unwrap();
    let return_edge = EdgeId::new(710).unwrap();
    let source_value = psi_core::ValueId::new(710).unwrap();
    let scalar_type = psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 64).unwrap();
    let immediate = psi_core::IntegerValue::Signed(-37);
    let provider_execution = ProviderExecutionBinding::from_execution_record(
        ProviderPlanReportIdentity::new(711).unwrap(),
        712,
        713,
        714,
        715,
    )
    .unwrap();

    for profile in [
        omega_target::TargetProfile::LinuxX64,
        omega_target::TargetProfile::LinuxArm64,
    ] {
        let target = profile.native_target();
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: None,
        };
        let boundary_entry_plan = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::native_for_target(target),
            &signature,
        )
        .unwrap()
        .plan()
        .clone();
        let placement = boundary_entry_plan.call.parameters[0].clone();
        let locator = omega_target::normalize_foreign_locator(
            omega_target::ForeignLocatorCandidate::ElfVersioned {
                object: b"libomega_foreign_test.so".to_vec(),
                symbol: b"omega_foreign_i64".to_vec(),
                version: b"OMEGA_TEST_1".to_vec(),
            },
            profile,
        )
        .unwrap();
        let plan = TargetOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([0x71; 32]),
            },
            target,
            entry: machine,
            functions: vec![TargetFunction {
                fixed_integer_scalar_abi: None,
                mixed_structural_scalar_abi: None,
                machine,
                attachment: None,
                provenance: TerminalPsiProvenance {
                    operations: vec![constant_operation, call_operation],
                    edges: vec![return_edge],
                },
                operation: TargetOperation::UnitBody(TargetUnitBody {
                    structural_types: Vec::new(),
                    call_plan: evaluate_call_plan(
                        CallingPolicy::native_for_target(target),
                        &CallSignature {
                            parameters: Vec::new(),
                            result: None,
                        },
                    )
                    .unwrap(),
                    scalar_parameters: Vec::new(),
                    parameters: Vec::new(),
                    operations: vec![
                        TargetUnitOperation::IntegerConstant {
                            psi_operation: constant_operation,
                            result: source_value,
                            scalar_type,
                            value: immediate,
                        },
                        TargetUnitOperation::NormalizedForeignCall {
                            psi_operation: call_operation,
                            boundary,
                            provider_execution,
                            binding: NormalizedForeignCallBinding {
                                locator: locator.clone(),
                                boundary_entry_plan: boundary_entry_plan.clone(),
                                same_stack_contribution: admitted_foreign_stack(),
                            },
                            scalar_arguments: vec![NormalizedForeignScalarArgument {
                                parameter_index: 0,
                                source: omega_target_operations::TargetUnitScalarArgumentSource::IntegerImmediate {
                                    defining_operation: constant_operation,
                                    source_value,
                                    scalar_type,
                                    value: immediate,
                                },
                                placement: placement.clone(),
                            }],
                            result_home: None,
                        },
                        TargetUnitOperation::Return {
                            psi_edge: return_edge,
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }),
            }],
        };

        let mut outer_target_drift = plan.clone();
        outer_target_drift.target = match target.architecture {
            omega_target::Architecture::X86_64 => NativeTarget::linux_arm64(),
            omega_target::Architecture::Aarch64 => NativeTarget::linux_x64(),
        };
        assert!(assign_registers(&outer_target_drift).is_err());

        let emitted = emit_machine_code(&plan).expect("emit evaluated foreign literal call");
        let function = &emitted.functions[0];
        let [call] = function.foreign_calls.as_slice() else {
            panic!("one foreign relocation")
        };
        let [argument] = call.scalar_arguments.as_slice() else {
            panic!("one retained foreign scalar argument")
        };
        assert_eq!(argument.parameter_index, 0);
        assert_eq!(
            argument.source,
            omega_machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
                defining_operation: constant_operation,
                source_value,
                scalar_type,
                value: immediate,
            }
        );
        assert_eq!(argument.placement, placement);
        assert!(argument.byte_count > 0);
        assert!(argument.code_offset + argument.byte_count <= call.offset);
        match target.architecture {
            omega_target::Architecture::X86_64 => {
                let mut expected = vec![0x48, 0xbf];
                expected.extend_from_slice(&(-37_i64 as u64).to_le_bytes());
                assert_eq!(
                    &function.bytes
                        [argument.code_offset..argument.code_offset + argument.byte_count],
                    expected
                );
                assert_eq!(function.bytes[call.offset - 1], 0xe8);
            }
            omega_target::Architecture::Aarch64 => {
                let expected = [0xd29f_fb60_u32, 0xf2bf_ffe0, 0xf2df_ffe0, 0xf2ff_ffe0]
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>();
                assert_eq!(
                    &function.bytes
                        [argument.code_offset..argument.code_offset + argument.byte_count],
                    expected
                );
                assert_eq!(
                    &function.bytes[call.offset..call.offset + 4],
                    &0x9400_0000_u32.to_le_bytes()
                );
            }
        }

        let assigned = assign_registers(&plan).unwrap();
        let mutate_argument =
            |mut assigned: omega_assigned_target_operations::AssignedOperationPlan,
             mutate: &dyn Fn(
                &mut omega_assigned_target_operations::AssignedNormalizedForeignScalarArgument,
            )| {
                let omega_assigned_target_operations::AssignedOperation::UnitBody(body) =
                    &mut assigned.functions[0].operation
                else {
                    unreachable!()
                };
                let omega_assigned_target_operations::AssignedUnitOperation::NormalizedForeignCall {
                scalar_arguments,
                ..
            } = &mut body.operations[1]
            else {
                unreachable!()
            };
                mutate(&mut scalar_arguments[0]);
                assert_eq!(
                    super::emit_machine_code(&assigned),
                    Err(EmissionError::InvalidNormalizedForeignCallCustody)
                );
            };
        mutate_argument(assigned.clone(), &|argument| {
            let omega_assigned_target_operations::AssignedUnitScalarArgumentSource::IntegerImmediate {
                source_value,
                ..
            } = &mut argument.source
            else {
                unreachable!()
            };
            *source_value = psi_core::ValueId::new(711).unwrap();
        });
        mutate_argument(assigned.clone(), &|argument| {
            let omega_assigned_target_operations::AssignedUnitScalarArgumentSource::IntegerImmediate {
                scalar_type,
                ..
            } = &mut argument.source
            else {
                unreachable!()
            };
            *scalar_type = psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 32).unwrap();
        });
        mutate_argument(assigned.clone(), &|argument| {
            let omega_assigned_target_operations::AssignedUnitScalarArgumentSource::IntegerImmediate {
                value,
                ..
            } = &mut argument.source
            else {
                unreachable!()
            };
            *value = psi_core::IntegerValue::Signed(-38);
        });
        mutate_argument(assigned.clone(), &|argument| {
            argument.parameter_index = 1;
        });
        mutate_argument(assigned.clone(), &|argument| {
            argument.placement.locations = vec![omega_calling_conventions::ValueLocation::Stack {
                stack_byte_offset: 0,
                value_byte_offset: 0,
                byte_size: 4,
                alignment: 4,
            }];
        });
        mutate_argument(assigned.clone(), &|argument| {
            argument.placement.locations[0] = omega_calling_conventions::ValueLocation::Register {
                register: match target.architecture {
                    omega_target::Architecture::X86_64 => MachineRegister::X86Rsi,
                    omega_target::Architecture::Aarch64 => MachineRegister::Aarch64X(1),
                },
                value_byte_offset: 0,
                byte_size: 8,
            };
        });

        let mut address_carrier = assigned.clone();
        let omega_assigned_target_operations::AssignedOperation::UnitBody(body) =
            &mut address_carrier.functions[0].operation
        else {
            unreachable!()
        };
        let omega_assigned_target_operations::AssignedUnitOperation::IntegerConstant {
            scalar_type,
            value,
            ..
        } = &mut body.operations[0]
        else {
            unreachable!()
        };
        *scalar_type = psi_core::IntegerType::address(64).unwrap();
        *value = psi_core::IntegerValue::Unsigned((-37_i64) as u64 as u128);
        let omega_assigned_target_operations::AssignedUnitOperation::NormalizedForeignCall {
            scalar_arguments,
            ..
        } = &mut body.operations[1]
        else {
            unreachable!()
        };
        scalar_arguments[0].source =
            omega_assigned_target_operations::AssignedUnitScalarArgumentSource::IntegerImmediate {
                defining_operation: constant_operation,
                source_value,
                scalar_type: psi_core::IntegerType::address(64).unwrap(),
                value: psi_core::IntegerValue::Unsigned((-37_i64) as u64 as u128),
            };
        assert_eq!(
            super::emit_machine_code(&address_carrier),
            Err(EmissionError::InvalidNormalizedForeignCallCustody)
        );

        let mut stripped = assigned.clone();
        let omega_assigned_target_operations::AssignedOperation::UnitBody(body) =
            &mut stripped.functions[0].operation
        else {
            unreachable!()
        };
        let omega_assigned_target_operations::AssignedUnitOperation::NormalizedForeignCall {
            scalar_arguments,
            ..
        } = &mut body.operations[1]
        else {
            unreachable!()
        };
        scalar_arguments.clear();
        assert_eq!(
            super::emit_machine_code(&stripped),
            Err(EmissionError::InvalidNormalizedForeignCallCustody)
        );

        let mut triplicated = assigned;
        let omega_assigned_target_operations::AssignedOperation::UnitBody(body) =
            &mut triplicated.functions[0].operation
        else {
            unreachable!()
        };
        let omega_assigned_target_operations::AssignedUnitOperation::NormalizedForeignCall {
            scalar_arguments,
            ..
        } = &mut body.operations[1]
        else {
            unreachable!()
        };
        scalar_arguments.push(scalar_arguments[0].clone());
        scalar_arguments.push(scalar_arguments[0].clone());
        assert_eq!(
            super::emit_machine_code(&triplicated),
            Err(EmissionError::InvalidNormalizedForeignCallCustody)
        );

        let mut target_drift = assign_registers(&plan).unwrap();
        target_drift.target = match target.architecture {
            omega_target::Architecture::X86_64 => NativeTarget::linux_arm64(),
            omega_target::Architecture::Aarch64 => NativeTarget::linux_x64(),
        };
        assert_eq!(
            super::emit_machine_code(&target_drift),
            Err(EmissionError::InvalidNormalizedForeignCallCustody)
        );
    }
}

#[test]
fn linux_exit_group_consumes_i32_and_traps_on_both_linux_architectures() {
    let machine = MachineId::new(990).unwrap();
    let boundary = BoundaryMachineId::new(990).unwrap();
    let constant_operation = OperationId::new(990).unwrap();
    let settlement_operation = OperationId::new(991).unwrap();
    let nominal_return_edge = EdgeId::new(990).unwrap();
    let source_value = psi_core::ValueId::new(990).unwrap();
    let i32_type = psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 32).unwrap();
    let plan_for = |target: NativeTarget, destination| TargetOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x99; 32]),
        },
        target,
        entry: machine,
        functions: vec![TargetFunction {
            fixed_integer_scalar_abi: None,
            mixed_structural_scalar_abi: None,
            machine,
            attachment: None,
            provenance: TerminalPsiProvenance {
                operations: vec![constant_operation, settlement_operation],
                edges: vec![nominal_return_edge],
            },
            operation: TargetOperation::ExitProcessI32 {
                constant_operation,
                psi_operation: settlement_operation,
                nominal_return_edge,
                boundary,
                execution: omega_target_operations::BoundaryExecutionBinding::CompilerBuiltin(
                    omega_target_operations::CompilerBuiltinExecution::LinuxExitGroupI32,
                ),
                realization: LinuxExitGroupI32Realization,
                argument: BoundaryScalarArgument {
                    source_value,
                    scalar_type: psi_core::ScalarType::Integer(i32_type),
                    immediate: psi_core::IntegerValue::Signed(37),
                    destination,
                },
                completion_claim_sources: Vec::new(),
                completion_receipts: Vec::new(),
            },
        }],
    };

    let x86_plan = plan_for(NativeTarget::linux_x64(), MachineRegister::X86Rdi);
    let x86 = emit_machine_code(&x86_plan).expect("x86 exit_group emission");
    assert_eq!(
        x86,
        emit_machine_code(&x86_plan).expect("deterministic x86 emission")
    );
    assert_eq!(
        x86.functions[0].bytes,
        omega_isa_x86_64::encode_linux_exit_group_i32(37)
    );
    let settlement = &x86.functions[0].boundary_settlements[0];
    assert_eq!(settlement.code_offset, 0);
    assert_eq!(settlement.byte_count, x86.functions[0].bytes.len());
    assert_eq!(settlement.scalar_arguments.len(), 1);
    assert!(settlement.arguments.is_empty());
    assert_eq!(
        &x86.functions[0].bytes[x86.functions[0].bytes.len() - 2..],
        &[0x0f, 0x0b]
    );
    assert!(matches!(
        x86.functions[0].semantic_code_attribution[2].site,
        SemanticCodeSite::Edge(edge) if edge == nominal_return_edge
    ));
    assert_eq!(x86.functions[0].semantic_code_attribution[2].byte_count, 0);

    let arm_plan = plan_for(NativeTarget::linux_arm64(), MachineRegister::Aarch64X(0));
    let arm = emit_machine_code(&arm_plan).expect("AArch64 exit_group emission");
    assert_eq!(
        arm.functions[0].bytes,
        omega_isa_aarch64::encode_linux_exit_group_i32(37).unwrap()
    );
    assert_eq!(
        &arm.functions[0].bytes[arm.functions[0].bytes.len() - 4..],
        &0xd420_0000_u32.to_le_bytes()
    );

    let windows = plan_for(NativeTarget::windows_x64(), MachineRegister::X86Rdi);
    assert!(assign_registers(&windows).is_err());
    let darwin = plan_for(NativeTarget::macos_arm64(), MachineRegister::Aarch64X(0));
    assert!(assign_registers(&darwin).is_err());
}

#[test]
fn linux_write_line_then_exit_owns_exact_code_data_and_argument_custody() {
    let machine = MachineId::new(980).unwrap();
    let literal_place = PlaceId::new(980).unwrap();
    let structural_type_id = StructuralTypeId::new(980).unwrap();
    let literal_operation = OperationId::new(980).unwrap();
    let write_operation = OperationId::new(981).unwrap();
    let constant_operation = OperationId::new(982).unwrap();
    let exit_operation = OperationId::new(983).unwrap();
    let return_edge = EdgeId::new(980).unwrap();
    let write_boundary = BoundaryMachineId::new(980).unwrap();
    let exit_boundary = BoundaryMachineId::new(981).unwrap();
    let exit_value = psi_core::ValueId::new(980).unwrap();
    let literal_bytes = vec![0, 0x80, 0xff];
    let structural_type = StructuralTypeDeclaration {
        id: structural_type_id,
        identity: "test::BorrowedBytes".into(),
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
    };
    let place = StructuralPlaceDeclaration {
        id: literal_place,
        kind: psi_core::StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal: 0,
            structural_type: structural_type_id,
        },
    };
    let argument = StructuralArgument {
        access: StructuralAccess::SharedBorrow,
        place: literal_place,
        path: Vec::new(),
    };
    let i32_type = psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 32).unwrap();
    let provider = |seed| {
        ProviderExecutionBinding::from_execution_record(
            ProviderPlanReportIdentity::new(seed).unwrap(),
            seed + 1,
            seed + 2,
            seed + 3,
            seed + 4,
        )
        .unwrap()
    };
    let plan_for = |target: NativeTarget, destination| TargetOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x98; 32]),
        },
        target,
        entry: machine,
        functions: vec![TargetFunction {
            fixed_integer_scalar_abi: None,
            mixed_structural_scalar_abi: None,
            machine,
            attachment: None,
            provenance: TerminalPsiProvenance {
                operations: vec![
                    literal_operation,
                    write_operation,
                    constant_operation,
                    exit_operation,
                ],
                edges: vec![return_edge],
            },
            operation: TargetOperation::UnitBody(TargetUnitBody {
                structural_types: vec![structural_type.clone()],
                call_plan: evaluate_call_plan(
                    CallingPolicy::native_for_target(target),
                    &CallSignature {
                        parameters: Vec::new(),
                        result: None,
                    },
                )
                .unwrap(),
                scalar_parameters: Vec::new(),
                parameters: Vec::new(),
                operations: vec![
                    TargetUnitOperation::EstablishByteSequenceLiteral {
                        psi_operation: literal_operation,
                        place: place.clone(),
                        structural_type: structural_type.clone(),
                        bytes: literal_bytes.clone(),
                    },
                    TargetUnitOperation::BoundarySettlement {
                        psi_operation: write_operation,
                        boundary: write_boundary,
                        execution: provider(980).into(),
                        realization: LinuxWriteLineRealization.into(),
                        scalar_arguments: Vec::new(),
                        arguments: vec![argument.clone()],
                        byte_sequence_arguments: vec![BoundaryByteSequenceArgument {
                            argument: argument.clone(),
                            literal_operation,
                            structural_type: structural_type.clone(),
                            bytes: literal_bytes.clone(),
                        }],
                        completion_claim_sources: Vec::new(),
                        completion_receipts: Vec::new(),
                    },
                    TargetUnitOperation::IntegerConstant {
                        psi_operation: constant_operation,
                        result: exit_value,
                        scalar_type: i32_type,
                        value: psi_core::IntegerValue::Signed(37),
                    },
                    TargetUnitOperation::BoundarySettlement {
                        psi_operation: exit_operation,
                        boundary: exit_boundary,
                        execution: provider(990).into(),
                        realization: LinuxExitGroupI32Realization.into(),
                        scalar_arguments: vec![BoundaryScalarArgument {
                            source_value: exit_value,
                            scalar_type: psi_core::ScalarType::Integer(i32_type),
                            immediate: psi_core::IntegerValue::Signed(37),
                            destination,
                        }],
                        arguments: Vec::new(),
                        byte_sequence_arguments: Vec::new(),
                        completion_claim_sources: Vec::new(),
                        completion_receipts: Vec::new(),
                    },
                    TargetUnitOperation::Return {
                        psi_edge: return_edge,
                        cleanup_actions: Vec::new(),
                    },
                ],
            }),
        }],
    };

    for (target, destination) in [
        (NativeTarget::linux_x64(), MachineRegister::X86Rdi),
        (NativeTarget::linux_arm64(), MachineRegister::Aarch64X(0)),
    ] {
        let plan = plan_for(target, destination);
        let emitted = emit_machine_code(&plan).expect("composed Linux emission");
        assert_eq!(
            emitted,
            emit_machine_code(&plan).expect("deterministic composed emission")
        );
        let function = &emitted.functions[0];
        assert_eq!(function.boundary_settlements.len(), 2);
        let write = &function.boundary_settlements[0];
        let [custody] = write.byte_sequence_arguments.as_slice() else {
            panic!("write_line has one exact structural argument custody row")
        };
        assert_eq!(custody.literal_operation, literal_operation);
        assert_eq!(custody.bytes, literal_bytes);
        assert!(custody.code_byte_count > 0);
        assert_eq!(custody.data_byte_count, literal_bytes.len() + 1);
        assert_eq!(
            &function.bytes[custody.data_offset..custody.data_offset + custody.data_byte_count],
            &[0, 0x80, 0xff, b'\n']
        );
        assert!(write.byte_count > custody.data_byte_count);
        let exit = &function.boundary_settlements[1];
        assert_eq!(exit.code_offset, write.code_offset + write.byte_count);
        assert!(exit.byte_count > 0);
        assert_eq!(
            exit.scalar_arguments[0].immediate,
            psi_core::IntegerValue::Signed(37)
        );
        assert!(exit.byte_sequence_arguments.is_empty());
        assert_eq!(function.semantic_code_attribution.len(), 5);
    }

    for (target, destination) in [
        (NativeTarget::windows_x64(), MachineRegister::X86Rdi),
        (NativeTarget::macos_arm64(), MachineRegister::Aarch64X(0)),
    ] {
        let plan = plan_for(target, destination);
        assert!(emit_machine_code(&plan).is_err());
    }
}

#[test]
fn partial_cleanup_partition_rejects_noncanonical_type_closures() {
    let root_type = StructuralTypeId::new(1).expect("root type");
    let moved_type = StructuralTypeId::new(2).expect("moved type");
    let residual_type = StructuralTypeId::new(3).expect("residual type");
    let mut declarations = vec![
        psi_terminal::StructuralTypeDeclaration {
            id: root_type,
            identity: "Root".into(),
            shape: psi_terminal::StructuralTypeShape::Record {
                fields: vec![
                    psi_terminal::StructuralFieldDeclaration {
                        id: psi_core::StructuralFieldId::new(1).expect("moved field"),
                        identity: "moved".into(),
                        relevance: psi_terminal::BindingRelevance::Relevant,
                        field_type: psi_terminal::StructuralFieldType::Structural(moved_type),
                    },
                    psi_terminal::StructuralFieldDeclaration {
                        id: psi_core::StructuralFieldId::new(2).expect("scalar field"),
                        identity: "count".into(),
                        relevance: psi_terminal::BindingRelevance::Relevant,
                        field_type: psi_terminal::StructuralFieldType::Scalar(
                            psi_core::ScalarType::Integer(
                                psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 64)
                                    .expect("u64"),
                            ),
                        ),
                    },
                    psi_terminal::StructuralFieldDeclaration {
                        id: psi_core::StructuralFieldId::new(4).expect("float field"),
                        identity: "ratio".into(),
                        relevance: psi_terminal::BindingRelevance::Relevant,
                        field_type: psi_terminal::StructuralFieldType::IeeeFloat(
                            psi_core::IeeeFloatFormat::Binary32,
                        ),
                    },
                    psi_terminal::StructuralFieldDeclaration {
                        id: psi_core::StructuralFieldId::new(5).expect("byte field"),
                        identity: "bytes".into(),
                        relevance: psi_terminal::BindingRelevance::Relevant,
                        field_type: psi_terminal::StructuralFieldType::ByteSequence(
                            psi_terminal::ByteSequenceCarrier::BoundedOwned { capacity: 8 },
                        ),
                    },
                    psi_terminal::StructuralFieldDeclaration {
                        id: psi_core::StructuralFieldId::new(3).expect("residual field"),
                        identity: "residual".into(),
                        relevance: psi_terminal::BindingRelevance::Relevant,
                        field_type: psi_terminal::StructuralFieldType::Structural(residual_type),
                    },
                ],
            },
        },
        psi_terminal::StructuralTypeDeclaration {
            id: moved_type,
            identity: "Moved".into(),
            shape: psi_terminal::StructuralTypeShape::Record { fields: Vec::new() },
        },
        psi_terminal::StructuralTypeDeclaration {
            id: residual_type,
            identity: "Residual".into(),
            shape: psi_terminal::StructuralTypeShape::Record { fields: Vec::new() },
        },
    ];
    let moved_path = vec![StructuralPathSegment::Field("moved".into())];
    let moved = vec![(moved_path.as_slice(), moved_type)];
    let residual = psi_terminal::StructuralAffineDiscard {
        place: PlaceId::new(1).expect("place"),
        path: vec![StructuralPathSegment::Field("residual".into())],
        structural_type: residual_type,
    };
    let residuals = vec![&residual];

    assert!(exact_partial_cleanup_partition(
        &declarations,
        root_type,
        &moved,
        &residuals,
    ));

    let scalar_path = vec![StructuralPathSegment::Field("count".into())];
    assert!(
        !exact_partial_cleanup_partition(
            &declarations,
            root_type,
            &[(scalar_path.as_slice(), residual_type)],
            &residuals,
        ),
        "a scalar field cannot become a moved affine structural path"
    );
    let scalar_residual = psi_terminal::StructuralAffineDiscard {
        place: PlaceId::new(1).expect("place"),
        path: scalar_path,
        structural_type: residual_type,
    };
    assert!(
        !exact_partial_cleanup_partition(
            &declarations,
            root_type,
            &moved,
            &[&scalar_residual, &residual],
        ),
        "a scalar field cannot acquire an affine residual cleanup"
    );

    let float_path = vec![StructuralPathSegment::Field("ratio".into())];
    assert!(
        !exact_partial_cleanup_partition(
            &declarations,
            root_type,
            &[(float_path.as_slice(), residual_type)],
            &residuals,
        ),
        "a cleanup-free float field cannot become a moved affine structural path"
    );
    let float_residual = psi_terminal::StructuralAffineDiscard {
        place: PlaceId::new(1).expect("place"),
        path: float_path,
        structural_type: residual_type,
    };
    assert!(
        !exact_partial_cleanup_partition(
            &declarations,
            root_type,
            &moved,
            &[&float_residual, &residual],
        ),
        "a cleanup-free float field cannot acquire an affine residual cleanup"
    );

    let bytes_path = vec![StructuralPathSegment::Field("bytes".into())];
    assert!(
        !exact_partial_cleanup_partition(
            &declarations,
            root_type,
            &[(bytes_path.as_slice(), residual_type)],
            &residuals,
        ),
        "bounded byte storage cannot become a moved affine structural path"
    );
    let bytes_residual = psi_terminal::StructuralAffineDiscard {
        place: PlaceId::new(1).expect("place"),
        path: bytes_path,
        structural_type: residual_type,
    };
    assert!(
        !exact_partial_cleanup_partition(
            &declarations,
            root_type,
            &moved,
            &[&bytes_residual, &residual],
        ),
        "bounded byte storage cannot acquire an affine residual cleanup"
    );

    let mut borrowed_view = declarations.clone();
    let psi_terminal::StructuralTypeShape::Record { fields } = &mut borrowed_view[0].shape else {
        unreachable!()
    };
    fields
        .iter_mut()
        .find(|field| field.identity == "bytes")
        .expect("bounded byte field")
        .field_type = psi_terminal::StructuralFieldType::ByteSequence(
        psi_terminal::ByteSequenceCarrier::BorrowedView,
    );
    assert!(
        !exact_partial_cleanup_partition(&borrowed_view, root_type, &moved, &residuals),
        "borrowed views require a loan-retirement event"
    );

    let mut scalar_as_structural = declarations.clone();
    let psi_terminal::StructuralTypeShape::Record { fields } = &mut scalar_as_structural[0].shape
    else {
        unreachable!()
    };
    fields[1].field_type = psi_terminal::StructuralFieldType::Structural(residual_type);
    assert!(
        !exact_partial_cleanup_partition(&scalar_as_structural, root_type, &moved, &residuals,),
        "changing scalar classification changes the exact partition"
    );

    let mut moved_as_scalar = declarations.clone();
    let psi_terminal::StructuralTypeShape::Record { fields } = &mut moved_as_scalar[0].shape else {
        unreachable!()
    };
    fields[0].field_type = fields[1].field_type.clone();
    assert!(!exact_partial_cleanup_partition(
        &moved_as_scalar,
        root_type,
        &moved,
        &residuals,
    ));

    declarations[2].identity = "Moved".into();
    assert!(!exact_partial_cleanup_partition(
        &declarations,
        root_type,
        &moved,
        &residuals,
    ));

    declarations[2].identity = "Residual".into();
    declarations.swap(1, 2);
    assert!(!exact_partial_cleanup_partition(
        &declarations,
        root_type,
        &moved,
        &residuals,
    ));
}

#[test]
fn partial_cleanup_partition_accepts_only_opposite_element_of_affine_pair() {
    let root_type = StructuralTypeId::new(1).expect("root type");
    let element_type = StructuralTypeId::new(2).expect("element type");
    let declarations = vec![
        psi_terminal::StructuralTypeDeclaration {
            id: root_type,
            identity: "Pair".into(),
            shape: psi_terminal::StructuralTypeShape::FixedArray {
                element: element_type,
                length: 2,
            },
        },
        psi_terminal::StructuralTypeDeclaration {
            id: element_type,
            identity: "Token".into(),
            shape: psi_terminal::StructuralTypeShape::Record { fields: Vec::new() },
        },
    ];
    let moved_path = vec![StructuralPathSegment::FixedIndex(1)];
    let moved = vec![(moved_path.as_slice(), element_type)];
    let residual = psi_terminal::StructuralAffineDiscard {
        place: PlaceId::new(1).expect("place"),
        path: vec![StructuralPathSegment::FixedIndex(0)],
        structural_type: element_type,
    };
    assert!(exact_partial_cleanup_partition(
        &declarations,
        root_type,
        &moved,
        &[&residual],
    ));

    let same_as_moved = psi_terminal::StructuralAffineDiscard {
        path: vec![StructuralPathSegment::FixedIndex(1)],
        ..residual.clone()
    };
    assert!(!exact_partial_cleanup_partition(
        &declarations,
        root_type,
        &moved,
        &[&same_as_moved],
    ));
    let wrong_type = psi_terminal::StructuralAffineDiscard {
        structural_type: root_type,
        ..residual.clone()
    };
    assert!(!exact_partial_cleanup_partition(
        &declarations,
        root_type,
        &moved,
        &[&wrong_type],
    ));
    let mut wrong_length = declarations.clone();
    let psi_terminal::StructuralTypeShape::FixedArray { length, .. } = &mut wrong_length[0].shape
    else {
        unreachable!()
    };
    *length = 3;
    assert!(!exact_partial_cleanup_partition(
        &wrong_length,
        root_type,
        &moved,
        &[&residual],
    ));
}

fn executable_nominal_cleanup_plan(
    target: NativeTarget,
) -> (
    TargetOperationPlan,
    EdgeId,
    OperationId,
    MachineId,
    MachineId,
) {
    let receiver_shape = ValueShape::integer(8, 8);
    let root_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![receiver_shape],
            result: None,
        },
    )
    .expect("one-field receiver ABI");
    let empty_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature::default(),
    )
    .expect("empty Unit ABI");
    let root = MachineId::new(1).expect("root");
    let cleanup_machine = MachineId::new(2).expect("cleanup");
    let helper = MachineId::new(3).expect("helper");
    let receiver_place = PlaceId::new(1).expect("receiver place");
    let receiver_type = StructuralTypeId::new(1).expect("receiver type");
    let helper_type = StructuralTypeId::new(2).expect("helper type");
    let root_return = EdgeId::new(1).expect("root return");
    let cleanup_call = OperationId::new(1).expect("cleanup helper call");
    let cleanup_return = EdgeId::new(2).expect("cleanup return");
    let helper_return = EdgeId::new(3).expect("helper return");
    let cleanup = NominalAffineCleanup {
        place: receiver_place,
        structural_type: receiver_type,
        cleanup_machine,
        cleanup_receiver: None,
        requirement_obligations: Vec::new(),
    };
    let root_parameter = TargetStructuralParameter {
        access: StructuralAccess::Owned,
        place: receiver_place,
        structural_type: receiver_type,
        multiplicity: StructuralMultiplicity::Affine,
        projected_qualifications: Vec::new(),
        shape: receiver_shape,
        placement: root_call_plan.parameters[0].clone(),
    };
    (
        TargetOperationPlan {
            psi: identity(),
            target,
            entry: root,
            functions: vec![
                TargetFunction {
                    fixed_integer_scalar_abi: None,
                    mixed_structural_scalar_abi: None,
                    machine: root,
                    attachment: None,
                    provenance: TerminalPsiProvenance {
                        operations: Vec::new(),
                        edges: vec![root_return],
                    },
                    operation: TargetOperation::UnitBody(TargetUnitBody {
                        structural_types: Vec::new(),
                        call_plan: root_call_plan,
                        scalar_parameters: Vec::new(),
                        parameters: vec![root_parameter],
                        operations: vec![TargetUnitOperation::Return {
                            psi_edge: root_return,
                            cleanup_actions: vec![
                                psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup),
                            ],
                        }],
                    }),
                },
                TargetFunction {
                    fixed_integer_scalar_abi: None,
                    mixed_structural_scalar_abi: None,
                    machine: cleanup_machine,
                    attachment: Some(receiver_type),
                    provenance: TerminalPsiProvenance {
                        operations: vec![cleanup_call],
                        edges: vec![cleanup_return],
                    },
                    operation: TargetOperation::UnitBody(TargetUnitBody {
                        structural_types: Vec::new(),
                        call_plan: empty_call_plan.clone(),
                        scalar_parameters: Vec::new(),
                        parameters: Vec::new(),
                        operations: vec![
                            TargetUnitOperation::Call {
                                psi_operation: cleanup_call,
                                callee: helper,
                                arguments: Vec::new(),
                                claim_transfers: Vec::new(),
                                requirement_obligations: Vec::new(),
                                crash_continuations: Vec::new(),
                            },
                            TargetUnitOperation::Return {
                                psi_edge: cleanup_return,
                                cleanup_actions: Vec::new(),
                            },
                        ],
                    }),
                },
                TargetFunction {
                    fixed_integer_scalar_abi: None,
                    mixed_structural_scalar_abi: None,
                    machine: helper,
                    attachment: Some(helper_type),
                    provenance: TerminalPsiProvenance {
                        operations: Vec::new(),
                        edges: vec![helper_return],
                    },
                    operation: TargetOperation::UnitBody(TargetUnitBody {
                        structural_types: Vec::new(),
                        call_plan: empty_call_plan,
                        scalar_parameters: Vec::new(),
                        parameters: Vec::new(),
                        operations: vec![TargetUnitOperation::Return {
                            psi_edge: helper_return,
                            cleanup_actions: Vec::new(),
                        }],
                    }),
                },
            ],
        },
        root_return,
        cleanup_call,
        cleanup_machine,
        helper,
    )
}

fn boolean_control_nominal_cleanup_plan(target: NativeTarget) -> TargetOperationPlan {
    let (mut plan, _, _, _, _) = executable_nominal_cleanup_plan(target);
    let TargetOperation::UnitBody(root_body) = &plan.functions[0].operation else {
        unreachable!("fixture root starts as Unit")
    };
    let structural_types = root_body.structural_types.clone();
    let mut structural_parameters = root_body.parameters.clone();
    let [
        TargetUnitOperation::Return {
            cleanup_actions, ..
        },
    ] = root_body.operations.as_slice()
    else {
        unreachable!("fixture root has one return")
    };
    let cleanup_actions = cleanup_actions.clone();
    let scalar_shape = ValueShape::integer(1, 1);
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![scalar_shape, structural_parameters[0].shape],
            result: Some(scalar_shape),
        },
    )
    .expect("Boolean control plus nominal receiver ABI");
    structural_parameters[0].placement = call_plan.parameters[1].clone();
    let [ValueLocation::Register { register, .. }] = call_plan.parameters[0].locations.as_slice()
    else {
        panic!("Boolean condition has one direct register home")
    };
    let condition_location = ScalarParameterLocation::Register(*register);
    let leaf = |edge: u64, value: bool| TargetBooleanControl::ReturnImmediate {
        psi_return_edge: EdgeId::new(edge).unwrap(),
        source_value: ValueId::new(edge).unwrap(),
        value,
    };
    let arm = |edge: u64, control| TargetConditionalBooleanArm {
        psi_edge: EdgeId::new(edge).unwrap(),
        control: Box::new(control),
    };
    let nested = TargetBooleanControl::Conditional {
        condition_source: ValueId::new(1).unwrap(),
        condition_parameter_index: 0,
        condition_location,
        when_true: arm(4, leaf(10, true)),
        when_false: arm(5, leaf(11, false)),
    };
    plan.functions[0].provenance.edges = (1..=5)
        .chain(10..=12)
        .map(|edge| EdgeId::new(edge).unwrap())
        .collect();
    let return_edges = [10, 11, 12].map(|edge| EdgeId::new(edge).unwrap()).to_vec();
    plan.functions[0].operation = TargetOperation::ScalarReturnWithCleanup {
        scalar: Box::new(TargetOperation::ReturnBooleanSharedConvergence {
            psi_edge: return_edges[0],
            return_edges,
            control: TargetBooleanControl::Conditional {
                condition_source: ValueId::new(1).unwrap(),
                condition_parameter_index: 0,
                condition_location,
                when_true: arm(2, nested),
                when_false: arm(3, leaf(12, true)),
            },
        }),
        structural_types,
        call_plan,
        structural_parameters,
        cleanup_actions,
        psi_edge: EdgeId::new(10).unwrap(),
    };
    plan
}

#[test]
fn bounded_boolean_control_emits_one_shared_edge_rostered_cleanup_tail() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let emitted = emit_machine_code(&boolean_control_nominal_cleanup_plan(target))
            .expect("bounded Boolean control cleanup emits");
        let root = &emitted.functions[0];
        let cleanup = root
            .scalar_affine_cleanup
            .as_ref()
            .expect("one shared affine cleanup");
        assert!(root.scalar_control_affine_cleanups.is_empty());
        assert_eq!(root.internal_calls.len(), 1);
        assert_eq!(root.internal_unit_calls.len(), 1);
        assert_eq!(root.semantic_code_attribution.len(), 3);
        assert_eq!(cleanup.psi_edge, EdgeId::new(10).unwrap());
        assert_eq!(cleanup.code_offset + cleanup.byte_count, root.bytes.len());
        let preservation = root
            .scalar_stack
            .as_ref()
            .and_then(|stack| stack.cleanup_preservation)
            .expect("shared cleanup preserves the scalar result");
        assert!(preservation.frame.allocation_offset >= cleanup.code_offset);
        assert!(preservation.result_store_offset >= cleanup.code_offset);
        assert!(preservation.result_load_offset >= preservation.result_store_offset);
        assert!(preservation.frame.release_offset >= preservation.result_load_offset);
        assert!(matches!(
            root.internal_unit_calls[0].owner,
            CallSiteOwner::CleanupAction {
                edge,
                action_ordinal: 0,
            } if edge == cleanup.psi_edge
        ));
        let ScalarControlFlowEvidence::BooleanSharedConvergence { return_edges, .. } = &root
            .scalar_stack
            .as_ref()
            .expect("scalar stack")
            .control_flow
        else {
            panic!("shared cleanup retains native convergence evidence")
        };
        assert_eq!(
            return_edges,
            &[10, 11, 12]
                .map(|edge| EdgeId::new(edge).unwrap())
                .as_slice()
        );
        for (ordinal, edge) in return_edges.iter().enumerate() {
            assert_eq!(
                root.semantic_code_attribution[ordinal].site,
                SemanticCodeSite::Edge(*edge)
            );
            assert_eq!(
                root.semantic_code_attribution[ordinal].operation_ordinal,
                ordinal
            );
            assert_eq!(
                root.semantic_code_attribution[ordinal].code_offset,
                cleanup.code_offset
            );
            assert_eq!(
                root.semantic_code_attribution[ordinal].byte_count,
                cleanup.byte_count
            );
        }
        match target.architecture {
            Architecture::X86_64 => assert!(preservation.aarch64_return_link.is_none()),
            Architecture::Aarch64 => assert!(preservation.aarch64_return_link.is_some()),
        }
        let stack = root.scalar_stack.as_ref().expect("scalar stack evidence");
        assert!(stack.cleanup_preservation.is_some());
        let ScalarControlFlowEvidence::BooleanSharedConvergence {
            decisions, joins, ..
        } = &stack.control_flow
        else {
            panic!("exact two-decision/three-return convergence is retained")
        };
        assert_eq!(joins.len(), 2);
        let [root_branch, nested] = decisions.as_slice() else {
            panic!("two decisions are retained")
        };
        assert!(root_branch.branch_offset < nested.branch_offset);
        assert!(nested.false_arm_offset < root_branch.false_arm_offset);
        assert_eq!(root_branch.condition, ScalarConditionalCondition::Parameter);
        assert_eq!(nested.condition, ScalarConditionalCondition::Parameter);
        assert_eq!(
            stack
                .mutations
                .iter()
                .filter(|mutation| matches!(
                    mutation.kind,
                    ScalarStackMutationKind::Allocate { byte_size: 16 }
                ))
                .count(),
            1
        );
    }
}

#[test]
fn bounded_boolean_cleanup_emission_rechecks_distinct_leaf_edges() {
    let target = NativeTarget::linux_x64();
    let mut assigned = assign_registers(&boolean_control_nominal_cleanup_plan(target))
        .expect("valid fixture assigns");
    let AssignedOperation::ScalarReturnWithCleanup { scalar, .. } =
        &mut assigned.functions[0].operation
    else {
        unreachable!("fixture retains Boolean cleanup control")
    };
    let AssignedOperation::ReturnBooleanSharedConvergence { control, .. } = scalar.as_mut() else {
        unreachable!("fixture retains shared convergence")
    };
    let AssignedBooleanControl::Conditional { when_true, .. } = control else {
        unreachable!("fixture root remains conditional")
    };
    let AssignedBooleanControl::Conditional { when_false, .. } = when_true.control.as_mut() else {
        unreachable!("fixture true arm remains nested conditional")
    };
    let AssignedBooleanControl::ReturnImmediate {
        psi_return_edge, ..
    } = when_false.control.as_mut()
    else {
        unreachable!("fixture nested false arm remains a return")
    };
    *psi_return_edge = EdgeId::new(10).unwrap();
    assert_eq!(
        super::emit_machine_code(&assigned),
        Err(EmissionError::UnsupportedScalarCleanup)
    );
}

#[test]
fn scalar_result_materializes_before_ordered_nominal_cleanup() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (mut plan, edge, _, cleanup_machine, _) = executable_nominal_cleanup_plan(target);
        let TargetOperation::UnitBody(root_body) = &mut plan.functions[0].operation else {
            unreachable!("fixture root starts as Unit")
        };
        let parameters = root_body.parameters.clone();
        let structural_types = root_body.structural_types.clone();
        let [
            TargetUnitOperation::Return {
                cleanup_actions, ..
            },
        ] = root_body.operations.as_slice()
        else {
            unreachable!("fixture root has one return")
        };
        let cleanup_actions = cleanup_actions.clone();
        let call_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: parameters.iter().map(|parameter| parameter.shape).collect(),
                result: Some(ValueShape::integer(1, 1)),
            },
        )
        .expect("scalar cleanup ABI");
        let mut structural_parameters = parameters;
        for (parameter, placement) in structural_parameters.iter_mut().zip(&call_plan.parameters) {
            parameter.placement = placement.clone();
        }
        plan.functions[0].operation = TargetOperation::ScalarReturnWithCleanup {
            scalar: Box::new(TargetOperation::ReturnBooleanImmediate {
                psi_edge: edge,
                source_value: ValueId::new(1).expect("result value"),
                value: true,
            }),
            structural_types,
            call_plan,
            structural_parameters,
            cleanup_actions,
            psi_edge: edge,
        };

        let emitted = emit_machine_code(&plan).expect("scalar cleanup emits");
        let root = &emitted.functions[0];
        let cleanup = root
            .scalar_affine_cleanup
            .as_ref()
            .expect("scalar cleanup custody");
        assert!(root.unit_affine_cleanup.is_none());
        assert_eq!(root.scalar_structural_parameters.len(), 1);
        assert_eq!(root.scalar_structural_parameter_homes.len(), 1);
        assert_eq!(root.internal_unit_calls.len(), 1);
        assert_eq!(root.internal_unit_calls[0].target, cleanup_machine);
        assert!(root.internal_unit_calls[0].code_offset >= cleanup.code_offset);
        assert_eq!(cleanup.code_offset + cleanup.byte_count, root.bytes.len());
        match target.architecture {
            Architecture::X86_64 => {
                assert!(cleanup.code_offset >= 5, "EAX is materialized first");
                assert_eq!(root.bytes.last(), Some(&0xc3));
            }
            Architecture::Aarch64 => {
                assert!(cleanup.code_offset >= 4, "W0 is materialized first");
                assert_eq!(
                    root.bytes.get(root.bytes.len() - 4..),
                    Some(0xd65f_03c0_u32.to_le_bytes().as_slice())
                );
            }
        }
    }
}

fn runtime_expression_cleanup_plan(target: NativeTarget) -> TargetOperationPlan {
    let (mut plan, edge, _, _, _) = executable_nominal_cleanup_plan(target);
    let TargetOperation::UnitBody(root_body) = &mut plan.functions[0].operation else {
        unreachable!("fixture root starts as Unit")
    };
    let structural_types = root_body.structural_types.clone();
    let mut structural_parameters = root_body.parameters.clone();
    let [
        TargetUnitOperation::Return {
            cleanup_actions, ..
        },
    ] = root_body.operations.as_slice()
    else {
        unreachable!("fixture root has one return")
    };
    let cleanup_actions = cleanup_actions.clone();
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![
                ValueShape::integer(8, 8),
                ValueShape::integer(8, 8),
                structural_parameters[0].shape,
            ],
            result: Some(ValueShape::integer(8, 8)),
        },
    )
    .expect("two scalar inputs and one structural root have an ABI");
    structural_parameters[0].placement = call_plan.parameters[2].clone();
    let scalar_register = |index: usize| {
        let [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size: 8,
            },
        ] = call_plan.parameters[index].locations.as_slice()
        else {
            unreachable!("first two scalar inputs are direct native registers")
        };
        ScalarParameterLocation::Register(*register)
    };
    plan.functions[0].operation = TargetOperation::ScalarReturnWithCleanup {
        scalar: Box::new(TargetOperation::ReturnIntegerExpression {
            psi_edge: edge,
            source_value: ValueId::new(3).expect("result value"),
            scalar_type,
            expression: wrapping_expression(scalar_register(0), scalar_register(1)),
        }),
        structural_types,
        call_plan,
        structural_parameters,
        cleanup_actions,
        psi_edge: edge,
    };
    plan
}

#[test]
fn runtime_scalar_result_is_spilled_across_executable_cleanup_calls() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let emitted = emit_machine_code(&runtime_expression_cleanup_plan(target))
            .expect("runtime scalar cleanup emits");
        let root = &emitted.functions[0];
        let cleanup = root
            .scalar_affine_cleanup
            .as_ref()
            .expect("runtime cleanup custody");
        let stack = root
            .scalar_stack
            .as_ref()
            .expect("composed scalar stack evidence");
        assert_eq!(root.unit_stack, None);
        assert_eq!(root.internal_calls.len(), 1);
        assert_eq!(root.internal_unit_calls.len(), 1);
        let call = root.internal_calls[0];
        assert_eq!(call.unit_stack, None);
        let call_stack = call
            .scalar_stack
            .expect("cleanup call uses composed scalar stack evidence");
        assert!(cleanup.code_offset < call.offset);
        assert!(stack.mutations.iter().any(|mutation| {
            mutation.offset == cleanup.code_offset
                && mutation.kind == ScalarStackMutationKind::Allocate { byte_size: 16 }
        }));
        assert!(stack.mutations.iter().any(|mutation| {
            mutation.kind == ScalarStackMutationKind::Release { byte_size: 16 }
        }));
        let preservation = stack
            .cleanup_preservation
            .expect("cleanup retains exact result-preservation evidence");
        assert_eq!(preservation.frame.allocation_offset, cleanup.code_offset);
        assert_eq!(preservation.frame.byte_size, 16);
        assert_eq!(preservation.result_byte_offset, 0);
        assert!(preservation.result_store_offset < call.offset);
        assert!(preservation.result_load_offset > call.offset);

        match target.architecture {
            Architecture::X86_64 => {
                assert!(
                    stack
                        .mutations
                        .iter()
                        .any(|mutation| mutation.kind == ScalarStackMutationKind::X86Push)
                );
                assert!(
                    stack
                        .mutations
                        .iter()
                        .any(|mutation| mutation.kind == ScalarStackMutationKind::X86Pop)
                );
                assert_eq!(
                    root.bytes.get(cleanup.code_offset..cleanup.code_offset + 9),
                    Some(&[0x48, 0x83, 0xec, 16, 0x48, 0x89, 0x44, 0x24, 0][..])
                );
                assert_eq!(
                    &root.bytes[root.bytes.len() - 10..],
                    &[0x48, 0x8b, 0x44, 0x24, 0, 0x48, 0x83, 0xc4, 16, 0xc3]
                );
                assert!(call_stack.outbound.is_some());
                assert_eq!(call_stack.aarch64_return_link, None);
                assert_eq!(preservation.aarch64_return_link, None);
            }
            Architecture::Aarch64 => {
                let instructions = aarch64_instructions(&root.bytes);
                let cleanup_instruction = cleanup.code_offset / 4;
                assert_eq!(
                    &instructions[cleanup_instruction..cleanup_instruction + 3],
                    &[0xd100_43ff, 0xf900_03e0, 0xf900_07fe]
                );
                assert_eq!(
                    &instructions[instructions.len() - 4..],
                    &[0xf940_03e0, 0xf940_07fe, 0x9100_43ff, 0xd65f_03c0]
                );
                assert_eq!(call_stack.outbound, None);
                assert_eq!(call_stack.aarch64_return_link, None);
                let link = preservation
                    .aarch64_return_link
                    .expect("cleanup lifetime frame preserves X30");
                assert_eq!(link.frame_byte_offset, 8);
                assert_eq!(link.store_offset, preservation.result_store_offset + 4);
                assert_eq!(link.load_offset, preservation.result_load_offset + 4);
            }
        }
    }
}

fn two_call_executable_nominal_cleanup_plan(
    target: NativeTarget,
) -> (
    TargetOperationPlan,
    EdgeId,
    [OperationId; 2],
    MachineId,
    [MachineId; 2],
) {
    let (mut plan, root_return, first_call, cleanup_machine, first_helper) =
        executable_nominal_cleanup_plan(target);
    let second_call = OperationId::new(2).expect("second cleanup helper call");
    let second_helper = MachineId::new(4).expect("second helper");
    let second_helper_type = StructuralTypeId::new(3).expect("second helper type");
    let second_helper_return = EdgeId::new(4).expect("second helper return");
    let mut helper = plan.functions[2].clone();
    helper.machine = second_helper;
    helper.attachment = Some(second_helper_type);
    helper.provenance.edges = vec![second_helper_return];
    let TargetOperation::UnitBody(helper_body) = &mut helper.operation else {
        unreachable!("helper remains Unit")
    };
    let TargetUnitOperation::Return { psi_edge, .. } = &mut helper_body.operations[0] else {
        unreachable!("helper remains empty")
    };
    *psi_edge = second_helper_return;
    let cleanup = &mut plan.functions[1];
    cleanup.provenance.operations.push(second_call);
    let TargetOperation::UnitBody(cleanup_body) = &mut cleanup.operation else {
        unreachable!("cleanup remains Unit")
    };
    cleanup_body.operations.insert(
        1,
        TargetUnitOperation::Call {
            psi_operation: second_call,
            callee: second_helper,
            arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    );
    plan.functions.push(helper);
    (
        plan,
        root_return,
        [first_call, second_call],
        cleanup_machine,
        [first_helper, second_helper],
    )
}

fn two_nominal_one_executable_plan(
    target: NativeTarget,
    executable_action_ordinal: u32,
) -> (TargetOperationPlan, EdgeId, MachineId) {
    let (mut plan, root_return, _, cleanup_machine, empty_cleanup_machine) =
        executable_nominal_cleanup_plan(target);
    let receiver_type = plan.functions[1].attachment.expect("receiver attachment");
    plan.functions[2].attachment = Some(receiver_type);

    let TargetOperation::UnitBody(root_body) = &mut plan.functions[0].operation else {
        unreachable!("root remains Unit")
    };
    let first_parameter = root_body.parameters[0].clone();
    let second_place = PlaceId::new(2).expect("second receiver place");
    let two_parameter_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![first_parameter.shape, first_parameter.shape],
            result: None,
        },
    )
    .expect("two receiver ABI");
    root_body.call_plan = two_parameter_plan.clone();
    root_body.parameters[0].placement = two_parameter_plan.parameters[0].clone();
    root_body.parameters.push(TargetStructuralParameter {
        place: second_place,
        placement: two_parameter_plan.parameters[1].clone(),
        ..first_parameter.clone()
    });
    let cleanup_for = |place, cleanup_machine| NominalAffineCleanup {
        place,
        structural_type: receiver_type,
        cleanup_machine,
        cleanup_receiver: None,
        requirement_obligations: Vec::new(),
    };
    let actions = match executable_action_ordinal {
        0 => vec![
            psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup_for(
                second_place,
                cleanup_machine,
            )),
            psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup_for(
                first_parameter.place,
                empty_cleanup_machine,
            )),
        ],
        1 => vec![
            psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup_for(
                second_place,
                empty_cleanup_machine,
            )),
            psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup_for(
                first_parameter.place,
                cleanup_machine,
            )),
        ],
        _ => unreachable!("test action ordinal is bounded"),
    };
    let TargetUnitOperation::Return {
        cleanup_actions, ..
    } = &mut root_body.operations[0]
    else {
        unreachable!("root remains a direct return")
    };
    *cleanup_actions = actions;
    (plan, root_return, cleanup_machine)
}

#[test]
fn two_nominal_cleanups_emit_the_exact_executable_action_owner() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        for executable_action_ordinal in 0..=1 {
            let (plan, edge, cleanup_machine) =
                two_nominal_one_executable_plan(target, executable_action_ordinal);
            let emitted =
                emit_machine_code(&plan).expect("exactly one executable nominal cleanup emits");
            let root = &emitted.functions[0];
            assert_eq!(root.internal_calls.len(), 1);
            assert_eq!(root.internal_unit_calls.len(), 1);
            let expected_owner = CallSiteOwner::CleanupAction {
                edge,
                action_ordinal: executable_action_ordinal,
            };
            assert_eq!(root.internal_calls[0].owner, expected_owner);
            assert_eq!(root.internal_unit_calls[0].owner, expected_owner);
            assert_eq!(root.internal_calls[0].target, cleanup_machine);
            assert_eq!(root.internal_unit_calls[0].target, cleanup_machine);
        }

        let (mut both_empty, _, _) = two_nominal_one_executable_plan(target, 0);
        let TargetOperation::UnitBody(cleanup_body) = &mut both_empty.functions[1].operation else {
            unreachable!("cleanup remains Unit")
        };
        cleanup_body.operations.remove(0);
        let emitted = emit_machine_code(&both_empty).expect("two empty cleanups remain admitted");
        assert!(emitted.functions[0].internal_calls.is_empty());
        assert!(emitted.functions[0].internal_unit_calls.is_empty());

        let (mut shared, edge, cleanup_machine) = two_nominal_one_executable_plan(target, 0);
        let TargetOperation::UnitBody(root_body) = &mut shared.functions[0].operation else {
            unreachable!("root remains Unit")
        };
        let TargetUnitOperation::Return {
            cleanup_actions, ..
        } = &mut root_body.operations[0]
        else {
            unreachable!("root remains a direct return")
        };
        let psi_terminal::TerminalAffineCleanupAction::InvokeNominal(second) =
            &mut cleanup_actions[1]
        else {
            unreachable!("second action remains nominal")
        };
        second.cleanup_machine = cleanup_machine;
        let emitted =
            emit_machine_code(&shared).expect("two actions sharing one executable cleanup emit");
        let root = &emitted.functions[0];
        assert_eq!(root.internal_calls.len(), 2);
        assert_eq!(root.internal_unit_calls.len(), 2);
        for (action_ordinal, (relocation, custody)) in root
            .internal_calls
            .iter()
            .zip(&root.internal_unit_calls)
            .enumerate()
        {
            let owner = CallSiteOwner::CleanupAction {
                edge,
                action_ordinal: u32::try_from(action_ordinal).unwrap(),
            };
            assert_eq!(relocation.owner, owner);
            assert_eq!(custody.owner, owner);
            assert_eq!(relocation.target, cleanup_machine);
            assert_eq!(custody.target, cleanup_machine);
        }
        assert!(root.internal_calls[0].offset < root.internal_calls[1].offset);
        assert!(root.internal_unit_calls[0].code_offset < root.internal_unit_calls[1].code_offset);

        let (mut distinct, edge, first_cleanup) = two_nominal_one_executable_plan(target, 0);
        let second_cleanup = distinct.functions[2].machine;
        let helper_machine = MachineId::new(4).expect("shared empty helper");
        let helper_return = EdgeId::new(4).expect("shared helper return");
        let mut helper = distinct.functions[2].clone();
        helper.machine = helper_machine;
        helper.provenance.edges = vec![helper_return];
        let TargetOperation::UnitBody(helper_body) = &mut helper.operation else {
            unreachable!("helper remains Unit")
        };
        let TargetUnitOperation::Return { psi_edge, .. } = &mut helper_body.operations[0] else {
            unreachable!("helper remains empty")
        };
        *psi_edge = helper_return;
        let TargetOperation::UnitBody(first_body) = &mut distinct.functions[1].operation else {
            unreachable!("first cleanup remains Unit")
        };
        let TargetUnitOperation::Call { callee, .. } = &mut first_body.operations[0] else {
            unreachable!("first cleanup remains executable")
        };
        *callee = helper_machine;
        let second_call = OperationId::new(2).expect("second cleanup call");
        distinct.functions[2]
            .provenance
            .operations
            .push(second_call);
        let TargetOperation::UnitBody(second_body) = &mut distinct.functions[2].operation else {
            unreachable!("second cleanup remains Unit")
        };
        second_body.operations.insert(
            0,
            TargetUnitOperation::Call {
                psi_operation: second_call,
                callee: helper_machine,
                arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        );
        distinct.functions.push(helper);
        let emitted = emit_machine_code(&distinct)
            .expect("two distinct executable cleanup targets emit in action order");
        let root = &emitted.functions[0];
        assert_eq!(
            root.internal_calls
                .iter()
                .map(|call| (call.owner, call.target))
                .collect::<Vec<_>>(),
            vec![
                (
                    CallSiteOwner::CleanupAction {
                        edge,
                        action_ordinal: 0,
                    },
                    first_cleanup,
                ),
                (
                    CallSiteOwner::CleanupAction {
                        edge,
                        action_ordinal: 1,
                    },
                    second_cleanup,
                ),
            ]
        );
        assert_eq!(
            root.internal_unit_calls
                .iter()
                .map(|call| (call.owner, call.target))
                .collect::<Vec<_>>(),
            root.internal_calls
                .iter()
                .map(|call| (call.owner, call.target))
                .collect::<Vec<_>>()
        );
        assert!(root.internal_calls[0].offset < root.internal_calls[1].offset);
        assert!(root.internal_unit_calls[0].code_offset < root.internal_unit_calls[1].code_offset);
    }
}

fn assert_two_call_nominal_cleanup(target: NativeTarget) {
    let (plan, root_return, operations, cleanup_machine, helpers) =
        two_call_executable_nominal_cleanup_plan(target);
    let emitted = emit_machine_code(&plan).expect("two-call nominal cleanup emits");
    let root = &emitted.functions[0];
    assert_eq!(root.internal_calls.len(), 1);
    assert_eq!(
        root.internal_calls[0].owner,
        CallSiteOwner::CleanupAction {
            edge: root_return,
            action_ordinal: 0,
        }
    );
    assert_eq!(root.internal_calls[0].target, cleanup_machine);
    let drop = &emitted.functions[1];
    assert_eq!(drop.internal_calls.len(), 2);
    assert_eq!(drop.internal_unit_calls.len(), 2);
    for (ordinal, ((relocation, custody), (operation, helper))) in drop
        .internal_calls
        .iter()
        .zip(&drop.internal_unit_calls)
        .zip(operations.into_iter().zip(helpers))
        .enumerate()
    {
        assert_eq!(relocation.owner, CallSiteOwner::Operation(operation));
        assert_eq!(custody.owner, relocation.owner);
        assert_eq!(relocation.target, helper);
        assert_eq!(custody.target, helper);
        assert_eq!(custody.operation_ordinal, ordinal);
        assert!(custody.arguments.is_empty());
        assert!(custody.claim_transfers.is_empty());
    }
    assert!(drop.internal_calls[0].offset < drop.internal_calls[1].offset);
    assert!(drop.internal_unit_calls[0].code_offset < drop.internal_unit_calls[1].code_offset);
}

#[test]
fn x86_nominal_cleanup_preserves_two_ordered_helper_calls() {
    assert_two_call_nominal_cleanup(NativeTarget::linux_x64());
}

#[test]
fn aarch64_nominal_cleanup_preserves_two_ordered_helper_calls() {
    assert_two_call_nominal_cleanup(NativeTarget::linux_arm64());
}

#[test]
fn x86_executable_nominal_cleanup_call_is_edge_owned_and_precedes_epilogue() {
    let target = NativeTarget::linux_x64();
    let (plan, root_return, cleanup_call, cleanup_machine, helper) =
        executable_nominal_cleanup_plan(target);
    let emitted = emit_machine_code(&plan).expect("x86 executable nominal cleanup emits");
    let root = &emitted.functions[0];
    let [relocation] = root.internal_calls.as_slice() else {
        panic!("root has exactly one executable cleanup call")
    };
    assert_eq!(
        relocation.owner,
        CallSiteOwner::CleanupAction {
            edge: root_return,
            action_ordinal: 0,
        }
    );
    assert_eq!(relocation.target, cleanup_machine);
    assert_eq!(root.bytes[relocation.offset - 1], 0xe8);
    assert_eq!(
        &root.bytes[relocation.offset..relocation.offset + 4],
        &[0; 4]
    );
    let [custody] = root.internal_unit_calls.as_slice() else {
        panic!("root retains exactly one cleanup-call custody row")
    };
    assert_eq!(custody.owner, relocation.owner);
    assert_eq!(custody.target, cleanup_machine);
    assert!(custody.arguments.is_empty());
    assert!(custody.claim_transfers.is_empty());
    let cleanup = root.unit_affine_cleanup.as_ref().expect("cleanup ledger");
    assert_eq!(cleanup.psi_edge, root_return);
    assert!(cleanup.code_offset <= relocation.offset - 1);
    assert!(relocation.offset + 4 < cleanup.code_offset + cleanup.byte_count);
    let frame = root
        .unit_stack
        .expect("one-field receiver has a frame")
        .frame
        .expect("x86 receiver home frame");
    assert!(relocation.offset + 4 < frame.release_offset);
    assert_eq!(root.bytes.last(), Some(&0xc3));

    let drop = &emitted.functions[1];
    assert_eq!(
        drop.internal_calls[0].owner,
        CallSiteOwner::Operation(cleanup_call)
    );
    assert_eq!(drop.internal_calls[0].target, helper);
}

#[test]
fn aarch64_executable_nominal_cleanup_call_is_edge_owned_and_precedes_link_restore() {
    let target = NativeTarget::linux_arm64();
    let (plan, root_return, cleanup_call, cleanup_machine, helper) =
        executable_nominal_cleanup_plan(target);
    let emitted = emit_machine_code(&plan).expect("AArch64 executable nominal cleanup emits");
    let root = &emitted.functions[0];
    let [relocation] = root.internal_calls.as_slice() else {
        panic!("root has exactly one executable cleanup call")
    };
    assert_eq!(
        relocation.owner,
        CallSiteOwner::CleanupAction {
            edge: root_return,
            action_ordinal: 0,
        }
    );
    assert_eq!(relocation.target, cleanup_machine);
    assert_eq!(
        &root.bytes[relocation.offset..relocation.offset + 4],
        &0x9400_0000_u32.to_le_bytes()
    );
    let [custody] = root.internal_unit_calls.as_slice() else {
        panic!("root retains exactly one cleanup-call custody row")
    };
    assert_eq!(custody.owner, relocation.owner);
    assert_eq!(custody.target, cleanup_machine);
    assert!(custody.arguments.is_empty());
    assert!(custody.claim_transfers.is_empty());
    let stack = root.unit_stack.expect("AArch64 Unit stack evidence");
    let link = stack
        .aarch64_return_link
        .expect("AArch64 link preservation");
    assert!(relocation.offset + 4 <= link.load_offset);
    let cleanup = root.unit_affine_cleanup.as_ref().expect("cleanup ledger");
    assert!(cleanup.code_offset <= relocation.offset);
    assert!(link.load_offset < cleanup.code_offset + cleanup.byte_count);
    assert_eq!(
        root.bytes.last_chunk::<4>(),
        Some(&0xd65f_03c0_u32.to_le_bytes())
    );

    let drop = &emitted.functions[1];
    assert_eq!(
        drop.internal_calls[0].owner,
        CallSiteOwner::Operation(cleanup_call)
    );
    assert_eq!(drop.internal_calls[0].target, helper);
}

#[test]
fn nominal_cleanup_emission_preserves_empty_erasure_and_rejects_an_unapproved_two_op_body() {
    let target = NativeTarget::linux_x64();
    let (mut empty, _, _, _, _) = executable_nominal_cleanup_plan(target);
    let TargetOperation::UnitBody(cleanup_body) = &mut empty.functions[1].operation else {
        panic!("cleanup remains a Unit body")
    };
    cleanup_body.operations.remove(0);
    let emitted = emit_machine_code(&empty).expect("empty cleanup remains erasable");
    assert!(emitted.functions[0].internal_calls.is_empty());
    assert!(emitted.functions[0].internal_unit_calls.is_empty());
    assert!(
        emitted.functions[0]
            .unit_affine_cleanup
            .as_ref()
            .is_some_and(|cleanup| {
                matches!(
                    cleanup.actions.as_slice(),
                    [psi_terminal::TerminalAffineCleanupAction::InvokeNominal(_)]
                )
            })
    );

    let (mut forged, _, cleanup_call, cleanup_machine, _) = executable_nominal_cleanup_plan(target);
    let TargetOperation::UnitBody(cleanup_body) = &mut forged.functions[1].operation else {
        panic!("cleanup remains a Unit body")
    };
    cleanup_body.operations[0] = TargetUnitOperation::PortWrite {
        psi_operation: cleanup_call,
        service: ServiceId::new(1).expect("service"),
        port: 0x20,
        value: 0x20,
    };
    assert_eq!(
        emit_machine_code(&forged),
        Err(EmissionError::InvalidNominalCleanupTarget(cleanup_machine))
    );

    let (mut duplicate, _, _, cleanup_machine, helpers) =
        two_call_executable_nominal_cleanup_plan(target);
    let TargetOperation::UnitBody(cleanup_body) = &mut duplicate.functions[1].operation else {
        panic!("cleanup remains a Unit body")
    };
    let TargetUnitOperation::Call { callee, .. } = &mut cleanup_body.operations[1] else {
        panic!("second operation remains a call")
    };
    *callee = helpers[0];
    assert_eq!(
        emit_machine_code(&duplicate),
        Err(EmissionError::InvalidNominalCleanupTarget(cleanup_machine))
    );

    let (mut duplicate_owner, _, operations, cleanup_machine, _) =
        two_call_executable_nominal_cleanup_plan(target);
    let TargetOperation::UnitBody(cleanup_body) = &mut duplicate_owner.functions[1].operation
    else {
        panic!("cleanup remains a Unit body")
    };
    let TargetUnitOperation::Call { psi_operation, .. } = &mut cleanup_body.operations[1] else {
        panic!("second operation remains a call")
    };
    *psi_operation = operations[0];
    assert_eq!(
        emit_machine_code(&duplicate_owner),
        Err(EmissionError::InvalidNominalCleanupTarget(cleanup_machine))
    );
}

#[test]
fn x86_unit_call_port_write_and_settlement_keep_exact_order() {
    let target = NativeTarget::linux_x64();
    let empty_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature::default(),
    )
    .expect("empty Unit ABI");
    let call_operation = OperationId::new(1).expect("call");
    let port_operation = OperationId::new(2).expect("port");
    let settlement_operation = OperationId::new(3).expect("settlement");
    let root_return = EdgeId::new(1).expect("root return");
    let leaf_return = EdgeId::new(2).expect("leaf return");
    let boundary = BoundaryMachineId::new(1).expect("boundary");
    let provider_plan = ProviderPlanReportIdentity::new(7).expect("provider");
    let provider_execution =
        ProviderExecutionBinding::from_execution_record(provider_plan, 8, 9, 10, 11)
            .expect("provider execution");
    let realization = MetadataOnlyPortRealization {
        effect_operation: port_operation,
        service: ServiceId::new(1).expect("PortIo"),
        port: 0x20,
        value: 0x20,
    };
    let settlement_arguments = vec![StructuralArgument {
        access: StructuralAccess::Owned,
        place: PlaceId::new(41).expect("custody argument"),
        path: vec![
            StructuralPathSegment::Field("#payload".into()),
            StructuralPathSegment::FixedIndex(3),
        ],
    }];
    let plan = TargetOperationPlan {
        psi: identity(),
        target,
        entry: MachineId::new(1).expect("root"),
        functions: vec![
            TargetFunction {
                fixed_integer_scalar_abi: None,
                mixed_structural_scalar_abi: None,
                machine: MachineId::new(1).expect("root"),
                attachment: None,
                provenance: TerminalPsiProvenance {
                    operations: vec![call_operation],
                    edges: vec![root_return],
                },
                operation: TargetOperation::UnitBody(TargetUnitBody {
                    structural_types: Vec::new(),
                    call_plan: empty_call_plan.clone(),
                    scalar_parameters: Vec::new(),
                    parameters: Vec::new(),
                    operations: vec![
                        TargetUnitOperation::Call {
                            psi_operation: call_operation,
                            callee: MachineId::new(2).expect("leaf"),
                            arguments: Vec::new(),
                            claim_transfers: Vec::new(),
                            requirement_obligations: Vec::new(),
                            crash_continuations: Vec::new(),
                        },
                        TargetUnitOperation::Return {
                            psi_edge: root_return,
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }),
            },
            TargetFunction {
                fixed_integer_scalar_abi: None,
                mixed_structural_scalar_abi: None,
                machine: MachineId::new(2).expect("leaf"),
                attachment: None,
                provenance: TerminalPsiProvenance {
                    operations: vec![port_operation, settlement_operation],
                    edges: vec![leaf_return],
                },
                operation: TargetOperation::UnitBody(TargetUnitBody {
                    structural_types: Vec::new(),
                    call_plan: empty_call_plan,
                    scalar_parameters: Vec::new(),
                    parameters: Vec::new(),
                    operations: vec![
                        TargetUnitOperation::PortWrite {
                            psi_operation: port_operation,
                            service: ServiceId::new(1).expect("PortIo"),
                            port: 0x20,
                            value: 0x20,
                        },
                        TargetUnitOperation::BoundarySettlement {
                            psi_operation: settlement_operation,
                            boundary,
                            execution: provider_execution.into(),
                            realization: realization.into(),
                            scalar_arguments: Vec::new(),
                            arguments: settlement_arguments.clone(),
                            byte_sequence_arguments: Vec::new(),
                            completion_claim_sources: Vec::new(),
                            completion_receipts: Vec::new(),
                        },
                        TargetUnitOperation::Return {
                            psi_edge: leaf_return,
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }),
            },
        ],
    };

    let emitted = emit_machine_code(&plan).expect("Unit/effect emission");
    let root = &emitted.functions[0];
    assert_eq!(
        root.bytes,
        [
            0x48, 0x83, 0xec, 0x08, 0xe8, 0, 0, 0, 0, 0x48, 0x83, 0xc4, 0x08, 0xc3,
        ]
    );
    assert_eq!(root.internal_calls[0].offset, 5);
    assert_eq!(root.internal_calls[0].target, MachineId::new(2).unwrap());
    assert_eq!(
        root.unit_stack,
        Some(UnitStackEvidence {
            frame: None,
            aarch64_return_link: None,
            stack_alignment: 16,
        })
    );
    assert_eq!(
        root.internal_calls[0].unit_stack,
        Some(UnitCallStackEvidence {
            outbound: Some(StackAdjustmentPair {
                byte_size: 8,
                allocation_offset: 0,
                allocation_byte_count: 4,
                release_offset: 9,
                release_byte_count: 4,
            }),
        })
    );

    let leaf = &emitted.functions[1];
    let mut expected = omega_x86_encoding::encode_immediate_port_write(0x20, 0x20).to_vec();
    expected.push(0xc3);
    assert_eq!(leaf.bytes, expected);
    assert_eq!(
        leaf.unit_stack,
        Some(UnitStackEvidence {
            frame: None,
            aarch64_return_link: None,
            stack_alignment: 16,
        })
    );
    assert_eq!(leaf.bytes.iter().filter(|byte| **byte == 0xee).count(), 1);
    assert_eq!(leaf.boundary_settlements.len(), 1);
    assert_eq!(leaf.boundary_settlements[0].code_offset, 27);
    assert_eq!(leaf.boundary_settlements[0].boundary, boundary);
    assert_eq!(
        leaf.boundary_settlements[0].execution,
        omega_machine_code::BoundaryExecutionRecord::AdmittedProvider(provider_execution.into())
    );
    assert_eq!(leaf.boundary_settlements[0].realization, realization.into());
    assert_eq!(leaf.boundary_settlements[0].arguments, settlement_arguments);
    assert_eq!(leaf.port_effects.len(), 1);
    assert_eq!(leaf.port_effects[0].service, realization.service);
    assert_eq!(leaf.semantic_code_attribution.len(), 3);
    assert_eq!(
        leaf.semantic_code_attribution
            .iter()
            .map(|row| (row.site, row.code_offset, row.byte_count))
            .collect::<Vec<_>>(),
        [
            (SemanticCodeSite::Operation(port_operation), 0, 27),
            (SemanticCodeSite::Operation(settlement_operation), 27, 0),
            (SemanticCodeSite::Edge(leaf_return), 27, 1),
        ]
    );
}

#[test]
fn aarch64_rejects_port_write_before_emitting_a_partial_body() {
    let target = NativeTarget::linux_arm64();
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature::default(),
    )
    .expect("empty Unit ABI");
    let plan = TargetOperationPlan {
        psi: identity(),
        target,
        entry: MachineId::new(1).unwrap(),
        functions: vec![TargetFunction {
            fixed_integer_scalar_abi: None,
            mixed_structural_scalar_abi: None,
            machine: MachineId::new(1).unwrap(),
            attachment: None,
            provenance: TerminalPsiProvenance::default(),
            operation: TargetOperation::UnitBody(TargetUnitBody {
                structural_types: Vec::new(),
                call_plan,
                scalar_parameters: Vec::new(),
                parameters: Vec::new(),
                operations: vec![
                    TargetUnitOperation::PortWrite {
                        psi_operation: OperationId::new(1).unwrap(),
                        service: ServiceId::new(1).unwrap(),
                        port: 0x20,
                        value: 0x20,
                    },
                    TargetUnitOperation::Return {
                        psi_edge: EdgeId::new(1).unwrap(),
                        cleanup_actions: Vec::new(),
                    },
                ],
            }),
        }],
    };
    assert_eq!(
        emit_machine_code(&plan),
        Err(EmissionError::PortWriteUnsupportedOnArchitecture(
            Architecture::Aarch64
        ))
    );
}

#[test]
fn forty_byte_unit_argument_is_copied_for_sysv_and_forwarded_indirectly_elsewhere() {
    for (target, expected_length, expected_relocation) in [
        (NativeTarget::linux_x64(), 122, 109),
        (NativeTarget::windows_x64(), 32, 19),
        (NativeTarget::linux_arm64(), 84, 64),
    ] {
        let shape = omega_calling_conventions::ValueShape::integer(40, 8);
        let call_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![shape],
                result: None,
            },
        )
        .unwrap();
        let place = PlaceId::new(1).unwrap();
        let structural_type = StructuralTypeId::new(1).unwrap();
        let argument = TargetStructuralArgument {
            access: StructuralAccess::Owned,
            place,
            path: Vec::new(),
            root_structural_type: structural_type,
            structural_type,
            shape,
            source_byte_offset: 0,
            fixed_array_length: None,
            element_stride: None,
            source: call_plan.parameters[0].clone(),
            destination: call_plan.parameters[0].clone(),
        };
        let parameter = TargetStructuralParameter {
            access: StructuralAccess::Owned,
            place,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            projected_qualifications: Vec::new(),
            shape,
            placement: call_plan.parameters[0].clone(),
        };
        let plan = TargetOperationPlan {
            psi: identity(),
            target,
            entry: MachineId::new(1).unwrap(),
            functions: vec![
                TargetFunction {
                    fixed_integer_scalar_abi: None,
                    mixed_structural_scalar_abi: None,
                    machine: MachineId::new(1).unwrap(),
                    attachment: None,
                    provenance: TerminalPsiProvenance::default(),
                    operation: TargetOperation::UnitBody(TargetUnitBody {
                        structural_types: Vec::new(),
                        call_plan: call_plan.clone(),
                        scalar_parameters: Vec::new(),
                        parameters: vec![parameter.clone()],
                        operations: vec![
                            TargetUnitOperation::Call {
                                psi_operation: OperationId::new(1).unwrap(),
                                callee: MachineId::new(2).unwrap(),
                                arguments: vec![argument],
                                claim_transfers: Vec::new(),
                                requirement_obligations: Vec::new(),
                                crash_continuations: Vec::new(),
                            },
                            TargetUnitOperation::Return {
                                psi_edge: EdgeId::new(1).unwrap(),
                                cleanup_actions: Vec::new(),
                            },
                        ],
                    }),
                },
                TargetFunction {
                    fixed_integer_scalar_abi: None,
                    mixed_structural_scalar_abi: None,
                    machine: MachineId::new(2).unwrap(),
                    attachment: None,
                    provenance: TerminalPsiProvenance::default(),
                    operation: TargetOperation::UnitBody(TargetUnitBody {
                        structural_types: Vec::new(),
                        call_plan,
                        scalar_parameters: Vec::new(),
                        parameters: vec![parameter],
                        operations: vec![TargetUnitOperation::Return {
                            psi_edge: EdgeId::new(2).unwrap(),
                            cleanup_actions: Vec::new(),
                        }],
                    }),
                },
            ],
        };
        let emitted = emit_machine_code(&plan).unwrap();
        assert_eq!(emitted.functions[0].bytes.len(), expected_length);
        assert_eq!(
            emitted.functions[0].internal_calls[0].offset,
            expected_relocation
        );
    }
}

#[test]
fn x86_unit_parameter_homes_survive_effects_and_parallel_reordering() {
    let target = NativeTarget::linux_x64();
    let shape = omega_calling_conventions::ValueShape::integer(8, 8);
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![shape, shape],
            result: None,
        },
    )
    .unwrap();
    let first = PlaceId::new(1).unwrap();
    let second = PlaceId::new(2).unwrap();
    let ty = StructuralTypeId::new(1).unwrap();
    let parameter = |place: PlaceId, index: usize| TargetStructuralParameter {
        access: StructuralAccess::Owned,
        place,
        structural_type: ty,
        multiplicity: StructuralMultiplicity::Unrestricted,
        projected_qualifications: Vec::new(),
        shape,
        placement: call_plan.parameters[index].clone(),
    };
    let argument = |place: PlaceId, source: usize, destination: usize| TargetStructuralArgument {
        access: StructuralAccess::Owned,
        place,
        path: Vec::new(),
        root_structural_type: ty,
        structural_type: ty,
        shape,
        source_byte_offset: 0,
        fixed_array_length: None,
        element_stride: None,
        source: call_plan.parameters[source].clone(),
        destination: call_plan.parameters[destination].clone(),
    };
    let plan = TargetOperationPlan {
        psi: identity(),
        target,
        entry: MachineId::new(1).unwrap(),
        functions: vec![
            TargetFunction {
                fixed_integer_scalar_abi: None,
                mixed_structural_scalar_abi: None,
                machine: MachineId::new(1).unwrap(),
                attachment: None,
                provenance: TerminalPsiProvenance::default(),
                operation: TargetOperation::UnitBody(TargetUnitBody {
                    structural_types: Vec::new(),
                    call_plan: call_plan.clone(),
                    scalar_parameters: Vec::new(),
                    parameters: vec![parameter(first, 0), parameter(second, 1)],
                    operations: vec![
                        TargetUnitOperation::PortWrite {
                            psi_operation: OperationId::new(1).unwrap(),
                            service: ServiceId::new(1).unwrap(),
                            port: 0x20,
                            value: 0x20,
                        },
                        TargetUnitOperation::Call {
                            psi_operation: OperationId::new(2).unwrap(),
                            callee: MachineId::new(2).unwrap(),
                            arguments: vec![argument(second, 1, 0), argument(first, 0, 1)],
                            claim_transfers: Vec::new(),
                            requirement_obligations: Vec::new(),
                            crash_continuations: Vec::new(),
                        },
                        TargetUnitOperation::Return {
                            psi_edge: EdgeId::new(1).unwrap(),
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }),
            },
            TargetFunction {
                fixed_integer_scalar_abi: None,
                mixed_structural_scalar_abi: None,
                machine: MachineId::new(2).unwrap(),
                attachment: None,
                provenance: TerminalPsiProvenance::default(),
                operation: TargetOperation::UnitBody(TargetUnitBody {
                    structural_types: Vec::new(),
                    call_plan: call_plan.clone(),
                    scalar_parameters: Vec::new(),
                    parameters: vec![parameter(first, 0), parameter(second, 1)],
                    operations: vec![TargetUnitOperation::Return {
                        psi_edge: EdgeId::new(2).unwrap(),
                        cleanup_actions: Vec::new(),
                    }],
                }),
            },
        ],
    };
    let emitted = emit_machine_code(&plan).unwrap();
    let bytes = &emitted.functions[0].bytes;
    let out = bytes.iter().position(|byte| *byte == 0xee).unwrap();
    let load_second_into_first = bytes
        .windows(5)
        .position(|window| window == [0x48, 0x8b, 0x7c, 0x24, 0x10])
        .unwrap();
    let load_first_into_second = bytes
        .windows(5)
        .position(|window| window == [0x48, 0x8b, 0x74, 0x24, 0x08])
        .unwrap();
    assert!(out < load_second_into_first);
    assert!(out < load_first_into_second);
}

#[test]
fn aarch64_unit_parameter_homes_survive_parallel_reordering_and_restore_lr() {
    let target = NativeTarget::linux_arm64();
    let shape = omega_calling_conventions::ValueShape::integer(8, 8);
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![shape, shape],
            result: None,
        },
    )
    .unwrap();
    let first = PlaceId::new(1).unwrap();
    let second = PlaceId::new(2).unwrap();
    let ty = StructuralTypeId::new(1).unwrap();
    let parameter = |place: PlaceId, index: usize| TargetStructuralParameter {
        access: StructuralAccess::Owned,
        place,
        structural_type: ty,
        multiplicity: StructuralMultiplicity::Unrestricted,
        projected_qualifications: Vec::new(),
        shape,
        placement: call_plan.parameters[index].clone(),
    };
    let argument = |place: PlaceId, source: usize, destination: usize| TargetStructuralArgument {
        access: StructuralAccess::Owned,
        place,
        path: Vec::new(),
        root_structural_type: ty,
        structural_type: ty,
        shape,
        source_byte_offset: 0,
        fixed_array_length: None,
        element_stride: None,
        source: call_plan.parameters[source].clone(),
        destination: call_plan.parameters[destination].clone(),
    };
    let plan = TargetOperationPlan {
        psi: identity(),
        target,
        entry: MachineId::new(1).unwrap(),
        functions: vec![
            TargetFunction {
                fixed_integer_scalar_abi: None,
                mixed_structural_scalar_abi: None,
                machine: MachineId::new(1).unwrap(),
                attachment: None,
                provenance: TerminalPsiProvenance::default(),
                operation: TargetOperation::UnitBody(TargetUnitBody {
                    structural_types: Vec::new(),
                    call_plan: call_plan.clone(),
                    scalar_parameters: Vec::new(),
                    parameters: vec![parameter(first, 0), parameter(second, 1)],
                    operations: vec![
                        TargetUnitOperation::Call {
                            psi_operation: OperationId::new(1).unwrap(),
                            callee: MachineId::new(2).unwrap(),
                            arguments: vec![argument(second, 1, 0), argument(first, 0, 1)],
                            claim_transfers: Vec::new(),
                            requirement_obligations: Vec::new(),
                            crash_continuations: Vec::new(),
                        },
                        TargetUnitOperation::Return {
                            psi_edge: EdgeId::new(1).unwrap(),
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }),
            },
            TargetFunction {
                fixed_integer_scalar_abi: None,
                mixed_structural_scalar_abi: None,
                machine: MachineId::new(2).unwrap(),
                attachment: None,
                provenance: TerminalPsiProvenance::default(),
                operation: TargetOperation::UnitBody(TargetUnitBody {
                    structural_types: Vec::new(),
                    call_plan: call_plan.clone(),
                    scalar_parameters: Vec::new(),
                    parameters: vec![parameter(first, 0), parameter(second, 1)],
                    operations: vec![TargetUnitOperation::Return {
                        psi_edge: EdgeId::new(2).unwrap(),
                        cleanup_actions: Vec::new(),
                    }],
                }),
            },
        ],
    };
    let emitted = emit_machine_code(&plan).unwrap();
    let caller = &emitted.functions[0];
    let instructions = aarch64_instructions(&caller.bytes);
    assert_eq!(caller.internal_calls[0].offset, 24);
    assert_eq!(caller.internal_calls[0].target, MachineId::new(2).unwrap());
    assert_eq!(
        caller.unit_stack,
        Some(UnitStackEvidence {
            frame: Some(StackAdjustmentPair {
                byte_size: 32,
                allocation_offset: 0,
                allocation_byte_count: 4,
                release_offset: 32,
                release_byte_count: 4,
            }),
            aarch64_return_link: Some(Aarch64ReturnLinkEvidence {
                frame_byte_offset: 16,
                store_offset: 4,
                load_offset: 28,
            }),
            stack_alignment: 16,
        })
    );
    assert_eq!(
        caller.internal_calls[0].unit_stack,
        Some(UnitCallStackEvidence { outbound: None })
    );
    assert_eq!(instructions[0], 0xd100_83ff); // sub sp, sp, #32
    assert_eq!(instructions[1], 0xf900_0bfe); // str x30, [sp, #16]
    assert_eq!(instructions[2], 0xf900_03e0); // str x0, [sp]
    assert_eq!(instructions[3], 0xf900_07e1); // str x1, [sp, #8]
    assert_eq!(instructions[4], 0xf940_07e0); // ldr x0, [sp, #8]
    assert_eq!(instructions[5], 0xf940_03e1); // ldr x1, [sp]
    assert_eq!(instructions[6], 0x9400_0000); // bl #0
    assert_eq!(instructions[7], 0xf940_0bfe); // ldr x30, [sp, #16]
    assert_eq!(instructions[8], 0x9100_83ff); // add sp, sp, #32
    assert_eq!(instructions[9], 0xd65f_03c0); // ret x30
    assert_eq!(caller.semantic_code_attribution[0].code_offset, 16);
    assert_eq!(caller.semantic_code_attribution[0].byte_count, 12);
    assert_eq!(caller.semantic_code_attribution[1].code_offset, 28);
    assert_eq!(caller.semantic_code_attribution[1].byte_count, 12);
}

#[test]
fn aarch64_unit_calls_cover_stack_fragments_and_stack_indirect_copies() {
    for final_shape in [
        omega_calling_conventions::ValueShape::integer(16, 8),
        omega_calling_conventions::ValueShape::integer(24, 16),
    ] {
        let target = NativeTarget::linux_arm64();
        let word = omega_calling_conventions::ValueShape::integer(8, 8);
        let mut shapes = vec![word; 8];
        shapes.push(final_shape);
        let call_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: shapes.clone(),
                result: None,
            },
        )
        .unwrap();
        let ty = StructuralTypeId::new(1).unwrap();
        let parameters = shapes
            .iter()
            .enumerate()
            .map(|(index, shape)| TargetStructuralParameter {
                access: StructuralAccess::Owned,
                place: PlaceId::new(index as u64 + 1).unwrap(),
                structural_type: ty,
                multiplicity: StructuralMultiplicity::Unrestricted,
                projected_qualifications: Vec::new(),
                shape: *shape,
                placement: call_plan.parameters[index].clone(),
            })
            .collect::<Vec<_>>();
        let arguments = parameters
            .iter()
            .map(|parameter| TargetStructuralArgument {
                access: StructuralAccess::Owned,
                place: parameter.place,
                path: Vec::new(),
                root_structural_type: ty,
                structural_type: ty,
                shape: parameter.shape,
                source_byte_offset: 0,
                fixed_array_length: None,
                element_stride: None,
                source: parameter.placement.clone(),
                destination: parameter.placement.clone(),
            })
            .collect::<Vec<_>>();
        let plan = TargetOperationPlan {
            psi: identity(),
            target,
            entry: MachineId::new(1).unwrap(),
            functions: vec![
                TargetFunction {
                    fixed_integer_scalar_abi: None,
                    mixed_structural_scalar_abi: None,
                    machine: MachineId::new(1).unwrap(),
                    attachment: None,
                    provenance: TerminalPsiProvenance::default(),
                    operation: TargetOperation::UnitBody(TargetUnitBody {
                        structural_types: Vec::new(),
                        call_plan: call_plan.clone(),
                        scalar_parameters: Vec::new(),
                        parameters: parameters.clone(),
                        operations: vec![
                            TargetUnitOperation::Call {
                                psi_operation: OperationId::new(1).unwrap(),
                                callee: MachineId::new(2).unwrap(),
                                arguments,
                                claim_transfers: Vec::new(),
                                requirement_obligations: Vec::new(),
                                crash_continuations: Vec::new(),
                            },
                            TargetUnitOperation::Return {
                                psi_edge: EdgeId::new(1).unwrap(),
                                cleanup_actions: Vec::new(),
                            },
                        ],
                    }),
                },
                TargetFunction {
                    fixed_integer_scalar_abi: None,
                    mixed_structural_scalar_abi: None,
                    machine: MachineId::new(2).unwrap(),
                    attachment: None,
                    provenance: TerminalPsiProvenance::default(),
                    operation: TargetOperation::UnitBody(TargetUnitBody {
                        structural_types: Vec::new(),
                        call_plan,
                        scalar_parameters: Vec::new(),
                        parameters,
                        operations: vec![TargetUnitOperation::Return {
                            psi_edge: EdgeId::new(2).unwrap(),
                            cleanup_actions: Vec::new(),
                        }],
                    }),
                },
            ],
        };
        let emitted = emit_machine_code(&plan).unwrap_or_else(|error| {
            panic!(
                "AAPCS64 {}-byte exhausted Unit argument failed: {error:?}",
                final_shape.byte_size
            )
        });
        let caller = &emitted.functions[0];
        assert_eq!(caller.internal_calls.len(), 1);
        assert_eq!(caller.internal_calls[0].target, MachineId::new(2).unwrap());
        assert!(aarch64_instructions(&caller.bytes).contains(&0x9400_0000));
        assert_eq!(
            *aarch64_instructions(&caller.bytes).last().unwrap(),
            0xd65f_03c0
        );
    }
}

#[test]
fn unit_argument_fragments_cover_native_scalar_widths() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::linux_arm64(),
    ] {
        for byte_size in [1_u16, 2, 4, 8, 12, 16] {
            let alignment = byte_size.min(8);
            let shape = omega_calling_conventions::ValueShape::integer(byte_size, alignment);
            let call_plan = evaluate_call_plan(
                CallingPolicy::native_for_target(target),
                &CallSignature {
                    parameters: vec![shape],
                    result: None,
                },
            )
            .unwrap();
            let place = PlaceId::new(1).unwrap();
            let ty = StructuralTypeId::new(1).unwrap();
            let parameter = TargetStructuralParameter {
                access: StructuralAccess::Owned,
                place,
                structural_type: ty,
                multiplicity: StructuralMultiplicity::Unrestricted,
                projected_qualifications: Vec::new(),
                shape,
                placement: call_plan.parameters[0].clone(),
            };
            let argument = TargetStructuralArgument {
                access: StructuralAccess::Owned,
                place,
                path: Vec::new(),
                root_structural_type: ty,
                structural_type: ty,
                shape,
                source_byte_offset: 0,
                fixed_array_length: None,
                element_stride: None,
                source: call_plan.parameters[0].clone(),
                destination: call_plan.parameters[0].clone(),
            };
            let plan = TargetOperationPlan {
                psi: identity(),
                target,
                entry: MachineId::new(1).unwrap(),
                functions: vec![
                    TargetFunction {
                        fixed_integer_scalar_abi: None,
                        mixed_structural_scalar_abi: None,
                        machine: MachineId::new(1).unwrap(),
                        attachment: None,
                        provenance: TerminalPsiProvenance::default(),
                        operation: TargetOperation::UnitBody(TargetUnitBody {
                            structural_types: Vec::new(),
                            call_plan: call_plan.clone(),
                            scalar_parameters: Vec::new(),
                            parameters: vec![parameter.clone()],
                            operations: vec![
                                TargetUnitOperation::Call {
                                    psi_operation: OperationId::new(1).unwrap(),
                                    callee: MachineId::new(2).unwrap(),
                                    arguments: vec![argument],
                                    claim_transfers: Vec::new(),
                                    requirement_obligations: Vec::new(),
                                    crash_continuations: Vec::new(),
                                },
                                TargetUnitOperation::Return {
                                    psi_edge: EdgeId::new(1).unwrap(),
                                    cleanup_actions: Vec::new(),
                                },
                            ],
                        }),
                    },
                    TargetFunction {
                        fixed_integer_scalar_abi: None,
                        mixed_structural_scalar_abi: None,
                        machine: MachineId::new(2).unwrap(),
                        attachment: None,
                        provenance: TerminalPsiProvenance::default(),
                        operation: TargetOperation::UnitBody(TargetUnitBody {
                            structural_types: Vec::new(),
                            call_plan,
                            scalar_parameters: Vec::new(),
                            parameters: vec![parameter],
                            operations: vec![TargetUnitOperation::Return {
                                psi_edge: EdgeId::new(2).unwrap(),
                                cleanup_actions: Vec::new(),
                            }],
                        }),
                    },
                ],
            };
            emit_machine_code(&plan).unwrap_or_else(|error| {
                panic!("{target:?} {byte_size}-byte Unit argument failed: {error:?}")
            });
        }
    }
}

fn plan(target: NativeTarget) -> TargetOperationPlan {
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    TargetOperationPlan {
        psi: identity(),
        target,
        entry: MachineId::new(1).expect("machine"),
        functions: vec![TargetFunction {
            fixed_integer_scalar_abi: None,
            mixed_structural_scalar_abi: None,
            machine: MachineId::new(1).expect("machine"),
            attachment: None,
            provenance: TerminalPsiProvenance::default(),
            operation: TargetOperation::ReturnIntegerImmediate {
                psi_edge: EdgeId::new(1).expect("edge"),
                source_value: ValueId::new(1).expect("value"),
                scalar_type: i32_type,
                value: IntegerValue::Signed(7),
            },
        }],
    }
}

fn conditional_plan(target: NativeTarget) -> TargetOperationPlan {
    let locations = match target.architecture {
        Architecture::X86_64 => [
            MachineRegister::X86Rdi,
            MachineRegister::X86Rsi,
            MachineRegister::X86Rdx,
        ],
        Architecture::Aarch64 => [
            MachineRegister::Aarch64X(0),
            MachineRegister::Aarch64X(1),
            MachineRegister::Aarch64X(2),
        ],
    };
    let arm =
        |edge, return_edge, source_value, parameter_index, register| TargetConditionalIntegerArm {
            psi_edge: EdgeId::new(edge).expect("edge"),
            control: Box::new(TargetIntegerControl::Return {
                psi_return_edge: EdgeId::new(return_edge).expect("return edge"),
                source_value: ValueId::new(source_value).expect("source value"),
                expression: TargetIntegerExpression::Parameter {
                    source_value: ValueId::new(source_value).expect("argument value"),
                    parameter_index,
                    location: ScalarParameterLocation::Register(register),
                },
            }),
        };
    TargetOperationPlan {
        psi: identity(),
        target,
        entry: MachineId::new(1).expect("machine"),
        functions: vec![TargetFunction {
            fixed_integer_scalar_abi: None,
            mixed_structural_scalar_abi: None,
            machine: MachineId::new(1).expect("machine"),
            attachment: None,
            provenance: TerminalPsiProvenance::default(),
            operation: TargetOperation::ReturnIntegerConditionalControl {
                condition_source: ValueId::new(1).expect("condition"),
                condition_parameter_index: 0,
                condition_location: ScalarParameterLocation::Register(locations[0]),
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
                when_true: arm(1, 3, 2, 1, locations[1]),
                when_false: arm(2, 4, 3, 2, locations[2]),
            },
        }],
    }
}

#[test]
fn emits_x86_64_return_immediate() {
    let emitted = emit_machine_code(&plan(NativeTarget::linux_x64())).expect("emit");
    assert_eq!(emitted.functions[0].bytes, [0xb8, 7, 0, 0, 0, 0xc3]);
}

#[test]
fn emits_aarch64_return_immediate() {
    let emitted = emit_machine_code(&plan(NativeTarget::linux_arm64())).expect("emit");
    assert_eq!(
        emitted.functions[0].bytes,
        [0xe0, 0x00, 0x80, 0x52, 0xc0, 0x03, 0x5f, 0xd6]
    );
}

fn integer_structural_field_plan(
    target: NativeTarget,
    integer_type: IntegerType,
) -> TargetOperationPlan {
    let pointer = match target.architecture {
        Architecture::X86_64 => MachineRegister::X86Rdi,
        Architecture::Aarch64 => MachineRegister::Aarch64X(1),
    };
    let source_value = ValueId::new(9).expect("structural field value");
    TargetOperationPlan {
        psi: identity(),
        target,
        entry: MachineId::new(1).expect("machine"),
        functions: vec![TargetFunction {
            fixed_integer_scalar_abi: None,
            mixed_structural_scalar_abi: None,
            machine: MachineId::new(1).expect("machine"),
            attachment: None,
            provenance: TerminalPsiProvenance::default(),
            operation: TargetOperation::ReturnIntegerExpression {
                psi_edge: EdgeId::new(1).expect("edge"),
                source_value,
                scalar_type: integer_type,
                expression: TargetIntegerExpression::StructuralField {
                    psi_operation: OperationId::new(8).expect("field read"),
                    source_value,
                    source: PlaceId::new(7).expect("structural source"),
                    field: StructuralFieldId::new(6).expect("field identity"),
                    source_placement: ValuePlacement {
                        shape: ValueShape::integer(24, 8),
                        locations: vec![ValueLocation::Indirect {
                            pointer: omega_calling_conventions::IndirectPointerLocation::Register(
                                pointer,
                            ),
                            copy_stack_byte_offset: None,
                            byte_size: 24,
                            alignment: 8,
                        }],
                    },
                    field_byte_offset: 12,
                    integer_type,
                },
            },
        }],
    }
}

#[test]
fn emits_signed_i32_structural_field_reads_for_native_targets() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let x86 = emit_machine_code(&integer_structural_field_plan(
        NativeTarget::linux_x64(),
        integer_type,
    ))
    .expect("emit x86-64 structural integer field");
    assert_eq!(
        x86.functions[0].bytes,
        [0x40, 0x8b, 0x47, 0x0c, 0x48, 0x63, 0xc0, 0xc3]
    );

    let aarch64 = emit_machine_code(&integer_structural_field_plan(
        NativeTarget::linux_arm64(),
        integer_type,
    ))
    .expect("emit AArch64 structural integer field");
    assert_eq!(
        aarch64_instructions(&aarch64.functions[0].bytes),
        [0xb940_0c20, 0x9340_7c00, 0xd65f_03c0]
    );
}

#[test]
fn rejects_structural_field_integer_type_drift_before_emission() {
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let mut plan = integer_structural_field_plan(NativeTarget::linux_x64(), i32_type);
    let TargetOperation::ReturnIntegerExpression { expression, .. } =
        &mut plan.functions[0].operation
    else {
        unreachable!()
    };
    let TargetIntegerExpression::StructuralField { integer_type, .. } = expression else {
        unreachable!()
    };
    *integer_type = u32_type;
    assert!(matches!(
        emit_machine_code(&plan),
        Err(EmissionError::StructuralIntegerTypeMismatch {
            expected,
            actual,
            ..
        }) if expected == i32_type && actual == u32_type
    ));
}

#[test]
fn emits_canonical_boolean_returns_for_both_architectures() {
    let boolean_plan = |target, value| TargetOperationPlan {
        psi: identity(),
        target,
        entry: MachineId::new(1).expect("machine"),
        functions: vec![TargetFunction {
            fixed_integer_scalar_abi: None,
            mixed_structural_scalar_abi: None,
            machine: MachineId::new(1).expect("machine"),
            attachment: None,
            provenance: TerminalPsiProvenance::default(),
            operation: TargetOperation::ReturnBooleanImmediate {
                psi_edge: EdgeId::new(1).expect("edge"),
                source_value: ValueId::new(1).expect("value"),
                value,
            },
        }],
    };

    assert_eq!(
        emit_machine_code(&boolean_plan(NativeTarget::linux_x64(), true))
            .unwrap()
            .functions[0]
            .bytes,
        [0xb8, 1, 0, 0, 0, 0xc3]
    );
    assert_eq!(
        emit_machine_code(&boolean_plan(NativeTarget::linux_arm64(), false))
            .unwrap()
            .functions[0]
            .bytes,
        [0x00, 0x00, 0x80, 0x52, 0xc0, 0x03, 0x5f, 0xd6]
    );
}

#[test]
fn emits_runtime_boolean_equality_for_both_architectures() {
    let x86 = emit_machine_code(&boolean_equality_plan(
        NativeTarget::linux_x64(),
        MachineRegister::X86Rdi,
        MachineRegister::X86Rsi,
    ))
    .unwrap();
    assert_eq!(
        x86.functions[0].bytes,
        [
            0x48, 0x89, 0xf8, // mov rax, rdi
            0x83, 0xe0, 0x01, // and eax, 1
            0x50, // push rax
            0x48, 0x89, 0xf0, // mov rax, rsi
            0x83, 0xe0, 0x01, // and eax, 1
            0x41, 0x5a, // pop r10
            0x49, 0x39, 0xc2, // cmp r10, rax
            0x0f, 0x94, 0xc0, // sete al
            0x0f, 0xb6, 0xc0, // movzx eax, al
            0xc3,
        ]
    );

    let aarch64 = emit_machine_code(&boolean_equality_plan(
        NativeTarget::linux_arm64(),
        MachineRegister::Aarch64X(0),
        MachineRegister::Aarch64X(1),
    ))
    .unwrap();
    assert_eq!(
        aarch64_instructions(&aarch64.functions[0].bytes),
        [
            0xd100_43ff, // sub sp, sp, #16
            0xf900_03e0, // str x0, [sp]
            0xf900_07e1, // str x1, [sp, #8]
            0xf940_03e0, // ldr x0, [sp]
            0x1200_0000, // and w0, w0, #1
            0xd100_43ff, // sub sp, sp, #16
            0xf900_03e0, // str x0, [sp]
            0xf940_0fe0, // ldr x0, [sp, #24]
            0x1200_0000, // and w0, w0, #1
            0xf940_03e9, // ldr x9, [sp]
            0x9100_43ff, // add sp, sp, #16
            0x6b00_013f, // cmp w9, w0
            0x1a9f_17e0, // cset w0, eq
            0x9100_43ff, // add sp, sp, #16
            0xd65f_03c0, // ret
        ]
    );
}

#[test]
fn retains_ordered_linear_scalar_stack_mutations() {
    let x86 = emit_machine_code(&boolean_equality_plan(
        NativeTarget::linux_x64(),
        MachineRegister::X86Rdi,
        MachineRegister::X86Rsi,
    ))
    .expect("x86 scalar expression");
    let x86_stack = x86.functions[0]
        .scalar_stack
        .as_ref()
        .expect("linear scalar evidence");
    assert_eq!(x86_stack.stack_alignment, 16);
    assert_eq!(x86_stack.mutations.len(), 2);
    assert_eq!(
        x86_stack.mutations[0].kind,
        ScalarStackMutationKind::X86Push
    );
    assert_eq!(x86_stack.mutations[1].kind, ScalarStackMutationKind::X86Pop);

    let aarch64 = emit_machine_code(&boolean_equality_plan(
        NativeTarget::linux_arm64(),
        MachineRegister::Aarch64X(0),
        MachineRegister::Aarch64X(1),
    ))
    .expect("AArch64 scalar expression");
    let aarch64_stack = aarch64.functions[0]
        .scalar_stack
        .as_ref()
        .expect("linear scalar evidence");
    assert_eq!(
        aarch64_stack
            .mutations
            .iter()
            .map(|mutation| mutation.kind)
            .collect::<Vec<_>>(),
        [
            ScalarStackMutationKind::Allocate { byte_size: 16 },
            ScalarStackMutationKind::Allocate { byte_size: 16 },
            ScalarStackMutationKind::Release { byte_size: 16 },
            ScalarStackMutationKind::Release { byte_size: 16 },
        ]
    );
}

#[test]
fn emits_runtime_u8_integer_equality_for_both_architectures() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let x86 = emit_machine_code(&integer_equality_plan(
        NativeTarget::linux_x64(),
        scalar_type,
        MachineRegister::X86Rdi,
        MachineRegister::X86Rsi,
    ))
    .unwrap();
    assert_eq!(
        x86.functions[0].bytes,
        [
            0x48, 0x89, 0xf8, // mov rax, rdi
            0x25, 0xff, 0, 0, 0,    // and eax, 0xff
            0x50, // push rax
            0x48, 0x89, 0xf0, // mov rax, rsi
            0x25, 0xff, 0, 0, 0, // and eax, 0xff
            0x41, 0x5a, // pop r10
            0x49, 0x39, 0xc2, // cmp r10, rax
            0x0f, 0x94, 0xc0, // sete al
            0x0f, 0xb6, 0xc0, // movzx eax, al
            0xc3,
        ]
    );

    let aarch64 = emit_machine_code(&integer_equality_plan(
        NativeTarget::linux_arm64(),
        scalar_type,
        MachineRegister::Aarch64X(0),
        MachineRegister::Aarch64X(1),
    ))
    .unwrap();
    assert_eq!(
        aarch64_instructions(&aarch64.functions[0].bytes),
        [
            0xd100_43ff, // sub sp, sp, #16
            0xf900_03e0, // str x0, [sp]
            0xf900_07e1, // str x1, [sp, #8]
            0xf940_03e0, // ldr x0, [sp]
            0xd340_1c00, // uxtb x0, x0
            0xd100_43ff, // sub sp, sp, #16
            0xf900_03e0, // str x0, [sp]
            0xf940_0fe0, // ldr x0, [sp, #24]
            0xd340_1c00, // uxtb x0, x0
            0xf940_03e9, // ldr x9, [sp]
            0x9100_43ff, // add sp, sp, #16
            0xeb00_013f, // cmp x9, x0
            0x1a9f_17e0, // cset w0, eq
            0x9100_43ff, // add sp, sp, #16
            0xd65f_03c0, // ret
        ]
    );
}

#[test]
fn emits_exact_signed_and_unsigned_integer_ordering_conditions() {
    for (sign, inclusive, x86_setcc, aarch64_cset) in [
        (IntegerSign::Signed, false, 0x9c, 0x1a9f_a7e0),
        (IntegerSign::Unsigned, false, 0x92, 0x1a9f_27e0),
        (IntegerSign::Signed, true, 0x9e, 0x1a9f_c7e0),
        (IntegerSign::Unsigned, true, 0x96, 0x1a9f_87e0),
    ] {
        let scalar_type = IntegerType::new(sign, 8).expect("8-bit ordering type");
        let x86 = emit_machine_code(&integer_ordering_plan(
            NativeTarget::linux_x64(),
            scalar_type,
            inclusive,
            MachineRegister::X86Rdi,
            MachineRegister::X86Rsi,
        ))
        .unwrap();
        assert!(
            x86.functions[0]
                .bytes
                .windows(3)
                .any(|bytes| bytes == [0x0f, x86_setcc, 0xc0]),
            "x86-64 ordering must select the exact signedness-aware condition"
        );

        let aarch64 = emit_machine_code(&integer_ordering_plan(
            NativeTarget::linux_arm64(),
            scalar_type,
            inclusive,
            MachineRegister::Aarch64X(0),
            MachineRegister::Aarch64X(1),
        ))
        .unwrap();
        assert!(
            aarch64_instructions(&aarch64.functions[0].bytes).contains(&aarch64_cset),
            "AArch64 ordering must select the exact signedness-aware condition"
        );
    }
}

#[test]
fn emits_boolean_expression_conditions_for_both_architectures() {
    let x86 = emit_machine_code(&boolean_expression_conditional_plan(
        NativeTarget::linux_x64(),
        MachineRegister::X86Rdi,
        MachineRegister::X86Rsi,
    ))
    .unwrap();
    assert!(matches!(
        x86.functions[0]
            .scalar_stack
            .as_ref()
            .map(|stack| &stack.control_flow),
        Some(ScalarControlFlowEvidence::ConditionalTree {
            decisions,
            crash_leaves,
            ..
        }) if decisions.len() == 1 && crash_leaves == &[false, false]
    ));
    assert!(
        x86.functions[0]
            .bytes
            .windows(8)
            .any(|window| window == [0x0f, 0xb6, 0xc0, 0x85, 0xc0, 0x0f, 0x84, 6])
    );

    let aarch64 = emit_machine_code(&boolean_expression_conditional_plan(
        NativeTarget::linux_arm64(),
        MachineRegister::Aarch64X(0),
        MachineRegister::Aarch64X(1),
    ))
    .unwrap();
    assert!(matches!(
        aarch64.functions[0]
            .scalar_stack
            .as_ref()
            .map(|stack| &stack.control_flow),
        Some(ScalarControlFlowEvidence::ConditionalTree {
            decisions,
            crash_leaves,
            ..
        }) if decisions.len() == 1 && crash_leaves == &[false, false]
    ));
    let instructions = aarch64_instructions(&aarch64.functions[0].bytes);
    assert!(instructions.windows(6).any(|window| window
        == [
            0x1a9f_17e0,
            0x7100_001f,
            0xf940_03e0,
            0xf940_07e1,
            0x9100_43ff,
            0x5400_0060,
        ]));
}

#[test]
fn emits_and_rebases_calls_across_conditional_control() {
    for (target, argument_register) in [
        (NativeTarget::linux_x64(), MachineRegister::X86Rdi),
        (NativeTarget::linux_arm64(), MachineRegister::Aarch64X(0)),
    ] {
        let emitted = emit_machine_code(&calling_conditional_plan(target, argument_register))
            .expect("emit conditional calls");
        let caller = &emitted.functions[0];
        assert!(matches!(
            caller
                .scalar_stack
                .as_ref()
                .map(|stack| &stack.control_flow),
            Some(ScalarControlFlowEvidence::ConditionalTree { .. })
        ));
        assert_eq!(caller.internal_calls.len(), 3);
        assert_eq!(
            caller
                .internal_calls
                .iter()
                .map(|relocation| {
                    relocation
                        .owner
                        .operation()
                        .expect("ordinary scalar call owner")
                        .get()
                })
                .collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert!(
            caller
                .internal_calls
                .windows(2)
                .all(|pair| pair[0].offset < pair[1].offset)
        );
        for relocation in &caller.internal_calls {
            assert_eq!(relocation.target, MachineId::new(2).unwrap());
            assert!(relocation.scalar_stack.is_some());
            match target.architecture {
                Architecture::X86_64 => {
                    assert_eq!(caller.bytes[relocation.offset - 1], 0xe8);
                }
                Architecture::Aarch64 => assert_eq!(
                    &caller.bytes[relocation.offset..relocation.offset + 4],
                    &0x9400_0000_u32.to_le_bytes()
                ),
            }
        }
        match target.architecture {
            Architecture::X86_64 => {
                assert!(
                    caller
                        .bytes
                        .windows(5)
                        .any(|window| window == [0x48, 0x8d, 0x64, 0x24, 32])
                );
            }
            Architecture::Aarch64 => {
                let instructions = aarch64_instructions(&caller.bytes);
                assert!(instructions.contains(&0xf940_03e0)); // restore x0 from outer frame
                assert!(
                    instructions
                        .iter()
                        .any(|instruction| instruction & 0xff00_001f == 0x5400_0000)
                );
                // b.eq false arm
            }
        }
    }
}

#[test]
fn emits_parameter_expression_conditionals_for_both_architectures() {
    let x86 = emit_machine_code(&conditional_plan(NativeTarget::linux_x64())).unwrap();
    assert_eq!(
        x86.functions[0].bytes,
        [
            0x89, 0xf8, // mov eax, edi
            0x85, 0xc0, // test eax, eax
            0x0f, 0x84, 9, 0, 0, 0, // jz false
            0x48, 0x89, 0xf0, // mov rax, rsi
            0x25, 0xff, 0, 0, 0, 0xc3, // mask to u8; ret
            0x48, 0x89, 0xd0, // mov rax, rdx
            0x25, 0xff, 0, 0, 0, 0xc3, // mask to u8; ret
        ]
    );
    let x86_stack = x86.functions[0]
        .scalar_stack
        .as_ref()
        .expect("top-level two-return x86 conditional stack evidence");
    assert_eq!(x86_stack.mutations, []);
    assert_eq!(
        x86_stack.control_flow,
        ScalarControlFlowEvidence::ConditionalTree {
            decisions: vec![ScalarConditionalBranchEvidence {
                condition: ScalarConditionalCondition::Parameter,
                branch_offset: 4,
                branch_byte_count: 6,
                false_arm_offset: 19,
            }],
            crash_leaves: vec![false; 2],
            branches: Vec::new(),
        }
    );
    let aarch64 = emit_machine_code(&conditional_plan(NativeTarget::linux_arm64())).unwrap();
    assert_eq!(
        aarch64_instructions(&aarch64.functions[0].bytes),
        [
            0x3400_00e0, // cbz w0, false
            0xd100_43ff, // sub sp, sp, #16
            0xf900_03e1, // str x1, [sp]
            0xf940_03e0, // ldr x0, [sp]
            0xd340_1c00, // mask to u8
            0x9100_43ff, // add sp, sp, #16
            0xd65f_03c0, // ret
            0xd100_43ff, // sub sp, sp, #16
            0xf900_03e2, // str x2, [sp]
            0xf940_03e0, // ldr x0, [sp]
            0xd340_1c00, // mask to u8
            0x9100_43ff, // add sp, sp, #16
            0xd65f_03c0, // ret
        ]
    );
    let aarch64_stack = aarch64.functions[0]
        .scalar_stack
        .as_ref()
        .expect("top-level two-return AArch64 conditional stack evidence");
    assert_eq!(
        aarch64_stack.control_flow,
        ScalarControlFlowEvidence::ConditionalTree {
            decisions: vec![ScalarConditionalBranchEvidence {
                condition: ScalarConditionalCondition::Parameter,
                branch_offset: 0,
                branch_byte_count: 4,
                false_arm_offset: 28,
            }],
            crash_leaves: vec![false; 2],
            branches: Vec::new(),
        }
    );
    assert_eq!(aarch64_stack.mutations.len(), 4);
}

#[test]
fn retains_arbitrarily_nested_integer_decisions() {
    let nested_plan = |target: NativeTarget, nested_false_arm: bool| {
        let mut plan = conditional_plan(target);
        let nested_register = match (target.architecture, nested_false_arm) {
            (Architecture::X86_64, false) => MachineRegister::X86Rsi,
            (Architecture::X86_64, true) => MachineRegister::X86Rdx,
            (Architecture::Aarch64, false) => MachineRegister::Aarch64X(1),
            (Architecture::Aarch64, true) => MachineRegister::Aarch64X(2),
        };
        let returned = |edge, return_edge, source, value| TargetConditionalIntegerArm {
            psi_edge: EdgeId::new(edge).expect("edge"),
            control: Box::new(TargetIntegerControl::Return {
                psi_return_edge: EdgeId::new(return_edge).expect("return edge"),
                source_value: ValueId::new(source).expect("source value"),
                expression: TargetIntegerExpression::Immediate {
                    source_value: ValueId::new(source).expect("source value"),
                    value: IntegerValue::Unsigned(value),
                },
            }),
        };
        let TargetOperation::ReturnIntegerConditionalControl {
            when_true,
            when_false,
            ..
        } = &mut plan.functions[0].operation
        else {
            unreachable!()
        };
        let nested_arm = if nested_false_arm {
            when_false
        } else {
            when_true
        };
        nested_arm.control = Box::new(TargetIntegerControl::Conditional {
            condition_source: ValueId::new(8).expect("nested condition"),
            condition_parameter_index: if nested_false_arm { 2 } else { 1 },
            condition_location: ScalarParameterLocation::Register(nested_register),
            when_true: returned(5, 7, 9, 7),
            when_false: returned(6, 8, 10, 9),
        });
        plan
    };

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        for nested_false_arm in [false, true] {
            let emitted = emit_machine_code(&nested_plan(target, nested_false_arm))
                .expect("emit one nested integer decision");
            let ScalarControlFlowEvidence::ConditionalTree {
                decisions,
                crash_leaves,
                branches,
            } = &emitted.functions[0]
                .scalar_stack
                .as_ref()
                .expect("nested conditional stack evidence")
                .control_flow
            else {
                panic!("one nested decision must retain three-leaf evidence")
            };
            assert!(branches.is_empty());
            assert_eq!(crash_leaves, &[false; 3]);
            let [root, nested] = decisions.as_slice() else {
                panic!("two decisions are retained")
            };
            assert!(root.branch_offset < nested.branch_offset);
            assert_eq!(
                nested.branch_offset >= root.false_arm_offset,
                nested_false_arm
            );
        }

        let mut four_leaf = nested_plan(target, false);
        let false_register = match target.architecture {
            Architecture::X86_64 => MachineRegister::X86Rdx,
            Architecture::Aarch64 => MachineRegister::Aarch64X(2),
        };
        let returned = |edge, return_edge, source, value| TargetConditionalIntegerArm {
            psi_edge: EdgeId::new(edge).expect("edge"),
            control: Box::new(TargetIntegerControl::Return {
                psi_return_edge: EdgeId::new(return_edge).expect("return edge"),
                source_value: ValueId::new(source).expect("source value"),
                expression: TargetIntegerExpression::Immediate {
                    source_value: ValueId::new(source).expect("source value"),
                    value: IntegerValue::Unsigned(value),
                },
            }),
        };
        let TargetOperation::ReturnIntegerConditionalControl { when_false, .. } =
            &mut four_leaf.functions[0].operation
        else {
            unreachable!()
        };
        when_false.control = Box::new(TargetIntegerControl::Conditional {
            condition_source: ValueId::new(12).expect("false nested condition"),
            condition_parameter_index: 2,
            condition_location: ScalarParameterLocation::Register(false_register),
            when_true: returned(11, 13, 13, 11),
            when_false: returned(12, 14, 14, 13),
        });
        let emitted =
            emit_machine_code(&four_leaf).expect("emit one nested decision in each outer arm");
        let ScalarControlFlowEvidence::ConditionalTree {
            decisions,
            crash_leaves,
            branches,
        } = &emitted.functions[0]
            .scalar_stack
            .as_ref()
            .expect("four-leaf conditional stack evidence")
            .control_flow
        else {
            panic!("two nested decisions must retain four-leaf evidence")
        };
        assert!(branches.is_empty());
        assert_eq!(crash_leaves, &[false; 4]);
        let [root, true_nested, false_nested] = decisions.as_slice() else {
            panic!("three decisions are retained")
        };
        assert!(root.branch_offset < true_nested.branch_offset);
        assert!(true_nested.false_arm_offset < root.false_arm_offset);
        assert!(root.false_arm_offset <= false_nested.branch_offset);

        let TargetOperation::ReturnIntegerConditionalControl { when_false, .. } =
            &mut four_leaf.functions[0].operation
        else {
            unreachable!()
        };
        let TargetIntegerControl::Conditional { when_true, .. } = when_false.control.as_mut()
        else {
            unreachable!()
        };
        when_true.control = Box::new(TargetIntegerControl::Crash {
            psi_crash_edge: EdgeId::new(16).expect("crash edge"),
            cause: psi_terminal::CrashCause::Trap,
            site_guard: Vec::new(),
            frontier_lower_bound: Vec::new(),
        });
        let emitted =
            emit_machine_code(&four_leaf).expect("emit four-leaf conditional with a crash leaf");
        assert!(matches!(
            &emitted.functions[0]
                .scalar_stack
                .as_ref()
                .expect("four-leaf crash stack evidence")
                .control_flow,
            ScalarControlFlowEvidence::ConditionalTree { crash_leaves, .. }
                if crash_leaves == &[false, false, true, false]
        ));

        let mut nested_crash = nested_plan(target, false);
        let TargetOperation::ReturnIntegerConditionalControl { when_true, .. } =
            &mut nested_crash.functions[0].operation
        else {
            unreachable!()
        };
        let TargetIntegerControl::Conditional { when_false, .. } = when_true.control.as_mut()
        else {
            unreachable!()
        };
        when_false.control = Box::new(TargetIntegerControl::Crash {
            psi_crash_edge: EdgeId::new(15).expect("crash edge"),
            cause: psi_terminal::CrashCause::Trap,
            site_guard: Vec::new(),
            frontier_lower_bound: Vec::new(),
        });
        let emitted =
            emit_machine_code(&nested_crash).expect("emit nested conditional with a crash leaf");
        assert!(matches!(
            &emitted.functions[0]
                .scalar_stack
                .as_ref()
                .expect("nested crash stack evidence")
                .control_flow,
            ScalarControlFlowEvidence::ConditionalTree { crash_leaves, .. }
                if crash_leaves == &[false, true, false]
        ));
    }

    let mut too_deep = nested_plan(NativeTarget::linux_x64(), false);
    let TargetOperation::ReturnIntegerConditionalControl { when_true, .. } =
        &mut too_deep.functions[0].operation
    else {
        unreachable!()
    };
    let TargetIntegerControl::Conditional {
        when_true: nested_true,
        ..
    } = when_true.control.as_mut()
    else {
        unreachable!()
    };
    let leaf = nested_true.control.clone();
    nested_true.control = Box::new(TargetIntegerControl::Conditional {
        condition_source: ValueId::new(11).expect("third condition"),
        condition_parameter_index: 1,
        condition_location: ScalarParameterLocation::Register(MachineRegister::X86Rsi),
        when_true: TargetConditionalIntegerArm {
            psi_edge: EdgeId::new(9).expect("edge"),
            control: leaf.clone(),
        },
        when_false: TargetConditionalIntegerArm {
            psi_edge: EdgeId::new(10).expect("edge"),
            control: leaf,
        },
    });
    let emitted = emit_machine_code(&too_deep).expect("third decision emits with evidence");
    let ScalarControlFlowEvidence::ConditionalTree {
        decisions,
        crash_leaves,
        branches,
    } = &emitted.functions[0]
        .scalar_stack
        .as_ref()
        .expect("third decision retains generic tree evidence")
        .control_flow
    else {
        panic!("third decision must retain conditional-tree evidence")
    };
    assert_eq!(decisions.len(), 3);
    assert_eq!(crash_leaves, &[false; 4]);
    assert!(branches.is_empty());
}

#[test]
fn conditional_division_and_crash_admit_accountable_arm_forms() {
    let mut division_arm = conditional_plan(NativeTarget::linux_x64());
    let TargetOperation::ReturnIntegerConditionalControl { when_true, .. } =
        &mut division_arm.functions[0].operation
    else {
        unreachable!()
    };
    let TargetIntegerControl::Return { expression, .. } = when_true.control.as_mut() else {
        unreachable!()
    };
    *expression = TargetIntegerExpression::WrappingDivide {
        psi_operation: OperationId::new(8).expect("divide operation"),
        obligation: proof_obligation(),
        left: Box::new(TargetIntegerExpression::Immediate {
            source_value: ValueId::new(8).expect("left"),
            value: IntegerValue::Unsigned(8),
        }),
        right: Box::new(TargetIntegerExpression::Immediate {
            source_value: ValueId::new(9).expect("right"),
            value: IntegerValue::Unsigned(2),
        }),
    };
    let emitted = emit_machine_code(&division_arm).expect("conditional division emits");
    assert!(matches!(
        emitted.functions[0]
            .scalar_stack
            .as_ref()
            .expect("branch-free x86 division arm has outer conditional evidence")
            .control_flow,
        ScalarControlFlowEvidence::ConditionalTree { .. }
    ));

    let signed_division_plan = |target: NativeTarget| {
        let mut plan = conditional_plan(target);
        let locations = match target.architecture {
            Architecture::X86_64 => (MachineRegister::X86Rsi, MachineRegister::X86Rdx),
            Architecture::Aarch64 => (MachineRegister::Aarch64X(1), MachineRegister::Aarch64X(2)),
        };
        let TargetOperation::ReturnIntegerConditionalControl {
            scalar_type,
            when_true,
            ..
        } = &mut plan.functions[0].operation
        else {
            unreachable!()
        };
        *scalar_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
        let TargetIntegerControl::Return { expression, .. } = when_true.control.as_mut() else {
            unreachable!()
        };
        *expression = TargetIntegerExpression::WrappingDivide {
            psi_operation: OperationId::new(8).expect("divide operation"),
            obligation: proof_obligation(),
            left: Box::new(TargetIntegerExpression::Parameter {
                source_value: ValueId::new(8).expect("left"),
                parameter_index: 1,
                location: ScalarParameterLocation::Register(locations.0),
            }),
            right: Box::new(TargetIntegerExpression::Parameter {
                source_value: ValueId::new(9).expect("right"),
                parameter_index: 2,
                location: ScalarParameterLocation::Register(locations.1),
            }),
        };
        plan
    };
    let signed_x86 = emit_machine_code(&signed_division_plan(NativeTarget::linux_x64()))
        .expect("signed x86 conditional division emits");
    let ScalarControlFlowEvidence::ConditionalTree { branches, .. } = &signed_x86.functions[0]
        .scalar_stack
        .as_ref()
        .expect("signed x86 conditional division stack evidence")
        .control_flow
    else {
        panic!("signed x86 conditional division must retain composite evidence")
    };
    assert_eq!(branches.len(), 1);
    assert!(matches!(
        emit_machine_code(&signed_division_plan(NativeTarget::linux_arm64()))
            .expect("signed AArch64 conditional division emits")
            .functions[0]
            .scalar_stack
            .as_ref()
            .expect("branch-free AArch64 signed division is retained")
            .control_flow,
        ScalarControlFlowEvidence::ConditionalTree { .. }
    ));

    let mut signed_return_crash = signed_division_plan(NativeTarget::linux_x64());
    let TargetOperation::ReturnIntegerConditionalControl { when_false, .. } =
        &mut signed_return_crash.functions[0].operation
    else {
        unreachable!()
    };
    when_false.control = Box::new(TargetIntegerControl::Crash {
        psi_crash_edge: EdgeId::new(10).expect("crash edge"),
        cause: psi_terminal::CrashCause::Trap,
        site_guard: Vec::new(),
        frontier_lower_bound: Vec::new(),
    });
    let emitted = emit_machine_code(&signed_return_crash)
        .expect("signed x86 division plus crash emits with stack evidence");
    let ScalarControlFlowEvidence::ConditionalTree {
        crash_leaves,
        branches,
        ..
    } = &emitted.functions[0]
        .scalar_stack
        .as_ref()
        .expect("signed x86 return/crash division stack evidence")
        .control_flow
    else {
        panic!("signed x86 return/crash division must retain composite evidence")
    };
    assert_eq!(crash_leaves, &[false, true]);
    assert_eq!(branches.len(), 1);

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        for crash_false_arm in [false, true] {
            let mut crash_plan = conditional_plan(target);
            let TargetOperation::ReturnIntegerConditionalControl {
                when_true,
                when_false,
                ..
            } = &mut crash_plan.functions[0].operation
            else {
                unreachable!()
            };
            let crash_arm = if crash_false_arm {
                when_false
            } else {
                when_true
            };
            crash_arm.control = Box::new(TargetIntegerControl::Crash {
                psi_crash_edge: EdgeId::new(9).expect("crash edge"),
                cause: psi_terminal::CrashCause::Trap,
                site_guard: Vec::new(),
                frontier_lower_bound: Vec::new(),
            });
            let emitted = emit_machine_code(&crash_plan)
                .expect("conditional return/crash emits with stack evidence");
            let ScalarControlFlowEvidence::ConditionalTree { crash_leaves, .. } = &emitted
                .functions[0]
                .scalar_stack
                .as_ref()
                .expect("conditional return/crash stack evidence")
                .control_flow
            else {
                panic!("conditional return/crash must retain terminal evidence")
            };
            assert_eq!(
                crash_leaves,
                if crash_false_arm {
                    &[false, true]
                } else {
                    &[true, false]
                }
            );
        }

        let mut two_crash_plan = conditional_plan(target);
        let TargetOperation::ReturnIntegerConditionalControl {
            when_true,
            when_false,
            ..
        } = &mut two_crash_plan.functions[0].operation
        else {
            unreachable!()
        };
        for (edge, arm) in [(11, when_true), (12, when_false)] {
            arm.control = Box::new(TargetIntegerControl::Crash {
                psi_crash_edge: EdgeId::new(edge).expect("crash edge"),
                cause: psi_terminal::CrashCause::Trap,
                site_guard: Vec::new(),
                frontier_lower_bound: Vec::new(),
            });
        }
        let emitted = emit_machine_code(&two_crash_plan)
            .expect("two-crash conditional emits with stack evidence");
        assert!(matches!(
            &emitted.functions[0]
                .scalar_stack
                .as_ref()
                .expect("two-crash conditional stack evidence")
                .control_flow,
            ScalarControlFlowEvidence::ConditionalTree { crash_leaves, .. }
                if crash_leaves == &[true, true]
        ));
    }
}

#[test]
fn branch_free_division_and_remainder_are_retained_in_either_conditional_arm() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        for kind in 0..6 {
            for false_arm in [false, true] {
                let mut plan = conditional_plan(target);
                let TargetOperation::ReturnIntegerConditionalControl {
                    when_true,
                    when_false,
                    ..
                } = &mut plan.functions[0].operation
                else {
                    unreachable!()
                };
                let arm = if false_arm { when_false } else { when_true };
                let TargetIntegerControl::Return { expression, .. } = arm.control.as_mut() else {
                    unreachable!()
                };
                let operation = OperationId::new(20 + kind).expect("division operation");
                let left = || {
                    Box::new(TargetIntegerExpression::Immediate {
                        source_value: ValueId::new(20).expect("left"),
                        value: IntegerValue::Unsigned(8),
                    })
                };
                let right = || {
                    Box::new(TargetIntegerExpression::Immediate {
                        source_value: ValueId::new(21).expect("right"),
                        value: IntegerValue::Unsigned(2),
                    })
                };
                *expression = match kind {
                    0 => TargetIntegerExpression::ExactDivide {
                        psi_operation: operation,
                        obligation: proof_obligation(),
                        left: left(),
                        right: right(),
                    },
                    1 => TargetIntegerExpression::ExactRemainder {
                        psi_operation: operation,
                        obligation: proof_obligation(),
                        left: left(),
                        right: right(),
                    },
                    2 => TargetIntegerExpression::WrappingDivide {
                        psi_operation: operation,
                        obligation: proof_obligation(),
                        left: left(),
                        right: right(),
                    },
                    3 => TargetIntegerExpression::WrappingRemainder {
                        psi_operation: operation,
                        obligation: proof_obligation(),
                        left: left(),
                        right: right(),
                    },
                    4 => TargetIntegerExpression::SaturatingDivide {
                        psi_operation: operation,
                        obligation: proof_obligation(),
                        left: left(),
                        right: right(),
                    },
                    5 => TargetIntegerExpression::SaturatingRemainder {
                        psi_operation: operation,
                        obligation: proof_obligation(),
                        left: left(),
                        right: right(),
                    },
                    _ => unreachable!(),
                };
                let emitted = emit_machine_code(&plan).unwrap_or_else(|error| {
                    panic!("{target:?} kind {kind} false_arm={false_arm}: {error:?}")
                });
                assert!(matches!(
                    emitted.functions[0]
                        .scalar_stack
                        .as_ref()
                        .expect("branch-free conditional division stack evidence")
                        .control_flow,
                    ScalarControlFlowEvidence::ConditionalTree { .. }
                ));
            }
        }
    }
}

#[test]
fn branch_free_division_and_remainder_are_retained_in_expression_conditions() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let argument_register = match target.architecture {
            Architecture::X86_64 => MachineRegister::X86Rdi,
            Architecture::Aarch64 => MachineRegister::Aarch64X(0),
        };
        for kind in 0..6 {
            let mut plan = calling_expression_condition_plan(target, argument_register);
            let TargetOperation::ReturnIntegerExpressionConditionalControl { condition, .. } =
                &mut plan.functions[0].operation
            else {
                unreachable!()
            };
            let operation = OperationId::new(20 + kind).expect("division operation");
            let left = || {
                Box::new(TargetIntegerExpression::Immediate {
                    source_value: ValueId::new(20).expect("left"),
                    value: IntegerValue::Unsigned(24),
                })
            };
            let right = || {
                Box::new(TargetIntegerExpression::Immediate {
                    source_value: ValueId::new(21).expect("right"),
                    value: IntegerValue::Unsigned(3),
                })
            };
            let quotient = match kind {
                0 => TargetIntegerExpression::ExactDivide {
                    psi_operation: operation,
                    obligation: proof_obligation(),
                    left: left(),
                    right: right(),
                },
                1 => TargetIntegerExpression::ExactRemainder {
                    psi_operation: operation,
                    obligation: proof_obligation(),
                    left: left(),
                    right: right(),
                },
                2 => TargetIntegerExpression::WrappingDivide {
                    psi_operation: operation,
                    obligation: proof_obligation(),
                    left: left(),
                    right: right(),
                },
                3 => TargetIntegerExpression::WrappingRemainder {
                    psi_operation: operation,
                    obligation: proof_obligation(),
                    left: left(),
                    right: right(),
                },
                4 => TargetIntegerExpression::SaturatingDivide {
                    psi_operation: operation,
                    obligation: proof_obligation(),
                    left: left(),
                    right: right(),
                },
                5 => TargetIntegerExpression::SaturatingRemainder {
                    psi_operation: operation,
                    obligation: proof_obligation(),
                    left: left(),
                    right: right(),
                },
                _ => unreachable!(),
            };
            *condition = TargetBooleanExpression::IntegerEqual {
                psi_operation: OperationId::new(30 + kind).expect("comparison operation"),
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"),
                left: Box::new(quotient),
                right: Box::new(TargetIntegerExpression::Immediate {
                    source_value: ValueId::new(22).expect("expected quotient"),
                    value: IntegerValue::Unsigned(if kind % 2 == 0 { 8 } else { 0 }),
                }),
            };
            let emitted = emit_machine_code(&plan)
                .unwrap_or_else(|error| panic!("{target:?} condition kind {kind}: {error:?}"));
            assert!(matches!(
                emitted.functions[0]
                    .scalar_stack
                    .as_ref()
                    .expect("branch-free condition division stack evidence")
                    .control_flow,
                ScalarControlFlowEvidence::ConditionalTree { ref decisions, .. }
                    if decisions[0].condition
                        == ScalarConditionalCondition::Expression
            ));
        }
    }
}

#[test]
fn expression_condition_division_retains_x86_policy_diamonds() {
    let signed_plan = |target: NativeTarget, argument_register: MachineRegister| {
        let mut plan = calling_expression_condition_plan(target, argument_register);
        let TargetOperation::ReturnIntegerExpressionConditionalControl { condition, .. } =
            &mut plan.functions[0].operation
        else {
            unreachable!()
        };
        let scalar_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
        *condition = TargetBooleanExpression::IntegerEqual {
            psi_operation: OperationId::new(31).expect("comparison operation"),
            scalar_type,
            left: Box::new(TargetIntegerExpression::WrappingDivide {
                psi_operation: OperationId::new(30).expect("division operation"),
                obligation: proof_obligation(),
                left: Box::new(TargetIntegerExpression::Immediate {
                    source_value: ValueId::new(30).expect("left"),
                    value: IntegerValue::Signed(i64::MIN.into()),
                }),
                right: Box::new(TargetIntegerExpression::Immediate {
                    source_value: ValueId::new(31).expect("right"),
                    value: IntegerValue::Signed((-1_i64).into()),
                }),
            }),
            right: Box::new(TargetIntegerExpression::Immediate {
                source_value: ValueId::new(32).expect("expected quotient"),
                value: IntegerValue::Signed(i64::MIN.into()),
            }),
        };
        plan
    };
    let signed_x86 = emit_machine_code(&signed_plan(
        NativeTarget::linux_x64(),
        MachineRegister::X86Rdi,
    ))
    .expect("signed x86 condition division emits");
    let ScalarControlFlowEvidence::ConditionalTree { branches, .. } = &signed_x86.functions[0]
        .scalar_stack
        .as_ref()
        .expect("signed x86 condition division stack evidence")
        .control_flow
    else {
        panic!("signed x86 condition division must retain composite evidence")
    };
    assert_eq!(branches.len(), 1);
    assert!(matches!(
        emit_machine_code(&signed_plan(
            NativeTarget::linux_arm64(),
            MachineRegister::Aarch64X(0),
        ))
        .expect("signed AArch64 condition division emits")
        .functions[0]
            .scalar_stack
            .as_ref()
            .expect("branch-free signed AArch64 condition division is retained")
            .control_flow,
        ScalarControlFlowEvidence::ConditionalTree { .. }
    ));
}

#[test]
fn parameter_conditional_retains_typed_calls_inside_direct_linear_arms() {
    for (target, condition_register, argument_register) in [
        (
            NativeTarget::linux_x64(),
            MachineRegister::X86Rdi,
            MachineRegister::X86Rsi,
        ),
        (
            NativeTarget::linux_arm64(),
            MachineRegister::Aarch64X(0),
            MachineRegister::Aarch64X(1),
        ),
    ] {
        let emitted = emit_machine_code(&calling_arm_conditional_plan(
            target,
            condition_register,
            argument_register,
        ))
        .expect("emit conditional arm call");
        let caller = &emitted.functions[0];
        assert!(matches!(
            caller
                .scalar_stack
                .as_ref()
                .expect("conditional call stack evidence")
                .control_flow,
            ScalarControlFlowEvidence::ConditionalTree { .. }
        ));
        assert_eq!(caller.internal_calls.len(), 1);
        assert!(caller.internal_calls[0].scalar_stack.is_some());
        assert_eq!(caller.internal_calls[0].target, MachineId::new(2).unwrap());
    }

    let mut division_argument = calling_arm_conditional_plan(
        NativeTarget::linux_x64(),
        MachineRegister::X86Rdi,
        MachineRegister::X86Rsi,
    );
    let TargetOperation::ReturnIntegerConditionalControl { when_true, .. } =
        &mut division_argument.functions[0].operation
    else {
        unreachable!()
    };
    let TargetIntegerControl::Return { expression, .. } = when_true.control.as_mut() else {
        unreachable!()
    };
    let TargetIntegerExpression::WrappingAdd { right, .. } = expression else {
        unreachable!()
    };
    let TargetIntegerExpression::Call { arguments, .. } = right.as_mut() else {
        unreachable!()
    };
    let TargetScalarExpression::Integer { expression, .. } = &mut arguments[0].expression else {
        unreachable!()
    };
    *expression = TargetIntegerExpression::WrappingDivide {
        psi_operation: OperationId::new(9).unwrap(),
        obligation: proof_obligation(),
        left: Box::new(TargetIntegerExpression::Immediate {
            source_value: ValueId::new(8).unwrap(),
            value: IntegerValue::Unsigned(8),
        }),
        right: Box::new(TargetIntegerExpression::Immediate {
            source_value: ValueId::new(9).unwrap(),
            value: IntegerValue::Unsigned(2),
        }),
    };
    let emitted = emit_machine_code(&division_argument)
        .expect("branch-free conditional call argument emits with stack evidence");
    assert!(matches!(
        emitted.functions[0]
            .scalar_stack
            .as_ref()
            .expect("conditional call-argument division stack evidence")
            .control_flow,
        ScalarControlFlowEvidence::ConditionalTree { .. }
    ));
    assert!(
        emitted.functions[0].internal_calls[0]
            .scalar_stack
            .is_some()
    );
}

#[test]
fn expression_conditional_retains_typed_call_in_linear_condition_prefix() {
    for (target, argument_register) in [
        (NativeTarget::linux_x64(), MachineRegister::X86Rdi),
        (NativeTarget::linux_arm64(), MachineRegister::Aarch64X(0)),
    ] {
        let emitted = emit_machine_code(&calling_expression_condition_plan(
            target,
            argument_register,
        ))
        .expect("emit expression-condition call");
        let caller = &emitted.functions[0];
        assert!(matches!(
            caller
                .scalar_stack
                .as_ref()
                .expect("expression-condition call stack evidence")
                .control_flow,
            ScalarControlFlowEvidence::ConditionalTree { ref decisions, .. }
                if decisions[0].condition
                    == ScalarConditionalCondition::Expression
        ));
        assert_eq!(caller.internal_calls.len(), 1);
        assert!(caller.internal_calls[0].scalar_stack.is_some());
        if target.architecture == Architecture::X86_64 {
            assert!(caller.scalar_stack.as_ref().unwrap().mutations.iter().any(
                |mutation| matches!(
                    mutation.kind,
                    ScalarStackMutationKind::X86ReleasePreservingFlags { .. }
                )
            ));
        }
    }
}

#[test]
fn expression_conditional_retains_division_in_typed_condition_call_argument() {
    let division_argument_plan =
        |target: NativeTarget, argument_register: MachineRegister, signed: bool| {
            let mut plan = calling_expression_condition_plan(target, argument_register);
            let TargetOperation::ReturnIntegerExpressionConditionalControl { condition, .. } =
                &mut plan.functions[0].operation
            else {
                unreachable!()
            };
            let TargetBooleanExpression::Call { arguments, .. } = condition else {
                unreachable!()
            };
            let TargetScalarExpression::Boolean(expression) = &mut arguments[0].expression else {
                unreachable!()
            };
            let scalar_type = IntegerType::new(
                if signed {
                    IntegerSign::Signed
                } else {
                    IntegerSign::Unsigned
                },
                64,
            )
            .expect("64-bit integer");
            let (left, right, result) = if signed {
                (
                    IntegerValue::Signed(i64::MIN.into()),
                    IntegerValue::Signed((-1_i64).into()),
                    IntegerValue::Signed(i64::MIN.into()),
                )
            } else {
                (
                    IntegerValue::Unsigned(24),
                    IntegerValue::Unsigned(3),
                    IntegerValue::Unsigned(8),
                )
            };
            *expression = TargetBooleanExpression::IntegerEqual {
                psi_operation: OperationId::new(12).expect("comparison operation"),
                scalar_type,
                left: Box::new(TargetIntegerExpression::WrappingDivide {
                    psi_operation: OperationId::new(11).expect("division operation"),
                    obligation: proof_obligation(),
                    left: Box::new(TargetIntegerExpression::Immediate {
                        source_value: ValueId::new(11).expect("left"),
                        value: left,
                    }),
                    right: Box::new(TargetIntegerExpression::Immediate {
                        source_value: ValueId::new(12).expect("right"),
                        value: right,
                    }),
                }),
                right: Box::new(TargetIntegerExpression::Immediate {
                    source_value: ValueId::new(13).expect("result"),
                    value: result,
                }),
            };
            plan
        };

    for (target, argument_register) in [
        (NativeTarget::linux_x64(), MachineRegister::X86Rdi),
        (NativeTarget::linux_arm64(), MachineRegister::Aarch64X(0)),
    ] {
        let emitted = emit_machine_code(&division_argument_plan(target, argument_register, false))
            .expect("emit branch-free condition call-argument division");
        assert!(matches!(
            emitted.functions[0]
                .scalar_stack
                .as_ref()
                .expect("condition call-argument division stack evidence")
                .control_flow,
            ScalarControlFlowEvidence::ConditionalTree { .. }
        ));
        assert!(
            emitted.functions[0].internal_calls[0]
                .scalar_stack
                .is_some()
        );
    }

    let signed_x86 = emit_machine_code(&division_argument_plan(
        NativeTarget::linux_x64(),
        MachineRegister::X86Rdi,
        true,
    ))
    .expect("signed x86 condition call-argument division emits");
    let ScalarControlFlowEvidence::ConditionalTree { branches, .. } = &signed_x86.functions[0]
        .scalar_stack
        .as_ref()
        .expect("signed x86 condition call-argument stack evidence")
        .control_flow
    else {
        panic!("signed x86 condition call argument must retain composite evidence")
    };
    assert_eq!(branches.len(), 1);
    assert!(
        emit_machine_code(&division_argument_plan(
            NativeTarget::linux_arm64(),
            MachineRegister::Aarch64X(0),
            true,
        ))
        .expect("signed AArch64 condition call-argument division emits")
        .functions[0]
            .scalar_stack
            .is_some()
    );
}

#[test]
fn emits_selected_register_parameter_returns_for_all_native_policies() {
    assert_eq!(
        emit_machine_code(&parameter_plan(
            NativeTarget::linux_x64(),
            ScalarParameterLocation::Register(MachineRegister::X86Rdi),
            false,
        ))
        .unwrap()
        .functions[0]
            .bytes,
        [0x89, 0xf8, 0xc3]
    );
    assert_eq!(
        emit_machine_code(&parameter_plan(
            NativeTarget::windows_x64(),
            ScalarParameterLocation::Register(MachineRegister::X86Rcx),
            false,
        ))
        .unwrap()
        .functions[0]
            .bytes,
        [0x89, 0xc8, 0xc3]
    );
    assert_eq!(
        emit_machine_code(&parameter_plan(
            NativeTarget::linux_arm64(),
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
            false,
        ))
        .unwrap()
        .functions[0]
            .bytes,
        [0xc0, 0x03, 0x5f, 0xd6]
    );
    assert_eq!(
        emit_machine_code(&parameter_plan(
            NativeTarget::linux_arm64(),
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
            false,
        ))
        .unwrap()
        .functions[0]
            .bytes,
        [0xe0, 0x03, 0x01, 0x2a, 0xc0, 0x03, 0x5f, 0xd6]
    );
    assert_eq!(
        emit_machine_code(&parameter_plan(
            NativeTarget::linux_x64(),
            ScalarParameterLocation::Register(MachineRegister::X86R9),
            true,
        ))
        .unwrap()
        .functions[0]
            .bytes,
        [0x4c, 0x89, 0xc8, 0xc3]
    );
    assert_eq!(
        emit_machine_code(&parameter_plan(
            NativeTarget::linux_arm64(),
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(3)),
            true,
        ))
        .unwrap()
        .functions[0]
            .bytes,
        [0xe0, 0x03, 0x03, 0xaa, 0xc0, 0x03, 0x5f, 0xd6]
    );
}

#[test]
fn emits_selected_incoming_stack_parameter_returns_for_both_architectures() {
    assert_eq!(
        emit_machine_code(&parameter_plan(
            NativeTarget::linux_x64(),
            ScalarParameterLocation::IncomingStack { byte_offset: 16 },
            false,
        ))
        .unwrap()
        .functions[0]
            .bytes,
        [0x8b, 0x44, 0x24, 24, 0xc3]
    );
    assert_eq!(
        emit_machine_code(&parameter_plan(
            NativeTarget::linux_arm64(),
            ScalarParameterLocation::IncomingStack { byte_offset: 0 },
            false,
        ))
        .unwrap()
        .functions[0]
            .bytes,
        [0xe0, 0x03, 0x40, 0xb9, 0xc0, 0x03, 0x5f, 0xd6]
    );
    assert_eq!(
        emit_machine_code(&parameter_plan(
            NativeTarget::linux_x64(),
            ScalarParameterLocation::IncomingStack { byte_offset: 16 },
            true,
        ))
        .unwrap()
        .functions[0]
            .bytes,
        [0x48, 0x8b, 0x44, 0x24, 24, 0xc3]
    );
    assert_eq!(
        emit_machine_code(&parameter_plan(
            NativeTarget::linux_arm64(),
            ScalarParameterLocation::IncomingStack { byte_offset: 0 },
            true,
        ))
        .unwrap()
        .functions[0]
            .bytes,
        [0xe0, 0x03, 0x40, 0xf9, 0xc0, 0x03, 0x5f, 0xd6]
    );
}

#[test]
fn emits_a_canonical_boolean_parameter_return() {
    let mut plan = parameter_plan(
        NativeTarget::linux_x64(),
        ScalarParameterLocation::Register(MachineRegister::X86Rdi),
        false,
    );
    plan.functions[0].operation = TargetOperation::ReturnBooleanParameter {
        psi_edge: EdgeId::new(1).expect("edge"),
        source_value: ValueId::new(1).expect("value"),
        parameter_index: 0,
        location: ScalarParameterLocation::Register(MachineRegister::X86Rdi),
    };
    assert_eq!(
        emit_machine_code(&plan).unwrap().functions[0].bytes,
        [0x89, 0xf8, 0xc3]
    );
}

#[test]
fn emits_boolean_not_parameter_returns_for_both_architectures() {
    let mut x86 = parameter_plan(
        NativeTarget::linux_x64(),
        ScalarParameterLocation::Register(MachineRegister::X86Rdi),
        false,
    );
    x86.functions[0].operation = TargetOperation::ReturnBooleanNotParameter {
        psi_edge: EdgeId::new(1).expect("edge"),
        source_value: ValueId::new(1).expect("value"),
        parameter_index: 0,
        location: ScalarParameterLocation::Register(MachineRegister::X86Rdi),
    };
    assert_eq!(
        emit_machine_code(&x86).unwrap().functions[0].bytes,
        [0x89, 0xf8, 0x83, 0xf0, 0x01, 0xc3]
    );

    let mut aarch64 = parameter_plan(
        NativeTarget::linux_arm64(),
        ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
        false,
    );
    aarch64.functions[0].operation = TargetOperation::ReturnBooleanNotParameter {
        psi_edge: EdgeId::new(1).expect("edge"),
        source_value: ValueId::new(1).expect("value"),
        parameter_index: 0,
        location: ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
    };
    assert_eq!(
        aarch64_instructions(&emit_machine_code(&aarch64).unwrap().functions[0].bytes),
        [0x5200_0000, 0xd65f_03c0]
    );
}

#[test]
fn emits_parameter_fed_wrapping_add_for_both_architectures() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let x86 = emit_machine_code(&expression_plan(
        NativeTarget::linux_x64(),
        scalar_type,
        wrapping_expression(
            ScalarParameterLocation::Register(MachineRegister::X86Rdi),
            ScalarParameterLocation::Register(MachineRegister::X86Rsi),
        ),
    ))
    .unwrap();
    assert_eq!(
        x86.functions[0].bytes,
        [
            0x48, 0x89, 0xf8, 0x25, 0xff, 0, 0, 0, 0x50, 0x48, 0x89, 0xf0, 0x25, 0xff, 0, 0, 0,
            0x41, 0x5a, 0x4c, 0x01, 0xd0, 0x25, 0xff, 0, 0, 0, 0xc3,
        ]
    );

    let aarch64 = emit_machine_code(&expression_plan(
        NativeTarget::linux_arm64(),
        scalar_type,
        wrapping_expression(
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
        ),
    ))
    .unwrap();
    assert_eq!(
        aarch64_instructions(&aarch64.functions[0].bytes),
        [
            0xd100_43ff,
            0xf900_03e0,
            0xf900_07e1,
            0xf940_03e0,
            0xd340_1c00,
            0xd100_43ff,
            0xf900_03e0,
            0xf940_0fe0,
            0xd340_1c00,
            0xf940_03e9,
            0x9100_43ff,
            0x8b00_0120,
            0xd340_1c00,
            0x9100_43ff,
            0xd65f_03c0,
        ]
    );
}

#[test]
fn emits_exact_parameter_fed_bitwise_instructions_for_both_architectures() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    for (kind, x86_opcode, aarch64_opcode) in [
        (0_u8, 0x21_u8, 0x8a00_0120_u32),
        (1, 0x09, 0xaa00_0120),
        (2, 0x31, 0xca00_0120),
    ] {
        let x86 = emit_machine_code(&expression_plan(
            NativeTarget::linux_x64(),
            scalar_type,
            bitwise_expression(
                kind,
                ScalarParameterLocation::Register(MachineRegister::X86Rdi),
                ScalarParameterLocation::Register(MachineRegister::X86Rsi),
            ),
        ))
        .expect("x86-64 bitwise expression emits");
        let bytes = &x86.functions[0].bytes;
        assert!(
            bytes
                .windows(3)
                .any(|window| window == [0x4c, x86_opcode, 0xd0])
        );

        let aarch64 = emit_machine_code(&expression_plan(
            NativeTarget::linux_arm64(),
            scalar_type,
            bitwise_expression(
                kind,
                ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                ScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
            ),
        ))
        .expect("AArch64 bitwise expression emits");
        assert!(
            aarch64_instructions(&aarch64.functions[0].bytes).contains(&aarch64_opcode),
            "bitwise kind {kind} must retain its exact AArch64 instruction"
        );
    }
}

#[test]
fn emits_modulo_count_wrapping_shifts_for_both_architectures() {
    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    for (left_shift, value_type, x86_opcode, aarch64_opcode) in [
        (true, u64_type, [0x49_u8, 0xd3, 0xe2], 0x9ac0_2120_u32),
        (false, u64_type, [0x49, 0xd3, 0xea], 0x9ac0_2520),
        (false, i64_type, [0x49, 0xd3, 0xfa], 0x9ac0_2920),
    ] {
        let x86 = emit_machine_code(&expression_plan(
            NativeTarget::linux_x64(),
            value_type,
            shift_expression(
                left_shift,
                i64_type,
                ScalarParameterLocation::Register(MachineRegister::X86Rdi),
                ScalarParameterLocation::Register(MachineRegister::X86Rsi),
            ),
        ))
        .expect("x86-64 wrapping shift emits");
        let bytes = &x86.functions[0].bytes;
        assert!(bytes.windows(3).any(|window| window == [0x83, 0xe1, 63]));
        assert!(bytes.windows(3).any(|window| window == x86_opcode));

        let aarch64 = emit_machine_code(&expression_plan(
            NativeTarget::linux_arm64(),
            value_type,
            shift_expression(
                left_shift,
                i64_type,
                ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                ScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
            ),
        ))
        .expect("AArch64 wrapping shift emits");
        let instructions = aarch64_instructions(&aarch64.functions[0].bytes);
        assert!(instructions.contains(&(0x9240_0000 | (5 << 10))));
        assert!(instructions.contains(&aarch64_opcode));
    }
}

#[test]
fn emits_x86_expression_after_assignment_spills_a_scratch_conflict() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let emitted = emit_machine_code(&expression_plan(
        NativeTarget::linux_x64(),
        scalar_type,
        wrapping_expression(
            ScalarParameterLocation::Register(MachineRegister::X86R10),
            ScalarParameterLocation::IncomingStack { byte_offset: 0 },
        ),
    ))
    .expect("assigned scratch conflict should emit");
    let bytes = &emitted.functions[0].bytes;
    assert_eq!(&bytes[..4], &[0x48, 0x83, 0xec, 16]); // sub rsp, frame
    assert_eq!(&bytes[4..9], &[0x4c, 0x89, 0x54, 0x24, 0]); // spill r10
    assert!(
        bytes
            .windows(5)
            .any(|window| window == [0x48, 0x8b, 0x44, 0x24, 32])
    ); // frame + return + expression push
    assert_eq!(&bytes[bytes.len() - 5..], &[0x48, 0x83, 0xc4, 16, 0xc3]);
}

#[test]
fn emits_parameter_fed_wrapping_subtract_for_both_architectures() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let expression = |left, right| TargetIntegerExpression::WrappingSubtract {
        psi_operation: OperationId::new(3).expect("operation"),
        left: Box::new(TargetIntegerExpression::Parameter {
            source_value: ValueId::new(1).expect("left"),
            parameter_index: 0,
            location: left,
        }),
        right: Box::new(TargetIntegerExpression::Parameter {
            source_value: ValueId::new(2).expect("right"),
            parameter_index: 1,
            location: right,
        }),
    };
    let x86 = emit_machine_code(&expression_plan(
        NativeTarget::linux_x64(),
        scalar_type,
        expression(
            ScalarParameterLocation::Register(MachineRegister::X86Rdi),
            ScalarParameterLocation::Register(MachineRegister::X86Rsi),
        ),
    ))
    .unwrap();
    assert_eq!(
        x86.functions[0].bytes,
        [
            0x48, 0x89, 0xf8, 0x25, 0xff, 0, 0, 0, 0x50, 0x48, 0x89, 0xf0, 0x25, 0xff, 0, 0, 0,
            0x41, 0x5a, 0x49, 0x29, 0xc2, 0x4c, 0x89, 0xd0, 0x25, 0xff, 0, 0, 0, 0xc3,
        ]
    );

    let aarch64 = emit_machine_code(&expression_plan(
        NativeTarget::linux_arm64(),
        scalar_type,
        expression(
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
        ),
    ))
    .unwrap();
    let instructions = aarch64_instructions(&aarch64.functions[0].bytes);
    assert!(instructions.contains(&0xcb00_0120)); // sub x0, x9, x0
    assert_eq!(instructions.last(), Some(&0xd65f_03c0));
}

#[test]
fn emits_parameter_fed_wrapping_multiply_for_both_architectures() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let expression = |left, right| TargetIntegerExpression::WrappingMultiply {
        psi_operation: OperationId::new(3).expect("operation"),
        left: Box::new(TargetIntegerExpression::Parameter {
            source_value: ValueId::new(1).expect("left"),
            parameter_index: 0,
            location: left,
        }),
        right: Box::new(TargetIntegerExpression::Parameter {
            source_value: ValueId::new(2).expect("right"),
            parameter_index: 1,
            location: right,
        }),
    };
    let x86 = emit_machine_code(&expression_plan(
        NativeTarget::linux_x64(),
        scalar_type,
        expression(
            ScalarParameterLocation::Register(MachineRegister::X86Rdi),
            ScalarParameterLocation::Register(MachineRegister::X86Rsi),
        ),
    ))
    .unwrap();
    assert_eq!(
        x86.functions[0].bytes,
        [
            0x48, 0x89, 0xf8, 0x25, 0xff, 0, 0, 0, 0x50, 0x48, 0x89, 0xf0, 0x25, 0xff, 0, 0, 0,
            0x41, 0x5a, 0x49, 0x0f, 0xaf, 0xc2, 0x25, 0xff, 0, 0, 0, 0xc3,
        ]
    );

    let aarch64 = emit_machine_code(&expression_plan(
        NativeTarget::linux_arm64(),
        scalar_type,
        expression(
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
        ),
    ))
    .unwrap();
    let instructions = aarch64_instructions(&aarch64.functions[0].bytes);
    assert!(instructions.contains(&0x9b00_7d20)); // mul x0, x9, x0
    assert_eq!(instructions.last(), Some(&0xd65f_03c0));
}

#[test]
fn emits_parameter_fed_exact_divide_for_both_architectures() {
    for (sign, x86_opcode, aarch64_opcode) in [
        (IntegerSign::Unsigned, [0x48, 0xf7, 0x34], 0x9ac0_0920),
        (IntegerSign::Signed, [0x48, 0xf7, 0x3c], 0x9ac0_0d20),
    ] {
        let scalar_type = IntegerType::new(sign, 64).expect("64-bit integer");
        let expression = |left, right| TargetIntegerExpression::ExactDivide {
            psi_operation: OperationId::new(4).expect("operation"),
            obligation: proof_obligation(),
            left: Box::new(TargetIntegerExpression::Parameter {
                source_value: ValueId::new(1).expect("left"),
                parameter_index: 0,
                location: left,
            }),
            right: Box::new(TargetIntegerExpression::Parameter {
                source_value: ValueId::new(2).expect("right"),
                parameter_index: 1,
                location: right,
            }),
        };
        let x86 = emit_machine_code(&expression_plan(
            NativeTarget::linux_x64(),
            scalar_type,
            expression(
                ScalarParameterLocation::Register(MachineRegister::X86Rdi),
                ScalarParameterLocation::Register(MachineRegister::X86Rsi),
            ),
        ))
        .expect("x86-64 exact divide emits");
        assert!(
            x86.functions[0]
                .bytes
                .windows(x86_opcode.len())
                .any(|window| window == x86_opcode)
        );

        let aarch64 = emit_machine_code(&expression_plan(
            NativeTarget::linux_arm64(),
            scalar_type,
            expression(
                ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                ScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
            ),
        ))
        .expect("AArch64 exact divide emits");
        let instructions = aarch64_instructions(&aarch64.functions[0].bytes);
        assert!(instructions.contains(&aarch64_opcode));
        assert_eq!(instructions.last(), Some(&0xd65f_03c0));
    }
}

#[test]
fn emits_parameter_fed_exact_remainder_for_both_architectures() {
    for (sign, x86_opcode, aarch64_divide) in [
        (IntegerSign::Unsigned, [0x48, 0xf7, 0x34], 0x9ac0_092a),
        (IntegerSign::Signed, [0x48, 0xf7, 0x3c], 0x9ac0_0d2a),
    ] {
        let scalar_type = IntegerType::new(sign, 64).expect("64-bit integer");
        let expression = |left, right| TargetIntegerExpression::ExactRemainder {
            psi_operation: OperationId::new(5).expect("operation"),
            obligation: proof_obligation(),
            left: Box::new(TargetIntegerExpression::Parameter {
                source_value: ValueId::new(1).expect("left"),
                parameter_index: 0,
                location: left,
            }),
            right: Box::new(TargetIntegerExpression::Parameter {
                source_value: ValueId::new(2).expect("right"),
                parameter_index: 1,
                location: right,
            }),
        };
        let x86 = emit_machine_code(&expression_plan(
            NativeTarget::linux_x64(),
            scalar_type,
            expression(
                ScalarParameterLocation::Register(MachineRegister::X86Rdi),
                ScalarParameterLocation::Register(MachineRegister::X86Rsi),
            ),
        ))
        .expect("x86-64 exact remainder emits");
        assert!(
            x86.functions[0]
                .bytes
                .windows(x86_opcode.len())
                .any(|window| window == x86_opcode)
        );
        assert!(
            x86.functions[0]
                .bytes
                .windows(3)
                .any(|window| window == [0x48, 0x89, 0xd0])
        ); // mov rax, rdx

        let aarch64 = emit_machine_code(&expression_plan(
            NativeTarget::linux_arm64(),
            scalar_type,
            expression(
                ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                ScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
            ),
        ))
        .expect("AArch64 exact remainder emits");
        let instructions = aarch64_instructions(&aarch64.functions[0].bytes);
        assert!(instructions.contains(&aarch64_divide));
        assert!(instructions.contains(&0x9b00_a540)); // msub x0, x10, x0, x9
        assert_eq!(instructions.last(), Some(&0xd65f_03c0));
    }
}

#[test]
fn emits_parameter_fed_wrapping_divide_for_both_architectures() {
    for (sign, x86_opcode, aarch64_opcode) in [
        (IntegerSign::Unsigned, [0x48, 0xf7, 0x34], 0x9ac0_0920),
        (IntegerSign::Signed, [0x48, 0xf7, 0x3c], 0x9ac0_0d20),
    ] {
        let scalar_type = IntegerType::new(sign, 64).expect("64-bit integer");
        let expression = |left, right| TargetIntegerExpression::WrappingDivide {
            psi_operation: OperationId::new(6).expect("operation"),
            obligation: proof_obligation(),
            left: Box::new(TargetIntegerExpression::Parameter {
                source_value: ValueId::new(1).expect("left"),
                parameter_index: 0,
                location: left,
            }),
            right: Box::new(TargetIntegerExpression::Parameter {
                source_value: ValueId::new(2).expect("right"),
                parameter_index: 1,
                location: right,
            }),
        };
        let x86 = emit_machine_code(&expression_plan(
            NativeTarget::linux_x64(),
            scalar_type,
            expression(
                ScalarParameterLocation::Register(MachineRegister::X86Rdi),
                ScalarParameterLocation::Register(MachineRegister::X86Rsi),
            ),
        ))
        .expect("x86-64 wrapping divide emits");
        assert!(
            x86.functions[0]
                .bytes
                .windows(x86_opcode.len())
                .any(|window| window == x86_opcode)
        );
        if sign == IntegerSign::Signed {
            assert!(
                x86.functions[0]
                    .bytes
                    .windows(5)
                    .any(|window| window == [0x48, 0x83, 0x3c, 0x24, 0xff])
            );
            assert!(
                x86.functions[0]
                    .bytes
                    .windows(3)
                    .any(|window| window == [0x48, 0xf7, 0xd8])
            );
        }

        let aarch64 = emit_machine_code(&expression_plan(
            NativeTarget::linux_arm64(),
            scalar_type,
            expression(
                ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                ScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
            ),
        ))
        .expect("AArch64 wrapping divide emits");
        let instructions = aarch64_instructions(&aarch64.functions[0].bytes);
        assert!(instructions.contains(&aarch64_opcode));
        assert_eq!(instructions.last(), Some(&0xd65f_03c0));
    }
}

#[test]
fn emits_parameter_fed_wrapping_remainder_for_both_architectures() {
    for (sign, x86_opcode, aarch64_opcode) in [
        (IntegerSign::Unsigned, [0x48, 0xf7, 0x34], 0x9ac0_092a),
        (IntegerSign::Signed, [0x48, 0xf7, 0x3c], 0x9ac0_0d2a),
    ] {
        let scalar_type = IntegerType::new(sign, 64).expect("64-bit integer");
        let expression = |left, right| TargetIntegerExpression::WrappingRemainder {
            psi_operation: OperationId::new(7).expect("operation"),
            obligation: proof_obligation(),
            left: Box::new(TargetIntegerExpression::Parameter {
                source_value: ValueId::new(1).expect("left"),
                parameter_index: 0,
                location: left,
            }),
            right: Box::new(TargetIntegerExpression::Parameter {
                source_value: ValueId::new(2).expect("right"),
                parameter_index: 1,
                location: right,
            }),
        };
        let x86 = emit_machine_code(&expression_plan(
            NativeTarget::linux_x64(),
            scalar_type,
            expression(
                ScalarParameterLocation::Register(MachineRegister::X86Rdi),
                ScalarParameterLocation::Register(MachineRegister::X86Rsi),
            ),
        ))
        .expect("x86-64 wrapping remainder emits");
        assert!(
            x86.functions[0]
                .bytes
                .windows(x86_opcode.len())
                .any(|window| window == x86_opcode)
        );
        assert!(
            x86.functions[0]
                .bytes
                .windows(3)
                .any(|window| window == [0x48, 0x89, 0xd0])
        );
        if sign == IntegerSign::Signed {
            assert!(
                x86.functions[0]
                    .bytes
                    .windows(5)
                    .any(|window| window == [0x48, 0x83, 0x3c, 0x24, 0xff])
            );
            assert!(
                x86.functions[0]
                    .bytes
                    .windows(2)
                    .any(|window| window == [0x31, 0xc0])
            );
        }

        let aarch64 = emit_machine_code(&expression_plan(
            NativeTarget::linux_arm64(),
            scalar_type,
            expression(
                ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                ScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
            ),
        ))
        .expect("AArch64 wrapping remainder emits");
        let instructions = aarch64_instructions(&aarch64.functions[0].bytes);
        assert!(instructions.contains(&aarch64_opcode));
        assert!(instructions.contains(&0x9b00_a540));
        assert_eq!(instructions.last(), Some(&0xd65f_03c0));
    }
}

#[test]
fn emits_parameter_fed_saturating_divide_for_both_architectures() {
    let scalar_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let expression = |left, right| TargetIntegerExpression::SaturatingDivide {
        psi_operation: OperationId::new(8).expect("operation"),
        obligation: proof_obligation(),
        left: Box::new(TargetIntegerExpression::Parameter {
            source_value: ValueId::new(1).expect("left"),
            parameter_index: 0,
            location: left,
        }),
        right: Box::new(TargetIntegerExpression::Parameter {
            source_value: ValueId::new(2).expect("right"),
            parameter_index: 1,
            location: right,
        }),
    };
    let x86 = emit_machine_code(&expression_plan(
        NativeTarget::linux_x64(),
        scalar_type,
        expression(
            ScalarParameterLocation::Register(MachineRegister::X86Rdi),
            ScalarParameterLocation::Register(MachineRegister::X86Rsi),
        ),
    ))
    .expect("x86-64 saturating divide emits");
    assert!(
        x86.functions[0]
            .bytes
            .windows(3)
            .any(|window| window == [0x49, 0x0f, 0x40])
    ); // cmovo
    assert!(
        x86.functions[0]
            .bytes
            .windows(5)
            .any(|window| window == [0x48, 0x83, 0x3c, 0x24, 0xff])
    );

    let aarch64 = emit_machine_code(&expression_plan(
        NativeTarget::linux_arm64(),
        scalar_type,
        expression(
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
        ),
    ))
    .expect("AArch64 saturating divide emits");
    let instructions = aarch64_instructions(&aarch64.functions[0].bytes);
    assert!(instructions.contains(&0x9ac0_0d2a)); // sdiv x10, x9, x0
    assert!(instructions.contains(&aarch64_csel(0, 11, 10, 0)));
    assert_eq!(instructions.last(), Some(&0xd65f_03c0));
}

#[test]
fn emits_parameter_fed_saturating_multiply_for_both_architectures() {
    let scalar_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let expression = |left, right| TargetIntegerExpression::SaturatingMultiply {
        psi_operation: OperationId::new(3).expect("operation"),
        left: Box::new(TargetIntegerExpression::Parameter {
            source_value: ValueId::new(1).expect("left"),
            parameter_index: 0,
            location: left,
        }),
        right: Box::new(TargetIntegerExpression::Parameter {
            source_value: ValueId::new(2).expect("right"),
            parameter_index: 1,
            location: right,
        }),
    };
    let x86 = emit_machine_code(&expression_plan(
        NativeTarget::linux_x64(),
        scalar_type,
        expression(
            ScalarParameterLocation::Register(MachineRegister::X86Rdi),
            ScalarParameterLocation::Register(MachineRegister::X86Rsi),
        ),
    ))
    .unwrap();
    assert!(
        x86.functions[0]
            .bytes
            .windows(3)
            .any(|window| window == [0x49, 0xf7, 0xea])
    ); // imul r10 -> rdx:rax
    assert!(
        x86.functions[0]
            .bytes
            .windows(4)
            .any(|window| window == [0x49, 0x0f, 0x40, 0xc3])
    ); // cmovo rax, r11

    let aarch64 = emit_machine_code(&expression_plan(
        NativeTarget::linux_arm64(),
        scalar_type,
        expression(
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
        ),
    ))
    .unwrap();
    let instructions = aarch64_instructions(&aarch64.functions[0].bytes);
    assert!(instructions.contains(&0x9b40_7d2a)); // smulh x10, x9, x0
    assert!(instructions.contains(&0x9b00_7d20)); // mul x0, x9, x0
    assert_eq!(instructions.last(), Some(&0xd65f_03c0));
}

#[test]
fn emits_parameter_fed_saturating_subtract_for_both_architectures() {
    let expression = |left, right| TargetIntegerExpression::SaturatingSubtract {
        psi_operation: OperationId::new(3).expect("operation"),
        left: Box::new(TargetIntegerExpression::Parameter {
            source_value: ValueId::new(1).expect("left"),
            parameter_index: 0,
            location: left,
        }),
        right: Box::new(TargetIntegerExpression::Parameter {
            source_value: ValueId::new(2).expect("right"),
            parameter_index: 1,
            location: right,
        }),
    };
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let x86 = emit_machine_code(&expression_plan(
        NativeTarget::linux_x64(),
        u8_type,
        expression(
            ScalarParameterLocation::Register(MachineRegister::X86Rdi),
            ScalarParameterLocation::Register(MachineRegister::X86Rsi),
        ),
    ))
    .unwrap();
    assert!(
        x86.functions[0]
            .bytes
            .windows(12)
            .any(|window| window == [0x49, 0x29, 0xc2, 0xb8, 0, 0, 0, 0, 0x49, 0x0f, 0x43, 0xc2])
    );

    let aarch64 = emit_machine_code(&expression_plan(
        NativeTarget::linux_arm64(),
        u8_type,
        expression(
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
        ),
    ))
    .unwrap();
    let instructions = aarch64_instructions(&aarch64.functions[0].bytes);
    assert!(instructions.contains(&0xeb00_0129)); // subs x9, x9, x0
    assert!(instructions.contains(&aarch64_csel(0, 9, 31, 2))); // cs

    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let x86 = emit_machine_code(&expression_plan(
        NativeTarget::linux_x64(),
        i64_type,
        expression(
            ScalarParameterLocation::Register(MachineRegister::X86Rdi),
            ScalarParameterLocation::Register(MachineRegister::X86Rsi),
        ),
    ))
    .unwrap();
    assert!(
        x86.functions[0]
            .bytes
            .windows(4)
            .any(|window| window == [0x49, 0x0f, 0x40, 0xc3])
    ); // cmovo

    let aarch64 = emit_machine_code(&expression_plan(
        NativeTarget::linux_arm64(),
        i64_type,
        expression(
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
        ),
    ))
    .unwrap();
    let instructions = aarch64_instructions(&aarch64.functions[0].bytes);
    assert!(instructions.contains(&0xeb00_0120)); // subs x0, x9, x0
    assert!(instructions.contains(&aarch64_csel(0, 0, 10, 7))); // vc
}

#[test]
fn runtime_expression_stack_loads_retain_the_incoming_stack_base() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let x86 = emit_machine_code(&expression_plan(
        NativeTarget::linux_x64(),
        scalar_type,
        wrapping_expression(
            ScalarParameterLocation::Register(MachineRegister::X86Rdi),
            ScalarParameterLocation::IncomingStack { byte_offset: 0 },
        ),
    ))
    .unwrap();
    assert!(
        x86.functions[0]
            .bytes
            .windows(5)
            .any(|window| window == [0x48, 0x8b, 0x44, 0x24, 16])
    );

    let aarch64 = emit_machine_code(&expression_plan(
        NativeTarget::linux_arm64(),
        scalar_type,
        wrapping_expression(
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
            ScalarParameterLocation::IncomingStack { byte_offset: 0 },
        ),
    ))
    .unwrap();
    assert!(aarch64_instructions(&aarch64.functions[0].bytes).contains(&0xf940_13e0));
}

#[test]
fn emits_signed_i64_saturation_for_both_architectures() {
    let scalar_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let expression = |left, right| TargetIntegerExpression::SaturatingAdd {
        psi_operation: OperationId::new(3).expect("operation"),
        left: Box::new(TargetIntegerExpression::Parameter {
            source_value: ValueId::new(1).expect("left"),
            parameter_index: 0,
            location: left,
        }),
        right: Box::new(TargetIntegerExpression::Parameter {
            source_value: ValueId::new(2).expect("right"),
            parameter_index: 1,
            location: right,
        }),
    };
    let x86 = emit_machine_code(&expression_plan(
        NativeTarget::linux_x64(),
        scalar_type,
        expression(
            ScalarParameterLocation::Register(MachineRegister::X86Rdi),
            ScalarParameterLocation::Register(MachineRegister::X86Rsi),
        ),
    ))
    .unwrap();
    let x86_bytes = &x86.functions[0].bytes;
    assert!(
        x86_bytes
            .windows(5)
            .any(|window| window == [0x49, 0x0f, 0xba, 0xfb, 0x3f])
    );
    assert!(
        x86_bytes
            .windows(4)
            .any(|window| window == [0x49, 0x0f, 0x40, 0xc3])
    );

    let aarch64 = emit_machine_code(&expression_plan(
        NativeTarget::linux_arm64(),
        scalar_type,
        expression(
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
        ),
    ))
    .unwrap();
    let instructions = aarch64_instructions(&aarch64.functions[0].bytes);
    assert!(instructions.contains(&0x937f_fd2a)); // asr x10, x9, 63
    assert!(instructions.contains(&0xca0b_014a)); // eor x10, x10, x11
    assert!(instructions.contains(&0xab00_0120)); // adds x0, x9, x0
    assert!(instructions.contains(&aarch64_csel(0, 0, 10, 7))); // vc
}

#[test]
fn emits_typed_direct_call_relocations_for_native_targets() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    for (target, argument_register, stack_byte_offset) in [
        (NativeTarget::linux_x64(), MachineRegister::X86Rdi, 0),
        (NativeTarget::windows_x64(), MachineRegister::X86Rcx, 32),
        (NativeTarget::linux_arm64(), MachineRegister::Aarch64X(0), 0),
    ] {
        let caller = MachineId::new(1).expect("caller");
        let callee = MachineId::new(2).expect("callee");
        let call_operation = OperationId::new(3).expect("call operation");
        let call_result = ValueId::new(4).expect("call result");
        let argument = ValueId::new(5).expect("argument");
        let plan = TargetOperationPlan {
            psi: identity(),
            target,
            entry: caller,
            functions: vec![
                TargetFunction {
                    fixed_integer_scalar_abi: None,
                    mixed_structural_scalar_abi: None,
                    machine: caller,
                    attachment: None,
                    provenance: TerminalPsiProvenance::default(),
                    operation: TargetOperation::ReturnIntegerExpression {
                        psi_edge: EdgeId::new(1).expect("return edge"),
                        source_value: call_result,
                        scalar_type,
                        expression: TargetIntegerExpression::WrappingAdd {
                            psi_operation: OperationId::new(7).expect("add operation"),
                            left: Box::new(TargetIntegerExpression::Immediate {
                                source_value: ValueId::new(7).expect("pending left value"),
                                value: IntegerValue::Unsigned(1),
                            }),
                            right: Box::new(TargetIntegerExpression::Call {
                                psi_operation: call_operation,
                                source_value: call_result,
                                callee,
                                requirement_obligations: Vec::new(),
                                crash_continuations: Vec::new(),
                                arguments: vec![
                                    TargetCallArgument {
                                        scalar_type: psi_core::ScalarType::Integer(scalar_type),
                                        location: ScalarParameterLocation::Register(
                                            argument_register,
                                        ),
                                        expression: TargetScalarExpression::Integer {
                                            scalar_type,
                                            expression: TargetIntegerExpression::Immediate {
                                                source_value: argument,
                                                value: IntegerValue::Unsigned(7),
                                            },
                                        },
                                    },
                                    TargetCallArgument {
                                        scalar_type: psi_core::ScalarType::Integer(scalar_type),
                                        location: ScalarParameterLocation::IncomingStack {
                                            byte_offset: stack_byte_offset,
                                        },
                                        expression: TargetScalarExpression::Integer {
                                            scalar_type,
                                            expression: TargetIntegerExpression::Immediate {
                                                source_value: ValueId::new(6)
                                                    .expect("stack argument"),
                                                value: IntegerValue::Unsigned(9),
                                            },
                                        },
                                    },
                                ],
                            }),
                        },
                    },
                },
                TargetFunction {
                    fixed_integer_scalar_abi: None,
                    mixed_structural_scalar_abi: None,
                    machine: callee,
                    attachment: None,
                    provenance: TerminalPsiProvenance::default(),
                    operation: TargetOperation::ReturnIntegerParameter {
                        psi_edge: EdgeId::new(2).expect("callee return edge"),
                        source_value: argument,
                        scalar_type,
                        parameter_index: 0,
                        location: ScalarParameterLocation::Register(argument_register),
                    },
                },
            ],
        };
        let emitted = emit_machine_code(&plan).expect("emit direct call");
        let caller = &emitted.functions[0];
        assert!(caller.scalar_stack.is_some());
        assert_eq!(caller.internal_calls.len(), 1);
        let relocation = caller.internal_calls[0];
        assert_eq!(relocation.owner, CallSiteOwner::Operation(call_operation));
        assert_eq!(relocation.target, callee);
        let call_stack = relocation
            .scalar_stack
            .expect("linear scalar call stack evidence");
        assert_eq!(relocation.unit_stack, None);
        match target.architecture {
            Architecture::X86_64 => {
                assert_eq!(call_stack.aarch64_return_link, None);
                assert!(call_stack.outbound.is_some());
                assert_eq!(caller.bytes[relocation.offset - 1], 0xe8);
                assert_eq!(
                    &caller.bytes[relocation.offset..relocation.offset + 4],
                    &[0; 4]
                );
                assert!(caller.bytes.windows(5).any(|window| {
                    window
                        == [
                            0x48,
                            0x89,
                            0x44,
                            0x24,
                            u8::try_from(stack_byte_offset).unwrap(),
                        ]
                }));
                if target.object_format == ObjectFormat::Coff {
                    assert_eq!(call_stack.outbound.expect("COFF outbound").byte_size, 48);
                    assert!(
                        caller
                            .bytes
                            .windows(4)
                            .any(|window| window == [0x48, 0x83, 0xec, 48])
                    );
                } else {
                    assert_eq!(call_stack.outbound.expect("SysV outbound").byte_size, 16);
                    assert!(
                        caller
                            .bytes
                            .windows(4)
                            .any(|window| window == [0x48, 0x83, 0xec, 16])
                    );
                }
            }
            Architecture::Aarch64 => {
                assert!(call_stack.outbound.is_some());
                assert!(call_stack.aarch64_return_link.is_some());
                assert_eq!(
                    &caller.bytes[relocation.offset..relocation.offset + 4],
                    &0x9400_0000_u32.to_le_bytes()
                );
                let instructions = aarch64_instructions(&caller.bytes);
                assert!(instructions.contains(&0xf900_0bfe)); // str x30, [sp, #16]
                assert!(instructions.contains(&0xf940_0bfe)); // ldr x30, [sp, #16]
                assert!(instructions.contains(&0xf900_03e0)); // str x0, [sp]
            }
        }
    }
}

#[test]
fn division_retains_exact_scalar_stack_control_evidence() {
    let scalar_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let expression = |kind, left, right| {
        let left = Box::new(TargetIntegerExpression::Parameter {
            source_value: ValueId::new(1).expect("left"),
            parameter_index: 0,
            location: left,
        });
        let right = Box::new(TargetIntegerExpression::Parameter {
            source_value: ValueId::new(2).expect("right"),
            parameter_index: 1,
            location: right,
        });
        let psi_operation = OperationId::new(3).expect("operation");
        match kind {
            0 => TargetIntegerExpression::ExactDivide {
                psi_operation,
                obligation: proof_obligation(),
                left,
                right,
            },
            1 => TargetIntegerExpression::ExactRemainder {
                psi_operation,
                obligation: proof_obligation(),
                left,
                right,
            },
            2 => TargetIntegerExpression::WrappingDivide {
                psi_operation,
                obligation: proof_obligation(),
                left,
                right,
            },
            3 => TargetIntegerExpression::WrappingRemainder {
                psi_operation,
                obligation: proof_obligation(),
                left,
                right,
            },
            4 => TargetIntegerExpression::SaturatingDivide {
                psi_operation,
                obligation: proof_obligation(),
                left,
                right,
            },
            5 => TargetIntegerExpression::SaturatingRemainder {
                psi_operation,
                obligation: proof_obligation(),
                left,
                right,
            },
            _ => unreachable!(),
        }
    };
    for kind in 0..6 {
        for (target, left, right) in [
            (
                NativeTarget::linux_x64(),
                ScalarParameterLocation::Register(MachineRegister::X86Rdi),
                ScalarParameterLocation::Register(MachineRegister::X86Rsi),
            ),
            (
                NativeTarget::linux_arm64(),
                ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                ScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
            ),
        ] {
            let emitted = emit_machine_code(&expression_plan(
                target,
                scalar_type,
                expression(kind, left, right),
            ))
            .expect("division emits with exact WCSU evidence");
            let stack = emitted.functions[0]
                .scalar_stack
                .as_ref()
                .expect("division stack evidence");
            if target.architecture == Architecture::X86_64 && kind >= 2 {
                let ScalarControlFlowEvidence::LinearWithDivisionBranches { branches } =
                    &stack.control_flow
                else {
                    panic!("signed x86 division must retain its generated diamond")
                };
                assert_eq!(branches.len(), 1);
                let branch = branches[0];
                assert_eq!(branch.branch_byte_count, 6);
                assert_eq!(branch.join_byte_count, 5);
                assert!(branch.branch_offset < branch.join_offset);
                assert_eq!(
                    branch.join_offset + branch.join_byte_count,
                    branch.ordinary_arm_offset
                );
                assert!(branch.ordinary_arm_offset < branch.merge_offset);
                assert_eq!(
                    stack
                        .mutations
                        .iter()
                        .filter(|mutation| matches!(
                            mutation.kind,
                            ScalarStackMutationKind::Release { byte_size: 8 }
                        ))
                        .count(),
                    2
                );
            } else {
                assert_eq!(stack.control_flow, ScalarControlFlowEvidence::Linear);
            }
        }
    }

    let parameter = |source, index, register| TargetIntegerExpression::Parameter {
        source_value: ValueId::new(source).expect("parameter"),
        parameter_index: index,
        location: ScalarParameterLocation::Register(register),
    };
    let divide = |operation| TargetIntegerExpression::WrappingDivide {
        psi_operation: OperationId::new(operation).expect("division operation"),
        obligation: proof_obligation(),
        left: Box::new(parameter(1, 0, MachineRegister::X86Rdi)),
        right: Box::new(parameter(2, 1, MachineRegister::X86Rsi)),
    };
    let repeated = TargetIntegerExpression::WrappingAdd {
        psi_operation: OperationId::new(5).expect("addition operation"),
        left: Box::new(divide(3)),
        right: Box::new(divide(4)),
    };
    let emitted = emit_machine_code(&expression_plan(
        NativeTarget::linux_x64(),
        scalar_type,
        repeated,
    ))
    .expect("repeated division emits");
    let ScalarControlFlowEvidence::LinearWithDivisionBranches { branches } = &emitted.functions[0]
        .scalar_stack
        .as_ref()
        .expect("repeated division stack evidence")
        .control_flow
    else {
        panic!("repeated signed divisions must retain both diamonds")
    };
    assert_eq!(branches.len(), 2);
    assert!(branches[0].merge_offset <= branches[1].branch_offset);
}

#[test]
fn rejects_integer_width_without_a_native_scalar_realization() {
    let mut plan = plan(NativeTarget::linux_x64());
    let TargetOperation::ReturnIntegerImmediate {
        scalar_type, value, ..
    } = &mut plan.functions[0].operation
    else {
        panic!("integer fixture must contain an integer return")
    };
    *scalar_type = IntegerType::new(IntegerSign::Signed, 128).expect("i128");
    *value = IntegerValue::Signed(7);
    assert!(matches!(
        emit_machine_code(&plan),
        Err(EmissionError::IntegerWidthNotNativelySupported { bits: 128, .. })
    ));
}

fn parameter_plan(
    target: NativeTarget,
    location: ScalarParameterLocation,
    is_64: bool,
) -> TargetOperationPlan {
    let scalar_type =
        IntegerType::new(IntegerSign::Unsigned, if is_64 { 64 } else { 8 }).expect("integer type");
    TargetOperationPlan {
        psi: identity(),
        target,
        entry: MachineId::new(1).expect("machine"),
        functions: vec![TargetFunction {
            fixed_integer_scalar_abi: None,
            mixed_structural_scalar_abi: None,
            machine: MachineId::new(1).expect("machine"),
            attachment: None,
            provenance: TerminalPsiProvenance::default(),
            operation: TargetOperation::ReturnIntegerParameter {
                psi_edge: EdgeId::new(1).expect("edge"),
                source_value: ValueId::new(1).expect("value"),
                scalar_type,
                parameter_index: 0,
                location,
            },
        }],
    }
}

fn expression_plan(
    target: NativeTarget,
    scalar_type: IntegerType,
    expression: TargetIntegerExpression,
) -> TargetOperationPlan {
    TargetOperationPlan {
        psi: identity(),
        target,
        entry: MachineId::new(1).expect("machine"),
        functions: vec![TargetFunction {
            fixed_integer_scalar_abi: None,
            mixed_structural_scalar_abi: None,
            machine: MachineId::new(1).expect("machine"),
            attachment: None,
            provenance: TerminalPsiProvenance::default(),
            operation: TargetOperation::ReturnIntegerExpression {
                psi_edge: EdgeId::new(1).expect("edge"),
                source_value: ValueId::new(3).expect("result"),
                scalar_type,
                expression,
            },
        }],
    }
}

fn boolean_equality_plan(
    target: NativeTarget,
    left_register: MachineRegister,
    right_register: MachineRegister,
) -> TargetOperationPlan {
    TargetOperationPlan {
        psi: identity(),
        target,
        entry: MachineId::new(1).expect("machine"),
        functions: vec![TargetFunction {
            fixed_integer_scalar_abi: None,
            mixed_structural_scalar_abi: None,
            machine: MachineId::new(1).expect("machine"),
            attachment: None,
            provenance: TerminalPsiProvenance::default(),
            operation: TargetOperation::ReturnBooleanExpression {
                psi_edge: EdgeId::new(1).expect("edge"),
                source_value: ValueId::new(3).expect("result"),
                expression: TargetBooleanExpression::Equal {
                    psi_operation: OperationId::new(1).expect("operation"),
                    left: Box::new(TargetBooleanExpression::Parameter {
                        source_value: ValueId::new(1).expect("left"),
                        parameter_index: 0,
                        location: ScalarParameterLocation::Register(left_register),
                    }),
                    right: Box::new(TargetBooleanExpression::Parameter {
                        source_value: ValueId::new(2).expect("right"),
                        parameter_index: 1,
                        location: ScalarParameterLocation::Register(right_register),
                    }),
                },
            },
        }],
    }
}

fn integer_equality_plan(
    target: NativeTarget,
    scalar_type: IntegerType,
    left_register: MachineRegister,
    right_register: MachineRegister,
) -> TargetOperationPlan {
    TargetOperationPlan {
        psi: identity(),
        target,
        entry: MachineId::new(1).expect("machine"),
        functions: vec![TargetFunction {
            fixed_integer_scalar_abi: None,
            mixed_structural_scalar_abi: None,
            machine: MachineId::new(1).expect("machine"),
            attachment: None,
            provenance: TerminalPsiProvenance::default(),
            operation: TargetOperation::ReturnBooleanExpression {
                psi_edge: EdgeId::new(1).expect("edge"),
                source_value: ValueId::new(3).expect("result"),
                expression: TargetBooleanExpression::IntegerEqual {
                    psi_operation: OperationId::new(1).expect("operation"),
                    scalar_type,
                    left: Box::new(TargetIntegerExpression::Parameter {
                        source_value: ValueId::new(1).expect("left"),
                        parameter_index: 0,
                        location: ScalarParameterLocation::Register(left_register),
                    }),
                    right: Box::new(TargetIntegerExpression::Parameter {
                        source_value: ValueId::new(2).expect("right"),
                        parameter_index: 1,
                        location: ScalarParameterLocation::Register(right_register),
                    }),
                },
            },
        }],
    }
}

fn integer_ordering_plan(
    target: NativeTarget,
    scalar_type: IntegerType,
    inclusive: bool,
    left_register: MachineRegister,
    right_register: MachineRegister,
) -> TargetOperationPlan {
    let left = Box::new(TargetIntegerExpression::Parameter {
        source_value: ValueId::new(1).expect("left"),
        parameter_index: 0,
        location: ScalarParameterLocation::Register(left_register),
    });
    let right = Box::new(TargetIntegerExpression::Parameter {
        source_value: ValueId::new(2).expect("right"),
        parameter_index: 1,
        location: ScalarParameterLocation::Register(right_register),
    });
    let expression = if inclusive {
        TargetBooleanExpression::IntegerLessOrEqual {
            psi_operation: OperationId::new(1).expect("operation"),
            scalar_type,
            left,
            right,
        }
    } else {
        TargetBooleanExpression::IntegerLessThan {
            psi_operation: OperationId::new(1).expect("operation"),
            scalar_type,
            left,
            right,
        }
    };
    TargetOperationPlan {
        psi: identity(),
        target,
        entry: MachineId::new(1).expect("machine"),
        functions: vec![TargetFunction {
            fixed_integer_scalar_abi: None,
            mixed_structural_scalar_abi: None,
            machine: MachineId::new(1).expect("machine"),
            attachment: None,
            provenance: TerminalPsiProvenance::default(),
            operation: TargetOperation::ReturnBooleanExpression {
                psi_edge: EdgeId::new(1).expect("edge"),
                source_value: ValueId::new(3).expect("result"),
                expression,
            },
        }],
    }
}

fn boolean_expression_conditional_plan(
    target: NativeTarget,
    left_register: MachineRegister,
    right_register: MachineRegister,
) -> TargetOperationPlan {
    let arm = |edge, return_edge, value| TargetConditionalBooleanArm {
        psi_edge: EdgeId::new(edge).expect("control edge"),
        control: Box::new(TargetBooleanControl::ReturnImmediate {
            psi_return_edge: EdgeId::new(return_edge).expect("return edge"),
            source_value: ValueId::new(if value { 4 } else { 5 }).expect("leaf value"),
            value,
        }),
    };
    TargetOperationPlan {
        psi: identity(),
        target,
        entry: MachineId::new(1).expect("machine"),
        functions: vec![TargetFunction {
            fixed_integer_scalar_abi: None,
            mixed_structural_scalar_abi: None,
            machine: MachineId::new(1).expect("machine"),
            attachment: None,
            provenance: TerminalPsiProvenance::default(),
            operation: TargetOperation::ReturnBooleanExpressionConditionalControl {
                condition_source: ValueId::new(3).expect("condition"),
                condition: TargetBooleanExpression::Equal {
                    psi_operation: OperationId::new(1).expect("operation"),
                    left: Box::new(TargetBooleanExpression::Parameter {
                        source_value: ValueId::new(1).expect("left"),
                        parameter_index: 0,
                        location: ScalarParameterLocation::Register(left_register),
                    }),
                    right: Box::new(TargetBooleanExpression::Parameter {
                        source_value: ValueId::new(2).expect("right"),
                        parameter_index: 1,
                        location: ScalarParameterLocation::Register(right_register),
                    }),
                },
                when_true: arm(1, 3, true),
                when_false: arm(2, 4, false),
            },
        }],
    }
}

fn calling_conditional_plan(
    target: NativeTarget,
    argument_register: MachineRegister,
) -> TargetOperationPlan {
    let caller = MachineId::new(1).unwrap();
    let callee = MachineId::new(2).unwrap();
    let parameter = ValueId::new(1).unwrap();
    let call = |operation: u64, result: u64| TargetBooleanExpression::Call {
        psi_operation: OperationId::new(operation).unwrap(),
        source_value: ValueId::new(result).unwrap(),
        callee,
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
        arguments: vec![TargetCallArgument {
            scalar_type: psi_core::ScalarType::Boolean,
            location: ScalarParameterLocation::Register(argument_register),
            expression: TargetScalarExpression::Boolean(TargetBooleanExpression::Parameter {
                source_value: parameter,
                parameter_index: 0,
                location: ScalarParameterLocation::Register(argument_register),
            }),
        }],
    };
    let arm = |edge, return_edge, operation, result| TargetConditionalBooleanArm {
        psi_edge: EdgeId::new(edge).unwrap(),
        control: Box::new(TargetBooleanControl::ReturnExpression {
            psi_return_edge: EdgeId::new(return_edge).unwrap(),
            source_value: ValueId::new(result).unwrap(),
            expression: call(operation, result),
        }),
    };
    TargetOperationPlan {
        psi: identity(),
        target,
        entry: caller,
        functions: vec![
            TargetFunction {
                fixed_integer_scalar_abi: None,
                mixed_structural_scalar_abi: None,
                machine: caller,
                attachment: None,
                provenance: TerminalPsiProvenance::default(),
                operation: TargetOperation::ReturnBooleanExpressionConditionalControl {
                    condition_source: ValueId::new(10).unwrap(),
                    condition: call(1, 10),
                    when_true: arm(1, 3, 2, 11),
                    when_false: arm(2, 4, 3, 12),
                },
            },
            TargetFunction {
                fixed_integer_scalar_abi: None,
                mixed_structural_scalar_abi: None,
                machine: callee,
                attachment: None,
                provenance: TerminalPsiProvenance::default(),
                operation: TargetOperation::ReturnBooleanParameter {
                    psi_edge: EdgeId::new(5).unwrap(),
                    source_value: parameter,
                    parameter_index: 0,
                    location: ScalarParameterLocation::Register(argument_register),
                },
            },
        ],
    }
}

fn calling_expression_condition_plan(
    target: NativeTarget,
    argument_register: MachineRegister,
) -> TargetOperationPlan {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let caller = MachineId::new(1).unwrap();
    let callee = MachineId::new(2).unwrap();
    let parameter = ValueId::new(1).unwrap();
    let arm = |edge, return_edge, source_value, value| TargetConditionalIntegerArm {
        psi_edge: EdgeId::new(edge).unwrap(),
        control: Box::new(TargetIntegerControl::Return {
            psi_return_edge: EdgeId::new(return_edge).unwrap(),
            source_value: ValueId::new(source_value).unwrap(),
            expression: TargetIntegerExpression::Immediate {
                source_value: ValueId::new(source_value).unwrap(),
                value: IntegerValue::Unsigned(value),
            },
        }),
    };
    TargetOperationPlan {
        psi: identity(),
        target,
        entry: caller,
        functions: vec![
            TargetFunction {
                fixed_integer_scalar_abi: None,
                mixed_structural_scalar_abi: None,
                machine: caller,
                attachment: None,
                provenance: TerminalPsiProvenance::default(),
                operation: TargetOperation::ReturnIntegerExpressionConditionalControl {
                    condition_source: ValueId::new(2).unwrap(),
                    condition: TargetBooleanExpression::Call {
                        psi_operation: OperationId::new(1).unwrap(),
                        source_value: ValueId::new(2).unwrap(),
                        callee,
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                        arguments: vec![TargetCallArgument {
                            scalar_type: psi_core::ScalarType::Boolean,
                            location: ScalarParameterLocation::Register(argument_register),
                            expression: TargetScalarExpression::Boolean(
                                TargetBooleanExpression::Parameter {
                                    source_value: parameter,
                                    parameter_index: 0,
                                    location: ScalarParameterLocation::Register(argument_register),
                                },
                            ),
                        }],
                    },
                    scalar_type,
                    when_true: arm(1, 3, 3, 1),
                    when_false: arm(2, 4, 4, 2),
                },
            },
            TargetFunction {
                fixed_integer_scalar_abi: None,
                mixed_structural_scalar_abi: None,
                machine: callee,
                attachment: None,
                provenance: TerminalPsiProvenance::default(),
                operation: TargetOperation::ReturnBooleanParameter {
                    psi_edge: EdgeId::new(5).unwrap(),
                    source_value: parameter,
                    parameter_index: 0,
                    location: ScalarParameterLocation::Register(argument_register),
                },
            },
        ],
    }
}

fn calling_arm_conditional_plan(
    target: NativeTarget,
    condition_register: MachineRegister,
    argument_register: MachineRegister,
) -> TargetOperationPlan {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let caller = MachineId::new(1).unwrap();
    let callee = MachineId::new(2).unwrap();
    let parameter = ValueId::new(1).unwrap();
    let true_arm = TargetConditionalIntegerArm {
        psi_edge: EdgeId::new(1).unwrap(),
        control: Box::new(TargetIntegerControl::Return {
            psi_return_edge: EdgeId::new(3).unwrap(),
            source_value: ValueId::new(5).unwrap(),
            expression: TargetIntegerExpression::WrappingAdd {
                psi_operation: OperationId::new(2).unwrap(),
                left: Box::new(TargetIntegerExpression::Immediate {
                    source_value: ValueId::new(2).unwrap(),
                    value: IntegerValue::Unsigned(1),
                }),
                right: Box::new(TargetIntegerExpression::Call {
                    psi_operation: OperationId::new(1).unwrap(),
                    source_value: ValueId::new(4).unwrap(),
                    callee,
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                    arguments: vec![TargetCallArgument {
                        scalar_type: psi_core::ScalarType::Integer(scalar_type),
                        location: ScalarParameterLocation::Register(argument_register),
                        expression: TargetScalarExpression::Integer {
                            scalar_type,
                            expression: TargetIntegerExpression::Immediate {
                                source_value: ValueId::new(3).unwrap(),
                                value: IntegerValue::Unsigned(7),
                            },
                        },
                    }],
                }),
            },
        }),
    };
    let false_arm = TargetConditionalIntegerArm {
        psi_edge: EdgeId::new(2).unwrap(),
        control: Box::new(TargetIntegerControl::Return {
            psi_return_edge: EdgeId::new(4).unwrap(),
            source_value: ValueId::new(6).unwrap(),
            expression: TargetIntegerExpression::Immediate {
                source_value: ValueId::new(6).unwrap(),
                value: IntegerValue::Unsigned(2),
            },
        }),
    };
    TargetOperationPlan {
        psi: identity(),
        target,
        entry: caller,
        functions: vec![
            TargetFunction {
                fixed_integer_scalar_abi: None,
                mixed_structural_scalar_abi: None,
                machine: caller,
                attachment: None,
                provenance: TerminalPsiProvenance::default(),
                operation: TargetOperation::ReturnIntegerConditionalControl {
                    condition_source: parameter,
                    condition_parameter_index: 0,
                    condition_location: ScalarParameterLocation::Register(condition_register),
                    scalar_type,
                    when_true: true_arm,
                    when_false: false_arm,
                },
            },
            TargetFunction {
                fixed_integer_scalar_abi: None,
                mixed_structural_scalar_abi: None,
                machine: callee,
                attachment: None,
                provenance: TerminalPsiProvenance::default(),
                operation: TargetOperation::ReturnIntegerParameter {
                    psi_edge: EdgeId::new(5).unwrap(),
                    source_value: parameter,
                    scalar_type,
                    parameter_index: 0,
                    location: ScalarParameterLocation::Register(argument_register),
                },
            },
        ],
    }
}

fn wrapping_expression(
    left_location: ScalarParameterLocation,
    right_location: ScalarParameterLocation,
) -> TargetIntegerExpression {
    TargetIntegerExpression::WrappingAdd {
        psi_operation: OperationId::new(3).expect("operation"),
        left: Box::new(TargetIntegerExpression::Parameter {
            source_value: ValueId::new(1).expect("left"),
            parameter_index: 0,
            location: left_location,
        }),
        right: Box::new(TargetIntegerExpression::Parameter {
            source_value: ValueId::new(2).expect("right"),
            parameter_index: 1,
            location: right_location,
        }),
    }
}

fn bitwise_expression(
    kind: u8,
    left_location: ScalarParameterLocation,
    right_location: ScalarParameterLocation,
) -> TargetIntegerExpression {
    let left = Box::new(TargetIntegerExpression::Parameter {
        source_value: ValueId::new(1).expect("left"),
        parameter_index: 0,
        location: left_location,
    });
    let right = Box::new(TargetIntegerExpression::Parameter {
        source_value: ValueId::new(2).expect("right"),
        parameter_index: 1,
        location: right_location,
    });
    let psi_operation = OperationId::new(3).expect("operation");
    match kind {
        0 => TargetIntegerExpression::BitwiseAnd {
            psi_operation,
            left,
            right,
        },
        1 => TargetIntegerExpression::BitwiseOr {
            psi_operation,
            left,
            right,
        },
        2 => TargetIntegerExpression::BitwiseXor {
            psi_operation,
            left,
            right,
        },
        _ => panic!("unknown bitwise test kind"),
    }
}

fn shift_expression(
    left_shift: bool,
    count_type: IntegerType,
    value_location: ScalarParameterLocation,
    count_location: ScalarParameterLocation,
) -> TargetIntegerExpression {
    let value = Box::new(TargetIntegerExpression::Parameter {
        source_value: ValueId::new(1).expect("value"),
        parameter_index: 0,
        location: value_location,
    });
    let count = Box::new(TargetIntegerExpression::Parameter {
        source_value: ValueId::new(2).expect("count"),
        parameter_index: 1,
        location: count_location,
    });
    let psi_operation = OperationId::new(3).expect("operation");
    if left_shift {
        TargetIntegerExpression::WrappingShiftLeft {
            psi_operation,
            count_type,
            value,
            count,
        }
    } else {
        TargetIntegerExpression::WrappingShiftRight {
            psi_operation,
            count_type,
            value,
            count,
        }
    }
}

fn aarch64_instructions(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("instruction")))
        .collect()
}

fn identity() -> TerminalPsiIdentity {
    TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::CURRENT,
        program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
    }
}
