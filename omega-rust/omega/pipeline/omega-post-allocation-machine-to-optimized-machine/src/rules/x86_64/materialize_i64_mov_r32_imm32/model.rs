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

use omega_physical_instructions::PostAllocationMachineIdentity;

pub const X86_MOV_R32_IMM32_BASELINE_BYTE_COUNT: u8 = 10;
pub const X86_MOV_R32_IMM32_LOW_REGISTER_BYTE_COUNT: u8 = 5;
pub const X86_MOV_R32_IMM32_EXTENDED_REGISTER_BYTE_COUNT: u8 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct X86MovR32Imm32MaterializationIdentity([u8; 32]);

impl X86MovR32Imm32MaterializationIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct X86MovR32Imm32MaterializationRevisionIdentity([u8; 32]);

impl X86MovR32Imm32MaterializationRevisionIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum X86MovR32Imm32MaterializationPolicy {
    X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1,
}

/// Exact physical destination overwritten by `MOV r32, imm32`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86MovR32Imm32PhysicalWrite {
    pub instruction: SelectedInstructionId,
    pub operand: u16,
    pub virtual_register: VirtualRegisterId,
    pub class: RegisterClassId,
    /// Retained selected-instruction destination, always a canonical r64 view.
    pub destination_view: RegisterViewId,
    pub destination_storage_units: Vec<RegisterUnitId>,
    pub destination_write_units: Vec<RegisterUnitId>,
    pub destination_write_semantics: RegisterWriteSemantics,
    /// Exact r32 view named by the replacement encoding.
    pub encoded_view: RegisterViewId,
    pub encoded_storage_units: Vec<RegisterUnitId>,
    pub encoded_write_units: Vec<RegisterUnitId>,
    pub encoded_write_semantics: RegisterWriteSemantics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum X86MovR32Imm32MaterializationAttemptOutcome {
    AlreadySelected,
    IntegerOutsideZeroExtendedU32,
    SelectedForRewrite,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86MovR32Imm32MaterializationAttempt {
    pub iteration: u64,
    pub input: X86MovR32Imm32MaterializationRevisionIdentity,
    pub machine: MachineId,
    pub block: SelectedBlockId,
    pub instruction: SelectedInstructionId,
    pub literal_bits: u64,
    pub destination: X86MovR32Imm32PhysicalWrite,
    pub baseline_byte_count: u8,
    pub selected_byte_count: u8,
    pub outcome: X86MovR32Imm32MaterializationAttemptOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86MovR32Imm32MaterializationAction {
    pub iteration: u64,
    pub input: X86MovR32Imm32MaterializationRevisionIdentity,
    pub output: X86MovR32Imm32MaterializationRevisionIdentity,
    pub machine: MachineId,
    pub block: SelectedBlockId,
    pub instruction: SelectedInstructionId,
    pub literal_bits: u64,
    pub destination: X86MovR32Imm32PhysicalWrite,
    pub baseline_byte_count: u8,
    pub selected_byte_count: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum X86MovR32Imm32InstructionDisposition {
    RetainedV1,
    MovR32Imm32MaterializationV1 {
        literal_bits: u64,
        destination: X86MovR32Imm32PhysicalWrite,
        baseline_byte_count: u8,
        selected_byte_count: u8,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86MovR32Imm32MaterializationInstruction {
    pub instruction: SelectedInstructionId,
    pub disposition: X86MovR32Imm32InstructionDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86MovR32Imm32MaterializationBlock {
    pub block: SelectedBlockId,
    pub instructions: Vec<X86MovR32Imm32MaterializationInstruction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86MovR32Imm32MaterializationFunction {
    pub machine: MachineId,
    pub blocks: Vec<X86MovR32Imm32MaterializationBlock>,
}

/// Immutable symbolic encoding-choice artifact. The selected instruction,
/// physical home, and baseline sidecar remain unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86MovR32Imm32MaterializationPlan {
    pub identity: X86MovR32Imm32MaterializationIdentity,
    pub source: PostAllocationMachineIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub target: NativeTarget,
    pub physical_register_model: PhysicalRegisterModelIdentity,
    pub policy: X86MovR32Imm32MaterializationPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub output_revision: X86MovR32Imm32MaterializationRevisionIdentity,
    pub attempts: Vec<X86MovR32Imm32MaterializationAttempt>,
    pub actions: Vec<X86MovR32Imm32MaterializationAction>,
    pub functions: Vec<X86MovR32Imm32MaterializationFunction>,
}

impl X86MovR32Imm32MaterializationPlan {
    pub fn encode(&self) -> Vec<u8> {
        super::codec::encode(self)
    }

    /// Decode and authenticate an unchecked artifact. Independent validation
    /// against its retained roots is still mandatory.
    pub fn decode(encoded: &[u8]) -> Result<Self, crate::X86MovR32Imm32MaterializationDecodeError> {
        super::codec::decode(encoded)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86MovR32Imm32MaterializationReceipt {
    identity: X86MovR32Imm32MaterializationIdentity,
    source: PostAllocationMachineIdentity,
    selected: SelectedInstructionPlanIdentity,
    action_count: usize,
    baseline_bytes: u64,
    selected_bytes: u64,
}

impl X86MovR32Imm32MaterializationReceipt {
    pub const fn identity(self) -> X86MovR32Imm32MaterializationIdentity {
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
pub struct ValidatedX86MovR32Imm32Materialization {
    plan: X86MovR32Imm32MaterializationPlan,
    receipt: X86MovR32Imm32MaterializationReceipt,
}

impl ValidatedX86MovR32Imm32Materialization {
    pub const fn plan(&self) -> &X86MovR32Imm32MaterializationPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> X86MovR32Imm32MaterializationReceipt {
        self.receipt
    }
    pub(crate) const fn new(
        plan: X86MovR32Imm32MaterializationPlan,
        receipt: X86MovR32Imm32MaterializationReceipt,
    ) -> Self {
        Self { plan, receipt }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86MovR32Imm32MaterializationWorkAxis {
    RuleEvaluations,
    Candidates,
    ValidationSteps,
    Commits,
    Iterations,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum X86MovR32Imm32MaterializationError {
    UnsupportedTarget(NativeTarget),
    RootMismatch,
    FunctionRosterMismatch(usize),
    BlockRosterMismatch { function: usize, block: usize },
    InstructionRosterMismatch(SelectedInstructionId),
    IntegerOutsideI64Bits(SelectedInstructionId),
    InvalidMaterializationFootprint(SelectedInstructionId),
    InvalidPhysicalDestination(SelectedInstructionId),
    BudgetExceeded(X86MovR32Imm32MaterializationWorkAxis),
    CountOverflow,
    ArtifactMismatch,
}

impl std::fmt::Display for X86MovR32Imm32MaterializationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "x86-64 MOV-r32-imm32 i64 materialization selection failed: {self:?}"
        )
    }
}

impl std::error::Error for X86MovR32Imm32MaterializationError {}

pub(crate) fn x86_mov_r32_imm32_materialization_receipt(
    plan: &X86MovR32Imm32MaterializationPlan,
) -> Result<X86MovR32Imm32MaterializationReceipt, X86MovR32Imm32MaterializationError> {
    let (baseline_bytes, selected_bytes) =
        plan.actions
            .iter()
            .try_fold((0_u64, 0_u64), |(baseline, selected), action| {
                Ok::<_, X86MovR32Imm32MaterializationError>((
                    baseline
                        .checked_add(u64::from(action.baseline_byte_count))
                        .ok_or(X86MovR32Imm32MaterializationError::CountOverflow)?,
                    selected
                        .checked_add(u64::from(action.selected_byte_count))
                        .ok_or(X86MovR32Imm32MaterializationError::CountOverflow)?,
                ))
            })?;
    Ok(X86MovR32Imm32MaterializationReceipt {
        identity: plan.identity,
        source: plan.source,
        selected: plan.selected,
        action_count: plan.actions.len(),
        baseline_bytes,
        selected_bytes,
    })
}
