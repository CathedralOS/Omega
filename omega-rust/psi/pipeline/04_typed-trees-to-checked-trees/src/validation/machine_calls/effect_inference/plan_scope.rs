//! Build-scoped whole-plan memos for the two program-pure inference plans.
//!
//! `infer_operational_may` and `infer_service_reaches` are called at ~16 sites
//! across the checked lowering — conformance, contract proofs, machine
//! parameters, monomorphization — and each call rebuilds the same
//! machines×states sweep. Both plans are pure functions of the program, but a
//! thread_local cache keyed by program identity cannot prove freshness (an
//! allocator can recycle a same-shaped successor program at the same address).
//! Instead the plans are memoized inside a scope opened once per checked
//! build: `lower_typed_trees` calls [`enter_program_plan_scope`] at entry and
//! the guard restores the previous slots on every exit path, so a value can
//! never outlive the program that produced it. Each slot also keys on the
//! program pointer: the checked program is borrowed for the entire scope so
//! its address cannot be recycled while the scope is open, and a nested build
//! over a different program can never observe the outer program's plans.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::flow_effects::{OperationalPlan, ServiceReachInferencePlan};
use symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceHandle;

use crate::validation::value_custody::claim_frontier::ClaimFrontierClaim;

type OperationalSlot = Option<
    Option<(
        *const symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
        OperationalPlan,
    )>,
>;
type ServiceReachSlot = Option<
    Option<(
        *const symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
        ServiceReachInferencePlan,
    )>,
>;
type ClaimFrontierSlot = Option<
    Option<(
        *const symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
        HashMap<TypeReferenceHandle, Vec<ClaimFrontierClaim>>,
    )>,
>;
/// The declaration-table answer for a symbol: absent, exactly one
/// definition at a position, or a duplicate (first position retained —
/// `find`-style consumers still read it).
#[derive(Clone, Copy)]
pub(crate) enum DataDefinitionLookup {
    Missing,
    Unique(u32),
    Duplicate(u32),
}

impl DataDefinitionLookup {
    pub(crate) fn first_position(self) -> Option<u32> {
        match self {
            Self::Unique(position) | Self::Duplicate(position) => Some(position),
            Self::Missing => None,
        }
    }
}

type DataDefinitionLookupSlot = Option<
    Option<(
        *const symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
        HashMap<symbols::SymbolHandle, DataDefinitionLookup>,
    )>,
>;
type DropHookSlot = Option<
    Option<(
        *const symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
        HashMap<symbols::SymbolHandle, bool>,
    )>,
>;
/// Whether a data definition requires establishment, keyed by the
/// definition's address: the scope borrows the program, so each definition
/// keeps one address and no two share it.
type EstablishmentSlot = Option<
    Option<(
        *const symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
        HashMap<usize, bool>,
    )>,
>;
/// (trait symbol, bound requirement name) -> conforming (carrier, entry
/// symbol) pairs, built once per scoped program.
type ConformanceSlotCarriersSlot = Option<
    Option<(
        *const symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
        HashMap<(symbols::SymbolHandle, String), Vec<(TypeReferenceHandle, symbols::SymbolHandle)>>,
    )>,
>;
/// Whole-program symbol -> data-definition position map, shared by `Rc` —
/// the claim-frontier walk's per-call table builds otherwise rescan the
/// declaration slice once per missed type reference.
type DataDefinitionPositionsSlot = Option<
    Option<(
        *const symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
        std::rc::Rc<HashMap<symbols::SymbolHandle, usize>>,
    )>,
>;
type TypeParameterMultiplicitySlot = Option<
    Option<(
        *const symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
        std::rc::Rc<
            HashMap<
                symbols::SymbolHandle,
                (
                    language_semantics::Multiplicity,
                    symbol_resolved_trees_to_typed_trees::typed_trees::data::TypeParameterKind,
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
        *const symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
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

pub fn memoized_operational_plan(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
) -> OperationalPlan {
    if let Some(plan) = OPERATIONAL_PLAN_SLOT.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|slot| slot.as_ref())
            .filter(|(owner, _)| std::ptr::eq(*owner, program))
            .map(|(_, plan)| plan.clone())
    }) {
        return plan;
    }
    let plan = crate::validation::machine_calls::effect_inference::operational::infer_operational_may_uncached(
        program,
    );
    OPERATIONAL_PLAN_SLOT.with(|cell| {
        if let Ok(mut slot) = cell.try_borrow_mut()
            && let Some(scope) = &mut *slot
        {
            *scope = Some((program, plan.clone()));
        }
    });
    plan
}

