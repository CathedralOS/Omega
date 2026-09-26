//! Live activations of the generated installed-entry bridge.
//!
//! Physical arrival at an installed entry linearizes into the component era
//! through the lifecycle ledger's `enter`; the activation it returns is the
//! non-clonable scope inside which program-local root subjects are observed
//! and established. This token records its issuing ledger so the
//! establishment path cannot be satisfied by an activation entered under a
//! different binding, and `leave` returns the same ledger's hold when the
//! semantic continuation completes.

use effects::{
    ActiveComponentEraEntry, ComponentEraEntryLedger, ComponentEraEntryReceipt,
    ComponentEraLeaveReceipt, ComponentEraLedgerId, EraEntryError,
};

use crate::ExternalRootDiagnostic;
use crate::program_local::program_local_roots::ProgramLocalRootEntryInvocationId;

/// One live activation of a generated installed-entry bridge.
///
/// [`ProgramLocalEntryActivation::enter`] is the sole mint: it is non-clonable,
/// bound to the exact issuing lifecycle ledger, and carries the ledger's own
/// active entry hold. Borrowing it proves the activation is still inside its
/// enter/leave scope — `leave` consumes the token — while its invocation
/// identity stamps every subject the activation observes.
#[derive(Debug)]
pub struct ProgramLocalEntryActivation {
    entry: ActiveComponentEraEntry,
    ledger: ComponentEraLedgerId,
}

impl ProgramLocalEntryActivation {
    /// Enter the lifecycle ledger's current open era for one generated entry
    /// activation. The receipt is the same exact linearization the component
    /// lifecycle demands of any entry: rejection returns it untouched.
    pub fn enter(
        lifecycle: &mut ComponentEraEntryLedger,
        receipt: ComponentEraEntryReceipt,
    ) -> Result<Self, EraEntryError> {
        let ledger = lifecycle.identity();
        let entry = lifecycle.enter(receipt)?;
        Ok(Self { entry, ledger })
    }

    /// Leave through the same ledger that minted this activation. Rejection
    /// returns the activation and receipt intact so the rightful scope can
    /// still complete or be audited.
    pub fn leave(
        self,
        lifecycle: &mut ComponentEraEntryLedger,
        receipt: ComponentEraLeaveReceipt,
    ) -> Result<(), ProgramLocalEntryActivationLeaveError> {
        let ledger = self.ledger;
        if lifecycle.identity() != ledger {
            return Err(ProgramLocalEntryActivationLeaveError {
                activation: self,
                receipt,
                diagnostic: ExternalRootDiagnostic(
                    "program-local entry activation cannot leave through a different lifecycle ledger"
                        .into(),
                ),
            });
        }
        match lifecycle.leave(self.entry, receipt) {
            Ok(()) => Ok(()),
            Err(error) => {
                let (entry, receipt) = (*error).into_parts();
                Err(ProgramLocalEntryActivationLeaveError {
                    activation: Self { entry, ledger },
                    receipt,
                    diagnostic: ExternalRootDiagnostic(
                        "program-local entry activation leave did not complete its exact active entry"
                            .into(),
                    ),
                })
            }
        }
    }

    /// The lifecycle ledger that minted this activation.
    pub const fn ledger(&self) -> ComponentEraLedgerId {
        self.ledger
    }

    /// The exact era this activation entered.
    pub const fn era_identity(&self) -> u64 {
        self.entry.era_identity()
    }

    /// The leave receipt this exact activation must present to complete: the
    /// ledger replays its bound invocation, contract, era and entry plan, so a
    /// receipt minted for any other entry cannot close this scope.
    pub fn leave_receipt(&self, leave_completed: bool) -> ComponentEraLeaveReceipt {
        ComponentEraLeaveReceipt::from_runtime(&self.entry, leave_completed)
    }

    /// The report identity the ledger linearized for this activation. The
    /// `enter` gate already rejects a zero or replayed invocation, so the
    /// normalized identity is always established.
    pub fn invocation(&self) -> ProgramLocalRootEntryInvocationId {
        ProgramLocalRootEntryInvocationId::from_normalized_identity(
            self.entry.invocation_identity(),
        )
        .expect("a linearized component-era activation carries a nonzero invocation identity")
    }
}

/// Rejection of [`ProgramLocalEntryActivation::leave`]: the activation and its
/// leave receipt are returned so the real scope is never silently dropped.
#[derive(Debug)]
pub struct ProgramLocalEntryActivationLeaveError {
    activation: ProgramLocalEntryActivation,
    receipt: ComponentEraLeaveReceipt,
    diagnostic: ExternalRootDiagnostic,
}

impl ProgramLocalEntryActivationLeaveError {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ProgramLocalEntryActivation, ComponentEraLeaveReceipt) {
        (self.activation, self.receipt)
    }
}
