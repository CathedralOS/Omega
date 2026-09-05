use crate::{
    StructuralAccess, StructuralMultiplicity, StructuralPathQualification, StructuralPathSegment,
};
use semantic_vocabulary::{PlaceId, ScalarType, StructuralDomainId, StructuralTypeId, ValueId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueDeclaration {
    pub id: ValueId,
    pub scalar_type: ScalarType,
}

/// The normal result shape of one terminal machine.
///
/// Unit is the absence of a runtime value. It therefore has no `ValueId`, no
/// scalar type, and no result pseudo-value that contracts can name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalMachineResult {
    Unit,
    Scalar(ValueDeclaration),
    Structural(StructuralResultDeclaration),
}

impl TerminalMachineResult {
    pub const fn scalar(&self) -> Option<ValueDeclaration> {
        match self {
            Self::Scalar(result) => Some(*result),
            Self::Unit | Self::Structural(_) => None,
        }
    }

    pub const fn scalar_ref(&self) -> Option<&ValueDeclaration> {
        match self {
            Self::Scalar(result) => Some(result),
            Self::Unit | Self::Structural(_) => None,
        }
    }

    pub fn scalar_mut(&mut self) -> Option<&mut ValueDeclaration> {
        match self {
            Self::Scalar(result) => Some(result),
            Self::Unit | Self::Structural(_) => None,
        }
    }

    pub const fn structural(&self) -> Option<&StructuralResultDeclaration> {
        match self {
            Self::Structural(result) => Some(result),
            Self::Unit | Self::Scalar(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralParameterDeclaration {
    pub place: PlaceId,
    pub position: u32,
    pub is_self: bool,
    pub structural_type: StructuralTypeId,
    pub multiplicity: StructuralMultiplicity,
    pub access: StructuralAccess,
    /// Strictly ordered exact signature preconditions. A parameter does not
    /// establish these facts by declaration: its caller or root installation
    /// must discharge them at invocation.
    pub qualifications: Vec<StructuralDomainId>,
    /// Strictly ordered exact qualification preconditions rooted beneath this
    /// parameter. Whole-root qualifications remain in `qualifications`; every
    /// row here must carry a nonempty path whose resolved structural type is
    /// the declared domain carrier.
    pub projected_qualifications: Vec<StructuralPathQualification>,
}

/// Exact normal structural result signature. The result place is proof-visible
/// and receives ownership only through a `ReturnStructural` edge.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralResultDeclaration {
    pub place: PlaceId,
    pub structural_type: StructuralTypeId,
    pub multiplicity: StructuralMultiplicity,
    /// Strictly ordered qualifications transferred with the value.
    pub qualifications: Vec<StructuralDomainId>,
    /// Strictly ordered exact qualifications transferred with nonempty paths
    /// beneath the result root.
    pub projected_qualifications: Vec<StructuralPathQualification>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralArgument {
    pub place: PlaceId,
    pub path: Vec<StructuralPathSegment>,
    pub access: StructuralAccess,
}
