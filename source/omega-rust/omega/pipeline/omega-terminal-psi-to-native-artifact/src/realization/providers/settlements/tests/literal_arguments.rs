use super::*;

mod fixture;

use fixture::{abstract_plan, literal_cases};

#[derive(Clone, Copy)]
struct LiteralCase {
    operation: psi_core::OperationId,
    value: psi_core::ValueId,
    scalar_type: psi_core::IntegerType,
    immediate: psi_core::IntegerValue,
    type_identity: &'static str,
}

#[derive(Clone, Copy)]
enum ExpectedTailPlacement {
    Register {
        register: omega_calling_conventions::MachineRegister,
        x86_prefix: Option<[u8; 2]>,
    },
    Stack {
        byte_offset: u32,
        outbound_bytes: u32,
    },
}

#[test]
fn exact_rejoined_all_register_literal_imports_reach_dynamic_elf() {
    let unsigned_32 = psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 32).unwrap();
    let unsigned_64 = psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 64).unwrap();
    for (profile, count, tail_type, tail_immediate, tail_identity, expected_register, x86_prefix) in [
        (
            omega_target::TargetProfile::LinuxX64,
            6,
            unsigned_32,
            psi_core::IntegerValue::Unsigned(0x7654_3210),
            "u32",
            omega_calling_conventions::MachineRegister::X86R9,
            Some([0x41, 0xb9]),
        ),
        (
            omega_target::TargetProfile::LinuxX64,
            6,
            unsigned_64,
            psi_core::IntegerValue::Unsigned(0x7654_3210_fedc_ba98),
            "u64",
            omega_calling_conventions::MachineRegister::X86R9,
            Some([0x49, 0xb9]),
        ),
        (
            omega_target::TargetProfile::LinuxArm64,
            8,
            unsigned_64,
            psi_core::IntegerValue::Unsigned(0x7654_3210_fedc_ba98),
            "u64",
            omega_calling_conventions::MachineRegister::Aarch64X(7),
            None,
        ),
    ] {
        let cases = literal_cases(count, tail_type, tail_immediate, tail_identity);
        assert_exact_rejoined_literal_import_reaches_dynamic_elf(
            profile,
            &cases,
            ExpectedTailPlacement::Register {
                register: expected_register,
                x86_prefix,
            },
        );
    }
}

#[test]
fn exact_rejoined_stack_literal_imports_reach_dynamic_elf() {
    let unsigned_64 = psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 64).unwrap();
    for (profile, count, outbound_bytes) in [
        (omega_target::TargetProfile::LinuxX64, 8, 24),
        (omega_target::TargetProfile::LinuxArm64, 10, 16),
    ] {
        let cases = literal_cases(
            count,
            unsigned_64,
            psi_core::IntegerValue::Unsigned(0x7654_3210_fedc_ba98),
            "u64",
        );
        assert_exact_rejoined_literal_import_reaches_dynamic_elf(
            profile,
            &cases,
            ExpectedTailPlacement::Stack {
                byte_offset: 8,
                outbound_bytes,
            },
        );
    }
}

