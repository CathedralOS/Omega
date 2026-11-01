//! Checked mathematical predicates use the ordinary completed-value namespace.
//!
//! A transparent definition's producer retains selected scalar meaning and
//! format. Substituting its captures must preserve those facts, including IEEE
//! relations, rather than treating every source capture as a kernel integer.
//! The caller supplies already observed actuals; this reader neither replays
//! source bodies nor rereads a callee formal in the caller's storage.

use super::{StructuralJudge, StructuralJudgment, StructuralTerm};
use typed_trees::expression::{ExpressionHandle, StaticMachineArgument};
use typed_trees::signature::StateParameter;
use typed_trees::types::TypeReferenceHandle;

impl StructuralJudge<'_> {
    pub(in crate::proof_contracts::contract_entailment) fn instantiated_mathematical_predicate(
        &self,
        owner: symbols::SymbolHandle,
        parameters: &[StateParameter],
        expression: ExpressionHandle,
        bindings: &[(String, StructuralTerm)],
        machine_bindings: &[(
            symbols::SymbolHandle,
            TypeReferenceHandle,
            StaticMachineArgument,
        )],
    ) -> StructuralJudgment {
        if !self.runtime_body_values || self.case_facts_only {
            return StructuralJudgment::Unknown;
        }
        let predicate = super::super::structural_terms::mathematical_application_term(
            self.program,
            expression,
            |capture| {
                let value = super::super::structural_terms::structural_term(
                    self.program,
                    None,
                    Some((owner, parameters)),
                    capture,
                    None,
                )?;
                Some(Self::substitute_call_term(
                    &value,
                    bindings,
                    machine_bindings,
                ))
            },
        );
        predicate.map_or(StructuralJudgment::Unknown, |predicate| {
            self.judge_observed_predicate(predicate, 0)
        })
    }
}
