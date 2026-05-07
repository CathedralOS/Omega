use crate::native::plan::NativePlan;
use crate::native::state_calls::StateCallArgumentKind;
use omega_core::arena::Arena;
use omega_typed_program::expression::Expression;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AliasFlowPlan {
    pub aliases: Arena<AliasBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasBinding {
    pub caller_machine: String,
    pub caller_state: String,
    pub statement_index: usize,
    pub callee_machine: String,
    pub callee_state: String,
    pub parameter_name: String,
    pub argument: Expression,
    pub required: bool,
}

impl Default for AliasBinding {
    fn default() -> Self {
        Self {
            caller_machine: String::new(),
            caller_state: String::new(),
            statement_index: 0,
            callee_machine: String::new(),
            callee_state: String::new(),
            parameter_name: String::new(),
            argument: Expression::Integer(0),
            required: false,
        }
    }
}

pub fn build_alias_flow_plan(native_plan: &NativePlan) -> AliasFlowPlan {
    let mut plan = AliasFlowPlan::default();

    for (_, state_call) in native_plan.state_calls.calls.iter() {
        let Some(arguments) = native_plan.state_calls.arguments.span(state_call.arguments) else {
            continue;
        };

        for argument in arguments {
            if argument.kind != StateCallArgumentKind::MutableAlias {
                continue;
            }

            plan.aliases.insert(AliasBinding {
                caller_machine: state_call.source_machine.clone(),
                caller_state: state_call.source_state.clone(),
                statement_index: state_call.statement_index,
                callee_machine: state_call.target_machine.clone(),
                callee_state: state_call.target_state.clone(),
                parameter_name: argument.parameter_name.clone(),
                argument: argument.expression.clone(),
                required: state_call.required && argument.required,
            });
        }
    }

    plan
}
