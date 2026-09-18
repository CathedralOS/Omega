//! Ephemeral source joins retained alongside, never inside, portable Terminal artifacts.

use super::LoweredPsi;
use semantic_vocabulary::{BlockId, MachineId, OperationId};
use terminal_psi::ValueDeclaration;

/// Exact selected source comparison joined to one emitted operation. Provider
/// authority is independently rejoined by Omega before execution is admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoweredSelectedIeeeFloatComparisonOccurrence {
    pub operator_use: checked_trees::CheckedOperatorUseHandle,
    pub application_site: checked_trees::CheckedBoundaryOperatorApplicationUseSite,
    pub requirement_operator: symbols::SymbolHandle,
    pub provider_plan_report_fingerprint: u64,
    pub provider_plan_commitment: checked_trees::CheckedProviderPlanCommitment,
    pub terminal_machine: MachineId,
    pub terminal_operation: OperationId,
    pub comparison: semantic_vocabulary::IeeeFloatComparisonOperation,
    pub format: semantic_vocabulary::IeeeFloatFormat,
}

/// The Terminal integer comparison operations a selected comparison emits.
/// The row always names the operation actually written; how that operation
/// reads the authored operands, and whether the authored meaning negates it,
/// are the separate exact facts below.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoweredSelectedIntegerComparisonOperation {
    Equal,
    LessThan,
    LessOrEqual,
}

/// Where the emitted Terminal operation reads each authored scalar operand.
///
/// Operation crash contracts publish their routes in the emitted operation's
/// positional formal telescope, because that is the telescope the verifier
/// reconstructs: formal `k + 1` is the operand at position `k`. The operator
/// declaration indexes the same routes by authored parameter ordinal, so a
/// swapped emission must reindex them before publication or the verifier's
/// positional substitution would read the wrong operand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoweredSelectedIntegerComparisonOperandOrder {
    /// Terminal operand position `k` is authored scalar operand ordinal `k`.
    Authored,
    /// Terminal operand position `k` is authored scalar operand ordinal
    /// `1 - k`: the emitted operation reverses the authored pair.
    Swapped,
}

impl LoweredSelectedIntegerComparisonOperandOrder {
    /// The Terminal operand position holding one authored scalar operand
    /// ordinal, within an emitted operand roster of `operand_count`. A roster
    /// this mapping cannot address exactly has no position, so callers fail
    /// closed instead of guessing one.
    pub const fn terminal_operand_position(
        self,
        authored_ordinal: usize,
        operand_count: usize,
    ) -> Option<usize> {
        match self {
            Self::Authored => {
                if authored_ordinal < operand_count {
                    Some(authored_ordinal)
                } else {
                    None
                }
            }
            Self::Swapped => {
                if operand_count == 2 && authored_ordinal < 2 {
                    Some(1 - authored_ordinal)
                } else {
                    None
                }
            }
        }
    }
}

impl LoweredSelectedIntegerComparisonOperation {
    /// The one admitted emission for an authored integer comparison spelling:
    /// the Terminal operation written, where it reads the authored operands,
    /// and whether the authored meaning is that operation's negation.
    ///
    /// This roster is exhaustive and is the only producer of these triples, so
    /// no other combination can be recorded: `==`, `<` and `<=` emit their own
    /// operation over the authored order; `>` and `>=` emit the swapped
    /// `IntegerLessThan`/`IntegerLessOrEqual`; `!=` emits `IntegerEqual` over
    /// the authored order and negates its result. It mirrors the checked
    /// stage's own comparison normalization, so a selected operator and a
    /// builtin comparison denote the same operation. Any other binary operator
    /// has no admitted emission and fails closed.
    pub const fn admitted_emission(
        operator: checked_trees::expression::BinaryOperator,
    ) -> Option<(Self, LoweredSelectedIntegerComparisonOperandOrder, bool)> {
        use LoweredSelectedIntegerComparisonOperandOrder::{Authored, Swapped};
        use checked_trees::expression::BinaryOperator;
        match operator {
            BinaryOperator::Equal => Some((Self::Equal, Authored, false)),
            BinaryOperator::NotEqual => Some((Self::Equal, Authored, true)),
            BinaryOperator::Less => Some((Self::LessThan, Authored, false)),
            BinaryOperator::LessOrEqual => Some((Self::LessOrEqual, Authored, false)),
            BinaryOperator::Greater => Some((Self::LessThan, Swapped, false)),
            BinaryOperator::GreaterOrEqual => Some((Self::LessOrEqual, Swapped, false)),
            _ => None,
        }
    }
}

