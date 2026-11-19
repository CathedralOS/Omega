//! The token cursor and the tree builder that the grammar functions drive.

use crate::errors::parse_error::ParseError;
use crate::errors::parse_error_kind::ParseErrorKind;
use crate::parsing::closed_node::ClosedNode;
use crate::parsing::open_node::OpenNode;
use crate::parsing::parse_failed::{ParseFailed, Parsed};
use crate::tree::node_kind::NodeKind;
use crate::tree::syntax_tree::SyntaxTree;
use crate::tree::tree_event::TreeEvent;
use omega_lexer::{Punctuation, SourceSpan, Token, TokenKind};

/*
    :NestingLimit
    The grammar functions call each other recursively, one group of calls per
    level of nesting in the source, so a file of ten thousand `(` would run the
    thread out of stack and take the process down with it. We count levels and
    refuse the file instead. The number is small enough that a debug build on a
    1 MB stack (the main thread on Windows) stays well inside it, and far larger
    than anything a person writes. The spec states no limit; GRAMMAR.md records
    this one as ours.
*/
const NESTING_LIMIT: u32 = 128;

/*
    The parser reads the lexer's tokens with whitespace and comments skipped,
    and writes a list of events. See `SyntaxTree` for what the events mean.

    The grammar functions (in `grammar/`) look at the next few tokens through
    `at`, `at_keyword` and friends, consume one with `bump`, and bracket what
    they consume with `open` and `close`. They never build a node object.

    Some nodes are only recognized after their first child is complete: we have
    parsed `a`, and only on seeing `+` do we learn it was the left side of an
    addition. `open_before` handles that by inserting an `Open` event in front
    of the finished child.

    Nothing here can take back a token once it is consumed. Every choice the
    grammar makes is made by looking ahead, never by trying one reading and
    rewinding to try another. That is deliberate: two readings that can each
    contain the other (a type inside an expression inside a type) make a
    try-and-rewind parser take time exponential in the nesting depth on input
    built to provoke it.

    @Speed: `open_before` shifts every event after the insertion point. The
    child being wrapped is always the most recent thing parsed, so the shift
    covers that child only, and a chain like `a + b + c + ...` costs the square
    of its length. Real expressions are short. If a profile ever shows this, the
    fix is the usual one: record a "my parent starts N events back" link
    instead of inserting, and resolve the links in one pass at the end.
*/
pub(crate) struct SyntaxParser<'source> {
    source_bytes: &'source [u8],
    tokens: &'source [Token],
    meaningful_token_indices: Vec<u32>, // Positions in `tokens` of everything that is not whitespace or a comment.
    token_position: usize,              // Index into `meaningful_token_indices` of the next token to consume.
    events: Vec<TreeEvent>,
    nesting_depth: u32,
    recorded_error: Option<ParseError>, // The error behind the most recent `ParseFailed`. See :SmallFailure.
}

impl<'source> SyntaxParser<'source> {
    pub(crate) fn new(
        source_bytes: &'source [u8],
        tokens: &'source [Token],
    ) -> Result<Self, ParseError> {
        // Small files often have more than half meaningful tokens. Reserving room for each
        // token avoids copying the index as it grows and caps the initial request at 4 KiB
        // on this path. See experiment 9 in EXPERIMENTS.md.
        let index_capacity = if tokens.len() <= 1024 { tokens.len() } else { tokens.len() / 2 + 1 };
        let mut meaningful_token_indices = Vec::with_capacity(index_capacity);

        for (token_index, token) in tokens.iter().enumerate() {
            if token.get_kind().is_trivia() {
                continue;
            }

            meaningful_token_indices.push(Self::compact_token_index(token_index, token.get_span())?);
        }

        // A tree holds a little under three events per meaningful token on the fixtures.
        let event_capacity = meaningful_token_indices.len() * 3 + 16;

        Ok(Self {
            source_bytes,
            tokens,
            meaningful_token_indices,
            token_position: 0,
            events: Vec::with_capacity(event_capacity),
            nesting_depth: 0,
            recorded_error: None,
        })
    }

