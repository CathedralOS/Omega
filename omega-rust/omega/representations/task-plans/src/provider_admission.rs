//! Provider-side admission: the boundary where one admitted runtime
//! instance turns invocation receipts and moved custody into lifecycle
//! claims.
//!
//! The carriers this gate sequences already exist independently: an
//! invocation receipt binds one dynamic call to the retained static
//! activation fact, `establish_stack_lease` proves provisioned backing
//! satisfies the plan's sealed whole-call-graph WCSU demand, and the
//! lifecycle ledger transacts both into a claim. What they did not have is
//! the provider's own boundary: fixed capacity that cannot be double-spent
//! by interleaved starters, and lease eras minted by the provider rather
//! than replayed from caller evidence. `TaskRuntimeAdmission` owns one
//! ledger plus the instance's provisioned backing so spending a slot and
//! committing the claim happen inside a single `&mut` admission — an
//! `available >= 1` observation alone is not spend authority.
//!
//! Storage custody differs by admission shape. `admit_pending` draws on the
//! provider's own provisioned backing — the arena-pool shape — so a
//! rejection conserves only the caller's moved arguments: the minted lease
//! was never caller custody. `admit_with_storage` accepts caller-supplied
//! custody — an already established lease/reservation or an inline
//! completion report — and therefore rejects through the ledger's own
//! `TaskStartRejection`, which returns the supplied storage whole.
//!
//! Admission is also where a call target the checker could not resolve — a
//! requirement slot, a machine parameter, or a dynamic descriptor — finally
//! names its concrete checked-body machine. `bind_call_targets` matches each
//! presented binding to a sealed `UnresolvedCallSite` coordinate, charges
//! the bound callee's validated subtree into the composed WCSU demand, joins
//! the subtree's canonical suspension crossings into the plan's roster, and
//! re-seals the activation plan; a site no binding names stays unresolved,
//! so the fail-closed lease rule is unchanged.

use crate::stack_leases::{StackLeaseBacking, TaskStorageProvenance, establish_stack_lease};
use crate::{
    ActivationInstanceId, ActivationPlanCandidate, CallTargetBinding, ClosedTaskRuntime,
    LiveCarryDemand, MovedTaskArguments, SettledTaskLifecycle, StackPlan, SuspensionCrossingId,
    TaskActivationPlanSet, TaskDependencyRecord, TaskLifecycleClaim, TaskLifecycleClaimId,
    TaskLifecycleLedger, TaskPlanDiagnostic, TaskRuntimeId, TaskRuntimeInstanceId,
    TaskRuntimeInvocationReceiptCandidate, TaskSettlementError, TaskSettlementOutcome,
    TaskStartRejection, TaskStartStorage, TaskStorageBinding, TaskStorageLeaseId,
    TaskStorageOwnerId, ValidatedActivationPlan, ValidatedTaskRuntimeInvocationReceipt,
    cover_unresolved_call_sites, validate_task_runtime_invocation_receipt,
    validate_wcsu_activation_plan,
};
use std::collections::BTreeMap;

/// The provider-side admission gate for one admitted runtime instance.
///
/// Owns the instance's lifecycle ledger and its fixed stack provisioning.
/// A pending admission selects provisioned backing satisfying the
/// activation's exact plan, mints a fresh lease era, establishes the
/// nonmoving `StackLease`, and transacts the claim in one operation;
/// settlement returns the backing to the free set while the spent era
/// stays burned, so storage reuse always carries a fresh lease era.
#[derive(Debug)]
pub struct TaskRuntimeAdmission {
    ledger: TaskLifecycleLedger,
    storage_owner: TaskStorageOwnerId,
    free_backing: Vec<StackPlan>,
    leased_backing: BTreeMap<TaskStorageProvenance, StackPlan>,
    next_lease_era: u64,
}

/// A rejected provider-provisioned admission.
///
/// The provider-minted storage was never caller custody: conservation
/// returns the moved argument bundle while the unspent backing stays
/// provisioned. `admit_with_storage` rejections instead use the ledger's
/// `TaskStartRejection`, which returns the supplied storage too.
#[derive(Debug)]
pub struct TaskAdmissionRejection {
    activation: ActivationInstanceId,
    arguments: MovedTaskArguments,
    diagnostic: TaskPlanDiagnostic,
}

