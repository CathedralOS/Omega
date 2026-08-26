#![forbid(unsafe_code)]

//! Production pre-register-allocation instruction CFG for the clean Terminal
//! lane.
//!
//! These are data shapes only. They record virtual values and target
//! constraints, but assign no physical homes and grant no liveness,
//! allocation, emission, or publication authority.

use omega_optimization_core::AcceptedObligationFactIdentity;
use omega_optimization_unit::{FuelSettlement, ValueDefinitionSite};
use omega_register_model::{
    RegisterClassId, RegisterConstraintKey, RegisterOperandAccess, RegisterUnitId, RegisterViewId,
};
use omega_target::NativeTarget;
use omega_terminal_abstract_operations::TerminalValueBinding;
use omega_terminal_target_operations::{MachineRegister, TerminalPsiProvenance};
use psi_core::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerValue, MachineId, ObligationId, OperationId,
    ScalarType, ValueId,
};
use psi_terminal::TerminalPsiIdentity;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalVirtualRegisterId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalSelectedBlockId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalSelectedInstructionId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalSelectedInstructionPlanIdentity([u8; 32]);

impl TerminalSelectedInstructionPlanIdentity {
    pub fn from_canonical_bytes(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Exact target-semantic constraint keys injected by ISA-aware orchestration.
/// Numeric variants are deliberately not inferred by target-neutral stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSelectedConstraintKeys {
    pub materialize_i64: RegisterConstraintKey,
    pub copy_i64: RegisterConstraintKey,
    pub add_i64: RegisterConstraintKey,
    pub compare_i64_zero: RegisterConstraintKey,
    pub conditional_branch: RegisterConstraintKey,
    pub return_i64: RegisterConstraintKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSelectedFixedInputConstraint {
    pub machine: MachineId,
    pub source_value: ValueId,
    pub parameter_index: usize,
    pub register: MachineRegister,
    pub fixed_view: RegisterViewId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalSelectedSelectionConstraints {
    pub keys: TerminalSelectedConstraintKeys,
    pub fixed_inputs: Vec<TerminalSelectedFixedInputConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalSelectedInstructionPlan {
    pub terminal_psi: TerminalPsiIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub target: NativeTarget,
    pub entry: MachineId,
    pub functions: Vec<TerminalSelectedFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalSelectedFunction {
    pub machine: MachineId,
    pub attachment: Option<psi_core::StructuralTypeId>,
    pub provenance: TerminalPsiProvenance,
    pub entry_block: TerminalSelectedBlockId,
    pub virtual_registers: Vec<TerminalVirtualRegister>,
    pub blocks: Vec<TerminalSelectedBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalVirtualRegister {
    pub id: TerminalVirtualRegisterId,
    pub scalar_type: ScalarType,
    pub class: RegisterClassId,
    pub origin: TerminalVirtualRegisterOrigin,
    pub definition_site: ValueDefinitionSite,
    /// An ABI live-in constraint. This is not an assigned physical home.
    pub entry_fixed_view: Option<RegisterViewId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalVirtualRegisterOrigin {
    EntryParameter {
        source_value: ValueId,
        parameter_index: usize,
    },
    InstructionResult {
        instruction: TerminalSelectedInstructionId,
        source_value: ValueId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalSelectedBlock {
    pub id: TerminalSelectedBlockId,
    pub source_block: BlockId,
    pub instructions: Vec<TerminalSelectedInstruction>,
    pub terminator: TerminalSelectedTerminator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalSelectedInstruction {
    pub id: TerminalSelectedInstructionId,
    pub kind: TerminalSelectedInstructionKind,
    pub constraint: RegisterConstraintKey,
    pub operands: Vec<TerminalSelectedOperand>,
    pub implicit_uses: Vec<RegisterUnitId>,
    pub implicit_defs: Vec<RegisterUnitId>,
    pub clobbers: Vec<RegisterUnitId>,
    pub provenance: TerminalSelectedInstructionProvenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSelectedOperand {
    pub operand: u16,
    pub virtual_register: TerminalVirtualRegisterId,
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
pub enum TerminalSelectedInstructionKind {
    CompareI64Zero,
    MaterializeI64 {
        value: IntegerValue,
    },
    CopyI64,
    /// Exact mathematical addition whose Psi proof obligation was discharged
    /// before target lowering. The obligation remains semantic custody even
    /// when the target uses the same physical row as wrapping addition.
    ExactAddI64 {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    ConditionalBranchNonZero,
    ReturnI64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TerminalSelectedInstructionProvenance {
    pub operations: Vec<OperationId>,
    pub values: Vec<ValueId>,
    pub edges: Vec<EdgeId>,
    pub obligations: Vec<ObligationId>,
    pub fuel: Vec<FuelSettlement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalSelectedTerminator {
    ConditionalBranch {
        instruction: TerminalSelectedInstruction,
        when_nonzero: TerminalSelectedSuccessor,
        when_zero: TerminalSelectedSuccessor,
    },
    Return {
        instruction: TerminalSelectedInstruction,
        psi_return_edge: EdgeId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalSelectedSuccessor {
    pub psi_edge: EdgeId,
    pub block: TerminalSelectedBlockId,
    pub source_target: BlockId,
    pub bindings: Vec<TerminalValueBinding>,
    /// Path-specific logical fuel for this exact semantic edge.
    pub fuel: Vec<FuelSettlement>,
}
