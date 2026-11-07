//! Exact resolver inputs without ownership of historical proof storage.
//!
//! Completed queries retain immutable premise/storage identity. Allocation identity is
//! sufficient only while it remains live. Weak handles prevent address reuse
//! without pinning payloads; a changed COW owner or expired handle misses.

use super::{ExpressionHandle, StructuralJudge, SymbolHandle};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, Weak};

// Erasure keeps allocation identity, not a type-dependent value operation.
// Arc's safe coercion preserves its control block and destruction behavior.
trait InputAllocation: Send + Sync {}
impl<T: Send + Sync> InputAllocation for T {}

pub(super) struct InputOwner<'program>(Weak<dyn InputAllocation + 'program>);

impl<'program> InputOwner<'program> {
    fn allocation_address(&self) -> usize {
        self.0.as_ptr().cast::<()>() as usize
    }

    pub(super) fn new<T: Send + Sync + 'program>(value: &Arc<T>) -> Self {
        let erased: Arc<dyn InputAllocation + 'program> = value.clone();
        Self(Arc::downgrade(&erased))
    }

    pub(super) fn same_current<T: Send + Sync + 'program>(&self, value: &Arc<T>) -> bool {
        LiveInputOwner::same_retained_allocation(value, self)
    }
}

impl PartialEq for InputOwner<'_> {
    fn eq(&self, other: &Self) -> bool {
        // An expired Some(owner) is never equal, including to another expired
        // handle. It must not turn into an absent optional observer.
        Weak::ptr_eq(&self.0, &other.0) && self.0.upgrade().is_some()
    }
}

/// The current question already keeps these allocations alive. Borrow them for
/// comparison; only a published completed answer needs retained weak owners.
trait LiveInputOwner<'program> {
    fn allocation_address(&self) -> usize;
    fn retain(&self) -> InputOwner<'program>;
    fn same_retained_allocation(&self, retained: &InputOwner<'program>) -> bool;
}

impl<'program, T: Send + Sync + 'program> LiveInputOwner<'program> for Arc<T> {
    fn allocation_address(&self) -> usize {
        Arc::as_ptr(self).cast::<()>() as usize
    }

    fn retain(&self) -> InputOwner<'program> {
        InputOwner::new(self)
    }

    fn same_retained_allocation(&self, retained: &InputOwner<'program>) -> bool {
        // This address test is only a rejection filter. The retained owner
        // must still be live and identify the same Arc allocation below.
        if !std::ptr::addr_eq(retained.0.as_ptr(), Arc::as_ptr(self)) {
            return false;
        }
        let Some(retained) = retained.0.upgrade() else {
            return false;
        };
        let current: Arc<dyn InputAllocation + 'program> = self.clone();
        Arc::ptr_eq(&retained, &current)
    }
}

const OWNER_COUNT: usize = 19;

#[derive(Clone, Copy, PartialEq)]
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

impl<'program> ReturnedValueInput<'program> {
    fn capture(source: &super::normal_returns::CompletedValueSource<'program>) -> Self {
        Self {
            machine: source.machine,
            state: source.state,
            expression: source.expression,
            type_reference: source.type_reference,
            storage_version: source.storage_version,
            context: InputOwner::new(&source.context),
        }
    }

    fn same_current(&self, source: &super::normal_returns::CompletedValueSource<'program>) -> bool {
        self.machine == source.machine
            && self.state == source.state
            && self.expression == source.expression
            && self.type_reference == source.type_reference
            && self.storage_version == source.storage_version
            && self.context.same_current(&source.context)
    }
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
    owners: [InputOwner<'program>; OWNER_COUNT],
    modes: Modes,
}

/// The same input classification, borrowed for one immutable lookup. This view
/// never enters the completed-answer table or retains a historical payload.
pub(super) struct CurrentResolutionInputs<'current, 'program> {
    program: &'program dyn crate::CheckingInput,
    machine_symbol: SymbolHandle,
    declaration_state: SymbolHandle,
    body_type_parameters: &'program [typed_trees::data::TypeParameter],
    premise_facts: &'current [ExpressionHandle],
    capture_sequence: usize,
    storage_write_sequence: usize,
    storage_version: Option<super::storage_effects::StorageVersion>,
    returned_value_source: Option<&'current super::normal_returns::CompletedValueSource<'program>>,
    owners: [&'current dyn LiveInputOwner<'program>; OWNER_COUNT],
    modes: Modes,
}

impl<'program> ResolutionInputs<'program> {
    pub(super) fn allocation_fingerprint(&self) -> u64 {
        allocation_fingerprint(self.owners.iter().map(InputOwner::allocation_address))
    }

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

