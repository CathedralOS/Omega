//! Source cases from V1's tests/psi/parser/test_parser.py. All verdicts here follow V2's grammar.

use omega_lexer::lex;
use omega_parser::{ParseErrorKind, parse_file};

#[test]
fn malformed_syntax_points_at_the_offending_token_or_end_of_file() {
    // Byte ranges come from the source cases. EOF has a zero-width span.
    let cases = [
        ("use a", 5..5),
        ("data X { field Type; }", 15..19),
        ("data X { field: ; }", 16..17),
        ("data X [copy,,] {}", 13..14),
        ("data X [copy copy] {}", 13..17),
        ("data X { case A = 1; }", 16..17),
        ("data X { case A(: T); }", 16..17),
        ("data X { case A(T); }", 17..18),
        ("data X { case A(value T); }", 22..23),
        ("data X { case A(value: ); }", 23..24),
        ("data X { case A(left: L right: R); }", 24..29),
        ("data X { case A(value: T", 24..24),
        ("data X { case A(v: T in ); }", 24..25),
        ("data X<const N:", 15..15),
        ("data X<T,", 9..9),
        ("data D { a: [u8 x]; }", 16..17),
        ("data D { a: [u8; 4 x]; }", 19..20),
        ("data D { a: A<B>>; }", 16..17),
        ("machine f(", 10..10),
        ("machine f(a: u8 b: u8) {}", 16..17),
        ("machine f() { let x = ; }", 22..23),
        ("machine f() { a = ; }", 18..19),
        ("machine f() { a = 1 }", 20..21),
        ("machine f() { g(,); }", 16..17),
        ("machine f() { g(1 2); }", 18..19),
        ("machine f() { V { a: }; }", 21..22),
        ("machine f() { V { a: 1 b: 2 }; }", 23..24),
    ];
    for (source, expected_span) in cases {
        let tokens = lex(source.as_bytes()).unwrap();
        let error = parse_file(source.as_bytes(), &tokens).expect_err(source);
        let expected_kind = if expected_span.is_empty() {
            ParseErrorKind::UnexpectedEnd
        } else {
            ParseErrorKind::UnexpectedToken
        };
        assert_eq!(error.get_kind(), expected_kind, "{source}");
        let span = error.get_span();
        assert_eq!(span.get_start_offset()..span.get_end_offset(), expected_span, "{source}");
    }
}

#[test]
fn complete_psi_examples_and_v2_followups_parse_as_files() {
    let cases = [
        "use a; data D { x: u8; } machine m() {}",
        "data D<'a> { v: @'a u8; w: @'a mut u16; }",
        "data D { a: Pair<u8, [u16; 2]>; }",
        "data Disk { #1 counter: i32; #2 case Done(#0 code: u8, #1 more: u8); }",
        // V2 permits contextual names, untyped let bindings, and trailing parameter commas.
        "use self;",
        "machine f() { g(self); }",
        "machine f() { let x = 1; }",
        "machine f(a: u8,) {}",
    ];
    for source in cases {
        let tokens = lex(source.as_bytes()).unwrap();
        parse_file(source.as_bytes(), &tokens).unwrap_or_else(|error| panic!("{source}: {error}"));
    }
}

#[test]
fn a_named_arrow_inside_generic_arguments_is_a_function_type() {
    use omega_parser::{NodeKind, TreeEvent};
    for source in [
        "machine f(x:Box<(n:u8)->bool>){}",
        "machine f(x:Box<[(n:u8)->bool]>){}",
        "machine f(){g::<(n:u8)->bool>(x)}",
    ] {
        let tokens = lex(source.as_bytes()).unwrap();
        let tree = parse_file(source.as_bytes(), &tokens).unwrap();
        assert!(
            tree.get_events()
                .contains(&TreeEvent::Open(NodeKind::FunctionType)),
            "{source}"
        );
    }
    let source = "machine f(){g::<(1 + 2)>(x)}";
    let tokens = lex(source.as_bytes()).unwrap();
    let tree = parse_file(source.as_bytes(), &tokens).unwrap();
    assert!(tree.get_events().contains(&TreeEvent::Open(NodeKind::Binary)));
    let source = "machine f(x:Box<(n:u8) bool>){}";
    let tokens = lex(source.as_bytes()).unwrap();
    assert_eq!(
        parse_file(source.as_bytes(), &tokens)
            .unwrap_err()
            .get_span()
            .slice(source.as_bytes())
            .unwrap(),
        b"bool"
    );
}
