use omega_abstract_operations::ValueBinding;
use omega_optimization_core::AcceptedObligationFactIdentity;
use omega_optimization_unit::{FuelSettlement, ValueDefinitionSite};
use omega_register_model::{
    RegisterClassId, RegisterConstraintKey, RegisterOperandAccess, RegisterUnitId, RegisterViewId,
};
use omega_target_operations::TerminalPsiProvenance;
use psi_core::{
    BlockId, EdgeId, IntegerValue, MachineId, ObligationId, OperationId, ScalarType, ValueId,
};

use crate::{SelectedBlockId, SelectedInstructionId, VirtualRegisterId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedFunction {
    pub machine: MachineId,
    pub attachment: Option<psi_core::StructuralTypeId>,
    pub provenance: TerminalPsiProvenance,
    pub entry_block: SelectedBlockId,
    pub virtual_registers: Vec<VirtualRegister>,
    pub blocks: Vec<SelectedBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualRegister {
    pub id: VirtualRegisterId,
    pub scalar_type: ScalarType,
    pub class: RegisterClassId,
    pub origin: VirtualRegisterOrigin,
    pub definition_site: ValueDefinitionSite,
    /// An ABI live-in constraint. This is not an assigned physical home.
    pub entry_fixed_view: Option<RegisterViewId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualRegisterOrigin {
    EntryParameter {
        source_value: ValueId,
        parameter_index: usize,
    },
    InstructionResult {
        instruction: SelectedInstructionId,
        source_value: ValueId,
    },
    /// A value introduced by mandatory target legalization. `source_value`
    /// retains Psi lineage without claiming that the source value itself has
    /// the legalized register type.
    LegalizationTemporary {
        instruction: SelectedInstructionId,
        temporary: omega_legalized_operations::LegalizedTemporaryId,
        source_value: ValueId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedBlock {
    pub id: SelectedBlockId,
    pub source_block: BlockId,
    pub instructions: Vec<SelectedInstruction>,
    pub terminator: SelectedTerminator,
}

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
pub struct SelectedOperand {
    pub operand: u16,
    pub virtual_register: VirtualRegisterId,
    pub access: RegisterOperandAccess,
    pub class: RegisterClassId,
    /// A fixed instruction-use/def constraint, not an assigned home.
    pub fixed_view: Option<RegisterViewId>,
    /// Canonical one-way allocation tie to an earlier operand.
    pub tied_to: Option<u16>,
    /// This definition may clobber before unrelated inputs are all read.
    pub early_clobber: bool,
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
    ReturnI64,
    /// Value-less semantic return. This is deliberately distinct from
    /// `ReturnI64` even on targets where both select the same opcode.
    ReturnUnit,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SelectedInstructionProvenance {
    pub operations: Vec<OperationId>,
    pub values: Vec<ValueId>,
    pub edges: Vec<EdgeId>,
    pub obligations: Vec<ObligationId>,
    pub fuel: Vec<FuelSettlement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedTerminator {
    ConditionalBranch {
        instruction: SelectedInstruction,
        when_nonzero: SelectedSuccessor,
        when_zero: SelectedSuccessor,
    },
    /// Predicate-aware unsigned-U64 control. Keeping the semantic successors
    /// distinct prevents later persistence and allocation stages from having
    /// to infer ordering meaning from a generic nonzero/zero branch.
    ConditionalBranchU64LessThan {
        instruction: SelectedInstruction,
        when_less: SelectedSuccessor,
        when_not_less: SelectedSuccessor,
    },
    Return {
        instruction: SelectedInstruction,
        psi_return_edge: EdgeId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedSuccessor {
    pub psi_edge: EdgeId,
    pub block: SelectedBlockId,
    pub source_target: BlockId,
    pub bindings: Vec<ValueBinding>,
    /// Path-specific logical fuel for this exact semantic edge.
    pub fuel: Vec<FuelSettlement>,
}
