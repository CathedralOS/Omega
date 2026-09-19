//! Execution state of one live activation: park/resume at canonical
//! suspension crossings and the safe-point cancellation observation.
//!
//! The lifecycle claim itself never parks — `Task<T>` ownership stays put
//! while the provider suspends the activation it accounts. Parking records
//! the exact canonical crossing the invocation is suspended at and keeps
//! the retained stack lease; it establishes no result or cleanup edge.
//! Resumption continues that same invocation: no new activation identity,
//! no new storage era, no receipt replay. A parked activation holds no
//! terminal outcome, so settlement must wait for a resume.
//!
//! Cooperative cancellation arrives only at declared safe points. The
//! provider records where the activation observed a recorded request —
//! `observe_cancellation` binds the observation to a canonical crossing of
//! the activation's plan, so a `Cancelled` settlement can never report an
//! observation the plan does not admit, and a never-suspending activation
//! (empty crossing roster) can never report one at all.

use super::{LiveTaskDependency, TaskLifecycleLedger};
use crate::{
    LiveCarryDemand, SuspensionCrossingId, TaskLifecycleClaim, TaskLifecycleClaimId,
    TaskPlanDiagnostic, TaskStorageBinding,
};

/// Provider-side execution state of one live activation.
///
/// This state stays out of `TaskDependencyRecord` — like the recorded
/// cancellation request, it is provider-internal progress, not part of the
/// claim's issuance-time binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TaskExecutionState {
    /// The activation is running or runnable — not suspended at a safe
    /// point. A running activation may still pass a safe point without
    /// parking (a poll that returns), so cancellation observation is
    /// allowed here.
    Running,
    /// The activation is suspended at the named canonical crossing. The
    /// parked continuation is compiler/provider-owned and is not an
    /// addressable field of the claim.
    Parked(SuspensionCrossingId),
}

/// The suspension crossings one activation plan declares. The retained
/// validated invocation carries the plan, so an execution transition never
/// re-reads checked-tree or carry facts.
fn canonical_crossings(dependency: &LiveTaskDependency) -> &[crate::CanonicalSuspensionCrossing] {
    &dependency
        .invocation
        .executor_selection()
        .plan()
        .candidate()
        .canonical_suspension_crossings
}

impl TaskLifecycleLedger {
    /// Park the claim's activation at one canonical suspension crossing of
    /// its plan.
    ///
    /// This is the provider-side half of a safe point reached through a
    /// may-suspend call: the crossing must belong to the plan's canonical
    /// roster, which validation already restricted to
    /// `suspension_allowed` rows. Parking keeps the lease authority and
    /// every claim binding; only the execution state changes. An
    /// inline-completed activation finished before its claim existed and
    /// has nothing left to park.
    pub fn park(
        &mut self,
        claim: &TaskLifecycleClaim,
        crossing: SuspensionCrossingId,
    ) -> Result<(), TaskPlanDiagnostic> {
        let dependency = self
            .live
            .get_mut(&claim.identity())
            .filter(|dependency| dependency.matches(claim))
            .ok_or_else(|| {
                TaskPlanDiagnostic("task park requires the exact live task lifecycle claim".into())
            })?;
        if dependency.record.storage == TaskStorageBinding::InlineCompletion {
            return Err(TaskPlanDiagnostic(
                "an inline-completed activation finished before its claim existed; \
                 no live activation can park"
                    .into(),
            ));
        }
        if matches!(dependency.execution, TaskExecutionState::Parked(_)) {
            return Err(TaskPlanDiagnostic(
                "task activation is already parked; resume must continue the same \
                 invocation before it can park again"
                    .into(),
            ));
        }
        if !canonical_crossings(dependency)
            .iter()
            .any(|candidate| candidate.identity == crossing)
        {
            return Err(TaskPlanDiagnostic(
                "task park names a suspension crossing outside the activation plan's \
                 canonical roster"
                    .into(),
            ));
        }
        dependency.execution = TaskExecutionState::Parked(crossing);
        Ok(())
    }