/// Exact selected integer comparison joined to one emitted operation: the
/// integer counterpart of [`LoweredSelectedIeeeFloatComparisonOccurrence`].
/// Operation crash contracts join a checked operator crash site to its
/// emitted operation only through this row's `operator_use`; provider
/// authority is independently rejoined by Omega before execution is admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoweredSelectedIntegerComparisonOccurrence {
    pub operator_use: checked_trees::CheckedOperatorUseHandle,
    pub application_site: checked_trees::CheckedBoundaryOperatorApplicationUseSite,
    pub requirement_operator: symbols::SymbolHandle,
    pub provider_plan_report_fingerprint: u64,
    pub provider_plan_commitment: checked_trees::CheckedProviderPlanCommitment,
    pub terminal_machine: MachineId,
    pub terminal_operation: OperationId,
    pub comparison: LoweredSelectedIntegerComparisonOperation,
    /// Where the emitted operation reads each authored scalar operand, so the
    /// operation crash contract can reindex the operator declaration's
    /// authored formal telescope into the operation's positional one.
    pub operand_order: LoweredSelectedIntegerComparisonOperandOrder,
    /// The authored comparison is the emitted operation's negation, completed
    /// by one `BooleanNot` over its result. The crash contract stays on the
    /// emitted comparison, which is the operation owning the scalar operands.
    pub negated: bool,
    pub integer_type: semantic_vocabulary::IntegerType,
}

/// Exact checked-to-Terminal join for the first bounded callback body cohort.
/// Source handles remain target-owned sidecar evidence; they are never encoded
/// into the canonical Terminal artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallbackTerminalLoweringReceipt {
    pub source_machine: symbols::SymbolHandle,
    pub source_entry: symbols::SymbolHandle,
    pub terminal_machine: MachineId,
    pub terminal_entry: BlockId,
}

/// Isolated callback body and the exact checked coordinate that produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredCallbackPsi {
    pub terminal: LoweredPsi,
    pub receipt: CallbackTerminalLoweringReceipt,
}

/// One exact checked source call joined to its emitted Terminal operation.
/// Source handles are deliberately confined to the producer result and never
/// enter the canonical Terminal artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredSourceCallOccurrence {
    pub source_site: Option<checked_trees::NominalMachineUseSite>,
    pub source_state: symbols::SymbolHandle,
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub terminal_operation: OperationId,
    pub source_target: symbols::SymbolHandle,
    /// Scalar environment immediately before this call, ordered as checked
    /// state parameters followed by established local bindings. Empty means
    /// this lowering route does not expose an exact scalar frontier mapping.
    pub source_values_before_call: Vec<ValueDeclaration>,
}

/// One exact selected checked IEEE FMA use joined to its emitted Terminal
/// operation. This sidecar is target-neutral custody, not hardware admission:
/// native realization must independently rejoin its plan evidence to an
/// admitted target provider before selecting an instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoweredSelectedIeeeFloatFmaOccurrence {
    pub source_state: symbols::SymbolHandle,
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub terminal_operation: OperationId,
    pub requirement_operator: symbols::SymbolHandle,
    pub provider_plan_report_fingerprint: u64,
    pub provider_plan_commitment: checked_trees::CheckedProviderPlanCommitment,
    pub format: semantic_vocabulary::IeeeFloatFormat,
}
