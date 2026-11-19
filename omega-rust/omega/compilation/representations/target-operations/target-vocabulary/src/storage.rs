// Durable scalar and structural home requirements, primitive store sources, and ABI parameter locations.
// Home requirements express semantic storage obligations (defining operation or block parameter declaration plus exact aggregate or sum layout) before register allocation assigns physical residences.

use crate::{TargetStructuralArgumentSource, TargetUnitScalarArgumentSource};
use calling_conventions::MachineRegister;
use calling_conventions::{ConventionalSumLayout, ValueShape};
use semantic_vocabulary::{
    BlockId, OperationId, PlaceId, ScalarType, StructuralDomainId, StructuralTypeId, ValueId,
};
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPathQualification,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetUnitScalarHomeRequirement {
    pub defining_operation: OperationId,
    pub source_value: ValueId,
    pub scalar_type: ScalarType,
    pub shape: ValueShape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetStructuralHomeLayout {
    Aggregate(ValueShape),
    Sum(ConventionalSumLayout),
}

impl TargetStructuralHomeLayout {
    pub const fn shape(&self) -> ValueShape {
        match self {
            Self::Aggregate(shape) => *shape,
            Self::Sum(layout) => layout.shape,
        }
    }

    pub const fn sum(&self) -> Option<&ConventionalSumLayout> {
        match self {
            Self::Aggregate(_) => None,
            Self::Sum(layout) => Some(layout),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetStructuralHomeOrigin {
    OperationResult {
        operation: OperationId,
        result: StructuralOperationResult,
    },
    BlockParameter {
        block: BlockId,
        declaration: StructuralParameterDeclaration,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetStructuralHomeRequirement {
    pub origin: TargetStructuralHomeOrigin,
    pub layout: TargetStructuralHomeLayout,
}

impl TargetStructuralHomeRequirement {
    pub const fn operation_result(&self) -> Option<(OperationId, &StructuralOperationResult)> {
        match &self.origin {
            TargetStructuralHomeOrigin::OperationResult { operation, result } => {
                Some((*operation, result))
            }
            TargetStructuralHomeOrigin::BlockParameter { .. } => None,
        }
    }

    pub const fn place(&self) -> PlaceId {
        match &self.origin {
            TargetStructuralHomeOrigin::OperationResult { result, .. } => result.place,
            TargetStructuralHomeOrigin::BlockParameter { declaration, .. } => declaration.place,
        }
    }

    pub const fn structural_type(&self) -> StructuralTypeId {
        match &self.origin {
            TargetStructuralHomeOrigin::OperationResult { result, .. } => result.structural_type,
            TargetStructuralHomeOrigin::BlockParameter { declaration, .. } => {
                declaration.structural_type
            }
        }
    }

    pub const fn multiplicity(&self) -> StructuralMultiplicity {
        match &self.origin {
            TargetStructuralHomeOrigin::OperationResult { result, .. } => result.multiplicity,
            TargetStructuralHomeOrigin::BlockParameter { declaration, .. } => {
                declaration.multiplicity
            }
        }
    }

    pub const fn access(&self) -> StructuralAccess {
        match &self.origin {
            TargetStructuralHomeOrigin::OperationResult { .. } => StructuralAccess::Owned,
            TargetStructuralHomeOrigin::BlockParameter { declaration, .. } => declaration.access,
        }
    }

    pub const fn argument_source(&self) -> TargetStructuralArgumentSource {
        match &self.origin {
            TargetStructuralHomeOrigin::OperationResult { operation, .. } => {
                TargetStructuralArgumentSource::StructuralHome {
                    psi_operation: *operation,
                }
            }
            TargetStructuralHomeOrigin::BlockParameter { block, declaration } => {
                TargetStructuralArgumentSource::BlockParameter {
                    block: *block,
                    place: declaration.place,
                }
            }
        }
    }

    pub fn qualifications(&self) -> &[StructuralDomainId] {
        match &self.origin {
            TargetStructuralHomeOrigin::OperationResult { result, .. } => &result.qualifications,
            TargetStructuralHomeOrigin::BlockParameter { declaration, .. } => {
                &declaration.qualifications
            }
        }
    }

    pub fn projected_qualifications(&self) -> &[StructuralPathQualification] {
        match &self.origin {
            TargetStructuralHomeOrigin::OperationResult { result, .. } => {
                &result.projected_qualifications
            }
            TargetStructuralHomeOrigin::BlockParameter { declaration, .. } => {
                &declaration.projected_qualifications
            }
        }
    }

    pub fn additional_input_qualifications(&self) -> &[StructuralPathQualification] {
        match &self.origin {
            TargetStructuralHomeOrigin::OperationResult { result, .. } => {
                &result.additional_input_qualifications
            }
            TargetStructuralHomeOrigin::BlockParameter { declaration, .. } => {
                &declaration.additional_input_qualifications
            }
        }
    }

    pub fn has_claims(&self) -> bool {
        self.operation_result()
            .is_some_and(|(_, result)| !result.claims.is_empty())
    }
}

pub type TargetUnitWriteOnlyPrimitiveStoreSource = TargetUnitScalarArgumentSource;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarParameterLocation {
    Register(MachineRegister),
    IncomingStack {
        byte_offset: u32, // Byte offset in the ABI's incoming stack-argument area, excluding return-address bias.
    },
}
