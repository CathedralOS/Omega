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

pub use omega_physical_instructions::Aarch64MovnMaterializationIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Aarch64MovnMaterializationRevisionIdentity([u8; 32]);

impl Aarch64MovnMaterializationRevisionIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Aarch64MovnMaterializationPolicy {
    Aarch64SelectShortestMovnSeededI64MaterializationV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Aarch64MovnPatch {
    /// Zero-based 16-bit halfword selected by the AArch64 `hw` field.
    pub halfword: u8,
    pub immediate: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Aarch64MovnRecipe {
    pub seed_halfword: u8,
    /// The immediate encoded by `MOVN`; the realized seed halfword is its
    /// bitwise complement and every other halfword starts as `0xffff`.
    pub seed_immediate: u16,
    /// Canonical ascending `MOVK` patches. Every row differs from `0xffff`.
    pub patches: Vec<Aarch64MovnPatch>,
}

impl Aarch64MovnRecipe {
    pub fn word_count(&self) -> Option<u8> {
        u8::try_from(self.patches.len())
            .ok()
            .and_then(|patches| patches.checked_add(1))
    }
}

/// Exact physical destination written by the replacement sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualifiedPhysicalWrite {
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
pub enum Aarch64MovnMaterializationAttemptOutcome {
    AlreadySelected,
    BaselineNotLonger,
    SelectedForRewrite,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aarch64MovnMaterializationAttempt {
    pub iteration: u64,
    pub input: Aarch64MovnMaterializationRevisionIdentity,
    pub machine: MachineId,
    pub block: SelectedBlockId,
    pub instruction: SelectedInstructionId,
    pub literal_bits: u64,
    pub destination: QualifiedPhysicalWrite,
    pub baseline_word_count: u8,
    pub recipe: Aarch64MovnRecipe,
    pub outcome: Aarch64MovnMaterializationAttemptOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aarch64MovnMaterializationAction {
    pub iteration: u64,
    pub input: Aarch64MovnMaterializationRevisionIdentity,
    pub output: Aarch64MovnMaterializationRevisionIdentity,
    pub machine: MachineId,
    pub block: SelectedBlockId,
    pub instruction: SelectedInstructionId,
    pub literal_bits: u64,
    pub destination: QualifiedPhysicalWrite,
    pub baseline_word_count: u8,
    pub recipe: Aarch64MovnRecipe,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Aarch64MovnInstructionDisposition {
    RetainedV1,
    MovnSeededMaterializationV1 {
        literal_bits: u64,
        destination: QualifiedPhysicalWrite,
        baseline_word_count: u8,
        recipe: Aarch64MovnRecipe,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aarch64MovnMaterializationInstruction {
    pub instruction: SelectedInstructionId,
    pub disposition: Aarch64MovnInstructionDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aarch64MovnMaterializationBlock {
    pub block: SelectedBlockId,
    pub instructions: Vec<Aarch64MovnMaterializationInstruction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aarch64MovnMaterializationFunction {
    pub machine: MachineId,
    pub blocks: Vec<Aarch64MovnMaterializationBlock>,
}

/// Immutable symbolic post-allocation encoding-choice artifact. It changes no
/// selected instruction, physical home, or effect declaration and owns no
/// encoded bytes, layout, emission, or publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aarch64MovnMaterializationPlan {
    pub identity: Aarch64MovnMaterializationIdentity,
    pub source: PostAllocationMachineIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub target: NativeTarget,
    pub physical_register_model: PhysicalRegisterModelIdentity,
    pub policy: Aarch64MovnMaterializationPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub output_revision: Aarch64MovnMaterializationRevisionIdentity,
    pub attempts: Vec<Aarch64MovnMaterializationAttempt>,
    pub actions: Vec<Aarch64MovnMaterializationAction>,
    pub functions: Vec<Aarch64MovnMaterializationFunction>,
}

impl Aarch64MovnMaterializationPlan {
    pub fn encode(&self) -> Vec<u8> {
        super::codec::encode(self)
    }

    /// Decode and content-authenticate an unchecked artifact. Call
    /// [`crate::validate_aarch64_movn_materialization`] before use.
    pub fn decode(encoded: &[u8]) -> Result<Self, crate::Aarch64MovnMaterializationDecodeError> {
        super::codec::decode(encoded)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aarch64MovnMaterializationReceipt {
    identity: Aarch64MovnMaterializationIdentity,
    source: PostAllocationMachineIdentity,
    selected: SelectedInstructionPlanIdentity,
    action_count: usize,
    baseline_words: u64,
    selected_words: u64,
}

impl Aarch64MovnMaterializationReceipt {
    pub const fn identity(self) -> Aarch64MovnMaterializationIdentity {
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
    pub const fn baseline_words(self) -> u64 {
        self.baseline_words
    }
    pub const fn selected_words(self) -> u64 {
        self.selected_words
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedAarch64MovnMaterialization {
    plan: Aarch64MovnMaterializationPlan,
    receipt: Aarch64MovnMaterializationReceipt,
}

impl ValidatedAarch64MovnMaterialization {
    pub const fn plan(&self) -> &Aarch64MovnMaterializationPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> Aarch64MovnMaterializationReceipt {
        self.receipt
    }
    pub(crate) const fn new(
        plan: Aarch64MovnMaterializationPlan,
        receipt: Aarch64MovnMaterializationReceipt,
    ) -> Self {
        Self { plan, receipt }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64MovnMaterializationWorkAxis {
    RuleEvaluations,
    Candidates,
    ValidationSteps,
    Commits,
    Iterations,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Aarch64MovnMaterializationError {
    UnsupportedTarget(NativeTarget),
    RootMismatch,
    FunctionRosterMismatch(usize),
    BlockRosterMismatch { function: usize, block: usize },
    InstructionRosterMismatch(SelectedInstructionId),
    IntegerOutsideI64Bits(SelectedInstructionId),
    InvalidMaterializationFootprint(SelectedInstructionId),
    InvalidPhysicalDestination(SelectedInstructionId),
    InvalidRecipe(SelectedInstructionId),
    BudgetExceeded(Aarch64MovnMaterializationWorkAxis),
    CountOverflow,
    ArtifactMismatch,
}

impl std::fmt::Display for Aarch64MovnMaterializationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "AArch64 MOVN-seeded i64 materialization selection failed: {self:?}"
        )
    }
}

impl std::error::Error for Aarch64MovnMaterializationError {}

pub(crate) fn materialization_receipt(
    plan: &Aarch64MovnMaterializationPlan,
) -> Result<Aarch64MovnMaterializationReceipt, Aarch64MovnMaterializationError> {
    let (baseline_words, selected_words) =
        plan.actions
            .iter()
            .try_fold((0_u64, 0_u64), |(baseline, selected), action| {
                let chosen = u64::from(
                    action
                        .recipe
                        .word_count()
                        .ok_or(Aarch64MovnMaterializationError::CountOverflow)?,
                );
                Ok::<_, Aarch64MovnMaterializationError>((
                    baseline
                        .checked_add(u64::from(action.baseline_word_count))
                        .ok_or(Aarch64MovnMaterializationError::CountOverflow)?,
                    selected
                        .checked_add(chosen)
                        .ok_or(Aarch64MovnMaterializationError::CountOverflow)?,
                ))
            })?;
    Ok(Aarch64MovnMaterializationReceipt {
        identity: plan.identity,
        source: plan.source,
        selected: plan.selected,
        action_count: plan.actions.len(),
        baseline_words,
        selected_words,
    })
}