fn assert_exact_rejoined_literal_import_reaches_dynamic_elf(
    profile: omega_target::TargetProfile,
    cases: &[LiteralCase],
    expected_tail: ExpectedTailPlacement,
) {
    let requirement = "omega::test::Foreign::leaf()";
    let boundary = psi_core::BoundaryMachineId::new(810).unwrap();
    let target = profile.native_target();
    let abstract_plan = abstract_plan(cases);
    let mut selected_plan = import_plan(b"selected_integer_leaf", profile);
    selected_plan.schema.methods[0].parameter_count = cases.len();
    selected_plan.schema.methods[0].parameter_type_identities =
        cases.iter().map(|case| case.type_identity.into()).collect();
    let report_identity = selected_plan.report_fingerprint();
    let locator = match &selected_plan.rows[0].binding {
        ProviderBinding::Import { evaluated } => evaluated.locator().clone(),
        _ => unreachable!(),
    };
    let signature = omega_calling_conventions::CallSignature {
        parameters: cases
            .iter()
            .map(|case| {
                let bytes = case.scalar_type.bits().div_ceil(8);
                omega_calling_conventions::ValueShape::integer(bytes, bytes)
            })
            .collect(),
        result: None,
    };
    let boundary_entry_plan = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
        omega_calling_conventions::CallingPolicy::native_for_target(target),
        &signature,
    )
    .unwrap()
    .plan()
    .clone();
    let external = omega_calling_conventions::ExternalBindingRow {
        target_name: selected_plan.target.clone(),
        trait_name: selected_plan.schema.trait_name.clone(),
        method: "leaf".into(),
        requirement_identity: requirement.into(),
        table_type: String::new(),
        boundary_entry_plan: Some(boundary_entry_plan.clone()),
        binding: omega_calling_conventions::ExternalBindingKind::Import {
            locator: locator.clone(),
        },
    };
    let provider_plan_commitment = omega_task_plans::SameStackProviderPlanCommitment::from_digest(
        *selected_plan.identity_digest().as_bytes(),
    );
    let same_stack = omega_task_plans::admit_same_stack_contribution(
        omega_task_plans::SameStackContributionAdmissionCandidate {
            provider_plan_report_identity: report_identity,
            provider_plan_commitment,
            requirement_identity: requirement.into(),
            receipt:
                omega_task_plans::SameStackContributionAdmissionReceiptId::from_normalized_identity(
                    811,
                )
                .unwrap(),
            bytes: 64,
            alignment: 16,
        },
        report_identity,
        provider_plan_commitment,
        requirement,
    )
    .unwrap();
    let foreign = rejoin_normalized_foreign_call(
        &selected_plan,
        &[external],
        &same_stack,
        report_identity,
        requirement,
        target,
    )
    .unwrap();
    let evidence = TestProviderExecution {
        requirement: requirement.into(),
        provider_plan_report_identity: report_identity,
    };
    let target_plan = omega_abstract_operations_to_target_operations::lower_to_target_operations_with_provider_executions(
        &abstract_plan,
        target,
        &[omega_abstract_operations_to_target_operations::AdmittedBoundarySettlement {
            boundary,
            execution: omega_abstract_operations_to_target_operations::AdmittedBoundaryExecution::Provider(&evidence),
            realization: omega_target_operations::BoundarySettlementRealization::NormalizedForeignCall(foreign),
        }],
    )
    .unwrap();
    let omega_target_operations::TargetOperation::UnitBody(target_body) =
        &target_plan.functions[0].operation
    else {
        panic!("literal import lowers as a Unit body")
    };
    let omega_target_operations::TargetUnitOperation::NormalizedForeignCall {
        scalar_arguments: target_arguments,
        ..
    } = &target_body.operations[cases.len()]
    else {
        panic!("literal import remains a normalized foreign call")
    };
    assert_eq!(target_arguments.len(), cases.len());
    for (parameter_index, (argument, case)) in target_arguments.iter().zip(cases).enumerate() {
        assert_eq!(argument.source_value(), case.value);
        assert_eq!(
            argument.scalar_type(),
            psi_core::ScalarType::Integer(case.scalar_type)
        );
        assert_eq!(
            argument.source,
            omega_target_operations::TargetUnitScalarArgumentSource::IntegerImmediate {
                defining_operation: case.operation,
                source_value: case.value,
                scalar_type: case.scalar_type,
                value: case.immediate,
            }
        );
        assert_eq!(argument.parameter_index, parameter_index as u32);
        assert_eq!(
            argument.placement,
            boundary_entry_plan.call.parameters[parameter_index]
        );
    }
    match expected_tail {
        ExpectedTailPlacement::Register { register, .. } => assert!(matches!(
            target_arguments.last().unwrap().placement.locations.as_slice(),
            [omega_calling_conventions::ValueLocation::Register { register: actual, .. }]
                if *actual == register
        )),
        ExpectedTailPlacement::Stack { byte_offset, .. } => assert!(matches!(
            target_arguments.last().unwrap().placement.locations.as_slice(),
            [omega_calling_conventions::ValueLocation::Stack {
                stack_byte_offset,
                ..
            }] if *stack_byte_offset == byte_offset
        )),
    }
    if profile == omega_target::TargetProfile::LinuxArm64 {
        for (parameter_index, expected_register) in [
            (5, omega_calling_conventions::MachineRegister::Aarch64X(5)),
            (6, omega_calling_conventions::MachineRegister::Aarch64X(6)),
            (7, omega_calling_conventions::MachineRegister::Aarch64X(7)),
        ] {
            assert!(matches!(
                target_arguments[parameter_index]
                    .placement
                    .locations
                    .as_slice(),
                [omega_calling_conventions::ValueLocation::Register { register, .. }]
                    if *register == expected_register
            ));
        }
    }

    let assigned =
        omega_target_operations_to_assigned_target_operations::assign_registers(&target_plan)
            .unwrap();
    let omega_assigned_target_operations::AssignedOperation::UnitBody(assigned_body) =
        &assigned.functions[0].operation
    else {
        panic!("literal import assigns as a Unit body")
    };
    let omega_assigned_target_operations::AssignedUnitOperation::NormalizedForeignCall {
        scalar_arguments,
        ..
    } = &assigned_body.operations[cases.len()]
    else {
        panic!("assigned literal import remains a normalized foreign call")
    };
    assert_eq!(scalar_arguments.len(), target_arguments.len());
    for (assigned, target) in scalar_arguments.iter().zip(target_arguments) {
        assert_eq!(assigned.parameter_index, target.parameter_index);
        assert_eq!(assigned.placement, target.placement);
        assert_eq!(assigned.source.source_value(), target.source_value());
        assert_eq!(assigned.source.scalar_type(), target.scalar_type());
    }

    let mutate_last = |mut candidate: omega_assigned_target_operations::AssignedOperationPlan,
                       mutate: &dyn Fn(
        &mut omega_assigned_target_operations::AssignedNormalizedForeignScalarArgument,
    )| {
        let arguments = assigned_scalar_arguments_mut(&mut candidate, cases.len());
        mutate(arguments.last_mut().unwrap());
        assert!(omega_machine_emission::emit_machine_code(&candidate).is_err());
    };
    mutate_last(assigned.clone(), &|argument| argument.parameter_index = 0);
    mutate_last(assigned.clone(), &|argument| {
        let omega_assigned_target_operations::AssignedUnitScalarArgumentSource::IntegerImmediate {
            source_value,
            ..
        } = &mut argument.source
        else {
            unreachable!()
        };
        *source_value = cases[0].value;
    });
    mutate_last(assigned.clone(), &|argument| {
        let omega_assigned_target_operations::AssignedUnitScalarArgumentSource::IntegerImmediate {
            value,
            ..
        } = &mut argument.source
        else {
            unreachable!()
        };
        *value = different_immediate(*value);
    });
    mutate_last(assigned.clone(), &|argument| {
        argument.placement = target_arguments[0].placement.clone()
    });
    mutate_last(assigned.clone(), &|argument| {
        let psi_core::ScalarType::Integer(argument_type) = argument.source.scalar_type() else {
            panic!("literal argument remains an integer")
        };
        let bytes = argument_type.bits().div_ceil(8);
        argument.placement.locations = vec![omega_calling_conventions::ValueLocation::Stack {
            stack_byte_offset: 0,
            value_byte_offset: 0,
            byte_size: u16::try_from(bytes).unwrap(),
            alignment: u16::try_from(bytes).unwrap(),
        }];
    });

    let mut reordered_assignment = assigned.clone();
    assigned_scalar_arguments_mut(&mut reordered_assignment, cases.len())
        .swap(cases.len() - 2, cases.len() - 1);
    assert!(omega_machine_emission::emit_machine_code(&reordered_assignment).is_err());
    let mut extra_assignment = assigned.clone();
    let arguments = assigned_scalar_arguments_mut(&mut extra_assignment, cases.len());
    arguments.push(arguments.last().unwrap().clone());
    assert!(omega_machine_emission::emit_machine_code(&extra_assignment).is_err());

    let machine_code = omega_machine_emission::emit_machine_code(&assigned).unwrap();
    let [call] = machine_code.functions[0].foreign_calls.as_slice() else {
        panic!("one retained foreign call")
    };
    assert_eq!(call.locator, locator);
    assert_eq!(call.call_plan, boundary_entry_plan.call);
    assert_eq!(call.scalar_arguments.len(), cases.len());
    for ((argument, target_argument), case) in call
        .scalar_arguments
        .iter()
        .zip(target_arguments)
        .zip(cases)
    {
        assert_eq!(
            argument.source,
            omega_machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
                defining_operation: case.operation,
                source_value: case.value,
                scalar_type: case.scalar_type,
                value: case.immediate,
            }
        );
        assert_eq!(
            argument.source.source_value(),
            target_argument.source_value()
        );
        assert_eq!(argument.source.scalar_type(), target_argument.scalar_type());
        assert_eq!(argument.parameter_index, target_argument.parameter_index);
        assert_eq!(argument.placement, target_argument.placement);
        assert!(argument.byte_count > 0);
    }
    for pair in call.scalar_arguments.windows(2) {
        assert_eq!(
            pair[0].code_offset + pair[0].byte_count,
            pair[1].code_offset
        );
    }
    let call_start = match target.architecture {
        omega_target::Architecture::X86_64 => call.offset - 1,
        omega_target::Architecture::Aarch64 => call.offset,
    };
    let last_index = cases.len() - 1;
    let last_argument = call.scalar_arguments.last().unwrap();
    let outbound = call.unit_stack.outbound;
    match expected_tail {
        ExpectedTailPlacement::Register { .. } => {
            let expected_outbound = match target.architecture {
                omega_target::Architecture::X86_64 => 8,
                omega_target::Architecture::Aarch64 => 0,
            };
            assert_eq!(outbound.map_or(0, |pair| pair.byte_size), expected_outbound);
        }
        ExpectedTailPlacement::Stack { outbound_bytes, .. } => {
            let outbound = outbound.expect("stack arguments own an outbound area");
            assert_eq!(outbound.byte_size, outbound_bytes);
            assert_eq!(
                outbound.allocation_offset + outbound.allocation_byte_count,
                call.scalar_arguments[0].code_offset,
            );
            assert_eq!(outbound.release_offset, call.offset + 4);
        }
    }
    assert_eq!(
        last_argument.code_offset + last_argument.byte_count,
        call_start,
    );
    let last_bytes = &machine_code.functions[0].bytes
        [last_argument.code_offset..last_argument.code_offset + last_argument.byte_count];
    match expected_tail {
        ExpectedTailPlacement::Register {
            x86_prefix: Some(prefix),
            ..
        } => assert_eq!(&last_bytes[..2], &prefix),
        ExpectedTailPlacement::Register {
            x86_prefix: None, ..
        } => {
            for instruction in last_bytes.chunks_exact(4) {
                assert_eq!(
                    u32::from_le_bytes(instruction.try_into().unwrap()) & 0x1f,
                    7
                );
            }
        }
        ExpectedTailPlacement::Stack { byte_offset, .. } => match target.architecture {
            omega_target::Architecture::X86_64 => {
                assert_eq!(&last_bytes[..2], &[0x49, 0xbb]);
                assert_eq!(
                    &last_bytes[last_bytes.len() - 5..],
                    &[0x4c, 0x89, 0x5c, 0x24, u8::try_from(byte_offset).unwrap()],
                );
            }
            omega_target::Architecture::Aarch64 => {
                assert_eq!(
                    &last_bytes[last_bytes.len() - 4..],
                    &(0xf900_03e9_u32 | ((byte_offset / 8) << 10)).to_le_bytes(),
                );
            }
        },
    }

    if matches!(expected_tail, ExpectedTailPlacement::Stack { .. }) {
        let mut stripped_outbound = machine_code.clone();
        stripped_outbound.functions[0].foreign_calls[0]
            .unit_stack
            .outbound = None;
        assert!(omega_image_emission::build_object_artifact(&stripped_outbound).is_err());
        let mut resized_outbound = machine_code.clone();
        resized_outbound.functions[0].foreign_calls[0]
            .unit_stack
            .outbound
            .as_mut()
            .unwrap()
            .byte_size += 16;
        assert!(omega_image_emission::build_object_artifact(&resized_outbound).is_err());
        let mut moved_allocation = machine_code.clone();
        moved_allocation.functions[0].foreign_calls[0]
            .unit_stack
            .outbound
            .as_mut()
            .unwrap()
            .allocation_offset += 1;
        assert!(omega_image_emission::build_object_artifact(&moved_allocation).is_err());
        let mut changed_store = machine_code.clone();
        let store_byte = last_argument.code_offset + last_argument.byte_count - 1;
        changed_store.functions[0].bytes[store_byte] ^= 1;
        assert!(omega_image_emission::build_object_artifact(&changed_store).is_err());
        let mut changed_stack_offset = machine_code.clone();
        let [
            omega_calling_conventions::ValueLocation::Stack {
                stack_byte_offset, ..
            },
        ] = changed_stack_offset.functions[0].foreign_calls[0].scalar_arguments[last_index]
            .placement
            .locations
            .as_mut_slice()
        else {
            unreachable!()
        };
        *stack_byte_offset = 0;
        assert!(omega_image_emission::build_object_artifact(&changed_stack_offset).is_err());
        let mut changed_stack_shape = machine_code.clone();
        let [
            omega_calling_conventions::ValueLocation::Stack {
                value_byte_offset,
                byte_size,
                alignment,
                ..
            },
        ] = changed_stack_shape.functions[0].foreign_calls[0].scalar_arguments[last_index]
            .placement
            .locations
            .as_mut_slice()
        else {
            unreachable!()
        };
        *value_byte_offset = 1;
        *byte_size = 4;
        *alignment = 4;
        assert!(omega_image_emission::build_object_artifact(&changed_stack_shape).is_err());
        let mut overlapped_argument = machine_code.clone();
        overlapped_argument.functions[0].foreign_calls[0].scalar_arguments[0].code_offset -= 1;
        assert!(omega_image_emission::build_object_artifact(&overlapped_argument).is_err());
    }

    let mut changed_value = machine_code.clone();
    if let omega_machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
        value,
        ..
    } = &mut changed_value.functions[0].foreign_calls[0].scalar_arguments[last_index].source
    {
        *value = different_immediate(cases[last_index].immediate);
    }
    assert!(omega_image_emission::build_object_artifact(&changed_value).is_err());
    let mut changed_carrier = machine_code.clone();
    if let omega_machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
        scalar_type,
        ..
    } = &mut changed_carrier.functions[0].foreign_calls[0].scalar_arguments[last_index].source
    {
        *scalar_type =
            psi_core::IntegerType::address(cases[last_index].scalar_type.bits()).unwrap();
    }
    assert!(omega_image_emission::build_object_artifact(&changed_carrier).is_err());
    let mut changed_source = machine_code.clone();
    if let omega_machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
        source_value,
        ..
    } = &mut changed_source.functions[0].foreign_calls[0].scalar_arguments[last_index].source
    {
        *source_value = cases[0].value;
    }
    assert!(omega_image_emission::build_object_artifact(&changed_source).is_err());
    let mut changed_definition = machine_code.clone();
    if let omega_machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
        defining_operation,
        ..
    } = &mut changed_definition.functions[0].foreign_calls[0].scalar_arguments[last_index].source
    {
        *defining_operation = cases[0].operation;
    }
    assert!(omega_image_emission::build_object_artifact(&changed_definition).is_err());
    let mut changed_bytes = machine_code.clone();
    changed_bytes.functions[0].bytes[last_argument.code_offset] ^= 1;
    assert!(omega_image_emission::build_object_artifact(&changed_bytes).is_err());
    let mut reordered = machine_code.clone();
    reordered.functions[0].foreign_calls[0]
        .scalar_arguments
        .swap(last_index - 1, last_index);
    assert!(omega_image_emission::build_object_artifact(&reordered).is_err());
    let mut changed_interval = machine_code.clone();
    changed_interval.functions[0].foreign_calls[0].scalar_arguments[last_index].code_offset -= 1;
    assert!(omega_image_emission::build_object_artifact(&changed_interval).is_err());
    let mut changed_register = machine_code.clone();
    changed_register.functions[0].foreign_calls[0].scalar_arguments[last_index]
        .placement
        .locations = call.scalar_arguments[0].placement.locations.clone();
    assert!(omega_image_emission::build_object_artifact(&changed_register).is_err());
    let mut changed_stack = machine_code.clone();
    let bytes = cases[last_index].scalar_type.bits().div_ceil(8);
    changed_stack.functions[0].foreign_calls[0].scalar_arguments[last_index]
        .placement
        .locations = vec![omega_calling_conventions::ValueLocation::Stack {
        stack_byte_offset: 0,
        value_byte_offset: 0,
        byte_size: u16::try_from(bytes).unwrap(),
        alignment: u16::try_from(bytes).unwrap(),
    }];
    assert!(omega_image_emission::build_object_artifact(&changed_stack).is_err());
    let mut stripped = machine_code.clone();
    stripped.functions[0].foreign_calls[0]
        .scalar_arguments
        .pop();
    assert!(omega_image_emission::build_object_artifact(&stripped).is_err());
    let mut extra = machine_code.clone();
    let duplicate = extra.functions[0].foreign_calls[0]
        .scalar_arguments
        .last()
        .unwrap()
        .clone();
    extra.functions[0].foreign_calls[0]
        .scalar_arguments
        .push(duplicate);
    assert!(omega_image_emission::build_object_artifact(&extra).is_err());

    let mut wrong_contribution_plan = machine_code.clone();
    wrong_contribution_plan.functions[0].foreign_calls[0]
        .provider_execution
        .provider_plan_report_identity ^= 1;
    assert!(matches!(
        omega_image_emission::build_object_artifact(&wrong_contribution_plan),
        Err(omega_image_emission::ObjectError::ForeignStackProviderPlanMismatch { .. })
    ));

    let mut over_aligned = machine_code.clone();
    over_aligned.functions[0].foreign_calls[0].same_stack_contribution =
        omega_task_plans::admit_same_stack_contribution(
            omega_task_plans::SameStackContributionAdmissionCandidate {
                provider_plan_report_identity: report_identity,
                provider_plan_commitment,
                requirement_identity: requirement.into(),
                receipt: omega_task_plans::SameStackContributionAdmissionReceiptId::from_normalized_identity(812)
                    .unwrap(),
                bytes: 64,
                alignment: 32,
            },
            report_identity,
            provider_plan_commitment,
            requirement,
        )
        .unwrap();
    assert!(matches!(
        omega_image_emission::build_object_artifact(&over_aligned),
        Err(
            omega_image_emission::ObjectError::UnsupportedForeignStackAlignment {
                admitted_alignment: 32,
                physical_alignment: 16,
                ..
            }
        )
    ));

    let object = omega_image_emission::build_object_artifact(&machine_code).unwrap();
    let [object_call] = object.foreign_calls() else {
        panic!("one object foreign call")
    };
    assert_eq!(object_call.operation_ordinal, call.operation_ordinal);
    assert_eq!(
        object_call.scalar_arguments.len(),
        call.scalar_arguments.len()
    );
    let object_function = object
        .functions()
        .iter()
        .find(|function| function.machine == machine_code.functions[0].machine)
        .expect("object function owning the foreign call");
    for (object_argument, machine_argument) in object_call
        .scalar_arguments
        .iter()
        .zip(&call.scalar_arguments)
    {
        assert_eq!(
            object_argument.parameter_index,
            machine_argument.parameter_index
        );
        assert_eq!(object_argument.source, machine_argument.source);
        assert_eq!(object_argument.placement, machine_argument.placement);
        assert_eq!(object_argument.byte_count, machine_argument.byte_count);
        assert_eq!(
            object_argument.code_offset,
            object_function.text_offset + machine_argument.code_offset,
        );
    }
    assert_eq!(object_call.same_stack_contribution, same_stack);
    assert_eq!(
        object_call.provider_execution.provider_plan_report_identity,
        object_call
            .same_stack_contribution
            .provider_plan_report_identity(),
    );
    let demand = omega_image_emission::derive_stack_demand(&object, object.entry()).unwrap();
    let expected_foreign_peak = u64::from(object_call.caller_live_bytes)
        .checked_add(same_stack.bytes())
        .unwrap();
    assert_eq!(demand.ceiling_bytes(), expected_foreign_peak);
    assert_eq!(
        demand.admitted_contribution_report_identities(),
        &std::collections::BTreeSet::from([same_stack.report_identity()]),
    );
    assert_eq!(
        demand.admitted_contribution_commitments(),
        &std::collections::BTreeSet::from([same_stack.commitment()]),
    );
    assert_eq!(object.object().layout.normalized_imports.len(), 1);
    assert_eq!(object.relocations().record_count(), 1);
    let interpreter = omega_target::normalize_elf_interpreter_plan(
        match profile {
            omega_target::TargetProfile::LinuxX64 => b"/lib64/ld-linux-x86-64.so.2".to_vec(),
            omega_target::TargetProfile::LinuxArm64 => b"/lib/ld-linux-aarch64.so.1".to_vec(),
            _ => unreachable!(),
        },
        profile,
    )
    .unwrap();
    let image = omega_image_emission::emit_dynamic_elf_image(&object, interpreter).unwrap();
    assert_eq!(image.output().final_image_imports, 1);
    assert_eq!(image.output().final_image_relocations, 1);
}

fn assigned_scalar_arguments_mut(
    plan: &mut omega_assigned_target_operations::AssignedOperationPlan,
    operation_index: usize,
) -> &mut Vec<omega_assigned_target_operations::AssignedNormalizedForeignScalarArgument> {
    let omega_assigned_target_operations::AssignedOperation::UnitBody(body) =
        &mut plan.functions[0].operation
    else {
        unreachable!()
    };
    let omega_assigned_target_operations::AssignedUnitOperation::NormalizedForeignCall {
        scalar_arguments,
        ..
    } = &mut body.operations[operation_index]
    else {
        unreachable!()
    };
    scalar_arguments
}

fn different_immediate(value: psi_core::IntegerValue) -> psi_core::IntegerValue {
    match value {
        psi_core::IntegerValue::Signed(0) => psi_core::IntegerValue::Signed(1),
        psi_core::IntegerValue::Signed(_) => psi_core::IntegerValue::Signed(0),
        psi_core::IntegerValue::Unsigned(0) => psi_core::IntegerValue::Unsigned(1),
        psi_core::IntegerValue::Unsigned(_) => psi_core::IntegerValue::Unsigned(0),
    }
}
