use omega_optimization_core::OptimizationUnitIdentity;
use omega_register_model::{
    RegisterConstraintCatalogIdentity, RegisterConstraintKey, RegisterUnitId,
    TargetRegisterEnvironmentIdentity,
};
use omega_selected_instructions::{
    MachineAlternative, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectCatalogIdentity, MachineMemoryEffect, MachineTrapBehavior, SelectedBlockId,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlanIdentity,
    SelectedInstructionProvenance, SelectedMicrosoftX64OwnedIndirectPairLayout,
    StructuralUnitCallEffectDeclaration,
};
use omega_target::NativeTarget;
use psi_core::{FuelScheduleIdentity, MachineId, OperationId};
use psi_terminal::ClaimTransfer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreAllocationMachineEffectIdentity([u8; 32]);

impl PreAllocationMachineEffectIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreAllocationMachineEffectPlan {
    pub identity: PreAllocationMachineEffectIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub target: NativeTarget,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub register_constraints: RegisterConstraintCatalogIdentity,
    pub machine_effect_catalog: MachineEffectCatalogIdentity,
    pub functions: Vec<FunctionMachineEffects>,
    pub structural_unit_functions: Vec<StructuralUnitFunctionMachineEffects>,
}

/// Independently replayable effects for one selected structural-signature
/// Unit function. This remains parallel to the ordinary scalar/VReg roster so
/// it cannot be mistaken for an encoded target alternative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralUnitFunctionMachineEffects {
    pub machine: MachineId,
    pub block: SelectedBlockId,
    pub call: Option<StructuralUnitCallMachineEffects>,
    pub return_instruction: InstructionMachineEffects,
    pub return_effect: omega_optimization_unit::EffectLink,
    pub return_ownership: Vec<omega_optimization_unit::OwnershipEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralUnitCallMachineEffects {
    pub instruction: SelectedInstructionId,
    pub operation: OperationId,
    pub callee: MachineId,
    pub constraint: RegisterConstraintKey,
    pub unit_uses: Vec<RegisterUnitId>,
    pub unit_defs: Vec<RegisterUnitId>,
    pub unit_clobbers: Vec<RegisterUnitId>,
    pub layout: SelectedMicrosoftX64OwnedIndirectPairLayout,
    pub effect: omega_optimization_unit::EffectLink,
    pub ownership: Vec<omega_optimization_unit::OwnershipEvent>,
    pub claim_transfers: Vec<ClaimTransfer>,
    pub provenance: SelectedInstructionProvenance,
    pub declaration: StructuralUnitCallEffectDeclaration,
}

