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
    pub encoding: ExitEncoding,
    pub abi: register_environment::TargetExitAbi,
    pub functions: Vec<ExitFunctionRequirements>,
}
/// The exit checker compares final layout bytes and register effects with the
/// encoder's declarations. The byte and footprint arenas are immutable and shared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitEncoding {
    pub machine: std::sync::Arc<post_allocation_machine::PostAllocationMachinePlan>,
    pub identity: crate::SelectedFormEncodingIdentity,
    pub frame: Option<std::sync::Arc<crate::TargetFrameLayoutPlan>>,
    pub decoded: std::sync::Arc<crate::SelectedFormEncodedRowStore>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitInstructionDisposition {
    Retained,
    Elided,
    FusedBranch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExitInstructionForm {
    Encoded,
    InternalCall(crate::SelectedFormInternalMachineFixup),
    ForeignCall(Box<crate::SelectedFormNormalizedForeignCallFixup>),
    DeferredControl,
    ImageAddress,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitInstructionEncoding {
    pub alternative: selected_instructions::MachineAlternativeKey,
    pub disposition: ExitInstructionDisposition,
    pub form: ExitInstructionForm,
    pub address: Option<Box<crate::ResolvedPhysicalAddress>>,
    pub code: crate::SelectedFormEncodedByteSpan,
    pub footprint: crate::SelectedFormDecodedFootprintHandle,
}

impl ExitInstructionEncoding {
    pub fn from_row(row: &crate::SelectedFormEncodingRow) -> Self {
        use crate::{
            SelectedFormEncodingState as State, SelectedFormMachineDisposition as Disposition,
        };
        let form = match &row.state {
            State::Encoded => ExitInstructionForm::Encoded,
            State::DeferredControl { .. } => ExitInstructionForm::DeferredControl,
            State::UnresolvedInternalMachineCall { fixup } => {
                ExitInstructionForm::InternalCall(**fixup)
            }
            State::UnresolvedNormalizedForeignCall { fixup } => {
                ExitInstructionForm::ForeignCall(fixup.clone())
            }
            State::UnresolvedImageAddress { .. } => ExitInstructionForm::ImageAddress,
        };
        let disposition = match row.machine_disposition {
            Disposition::RetainedV1 => ExitInstructionDisposition::Retained,
            Disposition::Aarch64ElidedCompareI64ZeroV1 { .. }
            | Disposition::Aarch64ElidedSameViewCopyI64V1 { .. } => {
                ExitInstructionDisposition::Elided
            }
            Disposition::Aarch64FusedBranchNonZeroToCbnzV1 { .. } => {
                ExitInstructionDisposition::FusedBranch
            }
        };
        Self {
            alternative: row.alternative,
            form,
            disposition,
            address: row.address.clone(),
            code: row.code,
            footprint: row.footprint,
        }
    }

    pub fn decoded<'a>(
        &self,
        store: &'a crate::SelectedFormEncodedRowStore,
    ) -> Option<(&'a [u8], &'a crate::SelectedFormDecodedFootprint)> {
        Some((store.bytes(self.code), store.footprint(self.footprint)?))
    }
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
    pub encoding: ExitInstructionEncoding,
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
