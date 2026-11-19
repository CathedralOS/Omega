//! Inputs built to break a parser: deep nesting, readings that contain each other, and noise.

mod support;

use omega_lexer::lex;
use omega_parser::{FragmentKind, ParseErrorKind, parse_file, parse_fragment};
use std::thread;
use std::time::{Duration, Instant};

/// The main thread's stack on Windows. Test threads get twice this, which would hide an overflow.
const SMALL_STACK_BYTE_COUNT: usize = 1024 * 1024;

/// Far more nesting than :NestingLimit allows, and enough to overflow any stack if it were followed.
const HOSTILE_DEPTH: usize = 100_000;

/// Parses `source_text` as a file on a 1 MB stack and returns the kind of error, if any.
fn parse_on_small_stack(source_text: String) -> Option<ParseErrorKind> {
    thread::Builder::new()
        .stack_size(SMALL_STACK_BYTE_COUNT)
        .spawn(move || {
            let source_bytes = source_text.as_bytes();
            let tokens = lex(source_bytes).expect("Expected the hostile source to be lexically valid.");

            parse_file(source_bytes, &tokens)
                .err()
                .map(|parse_error| parse_error.get_kind())
        })
        .expect("Expected a thread to start.")
        .join()
        .expect("Expected the parser to return, not to overflow its stack or panic.")
}

/// Each recursive rule of the grammar, nested far past the limit. All must be refused cleanly.
#[test]
fn deep_nesting_is_refused_without_overflowing_the_stack() {
    let hostile_sources = [
        (
            "parentheses",
            format!("machine f() {{ _ = {}a{}; }}", "(".repeat(HOSTILE_DEPTH), ")".repeat(HOSTILE_DEPTH)),
        ),
        (
            "array literals",
            format!("machine f() {{ _ = {}a{}; }}", "[".repeat(HOSTILE_DEPTH), "]".repeat(HOSTILE_DEPTH)),
        ),
        ("unary operators", format!("machine f() {{ _ = {}a; }}", "-".repeat(HOSTILE_DEPTH))),
        ("borrows", format!("machine f() {{ _ = {}a; }}", "& ".repeat(HOSTILE_DEPTH))),
        ("prefix words", format!("machine f() {{ _ = {}a; }}", "move ".repeat(HOSTILE_DEPTH))),
        (
            "calls as arguments",
            format!("machine f() {{ _ = {}a{}; }}", "g(".repeat(HOSTILE_DEPTH), ")".repeat(HOSTILE_DEPTH)),
        ),
        // Spaced, because the lexer reads `&&` as one token, which no type begins with.
        ("reference types", format!("machine f(a: {}u8) {{ }}", "& ".repeat(HOSTILE_DEPTH))),
        (
            "slice types",
            format!("machine f(a: {}u8{}) {{ }}", "[".repeat(HOSTILE_DEPTH), "]".repeat(HOSTILE_DEPTH)),
        ),
        ("function types", format!("machine f(a: {}u8) {{ }}", "u8 -> ".repeat(HOSTILE_DEPTH))),
        (
            "generic arguments",
            format!("machine f(a: {}u8{}) {{ }}", "V<".repeat(HOSTILE_DEPTH), ">".repeat(HOSTILE_DEPTH)),
        ),
        (
            "expression arguments",
            format!("machine f() {{ _ = {}a{}; }}", "g::<".repeat(HOSTILE_DEPTH), ">".repeat(HOSTILE_DEPTH)),
        ),
        (
            "tuple patterns",
            format!("machine f() {{ let {}a{} = b; }}", "(".repeat(HOSTILE_DEPTH), ")".repeat(HOSTILE_DEPTH)),
        ),
        (
            "nested matches",
            format!(
                "machine f() {{ _ = {}a{}; }}",
                "match { _ -> ".repeat(HOSTILE_DEPTH),
                " }".repeat(HOSTILE_DEPTH)
            ),
        ),
        (
            "nested machines",
            format!("{}{}", "machine f() { ".repeat(HOSTILE_DEPTH), "}".repeat(HOSTILE_DEPTH)),
        ),
        ("nested data", format!("{}{}", "data D { ".repeat(HOSTILE_DEPTH), "}".repeat(HOSTILE_DEPTH))),
        (
            "nested asm machines",
            format!("{}{}", "asm machine f { ".repeat(HOSTILE_DEPTH), "}".repeat(HOSTILE_DEPTH)),
        ),
        (
            "machine types",
            format!("machine f(a: {}u8{}) {{ }}", "machine(b: ".repeat(HOSTILE_DEPTH), ")".repeat(HOSTILE_DEPTH)),
        ),
    ];

    for (construct, source_text) in hostile_sources {
        assert_eq!(
            parse_on_small_stack(source_text),
            Some(ParseErrorKind::NestingTooDeep),
            "{HOSTILE_DEPTH} nested {construct} should be refused as too deeply nested"
        );
    }
}

