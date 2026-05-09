use crate::plan::NativePlan;
use crate::state_calls::StateCallArgumentKind;
use omega_control_flow::StateKey;
use omega_core::arena::Arena;
use omega_core::symbols::SymbolHandle;
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AliasFlowPlan {
    pub aliases: Arena<AliasBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasBinding {
    pub caller_key: StateKey,
    pub statement_index: usize,
    pub callee_key: StateKey,
    pub parameter_symbol: SymbolHandle,
    pub parameter_name: ProgramName,
    pub argument: Expression,
    pub required: bool,
}

impl Default for AliasBinding {
    fn default() -> Self {
        Self {
            caller_key: StateKey::default(),
            statement_index: 0,
            callee_key: StateKey::default(),
            parameter_symbol: SymbolHandle::invalid(),
            parameter_name: ProgramName::default(),
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
                caller_key: state_call.source_key,
                statement_index: state_call.statement_index,
                callee_key: state_call.target_key,
                parameter_symbol: argument.parameter_symbol,
                parameter_name: argument.parameter_name.clone(),
                argument: argument.expression.clone(),
                required: state_call.required && argument.required,
            });
        }
    }

    plan
}
