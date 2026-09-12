//! Rejoin constructor fields before their computations enter the ordinary
//! operand-scope walk. A matching final type cannot replace source identity.

use super::*;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};

pub(crate) fn construction(
    checked: &CheckedTrees,
    subject: &CheckedScalarCaseConstruction,
) -> Result<Vec<(ExpressionHandle, Computation)>, LoweringError> {
    let source = validation::scalar_case_constructor(&checked.typed, subject.expression).ok_or(
        LoweringError::Unsupported("computed case lost its exact authored constructor"),
    )?;
    if source.type_reference != subject.type_reference || source.case != subject.case {
        return unsupported("computed case constructor type or case changed");
    }
    // This is the existing no-code ownership classifier, not an inference
    // from the currently selected payload being scalar.
    if !validation::has_plain_owned_contents_with_numeric_constraints(
        &checked.typed,
        subject.type_reference,
    ) || !matches!(
        checked.type_multiplicity(subject.type_reference),
        Multiplicity::Affine | Multiplicity::Unrestricted
    ) {
        return unsupported("computed case has no eligible no-code disposition");
    }
    let retained = fields(checked, subject)?;
    if retained.len() != source.fields.len() {
        return unsupported("computed case omitted or duplicated authored fields");
    }
    let nodes = &checked.facts.values.scalar_computations.nodes;
    source
        .fields
        .into_iter()
        .zip(retained)
        .map(|((symbol, expression, primitive), field)| {
            if symbol != field.symbol
                || !nodes.is_valid(field.value)
                || nodes.get(field.value).authored_root != expression
                || nodes.get(field.value).primitive_type != primitive
            {
                return unsupported(
                    "computed case field differs from its authored position, root or type",
                );
            }
            Ok((expression, field.value))
        })
        .collect()
}

pub(crate) fn membership(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    expression: ExpressionHandle,
    subject: &CheckedScalarComputationStructuralArgument,
    case: symbols::SymbolHandle,
) -> Result<Vec<(ExpressionHandle, Computation)>, LoweringError> {
    let (source_machine, source_state) =
        crate::scalar_source_custody::authored_state(checked, state)?;
    if source_machine.symbol != machine || !checked.expression_table.expression_is_valid(expression)
    {
        return unsupported("computed case observation has no exact source owner");
    }
    let ExpressionNode::Binary(binary) = checked.expression_table.expression(expression) else {
        return unsupported("computed case observation lost its membership expression");
    };
    let type_reference = match subject {
        CheckedScalarComputationStructuralArgument::Case(subject) => {
            if binary.left != subject.expression {
                return unsupported("computed case changed its constructor expression");
            }
            subject.type_reference
        }
        CheckedScalarComputationStructuralArgument::Place(argument) => {
            local_place(checked, source_state, binary.left, argument)?
        }
        CheckedScalarComputationStructuralArgument::Array { .. } => {
            return unsupported("case membership cannot observe an array");
        }
    };
    if !validation::has_exact_case_membership_meaning(
        &checked.typed,
        source_machine,
        Some(source_state),
        expression,
        binary,
    ) {
        return unsupported("computed case observation changed its subject or selected meaning");
    }
    let ExpressionNode::Name(selected) = checked.expression_table.expression(binary.right) else {
        return unsupported("computed case observation lost its selected case");
    };
    let checked_trees::types::TypeReferenceNode::Named { symbol, .. } =
        checked.type_reference_table.type_reference(type_reference)
    else {
        return unsupported("computed case observation lost its nominal type");
    };
    let owner = validation::exact_case_reference_owner(&checked.typed, binary.right).ok_or(
        LoweringError::Unsupported("computed case observation has no exact case owner"),
    )?;
    if selected.symbol != case || owner.symbol != *symbol {
        return unsupported("computed case observation selected a foreign case");
    }
    match subject {
        CheckedScalarComputationStructuralArgument::Case(subject) => construction(checked, subject),
        CheckedScalarComputationStructuralArgument::Place(_) => Ok(Vec::new()),
        CheckedScalarComputationStructuralArgument::Array { .. } => {
            unsupported("case membership cannot observe an array")
        }
    }
}

/// Source identity is checked independently of the established-place lookup.
/// The lookup during emission additionally requires the declaration to precede use.
fn local_place(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    expression: ExpressionHandle,
    argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
) -> Result<checked_trees::types::TypeReferenceHandle, LoweringError> {
    let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } =
        argument.source
    else {
        return unsupported("case membership requires an exact structural local");
    };
    let ExpressionNode::Name(name) = checked.expression_table.expression(expression) else {
        return unsupported("case membership local lost its authored name");
    };
    if !symbol.is_valid()
        || name.symbol != symbol
        || name.head_symbol != symbol
        || name.members.count() != 1
        || checked
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
        || argument.access != checked_trees::CheckedStructuralAccess::SharedBorrow
        || !argument.path.is_empty()
        || checked
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter.symbol == symbol)
    {
        return unsupported("case membership substituted its local source or custody");
    }
    let mut locals = checked
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| match statement {
            checked_trees::statement::StatementNode::LocalData(local) if local.symbol == symbol => {
                Some(local)
            }
            _ => None,
        });
    let local = locals.next().ok_or(LoweringError::Unsupported(
        "case membership has no source local",
    ))?;
    if locals.next().is_some()
        || local.is_mutable
        || checked
            .normalized_type_identity(local.type_reference)
            .as_str()
            != argument.type_identity
    {
        return unsupported("case membership changed its local declaration or type");
    }
    Ok(local.type_reference)
}
