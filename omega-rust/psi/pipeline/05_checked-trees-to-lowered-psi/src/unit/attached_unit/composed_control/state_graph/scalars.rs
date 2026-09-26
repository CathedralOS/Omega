//! State-local scalar storage uses the shared source-bound value namespace.
use super::super::super::super::{StructuralParameterDeclaration, StructuralTypeDeclaration};
use super::super::super::{
    CheckedScalarExpressionRole, ValueDeclaration, direct_expression_contains_short_circuit,
    emit_direct_expression, terminal_scalar_type, unsupported, validate_direct_parameter_types,
};
use super::super::{CheckedTrees, LoweringError};
use super::CheckedComposedUnitControlStatePlan;
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::expression_preparation::bindings::ScalarBindings;
use typed_trees_to_checked_trees::checked_trees::{
    CheckedScalarBindingDestination, CheckedScalarBindingValue,
};

/// Select the retained value at its authored successor coordinate. Storage
/// reads use computation nodes; pure expressions keep their ordinary binding.
pub(in crate::unit::attached_unit::composed_control) fn successor_value(
    checked: &CheckedTrees,
    state: &CheckedComposedUnitControlStatePlan,
    edge: &super::CheckedStructuralControlSuccessorPlan,
    argument: &typed_trees_to_checked_trees::checked_trees::CheckedStructuralScalarArgumentPlan,
) -> Result<typed_trees_to_checked_trees::checked_trees::CheckedCallScalarArgument, LoweringError> {
    let role = CheckedScalarExpressionRole::TransitionArgument {
        argument_ordinal: argument.argument_ordinal,
    };
    let source = crate::expression_preparation::source_custody::locate(
        checked,
        state.state,
        edge.statement_ordinal,
        role,
    )?;
    if source.primitive_type != argument.primitive_type {
        return unsupported("Unit graph successor value type disagrees with source");
    }
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
            || root.machine != source.machine
            || !computations.nodes.is_valid(root.root)
            || computations.nodes.get(root.root).authored_root != source.expression
            || computations.nodes.get(root.root).primitive_type != argument.primitive_type
        {
            return unsupported("Unit graph successor computation disagrees with source");
        }
        crate::expression_preparation::source_custody::validate_computation_calls(
            checked,
            source.machine,
            state.state,
            edge.statement_ordinal,
            root.root,
            source.expression,
        )?;
        Ok(
            typed_trees_to_checked_trees::checked_trees::CheckedCallScalarArgument::Computation(
                root.root,
            ),
        )
    } else {
        let (binding, value) = pure
            .bound_expression_at(state.state, edge.statement_ordinal, role)
            .ok_or(LoweringError::Unsupported(
                "Unit graph scalar successor has no checked source expression",
            ))?;
        crate::expression_preparation::source_custody::validate_pure(
            checked,
            binding,
            terminal_scalar_type(argument.primitive_type)?,
        )?;
        Ok(
            typed_trees_to_checked_trees::checked_trees::CheckedCallScalarArgument::Pure(
                value.clone(),
            ),
        )
    }
}

fn role(
    binding: &typed_trees_to_checked_trees::checked_trees::CheckedScalarBinding,
    immutable_ordinal: u32,
) -> CheckedScalarExpressionRole {
    match binding.destination {
        CheckedScalarBindingDestination::Immutable => {
            CheckedScalarExpressionRole::LocalInitializer {
                binding_ordinal: immutable_ordinal,
            }
        }
        CheckedScalarBindingDestination::StorageInitialize { .. } => {
            CheckedScalarExpressionRole::StorageInitializer
        }
        CheckedScalarBindingDestination::StorageAssign { .. } => {
            CheckedScalarExpressionRole::AssignmentValue
        }
    }
}

pub(super) fn validate(
    checked: &CheckedTrees,
    state: &CheckedComposedUnitControlStatePlan,
) -> Result<(), LoweringError> {
    if state.bindings.len() != state.binding_initializers.len() {
        return unsupported("Unit graph scalar prefix lost an initializer");
    }
    let mut immutable_ordinal = 0;
    for (ordinal, (binding, value)) in state
        .bindings
        .iter()
        .zip(&state.binding_initializers)
        .enumerate()
    {
        if binding.statement_ordinal as usize != ordinal
            || binding.value != CheckedScalarBindingValue::Expression
        {
            return unsupported("Unit graph scalar prefix reordered or replaced a binding");
        }
        let (source, retained) = checked
            .facts
            .values
            .scalar_expressions
            .bound_expression_at(
                state.state,
                binding.statement_ordinal,
                role(binding, immutable_ordinal),
            )
            .ok_or(LoweringError::Unsupported(
                "Unit graph scalar prefix has no exact checked expression",
            ))?;
        if retained != value {
            return unsupported(
                "Unit graph scalar initializer disagrees with its checked expression",
            );
        }
        if let CheckedScalarBindingDestination::StorageInitialize { symbol }
        | CheckedScalarBindingDestination::StorageAssign { symbol } = binding.destination
            && source.destination != symbol
        {
            return unsupported("Unit graph scalar storage destination disagrees with source");
        }
        crate::expression_preparation::source_custody::validate_pure(
            checked,
            source,
            terminal_scalar_type(binding.primitive_type)?,
        )?;
        if binding.destination == CheckedScalarBindingDestination::Immutable {
            immutable_ordinal += 1;
        }
    }
    Ok(())
}

