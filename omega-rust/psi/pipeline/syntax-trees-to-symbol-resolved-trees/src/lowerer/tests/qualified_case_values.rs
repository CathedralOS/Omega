use super::{Arc, Lexer, PathBuf, SourceMap, lower_syntax_trees_with_sources};
use symbol_resolved_trees::{SymbolResolvedTrees, expression::ExpressionNode};
use symbols::SymbolKind;
use syntax_trees::SyntaxTrees;
use tokens_to_syntax_trees::parse_syntax_trees_into_with_id;

fn resolve(texts: &[&str]) -> Result<SymbolResolvedTrees, Vec<diagnostics::Diagnostic>> {
    let mut sources = SourceMap::default();
    let mut syntax = SyntaxTrees::default();
    for (ordinal, text) in texts.iter().enumerate() {
        let source = sources
            .add(
                PathBuf::from(format!("case_source_{ordinal}.omg")),
                (*text).to_owned(),
            )
            .source_id;
        let tokens = Lexer::new(text)
            .tokenize()
            .expect("tokenize case selection");
        parse_syntax_trees_into_with_id(&mut syntax, source, &tokens)
            .expect("parse case selection");
    }
    lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
}

#[test]
fn qualified_bare_case_ambiguity_cannot_select_a_segmentwise_owner() {
    for imports in [
        "use first::scope; use second::scope;",
        "use second::scope; use first::scope;",
    ] {
        let consumer = format!("{imports} machine read() -> u64 {{ scope::Choice::Ready }}");
        let diagnostics = resolve(&[
            "module first::scope; pub data Choice { case Ready; }",
            "module second::scope; pub data Choice { case Ready; }",
            &consumer,
        ])
        .expect_err("competing qualified carriers must not select by import order");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("ambiguous constructor `scope::Choice::Ready`")
                    && diagnostic.message.contains("first::scope::Choice::Ready")
                    && diagnostic.message.contains("second::scope::Choice::Ready")
            }),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn qualified_bare_cases_preserve_open_static_binder_heads() {
    for (parameters, contract, expected_kind) in [
        ("scope", "", SymbolKind::TypeParameter),
        (
            "machine scope",
            "where machine scope() -> u64;",
            SymbolKind::MachineParameter,
        ),
        (
            "Element, scope: Element satisfies Ranked",
            "",
            SymbolKind::ConformanceParameter,
        ),
    ] {
        let consumer = format!(
            "use scope; trait Ranked {{}} machine read<{parameters}>() -> u64 {contract} {{ scope::Choice::Ready }}"
        );
        let resolved = resolve(&["module scope; pub data Choice { case Ready; }", &consumer])
            .expect("raw resolution retains the open binder for later checking");
        let path = resolved
            .tables
            .bodies
            .expressions
            .iter_expressions()
            .find_map(|(_, expression)| match expression {
                ExpressionNode::Name(path)
                    if resolved
                        .tables
                        .bodies
                        .expressions
                        .name_path_members(path.members)
                        .first()
                        .is_some_and(|member| member.as_str() == "scope") =>
                {
                    Some(path)
                }
                _ => None,
            })
            .expect("authored qualified value path");
        assert_eq!(resolved.symbols.get(path.head_symbol).kind, expected_kind);
        assert!(
            !path.symbol.is_valid(),
            "an open binder cannot manufacture a case value"
        );
        assert_eq!(
            path.members.count(),
            3,
            "lexical paths retain their original segments"
        );
    }
}
