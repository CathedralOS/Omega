//! Scalar inputs retain exact ABI types and canonical literal values, even when unused.
use super::*;

#[test]
fn unused_stack_parameters_keep_abi_without_inventing_entry_transport() {
    for (target, capacity) in [
        (target::NativeTarget::linux_x64(), 6),
        (target::NativeTarget::linux_arm64(), 8),
        (target::NativeTarget::windows_x64(), 4),
        (target::NativeTarget::macos_arm64(), 8),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let mut source = fixture(target, 0);
        source.attachment = None;
        source.blocks[0].instructions.truncate(2);
        source.provenance.operations.truncate(2);
        source.call_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![ValueShape::integer(8, 8); capacity + 1],
                result: Some(ValueShape::integer(8, 8)),
            },
        )
        .unwrap();
        let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        source.parameters = source
            .call_plan
            .parameters
            .iter()
            .enumerate()
            .map(|(parameter_index, placement)| LegalizedScalarParameter {
                value: ValueId::new(100 + parameter_index as u64).unwrap(),
                scalar_type: ScalarType::Integer(integer),
                definition_site: ValueDefinitionSite::FunctionParameter(parameter_index as u32),
                placement: placement.clone(),
            })
            .collect();
        let parameter_index = capacity - 1;
        let used = &source.parameters[parameter_index];
        let [ValueLocation::Register { register, .. }] = used.placement.locations.as_slice() else {
            panic!("last register argument");
        };
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            projected_structural_call: None,
            fixed_inputs: vec![SelectedFixedInputConstraint {
                machine: source.machine,
                source_value: used.value,
                parameter_index,
                register: *register,
                fixed_view: environment.fixed_register_view(*register).unwrap(),
            }],
        };
        returned(&mut source.blocks[0]).value = LegalizedScalarReturnValue::Value {
            value: used.value,
            scalar_type: integer,
        };
        let selected = build(
            0,
            &source,
            target,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        crate::selection::validation::scalar_graph::validate(
            0,
            &source,
            &selected,
            target,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        assert!(matches!(selected.virtual_registers[0].origin,
            VirtualRegisterOrigin::EntryParameter { parameter_index: actual,.. } if actual==parameter_index));
        assert_eq!(
            selected
                .virtual_registers
                .iter()
                .filter(|value| matches!(
                    value.origin,
                    VirtualRegisterOrigin::EntryParameter { .. }
                ))
                .count(),
            1
        );
        // The same canonical stack placement becomes unsupported when actually read.
        assert!(matches!(
            source.parameters[capacity].placement.locations.as_slice(),
            [ValueLocation::Stack { .. }]
        ));
        returned(&mut source.blocks[0]).value = LegalizedScalarReturnValue::Value {
            value: source.parameters[capacity].value,
            scalar_type: integer,
        };
        assert!(
            build(
                0,
                &source,
                target,
                &constraints,
                environment.physical(),
                environment.constraints()
            )
            .is_err()
        );
        assert!(
            crate::selection::validation::scalar_graph::validate(
                0,
                &source,
                &selected,
                target,
                &constraints,
                environment.physical(),
                environment.constraints()
            )
            .is_err()
        );
    }
}

#[test]
fn u32_entry_snapshot_normalizes_bits_and_replay_rejects_copy_substitution() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let mut source = fixture(target, 0);
        let integer = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
        let scalar_type = ScalarType::Integer(integer);
        let input = ValueId::new(100).unwrap();
        source.call_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![ValueShape::integer(4, 4)],
                result: None,
            },
        )
        .unwrap();
        let placement = source.call_plan.parameters[0].clone();
        let [ValueLocation::Register { register, .. }] = placement.locations.as_slice() else {
            panic!("register argument");
        };
        let register = *register;
        source.parameters = vec![LegalizedScalarParameter {
            value: input,
            scalar_type,
            definition_site: ValueDefinitionSite::FunctionParameter(0),
            placement,
        }];
        source.blocks[0].instructions.truncate(2);
        source.provenance.operations.truncate(2);
        for row in &mut source.blocks[0].instructions {
            row.result.as_mut().unwrap().scalar_type = scalar_type;
        }
        source.blocks[0].instructions[0].kind =
            LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(0));
        source.blocks[0].instructions[1].kind = LegalizedScalarInstructionKind::ExactBinary {
            operator: legalized_operations::LegalizedExactIntegerOperator::Subtract,
            left: input,
            right: ValueId::new(1).unwrap(),
            obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
            accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes([17; 32]),
        };
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            projected_structural_call: None,
            fixed_inputs: vec![SelectedFixedInputConstraint {
                machine: source.machine,
                source_value: input,
                parameter_index: 0,
                register,
                fixed_view: environment.fixed_register_view(register).unwrap(),
            }],
        };
        let selected = build(
            0,
            &source,
            target,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        let validate = |proposed: &SelectedFunction| {
            crate::selection::validation::scalar_graph::validate(
                0,
                &source,
                proposed,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        validate(&selected).unwrap();
        assert_eq!(
            selected.blocks[0].instructions[0].kind,
            SelectedInstructionKind::ZeroExtendU32
        );
        assert_eq!(selected.virtual_registers[0].scalar_type, scalar_type);
        for corruption in 0..3 {
            let mut changed = selected.clone();
            match corruption {
                0 => changed.blocks[0].instructions[0].kind = SelectedInstructionKind::CopyI64,
                1 => {
                    changed.blocks[0].instructions[0].operands[0].virtual_register =
                        VirtualRegisterId(1)
                }
                _ => changed.blocks[0].instructions[0].provenance.values.clear(),
            }
            assert!(
                validate(&changed).is_err(),
                "normalization corruption {corruption}"
            );
        }
    }
}

#[test]
fn raw_boolean_constants_reject_unsigned_two_before_selection() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            projected_structural_call: None,
            fixed_inputs: Vec::new(),
        };
        let mut source = fixture(target, 0);
        source.blocks[0].instructions.truncate(1);
        source.provenance.operations.truncate(1);
        source.blocks[0].instructions[0]
            .result
            .as_mut()
            .unwrap()
            .scalar_type = ScalarType::Boolean;
        let construct = |source: &LegalizedScalarFunction| {
            build(
                0,
                source,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        for value in [0, 1] {
            source.blocks[0].instructions[0].kind =
                LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(value));
            let selected = construct(&source).unwrap();
            crate::selection::validation::scalar_graph::validate(
                0,
                &source,
                &selected,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            let mut invalid = source.clone();
            invalid.blocks[0].instructions[0].kind =
                LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(2));
            assert!(construct(&invalid).is_err());
            assert!(
                crate::selection::validation::scalar_graph::validate(
                    0,
                    &invalid,
                    &selected,
                    target,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                )
                .is_err()
            );
        }
    }
}