    pub(super) fn same_current_inputs(
        &self,
        current: &CurrentResolutionInputs<'_, 'program>,
    ) -> bool {
        std::ptr::eq(self.program, current.program)
            && self.machine_symbol == current.machine_symbol
            && self.declaration_state == current.declaration_state
            && std::ptr::eq(self.body_type_parameters, current.body_type_parameters)
            && self.premise_facts.as_slice() == current.premise_facts
            && self.capture_sequence == current.capture_sequence
            && self.storage_write_sequence == current.storage_write_sequence
            && self.storage_version == current.storage_version
            && match (&self.returned_value_source, current.returned_value_source) {
                (None, None) => true,
                (Some(retained), Some(current)) => retained.same_current(current),
                _ => false,
            }
            && self
                .owners
                .iter()
                .zip(current.owners)
                .all(|(retained, current)| current.same_retained_allocation(retained))
            && self.modes == current.modes
    }
}

impl<'program> CurrentResolutionInputs<'_, 'program> {
    pub(super) fn allocation_fingerprint(&self) -> u64 {
        allocation_fingerprint(self.owners.iter().map(|owner| owner.allocation_address()))
    }

    fn retain(self) -> ResolutionInputs<'program> {
        ResolutionInputs {
            program: self.program,
            machine_symbol: self.machine_symbol,
            declaration_state: self.declaration_state,
            body_type_parameters: self.body_type_parameters,
            premise_facts: self.premise_facts.to_vec(),
            capture_sequence: self.capture_sequence,
            storage_write_sequence: self.storage_write_sequence,
            storage_version: self.storage_version,
            returned_value_source: self.returned_value_source.map(ReturnedValueInput::capture),
            owners: self.owners.map(|owner| owner.retain()),
            modes: self.modes,
        }
    }
}

impl<'program> StructuralJudge<'program> {
    pub(super) fn resolution_inputs(&self) -> ResolutionInputs<'program> {
        self.current_resolution_inputs().retain()
    }

    pub(super) fn current_resolution_inputs(&self) -> CurrentResolutionInputs<'_, 'program> {
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
            body_invocations,
            runtime_induction,
            active_application: _,
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
            // Query state, not a resolution input. Its returned-field trace
            // is compared through `traversal_inputs`.
            query_state: _,
        } = self;
        CurrentResolutionInputs {
            program: *program,
            machine_symbol: *machine_symbol,
            declaration_state: *declaration_state,
            body_type_parameters,
            premise_facts,
            capture_sequence: *capture_sequence,
            storage_write_sequence: *storage_write_sequence,
            storage_version: *storage_version,
            returned_value_source: returned_value_source.as_ref(),
            // Capture and lookup share this exact ordered roster. A new judge
            // field must be classified by the exhaustive destructure above.
            owners: [
                entry_machines,
                declaration_parameters,
                normal_results,
                completed_nominal_values,
                returned_field_owners,
                body_invocations,
                runtime_induction,
                normal_memberships,
                normal_content_guarantees,
                invalidated_storage,
                stored_values,
                normal_guarantees,
                captured_result_equations,
                conditional_guarantees,
                substitutions,
                rewrites,
                disequalities,
                case_facts,
                case_substitutions,
            ],
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

// Equal input owners always share this filter. Hash collisions and expired
// allocations still need the complete input and live-owner comparisons.
fn allocation_fingerprint(owners: impl Iterator<Item = usize>) -> u64 {
    let mut hash = DefaultHasher::new();
    for owner in owners {
        owner.hash(&mut hash);
    }
    hash.finish()
}
