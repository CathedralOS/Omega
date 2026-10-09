//! Source-only inference and explicitly supplied ownership-complete effects.
//!
//! A checking invocation owns its source-query memos. Once selected cleanup
//! adds execution edges, the pipeline borrows that completed graph in a new
//! context; a thread's current scope never changes a query's meaning.

use flow_effects::{OperationalPlan, ServiceReachInferencePlan};

/// Completed effect inputs for the post-custody checking phase. These are
/// working inference results, not another portable program representation.
#[derive(Debug)]
pub struct PreparedEffectPlans {
    pub operational: OperationalPlan,
    pub service_reaches: ServiceReachInferencePlan,
}

/// Read the exact invocation's completed graph without cloning its arenas.
/// Queries that consume selected call identity share this owner with effect
/// inference; a bare program computes one temporary graph for the query.
pub(crate) fn with_operational_plan<Result>(
    program: &dyn crate::CheckingInput,
    query: impl FnOnce(&OperationalPlan) -> Result,
) -> Result {
    if let Some(plans) = program.prepared_effect_plans() {
        return query(&plans.operational);
    }
    let plan = crate::frozen_program::frozen_memo(
        program,
        |memos| &memos.operational,
        || super::operational::infer_operational_may_uncached(program),
    );
    query(&plan)
}

pub(super) fn operational_plan(program: &dyn crate::CheckingInput) -> OperationalPlan {
    with_operational_plan(program, Clone::clone)
}

pub(super) fn service_reach_plan(
    program: &dyn crate::CheckingInput,
    operational: &OperationalPlan,
) -> ServiceReachInferencePlan {
    if let Some(plans) = program.prepared_effect_plans() {
        return if &plans.operational == operational {
            plans.service_reaches.clone()
        } else {
            super::service_reach::infer_service_reaches_uncached(program, operational)
        };
    }
    let Some(memos) = program.checking_memos() else {
        return super::service_reach::infer_service_reaches_uncached(program, operational);
    };
    let (input, plan) = memos.service_reaches.get_or_init(|| {
        (
            operational.clone(),
            super::service_reach::infer_service_reaches_uncached(program, operational),
        )
    });
    if input == operational {
        plan.clone()
    } else {
        // Caller-supplied alternate graphs must not inherit another graph's
        // closure. The frozen invocation normally has one source graph.
        super::service_reach::infer_service_reaches_uncached(program, operational)
    }
}
