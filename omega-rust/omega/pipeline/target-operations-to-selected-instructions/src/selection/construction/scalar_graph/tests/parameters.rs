//! ABI declarations may remain without physical transport when unused.
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
                scalar_type: integer,
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
