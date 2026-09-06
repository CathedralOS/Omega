//! Executable selected forms and their explicit operand/effect interfaces.
use super::{SelectedInstructionId, SelectedInstructionProvenance, SelectedOperand};
use optimization_core::AcceptedObligationFactIdentity;
use register_model::{RegisterConstraintKey, RegisterUnitId};
use semantic_vocabulary::{IntegerValue, MachineId, ObligationId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedInstruction {
    pub id: SelectedInstructionId,
    pub kind: SelectedInstructionKind,
    pub constraint: RegisterConstraintKey,
    pub operands: Vec<SelectedOperand>,
    pub implicit_uses: Vec<RegisterUnitId>,
    pub implicit_defs: Vec<RegisterUnitId>,
    pub clobbers: Vec<RegisterUnitId>,
    pub provenance: SelectedInstructionProvenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedInstructionKind {
    CompareI64Zero,
    /// Compare two i64 register bit-patterns and define the target condition
    /// state consumed by conditional control. This instruction has no scalar
    /// result; equality is represented by the zero condition.
    CompareI64,
    MaterializeI64 {
        value: IntegerValue,
    },
    CopyI64,
    /// Exact mathematical addition whose source proof obligation was
    /// discharged before target lowering. A validated legalization theorem
    /// may transport a narrower exact operation to this i64 form; the selected
    /// receipt retains both the legal-plan and legalization-validator roots.
    ExactAddI64 {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    /// Exact mathematical subtraction whose Psi proof obligation was
    /// discharged before target lowering. Target constraints retain any
    /// architectural flag writes needed by its physical realization.
    ExactSubtractI64 {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    /// Exact mathematical addition with the right source value encoded as an
    /// instruction immediate. Proof and source-value custody remain explicit.
    ExactAddI64Immediate {
        immediate: IntegerValue,
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    /// Exact mathematical subtraction with the right source value encoded as
    /// an instruction immediate. Operand order, proof, and source-value
    /// custody remain explicit.
    ExactSubtractI64Immediate {
        immediate: IntegerValue,
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    ConditionalBranchNonZero,
    /// Branch on the unsigned-U64 less-than predicate established by the
    /// immediately preceding comparison. The comparison remains a distinct
    /// selected instruction; this zero-operand terminator names the exact
    /// predicate consumed from target condition state.
    ConditionalBranchU64LessThan,
    /// Branch on signed-I64 strict less-than from the immediately preceding
    /// comparison.
    ConditionalBranchI64LessThan,
    Jump,
    ReturnI64,
    /// Value-less semantic return. This is deliberately distinct from
    /// `ReturnI64` even on targets where both select the same opcode.
    ReturnUnit,
    /// Direct internal call with two fixed U64 inputs and one fixed U64
    /// result. The target constraint row owns the exact ABI views and complete
    /// call clobbers; `callee` retains relocation/publication custody.
    CallI64 {
        callee: MachineId,
    },
}
