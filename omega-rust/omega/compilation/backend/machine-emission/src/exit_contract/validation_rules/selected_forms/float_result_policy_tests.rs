//! Inline policy control is not a function exit or an external relocation.
use super::validate_non_return;
use machine_code::{
    ResolvedSelectedFormRow, SelectedFormDecodedFootprint, SelectedFormEncodingRow,
    SelectedFormEncodingState, SelectedFormMachineDisposition,
};
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
        let effects = &encoded.footprint().encoded;
        let encoding = SelectedFormEncodingRow {
            instruction,
            alternative,
            machine_disposition: SelectedFormMachineDisposition::RetainedV1,
            address: None,
            state: SelectedFormEncodingState::Encoded {
                bytes: encoded.bytes().to_vec(),
                footprint: Box::new(SelectedFormDecodedFootprint {
                    register_reads: encoded.footprint().register_reads.clone(),
                    register_writes: encoded.footprint().register_writes.clone(),
                    implicit_defs: effects.implicit_unit_defs.clone(),
                    implicit_clobbers: effects.implicit_unit_clobbers.clone(),
                    encoded: effects.clone(),
                }),
            },
        };
        let layout = ResolvedSelectedFormRow {
            instruction,
            alternative,
            offset: 0,
            bytes: encoded.bytes().to_vec(),
            branch: None,
            internal_machine_fixup: None,
            normalized_foreign_call_fixup: None,
            byte_literal_address_fixup: None,
        };
        validate_non_return(instruction, kind, &encoding, &layout).unwrap();
        for mutation in 0..5 {
            let mut changed = encoding.clone();
            let SelectedFormEncodingState::Encoded { footprint, .. } = &mut changed.state else {
                unreachable!()
            };
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
                validate_non_return(instruction, kind, &changed, &layout).is_err(),
                "{form:?}, mutation {mutation}"
            );
        }
    }
}
