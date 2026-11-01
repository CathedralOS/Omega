//! Exact traversal inputs without ownership of historical proof storage.
//!
//! A traversal names immutable resolver premises, storage epochs and dynamic
//! observers. Allocation identity is
//! sufficient only while it remains live. Weak handles prevent address reuse
//! without pinning payloads; a changed COW owner or expired handle misses.

use super::{ExpressionHandle, StructuralJudge, SymbolHandle};
use std::sync::{Arc, Weak};

// Erasure keeps allocation identity, not a type-dependent value operation.
// Arc's safe coercion preserves its control block and destruction behavior.
trait InputAllocation: Send + Sync {}
impl<T: Send + Sync> InputAllocation for T {}

struct InputOwner<'program>(Weak<dyn InputAllocation + 'program>);

impl<'program> InputOwner<'program> {
    fn new<T: Send + Sync + 'program>(value: &Arc<T>) -> Self {
        let erased: Arc<dyn InputAllocation + 'program> = value.clone();
        Self(Arc::downgrade(&erased))
    }
}

impl PartialEq for InputOwner<'_> {
    fn eq(&self, other: &Self) -> bool {
        // An expired Some(owner) is never equal, including to another expired
        // handle. It must not turn into an absent optional observer.
        Weak::ptr_eq(&self.0, &other.0) && self.0.upgrade().is_some()
    }
}

#[derive(PartialEq)]
struct Owners<'program> {
    entry_machines: InputOwner<'program>,
    declaration_parameters: InputOwner<'program>,
    normal_results: InputOwner<'program>,
    completed_nominal_captures: InputOwner<'program>,
    returned_field_owners: InputOwner<'program>,
    body_invocations: InputOwner<'program>,
    normal_memberships: InputOwner<'program>,
    normal_content_guarantees: InputOwner<'program>,
    invalidated_storage: InputOwner<'program>,
    storage_exposures: InputOwner<'program>,
    stored_values: InputOwner<'program>,
    normal_guarantees: InputOwner<'program>,
    captured_result_equations: InputOwner<'program>,
    substitutions: InputOwner<'program>,
    rewrites: InputOwner<'program>,
    disequalities: InputOwner<'program>,
    case_facts: InputOwner<'program>,
    case_substitutions: InputOwner<'program>,
    returned_field_trace: Option<InputOwner<'program>>,
    active_application: Option<InputOwner<'program>>,
    resolution_observations: Option<InputOwner<'program>>,
}

#[derive(PartialEq)]
struct Modes {
    resolve_applications: bool,
    require_closed_computation: bool,
    runtime_body_values: bool,
    specification_terms: bool,
    case_facts_only: bool,
    hypotheses_contradictory: bool,
}

pub(in crate::proof_contracts::contract_entailment) struct StructuralTraversalInputs<'program> {
    program: &'program dyn crate::CheckingInput,
    machine_symbol: SymbolHandle,
    declaration_state: SymbolHandle,
    body_type_parameters: &'program [typed_trees::data::TypeParameter],
    premise_facts: Vec<ExpressionHandle>,
    capture_sequence: usize,
    storage_write_sequence: usize,
    owners: Owners<'program>,
    modes: Modes,
    ring_licenses: Vec<SymbolHandle>,
    semiring_licenses: Vec<(SymbolHandle, SymbolHandle)>,
}

impl StructuralTraversalInputs<'_> {
    pub(in crate::proof_contracts::contract_entailment) fn same_inputs(
        &self,
        other: &Self,
    ) -> bool {
        std::ptr::eq(self.program, other.program)
            && self.machine_symbol == other.machine_symbol
            && self.declaration_state == other.declaration_state
            && std::ptr::eq(self.body_type_parameters, other.body_type_parameters)
            && self.premise_facts == other.premise_facts
            && self.capture_sequence == other.capture_sequence
            && self.storage_write_sequence == other.storage_write_sequence
            && self.owners == other.owners
            && self.modes == other.modes
            && self.ring_licenses == other.ring_licenses
            && self.semiring_licenses == other.semiring_licenses
    }
}

impl<'program> StructuralJudge<'program> {
    pub(in crate::proof_contracts::contract_entailment) fn traversal_inputs(
        &self,
    ) -> StructuralTraversalInputs<'program> {
        // Exhaustive on purpose: a new judge field must be classified here.
        // Program-pure memo cells supply no premises and are not retained.
        let Self {
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
            returned_field_trace,
            body_invocations,
            active_application,
            completed_resolutions: _,
            resolution_observations,
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
            prepared_integer_premises_cache: _,
        } = self;
        StructuralTraversalInputs {
            program: *program,
            machine_symbol: *machine_symbol,
            declaration_state: *declaration_state,
            body_type_parameters,
            premise_facts: premise_facts.clone(),
            capture_sequence: *capture_sequence,
            storage_write_sequence: *storage_write_sequence,
            owners: Owners {
                entry_machines: InputOwner::new(entry_machines),
                declaration_parameters: InputOwner::new(declaration_parameters),
                normal_results: InputOwner::new(normal_results),
                completed_nominal_captures: InputOwner::new(completed_nominal_captures),
                returned_field_owners: InputOwner::new(returned_field_owners),
                body_invocations: InputOwner::new(body_invocations),
                normal_memberships: InputOwner::new(normal_memberships),
                normal_content_guarantees: InputOwner::new(normal_content_guarantees),
                invalidated_storage: InputOwner::new(invalidated_storage),
                storage_exposures: InputOwner::new(storage_exposures),
                stored_values: InputOwner::new(stored_values),
                normal_guarantees: InputOwner::new(normal_guarantees),
                captured_result_equations: InputOwner::new(captured_result_equations),
                substitutions: InputOwner::new(substitutions),
                rewrites: InputOwner::new(rewrites),
                disequalities: InputOwner::new(disequalities),
                case_facts: InputOwner::new(case_facts),
                case_substitutions: InputOwner::new(case_substitutions),
                returned_field_trace: returned_field_trace.as_ref().map(InputOwner::new),
                active_application: active_application.as_ref().map(InputOwner::new),
                resolution_observations: resolution_observations.as_ref().map(InputOwner::new),
            },
            modes: Modes {
                resolve_applications: *resolve_applications,
                require_closed_computation: *require_closed_computation,
                runtime_body_values: *runtime_body_values,
                specification_terms: *specification_terms,
                case_facts_only: *case_facts_only,
                hypotheses_contradictory: *hypotheses_contradictory,
            },
            ring_licenses: ring_licenses
                .iter()
                .map(|license| license.add_machine)
                .collect(),
            semiring_licenses: semiring_licenses
                .iter()
                .map(|license| (license.add_machine, license.mul_machine))
                .collect(),
        }
    }
}
