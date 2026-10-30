//! A byte-literal image address is an `adrp`/`add` pair whose immediates stay
//! zero for object construction; every other bit is final and checked.
use super::{
    AARCH64_BYTE_LITERAL_ADDRESS_PAGE_FIELD_OFFSET,
    AARCH64_BYTE_LITERAL_ADDRESS_PAGE_OFFSET_FIELD_OFFSET, encode_aarch64_selected_form,
    validate_aarch64_selected_form_encoding,
};
use crate::test_support::rejects_every_bit_flip;
use register_model::validate_physical_register_model;
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedMemoryEffect,
    SelectedInstructionKind,
};
use semantic_vocabulary::{OperationId, PlaceId};

#[test]
fn byte_literal_address_is_an_unresolved_page_pair_into_its_destination() {
    let physical =
        validate_physical_register_model(crate::aarch64_physical_register_model()).unwrap();
    let views = ["x5", "x6"].map(|name| physical.model().view_named(name).unwrap().id);
    let kind = SelectedInstructionKind::ImageAddress {
        reference: selected_instructions::SelectedImageAddressReference::ByteLiteral {
            operation: OperationId::new(7).unwrap(),
            place: PlaceId::new(3).unwrap(),
        },
    };
    let key = MachineAlternativeKey {
        family: MachineAlternativeFamily::ImageAddress,
        variant: 0,
    };
    let encoded = encode_aarch64_selected_form(&physical, kind, key, &views[..1]).unwrap();
    // adrp x5, #0 ; add x5, x5, #0
    let mut expected = 0x9000_0005_u32.to_le_bytes().to_vec();
    crate::codec_bytes::push_encoded(&mut expected, 0x9100_00a5_u32.to_le_bytes());
    assert_eq!(encoded.bytes(), expected);
    assert_eq!(AARCH64_BYTE_LITERAL_ADDRESS_PAGE_FIELD_OFFSET, 0);
    assert_eq!(AARCH64_BYTE_LITERAL_ADDRESS_PAGE_OFFSET_FIELD_OFFSET, 4);
    validate_aarch64_selected_form_encoding(&physical, kind, key, &views[..1], encoded.bytes())
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
    // Another destination register, a resolved immediate, or any other
    // changed bit is a different instruction.
    assert!(
        validate_aarch64_selected_form_encoding(&physical, kind, key, &views[1..], encoded.bytes())
            .is_err()
    );
    rejects_every_bit_flip(&physical, kind, key, &views[..1], encoded.bytes());
}
