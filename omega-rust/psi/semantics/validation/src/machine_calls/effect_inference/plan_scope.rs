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
type DataDefinitionPositionSlot = Option<
    Option<(
        *const typed_trees::TypedTrees,
        HashMap<symbols::SymbolHandle, Option<u32>>,
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
    /// Data-definition lookups memoize a symbol's position in the
    /// declaration table (including negative answers) rather than one plan.
    static DATA_DEF_POSITION_SLOT: RefCell<DataDefinitionPositionSlot> =
        const { RefCell::new(None) };
}

/// Restores the slots a scope opened on top of when it drops, so nested
/// program builds cannot leak one program's plans into its caller's.
pub struct ProgramPlanScopeGuard {
    operational: OperationalSlot,
    service_reach: ServiceReachSlot,
    claim_frontiers: ClaimFrontierSlot,
    data_def_positions: DataDefinitionPositionSlot,
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
        DATA_DEF_POSITION_SLOT.with(|cell| {
            *cell.borrow_mut() = self.data_def_positions.take();
        });
    }
}

/// Open a memoization scope for one program build. Call once at the checked
/// lowering's entry; the returned guard must stay alive for the whole build.
pub fn enter_program_plan_scope() -> ProgramPlanScopeGuard {
    ProgramPlanScopeGuard {
        operational: OPERATIONAL_PLAN_SLOT
            .with(|cell| std::mem::replace(&mut *cell.borrow_mut(), Some(None))),
        service_reach: SERVICE_REACH_PLAN_SLOT
            .with(|cell| std::mem::replace(&mut *cell.borrow_mut(), Some(None))),
        claim_frontiers: CLAIM_FRONTIER_SLOT
            .with(|cell| std::mem::replace(&mut *cell.borrow_mut(), Some(None))),
        data_def_positions: DATA_DEF_POSITION_SLOT
            .with(|cell| std::mem::replace(&mut *cell.borrow_mut(), Some(None))),
    }
}

pub fn memoized_operational_plan(program: &typed_trees::TypedTrees) -> OperationalPlan {
    if let Some(plan) = OPERATIONAL_PLAN_SLOT.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|slot| slot.as_ref())
            .filter(|(owner, _)| std::ptr::eq(*owner, program))
            .map(|(_, plan)| plan.clone())
    }) {
        return plan;
    }
    let plan = crate::machine_calls::effect_inference::operational::infer_operational_may_uncached(
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
    program: &typed_trees::TypedTrees,
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
    let claims = crate::value_custody::claim_frontier::linear_claim_frontier_uncached(
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

/// The data-definition table's position for a symbol, memoized per
/// (program, symbol) inside the scope — the `.find` over the declaration
/// slice otherwise re-scans the whole table at every classification site.
pub(crate) fn memoized_data_definition_position(
    program: &typed_trees::TypedTrees,
    symbol: symbols::SymbolHandle,
) -> Option<u32> {
    enum SlotState {
        NoScope,
        Hit(Option<u32>),
        Miss,
        ForeignProgram,
    }
    let state = DATA_DEF_POSITION_SLOT.with(|cell| {
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
    if let SlotState::Hit(position) = state {
        return position;
    }
    let position = program
        .data_definitions()
        .iter()
        .position(|definition| definition.symbol == symbol)
        .map(|index| u32::try_from(index).expect("data definition position overflow"));
    if matches!(state, SlotState::Miss) {
        DATA_DEF_POSITION_SLOT.with(|cell| {
            if let Ok(mut slot) = cell.try_borrow_mut()
                && let Some(scope) = &mut *slot
            {
                match scope {
                    Some((owner, map)) if std::ptr::eq(*owner, program) => {
                        map.insert(symbol, position);
                    }
                    slot_none @ None => {
                        *slot_none = Some((program, HashMap::from([(symbol, position)])));
                    }
                    Some(_) => {}
                }
            }
        });
    }
    position
}

pub fn memoized_service_reach_plan(
    program: &typed_trees::TypedTrees,
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
        crate::machine_calls::effect_inference::service_reach::infer_service_reaches_uncached(
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
