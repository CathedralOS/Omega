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

use flow_effects::{OperationalPlan, ServiceReachInferencePlan};

type OperationalSlot = Option<Option<(*const typed_trees::TypedTrees, OperationalPlan)>>;
type ServiceReachSlot = Option<Option<(*const typed_trees::TypedTrees, ServiceReachInferencePlan)>>;

thread_local! {
    /// Outer `None`: no scope is open — calls compute without memoizing.
    /// `Some(None)`: scope open, not yet computed. `Some(Some(_))`: the plan
    /// for the program currently being checked.
    static OPERATIONAL_PLAN_SLOT: RefCell<OperationalSlot> = const { RefCell::new(None) };
    static SERVICE_REACH_PLAN_SLOT: RefCell<ServiceReachSlot> = const { RefCell::new(None) };
}

/// Restores the slots a scope opened on top of when it drops, so nested
/// program builds cannot leak one program's plans into its caller's.
pub struct ProgramPlanScopeGuard {
    operational: OperationalSlot,
    service_reach: ServiceReachSlot,
}

impl Drop for ProgramPlanScopeGuard {
    fn drop(&mut self) {
        OPERATIONAL_PLAN_SLOT.with(|cell| {
            *cell.borrow_mut() = self.operational.take();
        });
        SERVICE_REACH_PLAN_SLOT.with(|cell| {
            *cell.borrow_mut() = self.service_reach.take();
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
