use omega_target_operations::ScalarParameterLocation;
use psi_core::{EdgeId, IntegerType, ObligationId, OperationId, ValueId};

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

pub(in crate::validation::straight_line_parameter) struct ExactIntegerAddParametersSource {
    pub(in crate::validation::straight_line_parameter) arithmetic:
        IntegerArithmeticParametersSource,
    pub(in crate::validation::straight_line_parameter) obligation: ObligationId,
}

pub(in crate::validation::straight_line_parameter) struct ReconstructedExactIntegerAddParameters {
    pub(in crate::validation::straight_line_parameter) arithmetic:
        ReconstructedIntegerArithmeticParameters,
    pub(in crate::validation::straight_line_parameter) obligation: ObligationId,
}

pub(in crate::validation::straight_line_parameter) struct ExactIntegerSubtractParametersSource {
    pub(in crate::validation::straight_line_parameter) arithmetic:
        IntegerArithmeticParametersSource,
    pub(in crate::validation::straight_line_parameter) obligation: ObligationId,
}

pub(in crate::validation::straight_line_parameter) struct ReconstructedExactIntegerSubtractParameters
{
    pub(in crate::validation::straight_line_parameter) arithmetic:
        ReconstructedIntegerArithmeticParameters,
    pub(in crate::validation::straight_line_parameter) obligation: ObligationId,
}

pub(in crate::validation::straight_line_parameter) struct ExactIntegerMultiplyParametersSource {
    pub(in crate::validation::straight_line_parameter) arithmetic:
        IntegerArithmeticParametersSource,
    pub(in crate::validation::straight_line_parameter) obligation: ObligationId,
}

pub(in crate::validation::straight_line_parameter) struct ReconstructedExactIntegerMultiplyParameters
{
    pub(in crate::validation::straight_line_parameter) arithmetic:
        ReconstructedIntegerArithmeticParameters,
    pub(in crate::validation::straight_line_parameter) obligation: ObligationId,
}
