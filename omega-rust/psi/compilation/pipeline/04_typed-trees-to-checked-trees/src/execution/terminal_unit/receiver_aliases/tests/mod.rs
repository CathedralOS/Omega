use super::super::{CheckedUnitEffectOperationPlan, CheckedUnitStructuralPathSegment};
use super::{
    BorrowAccessKind, ExpressionNode, FlowBorrowWeakeningReason, StatementNode, SymbolHandle,
};
use crate::execution::terminal_unit::receiver_aliases::ReceiverAlias;
use crate::execution::terminal_unit::receiver_aliases::aliases as alias_roster;
use crate::tests::front_end::checked_program;
use checked_trees::CheckedUnitCallResult;

mod mutable;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    checked_program(source)
}

fn fixture(access: &str, prefix: &str, body: &str) -> checked_trees::CheckedTrees {
    checked(&format!(
        "data Record [copy] {{ value: u16; }}
         machine Record::replace(record: &write Record, value: u16) {{ record.value = value; }}
         machine consume(records: &write [Record; 2]) {{}}
         machine forward(records: &{access} [Record; 2], other: &write [Record; 2], value: u16) {{ {prefix} {body} }}"
    ))
}

fn aliases(checked: &checked_trees::CheckedTrees) -> Option<Vec<ReceiverAlias>> {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "forward")?;
    let state = checked.machine_states(machine).first()?;
    alias_roster(&checked.typed, &checked.facts, machine, state)
}

/// The Unit plan the body builder produces for `forward` from these exact
/// facts, rebuilt so forged or escaping evidence is judged by the plan that
/// would consume it rather than by a roster no producer reads on its own.
fn forward_plan(
    checked: &checked_trees::CheckedTrees,
) -> Option<checked_trees::CheckedUnitEffectMachinePlan> {
    let plans = crate::execution::terminal_unit::build_checked_unit_effect_plans(
        &checked.typed,
        &checked.facts,
        crate::execution::terminal_unit::ScalarCalleePlans {
            boundary_returns: &checked.facts.flow.terminal_boundary_scalar_returns,
            structural_returns: &checked.facts.flow.terminal_structural_scalar_returns,
        },
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("forward"))?;
    plans.for_machine(machine.symbol).cloned()
}

#[test]
fn direct_write_only_aliases_preserve_parameters_and_sequential_arguments() {
    for access in ["write", "mut"] {
        let checked = fixture(
            access,
            "let held: &write [Record; 2] = &write records;",
            "Record::replace(&write held[0], value); Record::replace(&write held[1], value);",
        );
        assert_eq!(aliases(&checked).expect("exact direct alias").len(), 1);
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter()
            .find(|plan| {
                checked.machines().iter().any(|machine| {
                    machine.symbol == plan.machine && machine.name.as_str() == "forward"
                })
            })
            .expect("retained caller plan");
        assert_eq!(plan.operations.len(), 3);
        assert!(matches!(
            plan.operations[2],
            CheckedUnitEffectOperationPlan::Complete { .. }
        ));
        for (statement_index, operation) in plan.operations[..2].iter().enumerate() {
            let Some(call) = operation.call().filter(|call| {
                !call.is_boundary() && *call.result() == CheckedUnitCallResult::Unit
            }) else {
                panic!("write-only argument call");
            };
            assert_eq!(
                call.coordinate.statement_index as usize,
                statement_index + 1
            );
            assert_eq!(call.structural_arguments.len(), 1);
            assert_eq!(
                call.structural_arguments[0].access,
                checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
            );
            assert_eq!(
                call.structural_arguments[0].source_parameter_index(),
                Some(0)
            );
            assert_eq!(
                call.structural_arguments[0].path,
                vec![CheckedUnitStructuralPathSegment::FixedIndex(
                    statement_index as u64
                )]
            );
        }
    }
}

fn nested_fixture() -> checked_trees::CheckedTrees {
    fixture(
        "write",
        "let held: &write [Record; 2] = &write records; let child: &write [Record; 2] = &write held; let leaf: &write [Record; 2] = &write child;",
        "Record::replace(&write leaf[1], value);",
    )
}