/// Nesting just inside the limit parses, on the same small stack.
#[test]
fn nesting_inside_the_limit_parses() {
    let depth = 100;
    let sources = [
        format!("machine f() {{ _ = {}a{}; }}", "(".repeat(depth), ")".repeat(depth)),
        format!("machine f(a: {}u8{}) {{ }}", "V<".repeat(depth), ">".repeat(depth)),
        format!("{}{}", "machine f() { ".repeat(depth), "}".repeat(depth)),
    ];

    for source_text in sources {
        assert_eq!(parse_on_small_stack(source_text), None);
    }
}

/// Each nested data declaration consumes exactly one level, including the outermost one.
#[test]
fn the_nesting_limit_accepts_128_levels_and_refuses_the_next() {
    for (depth, expected_error) in [(128, None), (129, Some(ParseErrorKind::NestingTooDeep))] {
        let source = format!("{}{}", "data D {".repeat(depth), "}".repeat(depth));
        assert_eq!(parse_on_small_stack(source), expected_error, "{depth} nested declarations");
    }
}

/*
    A long chain is not nesting. `a + a + a + ...` and `f()()()...` are one
    level deep however long they are, and the parser handles them in a loop.
    This would fail with NestingTooDeep if either were implemented by recursion
    on the left operand.
*/
#[test]
fn long_chains_are_not_nesting() {
    let link_count = 20_000;
    let sources = [
        format!("machine f() {{ _ = a{}; }}", " + a".repeat(link_count)),
        format!("machine f() {{ _ = a{}; }}", ".b".repeat(link_count)),
        format!("machine f() {{ _ = a{}; }}", "()".repeat(link_count)),
        format!("machine f() {{ _ = a{}; }}", "[0]".repeat(link_count)),
        format!("machine f() {{ _ = a{}; }}", " as u8".repeat(link_count)),
        format!("machine f() {{ {} }}", "g();".repeat(link_count)),
        "machine f() { }\n".repeat(link_count),
    ];

    for source_text in sources {
        assert_eq!(parse_on_small_stack(source_text), None);
    }
}

/*
    The inputs a try-and-rewind parser takes exponential time on.

    Each alternates two readings that can contain each other, nested to the
    limit, and ends in a way that makes the whole thing fail. A parser that
    tries the first reading, fails deep inside, rewinds and tries the second
    does so again at every level beneath, doubling the work per level: at 60
    levels that is longer than anyone will wait. This parser decides each by
    looking ahead, so it fails as fast as it succeeds. The bound here is
    generous by four orders of magnitude; the test is for doubling, not speed.
*/
#[test]
fn readings_that_contain_each_other_do_not_take_exponential_time() {
    let level_count = 60;
    let hostile_sources = [
        // A type argument holding an expression holding a type argument, never closed.
        format!("machine f(a: {} !) {{ }}", "A<f::<".repeat(level_count)),
        // The same, entered from an expression.
        format!("machine f() {{ _ = {} !; }}", "f::<A<".repeat(level_count)),
        // A cast whose type has arguments holding a cast.
        format!("machine f() {{ _ = {} !; }}", "x as T<".repeat(level_count)),
        // A parenthesized pattern holding a match holding a parenthesized pattern.
        format!("machine f() {{ _ = {} !; }}", "match { (".repeat(level_count)),
        // An array-shaped argument that may be a type or a literal at every level.
        format!("machine f(a: {} !) {{ }}", "A<[".repeat(level_count)),
    ];

    for source_text in hostile_sources {
        let started_at = Instant::now();
        let error_kind = parse_on_small_stack(source_text.clone());

        assert!(error_kind.is_some(), "this source is unfinished and must be refused: {source_text}");
        assert!(
            started_at.elapsed() < Duration::from_secs(2),
            "took {:?}, which suggests the parser is retrying: {source_text}",
            started_at.elapsed()
        );
    }
}

/// A deterministic stream of numbers, so a failure can be reproduced from the seed it prints.
struct NumberStream {
    state: u64,
}

impl NumberStream {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// SplitMix64. Small, well distributed, and has no state to get stuck in.
    fn next_number(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);

        let mut mixed = self.state;

        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);

        mixed ^ (mixed >> 31)
    }

    fn next_below(
        &mut self,
        bound: usize,
    ) -> usize {
        (self.next_number() % bound as u64) as usize
    }
}

/// The pieces random sources are built from: every word the grammar looks for, and every kind of token.
const SOURCE_PIECES: &[&str] = &[
    "machine",
    "data",
    "domain",
    "trait",
    "use",
    "const",
    "let",
    "mut",
    "match",
    "transition",
    "asm",
    "public",
    "boundary",
    "satisfies",
    "requires",
    "ensures",
    "reaches",
    "crashes",
    "blocks",
    "suspends",
    "terminates",
    "by",
    "in",
    "as",
    "invokes",
    "registers",
    "clobbers",
    "case",
    "move",
    "suspend",
    "block",
    "some",
    "if",
    "self",
    "none",
    "dropped",
    "access",
    "_",
    "a",
    "b",
    "T",
    "u32",
    "0",
    "1u8",
    "2.5",
    "\"s\"",
    "(",
    ")",
    "[",
    "]",
    "{",
    "}",
    "<",
    ">",
    ",",
    ";",
    ":",
    "::",
    ".",
    "..",
    "..=",
    "->",
    "=",
    "==",
    "!=",
    "<=",
    ">=",
    "+",
    "-",
    "*",
    "/",
    "%",
    "&",
    "&&",
    "|",
    "||",
    "^",
    "~",
    "!",
    "#",
    "'",
    "<<",
    "+%",
    "+=",
];

