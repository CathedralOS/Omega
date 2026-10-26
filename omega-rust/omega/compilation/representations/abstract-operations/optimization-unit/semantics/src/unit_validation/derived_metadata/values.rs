//! Scalar definitions and uses a function must reproduce, read from the
//! operation-level derivation `optimization_unit` owns.
use optimization_unit::{ValueDefinition, ValueDefinitionSite, ValueUse};
use semantic_vocabulary::BlockId;

pub(crate) fn expected_definitions(
    operation: &abstract_operations::AbstractOperation,
    block: BlockId,
    node: u32,
) -> Vec<ValueDefinition> {
    let definition = optimization_unit::operation_definition(operation);
    definition
        .into_iter()
        .map(|(value, scalar_type)| ValueDefinition {
            value,
            scalar_type,
            site: ValueDefinitionSite::Node { block, node },
        })
        .collect()
}

/// The values the operation reads, at the site that reads them. The roster is
/// the constructor's own (`operation_uses`): a local re-spelling stayed behind
/// when the constructor learned the activation-claim operations and the
/// structural call's selector, so the validator refused units the constructor
/// had just built.
pub(crate) fn expected_uses(
    operation: &abstract_operations::AbstractOperation,
    block: BlockId,
    node: u32,
) -> Vec<ValueUse> {
    optimization_unit::operation_uses(operation)
        .into_iter()
        .map(|value| ValueUse { value, block, node })
        .collect()
}
