//! A byte-literal image address is `lea r64, [rip+disp32]` whose disp32 stays
//! zero for object construction; every other byte is final and checked.
use super::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedMemoryEffect,
    SelectedInstructionKind, X86_64_BYTE_LITERAL_ADDRESS_FIELD_OFFSET, encode_x86_64_selected_form,
    validate_x86_64_selected_form_encoding,
};
use register_model::validate_physical_register_model;
use semantic_vocabulary::{OperationId, PlaceId};

#[test]
fn byte_literal_address_is_an_unresolved_rip_relative_lea() {
    let physical =
        validate_physical_register_model(crate::x86_64_physical_register_model()).unwrap();
    let views = ["rcx", "r9"].map(|name| physical.model().view_named(name).unwrap().id);
    let kind = SelectedInstructionKind::ByteLiteralImageAddress {
        operation: OperationId::new(7).unwrap(),
        place: PlaceId::new(3).unwrap(),
    };
    let key = MachineAlternativeKey {
        family: MachineAlternativeFamily::ByteLiteralImageAddress,
        variant: 0,
    };
    for (view, expected) in [
        (views[0], [0x48, 0x8d, 0x0d, 0, 0, 0, 0]),
        (views[1], [0x4c, 0x8d, 0x0d, 0, 0, 0, 0]),
    ] {
        let encoded = encode_x86_64_selected_form(&physical, kind, key, &[view]).unwrap();
        assert_eq!(encoded.bytes(), expected);
        assert_eq!(
            &encoded.bytes()[usize::from(X86_64_BYTE_LITERAL_ADDRESS_FIELD_OFFSET)..],
            [0, 0, 0, 0]
        );
        validate_x86_64_selected_form_encoding(&physical, kind, key, &[view], encoded.bytes())
            .unwrap();
        assert!(
            encoded
                .footprint()
                .encoded
                .external_operand_reads
                .is_empty()
        );
        assert_eq!(encoded.footprint().encoded.external_operand_writes, [0]);
        assert_eq!(
            encoded.footprint().encoded.memory,
            MachineEncodedMemoryEffect::NoneV1
        );
        for bit in 0..encoded.bytes().len() * 8 {
            let mut changed = encoded.bytes().to_vec();
            changed[bit / 8] ^= 1 << (bit % 8);
            assert!(
                validate_x86_64_selected_form_encoding(&physical, kind, key, &[view], &changed)
                    .is_err(),
                "changed encoding bit {bit}"
            );
        }
    }
}
