use super::{STRUCTURAL_INTEGER_STORE_SOURCE, check_dynamic_source, sole_direct_dynamic_plan};

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
    mutation_tampered =
        crate::settle_checked_execution(mutation_tampered, &crate::ExecutionSettlement::default())
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
    scalar_tampered =
        crate::settle_checked_execution(scalar_tampered, &crate::ExecutionSettlement::default())
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
    let [plan] = dynamic.direct_scalar_calls.as_slice() else {
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
    assert!(dynamic.rebound_scalar_calls.is_empty());
}

#[test]
fn descriptor_transfer_retains_one_unit_parameter_forwarding_hop() {
    let checked = check_dynamic_source(
        r#"
        trait Touch { machine touch(&self); }
        data Item { value: i32; }
        Primary: Item satisfies Touch { machine touch(&self) {} }
        data Main { item: Item; }

        machine Main::run(&self) {
            let erased: &dyn Touch = &self.item as &dyn Item::Primary;
            forward(erased);
        }

        machine forward(erased: &dyn Touch) {
            finish(erased);
        }

        machine finish(erased: &dyn Touch) {
            erased.touch();
        }
        "#,
    );
    let dynamic = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    let [selection_transfer, parameter_transfer] = dynamic.transfers.as_slice() else {
        panic!("two ordered Unit descriptor transfers expected, got {dynamic:#?}")
    };
    let [plan] = dynamic.direct_unit_calls.as_slice() else {
        panic!("one multi-hop dynamic Unit call expected, got {dynamic:#?}")
    };
    assert_eq!(
        plan.forwarding_transfers.as_slice(),
        std::slice::from_ref(parameter_transfer)
    );
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
    let checked_trees::CheckedDynamicUnitCallOrigin::Forwarded {
        machine,
        state,
        parameter,
        ..
    } = plan.origin
    else {
        panic!("multi-hop Unit call must retain its final dynamic helper")
    };
    assert_eq!(parameter_transfer.target_machine, machine);
    assert_eq!(parameter_transfer.target_state, state);
    assert_eq!(parameter_transfer.parameter, parameter);
    assert!(dynamic.rebound_unit_calls.is_empty());
}

