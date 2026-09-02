use super::*;

#[test]
fn every_fixed_integer_foreign_result_flows_through_a_durable_home_to_a_later_import() {
    for (sign, bits, type_identity) in [
        (psi_core::IntegerSign::Signed, 8_u16, "i8"),
        (psi_core::IntegerSign::Unsigned, 8, "u8"),
        (psi_core::IntegerSign::Signed, 16, "i16"),
        (psi_core::IntegerSign::Unsigned, 16, "u16"),
        (psi_core::IntegerSign::Signed, 32, "i32"),
        (psi_core::IntegerSign::Unsigned, 32, "u32"),
        (psi_core::IntegerSign::Signed, 64, "i64"),
        (psi_core::IntegerSign::Unsigned, 64, "u64"),
    ] {
        let integer = psi_core::IntegerType::new(sign, bits).unwrap();
        for profile in [
            omega_target::TargetProfile::LinuxX64,
            omega_target::TargetProfile::LinuxArm64,
        ] {
            assert_exact_fixed_integer_foreign_result_flow(profile, integer, type_identity);
        }
    }
}

fn assert_exact_fixed_integer_foreign_result_flow(
    profile: omega_target::TargetProfile,
    integer: psi_core::IntegerType,
    type_identity: &str,
) {
    let target = profile.native_target();
    let producer_requirement = format!("omega::test::Foreign::produce_{type_identity}()");
    let consumer_requirement =
        format!("omega::test::Foreign::consume_{type_identity}({type_identity})");
    let producer_symbol = format!("produce_{type_identity}");
    let consumer_symbol = format!("consume_{type_identity}");
    let producer_boundary = psi_core::BoundaryMachineId::new(830).unwrap();
    let consumer_boundary = psi_core::BoundaryMachineId::new(831).unwrap();
    let producer_plan = configured_plan(
        producer_symbol.as_bytes(),
        profile,
        "produce",
        &producer_requirement,
        type_identity,
        0,
        true,
    );
    let consumer_plan = configured_plan(
        consumer_symbol.as_bytes(),
        profile,
        "consume",
        &consumer_requirement,
        type_identity,
        1,
        false,
    );
    let shape = integer_shape(integer);
    let producer_call_plan = evaluated_plan(target, Vec::new(), Some(shape));
    let consumer_call_plan = evaluated_plan(target, vec![shape], None);
    let producer_external = external_row(
        &producer_plan,
        "produce",
        &producer_requirement,
        producer_call_plan.clone(),
    );
    let consumer_external = external_row(
        &consumer_plan,
        "consume",
        &consumer_requirement,
        consumer_call_plan.clone(),
    );
    let producer_stack = same_stack(&producer_plan, &producer_requirement, 830);
    let consumer_stack = same_stack(&consumer_plan, &consumer_requirement, 831);
    let producer_report = producer_plan.report_fingerprint();
    let consumer_report = consumer_plan.report_fingerprint();
    let producer_foreign = rejoin_normalized_foreign_call(
        &producer_plan,
        &[producer_external],
        &producer_stack,
        producer_report,
        &producer_requirement,
        target,
    )
    .unwrap();
    let consumer_foreign = rejoin_normalized_foreign_call(
        &consumer_plan,
        &[consumer_external],
        &consumer_stack,
        consumer_report,
        &consumer_requirement,
        target,
    )
    .unwrap();
    let producer_execution = TestProviderExecution {
        requirement: producer_requirement.clone(),
        provider_plan_report_identity: producer_report,
    };
    let consumer_execution = TestProviderExecution {
        requirement: consumer_requirement.clone(),
        provider_plan_report_identity: consumer_report,
    };

    let target_plan = omega_abstract_operations_to_target_operations::lower_to_target_operations_with_provider_executions(
        &abstract_plan(
            producer_boundary,
            consumer_boundary,
            integer,
            &producer_requirement,
            &consumer_requirement,
        ),
        target,
        &[
            omega_abstract_operations_to_target_operations::AdmittedBoundarySettlement {
                boundary: producer_boundary,
                execution: omega_abstract_operations_to_target_operations::AdmittedBoundaryExecution::Provider(&producer_execution),
                realization: omega_target_operations::BoundarySettlementRealization::NormalizedForeignCall(producer_foreign),
            },
            omega_abstract_operations_to_target_operations::AdmittedBoundarySettlement {
                boundary: consumer_boundary,
                execution: omega_abstract_operations_to_target_operations::AdmittedBoundaryExecution::Provider(&consumer_execution),
                realization: omega_target_operations::BoundarySettlementRealization::NormalizedForeignCall(consumer_foreign),
            },
        ],
    )
    .unwrap();
    let omega_target_operations::TargetOperation::UnitBody(target_body) =
        &target_plan.functions[0].operation
    else {
        panic!("foreign result dataflow lowers in one attached Unit body")
    };
    let omega_target_operations::TargetUnitOperation::NormalizedForeignCall {
        result_home: Some(target_home),
        scalar_arguments: producer_arguments,
        binding: producer_binding,
        ..
    } = &target_body.operations[0]
    else {
        panic!("producer owns one target scalar home")
    };
    let omega_target_operations::TargetUnitOperation::NormalizedForeignCall {
        result_home: None,
        scalar_arguments: consumer_arguments,
        ..
    } = &target_body.operations[1]
    else {
        panic!("consumer remains Unit-returning")
    };
    assert!(producer_arguments.is_empty());
    assert_eq!(consumer_arguments.len(), 1);
    assert_eq!(
        target_home.scalar_type,
        psi_core::ScalarType::Integer(integer)
    );
    assert_eq!(target_home.shape, shape);
    assert_eq!(
        consumer_arguments[0].source,
        omega_target_operations::TargetUnitScalarArgumentSource::Home(*target_home)
    );
    assert_eq!(
        producer_call_plan.call.result.as_ref(),
        producer_binding.boundary_entry_plan.call.result.as_ref()
    );
    assert!(matches!(
        producer_call_plan.call.result.as_ref().unwrap().locations.as_slice(),
        [omega_calling_conventions::ValueLocation::Register {
            register,
            value_byte_offset: 0,
            byte_size,
        }] if *register == expected_result_register(target.architecture)
            && *byte_size == shape.byte_size
    ));

    let assigned =
        omega_target_operations_to_assigned_target_operations::assign_registers(&target_plan)
            .unwrap();
    let omega_assigned_target_operations::AssignedOperation::UnitBody(assigned_body) =
        &assigned.functions[0].operation
    else {
        unreachable!()
    };
    let omega_assigned_target_operations::AssignedUnitOperation::NormalizedForeignCall {
        result_home: Some(assigned_home),
        ..
    } = &assigned_body.operations[0]
    else {
        unreachable!()
    };
    let omega_assigned_target_operations::AssignedUnitOperation::NormalizedForeignCall {
        scalar_arguments,
        ..
    } = &assigned_body.operations[1]
    else {
        unreachable!()
    };
    assert_eq!(assigned_home.byte_offset, 0);
    assert_eq!(
        assigned_home.scalar_type,
        psi_core::ScalarType::Integer(integer)
    );
    assert_eq!(assigned_home.shape, shape);
    assert_eq!(
        scalar_arguments[0].source,
        omega_assigned_target_operations::AssignedUnitScalarArgumentSource::Home(*assigned_home)
    );

    let mut missing_assigned_result = assigned.clone();
    let omega_assigned_target_operations::AssignedOperation::UnitBody(body) =
        &mut missing_assigned_result.functions[0].operation
    else {
        unreachable!()
    };
    let omega_assigned_target_operations::AssignedUnitOperation::NormalizedForeignCall {
        result_home,
        ..
    } = &mut body.operations[0]
    else {
        unreachable!()
    };
    *result_home = None;
    assert!(omega_machine_emission::emit_machine_code(&missing_assigned_result).is_err());

    let machine_code = omega_machine_emission::emit_machine_code(&assigned).unwrap();
    let function = &machine_code.functions[0];
    let [producer, consumer] = function.foreign_calls.as_slice() else {
        panic!("two normalized foreign calls survive emission")
    };
    let result = producer
        .scalar_result
        .as_ref()
        .expect("producer retains scalar result custody");
    let [argument] = consumer.scalar_arguments.as_slice() else {
        panic!("consumer retains one scalar argument")
    };
    assert_eq!(producer.call_plan, producer_call_plan.call);
    assert_eq!(consumer.call_plan, consumer_call_plan.call);
    assert_eq!(result.home, function.unit_scalar_homes[0]);
    assert_eq!(
        result.home.scalar_type,
        psi_core::ScalarType::Integer(integer)
    );
    assert_eq!(result.home.shape, shape);
    assert_eq!(
        argument.source,
        omega_machine_code::InternalUnitScalarArgumentSourceRecord::Home(result.home)
    );
    assert_eq!(
        &function.bytes[result.code_offset..result.code_offset + result.byte_count],
        expected_result_bytes(target.architecture, integer)
    );
    assert_eq!(
        &function.bytes[argument.code_offset..argument.code_offset + argument.byte_count],
        expected_argument_bytes(target.architecture)
    );
    match target.architecture {
        omega_target::Architecture::X86_64 => {
            assert_eq!(producer.aarch64_floating_control, None);
            assert_eq!(consumer.aarch64_floating_control, None);
            let producer_control = producer
                .x86_floating_control
                .expect("x86 producer preserves MXCSR");
            let consumer_control = consumer
                .x86_floating_control
                .expect("x86 consumer preserves MXCSR");
            assert_eq!(
                producer_control.saved_slot_byte_offset,
                consumer_control.saved_slot_byte_offset
            );
            assert_eq!(
                result.code_offset,
                producer_control.restore_offset + producer_control.restore_byte_count
            );
            assert!(
                producer_control.restore_offset + producer_control.restore_byte_count
                    <= consumer_control.save_offset
            );
        }
        omega_target::Architecture::Aarch64 => {
            assert_eq!(producer.x86_floating_control, None);
            assert_eq!(consumer.x86_floating_control, None);
            let producer_control = producer
                .aarch64_floating_control
                .expect("AArch64 producer preserves FPCR");
            let consumer_control = consumer
                .aarch64_floating_control
                .expect("AArch64 consumer preserves FPCR");
            assert_eq!(
                producer_control.saved_slot_byte_offset,
                consumer_control.saved_slot_byte_offset
            );
            assert_eq!(
                result.code_offset,
                producer_control.restore_offset + producer_control.restore_byte_count
            );
            assert!(
                producer_control.restore_offset + producer_control.restore_byte_count
                    <= consumer_control.save_offset
            );
        }
    }

    let mut stripped_result = machine_code.clone();
    stripped_result.functions[0].foreign_calls[0].scalar_result = None;
    assert!(omega_image_emission::build_object_artifact(&stripped_result).is_err());
    let mut stripped_home = machine_code.clone();
    stripped_home.functions[0].unit_scalar_homes.clear();
    assert!(omega_image_emission::build_object_artifact(&stripped_home).is_err());
    let mut changed_result_home = machine_code.clone();
    changed_result_home.functions[0].foreign_calls[0]
        .scalar_result
        .as_mut()
        .unwrap()
        .home
        .source_value = psi_core::ValueId::new(839).unwrap();
    assert!(omega_image_emission::build_object_artifact(&changed_result_home).is_err());
    let mut changed_result_type = machine_code.clone();
    changed_result_type.functions[0].foreign_calls[0]
        .scalar_result
        .as_mut()
        .unwrap()
        .home
        .scalar_type = psi_core::ScalarType::Integer(opposite_sign(integer));
    assert!(omega_image_emission::build_object_artifact(&changed_result_type).is_err());
    let mut changed_result_shape = machine_code.clone();
    changed_result_shape.functions[0].foreign_calls[0]
        .scalar_result
        .as_mut()
        .unwrap()
        .home
        .shape
        .alignment = shape.alignment.saturating_add(1);
    assert!(omega_image_emission::build_object_artifact(&changed_result_shape).is_err());
    let mut changed_result_bytes = machine_code.clone();
    changed_result_bytes.functions[0].bytes[result.code_offset] ^= 1;
    assert!(omega_image_emission::build_object_artifact(&changed_result_bytes).is_err());
    let mut changed_result_interval = machine_code.clone();
    changed_result_interval.functions[0].foreign_calls[0]
        .scalar_result
        .as_mut()
        .unwrap()
        .code_offset += 1;
    assert!(omega_image_emission::build_object_artifact(&changed_result_interval).is_err());
    let mut changed_result_placement = machine_code.clone();
    let changed_result = changed_result_placement.functions[0].foreign_calls[0]
        .scalar_result
        .as_mut()
        .unwrap();
    let omega_calling_conventions::ValueLocation::Register { byte_size, .. } =
        &mut changed_result.source.locations[0]
    else {
        unreachable!()
    };
    *byte_size = byte_size.saturating_add(1);
    assert!(omega_image_emission::build_object_artifact(&changed_result_placement).is_err());
    let mut duplicate_home = machine_code.clone();
    duplicate_home.functions[0]
        .unit_scalar_homes
        .push(result.home);
    assert!(omega_image_emission::build_object_artifact(&duplicate_home).is_err());
    let mut changed_consumer_home = machine_code.clone();
    let omega_machine_code::InternalUnitScalarArgumentSourceRecord::Home(home) =
        &mut changed_consumer_home.functions[0].foreign_calls[1].scalar_arguments[0].source
    else {
        unreachable!()
    };
    home.byte_offset += 8;
    assert!(omega_image_emission::build_object_artifact(&changed_consumer_home).is_err());
    let mut changed_ordinal = machine_code.clone();
    changed_ordinal.functions[0].foreign_calls[1].operation_ordinal = 0;
    assert!(omega_image_emission::build_object_artifact(&changed_ordinal).is_err());
    let mut changed_producer_ordinal = machine_code.clone();
    changed_producer_ordinal.functions[0].foreign_calls[0].operation_ordinal = 1;
    assert!(omega_image_emission::build_object_artifact(&changed_producer_ordinal).is_err());

    let object = omega_image_emission::build_object_artifact(&machine_code).unwrap();
    assert_eq!(object.foreign_calls().len(), 2);
    assert_eq!(
        object.foreign_calls()[0].operation_ordinal,
        machine_code.functions[0].foreign_calls[0].operation_ordinal,
    );
    let [object_argument] = object.foreign_calls()[1].scalar_arguments.as_slice() else {
        panic!("expected one retained object scalar argument")
    };
    let [machine_argument] = machine_code.functions[0].foreign_calls[1]
        .scalar_arguments
        .as_slice()
    else {
        panic!("expected one emitted machine scalar argument")
    };
    assert_eq!(
        object_argument.parameter_index,
        machine_argument.parameter_index
    );
    assert_eq!(object_argument.source, machine_argument.source);
    assert_eq!(object_argument.placement, machine_argument.placement);
    assert_eq!(object_argument.byte_count, machine_argument.byte_count);
    assert_eq!(
        object_argument.code_offset,
        object.functions()[0].text_offset + machine_argument.code_offset,
    );
    assert!(object.foreign_calls()[0].scalar_result.is_some());
    assert_eq!(
        object.foreign_calls()[0].x86_floating_control.is_some(),
        target.architecture == omega_target::Architecture::X86_64
    );
    assert_eq!(
        object.foreign_calls()[0].aarch64_floating_control.is_some(),
        target.architecture == omega_target::Architecture::Aarch64
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
    assert_eq!(image.output().final_image_imports, 2);
    assert_eq!(image.output().final_image_relocations, 2);
}

fn integer_shape(integer: psi_core::IntegerType) -> omega_calling_conventions::ValueShape {
    let bytes = integer.bits().div_ceil(8);
    omega_calling_conventions::ValueShape::integer(bytes, bytes.next_power_of_two().min(8))
}

fn opposite_sign(integer: psi_core::IntegerType) -> psi_core::IntegerType {
    psi_core::IntegerType::new(
        match integer.sign() {
            psi_core::IntegerSign::Signed => psi_core::IntegerSign::Unsigned,
            psi_core::IntegerSign::Unsigned => psi_core::IntegerSign::Signed,
        },
        integer.bits(),
    )
    .unwrap()
}

fn expected_result_register(
    architecture: omega_target::Architecture,
) -> omega_target_operations::MachineRegister {
    match architecture {
        omega_target::Architecture::X86_64 => omega_target_operations::MachineRegister::X86Rax,
        omega_target::Architecture::Aarch64 => {
            omega_target_operations::MachineRegister::Aarch64X(0)
        }
    }
}

fn evaluated_plan(
    target: omega_target::NativeTarget,
    parameters: Vec<omega_calling_conventions::ValueShape>,
    result: Option<omega_calling_conventions::ValueShape>,
) -> omega_calling_conventions::BoundaryEntryPlan {
    omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
        omega_calling_conventions::CallingPolicy::native_for_target(target),
        &omega_calling_conventions::CallSignature { parameters, result },
    )
    .unwrap()
    .plan()
    .clone()
}

