use super::{
    Identifier, StatementNode, report_nested_call_in_local_initializer,
    result_initializer_call_is_supported,
};
const ORDERED: &str = r#"
data Flags [copy] { value: bool; spare: bool; }
machine stamp(value: &mut bool, number: bool) -> bool { value = number; number }
machine choose(before: bool, left: Flags, after: bool, right: Flags) -> bool {
    let selected: bool = before;
    selected
}
machine inspect(marker: bool, left: Flags, other: bool, right: Flags) -> bool {
    let mut scratch: bool = marker;
    let answer: bool = choose(stamp(&mut scratch, left.value), right,
                              stamp(&mut scratch, right.value), left);
    let snapshot: bool = scratch;
    scratch = answer;
    snapshot
}
machine enter(left: Flags, marker: bool, right: Flags, other: bool) -> bool {
    let inspected: bool = inspect(marker, right, other, left);
    inspected
}
"#;

#[test]
fn linear_result_initializers_share_operand_admission_without_losing_fences() {
    use typed_trees::types::{DomainConstraint, TypeConstraintNode, TypeReferenceNode};

    for prefix in ["", "let marker: u64 = 1;"] {
        let program = crate::front_end::typed_program(&format!(
            "data Region [linear] {{}} domain Region::Owned; data Main {{}}
             machine marker(value: u64) -> u64 {{ value }}
             machine forward(before: u64, region: Region in Owned, after: u64) -> Region in Owned {{ region }}
             machine Main::caller(region: Region in Owned) -> Region in Owned {{
                 {prefix}
                 let retained: Region in Owned = forward(marker(7), region, marker(9));
                 retained
             }}"
        ));
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::caller")
            .unwrap();
        let state = &program.machine_states(machine)[0];
        let (statement_index, local) = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .enumerate()
            .find_map(|(position, statement)| match statement {
                StatementNode::LocalData(local) if local.name.as_str() == "retained" => {
                    Some((position, local))
                }
                _ => None,
            })
            .unwrap();
        assert!(result_initializer_call_is_supported(
            &program,
            machine,
            local.initial_value
        ));
        let mut diagnostics = Vec::new();
        report_nested_call_in_local_initializer(
            &program,
            machine,
            "caller",
            local.initial_value,
            &mut diagnostics,
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");

        for mutation in [
            "mutable",
            "reference",
            "qualification",
            "callee_reference",
            "requirement",
        ] {
            let mut invalid = program.clone();
            let reference = match mutation {
                "reference" | "callee_reference" => {
                    invalid
                        .type_reference_table
                        .insert(TypeReferenceNode::Reference {
                            referee: local.type_reference,
                            access: language_semantics::ReferenceAccess::Shared,
                            lifetime: None,
                        })
                }
                "qualification" => {
                    let constraints = invalid.type_reference_table.insert_constraints([
                        TypeConstraintNode::Domain(DomainConstraint {
                            name: Identifier::generated("Unestablished"),
                            ..DomainConstraint::default()
                        }),
                    ]);
                    invalid
                        .type_reference_table
                        .insert(TypeReferenceNode::Constrained {
                            base_type: local.type_reference,
                            constraints,
                        })
                }
                _ => local.type_reference,
            };
            let StatementNode::LocalData(changed) = &mut invalid
                .statement_table
                .statements_mut(state.statement_nodes)[statement_index]
            else {
                unreachable!()
            };
            if mutation == "mutable" {
                changed.is_mutable = true;
            } else if mutation != "callee_reference" {
                changed.type_reference = reference;
            }
            if mutation == "requirement" || mutation == "callee_reference" {
                let target = invalid
                    .machines()
                    .iter()
                    .find(|machine| machine.name.as_str() == "forward")
                    .unwrap()
                    .clone();
                if mutation == "requirement" {
                    invalid
                        .machines_mut()
                        .iter_mut()
                        .find(|machine| machine.symbol == target.symbol)
                        .unwrap()
                        .supply_mode = language_semantics::MachineSupplyMode::Requirement;
                } else {
                    invalid.machine_states_mut(&target)[0].return_type = reference;
                }
            }
            assert!(
                !result_initializer_call_is_supported(&invalid, machine, local.initial_value),
                "{mutation}"
            );
        }
    }
}

#[test]
fn owned_scalar_nested_borrows_admit_unchanged_ordered_fixture() {
    let program = crate::front_end::typed_program(ORDERED);
    crate::validate_program(&program).unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .unwrap();
    let [state] = program.machine_states(machine) else {
        panic!("one authored state")
    };
    let statements = program.statement_table.statements(state.statement_nodes);
    assert_eq!(statements.len(), 5);
    let StatementNode::LocalData(answer) = &statements[1] else {
        panic!("authored answer")
    };
    let mut diagnostics = Vec::new();
    report_nested_call_in_local_initializer(
        &program,
        machine,
        "entry",
        answer.initial_value,
        &mut diagnostics,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
}

#[test]
fn mutable_receiver_scalar_calls_share_initializer_and_assignment_admission() {
    let program = crate::front_end::typed_program(
        r#"
        data Counter { flag: bool; }
        machine Counter::flip(&mut self, value: bool) -> bool {
            self.flag = value;
            !value
        }
        machine Counter::returned(&mut self) -> bool { self.flip(self.flip(true)) }
        machine Counter::initialized(&mut self) -> bool {
            let value: bool = self.flip(self.flip(true));
            value
        }
        machine Counter::assigned(&mut self) -> bool {
            let mut value: bool = false;
            value = self.flip(self.flip(true));
            value
        }
        machine Counter::stored(&mut self) {
            self.flag = self.flip(self.flip(true));
        }
    "#,
    );
    crate::validate_program(&program).unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
}

/// A scalar local bound directly to a method of the caller's own receiver or
/// a field beneath it is a result operation, as a free call is. An owned
/// receiver and a receiver outside the caller's `self` keep their routes.
#[test]
fn borrowed_self_place_receiver_initializers_are_result_operations() {
    let program = crate::front_end::typed_program(
        r#"
        data Check { seen: u64; }
        data Main { check: Check; }
        machine Check::flag(&mut self, ok: bool) -> u64 {
            transition ok { true -> 0 false -> 7 }
        }
        machine Check::peek(&self, ok: bool) -> u64 { self.seen }
        machine Main::enter(&mut self, other: &mut Check) {
            let field: u64 = self.check.flag(true);
            let shared: u64 = self.check.peek(false);
            let parameter: u64 = other.flag(true);
        }
    "#,
    );
    crate::validate_program(&program).unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::enter")
        .unwrap();
    let [state] = program.machine_states(machine) else {
        panic!("one authored state")
    };
    let initializers = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| match statement {
            StatementNode::LocalData(local) => Some(local.initial_value),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [field, shared, parameter] = initializers.as_slice() else {
        panic!("three authored initializers")
    };
    assert!(result_initializer_call_is_supported(
        &program, machine, *field
    ));
    assert!(result_initializer_call_is_supported(
        &program, machine, *shared
    ));
    assert!(!result_initializer_call_is_supported(
        &program, machine, *parameter
    ));

    let flag = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Check::flag")
        .unwrap();
    let receiver = program.machine_states(flag)[0].parameters.start();
    let typed_trees::types::TypeReferenceNode::Reference { referee, .. } = program
        .type_reference_table
        .type_reference(program.state_parameters.get(receiver).type_reference)
    else {
        panic!("borrowed authored receiver")
    };
    let mut owned = program.clone();
    owned.state_parameters.get_mut(receiver).type_reference = *referee;
    assert!(
        !result_initializer_call_is_supported(&owned, machine, *field),
        "an owned receiver is not lent by the result operation"
    );
}
