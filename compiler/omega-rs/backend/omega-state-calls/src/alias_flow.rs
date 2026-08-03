use crate::{StateCallArgumentKind, StateCallPlan};
use omega_control_flow::StateKey;
use psi_arena::Arena;
use psi_checked_trees::expression::{ExpressionHandle, ExpressionTable};
use psi_checked_trees::name::Identifier;
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AliasFlowPlan {
    pub expressions: ExpressionTable,
    pub aliases: Arena<AliasBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasBinding {
    pub caller_key: StateKey,
    pub statement_index: usize,
    pub callee_key: StateKey,
    pub parameter_symbol: SymbolHandle,
    pub parameter_name: Identifier,
    pub argument: ExpressionHandle,
    pub required: bool,
}

impl Default for AliasBinding {
    fn default() -> Self {
        Self {
            caller_key: StateKey::default(),
            statement_index: 0,
            callee_key: StateKey::default(),
            parameter_symbol: SymbolHandle::invalid(),
            parameter_name: Identifier::default(),
            argument: ExpressionHandle::invalid(),
            required: false,
        }
    }
}

pub fn build_alias_flow_plan(state_calls: &StateCallPlan) -> AliasFlowPlan {
    let mut plan = AliasFlowPlan::default();

    for (_, state_call) in state_calls.calls.iter() {
        let Some(arguments) = state_calls.arguments.span(state_call.arguments) else {
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
                argument: plan
                    .expressions
                    .copy_from(&state_calls.expressions, argument.expression),
                required: state_call.required && argument.required,
            });
        }
    }

    plan
}