    /// Resume a parked activation. The same invocation continues — the
    /// claim's record, receipt binding, and retained lease are untouched —
    /// and the crossing it was suspended at is returned for audit.
    pub fn resume(
        &mut self,
        claim: &TaskLifecycleClaim,
    ) -> Result<SuspensionCrossingId, TaskPlanDiagnostic> {
        let dependency = self
            .live
            .get_mut(&claim.identity())
            .filter(|dependency| dependency.matches(claim))
            .ok_or_else(|| {
                TaskPlanDiagnostic(
                    "task resume requires the exact live task lifecycle claim".into(),
                )
            })?;
        match dependency.execution {
            TaskExecutionState::Parked(crossing) => {
                dependency.execution = TaskExecutionState::Running;
                Ok(crossing)
            }
            TaskExecutionState::Running => Err(TaskPlanDiagnostic(
                "task resume requires an activation parked at a canonical suspension crossing"
                    .into(),
            )),
        }
    }

    /// Record that the activation observed a recorded cancellation request
    /// at one canonical safe point of its plan.
    ///
    /// Ordering is transactional: the request must already be recorded on
    /// this claim — an activation cannot observe a request that was never
    /// made. The crossing must belong to the plan's canonical roster; a
    /// never-suspending plan carries none, so its claims can never report
    /// an observation. An activation suspended at a crossing can observe
    /// only there; a running activation may observe at any canonical
    /// crossing it traverses.
    pub fn observe_cancellation(
        &mut self,
        claim: &TaskLifecycleClaim,
        crossing: SuspensionCrossingId,
    ) -> Result<(), TaskPlanDiagnostic> {
        let dependency = self
            .live
            .get_mut(&claim.identity())
            .filter(|dependency| dependency.matches(claim))
            .ok_or_else(|| {
                TaskPlanDiagnostic(
                    "task cancellation observation requires the exact live task \
                     lifecycle claim"
                        .into(),
                )
            })?;
        if dependency.record.storage == TaskStorageBinding::InlineCompletion {
            return Err(TaskPlanDiagnostic(
                "an inline-completed activation finished before its claim existed; \
                 no live activation can observe cancellation"
                    .into(),
            ));
        }
        if !dependency.cancellation_requested {
            return Err(TaskPlanDiagnostic(
                "task cancellation observation requires a recorded cancellation \
                 request on the claim"
                    .into(),
            ));
        }
        if !canonical_crossings(dependency)
            .iter()
            .any(|candidate| candidate.identity == crossing)
        {
            return Err(TaskPlanDiagnostic(
                "task cancellation observation names a safe point outside the \
                 activation plan's canonical suspension crossings"
                    .into(),
            ));
        }
        if let TaskExecutionState::Parked(parked) = dependency.execution
            && parked != crossing
        {
            return Err(TaskPlanDiagnostic(
                "a parked task activation can observe cancellation only at its \
                 park crossing"
                    .into(),
            ));
        }
        dependency.cancellation_observed_at = Some(crossing);
        Ok(())
    }

    /// The canonical crossing the claim's activation is parked at, or
    /// `None` when the claim is running, settled, or unknown.
    pub fn parked_crossing(&self, claim: TaskLifecycleClaimId) -> Option<SuspensionCrossingId> {
        match self.live.get(&claim).map(|dependency| dependency.execution) {
            Some(TaskExecutionState::Parked(crossing)) => Some(crossing),
            _ => None,
        }
    }

    /// The exact live frontier the claim's activation retains while parked
    /// at its crossing, or `None` when the claim is running, settled, or
    /// unknown.
    ///
    /// Each row is one live place the retained nonmoving stack and machine
    /// storage keep stable across the park; rows with a nonempty `claims`
    /// roster are the suspension-safe loans still live through this
    /// crossing. The frontier is plan evidence, so it never re-reads
    /// checked-tree or carry facts.
    pub fn parked_frontier(&self, claim: TaskLifecycleClaimId) -> Option<&[LiveCarryDemand]> {
        let dependency = self.live.get(&claim)?;
        let TaskExecutionState::Parked(crossing) = dependency.execution else {
            return None;
        };
        canonical_crossings(dependency)
            .iter()
            .find(|candidate| candidate.identity == crossing)
            .map(|candidate| candidate.live_carry.as_slice())
    }

    /// The canonical crossing where the claim's activation observed its
    /// recorded cancellation request, or `None` while no observation is
    /// recorded.
    pub fn cancellation_observed(
        &self,
        claim: TaskLifecycleClaimId,
    ) -> Option<SuspensionCrossingId> {
        self.live
            .get(&claim)
            .and_then(|dependency| dependency.cancellation_observed_at)
    }
}
