//! Private address formation retains exact operand roles and never accesses memory.
use super::*;
use register_model::validate_physical_register_model;

#[test]
fn byte_view_address_has_distinct_family_and_exact_register_bytes() {
    let physical =
        validate_physical_register_model(crate::aarch64_physical_register_model()).unwrap();
    let views =
        ["x9", "x10", "x11", "x12"].map(|name| physical.model().view_named(name).unwrap().id);
    let operands = &views[..3];
    let kind = SelectedInstructionKind::ByteViewAddress;
    let key = MachineAlternativeKey {
        family: MachineAlternativeFamily::ByteViewAddress,
        variant: 0,
    };
    let encoded = encode_aarch64_selected_form(&physical, kind, key, operands).unwrap();
    validate_aarch64_selected_form_encoding(&physical, kind, key, operands, encoded.bytes())
        .unwrap();
    assert_eq!(encoded.footprint().encoded.external_operand_reads, [0, 1]);
    assert_eq!(encoded.footprint().encoded.external_operand_writes, [2]);
    assert_eq!(
        encoded.footprint().encoded.memory,
        MachineEncodedMemoryEffect::NoneV1
    );
    assert!(encoded.footprint().encoded.implicit_unit_defs.is_empty());
    assert!(
        encoded
            .footprint()
            .encoded
            .implicit_unit_clobbers
            .is_empty()
    );
    let wrong_family = MachineAlternativeKey {
        family: MachineAlternativeFamily::ExactAddI64,
        variant: 0,
    };
    assert!(
        validate_aarch64_selected_form_encoding(
            &physical,
            kind,
            wrong_family,
            operands,
            encoded.bytes()
        )
        .is_err()
    );
    for position in 0..3 {
        let mut changed = operands.to_vec();
        changed[position] = views[3];
        assert!(
            validate_aarch64_selected_form_encoding(
                &physical,
                kind,
                key,
                &changed,
                encoded.bytes()
            )
            .is_err()
        );
    }
    for bit in 0..encoded.bytes().len() * 8 {
        let mut changed = encoded.bytes().to_vec();
        changed[bit / 8] ^= 1 << (bit % 8);
        assert!(
            validate_aarch64_selected_form_encoding(&physical, kind, key, operands, &changed)
                .is_err(),
            "changed encoding bit {bit}"
        );
    }
}

#[test]
fn byte_view_address_arithmetic_wraps_only_exclusive_bound_empty_carrier() {
    let bound = 1_u128 << 64;
    for (backing, root_length, offset, length) in [
        (0_u64, 0_u64, 0_u64, 0_u64),
        (16, 32, 7, 25),
        (u64::MAX - 7, 8, 0, 8),
        (u64::MAX - 7, 8, 7, 1),
        (u64::MAX - 7, 8, 8, 0),
        (u64::MAX, 1, 1, 0),
    ] {
        assert!(u128::from(backing) + u128::from(root_length) <= bound);
        assert!(u128::from(offset) + u128::from(length) <= u128::from(root_length));
        let mathematical = u128::from(backing) + u128::from(offset);
        let address = backing.wrapping_add(offset);
        if mathematical == bound {
            assert_eq!(length, 0);
            assert_eq!(address, 0);
        } else {
            assert_eq!(u128::from(address), mathematical);
        }
    }
}
