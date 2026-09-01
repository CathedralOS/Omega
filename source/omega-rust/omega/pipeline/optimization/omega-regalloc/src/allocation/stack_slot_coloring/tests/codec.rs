use super::fixtures::{budget, source};
use crate::*;

fn plan() -> StackSlotColoringPlan {
    color_logical_spill_stack_slots(
        &source(),
        StackSlotColoringPolicy::BlockLocalNonAddressUnsignedU64ClosedIntervalFirstFitV1,
        budget(),
    )
    .unwrap()
    .plan()
    .clone()
}

#[test]
fn canonical_codec_round_trips() {
    let plan = plan();
    assert_eq!(StackSlotColoringPlan::decode(&plan.encode()), Ok(plan));
}

#[test]
fn codec_rejects_corruption_and_trailing_data() {
    let encoded = plan().encode();
    let mut corrupt = encoded.clone();
    *corrupt.last_mut().unwrap() ^= 1;
    assert_eq!(
        StackSlotColoringPlan::decode(&corrupt),
        Err(StackSlotColoringDecodeError::IdentityMismatch)
    );
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(
        StackSlotColoringPlan::decode(&trailing),
        Err(StackSlotColoringDecodeError::TrailingBytes)
    );
    assert_eq!(
        StackSlotColoringPlan::decode(&encoded[..encoded.len() - 1]),
        Err(StackSlotColoringDecodeError::Truncated)
    );

    let mut wrong_magic = encoded.clone();
    wrong_magic[0] ^= 1;
    assert_eq!(
        StackSlotColoringPlan::decode(&wrong_magic),
        Err(StackSlotColoringDecodeError::WrongMagic)
    );

    let mut wrong_version = encoded;
    wrong_version[8..12].copy_from_slice(&2_u32.to_le_bytes());
    assert_eq!(
        StackSlotColoringPlan::decode(&wrong_version),
        Err(StackSlotColoringDecodeError::UnsupportedVersion(2))
    );
}
