use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::source::SourceId;
use omega_core::Span;
use omega_source_files_to_tokens::{PunctuationKind, Token, TokenKind};

pub type SyntaxNodeHandle = Handle<SyntaxNode>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SyntaxKind {
    #[default]
    Missing,
    SourceRoot,
    UseItem,
    TargetItem,
    TrustItem,
    CapabilityItem,
    InvariantItem,
    LibraryItem,
    EnumItem,
    DataItem,
    PlatformItem,
    MachineItem,
    MachineContains,
    MachineOwns,
    MachineInvariant,
    CallableEntry,
    CallableState,
    CallableFn,
    StatementLet,
    StatementTransition,
    StatementTransitionBlock,
    StatementOpaque,
    OpaqueBlock,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxToken {
    pub kind: TokenKind,
    pub lexeme: String,
    pub span: Span,
}

impl SyntaxToken {
    pub fn from_token(token: &Token<'_>) -> Self {
        Self {
            kind: token.kind,
            lexeme: token.lexeme.as_str().to_owned(),
            span: token.span,
        }
    }
}

impl Default for SyntaxToken {
    fn default() -> Self {
        Self {
            kind: TokenKind::Punctuation(PunctuationKind::Unknown),
            lexeme: String::new(),
            span: Span::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SyntaxNode {
    pub kind: SyntaxKind,
    pub tokens: HandleSpan<SyntaxToken>,
    pub children: HandleSpan<SyntaxNodeHandle>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyntaxTable {
    pub tokens: Arena<SyntaxToken>,
    pub nodes: Arena<SyntaxNode>,
    pub node_handles: Arena<SyntaxNodeHandle>,
}

impl SyntaxTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_tokens(&mut self, tokens: &[Token<'_>]) -> HandleSpan<SyntaxToken> {
        self.tokens
            .insert_many(tokens.iter().map(SyntaxToken::from_token))
    }

    pub fn insert_node(
        &mut self,
        kind: SyntaxKind,
        tokens: HandleSpan<SyntaxToken>,
        children: impl IntoIterator<Item = SyntaxNodeHandle>,
    ) -> SyntaxNodeHandle {
        let children = self.node_handles.insert_many(children);
        self.nodes.insert(SyntaxNode {
            kind,
            tokens,
            children,
        })
    }

    pub fn token_span(
        &self,
        source_tokens: HandleSpan<SyntaxToken>,
        start_index: usize,
        end_index: usize,
    ) -> HandleSpan<SyntaxToken> {
        if end_index <= start_index {
            return HandleSpan::empty();
        }

        let base = source_tokens.start();
        let start = Handle::from_parts(
            base.arena_index()
                .checked_add(u32::try_from(start_index).expect("token index overflow"))
                .expect("token handle overflow"),
            base.generation(),
        );
        let count = u32::try_from(end_index - start_index).expect("token span count overflow");
        HandleSpan::from_parts(start, count)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxTree {
    pub source_id: SourceId,
    pub root: SyntaxNodeHandle,
    pub source_tokens: HandleSpan<SyntaxToken>,
    pub syntax: SyntaxTable,
}