    fn compact_token_index(
        index: usize,
        span: SourceSpan,
    ) -> Result<u32, ParseError> {
        u32::try_from(index).map_err(|_| ParseError::new(ParseErrorKind::SourceTooLarge, "fewer tokens", span))
    }

    /// Hands over the finished tree. Every token must have been consumed.
    pub(crate) fn finish(
        self,
        expected: &'static str,
    ) -> Result<SyntaxTree, ParseError> {
        if !self.at_end() {
            return Err(self.build_error(expected));
        }

        Ok(SyntaxTree::new(self.events, self.meaningful_token_indices))
    }

    // ---- Looking at tokens ----

    fn get_token(
        &self,
        ahead: usize,
    ) -> Option<Token> {
        let token_index = *self.meaningful_token_indices.get(self.token_position + ahead)?;

        self.tokens.get(token_index as usize).copied()
    }

    pub(crate) fn at_end(&self) -> bool {
        self.token_position >= self.meaningful_token_indices.len()
    }

    /// True if there is a token `ahead` places on.
    pub(crate) fn nth_exists(
        &self,
        ahead: usize,
    ) -> bool {
        self.token_position + ahead < self.meaningful_token_indices.len()
    }

    pub(crate) fn nth_punctuation(
        &self,
        ahead: usize,
    ) -> Option<Punctuation> {
        match self.get_token(ahead)?.get_kind() {
            TokenKind::Punctuation(punctuation) => Some(punctuation),
            _ => None,
        }
    }

    pub(crate) fn nth_at(
        &self,
        ahead: usize,
        punctuation: Punctuation,
    ) -> bool {
        self.nth_punctuation(ahead) == Some(punctuation)
    }

    pub(crate) fn at(
        &self,
        punctuation: Punctuation,
    ) -> bool {
        self.nth_at(0, punctuation)
    }

    /// The spelling of the token `ahead` places on, if it is a word.
    pub(crate) fn nth_word(
        &self,
        ahead: usize,
    ) -> Option<&'source [u8]> {
        let token = self.get_token(ahead)?;