impl TaskAdmissionRejection {
    const fn new(
        activation: ActivationInstanceId,
        arguments: MovedTaskArguments,
        diagnostic: TaskPlanDiagnostic,
    ) -> Self {
        Self {
            activation,
            arguments,
            diagnostic,
        }
    }

    /// The activation identity the rejected start was aimed at.
    pub const fn activation(&self) -> ActivationInstanceId {
        self.activation
    }

    /// Why admission refused custody.
    pub const fn diagnostic(&self) -> &TaskPlanDiagnostic {
        &self.diagnostic
    }

    /// Consume the rejection and return the conserved moved-argument bundle.
    /// Nothing linear stays behind in a failed provider call.
    pub fn into_arguments(self) -> MovedTaskArguments {
        self.arguments
    }
}

/// A failed runtime close: the gate is returned whole so its live claims
/// can settle first.
#[derive(Debug)]
pub struct TaskAdmissionCloseError {
    admission: TaskRuntimeAdmission,
    diagnostic: TaskPlanDiagnostic,
}

impl TaskAdmissionCloseError {
    pub const fn diagnostic(&self) -> &TaskPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_admission(self) -> TaskRuntimeAdmission {
        self.admission
    }
}

impl TaskRuntimeAdmission {
    /// Open the admission boundary for one runtime instance with its fixed
    /// provisioned backing. `provisioned` is the bounded capacity: every
    /// entry is one stack slot's physical shape, and a malformed entry
    /// (zero bytes or a non-power-of-two alignment) rejects here rather
    /// than sitting as silently unusable capacity. A provider with no
    /// provisioned slots can still admit inline completions and
    /// caller-supplied storage.
    pub fn new(
        runtime: TaskRuntimeId,
        instance: TaskRuntimeInstanceId,
        storage_owner: TaskStorageOwnerId,
        provisioned: Vec<StackPlan>,
    ) -> Result<Self, TaskPlanDiagnostic> {
        for (index, backing) in provisioned.iter().enumerate() {
            if backing.bytes == 0 {
                return Err(TaskPlanDiagnostic(format!(
                    "provisioned task stack slot {index} has zero bytes"
                )));
            }
            if backing.alignment == 0 || !backing.alignment.is_power_of_two() {
                return Err(TaskPlanDiagnostic(format!(
                    "provisioned task stack slot {index} alignment {} is not a nonzero power of two",
                    backing.alignment
                )));
            }
        }
        Ok(Self {
            ledger: TaskLifecycleLedger::new(runtime, instance),
            storage_owner,
            free_backing: provisioned,
            leased_backing: BTreeMap::new(),
            next_lease_era: 1,
        })
    }

    /// Auditable dependencies for every live claim this instance admitted.
    pub fn records(&self) -> impl Iterator<Item = &TaskDependencyRecord> {
        self.ledger.records()
    }

    /// Bind a provider-resolved call target to each matching unresolved site
    /// of a served activation plan, re-sealing its WCSU projection.
    ///
    /// A requirement slot, machine parameter, or dynamic descriptor the
    /// checker could not resolve names its concrete machine when this
    /// provider binds the call target at admission. Each
    /// [`CallTargetBinding`] must match one sealed `UnresolvedCallSite`
    /// exactly — frame, state, statement index and call ordinal — and carry
    /// the bound callee's validated subtree, whose frames charge into the
    /// recomposed demand. The covered plan re-projects and re-validates
    /// against the same activation candidate, so the returned plan is what
    /// the provider must swap into the served `TaskActivationPlanFact` before
    /// presenting a receipt. A site no binding names stays unresolved, so a
    /// partially covered projection still publishes partial and keeps
    /// rejecting a lease — binding never weakens the fail-closed rule.
    ///
    /// Covering also joins each bound subtree's canonical suspension
    /// crossings into the plan's roster — deduplicated by crossing identity
    /// and failing closed when a presented row conflicts with the retained
    /// one — so a bound callee that suspends can park at a crossing of its
    /// own subtree. The joined roster revalidates under the same
    /// activation-plan rules: a crossing forbidding suspension, disagreeing
    /// with its live frontier, or exceeding the plan's checked preservation
    /// envelope rejects, and a non-suspending plan cannot gain crossings at
    /// all.
    pub fn bind_call_targets(
        plan: &ValidatedActivationPlan,
        bindings: &[CallTargetBinding],
    ) -> Result<ValidatedActivationPlan, TaskPlanDiagnostic> {
        let Some(projection) = plan.wcsu_stack_projection() else {
            return Err(TaskPlanDiagnostic(
                "call target binding requires the activation plan's sealed whole-call-graph \
                 WCSU evidence"
                    .into(),
            ));
        };
        let covered = cover_unresolved_call_sites(projection, bindings)?;
        let mut candidate = plan.candidate().clone();
        candidate.stack_plan = covered.stack_plan();
        join_bound_subtree_crossings(&mut candidate, bindings)?;
        validate_wcsu_activation_plan(candidate, covered)
    }

