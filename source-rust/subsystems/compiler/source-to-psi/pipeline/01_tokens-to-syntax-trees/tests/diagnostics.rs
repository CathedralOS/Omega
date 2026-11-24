use crate::frontend_fixture::Lexer;
use crate::frontend_fixture::parse_syntax_trees;

fn parse_error_message(source: &str) -> String {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenization should succeed for diagnostics test");
    parse_syntax_trees(&tokens)
        .expect_err("parse should fail for diagnostics test")
        .message
}

#[test]
fn retired_ranking_clauses_report_their_replacements() {
    for (clause, retired, replacement) in [
        (
            "terminates { decreases remaining; }",
            "`terminates { decreases ...; }` block form is retired",
            "terminates by",
        ),
        (
            "decreases remaining;",
            "standalone `decreases` clause is retired",
            "terminates by",
        ),
        (
            "increases remaining;",
            "standalone `increases` clause is retired",
            "Nat::IncreasingTo(<bound>)",
        ),
    ] {
        let message = parse_error_message(&format!("machine walk(remaining: u64) {clause} {{ }}"));
        assert!(message.contains(retired), "{clause}: {message}");
        assert!(message.contains(replacement), "{message}");
    }
}

#[test]
fn case_payload_field_requires_name_and_type() {
    // Payload fields are NAMED: a bare type with no `name:` is a parse error.
    let message = parse_error_message("data Command { case None; case Say(String); }");
    assert_eq!(message, "expected `:`, found punctuation `)`");
}

#[test]
fn bare_variant_member_spelling_is_retired() {
    // The pre-`case` bare `Name;` variant member is no longer accepted.
    let message = parse_error_message("data Direction { North; South; }");
    assert_eq!(
        message,
        "expected `:` after data field `North` (alternatives are spelled `case North;`)"
    );
}

#[test]
fn incomplete_import_reports_the_missing_filename() {
    let message = parse_error_message("use ");
    assert_eq!(message, "expected a logical filename component");
}

#[test]
fn retired_state_declaration_names_its_replacement() {
    let message = parse_error_message("machine main { state ready() {} let value: i32; }");
    assert_eq!(
        message,
        "nested `state` declarations are retired; declare a nested `machine` with its own signature and contract"
    );
}

