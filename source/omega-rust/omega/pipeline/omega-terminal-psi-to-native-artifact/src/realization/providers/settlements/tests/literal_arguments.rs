use super::*;

#[test]
fn exact_rejoined_four_literal_import_reaches_dynamic_elf_on_both_targets() {
    let requirement = "omega::test::Foreign::leaf()";
    let machine = psi_core::MachineId::new(810).unwrap();
    let block = psi_core::BlockId::new(810).unwrap();
    let boundary = psi_core::BoundaryMachineId::new(810).unwrap();
    let first_constant_operation = psi_core::OperationId::new(810).unwrap();
    let second_constant_operation = psi_core::OperationId::new(811).unwrap();
    let third_constant_operation = psi_core::OperationId::new(812).unwrap();
    let fourth_constant_operation = psi_core::OperationId::new(813).unwrap();
    let call_operation = psi_core::OperationId::new(814).unwrap();
    let return_edge = psi_core::EdgeId::new(810).unwrap();
    let first_value = psi_core::ValueId::new(810).unwrap();
    let second_value = psi_core::ValueId::new(811).unwrap();
    let third_value = psi_core::ValueId::new(812).unwrap();
    let fourth_value = psi_core::ValueId::new(813).unwrap();
    let first_type = psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 16).unwrap();
    let second_type = psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 64).unwrap();
    let third_type = psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 32).unwrap();
    let fourth_type = psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap();
    let first_immediate = psi_core::IntegerValue::Unsigned(513);
    let second_immediate = psi_core::IntegerValue::Signed(-29);
    let third_immediate = psi_core::IntegerValue::Unsigned(0x1234_5678);
    let fourth_immediate = psi_core::IntegerValue::Unsigned(0xa5);
    let psi = psi_terminal::TerminalPsiIdentity {
        vocabulary_marker: psi_terminal::VocabularyMarker::CURRENT,
        program_fingerprint: psi_terminal::SemanticFingerprint::from_bytes([0x81; 32]),
    };
    let abstract_plan = omega_abstract_operations::AbstractOperationPlan {
        psi,
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: vec![psi_terminal::BoundaryMachineDeclaration {
            id: boundary,
            identity: requirement.into(),
            attachment: None,
            scalar_parameters: vec![
                psi_core::ScalarType::Integer(first_type),
                psi_core::ScalarType::Integer(second_type),
                psi_core::ScalarType::Integer(third_type),
                psi_core::ScalarType::Integer(fourth_type),
            ],
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
            operations: vec![
                omega_abstract_operations::AbstractOperation::IntegerConstant {
                    psi_operation: first_constant_operation,
                    result: first_value,
                    scalar_type: psi_core::ScalarType::Integer(first_type),
                    value: first_immediate,
                },
                omega_abstract_operations::AbstractOperation::IntegerConstant {
                    psi_operation: second_constant_operation,
                    result: second_value,
                    scalar_type: psi_core::ScalarType::Integer(second_type),
                    value: second_immediate,
                },
                omega_abstract_operations::AbstractOperation::IntegerConstant {
                    psi_operation: third_constant_operation,
                    result: third_value,
                    scalar_type: psi_core::ScalarType::Integer(third_type),
                    value: third_immediate,
                },
                omega_abstract_operations::AbstractOperation::IntegerConstant {
                    psi_operation: fourth_constant_operation,
                    result: fourth_value,
                    scalar_type: psi_core::ScalarType::Integer(fourth_type),
                    value: fourth_immediate,
                },
                omega_abstract_operations::AbstractOperation::BoundaryCall {
                    psi_operation: call_operation,
                    result: None,
                    boundary,
                    arguments: vec![first_value, second_value, third_value, fourth_value],
                    structural_arguments: Vec::new(),
                    completion_claim_sources: Vec::new(),
                    completion_receipts: Vec::new(),
                },
                omega_abstract_operations::AbstractOperation::ReturnUnit {
                    psi_edge: return_edge,
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    };

    for profile in [
        omega_target::TargetProfile::LinuxX64,
        omega_target::TargetProfile::LinuxArm64,
    ] {
        let target = profile.native_target();
        let mut selected_plan = import_plan(b"selected_integer_leaf", profile);
        selected_plan.schema.methods[0].parameter_count = 4;
        selected_plan.schema.methods[0].parameter_type_identities =
            vec!["u16".into(), "i64".into(), "u32".into(), "u8".into()];
        let report_identity = selected_plan.report_fingerprint();
        let locator = match &selected_plan.rows[0].binding {
            ProviderBinding::Import { locator } => locator.clone(),
            _ => unreachable!(),
        };
        let boundary_entry_plan = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
            omega_calling_conventions::CallingPolicy::native_for_target(target),
            &omega_calling_conventions::CallSignature {
                parameters: vec![
                    omega_calling_conventions::ValueShape::integer(2, 2),
                    omega_calling_conventions::ValueShape::integer(8, 8),
                    omega_calling_conventions::ValueShape::integer(4, 4),
                    omega_calling_conventions::ValueShape::integer(1, 1),
                ],
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
        let provider_plan_commitment =
            omega_task_plans::SameStackProviderPlanCommitment::from_digest(
                *selected_plan.identity_digest().as_bytes(),
            );
        let same_stack = omega_task_plans::admit_same_stack_contribution(
            omega_task_plans::SameStackContributionAdmissionCandidate {
                provider_plan_report_identity: report_identity,
                provider_plan_commitment,
                requirement_identity: requirement.into(),
                receipt: omega_task_plans::SameStackContributionAdmissionReceiptId::from_normalized_identity(811).unwrap(),
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
                provider_execution: &evidence,
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
            scalar_arguments,
            ..
        } = &target_body.operations[4]
        else {
            panic!("literal import remains a normalized foreign call")
        };
        let [first_target, second_target, third_target, fourth_target] =
            scalar_arguments.as_slice()
        else {
            panic!("four target arguments")
        };
        assert_eq!(first_target.source_value, first_value);
        assert_eq!(first_target.scalar_type, first_type);
        assert_eq!(first_target.immediate, first_immediate);
        assert_eq!(first_target.parameter_index, 0);
        assert_eq!(second_target.source_value, second_value);
        assert_eq!(second_target.scalar_type, second_type);
        assert_eq!(second_target.immediate, second_immediate);
        assert_eq!(second_target.parameter_index, 1);
        assert_eq!(third_target.source_value, third_value);
        assert_eq!(third_target.scalar_type, third_type);
        assert_eq!(third_target.immediate, third_immediate);
        assert_eq!(third_target.parameter_index, 2);
        assert_eq!(fourth_target.source_value, fourth_value);
        assert_eq!(fourth_target.scalar_type, fourth_type);
        assert_eq!(fourth_target.immediate, fourth_immediate);
        assert_eq!(fourth_target.parameter_index, 3);
        let expected_fourth_register = match profile {
            omega_target::TargetProfile::LinuxX64 => {
                omega_calling_conventions::MachineRegister::X86Rcx
            }
            omega_target::TargetProfile::LinuxArm64 => {
                omega_calling_conventions::MachineRegister::Aarch64X(3)
            }
            _ => unreachable!(),
        };
        assert!(matches!(
            fourth_target.placement.locations.as_slice(),
            [omega_calling_conventions::ValueLocation::Register { register, .. }]
                if *register == expected_fourth_register
        ));

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
        } = &assigned_body.operations[4]
        else {
            panic!("assigned literal import remains a normalized foreign call")
        };
        assert_eq!(
            scalar_arguments,
            &[
                first_target.clone(),
                second_target.clone(),
                third_target.clone(),
                fourth_target.clone(),
            ]
        );

        let mutate_fourth =
            |mut candidate: omega_assigned_target_operations::AssignedOperationPlan,
             mutate: &dyn Fn(&mut omega_target_operations::NormalizedForeignScalarArgument)| {
                let omega_assigned_target_operations::AssignedOperation::UnitBody(body) =
                    &mut candidate.functions[0].operation
                else {
                    unreachable!()
                };
                let omega_assigned_target_operations::AssignedUnitOperation::NormalizedForeignCall {
                    scalar_arguments,
                    ..
                } = &mut body.operations[4]
                else {
                    unreachable!()
                };
                mutate(&mut scalar_arguments[3]);
                assert!(omega_machine_emission::emit_machine_code(&candidate).is_err());
            };
        mutate_fourth(assigned.clone(), &|argument| {
            argument.parameter_index = 0;
        });
        mutate_fourth(assigned.clone(), &|argument| {
            argument.source_value = first_value;
        });
        mutate_fourth(assigned.clone(), &|argument| {
            argument.immediate = psi_core::IntegerValue::Unsigned(0xa6);
        });
        mutate_fourth(assigned.clone(), &|argument| {
            argument.placement = first_target.placement.clone();
        });
        let mut reordered_assignment = assigned.clone();
        let omega_assigned_target_operations::AssignedOperation::UnitBody(body) =
            &mut reordered_assignment.functions[0].operation
        else {
            unreachable!()
        };
        let omega_assigned_target_operations::AssignedUnitOperation::NormalizedForeignCall {
            scalar_arguments,
            ..
        } = &mut body.operations[4]
        else {
            unreachable!()
        };
        scalar_arguments.swap(0, 1);
        assert!(omega_machine_emission::emit_machine_code(&reordered_assignment).is_err());
        let mut fifth_assignment = assigned.clone();
        let omega_assigned_target_operations::AssignedOperation::UnitBody(body) =
            &mut fifth_assignment.functions[0].operation
        else {
            unreachable!()
        };
        let omega_assigned_target_operations::AssignedUnitOperation::NormalizedForeignCall {
            scalar_arguments,
            ..
        } = &mut body.operations[4]
        else {
            unreachable!()
        };
        scalar_arguments.push(fourth_target.clone());
        assert!(omega_machine_emission::emit_machine_code(&fifth_assignment).is_err());

        let machine_code = omega_machine_emission::emit_machine_code(&assigned).unwrap();
        let [call] = machine_code.functions[0].foreign_calls.as_slice() else {
            panic!("one retained foreign call")
        };
        let [first_argument, second_argument, third_argument, fourth_argument] =
            call.scalar_arguments.as_slice()
        else {
            panic!("four retained machine arguments")
        };
        assert_eq!(call.locator, locator);
        assert_eq!(call.call_plan, boundary_entry_plan.call);
        for (argument, target_argument) in [
            (first_argument, first_target),
            (second_argument, second_target),
            (third_argument, third_target),
            (fourth_argument, fourth_target),
        ] {
            assert_eq!(argument.source_value, target_argument.source_value);
            assert_eq!(argument.scalar_type, target_argument.scalar_type);
            assert_eq!(argument.immediate, target_argument.immediate);
            assert_eq!(argument.parameter_index, target_argument.parameter_index);
            assert_eq!(argument.placement, target_argument.placement);
            assert!(argument.byte_count > 0);
        }
        assert_eq!(
            first_argument.code_offset + first_argument.byte_count,
            second_argument.code_offset
        );
        assert_eq!(
            second_argument.code_offset + second_argument.byte_count,
            third_argument.code_offset
        );
        assert_eq!(
            third_argument.code_offset + third_argument.byte_count,
            fourth_argument.code_offset
        );

        let mut changed_value = machine_code.clone();
        changed_value.functions[0].foreign_calls[0].scalar_arguments[3].immediate =
            psi_core::IntegerValue::Unsigned(0xa6);
        assert!(omega_image_emission::build_object_artifact(&changed_value).is_err());
        let mut changed_carrier = machine_code.clone();
        changed_carrier.functions[0].foreign_calls[0].scalar_arguments[3].scalar_type =
            psi_core::IntegerType::address(32).unwrap();
        assert!(omega_image_emission::build_object_artifact(&changed_carrier).is_err());
        let mut changed_bytes = machine_code.clone();
        changed_bytes.functions[0].bytes[fourth_argument.code_offset] ^= 1;
        assert!(omega_image_emission::build_object_artifact(&changed_bytes).is_err());
        let mut reordered = machine_code.clone();
        reordered.functions[0].foreign_calls[0]
            .scalar_arguments
            .swap(2, 3);
        assert!(omega_image_emission::build_object_artifact(&reordered).is_err());
        let mut changed_interval = machine_code.clone();
        changed_interval.functions[0].foreign_calls[0].scalar_arguments[3].code_offset -= 1;
        assert!(omega_image_emission::build_object_artifact(&changed_interval).is_err());
        let mut changed_register = machine_code.clone();
        changed_register.functions[0].foreign_calls[0].scalar_arguments[3]
            .placement
            .locations = first_argument.placement.locations.clone();
        assert!(omega_image_emission::build_object_artifact(&changed_register).is_err());
        let mut stripped_custody = machine_code.clone();
        stripped_custody.functions[0].foreign_calls[0]
            .scalar_arguments
            .pop();
        assert!(omega_image_emission::build_object_artifact(&stripped_custody).is_err());
        let mut fifth_argument = machine_code.clone();
        let extra = fifth_argument.functions[0].foreign_calls[0].scalar_arguments[3].clone();
        fifth_argument.functions[0].foreign_calls[0]
            .scalar_arguments
            .push(extra);
        assert!(omega_image_emission::build_object_artifact(&fifth_argument).is_err());

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
}
