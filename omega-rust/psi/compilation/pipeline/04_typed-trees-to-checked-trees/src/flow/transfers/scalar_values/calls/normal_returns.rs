//! Capture normal values through selected scalar state paths.
//!
//! Each local, guard, return and delivered argument rejoins its checked source
//! binding and selected pure expression. Unknown guards retain both paths;
//! impossible guards skip a path, and explicit crashes contribute no normal
//! value. State transfers deliver copied values into a fresh scalar namespace.
//! A cycle requiring induction or a missing selected operation withholds the
//! whole capture, rather than trusting a partially inspected path.

use super::{CallValues, CapturedValue, CheckedScalarExpressionRole, FlowBuildContext};
use checked_trees::{BorrowFacts, CheckedScalarExpression};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{
    StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetHandle,
    TransitionTargetNode,
};

enum CapturedReturns<Value> {
    NoNormalValue,
    Value(Value),
}

impl<Value: CapturedValue> CapturedReturns<Value> {
    fn join(self, other: Self) -> Option<Self> {
        match (self, other) {
            (Self::NoNormalValue, other) | (other, Self::NoNormalValue) => Some(other),
            (Self::Value(left), Self::Value(right)) => left.join(right).map(Self::Value),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn capture<Value: CapturedValue>(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    context: &mut FlowBuildContext,
    machine: &Machine,
    state: &State,
    symbols: Vec<SymbolHandle>,
    values: CallValues<Value>,
) -> Option<Value> {
    let mut capture = NormalReturnCapture {
        program,
        borrow,
        context,
        machine,
        visiting: Vec::new(),
    };
    match capture.state(state, symbols, values)? {
        CapturedReturns::Value(value) => Some(value),
        CapturedReturns::NoNormalValue => None,
    }
}

struct NormalReturnCapture<'capture, 'plans> {
    program: &'capture dyn ::validation::CheckingInput,
    borrow: &'capture BorrowFacts,
    context: &'capture mut FlowBuildContext<'plans>,
    machine: &'capture Machine,
    visiting: Vec<SymbolHandle>,
}

impl NormalReturnCapture<'_, '_> {
    fn state<Value: CapturedValue>(
        &mut self,
        state: &State,
        mut symbols: Vec<SymbolHandle>,
        mut values: CallValues<Value>,
    ) -> Option<CapturedReturns<Value>> {
        if self.visiting.contains(&state.symbol) {
            return None;
        }
        self.visiting.push(state.symbol);
        let result = self.statements(state, &mut symbols, &mut values);
        self.visiting.pop();
        result
    }

    fn statements<Value: CapturedValue>(
        &mut self,
        state: &State,
        symbols: &mut Vec<SymbolHandle>,
        values: &mut CallValues<Value>,
    ) -> Option<CapturedReturns<Value>> {
        let statements = self
            .program
            .statement_table
            .statements(state.statement_nodes);
        let mut returned = CapturedReturns::NoNormalValue;
        let mut immutable_local_count = 0_u32;
        for (statement_index, statement) in statements.iter().enumerate() {
            if let Some(call) = statement.as_call() {
                super::retains_values_across_unit_call(
                    self.program,
                    self.borrow,
                    self.context,
                    self.machine,
                    state,
                    statement_index,
                    call,
                    symbols,
                    values,
                )?;
                continue;
            }
            let ordinal = u32::try_from(statement_index).ok()?;
            if let Some(transition) = statement.as_transition() {
                let condition = match transition.guard {
                    TransitionGuardNode::Always => Some(true),
                    TransitionGuardNode::When(source) => {
                        let expression = selected(
                            self.context,
                            state.symbol,
                            ordinal,
                            CheckedScalarExpressionRole::Guard,
                            source,
                            SymbolHandle::invalid(),
                            symbols,
                        )?;
                        Value::boolean(expression, values)
                    }
                };
                if condition != Some(false) {
                    let primary = if matches!(transition.exit, TransitionExit::Crash(_)) {
                        CapturedReturns::NoNormalValue
                    } else {
                        self.target(state, ordinal, transition.target, false, symbols, values)?
                    };
                    returned = returned.join(primary)?;
                }
                if condition != Some(true) {
                    if transition.continuation.is_valid() {
                        let continuation = self.target(
                            state,
                            ordinal,
                            transition.continuation,
                            true,
                            symbols,
                            values,
                        )?;
                        returned = returned.join(continuation)?;
                    } else {
                        // The guard's refutation continues in the same selected
                        // local namespace. No pre-guard value is manufactured.
                        continue;
                    }
                }
                return Some(returned);
            }
            let (source, destination, role) = match statement {
                StatementNode::LocalData(local) => {
                    if !local.symbol.is_valid()
                        || symbols.contains(&local.symbol)
                        || values
                            .storage
                            .iter()
                            .any(|(symbol, _)| *symbol == local.symbol)
                        || self
                            .program
                            .primitive_type_reference(local.type_reference)
                            .is_none()
                    {
                        return None;
                    }
                    (
                        local.initial_value,
                        local.symbol,
                        if local.is_mutable {
                            CheckedScalarExpressionRole::StorageInitializer
                        } else {
                            CheckedScalarExpressionRole::LocalInitializer {
                                binding_ordinal: immutable_local_count,
                            }
                        },
                    )
                }
                StatementNode::Assignment(assignment) => {
                    let path = self
                        .program
                        .expression_table
                        .name_path_node(assignment.target)?;
                    if !values
                        .storage
                        .iter()
                        .any(|(symbol, _)| *symbol == path.symbol)
                    {
                        return None;
                    }
                    (
                        assignment.value,
                        path.symbol,
                        CheckedScalarExpressionRole::AssignmentValue,
                    )
                }
                StatementNode::Expression(expression)
                    if statement_index + 1 == statements.len() =>
                {
                    (
                        *expression,
                        SymbolHandle::invalid(),
                        CheckedScalarExpressionRole::Return,
                    )
                }
                _ => return None,
            };
            let expression = selected(
                self.context,
                state.symbol,
                ordinal,
                role,
                source,
                destination,
                symbols,
            )?;
            let value = Value::evaluate_local(expression, values);
            match role {
                CheckedScalarExpressionRole::Return => {
                    return returned.join(CapturedReturns::Value(value?));
                }
                CheckedScalarExpressionRole::LocalInitializer { .. } => {
                    symbols.push(destination);
                    values.bindings.push(value);
                    immutable_local_count = immutable_local_count.checked_add(1)?;
                }
                CheckedScalarExpressionRole::StorageInitializer => {
                    values.storage.push((destination, value?))
                }
                CheckedScalarExpressionRole::AssignmentValue => {
                    let (_, current) = values
                        .storage
                        .iter_mut()
                        .find(|(symbol, _)| *symbol == destination)?;
                    *current = value?;
                }
                _ => return None,
            }
        }
        // A final declaration, store or refutable guard is an uncaptured normal
        // exit, not evidence that no normal value can escape this path.
        None
    }

    #[allow(clippy::too_many_arguments)]
    fn target<Value: CapturedValue>(
        &mut self,
        state: &State,
        ordinal: u32,
        target: TransitionTargetHandle,
        continuation: bool,
        symbols: &[SymbolHandle],
        values: &mut CallValues<Value>,
    ) -> Option<CapturedReturns<Value>> {
        if !target.is_valid() {
            return None;
        }
        match self.program.statement_table.transition_target(target) {
            TransitionTargetNode::Value(source) => {
                let role = if continuation {
                    CheckedScalarExpressionRole::ContinuationReturn
                } else {
                    CheckedScalarExpressionRole::Return
                };
                let expression = selected(
                    self.context,
                    state.symbol,
                    ordinal,
                    role,
                    *source,
                    SymbolHandle::invalid(),
                    symbols,
                )?;
                Some(CapturedReturns::Value(Value::evaluate_local(
                    expression, values,
                )?))
            }
            TransitionTargetNode::Named {
                path,
                arguments,
                static_machine_parameter,
                evidence_arguments,
                ..
            } => {
                if static_machine_parameter.is_valid() || !evidence_arguments.is_empty() {
                    return None;
                }
                let target_index = validation::activation_target_state_index(
                    self.program,
                    self.machine,
                    path.symbol,
                )?;
                let target_state = self
                    .program
                    .machine_states(self.machine)
                    .get(target_index)?;
                let arguments = self.program.statement_table.expression_handles(*arguments);
                let parameters = self.program.state_parameters(target_state);
                if parameters.len() != arguments.len() {
                    return None;
                }
                let mut delivered = CallValues {
                    bindings: Vec::new(),
                    storage: Vec::new(),
                    fields: Vec::new(),
                };
                let mut target_symbols = Vec::new();
                for (position, (parameter, source)) in parameters.iter().zip(arguments).enumerate()
                {
                    if parameter.relevance.is_erased() {
                        continue;
                    }
                    if parameter.is_const
                        || !parameter.symbol.is_valid()
                        || self
                            .program
                            .primitive_type_reference(parameter.type_reference)
                            .is_none()
                        || target_symbols.contains(&parameter.symbol)
                    {
                        return None;
                    }
                    let argument_ordinal = u32::try_from(position).ok()?;
                    let role = if continuation {
                        CheckedScalarExpressionRole::TransitionContinuationArgument {
                            argument_ordinal,
                        }
                    } else {
                        CheckedScalarExpressionRole::TransitionArgument { argument_ordinal }
                    };
                    let expression = selected(
                        self.context,
                        state.symbol,
                        ordinal,
                        role,
                        *source,
                        parameter.symbol,
                        symbols,
                    )?;
                    let value = Value::evaluate_local(expression, values);
                    target_symbols.push(parameter.symbol);
                    if parameter.is_mutable {
                        delivered.bindings.push(None);
                        delivered.storage.push((parameter.symbol, value?));
                    } else {
                        delivered.bindings.push(value);
                    }
                }
                self.state(target_state, target_symbols, delivered)
            }
            TransitionTargetNode::Terminal => None,
        }
    }
}

fn selected<'plans>(
    context: &FlowBuildContext<'plans>,
    state: SymbolHandle,
    ordinal: u32,
    role: CheckedScalarExpressionRole,
    source: ExpressionHandle,
    destination: SymbolHandle,
    symbols: &[SymbolHandle],
) -> Option<&'plans CheckedScalarExpression> {
    let (binding, expression) = context
        .selected_scalar_at(state, ordinal, |binding| {
            binding.role == role
                && binding.expression == source
                && binding.destination == destination
        })
        .flatten()?;
    if context
        .scalar_expressions
        .binding_symbols
        .span_or_empty(binding.symbols)
        != symbols
    {
        return None;
    }
    Some(expression)
}
