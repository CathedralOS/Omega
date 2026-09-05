//! Shared arithmetic carriers and proof-bearing policy custody.

use semantic_vocabulary::{EdgeId, IntegerType, OperationId, ValueId};
use target_operations::ScalarParameterLocation;

mod proof_bearing;
pub(in crate::validation::straight_line_parameter) use proof_bearing::*;

pub(in crate::validation::straight_line_parameter) struct IntegerArithmeticParametersSource {
    pub(in crate::validation::straight_line_parameter) operation: OperationId,
    pub(in crate::validation::straight_line_parameter) return_edge: EdgeId,
    pub(in crate::validation::straight_line_parameter) source_value: ValueId,
    pub(in crate::validation::straight_line_parameter) scalar_type: IntegerType,
    pub(in crate::validation::straight_line_parameter) left_value: ValueId,
    pub(in crate::validation::straight_line_parameter) right_value: ValueId,
    pub(in crate::validation::straight_line_parameter) left_parameter_index: usize,
    pub(in crate::validation::straight_line_parameter) right_parameter_index: usize,
}

pub(in crate::validation::straight_line_parameter) struct ReconstructedIntegerArithmeticParameters {
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
