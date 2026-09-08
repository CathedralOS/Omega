use source::{SourceId, SourceSpan, Span};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees::expression::ExpressionNode;
use symbol_resolved_trees::{AuthoredDeclarationSelectionTarget, SymbolResolvedTrees};
use symbols::SymbolKind;
use syntax_trees::SyntaxTrees;

fn resolve_indices(module: bool, body: &str) -> (SymbolResolvedTrees, Vec<SourceSpan>) {
    let source = format!(
        "{} const SIZE: u64 = 2; {body}",
        if module { "module combat;" } else { "" }
    );
    let mut syntax = SyntaxTrees::default();
    for (source_id, text) in [
        (SourceId(1), "data Buffer<const N: u64> { value: u64; }"),
        (SourceId(2), source.as_str()),
    ] {
        let tokens = Lexer::new(text)
            .tokenize()
            .expect("tokenize raw index owner");
        tokens_to_syntax_trees::parse_syntax_trees_into_with_id(&mut syntax, source_id, &tokens)
            .expect("parse raw index owner");
    }
    let references = source
        .match_indices("SIZE +")
        .map(|(start, _)| SourceSpan::new(SourceId(2), Span::new(start, start + 4)))
        .collect::<Vec<_>>();
    assert!(!references.is_empty());
    let program = crate::lower_syntax_trees_for_const_argument_selection(&syntax, None, Vec::new())
        .expect("resolve original index scopes without synthesis");
    (program, references)
}

fn assert_lexical_selection(
    program: &SymbolResolvedTrees,
    reference: SourceSpan,
    expected: SymbolKind,
) {
    let expressions = &program.tables.bodies.expressions;
    let selected = expressions
        .iter_expressions()
        .filter_map(|(_, expression)| {
            let ExpressionNode::Name(path) = expression else {
                return None;
            };
            let [name] = expressions.name_path_members(path.members) else {
                return None;
            };
            (name.source_span() == reference).then_some(path.symbol)
        })
        .collect::<Vec<_>>();
    assert!(!selected.is_empty(), "lexical leaf must remain a Name");
    assert!(
        selected
            .iter()
            .all(|symbol| symbol.is_valid() && program.symbols.get(*symbol).kind == expected)
    );
    assert!(
        !program
            .authored_declaration_selections()
            .iter()
            .any(|selection| selection.source_span() == reference),
        "lexical bindings must not manufacture constant-declaration custody"
    );
}

fn assert_constant_selection(program: &SymbolResolvedTrees, reference: SourceSpan) {
    let declaration = program
        .const_declarations
        .iter()
        .find(|declaration| declaration.symbol.is_valid())
        .expect("retained constant");
    let occurrences = program
        .authored_declaration_selections()
        .iter()
        .filter(|selection| selection.source_span() == reference)
        .collect::<Vec<_>>();
    assert!(
        !occurrences.is_empty(),
        "closed leaf must retain exact declaration custody"
    );
    assert!(occurrences.iter().all(|selection| matches!(selection.target(), AuthoredDeclarationSelectionTarget::Resolved(target) if target.selected_symbol() == declaration.symbol)));
    assert_eq!(
        program
            .symbols
            .symbol_source_span(declaration.symbol)
            .expect("declaration source")
            .source_id,
        SourceId(2)
    );
}

#[test]
fn raw_index_prepass_preserves_parameter_shadow_in_signature_return_and_local() {
    for module in [false, true] {
        let (program, references) = resolve_indices(
            module,
            "machine run(SIZE: u64, input: Buffer<SIZE + 1>) -> Buffer<SIZE + 1> { let local: Buffer<SIZE + 1>; transition { _ -> input } }",
        );
        assert_eq!(references.len(), 3);
        for reference in references {
            assert_lexical_selection(&program, reference, SymbolKind::Parameter);
        }
    }
}

