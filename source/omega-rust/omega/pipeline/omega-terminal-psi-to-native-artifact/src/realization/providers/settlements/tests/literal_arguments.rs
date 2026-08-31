use super::*;

#[derive(Clone, Copy)]
struct LiteralCase {
    operation: psi_core::OperationId,
    value: psi_core::ValueId,
    scalar_type: psi_core::IntegerType,
    immediate: psi_core::IntegerValue,
    type_identity: &'static str,
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
        assert_exact_rejoined_register_literal_import_reaches_dynamic_elf(
            profile,
            &cases,
            expected_register,
            x86_prefix,
        );
    }
}

fn literal_cases(
    count: usize,
    tail_type: psi_core::IntegerType,
    tail_immediate: psi_core::IntegerValue,
    tail_identity: &'static str,
) -> Vec<LiteralCase> {
    let mut types = vec![
        psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 16).unwrap(),
        psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 64).unwrap(),
        psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 32).unwrap(),
        psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap(),
        psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 32).unwrap(),
        psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 64).unwrap(),
        psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 16).unwrap(),
        psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 64).unwrap(),
    ];
    let mut immediates = vec![
        psi_core::IntegerValue::Unsigned(513),
        psi_core::IntegerValue::Signed(-29),
        psi_core::IntegerValue::Unsigned(0x1234_5678),
        psi_core::IntegerValue::Unsigned(0xa5),
        psi_core::IntegerValue::Unsigned(0x89ab_cdef),
        psi_core::IntegerValue::Unsigned(0x0123_4567_89ab_cdef),
        psi_core::IntegerValue::Unsigned(0x4321),
        psi_core::IntegerValue::Unsigned(0x0123_4567_89ab_cdef),
    ];
    let mut type_identities = vec!["u16", "i64", "u32", "u8", "u32", "u64", "u16", "u64"];
    assert!(matches!(count, 6 | 8));
    types[count - 1] = tail_type;
    immediates[count - 1] = tail_immediate;
    type_identities[count - 1] = tail_identity;
    types
        .into_iter()
        .zip(immediates)
        .zip(type_identities)
        .take(count)
        .enumerate()
        .map(
            |(index, ((scalar_type, immediate), type_identity))| LiteralCase {
                operation: psi_core::OperationId::new(810 + u64::try_from(index).unwrap()).unwrap(),
                value: psi_core::ValueId::new(810 + u64::try_from(index).unwrap()).unwrap(),
                scalar_type,
                immediate,
                type_identity,
            },
        )
        .collect()
}

