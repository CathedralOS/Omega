//! Contract checks: each call's `requires`, each exit's `ensures`, each
//! self-transition arrival requirement and each write into a domain-refined
//! field must be proved from the checked facts at that point.
//!
//! Two entries, both called by
//! `checks::check_checked_facts_recording`:
//! `bind_call_evidence_arguments` runs first among the checks and binds the
//! evidence arguments of contract expressions and calls into the proof facts
//! (`evidence`); `check_flow_call_contracts` runs after the borrow check.
//!
//! `check_flow_call_contracts` builds the content conservation plans and the
//! declared field requirements of nominal parameters (`nominal_inputs`),
//! checks every state's `asm` requires and ensures assertions (`assembly`),
//! and prepares the exit entailment results (`entailment`) and the cyclic
//! header invariants (`exits`). It then walks each state's flow:
//!
//! 1. for each call: the scalar tail result domains (`exits`) and the call's
//!    requirements (`calls`, which also checks the nominal inputs);
//! 2. for each exit: the scalar result domains, the `ensures` clauses, the
//!    result field domains and the field domains of readable `&mut`
//!    referents (`exits`);
//! 3. writes into domain-refined fields (`writes`).
//!
//! The other children are not steps. `prover` decides whether fact contexts
//! prove a Boolean expression or a contract fact, and reaches `direct`,
//! `domains` and `evaluator`. `calls` also consults `call_bounds`,
//! `guard_operands`, `intervals` and `reference_domains`; `exits` consults
//! `content_preservation` and `integer_embeddings`; both consult
//! `entailment`. `labels`, `places` and `return_values` are shared
//! vocabulary: contract labels, place matching and exit return occurrences.

// The steps: evidence binding, and the checks `check_flow_call_contracts`
// runs.
mod argument_intervals;
mod assembly;
pub(crate) mod calls;
pub(crate) mod contextual_cleanup;
mod evidence;
mod exits;

mod nominal_inputs;
mod writes;

// The fact-context prover and the provers it reaches.
mod direct;
mod domains;
mod evaluator;
pub(in crate::checks) mod prover;
pub(crate) use prover::boolean_value_at_place;
pub(crate) use prover::call_guarantees::current_predicates as current_call_guarantee_predicates;
pub(crate) use prover::call_guarantees::proves_current_receiver as call_guarantees_prove_current_receiver;
pub(crate) use prover::contexts_prove_proposition_label;

// Provers the call requirement and exit checks consult directly.
mod call_bounds;
pub(crate) mod call_values;
mod content_preservation;
mod current_values;
mod entailment;
mod float_predicate_domains;
mod guard_operands;
mod integer_embeddings;
mod intervals;
mod reference_domains;

// Shared vocabulary: contract labels, place matching and exit return
// occurrences.
// `pub(super)` so the operator-`requires` discharge (checks/operators) can
// reuse the domain-derived boolean proving labels.
pub(crate) mod labels;
mod places;
mod return_values;

use calls::check_call_requires;
use checked_trees::CheckFacts;
use diagnostics::Diagnostic;
pub(crate) use evaluator::call_site_boolean_contract_expression_value;
pub(crate) use evidence::{
    exact_target_evidence_parameters, instantiate_contract_expression_evidence_parameter,
};
use exits::{CyclicHeaderInvariants, check_exit_ensures};
pub(crate) use exits::{
    exclusive_referent_type, is_readable_mutable_reference, value_provable_domain,
};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use writes::check_domain_field_writes;

pub(super) struct ContractDecisions {
    pub(super) constant_obligations: Vec<checked_trees::ConstantPredicateObligation>,
    pub(super) call_requires: Vec<checked_trees::DecidedCallRequires>,
    pub(super) value_qualifications: arena::Arena<checked_trees::CheckedValueQualification>,
}

