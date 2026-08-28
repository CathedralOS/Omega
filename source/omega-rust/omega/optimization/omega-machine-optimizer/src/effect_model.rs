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
    TerminalSelectedMicrosoftX64OwnedIndirectPairLayout,
    TerminalStructuralUnitCallEffectDeclaration,
};
use psi_core::{FuelScheduleIdentity, MachineId, OperationId};
use psi_terminal::ClaimTransfer;

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
    pub structural_unit_functions: Vec<TerminalStructuralUnitFunctionMachineEffects>,
}

/// Independently replayable effects for one selected structural-signature
/// Unit function. This remains parallel to the ordinary scalar/VReg roster so
/// it cannot be mistaken for an encoded target alternative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalStructuralUnitFunctionMachineEffects {
    pub machine: MachineId,
    pub block: TerminalSelectedBlockId,
    pub call: Option<TerminalStructuralUnitCallMachineEffects>,
    pub return_instruction: TerminalInstructionMachineEffects,
    pub return_effect: omega_optimization_unit::EffectLink,
    pub return_ownership: Vec<omega_optimization_unit::OwnershipEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalStructuralUnitCallMachineEffects {
    pub instruction: TerminalSelectedInstructionId,
    pub operation: OperationId,
    pub callee: MachineId,
    pub constraint: RegisterConstraintKey,
    pub unit_uses: Vec<RegisterUnitId>,
    pub unit_defs: Vec<RegisterUnitId>,
    pub unit_clobbers: Vec<RegisterUnitId>,
    pub layout: TerminalSelectedMicrosoftX64OwnedIndirectPairLayout,
    pub effect: omega_optimization_unit::EffectLink,
    pub ownership: Vec<omega_optimization_unit::OwnershipEvent>,
    pub claim_transfers: Vec<ClaimTransfer>,
    pub provenance: TerminalSelectedInstructionProvenance,
    pub declaration: TerminalStructuralUnitCallEffectDeclaration,
}

impl TerminalPreAllocationMachineEffectPlan {
    pub fn encode(&self) -> Vec<u8> {
        crate::effect_codec::encode_terminal_pre_allocation_machine_effect_plan(self)
    }

    pub fn decode(
        encoded: &[u8],
    ) -> Result<Self, crate::TerminalPreAllocationMachineEffectDecodeError> {
        crate::effect_codec::decode_terminal_pre_allocation_machine_effect_plan(encoded)
    }
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
    StructuralFunctionMismatch {
        machine: MachineId,
    },
    StructuralCallMismatch {
        machine: MachineId,
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
    let ordinary_block_count = plan.functions.iter().try_fold(0usize, |count, function| {
        count
            .checked_add(function.blocks.len())
            .ok_or(TerminalMachineEffectError::CountOverflow)
    })?;
    let block_count = ordinary_block_count
        .checked_add(plan.structural_unit_functions.len())
        .ok_or(TerminalMachineEffectError::CountOverflow)?;
    let ordinary_instruction_count = plan
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .try_fold(0usize, |count, block| {
            count
                .checked_add(block.instructions.len())
                .ok_or(TerminalMachineEffectError::CountOverflow)
        })?;
    let structural_instruction_count =
        plan.structural_unit_functions
            .iter()
            .try_fold(0usize, |count, function| {
                count
                    .checked_add(1 + usize::from(function.call.is_some()))
                    .ok_or(TerminalMachineEffectError::CountOverflow)
            })?;
    let instruction_count = ordinary_instruction_count
        .checked_add(structural_instruction_count)
        .ok_or(TerminalMachineEffectError::CountOverflow)?;
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
                    .ok_or(TerminalMachineEffectError::CountOverflow)?;
                let mut fuel = function.return_instruction.provenance.fuel.len();
                if let Some(call) = &function.call {
                    actions = actions
                        .checked_add(call.unit_uses.len())
                        .and_then(|count| count.checked_add(call.unit_defs.len()))
                        .and_then(|count| count.checked_add(call.unit_clobbers.len()))
                        .ok_or(TerminalMachineEffectError::CountOverflow)?;
                    fuel = fuel
                        .checked_add(call.provenance.fuel.len())
                        .ok_or(TerminalMachineEffectError::CountOverflow)?;
                }
                Ok::<_, TerminalMachineEffectError>((
                    counts
                        .0
                        .checked_add(actions)
                        .ok_or(TerminalMachineEffectError::CountOverflow)?,
                    counts
                        .1
                        .checked_add(fuel)
                        .ok_or(TerminalMachineEffectError::CountOverflow)?,
                ))
            },
        )?;
    Ok(TerminalPreAllocationMachineEffectReceipt {
        identity: plan.identity,
        selected: plan.selected,
        register_environment: plan.register_environment,
        machine_effect_catalog: plan.machine_effect_catalog,
        function_count: plan
            .functions
            .len()
            .checked_add(plan.structural_unit_functions.len())
            .ok_or(TerminalMachineEffectError::CountOverflow)?,
        block_count,
        instruction_count,
        alternative_count,
        unit_action_count,
        fuel_settlement_count,
    })
}
