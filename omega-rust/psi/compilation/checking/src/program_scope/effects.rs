//! Program-scoped operational and service-reach inference results.

use super::{OPERATIONAL_PLAN_SLOT, SERVICE_REACH_PLAN_SLOT, program_key};
use flow_effects::{OperationalPlan, ServiceReachInferencePlan};

pub fn memoized_operational_plan(program: &typed_trees::TypedTrees) -> OperationalPlan {
    let key = program_key(program);
    if let Some(plan) = OPERATIONAL_PLAN_SLOT.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|slot| slot.as_ref())
            .filter(|(owner, _)| *owner == key)
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
            *scope = Some((key, plan.clone()));
        }
    });
    plan
}

pub fn memoized_service_reach_plan(
    program: &typed_trees::TypedTrees,
    operational: &OperationalPlan,
) -> ServiceReachInferencePlan {
    let key = program_key(program);
    if let Some(plan) = SERVICE_REACH_PLAN_SLOT.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|slot| slot.as_ref())
            .filter(|(owner, input, _)| *owner == key && input == operational)
            .map(|(_, _, plan)| plan.clone())
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
            *scope = Some((key, operational.clone(), plan.clone()));
        }
    });
    plan
}

/// A finalized dependency graph for one source preparation. Nested compilations
/// restore the enclosing graph on exit; queries cannot confuse a source-only
/// preliminary graph with the graph containing selected ownership edges.
pub struct PreparedEffectScope {
    operational: super::OperationalSlot,
    service_reach: super::ServiceReachSlot,
}

pub fn enter_prepared_effect_scope(
    program: &typed_trees::TypedTrees,
    operational: &OperationalPlan,
    service_reaches: &ServiceReachInferencePlan,
) -> PreparedEffectScope {
    let key = program_key(program);
    PreparedEffectScope {
        operational: OPERATIONAL_PLAN_SLOT
            .with(|cell| cell.replace(Some(Some((key, operational.clone()))))),
        service_reach: SERVICE_REACH_PLAN_SLOT.with(|cell| {
            cell.replace(Some(Some((
                key,
                operational.clone(),
                service_reaches.clone(),
            ))))
        }),
    }
}

impl Drop for PreparedEffectScope {
    fn drop(&mut self) {
        OPERATIONAL_PLAN_SLOT.with(|cell| {
            cell.replace(self.operational.take());
        });
        SERVICE_REACH_PLAN_SLOT.with(|cell| {
            cell.replace(self.service_reach.take());
        });
    }
}
