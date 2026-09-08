use super::*;

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

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

#[test]
fn owned_scalar_nested_borrows_admit_unchanged_ordered_fixture() {
    let program = typed(ORDERED);
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
    assert!(free_scalar_computation_call(
        &program,
        machine,
        answer.initial_value
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
fn owned_scalar_nested_borrow_admits_affine_read_before_transfer() {
    let program = typed(
        "data Flags { value: bool; }
        machine stamp(value: &mut bool, number: bool) -> bool { value = number; number }
        machine consume(marker: bool, flags: Flags) -> bool { marker }
        machine inspect(flags: Flags) -> bool {
            let mut scratch: bool = false;
            let answer: bool = consume(stamp(&mut scratch, flags.value), flags);
            answer
        }",
    );
    crate::validate_program(&program).unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
}

#[test]
fn owned_scalar_nested_call_admission_keeps_parameter_restrictions() {
    let original = typed(ORDERED);
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
