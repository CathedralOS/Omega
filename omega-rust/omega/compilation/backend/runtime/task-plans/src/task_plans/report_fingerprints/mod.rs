//! Compact, non-authoritative report fingerprints over validated plans,
//! selections, claims and receipts.

use crate::fnv1a::Fnv1a as Fingerprint;
use crate::task_plans::stack_composition::WcsuStackPlanProjection;
use crate::{ActivationPlanCandidate, LiveCarryStorage};
use language_core::{CarryAddress, CarryCpu, CarryHostThread, CarrySuspension};

pub(crate) fn activation_plan_report_fingerprint(
    plan: &ActivationPlanCandidate,
    wcsu_stack_projection: Option<&WcsuStackPlanProjection>,
) -> u64 {
    let mut fingerprint = Fingerprint::new();
    fingerprint.word(plan.machine_contract.normalized_identity());
    fingerprint.word(plan.entry.normalized_identity());
    fingerprint.word(plan.argument_layout.identity.normalized_identity());
    fingerprint.word(plan.terminal_outcome_layout.normalized_identity());
    fingerprint.word(plan.calling_plan.normalized_identity());
    fingerprint.word(plan.stack_plan.bytes);
    fingerprint.word(plan.stack_plan.alignment);
    fingerprint.word(plan.stack_plan.representation.normalized_identity());
    if let Some(projection) = wcsu_stack_projection {
        fingerprint.byte(1);
        fingerprint.word(projection.identity().normalized_identity());
    }
    fingerprint.flag(plan.may_suspend);
    fingerprint.flag(plan.may_block);
    fingerprint.word(plan.canonical_suspension_crossings.len() as u64);
    for crossing in &plan.canonical_suspension_crossings {
        fingerprint.word(crossing.identity.get());
        fingerprint.flag(crossing.suspension_allowed);
        fingerprint.flag(crossing.preserve_cpu);
        fingerprint.flag(crossing.preserve_host_thread);
        fingerprint.word(crossing.live_carry.len() as u64);
        for live in &crossing.live_carry {
            fingerprint.word(live.place.normalized_identity());
            fingerprint.word(live.ty.normalized_identity());
            fingerprint.byte(match live.storage {
                LiveCarryStorage::Persistent => 1,
                LiveCarryStorage::Parameter => 2,
                LiveCarryStorage::Local => 3,
                LiveCarryStorage::CallArgument => 4,
            });
            fingerprint.word(live.claims.len() as u64);
            for claim in &live.claims {
                fingerprint.word(claim.get());
            }
            fingerprint.flag(live.effective.suspension == CarrySuspension::Allowed);
            fingerprint.flag(live.effective.cpu == CarryCpu::Origin);
            fingerprint.flag(live.effective.host_thread == CarryHostThread::Origin);
            fingerprint.byte(match live.effective.address {
                CarryAddress::Movable => 1,
                CarryAddress::Stable => 2,
            });
        }
    }
    fingerprint.flag(plan.carry_obligations.preserve_cpu);
    fingerprint.flag(plan.carry_obligations.preserve_host_thread);
    fingerprint.flag(plan.cancellation_required);
    fingerprint.finish()
}
