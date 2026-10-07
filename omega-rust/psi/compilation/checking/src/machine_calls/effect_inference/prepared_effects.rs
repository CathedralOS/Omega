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

pub(super) fn operational_plan(program: &dyn crate::CheckingInput) -> OperationalPlan {
    if let Some(plans) = program.prepared_effect_plans() {
        return plans.operational.clone();
    }
    crate::frozen_program::frozen_memo(
        program,
        |memos| &memos.operational,
        || super::operational::infer_operational_may_uncached(program),
    )
    .as_ref()
    .clone()
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
