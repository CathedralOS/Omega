//! The two entry points of the layer: `parse_file` and `parse_fragment`.

use crate::errors::parse_error::ParseError;
use crate::parsing::fragment_kind::FragmentKind;
use crate::parsing::syntax_parser::SyntaxParser;
use crate::tree::syntax_tree::SyntaxTree;
use omega_lexer::Token;

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
/// use omega_parser::{NodeKind, TreeEvent, parse_file};
///
/// let source_bytes = b"machine double(value: u32) -> u32 { value * 2 }";
/// let tokens = lex(source_bytes).expect("Expected this source to be lexically valid.");
/// let syntax_tree = parse_file(source_bytes, &tokens).expect("Expected this source to parse.");
///
/// // The walk starts by opening the file, then the machine inside it.
/// let events = syntax_tree.get_events();
///
/// assert_eq!(events[0], TreeEvent::Open(NodeKind::File));
/// assert_eq!(events[1], TreeEvent::Open(NodeKind::Machine));
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
    let mut syntax_parser = SyntaxParser::new(source_bytes, tokens)?;

    if syntax_parser.parse_file_root().is_err() {
        return Err(syntax_parser.get_error());
    }

    syntax_parser.finish("a declaration")
}

/// Parses a piece of source that is not a whole file. See `FragmentKind` for why this exists.
///
/// ```
/// use omega_lexer::lex;
/// use omega_parser::{FragmentKind, NodeKind, TreeEvent, parse_fragment};
///
/// let source_bytes = b"requires count > 0 ensures result >= count";
/// let tokens = lex(source_bytes).expect("Expected this source to be lexically valid.");
/// let syntax_tree = parse_fragment(source_bytes, &tokens, FragmentKind::Clauses).expect("Expected these clauses to parse.");
///
/// assert_eq!(syntax_tree.get_events()[1], TreeEvent::Open(NodeKind::Requires));
/// ```
pub fn parse_fragment(
    source_bytes: &[u8],
    tokens: &[Token],
    fragment_kind: FragmentKind,
) -> Result<SyntaxTree, ParseError> {
    let mut syntax_parser = SyntaxParser::new(source_bytes, tokens)?;

    if syntax_parser.parse_fragment_root(fragment_kind).is_err() {
        return Err(syntax_parser.get_error());
    }

    syntax_parser.finish("the end of the fragment")
}