fn configured_plan(
    symbol: &[u8],
    profile: omega_target::TargetProfile,
    method: &str,
    requirement: &str,
    type_identity: &str,
    parameter_count: usize,
    has_result: bool,
) -> ProviderPlan {
    let mut plan = import_plan(symbol, profile);
    plan.name = format!("omega::test::{method}-provider");
    let schema = &mut plan.schema.methods[0];
    schema.name = method.into();
    schema.requirement_identity = requirement.into();
    schema.parameter_count = parameter_count;
    schema.parameter_type_identities = if parameter_count == 0 {
        Vec::new()
    } else {
        vec![type_identity.into()]
    };
    schema.has_result = has_result;
    schema.result_type_identity = has_result.then(|| type_identity.into());
    let row = &mut plan.rows[0];
    row.method = method.into();
    row.requirement_identity = requirement.into();
    plan
}

fn external_row(
    plan: &ProviderPlan,
    method: &str,
    requirement: &str,
    boundary_entry_plan: omega_calling_conventions::BoundaryEntryPlan,
) -> omega_calling_conventions::ExternalBindingRow {
    let locator = match &plan.rows[0].binding {
        ProviderBinding::Import { evaluated } => evaluated.locator().clone(),
        _ => unreachable!(),
    };
    omega_calling_conventions::ExternalBindingRow {
        target_name: plan.target.clone(),
        trait_name: plan.schema.trait_name.clone(),
        method: method.into(),
        requirement_identity: requirement.into(),
        table_type: String::new(),
        boundary_entry_plan: Some(boundary_entry_plan),
        binding: omega_calling_conventions::ExternalBindingKind::Import { locator },
    }
}

