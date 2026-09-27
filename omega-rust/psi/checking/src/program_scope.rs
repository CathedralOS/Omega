//! Checking-wide memoization lifetime and its domain-owned query caches.
//!
//! `enter_program_plan_scope` opens the memo slots for one checked
//! build; dropping its guard restores every enclosing slot. Cache operations
//! live beneath this owner: effect plans, declaration indexes, value properties
//! and call targets. Consumers reach their domain directly.
//!
//! Slots use program-pointer keys and compute uncached outside a matching scope. The
//! separate `frozen_program` scope additionally checks a retained program identity;
//! declaration queries use its index first when that exact frozen scope exists.

pub(crate) mod call_targets;
pub(crate) mod data_definitions;
pub(crate) mod effects;
pub(crate) mod machines;
pub(crate) mod value_properties;

use data_definitions::DataDefinitionLookup;
use std::cell::RefCell;
use std::collections::HashMap;

use flow_effects::{OperationalPlan, ServiceReachInferencePlan};
use typed_trees::types::TypeReferenceHandle;

use crate::value_custody::claim_frontier::ClaimFrontierClaim;

type OperationalSlot = Option<Option<(*const typed_trees::TypedTrees, OperationalPlan)>>;
type ServiceReachSlot = Option<Option<(*const typed_trees::TypedTrees, ServiceReachInferencePlan)>>;
type ClaimFrontierSlot = Option<
    Option<(
        *const typed_trees::TypedTrees,
        HashMap<TypeReferenceHandle, Vec<ClaimFrontierClaim>>,
    )>,
>;
type DataDefinitionLookupSlot = Option<
    Option<(
        *const typed_trees::TypedTrees,
        HashMap<symbols::SymbolHandle, DataDefinitionLookup>,
    )>,
>;
type DropHookSlot = Option<
    Option<(
        *const typed_trees::TypedTrees,
        HashMap<symbols::SymbolHandle, bool>,
    )>,
>;
/// Whether a data definition requires establishment, keyed by the
/// definition's address: the scope borrows the program, so each definition
/// keeps one address and no two share it.
type EstablishmentSlot = Option<Option<(*const typed_trees::TypedTrees, HashMap<usize, bool>)>>;
/// (trait symbol, bound requirement name) -> conforming (carrier, entry
/// symbol) pairs, built once per scoped program.
type ConformanceSlotCarriersSlot = Option<
    Option<(
        *const typed_trees::TypedTrees,
        HashMap<(symbols::SymbolHandle, String), Vec<(TypeReferenceHandle, symbols::SymbolHandle)>>,
    )>,
>;
/// Whole-program symbol -> data-definition position map, shared by `Rc` —
/// the claim-frontier walk's per-call table builds otherwise rescan the
/// declaration slice once per missed type reference.
type DataDefinitionPositionsSlot = Option<
    Option<(
        *const typed_trees::TypedTrees,
        std::rc::Rc<HashMap<symbols::SymbolHandle, usize>>,
    )>,
>;
type TypeParameterMultiplicitySlot = Option<
    Option<(
        *const typed_trees::TypedTrees,
        std::rc::Rc<
            HashMap<
                symbols::SymbolHandle,
                (
                    language_semantics::Multiplicity,
                    typed_trees::data::TypeParameterKind,
                ),
            >,
        >,
    )>,
>;
/// (target state symbol, argument count) -> resolved call-argument
/// destination types; each query rescans every machine and state to verify
/// symbol uniqueness, so the whole verdict is memoized per scoped program.
type CallArgumentDestinationsSlot = Option<
    Option<(
        *const typed_trees::TypedTrees,
        HashMap<(symbols::SymbolHandle, usize), Option<Vec<TypeReferenceHandle>>>,
    )>,
>;

thread_local! {
    /// Outer `None`: no scope is open — calls compute without memoizing.
    /// `Some(None)`: scope open, not yet computed. `Some(Some(_))`: the plan
    /// for the program currently being checked.
    static OPERATIONAL_PLAN_SLOT: RefCell<OperationalSlot> = const { RefCell::new(None) };
    static SERVICE_REACH_PLAN_SLOT: RefCell<ServiceReachSlot> = const { RefCell::new(None) };
    /// The claim frontier memoizes per queried type reference rather than a
    /// single plan; the map itself is the stored plan for the scoped program.
    static CLAIM_FRONTIER_SLOT: RefCell<ClaimFrontierSlot> = const { RefCell::new(None) };
    /// Data-definition lookups memoize a symbol's declaration answer
    /// (including negative and duplicate answers) rather than one plan.
    static DATA_DEF_LOOKUP_SLOT: RefCell<DataDefinitionLookupSlot> =
        const { RefCell::new(None) };
    /// Whether a machine attached to a data symbol realizes `::drop`,
    /// memoized per (program, symbol).
    static DROP_HOOK_SLOT: RefCell<DropHookSlot> = const { RefCell::new(None) };
    /// Whether a data definition requires establishment, memoized per
    /// (program, definition).
    static ESTABLISHMENT_SLOT: RefCell<EstablishmentSlot> = const { RefCell::new(None) };
    /// Conformance slot carriers memoize the whole index at once — the ring
    /// and semiring license builders both read it per judged machine.
    static CONFORMANCE_SLOT_CARRIERS_SLOT: RefCell<ConformanceSlotCarriersSlot> =
        const { RefCell::new(None) };
    /// The data-definition position table, shared once per scoped program.
    static DATA_DEF_POSITIONS_SLOT: RefCell<DataDefinitionPositionsSlot> =
        const { RefCell::new(None) };
    /// The type-parameter multiplicity table, shared once per scoped program.
    static TYPE_PARAMETER_MULTIPLICITY_SLOT: RefCell<TypeParameterMultiplicitySlot> =
        const { RefCell::new(None) };
    /// Call-argument destination verdicts memoize per (target, argument
    /// count) inside the scope.
    static CALL_ARGUMENT_DESTINATIONS_SLOT: RefCell<CallArgumentDestinationsSlot> =
        const { RefCell::new(None) };
}

