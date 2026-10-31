//! Call formals describe types and permissions, not storage inside a callee.
//!
//! Dynamic requirements have no body and therefore no parameter places. Native
//! argument planning reads this shared projection for direct and selected calls;
//! it never fabricates a machine or borrows a table row's private declaration.

use semantic_vocabulary::{ScalarType, StructuralDomainId, StructuralTypeId};
use terminal_psi::{
    BoundaryMachineResult, BoundaryStructuralResultDeclaration, StructuralAccess,
    StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathQualification, StructuralResultDeclaration, TerminalDynamicSlot,
    TerminalDynamicStructuralFormal,
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

impl<'declaration> From<&'declaration StructuralParameterDeclaration>
    for StructuralCallFormal<'declaration>
{
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

impl<'declaration> From<&'declaration TerminalDynamicStructuralFormal>
    for StructuralCallFormal<'declaration>
{
    fn from(parameter: &'declaration TerminalDynamicStructuralFormal) -> Self {
        Self {
            position: parameter.position,
            structural_type: parameter.structural_type,
            multiplicity: parameter.multiplicity,
            access: parameter.access,
            qualifications: &parameter.qualifications,
            projected_qualifications: &[],
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

/// Place-free result contract shared by an ordinary machine and a dynamic slot.
/// Result homes and invocation-local claims remain owned by each actual call.
#[derive(Debug, Clone, Copy)]
pub struct StructuralCallResult<'declaration> {
    pub structural_type: StructuralTypeId,
    pub multiplicity: StructuralMultiplicity,
    pub qualifications: &'declaration [StructuralDomainId],
    pub projected_qualifications: &'declaration [StructuralPathQualification],
}

impl<'declaration> From<&'declaration StructuralResultDeclaration>
    for StructuralCallResult<'declaration>
{
    fn from(result: &'declaration StructuralResultDeclaration) -> Self {
        Self {
            structural_type: result.structural_type,
            multiplicity: result.multiplicity,
            qualifications: &result.qualifications,
            projected_qualifications: &result.projected_qualifications,
        }
    }
}

impl<'declaration> From<&'declaration BoundaryStructuralResultDeclaration>
    for StructuralCallResult<'declaration>
{
    fn from(result: &'declaration BoundaryStructuralResultDeclaration) -> Self {
        Self {
            structural_type: result.structural_type,
            multiplicity: result.multiplicity,
            qualifications: &result.qualifications,
            projected_qualifications: &result.projected_qualifications,
        }
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
            Self::Machine(machine) => machine
                .parameters
                .get(position)
                .map(|parameter| parameter.scalar_type),
            Self::Requirement(requirement) => requirement.scalar_parameters.get(position).copied(),
        }
    }

    pub fn structural_parameter_count(self) -> usize {
        match self {
            Self::Machine(machine) => machine.structural_parameters.len(),
            Self::Requirement(requirement) => requirement.structural_parameters.len(),
        }
    }

    pub fn structural_parameter(
        self,
        position: usize,
    ) -> Option<StructuralCallFormal<'declaration>> {
        match self {
            Self::Machine(machine) => machine.structural_parameters.get(position).map(Into::into),
            Self::Requirement(requirement) => requirement
                .structural_parameters
                .get(position)
                .map(Into::into),
        }
    }

    pub fn is_unit_result(self) -> bool {
        match self {
            Self::Machine(machine) => matches!(machine.result, crate::AbstractFunctionResult::Unit),
            Self::Requirement(requirement) => requirement.result.is_unit(),
        }
    }

    pub fn structural_result(self) -> Option<StructuralCallResult<'declaration>> {
        match self {
            Self::Machine(machine) => machine.result.structural().map(Into::into),
            Self::Requirement(requirement) => match &requirement.result {
                BoundaryMachineResult::Structural(result) => Some(result.into()),
                BoundaryMachineResult::Unit | BoundaryMachineResult::Scalar(_) => None,
            },
        }
    }

    pub fn scalar_result(self) -> Option<ScalarType> {
        match self {
            Self::Machine(machine) => machine.result.scalar().map(|result| result.scalar_type),
            Self::Requirement(requirement) => requirement.result.scalar(),
        }
    }
}

/// The caller-side source a structural call parameter binds: match the
/// parameter's place against the callee's structural parameter roster, take
/// the argument at that position, and re-root it at the formal's path and
/// access.
pub fn call_source(
    structural_parameters: &[StructuralParameterDeclaration],
    arguments: &[StructuralArgument],
    formal: &StructuralArgument,
) -> Option<StructuralArgument> {
    let position =
        terminal_psi::structural_parameter_position(structural_parameters, formal.place)?;
    let mut source = arguments.get(position)?.clone();
    source.path.extend(formal.path.iter().cloned());
    source.access = formal.access;
    Some(source)
}
