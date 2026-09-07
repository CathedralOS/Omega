fn checked(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    crate::lower_typed_trees(typed(source))
}

fn typed(source: &str) -> typed_trees::TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

#[test]
fn selected_as_slice_target_cannot_supply_builtin_array_extent() {
    use typed_trees::expression::ExpressionNode;
    let mut program = typed(VIEW);
    let target = program.machines().last().unwrap().symbol;
    let expression = program
        .expression_table
        .iter_expressions()
        .find_map(|(expression, node)| {
            matches!(node, ExpressionNode::Call(call) if call.target.as_str() == "as_slice")
                .then_some(expression)
        })
        .unwrap();
    let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression) else {
        panic!("view call");
    };
    call.target_symbol = target;
    let diagnostics =
        crate::lower_typed_trees(program).expect_err("selected target is not a builtin view");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove requires")),
        "{diagnostics:#?}"
    );
}

const VIEW: &str = r#"
data Entry { value: u32; }
data Main { entries: [Entry; 4]; }
machine Main::main(&mut self) -> u64 {
    let view: &[Entry] = self.entries.as_slice();
    transition { _ -> self.walk(view, 4) }
}
machine Main::walk(&mut self, entries: &[Entry], capacity: u64) -> u64
requires entries.len <= capacity { 0 }
"#;

#[test]
fn fixed_array_view_extent_proves_tail_requirement_without_reading_contents() {
    checked(VIEW).unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
    checked(&VIEW.replace("requires entries.len", "requires self.entries.len"))
        .unwrap_or_else(|diagnostics| panic!("named target field extent: {diagnostics:#?}"));
}

#[test]
fn fixed_array_view_extent_rejects_too_small_capacity_and_mutable_descriptor() {
    for source in [
        VIEW.replace("view, 4", "view, 3"),
        VIEW.replace("let view", "let mut view"),
    ] {
        let diagnostics = checked(&source).expect_err("no stable sufficient extent");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove requires")),
            "{diagnostics:#?}"
        );
    }
}

#[test]
fn exposing_immutable_view_binding_does_not_replay_its_initializer_extent() {
    let source = VIEW.replace(
        "transition { _ -> self.walk(view, 4) }",
        "expose(&mut view); transition { _ -> self.walk(view, 4) }",
    ) + " machine expose(slot: &mut [Entry]) {}";
    let diagnostics = checked(&source).expect_err("exposed descriptor has no initializer evidence");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove requires")),
        "{diagnostics:#?}"
    );
}
