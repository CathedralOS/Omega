use super::{STRUCTURAL_INTEGER_STORE_SOURCE, check_dynamic_source, sole_direct_dynamic_plan};
use checked_trees::CheckedDynamicBinding::{Direct, Joined, Rebound};
use checked_trees::CheckedDynamicDispatchPlan::{Scalar, Unit};

#[test]
fn structural_field_store_planning_fails_closed_on_source_disagreement() {
    let checked = check_dynamic_source(
        r#"
        trait Shape {
            machine code(&self) -> i32;
        }

        data Item {
            value: i32;
        }

        Primary: Item satisfies Shape {
            machine code(&self) -> i32 {
                transition { _ -> self.value }
            }
        }

        data Main {
            selected: Item;
            other: Item;
        }

        machine Main::run(&mut self) {
            self.other.value = 17;
            let erased: &dyn Shape = &self.selected as &dyn Item::Primary;
            let result: i32 = erased.code();
        }
        "#,
    );
    assert!(
        sole_direct_dynamic_plan(&checked)
            .caller_structural_scalar_field_store
            .is_none(),
        "a store into a different carrier must not gain checked store custody"
    );
}

#[test]
fn structural_field_store_planning_rejects_tampered_checked_evidence() {
    let mut mutation_tampered = check_dynamic_source(STRUCTURAL_INTEGER_STORE_SOURCE);
    let caller_machine = sole_direct_dynamic_plan(&mutation_tampered).caller_machine;
    let caller_state = sole_direct_dynamic_plan(&mutation_tampered).caller_state;
    let mutation = mutation_tampered
        .facts
        .mutation
        .machines
        .iter_mut()
        .find(|mutation| mutation.machine == caller_machine)
        .expect("caller mutation fact");
    let state_frame = mutation
        .state_write_frames
        .iter_mut()
        .find(|frame| frame.state == caller_state)
        .expect("caller state mutation frame");
    state_frame.frame = facts::NormalizedWriteFrame::opaque();
    mutation_tampered = crate::settle_checked_execution(mutation_tampered)
        .expect("full rebuild suppresses the store with opaque mutation custody");
    assert!(
        sole_direct_dynamic_plan(&mutation_tampered)
            .caller_structural_scalar_field_store
            .is_none(),
        "opaque mutation custody must suppress the optional store plan"
    );

    let mut scalar_tampered = check_dynamic_source(STRUCTURAL_INTEGER_STORE_SOURCE);
    let caller_state = sole_direct_dynamic_plan(&scalar_tampered).caller_state;
    let assignment_value = scalar_tampered
        .facts
        .values
        .scalar_expressions
        .expressions
        .iter_mut()
        .find(|expression| {
            expression.state == caller_state
                && expression.statement_ordinal == 0
                && expression.role == checked_trees::CheckedScalarExpressionRole::AssignmentValue
        })
        .expect("checked assignment scalar expression");
    assignment_value.expression = checked_trees::CheckedScalarExpression::Boolean(Box::new(
        checked_trees::CheckedBooleanExpression::Constant(true),
    ));
    scalar_tampered = crate::settle_checked_execution(scalar_tampered)
        .expect("full rebuild suppresses the store with wrong-typed scalar custody");
    assert!(
        sole_direct_dynamic_plan(&scalar_tampered)
            .caller_structural_scalar_field_store
            .is_none(),
        "wrong-typed scalar custody must suppress the optional store plan"
    );
}

