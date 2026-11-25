/*
A clause that calls a pure, total declaration can name the result even when
the declaration has no body. The term records the selected operation, its
supplying record, and each typed argument. A cited law can then prove an
identity about those results for an arbitrary carrier.

For a closed scalar helper such as `domain bool::Held requires premise(self);`,
the reader can also compute the term by reading the helper's body with its
formal parameters bound to the caller's arguments.

`wiki/spec/language/domains.md`: "Eligible total, pure machine calls may
occur as denotational value terms inside propositions", and "Predicate calls
retain their ordinary formation and evaluation obligations."
`wiki/spec/terminal-psi/fact_validity.md`: "Pure call congruence requires the
same admitted operation and unchanged argument values". Unfolding meets that
requirement by construction: two calls of one machine on the same argument
terms unfold to the same proposition, and no fact about the call outlives its
argument terms.

Only a machine whose meaning is exactly one body expression the clause reader
itself accepts is unfolded, so the call adds no operation a clause could not
already state. The callee must be closed (no type parameters or static
arguments), take plain by-value scalar parameters and end in a single value
expression. Scalar helpers declare no contracts. Proof-only IEEE views may
declare result guarantees that their checked body establishes for every
input (`float_meaning`). A callee with `requires`
would owe them at the call, which this reader does not yet discharge, so it
is refused, as is any recursion through unfolding.
*/

use crate::source_state::expression::TableCallExpression;
use crate::source_state::statement::{
    StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode,
};
use semantic_vocabulary::{MachineId, ScalarTerm, ScalarType, ValueId};
use symbols::{SymbolHandle, SymbolKind};
use terminal_check_tables::{Checked, Construct};

use super::Program;
use super::clause_terms::ClauseUse;
use super::contracts::{ClauseScope, ResultTerm};
use crate::body::{Qualification, ValueType};
use crate::{Unserved, refuse};

/// A called machine prepared for unfolding: its parameter bindings, its body
/// expression and its declared result.
pub(super) struct Unfolded<R> {
    pub(super) machine: SymbolHandle,
    pub(super) bound: Vec<(SymbolHandle, ScalarTerm)>,
    pub(super) body: crate::source_state::expression::ExpressionHandle,
    pub(super) result: R,
}

/// A scalar term forms all its operands. Track substitutions during its
/// expansion so only discarded arguments need a separate formation clause.
pub(super) struct Unfolding {
    machine: SymbolHandle,
    used: Vec<bool>,
}

