//! Scalar call progress consumes the ordinary completed-prefix ledger. Save
//! the source rank at entry, then carry that same judge through the prefix and
//! ordered argument evaluation. Reference bindings remain live descriptors;
//! scalar arguments and the entry rank retain their original value snapshots.

use super::{ExpressionHandle, Machine, RankingRangeCallProgress, State};
use crate::proof_contracts::contract_entailment::{
    citations::machine_requires_facts,
    structural_case_arms::recognize_structural_state_prefix,
    structural_judgment::normal_returns::OperationOperandDemand,
    structural_judgment::{CapturedIntegerRank, FormedIntegerRank, StructuralJudge},
    structural_prefix::{PrefixObservation, StructuralPrefixSite, StructuralPrefixVisitor},
};
use symbols::SymbolHandle;
use typed_trees::{signature::SignatureContractKind, types::PrimitiveType};

#[derive(Clone, Copy)]
pub(crate) struct CapturedScalarRank {
    pub subject: ExpressionHandle,
    pub primitive: PrimitiveType,
    pub measure: SymbolHandle,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn prove_captured_scalar_rank_call(
    program: &dyn crate::CheckingInput,
    caller: &Machine,
    state: &State,
    source: CapturedScalarRank,
    callee: &Machine,
    destination: CapturedScalarRank,
    statement_index: usize,
    target: typed_trees::statement::TransitionTargetHandle,
    arguments: &[ExpressionHandle],
) -> Result<RankingRangeCallProgress, &'static str> {
    let transition = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)
        .and_then(|statement| statement.as_transition())
        .ok_or("the scalar rank call is not an exact transition occurrence")?;
    if transition.target != target || transition.continuation.is_valid() {
        return Err("the scalar rank call needs its own primary completion prefix");
    }
    let root = program
        .machine_states(caller)
        .first()
        .ok_or("the scalar rank has no invocation entry")?;
    if root.symbol != state.symbol {
        return Err("the scalar rank needs its state-local entry observation");
    }
    let site = crate::collect_call_sites_in_statement(program, caller, state, statement_index)
        .into_iter()
        .find(|site| site.completes_transition(program, target))
        .ok_or("the scalar rank call has no exact completion occurrence")?;
    let authored = crate::call_site_argument_expressions(program, &site);
    let leading_operands = authored
        .len()
        .checked_sub(arguments.len())
        .filter(|leading| authored.get(*leading..) == Some(arguments))
        .ok_or("the scalar rank call lost its exact operand telescope")?;
    let mut requires = machine_requires_facts(program, caller);
    requires.extend(
        crate::proof_contracts::proof_facts::contract_expression_facts(
            program,
            program.state_contracts(state).iter(),
            SignatureContractKind::Requires,
        ),
    );
    let mut judge = StructuralJudge::from_runtime_state_requires(program, caller, state, &requires);
    let input = judge
        .admitted_entry_scalar_value(caller, state, source.subject)
        .ok_or("the source rank input has no authenticated entry read")?;
    let formed = judge.form_captured_integer_rank(&CapturedIntegerRank {
        input,
        primitive: source.primitive,
        measure: source.measure,
    })?;
    let mut observation = CallProgress {
        state: state.symbol,
        statement_index,
        target,
        callee: callee.symbol,
        source: formed,
        destination,
        leading_operands,
        seen: false,
        strict: true,
        failure: None,
    };
    let mut diagnostics = Vec::new();
    if recognize_structural_state_prefix(
        program,
        caller,
        state,
        &judge,
        &crate::proof_only_classification(program),
        &mut diagnostics,
        &mut observation,
    )
    .is_none()
        || !diagnostics.is_empty()
    {
        return Err("the scalar rank call has no complete executed-prefix observation");
    }
    if let Some(reason) = observation.failure {
        return Err(reason);
    }
    if !observation.seen {
        return Err("the scalar rank call has no reached operand observation");
    }
    Ok(if observation.strict {
        RankingRangeCallProgress::Strict
    } else {
        RankingRangeCallProgress::NonIncreasing
    })
}

