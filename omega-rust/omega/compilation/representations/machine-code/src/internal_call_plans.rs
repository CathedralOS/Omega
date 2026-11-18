//! Physical argument and result obligations for published internal calls.
//! Byte positions are resolved only after layout; source graphs are not retained.

use crate::{
    InternalUnitCallSource, InternalUnitScalarArgumentSourceRecord,
    InternalUnitStructuralArgumentSourceRecord, StructuralSourceLocation, UnitParameterHomeRecord,
    UnitParameterRecord,
};
use calling_conventions::{ValuePlacement, ValueShape};
use selected_instructions::SelectedInstructionId;
use semantic_vocabulary::{MachineId, PlaceId, StructuralTypeId};
use terminal_psi::{ClaimTransfer, StructuralAccess, StructuralPathSegment};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InternalCallPlanError {
    Mismatch(&'static str),
    Unsupported(&'static str),
    Overflow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalInternalCallPlans {
    pub functions: Vec<PhysicalFunctionInternalCalls>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalFunctionInternalCalls {
    pub machine: MachineId,
    pub unpublished_calls: Vec<SelectedInstructionId>,
    pub publication: Result<InternalCallPublication, InternalCallPlanError>,
}

impl PhysicalInternalCallPlans {
    pub fn function(
        &self,
        machine: MachineId,
    ) -> Result<&PhysicalFunctionInternalCalls, InternalCallPlanError> {
        let mut rows = self.functions.iter().filter(|row| row.machine == machine);
        let row = rows.next().ok_or(InternalCallPlanError::Mismatch(
            "calls have no physical function",
        ))?;
        if rows.next().is_some() {
            return Err(InternalCallPlanError::Mismatch(
                "calls have duplicate physical functions",
            ));
        }
        Ok(row)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternalCallPublication {
    pub parameters: Vec<UnitParameterRecord>,
    pub homes: Vec<UnitParameterHomeRecord>,
    pub calls: Vec<PhysicalInternalCall>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalInternalCall {
    pub instruction: SelectedInstructionId,
    pub source: InternalUnitCallSource,
    pub owner: target_vocabulary::CallSiteOwner,
    pub target: MachineId,
    pub semantic_result: Option<native_vocabulary::values::AbstractResult>,
    pub scalar_arguments: Vec<PhysicalScalarCallArgument>,
    pub arguments: Vec<PhysicalStructuralCallArgument>,
    pub claim_transfers: Vec<ClaimTransfer>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalScalarCallArgument {
    pub parameter_index: u32,
    pub source: InternalUnitScalarArgumentSourceRecord,
    pub destination: ValuePlacement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalArgumentSpan {
    Call,
    Copy {
        extent: Vec<SelectedInstructionId>,
        transfers: Vec<SelectedInstructionId>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalStructuralCallArgument {
    pub place: PlaceId,
    pub access: StructuralAccess,
    pub path: Vec<StructuralPathSegment>,
    pub root_structural_type: StructuralTypeId,
    pub structural_type: StructuralTypeId,
    pub shape: ValueShape,
    pub source_byte_offset: u32,
    pub source_location: StructuralSourceLocation,
    pub call_stack_bytes: u32,
    pub fixed_array_length: Option<u64>,
    pub element_stride: Option<u32>,
    pub source: InternalUnitStructuralArgumentSourceRecord,
    pub destination: ValuePlacement,
    pub span: PhysicalArgumentSpan,
}