fn abstract_plan(cases: &[LiteralCase]) -> omega_abstract_operations::AbstractOperationPlan {
    let requirement = "omega::test::Foreign::leaf()";
    let machine = psi_core::MachineId::new(810).unwrap();
    let block = psi_core::BlockId::new(810).unwrap();
    let boundary = psi_core::BoundaryMachineId::new(810).unwrap();
    let call_operation = psi_core::OperationId::new(818).unwrap();
    let return_edge = psi_core::EdgeId::new(810).unwrap();
    let mut operations = cases
        .iter()
        .map(
            |case| omega_abstract_operations::AbstractOperation::IntegerConstant {
                psi_operation: case.operation,
                result: case.value,
                scalar_type: psi_core::ScalarType::Integer(case.scalar_type),
                value: case.immediate,
            },
        )
        .collect::<Vec<_>>();
    operations.push(omega_abstract_operations::AbstractOperation::BoundaryCall {
        psi_operation: call_operation,
        result: None,
        boundary,
        arguments: cases.iter().map(|case| case.value).collect(),
        structural_arguments: Vec::new(),
        completion_claim_sources: Vec::new(),
        completion_receipts: Vec::new(),
    });
    operations.push(omega_abstract_operations::AbstractOperation::ReturnUnit {
        psi_edge: return_edge,
        cleanup_actions: Vec::new(),
    });
    omega_abstract_operations::AbstractOperationPlan {
        psi: psi_terminal::TerminalPsiIdentity {
            vocabulary_marker: psi_terminal::VocabularyMarker::CURRENT,
            program_fingerprint: psi_terminal::SemanticFingerprint::from_bytes([0x81; 32]),
        },
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: vec![psi_terminal::BoundaryMachineDeclaration {
            id: boundary,
            identity: requirement.into(),
            attachment: None,
            scalar_parameters: cases
                .iter()
                .map(|case| psi_core::ScalarType::Integer(case.scalar_type))
                .collect(),
            structural_parameters: Vec::new(),
            result: None,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        }],
        provider_candidates: Vec::new(),
        functions: vec![omega_abstract_operations::AbstractFunction {
            machine,
            attachment: None,
            entry: block,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: omega_abstract_operations::AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![omega_abstract_operations::AbstractBlockEntry {
                block,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations,
        }],
    }
}

fn assert_exact_rejoined_register_literal_import_reaches_dynamic_elf(
    profile: omega_target::TargetProfile,
    cases: &[LiteralCase],
    expected_last_register: omega_calling_conventions::MachineRegister,
    expected_x86_prefix: Option<[u8; 2]>,
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
        ProviderBinding::Import { locator } => locator.clone(),
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
        assert_eq!(argument.source_value, case.value);
        assert_eq!(argument.scalar_type, case.scalar_type);
        assert_eq!(argument.immediate, case.immediate);
        assert_eq!(argument.parameter_index, parameter_index as u32);
        assert_eq!(
            argument.placement,
            boundary_entry_plan.call.parameters[parameter_index]
        );
    }
    assert!(matches!(
        target_arguments.last().unwrap().placement.locations.as_slice(),
        [omega_calling_conventions::ValueLocation::Register { register, .. }]
            if *register == expected_last_register
    ));
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
    assert_eq!(scalar_arguments, target_arguments);

    let mutate_last =
        |mut candidate: omega_assigned_target_operations::AssignedOperationPlan,
         mutate: &dyn Fn(&mut omega_target_operations::NormalizedForeignScalarArgument)| {
            let arguments = assigned_scalar_arguments_mut(&mut candidate, cases.len());
            mutate(arguments.last_mut().unwrap());
            assert!(omega_machine_emission::emit_machine_code(&candidate).is_err());
        };
    mutate_last(assigned.clone(), &|argument| argument.parameter_index = 0);
    mutate_last(assigned.clone(), &|argument| {
        argument.source_value = cases[0].value
    });
    mutate_last(assigned.clone(), &|argument| {
        argument.immediate = different_immediate(argument.immediate)
    });
    mutate_last(assigned.clone(), &|argument| {
        argument.placement = target_arguments[0].placement.clone()
    });
    mutate_last(assigned.clone(), &|argument| {
        let bytes = argument.scalar_type.bits().div_ceil(8);
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
    for (argument, target_argument) in call.scalar_arguments.iter().zip(target_arguments) {
        assert_eq!(argument.source_value, target_argument.source_value);
        assert_eq!(argument.scalar_type, target_argument.scalar_type);
        assert_eq!(argument.immediate, target_argument.immediate);
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
    let last_argument = call.scalar_arguments.last().unwrap();
    let last_bytes = &machine_code.functions[0].bytes
        [last_argument.code_offset..last_argument.code_offset + last_argument.byte_count];
    if let Some(prefix) = expected_x86_prefix {
        assert_eq!(&last_bytes[..2], &prefix);
    } else {
        for instruction in last_bytes.chunks_exact(4) {
            assert_eq!(
                u32::from_le_bytes(instruction.try_into().unwrap()) & 0x1f,
                7
            );
        }
    }

    let last_index = cases.len() - 1;
    let mut changed_value = machine_code.clone();
    changed_value.functions[0].foreign_calls[0].scalar_arguments[last_index].immediate =
        different_immediate(cases[last_index].immediate);
    assert!(omega_image_emission::build_object_artifact(&changed_value).is_err());
    let mut changed_carrier = machine_code.clone();
    changed_carrier.functions[0].foreign_calls[0].scalar_arguments[last_index].scalar_type =
        psi_core::IntegerType::address(cases[last_index].scalar_type.bits()).unwrap();
    assert!(omega_image_emission::build_object_artifact(&changed_carrier).is_err());
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

    let object = omega_image_emission::build_object_artifact(&machine_code).unwrap();
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
) -> &mut Vec<omega_target_operations::NormalizedForeignScalarArgument> {
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
