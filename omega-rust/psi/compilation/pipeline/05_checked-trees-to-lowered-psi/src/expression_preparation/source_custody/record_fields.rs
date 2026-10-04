//! Source custody for scalar observations of existing or freshly evaluated records.
//! Field resolution owns exact receiver identity; structural value replay owns
//! complete construction and its scalar operands without inventing a destination.

use super::structural::StructuralValueSource;
use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::CheckedStructuralValueHandle;
use checked_trees::CheckedTrees;
use checked_trees::CheckedUnitStructuralArgumentPlan;
use checked_trees::expression::ExpressionHandle;
use checked_trees::types::PrimitiveType;
use symbols::SymbolHandle;

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_source(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement: u32,
    expression: checked_trees::expression::ExpressionHandle,
    receiver_value: checked_trees::CheckedStructuralValueHandle,
    subject: &CheckedUnitStructuralArgumentPlan,
    field: symbols::SymbolHandle,
    primitive: PrimitiveType,
) -> Result<
    Vec<(
        checked_trees::CheckedScalarComputationHandle,
        checked_trees::expression::ExpressionHandle,
    )>,
    LoweringError,
> {
    let Some(source) = field_receiver_source(
        checked,
        machine,
        state,
        statement,
        expression,
        receiver_value,
        subject,
        field,
        primitive,
    )?
    else {
        return Ok(Vec::new());
    };
    super::structural::validate_value(checked, machine, state, source)
}

/// Rejoin a scalar field operation to its authored storage and, when fresh,
/// the complete structural value it evaluates. A registered dependency alone
/// is not a source: the operation must occur in this statement and agree with
/// the ordinary field resolver's exact storage, path and declared carrier.
#[allow(clippy::too_many_arguments)]
fn field_receiver_source<'a>(
    checked: &'a CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: u32,
    expression: ExpressionHandle,
    receiver_value: CheckedStructuralValueHandle,
    subject: &checked_trees::CheckedUnitStructuralArgumentPlan,
    field: SymbolHandle,
    primitive: PrimitiveType,
) -> Result<Option<StructuralValueSource<'a>>, LoweringError> {
    let (owner, authored) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
    let statement_node = checked
        .statement_table
        .statements(authored.statement_nodes)
        .get(statement as usize)
        .ok_or(LoweringError::Unsupported(
            "record read has no authored statement",
        ))?;
    let mut present = false;
    for (_, root) in checked.statement_table.expression_roots(statement_node) {
        if root.is_valid()
            && super::computation_calls::authored_expressions(checked, root)?.contains(&expression)
        {
            present = true;
            break;
        }
    }
    if owner.symbol != machine || !present {
        return unsupported("record read escaped its authored statement");
    }
    let source =
        validation::structural_scalar_field(&checked.typed, machine, state, statement, expression)
            .ok_or(LoweringError::Unsupported(
                "record read lost its authored storage and field",
            ))?;
    if subject.structural_storage_root() != Some(source.root)
        || subject.access != checked_trees::CheckedStructuralAccess::SharedBorrow
        || subject.path != source.path
        || subject.type_identity
            != checked
                .normalized_type_identity(source.carrier_type_reference)
                .as_str()
        || source.field != field
        || source.primitive_type != primitive
    {
        return unsupported("record read differs from its authored storage and field");
    }
    if !source.receiver_value.is_valid() {
        if receiver_value.is_valid() {
            return unsupported("established field receiver cannot be evaluated again");
        }
        return Ok(None);
    }
    let plans = &checked.facts.values.structural_values;
    let root = plans
        .root_for_expression(state, statement, source.receiver_value)
        .ok_or(LoweringError::Unsupported(
            "field receiver has no unique complete value root",
        ))?;
    if !plans.nodes.is_valid(receiver_value)
        || root.root != receiver_value
        || root.machine != machine
        || root.type_reference != source.type_reference
        || plans.nodes.get(receiver_value).expression != source.receiver_value
    {
        return unsupported("field receiver substituted its complete value dependency");
    }
    Ok(Some(StructuralValueSource {
        statement_index: statement,
        expression: source.receiver_value,
        type_reference: source.type_reference,
        value: receiver_value,
        operand_calls: plans.calls_for_root(receiver_value).unwrap_or(&[]),
        selection: None,
    }))
}

/// Discover value consumers only through the registered scalar dependency graph,
/// then rejoin each edge to its authored operation. Abandoned arena candidates
/// and a same-typed constructor elsewhere in the source grant no operand scope.
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
            source_expression,
            receiver_value,
            subject,
            field,
        } = &node.kind
        else {
            continue;
        };
        if !receiver_value.is_valid() {
            continue;
        }
        let source = field_receiver_source(
            checked,
            machine,
            state,
            statement,
            *source_expression,
            *receiver_value,
            subject,
            *field,
            node.primitive_type,
        )?
        .ok_or(LoweringError::Unsupported(
            "field receiver lost its evaluated dependency",
        ))?;
        if !sources
            .iter()
            .any(|existing| existing.value == source.value)
        {
            sources.push(source);
        }
    }
    Ok(sources)
}

