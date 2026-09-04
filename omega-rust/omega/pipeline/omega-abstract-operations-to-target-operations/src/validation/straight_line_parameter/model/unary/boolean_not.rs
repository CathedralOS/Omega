use omega_target_operations::ScalarParameterLocation;
use psi_core::{EdgeId, OperationId, ValueId};

pub(in crate::validation::straight_line_parameter) struct ReconstructedBooleanNotParameter {
    pub(in crate::validation::straight_line_parameter) not_operation: OperationId,
    pub(in crate::validation::straight_line_parameter) return_edge: EdgeId,
    pub(in crate::validation::straight_line_parameter) source_value: ValueId,
    pub(in crate::validation::straight_line_parameter) operand_value: ValueId,
    pub(in crate::validation::straight_line_parameter) parameter_index: usize,
    pub(in crate::validation::straight_line_parameter) location: ScalarParameterLocation,
}

pub(in crate::validation::straight_line_parameter) struct BooleanNotParameterSource {
    pub(in crate::validation::straight_line_parameter) not_operation: OperationId,
    pub(in crate::validation::straight_line_parameter) return_edge: EdgeId,
    pub(in crate::validation::straight_line_parameter) source_value: ValueId,
    pub(in crate::validation::straight_line_parameter) operand_value: ValueId,
    pub(in crate::validation::straight_line_parameter) parameter_index: usize,
}
