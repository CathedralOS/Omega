use super::*;
use crate::lower_syntax_trees;
use source_files_to_tokens::Lexer;
use tokens_to_syntax_trees::parse_syntax_trees;

#[test]
fn indexed_candidates_follow_declared_elements_not_index_values() {
    let cases = [
        ("literal", "", "self.cells[0]", true),
        ("variable", "", "self.cells[index]", true),
        ("call", "", "self.cells[compute()]", true),
        ("parameter", "", "cells[index]", true),
        ("slice", "", "slice[index]", true),
        (
            "local_reference",
            "let local: &mut [Cell; 2] = &mut self.cells;",
            "local[index]",
            true,
        ),
        (
            "local_owned",
            "let local: [Cell; 2] = [Cell {}, Cell {}];",
            "local[index]",
            true,
        ),
        ("member", "", "self.buckets[index].cell", true),
        ("nested", "", "self.matrix[index][index]", true),
        ("whole_inner_array", "", "self.matrix[index]", false),
        ("scalar_index", "", "self.cell[index]", false),
        ("missing_root", "", "missing[index]", false),
        ("missing_field", "", "self.buckets[index].missing", false),
        ("later_local", "", "later[index]", false),
    ];
    for (name, prefix, receiver, admitted) in cases {
        let source = format!(
            r#"
            data Cell {{}} data Bucket {{ cell: Cell; }}
            data Owner {{ cells: [Cell; 2]; buckets: [Bucket; 2]; matrix: [[Cell; 2]; 2]; cell: Cell; }}
            machine Cell::read(&self) -> u64 {{ 1 }}
            machine read() -> u64 {{ 2 }}
            machine compute() -> u64 {{ 0 }}
            machine Owner::run(&mut self, cells: &mut [Cell; 2], slice: &mut [Cell], index: u64) {{
                {prefix}
                let result: u64 = {receiver}.read();
                let later: [Cell; 2] = [Cell {{}}, Cell {{}}];
            }}
        "#
        );
        let syntax =
            parse_syntax_trees(&Lexer::new(&source).tokenize().expect("tokenize")).expect("parse");
        let program = lower_syntax_trees(&syntax).expect("resolve");
        let machine = program
            .machines
            .iter()
            .find(|machine| machine.name.as_str() == "Owner::run")
            .expect("caller");
        let state = program.machine_state(program.machine_state_handles(machine.states)[0]);
        let result = program
            .tables
            .bodies
            .statements
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| match statement {
                symbol_resolved_trees::statement::StatementNode::LocalData(local)
                    if local.name.as_str() == "result" =>
                {
                    Some(local)
                }
                _ => None,
            })
            .expect("result");
        let ExpressionNode::Call(call) = program
            .tables
            .bodies
            .expressions
            .expression(result.initial_value)
        else {
            panic!("method");
        };
        let callee = program
            .machines
            .iter()
            .find(|machine| machine.name.as_str() == "Cell::read")
            .expect("callee");
        let expected = if admitted {
            program
                .machine_state(program.machine_state_handles(callee.states)[0])
                .symbol
        } else {
            SymbolHandle::invalid()
        };
        assert_eq!(
            call.target_symbol, expected,
            "{name}: exact method candidate without free-name fallback"
        );
    }
}

#[test]
fn indexed_candidate_rejects_foreign_and_stale_parameter_roots() {
    let source = r#"
        data Cell {} data Owner {}
        machine Cell::read(&self) -> u64 { 1 }
        machine read() -> u64 { 2 }
        machine Owner::run(&mut self, cells: &mut [Cell; 2]) {
            let result: u64 = cells[0].read();
        }
        machine Owner::foreign(&mut self, cells: &mut [Cell; 2]) {}
    "#;
    let syntax =
        parse_syntax_trees(&Lexer::new(source).tokenize().expect("tokenize")).expect("parse");
    let program = lower_syntax_trees(&syntax).expect("resolve");
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "Owner::run")
        .expect("caller");
    let state = program.machine_state(program.machine_state_handles(machine.states)[0]);
    let foreign = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "Owner::foreign")
        .expect("foreign");
    let foreign_state = program.machine_state(program.machine_state_handles(foreign.states)[0]);
    let foreign_symbol = program
        .state_parameters(foreign_state.parameters)
        .iter()
        .find(|parameter| parameter.name.as_str() == "cells")
        .expect("foreign cells")
        .symbol;
    let symbol_resolved_trees::statement::StatementNode::LocalData(result) = &program
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes)[0]
    else {
        panic!("result");
    };
    let ExpressionNode::Call(call) = program
        .tables
        .bodies
        .expressions
        .expression(result.initial_value)
    else {
        panic!("call");
    };
    let scope = MachineScope {
        symbol: machine.symbol,
        type_parameters: &[],
        attached_data: machine.attached_data.as_ref(),
        attached_data_symbol: machine.attached_data_symbol,
        inherited_data_members: None,
        owned_data: &[],
        prior_statements: &[],
        data_definitions: &program.data_definitions,
        data_members: &program.tables.declarations.data_members,
        type_constraints: &program.tables.types.constraints,
    };
    for variant in ["exact", "foreign", "stale"] {
        let mut table = program.tables.bodies.expressions.clone();
        let ExpressionNode::Indexed(indexed) = table.expression(call.receiver) else {
            panic!("indexed receiver");
        };
        let root = indexed.collection;
        let ExpressionNode::Name(name) = table.expression_mut(root) else {
            panic!("root");
        };
        let symbol = match variant {
            "foreign" => foreign_symbol,
            "stale" => {
                SymbolHandle::from_parts(name.symbol.arena_index(), name.symbol.generation() + 1)
            }
            _ => name.symbol,
        };
        name.symbol = symbol;
        name.head_symbol = symbol;
        let target = call_target(
            &scope,
            program.state_parameters(state.parameters),
            state.symbol,
            call,
            &table,
            &program.tables.declarations.child_type_references,
            &program.symbols,
        );
        if variant == "exact" {
            assert_eq!(target, call.target_symbol);
            assert!(target.is_valid());
        } else {
            assert!(
                !target.is_valid(),
                "{variant} root cannot select by spelling"
            );
        }
    }
}
