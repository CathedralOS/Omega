use super::{
    Arc, IntegerSign, IntegerType, NativeTarget, ScalarType, SelectedInstructionId,
    SelectedInstructionKind, ValueId, VirtualRegisterId, VirtualRegisterOrigin,
    baseline_target_register_environment, fixture, selected_instruction_plan_identity,
};
use crate::RuntimeSpillError;
use crate::ValidatedRuntimeSpill;
use crate::rewrites::runtime_spill::tests::budget;
use crate::spill_selected_runtime_value;
use crate::validate_runtime_spill;
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
    // Address-carrier payloads round-trip through the same eight-byte private
    // storage; the reload register retains the exact address scalar type.
    for bits in [8, 16, 32, 64] {
        scalar_types.push(ScalarType::Integer(IntegerType::address(bits).unwrap()));
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
fn spill_admission_admits_address_carriers_and_rejects_non_gpr_integer_widths() {
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
    // The address carrier alone never widens the admitted widths: payloads
    // that do not fit one GPR keep rejecting.
    for bits in [1, 7, 24, 65, 128] {
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
    // At a GPR width the carrier no longer excludes the victim: the spill and
    // its independent replay accept, and the reload registers retain the exact
    // address scalar type.
    for bits in [8, 16, 32, 64] {
        let scalar_type = ScalarType::Integer(IntegerType::address(bits).unwrap());
        let source = typed_fixture(target, scalar_type);
        let result =
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
                .unwrap();
        assert!(
            result.transformed().functions[0]
                .virtual_registers
                .iter()
                .skip(5)
                .all(|register| matches!(
                    register.origin,
                    VirtualRegisterOrigin::SpillAddress { .. }
                ) || register.scalar_type == scalar_type)
        );
        assert!(
            validate_runtime_spill(
                &source,
                0,
                VirtualRegisterId(1),
                &environment,
                budget(),
                result.transformed().clone()
            )
            .is_ok()
        );
    }
}

#[test]
fn edge_initialized_address_parameters_spill_with_exact_carrier_bindings() {
    let address = ScalarType::Integer(IntegerType::address(64).unwrap());
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let mut source = super::parameters::parameter_fixture(target);
        {
            let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
            // The block parameter, its per-edge copy arguments, and the edge
            // bindings' declared types all carry the exact address carrier.
            for register in &mut function.virtual_registers {
                register.scalar_type = address;
            }
            for block in &mut function.blocks {
                for successor in
                    crate::rewrites::runtime_spill::control_successors_mut(&mut block.terminator)
                        .into_iter()
                        .flatten()
                {
                    for binding in &mut successor.bindings {
                        binding.semantic.scalar_type = address;
                    }
                }
            }
        }
        let identity = selected_instruction_plan_identity(source.transformed());
        source.receipt.source_selected = identity;
        source.receipt.transformed_selected = identity;
        let result =
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
                .unwrap();
        let transformed = &result.transformed().functions[0];
        assert_eq!(transformed.local_storage_slots.len(), 1);
        // Both edge copies gain a following store; every produced reload
        // register retains the address carrier rather than a fixed integer.
        for block_index in [1, 3] {
            assert!(matches!(
                transformed.blocks[block_index].instructions[1].kind,
                SelectedInstructionKind::Store64 { .. }
            ));
        }
        assert!(
            transformed
                .virtual_registers
                .iter()
                .all(|register| register.scalar_type == address
                    || matches!(register.origin, VirtualRegisterOrigin::SpillAddress { .. }))
        );
        assert!(
            validate_runtime_spill(
                &source,
                0,
                VirtualRegisterId(1),
                &environment,
                budget(),
                result.transformed().clone()
            )
            .is_ok()
        );
    }
}
