//! A decoded zero fill keeps its measured destination span through
//! whole-function admission. Neither a memory-free replacement nor a span
//! naming another operand is equal.
use super::assert_non_return_refused;
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedMemoryEffect,
    MachineEncodedTrapBehavior, SelectedInstructionId, SelectedInstructionKind,
};

#[test]
fn zero_fill_exit_contract_retains_the_span_and_register_roles() {
    let physical = super::aarch64_physical_register_model();
    let registers = ["x0", "x1", "x2"].map(|name| physical.model().view_named(name).unwrap().id);
    let kind = SelectedInstructionKind::ZeroBytes;
    let alternative = MachineAlternativeKey {
        family: MachineAlternativeFamily::ZeroBytes,
        variant: 0,
    };
    let encoded =
        isa_aarch64::encode_aarch64_selected_form(&physical, kind, alternative, &registers)
            .unwrap();
    let instruction = SelectedInstructionId(1);
    let (store, encoding, layout) =
        super::validated_aarch64_form_fixture(&encoded, instruction, alternative, kind);
    for mutation in 0..7 {
        let mut changed = store.clone();
        let footprint = changed.footprint_mut(encoding.footprint).unwrap();
        match mutation {
            0 => footprint.encoded.memory = MachineEncodedMemoryEffect::NoneV1,
            1 => {
                footprint.encoded.memory =
                    MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 }
            }
            2 => {
                footprint.encoded.memory = MachineEncodedMemoryEffect::ZeroBytesV1 {
                    destination_pointer_operand: 1,
                    count_operand: 0,
                }
            }
            3 => {
                footprint.encoded.memory = MachineEncodedMemoryEffect::ZeroBytesV1 {
                    destination_pointer_operand: 0,
                    count_operand: 2,
                }
            }
            4 => footprint
                .encoded
                .external_operand_reads
                .pop()
                .map(|_| ())
                .unwrap(),
            5 => footprint.encoded.external_operand_writes.push(0),
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
}
