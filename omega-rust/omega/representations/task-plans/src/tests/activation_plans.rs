use super::{candidate, id, wcsu_projection};
use crate::{
    ActivationCarryObligations, StackRepresentationId, validate_activation_plan,
    validate_wcsu_activation_plan,
};

#[test]
fn fixed_stack_and_canonical_crossings_form_a_valid_plan() {
    let plan = validate_activation_plan(candidate()).expect("activation plan");
    assert_eq!(plan.candidate().stack_plan.bytes, 4096);
    assert_eq!(plan.wcsu_stack_projection(), None);
    assert!(plan.candidate().carry_obligations.preserve_cpu);
    assert_ne!(plan.normalized_identity().normalized_identity(), 0);
}

#[test]
fn wcsu_projection_validates_exact_activation_stack_shape() {
    let projection = wcsu_projection(31);
    let projection_identity = projection.identity();
    let mut exact = candidate();
    exact.stack_plan = projection.stack_plan();
    let plan = validate_wcsu_activation_plan(exact.clone(), projection)
        .expect("WCSU-backed activation plan");

    assert_eq!(
        plan.wcsu_stack_projection().map(|value| value.identity()),
        Some(projection_identity)
    );

    let mut changed_bytes = exact.clone();
    changed_bytes.stack_plan.bytes += 1;
    assert!(
        validate_wcsu_activation_plan(changed_bytes, wcsu_projection(31))
            .expect_err("byte substitution")
            .0
            .contains("does not exactly match")
    );

    let mut changed_representation = exact;
    changed_representation.stack_plan.representation =
        id(32, StackRepresentationId::from_normalized_identity);
    assert!(
        validate_wcsu_activation_plan(changed_representation, wcsu_projection(31))
            .expect_err("representation substitution")
            .0
            .contains("does not exactly match")
    );
}

#[test]
fn stack_and_crossing_validation_fail_closed() {
    let mut zero_stack = candidate();
    zero_stack.stack_plan.bytes = 0;
    assert!(
        validate_activation_plan(zero_stack)
            .expect_err("zero stack")
            .0
            .contains("stack size")
    );

    let mut missing_crossing = candidate();
    missing_crossing.canonical_suspension_crossings.clear();
    missing_crossing.carry_obligations = ActivationCarryObligations::none();
    assert!(
        validate_activation_plan(missing_crossing)
            .expect_err("missing crossing")
            .0
            .contains("no canonical")
    );

    let mut unsafe_crossing = candidate();
    unsafe_crossing.canonical_suspension_crossings[0].suspension_allowed = false;
    assert!(
        validate_activation_plan(unsafe_crossing)
            .expect_err("unsafe crossing")
            .0
            .contains("forbids suspension")
    );

    let mut understated = candidate();
    understated.carry_obligations.preserve_cpu = false;
    assert!(
        validate_activation_plan(understated)
            .expect_err("understated preservation")
            .0
            .contains("understates")
    );
}

#[test]
fn normalized_plan_identity_binds_stack_crossings_and_preservation() {
    let plan = validate_activation_plan(candidate()).expect("activation plan");

    let mut changed_stack = candidate();
    changed_stack.stack_plan.bytes += 1;
    assert_ne!(
        plan.normalized_identity(),
        validate_activation_plan(changed_stack)
            .expect("changed stack")
            .normalized_identity()
    );

    let mut changed_crossing = candidate();
    changed_crossing.canonical_suspension_crossings[0].preserve_host_thread = true;
    changed_crossing.carry_obligations.preserve_host_thread = true;
    assert_ne!(
        plan.normalized_identity(),
        validate_activation_plan(changed_crossing)
            .expect("changed crossing")
            .normalized_identity()
    );
}

#[test]
fn activation_identity_binds_exact_wcsu_projection_identity() {
    let first_projection = wcsu_projection(40);
    let second_projection = wcsu_projection(41);
    assert_eq!(
        first_projection.stack_plan(),
        second_projection.stack_plan()
    );
    assert_ne!(first_projection.identity(), second_projection.identity());

    let mut first_candidate = candidate();
    first_candidate.stack_plan = first_projection.stack_plan();
    let mut second_candidate = candidate();
    second_candidate.stack_plan = second_projection.stack_plan();
    let first = validate_wcsu_activation_plan(first_candidate, first_projection)
        .expect("first WCSU-backed activation");
    let second = validate_wcsu_activation_plan(second_candidate, second_projection)
        .expect("second WCSU-backed activation");

    assert_ne!(first.normalized_identity(), second.normalized_identity());
}
