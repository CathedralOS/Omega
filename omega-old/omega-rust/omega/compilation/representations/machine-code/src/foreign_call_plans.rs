//! Physical obligations for foreign-call publication. Instruction IDs identify
//! spans that encoding must publish; these records contain no executable graph.

use selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedNormalizedForeignCall,
};
use semantic_vocabulary::MachineId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalForeignCallPlans {
    pub psi: terminal_psi::TerminalPsiIdentity,
    pub target: target::NativeTarget,
    pub physical_identity: [u8; 32],
    pub calls: Vec<PhysicalForeignCallPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalForeignCallPlan {
    pub machine: MachineId,
    pub block: SelectedBlockId,
    pub ordinal: usize,
    pub record: SelectedNormalizedForeignCall,
    pub operand_views: Vec<Option<register_model::RegisterViewId>>,
    pub operand_accesses: Vec<register_model::RegisterOperandAccess>,
    pub operation_ordinal: usize,
    pub caller_live_bytes: u32,
    pub arguments: Vec<ForeignScalarArgumentPlan>,
    pub result: Option<ForeignScalarResultPlan>,
    pub floating_control: Option<ForeignFloatingControlPlan>,
    pub spans: Vec<InstructionSpanObligation>,
}

/// All listed instructions must have unique, contiguous emitted spans, in this
/// order. Empty instruction encodings are allowed unless separately prohibited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionSpanSequence(pub Vec<SelectedInstructionId>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstructionSpanObligation {
    NonEmpty(SelectedInstructionId),
    Before {
        first: SelectedInstructionId,
        second: SelectedInstructionId,
    },
    Contiguous(InstructionSpanSequence),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignScalarArgumentPlan {
    pub parameter_index: u32,
    pub source: crate::InternalUnitScalarArgumentSourceRecord,
    pub placement: calling_conventions::ValuePlacement,
    pub span: InstructionSpanSequence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignScalarResultPlan {
    pub home: crate::UnitScalarHomeRecord,
    pub source: calling_conventions::ValuePlacement,
    pub instruction: SelectedInstructionId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForeignFloatingControlPlan {
    pub saved_slot_byte_offset: u32,
    pub save: SelectedInstructionId,
    pub restore: SelectedInstructionId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForeignCallPlanError {
    TextSizeOverflow,
    InvalidForeignCallProjection {
        caller: MachineId,
        owner: target_vocabulary::CallSiteOwner,
        reason: &'static str,
    },
    InvalidForeignCallArgument {
        caller: MachineId,
        owner: target_vocabulary::CallSiteOwner,
        import: Option<String>,
        parameter: Option<u32>,
        reason: &'static str,
        transport: Option<String>,
    },
    ForeignCallOwnerNotInProvenance {
        caller: MachineId,
        owner: target_vocabulary::CallSiteOwner,
    },
    DuplicateForeignCallOwner {
        caller: MachineId,
        owner: target_vocabulary::CallSiteOwner,
    },
}
