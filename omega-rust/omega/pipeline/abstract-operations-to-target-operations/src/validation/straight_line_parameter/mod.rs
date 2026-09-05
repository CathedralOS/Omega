//! Optimizer module role: executable entrance. Parameter translation join: direct returns or typed expressions.
//!
//! Result-kind leaves remain distinct catalog families. This entrance owns
//! their common source-envelope and ABI replay. Boolean and integer entrances
//! descend through direct, unary, and comparison semantics.

mod abi;
pub(crate) mod boolean;
pub(crate) mod integer;
mod model;
mod source;

use abstract_operations::AbstractFunction;
use semantic_vocabulary::ScalarType;
use target::NativeTarget;
use target_operations::TargetFunction;

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