fn build_random_source(
    number_stream: &mut NumberStream,
    piece_count: usize,
) -> String {
    let mut source_text = String::new();

    for _ in 0..piece_count {
        source_text.push_str(SOURCE_PIECES[number_stream.next_below(SOURCE_PIECES.len())]);

        // Sometimes no space, so that tokens touch and right shifts and policy operators can form.
        if number_stream.next_below(4) != 0 {
            source_text.push(' ');
        }
    }

    source_text
}

/*
    Noise built from the grammar's own vocabulary. Almost all of it is refused,
    which is the point: the parser must refuse it by returning an error, never
    by panicking or hanging, and on the rare input it accepts, the tree must
    still pass every grammar-independent check. Every entry point is driven.
*/
#[test]
fn random_token_soup_is_refused_or_parsed_into_a_sound_tree() {
    let mut number_stream = NumberStream::new(0x0E6A_2026_1118);
    let mut accepted_count = 0usize;
    let source_count = 30_000;

    for source_index in 0..source_count {
        let piece_count = 1 + number_stream.next_below(40);
        let source_text = build_random_source(&mut number_stream, piece_count);
        let source_bytes = source_text.as_bytes();

        // `'` and `"` pieces can form things the lexer refuses; those are not this layer's inputs.
        let Ok(tokens) = lex(source_bytes) else {
            continue;
        };

        let parse_results = [
            parse_file(source_bytes, &tokens),
            parse_fragment(source_bytes, &tokens, FragmentKind::Statements),
            parse_fragment(source_bytes, &tokens, FragmentKind::Clauses),
            parse_fragment(source_bytes, &tokens, FragmentKind::GenericParam),
        ];

        for syntax_tree in parse_results.into_iter().flatten() {
            accepted_count += 1;

            if let Some(problem) = support::describe_tree_problem(source_bytes, &tokens, &syntax_tree) {
                panic!("source {source_index} parsed into a malformed tree: {problem}\n{source_text}");
            }
        }
    }

    // If nothing were ever accepted the soundness check above would be testing nothing.
    assert!(
        accepted_count > 100,
        "only {accepted_count} of {source_count} random sources parsed under any entry point"
    );
    println!("{accepted_count} parses succeeded across {source_count} random sources");
}

/*
    Every accept fixture, with one token removed, for every token. This is the
    closest thing to "nearly right" source: one keystroke away from a file we
    know is valid. The parser must survive all of it, and what it still accepts
    must be a sound tree.
*/
#[test]
fn fixtures_with_one_token_removed_never_break_the_parser() {
    let fixture_directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/accept");
    let mut mutant_count = 0usize;
    let mut refused_count = 0usize;
    // Which tokens a file survives losing, by spelling. Printed, so a new optional token is noticed.
    let mut survivable_spellings: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();

    for directory_entry in std::fs::read_dir(&fixture_directory).expect("Expected the accept fixtures to be readable.") {
        let fixture_path = directory_entry.expect("Expected a directory entry.").path();

        if fixture_path
            .extension()
            .is_none_or(|extension| extension != "omg")
        {
            continue;
        }

        let source_bytes = std::fs::read(&fixture_path).expect("Expected the fixture to be readable.");
        let tokens = lex(&source_bytes).expect("Expected every accept fixture to lex.");

        for removed_token in tokens.iter().filter(|token| !token.get_kind().is_trivia()) {
            let removed_span = removed_token.get_span();
            let mut mutant_bytes = source_bytes.clone();

            // Blank the token rather than cut it out, so its neighbours do not fuse into a new token.
            for mutant_byte in &mut mutant_bytes[removed_span.get_start_offset()..removed_span.get_end_offset()] {
                *mutant_byte = b' ';
            }

            let Ok(mutant_tokens) = lex(&mutant_bytes) else {
                continue;
            };

            mutant_count += 1;

            match parse_file(&mutant_bytes, &mutant_tokens) {
                Err(_) => refused_count += 1,
                Ok(syntax_tree) => {
                    if let Some(problem) = support::describe_tree_problem(&mutant_bytes, &mutant_tokens, &syntax_tree) {
                        panic!(
                            "{} with bytes {}..{} blanked parsed into a malformed tree: {problem}",
                            fixture_path.display(),
                            removed_span.get_start_offset(),
                            removed_span.get_end_offset()
                        );
                    }

                    let removed_spelling = String::from_utf8_lossy(removed_span.slice(&source_bytes).unwrap_or_default()).into_owned();

                    *survivable_spellings.entry(removed_spelling).or_default() += 1;
                }
            }
        }
    }

    assert!(mutant_count > 1000, "only {mutant_count} mutants were built; are the fixtures there?");
    println!("{refused_count} of {mutant_count} one-token-removed fixtures were refused");
    println!("tokens a fixture survived losing: {survivable_spellings:?}");
}
