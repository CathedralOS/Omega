use super::*;

#[test]
fn computed_receiver_cannot_select_a_numeric_builtin_by_spelling() {
    for target in ["min", "max", "sqrt"] {
        let source = format!(
            r#"
            data Cell {{ value: u64; }}
            data Main {{ cell: Cell; }}
            machine identity(value: &mut Cell) -> &mut Cell {{ value }}
            machine Main::run(&mut self) {{
                let result: u64 = identity(&mut self.cell).{target}(1);
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
            .find(|machine| machine.name.as_str() == "Main::run")
            .expect("caller");
        let state = &typed.machine_states(machine)[0];
        let typed_trees::statement::StatementNode::LocalData(result) =
            &typed.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("result");
        };
        let resolver = validation::CallFrameResolver::new(&typed).expect("resolver");
        assert!(
            resolver
                .expression_write_frame(machine, result.initial_value)
                .into_complete_paths()
                .is_none(),
            "unresolved computed {target} is not the builtin"
        );
        assert!(
            resolver
                .inferred_state_write_frame(machine, state)
                .into_complete_paths()
                .is_none(),
            "unresolved computed {target} keeps its enclosing state opaque"
        );
    }
}

#[test]
fn computed_reference_method_receiver_reaches_checked_trees() {
    let source = r#"
        data Cell { value: u64; }
        data Main { cell: Cell; audit: u64; }
        machine identity(value: &mut Cell) -> &mut Cell { value }
        machine Cell::write_value(&mut self, audit: &mut u64) -> u64 {
            self.value = 1;
            audit = 1;
            1
        }
        machine Main::run(&mut self) {
            let result: u64 = identity(&mut self.cell).write_value(&mut self.audit);
        }
    "#;
    let syntax =
        parse_syntax_trees(&Lexer::new(source).tokenize().expect("tokenize")).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("computed reference receiver keeps its selected input loan");
}

#[test]
fn computed_method_receivers_transport_proven_origins_and_all_operand_writes() {
    let cases = [
        (
            "direct",
            "identity(&mut self.cell)",
            "",
            Some(vec!["self.audit", "self.cell.value"]),
        ),
        (
            "nested",
            "identity(identity(&mut self.cell))",
            "",
            Some(vec!["self.audit", "self.cell.value"]),
        ),
        (
            "indexed",
            "array(&mut self.cells)[0]",
            "",
            Some(vec!["self.audit", "self.cells"]),
        ),
        (
            "effectful_index",
            "array(&mut self.cells)[index(&mut self.receiver_audit)]",
            "",
            Some(vec!["self.audit", "self.cells", "self.receiver_audit"]),
        ),
        (
            "attached_result",
            "identity(&mut self.cell).again()",
            "",
            Some(vec!["self.audit", "self.cell.value"]),
        ),
        (
            "member",
            "bucket(&mut self.bucket).cell",
            "",
            Some(vec!["self.audit", "self.bucket.cell.value"]),
        ),
        (
            "effectful",
            "audited(&mut self.cell, &mut self.receiver_audit)",
            "",
            Some(vec!["self.audit", "self.cell.value", "self.receiver_audit"]),
        ),
        (
            "local",
            "identity(alias)",
            "let alias: &mut Cell = &mut self.cell;",
            Some(vec!["self.audit", "self.cell.value"]),
        ),
        ("recursive", "recursive(&mut self.cell)", "", None),
        ("unknown", "unknown(&mut self.cell)", "", None),
        (
            "unknown_index",
            "array(&mut self.cells)[unknown_index()]",
            "",
            None,
        ),
        ("shared_result", "shared(&self.cell)", "", None),
        ("owned_result", "owned()", "", None),
        (
            "reference_member",
            "carrier(&mut self.carrier).cell",
            "",
            None,
        ),
        (
            "binding_reborrow",
            "identity(&mut alias)",
            "let alias: &mut Cell = &mut self.cell;",
            None,
        ),
    ];
    let mut source = String::from(
        r#"
        data Cell { value: u64; }
        data Bucket { cell: Cell; }
        data Carrier { cell: &mut Cell; }
        data Main { cell: Cell; cells: [Cell; 2]; bucket: Bucket; carrier: Carrier; audit: u64; receiver_audit: u64; }
        machine Cell::write_value(&mut self, audit: &mut u64) -> u64 { self.value = 1; audit = 1; 1 }
        machine Cell::again(&mut self) -> &mut Cell { self }
        machine identity(value: &mut Cell) -> &mut Cell { value }
        machine shared(value: &Cell) -> &Cell { value }
        machine owned() -> Cell { Cell { value: 0 } }
        machine index(audit: &mut u64) -> u64 { audit = 1; 0 }
        machine array(value: &mut [Cell; 2]) -> &mut [Cell; 2] { value }
        machine bucket(value: &mut Bucket) -> &mut Bucket { value }
        machine carrier(value: &mut Carrier) -> &mut Carrier { value }
        machine audited<'value, 'audit>(value: &'value mut Cell, audit: &'audit mut u64) -> &'value mut Cell { audit = 1; value }
        machine recursive(value: &mut Cell) -> &mut Cell { recursive(value) }
    "#,
    );
    for (name, receiver, prefix, _) in &cases {
        source.push_str(&format!("machine Main::case_{name}(&mut self) {{ {prefix} let result: u64 = {receiver}.write_value(&mut self.audit); }}"));
    }
    let tokens = Lexer::new(&source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let resolver = validation::CallFrameResolver::new(&typed).expect("resolver");
    let mut failures = Vec::new();
    for (name, _, _, expected) in cases {
        let qualified = format!("Main::case_{name}");
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == qualified)
            .expect("caller");
        let state = &typed.machine_states(machine)[0];
        let typed_trees::statement::StatementNode::LocalData(result) = typed
            .statement_table
            .statements(state.statement_nodes)
            .last()
            .expect("result")
        else {
            panic!("result");
        };
        let typed_trees::expression::ExpressionNode::Call(call) =
            typed.expression_table.expression(result.initial_value)
        else {
            panic!("method call");
        };
        if expected.is_some() && !call.target_symbol.is_valid() {
            failures.push(format!("{name} method target unresolved"));
        }
        for (query, frame) in [
            resolver.inferred_state_write_frame(machine, state),
            resolver.expression_write_frame(machine, result.initial_value),
        ]
        .into_iter()
        .enumerate()
        {
            let actual = frame.into_complete_paths().map(|mut paths| {
                paths.sort();
                paths
            });
            let expected = expected.as_ref().map(|paths| {
                let mut paths: Vec<_> = paths.iter().map(|path| (*path).to_owned()).collect();
                if name == "local" && query == 1 {
                    paths.push("alias.value".to_owned());
                }
                paths.sort();
                paths
            });
            if actual != expected {
                failures.push(format!(
                    "{name} query {query}: expected {expected:?}, actual {actual:?}"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}
