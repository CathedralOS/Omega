use omega_target_operations::ScalarParameterLocation;
use psi_core::{EdgeId, IntegerType, OperationId, ValueId};

pub(in crate::validation::straight_line_parameter) struct WrappingIntegerAddParametersSource {
    pub(in crate::validation::straight_line_parameter) operation: OperationId,
    pub(in crate::validation::straight_line_parameter) return_edge: EdgeId,
    pub(in crate::validation::straight_line_parameter) source_value: ValueId,
    pub(in crate::validation::straight_line_parameter) scalar_type: IntegerType,
    pub(in crate::validation::straight_line_parameter) left_value: ValueId,
    pub(in crate::validation::straight_line_parameter) right_value: ValueId,
    pub(in crate::validation::straight_line_parameter) left_parameter_index: usize,
    pub(in crate::validation::straight_line_parameter) right_parameter_index: usize,
}

pub(in crate::validation::straight_line_parameter) struct ReconstructedWrappingIntegerAddParameters
{
    pub(in crate::validation::straight_line_parameter) operation: OperationId,
    pub(in crate::validation::straight_line_parameter) return_edge: EdgeId,
    pub(in crate::validation::straight_line_parameter) source_value: ValueId,
    pub(in crate::validation::straight_line_parameter) scalar_type: IntegerType,
    pub(in crate::validation::straight_line_parameter) left_value: ValueId,
    pub(in crate::validation::straight_line_parameter) right_value: ValueId,
    pub(in crate::validation::straight_line_parameter) left_parameter_index: usize,
    pub(in crate::validation::straight_line_parameter) right_parameter_index: usize,
    pub(in crate::validation::straight_line_parameter) left_location: ScalarParameterLocation,
    pub(in crate::validation::straight_line_parameter) right_location: ScalarParameterLocation,
}
