//! The transactional-start custody boundary: the moved arguments and storage
//! authority presented to one task start, and the rejection that conserves
//! them.
//!
//! `start<M>`/`try_start<M>` is an ownership transaction. The moved argument
//! bundle and any supplied stack lease move into the provider call together;
//! a rejection returns every one of them to the caller — no linear value
//! disappears in a failed provider call. The contract also permits returning
//! proof that custody transferred to another named authorized owner; no
//! producer of that alternative exists yet, so the only rejection carrier is
//! the conserving return below.

use crate::stack_leases::StackLease;
use crate::{ActivationInstanceId, TaskArgumentCustodyId, TaskPlanDiagnostic, ValueLayoutId};

/// Moved task-start arguments presented to the provider.
///
/// Linear carrier mirroring the source-level move into the callee: `layout`
/// must match the activation plan's argument layout, and `custody` is the
/// single-use identity this presentation is known by. The actual bytes live
/// in the provider's domain; this token is how the normalized layer accounts
/// for the bundle on both the started and rejected paths.
#[derive(Debug, PartialEq, Eq)]
pub struct MovedTaskArguments {
    layout: ValueLayoutId,
    custody: TaskArgumentCustodyId,
}

impl MovedTaskArguments {
    pub const fn new(layout: ValueLayoutId, custody: TaskArgumentCustodyId) -> Self {
        Self { layout, custody }
    }

    /// The value layout the moved bundle was marshalled under.
    pub const fn layout(&self) -> ValueLayoutId {
        self.layout
    }

    /// Single-use custody identity of this moved bundle.
    pub const fn custody(&self) -> TaskArgumentCustodyId {
        self.custody
    }
}

/// The activation-storage custody presented with one task start.
#[derive(Debug, PartialEq, Eq)]
pub enum TaskStartStorage {
    /// A nonmoving stack lease established against this activation's plan.
    Persistent(StackLease),
    /// The provider reports the activation completed during start and
    /// retained no persistent activation storage. Its lifecycle claim still
    /// requires settlement.
    InlineCompletion,
}

/// A rejected transactional task start.
///
/// Every moved argument and the supplied storage custody return to the
/// caller; nothing linear stays behind in a failed provider call. The
/// retained activation identity and diagnostic explain why admission failed.
#[derive(Debug)]
pub struct TaskStartRejection {
    activation: ActivationInstanceId,
    arguments: MovedTaskArguments,
    storage: TaskStartStorage,
    diagnostic: TaskPlanDiagnostic,
}

impl TaskStartRejection {
    pub(crate) const fn new(
        activation: ActivationInstanceId,
        arguments: MovedTaskArguments,
        storage: TaskStartStorage,
        diagnostic: TaskPlanDiagnostic,
    ) -> Self {
        Self {
            activation,
            arguments,
            storage,
            diagnostic,
        }
    }

    /// Why the provider call refused custody.
    pub const fn diagnostic(&self) -> &TaskPlanDiagnostic {
        &self.diagnostic
    }

    /// The activation identity the rejected start was aimed at.
    pub const fn activation(&self) -> ActivationInstanceId {
        self.activation
    }

    /// Consume the rejection and return the conserved custody: every moved
    /// argument and the supplied storage reservation/lease.
    pub fn into_custody(self) -> (MovedTaskArguments, TaskStartStorage) {
        (self.arguments, self.storage)
    }
}
