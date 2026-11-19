//! Decision 12 through the public lexer and parser. Each sigil owns exactly one reference layer.

mod support;

use omega_lexer::lex;
use omega_parser::{FragmentKind, ParseErrorKind, parse_file, parse_fragment};

#[test]
fn nested_access_modes_stay_on_their_authored_layer() {
    // Predict the tree independently of the parser, including the position of each access word.
    let modes = ["", "mut", "write"];
    for outer_mode in modes {
        for inner_mode in modes {
            for separator in ["", " ", "/* loan */", "\r\n"] {
                let operand_separator = if inner_mode.is_empty() { "" } else { " " };
                let prefix = format!("@{outer_mode}{separator}@{inner_mode}{operand_separator}");
                for (kind, source, parent, layer, leaf) in [
                    (
                        FragmentKind::GenericParam,
                        format!("const V: {prefix}T"),
                        "ConstParam\n    'const'\n    'V'\n    ':'",
                        "ReferenceType",
                        "PathType 'T'",
                    ),
                    (FragmentKind::Statements, format!("{prefix}x"), "Tail", "Borrow", "PathExpression 'x'"),
                ] {
                    let tokens = lex(source.as_bytes()).unwrap();
                    let tree = parse_fragment(source.as_bytes(), &tokens, kind).unwrap_or_else(|error| panic!("{source}: {error}"));
                    let mut expected = format!("Fragment\n  {parent}\n    {layer}\n      '@'\n");
                    if !outer_mode.is_empty() {
                        expected.push_str(&format!("      '{outer_mode}'\n"));
                    }
                    expected.push_str(&format!("      {layer}\n        '@'\n"));
                    if !inner_mode.is_empty() {
                        expected.push_str(&format!("        '{inner_mode}'\n"));
                    }
                    expected.push_str(&format!("        {leaf}\n"));
                    assert_eq!(support::dump_tree(source.as_bytes(), &tokens, &tree), expected, "{source}");
                }
            }
        }
    }
}

#[test]
fn incomplete_and_retired_prefixes_identify_the_first_bad_token() {
    // The marker locates the expected diagnostic, including zero-width EOF after a consumed @.
    for (marked_source, offending_token) in [
        ("machine f(a: @@|) {}", ")"),
        ("machine f(a: @'|) {}", ")"),
        ("machine f(a: @|", ""),
        ("machine f() { _ = @@|; }", ";"),
        ("machine f() { _ = @|", ""),
        ("machine f(a: |&mut T) {}", "&"),
        ("machine f() { _ = |&write x; }", "&"),
        ("machine f(a: |& &T) {}", "&"),
        ("machine f() { _ = |& &x; }", "&"),
        ("machine f() -> |&&T;", "&&"),
        ("machine f() { _ = f::<|&T>(x); }", "&"),
        ("machine f() { _ = x as |&T; }", "&"),
        ("machine f() { _ = x |@ y; }", "@"),
    ] {
        let (before_error, after_error) = marked_source.split_once('|').unwrap();
        let source = format!("{before_error}{after_error}");
        let tokens = lex(source.as_bytes()).unwrap();
        let error = parse_file(source.as_bytes(), &tokens).expect_err(&source);
        let expected_kind = if offending_token.is_empty() {
            ParseErrorKind::UnexpectedEnd
        } else {
            ParseErrorKind::UnexpectedToken
        };
        assert_eq!(error.get_kind(), expected_kind, "{source}");
        assert_eq!(error.get_span().get_start_offset(), before_error.len(), "{source}");
        assert_eq!(error.get_span().slice(source.as_bytes()), Some(offending_token.as_bytes()), "{source}");
    }
}
