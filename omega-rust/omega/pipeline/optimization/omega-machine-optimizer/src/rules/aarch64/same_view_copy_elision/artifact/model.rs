use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_regalloc::{LivenessIdentity, LivenessPlan};
use omega_register_model::{
    PhysicalRegisterModelIdentity, RegisterClassId, RegisterUnitId, RegisterViewId,
    ValidatedPhysicalRegisterModel,
};
use omega_selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlan,
    SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use omega_target::NativeTarget;
use psi_core::{MachineId, ValueId};

use crate::{PostAllocationMachineIdentity, PostAllocationMachinePlan};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Aarch64SameViewCopyElisionIdentity([u8; 32]);

impl Aarch64SameViewCopyElisionIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Aarch64SameViewCopyElisionRevisionIdentity([u8; 32]);

impl Aarch64SameViewCopyElisionRevisionIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Aarch64SameViewCopyElisionPolicy {
    Aarch64ElideSameViewCopyI64BeforeReturnV1,
    Aarch64ElideSameViewCopyI64BeforeCompareZeroV1,
    Aarch64ElideSameViewCopyI64BeforeCompareI64LeftOperandV1,
    Aarch64ElideSameViewCopyI64BeforeCompareI64RightOperandV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Aarch64SameViewCopyElisionAttemptOutcome {
    AlreadyElided,
    DifferentPhysicalStorage,
    DestinationNotConsumed,
    SemanticProvenance,
    SelectedForElision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Aarch64SameViewCopyElisionAttempt {
    pub iteration: u64,
    pub input: Aarch64SameViewCopyElisionRevisionIdentity,
    pub machine: MachineId,
    pub block: SelectedBlockId,
    pub copy: SelectedInstructionId,
    pub consumer: SelectedInstructionId,
    pub outcome: Aarch64SameViewCopyElisionAttemptOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QualifiedPhysicalOperand {
    pub instruction: SelectedInstructionId,
    pub operand: u16,
    pub virtual_register: VirtualRegisterId,
    pub class: RegisterClassId,
    pub view: RegisterViewId,
    pub storage_units: Vec<RegisterUnitId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Aarch64SameViewCopyElisionAction {
    pub iteration: u64,
    pub input: Aarch64SameViewCopyElisionRevisionIdentity,
    pub output: Aarch64SameViewCopyElisionRevisionIdentity,
    pub machine: MachineId,
    pub block: SelectedBlockId,
    pub copy: SelectedInstructionId,
    pub consumer: SelectedInstructionId,
    pub source: QualifiedPhysicalOperand,
    pub destination: QualifiedPhysicalOperand,
    pub consumed: QualifiedPhysicalOperand,
    pub source_value: ValueId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Aarch64SameViewCopyInstructionDisposition {
    RetainedV1,
    ElidedSameViewCopyI64V1 { consumer: SelectedInstructionId },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Aarch64SameViewCopyElisionInstruction {
    pub instruction: SelectedInstructionId,
    pub disposition: Aarch64SameViewCopyInstructionDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Aarch64SameViewCopyElisionBlock {
    pub block: SelectedBlockId,
    pub instructions: Vec<Aarch64SameViewCopyElisionInstruction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Aarch64SameViewCopyElisionFunction {
    pub machine: MachineId,
    pub blocks: Vec<Aarch64SameViewCopyElisionBlock>,
}

/// Symbolic plan. It owns no encoding, layout, emission, or publication
/// authority; downstream stages must replay and bind this receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aarch64SameViewCopyElisionPlan {
    pub identity: Aarch64SameViewCopyElisionIdentity,
    pub source: PostAllocationMachineIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub liveness: LivenessIdentity,
    pub target: NativeTarget,
    pub physical_register_model: PhysicalRegisterModelIdentity,
    pub policy: Aarch64SameViewCopyElisionPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub output_revision: Aarch64SameViewCopyElisionRevisionIdentity,
    pub attempts: Vec<Aarch64SameViewCopyElisionAttempt>,
    pub actions: Vec<Aarch64SameViewCopyElisionAction>,
    pub functions: Vec<Aarch64SameViewCopyElisionFunction>,
}

impl Aarch64SameViewCopyElisionPlan {
    pub fn encode(&self) -> Vec<u8> {
        super::codec::encode(self)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, Aarch64SameViewCopyElisionDecodeError> {
        super::codec::decode(encoded)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aarch64SameViewCopyElisionReceipt {
    identity: Aarch64SameViewCopyElisionIdentity,
    source: PostAllocationMachineIdentity,
    selected: SelectedInstructionPlanIdentity,
    liveness: LivenessIdentity,
    action_count: usize,
}

impl Aarch64SameViewCopyElisionReceipt {
    pub const fn identity(self) -> Aarch64SameViewCopyElisionIdentity {
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedAarch64SameViewCopyElision {
    plan: Aarch64SameViewCopyElisionPlan,
    receipt: Aarch64SameViewCopyElisionReceipt,
}

impl ValidatedAarch64SameViewCopyElision {
    pub const fn plan(&self) -> &Aarch64SameViewCopyElisionPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> Aarch64SameViewCopyElisionReceipt {
        self.receipt
    }
    pub(crate) const fn new(
        plan: Aarch64SameViewCopyElisionPlan,
        receipt: Aarch64SameViewCopyElisionReceipt,
    ) -> Self {
        Self { plan, receipt }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64SameViewCopyElisionWorkAxis {
    RuleEvaluations,
    Candidates,
    ValidationSteps,
    Commits,
    Iterations,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Aarch64SameViewCopyElisionError {
    UnsupportedTarget(NativeTarget),
    RootMismatch,
    MissingArchitecturalView(&'static str),
    FunctionRosterMismatch(usize),
    BlockRosterMismatch { function: usize, block: usize },
    InstructionRosterMismatch(SelectedInstructionId),
    LivenessRosterMismatch(SelectedInstructionId),
    InvalidCopyFootprint(SelectedInstructionId),
    InvalidReturnFootprint(SelectedInstructionId),
    InvalidCompareFootprint(SelectedInstructionId),
    InvalidPhysicalOperand(SelectedInstructionId),
    BudgetExceeded(Aarch64SameViewCopyElisionWorkAxis),
    ArtifactMismatch,
}

impl std::fmt::Display for Aarch64SameViewCopyElisionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "AArch64 same-view CopyI64 elision failed: {self:?}"
        )
    }
}

impl std::error::Error for Aarch64SameViewCopyElisionError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64SameViewCopyElisionDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    InvalidField,
    InvalidIdentity,
    TrailingBytes,
}

impl std::fmt::Display for Aarch64SameViewCopyElisionDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid AArch64 same-view copy-elision artifact: {self:?}"
        )
    }
}

impl std::error::Error for Aarch64SameViewCopyElisionDecodeError {}

pub(crate) struct SameViewCopyInputs<'a> {
    pub selected: &'a SelectedInstructionPlan,
    pub selected_identity: SelectedInstructionPlanIdentity,
    pub liveness: &'a LivenessPlan,
    pub liveness_identity: LivenessIdentity,
    pub source: &'a PostAllocationMachinePlan,
    pub source_identity: PostAllocationMachineIdentity,
    pub physical: &'a ValidatedPhysicalRegisterModel,
}

pub(crate) fn same_view_copy_elision_receipt(
    plan: &Aarch64SameViewCopyElisionPlan,
) -> Aarch64SameViewCopyElisionReceipt {
    Aarch64SameViewCopyElisionReceipt {
        identity: plan.identity,
        source: plan.source,
        selected: plan.selected,
        liveness: plan.liveness,
        action_count: plan.actions.len(),
    }
}
