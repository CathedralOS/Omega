//! Owned structural parameters projected into calls keep their untouched
//! complement as residual cleanup on the caller's return edge. The partial
//! terminator closes exactly one such root, so a second partially moved
//! parameter has no lane; an untouched sibling still dies whole on the same
//! edge as an ordinary trivial discard.
use crate::tests::flow::terminal_unit::{
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralPathSegment, checked, machine_named,
};

#[test]
fn projected_parameter_with_untouched_sibling_lands_in_the_partial_lane() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        data Pair { left: Token; right: Token; }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::enter(first: Pair, second: Pair) {
            Sink::take(first.left);
        }
        "#,
    );
    let machine = machine_named(&checked, "enter");
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine)
            .is_none(),
        "residual-bearing machines stay out of the root-only lane"
    );
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine)
        .expect("the parameter's complement owns the partial lane");
    let [
        CheckedUnitEffectOperationPlan::CallUnit {
            structural_arguments,
            ..
        },
        CheckedUnitEffectOperationPlan::Complete {
            trivial_affine_discards,
            ..
        },
    ] = plan.machine.operations.as_slice()
    else {
        panic!("one projected call and a completion")
    };
    let [argument] = structural_arguments.as_slice() else {
        panic!("one projected argument")
    };
    assert_eq!(
        argument.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 }
    );
    assert_eq!(
        argument.path.as_slice(),
        [CheckedUnitStructuralPathSegment::Field("left".to_owned())]
    );
    let [residual] = plan.residual_affine_discards.as_slice() else {
        panic!("the untouched sibling field is the residual")
    };
    assert_eq!(residual.source, argument.source);
    assert_eq!(
        residual.path.as_slice(),
        [CheckedUnitStructuralPathSegment::Field("right".to_owned())]
    );
    // The never-touched sibling parameter still dies whole on the same edge.
    assert_eq!(trivial_affine_discards.as_slice(), [1]);
}

#[test]
fn a_later_parameter_may_own_the_residual_root() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        data Pair { left: Token; right: Token; }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::enter(first: Pair, second: Pair) {
            Sink::take(second.left);
        }
        "#,
    );
    let machine = machine_named(&checked, "enter");
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine)
        .expect("the later parameter's complement owns the partial lane");
    let [residual] = plan.residual_affine_discards.as_slice() else {
        panic!("one residual field")
    };
    assert_eq!(
        residual.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 1 }
    );
    assert_eq!(
        residual.path.as_slice(),
        [CheckedUnitStructuralPathSegment::Field("right".to_owned())]
    );
    let Some(CheckedUnitEffectOperationPlan::Complete {
        trivial_affine_discards,
        ..
    }) = plan.machine.operations.last()
    else {
        panic!("the completion owns the first parameter's whole discard")
    };
    assert_eq!(trivial_affine_discards.as_slice(), [0]);
}

#[test]
fn a_projected_parameter_shares_one_consumer_with_a_dying_temporary() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        data Pair { left: Token; right: Token; }
        data Sink {}
        machine Sink::take2(first: Token, second: Token) {}
        data Root {}
        machine Root::forward(value: Pair) -> Pair { value }
        machine Root::enter(first: Pair, second: Pair) {
            Sink::take2(Root::forward(second).right, first.left);
        }
        "#,
    );
    let machine = machine_named(&checked, "enter");
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine)
            .is_none(),
        "the parameter residual keeps the machine out of the ordinary roster"
    );
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine)
        .expect("mixed dying roots still own the partial lane");
    let [
        CheckedUnitEffectOperationPlan::StructuralCall { .. },
        CheckedUnitEffectOperationPlan::CallUnit {
            structural_arguments,
            ..
        },
        CheckedUnitEffectOperationPlan::CallContinuationCleanup {
            affine_discards, ..
        },
        CheckedUnitEffectOperationPlan::Complete {
            trivial_affine_discards,
            ..
        },
    ] = plan.machine.operations.as_slice()
    else {
        panic!("producer, shared consumer, continuation cleanup, completion")
    };
    let [temporary_argument, parameter_argument] = structural_arguments.as_slice() else {
        panic!("the consumer mixes a temporary and a parameter operand")
    };
    assert_eq!(
        temporary_argument.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: 0
        }
    );
    assert_eq!(
        parameter_argument.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 }
    );
    // The temporary's complement dies on the consumer's continuation; only
    // the parameter's complement survives to the return edge.
    assert!(affine_discards.iter().all(|discard| matches!(
        discard.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult { .. }
    )));
    let [residual] = plan.residual_affine_discards.as_slice() else {
        panic!("the parameter keeps exactly one residual field")
    };
    assert_eq!(residual.source, parameter_argument.source);
    assert_eq!(
        residual.path.as_slice(),
        [CheckedUnitStructuralPathSegment::Field("right".to_owned())]
    );
    // `second` moved whole into its producer; `first` owns the residual.
    assert!(trivial_affine_discards.is_empty());
}

#[test]
fn a_root_moved_whole_after_projection_is_fully_consumed() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        data Pair { left: Token; right: Token; }
        data Sink {}
        machine Sink::take(token: Token) {}
        machine Sink::take_rest(pair: Pair) {}
        data Root {}
        machine Root::enter(first: Pair) {
            Sink::take(first.left);
            Sink::take_rest(first);
        }
        "#,
    );
    let machine = machine_named(&checked, "enter");
    assert!(
        checked
            .facts
            .flow
            .terminal_partial_affine_unit_cleanups
            .for_machine(machine)
            .is_none(),
        "the whole move consumes the complement; no residual remains"
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
        .expect("fully consumed roots keep the ordinary lane");
    let Some(CheckedUnitEffectOperationPlan::Complete {
        trivial_affine_discards,
        ..
    }) = plan.operations.last()
    else {
        panic!("the completion owns no discards")
    };
    assert!(trivial_affine_discards.is_empty());
}

#[test]
fn two_partial_parameter_roots_have_no_cleanup_lane() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        data Pair { left: Token; right: Token; }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::enter(first: Pair, second: Pair) {
            Sink::take(first.left);
            Sink::take(second.left);
        }
        "#,
    );
    let machine = machine_named(&checked, "enter");
    assert!(
        checked
            .facts
            .flow
            .terminal_partial_affine_unit_cleanups
            .for_machine(machine)
            .is_none(),
        "the partial terminator closes exactly one root"
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine)
            .is_none(),
        "the ordinary lane cannot publish a second partial root either"
    );
}
