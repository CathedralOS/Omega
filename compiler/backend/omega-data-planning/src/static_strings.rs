use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use omega_control_flow::StateKey;
use omega_state_storage::StateStoragePlan;
use omega_state_values::StateValuePlan;
use omega_target_operations::{TargetDataObject, TargetDataObjectKind, TargetDataPlan};

pub(super) fn collect_static_string_assignment_data(
    state_storage: &StateStoragePlan,
    data_plan: &mut TargetDataPlan,
) {
    for (_, mutation) in state_storage.mutations.iter() {
        if !mutation.required {
            continue;
        }

        collect_static_string_expression_data(
            &state_storage.expressions,
            mutation.value,
            mutation.source_key,
            mutation.statement_index,
            data_plan,
        );
    }
}

pub(super) fn collect_static_string_value_data(
    state_values: &StateValuePlan,
    data_plan: &mut TargetDataPlan,
) {
    for (_, value) in state_values.values.iter() {
        if !value.required {
            continue;
        }

        collect_static_string_expression_data(
            &state_values.expressions,
            value.expression,
            value.source_key,
            value.statement_index,
            data_plan,
        );
    }
}

fn collect_static_string_expression_data(
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    source_key: StateKey,
    source_statement: usize,
    data_plan: &mut TargetDataPlan,
) {
    match expressions.expression(expression) {
        ExpressionNode::String(value) => {
            let offset = data_plan.bytes.len();
            let byte_span = if value.is_empty() {
                data_plan.bytes.insert_many(std::iter::once(0))
            } else {
                data_plan
                    .bytes
                    .insert_many(value.as_bytes().iter().copied())
            };
            let symbol_index = data_plan.objects.len() + 1;

            data_plan.objects.insert(TargetDataObject {
                symbol: format!("omega_string_literal_{symbol_index}"),
                kind: TargetDataObjectKind::StaticString,
                offset,
                bytes: byte_span,
                alignment: 1,
                source_key,
                source_statement,
            });
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in expressions.struct_fields(struct_literal.fields) {
                collect_static_string_expression_data(
                    expressions,
                    field.value,
                    source_key,
                    source_statement,
                    data_plan,
                );
            }
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in expressions.expression_handles(*elements) {
                collect_static_string_expression_data(
                    expressions,
                    *element,
                    source_key,
                    source_statement,
                    data_plan,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_static_string_expression_data(
                expressions,
                binary.left,
                source_key,
                source_statement,
                data_plan,
            );
            collect_static_string_expression_data(
                expressions,
                binary.right,
                source_key,
                source_statement,
                data_plan,
            );
        }
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                collect_static_string_expression_data(
                    expressions,
                    call.receiver,
                    source_key,
                    source_statement,
                    data_plan,
                );
            }

            for argument in expressions.expression_handles(call.arguments) {
                collect_static_string_expression_data(
                    expressions,
                    *argument,
                    source_key,
                    source_statement,
                    data_plan,
                );
            }
        }
        ExpressionNode::Cast(cast) => {
            collect_static_string_expression_data(
                expressions,
                cast.value,
                source_key,
                source_statement,
                data_plan,
            );
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Indexed(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Mutable(_)
        | ExpressionNode::Name(_) => {}
    }
}
