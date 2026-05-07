use crate::native::abi::PlatformCallData;
use crate::native::host_calls::{HostCall, HostCallArgumentKind, HostCallPlan};
use crate::native::state_storage::StateStoragePlan;
use omega_core::arena::{Arena, HandleSpan};
use omega_typed_program::expression::Expression;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeDataPlan {
    pub objects: Arena<NativeDataObject>,
    pub bytes: Arena<u8>,
}

impl Default for NativeDataPlan {
    fn default() -> Self {
        Self {
            objects: Arena::new(),
            bytes: Arena::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeDataObject {
    pub symbol: String,
    pub offset: usize,
    pub bytes: HandleSpan<u8>,
    pub alignment: usize,
    pub source_machine: String,
    pub source_state: String,
    pub source_statement: usize,
}

impl Default for NativeDataObject {
    fn default() -> Self {
        Self {
            symbol: String::new(),
            offset: 0,
            bytes: HandleSpan::empty(),
            alignment: 1,
            source_machine: String::new(),
            source_state: String::new(),
            source_statement: 0,
        }
    }
}

pub fn build_native_data_plan(
    host_calls: &HostCallPlan,
    state_storage: &StateStoragePlan,
) -> NativeDataPlan {
    let mut data_plan = NativeDataPlan::default();

    for (_, host_call) in host_calls.calls.iter() {
        collect_host_call_data(host_calls, host_call, &mut data_plan);
    }
    collect_newline_data(host_calls, &mut data_plan);
    collect_static_string_assignment_data(state_storage, &mut data_plan);

    data_plan
}

fn collect_host_call_data(
    host_calls: &HostCallPlan,
    host_call: &HostCall,
    data_plan: &mut NativeDataPlan,
) {
    match host_call.data {
        PlatformCallData::FirstTextArgument { append_newline } => {
            collect_text_argument_data(host_calls, host_call, data_plan, append_newline);
        }
        PlatformCallData::MutableOutputBuffer { byte_capacity } => {
            collect_mutable_output_buffer(host_call, data_plan, byte_capacity);
        }
        PlatformCallData::None => {}
    }
}

fn collect_text_argument_data(
    host_calls: &HostCallPlan,
    host_call: &HostCall,
    data_plan: &mut NativeDataPlan,
    append_newline: bool,
) {
    let Some(arguments) = host_calls.arguments.span(host_call.arguments) else {
        return;
    };

    let Some(first_argument) = arguments.first() else {
        return;
    };

    let HostCallArgumentKind::Text(text) = &first_argument.kind else {
        return;
    };

    let mut bytes = text.as_bytes().to_vec();
    if append_newline {
        bytes.push(b'\n');
    }

    let offset = data_plan.bytes.len();
    let byte_span = data_plan.bytes.insert_many(bytes);
    let symbol_index = data_plan.objects.len() + 1;

    data_plan.objects.insert(NativeDataObject {
        symbol: format!("omega_string_literal_{symbol_index}"),
        offset,
        bytes: byte_span,
        alignment: 1,
        source_machine: host_call.machine.clone(),
        source_state: host_call.state.clone(),
        source_statement: host_call.statement_index,
    });
}

fn collect_mutable_output_buffer(
    host_call: &HostCall,
    data_plan: &mut NativeDataPlan,
    byte_capacity: usize,
) {
    let offset = data_plan.bytes.len();
    let byte_span = data_plan.bytes.insert_many(vec![0; byte_capacity]);
    let symbol_index = data_plan.objects.len() + 1;

    data_plan.objects.insert(NativeDataObject {
        symbol: format!("omega_mut_buffer_{symbol_index}"),
        offset,
        bytes: byte_span,
        alignment: 16,
        source_machine: host_call.machine.clone(),
        source_state: host_call.state.clone(),
        source_statement: host_call.statement_index,
    });
}

fn collect_newline_data(host_calls: &HostCallPlan, data_plan: &mut NativeDataPlan) {
    let needs_newline = host_calls.calls.iter().any(|(_, host_call)| {
        if !matches!(
            host_call.data,
            PlatformCallData::FirstTextArgument {
                append_newline: true
            }
        ) {
            return false;
        }
        host_calls
            .arguments
            .span(host_call.arguments)
            .and_then(|arguments| arguments.first())
            .is_some_and(|argument| matches!(argument.kind, HostCallArgumentKind::Expression(_)))
    });
    if !needs_newline {
        return;
    }

    let offset = data_plan.bytes.len();
    let byte_span = data_plan.bytes.insert_many(vec![b'\n']);

    data_plan.objects.insert(NativeDataObject {
        symbol: "omega_newline".to_owned(),
        offset,
        bytes: byte_span,
        alignment: 1,
        source_machine: String::new(),
        source_state: String::new(),
        source_statement: usize::MAX,
    });
}

fn collect_static_string_assignment_data(
    state_storage: &StateStoragePlan,
    data_plan: &mut NativeDataPlan,
) {
    for (_, mutation) in state_storage.mutations.iter() {
        if !mutation.required {
            continue;
        }

        collect_static_string_expression_data(
            &mutation.value,
            &mutation.machine,
            &mutation.state,
            mutation.statement_index,
            data_plan,
        );
    }
}

fn collect_static_string_expression_data(
    expression: &Expression,
    source_machine: &str,
    source_state: &str,
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
                source_machine: source_machine.to_owned(),
                source_state: source_state.to_owned(),
                source_statement,
            });
        }
        Expression::StructLiteral(struct_literal) => {
            for field in &struct_literal.fields {
                collect_static_string_expression_data(
                    &field.value,
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
                source_machine,
                source_state,
                source_statement,
                data_plan,
            );
            collect_static_string_expression_data(
                &binary.right,
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
