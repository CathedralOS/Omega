use omega_abstract_operations::{AbstractFunctionResult, AbstractParameter};
use omega_target_operations::ScalarParameterLocation;
use psi_core::{EdgeId, IntegerType, OperationId, ValueId};

#[derive(Clone, Copy)]
pub(super) enum ParameterResultKind {
    Integer,
    Boolean,
}

impl ParameterResultKind {
    pub(super) fn accepts(self, result: &AbstractFunctionResult) -> bool {
        match (self, result) {
            (Self::Integer, AbstractFunctionResult::Scalar(result)) => {
                matches!(result.scalar_type, psi_core::ScalarType::Integer(_))
            }
            (Self::Boolean, AbstractFunctionResult::Scalar(result)) => {
                result.scalar_type == psi_core::ScalarType::Boolean
            }
            _ => false,
        }
    }
}

pub(super) struct ReconstructedParameterReturn {
    pub(super) return_edge: EdgeId,
    pub(super) source_value: ValueId,
    pub(super) parameter_index: usize,
    pub(super) location: ScalarParameterLocation,
}

pub(super) struct ParameterReturnSource {
    pub(super) return_edge: EdgeId,
    pub(super) source_value: ValueId,
    pub(super) parameter_index: usize,
}

pub(super) struct ReconstructedEnvelope<'a> {
    pub(super) function_result: ValueId,
    pub(super) parameters: &'a [AbstractParameter],
}

pub(super) struct ReconstructedBooleanNotParameter {
    pub(super) not_operation: OperationId,
    pub(super) return_edge: EdgeId,
    pub(super) source_value: ValueId,
    pub(super) operand_value: ValueId,
    pub(super) parameter_index: usize,
    pub(super) location: ScalarParameterLocation,
}

pub(super) struct BooleanNotParameterSource {
    pub(super) not_operation: OperationId,
    pub(super) return_edge: EdgeId,
    pub(super) source_value: ValueId,
    pub(super) operand_value: ValueId,
    pub(super) parameter_index: usize,
}

pub(super) struct ReconstructedIntegerUnaryParameter {
    pub(super) operation: OperationId,
    pub(super) return_edge: EdgeId,
    pub(super) source_value: ValueId,
    pub(super) scalar_type: IntegerType,
    pub(super) operand_value: ValueId,
    pub(super) parameter_index: usize,
    pub(super) location: ScalarParameterLocation,
}

pub(super) struct IntegerUnaryParameterSource {
    pub(super) operation: OperationId,
    pub(super) return_edge: EdgeId,
    pub(super) source_value: ValueId,
    pub(super) scalar_type: IntegerType,
    pub(super) operand_value: ValueId,
    pub(super) parameter_index: usize,
}

pub(super) struct ReconstructedBooleanEqualParameters {
    pub(super) equal_operation: OperationId,
    pub(super) return_edge: EdgeId,
    pub(super) source_value: ValueId,
    pub(super) left_value: ValueId,
    pub(super) right_value: ValueId,
    pub(super) left_parameter_index: usize,
    pub(super) right_parameter_index: usize,
    pub(super) left_location: ScalarParameterLocation,
    pub(super) right_location: ScalarParameterLocation,
}

pub(super) struct BooleanEqualParametersSource {
    pub(super) equal_operation: OperationId,
    pub(super) return_edge: EdgeId,
    pub(super) source_value: ValueId,
    pub(super) left_value: ValueId,
    pub(super) right_value: ValueId,
    pub(super) left_parameter_index: usize,
    pub(super) right_parameter_index: usize,
}

pub(super) struct ReconstructedIntegerBinaryBooleanParameters {
    pub(super) operation: OperationId,
    pub(super) return_edge: EdgeId,
    pub(super) source_value: ValueId,
    pub(super) scalar_type: IntegerType,
    pub(super) left_value: ValueId,
    pub(super) right_value: ValueId,
    pub(super) left_parameter_index: usize,
    pub(super) right_parameter_index: usize,
    pub(super) left_location: ScalarParameterLocation,
    pub(super) right_location: ScalarParameterLocation,
}

pub(super) struct IntegerBinaryBooleanParametersSource {
    pub(super) operation: OperationId,
    pub(super) return_edge: EdgeId,
    pub(super) source_value: ValueId,
    pub(super) scalar_type: IntegerType,
    pub(super) left_value: ValueId,
    pub(super) right_value: ValueId,
    pub(super) left_parameter_index: usize,
    pub(super) right_parameter_index: usize,
}