struct CallProgress {
    state: SymbolHandle,
    statement_index: usize,
    target: typed_trees::statement::TransitionTargetHandle,
    callee: SymbolHandle,
    source: FormedIntegerRank,
    destination: CapturedScalarRank,
    leading_operands: usize,
    seen: bool,
    strict: bool,
    failure: Option<&'static str>,
}

impl StructuralPrefixVisitor for CallProgress {
    fn site(&self) -> StructuralPrefixSite {
        StructuralPrefixSite {
            state: self.state,
            statement_index: self.statement_index,
            in_transition_target: true,
        }
    }
    fn entered(&mut self) {
        self.seen = true;
    }
    fn unresolved(&mut self, _: &Machine) {
        self.failure = Some("the scalar rank call has an unresolved prefix state");
    }
    fn is_complete(&self) -> bool {
        self.failure.is_some()
    }
    fn observe<'program>(&mut self, observation: PrefixObservation<'_, 'program>) {
        if self.failure.is_some() {
            return;
        }
        let result = self.progress(observation);
        match result {
            Ok(strict) => self.strict &= strict,
            Err(reason) => self.failure = Some(reason),
        }
    }
}

impl CallProgress {
    fn progress(&self, observation: PrefixObservation<'_, '_>) -> Result<bool, &'static str> {
        let (program, machine, state, judge, environment) = observation;
        let callee = program
            .machine(self.callee)
            .ok_or("the captured rank call lost its destination")?;
        let entry = program
            .machine_states(callee)
            .first()
            .ok_or("the captured rank destination has no entry")?;
        let parameters = program
            .state_parameters(entry)
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        let site =
            crate::collect_call_sites_in_statement(program, machine, state, self.statement_index)
                .into_iter()
                .find(|site| site.completes_transition(program, self.target))
                .ok_or("the rank observation lost its exact call occurrence")?;
        let entries = match site {
            crate::CallSite::Expression { expression, .. } => judge.normal_operation_entries(
                self.statement_index,
                expression,
                OperationOperandDemand::All,
                environment,
                (machine, state, state.return_type),
            ),
            crate::CallSite::TransitionNamed { .. } => {
                let arguments = crate::call_site_argument_expressions(program, &site);
                let formal = site
                    .parameters(program, machine.symbol)
                    .ok_or("the rank call has no selected argument signature")?;
                judge.normal_argument_entries(
                    arguments,
                    formal,
                    environment,
                    (machine, state, state.return_type),
                    Some(site),
                )
            }
            crate::CallSite::Statement(_) => {
                return Err("a scalar rank edge is not a completing invocation");
            }
        }
        .ok_or("the scalar rank call operands have no complete ordered observations")?;
        if entries.is_empty() {
            return Err("the scalar rank call has no completed operand path");
        }
        let mut strict = true;
        for entry_observation in entries {
            let actuals = entry_observation
                .observed_arguments()
                .ok_or("a scalar rank call operand has no value observation")?;
            let actuals = actuals
                .get(self.leading_operands..)
                .filter(|values| values.len() == parameters.len())
                .ok_or("the scalar rank call has an incomplete captured telescope")?;
            let bindings = parameters
                .iter()
                .zip(actuals)
                .map(|(formal, value)| (formal.symbol, value.clone()))
                .collect::<Vec<_>>();
            let input = entry_observation.judge.bound_contract_term(
                self.destination.subject, entry.symbol, &parameters, callee.symbol,
                &bindings, &[], None, 0,
            ).ok_or("the arriving rank input has no authenticated projection from its captured actuals")?;
            let destination = CapturedIntegerRank {
                input,
                primitive: self.destination.primitive,
                measure: self.destination.measure,
            };
            strict &= entry_observation
                .judge
                .compare_captured_integer_ranks(&self.source, &destination)?;
        }
        Ok(strict)
    }
}
