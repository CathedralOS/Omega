//! Scalar call arguments, source identities and boundary result roles.

use crate::{
    ScalarParameterLocation, TargetScalarExpression, TargetStructuralHomeRequirement,
    TargetUnitScalarHomeRequirement,
};
use calling_conventions::ValuePlacement;
use semantic_vocabulary::{IntegerType, IntegerValue, OperationId, ScalarType, ValueId};

/// Closed result role of one compiler-builtin boundary settlement in a Unit
/// body. Unit is explicit rather than encoded as an absent scalar or absent
/// structural home.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetBoundaryResult {
    Unit,
    Structural(TargetStructuralHomeRequirement),
}

/// Exact source of one scalar argument in an attached-Unit body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetUnitScalarArgumentSource {
    /// One incoming Unit-function scalar parameter. The surrounding
    /// `TargetUnitBody::scalar_parameters` roster owns its exact physical
    /// placement; this occurrence retains the nominal parameter join.
    Parameter {
        parameter_index: u32,
        source_value: ValueId,
        scalar_type: ScalarType,
    },
    /// A preceding terminal integer-constant operation. The target call still
    /// carries its source identity; an immediate is not an anonymous literal.
    IntegerImmediate {
        defining_operation: OperationId,
        source_value: ValueId,
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    /// A preceding terminal Boolean-constant operation retained without
    /// pretending the value is an integer carrier.
    BooleanImmediate {
        defining_operation: OperationId,
        source_value: ValueId,
        value: bool,
    },
    /// A preceding scalar call result, read from the exact durable home that
    /// downstream assignment is required to create.
    Home(TargetUnitScalarHomeRequirement),
}

impl TargetUnitScalarArgumentSource {
    pub const fn source_value(self) -> ValueId {
        match self {
            Self::Parameter { source_value, .. } => source_value,
            Self::IntegerImmediate { source_value, .. } => source_value,
            Self::BooleanImmediate { source_value, .. } => source_value,
            Self::Home(home) => home.source_value,
        }
    }

    pub const fn scalar_type(self) -> ScalarType {
        match self {
            Self::Parameter { scalar_type, .. } => scalar_type,
            Self::IntegerImmediate { scalar_type, .. } => ScalarType::Integer(scalar_type),
            Self::BooleanImmediate { .. } => ScalarType::Boolean,
            Self::Home(home) => home.scalar_type,
        }
    }
}

/// One positional scalar argument and its exact selected ABI
/// destination. `placement` is retained from the complete call plan so a
/// later assignment cannot silently reinterpret an incoming-stack coordinate
/// or value shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetUnitScalarCallArgument {
    pub parameter_index: u32,
    pub source: TargetUnitScalarArgumentSource,
    pub placement: ValuePlacement,
}

impl TargetUnitScalarCallArgument {
    pub const fn source_value(&self) -> ValueId {
        self.source.source_value()
    }

    pub const fn scalar_type(&self) -> ScalarType {
        self.source.scalar_type()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetCallArgument {
    pub scalar_type: ScalarType,
    pub location: ScalarParameterLocation,
    pub expression: TargetScalarExpression,
}
