//! A state's scalar successor values and case payload reads, in the shared
//! source-bound value namespace.
use super::super::super::{CheckedScalarExpressionRole, unsupported};
use super::super::{CheckedTrees, LoweringError};
use super::CheckedComposedUnitControlStatePlan;

/// Select the retained value at its authored successor coordinate. Storage
/// reads use computation nodes; pure expressions keep their ordinary binding.
pub(in crate::unit::attached_unit::composed_control) fn successor_value(
    checked: &CheckedTrees,
    state: &CheckedComposedUnitControlStatePlan,
    edge: &super::CheckedStructuralControlSuccessorPlan,
    argument: &checked_trees::CheckedStructuralScalarArgumentPlan,
) -> Result<checked_trees::CheckedCallScalarArgument, LoweringError> {
    let role = CheckedScalarExpressionRole::TransitionArgument {
        argument_ordinal: argument.argument_ordinal,
    };
    let pure = &checked.facts.values.scalar_expressions;
    let computations = &checked.facts.values.scalar_computations;
    let mut roots = computations
        .roots
        .iter()
        .map(|(_, root)| root)
        .filter(|root| {
            root.state == state.state
                && root.statement_ordinal == edge.statement_ordinal
                && root.role == role
        });
    let root = roots.next();
    if roots.next().is_some() {
        return unsupported("Unit graph successor computation has duplicate source roots");
    }
    if let Some(root) = root {
        // Failed uniqueness is not absence: an incomplete or duplicated pure
        // roster must not disappear behind the computation lane.
        let has_pure = pure.source_bindings.iter().any(|(_, binding)| {
            binding.state == state.state
                && binding.statement_ordinal == edge.statement_ordinal
                && binding.role == role
        }) || pure.expressions.iter().any(|expression| {
            expression.state == state.state
                && expression.statement_ordinal == edge.statement_ordinal
                && expression.role == role
        });
        if has_pure
            || !computations.nodes.is_valid(root.root)
            || computations.nodes.get(root.root).primitive_type != argument.primitive_type
        {
            return unsupported("Unit graph successor computation disagrees with its argument");
        }
        Ok(checked_trees::CheckedCallScalarArgument::Computation(
            root.root,
        ))
    } else {
        let (_, value) = pure
            .bound_expression_at(state.state, edge.statement_ordinal, role)
            .ok_or(LoweringError::Unsupported(
                "Unit graph scalar successor has no checked source expression",
            ))?;
        Ok(checked_trees::CheckedCallScalarArgument::Pure(
            value.clone(),
        ))
    }
}

/// Direct payload reads in a successor argument retain their complete subject
/// prefix before `[Case, Field]`. The selected edge stages only fields from
/// that exact projection; same-type siblings do not share a payload namespace.
/// Nested payload-carrier reads keep the ordinary observation path below.
pub(in crate::unit::attached_unit::composed_control) fn case_payload_reads<'a>(
    expression: &'a checked_trees::CheckedScalarExpression,
    reads: &mut Vec<(
        u32,
        Vec<checked_trees::CheckedUnitStructuralPathSegment>,
        &'a str,
        &'a str,
    )>,
) {
    use checked_trees::{
        CheckedBooleanExpression as Boolean, CheckedScalarExpression as Scalar,
        CheckedStructuralPredicatePathSegment as Segment,
    };
    fn flat(
        path: &[Segment],
    ) -> Option<(
        Vec<checked_trees::CheckedUnitStructuralPathSegment>,
        &str,
        &str,
    )> {
        let [prefix @ .., Segment::Case(case), Segment::Field(field)] = path else {
            return None;
        };
        let prefix = prefix
            .iter()
            .map(|segment| match segment {
                Segment::Field(field) => {
                    checked_trees::CheckedUnitStructuralPathSegment::Field(field.clone())
                }
                Segment::Case(case) => {
                    checked_trees::CheckedUnitStructuralPathSegment::Case(case.clone())
                }
                Segment::FixedIndex(index) => {
                    checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(*index)
                }
                Segment::RuntimeIndex(index) => {
                    checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(index.clone())
                }
            })
            .collect();
        Some((prefix, case.as_str(), field.as_str()))
    }
    fn scalar<'a>(
        expression: &'a checked_trees::CheckedScalarExpression,
        reads: &mut Vec<(
            u32,
            Vec<checked_trees::CheckedUnitStructuralPathSegment>,
            &'a str,
            &'a str,
        )>,
    ) {
        match expression {
            Scalar::StructuralParameterField {
                parameter_position,
                path,
                ..
            } => {
                if let Some((prefix, case, field)) = flat(path) {
                    reads.push((*parameter_position, prefix, case, field));
                }
            }
            Scalar::IntegerBinary { left, right, .. } => {
                scalar(left, reads);
                scalar(right, reads);
            }
            Scalar::IntegerBitwiseNot { operand, .. }
            | Scalar::IntegerWiden { operand, .. }
            | Scalar::IntegerExactCast { operand, .. }
            | Scalar::IntegerSaturatingCast { operand, .. }
            | Scalar::IntegerWrappingCast { operand, .. }
            | Scalar::IntegerTrappingCast { operand, .. } => scalar(operand, reads),
            Scalar::StructuralParameterIndexedRead {
                index,
                element_path,
                ..
            } => {
                scalar(&index.expression, reads);
                for step in element_path {
                    if let checked_trees::CheckedScalarReadProjection::RuntimeIndex(selector) = step
                    {
                        scalar(&selector.expression, reads);
                    }
                }
            }
            Scalar::Boolean(expression) => boolean(expression, reads),
            _ => {}
        }
    }
    fn boolean<'a>(
        expression: &'a checked_trees::CheckedBooleanExpression,
        reads: &mut Vec<(
            u32,
            Vec<checked_trees::CheckedUnitStructuralPathSegment>,
            &'a str,
            &'a str,
        )>,
    ) {
        match expression {
            Boolean::StructuralParameterField {
                parameter_position,
                path,
            } => {
                if let Some((prefix, case, field)) = flat(path) {
                    reads.push((*parameter_position, prefix, case, field));
                }
            }
            Boolean::Not(operand) => boolean(operand, reads),
            Boolean::Equal { left, right }
            | Boolean::And { left, right }
            | Boolean::Or { left, right } => {
                boolean(left, reads);
                boolean(right, reads);
            }
            Boolean::IntegerComparison { left, right, .. }
            | Boolean::ScalarIeeeFloatComparison { left, right, .. } => {
                scalar(left, reads);
                scalar(right, reads);
            }
            _ => {}
        }
    }
    scalar(expression, reads);
}
