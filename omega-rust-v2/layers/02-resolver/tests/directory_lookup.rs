use omega_lexer::{SourceSpan, lex};
use omega_parser::parse_file;
use omega_resolver::{ParsedSource, ResolutionError, resolve_machine_calls};

const SIBLING_CALLER: &str = include_str!("../fixtures/accept/sibling_machine/combat/main.omg");
const SIBLING_DECLARATION: &str = include_str!("../fixtures/accept/sibling_machine/combat/scoring.omg");
const CHILD_CALLER: &str = include_str!("../fixtures/reject/child_machine/combat/main.omg");
const CHILD_DECLARATION: &str = include_str!("../fixtures/reject/child_machine/combat/ai/scoring.omg");

fn check_files(
    files: &[(&str, &str)],
    check: impl Fn(&[ParsedSource<'_>]),
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
        .map(|(path, text, tokens, tree)| ParsedSource {
            logical_path: path,
            text,
            tokens,
            tree,
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

#[test]
fn sibling_machine_resolves_to_its_package_identity_in_either_file_order() {
    check_files(
        &[
            ("combat/main.omg", SIBLING_CALLER),
            ("combat/scoring.omg", SIBLING_DECLARATION),
        ],
        |files| {
            let calls = resolve_machine_calls("game", files).unwrap();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].logical_path, "combat/main.omg");
            assert_eq!(calls[0].span, name_span(SIBLING_CALLER, "score"));
            assert_eq!(calls[0].target.package, "game");
            assert_eq!(calls[0].target.qualified_name(), "combat::score");
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
            assert_eq!(
                resolve_machine_calls("game", files).unwrap_err(),
                ResolutionError::Unresolved {
                    logical_path: "combat/main.omg",
                    span: name_span(CHILD_CALLER, "score"),
                    name: "score"
                }
            );
        },
    );
}

#[test]
fn root_directory_forward_and_recursive_calls_keep_source_order_and_skip_trivia() {
    let text = "machine main() { /* score() */ score(); main() } machine score() { 7 }";
    check_files(&[("renamed.omg", text)], |files| {
        let calls = resolve_machine_calls("game", files).unwrap();
        assert_eq!(
            calls
                .iter()
                .map(|c| c.target.qualified_name())
                .collect::<Vec<_>>(),
            ["score", "main"]
        );
        for call in calls {
            assert_eq!(call.span.slice(text.as_bytes()).unwrap(), call.target.name.as_bytes());
        }
        assert_eq!(resolve_machine_calls("other", files).unwrap()[0].target.package, "other");
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
            let calls = resolve_machine_calls("game", files).unwrap();
            assert_eq!(calls[0].target.qualified_name(), "p::q::score");
        },
    );
}

#[test]
fn machine_parameters_as_arguments_do_not_hide_unrelated_callees() {
    let text = "machine main(points: u32) -> u32 { add(points, score()) } machine add(a: u32, b: u32) -> u32 { a + b } machine score() -> u32 { 7 }";
    check_files(&[("play.omg", text)], |files| {
        let calls = resolve_machine_calls("game", files).unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].target.name, "add");
        assert_eq!(calls[1].target.name, "score");
    });
}

#[test]
fn competing_declarations_and_unimplemented_scopes_never_return_a_guessed_binding() {
    check_files(
        &[
            ("main.omg", SIBLING_CALLER),
            ("a.omg", SIBLING_DECLARATION),
            ("b.omg", SIBLING_DECLARATION),
        ],
        |files| {
            assert!(matches!(
                resolve_machine_calls("game", files),
                Err(ResolutionError::CompetingMachines { name: "score", .. })
            ));
        },
    );
    for source in [
        "use another; machine main() { score() }",
        "machine main() { machine score() { 7 } }",
        "machine main(score: T) { score() }",
        "machine main() { Thing::score() }",
    ] {
        check_files(&[("main.omg", source), ("scoring.omg", SIBLING_DECLARATION)], |files| {
            assert!(
                matches!(resolve_machine_calls("game", files), Err(ResolutionError::Unsupported { .. })),
                "{source}"
            );
        });
    }
}

#[test]
fn duplicate_logical_files_and_mismatched_token_storage_return_errors() {
    check_files(
        &[
            ("combat/main.omg", SIBLING_CALLER),
            ("combat/main.omg", SIBLING_CALLER),
        ],
        |files| {
            assert_eq!(
                resolve_machine_calls("game", files),
                Err(ResolutionError::DuplicateFile {
                    logical_path: "combat/main.omg"
                })
            );
        },
    );
    check_files(&[("scoring.omg", SIBLING_DECLARATION)], |files| {
        let mismatched = ParsedSource { tokens: &[], ..files[0] };
        assert_eq!(
            resolve_machine_calls("game", &[mismatched]),
            Err(ResolutionError::InconsistentParse { logical_path: "scoring.omg" })
        );
    });
}