#[test]
fn raw_bytes_remain_rejected_in_utf16_text_sugar() {
    let message = parse_error_message(r#"machine emit() { utf16"\x80" }"#);
    assert_eq!(
        message,
        "raw byte string literal requires the terminal byte-sequence lowering path"
    );
}

/// `expressions.md` spells a token-bearing requirement as the ordinary
/// `machine + Name(...)`, the grammar a concrete crowned declaration already
/// uses. The separate `operator` introducer is retired on trait requirements:
/// it refuses with the replacement instead of parsing as an alias.
#[test]
fn a_trait_requirement_carries_its_token_on_the_ordinary_machine_head() {
    let tokens = Lexer::new("trait Ranked { machine < before(left: Self, right: Self) -> bool; }")
        .tokenize()
        .expect("tokenize");
    parse_syntax_trees(&tokens).expect("a token-bearing trait requirement parses");

    let message =
        parse_error_message("trait Ranked { operator < before(left: Self, right: Self) -> bool; }");
    assert!(
        message.contains("the `operator` introducer is retired"),
        "unexpected diagnostic: {message}"
    );
}

use source_files::LoadedSource;
use source_lexer as lexing;
use source_parser as parsing;

#[path = "../../../../../../../tests/support/frontend_operations.rs"]
mod frontend_fixture;

#[test]
fn authored_module_headers_cannot_replace_a_directory_namespace() {
    for source in ["module other; machine check() {}", "machine check() {} module other;"] {
        assert!(parse_error_message(source).contains("`module` headers are not valid"));
    }
}

#[test]
fn module_remains_an_identifier_after_retiring_the_header() {
    let source = "trait Evidence {} module satisfies Evidence {}";
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    parse_syntax_trees(&tokens).expect("a conformance can be named module");
    assert!(!parse_error_message("123").contains("`module`"));
}

/// The line the compiler prints for a file that does not parse, with the
/// `path:line:column:` position it reports.
fn parse_diagnostic(source: &str) -> String {
    let loaded = [LoadedSource {
        path: "case.omg".into(),
        source: std::sync::Arc::from(source),
        ..Default::default()
    }]
    .into_iter()
    .collect();
    let lexed = lexing::lex(loaded).expect("tokenization should succeed for diagnostics test");
    parsing::parse(&lexed, Default::default())
        .err()
        .expect("parse should fail for diagnostics test")
        .into_diagnostic()
        .message
}

/// Each of these errors was once built without a span and so was reported at
/// line 1, column 1 whatever the fault. Every program keeps its fault off the
/// first line, so a position that falls back to the file start fails here.
#[test]
fn errors_found_after_an_expression_is_built_report_where_the_fault_is() {
    let mut wrong = Vec::new();
    for (source, expected) in [
        (
            "machine main() {\n    let values = [1, 2, 3];\n    let tail = values[1..=];\n}\n",
            "case.omg:3:24: inclusive range `..=` requires an end expression",
        ),
        (
            "machine main() {\n    let values = [1, 2, 3];\n    let all = values[..=];\n}\n",
            "case.omg:3:22: inclusive range `..=` requires an end expression",
        ),
        (
            "machine plan(builder: Build) {\n    builder.roots.bind(slot, entry);\n}\n",
            "case.omg:2:19: receiver-style root binding is retired; use Build::bind(builder, slot, entry)",
        ),

        (
            "machine main() {\n    let size = (Describe::<u8>)(1);\n}\n",
            "case.omg:2:16: a declaration description is not an executable callable value",
        ),
    ] {
        let reported = parse_diagnostic(source);
        if reported != expected {
            wrong.push(format!("reported `{reported}`, expected `{expected}`"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// A source that stops early has no token to blame, so the error points at
/// the end of the file, where the missing token would have been written.
#[test]
fn a_source_that_ends_early_reports_the_end_of_the_file() {
    assert_eq!(
        [
            parse_diagnostic("machine main() {\n    let size = 1;\n"),
            parse_diagnostic("machine main() {\n    let size ="),
        ],
        [
            "case.omg:3:1: expected expression",
            "case.omg:2:15: expected expression",
        ]
    );
}

/// Running out of page identities is a limit of the parse, not a mistake in
/// a file. With one identity left, the first of two sources is placed and the
/// error names the first token of the second, which sits on its third line.
#[test]
fn a_page_directory_limit_reports_the_first_source_left_without_a_page() {
    let loaded = [
        ("first.omg", "machine first() {}"),
        ("second.omg", "\n\n    machine second() {}"),
    ]
    .into_iter()
    .map(|(path, source)| LoadedSource {
        path: path.into(),
        source: std::sync::Arc::from(source),
        ..Default::default()
    })
    .collect();
    let lexed = lexing::lex(loaded).expect("tokenize");
    let diagnostic = parsing::parse(
        &lexed,
        parsing::ParseOptions {
            first_page_identity: u32::MAX,
            layout: parsing::ParseLayout::IndependentUnits,
        },
    )
    .err()
    .expect("two sources need two page identities")
    .into_diagnostic();
    assert_eq!(
        diagnostic.message,
        "second.omg:3:5: parser exhausted its source-page directory"
    );
}

/// Five unclosed nestings that a parser which tries a generic argument list
/// and rewinds re-reads at every level. Before the parser looked for a
/// list's end ahead of reading it, the first two doubled or worse per level
/// (depth 15 took over ten seconds); each now fails in under a millisecond
/// at depth 60. Depth grows in steps and each step must finish in two
/// seconds, so exponential time fails near depth 20 instead of running for
/// hours, and ordinary load on a shared machine stays far below the bound.
#[test]
fn unclosed_nested_argument_lists_are_refused_without_exponential_rereading() {
    let shapes: [(&str, fn(usize) -> String); 5] = [
        ("type holding expression holding type", |depth| {
            format!("machine f(a: {} !) {{ }}", "A<f::<".repeat(depth))
        }),
        ("the same from an expression", |depth| {
            format!("machine f() {{ _ = {} !; }}", "f::<A<".repeat(depth))
        }),
        ("cast whose type holds a cast", |depth| {
            format!("machine f() {{ _ = {} !; }}", "x as T<".repeat(depth))
        }),
        ("parenthesized pattern holding a match", |depth| {
            format!("machine f() {{ _ = {} !; }}", "match { (".repeat(depth))
        }),
        ("array-shaped argument", |depth| {
            format!("machine f(a: {} !) {{ }}", "A<[".repeat(depth))
        }),
    ];
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(move || {
            for (name, shape) in shapes {
                for depth in (4..=60).step_by(4) {
                    let source = shape(depth);
                    let tokens = Lexer::new(&source).tokenize().expect("tokenize");
                    let started = std::time::Instant::now();
                    assert!(parse_syntax_trees(&tokens).is_err(), "{name}");
                    let elapsed = started.elapsed();
                    assert!(
                        elapsed < std::time::Duration::from_secs(2),
                        "{name} at depth {depth} took {elapsed:?}"
                    );
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
