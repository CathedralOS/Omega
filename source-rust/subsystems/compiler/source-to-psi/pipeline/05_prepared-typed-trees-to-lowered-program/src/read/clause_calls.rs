use crate::read::refuse_at;
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
use semantic_vocabulary::{MachineId, ScalarTerm, ScalarType, ValueId};
use symbols::{SymbolHandle, SymbolKind};
use terminal_check_tables::Checked;

use super::Program;
use super::clause_terms::ClauseUse;
use super::contracts::{ClauseScope, ResultTerm};
use crate::Unserved;
use crate::body::{Qualification, ValueType};

/// A called machine prepared for unfolding: its parameter bindings, its body
/// expression and its declared result.
pub(super) struct Unfolded<R> {
    pub(super) machine: SymbolHandle,
    pub(super) bound: Vec<(SymbolHandle, ScalarTerm)>,
    pub(super) body: crate::source_state::expression::ExpressionHandle,
    pub(super) result: R,
}

/// A called machine prepared for clause unfolding, including optional record
/// formal slots and preconditions that must be established by the enclosing
/// machine's `requires`.
pub(super) struct UnfoldedCall {
    pub(super) machine: SymbolHandle,
    pub(super) bound: Vec<(SymbolHandle, ScalarTerm)>,
    pub(super) callee_slots: Vec<super::Slot>,
    pub(super) formal_fields: bool,
    pub(super) requires: Vec<crate::source_state::expression::ExpressionHandle>,
    pub(super) body: crate::source_state::expression::ExpressionHandle,
    pub(super) result: ScalarType,
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
            refuse_at(
                id,
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
                refuse_at(
                    id,
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
    ) -> Checked<UnfoldedCall> {
        let unserved = |what| Err(refuse_at(id, Unserved::Contract(what)));
        let machine = self.clause_call_machine(id, call)?;
        let typed = self.typed;
        let (declaration, state) = self.declaration(id, machine)?;
        self.served(id, machine)?;
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
        let mut requires = Vec::new();
        for contract in typed
            .machine_contracts(declaration)
            .iter()
            .chain(typed.state_contracts(state).iter())
        {
            if contract.kind != crate::source_state::signature::SignatureContractKind::Requires
                || contract.binding.is_some()
            {
                return unserved("called machine in a clause is not a closed, contract-free body");
            }
            for fact in typed.proof_facts.span_or_empty(contract.facts) {
                let expr = fact.as_expression().copied().ok_or_else(|| {
                    refuse_at(
                        id,
                        Unserved::Contract(
                            "called machine in a clause has a non-expression precondition",
                        ),
                    )
                })?;
                requires.push(expr);
            }
        }
        let Some(result) = self
            .result_type(id, state.return_type)?
            .filter(|result| result.qualification() == Qualification::NONE)
            .and_then(|result| result.scalar())
        else {
            return unserved("called machine in a clause has no plain scalar result");
        };
        let Some(body) = state.returned_expression(typed) else {
            return unserved("called machine in a clause is not one value expression");
        };
        let parameters = typed.state_parameters(state);
        let arguments = typed.expression_table.expression_handles(call.arguments);
        if arguments.len() != parameters.len() {
            return unserved("call in a clause with the wrong number of arguments");
        }
        let mut bound = Vec::new();
        let mut callee_slots: Vec<super::Slot> = scope
            .slots
            .iter()
            .map(|slot| super::Slot {
                symbol: SymbolHandle::invalid(),
                ..slot.clone()
            })
            .collect();
        let mut has_record_param = false;
        for (parameter, &argument) in parameters.iter().zip(arguments) {
            let (value_type, erased, borrowed) = self.formal(id, parameter)?;
            if erased || borrowed.is_some() || parameter.is_mutable {
                return unserved("called machine in a clause takes a non-value parameter");
            }
            match value_type {
                ValueType::Scalar(scalar, qualification) => {
                    if qualification != Qualification::NONE {
                        return unserved("called machine in a clause takes a qualified parameter");
                    }
                    let (actual, supplied) = self
                        .clause_term(id, scope, argument, ClauseUse::Scalar(Some(scalar)))?
                        .scalar(id)?;
                    if supplied.scalar() != Some(scalar) {
                        return unserved(
                            "argument in a clause call differs from its parameter type",
                        );
                    }
                    bound.push((parameter.symbol, actual));
                }
                ValueType::Record(_, qualification, regions)
                    if qualification == Qualification::NONE
                        && regions == crate::body::Regions::NONE =>
                {
                    let crate::source_state::expression::ExpressionNode::Name(path) =
                        typed.expression_table.expression(argument)
                    else {
                        return unserved(
                            "record argument in a clause call must name a formal slot",
                        );
                    };
                    let Some(slot_index) = scope
                        .slots
                        .iter()
                        .position(|slot| slot.symbol == path.symbol)
                    else {
                        return unserved(
                            "record argument in a clause call is not a formal slot in scope",
                        );
                    };
                    if scope.slots[slot_index].value_type != value_type {
                        return unserved(
                            "record argument in a clause call differs from its parameter type",
                        );
                    }
                    callee_slots[slot_index].symbol = parameter.symbol;
                    has_record_param = true;
                }
                _ => {
                    return unserved("called machine in a clause takes a non-scalar parameter");
                }
            }
        }
        Ok(UnfoldedCall {
            machine,
            bound,
            callee_slots: if has_record_param {
                callee_slots
            } else {
                Vec::new()
            },
            formal_fields: has_record_param && scope.formal_fields,
            requires,
            body,
            result,
        })
    }

    pub(super) fn clause_call_machine(
        &mut self,
        id: MachineId,
        call: &TableCallExpression,
    ) -> Checked<SymbolHandle> {
        let unserved = |what| Err(refuse_at(id, Unserved::Contract(what)));
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
            SymbolKind::Machine | SymbolKind::State => self.call_target_symbol(id, target)?,
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
        let unserved = |what| Err(refuse_at(id, Unserved::Contract(what)));
        let typed = self.typed;
        let (declaration, state) = self.declaration(id, machine)?;
        self.served(id, machine)?;
        // A clause runs the called machine from its entry, like any call
        // (`call_target_symbol`); the target must name the machine unfolded.
        if self.call_target_symbol(id, target)? != machine {
            return unserved("call in a clause names another machine than the one it unfolds");
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
        let Some(body) = state.returned_expression(typed) else {
            return unserved("called machine in a clause is not one value expression");
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
            let formal = ValueId::new(position as u64 + 1)
                .ok_or_else(|| refuse_at(id, Unserved::Contract("too many clause parameters")))?;
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
        let unserved = |what| refuse_at(id, Unserved::Contract(what));
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
