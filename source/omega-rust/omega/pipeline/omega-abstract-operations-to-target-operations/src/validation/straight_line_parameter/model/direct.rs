use omega_abstract_operations::{AbstractFunctionResult, AbstractParameter};
use omega_target_operations::ScalarParameterLocation;
use psi_core::{EdgeId, ValueId};

#[derive(Clone, Copy)]
pub(in crate::validation::straight_line_parameter) enum ParameterResultKind {
    Integer,
    Boolean,
}

impl ParameterResultKind {
    pub(in crate::validation::straight_line_parameter) fn accepts(
        self,
        result: &AbstractFunctionResult,
    ) -> bool {
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

pub(in crate::validation::straight_line_parameter) struct ReconstructedParameterReturn {
    pub(in crate::validation::straight_line_parameter) return_edge: EdgeId,
    pub(in crate::validation::straight_line_parameter) source_value: ValueId,
    pub(in crate::validation::straight_line_parameter) parameter_index: usize,
    pub(in crate::validation::straight_line_parameter) location: ScalarParameterLocation,
}

pub(in crate::validation::straight_line_parameter) struct ParameterReturnSource {
    pub(in crate::validation::straight_line_parameter) return_edge: EdgeId,
    pub(in crate::validation::straight_line_parameter) source_value: ValueId,
    pub(in crate::validation::straight_line_parameter) parameter_index: usize,
}

pub(in crate::validation::straight_line_parameter) struct ReconstructedEnvelope<'a> {
    pub(in crate::validation::straight_line_parameter) function_result: ValueId,
    pub(in crate::validation::straight_line_parameter) parameters: &'a [AbstractParameter],
}
