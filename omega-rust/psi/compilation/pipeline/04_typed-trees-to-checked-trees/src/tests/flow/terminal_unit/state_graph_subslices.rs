//! Borrowed edges retain source ranges, rooted projections, and exact access.
use super::CheckedScalarExpressionRole;
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;
use checked_trees::{
    CheckedComposedUnitControlTerminatorPlan, CheckedStructuralControlTransferSourcePlan,
};
use typed_trees::expression::ExpressionNode;

const SOURCE: &str = r#"
    boundary trait Output { machine write(bytes: &[u8], marker: u8); }
    machine writer(bytes: &[u8], marker: u8) reaches Output {
        transition bytes.len > 0 {
            true -> tail(marker, bytes[1..bytes.len])
            false -> done()
        }
        state tail(marker: u8, bytes: &[u8]) { Output::write(bytes, marker); }
        state done() {}
    }
"#;

#[test]
fn borrowed_root_projection_successors_keep_referent_custody() {
    for access in ["&", "&mut "] {
        let source = format!(
            r#"
            boundary trait Output {{ machine write(value: u64); }}
            data Buffer {{ value: u64; }}
            data Parser {{ output: Buffer; }}
            machine parse(parser: {access}Parser, accepted: bool) reaches Output {{
                transition accepted {{
                    true -> observe(&parser.output)
                    false -> observe(&parser.output)
                }}
                state observe(output: &Buffer) {{ Output::write(output.value); }}
            }}
            "#,
        );
        let checked = checked(&source);
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_for_machine(machine_named(&checked, "parse"))
            .expect("an outer parameter loan does not make its referent reference-bearing");
        let CheckedComposedUnitControlTerminatorPlan::Conditional {
            when_true,
            when_false,
            ..
        } = &plan.states[0].terminator
        else {
            panic!("the authored conditional survives")
        };
        for successor in [when_true, when_false] {
            let [transfer] = successor.transfers.as_slice() else {
                panic!("exactly one projected parameter loan")
            };
            let CheckedStructuralControlTransferSourcePlan::ProjectedLoan {
                subject,
                carrier_identity,
            } = &transfer.source
            else {
                panic!("the edge retains the rooted projection")
            };
            assert_eq!(
                subject.access,
                checked_trees::CheckedStructuralAccess::SharedBorrow
            );
            assert!(matches!(
                subject.source,
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index: 0
                }
            ));
            assert!(matches!(
                subject.path.as_slice(),
                [checked_trees::CheckedUnitStructuralPathSegment::Field(_)]
            ));
            assert!(!carrier_identity.is_empty());
        }
    }
}

#[test]
fn borrowed_fixed_array_state_transfers_keep_access_and_shape() {
    for (element, declarations) in [
        ("[i32 in Wrapping; 4]", ""),
        ("Row", "data Row { value: i32 in Wrapping; }"),
    ] {
        for access in ["&", "&mut "] {
            let source = format!(
                r#"
                {declarations}
                machine forward(values: {access}[{element}; 3]) {{
                    transition {{ _ -> receive(values) }}
                    state receive(values: {access}[{element}; 3]) {{}}
                }}
            "#
            );
            let checked = checked(&source);
            let plan = checked
                .facts
                .flow
                .terminal_unit_effects
                .composed_for_machine(machine_named(&checked, "forward"))
                .expect("plain fixed arrays retain ordinary borrowed state custody");
            assert_eq!(plan.states.len(), 2);
            let source = &plan.states[0].structural_parameters[0];
            let target = &plan.states[1].structural_parameters[0];
            assert_eq!(source.type_identity, target.type_identity);
            assert_eq!(source.access, target.access);
            assert_eq!(
                target.access,
                if access == "&" {
                    checked_trees::CheckedStructuralAccess::SharedBorrow
                } else {
                    checked_trees::CheckedStructuralAccess::MutableBorrow
                }
            );
        }
    }
}