impl PreAllocationMachineEffectPlan {
    pub fn encode(&self) -> Vec<u8> {
        crate::effect_codec::encode_terminal_pre_allocation_machine_effect_plan(self)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, crate::PreAllocationMachineEffectDecodeError> {
        crate::effect_codec::decode_terminal_pre_allocation_machine_effect_plan(encoded)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionMachineEffects {
    pub machine: MachineId,
    pub blocks: Vec<BlockMachineEffects>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockMachineEffects {
    pub block: SelectedBlockId,
    /// Ordinary selected instructions followed by the selected terminator.
    pub instructions: Vec<InstructionMachineEffects>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionMachineEffects {
    pub instruction: SelectedInstructionId,
    pub kind: SelectedInstructionKind,
    pub constraint: RegisterConstraintKey,
    pub unit_uses: Vec<RegisterUnitId>,
    pub unit_defs: Vec<RegisterUnitId>,
    pub unit_clobbers: Vec<RegisterUnitId>,
    pub memory: MachineMemoryEffect,
    pub trap: MachineTrapBehavior,
    pub barrier: MachineBarrier,
    pub call: MachineCallEffect,
    pub cleanup: MachineCleanupEffect,
    pub provenance: SelectedInstructionProvenance,
    pub alternatives: Vec<MachineAlternative>,
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
    plan: PreAllocationMachineEffectPlan,
    receipt: PreAllocationMachineEffectReceipt,
}

impl ValidatedPreAllocationMachineEffects {
    pub const fn plan(&self) -> &PreAllocationMachineEffectPlan {
        &self.plan
    }

    pub const fn receipt(&self) -> PreAllocationMachineEffectReceipt {
        self.receipt
    }

    pub(crate) const fn new(
        plan: PreAllocationMachineEffectPlan,
        receipt: PreAllocationMachineEffectReceipt,
    ) -> Self {
        Self { plan, receipt }
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
    NonCanonicalFunction,
    NonCanonicalBlock,
    InstructionMismatch { instruction: SelectedInstructionId },
    StructuralFunctionMismatch { machine: MachineId },
    StructuralCallMismatch { machine: MachineId },
    IdentityMismatch,
    CountOverflow,
}

impl std::fmt::Display for MachineEffectError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "machine-effect analysis failed: {self:?}")
    }
}

impl std::error::Error for MachineEffectError {}

pub(crate) fn receipt(
    plan: &PreAllocationMachineEffectPlan,
) -> Result<PreAllocationMachineEffectReceipt, MachineEffectError> {
    let ordinary_block_count = plan.functions.iter().try_fold(0usize, |count, function| {
        count
            .checked_add(function.blocks.len())
            .ok_or(MachineEffectError::CountOverflow)
    })?;
    let block_count = ordinary_block_count
        .checked_add(plan.structural_unit_functions.len())
        .ok_or(MachineEffectError::CountOverflow)?;
    let ordinary_instruction_count = plan
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .try_fold(0usize, |count, block| {
            count
                .checked_add(block.instructions.len())
                .ok_or(MachineEffectError::CountOverflow)
        })?;
    let structural_instruction_count =
        plan.structural_unit_functions
            .iter()
            .try_fold(0usize, |count, function| {
                count
                    .checked_add(1 + usize::from(function.call.is_some()))
                    .ok_or(MachineEffectError::CountOverflow)
            })?;
    let instruction_count = ordinary_instruction_count
        .checked_add(structural_instruction_count)
        .ok_or(MachineEffectError::CountOverflow)?;
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
    let (unit_action_count, fuel_settlement_count) =
        plan.structural_unit_functions.iter().try_fold(
            (unit_action_count, fuel_settlement_count),
            |counts, function| {
                let mut actions = function
                    .return_instruction
                    .unit_uses
                    .len()
                    .checked_add(function.return_instruction.unit_defs.len())
                    .and_then(|count| {
                        count.checked_add(function.return_instruction.unit_clobbers.len())
                    })
                    .ok_or(MachineEffectError::CountOverflow)?;
                let mut fuel = function.return_instruction.provenance.fuel.len();
                if let Some(call) = &function.call {
                    actions = actions
                        .checked_add(call.unit_uses.len())
                        .and_then(|count| count.checked_add(call.unit_defs.len()))
                        .and_then(|count| count.checked_add(call.unit_clobbers.len()))
                        .ok_or(MachineEffectError::CountOverflow)?;
                    fuel = fuel
                        .checked_add(call.provenance.fuel.len())
                        .ok_or(MachineEffectError::CountOverflow)?;
                }
                Ok::<_, MachineEffectError>((
                    counts
                        .0
                        .checked_add(actions)
                        .ok_or(MachineEffectError::CountOverflow)?,
                    counts
                        .1
                        .checked_add(fuel)
                        .ok_or(MachineEffectError::CountOverflow)?,
                ))
            },
        )?;
    Ok(PreAllocationMachineEffectReceipt {
        identity: plan.identity,
        selected: plan.selected,
        register_environment: plan.register_environment,
        machine_effect_catalog: plan.machine_effect_catalog,
        function_count: plan
            .functions
            .len()
            .checked_add(plan.structural_unit_functions.len())
            .ok_or(MachineEffectError::CountOverflow)?,
        block_count,
        instruction_count,
        alternative_count,
        unit_action_count,
        fuel_settlement_count,
    })
}
