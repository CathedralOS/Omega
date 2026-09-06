//! Required durable homes, primitive store sources and ABI parameter locations.

use calling_conventions::MachineRegister;
use calling_conventions::{ConventionalSumLayout, ValueShape};
use semantic_vocabulary::{
    IeeeFloatValue, IntegerType, IntegerValue, OperationId, ScalarType, ValueId,
};
use terminal_psi::StructuralOperationResult;

/// A scalar value produced by an attached-Unit call that must survive the
/// call-result register's next clobber.
///
/// This is a storage requirement, not a storage decision. The assignment
/// stage must give this exact terminal value a durable physical home and must
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

/// One structural result that requires durable caller-frame storage.
///
/// The semantic result remains target-neutral; its layout is the receiving
/// target lowerer's exact, replayable storage decision. A sum retains its full
/// tag/payload layout, while a plain aggregate has no synthetic case or tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetStructuralHomeRequirement {
    pub defining_operation: OperationId,
    pub result: StructuralOperationResult,
    pub layout: TargetStructuralHomeLayout,
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
