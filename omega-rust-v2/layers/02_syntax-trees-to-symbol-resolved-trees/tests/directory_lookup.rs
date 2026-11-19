use omega_lexer::{SourceSpan, lex};
use omega_parser::NodeKind;
use omega_parser::parse_file;
use omega_resolver::{NameRole, ParsedSource, StageSource, Symbol, SymbolResolvedTrees, resolve_stage};

const SIBLING_CALLER: &str = include_str!("../fixtures/accept/sibling_machine/combat/main.omg");
const SIBLING_DECLARATION: &str = include_str!("../fixtures/accept/sibling_machine/combat/scoring.omg");
const CHILD_CALLER: &str = include_str!("../fixtures/reject/child_machine/combat/main.omg");
const CHILD_DECLARATION: &str = include_str!("../fixtures/reject/child_machine/combat/ai/scoring.omg");

fn check_files(
    files: &[(&str, &str)],
    check: impl Fn(&[StageSource<'_, '_>]),
) {
    let parsed: Vec<_> = files
        .iter()
        .map(|&(path, text)| {
            let tokens = lex(text.as_bytes()).unwrap();
            let tree = parse_file(text.as_bytes(), &tokens).unwrap();
            (path, text, tokens, tree)
        })
        .collect();
    let mut sources: Vec<_> = parsed
        .iter()
        .map(|(path, text, tokens, tree)| StageSource {
            package: "game",
            source: ParsedSource {
                logical_path: path,
                text,
                tokens,
                tree,
            },
        })
        .collect();
    check(&sources);
    sources.reverse();
    check(&sources);
}

fn name_span(
    text: &str,
    name: &str,
) -> SourceSpan {
    let start = text.find(name).unwrap();
    SourceSpan::new(start, start + name.len())
}

struct DirectCall<'a> {
    file: &'a str,
    span: SourceSpan,
    target: &'a omega_resolver::DeclarationId,
}

fn direct_calls<'a>(trees: &'a SymbolResolvedTrees<'_>) -> Vec<DirectCall<'a>> {
    trees
        .trees()
        .flat_map(|tree| {
            tree.file
                .nodes
                .iter()
                .filter(|node| node.kind == NodeKind::Call)
                .map(move |call| {
                    let callee = call.children[0];
                    let Some(Symbol::Declaration(target)) = tree.symbol(callee) else {
                        panic!("expected a direct call")
                    };
                    DirectCall {
                        file: tree.file.path,
                        span: tree.file.span(callee),
                        target,
                    }
                })
        })
        .collect()
}

#[test]
fn sibling_machine_resolves_to_its_package_identity_in_either_file_order() {
    check_files(
        &[
            ("combat/main.omg", SIBLING_CALLER),
            ("combat/scoring.omg", SIBLING_DECLARATION),
        ],
        |files| {
            let trees = resolve_stage(files, &[]).unwrap();
            let calls = direct_calls(&trees);
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].file, "combat/main.omg");
            assert_eq!(calls[0].span, name_span(SIBLING_CALLER, "score"));
            assert_eq!(calls[0].target.package, "game");
            assert_eq!(calls[0].target.path, "combat::score");
        },
    );
}

#[test]
fn moving_only_the_declaration_into_a_child_reports_the_callers_exact_name() {
    assert_eq!(SIBLING_CALLER, CHILD_CALLER);
    assert_eq!(SIBLING_DECLARATION, CHILD_DECLARATION);
    check_files(
        &[
            ("combat/main.omg", CHILD_CALLER),
            ("combat/ai/scoring.omg", CHILD_DECLARATION),
        ],
        |files| {
            let error = resolve_stage(files, &[]).unwrap_err();
            assert_eq!(error.file, "combat/main.omg");
            assert_eq!(error.span, name_span(CHILD_CALLER, "score"));
            assert_eq!(error.message, "unresolved name score");
        },
    );
}

#[test]
fn root_directory_forward_and_recursive_calls_keep_source_order_and_skip_trivia() {
    let text = "machine main() { /* score() */ score(); main() } machine score() { 7 }";
    check_files(&[("renamed.omg", text)], |files| {
        let trees = resolve_stage(files, &[]).unwrap();
        let calls = direct_calls(&trees);
        assert_eq!(calls.iter().map(|c| c.target.path.as_str()).collect::<Vec<_>>(), ["score", "main"]);
        for call in calls {
            assert_eq!(call.span.slice(text.as_bytes()).unwrap(), call.target.path.as_bytes());
        }
        let other = files
            .iter()
            .map(|file| StageSource {
                package: "other",
                source: ParsedSource { ..file.source },
            })
            .collect::<Vec<_>>();
        let trees = resolve_stage(&other, &[]).unwrap();
        assert!(
            direct_calls(&trees)
                .iter()
                .all(|call| call.target.package == "other")
        );
    });
}