#[test]
fn borrowed_fixed_array_state_transfers_reject_invalid_bounds_and_loans() {
    let mut accepted_controls = Vec::new();
    for source in [
        r#"
            machine forward(values: &mut [u64; 3]) {
                transition { _ -> receive(values, 3) }
                state receive(values: &mut [u64; 3], index: u64)
                requires index < 3 { values[index] = 1; }
            }
        "#,
        r#"
            machine forward(values: &[u64; 3]) {
                transition { _ -> receive(values) }
                state receive(values: &mut [u64; 3]) { values[0] = 1; }
            }
        "#,
        r#"
            machine forward(values: &mut [u64; 3]) {
                transition { _ -> receive(values, values) }
                state receive(left: &mut [u64; 3], right: &mut [u64; 3]) {
                    left[0] = 1;
                    right[0] = 2;
                }
            }
        "#,
    ] {
        let result = crate::tests::front_end::typed_program_result(source)
            .and_then(|typed| crate::lower_typed_trees(typed, &crate::CheckingRequest::settled()));
        if let Ok(checked) = result {
            let control = if source.contains("receive(values, 3)") {
                "out-of-bounds successor argument"
            } else if source.contains("receive(values, values)") {
                "duplicated mutable loan"
            } else {
                "shared-to-mutable loan escalation"
            };
            let machine = machine_named(&checked, "forward");
            accepted_controls.push(format!(
                "{control}: omission={:?}, composed={}",
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .omission_for_machine(machine),
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .composed_for_machine(machine)
                    .is_some(),
            ));
        }
    }
    assert!(
        accepted_controls.is_empty(),
        "state shape admission cannot supply bounds or loan authority: {accepted_controls:#?}"
    );
}

#[test]
fn borrowed_root_projection_cannot_upgrade_shared_authority() {
    let source = r#"
        data Buffer { value: u64; }
        data Parser { output: Buffer; }
        machine parse(parser: &Parser) {
            transition { _ -> observe(&mut parser.output) }
            state observe(output: &mut Buffer) { output.value = 1; }
        }
    "#;
    let result = crate::tests::front_end::typed_program_result(source)
        .and_then(|typed| crate::lower_typed_trees(typed, &crate::CheckingRequest::settled()));
    assert!(result.is_err());
}

#[test]
fn forwarded_reference_arguments_reject_duplicate_mutable_record_loans() {
    for invocation in [
        "transition { _ -> receive(values, values) } state receive(left: &mut Buffer, right: &mut Buffer) { left.value = 1; right.value = 2; }",
        "receive(values, values); } machine receive(left: &mut Buffer, right: &mut Buffer) { left.value = 1; right.value = 2;",
    ] {
        let source = format!(
            "data Buffer {{ value: u64; }} machine forward(values: &mut Buffer) {{ {invocation} }}"
        );
        let diagnostics = crate::tests::front_end::checked_program_result(&source)
            .expect_err("forwarding a named record loan cannot duplicate exclusive authority");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("as mutable more than once")),
            "the common loan conflict check must reject: {diagnostics:?}"
        );
    }
}

#[test]
fn forwarded_reference_arguments_allow_shared_aliases_and_disjoint_mutable_projections() {
    for source in [
        r#"
            data Buffer { value: u64; }
            machine forward(values: &Buffer) {
                transition { _ -> receive(values, values) }
                state receive(left: &Buffer, right: &Buffer) {}
            }
        "#,
        r#"
            data Buffer { value: u64; }
            machine forward(values: &mut Buffer) {
                transition { _ -> receive(values, values) }
                state receive(left: &Buffer, right: &Buffer) {}
            }
        "#,
        r#"
            data Buffer { value: u64; }
            data Pair { left: Buffer; right: Buffer; }
            machine forward(pair: &mut Pair) {
                transition { _ -> receive(&mut pair.left, &mut pair.right) }
                state receive(left: &mut Buffer, right: &mut Buffer) {
                    left.value = 1;
                    right.value = 2;
                }
            }
        "#,
    ] {
        let checked = checked(source);
        assert!(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .composed_for_machine(machine_named(&checked, "forward"))
                .is_some(),
            "shared aliases and disjoint projections preserve exact state custody"
        );
    }
}

#[test]
fn forwarded_reference_member_arguments_reject_duplicate_mutable_loans() {
    let diagnostics = crate::tests::front_end::checked_program_result(
        r#"
        data View { body: &mut u64; }
        machine receive(left: &mut u64, right: &mut u64) {
            left = 1;
            right = 2;
        }
        machine forward(view: View) { receive(view.body, view.body); }
    "#,
    )
    .expect_err("reference members retain exclusive access through invocation");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("as mutable more than once")),
        "{diagnostics:?}"
    );
}

