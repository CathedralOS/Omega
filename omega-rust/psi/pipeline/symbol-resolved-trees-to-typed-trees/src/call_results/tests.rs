use super::*;
use crate::lower_symbol_resolved_trees;
use source_files_to_tokens::Lexer;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;

#[test]
fn computed_receiver_candidates_follow_exact_declared_result_types() {
    let cases = [
        ("identity(&mut self.cell)", true),
        ("identity(identity(&mut self.cell))", true),
        ("array(&mut self.cells)[0]", true),
        ("array(&mut self.cells)[index]", true),
        ("slice(values)[index]", true),
        ("bucket(&mut self.bucket).cell", true),
        ("identity(&mut self.cell).again()", true),
        ("array(&mut self.cells)", false),
        ("scalar()", false),
        ("unknown(&mut self.cell)", false),
        ("bucket(&mut self.bucket).missing", false),
    ];
    for (receiver, admitted) in cases {
        let source = format!(
            r#"
            data Cell {{}} data Bucket {{ cell: Cell; }}
            data Owner {{ cell: Cell; cells: [Cell; 2]; bucket: Bucket; }}
            machine Cell::read(&self) -> u64 {{ 1 }}
            machine Cell::again(&mut self) -> &mut Cell {{ self }}
            machine identity(value: &mut Cell) -> &mut Cell {{ value }}
            machine array(value: &mut [Cell; 2]) -> &mut [Cell; 2] {{ value }}
            machine slice(value: &mut [Cell]) -> &mut [Cell] {{ value }}
            machine bucket(value: &mut Bucket) -> &mut Bucket {{ value }}
            machine scalar() -> u64 {{ 1 }}
            machine read() -> u64 {{ 2 }}
            machine Owner::run(&mut self, values: &mut [Cell], index: u64) {{
                let result: u64 = {receiver}.read();
            }}
        "#
        );
        let syntax =
            parse_syntax_trees(&Lexer::new(&source).tokenize().expect("tokenize")).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Owner::run")
            .expect("caller");
        let state = &typed.machine_states(machine)[0];
        let typed_trees::statement::StatementNode::LocalData(result) =
            &typed.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("result");
        };
        let ExpressionNode::Call(call) = typed.expression_table.expression(result.initial_value)
        else {
            panic!("call");
        };
        let callee = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Cell::read")
            .expect("callee");
        let expected = if admitted {
            typed.machine_states(callee)[0].symbol
        } else {
            SymbolHandle::invalid()
        };
        assert_eq!(
            call.target_symbol, expected,
            "{receiver}: no scalar/array/free-name target fallback"
        );
    }
}

#[test]
fn computed_receiver_rejects_stale_and_foreign_producer_targets() {
    let source = r#"
        data Cell {} data Owner { cell: Cell; }
        machine Cell::read(&self) -> u64 { 1 }
        machine Cell::identity(&mut self) -> &mut Cell { self }
        machine identity(value: &mut Cell) -> &mut Cell { value }
        machine Owner::run(&mut self) { let result: u64 = identity(&mut self.cell).read(); }
    "#;
    let syntax =
        parse_syntax_trees(&Lexer::new(source).tokenize().expect("tokenize")).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Owner::run")
        .expect("caller");
    let state = &typed.machine_states(machine)[0];
    let typed_trees::statement::StatementNode::LocalData(result) =
        &typed.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("result");
    };
    let ExpressionNode::Call(outer) = typed.expression_table.expression(result.initial_value)
    else {
        panic!("outer");
    };
    let foreign = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Cell::identity")
        .expect("foreign");
    let foreign_state = typed.machine_states(foreign)[0].symbol;
    let target = resolved::name::DiagnosticName::generated(outer.target.as_str());
    for variant in ["exact", "stale", "foreign", "non_state", "missing"] {
        let mut expressions = typed.expression_table.clone();
        let ExpressionNode::Call(producer) = expressions.expression_mut(outer.receiver) else {
            panic!("producer");
        };
        producer.target_symbol = match variant {
            "stale" => SymbolHandle::from_parts(
                producer.target_symbol.arena_index(),
                producer.target_symbol.generation() + 1,
            ),
            "foreign" => foreign_state,
            "non_state" => machine.symbol,
            "missing" => SymbolHandle::invalid(),
            _ => producer.target_symbol,
        };
        let actual =
            computed_receiver_method_target(&resolved, &expressions, outer.receiver, &target);
        if variant == "exact" {
            assert!(actual.is_valid());
            assert_eq!(actual, outer.target_symbol);
        } else {
            assert!(
                !actual.is_valid(),
                "{variant} cannot supply a declared producer result"
            );
        }
    }
}
