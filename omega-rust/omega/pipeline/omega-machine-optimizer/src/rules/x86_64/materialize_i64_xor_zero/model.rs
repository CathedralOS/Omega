use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_register_model::{
    PhysicalRegisterModelIdentity, RegisterClassId, RegisterUnitId, RegisterViewId,
    RegisterWriteSemantics,
};
use omega_selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use omega_selected_instructions_to_register_homes::LivenessIdentity;
use omega_target::NativeTarget;
use psi_core::MachineId;

use crate::PostAllocationMachineIdentity;

pub const X86_MOVABS_I64_BYTE_COUNT: u8 = 10;
pub const X86_XOR_R64_SELF_BYTE_COUNT: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct X86XorZeroMaterializationIdentity([u8; 32]);

impl X86XorZeroMaterializationIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct X86XorZeroMaterializationRevisionIdentity([u8; 32]);

impl X86XorZeroMaterializationRevisionIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum X86XorZeroMaterializationPolicy {
    X86SelectXorZeroI64MaterializationV1,
}

/// Exact physical destination overwritten by `XOR r64, r64`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86XorZeroPhysicalWrite {
    pub instruction: SelectedInstructionId,
    pub operand: u16,
    pub virtual_register: VirtualRegisterId,
    pub class: RegisterClassId,
    pub view: RegisterViewId,
    pub storage_units: Vec<RegisterUnitId>,
    pub write_units: Vec<RegisterUnitId>,
    pub write_semantics: RegisterWriteSemantics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum X86XorZeroMaterializationAttemptOutcome {
    AlreadySelected,
    NonZeroLiteral,
    RflagsLiveOut,
    SelectedForRewrite,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86XorZeroMaterializationAttempt {
    pub iteration: u64,
    pub input: X86XorZeroMaterializationRevisionIdentity,
    pub machine: MachineId,
    pub block: SelectedBlockId,
    pub instruction: SelectedInstructionId,
    pub literal_bits: u64,
    pub destination: X86XorZeroPhysicalWrite,
    pub rflags_units: Vec<RegisterUnitId>,
    pub baseline_byte_count: u8,
    pub selected_byte_count: u8,
    pub outcome: X86XorZeroMaterializationAttemptOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86XorZeroMaterializationAction {
    pub iteration: u64,
    pub input: X86XorZeroMaterializationRevisionIdentity,
    pub output: X86XorZeroMaterializationRevisionIdentity,
    pub machine: MachineId,
    pub block: SelectedBlockId,
    pub instruction: SelectedInstructionId,
    pub destination: X86XorZeroPhysicalWrite,
    pub rflags_units: Vec<RegisterUnitId>,
    pub baseline_byte_count: u8,
    pub selected_byte_count: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum X86XorZeroInstructionDisposition {
    RetainedV1,
    XorZeroMaterializationV1 {
        destination: X86XorZeroPhysicalWrite,
        rflags_units: Vec<RegisterUnitId>,
        baseline_byte_count: u8,
        selected_byte_count: u8,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86XorZeroMaterializationInstruction {
    pub instruction: SelectedInstructionId,
    pub disposition: X86XorZeroInstructionDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86XorZeroMaterializationBlock {
    pub block: SelectedBlockId,
    pub instructions: Vec<X86XorZeroMaterializationInstruction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86XorZeroMaterializationFunction {
    pub machine: MachineId,
    pub blocks: Vec<X86XorZeroMaterializationBlock>,
}

/// Immutable symbolic encoding-choice artifact. The selected instruction,
/// physical home, and baseline sidecar remain unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86XorZeroMaterializationPlan {
    pub identity: X86XorZeroMaterializationIdentity,
    pub source: PostAllocationMachineIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub liveness: LivenessIdentity,
    pub target: NativeTarget,
    pub physical_register_model: PhysicalRegisterModelIdentity,
    pub policy: X86XorZeroMaterializationPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub output_revision: X86XorZeroMaterializationRevisionIdentity,
    pub attempts: Vec<X86XorZeroMaterializationAttempt>,
    pub actions: Vec<X86XorZeroMaterializationAction>,
    pub functions: Vec<X86XorZeroMaterializationFunction>,
}

impl X86XorZeroMaterializationPlan {
    pub fn encode(&self) -> Vec<u8> {
        super::codec::encode(self)
    }

    /// Decode and authenticate an unchecked artifact. Independent validation
    /// against its retained roots is still mandatory.
    pub fn decode(encoded: &[u8]) -> Result<Self, crate::X86XorZeroMaterializationDecodeError> {
        super::codec::decode(encoded)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86XorZeroMaterializationReceipt {
    identity: X86XorZeroMaterializationIdentity,
    source: PostAllocationMachineIdentity,
    selected: SelectedInstructionPlanIdentity,
    liveness: LivenessIdentity,
    action_count: usize,
    baseline_bytes: u64,
    selected_bytes: u64,
}

impl X86XorZeroMaterializationReceipt {
    pub const fn identity(self) -> X86XorZeroMaterializationIdentity {
        self.identity
    }
    pub const fn source(self) -> PostAllocationMachineIdentity {
        self.source
    }
    pub const fn selected(self) -> SelectedInstructionPlanIdentity {
        self.selected
    }
    pub const fn liveness(self) -> LivenessIdentity {
        self.liveness
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
pub struct ValidatedX86XorZeroMaterialization {
    plan: X86XorZeroMaterializationPlan,
    receipt: X86XorZeroMaterializationReceipt,
}

impl ValidatedX86XorZeroMaterialization {
    pub const fn plan(&self) -> &X86XorZeroMaterializationPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> X86XorZeroMaterializationReceipt {
        self.receipt
    }
    pub(crate) const fn new(
        plan: X86XorZeroMaterializationPlan,
        receipt: X86XorZeroMaterializationReceipt,
    ) -> Self {
        Self { plan, receipt }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86XorZeroMaterializationWorkAxis {
    RuleEvaluations,
    Candidates,
    ValidationSteps,
    Commits,
    Iterations,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum X86XorZeroMaterializationError {
    UnsupportedTarget(NativeTarget),
    RootMismatch,
    MissingArchitecturalView(&'static str),
    FunctionRosterMismatch(usize),
    BlockRosterMismatch { function: usize, block: usize },
    InstructionRosterMismatch(SelectedInstructionId),
    LivenessRosterMismatch(SelectedInstructionId),
    IntegerOutsideI64Bits(SelectedInstructionId),
    InvalidMaterializationFootprint(SelectedInstructionId),
    InvalidPhysicalDestination(SelectedInstructionId),
    BudgetExceeded(X86XorZeroMaterializationWorkAxis),
    CountOverflow,
    ArtifactMismatch,
}

impl std::fmt::Display for X86XorZeroMaterializationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "x86-64 XOR-zero i64 materialization selection failed: {self:?}"
        )
    }
}

impl std::error::Error for X86XorZeroMaterializationError {}

pub(crate) fn x86_xor_zero_materialization_receipt(
    plan: &X86XorZeroMaterializationPlan,
) -> Result<X86XorZeroMaterializationReceipt, X86XorZeroMaterializationError> {
    let (baseline_bytes, selected_bytes) =
        plan.actions
            .iter()
            .try_fold((0_u64, 0_u64), |(baseline, selected), action| {
                Ok::<_, X86XorZeroMaterializationError>((
                    baseline
                        .checked_add(u64::from(action.baseline_byte_count))
                        .ok_or(X86XorZeroMaterializationError::CountOverflow)?,
                    selected
                        .checked_add(u64::from(action.selected_byte_count))
                        .ok_or(X86XorZeroMaterializationError::CountOverflow)?,
                ))
            })?;
    Ok(X86XorZeroMaterializationReceipt {
        identity: plan.identity,
        source: plan.source,
        selected: plan.selected,
        liveness: plan.liveness,
        action_count: plan.actions.len(),
        baseline_bytes,
        selected_bytes,
    })
}
