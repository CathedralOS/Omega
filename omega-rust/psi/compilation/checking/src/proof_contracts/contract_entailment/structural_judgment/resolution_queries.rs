//! Active application questions belong to the whole cloned judge tree.
//!
//! Integer premise projection can return to the resolver with a fresh local
//! depth. Retain the exact question and its premise context until its contract
//! or body finishes instead of treating that return as a new independent
//! computation. The caller chooses its term mode before entering the question;
//! guarding contract equations must not change the surrounding proof meaning.

use super::{StructuralJudge, StructuralTerm};
use std::sync::Arc;

pub(super) struct ApplicationResolution<'program> {
    application: StructuralTerm,
    context: StructuralJudge<'program>,
    previous: Option<Arc<Self>>,
}

impl<'program> StructuralJudge<'program> {
    pub(super) fn resolving_application(mut self, application: &StructuralTerm) -> Option<Self> {
        let mut context = self.clone();
        // The context holds its semantic owners, not the active query chain.
        // The separate predecessor edge below keeps this an acyclic history.
        context.active_application = None;
        let mut previous = self.active_application.as_deref();
        while let Some(active) = previous {
            if active.application.same_resolution_payload(application)
                && active.context.same_resolution_context(&context)
            {
                return None;
            }
            previous = active.previous.as_deref();
        }
        self.active_application = Some(Arc::new(ApplicationResolution {
            application: application.clone(),
            context,
            previous: self.active_application.clone(),
        }));
        Some(self)
    }

    /// Opaque leaf retention must not omit mutable dependency collection.
    pub(super) fn has_resolution_dependency_collector(&self) -> bool {
        if self.returned_field_trace.is_some() {
            return true;
        }
        let mut previous = self.active_application.as_deref();
        while let Some(active) = previous {
            if active.context.returned_field_trace.is_some()
                // resolving_application clears this edge in every saved context.
                // Refuse an unfamiliar nested context rather than omit its effects.
                || active.context.active_application.is_some()
            {
                return true;
            }
            previous = active.previous.as_deref();
        }
        false
    }

    fn same_resolution_context(&self, other: &Self) -> bool {
        // Each active frame keeps these immutable owners alive. Pointer
        // equality therefore identifies the same premises/storage epoch, not
        // a recycled address. COW fact changes remain independent questions.
        // Program-pure memo cells and diagnostic traces supply no premises.
        std::ptr::eq(self.program, other.program)
            && Arc::ptr_eq(&self.entry_machines, &other.entry_machines)
            && self.machine_symbol == other.machine_symbol
            && self.declaration_state == other.declaration_state
            && Arc::ptr_eq(&self.declaration_parameters, &other.declaration_parameters)
            && std::ptr::eq(self.body_type_parameters, other.body_type_parameters)
            && self.premise_facts == other.premise_facts
            && Arc::ptr_eq(&self.normal_results, &other.normal_results)
            && Arc::ptr_eq(
                &self.completed_nominal_captures,
                &other.completed_nominal_captures,
            )
            && Arc::ptr_eq(&self.returned_field_owners, &other.returned_field_owners)
            && Arc::ptr_eq(&self.body_invocations, &other.body_invocations)
            && Arc::ptr_eq(&self.normal_memberships, &other.normal_memberships)
            && Arc::ptr_eq(
                &self.normal_content_guarantees,
                &other.normal_content_guarantees,
            )
            && Arc::ptr_eq(&self.invalidated_storage, &other.invalidated_storage)
            && Arc::ptr_eq(&self.storage_exposures, &other.storage_exposures)
            && Arc::ptr_eq(&self.stored_values, &other.stored_values)
            && self.capture_sequence == other.capture_sequence
            && self.storage_write_sequence == other.storage_write_sequence
            && Arc::ptr_eq(&self.normal_guarantees, &other.normal_guarantees)
            && Arc::ptr_eq(
                &self.captured_result_equations,
                &other.captured_result_equations,
            )
            && self.resolve_applications == other.resolve_applications
            && self.require_closed_computation == other.require_closed_computation
            && self.runtime_body_values == other.runtime_body_values
            && self.specification_terms == other.specification_terms
            && self.case_facts_only == other.case_facts_only
            && Arc::ptr_eq(&self.substitutions, &other.substitutions)
            && Arc::ptr_eq(&self.rewrites, &other.rewrites)
            && Arc::ptr_eq(&self.disequalities, &other.disequalities)
            && Arc::ptr_eq(&self.case_facts, &other.case_facts)
            && Arc::ptr_eq(&self.case_substitutions, &other.case_substitutions)
            && self.hypotheses_contradictory == other.hypotheses_contradictory
    }
}
