//! Inline policy control is not a function exit or an external relocation.
use super::validate_non_return;
use selected_instructions::{
    FloatResultPolicyForm, MachineAlternativeFamily, MachineAlternativeKey,
    MachineEncodedControlEffect, MachineEncodedMemoryEffect, MachineEncodedTrapBehavior,
    SelectedInstructionId, SelectedInstructionKind,
};

#[test]
fn float_result_policy_exit_admission_binds_control_trap_and_operand_footprint() {
    let physical = register_model::validate_physical_register_model(
        isa_aarch64::aarch64_physical_register_model(),
    )
    .unwrap();
    let registers =
        ["x2", "x3", "x4", "x5"].map(|name| physical.model().view_named(name).unwrap().id);
    for form in FloatResultPolicyForm::ALL {
        let kind = SelectedInstructionKind::FloatResultPolicy { form };
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::FloatResultPolicy(form),
            variant: 0,
        };
        let encoded = isa_aarch64::encode_aarch64_selected_form(
            &physical,
            kind,
            alternative,
            &registers[..form.operand_count() + 1],
        )
        .unwrap();
        let instruction = SelectedInstructionId(1);
        let (store, encoding, layout) =
            super::validated_aarch64_form_fixture(&encoded, instruction, alternative, kind);
        for mutation in 0..5 {
            let mut changed = store.clone();
            let footprint = changed.footprint_mut(encoding.footprint).unwrap();
            match mutation {
                0 => {
                    footprint.encoded.control = if form.can_trap() {
                        MachineEncodedControlEffect::FallThroughV1
                    } else {
                        MachineEncodedControlEffect::FallThroughOrTrapV1
                    }
                }
                1 => {
                    footprint.encoded.trap = if form.can_trap() {
                        MachineEncodedTrapBehavior::NeverV1
                    } else {
                        MachineEncodedTrapBehavior::TrappingIntegerV1
                    }
                }
                2 => footprint.encoded.external_operand_reads.clear(),
                3 => footprint.encoded.external_operand_writes = vec![0],
                _ => {
                    footprint.encoded.memory = MachineEncodedMemoryEffect::ReadPointerV1 {
                        pointer_operand: 0,
                        byte_count: 4,
                    }
                }
            }
            assert!(
                validate_non_return(
                    target::Architecture::Aarch64,
                    instruction,
                    kind,
                    &encoding,
                    &layout,
                    &changed
                )
                .is_err(),
                "{form:?}, mutation {mutation}"
            );
        }
    }
}
