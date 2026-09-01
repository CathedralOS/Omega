use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_register_model::{
    PhysicalRegisterModelIdentity, RegisterClassId, RegisterUnitId, RegisterViewId,
    RegisterWriteSemantics,
};
use omega_selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use omega_target::NativeTarget;
use psi_core::MachineId;

use crate::PostAllocationMachineIdentity;

pub const X86_MOV_R64_IMM32_SIGN_EXTENDED_BASELINE_BYTE_COUNT: u8 = 10;
pub const X86_MOV_R64_IMM32_SIGN_EXTENDED_LOW_REGISTER_BYTE_COUNT: u8 = 7;
pub const X86_MOV_R64_IMM32_SIGN_EXTENDED_EXTENDED_REGISTER_BYTE_COUNT: u8 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct X86MovR64Imm32SignExtendedMaterializationIdentity([u8; 32]);

impl X86MovR64Imm32SignExtendedMaterializationIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct X86MovR64Imm32SignExtendedMaterializationRevisionIdentity([u8; 32]);

impl X86MovR64Imm32SignExtendedMaterializationRevisionIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum X86MovR64Imm32SignExtendedMaterializationPolicy {
    X86SelectMovR64Imm32SignExtendedI64MaterializationV1,
}

/// Exact physical destination overwritten by `MOV r64, imm32 sign-extended`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86MovR64Imm32SignExtendedPhysicalWrite {
    pub instruction: SelectedInstructionId,
    pub operand: u16,
    pub virtual_register: VirtualRegisterId,
    pub class: RegisterClassId,
    /// Retained selected-instruction destination, always a canonical r64 view.
    pub destination_view: RegisterViewId,
    pub destination_storage_units: Vec<RegisterUnitId>,
    pub destination_write_units: Vec<RegisterUnitId>,
    pub destination_write_semantics: RegisterWriteSemantics,
    /// Exact r64 view named by the replacement encoding.
    pub encoded_view: RegisterViewId,
    pub encoded_storage_units: Vec<RegisterUnitId>,
    pub encoded_write_units: Vec<RegisterUnitId>,
    pub encoded_write_semantics: RegisterWriteSemantics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum X86MovR64Imm32SignExtendedMaterializationAttemptOutcome {
    AlreadySelected,
    IntegerOutsideSignExtendedI32,
    SelectedForRewrite,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86MovR64Imm32SignExtendedMaterializationAttempt {
    pub iteration: u64,
    pub input: X86MovR64Imm32SignExtendedMaterializationRevisionIdentity,
    pub machine: MachineId,
    pub block: SelectedBlockId,
    pub instruction: SelectedInstructionId,
    pub literal_bits: u64,
    pub destination: X86MovR64Imm32SignExtendedPhysicalWrite,
    pub baseline_byte_count: u8,
    pub selected_byte_count: u8,
    pub outcome: X86MovR64Imm32SignExtendedMaterializationAttemptOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86MovR64Imm32SignExtendedMaterializationAction {
    pub iteration: u64,
    pub input: X86MovR64Imm32SignExtendedMaterializationRevisionIdentity,
    pub output: X86MovR64Imm32SignExtendedMaterializationRevisionIdentity,
    pub machine: MachineId,
    pub block: SelectedBlockId,
    pub instruction: SelectedInstructionId,
    pub literal_bits: u64,
    pub destination: X86MovR64Imm32SignExtendedPhysicalWrite,
    pub baseline_byte_count: u8,
    pub selected_byte_count: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum X86MovR64Imm32SignExtendedInstructionDisposition {
    RetainedV1,
    MovR64Imm32SignExtendedMaterializationV1 {
        literal_bits: u64,
        destination: X86MovR64Imm32SignExtendedPhysicalWrite,
        baseline_byte_count: u8,
        selected_byte_count: u8,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86MovR64Imm32SignExtendedMaterializationInstruction {
    pub instruction: SelectedInstructionId,
    pub disposition: X86MovR64Imm32SignExtendedInstructionDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86MovR64Imm32SignExtendedMaterializationBlock {
    pub block: SelectedBlockId,
    pub instructions: Vec<X86MovR64Imm32SignExtendedMaterializationInstruction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86MovR64Imm32SignExtendedMaterializationFunction {
    pub machine: MachineId,
    pub blocks: Vec<X86MovR64Imm32SignExtendedMaterializationBlock>,
}

/// Immutable symbolic encoding-choice artifact. The selected instruction,
/// physical home, and baseline sidecar remain unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86MovR64Imm32SignExtendedMaterializationPlan {
    pub identity: X86MovR64Imm32SignExtendedMaterializationIdentity,
    pub source: PostAllocationMachineIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub target: NativeTarget,
    pub physical_register_model: PhysicalRegisterModelIdentity,
    pub policy: X86MovR64Imm32SignExtendedMaterializationPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub output_revision: X86MovR64Imm32SignExtendedMaterializationRevisionIdentity,
    pub attempts: Vec<X86MovR64Imm32SignExtendedMaterializationAttempt>,
    pub actions: Vec<X86MovR64Imm32SignExtendedMaterializationAction>,
    pub functions: Vec<X86MovR64Imm32SignExtendedMaterializationFunction>,
}

impl X86MovR64Imm32SignExtendedMaterializationPlan {
    pub fn encode(&self) -> Vec<u8> {
        super::codec::encode(self)
    }

    /// Decode and authenticate an unchecked artifact. Independent validation
    /// against its retained roots is still mandatory.
    pub fn decode(
        encoded: &[u8],
    ) -> Result<Self, crate::X86MovR64Imm32SignExtendedMaterializationDecodeError> {
        super::codec::decode(encoded)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86MovR64Imm32SignExtendedMaterializationReceipt {
    identity: X86MovR64Imm32SignExtendedMaterializationIdentity,
    source: PostAllocationMachineIdentity,
    selected: SelectedInstructionPlanIdentity,
    action_count: usize,
    baseline_bytes: u64,
    selected_bytes: u64,
}

impl X86MovR64Imm32SignExtendedMaterializationReceipt {
    pub const fn identity(self) -> X86MovR64Imm32SignExtendedMaterializationIdentity {
        self.identity
    }
    pub const fn source(self) -> PostAllocationMachineIdentity {
        self.source
    }
    pub const fn selected(self) -> SelectedInstructionPlanIdentity {
        self.selected
    }
    pub const fn action_count(self) -> usize {
        self.action_count
    }
    pub const fn baseline_bytes(self) -> u64 {
        self.baseline_bytes
    }
    pub const fn selected_bytes(self) -> u64 {
        self.selected_bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedX86MovR64Imm32SignExtendedMaterialization {
    plan: X86MovR64Imm32SignExtendedMaterializationPlan,
    receipt: X86MovR64Imm32SignExtendedMaterializationReceipt,
}

impl ValidatedX86MovR64Imm32SignExtendedMaterialization {
    pub const fn plan(&self) -> &X86MovR64Imm32SignExtendedMaterializationPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> X86MovR64Imm32SignExtendedMaterializationReceipt {
        self.receipt
    }
    pub(crate) const fn new(
        plan: X86MovR64Imm32SignExtendedMaterializationPlan,
        receipt: X86MovR64Imm32SignExtendedMaterializationReceipt,
    ) -> Self {
        Self { plan, receipt }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86MovR64Imm32SignExtendedMaterializationWorkAxis {
    RuleEvaluations,
    Candidates,
    ValidationSteps,
    Commits,
    Iterations,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum X86MovR64Imm32SignExtendedMaterializationError {
    UnsupportedTarget(NativeTarget),
    RootMismatch,
    FunctionRosterMismatch(usize),
    BlockRosterMismatch { function: usize, block: usize },
    InstructionRosterMismatch(SelectedInstructionId),
    IntegerOutsideI64Bits(SelectedInstructionId),
    InvalidMaterializationFootprint(SelectedInstructionId),
    InvalidPhysicalDestination(SelectedInstructionId),
    BudgetExceeded(X86MovR64Imm32SignExtendedMaterializationWorkAxis),
    CountOverflow,
    ArtifactMismatch,
}

impl std::fmt::Display for X86MovR64Imm32SignExtendedMaterializationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "x86-64 MOV-r64-imm32-sign-extended i64 materialization selection failed: {self:?}"
        )
    }
}

impl std::error::Error for X86MovR64Imm32SignExtendedMaterializationError {}

pub(crate) fn x86_mov_r64_imm32_sign_extended_materialization_receipt(
    plan: &X86MovR64Imm32SignExtendedMaterializationPlan,
) -> Result<
    X86MovR64Imm32SignExtendedMaterializationReceipt,
    X86MovR64Imm32SignExtendedMaterializationError,
> {
    let (baseline_bytes, selected_bytes) =
        plan.actions
            .iter()
            .try_fold((0_u64, 0_u64), |(baseline, selected), action| {
                Ok::<_, X86MovR64Imm32SignExtendedMaterializationError>((
                    baseline
                        .checked_add(u64::from(action.baseline_byte_count))
                        .ok_or(X86MovR64Imm32SignExtendedMaterializationError::CountOverflow)?,
                    selected
                        .checked_add(u64::from(action.selected_byte_count))
                        .ok_or(X86MovR64Imm32SignExtendedMaterializationError::CountOverflow)?,
                ))
            })?;
    Ok(X86MovR64Imm32SignExtendedMaterializationReceipt {
        identity: plan.identity,
        source: plan.source,
        selected: plan.selected,
        action_count: plan.actions.len(),
        baseline_bytes,
        selected_bytes,
    })
}
