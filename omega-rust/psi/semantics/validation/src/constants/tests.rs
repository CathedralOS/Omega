use super::*;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolution");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("typing")
}

#[test]
fn complete_constant_types_reject_copy_and_cleanup_debt() {
    for source in [
        "data Value { value:u64; } const VALUE:Value = Value { value:1 };",
        "data Value [linear] { value:u64; } const VALUE:Value = Value { value:1 };",
        "data Value [copy] { value:u64; } machine Value::drop(&mut self) {} const VALUE:Value = Value { value:1 };",
        "data Owned [linear] { value:u64; } data Value [copy] { case Empty; case Full(value:Owned); } const VALUE:Value = Value::Empty;",
        "data Owned [copy] { value:u64; } machine Owned::drop(&mut self) {} data Value [copy] { case Empty; case Full(value:Owned); } const VALUE:Value = Value::Empty;",
    ] {
        let program = typed(source);
        let mut diagnostics = Vec::new();
        validate_constants(&program, &mut diagnostics);
        assert_eq!(diagnostics.len(), 1, "{source}: {diagnostics:?}");
    }
}

#[test]
fn destination_rechecks_live_constant_constructor_without_rebuilding_ledger() {
    let program = typed(
        "data Value [copy] { value:u64; } data Other [copy] { value:u64; } const VALUE:Value = Value { value:1 }; machine keep()->Value { let value:Value = VALUE; value }",
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
    let expected = local.type_reference;
    let other = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Other")
        .unwrap()
        .symbol;
    assert!(crate::checked_argument_matches_type_reference(
        &program, expression, expected
    ));
    let original_ledger = program.authored_declaration_selections().clone();
    for substitute in [other, SymbolHandle::invalid()] {
        let mut changed = program.clone();
        let ExpressionNode::StructLiteral(literal) =
            changed.expression_table.expression_mut(expression)
        else {
            panic!("copied constructor");
        };
        literal.type_symbol = substitute;
        assert_eq!(changed.authored_declaration_selections(), &original_ledger);
        assert!(!crate::checked_argument_matches_type_reference(
            &changed, expression, expected
        ));
    }
}

#[test]
fn constant_type_custody_rejects_stale_handles_and_wrapper_cycles() {
    let program = typed("const VALUE:[u8;0] = [];");
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
    let program = typed(
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
