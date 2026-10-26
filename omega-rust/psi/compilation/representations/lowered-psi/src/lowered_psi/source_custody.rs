//! Ephemeral source joins retained alongside, never inside, portable Terminal artifacts.

pub use semantic_vocabulary::BoundaryInvocationSite as LoweredSourceCallSite;
use semantic_vocabulary::{BlockId, MachineId, OperationId};
use terminal_psi::ValueDeclaration;

impl LoweredSourceCallOccurrence {
    /// Resolve nested expression provenance to this call's statement owner.
    pub fn matches_checked_origin(
        &self,
        checked: &checked_trees::CheckedTrees,
        origin: checked_trees::CheckedValueOrigin,
    ) -> bool {
        let Some(origin) = checked.consistent_terminal_origin(origin) else {
            return false;
        };
        matches!(origin, checked_trees::CheckedValueOrigin::StateStatement {
        state_symbol, statement_index, ..
    } if state_symbol == self.source_state && statement_index == self.statement_index)
    }
}

/// One exact selected source comparison joined to the Terminal operation it
/// emitted, carrying the two facts no other record holds: which emitted
/// operation answers for the checked `operator_use`, and where that operation
/// reads the authored scalar operands.
///
/// IEEE and integer comparisons share this row because every consumer reads
/// the same fields: boundary applications join the selected requirement
/// application through `application_site`/`requirement_operator` and the
/// emitted operation through `terminal_operation`, and operation crash
/// contracts join the checked crash site through `operator_use`. The emitted
/// operation itself carries the comparison's meaning — `IeeeFloatCompare`'s
/// comparison, the integer operation's own variant, and for `!=` the
/// `BooleanNot` over its result — and the checked operator use keeps the
/// source coordinate; the row restates neither. Provider authority is
/// independently rejoined by Omega before execution is admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoweredSelectedComparisonOccurrence {
    pub operator_use: checked_trees::CheckedOperatorUseHandle,
    pub application_site: checked_trees::CheckedBoundaryOperatorApplicationUseSite,
    pub requirement_operator: symbols::SymbolHandle,
    pub terminal_machine: MachineId,
    pub terminal_operation: OperationId,
    /// Where the emitted operation reads each authored scalar operand. IEEE
    /// comparisons emit the authored comparison itself, so they always record
    /// `Authored`; `>` and `>=` emit the reversed
    /// `IntegerLessThan`/`IntegerLessOrEqual`.
    pub operand_order: LoweredSelectedComparisonOperandOrder,
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
pub enum LoweredSelectedComparisonOperandOrder {
    /// Terminal operand position `k` is authored scalar operand ordinal `k`.
    Authored,
    /// Terminal operand position `k` is authored scalar operand ordinal
    /// `1 - k`: the emitted operation reverses the authored pair.
    Swapped,
}

impl LoweredSelectedComparisonOperandOrder {
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

/// One scalar value in the checked source environment at a call. Source
/// positions include structural formals and locals; the scalar lane is sparse
/// and must not renumber those positions when it omits another storage kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoweredSourceScalarBinding {
    pub source_position: usize,
    pub value: ValueDeclaration,
}

/// Existing structural storage named by one checked source position. The path
/// retains projected aliases; joining a local never creates another owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredSourceStructuralBinding {
    pub source_position: usize,
    pub argument: terminal_psi::StructuralArgument,
    pub structural_type: semantic_vocabulary::StructuralTypeId,
}

/// The reference emitter's capture-time address and result, joined to the
/// exact checked occurrence. Source handles never enter portable Terminal.
/// The source operand is interpreted at `formation`, not read again at a
/// later crossing; descriptor replacement cannot retarget this capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredSourceLoanCapture {
    pub loan: arena::Handle<checked_trees::BorrowLoanFact>,
    pub formation: OperationId,
    pub carrier: semantic_vocabulary::PlaceId,
    pub source: terminal_psi::StructuralArgument,
}

/// The call emitter's source-to-Terminal namespaces at a checked crossing.
/// Claims are mappings of established identities, not new establishments.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoweredSourceFrontierBindings {
    /// Formation-time observations retained for this invocation's loans and
    /// their parent/supplier closure. These establish no parking authority.
    pub loan_captures: Vec<LoweredSourceLoanCapture>,
    pub scalars: Vec<LoweredSourceScalarBinding>,
    pub structural: Vec<LoweredSourceStructuralBinding>,
    pub claims: Vec<(
        language_semantics::PermissionClaimIdentity,
        semantic_vocabulary::ClaimId,
    )>,
}

impl LoweredSourceFrontierBindings {
    /// Every scalar dependency of the emitted source join, including captured
    /// runtime selectors in projected structural aliases.
    pub fn visit_value_ids(&self, mut visit: impl FnMut(semantic_vocabulary::ValueId)) {
        for binding in &self.scalars {
            visit(binding.value.id);
        }
        for binding in &self.structural {
            for segment in &binding.argument.path {
                if let terminal_psi::StructuralPathSegment::RuntimeIndex { index, .. } = segment {
                    visit(*index);
                }
            }
        }
        for capture in &self.loan_captures {
            for segment in &capture.source.path {
                if let terminal_psi::StructuralPathSegment::RuntimeIndex { index, .. } = segment {
                    visit(*index);
                }
            }
        }
    }
}

/// One exact checked source call joined to its emitted Terminal control site.
/// Source handles are deliberately confined to the producer result and never
/// enter the canonical Terminal artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredSourceCallOccurrence {
    pub source_site: Option<checked_trees::NominalMachineUseSite>,
    pub source_state: symbols::SymbolHandle,
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub terminal_site: LoweredSourceCallSite,
    /// Ordinary direct-module calls retain the selected checked state for this
    /// coordinate. Authored/public identities remain in the checked flow row.
    /// Boundary and dynamic calls retain their own selected declaration identity.
    pub source_target: symbols::SymbolHandle,
    /// Exact storage bindings and claim identities immediately before this call.
    /// A missing source position has no storage join, never a shifted or fabricated value.
    pub frontier_before_call: LoweredSourceFrontierBindings,
}

/// The header block one emitted state graph state took. Like a call
/// occurrence it is an ephemeral source-to-Terminal join: consumers read it
/// while both representations are available, instead of inferring the block
/// from the state's position in some roster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoweredStateBlock {
    pub machine: MachineId,
    pub state: symbols::SymbolHandle,
    pub block: BlockId,
}