#[test]
fn raw_index_prepass_uses_the_prior_local_frontier_not_later_bindings() {
    for module in [false, true] {
        let (program, references) = resolve_indices(
            module,
            "machine run() { let before: Buffer<SIZE + 1>; let SIZE: u64 = 7u64; let after: Buffer<SIZE + 1>; }",
        );
        assert_eq!(references.len(), 2);
        assert_constant_selection(&program, references[0]);
        assert_lexical_selection(&program, references[1], SymbolKind::Local);
    }
}

#[test]
fn raw_index_prepass_retains_const_binder_without_global_capture() {
    for module in [false, true] {
        let (program, references) = resolve_indices(
            module,
            "machine run<const SIZE: u64>(input: Buffer<SIZE + 1>) -> Buffer<SIZE + 1> { transition { _ -> input } }",
        );
        assert_eq!(references.len(), 2);
        for reference in references {
            assert_lexical_selection(&program, reference, SymbolKind::TypeParameter);
        }
    }
}

#[test]
fn raw_index_prepass_retains_exact_constant_leaves_without_instances() {
    for module in [false, true] {
        let (program, references) = resolve_indices(
            module,
            "machine run(input: Buffer<SIZE + 1>) -> Buffer<SIZE + 1> { let local: Buffer<SIZE + 1>; transition { _ -> input } }",
        );
        assert_eq!(references.len(), 3);
        for reference in references {
            assert_constant_selection(&program, reference);
        }
        assert_eq!(
            program.data_definitions.len(),
            1,
            "raw prepass must not synthesize Buffer instances"
        );
    }
}

#[test]
fn qualified_index_leaf_failure_retains_lexical_head_and_prior_local_frontier() {
    let (program, references) = resolve_indices(true,
        "machine parameter(combat: u64, input: Buffer<combat::SIZE + 1>) {}
         machine local() { let before: Buffer<combat::SIZE + 1>; let combat: u64 = 9; let after: Buffer<combat::SIZE + 1>; }");
    assert_eq!(references.len(), 3);
    let expressions = &program.tables.bodies.expressions;
    for (reference, expected) in [
        (references[0], SymbolKind::Parameter),
        (references[2], SymbolKind::Local),
    ] {
        let paths = expressions
            .iter_expressions()
            .filter_map(|(_, node)| {
                let ExpressionNode::Name(path) = node else {
                    return None;
                };
                let [_, leaf] = expressions.name_path_members(path.members) else {
                    return None;
                };
                (leaf.source_span() == reference).then_some(path)
            })
            .collect::<Vec<_>>();
        assert!(
            !paths.is_empty(),
            "runtime qualified root must not become a constant"
        );
        for path in paths {
            assert_eq!(program.symbols.get(path.head_symbol).kind, expected);
            assert!(
                !path.symbol.is_valid(),
                "unresolved suffix must remain unresolved"
            );
        }
        let suffix_selections = program
            .authored_declaration_selections()
            .iter()
            .filter(|selection| {
                selection.source_span().source_id == reference.source_id
                    && selection.source_span().span.end == reference.span.end
            })
            .collect::<Vec<_>>();
        assert!(
            !suffix_selections.is_empty(),
            "the unresolved authored suffix retains its checking obligation"
        );
        assert!(suffix_selections.iter().all(|selection| {
            selection.target() == AuthoredDeclarationSelectionTarget::LateBound(
                symbol_resolved_trees::AuthoredDeclarationSelectionLateBinding::CheckedStaticPathSegment,
            )
        }), "runtime root cannot acquire resolved constant custody through its unresolved suffix");
    }
    assert!(program.authored_declaration_selections().iter().any(|selection| {
        selection.source_span().source_id == references[1].source_id
            && selection.source_span().span.end == references[1].span.end
            && matches!(selection.target(), AuthoredDeclarationSelectionTarget::Resolved(target)
                if program.symbols.get(target.selected_symbol()).kind == SymbolKind::Const)
    }), "a later local must not capture the earlier module-qualified constant");
}
