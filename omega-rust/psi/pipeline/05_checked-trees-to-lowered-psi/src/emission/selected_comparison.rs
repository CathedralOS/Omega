/// Checked occurrence before emission assigns real Terminal identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SelectedComparison {
    pub operator_use: checked_trees::CheckedOperatorUseHandle,
    pub application_site: checked_trees::CheckedBoundaryOperatorApplicationUseSite,
    pub requirement_operator: symbols::SymbolHandle,
    pub provider_plan_report_fingerprint: u64,
    pub provider_plan_commitment: checked_trees::CheckedProviderPlanCommitment,
    pub meaning: SelectedComparisonMeaning,
}

/// The exact selected Terminal meaning of one comparison use. Each meaning
/// emits one comparison operation and records the matching occurrence row
/// beside it. IEEE comparisons carry their operation identity and therefore
/// always keep the authored operand order; integer comparisons additionally
/// name where that operation reads the authored operands and whether the
/// authored meaning negates it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelectedComparisonMeaning {
    IeeeFloat {
        comparison: semantic_vocabulary::IeeeFloatComparisonOperation,
        format: semantic_vocabulary::IeeeFloatFormat,
    },
    Integer {
        comparison: lowered_psi::LoweredSelectedIntegerComparisonOperation,
        operand_order: lowered_psi::LoweredSelectedIntegerComparisonOperandOrder,
        negated: bool,
        integer_type: semantic_vocabulary::IntegerType,
    },
}

impl SelectedComparison {
    /// The scalar type both operands must carry.
    pub(crate) const fn operand_type(&self) -> semantic_vocabulary::ScalarType {
        match self.meaning {
            SelectedComparisonMeaning::IeeeFloat { format, .. } => {
                semantic_vocabulary::ScalarType::IeeeFloat(format)
            }
            SelectedComparisonMeaning::Integer { integer_type, .. } => {
                semantic_vocabulary::ScalarType::Integer(integer_type)
            }
        }
    }
}
