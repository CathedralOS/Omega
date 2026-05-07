use crate::ir::expression::Expression;
use crate::native::abi::PlatformCallData;
use crate::native::host_calls::{HostCall, HostCallArgumentKind, HostCallPlan};
use omega_core::arena::Arena;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeTextPlan {
    pub uses: Arena<RuntimeTextUse>,
    pub buffers: Arena<RuntimeTextBuffer>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTextUse {
    pub machine: String,
    pub state: String,
    pub statement_index: usize,
    pub platform_call: String,
    pub expression: Expression,
    pub source: RuntimeTextSource,
    pub append_newline: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTextBuffer {
    pub machine: String,
    pub state: String,
    pub statement_index: usize,
    pub platform_call: String,
    pub target: Expression,
    pub byte_capacity: usize,
}

impl Default for RuntimeTextBuffer {
    fn default() -> Self {
        Self {
            machine: String::new(),
            state: String::new(),
            statement_index: 0,
            platform_call: String::new(),
            target: Expression::String(String::new()),
            byte_capacity: 0,
        }
    }
}

impl Default for RuntimeTextUse {
    fn default() -> Self {
        Self {
            machine: String::new(),
            state: String::new(),
            statement_index: 0,
            platform_call: String::new(),
            expression: Expression::String(String::new()),
            source: RuntimeTextSource::OtherExpression,
            append_newline: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimeTextSource {
    StoredPlace,
    GeneratedString,
    MutablePlace,
    #[default]
    OtherExpression,
}

pub fn build_runtime_text_plan(host_calls: &HostCallPlan) -> RuntimeTextPlan {
    let mut plan = RuntimeTextPlan::default();

    for (_, host_call) in host_calls.calls.iter() {
        collect_host_call_runtime_text(host_calls, host_call, &mut plan);
    }

    plan
}

fn collect_host_call_runtime_text(
    host_calls: &HostCallPlan,
    host_call: &HostCall,
    plan: &mut RuntimeTextPlan,
) {
    match host_call.data {
        PlatformCallData::FirstTextArgument { append_newline } => {
            collect_runtime_text_use(host_calls, host_call, plan, append_newline);
        }
        PlatformCallData::MutableOutputBuffer { byte_capacity } => {
            collect_runtime_text_buffer(host_calls, host_call, plan, byte_capacity);
        }
        PlatformCallData::None => {}
    }
}

fn collect_runtime_text_use(
    host_calls: &HostCallPlan,
    host_call: &HostCall,
    plan: &mut RuntimeTextPlan,
    append_newline: bool,
) {
    let Some(first_argument) = first_host_argument(host_calls, host_call) else {
        return;
    };

    if let HostCallArgumentKind::Expression(expression) = &first_argument.kind {
        plan.uses.insert(RuntimeTextUse {
            machine: host_call.machine.clone(),
            state: host_call.state.clone(),
            statement_index: host_call.statement_index,
            platform_call: host_call.platform_call.clone(),
            expression: expression.clone(),
            source: classify_runtime_text_source(expression),
            append_newline,
        });
    }
}

fn collect_runtime_text_buffer(
    host_calls: &HostCallPlan,
    host_call: &HostCall,
    plan: &mut RuntimeTextPlan,
    byte_capacity: usize,
) {
    let Some(first_argument) = first_host_argument(host_calls, host_call) else {
        return;
    };

    let HostCallArgumentKind::Expression(Expression::Mutable(target)) = &first_argument.kind else {
        return;
    };

    plan.buffers.insert(RuntimeTextBuffer {
        machine: host_call.machine.clone(),
        state: host_call.state.clone(),
        statement_index: host_call.statement_index,
        platform_call: host_call.platform_call.clone(),
        target: (**target).clone(),
        byte_capacity,
    });
}

fn first_host_argument<'plan>(
    host_calls: &'plan HostCallPlan,
    host_call: &HostCall,
) -> Option<&'plan crate::native::host_calls::HostCallArgument> {
    host_calls
        .arguments
        .span(host_call.arguments)
        .and_then(|arguments| arguments.first())
}

fn classify_runtime_text_source(expression: &Expression) -> RuntimeTextSource {
    match expression {
        Expression::Name(_) | Expression::Indexed(_) => RuntimeTextSource::StoredPlace,
        Expression::Binary(_) => RuntimeTextSource::GeneratedString,
        Expression::Mutable(_) => RuntimeTextSource::MutablePlace,
        Expression::ArrayLiteral(_)
        | Expression::Boolean(_)
        | Expression::Float(_)
        | Expression::Integer(_)
        | Expression::StructLiteral(_)
        | Expression::String(_) => RuntimeTextSource::OtherExpression,
    }
}
