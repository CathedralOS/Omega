use crate::labels::semantic_proof_obligation_kind;
use checked_trees::ProofFacts;
use facts::{Fact, FactOrigin, FactPayload, FactPlace, FactPlan, QualificationEvidence};
mod contracts;
mod field_domains;
mod points;
use contracts::append_contract_semantic_facts;
pub(crate) use contracts::{contract_fact_place, inherited_contract_scope_parts};
pub(crate) use contracts::{contract_parameter_position, projected_argument_place};
use field_domains::{append_local_zii_field_domain_facts, append_state_parameter_domain_facts};
use points::proof_obligation_point;

/// One observed property and its storage dependencies share one lifetime.
/// A dependency asserts no property of its own; an overlapping write retires
/// the context through ordinary invalidation instead of leaving a stale row.
pub(crate) fn append_observation_context(
    facts: &mut FactPlan,
    observation: Fact,
    dependencies: &[FactPlace],
) -> facts::FactContextHandle {
    let dependent = facts.append_fact(observation);
    let mut references = arena::HandleSpan::empty();
    facts.append_ref(&mut references, dependent);
    for &place in dependencies {
        let dependency = facts.append_fact(Fact {
            place,
            payload: FactPayload::StorageDependency { dependent },
            ..observation
        });
        facts.append_ref(&mut references, dependency);
    }
    facts.append_context(observation.point, references)
}

pub(crate) fn build_semantic_facts(
    program: &dyn ::validation::CheckingInput,
    proof: &ProofFacts,
    operators: &checked_trees::CheckedOperatorFacts,
) -> Result<FactPlan, diagnostics::Diagnostic> {
    let mut facts = validation::build_definition_fact_plan(program);
    crate::facts::qualification_evidence::prepare_issuers(program, proof, &mut facts)?;
    append_proof_obligation_semantic_facts(proof, &mut facts);
    append_contract_semantic_facts(program, proof, operators, &mut facts);
    // Receivers and named inputs have the same caller-enforced declared domains.
    // Actual zero-initialized storage is established separately below.
    append_state_parameter_domain_facts(program, &mut facts);
    append_state_parameter_predicates(program, operators, &mut facts);
    // ZII locals (`let room: Room;`): the zeroed storage already satisfies
    // every declared field domain that admits the empty byte sequence -- the
    // establishment gate -- so those fields are live evidence
    // from the binding onward.
    append_local_zii_field_domain_facts(program, &mut facts);

    Ok(facts)
}

/// Call admission checks these same declaration rows. Bind the complete rows
/// at entry, rather than rediscovering individual fields at a later consumer.
/// The roster excludes write-only contents; the normal predicate dependencies
/// retire a row when any storage it observes is changed.
fn append_state_parameter_predicates(
    program: &dyn validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    facts: &mut FactPlan,
) {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for row in validation::state_declaration_predicates(program, operators, state) {
                if std::env::var("OMEGA_TRACE_READ_BOUNDS")
                    .is_ok_and(|name| name == machine.name.as_str())
                {
                    eprintln!(
                        "READ-BOUNDS declaration {} {:?}",
                        machine.name, row.predicate
                    );
                }
                let Some(predicate) = crate::facts::boolean_predicates::bind_parameters(
                    program,
                    machine.symbol,
                    state.symbol,
                    program.state_parameters(state),
                    None,
                    row.predicate,
                ) else {
                    continue;
                };
                crate::facts::boolean_predicates::seed(
                    facts,
                    &predicate,
                    facts::ProgramPoint::State {
                        machine_symbol: machine.symbol,
                        state_symbol: state.symbol,
                    },
                    FactOrigin::StatementTransfer,
                );
            }
        }
    }
}

fn append_proof_obligation_semantic_facts(proof: &ProofFacts, facts: &mut FactPlan) {
    for (_, obligation) in proof.obligations.iter() {
        let point = proof_obligation_point(obligation);
        facts.append_fact_context(Fact {
            place: FactPlace::Unknown,
            point,
            origin: FactOrigin::ProofObligation,
            evidence: QualificationEvidence::default(),
            payload: FactPayload::ProofObligation {
                kind: semantic_proof_obligation_kind(obligation.kind.clone()),
            },
        });
    }
}
