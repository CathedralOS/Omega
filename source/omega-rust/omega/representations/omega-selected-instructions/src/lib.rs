#![forbid(unsafe_code)]

//! Production pre-register-allocation instruction CFG for the clean Terminal
//! lane.
//!
//! These are data shapes only. They record virtual values and target
//! constraints, but assign no physical homes and grant no liveness,
//! allocation, emission, or publication authority.

use omega_abstract_operations::ValueBinding;
use omega_calling_conventions::CallPlan;
use omega_optimization_core::AcceptedObligationFactIdentity;
use omega_optimization_unit::{EffectLink, FuelSettlement, OwnershipEvent, ValueDefinitionSite};
use omega_register_model::{
    RegisterClassId, RegisterConstraintKey, RegisterOperandAccess, RegisterUnitId, RegisterViewId,
};
use omega_target::NativeTarget;
use omega_target_operations::{MachineRegister, TerminalPsiProvenance};
use psi_core::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerValue, MachineId, ObligationId, OperationId,
    ScalarType, ServiceId, ValueId,
};
use psi_terminal::{
    ClaimTransfer, EntryClaim, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, TerminalPsiIdentity,
};
use sha2::{Digest, Sha256};

mod machine_effect_identities;
mod machine_effects;

pub use machine_effect_identities::machine_effect_catalog_identity;
pub use machine_effects::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VirtualRegisterId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SelectedBlockId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SelectedInstructionId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SelectedInstructionPlanIdentity([u8; 32]);

impl SelectedInstructionPlanIdentity {
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
pub struct SelectedConstraintKeys {
    /// Target-applicable structural Unit call. Absence is an explicit refusal
    /// to select the bounded structural-call roster on this target.
    pub structural_unit_call: Option<RegisterConstraintKey>,
    pub materialize_i64: RegisterConstraintKey,
    pub copy_i64: RegisterConstraintKey,
    pub add_i64: RegisterConstraintKey,
    pub subtract_i64: RegisterConstraintKey,
    pub add_i64_immediate: RegisterConstraintKey,
    pub subtract_i64_immediate: RegisterConstraintKey,
    pub compare_i64_zero: RegisterConstraintKey,
    pub conditional_branch: RegisterConstraintKey,
    pub return_i64: RegisterConstraintKey,
    pub return_unit: RegisterConstraintKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedFixedInputConstraint {
    pub machine: MachineId,
    pub source_value: ValueId,
    pub parameter_index: usize,
    pub register: MachineRegister,
    pub fixed_view: RegisterViewId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedSelectionConstraints {
    pub keys: SelectedConstraintKeys,
    pub fixed_inputs: Vec<SelectedFixedInputConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedInstructionPlan {
    pub psi: TerminalPsiIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub target: NativeTarget,
    pub entry: MachineId,
    pub functions: Vec<SelectedFunction>,
    /// Structural-ABI Unit functions are deliberately kept out of the scalar
    /// VReg roster. Their selected call bundle has no allocator-managed value
    /// and cannot acquire a fabricated scalar operand merely to enter the
    /// ordinary instruction vocabulary.
    pub structural_unit_functions: Vec<SelectedStructuralUnitFunction>,
}

/// Exact bounded selected form for a structural-signature Unit function.
///
/// This record owns semantic/ABI selection custody plus the injected
/// zero-operand register constraint. It does not grant a machine-effect
/// declaration, encoding, symbol, relocation, or object span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedStructuralUnitFunction {
    pub machine: MachineId,
    pub attachment: Option<psi_core::StructuralTypeId>,
    pub provenance: TerminalPsiProvenance,
    pub structural_types: Vec<StructuralTypeDeclaration>,
    pub abi: SelectedStructuralUnitAbi,
    pub structural_places: Vec<StructuralPlaceDeclaration>,
    pub entry_claims: Vec<EntryClaim>,
    pub published_service_ceiling: Vec<ServiceId>,
    pub entry_block: SelectedBlockId,
    pub source_entry_block: BlockId,
    /// Ordered claim-completion settlements. Each row is metadata and owns no
    /// selected instruction identifier.
    pub boundary_settlements: Vec<SelectedBoundarySettlement>,
    pub call: Option<SelectedStructuralUnitCallInstruction>,
    pub terminator: SelectedStructuralUnitReturn,
}

pub type SelectedBoundarySettlement = omega_legalized_operations::LegalizedBoundarySettlement;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedStructuralUnitAbiRecipe {
    /// Two owned 16-byte indirect parameters arrive in RCX/RDX. A call copies
    /// them into the Microsoft shadow-relative caller area before transferring
    /// them to a Unit callee.
    MicrosoftX64OwnedIndirectPairV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedStructuralUnitAbi {
    pub recipe: SelectedStructuralUnitAbiRecipe,
    pub call_plan: CallPlan,
    pub parameters: Vec<SelectedStructuralUnitParameter>,
    pub layout: SelectedMicrosoftX64OwnedIndirectPairLayout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedStructuralUnitParameter {
    pub semantic: StructuralParameterDeclaration,
    pub target: omega_target_operations::TargetStructuralParameter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedStructuralUnitIndirectBinding {
    pub parameter_index: usize,
    pub pointer: MachineRegister,
    pub copy_stack_byte_offset: u32,
    pub byte_count: u16,
    pub alignment: u16,
}

/// Address-free ABI geometry selected for the exact call bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedMicrosoftX64OwnedIndirectPairLayout {
    pub shadow_byte_count: u32,
    pub outgoing_frame_byte_count: u32,
    pub pre_call_stack_alignment: u16,
    pub bindings: [SelectedStructuralUnitIndirectBinding; 2],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedStructuralUnitCallArgument {
    pub semantic: psi_terminal::StructuralArgument,
    pub target: omega_target_operations::TargetStructuralArgument,
}

/// Selected calls retain the exact legalized semantic origin without
/// introducing a second source vocabulary that could drift during replay.
pub use omega_legalized_operations::LegalizedCallUnitSource as SelectedStructuralUnitCallSource;

/// One atomic semantic/ABI call bundle. The later target-owned machine layer
/// must refine this row rather than decomposing it into untracked loads,
/// stores, stack adjustments, and a call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedStructuralUnitCallInstruction {
    pub id: SelectedInstructionId,
    pub source: SelectedStructuralUnitCallSource,
    pub operation: OperationId,
    pub callee: MachineId,
    pub caller_call_plan: CallPlan,
    pub callee_call_plan: CallPlan,
    pub arguments: Vec<SelectedStructuralUnitCallArgument>,
    pub claim_transfers: Vec<ClaimTransfer>,
    pub layout: SelectedMicrosoftX64OwnedIndirectPairLayout,
    /// One target-injected, zero-explicit-operand call constraint. Its
    /// implicit state is copied into this atomic row so later traversal cannot
    /// silently decompose or weaken the call boundary.
    pub constraint: RegisterConstraintKey,
    pub implicit_uses: Vec<RegisterUnitId>,
    pub implicit_defs: Vec<RegisterUnitId>,
    pub clobbers: Vec<RegisterUnitId>,
    pub provenance: SelectedInstructionProvenance,
    pub effect: EffectLink,
    pub ownership: Vec<OwnershipEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedStructuralUnitReturn {
    /// The ordinary, already target-constrained `ReturnUnit` instruction.
    pub instruction: SelectedInstruction,
    pub psi_return_edge: EdgeId,
    pub effect: EffectLink,
    pub ownership: Vec<OwnershipEvent>,
}

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
