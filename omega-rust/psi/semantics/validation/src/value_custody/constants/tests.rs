use super::{DataMember, TypeReferenceHandle, TypeReferenceNode, validate_constants};
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

#[test]
fn constant_type_custody_rejects_stale_handles_and_wrapper_cycles() {
    let program = crate::front_end::typed_program("const VALUE:[u8;0] = [];");
    let declaration = program.roots.const_declarations.start();
    let reference = program.const_declarations()[0].declared_type;
    let mut stale = program.clone();
    stale
        .tables
        .const_declarations
        .get_mut(declaration)
        .declared_type =
        TypeReferenceHandle::from_parts(reference.arena_index(), reference.generation() + 1);
    let mut diagnostics = Vec::new();
    validate_constants(&stale, &mut diagnostics);
    assert_eq!(diagnostics.len(), 1);
    for node in [
        TypeReferenceNode::FixedArray {
            element_type: reference,
            length: 0.into(),
        },
        TypeReferenceNode::Constrained {
            base_type: reference,
            constraints: arena::HandleSpan::empty(),
        },
    ] {
        let mut cyclic = program.clone();
        cyclic.type_reference_table.substitute_node(reference, node);
        let mut diagnostics = Vec::new();
        validate_constants(&cyclic, &mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
    }
}

#[test]
fn live_case_and_field_selections_reject_substitution_with_original_ledger() {
    let program = crate::front_end::typed_program(
        "data Value [copy] { case One(value:u64); case Two(value:u64); } const VALUE:Value = Value::One { value:1 }; machine keep()->Value { let value:Value = VALUE; value }",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "keep")
        .unwrap();
    let [state] = program.machine_states(machine) else {
        panic!("one state");
    };
    let StatementNode::LocalData(local) =
        &program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("constant local");
    };
    let expression = local.initial_value;
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Value")
        .unwrap();
    let second = program
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            DataMember::Variant(case) if case.name.as_str() == "Two" => Some(case),
            _ => None,
        })
        .unwrap();
    let wrong_case = second.symbol;
    let wrong_field = program.data_payload_fields(second)[0].symbol;
    crate::validate_program(&program).expect("valid case constant");
    for change_case in [false, true] {
        let mut changed = program.clone();
        let ExpressionNode::StructLiteral(literal) =
            changed.expression_table.expression_mut(expression)
        else {
            panic!("copied case");
        };
        if change_case {
            literal.case_symbol = Some(wrong_case);
        } else {
            let fields = literal.fields;
            let mut field = changed
                .expression_table
                .struct_field_at_offset(fields, 0)
                .clone();
            field.field_symbol = wrong_field;
            changed
                .expression_table
                .set_struct_field_at_offset(fields, 0, field);
        }
        assert_eq!(
            changed.authored_declaration_selections(),
            program.authored_declaration_selections()
        );
        assert!(crate::validate_program(&changed).is_err());
    }
}