#[test]
fn descriptor_transfer_retains_one_parameter_forwarding_hop() {
    let checked = check_dynamic_source(
        r#"
        trait Shape {
            machine code(&self) -> i32;
        }

        data Item { value: i32; }

        Primary: Item satisfies Shape {
            machine code(&self) -> i32 { transition { _ -> self.value } }
        }

        data Main { item: Item; }

        machine Main::run(&self) {
            let erased: &dyn Shape = &self.item as &dyn Item::Primary;
            let result: i32 = forward(erased);
        }

        machine forward(erased: &dyn Shape) -> i32 {
            let result: i32 = finish(erased);
            transition { _ -> result }
        }

        machine finish(erased: &dyn Shape) -> i32 {
            let result: i32 = erased.code();
            transition { _ -> result }
        }
        "#,
    );
    let dynamic = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    let [selection_transfer, parameter_transfer] = dynamic.transfers.as_slice() else {
        panic!("two ordered descriptor transfers expected, got {dynamic:#?}")
    };
    assert!(matches!(
        selection_transfer.source,
        checked_trees::CheckedDynamicDescriptorTransferSource::Selection
    ));
    assert!(matches!(
        parameter_transfer.source,
        checked_trees::CheckedDynamicDescriptorTransferSource::Parameter {
            parameter_position: 0
        }
    ));
    assert_eq!(
        selection_transfer.target_machine,
        parameter_transfer.caller_machine
    );
    assert_eq!(
        selection_transfer.target_state,
        parameter_transfer.caller_state
    );
    assert_eq!(
        selection_transfer.parameter,
        parameter_transfer.source_binding
    );
    assert_eq!(
        selection_transfer.sole_selection(),
        parameter_transfer.sole_selection()
    );
    let [Scalar(Direct(plan))] = dynamic.calls.as_slice() else {
        panic!("one multi-hop dynamic scalar call expected, got {dynamic:#?}")
    };
    assert_eq!(
        plan.forwarding_transfers.as_slice(),
        std::slice::from_ref(parameter_transfer)
    );
    let checked_trees::CheckedDynamicScalarCallOrigin::Forwarded {
        machine,
        state,
        parameter,
        ..
    } = plan.origin
    else {
        panic!("multi-hop call must retain its final dynamic helper")
    };
    assert_eq!(parameter_transfer.target_machine, machine);
    assert_eq!(parameter_transfer.target_state, state);
    assert_eq!(parameter_transfer.parameter, parameter);
    assert!(
        dynamic
            .calls
            .iter()
            .all(|plan| !matches!(plan, Scalar(Rebound { .. })))
    );
}

#[test]
fn descriptor_transfer_fences_join_with_an_unadmitted_third_predecessor() {
    let checked = check_dynamic_source(
        r#"
        trait Shape {
            machine code(&self) -> i32;
        }

        data Item { value: i32; }

        Primary: Item satisfies Shape {
            machine code(&self) -> i32 { transition { _ -> self.value } }
        }

        data Main { first: Item; second: Item; }

        machine Main::run(&self, ambient: &dyn Shape, choice: u8) {
            let selected_first: &dyn Shape = &self.first as &dyn Item::Primary;
            let selected_second: &dyn Shape = &self.second as &dyn Item::Primary;
            transition choice {
                0 -> join(selected_first)
                1 -> join(selected_second)
                _ -> join(ambient)
            }

            state join(&self, erased: &dyn Shape) {
                let result: i32 = finish(erased);
            }
        }

        machine finish(erased: &dyn Shape) -> i32 {
            let result: i32 = erased.code();
            transition { _ -> result }
        }
        "#,
    );
    let dynamic = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    assert_eq!(
        dynamic
            .transfers
            .iter()
            .filter(|transfer| {
                transfer.source == checked_trees::CheckedDynamicDescriptorTransferSource::Selection
            })
            .count(),
        2
    );
    assert!(dynamic.transfers.iter().all(|transfer| {
        transfer.source
            != checked_trees::CheckedDynamicDescriptorTransferSource::Parameter {
                parameter_position: 0,
            }
    }));
    assert!(
        dynamic
            .calls
            .iter()
            .all(|plan| !matches!(plan, Scalar(Direct(_))))
    );
    assert!(
        dynamic
            .calls
            .iter()
            .all(|plan| !matches!(plan, Scalar(Rebound { .. })))
    );
}

