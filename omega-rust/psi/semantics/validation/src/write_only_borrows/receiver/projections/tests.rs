//! Attached array projections retain their exact declared receiver type.

use super::*;

#[test]
fn bare_attached_array_receiver_rejoins_its_exact_record() {
    let source = "data Record [copy] { value: u16; }
        data Container { records: [Record; 2]; }
        machine Record::replace(&write self, replacement: u16) { self.value = replacement; }
        machine Container::invoke(&write self) { records[0].replace(17); }";
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolved");
    let program = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("typed");
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
