//! Independently typed value/count carriers for integer shift replay.

use omega_target_operations::ScalarParameterLocation;
use psi_core::{EdgeId, IntegerType, OperationId, ValueId};

pub(in crate::validation::straight_line_parameter) struct IntegerShiftParametersSource {
    pub(in crate::validation::straight_line_parameter) operation: OperationId,
    pub(in crate::validation::straight_line_parameter) return_edge: EdgeId,
    pub(in crate::validation::straight_line_parameter) source_value: ValueId,
    pub(in crate::validation::straight_line_parameter) value_type: IntegerType,
    pub(in crate::validation::straight_line_parameter) count_type: IntegerType,
    pub(in crate::validation::straight_line_parameter) value: ValueId,
    pub(in crate::validation::straight_line_parameter) count: ValueId,
    pub(in crate::validation::straight_line_parameter) value_parameter_index: usize,
    pub(in crate::validation::straight_line_parameter) count_parameter_index: usize,
}

pub(in crate::validation::straight_line_parameter) struct ReconstructedIntegerShiftParameters {
    pub(in crate::validation::straight_line_parameter) operation: OperationId,
    pub(in crate::validation::straight_line_parameter) return_edge: EdgeId,
    pub(in crate::validation::straight_line_parameter) source_value: ValueId,
    pub(in crate::validation::straight_line_parameter) value_type: IntegerType,
    pub(in crate::validation::straight_line_parameter) count_type: IntegerType,
    pub(in crate::validation::straight_line_parameter) value: ValueId,
    pub(in crate::validation::straight_line_parameter) count: ValueId,
    pub(in crate::validation::straight_line_parameter) value_parameter_index: usize,
    pub(in crate::validation::straight_line_parameter) count_parameter_index: usize,
    pub(in crate::validation::straight_line_parameter) value_location: ScalarParameterLocation,
    pub(in crate::validation::straight_line_parameter) count_location: ScalarParameterLocation,
}
