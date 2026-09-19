use super::{Lexer, ResolutionRequest, lower_symbol_resolved_trees, parse_syntax_trees, resolve};

/// An exclusive `&mut`/`&write` borrow of a bare reference binding is a
/// reborrow: the callee receives the binding's own referent, never the binding
/// slot. When that referent resolves to one proven caller path, calls through
/// the reborrow publish an exact frame. Unresolved, ambiguous, or
/// helper-internal reborrow relations keep failing closed.
#[test]
fn computed_reborrows_publish_proven_referents_and_fail_closed() {
    let cases = [
        // (name, statement, caller parameters, prefix, expected)
        (
            "direct",
            "consume(&mut self.value);",
            "",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "reborrow_local",
            "consume(&mut alias);",
            "",
            "let alias: &mut u64 = &mut self.value;",
            Some(vec!["self.value"]),
        ),
        (
            "reborrow_param",
            "consume(&mut param);",
            ", param: &mut u64",
            "",
            Some(vec!["$P0"]),
        ),
        (
            "reborrow_stored_leaf",
            "consume(&mut self.holder.slot);",
            "",
            "",
            Some(vec!["self.holder.slot"]),
        ),
        (
            "chained_reborrow",
            "let second: &mut u64 = &mut first; consume(&mut second);",
            "",
            "let first: &mut u64 = &mut self.value;",
            Some(vec!["self.value"]),
        ),
        (
            "reborrow_local_member",
            "consume_cell(&mut alias.value);",
            "",
            "let alias: &mut Cell = &mut self.cell;",
            Some(vec!["self.cell.value.value"]),
        ),
        (
            "reborrow_in_literal",
            "consume_view(View { body: &mut alias });",
            "",
            "let alias: &mut u64 = &mut self.value;",
            Some(vec!["self.value"]),
        ),
        (
            "reborrow_in_value_call",
            "let r: u64 = compute(&mut alias);",
            "",
            "let alias: &mut u64 = &mut self.value;",
            Some(vec!["self.value"]),
        ),
        (
            "reborrow_as_receiver",
            "let r: u64 = (&mut alias).write_value(&mut self.audit);",
            "",
            "let alias: &mut Cell = &mut self.cell;",
            Some(vec!["self.audit", "self.cell.value"]),
        ),
        (
            "reborrow_indexed_alias",
            "consume(&mut alias);",
            "",
            "let alias: &mut u64 = &mut self.cells[0];",
            Some(vec!["self.cells"]),
        ),
        (
            "reborrow_inside_rebind_index",
            "alias = &mut self.other_cells[identity_index(write_index(&mut alias))]; alias = 1;",
            "",
            "let alias: &mut u64 = &mut self.cells[0];",
            Some(vec!["self.cells", "self.other_cells"]),
        ),
        // An unresolved result origin keeps the borrowed binding opaque.
        (
            "unresolved_reborrow",
            "let alias: &mut u64 = recursive_ref(&mut self.value); consume(&mut alias);",
            "",
            "",
            None,
        ),
        // Two distinct proven referents keep their exact finite union when
        // the divergent binding is lent through a direct reborrow argument.
        (
            "divergent_reborrow_argument",
            "let alias: &mut u64 = match self.tag { 0 -> &mut self.value, _ -> &mut self.other }; consume(&mut alias);",
            "",
            "",
            Some(vec!["self.other", "self.value"]),
        ),
        // A helper that reborrows its own parameter keeps its relation opaque.
        (
            "reborrow_in_helper_body",
            "let r: &mut u64 = helper(&mut self.value);",
            "",
            "",
            None,
        ),
        // A reference leaf projected out of an owned result is not a caller
        // place the reborrow can spell.
        (
            "member_of_owned_result",
            "consume(&mut owned(&mut self.value).slot);",
            "",
            "",
            None,
        ),
    ];
    for (name, statement, parameters, prefix, expected) in cases {
        let source = format!(
            r#"
            data Cell {{ value: u64; }}
            data Holder {{ slot: &mut u64; }}
            data View {{ body: &mut u64; }}
            data Main {{ value: u64; other: u64; tag: u64; holder: Holder; cell: Cell; cells: [u64; 2]; other_cells: [u64; 2]; audit: u64; }}
            machine consume(value: &mut u64) {{ value = 1; }}
            machine consume_cell(value: &mut Cell) {{ value.value = 1; }}
            machine consume_view(value: View) {{ value.body = 1; }}
            machine compute(value: &mut u64) -> u64 {{ value = 1; 0 }}
            machine write_index(value: &mut u64) -> u64 [0..=1] {{ value = 1; 0 }}
            machine identity_index(index: u64 [0..=1]) -> u64 [0..=1] {{ index }}
            machine recursive_ref(value: &mut u64) -> &mut u64 {{ recursive_ref(value) }}
            machine Cell::write_value(&mut self, audit: &mut u64) -> u64 {{ self.value = 1; audit = 1; 1 }}
            machine owned(slot: &mut u64) -> Holder {{ Holder {{ slot: slot }} }}
            machine helper(value: &mut u64) -> &mut u64 {{ consume(&mut value); value }}
            machine Main::run(&mut self{parameters}) {{ {prefix} {statement} }}
            "#
        );
        let tokens = Lexer::new(&source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let resolver = validation::CallFrameResolver::new(&typed).expect("resolver");
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::run")
            .expect("caller");
        let state = typed.machine_states(machine).first().expect("entry");
        let actual = resolver
            .inferred_state_write_frame(machine, state)
            .complete_paths()
            .map(|paths| {
                let mut paths: Vec<_> = paths.to_vec();
                paths.sort();
                paths
            });
        let expected = expected.map(|paths| {
            let mut paths: Vec<_> = paths.iter().map(|path| (*path).to_owned()).collect();
            paths.sort();
            paths
        });
        assert_eq!(actual, expected, "{name}");
    }
}