#[test]
fn descriptor_transfer_fences_a_second_join_over_joined_custody() {
    let checked = check_dynamic_source(
        r#"
        trait Shape {
            machine code(&self) -> i32;
        }

        data Item { value: i32; }

        Primary: Item satisfies Shape {
            machine code(&self) -> i32 { transition { _ -> self.value } }
        }

        data Main { first: Item; second: Item; third: Item; }

        machine Main::run(&self, choose_first: bool, choose_third: bool) {
            let selected_first: &dyn Shape = &self.first as &dyn Item::Primary;
            let selected_second: &dyn Shape = &self.second as &dyn Item::Primary;
            transition choose_first {
                true -> first_join(selected_first, choose_third)
                _ -> first_join(selected_second, choose_third)
            }

            state first_join(&self, erased: &dyn Shape, choose_third: bool) {
                let selected_third: &dyn Shape = &self.third as &dyn Item::Primary;
                transition choose_third {
                    true -> second_join(erased)
                    _ -> second_join(selected_third)
                }
            }

            state second_join(&self, erased: &dyn Shape) {
                let result: i32 = finish(erased);
            }
        }

        machine finish(erased: &dyn Shape) -> i32 {
            let result: i32 = erased.code();
            transition { _ -> result }
        }
        "#,
    );
    let dynamic = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    let first_join = dynamic
        .transfers
        .iter()
        .find(|transfer| transfer.source_paths.len() == 2)
        .expect("the first exact join must be retained");
    assert!(first_join.has_complete_source_custody(&dynamic.transfers));
    assert!(!dynamic.transfers.iter().any(|transfer| {
        transfer.caller_state == first_join.target_state
            && matches!(
                transfer.source,
                checked_trees::CheckedDynamicDescriptorTransferSource::Parameter {
                    parameter_position: 0
                }
            )
    }));
    assert!(
        dynamic
            .calls
            .iter()
            .all(|plan| !matches!(plan, Scalar(Direct(_))))
    );
    assert!(
        dynamic
            .calls
            .iter()
            .all(|plan| !matches!(plan, Scalar(Rebound { .. })))
    );
}

#[test]
fn two_branch_dynamic_unit_calls_share_one_checked_join() {
    let checked = check_dynamic_source(
        r#"
        trait Touch { machine touch(&self); }

        data Item { value: i32; }

        Primary: Item satisfies Touch { machine touch(&self) {} }
        Secondary: Item satisfies Touch { machine touch(&self) {} }

        data Main { first: Item; second: Item; }

        machine Main::run(&self, choose_first: bool) {
            transition choose_first {
                true -> take_first()
                _ -> take_second()
            }

            state take_first(&self) {
                let selected: &dyn Touch = &self.first as &dyn Item::Primary;
                finish(selected);
            }

            state take_second(&self) {
                let selected: &dyn Touch = &self.second as &dyn Item::Secondary;
                finish(selected);
            }
        }

        machine finish(erased: &dyn Touch) { erased.touch(); }
        "#,
    );
    let dynamic = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    let [
        Unit(Joined {
            control,
            when_true,
            when_false,
        }),
    ] = dynamic.calls.as_slice()
    else {
        panic!("one atomic result-less join expected: {dynamic:#?}")
    };
    assert_ne!(
        when_true.call.selection.conformance,
        when_false.call.selection.conformance,
    );
    assert_eq!(control.scalar_parameters.len(), 1);
    assert_eq!(
        when_true.call.forwarding_transfers,
        when_false.call.forwarding_transfers,
    );
}

