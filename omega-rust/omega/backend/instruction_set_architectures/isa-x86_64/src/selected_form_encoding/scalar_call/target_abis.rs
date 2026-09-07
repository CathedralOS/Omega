use super::*;
use register_model::validate_physical_register_model;

#[test]
fn target_register_arities_encode_and_reject_opposite_abi_effects() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let constraints = crate::validate_x86_64_register_constraint_catalog(
        x86_64_register_constraint_catalog(&physical),
        &physical,
    )
    .unwrap();
    assert!(
        crate::X86_64_REQUIRED_REGISTER_CONSTRAINTS
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    );
    let cases: [(NativeTarget, &[&str]); 2] = [
        (
            NativeTarget::linux_x64(),
            &["rdi", "rsi", "rdx", "rcx", "r8", "r9"],
        ),
        (NativeTarget::windows_x64(), &["rcx", "rdx", "r8", "r9"]),
    ];
    let kind = SelectedInstructionKind::CallI64 {
        callee: MachineId::new(7).unwrap(),
    };
    let alternative = MachineAlternativeKey {
        family: MachineAlternativeFamily::CallI64,
        variant: 0,
    };
    for (target, arguments) in cases {
        let catalog = crate::x86_64_machine_effect_catalog(target, &constraints).unwrap();
        for arity in 0..=arguments.len() {
            let operands = arguments[..arity]
                .iter()
                .copied()
                .chain(["rax"])
                .map(|name| physical.model().view_named(name).unwrap().id)
                .collect::<Vec<_>>();
            let effects = &catalog
                .declarations
                .iter()
                .find(|row| {
                    row.semantic == selected_instructions::MachineSemanticKind::CallI64
                        && row.alternatives[0].encoded.external_operand_reads.len() == arity
                })
                .unwrap()
                .alternatives[0]
                .encoded;
            let template = encode_x86_64_selected_scalar_call_template(
                target,
                &physical,
                kind,
                alternative,
                &operands,
                effects,
            )
            .unwrap();
            assert_eq!(
                validate_x86_64_selected_scalar_call_template(
                    target,
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    effects,
                    template.bytes(),
                    template.fixup(),
                )
                .unwrap(),
                template
            );
            let opposite = if target == NativeTarget::linux_x64() {
                NativeTarget::windows_x64()
            } else {
                NativeTarget::linux_x64()
            };
            let mut changed = effects.clone();
            changed.implicit_unit_clobbers =
                expected_effects(opposite, &physical, 0).implicit_unit_clobbers;
            assert_eq!(
                validate_x86_64_selected_scalar_call_template(
                    target,
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    &changed,
                    template.bytes(),
                    template.fixup(),
                ),
                Err(X86_64ScalarCallTemplateError::EffectMismatch)
            );
            let mut changed = operands.clone();
            *changed.last_mut().unwrap() = physical.model().view_named("rsp").unwrap().id;
            assert_eq!(
                validate_x86_64_selected_scalar_call_template(
                    target,
                    &physical,
                    kind,
                    alternative,
                    &changed,
                    effects,
                    template.bytes(),
                    template.fixup(),
                ),
                Err(X86_64ScalarCallTemplateError::OperandViewMismatch)
            );
        }
    }
}
