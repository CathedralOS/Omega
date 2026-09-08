//! Isolate extent identity from the separate domain-proof transport checker.

#[test]
fn bounded_byte_state_alias_names_cannot_relabel_an_old_receiver_extent() {
    for (receiver, accepted) in [("left", false), ("right", true)] {
        let source = format!(
            r#"
            domain [u8;3]::Utf8 requires valid_utf8(self);
            data Record {{ out: [u8;3] in Utf8; }}
            data Pair {{ left: Record; right: Record; }}
            machine Pair::read(&mut self) {{
                let alias: &mut Record = &mut self.left;
                alias.out = "";
                transition {{ _ -> next() }}
                state next(&mut self) {{
                    let alias: &mut Record = &mut self.right;
                    alias.out = "XXX";
                    let observed: u8 = self.{receiver}.out[2];
                }}
            }}
        "#
        );
        let tokens = source_files_to_tokens::Lexer::new(&source)
            .tokenize()
            .expect("tokenize alias extent fixture");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens)
            .expect("parse alias extent fixture");
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
            .expect("resolve alias extent fixture");
        let program = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type alias extent fixture");
        let borrows = crate::build_borrow_facts(&program);
        let proof_plan = proof::obligations::build_proof_plan(&program);
        let values = crate::values::build_value_facts(&program, &proof_plan);
        let operators = crate::operators::build_operator_facts(&program, &values);
        let frames = validation::CallFrameResolver::new(&program)
            .expect("resolved alias fixture has call frames");
        let incoming = super::incoming_guards::IncomingGuardIndex::build(&program, Some(&frames));
        let result =
            super::check_indexed_accesses(&program, &operators, &borrows, Some(&frames), &incoming);
        assert_eq!(result.is_ok(), accepted, "{result:#?}\n{source}");
        if let Err(diagnostics) = result {
            assert!(
                diagnostics
                    .iter()
                    .all(|diagnostic| { diagnostic.message.contains("cannot prove index `2`") }),
                "expected only live-prefix bounds failure: {diagnostics:#?}"
            );
        }
    }
}
