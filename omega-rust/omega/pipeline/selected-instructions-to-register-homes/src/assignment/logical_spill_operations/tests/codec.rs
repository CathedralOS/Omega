use super::fixtures::fixture;
use crate::LogicalSpillOperationPlan;

#[test]
fn canonical_codec_round_trips() {
    let fixture = fixture();
    let encoded = fixture.plan.encode();
    assert_eq!(
        LogicalSpillOperationPlan::decode(&encoded),
        Ok(fixture.plan)
    );
}
