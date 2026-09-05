use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{
    PhysicalRegisterModelIdentity, RegisterClassId, RegisterUnitId, RegisterViewId,
    ValidatedPhysicalRegisterModel,
};
use selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlan,
    SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use selected_instructions_to_register_homes::{LivenessIdentity, LivenessPlan};
use semantic_vocabulary::{EdgeId, MachineId};
use target::NativeTarget;

use physical_instructions::{PostAllocationMachineIdentity, PostAllocationMachinePlan};

pub use physical_instructions::Aarch64CbnzFusionIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Aarch64CbnzFusionRevisionIdentity([u8; 32]);

impl Aarch64CbnzFusionRevisionIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Aarch64CbnzFusionPolicy {
    Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Aarch64CbnzFusionAttemptOutcome {
    AlreadyFused,
    CompareCarriesFuel,
    NzcvLiveOut,
    SelectedForFusion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Aarch64CbnzFusionAttempt {
    pub iteration: u64,
    pub input: Aarch64CbnzFusionRevisionIdentity,
    pub machine: MachineId,
    pub block: SelectedBlockId,
    pub compare: SelectedInstructionId,
    pub branch: SelectedInstructionId,
    pub outcome: Aarch64CbnzFusionAttemptOutcome,
}

/// A physical dependency qualified by the selected instruction and operand
/// that owns the value. The fused branch has no selected operands of its own,
/// so this must not be represented as branch operand zero.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QualifiedPhysicalRead {
    pub source_instruction: SelectedInstructionId,
    pub operand: u16,
    pub virtual_register: VirtualRegisterId,
    pub class: RegisterClassId,
    pub view: RegisterViewId,
    pub units: Vec<RegisterUnitId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Aarch64CbnzFusionAction {
    pub iteration: u64,
    pub input: Aarch64CbnzFusionRevisionIdentity,
    pub output: Aarch64CbnzFusionRevisionIdentity,
    pub machine: MachineId,
    pub block: SelectedBlockId,
    pub compare: SelectedInstructionId,
    pub branch: SelectedInstructionId,
    pub source_read: QualifiedPhysicalRead,
    pub nzcv_units: Vec<RegisterUnitId>,
    pub pc_units: Vec<RegisterUnitId>,
    pub when_nonzero_edge: EdgeId,
    pub when_nonzero_block: SelectedBlockId,
    pub when_zero_edge: EdgeId,
    pub when_zero_block: SelectedBlockId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Aarch64CbnzInstructionDisposition {
    RetainedV1,
    ElidedCompareI64ZeroV1 {
        consumer: SelectedInstructionId,
    },
    FusedBranchNonZeroToCbnzV1 {
        compare: SelectedInstructionId,
        source_read: QualifiedPhysicalRead,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Aarch64CbnzFusionInstruction {
    pub instruction: SelectedInstructionId,
    pub disposition: Aarch64CbnzInstructionDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Aarch64CbnzFusionBlock {
    pub block: SelectedBlockId,
    pub instructions: Vec<Aarch64CbnzFusionInstruction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Aarch64CbnzFusionFunction {
    pub machine: MachineId,
    pub blocks: Vec<Aarch64CbnzFusionBlock>,
}

/// Immutable symbolic post-allocation transformation. It deliberately owns no
/// branch displacement, encoded bytes, layout, emission, or publication
/// authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aarch64CbnzFusionPlan {
    pub identity: Aarch64CbnzFusionIdentity,
    pub source: PostAllocationMachineIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub liveness: LivenessIdentity,
    pub target: NativeTarget,
    pub physical_register_model: PhysicalRegisterModelIdentity,
    pub policy: Aarch64CbnzFusionPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub output_revision: Aarch64CbnzFusionRevisionIdentity,
    pub attempts: Vec<Aarch64CbnzFusionAttempt>,
    pub actions: Vec<Aarch64CbnzFusionAction>,
    pub functions: Vec<Aarch64CbnzFusionFunction>,
}

impl Aarch64CbnzFusionPlan {
    pub fn encode(&self) -> Vec<u8> {
        super::codec::encode(self)
    }

    /// Decode and content-authenticate a plain unchecked artifact. Call
    /// [`crate::validate_aarch64_cbnz_fusion`] against the retained inputs
    /// before using any disposition.
    pub fn decode(encoded: &[u8]) -> Result<Self, crate::Aarch64CbnzFusionDecodeError> {
        super::codec::decode(encoded)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aarch64CbnzFusionReceipt {
    identity: Aarch64CbnzFusionIdentity,
    source: PostAllocationMachineIdentity,
    selected: SelectedInstructionPlanIdentity,
    liveness: LivenessIdentity,
    action_count: usize,
}

impl Aarch64CbnzFusionReceipt {
    pub const fn identity(self) -> Aarch64CbnzFusionIdentity {
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
pub struct ValidatedAarch64CbnzFusion {
    plan: Aarch64CbnzFusionPlan,
    receipt: Aarch64CbnzFusionReceipt,
}

pub(crate) struct CbnzFusionInputs<'a> {
    pub selected: &'a SelectedInstructionPlan,
    pub selected_identity: SelectedInstructionPlanIdentity,
    pub liveness: &'a LivenessPlan,
    pub liveness_identity: LivenessIdentity,
    pub source: &'a PostAllocationMachinePlan,
    pub source_identity: PostAllocationMachineIdentity,
    pub physical: &'a ValidatedPhysicalRegisterModel,
}

impl ValidatedAarch64CbnzFusion {
    pub const fn plan(&self) -> &Aarch64CbnzFusionPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> Aarch64CbnzFusionReceipt {
        self.receipt
    }
    pub(crate) const fn new(
        plan: Aarch64CbnzFusionPlan,
        receipt: Aarch64CbnzFusionReceipt,
    ) -> Self {
        Self { plan, receipt }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64CbnzFusionWorkAxis {
    RuleEvaluations,
    Candidates,
    ValidationSteps,
    Commits,
    Iterations,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Aarch64CbnzFusionError {
    UnsupportedTarget(NativeTarget),
    RootMismatch,
    MissingArchitecturalView(&'static str),
    FunctionRosterMismatch(usize),
    BlockRosterMismatch { function: usize, block: usize },
    InstructionRosterMismatch(SelectedInstructionId),
    LivenessRosterMismatch(SelectedInstructionId),
    InvalidCompareFootprint(SelectedInstructionId),
    InvalidBranchFootprint(SelectedInstructionId),
    InvalidPhysicalSource(SelectedInstructionId),
    BudgetExceeded(Aarch64CbnzFusionWorkAxis),
    CountOverflow,
    ArtifactMismatch,
}

impl std::fmt::Display for Aarch64CbnzFusionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "AArch64 compare/branch CBNZ fusion failed: {self:?}"
        )
    }
}

impl std::error::Error for Aarch64CbnzFusionError {}

pub(crate) fn fusion_receipt(plan: &Aarch64CbnzFusionPlan) -> Aarch64CbnzFusionReceipt {
    Aarch64CbnzFusionReceipt {
        identity: plan.identity,
        source: plan.source,
        selected: plan.selected,
        liveness: plan.liveness,
        action_count: plan.actions.len(),
    }
}
