//! The task lifecycle ledger: claims that pin a runtime, plan and storage
//! until settlement, dependency records, and the settled and closed carriers.
//!
//! `start.rs` owns the transactional-start custody boundary: the moved
//! arguments and the supplied nonmoving stack lease enter `accept_invocation`
//! together, every rejection returns them whole, and the ledger retains the
//! lease authority while the claim lives. `mod.rs` owns the accounting
//! itself: the live dependency map, the single-use identity sets, settlement,
//! reclaim validation, and close.

mod start;

pub use start::{MovedTaskArguments, TaskStartRejection, TaskStartStorage};

use crate::report_fingerprints::task_claim_report_fingerprint;
use crate::stack_leases::{StackLease, TaskStorageProvenance};
use crate::{
    ActivationInstanceId, ActivationPlanId, ExecutorSelectionId, TaskArgumentCustodyId,
    TaskLifecycleClaimId, TaskPlanDiagnostic, TaskRuntimeId, TaskRuntimeInstanceId,
    TaskRuntimeInvocationBindingId, TaskRuntimeInvocationId, TaskRuntimeInvocationReceiptId,
    TaskStartOperation, ValidatedTaskRuntimeInvocationReceipt,
};
use std::collections::{BTreeMap, BTreeSet};

/// Physical-storage relationship recorded for one accepted activation.
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
    /// Custody identity of the moved argument bundle this activation
    /// consumed at start. Rejection returns the bundle to the caller; a
    /// started task records here where it went.
    pub argument_custody: TaskArgumentCustodyId,
    pub storage: TaskStorageBinding,
}

/// Source-level `Task<T>` is linear. This normalized carrier mirrors that
/// property by withholding `Clone`/`Copy` and exposing no public constructor.
#[derive(Debug, PartialEq, Eq)]
pub struct TaskLifecycleClaim {
    record: TaskDependencyRecord,
    invocation: Box<ValidatedTaskRuntimeInvocationReceipt>,
}

impl TaskLifecycleClaim {
    pub const fn identity(&self) -> TaskLifecycleClaimId {
        self.record.claim
    }

    pub const fn runtime_instance(&self) -> TaskRuntimeInstanceId {
        self.record.runtime_instance
    }

    pub const fn activation(&self) -> ActivationInstanceId {
        self.record.activation
    }

    pub const fn storage(&self) -> TaskStorageBinding {
        self.record.storage
    }
}

/// Exact validated provider invocation retained behind one live lifecycle
/// claim, including its selected executor, activation plan, and the stack
/// lease authority held for the claim's lifetime.
///
/// `TaskDependencyRecord` remains the compact report form. Custody and
/// settlement compare the record and invocation so compact identity
/// collisions cannot move a claim between distinct invocations or activation
/// plans.
#[derive(Debug)]
struct LiveTaskDependency {
    record: TaskDependencyRecord,
    invocation: Box<ValidatedTaskRuntimeInvocationReceipt>,
    /// Provider-held stack-lease authority while the claim lives. `None` for
    /// an inline completion, which retains no persistent activation storage.
    storage_authority: Option<StackLease>,
}