    /// Admission for a pending activation: the provider selects unleased
    /// provisioned backing satisfying the activation's exact plan, mints a
    /// fresh lease era, establishes the nonmoving `StackLease`, and
    /// transacts the claim. Every rejection conserves the moved arguments;
    /// the unspent lease returns to provisioning because it was never
    /// caller custody.
    pub fn admit_pending(
        &mut self,
        activations: &TaskActivationPlanSet,
        receipt: TaskRuntimeInvocationReceiptCandidate,
        activation: ActivationInstanceId,
        arguments: MovedTaskArguments,
    ) -> Result<TaskLifecycleClaim, TaskAdmissionRejection> {
        let receipt = match bind_activation_receipt(activations, receipt) {
            Ok(receipt) => receipt,
            Err(diagnostic) => {
                return Err(TaskAdmissionRejection::new(
                    activation, arguments, diagnostic,
                ));
            }
        };
        let plan = receipt.executor_selection().plan();
        if plan.wcsu_stack_projection().is_none() {
            return Err(TaskAdmissionRejection::new(
                activation,
                arguments,
                TaskPlanDiagnostic(
                    "provider cannot establish a stack lease: the activation plan carries no \
                     sealed whole-call-graph WCSU evidence"
                        .into(),
                ),
            ));
        }
        let required = plan.candidate().stack_plan;
        if self.free_backing.is_empty() {
            return Err(TaskAdmissionRejection::new(
                activation,
                arguments,
                TaskPlanDiagnostic(
                    "task provider exhausted its fixed stack provisioning; no unleased backing \
                     remains"
                        .into(),
                ),
            ));
        }
        let era = match self.mint_lease_era() {
            Ok(era) => era,
            Err(diagnostic) => {
                return Err(TaskAdmissionRejection::new(
                    activation, arguments, diagnostic,
                ));
            }
        };
        // The probe itself is `establish_stack_lease`: the first slot whose
        // backing satisfies the sealed plan supplies the lease, so provider
        // admission cannot drift from lease validation.
        let mut established = None;
        for (index, backing) in self.free_backing.iter().copied().enumerate() {
            if let Ok(lease) = establish_stack_lease(
                plan,
                StackLeaseBacking {
                    provenance: TaskStorageProvenance {
                        owner: self.storage_owner,
                        lease: era,
                    },
                    backing,
                },
            ) {
                established = Some((index, lease));
                break;
            }
        }
        let Some((index, lease)) = established else {
            return Err(TaskAdmissionRejection::new(
                activation,
                arguments,
                TaskPlanDiagnostic(format!(
                    "task provider has {} unleased stack slot(s), none satisfying the \
                     activation's {}-byte {}-aligned plan",
                    self.free_backing.len(),
                    required.bytes,
                    required.alignment
                )),
            ));
        };
        let provenance = lease.provenance();
        let backing = lease.backing();
        match self.ledger.accept_invocation(
            &receipt,
            activation,
            arguments,
            TaskStartStorage::Persistent(lease),
        ) {
            Ok(claim) => {
                self.free_backing.remove(index);
                self.leased_backing.insert(provenance, backing);
                Ok(claim)
            }
            Err(rejection) => {
                // The ledger's own conserving rejection returns the unspent
                // lease; it was provider-minted, so the slot stays free and
                // the public rejection conserves only the caller's bundle.
                let diagnostic = rejection.diagnostic().clone();
                let (arguments, _unspent_lease) = rejection.into_custody();
                Err(TaskAdmissionRejection::new(
                    activation, arguments, diagnostic,
                ))
            }
        }
    }

