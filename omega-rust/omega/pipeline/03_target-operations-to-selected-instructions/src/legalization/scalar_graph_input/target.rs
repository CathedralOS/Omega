//! Target graph custody binds operation-local observations to available source values.
use super::{
    AbstractFunction, AbstractOperation, AbstractOperationPlan, PsiOptimizationFunction,
    PsiOptimizationUnit, TargetFunction, TargetOperationPlan, ValueId, ValueLocation,
    ValuePlacement,
};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input::instruction;
use semantic_vocabulary::BlockId;
use target_operations::{
    ScalarParameterLocation, TargetBooleanExpression as Boolean,
    TargetIntegerExpression as Expression, TargetScalarExpression, TargetUnitOperation,
};
mod byte_view;
pub(in crate::legalization::scalar_graph_input) mod control_flow;
mod element_view;
mod expressions;
mod hosted_scalar;
mod normalized_foreign;
mod scalar_definitions;
mod unit;
pub(super) fn validate_target(
    target: &TargetFunction,
    abstracted: &AbstractFunction,
    optimized: &PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    super::hosted_scalar::validate_tails(native, optimized, plan)?;
    let operations = optimized
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .filter_map(|node| instruction(node).map(|row| row.0))
        .collect::<Vec<_>>();
    let mut edges = optimized
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .flat_map(|node| match &node.operation {
            AbstractOperation::Return { psi_edge, .. }
            | AbstractOperation::ReturnStructural { psi_edge, .. }
            | AbstractOperation::ReturnUnit { psi_edge, .. }
            | AbstractOperation::Crash { psi_edge, .. }
            | AbstractOperation::Jump { psi_edge, .. } => vec![*psi_edge],
            AbstractOperation::Conditional {
                when_true,
                when_false,
                ..
            } => vec![when_true.psi_edge, when_false.psi_edge],
            AbstractOperation::StructuralCase { cases, .. } => {
                cases.iter().map(|case| case.psi_edge).collect()
            }
            _ => vec![],
        })
        .collect::<Vec<_>>();
    let mut target_edges = target.provenance.edges.clone();
    edges.sort();
    target_edges.sort();
    if target.provenance.operations != operations || target_edges != edges {
        return Err(LegalizationError::custody());
    }
    control_flow::validate(
        target,
        &target.graph,
        abstracted,
        optimized,
        native,
        plan,
        unit,
    )
}

struct Checker<'a> {
    function: &'a TargetFunction,
    available: Option<&'a [(ValueId, target_operations::TargetUnitScalarArgumentSource)]>,
    optimized: &'a PsiOptimizationFunction,
    types: &'a [terminal_psi::StructuralTypeDeclaration],
}
fn resolve(value: ValueId, aliases: &[(ValueId, ValueId)]) -> ValueId {
    aliases
        .iter()
        .rev()
        .find(|(parameter, _)| *parameter == value)
        .map_or(value, |(_, source)| *source)
}
fn location_matches(location: ScalarParameterLocation, placement: &ValuePlacement) -> bool {
    matches!(placement.locations.as_slice(),[ValueLocation::Register {register,value_byte_offset:0,byte_size}] if *byte_size == placement.shape.byte_size && location == ScalarParameterLocation::Register(*register))
        || (super::scalar_stack(placement)
            && matches!(placement.locations.as_slice(),
                [ValueLocation::Stack { stack_byte_offset, .. }]
                    if location == ScalarParameterLocation::IncomingStack { byte_offset: *stack_byte_offset }))
}
