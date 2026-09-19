#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExpressionContext {
    Default,
    NoStructLiteral,
    NoStructLiteralOrMembership,
    ProofFact,
    GroupedProofFact,
}

impl ExpressionContext {
    pub(crate) fn allows_struct_literal(self) -> bool {
        matches!(self, Self::Default | Self::GroupedProofFact)
    }

    pub(crate) fn allows_membership(self) -> bool {
        !matches!(self, Self::NoStructLiteralOrMembership | Self::ProofFact)
    }

    pub(crate) fn allows_type_expression(self) -> bool {
        matches!(self, Self::ProofFact | Self::GroupedProofFact)
    }

    pub(crate) fn grouped(self) -> Self {
        if self.allows_type_expression() {
            Self::GroupedProofFact
        } else {
            Self::Default
        }
    }
}
