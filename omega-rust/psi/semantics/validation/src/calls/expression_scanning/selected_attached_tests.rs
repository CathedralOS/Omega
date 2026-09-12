use typed_trees::expression::ExpressionNode;

#[test]
fn attached_call_requires_its_selected_state_and_receiver_owner() {
    let source = "data First { value: u64; } data Second { other: u64; }
        machine First::read(&self) -> u64 { self.value }
        machine Second::read(&self) -> u64 { self.other }
        machine read(value: &First) -> u64 { value.read() }";
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    crate::validate_program(&program).expect("exact selected call");
    let call = program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, expression)| {
            matches!(expression, ExpressionNode::Call(_)).then_some(handle)
        })
        .unwrap();
    let other = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Second::read")
        .unwrap();
    let other_state = program.machine_states(other)[0].symbol;
    for target in [other_state, symbols::SymbolHandle::invalid()] {
        let mut forged = program.clone();
        let ExpressionNode::Call(call) = forged.expression_table.expression_mut(call) else {
            unreachable!()
        };
        call.target_symbol = target;
        assert!(
            crate::validate_program(&forged).is_err(),
            "substituted/absent selected state cannot use receiver spelling as authority"
        );
    }
}

#[test]
fn an_attached_call_cannot_use_a_collection_element_as_its_receiver_owner() {
    use typed_trees::types::{FixedArrayLength, TypeReferenceNode};
    let source = "data First { value: u64; }
        machine First::read(&self) -> u64 { self.value }
        machine read(value: &First) -> u64 { value.read() }";
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    crate::validate_program(&program).expect("ordinary nominal receiver");
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "read")
        .unwrap();
    let state = program.machine_states(machine)[0].clone();
    let reference = program.state_parameters(&state)[0].type_reference;
    let TypeReferenceNode::Reference {
        referee,
        access,
        lifetime,
    } = program
        .type_reference_table
        .type_reference(reference)
        .clone()
    else {
        panic!("borrowed nominal receiver");
    };
    for collection in [
        TypeReferenceNode::FixedArray {
            element_type: referee,
            length: FixedArrayLength::Literal(2),
        },
        TypeReferenceNode::Slice {
            element_type: referee,
        },
    ] {
        let mut forged = program.clone();
        let collection = forged.type_reference_table.insert(collection);
        let reference = forged
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: collection,
                access,
                lifetime: lifetime.clone(),
            });
        forged.state_parameters.span_mut_or_empty(state.parameters)[0].type_reference = reference;
        assert!(
            crate::validate_program(&forged).is_err(),
            "whole collection is not its element's nominal receiver"
        );
    }
}

#[test]
fn static_attached_calls_check_explicit_self_and_following_arguments() {
    for (parameters, arguments, accepted) in [
        ("value: First", "value, true", true),
        ("", "First { value: 7 }, true", true),
        ("value: Second", "value, true", false),
        ("value: First", "value", false),
        ("value: First", "value, false", false),
    ] {
        let source = format!(
            "data First {{ value: u64; }} data Second {{ value: u64; }}
             machine First::forward(self, allowed: bool) -> First
             requires allowed == true
             {{ self }}
             machine invoke({parameters}) -> First {{ First::forward({arguments}) }}"
        );
        let tokens = source_files_to_tokens::Lexer::new(&source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
        let program =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        assert_eq!(
            crate::validate_program(&program).is_ok(),
            accepted,
            "static call {parameters}: {arguments}"
        );
    }
}
