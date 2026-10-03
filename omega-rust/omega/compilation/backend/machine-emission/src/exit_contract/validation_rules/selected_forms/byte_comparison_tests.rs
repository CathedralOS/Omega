//! A decoded byte scan keeps both measured spans through whole-function
//! admission. Neither a memory-free replacement nor a one-span scan is equal.
use super::validate_non_return;
use machine_code::{
    ResolvedSelectedFormRow, SelectedFormDecodedFootprint, SelectedFormEncodingRow,
    SelectedFormEncodingState, SelectedFormMachineDisposition,
};
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedMemoryEffect, MachineEncodedTrapBehavior, SelectedInstructionId,
    SelectedInstructionKind,
};

#[test]
fn byte_comparison_exit_contract_retains_both_spans_and_register_roles() {
    let physical = register_model::validate_physical_register_model(
        isa_aarch64::aarch64_physical_register_model(),
    )
    .unwrap();
    let registers = ["x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7"]
        .map(|name| physical.model().view_named(name).unwrap().id);
    let kind = SelectedInstructionKind::CompareByteSequences;
    let alternative = MachineAlternativeKey {
        family: MachineAlternativeFamily::CompareByteSequences,
        variant: 0,
    };
    let encoded =
        isa_aarch64::encode_aarch64_selected_form(&physical, kind, alternative, &registers)
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
    for mutation in 0..9 {
        let mut changed = encoding.clone();
        let SelectedFormEncodingState::Encoded { footprint, .. } = &mut changed.state else {
            unreachable!()
        };
        match mutation {
            0 => footprint.encoded.memory = MachineEncodedMemoryEffect::NoneV1,
            1 => {
                footprint.encoded.memory = MachineEncodedMemoryEffect::ScanByteSequenceV1 {
                    base_pointer_operand: 0,
                    count_operand: 1,
                }
            }
            2..=5 => {
                let mut positions = [0, 1, 2, 3];
                positions[mutation - 2] = 7;
                footprint.encoded.memory = MachineEncodedMemoryEffect::CompareByteSequencesV1 {
                    left_base_operand: positions[0],
                    left_count_operand: positions[1],
                    right_base_operand: positions[2],
                    right_count_operand: positions[3],
                };
            }
            6 => footprint
                .encoded
                .external_operand_reads
                .pop()
                .map(|_| ())
                .unwrap(),
            7 => footprint.encoded.external_operand_writes.push(0),
            _ => footprint.encoded.trap = MachineEncodedTrapBehavior::NeverV1,
        }
        assert!(
            validate_non_return(instruction, kind, &changed, &layout).is_err(),
            "mutation {mutation}"
        );
    }
    let mut changed = encoding.clone();
    let SelectedFormEncodingState::Encoded { footprint, .. } = &mut changed.state else {
        unreachable!()
    };
    footprint.encoded.control = MachineEncodedControlEffect::DirectRelativeCallV1;
    assert!(validate_non_return(instruction, kind, &changed, &layout).is_err());
}
