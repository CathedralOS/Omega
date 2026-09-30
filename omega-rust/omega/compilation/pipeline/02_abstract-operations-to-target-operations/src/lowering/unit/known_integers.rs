//! Known integer sources: every dominating integer definition a scalar operand
//! or call argument may name. Direct calls lower in `control_flow::direct_calls`.

use crate::LoweringError;
use semantic_vocabulary::{BlockId, IntegerType, IntegerValue, OperationId, ScalarType, ValueId};
use std::collections::BTreeMap;
use target_operations::{TargetUnitScalarArgumentSource, TargetUnitScalarHomeRequirement};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::lowering) enum KnownUnitInteger {
    Parameter {
        parameter_index: u32,
        scalar_type: IntegerType,
    },
    Immediate {
        defining_operation: OperationId,
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    Home(TargetUnitScalarHomeRequirement),
    BlockParameter {
        block: BlockId,
        value: ValueId,
        scalar_type: IntegerType,
    },
}

impl KnownUnitInteger {
    pub(in crate::lowering) fn scalar_type(self) -> IntegerType {
        match self {
            Self::Parameter { scalar_type, .. } => scalar_type,
            Self::Immediate { scalar_type, .. } => scalar_type,
            Self::Home(home) => match home.scalar_type {
                ScalarType::Integer(integer) => integer,
                _ => unreachable!("known Unit integer home retains an integer type"),
            },
            Self::BlockParameter { scalar_type, .. } => scalar_type,
        }
    }

    pub(in crate::lowering) const fn into_target_source(
        self,
        source_value: ValueId,
    ) -> TargetUnitScalarArgumentSource {
        match self {
            Self::Parameter {
                parameter_index,
                scalar_type,
            } => TargetUnitScalarArgumentSource::Parameter {
                parameter_index,
                source_value,
                scalar_type: ScalarType::Integer(scalar_type),
            },
            Self::Immediate {
                defining_operation,
                scalar_type,
                value,
            } => TargetUnitScalarArgumentSource::IntegerImmediate {
                defining_operation,
                source_value,
                scalar_type,
                value,
            },
            Self::Home(home) => TargetUnitScalarArgumentSource::Home(home),
            Self::BlockParameter {
                block,
                value,
                scalar_type,
            } => TargetUnitScalarArgumentSource::BlockParameter(
                target_operations::TargetScalarBlockValue {
                    block,
                    value,
                    scalar_type: ScalarType::Integer(scalar_type),
                },
            ),
        }
    }
}

pub(in crate::lowering) fn insert_known_unit_integer(
    values: &mut BTreeMap<ValueId, KnownUnitInteger>,
    value: ValueId,
    known: KnownUnitInteger,
) -> Result<(), LoweringError> {
    if values.insert(value, known).is_some() {
        return Err(LoweringError::DuplicateValue(value));
    }
    Ok(())
}
