/*
A contract clause or domain predicate that calls a declared machine
(`domain bool::Held requires premise(self);`) states what that machine's body
computes for the arguments. The clause reader unfolds the call: it reads the
arguments as terms in the caller's scope, binds the callee's parameters to
those terms and reads the callee's body under that binding. The result is the
same proposition or term the author could have written inline.

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
arguments), take plain by-value scalar parameters, declare no contracts or
effects, and end in a single value expression. A callee with `requires`
would owe them at the call, which this reader does not yet discharge, so it
is refused, as is any recursion through unfolding. Anything else stays
refused at the clause that calls it.
*/

use semantic_vocabulary::{MachineId, Proposition, ScalarTerm, ScalarType};
use symbols::{SymbolHandle, SymbolKind};
use terminal_check_tables::{Checked, Construct};
use typed_trees::expression::TableCallExpression;
use typed_trees::statement::{
    StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode,
};

use super::Program;
use super::contracts::{ClauseScope, ResultTerm};
use crate::body::{Qualification, ValueType};
use crate::{Unserved, refuse};

/// A called machine prepared for unfolding: its parameter bindings, its body
/// expression and its declared result.
struct Unfolded {
    machine: SymbolHandle,
    bound: Vec<(SymbolHandle, ScalarTerm)>,
    body: typed_trees::expression::ExpressionHandle,
    result: ScalarType,
}

impl Program<'_> {
    /// The clause a call of a declared Boolean machine states.
    pub(super) fn called_clause(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        call: &TableCallExpression,
    ) -> Checked<Proposition> {
        let unfolded = self.unfold(id, scope, call)?;
        if unfolded.result != ScalarType::Boolean {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Contract("called predicate whose result is not Boolean"),
            ));
        }
        let body = unfolded.body;
        self.within_unfolding(unfolded.machine, |program| {
            let callee = callee_scope(&unfolded.bound, unfolded.machine);
            program.clause(id, &callee, body)
        })
    }

    /// The term a call of a declared scalar machine denotes.
    pub(super) fn called_term(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        call: &TableCallExpression,
    ) -> Checked<(ScalarTerm, ValueType)> {
        let unfolded = self.unfold(id, scope, call)?;
        let body = unfolded.body;
        let result = unfolded.result;
        let (term, value_type) = self.within_unfolding(unfolded.machine, |program| {
            let callee = callee_scope(&unfolded.bound, unfolded.machine);
            program.term(id, &callee, body, Some(result))
        })?;
        if value_type.scalar() != Some(result) {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Contract("called machine's body differs from its result type"),
            ));
        }
        Ok((term, ValueType::plain(result)))
    }

    /// Read `body` with `machine` marked as being unfolded, refusing a
    /// machine that is already being unfolded: a recursive predicate has no
    /// finite unfolding.
    fn within_unfolding<T>(
        &mut self,
        machine: SymbolHandle,
        body: impl FnOnce(&mut Self) -> Checked<T>,
    ) -> Checked<T> {
        self.unfolding_calls.push(machine);
        let read = body(self);
        self.unfolding_calls.pop();
        if read.is_ok() {
            self.admitted_declarations.include(machine);
            self.admitted_bodies.include(machine);
        }
        read
    }

    fn unfold(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        call: &TableCallExpression,
    ) -> Checked<Unfolded> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Contract(what)));
        if call.receiver.is_valid()
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
        let typed = self.typed;
        let (declaration, state) = self.declaration(id, machine)?;
        if target != machine && target != state.symbol {
            return unserved("call in a clause of a state other than the entry");
        }
        if self.unfolding_calls.contains(&machine) {
            return unserved("recursive call in a clause");
        }
        if !declaration.body_is_present
            || declaration.suspends
            || declaration.blocks
            || !declaration.type_parameters.is_empty()
            || !declaration.contracts.is_empty()
            || !state.contracts.is_empty()
        {
            return unserved("called machine in a clause is not a closed, contract-free body");
        }
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
        let Some(result) = self
            .result_type(id, state.return_type)?
            .filter(|result| result.qualification() == Qualification::NONE)
            .and_then(|result| result.scalar())
        else {
            return unserved("called machine in a clause has no plain scalar result");
        };
        let parameters = typed.state_parameters(state);
        let arguments = typed.expression_table.expression_handles(call.arguments);
        if parameters.len() != arguments.len() {
            return unserved("call in a clause with the wrong number of arguments");
        }
        let mut bound = Vec::with_capacity(parameters.len());
        for (parameter, &argument) in parameters.iter().zip(arguments) {
            let (value_type, erased, borrowed) = self.formal(id, parameter, None)?;
            if erased || borrowed.is_some() || parameter.is_mutable {
                return unserved("called machine in a clause takes a non-value parameter");
            }
            let ValueType::Scalar(scalar_type, qualification) = value_type else {
                return unserved("called machine in a clause takes a non-scalar parameter");
            };
            if qualification != Qualification::NONE {
                return unserved("called machine in a clause takes a qualified parameter");
            }
            let (term, supplied) = self.term(id, scope, argument, Some(scalar_type))?;
            if supplied.scalar() != Some(scalar_type) {
                return unserved("argument in a clause call differs from its parameter type");
            }
            bound.push((parameter.symbol, term));
        }
        Ok(Unfolded {
            machine,
            bound,
            body,
            result,
        })
    }
}

/// The scope the callee's body is read in: its parameters, each bound to the
/// caller's argument term, and nothing of the caller's own names.
fn callee_scope<'b>(
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
