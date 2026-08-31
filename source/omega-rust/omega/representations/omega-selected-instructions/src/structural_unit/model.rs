use omega_calling_conventions::CallPlan;
use omega_optimization_unit::{EffectLink, OwnershipEvent};
use omega_register_model::{RegisterConstraintKey, RegisterUnitId};
use omega_target_operations::{MachineRegister, TerminalPsiProvenance};
use psi_core::{BlockId, EdgeId, MachineId, OperationId, ServiceId};
use psi_terminal::{
    ClaimTransfer, EntryClaim, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralTypeDeclaration,
};

use crate::{
    SelectedBlockId, SelectedInstruction, SelectedInstructionId, SelectedInstructionProvenance,
};

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
