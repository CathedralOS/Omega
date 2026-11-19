use super::TaskPlanDiagnostic;
use language_core::{CarryCpu, CarryHostThread, CarrySuspension};
use semantic_vocabulary::{ClaimId, SuspensionCrossingId};
use terminal_psi::{TerminalTaskActivationDemand, TerminalTaskSuspensionCrossing};

pub(super) fn validate_crossing_identities(
    crossings: &[TerminalTaskSuspensionCrossing],
) -> Result<(), TaskPlanDiagnostic> {
    for crossing in crossings {
        for live in &crossing.live_carry {
            if live.place == 0 {
                return Err(TaskPlanDiagnostic(
                    "normalized live-carry-place identity cannot be zero".into(),
                ));
            }
            if live.value_type == 0 {
                return Err(TaskPlanDiagnostic(
                    "normalized live-carry-type identity cannot be zero".into(),
                ));
            }
            if live
                .claims
                .iter()
                .any(|claim| ClaimId::new(*claim).is_none())
            {
                return Err(TaskPlanDiagnostic(
                    "task carry claim has zero identity".into(),
                ));
            }
        }
        if SuspensionCrossingId::new(crossing.identity).is_none() {
            return Err(TaskPlanDiagnostic("task crossing has zero identity".into()));
        }
    }
    Ok(())
}

pub(super) fn validate_crossings(
    demand: &TerminalTaskActivationDemand,
) -> Result<(), TaskPlanDiagnostic> {
    if demand.may_suspend && demand.crossings.is_empty() {
        return Err(TaskPlanDiagnostic(
            "a suspending activation has no canonical suspension crossings".into(),
        ));
    }
    if !demand.may_suspend && !demand.crossings.is_empty() {
        return Err(TaskPlanDiagnostic(
            "a non-suspending activation cannot publish suspension crossings".into(),
        ));
    }
    if demand
        .crossings
        .iter()
        .any(|crossing| !crossing.suspension_allowed)
    {
        return Err(TaskPlanDiagnostic(
            "a possible suspension crossing carries a value that forbids suspension".into(),
        ));
    }
    let mut previous_crossing = None;
    let mut needs_cpu = false;
    let mut needs_host_thread = false;
    for crossing in &demand.crossings {
        needs_cpu |= crossing.preserve_cpu;
        needs_host_thread |= crossing.preserve_host_thread;
        if previous_crossing == Some(crossing.identity) {
            return Err(TaskPlanDiagnostic(
                "canonical suspension crossings carry a duplicate crossing identity".into(),
            ));
        }
        if previous_crossing.is_some_and(|previous| crossing.identity < previous) {
            return Err(TaskPlanDiagnostic(
                "canonical suspension crossings are not in canonical identity order".into(),
            ));
        }
        previous_crossing = Some(crossing.identity);
        let mut places = Vec::new();
        for live in &crossing.live_carry {
            if places.contains(&live.place) {
                return Err(TaskPlanDiagnostic(
                    "a canonical suspension crossing carries duplicate live places".into(),
                ));
            }
            places.push(live.place);
            let mut claims = Vec::new();
            for claim in &live.claims {
                if claims.contains(claim) {
                    return Err(TaskPlanDiagnostic(
                        "a live place at a suspension crossing carries a duplicate claim".into(),
                    ));
                }
                claims.push(*claim);
            }
            if live.effective.suspension == CarrySuspension::Forbidden {
                return Err(TaskPlanDiagnostic(
                    "a canonical suspension crossing carries a live value that forbids suspension"
                        .into(),
                ));
            }
        }
        if crossing.preserve_cpu
            != crossing
                .live_carry
                .iter()
                .any(|live| live.effective.cpu == CarryCpu::Origin)
            || crossing.preserve_host_thread
                != crossing
                    .live_carry
                    .iter()
                    .any(|live| live.effective.host_thread == CarryHostThread::Origin)
        {
            return Err(TaskPlanDiagnostic(
                "a canonical suspension crossing's preservation does not match its live \
                 frontier"
                    .into(),
            ));
        }
    }
    if (needs_cpu && !demand.carry_obligations.preserve_cpu)
        || (needs_host_thread && !demand.carry_obligations.preserve_host_thread)
    {
        return Err(TaskPlanDiagnostic(
            "activation-wide CPU/thread preservation understates a canonical crossing".into(),
        ));
    }
    Ok(())
}