/// Restores the slots a scope opened on top of when it drops, so nested
/// program builds cannot leak one program's plans into its caller's.
pub struct ProgramPlanScopeGuard {
    operational: OperationalSlot,
    service_reach: ServiceReachSlot,
    claim_frontiers: ClaimFrontierSlot,
    data_def_lookups: DataDefinitionLookupSlot,
    drop_hooks: DropHookSlot,
    establishment: EstablishmentSlot,
    conformance_slot_carriers: ConformanceSlotCarriersSlot,
    data_def_positions: DataDefinitionPositionsSlot,
    type_parameter_multiplicities: TypeParameterMultiplicitySlot,
    call_argument_destinations: CallArgumentDestinationsSlot,
}

impl Drop for ProgramPlanScopeGuard {
    fn drop(&mut self) {
        OPERATIONAL_PLAN_SLOT.with(|cell| {
            *cell.borrow_mut() = self.operational.take();
        });
        SERVICE_REACH_PLAN_SLOT.with(|cell| {
            *cell.borrow_mut() = self.service_reach.take();
        });
        CLAIM_FRONTIER_SLOT.with(|cell| {
            *cell.borrow_mut() = self.claim_frontiers.take();
        });
        DATA_DEF_LOOKUP_SLOT.with(|cell| {
            *cell.borrow_mut() = self.data_def_lookups.take();
        });
        DROP_HOOK_SLOT.with(|cell| {
            *cell.borrow_mut() = self.drop_hooks.take();
        });
        ESTABLISHMENT_SLOT.with(|cell| {
            *cell.borrow_mut() = self.establishment.take();
        });
        CONFORMANCE_SLOT_CARRIERS_SLOT.with(|cell| {
            *cell.borrow_mut() = self.conformance_slot_carriers.take();
        });
        DATA_DEF_POSITIONS_SLOT.with(|cell| {
            *cell.borrow_mut() = self.data_def_positions.take();
        });
        TYPE_PARAMETER_MULTIPLICITY_SLOT.with(|cell| {
            *cell.borrow_mut() = self.type_parameter_multiplicities.take();
        });
        CALL_ARGUMENT_DESTINATIONS_SLOT.with(|cell| {
            *cell.borrow_mut() = self.call_argument_destinations.take();
        });
    }
}

/// Open a memoization scope for one program build. Call once at the checked
/// lowering's entry; the returned guard must stay alive for the whole build.
pub fn enter_program_plan_scope() -> ProgramPlanScopeGuard {
    ProgramPlanScopeGuard {
        operational: OPERATIONAL_PLAN_SLOT.with(|cell| cell.borrow_mut().replace(None)),
        service_reach: SERVICE_REACH_PLAN_SLOT.with(|cell| cell.borrow_mut().replace(None)),
        claim_frontiers: CLAIM_FRONTIER_SLOT.with(|cell| cell.borrow_mut().replace(None)),
        data_def_lookups: DATA_DEF_LOOKUP_SLOT.with(|cell| cell.borrow_mut().replace(None)),
        drop_hooks: DROP_HOOK_SLOT.with(|cell| cell.borrow_mut().replace(None)),
        establishment: ESTABLISHMENT_SLOT.with(|cell| cell.borrow_mut().replace(None)),
        conformance_slot_carriers: CONFORMANCE_SLOT_CARRIERS_SLOT
            .with(|cell| cell.borrow_mut().replace(None)),
        data_def_positions: DATA_DEF_POSITIONS_SLOT.with(|cell| cell.borrow_mut().replace(None)),
        type_parameter_multiplicities: TYPE_PARAMETER_MULTIPLICITY_SLOT
            .with(|cell| cell.borrow_mut().replace(None)),
        call_argument_destinations: CALL_ARGUMENT_DESTINATIONS_SLOT
            .with(|cell| cell.borrow_mut().replace(None)),
    }
}
