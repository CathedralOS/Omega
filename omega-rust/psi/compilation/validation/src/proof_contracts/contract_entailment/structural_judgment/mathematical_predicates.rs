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
        // Proof contexts and runtime prefixes both supply instantiated actuals.
        // Tag preservation alone cannot establish a value predicate.
        if self.case_facts_only {
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
                let instantiated = Self::substitute_call_term(&value, bindings, machine_bindings);
                self.trace_le_case(
                    "call-capture",
                    &[("source", &value), ("actual", &instantiated)],
                    format_args!("owner={owner:?} expression={expression:?} capture={capture:?}"),
                );
                Some(instantiated)
            },
        );
        if self.le_case_trace_enabled() {
            let prepared = self
                .program
                .prepared_mathematics()
                .is_some_and(|view| view.application(expression).is_some());
            if let Some(predicate) = &predicate {
                self.trace_le_case(
                    "predicate-formation",
                    &[("predicate", predicate)],
                    format_args!(
                        "owner={owner:?} expression={expression:?} prepared={} formed=true",
                        prepared
                    ),
                );
            } else {
                self.trace_le_case(
                    "predicate-formation",
                    &[],
                    format_args!(
                        "owner={owner:?} expression={expression:?} prepared={} formed=false",
                        prepared
                    ),
                );
            }
            let total = self
                .captured_result_equations
                .iter()
                .filter(|row| row.is_data_identity())
                .count();
            for (index, row) in self
                .captured_result_equations
                .iter()
                .filter(|row| row.is_data_identity())
                .enumerate()
                .take(12)
            {
                self.trace_le_case(
                    "typed-row",
                    &[("left", &row.left), ("right", &row.right)],
                    format_args!("predicate entry row={index} total={total}"),
                );
            }
        }
        predicate.map_or(StructuralJudgment::Unknown, |predicate| {
            self.judge_observed_predicate(predicate, 0)
        })
    }
}
