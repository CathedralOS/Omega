use semantic_vocabulary::{EdgeId, IntegerType, OperationId, ValueId};
use target_operations::ScalarParameterLocation;

pub(in crate::validation::straight_line_parameter) struct ReconstructedIntegerUnaryParameter {
    pub(in crate::validation::straight_line_parameter) operation: OperationId,
    pub(in crate::validation::straight_line_parameter) return_edge: EdgeId,
    pub(in crate::validation::straight_line_parameter) source_value: ValueId,
    pub(in crate::validation::straight_line_parameter) scalar_type: IntegerType,
    pub(in crate::validation::straight_line_parameter) operand_value: ValueId,
    pub(in crate::validation::straight_line_parameter) parameter_index: usize,
    pub(in crate::validation::straight_line_parameter) location: ScalarParameterLocation,
}

pub(in crate::validation::straight_line_parameter) struct IntegerUnaryParameterSource {
    pub(in crate::validation::straight_line_parameter) operation: OperationId,
    pub(in crate::validation::straight_line_parameter) return_edge: EdgeId,
    pub(in crate::validation::straight_line_parameter) source_value: ValueId,
    pub(in crate::validation::straight_line_parameter) scalar_type: IntegerType,
    pub(in crate::validation::straight_line_parameter) operand_value: ValueId,
    pub(in crate::validation::straight_line_parameter) parameter_index: usize,
}
