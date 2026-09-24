//! Optimizer module role: executable entrance. Complete machine-effect analysis for one validated selected CFG.

mod compute;
mod validate;

pub(crate) use compute::machine_semantic_kind;
pub use selected_instructions::{
    BlockMachineEffects, FunctionMachineEffects, InstructionMachineEffects,
    PreAllocationMachineEffectDecodeError, PreAllocationMachineEffectIdentity,
    PreAllocationMachineEffectPlan, pre_allocation_machine_effect_identity,
};
pub use validate::validate_pre_allocation_machine_effects;

use crate::ValidatedSelectedAnalysis;
use register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile,
};
use selected_instructions::ValidatedMachineEffectCatalog;
use selected_instructions::{
    MachineEffectCatalogIdentity, SelectedInstructionId, SelectedInstructionPlanIdentity,
};
use std::sync::Arc;

/// Compute and independently reconstruct the complete pre-allocation effect
/// sidecar. This grants no transformation, home, emission, or publication
/// authority.
#[allow(clippy::too_many_arguments)]
pub(crate) fn analyze_pre_allocation_machine_effects<S: ValidatedSelectedAnalysis>(
    selected: &S,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
    catalog: &ValidatedMachineEffectCatalog,
) -> Result<ValidatedPreAllocationMachineEffects, MachineEffectError> {
    let plan = compute::compute_terminal_pre_allocation_machine_effects(
        selected,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        catalog,
    )?;
    validate_pre_allocation_machine_effects(
        selected,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        catalog,
        plan,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreAllocationMachineEffectReceipt {
    identity: PreAllocationMachineEffectIdentity,
    selected: SelectedInstructionPlanIdentity,
    register_environment: TargetRegisterEnvironmentIdentity,
    machine_effect_catalog: MachineEffectCatalogIdentity,
    function_count: usize,
    block_count: usize,
    instruction_count: usize,
    alternative_count: usize,
    unit_action_count: usize,
    fuel_settlement_count: usize,
}

impl PreAllocationMachineEffectReceipt {
    pub const fn identity(self) -> PreAllocationMachineEffectIdentity {
        self.identity
    }
    pub const fn selected(self) -> SelectedInstructionPlanIdentity {
        self.selected
    }
    pub const fn register_environment(self) -> TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub const fn machine_effect_catalog(self) -> MachineEffectCatalogIdentity {
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
pub struct ValidatedPreAllocationMachineEffects {
    plan: Arc<PreAllocationMachineEffectPlan>,
    receipt: PreAllocationMachineEffectReceipt,
}

impl ValidatedPreAllocationMachineEffects {
    pub fn plan(&self) -> &PreAllocationMachineEffectPlan {
        &self.plan
    }

    /// Retain the original immutable facts, not an analyzer or admission token.
    pub fn shared_plan(&self) -> Arc<PreAllocationMachineEffectPlan> {
        Arc::clone(&self.plan)
    }

    pub const fn receipt(&self) -> PreAllocationMachineEffectReceipt {
        self.receipt
    }

    fn new(
        plan: PreAllocationMachineEffectPlan,
        receipt: PreAllocationMachineEffectReceipt,
    ) -> Self {
        Self {
            plan: Arc::new(plan),
            receipt,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MachineEffectError {
    RegisterEnvironmentMismatch,
    CatalogTargetMismatch,
    CatalogConstraintMismatch,
    CatalogSelectedKeysMismatch,
    SelectedRootMismatch,
    MissingDeclaration { instruction: SelectedInstructionId },
    AmbiguousDeclaration { instruction: SelectedInstructionId },
    ConstraintEffectMismatch { instruction: SelectedInstructionId },
    ConstraintOperandMismatch { instruction: SelectedInstructionId },
    FuelProvenanceMismatch { instruction: SelectedInstructionId },
    NonCanonicalFunction,
    NonCanonicalBlock,
    InstructionMismatch { instruction: SelectedInstructionId },
    IdentityMismatch,
    CountOverflow,
}

impl std::fmt::Display for MachineEffectError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "machine-effect analysis failed: {self:?}")
    }
}

impl std::error::Error for MachineEffectError {}

fn receipt(
    plan: &PreAllocationMachineEffectPlan,
) -> Result<PreAllocationMachineEffectReceipt, MachineEffectError> {
    let ordinary_block_count = plan.functions.iter().try_fold(0usize, |count, function| {
        count
            .checked_add(function.blocks.len())
            .ok_or(MachineEffectError::CountOverflow)
    })?;
    let block_count = ordinary_block_count;
    let ordinary_instruction_count = plan
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .try_fold(0usize, |count, block| {
            count
                .checked_add(block.instructions.len())
                .ok_or(MachineEffectError::CountOverflow)
        })?;
    let instruction_count = ordinary_instruction_count;
    let (alternative_count, unit_action_count, fuel_settlement_count) = plan
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.instructions)
        .try_fold((0usize, 0usize, 0usize), |counts, instruction| {
            Ok::<_, MachineEffectError>((
                counts
                    .0
                    .checked_add(instruction.alternatives.len())
                    .ok_or(MachineEffectError::CountOverflow)?,
                counts
                    .1
                    .checked_add(instruction.unit_uses.len())
                    .and_then(|count| count.checked_add(instruction.unit_defs.len()))
                    .and_then(|count| count.checked_add(instruction.unit_clobbers.len()))
                    .ok_or(MachineEffectError::CountOverflow)?,
                counts
                    .2
                    .checked_add(instruction.provenance.fuel.len())
                    .ok_or(MachineEffectError::CountOverflow)?,
            ))
        })?;
    Ok(PreAllocationMachineEffectReceipt {
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
