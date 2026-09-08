//! Attached array projections retain their exact declared receiver type.

use super::*;

fn typed_source(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolved");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("typed")
}

fn capture_fixture(source: &str) -> (TypedTrees, ExpressionHandle, Vec<WriteOnlyRoot>) {
    let program = typed_source(source);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "invoke")
        .expect("caller");
    let state = &program.machine_states(machine)[0];
    let parameter = &program.state_parameters(state)[0];
    let TypeReferenceNode::Reference { referee, .. } = program
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        panic!("reference");
    };
    let roots = vec![WriteOnlyRoot {
        symbol: parameter.symbol,
        receiver_machine: SymbolHandle::invalid(),
        name: parameter.name.as_str().to_owned(),
        referee: *referee,
        is_parameter: true,
    }];
    let StatementNode::LocalData(local) =
        &program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("local");
    };
    let ExpressionNode::Borrow(borrow) = program.expression_table.expression(local.initial_value)
    else {
        panic!("borrow");
    };
    let target = borrow.target;
    (program, target, roots)
}

#[test]
fn bare_attached_array_receiver_rejoins_its_exact_record() {
    let source = "data Record [copy] { value: u16; }
        data Container { records: [Record; 2]; }
        machine Record::replace(&write self, replacement: u16) { self.value = replacement; }
        machine Container::invoke(&write self) { records[0].replace(17); }";
    let program = typed_source(source);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Container::invoke")
        .expect("caller");
    let state = &program.machine_states(machine)[0];
    let parameter = &program.state_parameters(state)[0];
    let TypeReferenceNode::Reference { referee, .. } = program
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        panic!("reference");
    };
    let roots = [WriteOnlyRoot {
        symbol: parameter.symbol,
        receiver_machine: machine.symbol,
        name: parameter.name.as_str().to_owned(),
        referee: *referee,
        is_parameter: true,
    }];
    let StatementNode::Expression(expression) =
        &program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("expression statement");
    };
    let ExpressionNode::Call(call) = program.expression_table.expression(*expression) else {
        panic!("call");
    };
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(call.receiver)
    else {
        panic!("indexed receiver");
    };
    assert!(
        record(&program, call.receiver, &roots).is_some(),
        "collection {:?}, bare_field {}, builtin {}, declared {:?}",
        program.expression_table.expression(indexed.collection),
        bare_field(&program, indexed.collection, &roots).is_some(),
        crate::place_has_builtin_coordinates(&program, machine, Some(state), call.receiver),
        crate::declared_place_type_raw(&program, machine, Some(state), indexed.collection)
    );
}

#[test]
fn captured_projection_rejects_reference_bearing_intermediates() {
    for (declarations, projection, indirect_type) in [
        (
            "data Inner { records: [Record; 2]; } data Container { link: Inner; }",
            "root.link.records[0]",
            "Inner",
        ),
        (
            "data Container { records: [Record; 2]; }",
            "root.records[0]",
            "Record",
        ),
    ] {
        let text = format!("data Record [copy] {{ value: u16; }} {declarations}
            machine invoke(root: &write Container, indirect: &{indirect_type}) {{ let held: &write Record = &write {projection}; }}");
        let (mut program, target, roots) = capture_fixture(&text);
        assert!(captured_type(&program, target, &roots).is_some(), "{text}");
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "invoke")
            .unwrap();
        let state = &program.machine_states(machine)[0];
        let indirect = program.state_parameters(state)[1].type_reference;
        let container = program
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == "Container")
            .unwrap();
        let DataMember::Field(field) = &program.data_members(container)[0] else {
            panic!("field");
        };
        let field_type = field.type_reference;
        // Reference-array source syntax is not needed to test the receiving
        // shape check. Substitute an ordinary authored reference type into
        // the otherwise valid record field or array element.
        let replacement = match program
            .type_reference_table
            .type_reference(field_type)
            .clone()
        {
            TypeReferenceNode::FixedArray { length, .. } => TypeReferenceNode::FixedArray {
                element_type: indirect,
                length,
            },
            _ => program
                .type_reference_table
                .type_reference(indirect)
                .clone(),
        };
        program
            .type_reference_table
            .substitute_node(field_type, replacement);
        assert!(captured_type(&program, target, &roots).is_none(), "{text}");
    }
}

#[test]
fn captured_projection_rejects_stale_and_cyclic_paths() {
    let (original, target, roots) = capture_fixture(
        "data Record [copy] { value: u16; } data Container { records: [Record; 2]; }
         machine invoke(root: &write Container) { let held: &write Record = &write root.records[0]; }",
    );
    assert!(captured_type(&original, target, &roots).is_some());
    assert!(captured_type(&original, ExpressionHandle::invalid(), &roots).is_none());
    for mutation in 0..3 {
        let mut program = original.clone();
        let ExpressionNode::Indexed(indexed) = program.expression_table.expression_mut(target)
        else {
            panic!("index");
        };
        match mutation {
            0 => indexed.collection = ExpressionHandle::invalid(),
            1 => indexed.collection = target,
            _ => indexed.index = ExpressionHandle::invalid(),
        }
        assert!(
            captured_type(&program, target, &roots).is_none(),
            "capture mutation {mutation}"
        );
    }
}
