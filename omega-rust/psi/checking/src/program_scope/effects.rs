//! Program-scoped operational and service-reach inference results.

use super::{OPERATIONAL_PLAN_SLOT, SERVICE_REACH_PLAN_SLOT};
use flow_effects::{OperationalPlan, ServiceReachInferencePlan};

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
