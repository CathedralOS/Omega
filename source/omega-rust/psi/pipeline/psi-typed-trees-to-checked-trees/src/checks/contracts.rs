mod arrivals;
mod assembly;
mod calls;
mod direct;
mod domains;
mod evaluator;
mod evidence;
mod exits;
mod writes;
// `pub(super)` so the operator-`requires` discharge (checks/operators) can
// reuse the domain-derived boolean proving labels.
pub(crate) mod labels;
mod places;
mod prover;

use calls::check_call_requires;
pub(crate) use evaluator::call_site_boolean_contract_expression_value;
pub(crate) use evidence::{
    exact_target_evidence_parameters, instantiate_contract_expression_evidence_parameter,
};
use exits::check_exit_ensures;
use psi_checked_trees::CheckFacts;
use psi_diagnostics::Diagnostic;
use writes::check_domain_field_writes;

pub(super) fn check_flow_call_contracts(
    program: &psi_typed_trees::TypedTrees,
    facts: &CheckFacts,
    incoming_guards: &crate::checks::ranges::incoming_guards::IncomingGuardIndex,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    assembly::check_assembly_fact_contracts(program, facts, &mut diagnostics);

    // PROOF-machine calls are exempt from the runtime requires prover: a
    // proof machine emits no runtime code, and a call between proof
    // machines denotes a mathematical application whose VALUE does not
    // depend on the callee's requires -- the requires conditions only the
    // callee's ENSURES, and every ensures-consumption face is gated in the
    // structural validation layer (citation site discharge, IH premise
    // discharge, functional-ensures exclusion for requires-bearing
    // callees). Keeping this prover on proof-proof calls double-gates and
    // refuses sound requires-bearing INDUCTION, whose recursive call's
    // premise only the structural judge can derive (injectivity
    // decomposition of the arm-refined requires; probed 2026-07-16 with
    // add_cancel).
    let proof_only = psi_typed_trees::proof_only::classify(program);
    // Call targets carry the callee's ENTRY-STATE symbol (sub-state targets
    // carry that state's); resolve through states as well as the machine
    // symbol itself.
    let is_proof_machine = |symbol: psi_symbols::SymbolHandle| {
        program
            .machines()
            .iter()
            .find(|machine| {
                machine.symbol == symbol
                    || program
                        .machine_states(machine)
                        .iter()
                        .any(|state| state.symbol == symbol)
            })
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
            if caller_is_proof && is_proof_machine(call_flow.target_symbol) {
                continue;
            }
            check_call_requires(
                program,
                facts,
                state_flow,
                call_flow,
                incoming_guards.for_machine(state_flow.machine_symbol),
                &mut diagnostics,
            );
        }
        for exit_flow in facts.flow.control.exits.span_or_empty(state_flow.exits) {
            check_exit_ensures(program, facts, state_flow, exit_flow, &mut diagnostics);
        }
        arrivals::check_self_transition_arrival_requires(
            program,
            facts,
            state_flow,
            &mut diagnostics,
        );
        // #66 write-enforcement: every assignment into a domain-refined field must
        // establish the value in that domain (the soundness floor for trusting the
        // field's declared domain on read).
        check_domain_field_writes(program, facts, state_flow, &mut diagnostics);
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

pub(crate) fn bind_call_evidence_arguments(
    program: &psi_typed_trees::TypedTrees,
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
