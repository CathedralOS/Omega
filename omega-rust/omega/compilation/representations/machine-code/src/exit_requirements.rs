//! Checks which require source operands run during encoding. Layout retains
//! their return, trap and effect obligations alongside the encoded rows.
use crate::{WholeFunctionReturnValueEvidence, layout_requirements::EncodedAlternativeDeclaration};
use register_model::RegisterUnitId;
use selected_instructions::{
    MachineAlternativeFamily, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedTrapBehavior, SelectedBlockId, SelectedInstructionId,
};
use semantic_vocabulary::MachineId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WholeFunctionExitContractError {
    OptimizationCustodyMismatch,
    RootMismatch,
    UnsupportedTargetPolicy,
    MissingArchitecturalView(&'static str),
    InvalidConvention,
    DuplicateInstruction(SelectedInstructionId),
    MissingInstruction(SelectedInstructionId),
    FunctionRosterMismatch(MachineId),
    FramePreservationMismatch(MachineId),
    BlockRosterMismatch(SelectedBlockId),
    InstructionRosterMismatch(SelectedInstructionId),
    CalleeSavedWrite {
        instruction: SelectedInstructionId,
        unit: RegisterUnitId,
    },
    LinkRegisterWrite(SelectedInstructionId),
    NonReturnStackEffect(SelectedInstructionId),
    NonReturnMemoryEffect(SelectedInstructionId),
    NonReturnControlEffect(SelectedInstructionId),
    ReturnOperandMismatch(SelectedInstructionId),
    ReturnEncodingMismatch(SelectedInstructionId),
    ReturnEffectsMismatch(SelectedInstructionId),
    ReturnPlacementMismatch(SelectedInstructionId),
    OffsetOverflow,
    ArtifactMismatch,
}

impl std::fmt::Display for WholeFunctionExitContractError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid terminal whole-function exit contract: {self:?}"
        )
    }
}

impl std::error::Error for WholeFunctionExitContractError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedExitRequirements {
    pub functions: Vec<ExitFunctionRequirements>,
}
impl EncodedExitRequirements {
    pub fn function(&self, machine: MachineId) -> Option<&ExitFunctionRequirements> {
        self.functions.iter().find(|f| f.machine == machine)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitFunctionRequirements {
    pub machine: MachineId,
    pub blocks: Vec<ExitBlockRequirements>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitBlockRequirements {
    pub block: SelectedBlockId,
    pub instructions: Vec<ExitRowRequirements>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitRowRequirements {
    pub instruction: SelectedInstructionId,
    pub alternative: EncodedAlternativeDeclaration,
    pub unit_defs: Vec<RegisterUnitId>,
    pub unit_clobbers: Vec<RegisterUnitId>,
    pub source: ExitSourceRequirements,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitSourceRequirements {
    pub id: SelectedInstructionId,
    pub comparison_branch: Option<MachineAlternativeFamily>,
    pub call: Option<selected_instructions::SelectedCallTarget>,
    pub foreign: Option<register_model::RegisterConstraintKey>,
    pub returned: Result<WholeFunctionReturnValueEvidence, WholeFunctionExitContractError>,
    pub trap: Result<TrapRequirements, WholeFunctionExitContractError>,
    pub process_exit: Result<ProcessExitRequirements, WholeFunctionExitContractError>,
    pub non_return: Result<NonReturnRequirements, WholeFunctionExitContractError>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrapRequirements {
    pub control: MachineEncodedControlEffect,
    pub trap: MachineEncodedTrapBehavior,
    pub counter_units: Vec<RegisterUnitId>,
    pub clobbers: Vec<RegisterUnitId>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessExitRequirements {
    Hosted,
    Trap(TrapRequirements),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonReturnRequirements {
    pub effects: MachineEncodedEffects,
    pub address: Option<crate::ResolvedPhysicalAddress>,
}