pub(super) fn emit_prefix(
    checked: &CheckedTrees,
    state: &CheckedComposedUnitControlStatePlan,
    parameters: &[(u32, StructuralParameterDeclaration)],
    structural_types: &[StructuralTypeDeclaration],
    values: &mut Vec<ValueDeclaration>,
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<ScalarBindings, LoweringError> {
    let mut bindings = ScalarBindings::new(values.len())
        .with_structural_parameters(parameters)
        .with_structural_observations(structural_types);
    let (_, authored) =
        crate::expression_preparation::source_custody::authored_state(checked, state.state)?;
    for (position, parameter) in state.scalar_parameters.iter().enumerate() {
        let source = &checked.state_parameters(authored)[parameter.source_position as usize];
        if source.is_mutable {
            bindings.initialize_parameter(
                source.symbol,
                terminal_scalar_type(parameter.primitive_type)?,
                position,
            )?;
        }
    }
    let mut immutable_ordinal = 0;
    for binding in &state.bindings {
        let expression = bindings.expression_at(
            checked,
            state.state,
            binding.statement_ordinal,
            role(binding, immutable_ordinal),
        )?;
        let scalar_type = terminal_scalar_type(binding.primitive_type)?;
        if expression.scalar_type() != scalar_type
            || direct_expression_contains_short_circuit(&expression)
        {
            return unsupported("Unit graph scalar prefix needs a matching branch-free value");
        }
        validate_direct_parameter_types(
            &expression,
            &values
                .iter()
                .map(|value| value.scalar_type)
                .collect::<Vec<_>>(),
        )?;
        let id = emit_direct_expression(&expression, values, next_value, operations);
        bindings.append(binding.destination, scalar_type, values.len())?;
        values.push(ValueDeclaration {
            qualifications: Default::default(),
            id,
            scalar_type,
        });
        if binding.destination == CheckedScalarBindingDestination::Immutable {
            immutable_ordinal += 1;
        }
    }
    Ok(bindings)
}

/// Direct payload reads in one successor argument: each `[Case, Field]` path
/// binds `(parameter position, case name, field name)` so the case edge can
/// stage the field as a block formal the expression reads back. Longer paths
/// are the nested walker below's shape; other shapes stay on the ordinary
/// observation path.
pub(in crate::unit::attached_unit::composed_control) fn flat_case_payload_reads<'a>(
    expression: &'a checked_trees::CheckedScalarExpression,
    reads: &mut Vec<(u32, &'a str, &'a str)>,
) {
    use checked_trees::{
        CheckedBooleanExpression as Boolean, CheckedScalarExpression as Scalar,
        CheckedStructuralPredicatePathSegment as Segment,
    };
    fn flat(path: &[Segment]) -> Option<(&str, &str)> {
        let [Segment::Case(case), Segment::Field(field)] = path else {
            return None;
        };
        Some((case.as_str(), field.as_str()))
    }
    fn scalar<'a>(
        expression: &'a checked_trees::CheckedScalarExpression,
        reads: &mut Vec<(u32, &'a str, &'a str)>,
    ) {
        match expression {
            Scalar::StructuralParameterField {
                parameter_position,
                path,
                ..
            } => {
                if let Some((case, field)) = flat(path) {
                    reads.push((*parameter_position, case, field));
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
            Scalar::StructuralParameterIndexedRead { index, .. } => scalar(index, reads),
            Scalar::Boolean(expression) => boolean(expression, reads),
            _ => {}
        }
    }
    fn boolean<'a>(
        expression: &'a checked_trees::CheckedBooleanExpression,
        reads: &mut Vec<(u32, &'a str, &'a str)>,
    ) {
        match expression {
            Boolean::StructuralParameterField {
                parameter_position,
                path,
            } => {
                if let Some((case, field)) = flat(path) {
                    reads.push((*parameter_position, case, field));
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

/// Reads below a case payload's record member in one successor argument: each
/// `[Case, Field(member), Field(leaf)]` path binds `(parameter position, case,
/// member, leaf)` so the staged block can mint the member copy and its scalar
/// leaf read. Other shapes stay on the ordinary observation path and decline.
pub(in crate::unit::attached_unit::composed_control) fn nested_case_payload_reads<'a>(
    expression: &'a typed_trees_to_checked_trees::checked_trees::CheckedScalarExpression,
    reads: &mut Vec<(u32, &'a str, &'a str, &'a str)>,
) {
    use typed_trees_to_checked_trees::checked_trees::{
        CheckedBooleanExpression as Boolean, CheckedScalarExpression as Scalar,
        CheckedStructuralPredicatePathSegment as Segment,
    };
    fn nested(path: &[Segment]) -> Option<(&str, &str, &str)> {
        let [
            Segment::Case(case),
            Segment::Field(member),
            Segment::Field(leaf),
        ] = path
        else {
            return None;
        };
        Some((case.as_str(), member.as_str(), leaf.as_str()))
    }
    fn scalar<'a>(
        expression: &'a typed_trees_to_checked_trees::checked_trees::CheckedScalarExpression,
        reads: &mut Vec<(u32, &'a str, &'a str, &'a str)>,
    ) {
        match expression {
            Scalar::StructuralParameterField {
                parameter_position,
                path,
                ..
            } => {
                if let Some((case, member, leaf)) = nested(path) {
                    reads.push((*parameter_position, case, member, leaf));
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
            Scalar::StructuralParameterIndexedRead { index, .. } => scalar(index, reads),
            Scalar::Boolean(expression) => boolean(expression, reads),
            _ => {}
        }
    }
    fn boolean<'a>(
        expression: &'a typed_trees_to_checked_trees::checked_trees::CheckedBooleanExpression,
        reads: &mut Vec<(u32, &'a str, &'a str, &'a str)>,
    ) {
        match expression {
            Boolean::StructuralParameterField {
                parameter_position,
                path,
            } => {
                if let Some((case, member, leaf)) = nested(path) {
                    reads.push((*parameter_position, case, member, leaf));
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
