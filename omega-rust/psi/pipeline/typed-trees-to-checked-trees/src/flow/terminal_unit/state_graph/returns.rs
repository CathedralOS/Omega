//! Normal graph results and source-bound scalar case construction.

use super::*;
use checked_trees::{CheckedControlResultPlan, CheckedScalarCaseFieldPlan};

pub(in crate::flow::terminal_unit) fn signature(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    reference: TypeReferenceHandle,
) -> Option<CheckedControlResultPlan> {
    if is_unit(program, reference) {
        return Some(CheckedControlResultPlan::Unit);
    }
    let multiplicity = crate::checks::type_multiplicity(program, reference);
    let qualifications = parameter_qualifications(program, shapes, reference, &[])?;
    if is_reference(program, reference)
        || type_graph_requires_nominal_drop(program, reference)
        || (multiplicity != Multiplicity::Linear
            && (!qualifications.is_empty()
                || !validation::has_plain_owned_contents_with_numeric_constraints(
                    program, reference,
                )))
    {
        return None;
    }
    let type_identity = shapes.add_type(reference, &[], &[])?;
    let valid_fields = |fields: &[CheckedUnitStructuralFieldPlan]| {
        fields.iter().all(|field| {
            !field.relevance.is_erased()
                && matches!(
                    field.field_type,
                    CheckedUnitStructuralFieldType::Scalar(_)
                        | CheckedUnitStructuralFieldType::BoundedInteger(_)
                )
        })
    };
    let valid = match &shapes.types.get(&type_identity)?.shape {
        CheckedUnitStructuralTypeShape::Record { fields } => valid_fields(fields),
        CheckedUnitStructuralTypeShape::Sum { cases } => {
            !cases.is_empty()
                && cases.iter().all(|case| {
                    case.fields.iter().all(|field| {
                        matches!(
                            field.field_type,
                            CheckedUnitStructuralFieldType::Scalar(_)
                                | CheckedUnitStructuralFieldType::BoundedInteger(_)
                        )
                    })
                })
        }
        _ => false,
    };
    // Whole linear forwarding does not inspect or construct payload fields.
    // Its exact input-origin claims are admitted by the completion/call joins.
    if !valid && multiplicity != Multiplicity::Linear {
        return None;
    }
    Some(CheckedControlResultPlan::Structural(
        CheckedStructuralResultPlan {
            type_identity,
            multiplicity,
            qualifications,
            projected_qualifications: projected_parameter_qualifications(
                program,
                shapes,
                reference,
                &[],
            )?,
        },
    ))
}

pub(super) fn constructor(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    statement_ordinal: u32,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<checked_trees::CheckedStructuralCaseReturnPlan> {
    // A shape classification does not establish fresh linear authority.
    if program.type_multiplicity(state.return_type) == Multiplicity::Linear {
        return None;
    }
    let TypeReferenceNode::Named { symbol, .. } = program
        .type_reference_table
        .type_reference(state.return_type)
    else {
        return None;
    };
    let constructor = validation::scalar_case_constructor(program, expression)?;
    if program.normalized_type_identity(constructor.type_reference)
        != program.normalized_type_identity(state.return_type)
    {
        return None;
    }
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == *symbol)?;
    let variant = program
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            DataMember::Variant(variant) if variant.symbol == constructor.case => Some(variant),
            _ => None,
        })?;
    let declarations = program.data_payload_fields(variant);
    let mut planned = Vec::new();
    for (ordinal, (field_symbol, source, primitive_type)) in constructor.fields.iter().enumerate() {
        let declaration = declarations
            .iter()
            .find(|declaration| declaration.symbol == *field_symbol)?;
        let identity = declaration
            .identity
            .map(|identity| format!("#{identity}"))
            .unwrap_or_else(|| declaration.name.as_str().to_owned());
        if planned
            .iter()
            .any(|field: &CheckedScalarCaseFieldPlan| field.field_identity == identity)
        {
            return None;
        }
        let field_ordinal = u32::try_from(ordinal).ok()?;
        let (binding, expression) = facts.values.scalar_expressions.bound_expression_at(
            state.symbol,
            statement_ordinal,
            CheckedScalarExpressionRole::ReturnCaseField { field_ordinal },
        )?;
        if binding.expression != *source || binding.destination.is_valid() {
            return None;
        }
        planned.push(CheckedScalarCaseFieldPlan {
            field_ordinal,
            field_identity: identity,
            primitive_type: *primitive_type,
            expression: expression.clone(),
        });
    }
    Some(checked_trees::CheckedStructuralCaseReturnPlan {
        statement_ordinal,
        case_identity: variant
            .identity
            .map(|identity| format!("#{identity}"))
            .unwrap_or_else(|| variant.name.as_str().to_owned()),
        fields: planned,
    })
}

pub(super) fn guarded(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    first_ordinal: u32,
) -> Option<CheckedComposedUnitControlTerminatorPlan> {
    use checked_trees::CheckedScalarBranchDestination;
    let mut tails = facts
        .flow
        .terminal_scalar_graphs
        .guarded_tails
        .iter()
        .filter(|tail| tail.state == state.symbol);
    let tail = tails.next()?;
    if tails.next().is_some() {
        return None;
    }
    let arms = facts
        .flow
        .terminal_scalar_graphs
        .guarded_exits
        .span(tail.arms)?;
    if arms.first()?.guard_statement_ordinal != first_ordinal {
        return None;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let mut returns = Vec::new();
    for destination in arms
        .iter()
        .map(|arm| &arm.destination)
        .chain(tail.fallback.iter())
    {
        let CheckedScalarBranchDestination::Return {
            statement_ordinal,
            is_continuation: false,
        } = destination
        else {
            return None;
        };
        let expression = match statements.get(*statement_ordinal as usize)? {
            StatementNode::Expression(expression) => *expression,
            StatementNode::Transition(transition) => {
                let TransitionTargetNode::Value(expression) =
                    program.statement_table.transition_target(transition.target)
                else {
                    return None;
                };
                *expression
            }
            _ => return None,
        };
        returns.push(constructor(
            program,
            facts,
            state,
            *statement_ordinal,
            expression,
        )?);
    }
    Some(CheckedComposedUnitControlTerminatorPlan::Guarded {
        arms: tail.arms,
        fallback: tail.fallback.clone(),
        returns,
    })
}
