#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExpressionContext {
    Default,
    NoStructLiteral,
    NoStructLiteralOrMembership,
}

impl ExpressionContext {
    pub(super) fn allows_struct_literal(self) -> bool {
        matches!(self, Self::Default)
    }

    pub(super) fn allows_membership(self) -> bool {
        !matches!(self, Self::NoStructLiteralOrMembership)
    }
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
