#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExpressionContext {
    Default,
    NoStructLiteral,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StateKind {
    Entry,
    State,
}

impl StateKind {
    pub(super) fn allows_implicit_entry_name(self) -> bool {
        matches!(self, Self::Entry)
    }
}
