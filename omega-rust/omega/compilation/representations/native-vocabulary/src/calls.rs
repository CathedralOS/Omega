// The formal a structural call argument binds to, read the same way from a
// machine's parameter declaration and from a dynamic requirement slot.

use semantic_vocabulary::{MachineId, StructuralDomainId, StructuralTypeId, ValueId};
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

/// What a call names: a machine directly, or a dynamic dispatch slot read
/// through a selector value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AbstractCallTarget {
    Machine(MachineId),
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
            Self::Dynamic { .. } => None,
        }
    }

    pub const fn selector(self) -> Option<ValueId> {
        match self {
            Self::Machine(_) => None,
            Self::Dynamic { selector, .. } => Some(selector),
        }
    }

    pub fn canonical_bytes(self) -> Vec<u8> {
        let mut bytes = Vec::new();
        match self {
            Self::Machine(machine) => {
                bytes.push(0);
                optimization_core::codec_bytes::push_encoded(
                    &mut bytes,
                    machine.get().to_le_bytes(),
                );
            }
            Self::Dynamic {
                selector,
                interface,
                slot,
            } => {
                bytes.push(1);
                optimization_core::codec_bytes::push_encoded(
                    &mut bytes,
                    selector.get().to_le_bytes(),
                );
                optimization_core::codec_bytes::push_encoded(&mut bytes, interface.to_le_bytes());
                optimization_core::codec_bytes::push_encoded(&mut bytes, slot.to_le_bytes());
            }
        }
        bytes
    }
}

impl From<MachineId> for AbstractCallTarget {
    fn from(machine: MachineId) -> Self {
        Self::Machine(machine)
    }
}
