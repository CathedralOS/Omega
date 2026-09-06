//! Publish validated scalar fragments with exact current-program attribution.
//! Encoding, allocation and frame application finish before this projection.

mod attribution;
mod control;
#[cfg(test)]
mod tests;

use super::{LeafEvidence, leaf_function};
use abstract_operations::AbstractFunction;
use machine_code::{FunctionFragment, MachineCodeFunction};
use target_operations::FixedIntegerScalarFunctionAbi;

pub(super) fn project_function(
    fragment: &FunctionFragment,
    abi: &FixedIntegerScalarFunctionAbi,
    source: &AbstractFunction,
    architecture: target::Architecture,
) -> Result<MachineCodeFunction, &'static str> {
    if fragment.machine != source.machine || fragment.attachment != source.attachment {
        return Err("scalar fragment has a different current abstract function");
    }
    let control_flow = control::project(fragment, source)?;
    let attribution = attribution::project(fragment, source)?;
    let stack = crate::scalar::collect_scalar_stack_evidence(
        architecture,
        &fragment.bytes,
        control_flow,
        None,
    )
    .map_err(|_| "scalar fragment stack instructions are invalid")?;
    Ok(leaf_function(
        fragment,
        LeafEvidence::Scalar {
            abi: abi.clone(),
            stack,
            attribution,
        },
    ))
}
