//! Runtime scalar-home import realization and retained-evidence checks.

use super::*;

#[test]
fn exact_runtime_scalar_home_import_reaches_dynamic_elf() {
    for (profile, argument_count) in [
        (omega_target::TargetProfile::LinuxX64, 1),
        (omega_target::TargetProfile::LinuxArm64, 1),
        (omega_target::TargetProfile::LinuxX64, 8),
        (omega_target::TargetProfile::LinuxArm64, 10),
    ] {
        assert_exact_runtime_scalar_home_import_reaches_dynamic_elf(profile, argument_count);
    }
}

fn assert_exact_runtime_scalar_home_import_reaches_dynamic_elf(
    profile: omega_target::TargetProfile,
    argument_count: usize,
) {
    let requirement = "omega::test::Foreign::leaf()";
    let boundary = psi_core::BoundaryMachineId::new(822).unwrap();
    let target = profile.native_target();
    let scalar_type = psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 32).unwrap();
    let abstract_plan = runtime_argument_abstract_plan(scalar_type, argument_count);
    let mut selected_plan = import_plan(b"selected_runtime_leaf", profile);
    selected_plan.schema.methods[0].parameter_count = argument_count;
    selected_plan.schema.methods[0].parameter_type_identities = vec!["i32".into(); argument_count];
    let report_identity = selected_plan.report_fingerprint();
    let locator = match &selected_plan.rows[0].binding {
        ProviderBinding::Import { evaluated } => evaluated.locator().clone(),
        _ => unreachable!(),
    };
    let boundary_entry_plan = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
        omega_calling_conventions::CallingPolicy::native_for_target(target),
        &omega_calling_conventions::CallSignature {
            parameters: vec![omega_calling_conventions::ValueShape::integer(4, 4); argument_count],
            result: None,
        },
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
                    823,
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
        panic!("runtime import lowers inside an attached Unit body")
    };
    let omega_target_operations::TargetUnitOperation::ScalarCall { result_home, .. } =
        &target_body.operations[1]
    else {
        panic!("runtime source has one durable scalar home")
    };
    let omega_target_operations::TargetUnitOperation::NormalizedForeignCall {
        scalar_arguments,
        ..
    } = &target_body.operations[2]
    else {
        panic!("runtime import remains a normalized foreign call")
    };
    assert_eq!(scalar_arguments.len(), argument_count);
    for (index, argument) in scalar_arguments.iter().enumerate() {
        assert_eq!(
            argument.source,
            omega_target_operations::TargetUnitScalarArgumentSource::Home(*result_home)
        );
        assert_eq!(
            argument.placement,
            boundary_entry_plan.call.parameters[index]
        );
    }

    let assigned =
        omega_target_operations_to_assigned_target_operations::assign_registers(&target_plan)
            .unwrap();
    let omega_assigned_target_operations::AssignedOperation::UnitBody(assigned_body) =
        &assigned.functions[0].operation
    else {
        unreachable!()
    };
    let omega_assigned_target_operations::AssignedUnitOperation::ScalarCall {
        result_home: assigned_home,
        ..
    } = &assigned_body.operations[1]
    else {
        unreachable!()
    };
    let omega_assigned_target_operations::AssignedUnitOperation::NormalizedForeignCall {
        scalar_arguments,
        ..
    } = &assigned_body.operations[2]
    else {
        unreachable!()
    };
    assert_eq!(scalar_arguments.len(), argument_count);
    assert!(scalar_arguments.iter().all(|argument| argument.source
        == omega_assigned_target_operations::AssignedUnitScalarArgumentSource::Home(
            *assigned_home
        )));

    let machine_code = omega_machine_emission::emit_machine_code(&assigned).unwrap();
    let function = &machine_code.functions[0];
    let [call] = function.foreign_calls.as_slice() else {
        panic!("one runtime foreign call")
    };
    assert_eq!(call.scalar_arguments.len(), argument_count);
    let argument = &call.scalar_arguments[0];
    let last_argument = call.scalar_arguments.last().unwrap();
    let expected_home = omega_machine_code::UnitScalarHomeRecord {
        defining_operation: assigned_home.defining_operation,
        source_value: assigned_home.source_value,
        scalar_type: assigned_home.scalar_type,
        shape: assigned_home.shape,
        byte_offset: assigned_home.byte_offset,
    };
    assert!(call.scalar_arguments.iter().all(|argument| argument.source
        == omega_machine_code::InternalUnitScalarArgumentSourceRecord::Home(expected_home)));
    let expected_outbound = match (target.architecture, argument_count) {
        (omega_target::Architecture::X86_64, 1) => 8,
        (omega_target::Architecture::X86_64, 8) => 24,
        (omega_target::Architecture::Aarch64, 1) => 0,
        (omega_target::Architecture::Aarch64, 10) => 16,
        _ => unreachable!(),
    };
    assert_eq!(
        call.unit_stack.outbound.map_or(0, |pair| pair.byte_size),
        expected_outbound,
    );
    let argument_bytes =
        &function.bytes[argument.code_offset..argument.code_offset + argument.byte_count];
    match (target.architecture, argument_count) {
        (omega_target::Architecture::X86_64, 1) => {
            assert_eq!(argument_bytes, &[0x48, 0x8b, 0x7c, 0x24, 0x08]);
        }
        (omega_target::Architecture::Aarch64, 1) => {
            assert_eq!(argument_bytes, &0xf940_03e0_u32.to_le_bytes());
        }
        (omega_target::Architecture::X86_64, 8) => {
            assert_eq!(argument_bytes, &[0x48, 0x8b, 0x7c, 0x24, 0x18]);
            let last_bytes = &function.bytes
                [last_argument.code_offset..last_argument.code_offset + last_argument.byte_count];
            assert_eq!(
                last_bytes,
                &[0x4c, 0x8b, 0x5c, 0x24, 0x18, 0x4c, 0x89, 0x5c, 0x24, 0x08],
            );
        }
        (omega_target::Architecture::Aarch64, 10) => {
            assert_eq!(argument_bytes, &0xf940_0be0_u32.to_le_bytes());
            let last_bytes = &function.bytes
                [last_argument.code_offset..last_argument.code_offset + last_argument.byte_count];
            assert_eq!(
                last_bytes,
                [0xf940_0be9_u32, 0xf900_07e9_u32]
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>(),
            );
        }
        _ => unreachable!(),
    }

    let mut changed_home = machine_code.clone();
    let omega_machine_code::InternalUnitScalarArgumentSourceRecord::Home(home) =
        &mut changed_home.functions[0].foreign_calls[0]
            .scalar_arguments
            .last_mut()
            .unwrap()
            .source
    else {
        unreachable!()
    };
    home.byte_offset += 8;
    assert!(omega_image_emission::build_object_artifact(&changed_home).is_err());
    let mut changed_source = machine_code.clone();
    let omega_machine_code::InternalUnitScalarArgumentSourceRecord::Home(home) =
        &mut changed_source.functions[0].foreign_calls[0]
            .scalar_arguments
            .last_mut()
            .unwrap()
            .source
    else {
        unreachable!()
    };
    home.source_value = psi_core::ValueId::new(829).unwrap();
    assert!(omega_image_emission::build_object_artifact(&changed_source).is_err());
    let mut changed_bytes = machine_code.clone();
    changed_bytes.functions[0].bytes[last_argument.code_offset] ^= 1;
    assert!(omega_image_emission::build_object_artifact(&changed_bytes).is_err());

    let object = omega_image_emission::build_object_artifact(&machine_code).unwrap();
    let [object_call] = object.foreign_calls() else {
        panic!("one object foreign call")
    };
    assert_eq!(object_call.operation_ordinal, call.operation_ordinal);
    assert_eq!(
        object_call.scalar_arguments.len(),
        call.scalar_arguments.len()
    );
    assert!(
        object_call
            .scalar_arguments
            .iter()
            .all(|argument| argument.source
                == omega_machine_code::InternalUnitScalarArgumentSourceRecord::Home(expected_home))
    );
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
    assert_eq!(image.output().final_image_relocations, 2);
}
