#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExpressionContext {
    Default,
    NoStructLiteral,
    NoStructLiteralOrMembership,
    ProofFact,
    GroupedProofFact,
    LogicalProofFact,
    LogicalValue,
    GroupedLogicalProofFact,
}

impl ExpressionContext {
    pub(crate) fn allows_struct_literal(self) -> bool {
        matches!(
            self,
            Self::Default
                | Self::GroupedProofFact
                | Self::GroupedLogicalProofFact
                | Self::LogicalValue
        )
    }

    pub(crate) fn allows_membership(self) -> bool {
        !matches!(
            self,
            Self::NoStructLiteralOrMembership | Self::ProofFact | Self::LogicalProofFact
        )
    }

    pub(crate) fn allows_type_expression(self) -> bool {
        matches!(
            self,
            Self::ProofFact
                | Self::GroupedProofFact
                | Self::LogicalProofFact
                | Self::GroupedLogicalProofFact
        )
    }

    pub(crate) fn allows_logical_subject(self) -> bool {
        matches!(
            self,
            Self::LogicalProofFact | Self::GroupedLogicalProofFact | Self::LogicalValue
        )
    }

    pub(crate) fn value(self) -> Self {
        if self.allows_logical_subject() {
            Self::LogicalValue
        } else {
            Self::Default
        }
    }

    pub(crate) fn grouped(self) -> Self {
        if self == Self::LogicalValue {
            Self::LogicalValue
        } else if self.allows_logical_subject() {
            Self::GroupedLogicalProofFact
        } else if self.allows_type_expression() {
            Self::GroupedProofFact
        } else {
            Self::Default
        }
    }
}
