//! Required durable homes, primitive store sources and ABI parameter locations.

use calling_conventions::MachineRegister;
use calling_conventions::{ConventionalSumLayout, ValueShape};
use semantic_vocabulary::{
    BlockId, IeeeFloatValue, IntegerType, IntegerValue, OperationId, PlaceId, ScalarType,
    StructuralDomainId, StructuralTypeId, ValueId,
};
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPathQualification,
};

/// A scalar value produced by an ordered Unit definition or call that must
/// remain available to its later uses.
///
/// This is a storage requirement, not a storage decision. The assignment
/// stage must give this exact terminal value a physical residence and must
/// use that same home for every later [`crate::TargetUnitScalarArgumentSource::Home`]
/// occurrence. Keeping the defining operation, value identity, type, and
/// shape together prevents a same-typed result from being substituted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetUnitScalarHomeRequirement {
    pub defining_operation: OperationId,
    pub source_value: ValueId,
    pub scalar_type: ScalarType,
    pub shape: ValueShape,
}

/// Exact storage shape, without imposing a sum tag on a plain aggregate.
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

/// The semantic establishment of a structural home. Block arrivals own their
/// destination declaration; they never inherit an incoming operation's identity.
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

/// Required activation-local storage, independently of the chosen physical home.
/// Exact source origin and layout remain available to receiving replay. A sum
/// retains its full tag/payload layout; an aggregate has no synthetic case tag.
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

    /// Result-local claim bindings only. Block declarations have no claim field;
    /// their claim-free eligibility must be rejoined to current ownership input.
    pub fn has_claims(&self) -> bool {
        self.operation_result()
            .is_some_and(|(_, result)| !result.claims.is_empty())
    }
}

/// Exact source of one whole-root primitive replacement. This is distinct
/// from scalar-call arguments: admitting a store literal must not silently
/// widen any call ABI or foreign-boundary vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetUnitWriteOnlyPrimitiveStoreSource {
    Parameter {
        parameter_index: u32,
        source_value: ValueId,
        scalar_type: ScalarType,
    },
    IntegerImmediate {
        defining_operation: OperationId,
        source_value: ValueId,
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    BooleanImmediate {
        defining_operation: OperationId,
        source_value: ValueId,
        value: bool,
    },
    IeeeFloatImmediate {
        defining_operation: OperationId,
        source_value: ValueId,
        value: IeeeFloatValue,
    },
    /// One exact preceding scalar call result, read from the durable home
    /// assigned for that producer in this Unit body.
    Home(TargetUnitScalarHomeRequirement),
}

impl TargetUnitWriteOnlyPrimitiveStoreSource {
    pub const fn source_value(self) -> ValueId {
        match self {
            Self::Parameter { source_value, .. }
            | Self::IntegerImmediate { source_value, .. }
            | Self::BooleanImmediate { source_value, .. }
            | Self::IeeeFloatImmediate { source_value, .. } => source_value,
            Self::Home(home) => home.source_value,
        }
    }

    pub const fn scalar_type(self) -> ScalarType {
        match self {
            Self::Parameter { scalar_type, .. } => scalar_type,
            Self::IntegerImmediate { scalar_type, .. } => ScalarType::Integer(scalar_type),
            Self::BooleanImmediate { .. } => ScalarType::Boolean,
            Self::IeeeFloatImmediate { value, .. } => ScalarType::IeeeFloat(value.format()),
            Self::Home(home) => home.scalar_type,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarParameterLocation {
    Register(MachineRegister),
    /// Byte offset in the ABI's incoming stack-argument area, excluding an
    /// architecture-specific return-address bias.
    IncomingStack {
        byte_offset: u32,
    },
}
