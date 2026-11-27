use crate::read::refuse_at;
/*
Mathematical machines that are generic over other machines.

`cauchy_at<machine Sequence, machine Modulus>` is a definition about every
machine with the contract each parameter declares. Nothing instantiates it,
so the signature gives it one more formal per machine parameter, of that
machine's function type, and a call of the parameter is an application of
that formal.

Such a machine's body is not lowered: lowering a call needs a machine to
call. A body that only binds values, cites laws and returns a value is read
here instead, through the clause reader, with each `let` standing for the
expression it is bound to. A body that does more is refused.
*/

use crate::source_state::data::{MachineParameterContract, TypeParameterKind};
use crate::source_state::expression::{ExpressionHandle, ExpressionNode};
use crate::source_state::machine::Machine;
use crate::source_state::signature::StateSignature;
use crate::source_state::statement::{
    StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode,
};
use semantic_vocabulary::{ClauseValue, MachineId};
use symbols::SymbolHandle;
use terminal_check_tables::Checked;

use super::Program;
use super::clause_terms::ClauseUse;
use super::contracts::{AppliedProposition, ClauseScope};
use crate::Unserved;
use crate::body::ValueType;

/// A machine parameter as the signature needs it: a function from its
/// parameters to its result.
#[derive(Debug, Clone)]
pub(crate) struct Callable {
    pub(crate) symbol: SymbolHandle,
    pub(crate) name: String,
    pub(crate) parameters: Vec<ValueType>,
    pub(crate) result: ValueType,
}

/// The body of a generic mathematical machine, read as terms.
#[derive(Debug, Clone, Default)]
pub(crate) struct TemplateBody {
    /// The machine's own machine parameters, in declared order.
    pub(crate) callables: Vec<Callable>,
    /// Every term the body states: its value and each citation's arguments
    /// index this table.
    pub(crate) values: Vec<ClauseValue>,
    /// The value the machine returns, when it returns one.
    pub(crate) value: Option<u32>,
    /// Each machine the body calls as a statement, with its arguments.
    pub(crate) citations: Vec<(SymbolHandle, Vec<u32>)>,
}

impl<'t> Program<'t> {
    /// Whether the machine is one this module reads: a machine of the
    /// mathematical signature whose every generic parameter is a machine
    /// with a declared signature.
    pub(crate) fn mathematical_template(&self, machine: &Machine) -> bool {
        if !self.mathematical || !self.is_generic_template(machine) {
            return false;
        }
        let parameters = self.typed.machine_type_parameters(machine);
        machine.conformance_bounds.is_empty()
            && !parameters.is_empty()
            && parameters.iter().all(|parameter| {
                matches!(
                    parameter.kind,
                    TypeParameterKind::Machine {
                        contract: MachineParameterContract::Structural(_)
                    } | TypeParameterKind::Value { .. }
                )
            })
    }

