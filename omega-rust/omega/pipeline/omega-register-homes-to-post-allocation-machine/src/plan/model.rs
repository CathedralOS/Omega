use omega_optimization_core::PostAllocationOptimizationManifestIdentity;
use omega_physical_instructions::{PostAllocationMachineIdentity, PostAllocationMachinePlan};
use omega_register_model::TargetRegisterEnvironmentIdentity;
use omega_selected_instructions::PreAllocationMachineEffectIdentity;
use omega_selected_instructions::SelectedInstructionPlanIdentity;
use omega_selected_instructions_to_register_homes::RegisterHomeIdentity;
use psi_core::MachineId;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostAllocationMachineReceipt {
    identity: PostAllocationMachineIdentity,
    selected: SelectedInstructionPlanIdentity,
    effects: PreAllocationMachineEffectIdentity,
    homes: RegisterHomeIdentity,
    post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    register_environment: TargetRegisterEnvironmentIdentity,
    function_count: usize,
    block_count: usize,
    instruction_count: usize,
    operand_count: usize,
    unit_action_count: usize,
}

impl PostAllocationMachineReceipt {
    pub const fn identity(self) -> PostAllocationMachineIdentity {
        self.identity
    }
    pub const fn selected(self) -> SelectedInstructionPlanIdentity {
        self.selected
    }
    pub const fn effects(self) -> PreAllocationMachineEffectIdentity {
        self.effects
    }
    pub const fn homes(self) -> RegisterHomeIdentity {
        self.homes
    }
    pub const fn post_allocation_manifest(self) -> PostAllocationOptimizationManifestIdentity {
        self.post_allocation_manifest
    }
    pub const fn register_environment(self) -> TargetRegisterEnvironmentIdentity {
        self.register_environment
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
    pub const fn operand_count(self) -> usize {
        self.operand_count
    }
    pub const fn unit_action_count(self) -> usize {
        self.unit_action_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPostAllocationMachinePlan {
    plan: Arc<PostAllocationMachinePlan>,
    receipt: PostAllocationMachineReceipt,
}

impl ValidatedPostAllocationMachinePlan {
    pub fn plan(&self) -> &PostAllocationMachinePlan {
        &self.plan
    }

    /// Retain the original immutable program without retaining its producer.
    /// The raw data do not carry this wrapper's admission authority.
    pub fn shared_plan(&self) -> Arc<PostAllocationMachinePlan> {
        Arc::clone(&self.plan)
    }

    pub const fn receipt(&self) -> PostAllocationMachineReceipt {
        self.receipt
    }

    pub(crate) fn new(
        plan: PostAllocationMachinePlan,
        receipt: PostAllocationMachineReceipt,
    ) -> Self {
        Self {
            plan: Arc::new(plan),
            receipt,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PostAllocationMachineError {
    TargetMismatch,
    SelectedRootMismatch,
    EffectRootMismatch,
    RangeRootMismatch,
    LegalityRootMismatch,
    HomeRootMismatch,
    RegisterEnvironmentMismatch,
    PhysicalRegisterModelMismatch,
    RegisterConstraintCatalogMismatch,
    PostAllocationManifestMismatch,
    OptimizationUnitMismatch,
    FuelScheduleMismatch,
    StructuralFunctionMismatch {
        machine: MachineId,
    },
    StructuralAllocationMismatch {
        machine: MachineId,
    },
    FunctionMismatch {
        function: usize,
    },
    BlockMismatch {
        function: usize,
        block: usize,
    },
    InstructionMismatch {
        function: usize,
        instruction: u32,
    },
    MissingHome {
        function: usize,
        register: u32,
    },
    UnknownView {
        function: usize,
        register: u32,
        view: u16,
    },
    HomeClassMismatch {
        function: usize,
        register: u32,
    },
    MissingApplicabilityOperand {
        instruction: u32,
        operand: u16,
    },
    NoApplicableAlternative {
        instruction: u32,
    },
    AmbiguousApplicableAlternatives {
        instruction: u32,
    },
    IdentityMismatch,
    CountOverflow,
}

impl std::fmt::Display for PostAllocationMachineError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid post-allocation machine sidecar: {self:?}"
        )
    }
}

impl std::error::Error for PostAllocationMachineError {}

pub(crate) fn post_allocation_receipt(
    plan: &PostAllocationMachinePlan,
) -> Result<PostAllocationMachineReceipt, PostAllocationMachineError> {
    let ordinary_block_count = plan.functions.iter().try_fold(0_usize, |count, function| {
        count
            .checked_add(function.blocks.len())
            .ok_or(PostAllocationMachineError::CountOverflow)
    })?;
    let block_count = ordinary_block_count
        .checked_add(plan.structural_unit_functions.len())
        .ok_or(PostAllocationMachineError::CountOverflow)?;
    let (ordinary_instruction_count, operand_count, ordinary_unit_action_count) = plan
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.instructions)
        .try_fold((0_usize, 0_usize, 0_usize), |counts, instruction| {
            Ok::<_, PostAllocationMachineError>((
                counts
                    .0
                    .checked_add(1)
                    .ok_or(PostAllocationMachineError::CountOverflow)?,
                counts
                    .1
                    .checked_add(instruction.operands.len())
                    .ok_or(PostAllocationMachineError::CountOverflow)?,
                counts
                    .2
                    .checked_add(instruction.unit_uses.len())
                    .and_then(|count| count.checked_add(instruction.unit_defs.len()))
                    .and_then(|count| count.checked_add(instruction.unit_clobbers.len()))
                    .ok_or(PostAllocationMachineError::CountOverflow)?,
            ))
        })?;
    let structural_instruction_count =
        plan.structural_unit_functions
            .iter()
            .try_fold(0_usize, |count, function| {
                count
                    .checked_add(1 + usize::from(function.call.is_some()))
                    .ok_or(PostAllocationMachineError::CountOverflow)
            })?;
    let structural_unit_action_count =
        plan.structural_unit_functions
            .iter()
            .try_fold(0_usize, |count, function| {
                let count = count
                    .checked_add(function.return_instruction.unit_uses.len())
                    .and_then(|count| {
                        count.checked_add(function.return_instruction.unit_defs.len())
                    })
                    .and_then(|count| {
                        count.checked_add(function.return_instruction.unit_clobbers.len())
                    })
                    .ok_or(PostAllocationMachineError::CountOverflow)?;
                function.call.as_ref().map_or(Ok(count), |call| {
                    count
                        .checked_add(call.unit_uses.len())
                        .and_then(|count| count.checked_add(call.unit_defs.len()))
                        .and_then(|count| count.checked_add(call.unit_clobbers.len()))
                        .ok_or(PostAllocationMachineError::CountOverflow)
                })
            })?;
    Ok(PostAllocationMachineReceipt {
        identity: plan.identity,
        selected: plan.selected,
        effects: plan.effects,
        homes: plan.homes,
        post_allocation_manifest: plan.post_allocation_manifest,
        register_environment: plan.register_environment,
        function_count: plan
            .functions
            .len()
            .checked_add(plan.structural_unit_functions.len())
            .ok_or(PostAllocationMachineError::CountOverflow)?,
        block_count,
        instruction_count: ordinary_instruction_count
            .checked_add(structural_instruction_count)
            .ok_or(PostAllocationMachineError::CountOverflow)?,
        operand_count,
        unit_action_count: ordinary_unit_action_count
            .checked_add(structural_unit_action_count)
            .ok_or(PostAllocationMachineError::CountOverflow)?,
    })
}
