//! Call formals describe types and permissions, not storage inside a callee.
//!
//! Dynamic requirements have no body and therefore no parameter places. Native
//! argument planning reads this shared projection for direct and selected calls;
//! it never fabricates a machine or borrows a table row's private declaration.

use semantic_vocabulary::{ScalarType, StructuralDomainId, StructuralTypeId};
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathQualification, TerminalDynamicSlot, TerminalDynamicStructuralFormal,
};

use crate::AbstractFunction;

#[derive(Debug, Clone, Copy)]
pub struct StructuralCallFormal<'declaration> {
    pub position: u32,
    pub structural_type: StructuralTypeId,
    pub multiplicity: StructuralMultiplicity,
    pub access: StructuralAccess,
    pub qualifications: &'declaration [StructuralDomainId],
    pub projected_qualifications: &'declaration [StructuralPathQualification],
}

impl<'declaration> From<&'declaration StructuralParameterDeclaration> for StructuralCallFormal<'declaration> {
    fn from(parameter: &'declaration StructuralParameterDeclaration) -> Self {
        Self {
            position: parameter.position,
            structural_type: parameter.structural_type,
            multiplicity: parameter.multiplicity,
            access: parameter.access,
            qualifications: &parameter.qualifications,
            projected_qualifications: &parameter.projected_qualifications,
        }
    }
}

impl<'declaration> From<&'declaration TerminalDynamicStructuralFormal> for StructuralCallFormal<'declaration> {
    fn from(parameter: &'declaration TerminalDynamicStructuralFormal) -> Self {
        Self {
            position: parameter.position,
            structural_type: parameter.structural_type,
            multiplicity: parameter.multiplicity,
            access: parameter.access,
            qualifications: &[],
            projected_qualifications: &[],
        }
    }
}

impl<'declaration> From<&StructuralCallFormal<'declaration>> for StructuralCallFormal<'declaration> {
    fn from(parameter: &StructuralCallFormal<'declaration>) -> Self {
        *parameter
    }
}

#[derive(Clone, Copy)]
pub enum AbstractCallSignature<'declaration> {
    Machine(&'declaration AbstractFunction),
    Requirement(&'declaration TerminalDynamicSlot),
}

impl<'declaration> AbstractCallSignature<'declaration> {
    pub fn scalar_parameter_count(self) -> usize {
        match self {
            Self::Machine(machine) => machine.parameters.len(),
            Self::Requirement(requirement) => requirement.scalar_parameters.len(),
        }
    }

    pub fn scalar_parameter(self, position: usize) -> Option<ScalarType> {
        match self {
            Self::Machine(machine) => machine.parameters.get(position).map(|parameter| parameter.scalar_type),
            Self::Requirement(requirement) => requirement.scalar_parameters.get(position).copied(),
        }
    }

    pub fn structural_parameter_count(self) -> usize {
        match self {
            Self::Machine(machine) => machine.structural_parameters.len(),
            Self::Requirement(requirement) => requirement.structural_parameters.len(),
        }
    }

    pub fn structural_parameter(self, position: usize) -> Option<StructuralCallFormal<'declaration>> {
        match self {
            Self::Machine(machine) => machine.structural_parameters.get(position).map(Into::into),
            Self::Requirement(requirement) => requirement.structural_parameters.get(position).map(Into::into),
        }
    }

    pub fn scalar_result(self) -> Option<ScalarType> {
        match self {
            Self::Machine(machine) => machine.result.scalar().map(|result| result.scalar_type),
            Self::Requirement(requirement) => requirement.result,
        }
    }
}
