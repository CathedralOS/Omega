use super::{
    Identifier, StatementNode, free_scalar_machine, report_nested_call_in_local_initializer,
    result_initializer_call_is_supported, scalar_computation_call,
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
    assert!(scalar_computation_call(
        &program,
        machine,
        answer.initial_value,
        false,
    ));
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
fn owned_scalar_nested_call_admission_keeps_parameter_restrictions() {
    let original = crate::front_end::typed_program(ORDERED);
    let caller = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .unwrap();
    let state = &original.machine_states(caller)[0];
    let symbol = original.state_parameters(state)[1].symbol;
    for mutation in 0..4 {
        let mut program = original.clone();
        let parameter = program
            .state_parameters
            .iter()
            .find(|(_, parameter)| parameter.symbol == symbol)
            .unwrap()
            .0;
        match mutation {
            0 => program.state_parameters.get_mut(parameter).is_mutable = true,
            1 => {
                let definition = program
                    .data_definitions
                    .iter()
                    .find(|(_, definition)| definition.name.as_str() == "Flags")
                    .unwrap()
                    .0;
                program
                    .data_definitions
                    .get_mut(definition)
                    .properties
                    .multiplicity = language_semantics::Multiplicity::Linear;
            }
            2 => {
                let base_type = program.state_parameters.get(parameter).type_reference;
                let reference = program.type_reference_table.insert(
                    typed_trees::types::TypeReferenceNode::Constrained {
                        base_type,
                        constraints: arena::HandleSpan::empty(),
                    },
                );
                program.state_parameters.get_mut(parameter).type_reference = reference;
            }
            _ => {
                let referee = program.state_parameters.get(parameter).type_reference;
                let reference = program.type_reference_table.insert(
                    typed_trees::types::TypeReferenceNode::Reference {
                        referee,
                        access: language_semantics::ReferenceAccess::Shared,
                        lifetime: None,
                    },
                );
                program.state_parameters.get_mut(parameter).type_reference = reference;
            }
        }
        assert!(
            !free_scalar_machine(&program, caller),
            "parameter mutation {mutation}"
        );
    }
}