        match token.get_kind() {
            TokenKind::Word => token.get_span().slice(self.source_bytes),
            _ => None,
        }
    }

    pub(crate) fn nth_at_word(
        &self,
        ahead: usize,
    ) -> bool {
        matches!(self.get_token(ahead), Some(token) if token.get_kind() == TokenKind::Word)
    }

    pub(crate) fn at_word(&self) -> bool {
        self.nth_at_word(0)
    }

    /// True if the token `ahead` places on is the word `keyword`.
    ///
    /// No word is reserved, so this is how the grammar recognizes `machine`, `let` and the rest:
    /// by spelling, and only in the positions where the grammar asks.
    pub(crate) fn nth_at_keyword(
        &self,
        ahead: usize,
        keyword: &str,
    ) -> bool {
        self.nth_word(ahead) == Some(keyword.as_bytes())
    }

    pub(crate) fn at_keyword(
        &self,
        keyword: &str,
    ) -> bool {
        self.nth_at_keyword(0, keyword)
    }

    pub(crate) fn nth_at_literal(
        &self,
        ahead: usize,
    ) -> bool {
        matches!(
            self.get_token(ahead).map(|token| token.get_kind()),
            Some(TokenKind::Integer { .. } | TokenKind::Float { .. } | TokenKind::Quoted)
        )
    }

    pub(crate) fn at_literal(&self) -> bool {
        self.nth_at_literal(0)
    }

    pub(crate) fn at_integer(&self) -> bool {
        matches!(self.get_token(0).map(|token| token.get_kind()), Some(TokenKind::Integer { .. }))
    }

    /// True if the token `ahead` places on and the one after it have no byte between them.
    ///
    /// The lexer has no `>>` token, so right shift is two `>` that touch; this is the test.
    pub(crate) fn nth_touches_next(
        &self,
        ahead: usize,
    ) -> bool {
        match (self.get_token(ahead), self.get_token(ahead + 1)) {
            (Some(first_token), Some(second_token)) => first_token.get_span().get_end_offset() == second_token.get_span().get_start_offset(),
            _ => false,
        }
    }

    // ---- Consuming tokens ----

    /// Consumes the next token into the node that is currently open. Does nothing at the end.
    pub(crate) fn bump(&mut self) {
        if self.at_end() {
            return;
        }

        self.events.push(TreeEvent::Token);
        self.token_position += 1;
    }

    pub(crate) fn eat(
        &mut self,
        punctuation: Punctuation,
    ) -> bool {
        let is_match = self.at(punctuation);

        if is_match {
            self.bump();
        }

        is_match
    }

    pub(crate) fn eat_keyword(
        &mut self,
        keyword: &str,
    ) -> bool {
        let is_match = self.at_keyword(keyword);

        if is_match {
            self.bump();
        }

        is_match
    }

    pub(crate) fn expect(
        &mut self,
        punctuation: Punctuation,
        expected: &'static str,
    ) -> Parsed {
        if self.eat(punctuation) { Ok(()) } else { Err(self.error(expected)) }
    }

    pub(crate) fn expect_word(
        &mut self,
        expected: &'static str,
    ) -> Parsed {
        if !self.at_word() {
            return Err(self.error(expected));
        }

        self.bump();

        Ok(())
    }

    pub(crate) fn expect_keyword(
        &mut self,
        keyword: &str,
        expected: &'static str,
    ) -> Parsed {
        if self.eat_keyword(keyword) { Ok(()) } else { Err(self.error(expected)) }
    }

    // ---- Errors ----

    /// Records an error at the next token (unexpected token, or unexpected end if there is none)
    /// and returns the marker to fail with. See :SmallFailure.
    #[cold]
    pub(crate) fn error(
        &mut self,
        expected: &'static str,
    ) -> ParseFailed {
        self.fail_with(self.build_error(expected))
    }

    /// Records an error of a specific kind at the next token.
    #[cold]
    pub(crate) fn error_of_kind(
        &mut self,
        kind: ParseErrorKind,
        expected: &'static str,
    ) -> ParseFailed {
        self.fail_with(ParseError::new(kind, expected, self.get_span(0)))
    }

    /// Records `parse_error` as the reason parsing is stopping.
    ///
    /// Nothing in the grammar recovers from a failure, so the error recorded last is the one the
    /// caller is told about. The one place that records twice is :IfStatement, which replaces a
    /// plain error with a more useful one.
    pub(crate) fn fail_with(
        &mut self,
        parse_error: ParseError,
    ) -> ParseFailed {
        ParseFailed::record(&mut self.recorded_error, parse_error)
    }

    /// A failure marker is only issued after recording a diagnostic.
    #[cold]
    pub(crate) fn get_error(
        &self,
        _failure: ParseFailed,
    ) -> ParseError {
        self.recorded_error
            .expect("a failed grammar rule recorded its diagnostic")
    }

    fn build_error(
        &self,
        expected: &'static str,
    ) -> ParseError {
        let kind = if self.at_end() {
            ParseErrorKind::UnexpectedEnd
        } else {
            ParseErrorKind::UnexpectedToken
        };

        ParseError::new(kind, expected, self.get_span(0))
    }

    /// The bytes of the token `ahead` places on, or an empty range at the end of the source.
    pub(crate) fn get_span(
        &self,
        ahead: usize,
    ) -> SourceSpan {
        match self.get_token(ahead) {
            Some(token) => token.get_span(),
            None => SourceSpan::new(self.source_bytes.len(), self.source_bytes.len()),
        }
    }

    // ---- Building the tree ----

    pub(crate) fn open(&mut self) -> OpenNode {
        let open_node = OpenNode::new(self.events.len());

        self.events.push(TreeEvent::Open(NodeKind::Unfinished));

        open_node
    }

    pub(crate) fn close(
        &mut self,
        open_node: OpenNode,
        kind: NodeKind,
    ) -> ClosedNode {
        self.events[open_node.get_event_index()] = TreeEvent::Open(kind);

        self.events.push(TreeEvent::Close);

        ClosedNode::new(open_node.get_event_index())
    }

    /// Opens a node that starts in front of `closed_node`, so `closed_node` becomes its first child.
    pub(crate) fn open_before(
        &mut self,
        closed_node: ClosedNode,
    ) -> OpenNode {
        self.events
            .insert(closed_node.get_event_index(), TreeEvent::Open(NodeKind::Unfinished));

        OpenNode::new(closed_node.get_event_index())
    }

    pub(crate) fn has_kind(
        &self,
        node: ClosedNode,
        kind: NodeKind,
    ) -> bool {
        self.events[node.get_event_index()] == TreeEvent::Open(kind)
    }

    /// Renames a finished node. Used where a rule learns late that a node was something else.
    pub(crate) fn set_kind(
        &mut self,
        closed_node: ClosedNode,
        kind: NodeKind,
    ) {
        self.events[closed_node.get_event_index()] = TreeEvent::Open(kind);
    }

    /// True if `closed_node` is the most recent node and holds exactly one token and nothing else.
    pub(crate) fn is_single_token(
        &self,
        closed_node: ClosedNode,
    ) -> bool {
        self.events.len() == closed_node.get_event_index() + 3 && matches!(self.events.get(closed_node.get_event_index() + 1), Some(TreeEvent::Token))
    }

    // ---- :NestingLimit ----

    pub(crate) fn enter_nesting(&mut self) -> Parsed {
        self.nesting_depth += 1;

        if self.nesting_depth > NESTING_LIMIT {
            return Err(self.error_of_kind(ParseErrorKind::NestingTooDeep, "less nesting"));
        }

        Ok(())
    }

    pub(crate) fn leave_nesting(&mut self) {
        self.nesting_depth = self.nesting_depth.saturating_sub(1);
    }
}