impl LiveTaskDependency {
    /// The claim compares the exact record and invocation it was issued
    /// against; the retained lease authority is provider-side state.
    fn matches(&self, claim: &TaskLifecycleClaim) -> bool {
        self.record == claim.record && self.invocation.as_ref() == claim.invocation.as_ref()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct SettledTaskLifecycle {
    record: TaskDependencyRecord,
    storage_authority: Option<StackLease>,
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

    /// Consume the settlement and return the stack-lease authority it
    /// released. The returned lease is era-spent: reusing the storage still
    /// requires minting a fresh lease.
    pub fn into_released_lease(self) -> Option<StackLease> {
        self.storage_authority
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
/// receipt, activation plan, moved-argument custody, and storage dependency
/// before a `Task<T>` claim is issued. Terminal settlement removes that
/// dependency and releases the retained lease authority. Runtime close and
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

    /// The transactional start: the moved argument bundle and the supplied
    /// storage custody move into the provider call together. Success issues
    /// the linear lifecycle claim and retains the stack lease for the
    /// claim's lifetime; any rejection conserves every moved argument and
    /// the supplied lease inside `TaskStartRejection`.
    pub fn accept_invocation(
        &mut self,
        invocation: &ValidatedTaskRuntimeInvocationReceipt,
        activation: ActivationInstanceId,
        arguments: MovedTaskArguments,
        storage: TaskStartStorage,
    ) -> Result<TaskLifecycleClaim, TaskStartRejection> {
        let (claim, storage_binding) =
            match self.check_start(invocation, activation, &arguments, &storage) {
                Ok(accepted) => accepted,
                Err(diagnostic) => {
                    return Err(TaskStartRejection::new(
                        activation, arguments, storage, diagnostic,
                    ));
                }
            };
        self.used_invocations
            .insert(invocation.candidate().invocation);
        self.used_invocation_receipts
            .insert(invocation.candidate().receipt);
        self.used_activations.insert(activation);
        let storage_authority = match storage {
            TaskStartStorage::Persistent(lease) => {
                self.used_storage_leases.insert(lease.provenance());
                Some(lease)
            }
            TaskStartStorage::InlineCompletion => None,
        };
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
            argument_custody: arguments.custody(),
            storage: storage_binding,
        };
        let dependency = LiveTaskDependency {
            record,
            invocation: Box::new(invocation.clone()),
            storage_authority,
        };
        self.live.insert(claim, dependency);
        Ok(TaskLifecycleClaim {
            record,
            invocation: Box::new(invocation.clone()),
        })
    }

    /// Admission checks for one transactional start. Every failure leaves the
    /// caller's moved arguments and storage custody untouched so the
    /// rejection can return them whole.
    fn check_start(
        &self,
        invocation: &ValidatedTaskRuntimeInvocationReceipt,
        activation: ActivationInstanceId,
        arguments: &MovedTaskArguments,
        storage: &TaskStartStorage,
    ) -> Result<(TaskLifecycleClaimId, TaskStorageBinding), TaskPlanDiagnostic> {
        if invocation.candidate().runtime != self.runtime
            || invocation.candidate().runtime_instance != self.instance
        {
            return Err(TaskPlanDiagnostic(
                "task invocation receipt belongs to a different runtime instance".into(),
            ));
        }
        if arguments.layout()
            != invocation
                .executor_selection()
                .plan()
                .candidate()
                .argument_layout
        {
            return Err(TaskPlanDiagnostic(
                "moved task start arguments do not match the activation plan's argument layout"
                    .into(),
            ));
        }
        let storage_binding = match storage {
            TaskStartStorage::Persistent(lease) => {
                if lease.activation_plan() != invocation.activation().activation_plan {
                    return Err(TaskPlanDiagnostic(
                        "task stack lease was established for a different activation plan".into(),
                    ));
                }
                TaskStorageBinding::Persistent(lease.provenance())
            }
            TaskStartStorage::InlineCompletion => TaskStorageBinding::InlineCompletion,
        };
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
        if let TaskStorageBinding::Persistent(provenance) = storage_binding
            && self.used_storage_leases.contains(&provenance)
        {
            return Err(TaskPlanDiagnostic(
                "task storage lease identity has already been used; storage reuse requires a new lease era"
                    .into(),
            ));
        }
        let claim = TaskLifecycleClaimId(task_claim_report_fingerprint(
            invocation,
            activation,
            arguments.custody(),
            storage_binding,
        ));
        if self.live.contains_key(&claim) {
            return Err(TaskPlanDiagnostic(
                "normalized task lifecycle claim identity collides with a live claim".into(),
            ));
        }
        Ok((claim, storage_binding))
    }

    /// Cancellation requests preserve the lifecycle obligation. This check is
    /// intentionally read-only: only terminal settlement removes the record.
    pub fn validate_cancellation_request(
        &self,
        claim: &TaskLifecycleClaim,
    ) -> Result<(), TaskPlanDiagnostic> {
        if self
            .live
            .get(&claim.record.claim)
            .is_some_and(|dependency| dependency.matches(claim))
        {
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
        let matches = self
            .live
            .get(&claim.record.claim)
            .is_some_and(|dependency| dependency.matches(&claim));
        if !matches {
            return Err(TaskSettlementError {
                claim,
                diagnostic: TaskPlanDiagnostic(
                    "task settlement requires the exact live runtime/activation/storage claim"
                        .into(),
                ),
            });
        }
        let dependency = self
            .live
            .remove(&claim.record.claim)
            .expect("matched live claim");
        Ok(SettledTaskLifecycle {
            record: dependency.record,
            storage_authority: dependency.storage_authority,
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