/// The exact linear claim frontier of a type is pure in the program, and
/// each caller's walk rebuilds the declaration/parameter indexes before
/// recursing; inside a scope, each queried type reference walks once.
pub(crate) fn memoized_claim_frontier(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Vec<ClaimFrontierClaim> {
    enum SlotState {
        NoScope,
        Hit(Vec<ClaimFrontierClaim>),
        Miss,
        ForeignProgram,
    }
    let state = CLAIM_FRONTIER_SLOT.with(|cell| {
        let cell = cell.borrow();
        match cell.as_ref() {
            None => SlotState::NoScope,
            Some(None) => SlotState::Miss,
            Some(Some((owner, map))) => {
                if std::ptr::eq(*owner, program) {
                    map.get(&type_reference)
                        .cloned()
                        .map_or(SlotState::Miss, SlotState::Hit)
                } else {
                    SlotState::ForeignProgram
                }
            }
        }
    });
    if let SlotState::Hit(claims) = state {
        return claims;
    }
    let claims = crate::validation::value_custody::claim_frontier::linear_claim_frontier_uncached(
        program,
        type_reference,
    );
    if matches!(state, SlotState::Miss) {
        CLAIM_FRONTIER_SLOT.with(|cell| {
            if let Ok(mut slot) = cell.try_borrow_mut()
                && let Some(scope) = &mut *slot
            {
                match scope {
                    Some((owner, map)) if std::ptr::eq(*owner, program) => {
                        map.insert(type_reference, claims.clone());
                    }
                    slot_none @ None => {
                        *slot_none =
                            Some((program, HashMap::from([(type_reference, claims.clone())])));
                    }
                    Some(_) => {}
                }
            }
        });
    }
    claims
}

/// The data-definition table's answer for a symbol, memoized per
/// (program, symbol) inside the scope — the `.find`/`.filter` over the
/// declaration slice otherwise re-scans the whole table at every
/// classification site, and the uniqueness answer some callers require is
/// recorded with the same walk.
pub(crate) fn memoized_data_definition_lookup(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    symbol: symbols::SymbolHandle,
) -> DataDefinitionLookup {
    enum SlotState {
        NoScope,
        Hit(DataDefinitionLookup),
        Miss,
        ForeignProgram,
    }
    let state = DATA_DEF_LOOKUP_SLOT.with(|cell| {
        let cell = cell.borrow();
        match cell.as_ref() {
            None => SlotState::NoScope,
            Some(None) => SlotState::Miss,
            Some(Some((owner, map))) => {
                if std::ptr::eq(*owner, program) {
                    map.get(&symbol)
                        .copied()
                        .map_or(SlotState::Miss, SlotState::Hit)
                } else {
                    SlotState::ForeignProgram
                }
            }
        }
    });
    if let SlotState::Hit(lookup) = state {
        return lookup;
    }
    let mut matches = program
        .data_definitions()
        .iter()
        .enumerate()
        .filter(|(_, definition)| definition.symbol == symbol)
        .map(|(index, _)| u32::try_from(index).expect("data definition position overflow"));
    let lookup = match (matches.next(), matches.next()) {
        (None, _) => DataDefinitionLookup::Missing,
        (Some(first), None) => DataDefinitionLookup::Unique(first),
        (Some(first), Some(_)) => DataDefinitionLookup::Duplicate(first),
    };
    if matches!(state, SlotState::Miss) {
        DATA_DEF_LOOKUP_SLOT.with(|cell| {
            if let Ok(mut slot) = cell.try_borrow_mut()
                && let Some(scope) = &mut *slot
            {
                match scope {
                    Some((owner, map)) if std::ptr::eq(*owner, program) => {
                        map.insert(symbol, lookup);
                    }
                    slot_none @ None => {
                        *slot_none = Some((program, HashMap::from([(symbol, lookup)])));
                    }
                    Some(_) => {}
                }
            }
        });
    }
    lookup
}

/// A data definition by symbol, resolved through the build-scope memo when
/// one is open — the declaration slice is otherwise re-scanned per query.
/// `find`-style callers read the first match; callers that require uniqueness
/// should use [`memoized_data_definition_lookup`] directly.
pub(crate) fn data_definition_by_symbol(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    symbol: symbols::SymbolHandle,
) -> Option<&symbol_resolved_trees_to_typed_trees::typed_trees::data::DataDefinition> {
    memoized_data_definition_lookup(program, symbol)
        .first_position()
        .and_then(|position| program.data_definitions().get(position as usize))
}

/// Whether a machine attached to `symbol` realizes `::drop`, memoized per
/// (program, symbol): storage-content classification asks it at every data
/// node and the scan is otherwise O(machines) per node.
pub(crate) fn memoized_owns_drop_hook(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    symbol: symbols::SymbolHandle,
) -> bool {
    enum SlotState {
        NoScope,
        Hit(bool),
        Miss,
        ForeignProgram,
    }
    let state = DROP_HOOK_SLOT.with(|cell| {
        let cell = cell.borrow();
        match cell.as_ref() {
            None => SlotState::NoScope,
            Some(None) => SlotState::Miss,
            Some(Some((owner, map))) => {
                if std::ptr::eq(*owner, program) {
                    map.get(&symbol)
                        .copied()
                        .map_or(SlotState::Miss, SlotState::Hit)
                } else {
                    SlotState::ForeignProgram
                }
            }
        }
    });
    if let SlotState::Hit(owns_hook) = state {
        return owns_hook;
    }
    let owns_hook = program.machines().iter().any(|machine| {
        machine.attached_data_symbol == symbol && machine.name.as_str().ends_with("::drop")
    });
    if matches!(state, SlotState::Miss) {
        DROP_HOOK_SLOT.with(|cell| {
            if let Ok(mut slot) = cell.try_borrow_mut()
                && let Some(scope) = &mut *slot
            {
                match scope {
                    Some((owner, map)) if std::ptr::eq(*owner, program) => {
                        map.insert(symbol, owns_hook);
                    }
                    slot_none @ None => {
                        *slot_none = Some((program, HashMap::from([(symbol, owns_hook)])));
                    }
                    Some(_) => {}
                }
            }
        });
    }
    owns_hook
}

/// Whether `definition` requires establishment, memoized per (program,
/// definition): the default-domain read and write scans ask it at every data
/// read, and each answer re-walks the definition's fields and evaluates their
/// range endpoints.
pub(crate) fn memoized_data_requires_establishment(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    definition: &symbol_resolved_trees_to_typed_trees::typed_trees::data::DataDefinition,
    compute: impl FnOnce() -> bool,
) -> bool {
    enum SlotState {
        NoScope,
        Hit(bool),
        Miss,
        ForeignProgram,
    }
    let key = std::ptr::from_ref(definition).addr();
    let state = ESTABLISHMENT_SLOT.with(|cell| {
        let cell = cell.borrow();
        match cell.as_ref() {
            None => SlotState::NoScope,
            Some(None) => SlotState::Miss,
            Some(Some((owner, map))) => {
                if std::ptr::eq(*owner, program) {
                    map.get(&key)
                        .copied()
                        .map_or(SlotState::Miss, SlotState::Hit)
                } else {
                    SlotState::ForeignProgram
                }
            }
        }
    });
    if let SlotState::Hit(requires) = state {
        return requires;
    }
    let requires = compute();
    if matches!(state, SlotState::Miss) {
        ESTABLISHMENT_SLOT.with(|cell| {
            if let Ok(mut slot) = cell.try_borrow_mut()
                && let Some(scope) = &mut *slot
            {
                match scope {
                    Some((owner, map)) if std::ptr::eq(*owner, program) => {
                        map.insert(key, requires);
                    }
                    slot_none @ None => {
                        *slot_none = Some((program, HashMap::from([(key, requires)])));
                    }
                    Some(_) => {}
                }
            }
        });
    }
    requires
}

/// The conformance slot-carrier index — (trait symbol, bound requirement
/// name) -> (carrier, entry symbol) rows — is pure in the program, and the
/// license builders rebuild it per judged machine. Inside a scope it is
/// built once per build; the caller keeps the clone for both license passes.
pub(crate) fn memoized_conformance_slot_carriers(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
) -> HashMap<(symbols::SymbolHandle, String), Vec<(TypeReferenceHandle, symbols::SymbolHandle)>> {
    enum SlotState {
        NoScope,
        Hit(
            HashMap<
                (symbols::SymbolHandle, String),
                Vec<(TypeReferenceHandle, symbols::SymbolHandle)>,
            >,
        ),
        Miss,
        ForeignProgram,
    }
    let state = CONFORMANCE_SLOT_CARRIERS_SLOT.with(|cell| {
        let cell = cell.borrow();
        match cell.as_ref() {
            None => SlotState::NoScope,
            Some(None) => SlotState::Miss,
            Some(Some((owner, map))) => {
                if std::ptr::eq(*owner, program) {
                    SlotState::Hit(map.clone())
                } else {
                    SlotState::ForeignProgram
                }
            }
        }
    });
    if let SlotState::Hit(index) = state {
        return index;
    }
    let index = crate::validation::proof_contracts::contract_entailment::structural_judgment::conformance_slot_carriers_uncached(
        program,
    );
    if matches!(state, SlotState::Miss) {
        CONFORMANCE_SLOT_CARRIERS_SLOT.with(|cell| {
            if let Ok(mut slot) = cell.try_borrow_mut()
                && let Some(scope) = &mut *slot
            {
                *scope = Some((program, index.clone()));
            }
        });
    }
    index
}

/// The symbol -> data-definition position table, built once per scoped
/// program and shared by `Rc`: the claim-frontier walk built this table per
/// missed type reference, rescans of the declaration slice dominated its
/// cost.
pub(crate) fn memoized_data_definition_positions(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
) -> std::rc::Rc<HashMap<symbols::SymbolHandle, usize>> {
    enum SlotState {
        NoScope,
        Hit(std::rc::Rc<HashMap<symbols::SymbolHandle, usize>>),
        Miss,
        ForeignProgram,
    }
    let state = DATA_DEF_POSITIONS_SLOT.with(|cell| {
        let cell = cell.borrow();
        match cell.as_ref() {
            None => SlotState::NoScope,
            Some(None) => SlotState::Miss,
            Some(Some((owner, map))) => {
                if std::ptr::eq(*owner, program) {
                    SlotState::Hit(std::rc::Rc::clone(map))
                } else {
                    SlotState::ForeignProgram
                }
            }
        }
    });
    if let SlotState::Hit(map) = state {
        return map;
    }
    let map = std::rc::Rc::new(
        program
            .data_definitions()
            .iter()
            .enumerate()
            .map(|(index, definition)| (definition.symbol, index))
            .collect::<HashMap<symbols::SymbolHandle, usize>>(),
    );
    if matches!(state, SlotState::Miss) {
        DATA_DEF_POSITIONS_SLOT.with(|cell| {
            if let Ok(mut slot) = cell.try_borrow_mut()
                && let Some(scope) = &mut *slot
            {
                *scope = Some((program, map.clone()));
            }
        });
    }
    map
}

/// The symbol -> (multiplicity, kind) table over the type-parameter
/// declarations, built once per scoped program and shared by `Rc` for the
/// same reason as the data-definition positions.
pub(crate) fn memoized_type_parameter_multiplicities(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
) -> std::rc::Rc<
    HashMap<
        symbols::SymbolHandle,
        (
            language_semantics::Multiplicity,
            symbol_resolved_trees_to_typed_trees::typed_trees::data::TypeParameterKind,
        ),
    >,
> {
    enum SlotState {
        NoScope,
        Hit(
            std::rc::Rc<
                HashMap<
                    symbols::SymbolHandle,
                    (
                        language_semantics::Multiplicity,
                        symbol_resolved_trees_to_typed_trees::typed_trees::data::TypeParameterKind,
                    ),
                >,
            >,
        ),
        Miss,
        ForeignProgram,
    }
    let state = TYPE_PARAMETER_MULTIPLICITY_SLOT.with(|cell| {
        let cell = cell.borrow();
        match cell.as_ref() {
            None => SlotState::NoScope,
            Some(None) => SlotState::Miss,
            Some(Some((owner, map))) => {
                if std::ptr::eq(*owner, program) {
                    SlotState::Hit(std::rc::Rc::clone(map))
                } else {
                    SlotState::ForeignProgram
                }
            }
        }
    });
    if let SlotState::Hit(map) = state {
        return map;
    }
    let map = std::rc::Rc::new(
        program
            .data_type_parameters
            .iter()
            .filter(|(_, parameter)| parameter.symbol.is_valid())
            .map(|(_, parameter)| {
                (
                    parameter.symbol,
                    (parameter.bounds.multiplicity, parameter.kind.clone()),
                )
            })
            .collect::<HashMap<
                symbols::SymbolHandle,
                (
                    language_semantics::Multiplicity,
                    symbol_resolved_trees_to_typed_trees::typed_trees::data::TypeParameterKind,
                ),
            >>(),
    );
    if matches!(state, SlotState::Miss) {
        TYPE_PARAMETER_MULTIPLICITY_SLOT.with(|cell| {
            if let Ok(mut slot) = cell.try_borrow_mut()
                && let Some(scope) = &mut *slot
            {
                *scope = Some((program, map.clone()));
            }
        });
    }
    map
}

/// The call-argument destination answer for a target state, memoized per
/// (target, argument count): computing it rescans every machine and every
/// state to prove the target symbol is unique, which dominated its callers
/// before verdicts were shared inside the scope.
pub(crate) fn memoized_call_argument_destinations(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    target: symbols::SymbolHandle,
    argument_count: usize,
    compute: impl FnOnce() -> Option<Vec<TypeReferenceHandle>>,
) -> Option<Vec<TypeReferenceHandle>> {
    enum SlotState {
        NoScope,
        Hit(Option<Vec<TypeReferenceHandle>>),
        Miss,
        ForeignProgram,
    }
    let state = CALL_ARGUMENT_DESTINATIONS_SLOT.with(|cell| {
        let cell = cell.borrow();
        match cell.as_ref() {
            None => SlotState::NoScope,
            Some(None) => SlotState::Miss,
            Some(Some((owner, map))) => {
                if std::ptr::eq(*owner, program) {
                    map.get(&(target, argument_count))
                        .cloned()
                        .map_or(SlotState::Miss, SlotState::Hit)
                } else {
                    SlotState::ForeignProgram
                }
            }
        }
    });
    if let SlotState::Hit(destinations) = state {
        return destinations;
    }
    let destinations = compute();
    if matches!(state, SlotState::Miss) {
        CALL_ARGUMENT_DESTINATIONS_SLOT.with(|cell| {
            if let Ok(mut slot) = cell.try_borrow_mut()
                && let Some(scope) = &mut *slot
            {
                match scope {
                    Some((owner, map)) if std::ptr::eq(*owner, program) => {
                        map.insert((target, argument_count), destinations.clone());
                    }
                    slot_none @ None => {
                        *slot_none = Some((
                            program,
                            HashMap::from([((target, argument_count), destinations.clone())]),
                        ));
                    }
                    Some(_) => {}
                }
            }
        });
    }
    destinations
}

pub fn memoized_service_reach_plan(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    operational: &OperationalPlan,
) -> ServiceReachInferencePlan {
    if let Some(plan) = SERVICE_REACH_PLAN_SLOT.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|slot| slot.as_ref())
            .filter(|(owner, _)| std::ptr::eq(*owner, program))
            .map(|(_, plan)| plan.clone())
    }) {
        return plan;
    }
    let plan =
        crate::validation::machine_calls::effect_inference::service_reach::infer_service_reaches_uncached(
            program,
            operational,
        );
    SERVICE_REACH_PLAN_SLOT.with(|cell| {
        if let Ok(mut slot) = cell.try_borrow_mut()
            && let Some(scope) = &mut *slot
        {
            *scope = Some((program, plan.clone()));
        }
    });
    plan
}