#[test]
fn projected_parameter_alias_retains_capture_and_normalized_receiver_paths() {
    let checked = checked(
        "data Record [copy] { value: u16; }
        data Container [copy] { records: [Record; 2]; }
        machine Record::replace(record: &write Record, value: u16) { record.value = value; }
        machine Container::forward(container: &write Container, value: u16) {
            let held: &write [Record; 2] = &write container.records;
            let leaf: &write Record = &write held[1];
            Record::replace(&write leaf, value);
        }",
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Container::forward")
        .expect("attached caller");
    let state = checked.machine_states(machine).first().expect("entry");
    let aliases = alias_roster(&checked.typed, &checked.facts, machine, state)
        .expect("projected parameter aliases");
    let parameter = &checked.state_parameters(state)[0];
    assert_eq!(aliases[1].root, parameter.symbol);
    assert_eq!(aliases[1].segments.len(), 2);
    let resource = checked
        .facts
        .borrow
        .direct_loan_resources
        .iter()
        .find(|(_, row)| row.owner_symbol == aliases[0].owner)
        .expect("direct resource")
        .1;
    assert_eq!(resource.captured_place.root_symbol, parameter.symbol);
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == machine.symbol)
        .expect("projected attached caller plan");
    let Some(call) = plan.operations[0]
        .call()
        .filter(|call| !call.is_boundary() && *call.result() == CheckedUnitCallResult::Unit)
    else {
        panic!("write-only argument call");
    };
    assert_eq!(call.structural_arguments.len(), 1);
    assert_eq!(
        call.structural_arguments[0].source_parameter_index(),
        Some(0)
    );
    assert_eq!(
        call.structural_arguments[0].access,
        checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
    );
    assert_eq!(
        call.structural_arguments[0].path,
        vec![
            CheckedUnitStructuralPathSegment::Field("records".into()),
            CheckedUnitStructuralPathSegment::FixedIndex(1),
        ]
    );
    let StatementNode::LocalData(local) =
        &checked.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("alias");
    };
    let ExpressionNode::Borrow(borrow) = checked.expression_table.expression(local.initial_value)
    else {
        panic!("borrow");
    };
    let target = borrow.target;
    let wrong_field = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Record")
        .and_then(|definition| {
            checked
                .data_members(definition)
                .iter()
                .find_map(|member| match member {
                    typed_trees::data::DataMember::Field(field) => Some(field.symbol),
                    _ => None,
                })
        })
        .expect("unrelated field");
    let mut forged = checked.clone();
    let member_symbols = match forged.typed.expression_table.expression_mut(target) {
        ExpressionNode::Member(member) => {
            member.member_symbol = wrong_field;
            None
        }
        ExpressionNode::Name(name) => {
            name.symbol = wrong_field;
            Some(name.member_symbols)
        }
        _ => panic!("projected parameter field"),
    };
    if let Some(member_symbols) = member_symbols {
        let count = forged
            .typed
            .expression_table
            .name_path_member_symbols(member_symbols)
            .len();
        forged
            .typed
            .expression_table
            .set_name_path_member_symbol_at_offset(
                member_symbols,
                u32::try_from(count - 1).unwrap(),
                wrong_field,
            );
    }
    assert!(
        forward_plan(&forged).is_none(),
        "another declaration's field cannot replace the captured parameter field"
    );
}

#[test]
fn nested_aliases_reject_omitted_reordered_and_retargeted_closure() {
    let original = nested_fixture();
    let event_handle = original
        .facts
        .borrow
        .reborrow_disposition_events
        .iter()
        .find(|(_, row)| row.retired_parent_path.len() == 2)
        .expect("leaf closure")
        .0;
    for mutation in 0..8 {
        let mut checked = original.clone();
        let event = checked
            .facts
            .borrow
            .reborrow_disposition_events
            .get_mut(event_handle);
        match mutation {
            0 => {
                event.retired_parent_path.pop();
            }
            1 => event.retired_parent_path.reverse(),
            2 => event.retired_parent_path[0].weakening = arena::Handle::invalid(),
            3 => event.disposition = checked_trees::CheckedReborrowResourceDisposition::Reactivate,
            4 => event.child_activation = arena::Handle::invalid(),
            5 => {
                event.final_target =
                    checked_trees::CheckedBorrowResourceDispositionTarget::default()
            }
            6 => event.shared_cohort.push(event.child_resource),
            _ => event.parent_loan = arena::Handle::invalid(),
        }
        assert!(
            forward_plan(&checked).is_none(),
            "closure mutation {mutation}"
        );
    }
    let containment_handle = original
        .facts
        .borrow
        .reborrow_containment_certificates
        .iter()
        .find(|(_, row)| {
            row.child_loan
                == original
                    .facts
                    .borrow
                    .reborrow_disposition_events
                    .get(event_handle)
                    .child_loan
        })
        .expect("leaf containment")
        .0;
    for mutation in 0..6 {
        let mut checked = original.clone();
        let containment = checked
            .facts
            .borrow
            .reborrow_containment_certificates
            .get_mut(containment_handle);
        match mutation {
            0 => containment.parent_loan = arena::Handle::invalid(),
            1 => containment.parent_access = BorrowAccessKind::Mutable,
            2 => containment.parent_weakening = arena::Handle::invalid(),
            3 => containment.parent_place.root_symbol = SymbolHandle::invalid(),
            4 => containment
                .projection_remainder
                .push(facts::PlaceSegment::FixedIndex { index: 1 }),
            _ => {
                containment.containment =
                    checked_trees::CheckedReborrowContainmentKind::SharedFreeze
            }
        }
        assert!(
            forward_plan(&checked).is_none(),
            "containment mutation {mutation}"
        );
    }
}