    /// Admission presenting caller-supplied storage custody: a reservation
    /// or lease the caller established against the plan, or an inline
    /// completion report. Rejection returns the supplied storage and every
    /// moved argument through the ledger's own conserving carrier.
    pub fn admit_with_storage(
        &mut self,
        activations: &TaskActivationPlanSet,
        receipt: TaskRuntimeInvocationReceiptCandidate,
        activation: ActivationInstanceId,
        arguments: MovedTaskArguments,
        storage: TaskStartStorage,
    ) -> Result<TaskLifecycleClaim, TaskStartRejection> {
        let receipt = match bind_activation_receipt(activations, receipt) {
            Ok(receipt) => receipt,
            Err(diagnostic) => {
                return Err(TaskStartRejection::new(
                    activation, arguments, storage, diagnostic,
                ));
            }
        };
        self.ledger
            .accept_invocation(&receipt, activation, arguments, storage)
    }

    /// Record a cancellation request against the exact live claim. The
    /// request is a retained provider-side transition — it disposes no
    /// parked continuation and never removes the record — and it is the
    /// only route by which a later `Cancelled` settlement can succeed.
    pub fn request_cancellation(
        &mut self,
        claim: &TaskLifecycleClaim,
    ) -> Result<(), TaskPlanDiagnostic> {
        self.ledger.request_cancellation(claim)
    }

    /// Whether a cancellation request was recorded against a live claim.
    pub fn cancellation_requested(&self, claim: TaskLifecycleClaimId) -> bool {
        self.ledger.cancellation_requested(claim)
    }

    /// Park the claim's activation at one canonical suspension crossing of
    /// its plan. Parking keeps the claim's lease authority and every
    /// binding; it establishes no result or cleanup edge.
    pub fn park(
        &mut self,
        claim: &TaskLifecycleClaim,
        crossing: SuspensionCrossingId,
    ) -> Result<(), TaskPlanDiagnostic> {
        self.ledger.park(claim, crossing)
    }

    /// Resume a parked activation: the same invocation continues under the
    /// same claim, receipt binding and retained lease. Returns the crossing
    /// the activation was suspended at.
    pub fn resume(
        &mut self,
        claim: &TaskLifecycleClaim,
    ) -> Result<SuspensionCrossingId, TaskPlanDiagnostic> {
        self.ledger.resume(claim)
    }

    /// Record that the activation observed a recorded cancellation request
    /// at one canonical safe point of its plan — the transition that makes
    /// a later `Cancelled` settlement honest.
    pub fn observe_cancellation(
        &mut self,
        claim: &TaskLifecycleClaim,
        crossing: SuspensionCrossingId,
    ) -> Result<(), TaskPlanDiagnostic> {
        self.ledger.observe_cancellation(claim, crossing)
    }

    /// The canonical crossing the claim's activation is parked at, or
    /// `None` when it is running, settled, or unknown.
    pub fn parked_crossing(&self, claim: TaskLifecycleClaimId) -> Option<SuspensionCrossingId> {
        self.ledger.parked_crossing(claim)
    }

    /// The exact live frontier the claim's activation retains while parked
    /// — the suspension-safe loan roster a provider must keep alive and
    /// address-stable across this crossing — or `None` when it is running,
    /// settled, or unknown.
    pub fn parked_frontier(&self, claim: TaskLifecycleClaimId) -> Option<&[LiveCarryDemand]> {
        self.ledger.parked_frontier(claim)
    }

    /// The canonical crossing where the claim's activation observed its
    /// recorded cancellation request, or `None` while none is recorded.
    pub fn cancellation_observed(
        &self,
        claim: TaskLifecycleClaimId,
    ) -> Option<SuspensionCrossingId> {
        self.ledger.cancellation_observed(claim)
    }

    /// The provider's storage-reclaim precondition for an external storage
    /// authority. Pool backing is reclaimed by `settle` directly.
    pub fn validate_storage_reclaim(
        &self,
        storage: TaskStorageProvenance,
    ) -> Result<(), TaskPlanDiagnostic> {
        self.ledger.validate_storage_reclaim(storage)
    }

    /// Terminal settlement reporting the observed lifecycle outcome.
    /// Released pool backing returns to the free set; its era stays burned
    /// in the ledger, so reusing the slot still mints a fresh lease era.
    /// Caller-supplied storage is not pool backing and passes back to its
    /// owner through the settlement carrier. A parked activation must resume
    /// before any settlement, and a `Cancelled` outcome settles only when a
    /// cancellation request was recorded on the claim and observed at a
    /// canonical safe point.
    pub fn settle(
        &mut self,
        claim: TaskLifecycleClaim,
        outcome: TaskSettlementOutcome,
    ) -> Result<SettledTaskLifecycle, TaskSettlementError> {
        let settled = self.ledger.settle(claim, outcome)?;
        if let TaskStorageBinding::Persistent(provenance) = settled.released_storage()
            && let Some(backing) = self.leased_backing.remove(&provenance)
        {
            self.free_backing.push(backing);
        }
        Ok(settled)
    }

