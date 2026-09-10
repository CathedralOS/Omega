//! Unit call templates preserve arity and never manufacture a result operand.
use super::*;
use register_model::{RegisterOperandAccess, validate_physical_register_model};

#[test]
fn unit_call_templates_cover_native_register_arities_and_reject_forged_results() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let catalog = aarch64_register_constraint_catalog(&physical);
    for (target, keys) in [
        (
            NativeTarget::linux_arm64(),
            crate::aarch64_aapcs64_register_unit_call_keys(),
        ),
        (
            NativeTarget::macos_arm64(),
            crate::aarch64_darwin_register_unit_call_keys(),
        ),
    ] {
        for (arity, key) in keys.iter().enumerate() {
            let row = catalog
                .constraints
                .iter()
                .find(|row| row.key == *key)
                .unwrap();
            assert_eq!(row.operands.len(), arity);
            assert!(
                row.operands
                    .iter()
                    .all(|operand| operand.access == RegisterOperandAccess::Use)
            );
            let mut missing_clobber = catalog.clone();
            missing_clobber
                .constraints
                .iter_mut()
                .find(|row| row.key == *key)
                .unwrap()
                .clobbers
                .clear();
            assert!(
                crate::validate_aarch64_register_constraint_catalog(missing_clobber, &physical)
                    .is_err()
            );
            let mut operands = expected_operand_views(&physical, arity);
            operands.pop();
            let mut effects = expected_effects(target, &physical, arity);
            effects.external_operand_writes.clear();
            effects.implicit_unit_uses = row.implicit_uses.clone();
            effects.implicit_unit_defs = row.implicit_defs.clone();
            effects.implicit_unit_clobbers = row.clobbers.clone();
            let kind = SelectedInstructionKind::CallUnit {
                callee: MachineId::new(7).unwrap(),
            };
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::CallUnit,
                variant: 0,
            };
            let template = encode_aarch64_selected_scalar_call_template(
                target,
                &physical,
                kind,
                alternative,
                &operands,
                &effects,
            )
            .unwrap();
            assert_eq!(template.operand_views(), operands);
            validate_aarch64_selected_scalar_call_template(
                target,
                &physical,
                kind,
                alternative,
                &operands,
                &effects,
                template.bytes(),
                template.fixup(),
            )
            .unwrap();
            let mut forged = effects.clone();
            forged.external_operand_writes.push(arity as u16);
            assert!(
                encode_aarch64_selected_scalar_call_template(
                    target,
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    &forged
                )
                .is_err()
            );
            let wrong_family = MachineAlternativeKey {
                family: MachineAlternativeFamily::CallScalar,
                variant: 0,
            };
            assert!(
                encode_aarch64_selected_scalar_call_template(
                    target,
                    &physical,
                    kind,
                    wrong_family,
                    &operands,
                    &effects
                )
                .is_err()
            );
            if !operands.is_empty() {
                let mut wrong = operands.clone();
                wrong[0] = physical.model().view_named("sp").unwrap().id;
                assert!(
                    encode_aarch64_selected_scalar_call_template(
                        target,
                        &physical,
                        kind,
                        alternative,
                        &wrong,
                        &effects
                    )
                    .is_err()
                );
            }
        }
    }
}
