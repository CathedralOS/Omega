//! The nonmoving stack lease: provider-established backing authority for one
//! activation's fixed stack.
//!
//! A `StackPlan` is demand; a `StackLease` is authority. The provider
//! establishes a lease only when its presented backing satisfies the exact
//! plan — the whole-call-graph WCSU bytes, the alignment, and the selected
//! fixed representation — for the exact activation it will serve. A numeric
//! fit alone supplies no lease: `establish_stack_lease` refuses a plan that
//! lacks its sealed WCSU projection, and the issued lease binds the plan's
//! normalized identity so backing proven for one activation cannot be
//! presented for another.
//!
//! The lease is linear and has no mobility parameter: address stability for
//! stack residents follows from the lease itself, and there is no
//! provider-selectable movable continuation-storage mode. The lifecycle
//! ledger retains the authority for the claim's whole lifetime, parked
//! included; only transactional-start rejection or terminal settlement
//! releases it, and storage reuse always requires a fresh lease era.

use crate::{
    ActivationPlanId, StackPlan, TaskPlanDiagnostic, TaskStorageLeaseId, TaskStorageOwnerId,
    ValidatedActivationPlan,
};

/// Provider-normalized identity of one persistent activation-storage lease.
///
/// This record is provenance, not the source-visible lease authority. The
/// provider owns minting and hands the corresponding `StackLease` to the
/// task-start transaction; the lifecycle ledger retains that authority while
/// the claim lives and keeps this exact owner/lease edge so premature
/// reclamation and stale-era reuse reject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskStorageProvenance {
    pub owner: TaskStorageOwnerId,
    pub lease: TaskStorageLeaseId,
}

/// Untrusted fixed-stack backing a provider presents when establishing a
/// lease for one activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackLeaseBacking {
    /// Owner/lease identity the provider minted for this backing era.
    pub provenance: TaskStorageProvenance,
    /// The physical backing shape presented for validation. It may exceed
    /// the plan's demand — a pre-provisioned pool slot — but never
    /// undersatisfies it.
    pub backing: StackPlan,
}

/// Validated authority that fixed nonmoving backing satisfies one exact
/// activation plan.
///
/// Deliberately linear: the lease moves into a task start and leaves only
/// through rejection or settlement release. `satisfied` records the exact
/// `StackPlan` the backing was proved against; `backing` retains the
/// provisioned shape for audit.
#[derive(Debug, PartialEq, Eq)]
pub struct StackLease {
    provenance: TaskStorageProvenance,
    activation_plan: ActivationPlanId,
    satisfied: StackPlan,
    backing: StackPlan,
}

impl StackLease {
    /// The owner/lease edge the lifecycle ledger accounts against premature
    /// reclamation and stale-era reuse.
    pub const fn provenance(&self) -> TaskStorageProvenance {
        self.provenance
    }

    /// The exact activation plan this lease may serve.
    pub const fn activation_plan(&self) -> ActivationPlanId {
        self.activation_plan
    }

    /// The exact stack demand the backing was validated to satisfy.
    pub const fn satisfied_plan(&self) -> StackPlan {
        self.satisfied
    }

    /// The physical backing shape actually provisioned.
    pub const fn backing(&self) -> StackPlan {
        self.backing
    }
}

/// Establish the nonmoving stack lease for one activation.
///
/// `plan` must carry its sealed whole-call-graph WCSU projection: a numeric
/// fit alone supplies no `StackLease`. The presented backing must cover the
/// plan's exact byte demand at its exact alignment in the selected fixed
/// representation; the issued lease binds the plan's normalized identity so
/// it cannot be offered to a different activation.
pub fn establish_stack_lease(
    plan: &ValidatedActivationPlan,
    backing: StackLeaseBacking,
) -> Result<StackLease, TaskPlanDiagnostic> {
    if plan.wcsu_stack_projection().is_none() {
        return Err(TaskPlanDiagnostic(
            "stack lease requires the activation plan's sealed whole-call-graph WCSU evidence; \
             a numeric fit alone supplies no StackLease"
                .into(),
        ));
    }
    let required = plan.candidate().stack_plan;
    if backing.backing.representation != required.representation {
        return Err(TaskPlanDiagnostic(
            "stack lease backing representation does not match the activation plan's selected \
             fixed representation"
                .into(),
        ));
    }
    if backing.backing.alignment == 0 || !backing.backing.alignment.is_power_of_two() {
        return Err(TaskPlanDiagnostic(format!(
            "stack lease backing alignment {} is not a nonzero power of two",
            backing.backing.alignment
        )));
    }
    if backing.backing.alignment < required.alignment {
        return Err(TaskPlanDiagnostic(format!(
            "stack lease backing alignment {} understates the activation plan's required \
             alignment {}",
            backing.backing.alignment, required.alignment
        )));
    }
    if backing.backing.bytes < required.bytes {
        return Err(TaskPlanDiagnostic(format!(
            "stack lease backing of {} bytes does not cover the activation plan's {}-byte WCSU \
             demand",
            backing.backing.bytes, required.bytes
        )));
    }
    Ok(StackLease {
        provenance: backing.provenance,
        activation_plan: plan.normalized_identity(),
        satisfied: required,
        backing: backing.backing,
    })
}
