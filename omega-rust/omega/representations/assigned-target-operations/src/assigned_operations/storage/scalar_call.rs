//! Scalar call storage: durable result homes, argument sources and exact
//! outgoing register-preservation geometry. These data do not execute a call.

use crate::AssignedCallDestination;
use calling_conventions::{ValuePlacement, ValueShape};
use semantic_vocabulary::{IntegerType, IntegerValue, OperationId, ScalarType, ValueId};
use target_operations::MachineRegister;

/// Address-free outgoing call storage and ordered preservation of incoming
/// scalar registers. Offsets are relative to the outgoing call stack area.
/// These are physical facts, not authority: consumers independently validate
/// the ABI extent, register hazards, slot ordering and stack alignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitScalarTransportPlan {
    pub call_stack_bytes: u32,
    pub snapshot_slots: Vec<(MachineRegister, u32)>,
}

/// Durable physical home assigned to one scalar value produced by a scalar
/// call in an attached Unit body.
///
/// `byte_offset` is relative to the function's allocated Unit frame. Machine
/// emission independently reconstructs the complete structural-plus-scalar
/// frame and rejects a stale, overlapping, or substituted home.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssignedUnitScalarHome {
    pub defining_operation: OperationId,
    pub source_value: ValueId,
    pub scalar_type: ScalarType,
    pub shape: ValueShape,
    pub byte_offset: u32,
}

/// Exact physical source of one attached-Unit scalar-call argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignedUnitScalarArgumentSource {
    Parameter {
        parameter_index: u32,
        source_value: ValueId,
        scalar_type: ScalarType,
        location: crate::AssignedScalarLocation,
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
    Home(AssignedUnitScalarHome),
}

impl AssignedUnitScalarArgumentSource {
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

/// One positional scalar argument after durable-home assignment. The complete
/// ABI placement remains explicit; it is not reconstructed from register
/// ordinals during emission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedUnitScalarCallArgument {
    pub parameter_index: u32,
    pub source: AssignedUnitScalarArgumentSource,
    pub destination: AssignedCallDestination,
}

/// One normalized foreign-call scalar argument after exact durable-home
/// assignment. Unlike an in-module scalar call, the complete evaluated ABI
/// placement remains explicit for later source-free object replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedNormalizedForeignScalarArgument {
    pub parameter_index: u32,
    pub source: AssignedUnitScalarArgumentSource,
    pub placement: ValuePlacement,
}
