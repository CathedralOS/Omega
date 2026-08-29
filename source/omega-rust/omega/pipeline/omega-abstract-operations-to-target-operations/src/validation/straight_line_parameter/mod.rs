//! Parameter translation join: direct returns or derived expressions.
//!
//! Result-kind leaves remain distinct catalog families. This entrance owns
//! their common source-envelope and ABI replay. Derived expression provenance
//! descends through `derived.rs` before typed target validation.

mod abi;
pub(crate) mod boolean;
pub(crate) mod boolean_equal;
pub(crate) mod boolean_not;
mod derived;
pub(crate) mod integer;
pub(crate) mod integer_equal;
pub(crate) mod integer_less_or_equal;
pub(crate) mod integer_less_than;
mod model;
mod source;

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;
use psi_core::ScalarType;

use self::model::ReconstructedParameterReturn;
use super::model::StraightLineParameterReconstructionError;

fn reconstruct_parameter_return(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
    expected_result: ScalarType,
) -> Result<ReconstructedParameterReturn, StraightLineParameterReconstructionError> {
    let source = source::reconstruct_direct(function, expected_result)?;
    let locations = abi::replay(&function.parameters, expected_result, expected_target)?;
    if !target.provenance.operations.is_empty()
        || target.provenance.edges.as_slice() != [source.return_edge]
    {
        return Err(StraightLineParameterReconstructionError::TargetProvenance);
    }
    Ok(ReconstructedParameterReturn {
        return_edge: source.return_edge,
        source_value: source.source_value,
        parameter_index: source.parameter_index,
        location: locations[source.parameter_index],
    })
}
