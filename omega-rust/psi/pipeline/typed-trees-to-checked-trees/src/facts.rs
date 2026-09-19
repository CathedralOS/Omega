//!
//! This file builds the check facts. `placed_views_and_uses.rs` builds placed
//! view inputs and nominal machine uses, `machine_facts.rs` termination,
//! blocking, suspension and invocation facts, `dynamic_conformance.rs`
//! dynamic conformance facts, `contract_plan_facts.rs` contract plans and
//! mutation facts, `crash_plan_facts.rs` crash capsules, buckets and sites,
//! `canonical_encoding.rs` canonical contract and expression encodings,
//! `requirement_call_specializations.rs` the specializations generic calls
//! derived at their static machine arguments, and `qualification_facts.rs`
//! qualification and service reach facts.

mod canonical_encoding;
pub(crate) mod capabilities;
mod carry;
pub(crate) mod contract_occurrences;
mod contract_plan_facts;
mod crash_calls;
mod crash_entry_values;
mod crash_plan_facts;
mod dynamic_conformance;
pub(crate) mod field_domain;
mod index_compatibility;
mod machine_facts;
pub(crate) mod operator_crashes;
mod placed_views_and_uses;
pub(crate) mod qualification_evidence;
mod qualification_facts;
mod requirement_call_specializations;
pub(crate) mod review_sources;
#[cfg(test)]
mod scalar_contract_tests;
mod where_requirements;

pub(crate) use canonical_encoding::{domain_is_vacuous, encode_contract_set_canonical};
pub(crate) use crash_calls::{infer_checked_crash_causes, infer_checked_machine_crash_causes};
pub(crate) use crash_plan_facts::{
    canonical_crash_binary_path_predicate, canonical_crash_operand_identity,
    canonical_crash_path_predicate, derive_authored_machine_crash_buckets,
    derive_authored_operator_crash_buckets, derive_authored_signature_crash_buckets,
};
pub(crate) use dynamic_conformance::normalized_dynamic_row_identities;