#[test]
fn nested_aliases_require_unique_complete_custody_rosters() {
    let original = nested_fixture();
    for mutation in 0..6 {
        let mut checked = original.clone();
        match mutation {
            0 => checked.facts.borrow.reborrow_loan_resources.clear(),
            1 => checked
                .facts
                .borrow
                .reborrow_containment_certificates
                .clear(),
            2 => checked.facts.borrow.reborrow_disposition_events.clear(),
            3 => {
                let row = checked
                    .facts
                    .borrow
                    .reborrow_loan_resources
                    .iter()
                    .next()
                    .expect("resource")
                    .1
                    .clone();
                checked.facts.borrow.reborrow_loan_resources.append(row);
            }
            4 => {
                let row = checked
                    .facts
                    .borrow
                    .reborrow_containment_certificates
                    .iter()
                    .next()
                    .expect("containment")
                    .1
                    .clone();
                checked
                    .facts
                    .borrow
                    .reborrow_containment_certificates
                    .append(row);
            }
            _ => {
                let row = checked
                    .facts
                    .borrow
                    .reborrow_disposition_events
                    .iter()
                    .next()
                    .expect("disposition")
                    .1
                    .clone();
                checked.facts.borrow.reborrow_disposition_events.append(row);
            }
        }
        assert!(
            forward_plan(&checked).is_none(),
            "roster mutation {mutation}"
        );
    }
}

#[test]
fn captured_aliases_close_before_a_later_parent_observation() {
    let original = checked(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../tests/omega/pass/calls/runtime_explicit_reference_alias_argument_exit/main.omg"
    )));
    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .unwrap();
    let state = &original.machine_states(machine)[0];
    let replay = |checked: &checked_trees::CheckedTrees| {
        alias_roster(&checked.typed, &checked.facts, machine, state)
    };
    let aliases = replay(&original).expect("the last use closes the captured alias lineage");
    assert_eq!(aliases.len(), 2);
    assert_eq!(aliases[1].segments.len(), 2);
    let (event_handle, event) = original
        .facts
        .borrow
        .reborrow_disposition_events
        .iter()
        .find(|(_, event)| event.machine_symbol == machine.symbol)
        .expect("the child restores its retired parent path");
    assert_eq!(
        event.boundary_phase,
        checked_trees::CheckedBorrowResourceLifecyclePhase::LastUseExpired
    );
    assert_eq!(
        event.disposition,
        checked_trees::CheckedReborrowResourceDisposition::CascadeThroughRetiredParent
    );
    let plan = original
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .find(|plan| plan.machine == machine.symbol)
        .expect("the guarded caller retains its composed body");
    let calls = plan
        .states
        .iter()
        .flat_map(|state| &state.operations)
        .filter_map(|operation| operation.call())
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2);
    for call in calls {
        assert_eq!(call.structural_arguments.len(), 1);
        let argument = &call.structural_arguments[0];
        assert_eq!(argument.source_parameter_index(), Some(0));
        assert_eq!(
            argument.path,
            vec![
                CheckedUnitStructuralPathSegment::Field("cell".into()),
                CheckedUnitStructuralPathSegment::Field("value".into()),
            ]
        );
    }
    for mutation in 0..5 {
        let mut changed = original.clone();
        let event = changed
            .facts
            .borrow
            .reborrow_disposition_events
            .get_mut(event_handle);
        match mutation {
            0 => {
                event.boundary_phase = checked_trees::CheckedBorrowResourceLifecyclePhase::StateExit
            }
            1 => {
                event.disposition =
                    checked_trees::CheckedReborrowResourceDisposition::StateExitDirectRootHandoff
            }
            2 => event.retired_parent_path.clear(),
            3 => {
                event.final_target =
                    checked_trees::CheckedBorrowResourceDispositionTarget::default()
            }
            4 => {
                event.boundary_source =
                    checked_trees::FlowInvalidationSource::Statement { statement_index: 0 }
            }
            _ => unreachable!(),
        }
        assert!(
            replay(&changed).is_none(),
            "changed closure {mutation} cannot authorize an alias"
        );
    }
}
