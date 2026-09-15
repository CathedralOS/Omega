//! The task lifecycle ledger: claims that pin a runtime, plan and storage
//! until settlement, dependency records, and the settled and closed carriers.

use crate::report_fingerprints::task_claim_report_fingerprint;
use crate::{
    ActivationInstanceId, ActivationPlanId, ExecutorSelectionId, TaskLifecycleClaimId,
    TaskPlanDiagnostic, TaskRuntimeId, TaskRuntimeInstanceId, TaskRuntimeInvocationBindingId,
    TaskRuntimeInvocationId, TaskRuntimeInvocationReceiptId, TaskStartOperation,
    TaskStorageLeaseId, TaskStorageOwnerId, ValidatedTaskRuntimeInvocationReceipt,
};
use std::collections::{BTreeMap, BTreeSet};

/// Provider-normalized identity of one persistent activation-storage lease.
///
/// This record is provenance, not the source-visible lease authority. The
/// provider owns minting and transfers the corresponding linear lease through
/// the task-start outcome; the lifecycle ledger retains only the exact
/// owner/lease edge needed to reject premature reclamation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskStorageProvenance {
    pub owner: TaskStorageOwnerId,
    pub lease: TaskStorageLeaseId,
}

/// Physical-storage relationship selected for one accepted activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStorageBinding {
    Persistent(TaskStorageProvenance),
    /// The activation completed during start and retained no persistent
    /// activation storage. Its lifecycle claim still requires settlement.
    InlineCompletion,
}

/// Auditable dependency retained for every live task claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskDependencyRecord {
    pub claim: TaskLifecycleClaimId,
    pub runtime: TaskRuntimeId,
    pub runtime_instance: TaskRuntimeInstanceId,
    pub activation_plan: ActivationPlanId,
    pub executor_selection: ExecutorSelectionId,
    pub invocation_binding: TaskRuntimeInvocationBindingId,
    pub invocation: TaskRuntimeInvocationId,
    pub invocation_receipt: TaskRuntimeInvocationReceiptId,
    pub operation: TaskStartOperation,
    pub activation: ActivationInstanceId,
    pub storage: TaskStorageBinding,
}

/// Source-level `Task<T>` is linear. This normalized carrier mirrors that
/// property by withholding `Clone`/`Copy` and exposing no public constructor.
#[derive(Debug, PartialEq, Eq)]
pub struct TaskLifecycleClaim {
    dependency: Box<LiveTaskDependency>,
}

impl TaskLifecycleClaim {
    pub const fn identity(&self) -> TaskLifecycleClaimId {
        self.dependency.record.claim
    }

    pub const fn runtime_instance(&self) -> TaskRuntimeInstanceId {
        self.dependency.record.runtime_instance
    }

    pub const fn activation(&self) -> ActivationInstanceId {
        self.dependency.record.activation
    }

    pub const fn storage(&self) -> TaskStorageBinding {
        self.dependency.record.storage
    }
}

/// Exact validated provider invocation retained behind one live lifecycle
/// claim, including its selected executor and activation plan.
///
/// `TaskDependencyRecord` remains the compact report form. Custody and
/// settlement compare this carrier so compact identity collisions cannot move
/// a claim between distinct invocations or activation plans.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LiveTaskDependency {
    record: TaskDependencyRecord,
    invocation: Box<ValidatedTaskRuntimeInvocationReceipt>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct SettledTaskLifecycle {
    record: TaskDependencyRecord,
}

impl SettledTaskLifecycle {
    pub const fn identity(&self) -> TaskLifecycleClaimId {
        self.record.claim
    }

    /// Returns the exact storage relationship released by terminal task
    /// settlement. A provider may reclaim/recycle persistent storage only
    /// after receiving this result.
    pub const fn released_storage(&self) -> TaskStorageBinding {
        self.record.storage
    }
}

#[derive(Debug)]
pub struct TaskSettlementError {
    claim: TaskLifecycleClaim,
    diagnostic: TaskPlanDiagnostic,
}

