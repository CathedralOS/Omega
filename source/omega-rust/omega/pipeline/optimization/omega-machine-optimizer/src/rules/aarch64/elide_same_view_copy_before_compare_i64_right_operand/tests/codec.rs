use crate::Aarch64SameViewCopyElisionDecodeError;

use super::super::super::same_view_copy_elision::test_support::fixture;

#[test]
fn v4_codec_round_trips_and_malformed_envelopes_fail_closed() {
    let fixture = fixture::compare_i64_right_operand_fixture();
    let plan =
        super::super::compute::compute_from_inputs(fixture.inputs(), fixture::budget()).unwrap();
    let encoded = plan.encode();
    assert_eq!(u32::from_le_bytes(encoded[8..12].try_into().unwrap()), 4);
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

    let mut wrong_version = encoded.clone();
    wrong_version[8..12].copy_from_slice(&3_u32.to_le_bytes());
    assert_eq!(
        crate::Aarch64SameViewCopyElisionPlan::decode(&wrong_version),
        Err(Aarch64SameViewCopyElisionDecodeError::UnsupportedVersion(3))
    );

    let mut wrong_identity = encoded.clone();
    wrong_identity[12] ^= 1;
    assert_eq!(
        crate::Aarch64SameViewCopyElisionPlan::decode(&wrong_identity),
        Err(Aarch64SameViewCopyElisionDecodeError::InvalidIdentity)
    );

    let mut unknown_policy = encoded.clone();
    unknown_policy[190] = u8::MAX;
    assert_eq!(
        crate::Aarch64SameViewCopyElisionPlan::decode(&unknown_policy),
        Err(Aarch64SameViewCopyElisionDecodeError::InvalidField)
    );

    let mut trailing = encoded;
    trailing.push(0);
    assert_eq!(
        crate::Aarch64SameViewCopyElisionPlan::decode(&trailing),
        Err(Aarch64SameViewCopyElisionDecodeError::TrailingBytes)
    );
}