#[cfg(test)]
mod tests {
    use super::SyntaxParser;
    use omega_lexer::lex;

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn token_indices_report_the_first_unrepresentable_position() {
        let last = u32::MAX as usize;
        let span = omega_lexer::SourceSpan::new(last + 1, last + 2);
        assert_eq!(SyntaxParser::compact_token_index(last, span), Ok(u32::MAX));
        let error = SyntaxParser::compact_token_index(last + 1, span).unwrap_err();
        assert_eq!(error.get_kind(), crate::ParseErrorKind::SourceTooLarge);
        assert_eq!(error.get_span(), span);
        assert!(error.to_string().contains("more tokens than one file may hold"));
    }

    #[test]
    fn consuming_at_end_preserves_the_token_stream_and_the_eof_span() {
        let text = b"> ";
        let tokens = lex(text).unwrap();
        let mut parser = SyntaxParser::new(text, &tokens).unwrap();
        assert!(!parser.nth_touches_next(0));
        parser.bump();
        let expected = parser.events.clone();
        parser.bump();
        assert_eq!(parser.events, expected);
        assert_eq!(parser.get_span(0), omega_lexer::SourceSpan::new(text.len(), text.len()));
    }

    #[test]
    fn index_capacity_is_bounded_on_small_and_trivia_heavy_inputs() {
        let dense = b"@".repeat(1024);
        let dense_tokens = lex(&dense).unwrap();
        let parser = SyntaxParser::new(&dense, &dense_tokens).unwrap();
        assert!(parser.meaningful_token_indices.iter().copied().eq(0..1024));
        assert_eq!(parser.meaningful_token_indices.capacity(), 1024);

        let sparse = b"/*x*/".repeat(1025);
        let sparse_tokens = lex(&sparse).unwrap();
        let parser = SyntaxParser::new(&sparse, &sparse_tokens).unwrap();
        assert!(parser.meaningful_token_indices.is_empty());
        assert_eq!(parser.meaningful_token_indices.capacity(), 513);
    }
}
