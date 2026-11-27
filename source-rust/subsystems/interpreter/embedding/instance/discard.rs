/*
We discard a faulted, stopped or waiting guest in the order of
embedding.md "Closing and failure".

1. The instance refuses every later `invoke`, so the guest cannot resume and
   cannot originate another host call.
2. A provider operation the guest was waiting on is asked to cancel under its
   adapter's contract. A pending component invocation discards its own
   provider instance the same way.
3. The rest of the execution is dropped. The waiting call keeps its argument
   buffers and any provider homes they name, so nothing the provider was
   given is reused before it settles.
4. The ledger keeps the waiting call and charges the storage its provider
   reports retained to the invocation account. Each later `discard` consumes
   late completions through the adapter's settlement entry, never through
   resumption, and `close` succeeds only once everything has settled.
*/

use build_model::accounting::{EvaluationInvocationAccount, EvaluationStorageLease};

use super::{TerminalImportBinding, TerminalInstance};
use crate::boundary::effects::{TerminalAbandonedEffect, TerminalEffectRejection};
use crate::engine::calls::PendingBoundaryCall;
use semantic_vocabulary::BoundaryMachineId;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalDiscardStatus {
    /// Nothing is outstanding. The instance can close.
    Settled,
    /// Provider operations are still outstanding. The instance keeps them,
    /// and the storage they hold, until they settle.
    Outstanding(TerminalOutstandingCustody),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalOutstandingCustody {
    /// Unsettled provider operations of this instance and of the provider
    /// instances of its pending component invocations.
    pub provider_operations: usize,
    /// Storage those providers report they still hold.
    pub retained_bytes: u64,
    /// Why an invocation account refused to charge retained storage. The
    /// storage stays retained either way.
    pub charge_refusal: Option<String>,
    /// The last refusal a provider gave when asked to cancel or settle.
    pub provider_refusal: Option<String>,
}

pub(crate) struct DiscardCustody {
    abandoned: Option<Box<PendingBoundaryCall>>,
    account: Option<EvaluationInvocationAccount>,
    retained_bytes: u64,
    charge: Option<EvaluationStorageLease>,
    charge_refusal: Option<String>,
    provider_refusal: Option<String>,
}

impl DiscardCustody {
    fn observe(&mut self, outcome: Result<TerminalAbandonedEffect, TerminalEffectRejection>) {
        match outcome {
            Ok(TerminalAbandonedEffect::Settled) => {
                // Settlement returns the call's buffers and homes, then its charge.
                self.abandoned = None;
                self.charge = None;
                self.retained_bytes = 0;
                self.charge_refusal = None;
            }
            Ok(TerminalAbandonedEffect::Outstanding { retained_bytes }) => {
                self.charge_retained(retained_bytes);
            }
            // A refusal settles nothing: the provider may still hold the operation.
            Err(rejection) => self.provider_refusal = Some(rejection.reason),
        }
    }

    fn charge_retained(&mut self, retained_bytes: u64) {
        if retained_bytes == self.retained_bytes && (self.charge.is_some() || retained_bytes == 0) {
            return;
        }
        self.charge = None;
        self.retained_bytes = retained_bytes;
        self.charge_refusal = None;
        if retained_bytes == 0 {
            return;
        }
        if let Some(account) = &self.account {
            match account.reserve_storage(0, retained_bytes) {
                Ok(lease) => self.charge = Some(lease),
                Err(error) => self.charge_refusal = Some(format!("{error:?}")),
            }
        }
    }
}

fn adapter_for<'a, 'host>(
    bindings: &'a mut BTreeMap<BoundaryMachineId, TerminalImportBinding<'host>>,
    pending: &PendingBoundaryCall,
) -> Result<&'a mut (dyn crate::TerminalImportAdapter + 'host), TerminalEffectRejection> {
    bindings
        .get_mut(&pending.site.boundary)
        .map(|binding| &mut *binding.adapter)
        .ok_or_else(|| {
            TerminalEffectRejection::new("an abandoned provider operation lost its import binding")
        })
}

impl TerminalInstance<'_, '_> {
    /// Discard this guest. A guest that never ran or has finished simply
    /// closes. A faulted, stopped or waiting guest never resumes: its waiting
    /// provider operation is asked to cancel, its interpreter storage is
    /// reclaimed, and what the provider may still use stays in custody.
    ///
    /// Call it again to consume late completions. Each call reports what is
    /// still outstanding; `close` succeeds once the answer is `Settled`.
    pub fn discard(&mut self) -> TerminalDiscardStatus {
        if self.closed {
            return TerminalDiscardStatus::Settled;
        }
        let Some(custody) = self.discard.as_mut() else {
            return self.begin_discard();
        };
        if let Some(pending) = custody.abandoned.as_deref() {
            let outcome = adapter_for(&mut self.bindings, pending)
                .and_then(|adapter| adapter.settle_abandoned_effect(pending.effect()));
            custody.observe(outcome);
        }
        self.discard_status()
    }

    fn begin_discard(&mut self) -> TerminalDiscardStatus {
        let unfinished = self.invoked
            && self
                .execution
                .as_ref()
                .is_some_and(|execution| !execution.is_finished());
        if !unfinished {
            self.close()
                .expect("an unopened or finished instance closes without custody");
            return TerminalDiscardStatus::Settled;
        }
        let mut execution = self
            .execution
            .take()
            .expect("an unfinished instance keeps its execution");
        let abandoned = execution.awaiting_effect.take();
        let account = execution.account.clone();
        drop(execution);
        let mut custody = DiscardCustody {
            abandoned,
            account,
            retained_bytes: 0,
            charge: None,
            charge_refusal: None,
            provider_refusal: None,
        };
        if let Some(pending) = custody.abandoned.as_deref() {
            let outcome = adapter_for(&mut self.bindings, pending)
                .and_then(|adapter| adapter.request_effect_cancellation(pending.effect()));
            custody.observe(outcome);
        }
        self.discard = Some(custody);
        self.discard_status()
    }

    fn discard_status(&mut self) -> TerminalDiscardStatus {
        let custody = self
            .discard
            .as_ref()
            .expect("only a discarded instance reports discard custody");
        let mut outstanding = TerminalOutstandingCustody {
            provider_operations: usize::from(custody.abandoned.is_some()),
            retained_bytes: custody.retained_bytes,
            charge_refusal: custody.charge_refusal.clone(),
            provider_refusal: custody.provider_refusal.clone(),
        };
        for component in self.components.values_mut() {
            if let Some(TerminalDiscardStatus::Outstanding(nested)) = component.discard_pending() {
                outstanding.provider_operations += nested.provider_operations;
                outstanding.retained_bytes =
                    outstanding.retained_bytes.saturating_add(nested.retained_bytes);
                outstanding.charge_refusal = outstanding.charge_refusal.or(nested.charge_refusal);
                outstanding.provider_refusal =
                    outstanding.provider_refusal.or(nested.provider_refusal);
            }
        }
        if outstanding.provider_operations == 0 {
            TerminalDiscardStatus::Settled
        } else {
            TerminalDiscardStatus::Outstanding(outstanding)
        }
    }
}