#[test]
fn two_branch_dynamic_calls_under_a_negated_equality_guard_share_one_join() {
    let checked = check_dynamic_source(
        r#"
        trait Shape {
            machine code(&self) -> i32;
        }

        data Item { value: i32; }

        Primary: Item satisfies Shape {
            machine code(&self) -> i32 { transition { _ -> self.value } }
        }

        Secondary: Item satisfies Shape {
            machine code(&self) -> i32 { transition { _ -> self.value } }
        }

        data Main { first: Item; second: Item; }

        machine Main::run(&self, choose_first: bool) {
            transition choose_first != false {
                true -> take_first()
                _ -> take_second()
            }

            state take_first(&self) {
                let selected: &dyn Shape = &self.first as &dyn Item::Primary;
                let result: i32 = finish(selected);
            }

            state take_second(&self) {
                let selected: &dyn Shape = &self.second as &dyn Item::Secondary;
                let result: i32 = finish(selected);
            }
        }

        machine finish(erased: &dyn Shape) -> i32 {
            let result: i32 = erased.code();
            transition { _ -> result }
        }
        "#,
    );
    let dynamic = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    let [
        Scalar(Joined {
            control,
            when_true,
            when_false,
        }),
    ] = dynamic.calls.as_slice()
    else {
        panic!("one atomic joined call expected: {dynamic:#?}")
    };
    assert_eq!(control.scalar_parameters.len(), 1);
    let checked_trees::CheckedScalarExpression::Boolean(boolean) = &control.guard else {
        panic!(
            "the negated equality guard is a Boolean scalar: {:?}",
            control.guard
        )
    };
    assert!(matches!(
        boolean.as_ref(),
        checked_trees::CheckedBooleanExpression::Not(inner)
            if matches!(
                inner.as_ref(),
                checked_trees::CheckedBooleanExpression::Equal { left, right }
                    if matches!(
                        left.as_ref(),
                        checked_trees::CheckedBooleanExpression::Parameter { position: 0 }
                    ) && matches!(
                        right.as_ref(),
                        checked_trees::CheckedBooleanExpression::Constant(false)
                    )
            )
    ));
    assert_ne!(
        when_true.call.selection.conformance,
        when_false.call.selection.conformance,
    );
}

#[test]
fn two_branch_dynamic_calls_under_a_field_and_parameter_equality_guard_share_one_join() {
    let checked = check_dynamic_source(
        r#"
        trait Shape {
            machine code(&self) -> i32;
        }

        data Item { value: i32; }

        Primary: Item satisfies Shape {
            machine code(&self) -> i32 { transition { _ -> self.value } }
        }

        Secondary: Item satisfies Shape {
            machine code(&self) -> i32 { transition { _ -> self.value } }
        }

        data Main { flag: bool; first: Item; second: Item; }

        machine Main::run(&self, pick: bool) {
            transition self.flag == pick {
                true -> take_first()
                _ -> take_second()
            }

            state take_first(&self) {
                let selected: &dyn Shape = &self.first as &dyn Item::Primary;
                let result: i32 = finish(selected);
            }

            state take_second(&self) {
                let selected: &dyn Shape = &self.second as &dyn Item::Secondary;
                let result: i32 = finish(selected);
            }
        }

        machine finish(erased: &dyn Shape) -> i32 {
            let result: i32 = erased.code();
            transition { _ -> result }
        }
        "#,
    );
    let dynamic = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    let [
        Scalar(Joined {
            control,
            when_true,
            when_false,
        }),
    ] = dynamic.calls.as_slice()
    else {
        panic!("one atomic joined call expected: {dynamic:#?}")
    };
    assert_eq!(control.scalar_parameters.len(), 1);
    let checked_trees::CheckedScalarExpression::Boolean(boolean) = &control.guard else {
        panic!(
            "the field-parameter equality guard is a Boolean scalar: {:?}",
            control.guard
        )
    };
    assert!(matches!(
        boolean.as_ref(),
        checked_trees::CheckedBooleanExpression::Equal { left, right }
            if matches!(
                left.as_ref(),
                checked_trees::CheckedBooleanExpression::StructuralParameterField {
                    parameter_position: 0,
                    ..
                }
            ) && matches!(
                right.as_ref(),
                checked_trees::CheckedBooleanExpression::Parameter { position: 0 }
            )
    ));
    assert_ne!(
        when_true.call.selection.conformance,
        when_false.call.selection.conformance,
    );
}

