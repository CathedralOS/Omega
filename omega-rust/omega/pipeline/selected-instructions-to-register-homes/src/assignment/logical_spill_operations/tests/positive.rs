use super::fixtures::fixture;
use crate::logical_spill_operation_identity;

#[test]
fn identity_binds_the_complete_logical_decision() {
    let fixture = fixture();
    let baseline = logical_spill_operation_identity(&fixture.plan);
    let mut changed = fixture.plan;
    changed.functions[0].action.as_mut().unwrap().rewrites[0].operand += 1;
    assert_ne!(baseline, logical_spill_operation_identity(&changed));
}
