use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_regalloc::TerminalLivenessIdentity;
use omega_register_model::{
    PhysicalRegisterModelIdentity, RegisterClassId, RegisterUnitId, RegisterViewId,
};
use omega_target::NativeTarget;
use omega_terminal_selected_instructions::{
    TerminalSelectedBlockId, TerminalSelectedInstructionId,
    TerminalSelectedInstructionPlanIdentity, TerminalVirtualRegisterId,
};
use psi_core::{EdgeId, MachineId};

use crate::TerminalPostAllocationMachineIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalAarch64CbnzFusionIdentity([u8; 32]);

impl TerminalAarch64CbnzFusionIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalAarch64CbnzFusionRevisionIdentity([u8; 32]);

impl TerminalAarch64CbnzFusionRevisionIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerminalAarch64CbnzFusionPolicy {
    Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerminalAarch64CbnzFusionAttemptOutcome {
    AlreadyFused,
    CompareCarriesFuel,
    NzcvLiveOut,
    SelectedForFusion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalAarch64CbnzFusionAttempt {
    pub iteration: u64,
    pub input: TerminalAarch64CbnzFusionRevisionIdentity,
    pub machine: MachineId,
    pub block: TerminalSelectedBlockId,
    pub compare: TerminalSelectedInstructionId,
    pub branch: TerminalSelectedInstructionId,
    pub outcome: TerminalAarch64CbnzFusionAttemptOutcome,
}

/// A physical dependency qualified by the selected instruction and operand
/// that owns the value. The fused branch has no selected operands of its own,
/// so this must not be represented as branch operand zero.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TerminalQualifiedPhysicalRead {
    pub source_instruction: TerminalSelectedInstructionId,
    pub operand: u16,
    pub virtual_register: TerminalVirtualRegisterId,
    pub class: RegisterClassId,
    pub view: RegisterViewId,
    pub units: Vec<RegisterUnitId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TerminalAarch64CbnzFusionAction {
    pub iteration: u64,
    pub input: TerminalAarch64CbnzFusionRevisionIdentity,
    pub output: TerminalAarch64CbnzFusionRevisionIdentity,
    pub machine: MachineId,
    pub block: TerminalSelectedBlockId,
    pub compare: TerminalSelectedInstructionId,
    pub branch: TerminalSelectedInstructionId,
    pub source_read: TerminalQualifiedPhysicalRead,
    pub nzcv_units: Vec<RegisterUnitId>,
    pub pc_units: Vec<RegisterUnitId>,
    pub when_nonzero_edge: EdgeId,
    pub when_nonzero_block: TerminalSelectedBlockId,
    pub when_zero_edge: EdgeId,
    pub when_zero_block: TerminalSelectedBlockId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TerminalAarch64CbnzInstructionDisposition {
    RetainedV1,
    ElidedCompareI64ZeroV1 {
        consumer: TerminalSelectedInstructionId,
    },
    FusedBranchNonZeroToCbnzV1 {
        compare: TerminalSelectedInstructionId,
        source_read: TerminalQualifiedPhysicalRead,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TerminalAarch64CbnzFusionInstruction {
    pub instruction: TerminalSelectedInstructionId,
    pub disposition: TerminalAarch64CbnzInstructionDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TerminalAarch64CbnzFusionBlock {
    pub block: TerminalSelectedBlockId,
    pub instructions: Vec<TerminalAarch64CbnzFusionInstruction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TerminalAarch64CbnzFusionFunction {
    pub machine: MachineId,
    pub blocks: Vec<TerminalAarch64CbnzFusionBlock>,
}

/// Immutable symbolic post-allocation transformation. It deliberately owns no
/// branch displacement, encoded bytes, layout, emission, or publication
/// authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAarch64CbnzFusionPlan {
    pub identity: TerminalAarch64CbnzFusionIdentity,
    pub source: TerminalPostAllocationMachineIdentity,
    pub selected: TerminalSelectedInstructionPlanIdentity,
    pub liveness: TerminalLivenessIdentity,
    pub target: NativeTarget,
    pub physical_register_model: PhysicalRegisterModelIdentity,
    pub policy: TerminalAarch64CbnzFusionPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub output_revision: TerminalAarch64CbnzFusionRevisionIdentity,
    pub attempts: Vec<TerminalAarch64CbnzFusionAttempt>,
    pub actions: Vec<TerminalAarch64CbnzFusionAction>,
    pub functions: Vec<TerminalAarch64CbnzFusionFunction>,
}

impl TerminalAarch64CbnzFusionPlan {
    pub fn encode(&self) -> Vec<u8> {
        crate::aarch64_cbnz_codec::encode(self)
    }

    /// Decode and content-authenticate a plain unchecked artifact. Call
    /// [`crate::validate_aarch64_cbnz_fusion`] against the retained inputs
    /// before using any disposition.
    pub fn decode(encoded: &[u8]) -> Result<Self, crate::TerminalAarch64CbnzFusionDecodeError> {
        crate::aarch64_cbnz_codec::decode(encoded)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalAarch64CbnzFusionReceipt {
    identity: TerminalAarch64CbnzFusionIdentity,
    source: TerminalPostAllocationMachineIdentity,
    selected: TerminalSelectedInstructionPlanIdentity,
    liveness: TerminalLivenessIdentity,
    action_count: usize,
}

impl TerminalAarch64CbnzFusionReceipt {
    pub const fn identity(self) -> TerminalAarch64CbnzFusionIdentity {
        self.identity
    }
    pub const fn source(self) -> TerminalPostAllocationMachineIdentity {
        self.source
    }
    pub const fn selected(self) -> TerminalSelectedInstructionPlanIdentity {
        self.selected
    }
    pub const fn liveness(self) -> TerminalLivenessIdentity {
        self.liveness
    }
    pub const fn action_count(self) -> usize {
        self.action_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTerminalAarch64CbnzFusion {
    plan: TerminalAarch64CbnzFusionPlan,
    receipt: TerminalAarch64CbnzFusionReceipt,
}

impl ValidatedTerminalAarch64CbnzFusion {
    pub const fn plan(&self) -> &TerminalAarch64CbnzFusionPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> TerminalAarch64CbnzFusionReceipt {
        self.receipt
    }
    pub(crate) const fn new(
        plan: TerminalAarch64CbnzFusionPlan,
        receipt: TerminalAarch64CbnzFusionReceipt,
    ) -> Self {
        Self { plan, receipt }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalAarch64CbnzFusionWorkAxis {
    RuleEvaluations,
    Candidates,
    ValidationSteps,
    Commits,
    Iterations,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalAarch64CbnzFusionError {
    UnsupportedTarget(NativeTarget),
    RootMismatch,
    MissingArchitecturalView(&'static str),
    FunctionRosterMismatch(usize),
    BlockRosterMismatch { function: usize, block: usize },
    InstructionRosterMismatch(TerminalSelectedInstructionId),
    LivenessRosterMismatch(TerminalSelectedInstructionId),
    InvalidCompareFootprint(TerminalSelectedInstructionId),
    InvalidBranchFootprint(TerminalSelectedInstructionId),
    InvalidPhysicalSource(TerminalSelectedInstructionId),
    BudgetExceeded(TerminalAarch64CbnzFusionWorkAxis),
    CountOverflow,
    ArtifactMismatch,
}

impl std::fmt::Display for TerminalAarch64CbnzFusionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "AArch64 compare/branch CBNZ fusion failed: {self:?}"
        )
    }
}

impl std::error::Error for TerminalAarch64CbnzFusionError {}

pub(crate) fn fusion_receipt(
    plan: &TerminalAarch64CbnzFusionPlan,
) -> TerminalAarch64CbnzFusionReceipt {
    TerminalAarch64CbnzFusionReceipt {
        identity: plan.identity,
        source: plan.source,
        selected: plan.selected,
        liveness: plan.liveness,
        action_count: plan.actions.len(),
    }
}