    /// Close consumes the gate only when every admitted claim has settled.
    pub fn close(self) -> Result<ClosedTaskRuntime, TaskAdmissionCloseError> {
        let Self {
            ledger,
            storage_owner,
            free_backing,
            leased_backing,
            next_lease_era,
        } = self;
        match ledger.close() {
            Ok(closed) => Ok(closed),
            Err(error) => {
                let diagnostic = error.diagnostic().clone();
                Err(TaskAdmissionCloseError {
                    diagnostic,
                    admission: Self {
                        ledger: error.into_ledger(),
                        storage_owner,
                        free_backing,
                        leased_backing,
                        next_lease_era,
                    },
                })
            }
        }
    }

    /// Mint one fresh lease era. Eras are monotone per instance and never
    /// repeat — including eras burned by failed admissions — so reuse
    /// cannot replay old evidence.
    fn mint_lease_era(&mut self) -> Result<TaskStorageLeaseId, TaskPlanDiagnostic> {
        let era = self.next_lease_era;
        let identity = TaskStorageLeaseId::from_normalized_identity(era)?;
        self.next_lease_era = era.checked_add(1).ok_or_else(|| {
            TaskPlanDiagnostic("task provider lease era space is exhausted".into())
        })?;
        Ok(identity)
    }
}

/// Join each bound subtree's canonical suspension crossings into the
/// candidate's roster. An identity the roster already holds must carry the
/// identical row — a same-identity crossing with different content is a
/// coordinate conflict and fails closed — while a novel identity joins the
/// roster. The joined roster lands in canonical identity order, matching
/// what graph derivation would have published had it resolved the call
/// itself, so the re-minted plan identity binds one canonical ordering.
fn join_bound_subtree_crossings(
    candidate: &mut ActivationPlanCandidate,
    bindings: &[CallTargetBinding],
) -> Result<(), TaskPlanDiagnostic> {
    for crossing in bindings.iter().flat_map(|binding| binding.crossings.iter()) {
        match candidate
            .canonical_suspension_crossings
            .iter()
            .position(|existing| existing.identity == crossing.identity)
        {
            Some(position) if candidate.canonical_suspension_crossings[position] == *crossing => {
                // A crossing the bound subtree shares with the graph's own
                // roster — or a covered subtree presented again — dedupes.
            }
            Some(_) => {
                return Err(TaskPlanDiagnostic(format!(
                    "call target binding joins suspension crossing 0x{:016x} that conflicts \
                     with the plan's retained canonical row",
                    crossing.identity.get()
                )));
            }
            None => candidate
                .canonical_suspension_crossings
                .push(crossing.clone()),
        }
    }
    candidate
        .canonical_suspension_crossings
        .sort_unstable_by_key(|crossing| crossing.identity);
    Ok(())
}

/// Bind an untrusted invocation receipt candidate to the exact retained
/// activation fact it names. The candidate's plan identity and operation
/// select the fact; validation then binds every remaining field before any
/// storage or custody moves. A receipt naming no served plan/operation
/// fails closed.
fn bind_activation_receipt(
    activations: &TaskActivationPlanSet,
    receipt: TaskRuntimeInvocationReceiptCandidate,
) -> Result<ValidatedTaskRuntimeInvocationReceipt, TaskPlanDiagnostic> {
    let mut first_failure = None;
    for fact in activations.as_slice() {
        if fact.operation != receipt.operation
            || fact.plan.normalized_identity() != receipt.activation_plan
        {
            continue;
        }
        match validate_task_runtime_invocation_receipt(fact, receipt.clone()) {
            Ok(validated) => return Ok(validated),
            Err(diagnostic) => {
                first_failure.get_or_insert(diagnostic);
            }
        }
    }
    Err(first_failure.unwrap_or_else(|| {
        TaskPlanDiagnostic(
            "task start names no activation plan and operation this provider serves".into(),
        )
    }))
}
