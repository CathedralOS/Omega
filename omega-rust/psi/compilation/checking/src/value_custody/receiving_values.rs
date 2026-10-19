//! Join an established claim to the value that actually supplied it.
//!
//! Destination storage and expression completion are different coordinates.
//! A constructor folds completed children into its typed field/index paths;
//! it does not replace their claims. Follow those paths back to the child
//! producer before applying call-result correspondence. The same join serves
//! permission reconciliation and executable call custody, so a nested call
//! cannot be classified as an unreceived result by only one consumer.

use checked_trees::FlowPermissionEventFact;
use facts::PlaceSegment;
use language_semantics::PermissionEventSource;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;

/// The producing expression and remaining result-relative claim path for an
/// exact establishment. This describes custody, not a read of old storage or
/// replay of a local initializer: the event selects this statement's write.
pub fn receiving_value<'a>(
    program: &dyn crate::CheckingInput,
    segments: &'a arena::Arena<PlaceSegment>,
    event: &FlowPermissionEventFact,
) -> Option<(usize, ExpressionHandle, &'a [PlaceSegment])> {
    let statement_index = match event.source {
        PermissionEventSource::Statement { statement_index }
        | PermissionEventSource::Call {
            statement_index, ..
        } => statement_index,
        _ => return None,
    };
    let event_path = segments.span_or_empty(event.segments);
    let (mut expression, mut path) = if let Some(expression) = event.root.as_expression() {
        (expression, event_path)
    } else {
        let state = program.state_by_symbol(event.state_symbol)?;
        match program
            .statement_table
            .statement_at(state, statement_index)?
        {
            StatementNode::LocalData(local)
                if event.root == facts::PlaceRoot::Symbol(local.symbol) =>
            {
                (local.initial_value, event_path)
            }
            StatementNode::Assignment(assignment) => {
                let destination = crate::canonical_place_from_expression_in_state(
                    program,
                    state.symbol,
                    statement_index,
                    assignment.target,
                )?;
                if destination.root != event.root {
                    return None;
                }
                (
                    assignment.value,
                    event_path.strip_prefix(destination.segments.as_slice())?,
                )
            }
            _ => return None,
        }
    };
    if !expression.is_valid() {
        return None;
    }
    loop {
        expression = crate::ownership_source_expression(
            program,
            event.state_symbol,
            statement_index,
            expression,
        );
        let Some(segment) = path.first() else {
            break;
        };
        match (program.expression_table.expression(expression), segment) {
            (ExpressionNode::ArrayLiteral(values), PlaceSegment::FixedIndex { index }) => {
                expression = *program
                    .expression_table
                    .expression_handles(*values)
                    .get(*index)?;
            }
            (ExpressionNode::StructLiteral(literal), PlaceSegment::Case { variant }) => {
                let definition = program.data_definition_by_symbol(literal.type_symbol)?;
                let declared = program.data_variant(definition, *variant)?;
                if literal.case_name.as_ref() != Some(&declared.name) {
                    return None;
                }
            }
            (ExpressionNode::StructLiteral(literal), PlaceSegment::Field { symbol }) => {
                let (definition, declared) = program.any_field_owner(*symbol)?;
                if definition.symbol != literal.type_symbol {
                    return None;
                }
                expression = program
                    .expression_table
                    .struct_fields(literal.fields)
                    .iter()
                    .find(|field| field.name == declared.name)?
                    .value;
            }
            _ => break,
        }
        path = &path[1..];
    }
    Some((statement_index, expression, path))
}
