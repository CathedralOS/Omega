//! Replay an intrinsic enum equality as one complete observation transaction.
//! Its generated memberships are not independently authored Boolean values:
//! only the exact ordered roster and its equivalence template denote equality.
use super::case_sources;
use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::data::{DataDefinition, DataMember, DataShapeKind};
use checked_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use checked_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
use checked_trees::{
    CheckedBooleanExpression as Boolean, CheckedScalarComputationHandle,
    CheckedScalarComputationKind as Computation, CheckedScalarComputationStructuralArgument,
    CheckedScalarExpression, CheckedTrees, CheckedUnitStructuralArgumentSourcePlan,
};

/// `false` leaves an ordinary application to its existing receiver. An enum
/// application either rejoins every operand and the template or fails closed.
pub(crate) fn validate_application(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    root: CheckedScalarComputationHandle,
) -> Result<bool, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    if !plans.nodes.is_valid(root) {
        return unsupported("enum equality has a stale computation");
    }
    let node = plans.nodes.get(root);
    let Computation::Apply {
        source_expression,
        expression,
        operands,
    } = &node.kind
    else {
        return Ok(false);
    };
    if !checked
        .expression_table
        .expression_is_valid(*source_expression)
    {
        return unsupported("enum equality has a stale source occurrence");
    }
    let ExpressionNode::Binary(binary) = checked.expression_table.expression(*source_expression)
    else {
        return Ok(false);
    };
    if !matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) {
        return Ok(false);
    }
    let (machine, source_state) = crate::declarations::state_declaration(checked, state)?;
    let source_type = |source| {
        validation::declared_place_type_raw(&checked.typed, machine, Some(source_state), source)
            .or_else(|| {
                validation::scalar_case_constructor(&checked.typed, source)
                    .map(|constructor| constructor.type_reference)
            })
    };
    let Some(owner) = source_type(binary.left).and_then(|reference| enum_owner(checked, reference))
    else {
        return Ok(false);
    };
    let Some(right_owner) =
        source_type(binary.right).and_then(|reference| enum_owner(checked, reference))
    else {
        return unsupported("enum equality changed its right nominal carrier");
    };
    if owner.symbol != right_owner.symbol
        || node.primitive_type != PrimitiveType::Bool
        || !selected_builtin(checked, *source_expression, binary.operator)
        || !validation::has_builtin_binary_expression_meaning(
            &checked.typed,
            machine,
            Some(source_state),
            *source_expression,
        )
    {
        return unsupported("enum equality changed its carrier or selected meaning");
    }
    let cases = checked.data_members(owner);
    if cases.is_empty()
        || cases.iter().any(|member| {
            !matches!(member, DataMember::Variant(variant)
            if variant.symbol.is_valid() && checked.data_payload_fields(variant).is_empty())
        })
    {
        return unsupported("enum equality requires a complete payloadless case roster");
    }
    let operands = plans
        .operands
        .span(*operands)
        .ok_or(LoweringError::Unsupported(
            "enum equality has a stale operand roster",
        ))?;
    if cases.len().checked_mul(2) != Some(operands.len()) {
        return unsupported("enum equality omitted or duplicated an observation");
    }
    let mut observed_type = None;
    for (position, operand) in operands.iter().enumerate() {
        if !plans.nodes.is_valid(*operand) || operands[..position].contains(operand) {
            return unsupported("enum equality has a stale or reused observation");
        }
        let observed = plans.nodes.get(*operand);
        let DataMember::Variant(expected) = &cases[position % cases.len()] else {
            return unsupported("enum equality lost its case declaration");
        };
        if observed.primitive_type != PrimitiveType::Bool {
            return unsupported("enum equality has a non-Boolean observation");
        }
        let source = if position < cases.len() {
            binary.left
        } else {
            binary.right
        };
        let reference = match &observed.kind {
            Computation::CaseMembership {
                source_expression: occurrence,
                subject: CheckedScalarComputationStructuralArgument::Place(subject),
                case,
            } if matches!(
                subject.source,
                CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
            ) =>
            {
                if occurrence != source_expression || *case != expected.symbol {
                    return unsupported(
                        "enum equality changed its observation occurrence or order",
                    );
                }
                if !validation::place_has_builtin_coordinates(
                    &checked.typed,
                    machine,
                    Some(source_state),
                    source,
                ) {
                    return unsupported("enum equality requires an established builtin place");
                }
                case_sources::named_place(checked, machine, source_state, source, subject)?
            }
            Computation::Value(CheckedScalarExpression::Boolean(value)) => {
                let constructor = validation::scalar_case_constructor(&checked.typed, source)
                    .ok_or(LoweringError::Unsupported(
                        "enum equality replaced a runtime observation with a constant",
                    ))?;
                if !constructor.fields.is_empty()
                    || **value != Boolean::Constant(constructor.case == expected.symbol)
                {
                    return unsupported("enum equality changed its constant constructor tag");
                }
                constructor.type_reference
            }
            _ => return unsupported("enum equality substituted its operand source"),
        };
        if enum_owner(checked, reference).is_none_or(|observed| observed.symbol != owner.symbol) {
            return unsupported("enum equality substituted its observed nominal type");
        }
        let identity = checked.normalized_type_identity(reference);
        if observed_type
            .as_ref()
            .is_some_and(|expected| expected != &identity)
        {
            return unsupported("enum equality changed its exact operand type");
        }
        observed_type = Some(identity);
    }
    let CheckedScalarExpression::Boolean(template) = expression else {
        return unsupported("enum equality has a non-Boolean template");
    };
    if !exact_template(
        template,
        cases.len(),
        binary.operator == BinaryOperator::NotEqual,
    ) {
        return unsupported("enum equality changed its complete equivalence template");
    }
    Ok(true)
}

