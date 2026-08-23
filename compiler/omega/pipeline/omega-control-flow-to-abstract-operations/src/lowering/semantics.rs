use omega_abstract_operations::AbstractSemanticSummary;

use crate::lowering::AbstractOperationLoweringInput;

use super::boundary::build_abstract_boundary_summary;
use super::ownership::build_abstract_ownership_summary;
use super::values::build_abstract_value_summary;

pub(super) fn build_abstract_semantic_summary(
    input: &AbstractOperationLoweringInput<'_>,
) -> AbstractSemanticSummary {
    AbstractSemanticSummary::with_roots(
        build_abstract_value_summary(input.control_flow),
        build_abstract_boundary_summary(input.control_flow, input.host_calls),
        build_abstract_ownership_summary(input.control_flow),
    )
}
