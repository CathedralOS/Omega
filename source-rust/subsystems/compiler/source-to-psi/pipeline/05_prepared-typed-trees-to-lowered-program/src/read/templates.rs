use crate::read::refuse_at;
/*
Mathematical machines that are generic over other machines.

`cauchy_at<machine Sequence, machine Modulus>` is a definition about every
machine with the contract each parameter declares. Nothing instantiates it,
so the signature gives it one more formal per machine parameter, of that
machine's function type, and a call of the parameter is an application of
that formal.

A clause or a citation may pass such a machine its machine arguments: a
machine parameter of the machine being read, or a concrete mathematical
machine with exactly the parameter's signature and no premise, which is then
the definition the signature gives it (`machine_argument`). One generic
machine may so cite another.

A body that only binds values, cites laws and returns a value is read here,
through the clause reader, with each `let` standing for the expression it is
bound to, and is not lowered. A body that dispatches is lowered, with every
machine declared inside it, as is a machine that states nothing and any body
the clause reader cannot read, and read by the signature reader like any other:
there a call of a machine parameter, or of a generic machine at some
machines, is an `ExpressionKind::Applied` (`mathematical_application`),
which applies no operation and only the signature reads. A body that is not
generic calls or cites a generic machine at concrete machines the same way.
*/

use crate::source_state::data::{MachineParameterContract, TypeParameterKind};
use crate::source_state::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
use crate::source_state::machine::Machine;
use crate::source_state::signature::StateSignature;
use crate::source_state::statement::{
    StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode,
};
use semantic_vocabulary::{
    ClauseCarrier, ClauseDeclaration, ClauseOperand, ClauseValue, MachineId,
};
use symbols::SymbolHandle;
use terminal_check_tables::Checked;