#[test]
fn two_branch_dynamic_calls_under_an_indexed_field_guard_share_one_join() {
    let checked = check_dynamic_source(
        r#"
        trait Shape {
            machine code(&self) -> i32;
        }

        data Item { value: i32; }

        Primary: Item satisfies Shape {
            machine code(&self) -> i32 { transition { _ -> self.value } }
        }

        Secondary: Item satisfies Shape {
            machine code(&self) -> i32 { transition { _ -> self.value } }
        }

        data Slot { flag: bool; }

        data Main { first: Item; second: Item; items: [Slot; 2]; }

        machine Main::run(&self) {
            transition self.items[0].flag {
                true -> take_first()
                _ -> take_second()
            }

            state take_first(&self) {
                let selected: &dyn Shape = &self.first as &dyn Item::Primary;
                let result: i32 = finish(selected);
            }

            state take_second(&self) {
                let selected: &dyn Shape = &self.second as &dyn Item::Secondary;
                let result: i32 = finish(selected);
            }
        }

        machine finish(erased: &dyn Shape) -> i32 {
            let result: i32 = erased.code();
            transition { _ -> result }
        }
        "#,
    );
    let dynamic = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    let [
        Scalar(Joined {
            control,
            when_true,
            when_false,
        }),
    ] = dynamic.calls.as_slice()
    else {
        panic!("one atomic joined call expected: {dynamic:#?}")
    };
    let checked_trees::CheckedScalarExpression::Boolean(boolean) = &control.guard else {
        panic!(
            "the indexed field guard is a Boolean scalar: {:?}",
            control.guard
        )
    };
    assert!(matches!(
        boolean.as_ref(),
        checked_trees::CheckedBooleanExpression::StructuralParameterField {
            parameter_position: 0,
            path,
        } if path.len() == 3
            && matches!(path[0], checked_trees::CheckedStructuralPredicatePathSegment::Field(_))
            && matches!(path[1], checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(0))
            && matches!(path[2], checked_trees::CheckedStructuralPredicatePathSegment::Field(_))
    ));
    assert_ne!(
        when_true.call.selection.conformance,
        when_false.call.selection.conformance,
    );
}

#[test]
fn two_branch_dynamic_calls_under_an_indexed_element_guard_share_one_join() {
    let checked = check_dynamic_source(
        r#"
        trait Shape {
            machine code(&self) -> i32;
        }

        data Item { value: i32; }

        Primary: Item satisfies Shape {
            machine code(&self) -> i32 { transition { _ -> self.value } }
        }

        Secondary: Item satisfies Shape {
            machine code(&self) -> i32 { transition { _ -> self.value } }
        }

        data Main { first: Item; second: Item; counts: [u64; 4]; }

        machine Main::run(&self) {
            transition self.counts[2] < 5 {
                true -> take_first()
                _ -> take_second()
            }

            state take_first(&self) {
                let selected: &dyn Shape = &self.first as &dyn Item::Primary;
                let result: i32 = finish(selected);
            }

            state take_second(&self) {
                let selected: &dyn Shape = &self.second as &dyn Item::Secondary;
                let result: i32 = finish(selected);
            }
        }

        machine finish(erased: &dyn Shape) -> i32 {
            let result: i32 = erased.code();
            transition { _ -> result }
        }
        "#,
    );
    let dynamic = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    let [
        Scalar(Joined {
            control,
            when_true,
            when_false,
        }),
    ] = dynamic.calls.as_slice()
    else {
        panic!("one atomic joined call expected: {dynamic:#?}")
    };
    let checked_trees::CheckedScalarExpression::Boolean(boolean) = &control.guard else {
        panic!(
            "the indexed element guard is a Boolean scalar: {:?}",
            control.guard
        )
    };
    assert!(matches!(
        boolean.as_ref(),
        checked_trees::CheckedBooleanExpression::IntegerComparison {
            kind: checked_trees::CheckedIntegerComparisonKind::LessThan,
            left,
            right,
        } if matches!(
            left.as_ref(),
            checked_trees::CheckedScalarExpression::StructuralParameterField {
                parameter_position: 0,
                ..
            }
        ) && matches!(
            right.as_ref(),
            checked_trees::CheckedScalarExpression::IntegerLiteral { .. }
        )
    ));
    assert_ne!(
        when_true.call.selection.conformance,
        when_false.call.selection.conformance,
    );
}

