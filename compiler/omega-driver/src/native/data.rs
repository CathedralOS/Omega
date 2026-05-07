use crate::native::host_calls::{HostCall, HostCallArgumentKind, HostCallPlan};
use omega_core::arena::{Arena, HandleSpan};

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

pub fn build_native_data_plan(host_calls: &HostCallPlan) -> NativeDataPlan {
    let mut data_plan = NativeDataPlan::default();

    for (_, host_call) in host_calls.calls.iter() {
        collect_host_call_data(host_calls, host_call, &mut data_plan);
    }

    data_plan
}

fn collect_host_call_data(
    host_calls: &HostCallPlan,
    host_call: &HostCall,
    data_plan: &mut NativeDataPlan,
) {
    let append_newline = if host_call.platform_call.ends_with(".write_line") {
        true
    } else if host_call.platform_call.ends_with(".write") {
        false
    } else {
        return;
    };

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
