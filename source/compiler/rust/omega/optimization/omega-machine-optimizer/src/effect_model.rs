use omega_optimization_core::OptimizationUnitIdentity;
use omega_register_model::{
    RegisterConstraintCatalogIdentity, RegisterConstraintKey, RegisterUnitId,
    TargetRegisterEnvironmentIdentity,
};
use omega_target::NativeTarget;
use omega_terminal_selected_instructions::{
    TerminalMachineAlternative, TerminalMachineBarrier, TerminalMachineCallEffect,
    TerminalMachineCleanupEffect, TerminalMachineEffectCatalogIdentity,
    TerminalMachineMemoryEffect, TerminalMachineTrapBehavior, TerminalSelectedBlockId,
    TerminalSelectedInstructionId, TerminalSelectedInstructionKind,
    TerminalSelectedInstructionPlanIdentity, TerminalSelectedInstructionProvenance,
};
use psi_core::{FuelScheduleIdentity, MachineId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalPreAllocationMachineEffectIdentity([u8; 32]);

impl TerminalPreAllocationMachineEffectIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalPreAllocationMachineEffectPlan {
    pub identity: TerminalPreAllocationMachineEffectIdentity,
    pub selected: TerminalSelectedInstructionPlanIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub target: NativeTarget,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub register_constraints: RegisterConstraintCatalogIdentity,
    pub machine_effect_catalog: TerminalMachineEffectCatalogIdentity,
    pub functions: Vec<TerminalFunctionMachineEffects>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalFunctionMachineEffects {
    pub machine: MachineId,
    pub blocks: Vec<TerminalBlockMachineEffects>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalBlockMachineEffects {
    pub block: TerminalSelectedBlockId,
    /// Ordinary selected instructions followed by the selected terminator.
    pub instructions: Vec<TerminalInstructionMachineEffects>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalInstructionMachineEffects {
    pub instruction: TerminalSelectedInstructionId,
    pub kind: TerminalSelectedInstructionKind,
    pub constraint: RegisterConstraintKey,
    pub unit_uses: Vec<RegisterUnitId>,
    pub unit_defs: Vec<RegisterUnitId>,
    pub unit_clobbers: Vec<RegisterUnitId>,
    pub memory: TerminalMachineMemoryEffect,
    pub trap: TerminalMachineTrapBehavior,
    pub barrier: TerminalMachineBarrier,
    pub call: TerminalMachineCallEffect,
    pub cleanup: TerminalMachineCleanupEffect,
    pub provenance: TerminalSelectedInstructionProvenance,
    pub alternatives: Vec<TerminalMachineAlternative>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalPreAllocationMachineEffectReceipt {
    identity: TerminalPreAllocationMachineEffectIdentity,
    selected: TerminalSelectedInstructionPlanIdentity,
    register_environment: TargetRegisterEnvironmentIdentity,
    machine_effect_catalog: TerminalMachineEffectCatalogIdentity,
    function_count: usize,
    block_count: usize,
    instruction_count: usize,
    alternative_count: usize,
    unit_action_count: usize,
    fuel_settlement_count: usize,
}

impl TerminalPreAllocationMachineEffectReceipt {
    pub const fn identity(self) -> TerminalPreAllocationMachineEffectIdentity {
        self.identity
    }
    pub const fn selected(self) -> TerminalSelectedInstructionPlanIdentity {
        self.selected
    }
    pub const fn register_environment(self) -> TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub const fn machine_effect_catalog(self) -> TerminalMachineEffectCatalogIdentity {
        self.machine_effect_catalog
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn block_count(self) -> usize {
        self.block_count
    }
    pub const fn instruction_count(self) -> usize {
        self.instruction_count
    }
    pub const fn alternative_count(self) -> usize {
        self.alternative_count
    }
    pub const fn unit_action_count(self) -> usize {
        self.unit_action_count
    }
    pub const fn fuel_settlement_count(self) -> usize {
        self.fuel_settlement_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTerminalPreAllocationMachineEffects {
    plan: TerminalPreAllocationMachineEffectPlan,
    receipt: TerminalPreAllocationMachineEffectReceipt,
}

impl ValidatedTerminalPreAllocationMachineEffects {
    pub const fn plan(&self) -> &TerminalPreAllocationMachineEffectPlan {
        &self.plan
    }

    pub const fn receipt(&self) -> TerminalPreAllocationMachineEffectReceipt {
        self.receipt
    }

    pub(crate) const fn new(
        plan: TerminalPreAllocationMachineEffectPlan,
        receipt: TerminalPreAllocationMachineEffectReceipt,
    ) -> Self {
        Self { plan, receipt }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalMachineEffectError {
    RegisterEnvironmentMismatch,
    CatalogTargetMismatch,
    CatalogConstraintMismatch,
    CatalogSelectedKeysMismatch,
    SelectedRootMismatch,
    MissingDeclaration {
        instruction: TerminalSelectedInstructionId,
    },
    AmbiguousDeclaration {
        instruction: TerminalSelectedInstructionId,
    },
    ConstraintEffectMismatch {
        instruction: TerminalSelectedInstructionId,
    },
    NonCanonicalFunction,
    NonCanonicalBlock,
    InstructionMismatch {
        instruction: TerminalSelectedInstructionId,
    },
    IdentityMismatch,
    CountOverflow,
}

impl std::fmt::Display for TerminalMachineEffectError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Terminal machine-effect analysis failed: {self:?}"
        )
    }
}

impl std::error::Error for TerminalMachineEffectError {}

pub(crate) fn receipt(
    plan: &TerminalPreAllocationMachineEffectPlan,
) -> Result<TerminalPreAllocationMachineEffectReceipt, TerminalMachineEffectError> {
    let block_count = plan.functions.iter().try_fold(0usize, |count, function| {
        count
            .checked_add(function.blocks.len())
            .ok_or(TerminalMachineEffectError::CountOverflow)
    })?;
    let instruction_count = plan
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .try_fold(0usize, |count, block| {
            count
                .checked_add(block.instructions.len())
                .ok_or(TerminalMachineEffectError::CountOverflow)
        })?;
    let (alternative_count, unit_action_count, fuel_settlement_count) = plan
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.instructions)
        .try_fold((0usize, 0usize, 0usize), |counts, instruction| {
            Ok::<_, TerminalMachineEffectError>((
                counts
                    .0
                    .checked_add(instruction.alternatives.len())
                    .ok_or(TerminalMachineEffectError::CountOverflow)?,
                counts
                    .1
                    .checked_add(instruction.unit_uses.len())
                    .and_then(|count| count.checked_add(instruction.unit_defs.len()))
                    .and_then(|count| count.checked_add(instruction.unit_clobbers.len()))
                    .ok_or(TerminalMachineEffectError::CountOverflow)?,
                counts
                    .2
                    .checked_add(instruction.provenance.fuel.len())
                    .ok_or(TerminalMachineEffectError::CountOverflow)?,
            ))
        })?;
    Ok(TerminalPreAllocationMachineEffectReceipt {
        identity: plan.identity,
        selected: plan.selected,
        register_environment: plan.register_environment,
        machine_effect_catalog: plan.machine_effect_catalog,
        function_count: plan.functions.len(),
        block_count,
        instruction_count,
        alternative_count,
        unit_action_count,
        fuel_settlement_count,
    })
}
