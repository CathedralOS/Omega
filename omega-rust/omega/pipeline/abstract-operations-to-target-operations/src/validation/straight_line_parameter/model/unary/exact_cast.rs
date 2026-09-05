use semantic_vocabulary::{EdgeId, IntegerType, ObligationId, OperationId, ValueId};
use target_operations::ScalarParameterLocation;

pub(in crate::validation::straight_line_parameter) struct ReconstructedIntegerExactCastParameter {
    pub(in crate::validation::straight_line_parameter) operation: OperationId,
    pub(in crate::validation::straight_line_parameter) obligation: ObligationId,
    pub(in crate::validation::straight_line_parameter) return_edge: EdgeId,
    pub(in crate::validation::straight_line_parameter) source_value: ValueId,
    pub(in crate::validation::straight_line_parameter) source_type: IntegerType,
    pub(in crate::validation::straight_line_parameter) target_type: IntegerType,
    pub(in crate::validation::straight_line_parameter) operand_value: ValueId,
    pub(in crate::validation::straight_line_parameter) parameter_index: usize,
    pub(in crate::validation::straight_line_parameter) location: ScalarParameterLocation,
}

pub(in crate::validation::straight_line_parameter) struct IntegerExactCastParameterSource {
    pub(in crate::validation::straight_line_parameter) operation: OperationId,
    pub(in crate::validation::straight_line_parameter) obligation: ObligationId,
    pub(in crate::validation::straight_line_parameter) return_edge: EdgeId,
    pub(in crate::validation::straight_line_parameter) source_value: ValueId,
    pub(in crate::validation::straight_line_parameter) source_type: IntegerType,
    pub(in crate::validation::straight_line_parameter) target_type: IntegerType,
    pub(in crate::validation::straight_line_parameter) operand_value: ValueId,
    pub(in crate::validation::straight_line_parameter) parameter_index: usize,
}
