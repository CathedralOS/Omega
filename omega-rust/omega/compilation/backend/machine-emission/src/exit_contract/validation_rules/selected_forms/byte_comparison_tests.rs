//! A decoded byte scan keeps both measured spans through whole-function
//! admission. Neither a memory-free replacement nor a one-span scan is equal.
use super::assert_non_return_refused;
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedMemoryEffect, MachineEncodedTrapBehavior, SelectedInstructionId,
    SelectedInstructionKind,
};

#[test]
fn byte_comparison_exit_contract_retains_both_spans_and_register_roles() {
    let physical = super::aarch64_physical_register_model();
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
    let (store, encoding, layout) =
        super::validated_aarch64_form_fixture(&encoded, instruction, alternative, kind);
    for mutation in 0..9 {
        let mut changed = store.clone();
        let footprint = changed.footprint_mut(encoding.footprint).unwrap();
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
        assert_non_return_refused(
            &format!("mutation {mutation}"),
            instruction,
            kind,
            &encoding,
            &layout,
            &changed,
        );
    }
    let mut changed = store.clone();
    let footprint = changed.footprint_mut(encoding.footprint).unwrap();
    footprint.encoded.control = MachineEncodedControlEffect::DirectRelativeCallV1;
    assert_non_return_refused(
        "a direct relative call leaves the function",
        instruction,
        kind,
        &encoding,
        &layout,
        &changed,
    );
}