fn enum_owner(checked: &CheckedTrees, reference: TypeReferenceHandle) -> Option<&DataDefinition> {
    let reference = validation::unwrapped_type_reference(checked, reference)?;
    let owner = if let Some(instance) = checked.closed_data_definition(reference) {
        instance
    } else {
        let TypeReferenceNode::Named { symbol, .. } =
            checked.type_reference_table.type_reference(reference)
        else {
            return None;
        };
        let mut declarations = checked
            .data_definitions()
            .iter()
            .filter(|data| data.symbol == *symbol);
        let owner = declarations.next()?;
        if !symbol.is_valid() || declarations.next().is_some() || !owner.type_parameters.is_empty()
        {
            return None;
        }
        owner
    };
    (DataDefinition::shape_kind_from_members(checked.data_members(owner)) == DataShapeKind::Enum)
        .then_some(owner)
}

fn selected_builtin(
    checked: &CheckedTrees,
    source: ExpressionHandle,
    operator: BinaryOperator,
) -> bool {
    let mut selected = checked
        .facts
        .operators
        .uses
        .iter()
        .map(|(_, row)| row)
        .filter(|row| row.expression == source);
    let Some(row) = selected.next() else {
        return true;
    };
    let spelling = if operator == BinaryOperator::Equal {
        language_core::OperatorSpelling::Equal
    } else {
        language_core::OperatorSpelling::NotEqual
    };
    selected.next().is_none()
        && row.spelling == spelling
        && row.status == checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
        && !row.selected_operator_symbol.is_valid()
        && checked
            .facts
            .operators
            .candidates
            .span(row.candidates)
            .is_some_and(|candidates| {
                candidates.len() == row.candidate_count
                    && candidates
                        .iter()
                        .all(|candidate| candidate.is_domain_owned())
            })
}

fn exact_template(mut template: &Boolean, count: usize, unequal: bool) -> bool {
    if unequal {
        let Boolean::Not(inner) = template else {
            return false;
        };
        template = inner;
    }
    let pair = |template: &Boolean, position| {
        matches!(template,
        Boolean::Equal { left, right }
            if matches!(left.as_ref(), Boolean::Parameter { position: actual } if *actual == position)
                && matches!(right.as_ref(), Boolean::Parameter { position: actual } if *actual == count + position))
    };
    for position in (1..count).rev() {
        let Boolean::And { left, right } = template else {
            return false;
        };
        if !pair(right, position) {
            return false;
        }
        template = left;
    }
    count != 0 && pair(template, 0)
}
