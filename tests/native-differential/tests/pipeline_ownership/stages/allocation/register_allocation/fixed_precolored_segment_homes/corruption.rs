use crate::tests::*;

use super::fixture::{assign, generous_budget, source, validate};

#[test]
fn independent_replay_rejects_assignment_and_root_corruption() {
    let fixture = source(NativeTarget::linux_x64());
    let canonical = assign(&fixture, generous_budget()).unwrap();

    let mut root = canonical.plan().clone();
    root.split_requirements =
        register_homes::FixedPrecoloredSplitRequirementPlanIdentity::from_bytes([9; 32]);
    assert_eq!(
        validate(&fixture, root),
        Err(selected_instructions_to_register_homes::FixedPrecoloredSegmentHomeError::RootMismatch)
    );

    let mut corruptions = Vec::new();
    let mut plan = canonical.plan().clone();
    plan.functions[0].machine = MachineId::new(99_001).unwrap();
    corruptions.push(plan);
    let mut plan = canonical.plan().clone();
    plan.functions[0].assignments[0].virtual_register = VirtualRegisterId(99);
    corruptions.push(plan);
    let mut plan = canonical.plan().clone();
    plan.functions[0].assignments[0].class = register_model::RegisterClassId(99);
    corruptions.push(plan);
    let mut plan = canonical.plan().clone();
    plan.functions[0].assignments[0].source_segment =
        register_homes::FixedPrecoloredSourceSegmentId(99);
    corruptions.push(plan);
    let mut plan = canonical.plan().clone();
    plan.functions[0].assignments[0].allocation_domain =
        register_homes::FixedPrecoloredHomeDomainId(99);
    corruptions.push(plan);
    let mut plan = canonical.plan().clone();
    plan.functions[0].assignments[0].view = register_model::RegisterViewId(99);
    corruptions.push(plan);
    for corruption in corruptions {
        assert_eq!(
            validate(&fixture, corruption),
            Err(selected_instructions_to_register_homes::FixedPrecoloredSegmentHomeError::NonCanonicalFunctions)
        );
    }

    let mut usage = canonical.plan().clone();
    usage.usage.iterations += 1;
    assert_eq!(
        validate(&fixture, usage),
        Err(
            selected_instructions_to_register_homes::FixedPrecoloredSegmentHomeError::UsageMismatch
        )
    );

    let other = source(NativeTarget::linux_arm64());
    assert_eq!(
        validate(&other, canonical.plan().clone()),
        Err(selected_instructions_to_register_homes::FixedPrecoloredSegmentHomeError::RootMismatch)
    );
}
