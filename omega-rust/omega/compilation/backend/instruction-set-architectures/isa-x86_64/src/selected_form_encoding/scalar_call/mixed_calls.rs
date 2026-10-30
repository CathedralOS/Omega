//! Mixed argument templates preserve both banks and exact ABI effects.
use super::{
    MachineId, NativeTarget, ValidatedX86_64SelectedScalarCallTemplate, canonical_fixup,
    encode_x86_64_selected_scalar_call_template, expected_effects, x86_64_selected_abi,
};
use crate::{x86_64_physical_register_model, x86_64_register_constraint_catalog};
use register_model::{RegisterViewId, validate_physical_register_model};
use selected_instructions::fixture;

impl fixture::CallTemplate for ValidatedX86_64SelectedScalarCallTemplate {
    fn bound_operands(&self) -> &[RegisterViewId] {
        self.operand_views()
    }

    fn names_callee(&self, callee: MachineId) -> bool {
        self.fixup() == canonical_fixup(callee)
    }
}

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
    let callee = MachineId::new(7).unwrap();
    let stack_pointer = physical.model().view_named("rsp").unwrap().id;
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
        let base = expected_effects(x86_64_selected_abi(target).unwrap(), &physical, 0);
        for key in keys {
            fixture::assert_mixed_unit_call_row(
                catalog.constraint(key).unwrap(),
                base.clone(),
                callee,
                stack_pointer,
                |kind, alternative, operands, effects| {
                    encode_x86_64_selected_scalar_call_template(
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
