//! Mixed call operands and integer result fragments retain exact encoding effects.
use super::*;

#[test]
fn mixed_aggregate_templates_replay_inputs_results_and_clobbers() {
    let (physical, _, _, _, _) = inputs();
    let constraints = crate::validate_x86_64_register_constraint_catalog(
        crate::x86_64_register_constraint_catalog(&physical),
        &physical,
    )
    .unwrap();
    let kind = SelectedInstructionKind::CallAggregate {
        callee: MachineId::new(7).unwrap(),
    };
    for (native, keys) in [
        (
            NativeTarget::linux_x64(),
            crate::x86_64_system_v_mixed_aggregate_call_keys(),
        ),
        (
            NativeTarget::windows_x64(),
            crate::x86_64_microsoft_mixed_aggregate_call_keys(),
        ),
    ] {
        let catalog = crate::x86_64_machine_effect_catalog(native, &constraints).unwrap();
        for key in keys {
            let declaration = catalog
                .declarations
                .iter()
                .find(|declaration| {
                    declaration.constraint == key
                        && declaration.semantic
                            == selected_instructions::MachineSemanticKind::CallAggregate
                })
                .unwrap();
            let alternative = &declaration.alternatives[0];
            let row = constraints
                .catalog()
                .constraints
                .iter()
                .find(|row| row.key == key)
                .unwrap();
            let operands = row
                .operands
                .iter()
                .map(|operand| operand.fixed_view.unwrap())
                .collect::<Vec<_>>();
            let template = encode_x86_64_selected_scalar_call_template(
                native,
                &physical,
                kind,
                alternative.key,
                &operands,
                &alternative.encoded,
            )
            .unwrap();
            assert_eq!(template.bytes(), &[0xe8, 0, 0, 0, 0]);
            assert_eq!(
                template.fixup(),
                canonical_fixup(MachineId::new(7).unwrap())
            );
            validate_x86_64_selected_scalar_call_template(
                native,
                &physical,
                kind,
                alternative.key,
                &operands,
                &alternative.encoded,
                template.bytes(),
                template.fixup(),
            )
            .unwrap();
            let mut changed = alternative.encoded.clone();
            changed
                .implicit_unit_clobbers
                .extend(&physical.model().view_named("rax").unwrap().write_units);
            changed.implicit_unit_clobbers.sort_unstable();
            assert!(
                encode_x86_64_selected_scalar_call_template(
                    native,
                    &physical,
                    kind,
                    alternative.key,
                    &operands,
                    &changed,
                )
                .is_err()
            );
            let mut changed = operands.clone();
            *changed.last_mut().unwrap() = physical.model().view_named("r11").unwrap().id;
            assert!(
                encode_x86_64_selected_scalar_call_template(
                    native,
                    &physical,
                    kind,
                    alternative.key,
                    &changed,
                    &alternative.encoded,
                )
                .is_err()
            );
            let mut changed = operands.clone();
            changed[0] = physical.model().view_named("xmm15").unwrap().id;
            assert!(
                encode_x86_64_selected_scalar_call_template(
                    native,
                    &physical,
                    kind,
                    alternative.key,
                    &changed,
                    &alternative.encoded,
                )
                .is_err()
            );
        }
    }
}
