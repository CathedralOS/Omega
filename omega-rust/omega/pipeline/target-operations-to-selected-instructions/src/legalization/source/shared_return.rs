//! Preserve the source CFG and its parameter transfers at a shared scalar return.

mod input;
use super::leaves::{exact_edge_fuel, exact_operation_fuel};
use super::shared::*;
pub(super) use input::match_input;
use legalized_operations::{
    LegalizedConstantTransferArm, LegalizedSharedReturnConditionalFunction,
};

pub(super) fn derive(
    function: usize,
    native_target: target::NativeTarget,
    target: &target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
) -> Result<LegalizedSharedReturnConditionalFunction, LegalizationError> {
    let matched = match_input(function, native_target, target, abstracted, optimized)?;
    Ok(LegalizedSharedReturnConditionalFunction {
        machine: target.machine,
        attachment: target.attachment,
        provenance: target.provenance.clone(),
        abi: target
            .fixed_integer_scalar_abi
            .as_ref()
            .expect("matched ABI")
            .clone(),
        condition_source: matched.condition.source,
        condition: matched.condition.legalized,
        entry_block: matched.entry.id,
        when_true: project_arm(function, matched.true_arm)?,
        when_false: project_arm(function, matched.false_arm)?,
        return_block: matched.return_block.id,
        return_parameter: *matched.parameter,
        return_edge: matched.return_edge,
        return_fuel: exact_edge_fuel(matched.returned, matched.return_edge, function)?,
    })
}
fn project_arm(
    function: usize,
    matched: input::MatchedArm<'_>,
) -> Result<LegalizedConstantTransferArm, LegalizationError> {
    Ok(LegalizedConstantTransferArm {
        block: matched.block.id,
        parameters: matched.block.parameters.clone(),
        branch_edge: matched.successor.psi_edge,
        branch_bindings: matched.successor.bindings.clone(),
        branch_fuel: exact_edge_fuel(matched.branch, matched.successor.psi_edge, function)?,
        constant: SourceImmediate {
            source_value: matched.result,
            value: matched.value,
            constant_operation: matched.operation,
            definition_site: matched.definition.site,
            fuel: exact_operation_fuel(matched.constant, matched.operation, function)?,
        },
        transfer_edge: matched.edge,
        transfer_binding: *matched.binding,
        transfer_fuel: exact_edge_fuel(matched.jump, matched.edge, function)?,
    })
}
