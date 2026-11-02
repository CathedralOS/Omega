//! Ordered call ranks consume the ordinary completed-prefix ledger. Save
//! the source rank at entry, then carry that same judge through the prefix and
//! ordered argument evaluation. Reference bindings remain live descriptors;
//! scalar arguments and the entry rank retain their original value snapshots.

use super::{ExpressionHandle, Machine, RankingRangeCallProgress, State};
use crate::proof_contracts::contract_entailment::{
    citations::machine_requires_facts,
    structural_case_arms::recognize_structural_state_prefix,
    structural_judgment::{CapturedIntegerInput, CapturedIntegerRank, StructuralJudge},
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
    pub primitive: Option<PrimitiveType>,
    /// Prove the selected relation's lower endpoint on this produced value.
    /// A desired floor cannot establish formation or become an entry premise.
    pub minimum: numerics::bignum::BigInt,
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
        let input = if let Some(primitive) = component.primitive {
            CapturedIntegerInput::Scalar {
                value: judge.admitted_entry_scalar_projection(
                    caller,
                    state,
                    component.subject,
                    &component.projection,
                )?,
                primitive,
            }
        } else {
            let declaration = program
                .measures()
                .iter()
                .find(|declaration| declaration.symbol == component.measure)
                .ok_or("the record rank lost its supplying declaration")?;
            let shape = super::declared_input_shape(program, declaration)
                .ok_or("the record rank has no complete typed read shape")?;
            let actual =
                crate::declared_place_type_raw(program, caller, Some(state), component.subject)
                    .and_then(|reference| {
                        crate::project_type_reference_from_segments(
                            program,
                            reference,
                            &component.projection,
                        )
                    })
                    .ok_or("the record rank lost its exact input type")?;
            if !super::declared_input_type_matches(program, actual, shape.type_reference) {
                return Err("the record rank input does not match its declared nominal type");
            }
            let mut reads = Vec::new();
            for read in &shape.reads {
                let path = component
                    .projection
                    .iter()
                    .chain(&read.coordinate.path)
                    .cloned()
                    .collect::<Vec<_>>();
                let value = judge.admitted_entry_scalar_projection(
                    caller,
                    state,
                    component.subject,
                    &path,
                )?;
                reads.push((read.coordinate.clone(), value, read.primitive_type));
            }
            let snapshot = crate::canonical_place_from_expression(program, component.subject)
                .and_then(|mut place| {
                    place.segments.extend_from_slice(&component.projection);
                    judge.captured_place_projection(&place, language_semantics::content::ContentPlaceVersion::Entry)
                }).and_then(|value| {
                    super::super::structural_judgment::owned_snapshots::CapturedOwnedSnapshot::completed(
                        &judge, state.symbol, actual, &value,
                    )
                });
            CapturedIntegerInput::Record {
                type_reference: actual,
                snapshot,
                reads,
            }
        };
        formed.push(judge.form_captured_integer_rank(&CapturedIntegerRank {
            input,
            minimum: component.minimum.clone(),
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
