//! The token cursor and the tree builder that the grammar functions drive.

use crate::event::Event;
use crate::node_kind::NodeKind;
use crate::parse_error::ParseError;
use crate::parse_error_kind::ParseErrorKind;
use crate::syntax_tree::SyntaxTree;
use omega_lexer::{Punctuation, SourceSpan, Token, TokenKind};

/// What every grammar function returns: the thing it parsed, or the first error.
pub(crate) type Parsed<T = ()> = Result<T, ParseError>;

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

/// A node that has been opened and not yet closed.
pub(crate) struct OpenNode {
    event_index: usize,
}

/// A node that is complete. Holding one lets a later rule wrap the node in a new parent.
#[derive(Clone, Copy)]
pub(crate) struct ClosedNode {
    event_index: usize,
}

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
pub(crate) struct Parser<'source> {
    source_bytes: &'source [u8],
    tokens: &'source [Token],
    meaningful_token_indices: Vec<u32>, // Positions in `tokens` of everything that is not whitespace or a comment.
    position: usize,                    // Index into `meaningful_token_indices` of the next token to consume.
    events: Vec<Event>,
    nesting_depth: u32,
}

impl<'source> Parser<'source> {
    pub(crate) fn new(
        source_bytes: &'source [u8],
        tokens: &'source [Token],
    ) -> Parsed<Self> {
        let mut meaningful_token_indices = Vec::with_capacity(tokens.len() / 2 + 1);

        for (token_index, token) in tokens.iter().enumerate() {
            if token.get_kind().is_trivia() {
                continue;
            }

            let Ok(narrow_token_index) = u32::try_from(token_index) else {
                return Err(ParseError::new(ParseErrorKind::SourceTooLarge, "fewer tokens", token.get_span()));
            };

            meaningful_token_indices.push(narrow_token_index);
        }

        // A tree holds a little under three events per meaningful token on the fixtures.
        let event_capacity = meaningful_token_indices.len() * 3 + 16;

        Ok(Self {
            source_bytes,
            tokens,
            meaningful_token_indices,
            position: 0,
            events: Vec::with_capacity(event_capacity),
            nesting_depth: 0,
        })
    }

    /// Hands over the finished tree. Every token must have been consumed.
    pub(crate) fn finish(
        self,
        expected: &'static str,
    ) -> Parsed<SyntaxTree> {
        if !self.at_end() {
            return Err(self.error(expected));
        }

        Ok(SyntaxTree::new(self.events, self.meaningful_token_indices))
    }

    // ---- Looking at tokens ----

    fn get_token(
        &self,
        ahead: usize,
    ) -> Option<Token> {
        let token_index = *self.meaningful_token_indices.get(self.position + ahead)?;

        self.tokens.get(token_index as usize).copied()
    }

    pub(crate) fn at_end(&self) -> bool {
        self.position >= self.meaningful_token_indices.len()
    }

    /// True if there is a token `ahead` places on.
    pub(crate) fn nth_exists(
        &self,
        ahead: usize,
    ) -> bool {
        self.position + ahead < self.meaningful_token_indices.len()
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

        self.events.push(Event::Token);
        self.position += 1;
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

    /// An error at the next token: unexpected token, or unexpected end if there is none.
    pub(crate) fn error(
        &self,
        expected: &'static str,
    ) -> ParseError {
        let kind = if self.at_end() {
            ParseErrorKind::UnexpectedEnd
        } else {
            ParseErrorKind::UnexpectedToken
        };

        self.error_of_kind(kind, expected)
    }

    pub(crate) fn error_of_kind(
        &self,
        kind: ParseErrorKind,
        expected: &'static str,
    ) -> ParseError {
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
        let open_node = OpenNode {
            event_index: self.events.len(),
        };

        self.events.push(Event::Open(NodeKind::Unfinished));

        open_node
    }

    pub(crate) fn close(
        &mut self,
        open_node: OpenNode,
        kind: NodeKind,
    ) -> ClosedNode {
        if let Some(open_event) = self.events.get_mut(open_node.event_index) {
            *open_event = Event::Open(kind);
        }

        self.events.push(Event::Close);

        ClosedNode {
            event_index: open_node.event_index,
        }
    }

    /// Opens a node that starts in front of `closed_node`, so `closed_node` becomes its first child.
    pub(crate) fn open_before(
        &mut self,
        closed_node: ClosedNode,
    ) -> OpenNode {
        self.events
            .insert(closed_node.event_index, Event::Open(NodeKind::Unfinished));

        OpenNode {
            event_index: closed_node.event_index,
        }
    }

    pub(crate) fn get_kind(
        &self,
        closed_node: ClosedNode,
    ) -> Option<NodeKind> {
        match self.events.get(closed_node.event_index) {
            Some(Event::Open(kind)) => Some(*kind),
            _ => None,
        }
    }

    /// Renames a finished node. Used where a rule learns late that a node was something else.
    pub(crate) fn set_kind(
        &mut self,
        closed_node: ClosedNode,
        kind: NodeKind,
    ) {
        if let Some(open_event) = self.events.get_mut(closed_node.event_index) {
            *open_event = Event::Open(kind);
        }
    }

    /// True if `closed_node` is the most recent node and holds exactly one token and nothing else.
    pub(crate) fn is_single_token(
        &self,
        closed_node: ClosedNode,
    ) -> bool {
        self.events.len() == closed_node.event_index + 3 && matches!(self.events.get(closed_node.event_index + 1), Some(Event::Token))
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
