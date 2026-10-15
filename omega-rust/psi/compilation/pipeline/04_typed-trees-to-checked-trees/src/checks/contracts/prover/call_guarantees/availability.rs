//! Contract and borrow consumers share call establishment, source identity,
//! and capture preservation. Availability comes from the exact use's live
//! flow contexts, never a certificate's assertion of dominance.

use validation::CallSite;
use validation::CanonicalPlace;

use super::{
    Invocation, actual_projection, builtin_predicate, capture_preserved, captured_place,
    invocation, stable_predicate_inputs,
};

use checked_trees::FlowStateFact;
use facts::{ContractFactKind, FactOrigin, FactPayload, FactPlan, PlaceRoot, ProgramPoint};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::state::State;

pub(in crate::checks) struct AvailableGuarantee<'program> {
    pub(in crate::checks) expression: ExpressionHandle,
    pub(in crate::checks) fact: arena::Handle<typed_trees::domain::ProofFact>,
    pub(super) invocation: Invocation<'program>,
    /// These subjects describe normal-return storage, not invocation inputs.
    pub(super) post_return_writes: Option<Vec<CanonicalPlace>>,
}

impl AvailableGuarantee<'_> {
    pub(in crate::checks) fn coordinates(&self) -> (usize, usize) {
        (self.invocation.statement, self.invocation.ordinal)
    }

    pub(in crate::checks) fn has_builtin_meaning(
        &self,
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> bool {
        self.invocation
            .callable
            .decomposed_builtin_meaning(program, expression)
    }

    /// The call expression's own handle when this guarantee's call site is a
    /// call nested inside a statement expression. Statement-site calls have
    /// no such operand — their results always bind a local name.
    pub(in crate::checks) fn call_expression(&self) -> Option<ExpressionHandle> {
        match self.invocation.site {
            CallSite::Expression { expression, .. } => Some(expression),
            _ => None,
        }
    }

    /// The member-projection segments behind this operand when it resolves
    /// to this guarantee's own reserved result (`result` or `result.first`).
    /// A projection rooted at a different callable's result does not qualify.
    pub(in crate::checks) fn result_segments(
        &self,
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<Vec<facts::PlaceSegment>> {
        let result = validation::reserved_result_place(program, expression)?;
        (result.machine_symbol == self.invocation.callable.owner_symbol())
            .then_some(result.segments)
    }

    /// The argument expression behind one contract operand, with the
    /// projection segments the operand carries beyond the parameter's own
    /// place (`value.first` → the actual plus `[Field{first}]`). Literal
    /// operands carry no projection.
    pub(in crate::checks) fn actual_projection(
        &self,
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<(ExpressionHandle, Vec<facts::PlaceSegment>)> {
        if matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Integer(_)
        ) {
            return Some((expression, Vec::new()));
        }
        actual_projection(program, &self.invocation, expression)
    }

    /// The initial assignment names an occurrence; it is never evaluated.
    /// Live provenance must independently join the binding to this exact
    /// call. A mutable local supplies the guarantee only while that
    /// provenance still pins it to the same occurrence the call produced;
    /// reassignment retires the assigned-value row, so the guarantee stops
    /// binding. Immutable copies normalize through the existing bound reader.
    /// Returns the binding symbol and whether that storage is mutable.
    pub(in crate::checks) fn result_binding(
        &self,
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        contexts: &[facts::FactContextHandle],
        state: &State,
    ) -> Option<(SymbolHandle, bool)> {
        if state.symbol != self.invocation.caller_state {
            return None;
        }
        let local = program
            .statement_table
            .statement_at(state, self.invocation.statement)?
            .as_local_data()?;
        let CallSite::Expression { expression, .. } = self.invocation.site else {
            return None;
        };
        if !local.symbol.is_valid() || local.initial_value != expression {
            return None;
        }
        let place = captured_place(
            program,
            semantic,
            contexts,
            CanonicalPlace {
                root: PlaceRoot::Symbol(local.symbol),
                segments: Vec::new(),
            },
        )?;
        (place.root == PlaceRoot::Expression(expression) && place.segments.is_empty())
            .then_some((local.symbol, local.is_mutable))
    }
}

/// Statement entry excludes every call in that statement. An evaluated use
/// may consume same-statement guarantees, but only those present in its exact
/// retained flow context (for example the pre-store frontier after operands).
#[derive(Clone, Copy)]
pub(in crate::checks) enum GuaranteeUse {
    StatementEntry(usize),
    EvaluatedContext(usize),
}

pub(in crate::checks) fn available<'program>(
    program: &'program dyn ::validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    caller: &FlowStateFact,
    use_point: GuaranteeUse,
    contexts: &[facts::FactContextHandle],
    frames: &validation::CallFrameResolver<'_>,
) -> Vec<AvailableGuarantee<'program>> {
    let Some(caller_machine) = program.machine_by_symbol(caller.machine_symbol) else {
        return Vec::new();
    };
    let mut guarantees = Vec::new();
    for fact in contexts.iter().flat_map(|context| {
        facts
            .semantic
            .context_view(facts.semantic.contexts.get(*context))
            .facts()
    }) {
        let FactPayload::ContractBooleanExpression {
            kind: ContractFactKind::Ensures,
            fact: source,
            expression,
            ..
        } = fact.payload
        else {
            continue;
        };
        let ProgramPoint::CallEnsures {
            machine_symbol,
            state_symbol,
            statement_index,
            call_ordinal,
        } = fact.point
        else {
            continue;
        };
        if fact.origin != FactOrigin::CallEnsures
            || machine_symbol != caller.machine_symbol
            || state_symbol != caller.state_symbol
            || match use_point {
                GuaranteeUse::StatementEntry(statement) => statement_index >= statement,
                GuaranteeUse::EvaluatedContext(statement) => statement_index > statement,
            }
        {
            continue;
        }
        let Some(supplied) = invocation(program, caller, statement_index, call_ordinal) else {
            continue;
        };
        // The write frame the supply site contributes: a call inside an
        // expression frames from its expression, a statement-position call
        // frames from its own call node, and a transition is not a call.
        let frame = match supplied.site {
            CallSite::Expression { expression, .. } => {
                frames.expression_write_frame(caller_machine, expression)
            }
            CallSite::Statement(call) => frames.may_write_frame(caller_machine, call),
            CallSite::TransitionNamed { .. } => continue,
        };
        if !owns_guarantee(program, &supplied, source, expression)
            || !stable_predicate_inputs(program, &supplied, expression)
            || !capture_preserved(
                program,
                &facts.borrow,
                caller_machine,
                &supplied,
                frame,
                expression,
                frames,
            )
            || !builtin_predicate(program, &facts.operators, &supplied, expression)
        {
            continue;
        }
        // Other consumers retain their existing availability law. Only an
        // arithmetic import needs the produced return-version footprint.
        let post_return_writes = super::invocation_fact(facts, caller, &supplied)
            .filter(|call| call.normal_return_writes_complete)
            .map(|call| {
                facts
                    .flow
                    .control
                    .operand_origin_writes
                    .span_or_empty(call.normal_return_writes)
                    .iter()
                    .map(|write| CanonicalPlace {
                        root: write.root,
                        segments: facts
                            .flow
                            .control
                            .operand_referent_segments
                            .span_or_empty(write.segments)
                            .to_vec(),
                    })
                    .collect()
            });
        if guarantees.iter().any(|prior: &AvailableGuarantee<'_>| {
            prior.fact == source && prior.coordinates() == (statement_index, call_ordinal)
        }) {
            continue;
        }
        guarantees.push(AvailableGuarantee {
            expression,
            fact: source,
            invocation: supplied,
            post_return_writes,
        });
    }
    guarantees
}

fn owns_guarantee(
    program: &dyn ::validation::CheckingInput,
    supplied: &Invocation<'_>,
    source: arena::Handle<typed_trees::domain::ProofFact>,
    expression: ExpressionHandle,
) -> bool {
    source.is_valid()
        && matches!(program.proof_facts.get(source), typed_trees::domain::ProofFact::Expression(actual) if *actual == expression)
        && supplied
            .callable
            .contracts(program)
            .filter(|contract| {
                contract.kind == typed_trees::signature::SignatureContractKind::Ensures
            })
            .any(|contract| contract.facts.handles().any(|fact| fact == source))
}
