//! L1 -> L2 handoff for GRAMMAR.md "right shift". L1 joins two touching `>` tokens
//! into one shift operator and leaves separated closers to the generic argument list.
//! L2 binds the operands under that grouping and names the owner of each argument list.
use omega_parser::parse_file;
use omega_resolver::{AssemblyVocabulary, NameRole, ParsedSource, StageSource, Symbol, SymbolResolvedTrees, resolve_stage_for_target};
use omega_syntax::NodeKind;

fn resolve(text: &str) -> SymbolResolvedTrees<'_> {
    let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
    let tree = parse_file(text.as_bytes(), &tokens).unwrap();
    let sources = [StageSource {
        package: "app",
        source: ParsedSource {
            logical_path: "main.omg",
            text,
            tokens: &tokens,
            tree: &tree,
        },
    }];
    resolve_stage_for_target(&sources, &[], &AssemblyVocabulary::default()).unwrap_or_else(|error| panic!("{error:?}"))
}

fn describe(symbol: &Symbol) -> String {
    match symbol {
        Symbol::Declaration(id) => format!("{:?} {}", id.kind, id.path),
        Symbol::Binding { kind, span, .. } => format!("{kind:?}@{}", span.get_start_offset()),
        Symbol::Builtin(name) => format!("builtin {name}"),
        other => format!("{other:?}"),
    }
}

fn between(
    text: &str,
    start: usize,
    end: usize,
) -> &str {
    &text[start..end]
}

fn at(
    text: &str,
    needle: &str,
) -> usize {
    text.find(needle)
        .unwrap_or_else(|| panic!("{needle:?} is not in {text:?}"))
}

#[test]
fn touching_closers_form_one_shift_whose_operands_bind_and_nested_lists_keep_their_owner() {
    let source = "data Vec<T> {} machine shifts(a: u32, b: u32, c: u32, v: Vec<Vec<u8>>) -> bool { _ = a >>% b; a >> b > c }";
    let stage = resolve(source);
    let tree = stage
        .trees()
        .find(|tree| tree.file.path == "main.omg")
        .unwrap();
    let file = tree.file;
    let span_text = |node: usize| {
        let span = file.span(node);
        between(source, span.get_start_offset(), span.get_end_offset())
    };
    let param = |name: &str| format!("Param@{}", at(source, &format!("{name}: u32")));

    // Each shift is one operator spanning both `>` and its policy suffix. The comparison
    // after the shift takes the whole shift as its left operand, and L2 binds the right one.
    let binaries: Vec<_> = (0..file.nodes.len())
        .filter(|&node| file.nodes[node].kind == NodeKind::Binary)
        .map(|node| {
            let [lhs, rhs] = file.nodes[node].children[..] else {
                panic!("binary operands")
            };
            let operator = file.operator(node).unwrap();
            (
                operator.token,
                operator.span.get_start_offset(),
                span_text(lhs),
                span_text(rhs),
                tree.symbol(rhs).map(describe),
            )
        })
        .collect();
    spec_corpus::observe_handoff!(
        "syntax-right-shift/consumer",
        assert_eq!(
            binaries,
            [
                (">>%", at(source, ">>%"), "a", "b", Some(param("b"))),
                (">", at(source, "> c"), "a >> b", "c", Some(param("c"))),
                (">>", at(source, ">> b >"), "a", "b", Some(param("b"))),
            ]
        )
    );
    let operands: Vec<_> = ["a", "b", "c"]
        .into_iter()
        .map(|name| {
            tree.occurrences()
                .filter(|u| u.role == NameRole::Reference && u.span.slice(source.as_bytes()) == Some(name.as_bytes()))
                .map(|u| describe(&u.target))
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(operands, [vec![param("a"), param("a")], vec![param("b"), param("b")], vec![param("c")]]);

    // The same two touching closers end two argument lists. Each list is owned by `Vec`.
    let applications: Vec<_> = tree
        .applications()
        .map(|(section, name)| {
            let arguments: Vec<_> = file.nodes[section]
                .children
                .iter()
                .map(|&argument| span_text(argument))
                .collect();
            (span_text(section), arguments, describe(&name.target))
        })
        .collect();
    assert_eq!(
        applications,
        [
            ("<Vec<u8>>", vec!["Vec<u8>"], "Data Vec".to_owned()),
            ("<u8>", vec!["u8"], "Data Vec".to_owned()),
        ]
    );

    // Negative control: separated `>` tokens are never a shift, so L2 receives no tree.
    let separated = "machine f(a: u32, b: u32) { _ = a > > b; }";
    let tokens = omega_lexer::lex(separated.as_bytes()).unwrap();
    assert!(parse_file(separated.as_bytes(), &tokens).is_err());
}
