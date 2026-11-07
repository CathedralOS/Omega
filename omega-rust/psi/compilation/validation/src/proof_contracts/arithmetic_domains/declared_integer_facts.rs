//! Completed records supply usable fixed-integer comparison facts.
//!
//! Constructor operands cannot assume the result's predicate. After a checked
//! value arrives, supported comparisons enter the ordinary relation environment
//! with exact field identities and both operands. Existing write invalidation
//! retires these facts; other predicate forms keep their existing proof owners.

use super::guard_narrowing::projected_facts::{
    ProjectedOperand, contribute_projected_comparison, has_fixed_integer_meaning,
};
use super::{ExpressionHandle, ExpressionNode, ValueEnvironment, ordered_values};
use facts::PlaceSegment;
use typed_trees::data::DataDefinition;
use typed_trees::expression::BinaryOperator;
use typed_trees::types::TypeReferenceHandle;

/// The caller has independently established this exact value. Unknown case
/// identity exposes common fields only; selected payloads enter separately.
pub(crate) fn record_admitted_integer_fields(
    program: &dyn crate::CheckingInput,
    environment: &mut ValueEnvironment,
    subject: &ordered_values::Operand,
    reference: TypeReferenceHandle,
) {
    collect(program, environment, subject, reference, &mut Vec::new());
}

fn collect(
    program: &dyn crate::CheckingInput,
    environment: &mut ValueEnvironment,
    subject: &ordered_values::Operand,
    reference: TypeReferenceHandle,
    active: &mut Vec<TypeReferenceHandle>,
) {
    let Some(reference) = crate::field_domains::readable_type_reference(program, reference) else {
        return;
    };
    let Some(definition) = crate::field_domains::readable_nominal_definition(program, reference)
    else {
        return;
    };
    // Follow exact application handles, as owned storage projection does.
    // Finite Nest<Nest<T>> applications may share one declaration symbol.
    if active.contains(&reference) {
        return;
    }
    active.push(reference);
    for fact in program.proof_facts.span_or_empty(definition.where_facts) {
        if let Some(expression) = fact.as_expression() {
            contribute(
                program,
                environment,
                definition,
                reference,
                subject,
                *expression,
            );
        }
    }
    for field in program
        .data_members(definition)
        .iter()
        .filter_map(|member| member.as_field())
    {
        let segments = [PlaceSegment::Field {
            symbol: field.symbol,
        }];
        let Some(child_type) =
            crate::project_type_reference_from_segments(program, reference, &segments)
        else {
            continue;
        };
        // A stored reference is a descriptor, not establishment of its referent.
        if crate::reference_shell(program, child_type).is_some() {
            continue;
        }
        let child = ordered_values::Operand::Field {
            receiver: Box::new(subject.clone()),
            field: field.symbol,
            name: field.name.as_str().to_owned(),
            primitive: program.primitive_type_reference(child_type),
        };
        collect(program, environment, &child, child_type, active);
    }
    active.pop();
}

fn contribute(
    program: &dyn crate::CheckingInput,
    environment: &mut ValueEnvironment,
    definition: &DataDefinition,
    reference: TypeReferenceHandle,
    subject: &ordered_values::Operand,
    expression: ExpressionHandle,
) {
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return;
    };
    if binary.operator == BinaryOperator::And {
        for conjunct in [binary.left, binary.right] {
            contribute(
                program,
                environment,
                definition,
                reference,
                subject,
                conjunct,
            );
        }
        return;
    }
    let (Some(left), Some(right)) = (
        side(program, definition, reference, subject, binary.left),
        side(program, definition, reference, subject, binary.right),
    ) else {
        return;
    };
    let Some(spelling) = typed_trees::operator::comparison_operator_spelling(binary.operator)
    else {
        return;
    };
    if !typed_trees::operator::has_builtin_spelled_expression_meaning(
        program,
        definition.symbol,
        expression,
        spelling,
        &[left.declared_type, right.declared_type],
    ) {
        return;
    }
    contribute_projected_comparison(
        program,
        environment,
        binary.operator,
        (&left, binary.left),
        (&right, binary.right),
    );
}

fn side(
    program: &dyn crate::CheckingInput,
    definition: &DataDefinition,
    reference: TypeReferenceHandle,
    subject: &ordered_values::Operand,
    expression: ExpressionHandle,
) -> Option<ProjectedOperand> {
    if let ExpressionNode::Integer(literal) = program.expression_table.expression(expression) {
        if literal.landing().is_some_and(|landing| {
            landing.landed_type == numerics::literals::LandedIntegerType::Addr
        }) {
            return None;
        }
        let mut result = ProjectedOperand::literal(literal);
        result.declared_type = crate::landed_integer_literal_type_reference(program, expression);
        return Some(result);
    }
    let place = crate::canonical_place_from_expression(program, expression)?;
    let root = place.root.as_symbol()?;
    let segments = if root == definition.predicate_subject_symbol
        && definition.predicate_subject_symbol.is_valid()
    {
        place.segments
    } else if place.segments.is_empty() {
        vec![PlaceSegment::Field { symbol: root }]
    } else {
        return None;
    };
    let mut current_type = reference;
    let mut operand = subject.clone();
    for segment in segments {
        let PlaceSegment::Field { symbol } = segment else {
            return None;
        };
        let owner = crate::field_domains::readable_nominal_definition(program, current_type)?;
        let field = program.data_constructor_field(current_type, owner.symbol, symbol)?;
        // No unselected payload and no reference traversal can be inferred from
        // a field spelling. The exact instantiated common-field row must exist.
        if !program
            .data_members(owner)
            .iter()
            .filter_map(|member| member.as_field())
            .any(|declared| declared.symbol == field.symbol)
        {
            return None;
        }
        current_type = crate::project_type_reference_from_segments(
            program,
            current_type,
            &[PlaceSegment::Field {
                symbol: field.symbol,
            }],
        )?;
        if crate::reference_shell(program, current_type).is_some() {
            return None;
        }
        operand = ordered_values::Operand::Field {
            receiver: Box::new(operand),
            field: field.symbol,
            name: field.name.as_str().to_owned(),
            primitive: program.primitive_type_reference(current_type),
        };
    }
    if !has_fixed_integer_meaning(program, current_type) {
        return None;
    }
    Some(ProjectedOperand {
        // Field identity is exact even when another case repeats its name.
        // Keep declaration premises in the symbol-exact relation carrier.
        spelled: None,
        operand: Some(operand),
        declared_type: Some(current_type),
    })
}
