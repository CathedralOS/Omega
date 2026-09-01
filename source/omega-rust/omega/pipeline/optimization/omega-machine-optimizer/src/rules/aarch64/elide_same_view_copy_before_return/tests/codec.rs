use super::super::Aarch64SameViewCopyElisionDecodeError;

#[test]
fn codec_round_trip_and_malformed_envelopes_fail_closed() {
    let fixture = super::fixture::fixture();
    let plan =
        super::super::compute::compute_from_inputs(fixture.inputs(), super::fixture::budget())
            .unwrap();
    let encoded = plan.encode();
    assert_eq!(
        super::super::Aarch64SameViewCopyElisionPlan::decode(&encoded),
        Ok(plan)
    );

    let mut wrong_magic = encoded.clone();
    wrong_magic[0] ^= 1;
    assert_eq!(
        super::super::Aarch64SameViewCopyElisionPlan::decode(&wrong_magic),
        Err(Aarch64SameViewCopyElisionDecodeError::WrongMagic)
    );

    let mut wrong_identity = encoded.clone();
    wrong_identity[12] ^= 1;
    assert_eq!(
        super::super::Aarch64SameViewCopyElisionPlan::decode(&wrong_identity),
        Err(Aarch64SameViewCopyElisionDecodeError::InvalidIdentity)
    );

    let mut trailing = encoded;
    trailing.push(0);
    assert_eq!(
        super::super::Aarch64SameViewCopyElisionPlan::decode(&trailing),
        Err(Aarch64SameViewCopyElisionDecodeError::TrailingBytes)
    );
}
