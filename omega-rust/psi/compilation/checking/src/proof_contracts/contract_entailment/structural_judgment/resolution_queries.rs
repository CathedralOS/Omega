//! Active application questions belong to the whole cloned judge tree.
//!
//! Integer premise projection can return to the resolver with a fresh local
//! depth. Retain the exact question and its premise context until its contract
//! or body finishes instead of treating that return as a new independent
//! computation. The caller chooses its term mode before entering the question;
//! guarding contract equations must not change the surrounding proof meaning.

use super::{ResolutionPurpose, StructuralJudge, StructuralTerm};
use std::sync::Arc;

#[derive(Default)]
pub(super) struct ResolutionResults<'program> {
    completed: std::sync::Mutex<Vec<CompletedResolution<'program>>>,
}

struct CompletedResolution<'program> {
    input: StructuralTerm,
    depth: usize,
    context: StructuralJudge<'program>,
    output: StructuralTerm,
}

pub(super) struct ApplicationResolution<'program> {
    application: StructuralTerm,
    context: StructuralJudge<'program>,
    previous: Option<Arc<Self>>,
}

impl<'program> StructuralJudge<'program> {
    pub(super) fn resolve_at(&self, term: StructuralTerm, depth: usize) -> StructuralTerm {
        self.resolve_with_at(term, depth, ResolutionPurpose::General)
    }

    pub(super) fn resolve_with_at(
        &self,
        term: StructuralTerm,
        depth: usize,
        purpose: ResolutionPurpose,
    ) -> StructuralTerm {
        // Completed general answers cannot replace numeric leaf retention or
        // omit the effects of an active dependency collector.
        if purpose != ResolutionPurpose::General || self.has_resolution_dependency_collector() {
            return self.resolve_uncached(term, depth, purpose);
        }
        let Some(results) = &self.resolution_results else {
            let mut resolver = self.clone();
            resolver.resolution_results = Some(Arc::default());
            return resolver.resolve_at(term, depth);
        };
        if !matches!(
            term,
            StructuralTerm::Application { .. } | StructuralTerm::CallProjection { .. }
        ) {
            return self.resolve_uncached(term, depth, purpose);
        }
        if let Ok(completed) = results.completed.lock()
            && let Some(previous) = completed.iter().find(|previous| {
                previous.depth == depth
                    && match (
                        &previous.context.active_application,
                        &self.active_application,
                    ) {
                        (Some(left), Some(right)) => Arc::ptr_eq(left, right),
                        (None, None) => true,
                        _ => false,
                    }
                    && previous.context.same_resolution_context(self)
                    && previous.input.same_resolution_payload(&term)
            })
        {
            phase_profile::count("05.resolution.hit", 1);
            return previous.output.clone();
        }
        phase_profile::count("05.resolution.miss", 1);
        let output = self.resolve_uncached(term.clone(), depth, purpose);
        let mut context = self.clone();
        context.resolution_results = None;
        if let Ok(mut completed) = results.completed.lock() {
            completed.push(CompletedResolution {
                input: term,
                depth,
                context,
                output: output.clone(),
            });
        }
        output
    }

    pub(super) fn resolving_application(mut self, application: &StructuralTerm) -> Option<Self> {
        let mut context = self.clone();
        // The context holds its semantic owners, not the active query chain.
        // The separate predecessor edge below keeps this an acyclic history.
        context.active_application = None;
        context.resolution_results = None;
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
