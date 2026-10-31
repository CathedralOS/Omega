//! Ordered call ranks consumes the ordinary completed-prefix ledger. Save
//! the source rank at entry, then carry that same judge through the prefix and
//! ordered argument evaluation. Reference bindings remain live descriptors;
//! scalar arguments and the entry rank retain their original value snapshots.

use super::{ExpressionHandle, Machine, RankingRangeCallProgress, State};
use crate::proof_contracts::contract_entailment::{
    citations::machine_requires_facts,
    structural_case_arms::recognize_structural_state_prefix,
    structural_judgment::{CapturedIntegerRank, StructuralJudge},
};
use symbols::SymbolHandle;
use typed_trees::{signature::SignatureContractKind, types::PrimitiveType};

mod observation;
use observation::CallProgress;

#[derive(Clone)]
pub(crate) struct CapturedRankComponent {
    pub subject: ExpressionHandle,
    /// Exact declaration-selected suffix on the authored subject. An empty
    /// suffix retains the ordinary scalar-read admission path.
    pub projection: Vec<facts::PlaceSegment>,
    pub primitive: PrimitiveType,
    pub measure: SymbolHandle,
}

pub(crate) struct CapturedRankCallProgress {
    pub progress: RankingRangeCallProgress,
    pub source_applications: Vec<Option<crate::IntegerApplicationFormation>>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn prove_captured_rank_call(
    program: &dyn crate::CheckingInput,
    caller: &Machine,
    state: &State,
    source: Vec<CapturedRankComponent>,
    callee: &Machine,
    destination: Vec<CapturedRankComponent>,
    statement_index: usize,
    target: typed_trees::statement::TransitionTargetHandle,
    arguments: &[ExpressionHandle],
) -> Result<CapturedRankCallProgress, &'static str> {
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
    if source.is_empty() || source.len() != destination.len() {
        return Err("the rank call has incompatible ordered component arity");
    }
    let mut formed = Vec::with_capacity(source.len());
    for component in &source {
        let input = judge.admitted_entry_scalar_projection(
            caller,
            state,
            component.subject,
            &component.projection,
        )?;
        formed.push(judge.form_captured_integer_rank(&CapturedIntegerRank {
            input,
            primitive: component.primitive,
            measure: component.measure,
        })?);
    }
    let mut observation = CallProgress {
        state: state.symbol,
        statement_index,
        target,
        callee: callee.symbol,
        callee_body: callee.states,
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
    Ok(CapturedRankCallProgress {
        progress: if observation.strict {
            RankingRangeCallProgress::Strict
        } else {
            RankingRangeCallProgress::NonIncreasing
        },
        source_applications: observation
            .source
            .into_iter()
            .map(|rank| rank.application)
            .collect(),
    })
}
