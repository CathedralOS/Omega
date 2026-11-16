//! Structural argument contracts shared by native forms.

use semantic_vocabulary::{StructuralDomainId, StructuralTypeId};
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity,
    StructuralParameterDeclaration as ParameterDeclaration, StructuralPathQualification,
    TerminalDynamicStructuralFormal as StructuralFormal,
};

#[derive(Debug, Clone, Copy)]
pub struct StructuralCallFormal<'declaration> {
    pub position: u32,
    pub structural_type: StructuralTypeId,
    pub multiplicity: StructuralMultiplicity,
    pub access: StructuralAccess,
    pub qualifications: &'declaration [StructuralDomainId],
    pub projected_qualifications: &'declaration [StructuralPathQualification],
    pub additional_input_qualifications: &'declaration [StructuralPathQualification],
}

impl<'declaration> From<&'declaration ParameterDeclaration> for StructuralCallFormal<'declaration> {
    fn from(parameter: &'declaration ParameterDeclaration) -> Self {
        Self {
            position: parameter.position,
            structural_type: parameter.structural_type,
            multiplicity: parameter.multiplicity,
            access: parameter.access,
            qualifications: &parameter.qualifications,
            projected_qualifications: &parameter.projected_qualifications,
            additional_input_qualifications: &parameter.additional_input_qualifications,
        }
    }
}

impl<'declaration> From<&'declaration StructuralFormal> for StructuralCallFormal<'declaration> {
    fn from(parameter: &'declaration StructuralFormal) -> Self {
        Self {
            position: parameter.position,
            structural_type: parameter.structural_type,
            multiplicity: parameter.multiplicity,
            access: parameter.access,
            qualifications: &parameter.qualifications,
            projected_qualifications: &[],
            additional_input_qualifications: &[],
        }
    }
}

impl<'declaration> From<&StructuralCallFormal<'declaration>>
    for StructuralCallFormal<'declaration>
{
    fn from(parameter: &StructuralCallFormal<'declaration>) -> Self {
        *parameter
    }
}
