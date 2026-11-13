/*
Argument planning needs public formals even when code is selected at runtime.
Named machines, table slots and machine values expose them through
`AbstractCallSignature`; indirect calls never borrow a body's private signature
or invent a machine declaration. Structural formals and results use the same
view across these targets.

Access-view inputs stay separate from ordinary structural parameters so
pointer-shaped `Access<P, T>` views are never mistaken for owned backing.
*/

use crate::{AbstractFunction, AbstractOperationPlan, AbstractResult};
use semantic_vocabulary::{MachineId, ScalarType, StructuralDomainId, StructuralTypeId, ValueId};
use terminal_psi::{
    BoundaryMachineResult, BoundaryStructuralResultDeclaration as ResultDeclaration,
    OutcomeSpecificCallEvidence, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    StructuralOperationResult, StructuralParameterDeclaration as ParameterDeclaration,
    StructuralPathQualification, StructuralResultDeclaration, TerminalAccessViewInput,
    TerminalDynamicSlot, TerminalDynamicStructuralFormal as StructuralFormal,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractOperationPlanWithAccessViewInputs {
    pub plan: AbstractOperationPlan,
    pub access_view_inputs: Vec<TerminalAccessViewInput>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractBoundaryResult {
    Unit,
    Scalar(AbstractResult),
    Structural(StructuralOperationResult),
}

impl AbstractBoundaryResult {
    pub const fn is_unit(&self) -> bool {
        matches!(self, Self::Unit)
    }

    pub const fn scalar(&self) -> Option<AbstractResult> {
        match self {
            Self::Scalar(result) => Some(*result),
            Self::Unit | Self::Structural(_) => None,
        }
    }

    optimization_core::structural_variant_accessor!(StructuralOperationResult; Self::Unit | Self::Scalar(_));
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractStructuralCallResult {
    Unit,
    Scalar(AbstractResult),
    Structural {
        result: StructuralOperationResult, // Result place and path-bound claims in semantic custody.
        selected_evidence: Vec<OutcomeSpecificCallEvidence>, // Present only beside a structural result.
    },
}

impl AbstractStructuralCallResult {
    pub const fn scalar(&self) -> Option<AbstractResult> {
        match self {
            Self::Scalar(result) => Some(*result),
            Self::Unit | Self::Structural { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AbstractCallTarget {
    Machine(MachineId),
    Callable {
        callee: ValueId,
        contract: semantic_vocabulary::CallableContractId,
    },
    Dynamic {
        selector: ValueId,
        interface: u32,
        slot: u32,
    },
}

impl AbstractCallTarget {
    pub const fn machine(self) -> Option<MachineId> {
        match self {
            Self::Machine(machine) => Some(machine),
            Self::Dynamic { .. } | Self::Callable { .. } => None,
        }
    }

    pub const fn selector(self) -> Option<ValueId> {
        match self {
            Self::Machine(_) => None,
            Self::Dynamic { selector, .. } => Some(selector),
            Self::Callable { callee, .. } => Some(callee),
        }
    }

    pub fn operand_type(self) -> Option<ScalarType> {
        match self {
            Self::Machine(_) => None,
            Self::Callable { contract, .. } => Some(ScalarType::Callable(contract)),
            Self::Dynamic { .. } => Some(ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    64,
                )
                .expect("dispatch selector uses the declared u64 carrier"),
            )),
        }
    }

    pub fn canonical_bytes(self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(17);
        self.visit_canonical_bytes(|part| bytes.extend_from_slice(part));
        bytes
    }

    /// Share the target's tagged identity without allocating at each hash pass.
    pub fn visit_canonical_bytes(self, mut emit: impl FnMut(&[u8])) {
        match self {
            Self::Machine(machine) => {
                emit(&[0]);
                emit(&machine.get().to_le_bytes());
            }
            Self::Callable { callee, contract } => {
                emit(&[2]);
                emit(&callee.get().to_le_bytes());
                emit(&contract.get().to_le_bytes());
            }
            Self::Dynamic {
                selector,
                interface,
                slot,
            } => {
                emit(&[1]);
                emit(&selector.get().to_le_bytes());
                emit(&interface.to_le_bytes());
                emit(&slot.to_le_bytes());
            }
        }
    }
}

impl From<MachineId> for AbstractCallTarget {
    fn from(machine: MachineId) -> Self {
        Self::Machine(machine)
    }
}

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

impl<'declaration> From<&'declaration ResultDeclaration> for StructuralCallResult<'declaration> {
    fn from(result: &'declaration ResultDeclaration) -> Self {
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
    Callable(&'declaration terminal_psi::TerminalCallableContract),
}

impl<'declaration> AbstractCallSignature<'declaration> {
    pub fn scalar_parameter_count(self) -> usize {
        match self {
            Self::Machine(machine) => machine.parameters.len(),
            Self::Requirement(requirement) => requirement.scalar_parameters.len(),
            Self::Callable(contract) => contract.scalar_parameters.len(),
        }
    }

    pub fn scalar_parameter(self, position: usize) -> Option<ScalarType> {
        match self {
            Self::Machine(machine) => machine
                .parameters
                .get(position)
                .map(|parameter| parameter.scalar_type),
            Self::Requirement(requirement) => requirement.scalar_parameters.get(position).copied(),
            Self::Callable(contract) => contract
                .scalar_parameters
                .get(position)
                .map(|formal| formal.scalar_type),
        }
    }

    pub fn structural_parameter_count(self) -> usize {
        match self {
            Self::Machine(machine) => machine.structural_parameters.len(),
            Self::Requirement(requirement) => requirement.structural_parameters.len(),
            Self::Callable(contract) => contract.structural_parameters.len(),
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
            Self::Callable(contract) => {
                contract.structural_parameters.get(position).map(Into::into)
            }
        }
    }

    pub fn is_unit_result(self) -> bool {
        match self {
            Self::Machine(machine) => matches!(machine.result, crate::AbstractFunctionResult::Unit),
            Self::Requirement(requirement) => requirement.result.is_unit(),
            Self::Callable(contract) => contract.result.is_unit(),
        }
    }

    pub fn structural_result(self) -> Option<StructuralCallResult<'declaration>> {
        match self {
            Self::Machine(machine) => machine.result.structural().map(Into::into),
            Self::Callable(contract) => match &contract.result {
                BoundaryMachineResult::Structural(result) => Some(result.into()),
                BoundaryMachineResult::Unit | BoundaryMachineResult::Scalar(_) => None,
            },
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
            Self::Callable(contract) => contract.result.scalar(),
        }
    }
}

pub fn call_source(
    structural_parameters: &[ParameterDeclaration],
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

#[cfg(test)]
mod tests {
    use super::AbstractCallTarget;
    use semantic_vocabulary::{CallableContractId, MachineId, ValueId};

    #[test]
    fn callable_target_identity_keeps_code_and_contract() {
        let callee = ValueId::new(1).unwrap();
        let contract = CallableContractId::new(1).unwrap();
        let callable = AbstractCallTarget::Callable { callee, contract };
        // These coordinates can have the same payload bytes across target kinds.
        // Reusing an identity must never change a code value into a table slot.
        let dynamic = AbstractCallTarget::Dynamic {
            selector: callee,
            interface: 1,
            slot: 0,
        };
        let direct = AbstractCallTarget::Machine(MachineId::new(1).unwrap());
        assert_ne!(callable.canonical_bytes(), dynamic.canonical_bytes());
        assert_ne!(callable.canonical_bytes(), direct.canonical_bytes());
        assert_ne!(
            callable.canonical_bytes(),
            AbstractCallTarget::Callable {
                callee: ValueId::new(2).unwrap(),
                contract,
            }
            .canonical_bytes()
        );
        assert_ne!(
            callable.canonical_bytes(),
            AbstractCallTarget::Callable {
                callee,
                contract: CallableContractId::new(2).unwrap(),
            }
            .canonical_bytes()
        );
    }
}
