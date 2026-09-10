//! Mixed argument templates preserve both banks and exact ABI effects.
use super::*;
use register_model::validate_physical_register_model;

#[test]
fn mixed_aggregate_call_templates_retain_integer_results_and_ieee_inputs() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let catalog = aarch64_register_constraint_catalog(&physical);
    let kind = SelectedInstructionKind::CallAggregate {
        callee: MachineId::new(7).unwrap(),
    };
    let alternative = MachineAlternativeKey {
        family: MachineAlternativeFamily::CallAggregate,
        variant: 0,
    };
    for (target, unit_keys) in [
        (
            NativeTarget::linux_arm64(),
            crate::aarch64_aapcs64_mixed_unit_call_keys(),
        ),
        (
            NativeTarget::macos_arm64(),
            crate::aarch64_darwin_mixed_unit_call_keys(),
        ),
    ] {
        for ordinal in [0, 7, 8, 71, 72, 79, 80, 143] {
            let fragments = ordinal / 72 + 1;
            let general_count = ordinal % 72 / 8;
            let float_count = ordinal % 8 + 1;
            let arity = general_count + float_count;
            let operands = (0..general_count)
                .map(|position| format!("x{position}"))
                .chain((0..float_count).map(|position| format!("d{position}")))
                .chain((0..fragments).map(|position| format!("x{position}")))
                .map(|name| physical.model().view_named(&name).unwrap().id)
                .collect::<Vec<_>>();
            let unit = catalog
                .constraints
                .iter()
                .find(|row| row.key == unit_keys[ordinal % 72])
                .unwrap();
            let mut effects = expected_effects(target, &physical, 0);
            effects.external_operand_reads = (0..arity as u16).collect();
            effects.external_operand_writes = (arity as u16..(arity + fragments) as u16).collect();
            effects.implicit_unit_uses = unit.implicit_uses.clone();
            effects.implicit_unit_defs = unit.implicit_defs.clone();
            effects.implicit_unit_clobbers = unit.clobbers.clone();
            for name in ["x0", "x1"].into_iter().take(fragments) {
                effects.implicit_unit_clobbers.retain(|unit| {
                    !physical
                        .model()
                        .view_named(name)
                        .unwrap()
                        .write_units
                        .contains(unit)
                });
            }
            let encode = |operands: &[RegisterViewId], effects: &MachineEncodedEffects| {
                encode_aarch64_selected_scalar_call_template(
                    target,
                    &physical,
                    kind,
                    alternative,
                    operands,
                    effects,
                )
            };
            let template = encode(&operands, &effects).expect("mixed aggregate template");
            assert_eq!(template.operand_views(), operands);
            assert_eq!(
                template.fixup(),
                canonical_fixup(MachineId::new(7).unwrap())
            );
            let mut changed = operands.clone();
            changed[arity] = physical.model().view_named("d0").unwrap().id;
            assert!(
                encode(&changed, &effects).is_err(),
                "result bank substitution"
            );
            let mut changed = operands.clone();
            changed[general_count] = physical.model().view_named("x2").unwrap().id;
            assert!(
                encode(&changed, &effects).is_err(),
                "IEEE input bank substitution"
            );
            assert!(
                encode(&operands[..operands.len() - 1], &effects).is_err(),
                "missing result definition"
            );
            let mut changed = effects.clone();
            changed.external_operand_writes.clear();
            assert!(
                encode(&operands, &changed).is_err(),
                "missing result effect"
            );
            let mut changed = effects.clone();
            changed.implicit_unit_clobbers = unit.clobbers.clone();
            assert!(
                encode(&operands, &changed).is_err(),
                "result cannot remain an unknown clobber"
            );
            let mut changed = effects.clone();
            changed.implicit_unit_clobbers.retain(|unit| {
                !physical
                    .model()
                    .view_named("x2")
                    .unwrap()
                    .write_units
                    .contains(unit)
            });
            assert!(
                encode(&operands, &changed).is_err(),
                "other caller clobbers remain required"
            );
        }
    }
}

#[test]
fn every_mixed_unit_call_row_has_exact_template_operands_and_effects() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let catalog = aarch64_register_constraint_catalog(&physical);
    let kind = SelectedInstructionKind::CallUnit {
        callee: MachineId::new(7).unwrap(),
    };
    let alternative = MachineAlternativeKey {
        family: MachineAlternativeFamily::CallUnit,
        variant: 0,
    };
    for (target, keys) in [
        (
            NativeTarget::linux_arm64(),
            crate::aarch64_aapcs64_mixed_unit_call_keys(),
        ),
        (
            NativeTarget::macos_arm64(),
            crate::aarch64_darwin_mixed_unit_call_keys(),
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
                encode_aarch64_selected_scalar_call_template(
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
            wrong[0] = physical.model().view_named("sp").unwrap().id;
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
