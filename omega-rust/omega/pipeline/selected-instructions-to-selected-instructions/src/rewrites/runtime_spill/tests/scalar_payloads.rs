use super::*;
use semantic_vocabulary::IeeeFloatFormat;

fn typed_fixture(target: NativeTarget, scalar_type: ScalarType) -> ValidatedRuntimeSpill {
    let mut source = fixture(target);
    for register in &mut Arc::make_mut(&mut source.transformed).functions[0].virtual_registers {
        register.scalar_type = scalar_type;
    }
    let identity = selected_instruction_plan_identity(source.transformed());
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    source
}

#[test]
fn primitive_gpr_spills_preserve_exact_types_and_full_private_storage_on_four_targets() {
    let mut scalar_types = vec![
        ScalarType::Boolean,
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
    ];
    for sign in [IntegerSign::Unsigned, IntegerSign::Signed] {
        for bits in [8, 16, 32, 64] {
            scalar_types.push(ScalarType::Integer(IntegerType::new(sign, bits).unwrap()));
        }
    }
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        for &scalar_type in &scalar_types {
            let source = typed_fixture(target, scalar_type);
            let result = spill_selected_runtime_value(
                &source,
                0,
                VirtualRegisterId(1),
                &environment,
                budget(),
            )
            .unwrap();
            let function = &result.transformed().functions[0];
            assert_eq!(function.local_storage_slots[0].byte_size, 8);
            assert_eq!(function.local_storage_slots[0].alignment, 8);
            for register in function.virtual_registers.iter().skip(5) {
                match register.origin {
                    VirtualRegisterOrigin::SpillAddress { .. } => assert_eq!(
                        register.scalar_type,
                        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
                    ),
                    VirtualRegisterOrigin::InstructionResult { source_value, .. } => {
                        assert_eq!(register.scalar_type, scalar_type);
                        assert_eq!(source_value, ValueId::new(1).unwrap());
                        assert_eq!(
                            register.definition_site,
                            source.transformed().functions[0].virtual_registers[1].definition_site
                        );
                    }
                    _ => panic!("unexpected spill origin"),
                }
            }
            assert!(
                validate_runtime_spill(
                    &source,
                    0,
                    VirtualRegisterId(1),
                    &environment,
                    budget(),
                    result.transformed().clone(),
                )
                .is_ok()
            );
            for mutation in 0..6 {
                let mut forged = result.transformed().clone();
                let function = &mut forged.functions[0];
                match mutation {
                    0 => {
                        function.virtual_registers[6].scalar_type = if scalar_type
                            == ScalarType::Boolean
                        {
                            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap())
                        } else {
                            ScalarType::Boolean
                        }
                    }
                    1 => {
                        function.blocks[0].instructions[3].kind =
                            SelectedInstructionKind::Load8 { byte_offset: 0 }
                    }
                    2 => {
                        function.blocks[0].instructions[3].kind =
                            SelectedInstructionKind::Load16 { byte_offset: 0 }
                    }
                    3 => {
                        function.blocks[0].instructions[3].kind =
                            SelectedInstructionKind::Load32 { byte_offset: 0 }
                    }
                    4 => function.local_storage_slots[0].byte_size = 1,
                    5 => {
                        function.virtual_registers[6].origin =
                            VirtualRegisterOrigin::InstructionResult {
                                instruction: SelectedInstructionId(8),
                                source_value: ValueId::new(2).unwrap(),
                            }
                    }
                    _ => unreachable!(),
                }
                assert!(
                    validate_runtime_spill(
                        &source,
                        0,
                        VirtualRegisterId(1),
                        &environment,
                        budget(),
                        forged,
                    )
                    .is_err(),
                    "{target:?} {scalar_type:?} mutation {mutation}"
                );
            }
        }
    }
}

#[test]
fn spill_admission_does_not_extend_to_address_or_other_integer_widths() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for sign in [IntegerSign::Unsigned, IntegerSign::Signed] {
        for bits in [1, 7, 24, 65, 128] {
            let source = typed_fixture(
                target,
                ScalarType::Integer(IntegerType::new(sign, bits).unwrap()),
            );
            assert_eq!(
                spill_selected_runtime_value(
                    &source,
                    0,
                    VirtualRegisterId(1),
                    &environment,
                    budget(),
                )
                .unwrap_err(),
                RuntimeSpillError::UnsupportedValue
            );
        }
    }
    for bits in [8, 16, 32, 64] {
        let source = typed_fixture(
            target,
            ScalarType::Integer(IntegerType::address(bits).unwrap()),
        );
        assert_eq!(
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget(),)
                .unwrap_err(),
            RuntimeSpillError::UnsupportedValue
        );
    }
}
