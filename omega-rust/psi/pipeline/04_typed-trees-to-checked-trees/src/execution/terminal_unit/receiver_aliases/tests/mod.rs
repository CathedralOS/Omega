use super::super::{CheckedUnitEffectOperationPlan, CheckedUnitStructuralPathSegment};
use super::{
    BorrowAccessKind, ExpressionNode, FlowBorrowWeakeningReason, StatementNode, SymbolHandle,
};
use crate::execution::terminal_unit::receiver_aliases::ReceiverAlias;
use crate::execution::terminal_unit::receiver_aliases::aliases as alias_roster;
use crate::tests::front_end::checked_program;

mod mutable;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    checked_program(source)
}

fn fixture(access: &str, prefix: &str, body: &str) -> checked_trees::CheckedTrees {
    checked(&format!(
        "data Record [copy] {{ value: u16; }}
         machine Record::replace(&write self, value: u16) {{ self.value = value; }}
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
fn direct_write_only_aliases_preserve_parameters_and_sequential_receivers() {
    for access in ["write", "mut"] {
        let checked = fixture(
            access,
            "let held: &write [Record; 2] = &write records;",
            "held[0].replace(value); held[1].replace(value);",
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
            let CheckedUnitEffectOperationPlan::CallUnit {
                coordinate,
                structural_arguments,
                ..
            } = operation
            else {
                panic!("receiver call");
            };
            assert_eq!(coordinate.statement_index as usize, statement_index + 1);
            assert_eq!(structural_arguments.len(), 1);
            assert_eq!(structural_arguments[0].source_parameter_index(), Some(0));
            assert_eq!(
                structural_arguments[0].path,
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
        "leaf[1].replace(value);",
    )
}

#[test]
fn projected_self_alias_retains_capture_and_normalized_receiver_paths() {
    let checked = checked(
        "data Record [copy] { value: u16; }
        data Container [copy] { records: [Record; 2]; }
        machine Record::replace(&write self, value: u16) { self.value = value; }
        machine Container::forward(&write self, value: u16) {
            let held: &write [Record; 2] = &write self.records;
            let leaf: &write Record = &write held[1];
            leaf.replace(value);
        }",
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Container::forward")
        .expect("attached caller");
    let state = checked.machine_states(machine).first().expect("entry");
    let aliases = alias_roster(&checked.typed, &checked.facts, machine, state)
        .expect("projected self aliases");
    assert_eq!(aliases[1].root, machine.symbol);
    assert_eq!(aliases[1].segments.len(), 2);
    let resource = checked
        .facts
        .borrow
        .direct_loan_resources
        .iter()
        .find(|(_, row)| row.owner_symbol == aliases[0].owner)
        .expect("direct resource")
        .1;
    assert_ne!(resource.captured_place.root_symbol, machine.symbol);
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == machine.symbol)
        .expect("projected attached caller plan");
    let CheckedUnitEffectOperationPlan::CallUnit {
        structural_arguments,
        ..
    } = &plan.operations[0]
    else {
        panic!("receiver call");
    };
    assert_eq!(structural_arguments[0].path.len(), 2);
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
    let ExpressionNode::Member(member) = forged.typed.expression_table.expression_mut(target)
    else {
        panic!("self member");
    };
    member.member_symbol = wrong_field;
    assert!(
        forward_plan(&forged).is_none(),
        "another declaration's field cannot replace inherited self field custody"
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
