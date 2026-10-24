use super::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedMemoryEffect,
    SelectedInstructionKind, encode_x86_64_selected_memory_form, replay_battery,
    validate_x86_64_selected_memory_form,
};
#[test]
fn load32_preserves_exact_four_byte_extent_and_rejects_a_widened_load() {
    let physical =
        register_model::validate_physical_register_model(crate::x86_64_physical_register_model())
            .unwrap();
    let operands = [
        physical.model().view_named("rax").unwrap().id,
        physical.model().view_named("rcx").unwrap().id,
    ];
    let kind = SelectedInstructionKind::Load32 { byte_offset: 4 };
    let alternative = MachineAlternativeKey {
        family: MachineAlternativeFamily::Load32,
        variant: 0,
    };
    let encoded =
        encode_x86_64_selected_memory_form(&physical, kind, alternative, &operands, 4).unwrap();
    assert_eq!(encoded.bytes(), &[0x40, 0x8b, 0x88, 4, 0, 0, 0]);
    assert_eq!(
        encoded.footprint().encoded.memory,
        MachineEncodedMemoryEffect::ReadPointerV1 {
            pointer_operand: 0,
            byte_count: 4
        }
    );
    replay_battery::replay_rejects_byte_mutations(encoded.bytes(), |bytes| {
        validate_x86_64_selected_memory_form(&physical, kind, alternative, &operands, 4, bytes)
    });
    replay_battery::replay_rejects_substituted_requests(
        encoded.bytes(),
        kind,
        alternative,
        &operands,
        4,
        |kind, alternative, operands, displacement, bytes| {
            validate_x86_64_selected_memory_form(
                &physical,
                kind,
                alternative,
                operands,
                displacement,
                bytes,
            )
        },
    );
    assert!(
        validate_x86_64_selected_memory_form(
            &physical,
            kind,
            alternative,
            &[operands[1], operands[0]],
            4,
            encoded.bytes()
        )
        .is_err()
    );
}
