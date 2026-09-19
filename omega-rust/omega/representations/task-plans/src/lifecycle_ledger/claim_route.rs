//! The routed `Task<T>` claim identity: the `provider`/`activation` field
//! pair a source task value carries.
//!
//! Source `Task<T>` is a linear two-field record (`provider: u64`,
//! `activation: u64`). Its fields are a bootstrap carrier for identity, not
//! public proof of custody — the admission path owns establishment of real
//! claims. That ownership lands here: `accept_invocation` mints the route on
//! the claim it issues, so a runtime serving `start`/`try_start` writes the
//! pair into the returned `Task<T>`, and every later operation that arrives
//! carrying only the value — `request_cancel`, `finish`, `settle` — routes
//! back through it. Resolution is scoped to the minting runtime instance and
//! the live claim set, so a fabricated pair, another instance's route, or a
//! settled claim's stale pair all fail closed.
//!
//! The route and the `TaskLifecycleClaim` object are two presentations of the
//! same authority. The claim object is what `accept_invocation` hands the
//! caller; the route is what crosses the source boundary inside the `Task<T>`
//! record. Route-presented operations resolve to the same live dependency the
//! claim object would, so the runtime needs no shadow map alongside the
//! ledger.

use super::TaskDependencyRecord;
use crate::TaskPlanDiagnostic;

/// The exact `provider`/`activation` field pair a source `Task<T>` value
/// carries.
///
/// `provider` names the admitted runtime *instance* that owns the lifecycle
/// record — two instances of one selected runtime keep disjoint route
/// spaces — and `activation` names the accepted activation inside it. Only
/// `accept_invocation` produces one: a pair that was never minted, or was
/// minted and then settled, resolves to nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskClaimRoute {
    provider: u64,
    activation: u64,
}

impl TaskClaimRoute {
    pub(crate) const fn for_record(record: &TaskDependencyRecord) -> Self {
        Self {
            provider: record.runtime_instance.normalized_identity(),
            activation: record.activation.normalized_identity(),
        }
    }

    /// A raw field pair. Production mints routes only through `for_record`
    /// on an accepted claim; this exists so tests can present the fabricated
    /// and foreign pairs a runtime could read out of an arbitrary `Task<T>`
    /// value.
    #[cfg(test)]
    pub(crate) const fn new(provider: u64, activation: u64) -> Self {
        Self {
            provider,
            activation,
        }
    }

    /// The `provider` field of the source `Task<T>`: the minting runtime
    /// instance's normalized identity.
    pub const fn provider(self) -> u64 {
        self.provider
    }

    /// The `activation` field of the source `Task<T>`: the accepted
    /// activation's normalized identity inside the minting instance.
    pub const fn activation(self) -> u64 {
        self.activation
    }
}

/// A failed routed settlement.
///
/// Nothing was consumed: the route is a copied field pair, so the `Task<T>`
/// it names stays live custody — the same shape as `TaskSettlementError`
/// returning the rejected claim object. The caller may retry after fixing
/// the rejected condition or route the claim elsewhere.
#[derive(Debug)]
pub struct TaskRouteSettlementError {
    route: TaskClaimRoute,
    diagnostic: TaskPlanDiagnostic,
}

impl TaskRouteSettlementError {
    pub(crate) const fn new(route: TaskClaimRoute, diagnostic: TaskPlanDiagnostic) -> Self {
        Self { route, diagnostic }
    }

    /// The routed pair the rejected settlement presented.
    pub const fn route(&self) -> TaskClaimRoute {
        self.route
    }

    /// Why settlement refused the outcome.
    pub const fn diagnostic(&self) -> &TaskPlanDiagnostic {
        &self.diagnostic
    }
}