impl TaskSettlementError {
    pub const fn diagnostic(&self) -> &TaskPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_claim(self) -> TaskLifecycleClaim {
        self.claim
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ClosedTaskRuntime {
    runtime: TaskRuntimeId,
    instance: TaskRuntimeInstanceId,
}

impl ClosedTaskRuntime {
    pub const fn runtime(&self) -> TaskRuntimeId {
        self.runtime
    }

    pub const fn instance(&self) -> TaskRuntimeInstanceId {
        self.instance
    }
}

#[derive(Debug)]
pub struct TaskRuntimeCloseError {
    ledger: Box<TaskLifecycleLedger>,
    diagnostic: TaskPlanDiagnostic,
}

impl TaskRuntimeCloseError {
    pub const fn diagnostic(&self) -> &TaskPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_ledger(self) -> TaskLifecycleLedger {
        *self.ledger
    }
}

/// Provider-local accounting for operational custody, physical storage, and
/// the independently held lifecycle claim.
///
/// A selected runtime instance has one ledger. Recording an already accepted
/// invocation binds its exact static provider/operation selection, dynamic
/// receipt, activation plan, and storage dependency before a `Task<T>` claim
/// is issued. Terminal settlement removes that dependency. Runtime close and
/// storage reclamation fail while any matching child claim remains live.
#[derive(Debug)]
pub struct TaskLifecycleLedger {
    runtime: TaskRuntimeId,
    instance: TaskRuntimeInstanceId,
    live: BTreeMap<TaskLifecycleClaimId, LiveTaskDependency>,
    used_activations: BTreeSet<ActivationInstanceId>,
    used_invocations: BTreeSet<TaskRuntimeInvocationId>,
    used_invocation_receipts: BTreeSet<TaskRuntimeInvocationReceiptId>,
    used_storage_leases: BTreeSet<TaskStorageProvenance>,
}

impl TaskLifecycleLedger {
    /// Create empty accounting for one runtime instance. Each accepted start
    /// must still supply its exact validated provider invocation receipt.
    pub fn new(runtime: TaskRuntimeId, instance: TaskRuntimeInstanceId) -> Self {
        Self {
            runtime,
            instance,
            live: BTreeMap::new(),
            used_activations: BTreeSet::new(),
            used_invocations: BTreeSet::new(),
            used_invocation_receipts: BTreeSet::new(),
            used_storage_leases: BTreeSet::new(),
        }
    }

    pub fn records(&self) -> impl Iterator<Item = &TaskDependencyRecord> {
        self.live.values().map(|dependency| &dependency.record)
    }

    pub fn accept_invocation(
        &mut self,
        invocation: &ValidatedTaskRuntimeInvocationReceipt,
        activation: ActivationInstanceId,
        storage: TaskStorageBinding,
    ) -> Result<TaskLifecycleClaim, TaskPlanDiagnostic> {
        if invocation.candidate().runtime != self.runtime
            || invocation.candidate().runtime_instance != self.instance
        {
            return Err(TaskPlanDiagnostic(
                "task invocation receipt belongs to a different runtime instance".into(),
            ));
        }
        if self
            .used_invocations
            .contains(&invocation.candidate().invocation)
        {
            return Err(TaskPlanDiagnostic(
                "task runtime invocation identity has already been accepted by this runtime instance"
                    .into(),
            ));
        }
        if self
            .used_invocation_receipts
            .contains(&invocation.candidate().receipt)
        {
            return Err(TaskPlanDiagnostic(
                "task runtime invocation receipt has already been accepted by this runtime instance"
                    .into(),
            ));
        }
        if self.used_activations.contains(&activation) {
            return Err(TaskPlanDiagnostic(
                "task activation identity has already been accepted by this runtime instance"
                    .into(),
            ));
        }
        match storage {
            TaskStorageBinding::Persistent(provenance) => {
                if self.used_storage_leases.contains(&provenance) {
                    return Err(TaskPlanDiagnostic(
                        "task storage lease identity has already been used; storage reuse requires a new lease era"
                            .into(),
                    ));
                }
            }
            TaskStorageBinding::InlineCompletion => {}
        }

        let claim = TaskLifecycleClaimId(task_claim_report_fingerprint(
            invocation, activation, storage,
        ));
        if self.live.contains_key(&claim) {
            return Err(TaskPlanDiagnostic(
                "normalized task lifecycle claim identity collides with a live claim".into(),
            ));
        }
        let record = TaskDependencyRecord {
            claim,
            runtime: self.runtime,
            runtime_instance: self.instance,
            activation_plan: invocation.activation().activation_plan,
            executor_selection: invocation.executor_selection().identity(),
            invocation_binding: invocation.identity(),
            invocation: invocation.candidate().invocation,
            invocation_receipt: invocation.candidate().receipt,
            operation: invocation.candidate().operation,
            activation,
            storage,
        };
        self.used_invocations
            .insert(invocation.candidate().invocation);
        self.used_invocation_receipts
            .insert(invocation.candidate().receipt);
        self.used_activations.insert(activation);
        if let TaskStorageBinding::Persistent(provenance) = storage {
            self.used_storage_leases.insert(provenance);
        }
        let dependency = LiveTaskDependency {
            record,
            invocation: Box::new(invocation.clone()),
        };
        self.live.insert(claim, dependency.clone());
        Ok(TaskLifecycleClaim {
            dependency: Box::new(dependency),
        })
    }

    /// Cancellation requests preserve the lifecycle obligation. This check is
    /// intentionally read-only: only terminal settlement removes the record.
    pub fn validate_cancellation_request(
        &self,
        claim: &TaskLifecycleClaim,
    ) -> Result<(), TaskPlanDiagnostic> {
        if self.live.get(&claim.dependency.record.claim) == Some(claim.dependency.as_ref()) {
            Ok(())
        } else {
            Err(TaskPlanDiagnostic(
                "cancellation requires the exact live task lifecycle claim".into(),
            ))
        }
    }

    pub fn settle(
        &mut self,
        claim: TaskLifecycleClaim,
    ) -> Result<SettledTaskLifecycle, TaskSettlementError> {
        if self.live.get(&claim.dependency.record.claim) != Some(claim.dependency.as_ref()) {
            return Err(TaskSettlementError {
                claim,
                diagnostic: TaskPlanDiagnostic(
                    "task settlement requires the exact live runtime/activation/storage claim"
                        .into(),
                ),
            });
        }
        self.live.remove(&claim.dependency.record.claim);
        Ok(SettledTaskLifecycle {
            record: claim.dependency.record,
        })
    }

    /// Validate the provider's storage-reclaim precondition. The storage
    /// authority itself remains outside this normalized ledger.
    pub fn validate_storage_reclaim(
        &self,
        storage: TaskStorageProvenance,
    ) -> Result<(), TaskPlanDiagnostic> {
        if self
            .live
            .values()
            .any(|dependency| dependency.record.storage == TaskStorageBinding::Persistent(storage))
        {
            return Err(TaskPlanDiagnostic(
                "task storage cannot be reclaimed while a dependent lifecycle claim is live".into(),
            ));
        }
        Ok(())
    }

    /// Close consumes the provider-local ledger only when every child task
    /// claim has been terminally settled or transferred out through a future
    /// explicitly accounted operation.
    pub fn close(self) -> Result<ClosedTaskRuntime, TaskRuntimeCloseError> {
        if !self.live.is_empty() {
            return Err(TaskRuntimeCloseError {
                diagnostic: TaskPlanDiagnostic(format!(
                    "task runtime cannot close while {} dependent lifecycle claim(s) remain live",
                    self.live.len()
                )),
                ledger: Box::new(self),
            });
        }
        Ok(ClosedTaskRuntime {
            runtime: self.runtime,
            instance: self.instance,
        })
    }
}
