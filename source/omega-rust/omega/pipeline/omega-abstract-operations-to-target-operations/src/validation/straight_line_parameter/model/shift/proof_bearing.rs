//! Shift policies that retain an exact proof obligation.

use psi_core::ObligationId;

use super::{IntegerShiftParametersSource, ReconstructedIntegerShiftParameters};

pub(in crate::validation::straight_line_parameter) struct ExactIntegerShiftLeftParametersSource {
    pub(in crate::validation::straight_line_parameter) shift: IntegerShiftParametersSource,
    pub(in crate::validation::straight_line_parameter) obligation: ObligationId,
}

pub(in crate::validation::straight_line_parameter) struct ReconstructedExactIntegerShiftLeftParameters
{
    pub(in crate::validation::straight_line_parameter) shift: ReconstructedIntegerShiftParameters,
    pub(in crate::validation::straight_line_parameter) obligation: ObligationId,
}

pub(in crate::validation::straight_line_parameter) struct ExactIntegerShiftRightParametersSource {
    pub(in crate::validation::straight_line_parameter) shift: IntegerShiftParametersSource,
    pub(in crate::validation::straight_line_parameter) obligation: ObligationId,
}

pub(in crate::validation::straight_line_parameter) struct ReconstructedExactIntegerShiftRightParameters
{
    pub(in crate::validation::straight_line_parameter) shift: ReconstructedIntegerShiftParameters,
    pub(in crate::validation::straight_line_parameter) obligation: ObligationId,
}
