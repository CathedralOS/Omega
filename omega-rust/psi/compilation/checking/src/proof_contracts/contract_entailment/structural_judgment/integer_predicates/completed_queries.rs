//! One last completed goal-first integer question in a cloned judge tree.
//!
//! Exact retained input owners identify one immutable premise/storage epoch.
//! This is deliberately stricter than semantic term equality or the existing
//! goal-free preparation key. No pending question supplies an answer.

use super::super::super::{ExpressionHandle, SymbolHandle};
use super::super::resolution_queries::ApplicationResolution;
use super::{FormedIntegerPredicate, StructuralJudge, StructuralTerm};
use std::sync::{Arc, Weak};

pub(super) struct CompletedIntegerQuery<'program> {
    context: IntegerQueryContext<'program>,
    predicate: StructuralTerm,
    answer: Option<Arc<FormedIntegerPredicate<'program>>>,
}

impl<'program> CompletedIntegerQuery<'program> {
    pub(super) fn new(
        context: IntegerQueryContext<'program>,
        predicate: StructuralTerm,
        answer: Option<Arc<FormedIntegerPredicate<'program>>>,
    ) -> Self {
        Self {
            context,
            predicate,
            answer,
        }
    }

    /// Outer Some means an exact completed question, including unavailable.
    pub(super) fn lookup(
        &self,
        context: &IntegerQueryContext<'program>,
        predicate: &StructuralTerm,
    ) -> Option<Option<Arc<FormedIntegerPredicate<'program>>>> {
        (self.context.matches(context) && self.predicate.same_resolution_payload(predicate))
            .then(|| self.answer.clone())
    }
}

pub(super) struct IntegerQueryContext<'program> {
    program: &'program dyn crate::CheckingInput,
    body_type_parameters: &'program [typed_trees::data::TypeParameter],
    premise_facts: Vec<ExpressionHandle>,
    // A strong frame would cycle through frame.context's shared query cache.
    active_application: Option<Weak<ApplicationResolution<'program>>>,
    ring_licenses: Vec<SymbolHandle>,
    semiring_licenses: Vec<(SymbolHandle, SymbolHandle)>,
    entry_machines: Arc<super::super::EntryMachines>,
    declaration_parameters: Arc<Vec<typed_trees::signature::StateParameter>>,
    normal_results: Arc<Vec<(ExpressionHandle, StructuralTerm)>>,
    completed_nominal_captures: Arc<Vec<super::super::CompletedNominalCapture>>,
    returned_field_owners: Arc<Vec<SymbolHandle>>,
    body_invocations: Arc<Vec<SymbolHandle>>,
    normal_memberships: Arc<Vec<super::super::MembershipGuarantee>>,
    normal_content_guarantees:
        Arc<Vec<super::super::super::content_conservation::ObservedContentEquation>>,
    invalidated_storage: Arc<Vec<SymbolHandle>>,
    storage_exposures: Arc<super::super::storage_effects::StorageExposureRecord>,
    stored_values: Arc<Vec<super::super::storage_effects::CapturedStorageValue>>,
    normal_guarantees: Arc<Vec<(StructuralTerm, StructuralTerm)>>,
    captured_result_equations: Arc<Vec<(StructuralTerm, StructuralTerm)>>,
    substitutions: Arc<Vec<(String, StructuralTerm)>>,
    rewrites: Arc<super::super::rewrite_table::RewriteTable>,
    disequalities: Arc<Vec<(StructuralTerm, StructuralTerm)>>,
    case_facts: Arc<Vec<(StructuralTerm, SymbolHandle, SymbolHandle)>>,
    case_substitutions: Arc<Vec<(String, StructuralTerm)>>,
    machine_symbol: SymbolHandle,
    declaration_state: SymbolHandle,
    capture_sequence: usize,
    storage_write_sequence: usize,
    resolve_applications: bool,
    require_closed_computation: bool,
    runtime_body_values: bool,
    specification_terms: bool,
    case_facts_only: bool,
    hypotheses_contradictory: bool,
}

