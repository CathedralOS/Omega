//! Decision 12 through the public lexer and parser. Each sigil owns exactly one reference layer.

mod support;

use omega_lexer::lex;
use omega_parser::{FragmentKind, ParseErrorKind, parse_file, parse_fragment};

#[test]
fn nested_access_modes_stay_on_their_authored_layer() {
    // Predict the tree independently of the parser, including the position of each access word.
    let modes = ["", "mutable", "write"];
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

fn assert_tree(
    kind: FragmentKind,
    source: &str,
    expected: &str,
) {
    let bytes = source.as_bytes();
    let tokens = lex(bytes).unwrap();
    let tree = parse_fragment(bytes, &tokens, kind).unwrap_or_else(|error| panic!("{source}: {error}"));
    assert_eq!(support::describe_tree_problem(bytes, &tokens, &tree), None, "{source}");
    assert_eq!(support::dump_tree(bytes, &tokens, &tree), expected, "{source}");
}

#[test]
fn contextual_borrows_leave_cast_and_membership_outside_the_borrow() {
    for word in ["mutable", "write"] {
        for gap in [" ", "/*gap*/", "\r\n"] {
            for (op, node, rhs) in [("as", "Cast", "PathType"), ("in", "Binary", "PathExpression")] {
                assert_tree(
                    FragmentKind::Statements,
                    &format!("_ = @{word}{gap}{op}{gap}S;"),
                    &format!(
                        "Fragment\n  Discard\n    '_'\n    '='\n    {node}\n      Borrow\n        '@'\n        PathExpression '{word}'\n      '{op}'\n      {rhs} 'S'\n    ';'\n"
                    ),
                );
                assert_tree(
                    FragmentKind::Statements,
                    &format!("_ = @{word}{gap}{op};"),
                    &format!("Fragment\n  Discard\n    '_'\n    '='\n    Borrow\n      '@'\n      '{word}'\n      PathExpression '{op}'\n    ';'\n"),
                );
            }
        }
    }
}

#[test]
fn contextual_reference_types_leave_qualification_and_expression_suffixes_to_the_caller() {
    for word in ["mutable", "write"] {
        for name in ["as", "in"] {
            assert_tree(
                FragmentKind::GenericParam,
                &format!("const V: @{word}/*type*/{name}"),
                &format!("Fragment\n  ConstParam\n    'const'\n    'V'\n    ':'\n    ReferenceType\n      '@'\n      '{word}'\n      PathType '{name}'\n"),
            );
        }
        assert_tree(
            FragmentKind::Statements,
            &format!("let v: @'a {word}/*domain*/in D;"),
            &format!(
                "Fragment\n  Let\n    'let'\n    BindingPattern 'v'\n    ':'\n    QualifiedType\n      ReferenceType\n        '@'\n        Lifetime ''' 'a'\n        PathType '{word}'\n      'in'\n      PathType 'D'\n    ';'\n"
            ),
        );
        for (op, node, rhs) in [("as", "Cast", "PathType"), ("in", "Binary", "PathExpression")] {
            assert_tree(
                FragmentKind::Statements,
                &format!("x as @{word}\r\n{op} S"),
                &format!(
                    "Fragment\n  Tail\n    {node}\n      Cast\n        PathExpression 'x'\n        'as'\n        ReferenceType\n          '@'\n          PathType '{word}'\n      '{op}'\n      {rhs} 'S'\n"
                ),
            );
            assert_tree(
                FragmentKind::Statements,
                &format!("x as @{word} {op}"),
                &format!(
                    "Fragment\n  Tail\n    Cast\n      PathExpression 'x'\n      'as'\n      ReferenceType\n        '@'\n        '{word}'\n        PathType '{op}'\n"
                ),
            );
        }
    }
}

#[test]
fn the_enclosing_syntax_owns_the_suffix() {
    // `=` cannot end the discard operand, but it can end the left side of an assignment.
    assert_tree(
        FragmentKind::Statements,
        "_ = @mutable as =D;",
        "Fragment\n  Discard\n    '_'\n    '='\n    Cast\n      Borrow\n        '@'\n        PathExpression 'mutable'\n      'as'\n      CastEntry\n        '='\n        PathType 'D'\n    ';'\n",
    );
    assert_tree(
        FragmentKind::Statements,
        "@mutable as = D;",
        "Fragment\n  Assign\n    Borrow\n      '@'\n      'mutable'\n      PathExpression 'as'\n    '='\n    PathExpression 'D'\n    ';'\n",
    );
    assert_tree(
        FragmentKind::Clauses,
        "requires @mutable as ensures ok",
        "Fragment\n  Requires\n    'requires'\n    Borrow\n      '@'\n      'mutable'\n      PathExpression 'as'\n  Ensures\n    'ensures'\n    PathExpression 'ok'\n",
    );
    assert_tree(
        FragmentKind::Statements,
        "machine f() -> @write in requires ok;",
        "Fragment\n  Machine\n    'machine'\n    Path 'f'\n    Params '(' ')'\n    Result\n      '->'\n      ReferenceType\n        '@'\n        'write'\n        PathType 'in'\n    Requires\n      'requires'\n      PathExpression 'ok'\n    ';'\n",
    );
    assert_tree(
        FragmentKind::Statements,
        "let x: @mutable in in S;",
        "Fragment\n  Let\n    'let'\n    BindingPattern 'x'\n    ':'\n    QualifiedType\n      ReferenceType\n        '@'\n        'mutable'\n        PathType 'in'\n      'in'\n      PathType 'S'\n    ';'\n",
    );
}

#[test]
fn operator_words_can_still_be_names_before_a_suffix() {
    assert_tree(
        FragmentKind::Statements,
        "@mutable as as u8",
        "Fragment\n  Tail\n    Cast\n      Borrow\n        '@'\n        'mutable'\n        PathExpression 'as'\n      'as'\n      PathType 'u8'\n",
    );
    assert_tree(
        FragmentKind::Statements,
        "@mutable as as",
        "Fragment\n  Tail\n    Cast\n      Borrow\n        '@'\n        PathExpression 'mutable'\n      'as'\n      PathType 'as'\n",
    );
    assert_tree(
        FragmentKind::Statements,
        "@write in(x)",
        "Fragment\n  Tail\n    Borrow\n      '@'\n      'write'\n      Call\n        PathExpression 'in'\n        Args\n          '('\n          PathExpression 'x'\n          ')'\n",
    );
}

#[test]
fn mut_can_be_a_name_and_cannot_mark_a_mutable_binding_or_reference() {
    let name = b"machine f(mut:u8)->u8{mut}";
    parse_file(name, &lex(name).unwrap()).unwrap();

    for source in [
        "machine f(){let mut value:u8=0;}",
        "machine f(value:@mut u8){}",
        "machine f(value:u8){let borrow=@mut value;}",
    ] {
        let bytes = source.as_bytes();
        assert!(parse_file(bytes, &lex(bytes).unwrap()).is_err(), "{source}");
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
        ("machine f(a: |&mutable T) {}", "&"),
        ("machine f() { _ = |&write x; }", "&"),
        ("machine f(a: |& &T) {}", "&"),
        ("machine f() { _ = |& &x; }", "&"),
        ("machine f() -> |&&T;", "&&"),
        ("machine f() { _ = f::<|&T>(x); }", "&"),
        ("machine f() { _ = x as |&T; }", "&"),
        ("machine f() { _ = x |@ y; }", "@"),
        ("machine f() { _ = @mutable as @|; }", ";"),
        ("machine f() { _ = @write in $|; }", ";"),
        ("machine f(x: @write in D & |) {}", ")"),
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
