//! Admit exact selected checked execution without exposing a second evaluator.
//! The private occurrence rewrite supplies graph edges only. Published source
//! effect summaries, invocation premises and declaration selections remain the
//! admission floor, and selected bodies contribute their complete own closure.

use super::{
    BuildTimeAdmissionPlan, BuildTimeInvocationCustody, BuildTimeProgram,
    BuildTimeSelectionAuthority,
};
use std::sync::Arc;
use symbols::SymbolHandle;

pub struct SelectedBuildTimeClosureAdmission<'program> {
    pub program: &'program checked_trees::CheckedTrees,
    pub selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    pub selections: crate::SelectedBuildTimeOperators<'program>,
    pub machine_bodies: &'program [crate::SelectedBuildTimeMachineBody],
}

impl SelectedBuildTimeClosureAdmission<'_> {
    /// Prepare one shared graph, then retain each invocation's own refusal.
    /// These are source admission results, not execution or native authority.
    pub fn admit(
        self,
        invocations: impl IntoIterator<Item = (SymbolHandle, BuildTimeInvocationCustody)>,
    ) -> Result<Vec<Result<Vec<SymbolHandle>, String>>, String> {
        let typed = self.program.typed.as_ref();
        let admission = BuildTimeAdmissionPlan::infer(typed, self.selection_authority)
            .with_selected_operators(typed, self.selections.operators)?
            .with_selected_provider_bodies(typed, self.selections.provider_bodies)?
            .with_selected_machine_bodies(typed, self.machine_bodies)?;
        Ok(invocations
            .into_iter()
            .map(|(machine, custody)| {
                admission.admitted_program_machine_closure_symbols(
                    BuildTimeProgram::Checked(self.program),
                    machine,
                    custody,
                )
            })
            .collect())
    }
}
