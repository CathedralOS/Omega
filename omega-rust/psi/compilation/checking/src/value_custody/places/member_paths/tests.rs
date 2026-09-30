use super::data_definition_for_type;
use typed_trees::data::DataMember;
use typed_trees::expression::{Expression, ExpressionNode, NamePath, TableMemberExpression};
use typed_trees::types::{FixedArrayLength, TypeReferenceNode};

fn case_payload_program(member: &str, cases: &str) -> typed_trees::TypedTrees {
    crate::front_end::typed_program(&format!(
        "data Reset {{ success: bool; }}
         data Metadata {{ count: u64; }}
         data Scanned {{ metadata: Metadata; }}
         data Response {{ {cases} }}
         machine inspect(response: Response) -> u64 {{
             transition response {{
                 Response::Reset {{ response }} -> 0
                 Response::Scanned {{ response }} -> response.{member}
             }}
         }}"
    ))
}

#[test]
fn nested_case_payload_members_use_selected_case_not_declaration_order() {
    for cases in [
        "case Reset(response: Reset); case Scanned(response: Scanned);",
        "case Scanned(response: Scanned); case Reset(response: Reset);",
    ] {
        let program = case_payload_program("metadata.count", cases);
        assert!(
            crate::validate_program(&program).is_ok(),
            "selected payload must validate: {:?}",
            crate::validate_program(&program)
        );
    }
}

#[test]
fn nested_case_payload_member_cannot_come_from_another_case() {
    let program = case_payload_program(
        "success",
        "case Reset(response: Reset); case Scanned(response: Scanned);",
    );
    let diagnostics = crate::validate_program(&program).expect_err("foreign payload member");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(
                "reads a nested member `success`, but data `Scanned` has no field `success`"
            )),
        "{diagnostics:?}"
    );
}

#[test]
fn borrowed_case_payload_preserves_nested_member_identity() {
    let program = crate::front_end::typed_program(
        "data Small { first: u64; } data Large { second: u64; }
         data Choice { case Small(value: Small); case Large(value: Large); }
         machine inspect(choice: &Choice) -> u64 {
             transition choice {
                 Choice::Small { value } -> value.first
                 Choice::Large { value } -> value.second
             }
         }",
    );
    assert!(crate::validate_program(&program).is_ok());
}

#[test]
fn ordinary_nested_record_diagnoses_first_missing_member() {
    let program = crate::front_end::typed_program(
        "data Inner { count: u64; } data Outer { inner: Inner; }
         machine inspect(value: &Outer) -> u64 { value.inner.count }
         machine invalid(value: &Outer) -> u64 { value.missing.count }",
    );
    let diagnostics = crate::validate_program(&program).expect_err("missing intermediate field");
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(diagnostics[0].message.contains(
        "machine `invalid` state `entry` reads a nested member `missing`, but data `Outer`"
    ));
}

#[test]
fn declared_const_array_members_use_exact_closed_extents_and_keep_open_fallback() {
    let mut program = crate::front_end::typed_program_with_generic_data(
        "data Buffer<const Count: u64> { items: [u32 in Wrapping; Count]; }
         machine inspect(first: &Buffer<2>, second: &Buffer<4>) {}
         machine open<const Count: u64>(value: &Buffer<Count>) {}",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .unwrap()
        .clone();
    let state = program.machine_states(&machine)[0].clone();
    let parameters = program.state_parameters(&state).to_vec();
    for (parameter, extent) in parameters.iter().zip([2, 4]) {
        let receiver = program
            .expression_table
            .insert_tree(&Expression::Name(NamePath::resolved(
                vec![parameter.name.clone()],
                parameter.symbol,
                parameter.symbol,
            )));
        let member =
            program
                .expression_table
                .insert(ExpressionNode::Member(TableMemberExpression {
                    receiver,
                    member: "items".into(),
                    member_symbol: symbols::SymbolHandle::invalid(),
                    case_variant: None,
                }));
        let reference =
            super::super::declared_place_type_raw(&program, &machine, Some(&state), member)
                .unwrap();
        assert!(
            matches!(program.type_reference_table.type_reference(reference),
            TypeReferenceNode::FixedArray { length: FixedArrayLength::Literal(length), .. } if *length == extent)
        );
    }
    let open_machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "open")
        .unwrap();
    let open = program.state_parameters(&program.machine_states(open_machine)[0])[0].type_reference;
    let template = data_definition_for_type(&program, open).unwrap();
    assert!(template.generic_instance.is_none());
    let DataMember::Field(field) = &program.data_members(template)[0] else {
        panic!("array field")
    };
    assert!(matches!(
        program
            .type_reference_table
            .type_reference(field.type_reference),
        TypeReferenceNode::FixedArray {
            length: FixedArrayLength::ConstParameter { .. },
            ..
        }
    ));

    let instance = data_definition_for_type(&program, parameters[0].type_reference)
        .unwrap()
        .clone();
    let reference = instance
        .generic_instance
        .expect("closed application origin");
    let owner = program
        .tables
        .data_definitions
        .iter()
        .find(|(_, data)| data.symbol == instance.symbol)
        .unwrap()
        .0;
    let mut missing = program.clone();
    missing
        .tables
        .data_definitions
        .get_mut(owner)
        .generic_instance = None;
    let fallback = data_definition_for_type(&missing, reference).unwrap();
    assert_eq!(fallback.symbol, template.symbol);
    assert!(!missing.data_type_parameters(fallback).is_empty());
    let mut ambiguous = program.clone();
    ambiguous.push_data_definition(instance);
    let fallback = data_definition_for_type(&ambiguous, reference).unwrap();
    assert_eq!(fallback.symbol, template.symbol);
    assert!(!ambiguous.data_type_parameters(fallback).is_empty());
}