#[test]
fn two_branch_dynamic_calls_under_a_local_boolean_guard_share_one_join() {
    let checked = check_dynamic_source(
        r#"
        trait Shape {
            machine code(&self) -> i32;
        }

        data Item { value: i32; }

        Primary: Item satisfies Shape {
            machine code(&self) -> i32 { transition { _ -> self.value } }
        }

        Secondary: Item satisfies Shape {
            machine code(&self) -> i32 { transition { _ -> self.value } }
        }

        data Main { first: Item; second: Item; }

        machine Main::run(&self, pick: u64) {
            let small: bool = pick < 5;
            transition small {
                true -> take_first()
                _ -> take_second()
            }

            state take_first(&self) {
                let selected: &dyn Shape = &self.first as &dyn Item::Primary;
                let result: i32 = finish(selected);
            }

            state take_second(&self) {
                let selected: &dyn Shape = &self.second as &dyn Item::Secondary;
                let result: i32 = finish(selected);
            }
        }

        machine finish(erased: &dyn Shape) -> i32 {
            let result: i32 = erased.code();
            transition { _ -> result }
        }
        "#,
    );
    let dynamic = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    let [
        Scalar(Joined {
            control,
            when_true,
            when_false,
        }),
    ] = dynamic.calls.as_slice()
    else {
        panic!("one atomic joined call expected: {dynamic:#?}")
    };
    let checked_trees::CheckedScalarExpression::Boolean(boolean) = &control.guard else {
        panic!(
            "the local boolean guard is a Boolean scalar: {:?}",
            control.guard
        )
    };
    assert!(matches!(
        boolean.as_ref(),
        checked_trees::CheckedBooleanExpression::IntegerComparison {
            kind: checked_trees::CheckedIntegerComparisonKind::LessThan,
            left,
            right,
        } if matches!(
            left.as_ref(),
            checked_trees::CheckedScalarExpression::Parameter { position: 0, .. }
        ) && matches!(
            right.as_ref(),
            checked_trees::CheckedScalarExpression::IntegerLiteral { .. }
        )
    ));
    assert_ne!(
        when_true.call.selection.conformance,
        when_false.call.selection.conformance,
    );
}

#[test]
fn two_branch_dynamic_calls_under_a_swapped_branch_guard_share_one_join() {
    let checked = check_dynamic_source(
        r#"
        trait Shape {
            machine code(&self) -> i32;
        }

        data Item { value: i32; }

        Primary: Item satisfies Shape {
            machine code(&self) -> i32 { transition { _ -> self.value } }
        }

        Secondary: Item satisfies Shape {
            machine code(&self) -> i32 { transition { _ -> self.value } }
        }

        data Main { first: Item; second: Item; }

        machine Main::run(&self, pick: u64) {
            transition pick != 5 {
                true -> take_second()
                _ -> take_first()
            }

            state take_first(&self) {
                let selected: &dyn Shape = &self.first as &dyn Item::Primary;
                let result: i32 = finish(selected);
            }

            state take_second(&self) {
                let selected: &dyn Shape = &self.second as &dyn Item::Secondary;
                let result: i32 = finish(selected);
            }
        }

        machine finish(erased: &dyn Shape) -> i32 {
            let result: i32 = erased.code();
            transition { _ -> result }
        }
        "#,
    );
    let dynamic = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    let [
        Scalar(Joined {
            when_true,
            when_false,
            ..
        }),
    ] = dynamic.calls.as_slice()
    else {
        panic!("one atomic joined call expected: {dynamic:#?}")
    };
    let take_first = checked
        .machines()
        .iter()
        .flat_map(|machine| checked.machine_states(machine))
        .find(|state| state.name.as_str() == "take_first")
        .map(|state| state.symbol)
        .expect("take_first state");
    let take_second = checked
        .machines()
        .iter()
        .flat_map(|machine| checked.machine_states(machine))
        .find(|state| state.name.as_str() == "take_second")
        .map(|state| state.symbol)
        .expect("take_second state");
    assert_eq!(when_true.successor.target_state, take_second);
    assert_eq!(when_false.successor.target_state, take_first);
    assert_ne!(
        when_true.call.selection.conformance,
        when_false.call.selection.conformance,
    );
}