impl<'program> IntegerQueryContext<'program> {
    pub(super) fn capture(judge: &StructuralJudge<'program>) -> Option<Self> {
        if judge.has_resolution_dependency_collector() {
            return None;
        }
        // Exhaustive: a new judge input requires a key/eligibility decision.
        let StructuralJudge {
            program,
            entry_machines,
            machine_symbol,
            declaration_state,
            declaration_parameters,
            body_type_parameters,
            premise_facts,
            normal_results,
            completed_nominal_captures,
            returned_field_owners,
            returned_field_trace: _,
            body_invocations,
            active_application,
            normal_memberships,
            normal_content_guarantees,
            invalidated_storage,
            storage_exposures,
            stored_values,
            capture_sequence,
            storage_write_sequence,
            normal_guarantees,
            captured_result_equations,
            resolve_applications,
            require_closed_computation,
            runtime_body_values,
            specification_terms,
            case_facts_only,
            substitutions,
            rewrites,
            disequalities,
            case_facts,
            case_substitutions,
            hypotheses_contradictory,
            ring_licenses,
            semiring_licenses,
            frame_resolver: _,
            integer_query_cache: _,
        } = judge;
        Some(Self {
            program: *program,
            body_type_parameters,
            premise_facts: premise_facts.clone(),
            active_application: active_application.as_ref().map(Arc::downgrade),
            ring_licenses: ring_licenses
                .iter()
                .map(|license| license.add_machine)
                .collect(),
            semiring_licenses: semiring_licenses
                .iter()
                .map(|license| (license.add_machine, license.mul_machine))
                .collect(),
            entry_machines: Arc::clone(entry_machines),
            declaration_parameters: Arc::clone(declaration_parameters),
            normal_results: Arc::clone(normal_results),
            completed_nominal_captures: Arc::clone(completed_nominal_captures),
            returned_field_owners: Arc::clone(returned_field_owners),
            body_invocations: Arc::clone(body_invocations),
            normal_memberships: Arc::clone(normal_memberships),
            normal_content_guarantees: Arc::clone(normal_content_guarantees),
            invalidated_storage: Arc::clone(invalidated_storage),
            storage_exposures: Arc::clone(storage_exposures),
            stored_values: Arc::clone(stored_values),
            normal_guarantees: Arc::clone(normal_guarantees),
            captured_result_equations: Arc::clone(captured_result_equations),
            substitutions: Arc::clone(substitutions),
            rewrites: Arc::clone(rewrites),
            disequalities: Arc::clone(disequalities),
            case_facts: Arc::clone(case_facts),
            case_substitutions: Arc::clone(case_substitutions),
            machine_symbol: *machine_symbol,
            declaration_state: *declaration_state,
            capture_sequence: *capture_sequence,
            storage_write_sequence: *storage_write_sequence,
            resolve_applications: *resolve_applications,
            require_closed_computation: *require_closed_computation,
            runtime_body_values: *runtime_body_values,
            specification_terms: *specification_terms,
            case_facts_only: *case_facts_only,
            hypotheses_contradictory: *hypotheses_contradictory,
        })
    }

    fn matches(&self, other: &Self) -> bool {
        std::ptr::eq(self.program, other.program)
            && std::ptr::eq(self.body_type_parameters, other.body_type_parameters)
            && self.premise_facts == other.premise_facts
            && self.ring_licenses == other.ring_licenses
            && self.semiring_licenses == other.semiring_licenses
            && match (&self.active_application, &other.active_application) {
                (None, None) => true,
                (Some(left), Some(right)) => match (left.upgrade(), right.upgrade()) {
                    (Some(left), Some(right)) => Arc::ptr_eq(&left, &right),
                    _ => false,
                },
                _ => false,
            }
            && Arc::ptr_eq(&self.entry_machines, &other.entry_machines)
            && Arc::ptr_eq(&self.declaration_parameters, &other.declaration_parameters)
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
            && Arc::ptr_eq(&self.normal_guarantees, &other.normal_guarantees)
            && Arc::ptr_eq(
                &self.captured_result_equations,
                &other.captured_result_equations,
            )
            && Arc::ptr_eq(&self.substitutions, &other.substitutions)
            && Arc::ptr_eq(&self.rewrites, &other.rewrites)
            && Arc::ptr_eq(&self.disequalities, &other.disequalities)
            && Arc::ptr_eq(&self.case_facts, &other.case_facts)
            && Arc::ptr_eq(&self.case_substitutions, &other.case_substitutions)
            && self.machine_symbol == other.machine_symbol
            && self.declaration_state == other.declaration_state
            && self.capture_sequence == other.capture_sequence
            && self.storage_write_sequence == other.storage_write_sequence
            && self.resolve_applications == other.resolve_applications
            && self.require_closed_computation == other.require_closed_computation
            && self.runtime_body_values == other.runtime_body_values
            && self.specification_terms == other.specification_terms
            && self.case_facts_only == other.case_facts_only
            && self.hypotheses_contradictory == other.hypotheses_contradictory
    }
}