#[test]
fn indexed_borrow_arguments_retain_selector_reads_before_the_loan() {
    let checked = checked(
        r#"
        machine receive(value: &mut u64) { value = 1; }
        machine forward(values: &mut [u64; 3], index: u64) requires index < 3 {
            receive(&mut values[index]);
        }
    "#,
    );
    let state = checked
        .facts
        .borrow
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| state.machine_symbol == machine_named(&checked, "forward"))
        .expect("forward borrow facts");
    let [call] = checked.facts.borrow.calls.span_or_empty(state.calls) else {
        panic!("one invocation")
    };
    let [selector, loan] = checked
        .facts
        .borrow
        .argument_accesses
        .span_or_empty(call.accesses)
    else {
        panic!("selector evaluation and retained loan both have exact rows")
    };
    assert_eq!(selector.kind, checked_trees::BorrowAccessKind::Read);
    assert_eq!(selector.end_before_access, Some(1));
    assert_eq!(loan.kind, checked_trees::BorrowAccessKind::Mutable);
    assert_eq!(loan.end_before_access, None);
    assert_ne!(selector.root_symbol, loan.root_symbol);
}

#[test]
fn nested_forwarded_mutable_reference_conflicts_with_retained_outer_observation() {
    let diagnostics = crate::tests::front_end::checked_program_result(
        r#"
        data Buffer { value: u64; }
        machine stamp(buffer: &mut Buffer) -> u64 {
            buffer.value = 1;
            transition { _ -> 1 }
        }
        machine observe(buffer: &Buffer, marker: u64) {}
        machine forward(buffer: &mut Buffer) { observe(&buffer, stamp(buffer)); }
    "#,
    )
    .expect_err("the first operand's shared loan spans nested mutation");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("both mutable and read-only")),
        "{diagnostics:?}"
    );
}

#[test]
fn nested_forwarded_mutable_reference_loans_end_between_scalar_operands() {
    let checked = checked(
        r#"
        data Buffer { value: u64; }
        machine stamp(buffer: &mut Buffer) -> u64 {
            buffer.value = 1;
            transition { _ -> 1 }
        }
        machine observe(first: u64, second: u64) {}
        machine forward(buffer: &mut Buffer) { observe(stamp(buffer), stamp(buffer)); }
    "#,
    );
    let state = checked
        .facts
        .borrow
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| state.machine_symbol == machine_named(&checked, "forward"))
        .expect("forward borrow facts");
    let outer = checked
        .facts
        .borrow
        .calls
        .span_or_empty(state.calls)
        .iter()
        .find(|call| {
            checked
                .facts
                .borrow
                .argument_accesses
                .span_or_empty(call.accesses)
                .len()
                == 2
        })
        .expect("both nested accesses belong to the outer invocation");
    let accesses = checked
        .facts
        .borrow
        .argument_accesses
        .span_or_empty(outer.accesses);
    for (ordinal, access) in accesses.iter().enumerate() {
        assert_eq!(access.kind, checked_trees::BorrowAccessKind::Mutable);
        assert_eq!(access.end_before_access, Some(ordinal + 1));
    }
}

#[test]
fn borrowed_root_projection_rejects_reference_bearing_storage() {
    let checked = checked(
        r#"
        data Buffer { value: u64; }
        data Parser<'a> { output: Buffer; retained: &'a Buffer; }
        machine parse<'a>(parser: &Parser<'a>) {
            transition { _ -> observe(&parser.output) }
            state observe(output: &Buffer) {}
        }
    "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    assert!(
        plans
            .composed_for_machine(machine_named(&checked, "parse"))
            .is_none(),
        "a nested reference is storage, unlike the parameter's outer loan"
    );
}

fn range_source(
    checked: &checked_trees::CheckedTrees,
) -> typed_trees::expression::ExpressionHandle {
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(checked, "writer"))
        .expect("borrowed-byte graph");
    let CheckedComposedUnitControlTerminatorPlan::Conditional { when_true, .. } =
        &plan.states[0].terminator
    else {
        panic!("selected edge");
    };
    let CheckedStructuralControlTransferSourcePlan::ByteSequenceSubslice {
        root: checked_trees::CheckedStorageRoot::Parameter { index: 0 },
        expression,
        storage_type_identity: _,
    } = when_true.transfers[0].source
    else {
        panic!("exact source parameter and retained range");
    };
    expression
}

