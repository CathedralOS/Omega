/*
    L1: the Omega parser. Tokens in, a syntax tree out.

    This is the second layer of Omega-V2. It takes the token list the lexer
    produced for one source file and decides how the tokens group: which run of
    them is a machine, which is its header and which its body, where each
    expression starts and ends and which operator applies to which operands. It
    refuses files whose tokens do not fit the grammar. It does not know what
    any name refers to; `Outcome`, `u32` and `x` are all just names here. The
    grammar it implements is GRAMMAR.md beside this crate; that file is the
    authority and this code follows it.

    How it works. It is a recursive descent parser: one function per rule of
    the grammar, each of which looks at the next token or two, decides which
    alternative it is in, and calls the functions for the parts. Binary
    operators are the exception; they use precedence climbing, one function
    with a table of binding levels, described where it lives in
    `grammar/expressions.rs`.

    The lexer has no keyword list, so no word is reserved. `machine` is a word
    like `x`, and it introduces a declaration only where a declaration can
    start and a name follows it. Each such decision is made by spelling, at the
    one place in the grammar that asks; see :DeclarationWords, :DispatchKeyword
    and :PrefixWords for the three families.

    The output is not a tree of allocated nodes. It is a flat list of events,
    `Open(kind)`, `Token` and `Close`, in the order a walk of the tree would
    visit them. `SyntaxTree` explains why, and what that buys.

    Every meaningful token of the file ends up in the tree exactly once, in
    order. As with the lexer's rule that tokens tile the source, this gives the
    tests a check that does not depend on the grammar being right: whatever the
    parser decided about grouping, it may not have dropped or repeated a token,
    and every bracket it consumed must close inside the node that opened it.
*/

/*
    Why the parser never tries one reading and falls back to another.

    Several places in the grammar start alike and diverge later. A generic
    argument may be a type or a value. An arm's pattern may be a tuple pattern
    or a parenthesized condition. A word followed by `<` at the top of a file
    may open a conformance. The easy way to handle each is to try the first
    reading and, if it fails, rewind and try the second.

    We do not, because those readings can contain each other. A type can hold
    an expression (`Buffer<N + 1>`), and an expression can hold a type
    (`f::<Buffer<N>>()`), so a failed attempt at one level retries everything
    beneath it, and each level beneath does the same. On ordinary source that
    never matters. On source built to provoke it, the time doubles with every
    level of nesting, and a compiler that can be made to hang by a 200-byte
    file is not solid.

    So every such choice is made by looking ahead a bounded distance and then
    committing. The rules are written at each site (:GenericArgumentKind,
    :ParenthesizedPattern), and each costs the language a corner: a few things
    that a backtracking parser would accept must be written with parentheses.
    Those corners are listed in GRAMMAR.md.

    Why the first error stops the file.

    For the reason the lexer gives. A parser that recovers and carries on
    produces a tree that is partly a guess, and every later layer would have to
    be correct on guessed input. One error per file keeps the output either
    exactly right or absent.
*/

/*
    How the code is laid out.

    This file declares the modules, names what the crate exports, and holds the
    two entry points. `parser.rs` is the machinery: the cursor over the tokens
    and the event list being built. `grammar/` is the grammar itself, one file
    per section of GRAMMAR.md, each a set of methods on `Parser`. Each type has
    a file of its own, named after it.

    Contract: GRAMMAR.md in this crate.
    Tests: `tests/golden_fixtures.rs` (accept and reject fixtures under `fixtures/`),
    `tests/spec_corpus.rs` (every `omega` example in `wiki/spec`), and
    `tests/random_inputs.rs` (the parser must refuse garbage without crashing or hanging).
    Reference: Omega V1's parser, `omega-rust/psi/compilation/pipeline/01_source-compilation/
    src/operations/parsing/`. GRAMMAR.md lists where V2 deliberately differs.
*/

#![forbid(unsafe_code)]

mod event;
mod expression_context;
mod fragment_kind;
mod grammar;
mod node_kind;
mod parse_error;
mod parse_error_kind;
mod parser;
mod syntax_tree;
mod tree_dump;

pub use event::Event;
pub use fragment_kind::FragmentKind;
pub use node_kind::NodeKind;
pub use parse_error::ParseError;
pub use parse_error_kind::ParseErrorKind;
pub use syntax_tree::SyntaxTree;
pub use tree_dump::{dump_parse_error, dump_tree};

use omega_lexer::Token;
use parser::Parser;

/// Groups one source file's tokens into a syntax tree, or reports the first place they leave the grammar.
///
/// `tokens` must be what the lexer produced for `source_bytes`; the parser reads each word's
/// spelling out of the source. The tree refers to tokens by their position in `tokens`, so the
/// caller keeps both alive for as long as it uses the tree.
///
/// A typical caller lexes a file, parses it, and walks the events:
///
/// ```
/// use omega_lexer::lex;
/// use omega_parser::{Event, NodeKind, parse_file};
///
/// let source_bytes = b"machine double(value: u32) -> u32 { value * 2 }";
/// let tokens = lex(source_bytes).expect("Expected this source to be lexically valid.");
/// let syntax_tree = parse_file(source_bytes, &tokens).expect("Expected this source to parse.");
///
/// // The walk starts by opening the file, then the machine inside it.
/// let events = syntax_tree.get_events();
///
/// assert_eq!(events[0], Event::Open(NodeKind::File));
/// assert_eq!(events[1], Event::Open(NodeKind::Machine));
///
/// // Every meaningful token is in the tree exactly once. To read one, follow its index back
/// // into the lexer's list: the first token event here is the word `machine`.
/// let first_token_index = syntax_tree.get_meaningful_token_indices()[0] as usize;
///
/// assert_eq!(tokens[first_token_index].get_span().slice(source_bytes), Some(&b"machine"[..]));
/// ```
pub fn parse_file(
    source_bytes: &[u8],
    tokens: &[Token],
) -> Result<SyntaxTree, ParseError> {
    let mut parser = Parser::new(source_bytes, tokens)?;

    if parser.parse_file_root().is_err() {
        return Err(parser.get_error());
    }

    parser.finish("a declaration")
}

/// Parses a piece of source that is not a whole file. See `FragmentKind` for why this exists.
///
/// ```
/// use omega_lexer::lex;
/// use omega_parser::{Event, FragmentKind, NodeKind, parse_fragment};
///
/// let source_bytes = b"requires count > 0 ensures result >= count";
/// let tokens = lex(source_bytes).expect("Expected this source to be lexically valid.");
/// let syntax_tree = parse_fragment(source_bytes, &tokens, FragmentKind::Clauses).expect("Expected these clauses to parse.");
///
/// assert_eq!(syntax_tree.get_events()[1], Event::Open(NodeKind::Requires));
/// ```
pub fn parse_fragment(
    source_bytes: &[u8],
    tokens: &[Token],
    fragment_kind: FragmentKind,
) -> Result<SyntaxTree, ParseError> {
    let mut parser = Parser::new(source_bytes, tokens)?;

    if parser.parse_fragment_root(fragment_kind).is_err() {
        return Err(parser.get_error());
    }

    parser.finish("the end of the fragment")
}
