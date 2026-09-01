//! Typed vocabulary for the exact projected structural call/return replay.

use omega_calling_conventions::ValueShape;
use psi_core::MachineId;
use psi_terminal::{
    StructuralOperationResult, StructuralParameterDeclaration, StructuralPathQualification,
    StructuralResultDeclaration, StructuralTypeDeclaration,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuralCallReturnRosterLocation {
    CallerParameter,
    CallerOperationResult,
    CallerFunctionResult,
    CalleeParameter,
    CalleeSource,
    CalleeFunctionResult,
    TargetParameter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuralCallReturnProjectedQualificationValidationError {
    SourceShape,
    SourceRosterMismatch(StructuralCallReturnRosterLocation),
    SourceRosterNotCanonical(StructuralCallReturnRosterLocation),
    TargetShape,
    TargetMachineMismatch,
    TargetCalleeMismatch,
    TargetRosterMismatch(StructuralCallReturnRosterLocation),
    TargetRosterNotCanonical(StructuralCallReturnRosterLocation),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralCallReturnProjectedQualificationReceipt {
    caller: MachineId,
    callee: MachineId,
    projected_qualifications: Vec<StructuralPathQualification>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralCallReturnCallerTranslationReceipt {
    machine: MachineId,
    callee: MachineId,
    projected_qualifications: Vec<StructuralPathQualification>,
}

impl StructuralCallReturnCallerTranslationReceipt {
    pub(super) fn new(
        machine: MachineId,
        callee: MachineId,
        projected_qualifications: Vec<StructuralPathQualification>,
    ) -> Self {
        Self {
            machine,
            callee,
            projected_qualifications,
        }
    }

    pub const fn machine(&self) -> MachineId {
        self.machine
    }
    pub const fn callee(&self) -> MachineId {
        self.callee
    }
    pub fn projected_qualifications(&self) -> &[StructuralPathQualification] {
        &self.projected_qualifications
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralParameterReturnCalleeTranslationReceipt {
    machine: MachineId,
    projected_qualifications: Vec<StructuralPathQualification>,
}

impl StructuralParameterReturnCalleeTranslationReceipt {
    pub(super) fn new(
        machine: MachineId,
        projected_qualifications: Vec<StructuralPathQualification>,
    ) -> Self {
        Self {
            machine,
            projected_qualifications,
        }
    }

    pub const fn machine(&self) -> MachineId {
        self.machine
    }
    pub fn projected_qualifications(&self) -> &[StructuralPathQualification] {
        &self.projected_qualifications
    }
}

impl StructuralCallReturnProjectedQualificationReceipt {
    pub(in crate::validation) fn new(
        caller: MachineId,
        callee: MachineId,
        projected_qualifications: Vec<StructuralPathQualification>,
    ) -> Self {
        Self {
            caller,
            callee,
            projected_qualifications,
        }
    }

    pub const fn caller(&self) -> MachineId {
        self.caller
    }

    pub const fn callee(&self) -> MachineId {
        self.callee
    }

    pub fn projected_qualifications(&self) -> &[StructuralPathQualification] {
        &self.projected_qualifications
    }
}

pub(super) struct StructuralCallReturnSource {
    pub(super) caller: MachineId,
    pub(super) callee: MachineId,
    pub(super) roster: Vec<StructuralPathQualification>,
    pub(super) caller_parameter: StructuralParameterDeclaration,
    pub(super) caller_operation_result: StructuralOperationResult,
    pub(super) caller_result: StructuralResultDeclaration,
    pub(super) callee_parameter: StructuralParameterDeclaration,
    pub(super) callee_result: StructuralResultDeclaration,
    pub(super) structural_types: Vec<StructuralTypeDeclaration>,
    pub(super) shape: ValueShape,
}

pub(super) fn is_canonical(rows: &[StructuralPathQualification]) -> bool {
    !rows.is_empty()
        && rows.iter().all(|row| !row.path.is_empty())
        && rows.windows(2).all(|pair| pair[0] < pair[1])
}
