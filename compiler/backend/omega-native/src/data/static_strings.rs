use crate::control_flow::StateKey;
use crate::data::{NativeDataObject, NativeDataPlan};
use crate::state_storage::StateStoragePlan;
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;

pub(super) fn collect_static_string_assignment_data(
    state_storage: &StateStoragePlan,
    data_plan: &mut NativeDataPlan,
) {
    for (_, mutation) in state_storage.mutations.iter() {
        if !mutation.required {
            continue;
        }

        collect_static_string_expression_data(
            &mutation.value,
            mutation.source_key,
            &mutation.machine,
            &mutation.state,
            mutation.statement_index,
            data_plan,
        );
    }
}

fn collect_static_string_expression_data(
    expression: &Expression,
    source_key: StateKey,
    source_machine: &ProgramName,
    source_state: &ProgramName,
    source_statement: usize,
    data_plan: &mut NativeDataPlan,
) {
    match expression {
        Expression::String(value) => {
            let offset = data_plan.bytes.len();
            let bytes = if value.is_empty() {
                vec![0]
            } else {
                value.as_bytes().to_vec()
            };
            let byte_span = data_plan.bytes.insert_many(bytes);
            let symbol_index = data_plan.objects.len() + 1;

            data_plan.objects.insert(NativeDataObject {
                symbol: format!("omega_string_literal_{symbol_index}"),
                offset,
                bytes: byte_span,
                alignment: 1,
                source_key,
                source_machine: source_machine.clone(),
                source_state: source_state.clone(),
                source_statement,
            });
        }
        Expression::StructLiteral(struct_literal) => {
            for field in &struct_literal.fields {
                collect_static_string_expression_data(
                    &field.value,
                    source_key,
                    source_machine,
                    source_state,
                    source_statement,
                    data_plan,
                );
            }
        }
        Expression::ArrayLiteral(elements) => {
            for element in elements {
                collect_static_string_expression_data(
                    element,
                    source_key,
                    source_machine,
                    source_state,
                    source_statement,
                    data_plan,
                );
            }
        }
        Expression::Binary(binary) => {
            collect_static_string_expression_data(
                &binary.left,
                source_key,
                source_machine,
                source_state,
                source_statement,
                data_plan,
            );
            collect_static_string_expression_data(
                &binary.right,
                source_key,
                source_machine,
                source_state,
                source_statement,
                data_plan,
            );
        }
        Expression::Boolean(_)
        | Expression::Float(_)
        | Expression::Indexed(_)
        | Expression::Integer(_)
        | Expression::Mutable(_)
        | Expression::Name(_) => {}
    }
}
