use omega_optimization_core::PostAllocationOptimizationManifestIdentity;
use omega_regalloc::{
    TerminalAllocationLegalityIdentity, TerminalLiveRangeIdentity, TerminalRegisterHomeIdentity,
};
use omega_register_model::{
    PhysicalRegisterModelIdentity, RegisterClassId, RegisterConstraintCatalogIdentity,
    RegisterOperandAccess, RegisterUnitId, RegisterViewId, RegisterWriteSemantics,
    TargetRegisterEnvironmentIdentity,
};
use omega_target::NativeTarget;
use omega_terminal_selected_instructions::{
    TerminalMachineAlternative, TerminalMachineEffectCatalogIdentity, TerminalSelectedBlockId,
    TerminalSelectedInstructionId, TerminalSelectedInstructionPlanIdentity,
    TerminalVirtualRegisterId,
};
use psi_core::MachineId;

use crate::TerminalPreAllocationMachineEffectIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalPostAllocationMachineIdentity([u8; 32]);

impl TerminalPostAllocationMachineIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// This is a legality rule, not an optimization level or cost policy. Current
/// target catalogs must partition physical-home configurations so exactly one
/// declared alternative applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalMachineAlternativeChoiceRule {
    UniqueApplicableInCatalogOrderV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalPostAllocationMachinePlan {
    pub identity: TerminalPostAllocationMachineIdentity,
    pub selected: TerminalSelectedInstructionPlanIdentity,
    pub effects: TerminalPreAllocationMachineEffectIdentity,
    pub ranges: TerminalLiveRangeIdentity,
    pub legality: TerminalAllocationLegalityIdentity,
    pub homes: TerminalRegisterHomeIdentity,
    pub post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    pub target: NativeTarget,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub physical_register_model: PhysicalRegisterModelIdentity,
    pub register_constraints: RegisterConstraintCatalogIdentity,
    pub machine_effect_catalog: TerminalMachineEffectCatalogIdentity,
    pub choice_rule: TerminalMachineAlternativeChoiceRule,
    pub functions: Vec<TerminalPostAllocationMachineFunction>,
}

impl TerminalPostAllocationMachinePlan {
    /// Encodes this unchecked plan in the strict, self-authenticating artifact
    /// envelope. This does not grant validation or emission authority.
    pub fn encode(&self) -> Vec<u8> {
        crate::alternative_codec::encode_terminal_post_allocation_machine_plan(self)
    }

