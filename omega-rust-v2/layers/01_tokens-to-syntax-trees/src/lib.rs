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

    This file only declares the modules and names what the crate exports.
    Everything else is in four folders, one for each thing a reader might be
    looking for:

        parsing/    the machinery: the two entry points, the parser that walks
                    the tokens and builds the tree, and the small types it
                    hands to the grammar.
        grammar/    the grammar itself, one file per section of GRAMMAR.md,
                    each a set of methods on `SyntaxParser`. Its `contexts/`
                    folder holds what a caller tells a rule about where it
                    sits.
        tree/       what comes out: the tree, its events and the node kinds.
        errors/     what a refusal carries: the error and the reason it names.

    The folders are not numbered, because they are not steps: the parser is
    one pass. There is still a good order to read them in:

        1. parsing/entry_points.rs
           `parse_file` and `parse_fragment`, with a narrated example on each.
           This is all a caller ever touches.

        2. tree/syntax_tree.rs, tree/tree_event.rs, tree/node_kind.rs
           What comes back. `syntax_tree.rs` says why the tree is a flat list
           of events and not nodes that point at each other; read that before
           the code that writes the list.

        3. parsing/syntax_parser.rs
           The machinery every grammar rule is written in terms of: looking at
           the next tokens, consuming one, and opening and closing nodes. Read
           it with `parsing/parse_failed.rs`, which says why a failure carries
           no error, and `open_node.rs` and `closed_node.rs` beside it.

        4. grammar/roots.rs, grammar/items.rs
           Where a parse starts, and how the parser decides which declaration
           begins at the cursor when no word is reserved.

        5. grammar/data.rs, grammar/traits.rs, grammar/assembly.rs,
           grammar/clauses.rs, grammar/generics.rs, grammar/types.rs
           The declarations and what their headers are made of. Each file
           follows its section of GRAMMAR.md, so read the two side by side.

        6. grammar/statements.rs, grammar/dispatch.rs
           Bodies, then `match` and `transition` and the patterns in their
           arms.

        7. grammar/expressions.rs
           The largest file, left until the rest is familiar. Binary operators
           are the one place that is not one function per rule.

        8. grammar/contexts/
           Not read in a sitting. Open one when a rule takes it as an argument:
           each says what the position allows and why the tokens cannot.

        9. errors/parse_error.rs, errors/parse_error_kind.rs
           What a refusal carries.

       10. parsing/fragment_kind.rs
           Only for parsing the spec's examples, which are mostly not whole
           files. The compiler never uses it.

    Each type that more than one file uses has a file of its own, named after
    it, and keeps its fields private behind `get_` accessors. A type only one
    grammar file uses stays in that file. A folder's `mod.rs` declares its
    files and nothing else. Callers do not see the folders: everything public
    is re-exported from here, so it is `omega_parser::SyntaxTree` and not a
    path.

    The text dump that the golden fixtures compare is not here. Only the tests
    and the `parse_spec` tool read it, so it lives with the tests in
    `tests/support/tree_dump_formatter.rs` and uses nothing but what this crate
    exports.

    Contract: GRAMMAR.md in this crate.
    Tests: `tests/golden_fixtures.rs` (accept and reject fixtures under `fixtures/`),
    `tests/spec_corpus.rs` (every `omega` example in `wiki/spec`), and
    `tests/hostile_inputs.rs` (the parser must refuse garbage without crashing or hanging).
    `mutation_check.sh` breaks the grammar one rule at a time and confirms the tests notice.
    Measurements: `measurements/parse_throughput.rs`, and what it found in EXPERIMENTS.md.
    Reference: Omega V1's parser, `omega-rust/psi/compilation/pipeline/01_source-compilation/
    src/operations/parsing/`. GRAMMAR.md lists where V2 deliberately differs.
*/

#![forbid(unsafe_code)]

mod errors;
mod grammar;
mod parsing;
mod tree;

pub use errors::parse_error::ParseError;
pub use errors::parse_error_kind::ParseErrorKind;
pub use parsing::entry_points::{parse_file, parse_fragment};
pub use parsing::fragment_kind::FragmentKind;
pub use tree::indexed_syntax::{IndexedSyntax, Leaf as SyntaxLeaf, MathBinding, Node as SyntaxNode, PauseFlag, RangeBounds, SyntaxOperator, TerminationClause};
pub use tree::node_kind::NodeKind;
pub use tree::syntax_tree::SyntaxTree;
pub use tree::tree_event::TreeEvent;