use super::Program;
use super::clause_terms::ClauseUse;
use super::contracts::{AppliedProposition, ClauseScope};
use crate::Unserved;
use crate::body::{AppliedHead, Expression, ExpressionKind, PassedMachine, ValueType};

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
    /// The body dispatches, so it is lowered and read like any other
    /// mathematical body, with each machine parameter a bound function.
    pub(crate) lowered: bool,
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

    /// Whether `machine` is generic over types only, with no bound: each of
    /// its closed applications is a machine typed preparation generated, so
    /// the mathematical signature reads those and never the template.
    pub(crate) fn type_template(&self, machine: &Machine) -> bool {
        let parameters = self.typed.machine_type_parameters(machine);
        self.mathematical
            && self.is_generic_template(machine)
            && machine.conformance_bounds.is_empty()
            && !parameters.is_empty()
            && parameters
                .iter()
                .all(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
    }

    /// Whether `machine` realizes a row of a conformance generic over types
    /// only (`SameCount<T> satisfies ...`): each closed application of that
    /// conformance has its own copy of the row, which the mathematical
    /// signature reads instead.
    pub(crate) fn conformance_template_row(&self, machine: &Machine) -> bool {
        let typed = self.typed;
        self.mathematical
            && typed.conformances().iter().any(|conformance| {
                let parameters = typed.conformance_type_parameters(conformance);
                !parameters.is_empty()
                    && parameters
                        .iter()
                        .all(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
                    && conformance
                        .rows
                        .iter()
                        .any(|row| row.realization_machine == machine.symbol)
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

    /// The generic mathematical machine that `instance` is a closed copy
    /// of, when the typed trees made it for a call at concrete machines,
    /// with the machine selected for each of its machine parameters, in
    /// declared order.
    pub(super) fn generic_mathematical_origin(
        &self,
        instance: SymbolHandle,
    ) -> Option<(SymbolHandle, Vec<&'t StaticMachineArgument>)> {
        use crate::source_state::static_applications::StaticMachineBindingValue;
        let typed = self.typed;
        let Ok(Some(crate::source_state::MachineInstantiation::Specialization(receipt))) =
            typed.machine_instantiation(instance)
        else {
            return None;
        };
        let template = typed
            .machine_by_symbol(receipt.template)
            .filter(|template| self.mathematical_template(template))?;
        let bindings = typed
            .expression_table
            .static_machine_application_bindings(receipt.binding_application);
        let mut selected = Vec::new();
        for parameter in typed.machine_type_parameters(template) {
            if !matches!(parameter.kind, TypeParameterKind::Machine { .. }) {
                continue;
            }
            // A parameter with no selected machine leaves the copy
            // unexplained; a citation of it is then refused as written.
            let argument = bindings.iter().find_map(|binding| match &binding.value {
                StaticMachineBindingValue::Machine(argument)
                    if binding.parameter == parameter.symbol =>
                {
                    Some(argument)
                }
                _ => None,
            });
            selected.push(argument?);
        }
        Some((template.symbol, selected))
    }

    /// The function types a generic mathematical machine's machine
    /// parameters declare, in declared order: each parameter's formals and
    /// result.
    pub(super) fn required_callables(
        &mut self,
        id: MachineId,
        callee: &'t Machine,
    ) -> Checked<Vec<(Vec<ValueType>, ValueType)>> {
        Ok(self
            .callables(id, callee)?
            .into_iter()
            .map(|callable| (callable.parameters, callable.result))
            .collect())
    }

    /// One machine argument of an application of a generic mathematical
    /// machine, pushed to `values` as a function passed whole: a machine
    /// parameter of the machine being read, or a concrete mathematical
    /// machine. Either must have exactly the formals and result `required`
    /// declares. A concrete machine is the definition the signature gives
    /// it, so it must be a total function of its formals: one with a premise
    /// is not, and a generic one has no single definition to pass.
    pub(super) fn machine_argument(
        &mut self,
        id: MachineId,
        argument: &StaticMachineArgument,
        required: &(Vec<ValueType>, ValueType),
        values: &mut Vec<ClauseValue>,
    ) -> Checked<u32> {
        let unserved = |what| refuse_at(id, Unserved::Contract(what));
        let (_, member, result) = self.passed_machine(id, argument, required)?;
        let ValueType::Record(record, ..) = result else {
            return Err(unserved(
                "machine argument whose result is not mathematical data",
            ));
        };
        let position =
            u32::try_from(values.len()).map_err(|_| unserved("clause term table overflow"))?;
        values.push(ClauseValue {
            carrier: ClauseCarrier::Structural(record),
            operand: ClauseOperand::Application {
                declaration: ClauseDeclaration {
                    member: (member.arena_index(), member.generation()),
                    supplier: None,
                },
                arguments: Vec::new(),
            },
        });
        Ok(position)
    }

    /// What a machine argument of a generic mathematical machine passes,
    /// with the symbol a clause names it by and its result: a machine
    /// parameter, or a concrete mathematical machine, either with exactly
    /// the formals and result `required` declares.
    fn passed_machine(
        &mut self,
        id: MachineId,
        argument: &StaticMachineArgument,
        required: &(Vec<ValueType>, ValueType),
    ) -> Checked<(PassedMachine, SymbolHandle, ValueType)> {
        let typed = self.typed;
        let unserved = |what| refuse_at(id, Unserved::Contract(what));
        if argument.application.is_some()
            || argument.const_literal.is_some()
            || argument.evidence_projection.is_some()
        {
            return Err(unserved(
                "machine argument of a generic mathematical machine with arguments of its own",
            ));
        }
        let (passed, member, parameters, result) = if let Some(signature) =
            self.callable_parameter(argument.symbol)
        {
            let formals = typed
                .state_parameters
                .span(signature.parameters)
                .ok_or_else(|| unserved("machine parameter lost its parameters"))?;
            let mut parameters = Vec::with_capacity(formals.len());
            for formal in formals.iter() {
                parameters.push(self.value_type(id, formal.type_reference)?);
            }
            (
                PassedMachine::Parameter(argument.symbol),
                argument.symbol,
                parameters,
                self.value_type(id, signature.return_type)?,
            )
        } else {
            let machine = self
                .entered_machine(argument.symbol)
                .and_then(|symbol| typed.machine_by_symbol(symbol))
                .filter(|machine| {
                    self.proof_machine(machine)
                        && !self.is_generic_template(machine)
                        && typed
                            .machine_states(machine)
                            .first()
                            .is_some_and(|entry| entry.return_type.is_valid())
                })
                .ok_or_else(|| {
                    unserved(
                        "machine argument other than a machine parameter or a mathematical machine with a result",
                    )
                })?;
            // The signature needs the passed machine's definition.
            let read = self.machine(machine.symbol)?;
            if self
                .signatures
                .get(read)
                .is_none_or(|signature| !signature.requires.is_empty())
            {
                return Err(unserved(
                    "machine argument of a generic mathematical machine with a premise",
                ));
            }
            let (_, entry) = self.declaration(id, machine.symbol)?;
            let formals = typed
                .state_parameters
                .span(entry.parameters)
                .ok_or_else(|| unserved("machine argument lost its parameters"))?;
            let mut parameters = Vec::with_capacity(formals.len());
            for formal in formals.iter() {
                parameters.push(self.value_type(id, formal.type_reference)?);
            }
            (
                PassedMachine::Machine(read),
                machine.symbol,
                parameters,
                self.value_type(id, entry.return_type)?,
            )
        };
        if parameters != required.0 || result != required.1 {
            return Err(unserved(
                "machine argument whose signature differs from the machine parameter's",
            ));
        }
        Ok((passed, member, result))
    }

    /// A call in a mathematical body that applies a machine parameter of
    /// the machine being read, or that passes machines to a generic
    /// mathematical machine, as `ExpressionKind::Applied`. A call at
    /// concrete machines names the closed copy the typed trees made for it;
    /// it is read as the generic machine applied to the machines that copy
    /// was made for. `None` for any other call.
    pub(super) fn mathematical_application<'a>(
        &mut self,
        id: MachineId,
        slots: &[super::Slot],
        target: SymbolHandle,
        machine_arguments: &'a [StaticMachineArgument],
        arguments: &[ExpressionHandle],
        source: crate::SourceOrigin,
    ) -> Checked<Option<Expression>>
    where
        't: 'a,
    {
        let typed = self.typed;
        let unserved = |what| refuse_at(id, Unserved::Call(what));
        if !self.mathematical || !typed.symbols.symbols().nodes().is_valid(target) {
            return Ok(None);
        }
        if let Some(signature) = self.callable_parameter(target) {
            // Only the machine being read binds its machine parameters.
            let owner = self
                .symbols
                .get(id)
                .and_then(|owner| typed.machine_by_symbol(*owner))
                .ok_or_else(|| unserved("machine parameter call outside a machine"))?;
            if !typed
                .machine_type_parameters(owner)
                .iter()
                .any(|parameter| parameter.symbol == target)
            {
                return Err(unserved("call of a machine parameter of another machine"));
            }
            if !machine_arguments.is_empty() {
                return Err(unserved("machine parameter called with machines"));
            }
            let formals = typed
                .state_parameters
                .span(signature.parameters)
                .ok_or_else(|| unserved("machine parameter lost its parameters"))?;
            if formals.len() != arguments.len() {
                return Err(unserved(
                    "machine parameter called with the wrong number of arguments",
                ));
            }
            let mut read = Vec::with_capacity(arguments.len());
            for (formal, &argument) in formals.iter().zip(arguments) {
                let ty = self.value_type(id, formal.type_reference)?;
                let value = self.expression(id, slots, argument, Some(ty))?;
                if value.result != Some(ty) {
                    return Err(unserved("machine parameter argument of another type"));
                }
                read.push(value);
            }
            let result = self.value_type(id, signature.return_type)?;
            return Ok(Some(Expression {
                call_context: None,
                kind: ExpressionKind::Applied {
                    head: AppliedHead::Parameter(target),
                    machines: Vec::new(),
                    arguments: read,
                },
                result: Some(result),
                source,
            }));
        }
        let Some(entered) = self.entered_machine(target) else {
            return Ok(None);
        };
        let (template, selected): (SymbolHandle, Vec<&'a StaticMachineArgument>) = match self
            .generic_mathematical_origin(entered)
        {
            Some(_) if !machine_arguments.is_empty() => {
                return Err(unserved(
                    "call of a closed copy of a generic mathematical machine with machines of its own",
                ));
            }
            Some(origin) => origin,
            None if !machine_arguments.is_empty()
                && typed
                    .machine_by_symbol(entered)
                    .is_some_and(|machine| self.mathematical_template(machine)) =>
            {
                (entered, machine_arguments.iter().collect())
            }
            None => return Ok(None),
        };
        let declared = typed
            .machine_by_symbol(template)
            .ok_or_else(|| unserved("generic mathematical machine lost its declaration"))?;
        let required = self.required_callables(id, declared)?;
        if required.len() != selected.len() {
            return Err(unserved(
                "generic mathematical machine applied to the wrong number of machines",
            ));
        }
        let mut machines = Vec::with_capacity(selected.len());
        for (&argument, required) in selected.iter().zip(&required) {
            machines.push(self.passed_machine(id, argument, required)?.0);
        }
        let callee = self.machine(template)?;
        let call = self.call(id, slots, template, arguments, source, None)?;
        let ExpressionKind::Call {
            callee: crate::body::CallTarget::Machine(called),
            arguments,
            ..
        } = call.kind
        else {
            return Err(unserved(
                "generic mathematical machine call read as something other than a call",
            ));
        };
        if called != callee {
            return Err(unserved(
                "generic mathematical machine call names another machine",
            ));
        }
        Ok(Some(Expression {
            call_context: None,
            kind: ExpressionKind::Applied {
                head: AppliedHead::Machine(callee),
                machines,
                arguments,
            },
            result: call.result,
            source: call.source,
        }))
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
    /// Whether the body of `machine` dispatches on a value.
    fn dispatches(&self, id: MachineId, machine: SymbolHandle) -> Checked<bool> {
        let (_, state) = self.declaration(id, machine)?;
        Ok(self
            .typed
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .any(|statement| {
                matches!(statement, StatementNode::Transition(transition) if !transition.patterns.is_empty())
            }))
    }

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
        let statements = typed.statement_table.statements(state.statement_nodes);
        // A body that dispatches is lowered and read like any other, and so
        // is each machine declared inside it: those are its cases' helpers,
        // read in place where it calls them.
        // A machine that states nothing and returns nothing is a helper a
        // law reads in place where it calls it, so it is lowered too.
        let states_nothing = result.is_none()
            && !super::span(
                |span| typed.signature_contracts.span(span),
                declared.contracts,
                id,
                "contract span",
            )?
            .iter()
            .any(|clause| {
                clause.kind == crate::source_state::signature::SignatureContractKind::Ensures
            });
        if states_nothing
            || self.dispatches(id, machine)?
            || std::iter::successors(self.enclosing_machine(machine), |&outer| {
                self.enclosing_machine(outer)
            })
            .map(|outer| self.dispatches(id, outer))
            .collect::<Checked<Vec<_>>>()?
            .into_iter()
            .any(|dispatches| dispatches)
        {
            body.lowered = true;
            return Ok(body);
        }
        let read = (|| {
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
                        if !call.evidence_arguments.is_empty() {
                            return Err(unserved(
                                "generic mathematical machine cites a machine with evidence arguments",
                            ));
                        }
                        let arguments = typed.statement_table.expression_handles(call.arguments);
                        self.template_citation(
                            id,
                            scope,
                            call.target_symbol,
                            &call.machine_arguments,
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
        // A body the clause reader cannot read as terms is lowered and read
        // like any other body instead, which refuses what it cannot read.
        if read.is_err() || result.is_some() != body.value.is_some() {
            return Ok(TemplateBody {
                callables: body.callables,
                lowered: true,
                ..TemplateBody::default()
            });
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
        if call.receiver.is_valid() || !call.evidence_arguments.is_empty() {
            return Err(refuse_at(
                id,
                Unserved::Machine(
                    "generic mathematical machine cites a machine with a receiver or evidence arguments",
                ),
            ));
        }
        let arguments = typed.expression_table.expression_handles(call.arguments);
        self.template_citation(
            id,
            scope,
            call.target_symbol,
            &call.machine_arguments,
            arguments,
            body,
        )
    }

    fn template_citation(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        target: SymbolHandle,
        machine_arguments: &[StaticMachineArgument],
        arguments: &[ExpressionHandle],
        body: &mut TemplateBody,
    ) -> Checked<()> {
        let typed = self.typed;
        let unserved = |what| refuse_at(id, Unserved::Machine(what));
        // A machine with no parameters of its own, or a generic
        // mathematical machine given one machine for each of its machine
        // parameters: its law is then about those machines.
        // A body's call at concrete machines names the closed copy the
        // typed trees made for it; the citation is of the generic machine.
        let entered = self.entered_machine(target);
        let origin = entered.and_then(|instance| self.generic_mathematical_origin(instance));
        let machine_arguments: Vec<&StaticMachineArgument> = match &origin {
            Some((_, selected)) if machine_arguments.is_empty() => selected.clone(),
            Some(_) => {
                return Err(unserved(
                    "citation of a closed copy of a generic mathematical machine with machines of its own",
                ));
            }
            None => machine_arguments.iter().collect(),
        };
        let (cited, declared) = origin
            .map(|(template, _)| template)
            .or(entered)
            .and_then(|cited| Some((cited, typed.machine_by_symbol(cited)?)))
            .filter(|(_, machine)| {
                if machine_arguments.is_empty() {
                    !self.is_generic_template(machine)
                } else {
                    self.mathematical_template(machine)
                }
            })
            .ok_or_else(|| {
                unserved("generic mathematical machine cites something other than a mathematical machine with every machine parameter supplied")
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
        let mut terms = Vec::with_capacity(machine_arguments.len() + arguments.len());
        if !machine_arguments.is_empty() {
            let required = self.required_callables(id, declared)?;
            if required.len() != machine_arguments.len() {
                return Err(unserved(
                    "citation of a generic mathematical machine with the wrong number of machines",
                ));
            }
            for (&argument, required) in machine_arguments.iter().zip(&required) {
                terms.push(self.machine_argument(id, argument, required, &mut body.values)?);
            }
        }
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