    /// Decodes and content-authenticates an unchecked plan. Call
    /// [`crate::validate_terminal_post_allocation_machine_plan`] before use.
    pub fn decode(encoded: &[u8]) -> Result<Self, crate::TerminalPostAllocationMachineDecodeError> {
        crate::alternative_codec::decode_terminal_post_allocation_machine_plan(encoded)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalPostAllocationMachineFunction {
    pub machine: MachineId,
    pub blocks: Vec<TerminalPostAllocationMachineBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalPostAllocationMachineBlock {
    pub block: TerminalSelectedBlockId,
    /// Ordinary selected instructions followed by the selected terminator.
    pub instructions: Vec<TerminalPostAllocationMachineInstruction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalPostAllocationMachineInstruction {
    pub instruction: TerminalSelectedInstructionId,
    pub alternative: TerminalMachineAlternative,
    pub operands: Vec<TerminalPhysicalOperandFootprint>,
    pub implicit_unit_uses: Vec<RegisterUnitId>,
    pub implicit_unit_defs: Vec<RegisterUnitId>,
    pub implicit_unit_clobbers: Vec<RegisterUnitId>,
    pub unit_uses: Vec<RegisterUnitId>,
    pub unit_defs: Vec<RegisterUnitId>,
    pub unit_clobbers: Vec<RegisterUnitId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalPhysicalOperandFootprint {
    pub operand: u16,
    pub virtual_register: TerminalVirtualRegisterId,
    pub class: RegisterClassId,
    pub view: RegisterViewId,
    pub access: RegisterOperandAccess,
    pub storage_units: Vec<RegisterUnitId>,
    pub read_units: Vec<RegisterUnitId>,
    pub write_units: Vec<RegisterUnitId>,
    pub write_semantics: Option<RegisterWriteSemantics>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalPostAllocationMachineReceipt {
    identity: TerminalPostAllocationMachineIdentity,
    selected: TerminalSelectedInstructionPlanIdentity,
    effects: TerminalPreAllocationMachineEffectIdentity,
    homes: TerminalRegisterHomeIdentity,
    post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    register_environment: TargetRegisterEnvironmentIdentity,
    function_count: usize,
    block_count: usize,
    instruction_count: usize,
    operand_count: usize,
    unit_action_count: usize,
}

impl TerminalPostAllocationMachineReceipt {
    pub const fn identity(self) -> TerminalPostAllocationMachineIdentity {
        self.identity
    }
    pub const fn selected(self) -> TerminalSelectedInstructionPlanIdentity {
        self.selected
    }
    pub const fn effects(self) -> TerminalPreAllocationMachineEffectIdentity {
        self.effects
    }
    pub const fn homes(self) -> TerminalRegisterHomeIdentity {
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
pub struct ValidatedTerminalPostAllocationMachinePlan {
    plan: TerminalPostAllocationMachinePlan,
    receipt: TerminalPostAllocationMachineReceipt,
}

impl ValidatedTerminalPostAllocationMachinePlan {
    pub const fn plan(&self) -> &TerminalPostAllocationMachinePlan {
        &self.plan
    }

    pub const fn receipt(&self) -> TerminalPostAllocationMachineReceipt {
        self.receipt
    }

    pub(crate) const fn new(
        plan: TerminalPostAllocationMachinePlan,
        receipt: TerminalPostAllocationMachineReceipt,
    ) -> Self {
        Self { plan, receipt }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalPostAllocationMachineError {
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
    UnsupportedStructuralUnitFunctions,
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

impl std::fmt::Display for TerminalPostAllocationMachineError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid post-allocation machine sidecar: {self:?}"
        )
    }
}

impl std::error::Error for TerminalPostAllocationMachineError {}

pub(crate) fn post_allocation_receipt(
    plan: &TerminalPostAllocationMachinePlan,
) -> Result<TerminalPostAllocationMachineReceipt, TerminalPostAllocationMachineError> {
    let block_count = plan.functions.iter().try_fold(0_usize, |count, function| {
        count
            .checked_add(function.blocks.len())
            .ok_or(TerminalPostAllocationMachineError::CountOverflow)
    })?;
    let (instruction_count, operand_count, unit_action_count) = plan
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.instructions)
        .try_fold((0_usize, 0_usize, 0_usize), |counts, instruction| {
            Ok::<_, TerminalPostAllocationMachineError>((
                counts
                    .0
                    .checked_add(1)
                    .ok_or(TerminalPostAllocationMachineError::CountOverflow)?,
                counts
                    .1
                    .checked_add(instruction.operands.len())
                    .ok_or(TerminalPostAllocationMachineError::CountOverflow)?,
                counts
                    .2
                    .checked_add(instruction.unit_uses.len())
                    .and_then(|count| count.checked_add(instruction.unit_defs.len()))
                    .and_then(|count| count.checked_add(instruction.unit_clobbers.len()))
                    .ok_or(TerminalPostAllocationMachineError::CountOverflow)?,
            ))
        })?;
    Ok(TerminalPostAllocationMachineReceipt {
        identity: plan.identity,
        selected: plan.selected,
        effects: plan.effects,
        homes: plan.homes,
        post_allocation_manifest: plan.post_allocation_manifest,
        register_environment: plan.register_environment,
        function_count: plan.functions.len(),
        block_count,
        instruction_count,
        operand_count,
        unit_action_count,
    })
}
