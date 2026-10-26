//! Optimizer module role: executable entrance. Complete machine-effect analysis for one validated selected CFG.

mod compute;
mod foreign_call;
mod validate;

pub(crate) use compute::{machine_alternative_is_transparent, machine_semantic_kind};
pub use selected_instructions::{
    PreAllocationMachineEffectIdentity, PreAllocationMachineEffectPlan,
};

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

/// Compute and seal the complete pre-allocation effect sidecar. This grants no transformation, home, emission, or publication
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
    validate::seal_pre_allocation_machine_effects(plan)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreAllocationMachineEffectReceipt {
    identity: PreAllocationMachineEffectIdentity,
    selected: SelectedInstructionPlanIdentity,
    register_environment: TargetRegisterEnvironmentIdentity,
    machine_effect_catalog: MachineEffectCatalogIdentity,
}

impl PreAllocationMachineEffectReceipt {
    receipt_accessor_rows!(PreAllocationMachineEffectReceipt {
        identity: PreAllocationMachineEffectIdentity,
        selected: SelectedInstructionPlanIdentity,
        machine_effect_catalog: MachineEffectCatalogIdentity,
        register_environment: TargetRegisterEnvironmentIdentity,
    });
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

    receipt_accessor_rows!(ref ValidatedPreAllocationMachineEffects {
        receipt: PreAllocationMachineEffectReceipt,
    });

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
    MissingDeclaration {
        machine: semantic_vocabulary::MachineId,
        instruction: SelectedInstructionId,
        semantic: selected_instructions::MachineSemanticKind,
        constraint: register_model::RegisterConstraintKey,
    },
    AmbiguousDeclaration {
        instruction: SelectedInstructionId,
    },
    ConstraintEffectMismatch {
        instruction: SelectedInstructionId,
    },
    ConstraintOperandMismatch {
        instruction: SelectedInstructionId,
    },
    FuelProvenanceMismatch {
        instruction: SelectedInstructionId,
    },
    NonCanonicalFunction,
    NonCanonicalBlock,
    InstructionMismatch {
        instruction: SelectedInstructionId,
    },
    IdentityMismatch,
}

failed_display!(MachineEffectError, "machine-effect analysis failed");

impl std::error::Error for MachineEffectError {}

fn receipt(plan: &PreAllocationMachineEffectPlan) -> PreAllocationMachineEffectReceipt {
    PreAllocationMachineEffectReceipt {
        identity: plan.identity,
        selected: plan.selected,
        register_environment: plan.register_environment,
        machine_effect_catalog: plan.machine_effect_catalog,
    }
}