#[test]
fn descriptor_transfer_retains_every_control_flow_join_alternative() {
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
            let selected_first: &dyn Shape = &self.first as &dyn Item::Primary;
            let selected_second: &dyn Shape = &self.second as &dyn Item::Secondary;
            transition choose_first {
                true -> join(selected_first)
                _ -> join(selected_second)
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
    let roots = dynamic
        .transfers
        .iter()
        .filter(|transfer| {
            transfer.source == checked_trees::CheckedDynamicDescriptorTransferSource::Selection
        })
        .collect::<Vec<_>>();
    let joined = dynamic
        .transfers
        .iter()
        .filter(|transfer| {
            matches!(
                transfer.source,
                checked_trees::CheckedDynamicDescriptorTransferSource::Parameter {
                    parameter_position: 0
                }
            )
        })
        .collect::<Vec<_>>();
    let [first_root, second_root] = roots.as_slice() else {
        panic!("two exact incoming selections expected, got {dynamic:#?}")
    };
    let [joined] = joined.as_slice() else {
        panic!("one joined outgoing descriptor edge expected, got {dynamic:#?}")
    };
    let (first_root, second_root, joined) = (*first_root, *second_root, *joined);
    assert_eq!(first_root.target_state, second_root.target_state);
    assert_eq!(first_root.target_state, joined.caller_state);
    assert_eq!(first_root.source_predecessor_count, 0);
    assert_eq!(second_root.source_predecessor_count, 0);
    assert_eq!(joined.source_predecessor_count, 2);
    assert_eq!(first_root.parameter, joined.source_binding);
    assert_eq!(second_root.parameter, joined.source_binding);
    assert_eq!(joined.source_paths.len(), 2);
    assert!(joined.source_paths.iter().all(|path| path.edges.len() == 2));
    assert_eq!(joined.source_paths[0].edges[0], first_root.edge());
    assert_eq!(joined.source_paths[1].edges[0], second_root.edge());
    assert!(
        joined
            .source_paths
            .iter()
            .all(|path| path.edges[1] == joined.edge())
    );
    assert_ne!(
        joined.source_paths[0].selection.source_symbol,
        joined.source_paths[1].selection.source_symbol,
        "the joined descriptor must retain both runtime referents"
    );
    assert_ne!(
        joined.source_paths[0].selection.conformance, joined.source_paths[1].selection.conformance,
        "the joined descriptor must retain every exact selected conformance"
    );
    assert!(joined.has_complete_source_custody(&dynamic.transfers));
    assert!(dynamic.direct_scalar_calls.is_empty());
    assert!(dynamic.rebound_scalar_calls.is_empty());

    let mut missing_path = joined.clone();
    missing_path.source_paths.pop();
    assert!(!missing_path.has_complete_source_custody(&dynamic.transfers));

    let mut substituted_selection = joined.clone();
    substituted_selection.source_paths[1].selection =
        substituted_selection.source_paths[0].selection.clone();
    assert!(!substituted_selection.has_complete_source_custody(&dynamic.transfers));

    let mut substituted_edge = joined.clone();
    substituted_edge.source_paths[0].edges[0]
        .coordinate
        .call_ordinal += 1;
    assert!(!substituted_edge.has_complete_source_custody(&dynamic.transfers));

    let mut substituted_parameter = joined.clone();
    substituted_parameter.source_paths[0].edges[0].parameter = joined.parameter;
    assert!(!substituted_parameter.has_complete_source_custody(&dynamic.transfers));

    let mut substituted_interface = joined.clone();
    substituted_interface.source_paths[0].edges[0].target_trait = symbols::SymbolHandle::default();
    assert!(!substituted_interface.has_complete_source_custody(&dynamic.transfers));

    let mut substituted_predecessor_count = joined.clone();
    substituted_predecessor_count.source_predecessor_count = 1;
    assert!(!substituted_predecessor_count.has_complete_source_custody(&dynamic.transfers));

    let roster_missing_predecessor = dynamic
        .transfers
        .iter()
        .filter(|transfer| transfer.edge() != second_root.edge())
        .cloned()
        .collect::<Vec<_>>();
    assert!(!joined.has_complete_source_custody(&roster_missing_predecessor));
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
    assert!(dynamic.direct_scalar_calls.is_empty());
    assert!(dynamic.rebound_scalar_calls.is_empty());
}

#[test]
fn descriptor_transfer_retains_transparent_forwarding_after_the_join() {
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

        machine Main::run(&self, choose_first: bool) {
            let selected_first: &dyn Shape = &self.first as &dyn Item::Primary;
            let selected_second: &dyn Shape = &self.second as &dyn Item::Primary;
            transition choose_first {
                true -> join(selected_first)
                _ -> join(selected_second)
            }

            state join(&self, erased: &dyn Shape) {
                let result: i32 = relay(erased);
            }
        }

        machine relay(erased: &dyn Shape) -> i32 {
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
    let joined = dynamic
        .transfers
        .iter()
        .find(|transfer| {
            matches!(
                transfer.source,
                checked_trees::CheckedDynamicDescriptorTransferSource::Parameter {
                    parameter_position: 0
                }
            ) && transfer.source_paths.len() == 2
        })
        .expect("the first exact two-way join must be retained");
    assert!(joined.has_complete_source_custody(&dynamic.transfers));
    let forwarded = dynamic
        .transfers
        .iter()
        .find(|transfer| {
            transfer.caller_machine == joined.target_machine
                && transfer.caller_state == joined.target_state
                && matches!(
                    transfer.source,
                    checked_trees::CheckedDynamicDescriptorTransferSource::Parameter {
                        parameter_position: 0
                    }
                )
        })
        .expect("the joined alternatives must cross the next transparent hop");
    assert_eq!(forwarded.source_predecessor_count, 1);
    assert_eq!(forwarded.source_paths.len(), 2);
    assert!(forwarded.has_complete_source_custody(&dynamic.transfers));
    assert!(dynamic.rebound_scalar_calls.is_empty());
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
    assert!(dynamic.direct_scalar_calls.is_empty());
    assert!(dynamic.rebound_scalar_calls.is_empty());
}

#[test]
fn two_branch_dynamic_calls_retain_both_terminal_join_predecessors() {
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
            transition choose_first {
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
    assert!(dynamic.direct_scalar_calls.is_empty(), "{dynamic:#?}");
    assert!(dynamic.rebound_scalar_calls.is_empty());
    assert!(dynamic.stored_scalar_calls.is_empty());
    let [joined] = dynamic.joined_scalar_calls.as_slice() else {
        panic!("one atomic joined call expected: {dynamic:#?}")
    };
    let first = &joined.when_true.call;
    let second = &joined.when_false.call;
    assert_eq!(joined.scalar_parameters.len(), 1);
    assert_ne!(joined.entry_state, first.caller_state);
    assert_ne!(joined.entry_state, second.caller_state);
    assert_eq!(first.caller_machine, second.caller_machine);
    assert_ne!(first.caller_state, second.caller_state);
    assert_eq!(first.origin, second.origin);
    assert_ne!(
        first.selection.source_symbol,
        second.selection.source_symbol
    );
    assert_ne!(first.selection.conformance, second.selection.conformance);
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .structural_types
            .iter()
            .any(|plan| plan.identity == joined.caller_attachment_type_identity)
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .structural_types
            .iter()
            .any(|plan| plan.identity == first.source_type_identity)
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
    assert!(dynamic.direct_unit_calls.is_empty(), "{dynamic:#?}");
    assert!(dynamic.rebound_unit_calls.is_empty());
    let [joined] = dynamic.joined_unit_calls.as_slice() else {
        panic!("one atomic result-less join expected: {dynamic:#?}")
    };
    assert_ne!(
        joined.when_true.call.selection.conformance,
        joined.when_false.call.selection.conformance,
    );
    assert_eq!(joined.scalar_parameters.len(), 1);
    assert_eq!(
        joined.when_true.call.forwarding_transfers,
        joined.when_false.call.forwarding_transfers,
    );
}