#[test]
fn same_leaf_in_distinct_namespaces_selects_the_callers_own_namespace() {
    check_files(
        &[
            ("p/q/main.omg", SIBLING_CALLER),
            ("p/q/other.omg", SIBLING_DECLARATION),
            ("q/other.omg", SIBLING_DECLARATION),
        ],
        |files| {
            let trees = resolve_stage(files, &[]).unwrap();
            let calls = direct_calls(&trees);
            assert_eq!(calls[0].target.path, "p::q::score");
        },
    );
}

#[test]
fn machine_parameters_as_arguments_do_not_hide_unrelated_callees() {
    let text = "machine main(points: u32) -> u32 { add(points, score()) } machine add(a: u32, b: u32) -> u32 { a + b } machine score() -> u32 { 7 }";
    check_files(&[("play.omg", text)], |files| {
        let trees = resolve_stage(files, &[]).unwrap();
        let calls = direct_calls(&trees);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].target.path, "add");
        assert_eq!(calls[1].target.path, "score");
    });
}

#[test]
fn duplicate_declarations_report_the_same_error_in_both_orders() {
    check_files(
        &[
            ("main.omg", SIBLING_CALLER),
            ("a.omg", SIBLING_DECLARATION),
            ("b.omg", SIBLING_DECLARATION),
        ],
        |files| {
            let error = resolve_stage(files, &[]).unwrap_err();
            assert_eq!(error.message, "duplicate declaration score");
            assert_eq!(error.file, "b.omg");
            assert_eq!(error.span, name_span(SIBLING_DECLARATION, "score"));
        },
    );
}

#[test]
fn duplicate_logical_files_and_mismatched_token_storage_return_errors() {
    check_files(
        &[
            ("combat/main.omg", SIBLING_CALLER),
            ("combat/main.omg", SIBLING_CALLER),
        ],
        |files| {
            let error = resolve_stage(files, &[]).unwrap_err();
            assert_eq!(error.file, "combat/main.omg");
            assert_eq!(error.message, "duplicate logical file");
        },
    );
    check_files(&[("scoring.omg", SIBLING_DECLARATION)], |files| {
        let mismatched = StageSource {
            package: "game",
            source: ParsedSource {
                tokens: &[],
                ..files[0].source
            },
        };
        let error = resolve_stage(&[mismatched], &[]).unwrap_err();
        assert_eq!(error.file, "scoring.omg");
        assert_eq!(error.message, "inconsistent parsed source");
    });
}

#[test]
fn shorthand_and_operators_retain_their_roles_on_the_exact_nodes() {
    let text = "data Point { x: u8; } machine point(x: u8) -> Point { Point { x } } machine add(x: u8) -> u8 { x +% 1 }";
    check_files(&[("point.omg", text)], |files| {
        let resolved = resolve_stage(files, &[]).unwrap();
        let tree = resolved.trees().next().unwrap();
        let node = |kind| tree.file.nodes.iter().position(|n| n.kind == kind).unwrap();
        let field = node(NodeKind::FieldInit);
        let names: Vec<_> = tree.names(field).collect();
        assert_eq!(names.len(), 2);
        assert_eq!(names[0].span, names[1].span);
        assert_eq!(names[0].role, NameRole::Member);
        assert_eq!(names[1].role, NameRole::Reference);
        assert_eq!(tree.member(field), Some("x"));
        let Some(Symbol::Binding {
            binder_node,
            span,
            kind: NodeKind::Param,
        }) = tree.symbol(field)
        else {
            panic!("shorthand must refer to the parameter");
        };
        let parameter = node(NodeKind::Param);
        assert_eq!(*binder_node, parameter);
        assert_eq!(*span, tree.file.nodes[parameter].words().next().unwrap().span);
        let op = tree.operator(node(NodeKind::Binary)).unwrap();
        assert_eq!(op.token, "+%");
        assert_eq!(op.span.slice(text.as_bytes()), Some(&b"+%"[..]));
    });
}