impl Program<'_> {
    /// Read `body` with `machine` marked as being unfolded, refusing a
    /// machine that is already being unfolded: a recursive predicate has no
    /// finite unfolding.
    pub(super) fn within_unfolding<T>(
        &mut self,
        id: MachineId,
        machine: SymbolHandle,
        arguments: usize,
        body: impl FnOnce(&mut Self) -> Checked<T>,
    ) -> Checked<(T, Vec<bool>)> {
        self.unfolding_calls.push(Unfolding {
            machine,
            used: vec![false; arguments],
        });
        let read = body(self);
        let unfolding = self.unfolding_calls.pop().ok_or_else(|| {
            refuse(
                id,
                Construct::Machine,
                Unserved::Contract("called machine lost its unfolding scope"),
            )
        })?;
        if read.is_ok() {
            self.admitted_declarations.include(machine);
            self.admitted_bodies.include(machine);
        }
        read.map(|result| (result, unfolding.used))
    }

    pub(super) fn use_unfolded_argument(
        &mut self,
        id: MachineId,
        owner: SymbolHandle,
        position: usize,
    ) -> Checked<()> {
        if let Some(unfolding) = self
            .unfolding_calls
            .last_mut()
            .filter(|frame| frame.machine == owner)
        {
            let used = unfolding.used.get_mut(position).ok_or_else(|| {
                refuse(
                    id,
                    Construct::Machine,
                    Unserved::Contract("called machine argument is outside its unfolding scope"),
                )
            })?;
            *used = true;
        }
        Ok(())
    }

    pub(super) fn unfold(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        call: &TableCallExpression,
    ) -> Checked<Unfolded<ScalarType>> {
        let machine = self.clause_call_machine(id, call)?;
        let mut unfolded = self.unfold_definition(
            id,
            call.target_symbol,
            machine,
            |program, declaration, state| {
                let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Contract(what)));
                if !declaration.contracts.is_empty() || !state.contracts.is_empty() {
                    return unserved(
                        "called machine in a clause is not a closed, contract-free body",
                    );
                }
                let Some(result) = program
                    .result_type(id, state.return_type)?
                    .filter(|result| result.qualification() == Qualification::NONE)
                    .and_then(|result| result.scalar())
                else {
                    return unserved("called machine in a clause has no plain scalar result");
                };
                Ok(result)
            },
        )?;
        self.bind_call_arguments(id, scope, call, &mut unfolded.bound)?;
        Ok(unfolded)
    }

    pub(super) fn clause_call_machine(
        &mut self,
        id: MachineId,
        call: &TableCallExpression,
    ) -> Checked<SymbolHandle> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Contract(what)));
        if call.receiver.is_valid()
            || call.runtime_call.is_some()
            || !call.evidence_arguments.is_empty()
            || call.requirement_application.is_valid()
            || call.exact_coercion_target.is_valid()
            || call.build_selection.is_valid()
            || call.static_machine_parameter.is_valid()
            || call.static_requirement_dispatch.is_some()
            || call.quotient_operation.is_some()
            || call.private_layout_operation.is_some()
            || call.installed_interrupt_operation.is_some()
            || call.layout_query.is_some()
            || call.native_code_layout.is_some()
            || call.operational_acknowledgement.acknowledges_suspend
            || call.operational_acknowledgement.acknowledges_block
        {
            return unserved("call in a clause with a receiver, evidence or another selection");
        }
        let target = call.target_symbol;
        if !target.is_valid() || !self.typed.symbols.symbols().nodes().is_valid(target) {
            return unserved("call in a clause without a resolved target");
        }
        let machine = match self.typed.symbols.get(target).kind {
            SymbolKind::Machine => target,
            SymbolKind::State => self.typed.symbols.get(target).parent,
            _ => return unserved("call in a clause of something that is not a machine"),
        };
        self.direct_selection(
            id,
            target,
            call.authored_call_selection,
            call.authored_call_binding,
        )?;
        self.applies_nothing(
            id,
            target,
            call.authored_machine_application,
            &call.machine_arguments,
        )?;
        Ok(machine)
    }

    pub(super) fn unfold_definition<R>(
        &mut self,
        id: MachineId,
        target: SymbolHandle,
        machine: SymbolHandle,
        result: impl FnOnce(
            &mut Self,
            &crate::source_state::machine::Machine,
            &crate::source_state::state::State,
        ) -> Checked<R>,
    ) -> Checked<Unfolded<R>> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Contract(what)));
        let typed = self.typed;
        let (declaration, state) = self.declaration(id, machine)?;
        self.served(id, machine)?;
        if target != machine && target != state.symbol {
            return unserved("call in a clause of a state other than the entry");
        }
        if self
            .unfolding_calls
            .iter()
            .any(|frame| frame.machine == machine)
        {
            return unserved("recursive call in a clause");
        }
        if !declaration.body_is_present
            || declaration.suspends
            || declaration.blocks
            || !declaration.type_parameters.is_empty()
        {
            return unserved("called machine in a clause is not a closed, contract-free body");
        }
        let result = result(self, declaration, state)?;
        // The body's one value: a trailing expression, or the unconditional
        // value transition stage 02 writes for a body that is only a value.
        let body = match typed.statement_table.statements(state.statement_nodes) {
            [StatementNode::Expression(body)] => *body,
            [StatementNode::Transition(transition)]
                if transition.patterns.is_empty()
                    && transition.guard == TransitionGuardNode::Always
                    && transition.exit == TransitionExit::Ordinary
                    && !transition.continuation.is_valid()
                    && transition.proof_selectors.is_empty()
                    && transition.target.is_valid() =>
            {
                match typed.statement_table.transition_target(transition.target) {
                    TransitionTargetNode::Value(body) => *body,
                    _ => return unserved("called machine in a clause is not one value expression"),
                }
            }
            _ => return unserved("called machine in a clause is not one value expression"),
        };
        let parameters = typed.state_parameters(state);
        let mut bound = Vec::with_capacity(parameters.len());
        for (position, parameter) in parameters.iter().enumerate() {
            let (value_type, erased, borrowed) = self.formal(id, parameter)?;
            if erased || borrowed.is_some() || parameter.is_mutable {
                return unserved("called machine in a clause takes a non-value parameter");
            }
            let ValueType::Scalar(scalar_type, qualification) = value_type else {
                return unserved("called machine in a clause takes a non-scalar parameter");
            };
            if qualification != Qualification::NONE {
                return unserved("called machine in a clause takes a qualified parameter");
            }
            let formal = ValueId::new(position as u64 + 1).ok_or_else(|| {
                refuse(
                    id,
                    Construct::Machine,
                    Unserved::Contract("too many clause parameters"),
                )
            })?;
            bound.push((parameter.symbol, ScalarTerm::value(formal, scalar_type)));
        }
        Ok(Unfolded {
            machine,
            bound,
            body,
            result,
        })
    }

    /// Substitute actuals simultaneously. Each argument is read in the
    /// caller's scope, independently of the other formals being replaced.
    pub(super) fn bind_call_arguments(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        call: &TableCallExpression,
        bound: &mut [(SymbolHandle, ScalarTerm)],
    ) -> Checked<()> {
        let arguments = self
            .typed
            .expression_table
            .expression_handles(call.arguments);
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Contract(what));
        if arguments.len() != bound.len() {
            return Err(unserved(
                "call in a clause with the wrong number of arguments",
            ));
        }
        for ((_, formal), &argument) in bound.iter_mut().zip(arguments) {
            let scalar = formal.scalar_type();
            let (actual, supplied) = self
                .clause_term(id, scope, argument, ClauseUse::Scalar(Some(scalar)))?
                .scalar(id)?;
            if supplied.scalar() != Some(scalar) {
                return Err(unserved(
                    "argument in a clause call differs from its parameter type",
                ));
            }
            *formal = actual;
        }
        Ok(())
    }
}

/// The scope the callee's body is read in: its parameters, each bound to the
/// caller's argument term, and nothing of the caller's own names.
pub(super) fn callee_scope<'b>(
    bound: &'b [(SymbolHandle, ScalarTerm)],
    owner: SymbolHandle,
) -> ClauseScope<'b> {
    ClauseScope {
        slots: &[],
        values: &[],
        bound,
        owner,
        result: ResultTerm::Absent("result in a called machine's body"),
        subject: None,
        formal_fields: false,
        logical: None,
    }
}
