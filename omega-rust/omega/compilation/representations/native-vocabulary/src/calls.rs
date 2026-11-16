// The formal a structural call argument binds to, read the same way from a
// machine's parameter declaration and from a dynamic requirement slot.

use semantic_vocabulary::{MachineId, ScalarType, StructuralDomainId, StructuralTypeId, ValueId};
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