/// Checker-side read of the shared crash entry-provenance law. Guard
/// admission and call-actual substitution must agree on when a source read
/// still evaluates to the invocation-entry operand its spelling claims;
/// `crash_entry_values` stays the only owner of that answer, so this
/// forwards without re-deriving provenance in the check layer.
pub(crate) fn crash_entry_operand(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    before_statement: usize,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<checked_trees::CrashPredicateExpression> {
    crash_entry_values::entry_operand(program, machine, state, before_statement, expression)
}

use crate::borrow::build_borrow_facts;
use crate::facts::capabilities::build_capability_facts;
use crate::facts::contract_plan_facts::{build_contract_plans, build_mutation_facts};
use crate::facts::dynamic_conformance::build_dynamic_conformance_facts;
use crate::facts::machine_facts::{
    build_blocking_facts, build_suspension_facts, build_synchronous_invocation_facts,
    build_termination_facts,
};
use crate::facts::placed_views_and_uses::{
    build_checked_placed_view_inputs, build_nominal_machine_use_facts,
};
use crate::facts::qualification_facts::{build_qualification_facts, build_service_reach_facts};
use crate::facts::requirement_call_specializations::build_requirement_call_specialization_facts;
use crate::flow::{build_domain_facts, build_flow_facts_with_service_reaches};
use crate::operators::{
    bind_boundary_operator_application_demands, build_operator_facts,
    select_pending_domain_operator_meanings,
};
use crate::proof::build_proof_facts_with_operators;
use crate::semantic::build_semantic_facts;
use crate::values::build_value_facts;
use checked_trees::CheckFacts;
use flow_effects::OperationalPlan;
use proof::obligations::ProofPlan;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

#[derive(Clone, Copy)]
struct MachineSuspensionRow {
    symbol: SymbolHandle,
    transitive_may_suspend: bool,
}

#[derive(Clone, Copy)]
struct MachineBlockingRow {
    symbol: SymbolHandle,
    transitive_may_block: bool,
}

fn project_operational_rows(
    operational: &OperationalPlan,
) -> (Vec<MachineSuspensionRow>, Vec<MachineBlockingRow>) {
    let suspensions = operational
        .machines()
        .iter()
        .map(|summary| MachineSuspensionRow {
            symbol: summary.symbol,
            transitive_may_suspend: summary.transitive_may_suspend,
        })
        .collect();
    let blocking = operational
        .machines()
        .iter()
        .map(|summary| MachineBlockingRow {
            symbol: summary.symbol,
            transitive_may_block: summary.transitive_may_block,
        })
        .collect();
    (suspensions, blocking)
}

pub(crate) fn build_check_facts(
    program: &TypedTrees,
    proof_plan: &ProofPlan<'_>,
    operational: OperationalPlan,
    service_reach_inference: flow_effects::ServiceReachInferencePlan,
    validation_facts: &validation::ProgramValidationFacts,
    static_machine_selections: validation::ValidatedStaticMachineSelections,
    mutation_summaries: &crate::flow::StateMutationSummaryCache,
) -> Result<CheckFacts, Vec<diagnostics::Diagnostic>> {
    let borrow = build_borrow_facts(program);
    let mut values = build_value_facts(program, proof_plan);
    let mut operators = build_operator_facts(program, &values);
    let mut proof = build_proof_facts_with_operators(program, proof_plan, &borrow, &operators)?;
    crate::proof::bind_float_meaning_projection_facts(
        program,
        &mut proof,
        &validation_facts.float_meaning_projection_invocations,
        &validation_facts.float_meaning_equality_propositions,
    )?;
    crate::proof::bind_proof_output_call_facts(program, &mut proof)?;
    crate::proof::bind_evidence_forwarding_facts(program, &mut proof)?;
    crate::proof::bind_outcome_specific_arm_facts(program, &mut proof)?;
    crate::proof::bind_evidence_projection_facts(program, &mut proof)?;
    let mut semantic = build_semantic_facts(program, &proof);
    let domains = build_domain_facts(program, &semantic);
    let dynamic_conformances = build_dynamic_conformance_facts(program)?;
    // Meaning selection depends only on declarations and signatures. Complete
    // selected scalar plans before flow captures their evaluated local values.
    select_pending_domain_operator_meanings(program, &mut operators);
    bind_boundary_operator_application_demands(
        program,
        &validation_facts.boundary_operator_applications,
        &mut operators,
    )?;
    values.scalar_expressions = crate::values::build_checked_scalar_expression_plans(
        program,
        &operators,
        &validation_facts.exact_integer_casts,
    );
    // One frame resolver serves the whole immutable fact-construction window:
    // flow classification, terminal ranking projections, termination progress
    // proofs, mutation frames, and crash-route refinement all classify the
    // same typed program.
    let call_frames = validation::CallFrameResolver::new(program);
    let mut flow = build_flow_facts_with_service_reaches(
        program,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operational,
        &service_reach_inference,
        &values.scalar_expressions,
        &operators,
        &validation_facts.exact_integer_casts,
        mutation_summaries,
        call_frames.as_ref(),
    );
    crate::facts::review_sources::bind_checked_body_call_source_spans(program, &mut flow)?;
    crate::values::retain_nested_structural_call_arguments(
        program,
        &operators,
        &flow,
        &mut values.scalar_expressions,
        &validation_facts.exact_integer_casts,
    );
    (values.scalar_computations, values.structural_values) =
        crate::values::build_checked_value_computation_plans(
            program,
            &operators,
            &flow,
            &borrow,
            &proof,
            &values.scalar_expressions,
            &validation_facts.exact_integer_casts,
        );
    let index_compatibility = index_compatibility::build_index_compatibility_facts(
        program, &operators, &semantic, &flow, &proof,
    )?;
    flow.terminal_scalar_graphs =
        crate::execution::terminal_scalar::build_checked_scalar_graph_plans_with_call_frames(
            program,
            &values.scalar_expressions,
            &values.scalar_computations,
            &values.structural_values,
            call_frames.as_ref(),
        );
    flow.terminal_machines =
        crate::execution::terminal_scalar::build_checked_terminal_machine_selections(
            program,
            call_frames.as_ref(),
        );
    flow.terminal_debug =
        crate::execution::terminal_debug::build_checked_terminal_debug_plans(program);
    let capabilities = build_capability_facts(program, &service_reach_inference, &flow);
    let (machine_suspensions, machine_blocking) = project_operational_rows(&operational);
    // STR4 checked plans, slice 2: semantic-domain commitments per machine.
    let qualifications = build_qualification_facts(program);
    // EFX: direct synchronous invocation is a separately published checked
    // axis, never reconstructed from service reach or flow-call topology.
    let synchronous_invocations = build_synchronous_invocation_facts(program);
    // EFX: suspension is published independently from worker blocking and
    // retains public negative guarantees separately from private inference.
    let suspensions = build_suspension_facts(program, &machine_suspensions);
    // EFX: worker blocking is published independently from suspension and
    // retains public negative guarantees separately from private inference.
    let blocking = build_blocking_facts(program, &machine_blocking);
    // TPR/EFX: termination is published as an independent exact-machine root.
    let termination = build_termination_facts(
        program,
        &flow,
        &semantic,
        validation_facts,
        call_frames.as_ref(),
    )?;
    let mut fact_call_projection_diagnostics = Vec::new();
    validation::validate_ordered_requirement_call_totality(
        program,
        &operational,
        &service_reach_inference,
        &termination,
        &mut fact_call_projection_diagnostics,
    );
    // Consume the service-reach arenas only after fact-call eligibility has
    // borrowed their exact inferred closure alongside finalized termination.
    let service_reaches = build_service_reach_facts(program, service_reach_inference);
    // R5/STR: body-derived mutation frames are an independent checked axis,
    // never a field of the published machine contract.
    let mutation = build_mutation_facts(program, call_frames.as_ref());
    // STR4 checked plans: the normalized machine contracts (published
    // halves + fingerprint; prover-independent by construction).
    let contract_plans = build_contract_plans(
        program,
        &service_reaches,
        &synchronous_invocations,
        &suspensions,
        &blocking,
        &termination,
        &mutation,
        &capabilities,
        &flow,
        &operators,
        &semantic,
        &validation_facts.exact_integer_casts,
        call_frames.as_ref(),
    )?;
    proof.contract_entailment_assumption_discharges =
        crate::proof::build_contract_entailment_assumption_discharges(program, &contract_plans)?;
    let nominal_machine_uses = build_nominal_machine_use_facts(
        program,
        static_machine_selections.nominal_uses,
        &contract_plans,
    )?;
    // A structural machine-parameter contract emits no nominal use row, so
    // the specialization MP2b derived at its call edge survives only here.
    let requirement_call_specializations = build_requirement_call_specialization_facts(
        static_machine_selections.requirement_call_specializations,
    )?;
    // CRY1: materialize the effective structural policy once in the checked
    // fact layer; authored clauses remain minimum promises on typed data.
    let carry = carry::build_carry_facts(program);
    let mut fact_call_projections = Vec::new();
    for call in &validation_facts.integer_embedding_calls {
        let exact_source = matches!(program.expression_table.expression(call.call_expression),
            typed_trees::expression::ExpressionNode::Call(source) if source.target_symbol == call.target_state)
            && program
                .machines()
                .iter()
                .filter(|machine| machine.symbol == call.target_machine)
                .filter(|machine| {
                    program
                        .machine_states(machine)
                        .iter()
                        .filter(|state| state.symbol == call.target_state)
                        .count()
                        == 1
                })
                .count()
                == 1;
        let total = termination.for_machine(call.target_machine).is_some_and(|plan| {
            matches!(&plan.checked_summary,
                language_semantics::TerminationGuarantee::Terminates { premises } if premises.is_empty())
        });
        if !exact_source || !total {
            let target = program.symbols.name(call.target_state);
            fact_call_projection_diagnostics.push(diagnostics::Diagnostic::error(format!(
                "`embed` source call `{target}` is not denotational: the exact selected machine is not unconditionally terminating"
            )));
        }
    }
    for projection in &validation_facts.fact_call_projections {
        let total = termination
            .for_machine(projection.target_machine)
            .is_some_and(|plan| {
                matches!(
                    &plan.checked_summary,
                    language_semantics::TerminationGuarantee::Terminates { premises }
                        if premises.is_empty()
                )
            });
        if !total {
            let target = program.symbols.name(projection.target_state);
            fact_call_projection_diagnostics.push(diagnostics::Diagnostic::error(format!(
                "fact-position projection from call `{target}` is not denotational: the selected machine is not unconditionally terminating"
            )));
            continue;
        }
        fact_call_projections.push(checked_trees::CheckedFactCallProjection {
            projection_expression: projection.projection_expression,
            call_expression: projection.call_expression,
            target_machine: projection.target_machine,
            target_state: projection.target_state,
            machine_arguments: projection.machine_arguments.clone(),
            result_type: projection.result_type,
            field: projection.field,
        });
    }
    if !fact_call_projection_diagnostics.is_empty() {
        return Err(fact_call_projection_diagnostics);
    }

    Ok(CheckFacts {
        semantic,
        borrow,
        proof,
        values,
        domains,
        dynamic_conformances,
        nominal_machine_uses,
        requirement_call_specializations,
        operators,
        capabilities,
        flow,
        index_compatibility,
        mutation,
        service_reaches,
        synchronous_invocations,
        suspensions,
        blocking,
        termination,
        qualifications,
        contract_plans,
        carry,
        fact_call_projections,
        placed_view_inputs: build_checked_placed_view_inputs(program),
        ..CheckFacts::default()
    })
}

/// Crash refinement gains path-conditioned and permission-frontier evidence
/// during checked-fact validation. Refresh only that independently mutable
/// axis so the realized envelope cannot retain the earlier pre-check snapshot.
pub(crate) fn refresh_realized_contract_envelopes(facts: &mut CheckFacts) {
    for envelope in &mut facts.contract_plans.realized_envelopes {
        let contract = facts
            .contract_plans
            .machines
            .iter()
            .find(|contract| contract.machine == envelope.machine)
            .expect("every realized envelope must retain its exact machine contract");
        assert_eq!(
            envelope.contract_report_fingerprint, contract.report_fingerprint,
            "realized envelope contract identity drifted during checked validation"
        );
        envelope.checked_crash = contract.crash.clone();
    }
    facts
        .contract_plans
        .validate_resource_envelopes()
        .expect("checked resource envelopes must survive independent post-validation replay");
}
