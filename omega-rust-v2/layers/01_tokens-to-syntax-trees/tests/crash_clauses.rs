//! A `crashes` clause names one cause and may add a guard. The cause is a path type and never a
//! family application, so parentheses after `Trap` or `Abort` open the guard.

mod support;

use omega_lexer::lex;
use omega_parser::parse_file;

/// Every place a header clause can be written, with `CLAUSE` standing for the clause.
const POSITIONS: &[&str] = &[
    "machine f(value: u32, done: bool) CLAUSE { 0 }",
    "data D { step: machine(value: u32, done: bool) CLAUSE; }",
    "machine h<const F: machine(value: u32, done: bool) CLAUSE>() { 0 }",
    "machine g() requires machine f(value: u32, done: bool) CLAUSE; { 0 }",
    "trait T { machine f(value: u32, done: bool) CLAUSE; }",
];

const BARE_GUARD: &str = "\
Crashes
  'crashes'
  PathType 'Trap'
  Binary
    PathExpression 'value'
    '=='
    Literal '0'
";

const PARENTHESIZED_GUARD: &str = "\
Crashes
  'crashes'
  PathType 'Trap'
  Paren
    '('
    Binary
      PathExpression 'value'
      '=='
      Literal '0'
    ')'
";

const PARENTHESIZED_ALTERNATIVE: &str = "\
Crashes
  'crashes'
  PathType 'Trap'
  Binary
    Paren
      '('
      Binary
        PathExpression 'value'
        '=='
        Literal '0'
      ')'
    '||'
    PathExpression 'done'
";

const ARGUMENTS_AND_GUARD: &str = "\
Crashes
  'crashes'
  PathType
    'Trap'
    TypeArgs
      '<'
      PathType 'u8'
      '>'
  Paren
    '('
    Binary
      PathExpression 'value'
      '=='
      Literal '0'
    ')'
";

/// The `Crashes` node of the dump, re-indented to start at column zero.
fn get_crashes_subtree(source: &str) -> String {
    let tokens = lex(source.as_bytes()).unwrap();
    let tree = parse_file(source.as_bytes(), &tokens).unwrap_or_else(|error| panic!("{source}: {error}"));
    let dump = support::dump_tree(source.as_bytes(), &tokens, &tree);
    let mut lines = dump.lines().skip_while(|line| line.trim_start() != "Crashes");
    let head = lines
        .next()
        .unwrap_or_else(|| panic!("{source}: no Crashes node in\n{dump}"));
    let indent = head.len() - head.trim_start().len();
    let mut subtree = String::from("Crashes\n");
    for line in lines.take_while(|line| line.len() - line.trim_start().len() > indent) {
        subtree.push_str(&line[indent..]);
        subtree.push('\n');
    }
    subtree
}

#[test]
fn parentheses_after_a_crash_cause_open_its_guard() {
    // The bare guard is the control: parentheses add a Paren node and change neither the cause nor the comparison.
    let abort_guard = PARENTHESIZED_GUARD.replace("'Trap'", "'Abort'");
    let cases = [
        ("crashes Trap value == 0", BARE_GUARD),
        ("crashes Trap (value == 0)", PARENTHESIZED_GUARD),
        ("crashes Trap (value == 0) || done", PARENTHESIZED_ALTERNATIVE),
        ("crashes Abort (value == 0)", &abort_guard),
        ("crashes Trap<u8> (value == 0)", ARGUMENTS_AND_GUARD),
        ("crashes Trap", "Crashes\n  'crashes'\n  PathType 'Trap'\n"),
    ];
    for (clause, expected) in cases {
        for position in POSITIONS {
            let source = position.replace("CLAUSE", clause);
            assert_eq!(get_crashes_subtree(&source), expected, "{source}");
        }
    }
}

#[test]
fn a_crash_cause_is_a_path_type() {
    // No other type form can name `Trap` or `Abort`, so the parser refuses each at the token that starts it.
    for (clause, offending_token) in [
        ("crashes @Trap", "@"),
        ("crashes [Trap]", "["),
        ("crashes Trap -> Abort", "->"),
    ] {
        for position in POSITIONS {
            let source = position.replace("CLAUSE", clause);
            let tokens = lex(source.as_bytes()).unwrap();
            let error = parse_file(source.as_bytes(), &tokens).expect_err(&source);
            let span = error.get_span();
            assert_eq!(&source[span.get_start_offset()..span.get_end_offset()], offending_token, "{source}");
        }
    }
}
