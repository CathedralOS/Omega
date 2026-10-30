//! Observe private body occurrences at ordered call entry, before completion.
//!
//! A returned value may be hoisted into an ordinary local by lowering. Its
//! callee still contains the same source occurrences. Statement ownership and
//! normal operation entries preserve earlier siblings, effects and snapshots;
//! evaluating a detached call expression would lose that context. This walk
//! observes the callee without importing its own uncompleted guarantees.

use super::*;
use crate::proof_contracts::contract_entailment::specification_calls::{
    SelectedCallContract, selected_call_site_contract,
};
use crate::proof_contracts::contract_entailment::structural_judgment::normal_returns::OperationOperandDemand;

#[allow(clippy::too_many_arguments)]
pub(super) fn observe_statement<'program>(
    program: &'program dyn crate::CheckingInput,
    theorem: &Machine,
    invocation: &Machine,
    judge: &StructuralJudge<'program>,
    classification: &typed_trees::proof_only::ProofOnlyClassification,
    diagnostics: &mut Vec<Diagnostic>,
    states: &[typed_trees::state::State],
    state: &typed_trees::state::State,
    statement_index: usize,
    parameter_names: &[String],
    environment: &[(String, StructuralTerm)],
    hypotheses: &[(String, StructuralTerm)],
    equations: &[(StructuralTerm, StructuralTerm)],
    citations: &[PendingStructuralCitation],
    path: &mut Vec<SymbolHandle>,
    fresh: &mut usize,
    requirement: &mut Option<&mut dyn StructuralPrefixVisitor>,
    guarantees: &CitationFacts,
) -> Option<()> {
    let observer = requirement.as_deref()?;
    if observer.site().state == state.symbol || path.contains(&observer.site().state) {
        return Some(());
    }
    for site in crate::machine_calls::calls::collect_call_sites_in_statement(
        program,
        invocation,
        state,
        statement_index,
    ) {
        let Some(SelectedCallContract::Concrete(callee, entry)) =
            selected_call_site_contract(program, invocation.symbol, site)
        else {
            continue;
        };
        let observer = requirement.as_deref()?;
        if path.contains(&entry.symbol) || !prefix_site_is_reachable(program, entry, observer) {
            continue;
        }
        if !body_can_be_observed(
            program,
            callee,
            classification,
            judge.requires_value_preservation(),
        ) {
            return None;
        }
        let parameters = program.state_parameters(entry);
        let expression = match site {
            crate::CallSite::Expression { expression, .. } => Some(expression),
            _ => None,
        };
        let frames = if let Some(expression) = expression {
            judge.normal_operation_entries(
                statement_index,
                expression,
                OperationOperandDemand::All,
                environment,
                (invocation, state, state.return_type),
            )?
        } else {
            judge.normal_argument_entries(
                crate::call_site_argument_expressions(program, &site),
                parameters,
                environment,
                (invocation, state, state.return_type),
                Some(site),
            )?
        };
        let subjects = if judge.requires_value_preservation() {
            let frames = judge.frame_resolver()?;
            match site {
                crate::CallSite::Expression { expression, .. } => {
                    frames.expression_entry_reference_subjects(invocation, expression, parameters)
                }
                crate::CallSite::Statement(call) => {
                    frames.statement_entry_reference_subjects(invocation, call, parameters)
                }
                _ => None,
            }
        } else {
            None
        }
        .unwrap_or_default();
        for frame in frames {
            let arguments = frame.observed_arguments()?;
            let (incoming, captured) = frame.judge.bind_invocation(
                entry,
                &arguments,
                &subjects,
                environment,
                (invocation, state, state.return_type),
            )?;
            if let Some(observer) = requirement.as_deref_mut() {
                observer.entered_body(state.symbol, statement_index, expression, entry.symbol);
            }
            recognize_structural_state_leaves(
                program,
                theorem,
                &captured,
                classification,
                diagnostics,
                states,
                entry,
                parameter_names,
                incoming,
                hypotheses.to_vec(),
                equations.to_vec(),
                citations.to_vec(),
                true,
                path,
                fresh,
                requirement,
                guarantees.clone(),
            )?;
        }
    }
    Some(())
}

/// Observing an executed prefix or normal completion does not treat the callee
/// as a repeatable mathematical function. Runtime observation retains ordered
/// storage effects; theorem and case-only traversal keep their existing policy.
pub(in crate::proof_contracts::contract_entailment) fn body_can_be_observed(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    classification: &typed_trees::proof_only::ProofOnlyClassification,
    preserve_runtime_values: bool,
) -> bool {
    if !machine.body_is_present
        || !machine.supply_mode.is_checked_body()
        || machine.target_family_interface
    {
        return false;
    }
    if preserve_runtime_values || classification.is_proof_machine(program, machine) {
        return true;
    }
    let Some(entry) = program.machine_states(machine).first() else {
        return false;
    };
    let eligible = |operational: &flow_effects::OperationalPlan,
                    reaches: &flow_effects::ServiceReachInferencePlan| {
        let result = crate::machine_calls::denotational_calls::checked_value_call_candidate(
            program,
            machine,
            entry,
            operational,
            reaches,
        );
        if super::super::struct_trace()
            && let Err(reason) = &result
        {
            eprintln!(
                "STRUCT body opaque machine={} reason={reason:?}",
                machine.name
            );
        }
        result.is_ok()
    };
    if let Some(plans) = program.prepared_effect_plans() {
        return eligible(&plans.operational, &plans.service_reaches);
    }
    let operational = crate::machine_calls::effect_inference::infer_operational_may(program);
    let reaches =
        crate::machine_calls::effect_inference::infer_service_reaches(program, &operational);
    eligible(&operational, &reaches)
}
