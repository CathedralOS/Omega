//! Exact resolver inputs without ownership of historical proof storage.
//!
//! Completed queries retain immutable premise/storage identity. Allocation identity is
//! sufficient only while it remains live. Weak handles prevent address reuse
//! without pinning payloads; a changed COW owner or expired handle misses.

use super::{ExpressionHandle, StructuralJudge, SymbolHandle};
use std::sync::{Arc, Weak};

// Erasure keeps allocation identity, not a type-dependent value operation.
// Arc's safe coercion preserves its control block and destruction behavior.
trait InputAllocation: Send + Sync {}
impl<T: Send + Sync> InputAllocation for T {}

pub(super) struct InputOwner<'program>(Weak<dyn InputAllocation + 'program>);

impl<'program> InputOwner<'program> {
    pub(super) fn new<T: Send + Sync + 'program>(value: &Arc<T>) -> Self {
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
    completed_nominal_values: InputOwner<'program>,
    returned_field_owners: InputOwner<'program>,
    body_invocations: InputOwner<'program>,
    runtime_induction: InputOwner<'program>,
    normal_memberships: InputOwner<'program>,
    normal_content_guarantees: InputOwner<'program>,
    invalidated_storage: InputOwner<'program>,
    stored_values: InputOwner<'program>,
    normal_guarantees: InputOwner<'program>,
    captured_result_equations: InputOwner<'program>,
    conditional_guarantees: InputOwner<'program>,
    substitutions: InputOwner<'program>,
    rewrites: InputOwner<'program>,
    disequalities: InputOwner<'program>,
    case_facts: InputOwner<'program>,
    case_substitutions: InputOwner<'program>,
}

#[derive(PartialEq)]
struct Modes {
    resolve_applications: bool,
    require_closed_computation: bool,
    runtime_body_values: bool,
    case_facts_only: bool,
    hypotheses_contradictory: bool,
}

/// Reusing a result query requires the same returned value and completion-time
/// storage version. Keep that source in the input comparison, with a weak
/// context reference so old proof storage can be released.
#[derive(PartialEq)]
struct ReturnedValueInput<'program> {
    machine: SymbolHandle,
    state: SymbolHandle,
    expression: ExpressionHandle,
    type_reference: typed_trees::types::TypeReferenceHandle,
    storage_version: Option<super::storage_effects::StorageVersion>,
    context: InputOwner<'program>,
}

pub(super) struct ResolutionInputs<'program> {
    program: &'program dyn crate::CheckingInput,
    machine_symbol: SymbolHandle,
    declaration_state: SymbolHandle,
    body_type_parameters: &'program [typed_trees::data::TypeParameter],
    premise_facts: Vec<ExpressionHandle>,
    capture_sequence: usize,
    storage_write_sequence: usize,
    // Graph validity is read at this version, so it is an input too.
    storage_version: Option<super::storage_effects::StorageVersion>,
    returned_value_source: Option<ReturnedValueInput<'program>>,
    owners: Owners<'program>,
    modes: Modes,
}

impl ResolutionInputs<'_> {
    pub(super) fn same_inputs(&self, other: &Self) -> bool {
        std::ptr::eq(self.program, other.program)
            && self.machine_symbol == other.machine_symbol
            && self.declaration_state == other.declaration_state
            && std::ptr::eq(self.body_type_parameters, other.body_type_parameters)
            && self.premise_facts == other.premise_facts
            && self.capture_sequence == other.capture_sequence
            && self.storage_write_sequence == other.storage_write_sequence
            && self.storage_version == other.storage_version
            && self.returned_value_source == other.returned_value_source
            && self.owners == other.owners
            && self.modes == other.modes
    }
}

impl<'program> StructuralJudge<'program> {
    pub(super) fn resolution_inputs(&self) -> ResolutionInputs<'program> {
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
            returned_value_source,
            completed_nominal_values,
            returned_field_owners,
            returned_field_trace: _,
            body_invocations,
            runtime_induction,
            active_application: _,
            completed_resolutions: _,
            resolution_observations: _,
            normal_memberships,
            normal_content_guarantees,
            invalidated_storage,
            storage_version,
            stored_values,
            capture_sequence,
            storage_write_sequence,
            normal_guarantees,
            captured_result_equations,
            conditional_guarantees,
            resolve_applications,
            require_closed_computation,
            runtime_body_values,
            specification_terms: _,
            case_facts_only,
            substitutions,
            rewrites,
            disequalities,
            case_facts,
            case_substitutions,
            hypotheses_contradictory,
            ring_licenses: _,
            semiring_licenses: _,
            frame_resolver: _,
            prepared_integer_premises_cache: _,
        } = self;
        ResolutionInputs {
            program: *program,
            machine_symbol: *machine_symbol,
            declaration_state: *declaration_state,
            body_type_parameters,
            premise_facts: premise_facts.clone(),
            capture_sequence: *capture_sequence,
            storage_write_sequence: *storage_write_sequence,
            storage_version: *storage_version,
            returned_value_source: returned_value_source.as_ref().map(|source| {
                ReturnedValueInput {
                    machine: source.machine,
                    state: source.state,
                    expression: source.expression,
                    type_reference: source.type_reference,
                    storage_version: source.storage_version,
                    context: InputOwner::new(&source.context),
                }
            }),
            owners: Owners {
                entry_machines: InputOwner::new(entry_machines),
                declaration_parameters: InputOwner::new(declaration_parameters),
                normal_results: InputOwner::new(normal_results),
                completed_nominal_values: InputOwner::new(completed_nominal_values),
                returned_field_owners: InputOwner::new(returned_field_owners),
                body_invocations: InputOwner::new(body_invocations),
                runtime_induction: InputOwner::new(runtime_induction),
                normal_memberships: InputOwner::new(normal_memberships),
                normal_content_guarantees: InputOwner::new(normal_content_guarantees),
                invalidated_storage: InputOwner::new(invalidated_storage),
                stored_values: InputOwner::new(stored_values),
                normal_guarantees: InputOwner::new(normal_guarantees),
                captured_result_equations: InputOwner::new(captured_result_equations),
                conditional_guarantees: InputOwner::new(conditional_guarantees),
                substitutions: InputOwner::new(substitutions),
                rewrites: InputOwner::new(rewrites),
                disequalities: InputOwner::new(disequalities),
                case_facts: InputOwner::new(case_facts),
                case_substitutions: InputOwner::new(case_substitutions),
            },
            modes: Modes {
                resolve_applications: *resolve_applications,
                require_closed_computation: *require_closed_computation,
                runtime_body_values: *runtime_body_values,
                case_facts_only: *case_facts_only,
                hypotheses_contradictory: *hypotheses_contradictory,
            },
        }
    }
}
