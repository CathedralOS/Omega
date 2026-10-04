//! Computed field receivers obtain storage and operand schedules from the checked
//! value graph. The checking stage has already resolved the field and its source;
//! lowering must not rediscover that relationship from the authored expression.

use super::structural::StructuralValueSource;
use crate::lowering_error::LoweringError;
use checked_trees::types::TypeReferenceHandle;
use checked_trees::{CheckedStructuralValueHandle, CheckedTrees};
use symbols::SymbolHandle;

pub(crate) fn receiver_operands(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: u32,
    receiver_value: CheckedStructuralValueHandle,
    source_type: TypeReferenceHandle,
) -> Result<
    Vec<(
        checked_trees::CheckedScalarComputationHandle,
        checked_trees::expression::ExpressionHandle,
    )>,
    LoweringError,
> {
    match field_receiver_source(checked, statement, receiver_value, source_type) {
        Some(source) => super::structural::validate_value(checked, machine, state, source),
        None => Ok(Vec::new()),
    }
}

fn field_receiver_source(
    checked: &CheckedTrees,
    statement: u32,
    receiver_value: CheckedStructuralValueHandle,
    source_type: TypeReferenceHandle,
) -> Option<StructuralValueSource<'_>> {
    if !receiver_value.is_valid() {
        return None;
    }
    let plans = &checked.facts.values.structural_values;
    Some(StructuralValueSource {
        statement_index: statement,
        expression: plans.nodes.get(receiver_value).expression,
        type_reference: source_type,
        value: receiver_value,
        operand_calls: plans.calls_for_root(receiver_value).unwrap_or(&[]),
        selection: None,
    })
}

/// Discover evaluated receivers through the registered scalar dependency graph.
pub(crate) fn field_receiver_sources(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: u32,
) -> Result<Vec<StructuralValueSource<'_>>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let roots = plans
        .roots
        .iter()
        .filter(|(_, root)| {
            root.machine == machine && root.state == state && root.statement_ordinal == statement
        })
        .map(|(_, root)| root.root)
        .collect::<Vec<_>>();
    let mut sources: Vec<StructuralValueSource<'_>> = Vec::new();
    for handle in
        crate::expression_preparation::computation_graph::reachable_nodes(checked, &roots)?
    {
        let node = plans.nodes.get(handle);
        let checked_trees::CheckedScalarComputationKind::StructuralField {
            source_type,
            receiver_value,
            ..
        } = &node.kind
        else {
            continue;
        };
        let Some(source) = field_receiver_source(checked, statement, *receiver_value, *source_type)
        else {
            continue;
        };
        if !sources
            .iter()
            .any(|existing| existing.value == source.value)
        {
            sources.push(source);
        }
    }
    Ok(sources)
}
