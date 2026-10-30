//! Mixed argument templates preserve both banks and exact ABI effects.
use super::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects, MachineId,
    NativeTarget, RegisterViewId, SelectedInstructionKind,
    ValidatedAarch64SelectedScalarCallTemplate, aarch64_selected_abi, canonical_fixup,
    encode_aarch64_selected_scalar_call_template, expected_effects,
};
use crate::{aarch64_physical_register_model, aarch64_register_constraint_catalog};
use register_model::validate_physical_register_model;
use selected_instructions::fixture;

impl fixture::CallTemplate for ValidatedAarch64SelectedScalarCallTemplate {
    fn bound_operands(&self) -> &[RegisterViewId] {
        self.operand_views()
    }

    fn names_callee(&self, callee: MachineId) -> bool {
        self.fixup() == canonical_fixup(callee)
    }
}

macro_rules! a {
    ($cond:expr => $msg:expr) => {
        assert!($cond, $msg);
    };
}

#[test]
fn mixed_aggregate_call_templates_retain_integer_results_and_ieee_inputs() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let catalog = aarch64_register_constraint_catalog(&physical);
    let kind = SelectedInstructionKind::CallAggregate {
        target: selected_instructions::SelectedCallTarget::Machine(MachineId::new(7).unwrap()),
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
            let unit = catalog.constraint(unit_keys[ordinal % 72]).unwrap();
            let mut effects = expected_effects(aarch64_selected_abi(target).unwrap(), &physical, 0);
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
            a!(encode(&changed, &effects).is_err() => "result bank substitution");
            let mut changed = operands.clone();
            changed[general_count] = physical.model().view_named("x2").unwrap().id;
            a!(encode(&changed, &effects).is_err() => "IEEE input bank substitution");
            assert!(
                encode(&operands[..operands.len() - 1], &effects).is_err(),
                "missing result definition"
            );
            let mut changed = effects.clone();
            changed.external_operand_writes.clear();
            a!(encode(&operands, &changed).is_err() => "missing result effect");
            let mut changed = effects.clone();
            changed.implicit_unit_clobbers = unit.clobbers.clone();
            a!(encode(&operands, &changed).is_err() => "result cannot remain an unknown clobber");
            let mut changed = effects.clone();
            changed.implicit_unit_clobbers.retain(|unit| {
                !physical
                    .model()
                    .view_named("x2")
                    .unwrap()
                    .write_units
                    .contains(unit)
            });
            a!(encode(&operands, &changed).is_err() => "other caller clobbers remain required");
        }
    }
}

#[test]
fn every_mixed_unit_call_row_has_exact_template_operands_and_effects() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let catalog = aarch64_register_constraint_catalog(&physical);
    let callee = MachineId::new(7).unwrap();
    let stack_pointer = physical.model().view_named("sp").unwrap().id;
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
        let base = expected_effects(aarch64_selected_abi(target).unwrap(), &physical, 0);
        for key in keys {
            fixture::assert_mixed_unit_call_row(
                catalog.constraint(key).unwrap(),
                base.clone(),
                callee,
                stack_pointer,
                |kind, alternative, operands, effects| {
                    encode_aarch64_selected_scalar_call_template(
                        target,
                        &physical,
                        kind,
                        alternative,
                        operands,
                        effects,
                    )
                },
            );
        }
    }
}