#[test]
fn byte_tail_plan_rejects_missing_duplicate_or_drifted_endpoint_custody() {
    let checked = checked(SOURCE);
    let machine = machine_named(&checked, "writer");
    let state = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine)
        .unwrap()
        .states[0]
        .state;
    for role in [
        CheckedScalarExpressionRole::SubsliceStart {
            site: checked_trees::CheckedSubsliceSite::TransitionArgument {
                argument_ordinal: 1,
            },
        },
        CheckedScalarExpressionRole::SubsliceEnd {
            site: checked_trees::CheckedSubsliceSite::TransitionArgument {
                argument_ordinal: 1,
            },
        },
    ] {
        for mutation in 0..5 {
            let mut facts = checked.facts.clone();
            let plans = &mut facts.values.scalar_expressions;
            let handle = plans
                .source_bindings
                .iter()
                .find(|(_, binding)| {
                    binding.state == state && binding.statement_ordinal == 0 && binding.role == role
                })
                .unwrap()
                .0;
            match mutation {
                0 => plans.source_bindings.get_mut(handle).expression = Default::default(),
                1 => plans.source_bindings.get_mut(handle).statement_ordinal = 1,
                2 => {
                    let duplicate = plans.source_bindings.get(handle).clone();
                    plans.source_bindings.append(duplicate);
                }
                3 => {
                    let binding = plans.source_bindings.get_mut(handle);
                    binding.destination = if binding.destination.is_valid() {
                        Default::default()
                    } else {
                        machine
                    };
                }
                _ => {
                    plans.source_bindings.get_mut(handle).role = CheckedScalarExpressionRole::Return
                }
            }
            let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
                &checked.typed,
                &facts,
                crate::execution::terminal_unit::ScalarCalleePlans {
                    boundary_returns: &facts.flow.terminal_boundary_scalar_returns,
                    structural_returns: &facts.flow.terminal_structural_scalar_returns,
                },
            );
            assert!(
                rebuilt.composed_for_machine(machine).is_none(),
                "{role:?}: mutation {mutation}"
            );
        }
    }
}

#[test]
fn byte_tail_plan_rejects_selected_or_wrong_range_meaning_and_unknown_source() {
    let checked = checked(SOURCE);
    let expression = range_source(&checked);
    let machine = machine_named(&checked, "writer");
    let operator = checked
        .facts
        .operators
        .uses
        .iter()
        .find(|(_, selected)| selected.expression == expression)
        .expect("range operator fact")
        .0;
    for mutation in 0..5 {
        let mut facts = checked.facts.clone();
        let selected = facts.operators.uses.get_mut(operator);
        match mutation {
            0 => selected.spelling = language_core::OperatorSpelling::Index,
            1 => selected.selected_operator_symbol = machine,
            2 => selected.candidate_count = 1,
            3 => selected.status = checked_trees::CheckedOperatorResolutionStatus::Resolved,
            _ => selected.status = checked_trees::CheckedOperatorResolutionStatus::DomainPending,
        }
        let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
            &checked.typed,
            &facts,
            crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &facts.flow.terminal_boundary_scalar_returns,
                structural_returns: &facts.flow.terminal_structural_scalar_returns,
            },
        );
        assert!(
            rebuilt.composed_for_machine(machine).is_none(),
            "operator mutation {mutation}"
        );
    }
    let mut typed = checked.typed.clone();
    let ExpressionNode::Indexed(indexed) = typed.expression_table.expression(expression) else {
        panic!("range")
    };
    let collection = indexed.collection;
    let ExpressionNode::Name(path) = typed.expression_table.expression_mut(collection) else {
        panic!("parameter")
    };
    path.symbol = machine;
    let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
        &typed,
        &checked.facts,
        crate::execution::terminal_unit::ScalarCalleePlans {
            boundary_returns: &checked.facts.flow.terminal_boundary_scalar_returns,
            structural_returns: &checked.facts.flow.terminal_structural_scalar_returns,
        },
    );
    assert!(
        rebuilt.composed_for_machine(machine).is_none(),
        "unknown source parameter"
    );
    let mut typed = checked.typed.clone();
    let ExpressionNode::Indexed(indexed) = typed.expression_table.expression(expression) else {
        panic!("range");
    };
    let range_handle = indexed.index;
    let ExpressionNode::Range(range) = typed.expression_table.expression_mut(range_handle) else {
        panic!("endpoints");
    };
    range.end_inclusive = true;
    let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
        &typed,
        &checked.facts,
        crate::execution::terminal_unit::ScalarCalleePlans {
            boundary_returns: &checked.facts.flow.terminal_boundary_scalar_returns,
            structural_returns: &checked.facts.flow.terminal_structural_scalar_returns,
        },
    );
    assert!(
        rebuilt.composed_for_machine(machine).is_none(),
        "authored inclusive range"
    );
}
