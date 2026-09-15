#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExpressionContext {
    Default,
    NoStructLiteral,
    NoStructLiteralOrMembership,
}

impl ExpressionContext {
    pub(crate) fn allows_struct_literal(self) -> bool {
        matches!(self, Self::Default)
    }

    pub(crate) fn allows_membership(self) -> bool {
        !matches!(self, Self::NoStructLiteralOrMembership)
    }
}