pub(super) fn check_flow_call_contracts(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    incoming_guards: &crate::checks::ranges::incoming_guards::IncomingGuardIndex,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    proven_machine_contracts: &[(SymbolHandle, Vec<ExpressionHandle>)],
    expression_intervals: &[validation::ExpressionInterval],
) -> Result<ContractDecisions, Vec<Diagnostic>> {
    let mut diagnostics =
        crate::facts::contextual_cleanup::check_cleanup_requirements(program, facts);
    let mut decided = Vec::new();
    let mut constant_obligations = Vec::new();
    let mut value_qualifications = arena::Arena::default();
    let content_plans = validation::content_conservation_plans(program);
    let nominal_requirements = nominal_inputs::DeclaredFieldRequirements::new(&facts.semantic);
    let mut owned_call_frames = None;
    let call_frames =
        crate::flow::shared_call_frames_or(call_frames, program, &mut owned_call_frames);

    assembly::check_assembly_fact_contracts(program, facts, &mut diagnostics);

    // Mathematical applications owe the same substituted premises as runtime
    // calls, even when their result is erased or no guarantee is consumed.
    // Recursive descent establishes termination, not a call's preconditions.
    let proof_only = validation::proof_only_classification(program);
    let mut entailment = entailment::ProvenExitExpressions::new(
        program,
        &proof_only,
        call_frames,
        proven_machine_contracts,
    );
    let mut cyclic_headers = CyclicHeaderInvariants::new();
    // Call targets carry the callee's ENTRY-STATE symbol (sub-state targets
    // carry that state's); resolve through states as well as the machine
    // symbol itself.
    let is_proof_machine = |symbol: symbols::SymbolHandle| {
        program
            .machines()
            .iter()
            .find(|machine| program.machine_or_state_symbol(machine, symbol))
            .is_some_and(|machine| proof_only.is_proof_machine(program, machine))
    };

    for (_, state_flow) in facts.flow.control.states.iter() {
        let caller_is_proof = is_proof_machine(state_flow.machine_symbol);
        for call_flow in facts.flow.control.calls.span_or_empty(state_flow.calls) {
            if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
                eprintln!(
                    "CALLREQ caller={} proof={} target={} proof={}",
                    crate::labels::machine_name(program, state_flow.machine_symbol),
                    caller_is_proof,
                    crate::labels::call_target_label(program, call_flow.target_symbol),
                    is_proof_machine(call_flow.target_symbol),
                );
            }
            exits::check_state_transfer_result_domains(
                program,
                state_flow,
                call_flow,
                &mut diagnostics,
            );
            let before = diagnostics.len();
            check_call_requires(
                program,
                facts,
                state_flow,
                call_flow,
                &nominal_requirements,
                incoming_guards.for_machine(state_flow.machine_symbol),
                expression_intervals,
                call_frames,
                &mut constant_obligations,
                &mut diagnostics,
            );
            // Every fact is proved here or retained as a constant obligation
            // that target closure must discharge before Terminal production.
            if diagnostics.len() == before
                && let Ok(statement_index) = u32::try_from(call_flow.statement_index)
            {
                decided.push(checked_trees::DecidedCallRequires {
                    state: state_flow.state_symbol,
                    statement_index,
                    target: call_flow.target_symbol,
                });
            }
        }
        for exit_flow in facts.flow.control.exits.span_or_empty(state_flow.exits) {
            exits::check_scalar_result_domains(program, facts, exit_flow, &mut diagnostics);
            check_exit_ensures(
                program,
                facts,
                state_flow,
                exit_flow,
                entailment.for_machine(facts, state_flow.machine_symbol),
                &content_plans,
                call_frames,
                &mut cyclic_headers,
                &mut diagnostics,
            );
            // An owned nominal result also owes its declared field predicates
            // on the returned value itself -- enforced here so a caller may
            // consume them from the signature (flow/calls.rs).
            let exit_diagnostics = diagnostics.len();
            exits::check_result_field_domains(
                program,
                facts,
                state_flow,
                exit_flow,
                call_frames,
                &mut diagnostics,
            );
            // The return hands every readable `&mut` referent back to the
            // caller, so the field facts it assumed on entry are due again.
            // A reference return into that referent names the same place
            // twice; report each exact place once per exit.
            exits::check_exclusive_referent_field_domains(
                program,
                facts,
                exit_flow,
                &mut diagnostics,
            );
            let mut reported = Vec::new();
            for diagnostic in diagnostics.split_off(exit_diagnostics) {
                if !reported.contains(&diagnostic) {
                    reported.push(diagnostic);
                }
            }
            diagnostics.extend(reported);
        }
        // #66 write-enforcement: every assignment into a domain-refined field must
        // establish the value in that domain (the soundness floor for trusting the
        // field's declared domain on read).
        check_domain_field_writes(
            program,
            facts,
            state_flow,
            call_frames,
            &mut value_qualifications,
            &mut diagnostics,
        );
    }

    if diagnostics.is_empty() {
        Ok(ContractDecisions {
            constant_obligations,
            call_requires: decided,
            value_qualifications,
        })
    } else {
        Err(diagnostics)
    }
}

pub(crate) fn bind_call_evidence_arguments(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    evidence::bind_contract_expression_evidence_arguments(program, facts, &mut diagnostics);
    evidence::bind_call_evidence_arguments(program, facts, &mut diagnostics);
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