    /// The declared signature of a machine parameter of a generic
    /// mathematical machine.
    pub(super) fn callable_parameter(&self, symbol: SymbolHandle) -> Option<&'t StateSignature> {
        let typed = self.typed;
        if !self.mathematical || !typed.symbols.symbols().nodes().is_valid(symbol) {
            return None;
        }
        typed.machines().iter().find_map(|machine| {
            if !self.mathematical_template(machine) {
                return None;
            }
            typed
                .machine_type_parameters(machine)
                .into_iter()
                .find(|parameter| parameter.symbol == symbol)
                .and_then(|parameter| match &parameter.kind {
                    TypeParameterKind::Machine {
                        contract: MachineParameterContract::Structural(signature),
                    } => Some(signature),
                    _ => None,
                })
        })
    }

    /// The machine parameters of a generic mathematical machine.
    fn callables(&mut self, id: MachineId, machine: &'t Machine) -> Checked<Vec<Callable>> {
        let typed = self.typed;
        let unserved = |what| refuse_at(id, Unserved::Machine(what));
        let mut callables = Vec::new();
        for parameter in typed.machine_type_parameters(machine) {
            // A value parameter is one more formal of the machine, read with
            // its signature.
            if matches!(parameter.kind, TypeParameterKind::Value { .. }) {
                continue;
            }
            let TypeParameterKind::Machine {
                contract: MachineParameterContract::Structural(signature),
            } = &parameter.kind
            else {
                return Err(unserved(
                    "generic mathematical machine over something other than machines and values",
                ));
            };
            let formals = typed
                .state_parameters
                .span(signature.parameters)
                .ok_or_else(|| unserved("machine parameter lost its parameters"))?;
            let mut parameters = Vec::with_capacity(formals.len());
            for formal in formals.iter() {
                parameters.push(self.value_type(id, formal.type_reference)?);
            }
            if !signature.return_type.is_valid() {
                return Err(unserved(
                    "machine parameter of a mathematical machine without a result",
                ));
            }
            callables.push(Callable {
                symbol: parameter.symbol,
                name: parameter.name.as_str().to_owned(),
                parameters,
                result: self.value_type(id, signature.return_type)?,
            });
        }
        Ok(callables)
    }

    /// Reads the body of the generic mathematical machine `machine` in the
    /// scope its `requires` clauses are read in.
    pub(super) fn template_body(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        machine: SymbolHandle,
    ) -> Checked<TemplateBody> {
        let typed = self.typed;
        let unserved = |what| refuse_at(id, Unserved::Machine(what));
        let (declared, state) = self.declaration(id, machine)?;
        let mut body = TemplateBody {
            callables: self.callables(id, declared)?,
            ..TemplateBody::default()
        };
        let result = if state.return_type.is_valid() {
            Some(self.value_type(id, state.return_type)?)
        } else {
            None
        };
        // Each `let` stands for its value from here on, the way a named
        // proposition's parameter stands for its argument.
        self.applied_propositions.push(AppliedProposition {
            declaration: None,
            arguments: Vec::new(),
        });
        let read = (|| {
            let statements = typed.statement_table.statements(state.statement_nodes);
            for (position, statement) in statements.iter().enumerate() {
                let last = position + 1 == statements.len();
                match statement {
                    StatementNode::LocalData(local)
                        if local.initial_value.is_valid() && !local.is_mutable =>
                    {
                        self.applied_propositions
                            .last_mut()
                            .expect("pushed above")
                            .arguments
                            .push((local.symbol, local.initial_value));
                    }
                    StatementNode::Call(call) => {
                        if !call.machine_arguments.is_empty() || !call.evidence_arguments.is_empty()
                        {
                            return Err(unserved(
                                "generic mathematical machine cites a machine with static arguments",
                            ));
                        }
                        let arguments = typed.statement_table.expression_handles(call.arguments);
                        self.template_citation(
                            id,
                            scope,
                            call.target_symbol,
                            arguments,
                            &mut body,
                        )?;
                    }
                    StatementNode::Transition(transition)
                        if last
                            && transition.patterns.is_empty()
                            && transition.guard == TransitionGuardNode::Always
                            && transition.exit == TransitionExit::Ordinary
                            && !transition.continuation.is_valid()
                            && transition.proof_selectors.is_empty()
                            && transition.target.is_valid() =>
                    {
                        let TransitionTargetNode::Value(value) =
                            typed.statement_table.transition_target(transition.target)
                        else {
                            return Err(unserved(
                                "generic mathematical machine body that ends in something other than a value",
                            ));
                        };
                        self.template_value(id, scope, *value, result, &mut body)?;
                    }
                    StatementNode::Expression(expression) if last && result.is_some() => {
                        self.template_value(id, scope, *expression, result, &mut body)?;
                    }
                    StatementNode::Expression(expression)
                    | StatementNode::Evaluate { expression, .. } => {
                        self.template_called(id, scope, *expression, &mut body)?;
                    }
                    StatementNode::Assignment(assignment) => {
                        self.template_called(id, scope, assignment.value, &mut body)?;
                    }
                    _ => {
                        return Err(unserved(
                            "generic mathematical machine body other than lets, citations and one value",
                        ));
                    }
                }
            }
            Ok(())
        })();
        self.applied_propositions.pop();
        read?;
        if result.is_some() != body.value.is_some() {
            return Err(unserved(
                "generic mathematical machine body without its value",
            ));
        }
        Ok(body)
    }

    fn template_value(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        value: ExpressionHandle,
        result: Option<ValueType>,
        body: &mut TemplateBody,
    ) -> Checked<()> {
        let result = result.ok_or_else(|| {
            refuse_at(
                id,
                Unserved::Machine(
                    "generic mathematical machine returns a value it does not declare",
                ),
            )
        })?;
        let term = self
            .clause_term(
                id,
                scope,
                value,
                ClauseUse::Operand(result, &mut body.values),
            )?
            .operand(id)?;
        body.value = Some(term);
        Ok(())
    }

    /// A call written as a statement, or assigned to `_`: a citation.
    fn template_called(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        expression: ExpressionHandle,
        body: &mut TemplateBody,
    ) -> Checked<()> {
        let typed = self.typed;
        let ExpressionNode::Call(call) = typed.expression_table.expression(expression) else {
            return Err(refuse_at(
                id,
                Unserved::Machine("generic mathematical machine body statement other than a call"),
            ));
        };
        if call.receiver.is_valid()
            || !call.machine_arguments.is_empty()
            || !call.evidence_arguments.is_empty()
        {
            return Err(refuse_at(
                id,
                Unserved::Machine(
                    "generic mathematical machine cites a machine with static arguments",
                ),
            ));
        }
        let arguments = typed.expression_table.expression_handles(call.arguments);
        self.template_citation(id, scope, call.target_symbol, arguments, body)
    }

    fn template_citation(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        target: SymbolHandle,
        arguments: &[ExpressionHandle],
        body: &mut TemplateBody,
    ) -> Checked<()> {
        let typed = self.typed;
        let unserved = |what| refuse_at(id, Unserved::Machine(what));
        let cited = self
            .entered_machine(target)
            .filter(|cited| {
                typed
                    .machine_by_symbol(*cited)
                    .is_some_and(|machine| !self.is_generic_template(machine))
            })
            .ok_or_else(|| {
                unserved("generic mathematical machine cites something other than a machine with no parameters of its own")
            })?;
        // The signature needs the cited machine's law.
        self.machine(cited)?;
        let (_, entry) = self.declaration(id, cited)?;
        let formals = typed
            .state_parameters
            .span(entry.parameters)
            .ok_or_else(|| unserved("cited machine lost its parameters"))?;
        if formals.len() != arguments.len() {
            return Err(unserved("citation with the wrong number of arguments"));
        }
        let mut terms = Vec::with_capacity(arguments.len());
        for (formal, &argument) in formals.iter().zip(arguments) {
            let ty = self.value_type(id, formal.type_reference)?;
            terms.push(
                self.clause_term(
                    id,
                    scope,
                    argument,
                    ClauseUse::Operand(ty, &mut body.values),
                )?
                .operand(id)?,
            );
        }
        body.citations.push((cited, terms));
        Ok(())
    }
}
