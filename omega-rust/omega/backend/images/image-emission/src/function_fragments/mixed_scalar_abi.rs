//! Rejoin mixed argument/result declarations before projecting ordinary fragments.

use super::Error;
use abstract_operations::AbstractFunction;
use selected_instructions::SelectedFunction;
use target_operations::{MixedStructuralScalarFunctionAbi, TargetFunction, TargetOperation};

pub(super) fn admit(
    abstracted: &AbstractFunction,
    targeted: &TargetFunction,
    selected: &SelectedFunction,
    abi: &MixedStructuralScalarFunctionAbi,
) -> Result<(), Error> {
    let invalid = || Error::Mismatch("mixed scalar ABI differs from current function");
    let result = abstracted.result.scalar().ok_or_else(invalid)?;
    let contract = selected.structural.as_ref().ok_or_else(invalid)?;
    let TargetOperation::ControlGraph(graph) = &targeted.operation else {
        return Err(Error::Unsupported(
            "mixed scalar publication requires an ordinary control graph",
        ));
    };
    if abi.structural_parameters.is_empty()
        || abi.scalar_parameters.len() != abstracted.parameters.len()
        || abi.structural_parameters.len() != abstracted.structural_parameters.len()
        || contract.parameters.len() != abi.structural_parameters.len()
        || abi.result.value != result.value
        || abi.result.scalar_type != result.scalar_type
        || abi.call_plan.result.as_ref() != Some(&abi.result.placement)
        || abi.call_plan.parameters.len()
            != abi.scalar_parameters.len() + abi.structural_parameters.len()
        || graph.call_plan != abi.call_plan
        || graph.scalar_parameters != abi.scalar_parameters
        || graph.parameters != abi.structural_parameters
        || abi
            .scalar_parameters
            .iter()
            .zip(&abstracted.parameters)
            .zip(&abi.call_plan.parameters)
            .any(|((parameter, declaration), placement)| {
                parameter.value != declaration.value
                    || parameter.scalar_type != declaration.scalar_type
                    || parameter.placement != *placement
            })
        || abi
            .structural_parameters
            .iter()
            .zip(&abstracted.structural_parameters)
            .zip(&contract.parameters)
            .zip(&abi.call_plan.parameters[abi.scalar_parameters.len()..])
            .any(|(((parameter, declaration), selected), placement)| {
                parameter.place != declaration.place
                    || parameter.structural_type != declaration.structural_type
                    || parameter.multiplicity != declaration.multiplicity
                    || parameter.access != declaration.access
                    || parameter.projected_qualifications != declaration.projected_qualifications
                    || selected.target != *parameter
                    || parameter.placement != *placement
            })
    {
        return Err(invalid());
    }
    Ok(())
}