fn same_stack(
    plan: &ProviderPlan,
    requirement: &str,
    receipt: u64,
) -> omega_task_plans::AdmittedSameStackContribution {
    let report = plan.report_fingerprint();
    let commitment = omega_task_plans::SameStackProviderPlanCommitment::from_digest(
        *plan.identity_digest().as_bytes(),
    );
    omega_task_plans::admit_same_stack_contribution(
        omega_task_plans::SameStackContributionAdmissionCandidate {
            provider_plan_report_identity: report,
            provider_plan_commitment: commitment,
            requirement_identity: requirement.into(),
            receipt:
                omega_task_plans::SameStackContributionAdmissionReceiptId::from_normalized_identity(
                    receipt,
                )
                .unwrap(),
            bytes: 64,
            alignment: 16,
        },
        report,
        commitment,
        requirement,
    )
    .unwrap()
}

fn abstract_plan(
    producer: psi_core::BoundaryMachineId,
    consumer: psi_core::BoundaryMachineId,
    integer: psi_core::IntegerType,
    producer_requirement: &str,
    consumer_requirement: &str,
) -> omega_abstract_operations::AbstractOperationPlan {
    let machine = psi_core::MachineId::new(830).unwrap();
    let block = psi_core::BlockId::new(830).unwrap();
    let runtime = psi_core::ValueId::new(830).unwrap();
    let scalar_type = psi_core::ScalarType::Integer(integer);
    omega_abstract_operations::AbstractOperationPlan {
        psi: psi_terminal::TerminalPsiIdentity {
            vocabulary_marker: psi_terminal::VocabularyMarker::CURRENT,
            program_fingerprint: psi_terminal::SemanticFingerprint::from_bytes([0x83; 32]),
        },
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: vec![
            psi_terminal::BoundaryMachineDeclaration {
                id: producer,
                identity: producer_requirement.into(),
                attachment: None,
                scalar_parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: Some(scalar_type),
                requires: Vec::new(),
                program_local_root_introductions: Vec::new(),
                content_guarantees: Vec::new(),
                published_service_ceiling: Vec::new(),
            },
            psi_terminal::BoundaryMachineDeclaration {
                id: consumer,
                identity: consumer_requirement.into(),
                attachment: None,
                scalar_parameters: vec![scalar_type],
                structural_parameters: Vec::new(),
                result: None,
                requires: Vec::new(),
                program_local_root_introductions: Vec::new(),
                content_guarantees: Vec::new(),
                published_service_ceiling: Vec::new(),
            },
        ],
        provider_candidates: Vec::new(),
        functions: vec![omega_abstract_operations::AbstractFunction {
            machine,
            attachment: Some(psi_core::StructuralTypeId::new(830).unwrap()),
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
                omega_abstract_operations::AbstractOperation::BoundaryCall {
                    psi_operation: psi_core::OperationId::new(830).unwrap(),
                    result: Some(omega_abstract_operations::AbstractResult {
                        value: runtime,
                        scalar_type,
                    }),
                    boundary: producer,
                    arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    completion_claim_sources: Vec::new(),
                    completion_receipts: Vec::new(),
                },
                omega_abstract_operations::AbstractOperation::BoundaryCall {
                    psi_operation: psi_core::OperationId::new(831).unwrap(),
                    result: None,
                    boundary: consumer,
                    arguments: vec![runtime],
                    structural_arguments: Vec::new(),
                    completion_claim_sources: Vec::new(),
                    completion_receipts: Vec::new(),
                },
                omega_abstract_operations::AbstractOperation::ReturnUnit {
                    psi_edge: psi_core::EdgeId::new(830).unwrap(),
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    }
}

fn expected_result_bytes(
    architecture: omega_target::Architecture,
    integer: psi_core::IntegerType,
) -> Vec<u8> {
    match architecture {
        omega_target::Architecture::X86_64 => {
            let mut bytes = match (integer.sign(), integer.bits()) {
                (psi_core::IntegerSign::Signed, 8) => vec![0x48, 0x0f, 0xbe, 0xc0],
                (psi_core::IntegerSign::Signed, 16) => vec![0x48, 0x0f, 0xbf, 0xc0],
                (psi_core::IntegerSign::Signed, 32) => vec![0x48, 0x63, 0xc0],
                (psi_core::IntegerSign::Unsigned, 8) => vec![0x40, 0x0f, 0xb6, 0xc0],
                (psi_core::IntegerSign::Unsigned, 16) => vec![0x40, 0x0f, 0xb7, 0xc0],
                (psi_core::IntegerSign::Unsigned, 32) => vec![0x40, 0x89, 0xc0],
                (_, 64) => Vec::new(),
                _ => unreachable!(),
            };
            bytes.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x00]);
            bytes
        }
        omega_target::Architecture::Aarch64 => {
            let mut bytes = Vec::new();
            if integer.bits() != 64 {
                let base = match integer.sign() {
                    psi_core::IntegerSign::Signed => 0x9340_0000,
                    psi_core::IntegerSign::Unsigned => 0xd340_0000,
                };
                let instruction = base | (u32::from(integer.bits() - 1) << 10);
                bytes.extend_from_slice(&instruction.to_le_bytes());
            }
            bytes.extend_from_slice(&0xf900_03e0_u32.to_le_bytes());
            bytes
        }
    }
}

fn expected_argument_bytes(architecture: omega_target::Architecture) -> &'static [u8] {
    match architecture {
        omega_target::Architecture::X86_64 => &[0x48, 0x8b, 0x7c, 0x24, 0x08],
        omega_target::Architecture::Aarch64 => &[0xe0, 0x03, 0x40, 0xf9],
    }
}
