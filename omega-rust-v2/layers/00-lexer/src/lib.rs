/*
    L0: the Omega lexer. Source bytes in, tokens out.

    This is the first layer of Omega-V2. It cuts one source file into tokens
    and does nothing else: it decides where each token starts and ends, and it
    refuses files that break a lexical rule. It does not know what any word
    means. The rules it enforces, each with the spec section it comes from, are
    in CONTRACT.md beside this crate; that file is the authority and this code
    implements it.

    How it works. We check once that the whole file is valid UTF-8, then walk
    it byte by byte. All of Omega's syntax is ASCII, so every decision about
    where a token starts can be made from a single byte: a letter or underscore
    starts a word, a digit starts a number, `"` starts a quoted literal, a
    slash followed by a slash or a star starts a comment, and anything else is
    looked up in the punctuation table. Bytes above 0x7F are only legal inside comments and quoted literals,
    where we skip over them without interpreting them, so we never need to
    decode a character except to report how wide an illegal one was.

    Every byte of the file ends up inside exactly one token. Whitespace and
    comments are tokens too, of kinds a parser will skip. That gives us a check
    that does not depend on the lexer's own logic: lay the tokens end to end
    and you must get the original file back, byte for byte. The tests run that
    check on every fixture and on every code example in the spec.

    A token is a kind and a byte range, 32 bytes, with no pointer into the
    source. The spelling of a token is `source[span]`; the bytes a quoted
    literal denotes are computed on request by `decode_quoted_bytes`.
*/

/*
    Why the lexer has no keyword list.

    The obvious design classifies `machine`, `let`, `if` and friends as keyword
    tokens, and Omega V1 does exactly that with a table of 28 words. We do not,
    for two reasons.

    First, the spec gives the lexer nothing to go on. Its only statement about
    words is their shape, `[A-Za-z_][A-Za-z0-9_]*`, plus the remark that "no
    identifier is reserved for builtin types". There is no list of reserved
    words anywhere in it. V1's table is an implementation choice, and it has
    already drifted: it still reserves `state`, which the spec retired.

    Second, which words are special depends on where they appear, and that is
    grammar. `self` is a legal subject inside a domain's `requires` clause and
    a rejected receiver in a machine. Deciding that here would mean the lexer
    knows the grammar. So every word is one kind, `Word`, and the parser, which
    has to know the grammar anyway, decides what each one is.

    Why the first error stops the file.

    A lexer can recover from most errors and keep going, which lets a compiler
    report several mistakes at once. We stop at the first, because a recovered
    token stream is a guess, and every layer after this one would have to be
    correct on guessed input too. One error per file keeps the output either
    exactly right or absent.
*/

/*
    How the code is laid out.

    This file only declares the modules and names what the crate exports.
    Everything else is in four folders, one for each thing a reader might be
    looking for:

        lexing/     the work itself: the two entry points, the scanner that
                    makes the one pass, the byte tables it reads, and the
                    escape rules of a quoted literal.
        tokens/     what comes out: the token, its kinds, the punctuation
                    vocabulary and the radix of an integer.
        errors/     what a refusal carries: the error and the rule it names.
        locations/  where something is in a file: a byte range, and the line
                    and column a person would look for.

    The folders are not numbered, because they are not steps: the lexer is one
    pass, and these are the things that pass uses and produces. There is still
    a good order to read them in, and it crosses the folders:

        1. lexing/lexer.rs
           The two entry points, `lex` and `lex_into`, with a narrated example
           on each. This is all a caller ever touches.

        2. tokens/token.rs, tokens/token_kind.rs, locations/source_span.rs
           What comes back. A token is a kind and a span, and that is the whole
           output, so read these before the code that fills them in.

        3. lexing/token_scanner.rs
           The pass. `scan_all` is the loop that every byte goes through; read
           it first. The methods under it take over for the three kinds of
           token with more rules: numbers, quoted literals and block comments.

        4. lexing/byte_class.rs
           The table the loop looks each first byte up in. It is why the loop
           has so little in it.

        5. tokens/punctuation.rs
           The punctuation list, and how the longest spelling is chosen.

        6. lexing/quoted_literal_decoder.rs, lexing/quoted_escape_error.rs,
           tokens/integer_radix.rs
           The small rules the scanner's slower paths lean on: escapes inside a
           quoted literal, and the bases a number can be written in.

        7. errors/lex_error.rs, errors/lex_error_kind.rs
           What a refusal carries. The kinds line up with the rules in
           CONTRACT.md, so read the two side by side.

        8. locations/line_index.rs, locations/source_position.rs
           Only for showing a person where something is. Nothing above depends
           on these, so they can be left for last or skipped.

    The text dump that the golden fixtures compare is not here. Only the tests
    read it, so it lives with them in `tests/support/token_dump_formatter.rs`
    and uses nothing but what this crate exports.

    Each type has a file of its own, named after it, and keeps its fields
    private behind `get_` accessors. A folder's `mod.rs` declares its files and
    nothing else. Callers do not see the folders: everything public is
    re-exported from here, so it is `omega_lexer::Token` and not a path.

    The code follows the conventions of the Squalr codebase, with one measured
    exception: methods on the small `Copy` enums take `self` by value, where
    Squalr would take `&self`. See experiment 8 in EXPERIMENTS.md.
*/

/*
    Contract: CONTRACT.md in this crate.
    Tests: `tests/golden_fixtures.rs` (accept and reject fixtures under `fixtures/`),
    `tests/byte_cases.rs` (inputs that cannot be written as readable files), and
    `tests/spec_corpus.rs` (every `omega` example in `wiki/spec`).
    Measurements: the tools in `measurements/`, and what they found in EXPERIMENTS.md.
    Reference: Omega V1's lexer, `omega-rust/psi/compilation/pipeline/01_source-compilation/
    src/operations/lexing/lexer.rs`. We read it for behaviour the spec leaves open; CONTRACT.md
    lists where we deliberately differ.
*/

#![forbid(unsafe_code)]

mod errors;
mod lexing;
mod locations;
mod tokens;

pub use errors::lex_error::LexError;
pub use errors::lex_error_kind::LexErrorKind;
pub use lexing::lexer::{lex, lex_into};
pub use lexing::quoted_literal_decoder::decode_quoted_bytes;
pub use locations::line_index::LineIndex;
pub use locations::source_position::SourcePosition;
pub use locations::source_span::SourceSpan;
pub use tokens::integer_radix::IntegerRadix;
pub use tokens::punctuation::Punctuation;
pub use tokens::token::Token;
pub use tokens::token_kind::TokenKind;
