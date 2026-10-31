//! Resultless calls complete the same public contract as value calls.
//!
//! Observe eager operands once, capture entry requirements and fresh output
//! subjects, retire the call's own writes, then install its normal guarantees.
//! The statement handle owns occurrence identity when no result expression exists.

use super::super::super::specification_calls::{
    call_parameter_scope, owner_is_proof, selected_call_site_contract,
};
use super::*;
use typed_trees::statement::{StatementNode, TableCall};

impl<'program> StructuralJudge<'program> {
    pub(in crate::proof_contracts::contract_entailment) fn statement_has_runtime_completion(
        &self,
        caller: &typed_trees::machine::Machine,
        call: &'program TableCall,
    ) -> bool {
        self.requires_value_preservation()
            && selected_call_site_contract(
                self.program,
                caller.symbol,
                crate::CallSite::Statement(call),
            )
            .is_some_and(|owner| {
                !owner_is_proof(
                    self.program,
                    &crate::proof_only_classification(self.program),
                    &owner,
                )
            })
    }

    pub(in crate::proof_contracts::contract_entailment) fn normal_statement_completion_paths(
        &self,
        call: &'program TableCall,
        statement_index: usize,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
    ) -> Option<Vec<StructuralJudge<'program>>> {
        let program = self.program;
        let site = crate::CallSite::Statement(call);
        let owner = selected_call_site_contract(program, scope.0.symbol, site)?;
        let parameters = call_parameter_scope(program, scope.0.symbol, &owner, site)?;
        let arguments = crate::call_site_argument_expressions(program, &site);
        let application =
            AppliedProofBindings::for_call_site(program, scope.0.symbol, &owner, site);
        let statement = scope.1.statement_nodes.handles().nth(statement_index)?;
        if !matches!(program.statement_table.statements(scope.1.statement_nodes).get(statement_index), Some(StatementNode::Call(selected)) if std::ptr::eq(selected, call))
        {
            return None;
        }
        let mut completed = Vec::new();
        for entry in self.normal_argument_entries(
            arguments,
            parameters.as_ref(),
            environment,
            scope,
            Some(site),
        )? {
            let actuals = entry.observed_arguments()?;
            let mut judge = entry.judge;
            let writes = judge.frame_resolver()?.may_write_frame(scope.0, call);
            let mut post_storage = Vec::new();
            if call.evidence_arguments.is_empty()
                && let Some(application) = &application
                && let Some(captured) = judge.capture_call_bindings(
                    site,
                    ExpressionHandle::invalid(),
                    statement,
                    &owner,
                    application,
                    parameters.as_ref(),
                    Some(&actuals),
                    environment,
                    scope,
                    &writes,
                )
                && let Some(guarantees) = judge.selected_guarantees(
                    &owner,
                    &captured.entry_bindings,
                    &captured.bindings,
                    &application.types,
                    None,
                    0,
                )
            {
                judge.intake_normal_call_equations(
                    guarantees
                        .equations
                        .into_iter()
                        .map(CapturedResultEquation::public)
                        .collect(),
                );
                for case in judge.normal_call_case_guarantees(
                    &owner,
                    parameters.as_ref(),
                    &captured.bindings,
                    None,
                ) {
                    judge.intake_case_guarantee(&case);
                }
                std::sync::Arc::make_mut(&mut judge.normal_content_guarantees)
                    .extend(guarantees.content);
                post_storage = captured.post_storage;
            }
            judge.observe_storage_effects(scope.1, writes.complete_paths())?;
            for storage in post_storage {
                judge.install_post_call_storage(scope.1, storage);
            }
            completed.push(judge);
        }
        Some(completed)
    }
}
