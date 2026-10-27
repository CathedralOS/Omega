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
        if !callee.body_is_present
            || !callee.supply_mode.is_checked_body()
            || callee.target_family_interface
            || !transparent_body_is_observation_free(program, callee, classification)
        {
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
        for frame in frames {
            let arguments = frame.observed_arguments()?;
            if arguments.len() != parameters.len() {
                return None;
            }
            let incoming = parameters
                .iter()
                .zip(arguments)
                .map(|(parameter, value)| (parameter.name.as_str().to_owned(), value))
                .collect();
            if let Some(observer) = requirement.as_deref_mut() {
                observer.entered_body(state.symbol, statement_index, expression, entry.symbol);
            }
            recognize_structural_state_leaves(
                program,
                theorem,
                &frame.judge,
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
