use crate::Aarch64SameViewCopyElisionDecodeError;

use super::super::super::elide_same_view_copy_before_return::tests::fixture;

#[test]
fn codec_round_trip_and_malformed_envelopes_fail_closed() {
    let fixture = fixture::compare_i64_left_operand_fixture();
    let plan =
        super::super::compute::compute_from_inputs(fixture.inputs(), fixture::budget()).unwrap();
    let encoded = plan.encode();
    assert_eq!(u32::from_le_bytes(encoded[8..12].try_into().unwrap()), 3);
    assert_eq!(
        crate::Aarch64SameViewCopyElisionPlan::decode(&encoded),
        Ok(plan)
    );

    let mut wrong_magic = encoded.clone();
    wrong_magic[0] ^= 1;
    assert_eq!(
        crate::Aarch64SameViewCopyElisionPlan::decode(&wrong_magic),
        Err(Aarch64SameViewCopyElisionDecodeError::WrongMagic)
    );

    let mut wrong_identity = encoded.clone();
    wrong_identity[12] ^= 1;
    assert_eq!(
        crate::Aarch64SameViewCopyElisionPlan::decode(&wrong_identity),
        Err(Aarch64SameViewCopyElisionDecodeError::InvalidIdentity)
    );

    let mut trailing = encoded;
    trailing.push(0);
    assert_eq!(
        crate::Aarch64SameViewCopyElisionPlan::decode(&trailing),
        Err(Aarch64SameViewCopyElisionDecodeError::TrailingBytes)
    );
}
