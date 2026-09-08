//! Mixed argument templates preserve both banks and exact ABI effects.
use super::*;
use register_model::validate_physical_register_model;

#[test]
fn microsoft_mixed_calls_preserve_positional_register_holes() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let catalog = x86_64_register_constraint_catalog(&physical);
    for (variant, names) in [
        (901, vec!["rdx", "xmm0"]),
        (902, vec!["rcx", "xmm1"]),
        (903, vec!["xmm0", "xmm1"]),
        (911, vec!["rdx", "r8", "r9", "xmm0"]),
        (925, vec!["xmm0", "xmm1", "xmm2", "xmm3"]),
    ] {
        let row = catalog
            .constraints
            .iter()
            .find(|row| {
                row.key.family == register_model::RegisterConstraintFamily::Call
                    && row.key.variant == variant
            })
            .unwrap();
        assert_eq!(
            row.operands
                .iter()
                .map(|operand| operand.fixed_view.unwrap())
                .collect::<Vec<_>>(),
            names
                .iter()
                .map(|name| physical.model().view_named(name).unwrap().id)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn every_mixed_unit_call_row_has_exact_template_operands_and_effects() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let catalog = x86_64_register_constraint_catalog(&physical);
    let kind = SelectedInstructionKind::CallUnit {
        callee: MachineId::new(7).unwrap(),
    };
    let alternative = MachineAlternativeKey {
        family: MachineAlternativeFamily::CallUnit,
        variant: 0,
    };
    for (target, keys) in [
        (
            NativeTarget::linux_x64(),
            crate::x86_64_system_v_mixed_unit_call_keys(),
        ),
        (
            NativeTarget::windows_x64(),
            crate::x86_64_microsoft_mixed_unit_call_keys(),
        ),
    ] {
        for key in keys {
            let row = catalog
                .constraints
                .iter()
                .find(|row| row.key == key)
                .unwrap();
            let operands = row
                .operands
                .iter()
                .map(|operand| operand.fixed_view.unwrap())
                .collect::<Vec<_>>();
            let mut effects = expected_effects(target, &physical, 0);
            effects.external_operand_reads = (0..operands.len() as u16).collect();
            effects.external_operand_writes.clear();
            effects.implicit_unit_uses = row.implicit_uses.clone();
            effects.implicit_unit_defs = row.implicit_defs.clone();
            effects.implicit_unit_clobbers = row.clobbers.clone();
            let encode = |operands: &[RegisterViewId], effects: &MachineEncodedEffects| {
                encode_x86_64_selected_scalar_call_template(
                    target,
                    &physical,
                    kind,
                    alternative,
                    operands,
                    effects,
                )
            };
            let template = encode(&operands, &effects).expect("mixed ABI call template");
            assert_eq!(template.operand_views(), operands);
            assert_eq!(
                template.fixup(),
                canonical_fixup(MachineId::new(7).unwrap())
            );
            let mut wrong = operands.clone();
            wrong.push(operands[0]);
            assert!(
                encode(&wrong, &effects).is_err(),
                "duplicate operand rejects"
            );
            let mut wrong = operands.clone();
            wrong[0] = physical.model().view_named("rsp").unwrap().id;
            assert!(
                encode(&wrong, &effects).is_err(),
                "substituted operand rejects"
            );
            assert!(
                encode(&operands[..operands.len() - 1], &effects).is_err(),
                "missing operand rejects"
            );
            if operands.len() > 1 {
                let mut wrong = operands.clone();
                wrong.swap(0, operands.len() - 1);
                assert!(
                    encode(&wrong, &effects).is_err(),
                    "reordered banks or slots reject"
                );
            }
            let mut wrong = effects.clone();
            wrong.implicit_unit_clobbers.clear();
            assert!(
                encode(&operands, &wrong).is_err(),
                "missing caller clobbers reject"
            );
            let mut wrong = effects.clone();
            wrong.external_operand_writes.push(0);
            assert!(
                encode(&operands, &wrong).is_err(),
                "Unit call cannot define a result"
            );
        }
    }
}
