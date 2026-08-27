//! Deployment custody across reclaimable callback registration.
//!
//! The first phase installs an independently admitted external root and keeps
//! it pending. The registrar then remains an ordinary runtime boundary call
//! outside this module. The second phase consumes its provider result and
//! keeps registration and ledger custody together until explicit
//! unregister-and-quiesce completion. This module does not claim that an
//! emitted callback-address store has been attributed to an installed entry.

use crate::TerminalComponentDeploymentSession;
use omega_external_roots::{
    CompletedOpaqueCallbackUnregistration, InstalledExternalRoot, InstalledRootLedger,
    OpaqueCallbackRegistrationReceipt, OpaqueCallbackUnregistrationReceipt, ProviderExecution,
    ReclaimableOpaqueCallback, RootAdmission, RootAdmissionId, RootRemovalReceipt,
    RootSlotAuthority, ValidatedExternalRoot, admit_reclaimable_opaque_callback,
};

/// Complete non-clonable inputs for installing one callback root before the
/// registrar executes. Provider registration evidence is deliberately a
/// separate second phase because its constructor binds this installed root.
#[derive(Debug)]
#[must_use = "callback-root deployment inputs retain root and slot authority"]
pub struct ReclaimableCallbackRootDeployment {
    admission_identity: RootAdmissionId,
    root: ValidatedExternalRoot,
    provider_execution: ProviderExecution,
    slot: RootSlotAuthority,
}

impl ReclaimableCallbackRootDeployment {
    pub fn new(
        admission_identity: RootAdmissionId,
        root: ValidatedExternalRoot,
        provider_execution: ProviderExecution,
        slot: RootSlotAuthority,
    ) -> Self {
        Self {
            admission_identity,
            root,
            provider_execution,
            slot,
        }
    }

    pub const fn admission_identity(&self) -> RootAdmissionId {
        self.admission_identity
    }

    pub const fn root(&self) -> &ValidatedExternalRoot {
        &self.root
    }

    pub const fn provider_execution(&self) -> &ProviderExecution {
        &self.provider_execution
    }

    pub const fn slot(&self) -> &RootSlotAuthority {
        &self.slot
    }

    pub fn into_parts(
        self,
    ) -> (
        RootAdmissionId,
        ValidatedExternalRoot,
        ProviderExecution,
        RootSlotAuthority,
    ) {
        (
            self.admission_identity,
            self.root,
            self.provider_execution,
            self.slot,
        )
    }
}

impl TerminalComponentDeploymentSession {
    /// Install an independently admitted reclaimable callback root and retain
    /// it pending the provider's exact registrar-result receipt.
    ///
    /// Rejection returns every caller-owned input. Success borrows the
    /// deployment's installed code and ledger together, preventing component
    /// finalization while registration custody is unresolved.
    pub fn install_reclaimable_callback_root<'deployment>(
        &'deployment mut self,
        input: ReclaimableCallbackRootDeployment,
    ) -> Result<PendingReclaimableCallbackRegistration<'deployment>, CallbackRootDeploymentError>
    {
        let ReclaimableCallbackRootDeployment {
            admission_identity,
            root,
            provider_execution,
            slot,
        } = input;

        let admission = match RootAdmission::from_admitted_provider(
            admission_identity,
            &root,
            &provider_execution,
            &self.installed,
            &slot,
            root.candidate().trust_receipts.iter().copied(),
        ) {
            Ok(admission) => admission,
            Err(diagnostic) => {
                return Err(CallbackRootDeploymentError {
                    input: ReclaimableCallbackRootDeployment {
                        admission_identity,
                        root,
                        provider_execution,
                        slot,
                    },
                    diagnostic: diagnostic.0,
                });
            }
        };

        let installed = &self.installed;
        let roots = &mut self.roots;
        let root = match roots.install(installed, root, slot, admission) {
            Ok(root) => root,
            Err(error) => {
                let diagnostic = error.diagnostic().0.clone();
                let (root, slot, _admission) = (*error).into_parts();
                return Err(CallbackRootDeploymentError {
                    input: ReclaimableCallbackRootDeployment {
                        admission_identity,
                        root,
                        provider_execution,
                        slot,
                    },
                    diagnostic,
                });
            }
        };

        Ok(PendingReclaimableCallbackRegistration { root, roots })
    }
}

/// Installed external root held pending the provider's registrar-result
/// receipt. This is not yet a durable callback registration.
#[derive(Debug)]
#[must_use = "an installed callback root must be admitted or explicitly removed"]
pub struct PendingReclaimableCallbackRegistration<'deployment> {
    root: InstalledExternalRoot<'deployment>,
    roots: &'deployment mut InstalledRootLedger,
}

impl<'deployment> PendingReclaimableCallbackRegistration<'deployment> {
    pub const fn root(&self) -> &InstalledExternalRoot<'deployment> {
        &self.root
    }

    pub const fn roots(&self) -> &InstalledRootLedger {
        self.roots
    }

    /// Consume the exact provider result. A false or substituted receipt
    /// returns the installed pending root and receipt intact for correction.
    pub fn admit_registration(
        self,
        receipt: OpaqueCallbackRegistrationReceipt,
    ) -> Result<
        InstalledReclaimableCallback<'deployment>,
        CallbackRegistrationAdmissionError<'deployment>,
    > {
        let Self { root, roots } = self;
        match admit_reclaimable_opaque_callback(root, receipt) {
            Ok(registration) => Ok(InstalledReclaimableCallback {
                registration,
                roots,
            }),
            Err(error) => {
                let diagnostic = error.diagnostic().0.clone();
                let (root, receipt) = (*error).into_parts();
                Err(CallbackRegistrationAdmissionError {
                    pending: PendingReclaimableCallbackRegistration { root, roots },
                    receipt,
                    diagnostic,
                })
            }
        }
    }

    /// Recover the still-installed root and ledger borrow for an explicit
    /// non-registration cleanup path owned by the external-root layer.
    pub fn into_parts(
        self,
    ) -> (
        &'deployment mut InstalledRootLedger,
        InstalledExternalRoot<'deployment>,
    ) {
        (self.roots, self.root)
    }

    /// Remove a pending root when the registrar rejects registration. A
    /// failed removal returns the complete pending carrier and receipt.
    pub fn remove(
        self,
        receipt: RootRemovalReceipt,
    ) -> Result<RootSlotAuthority, Box<PendingCallbackRootRemovalError<'deployment>>> {
        let Self { root, roots } = self;
        match roots.remove(root, receipt) {
            Ok(slot) => Ok(slot),
            Err(error) => {
                let diagnostic = error.diagnostic().0.clone();
                let (root, receipt) = (*error).into_parts();
                Err(Box::new(PendingCallbackRootRemovalError {
                    pending: PendingReclaimableCallbackRegistration { root, roots },
                    receipt,
                    diagnostic,
                }))
            }
        }
    }
}

/// Live reclaimable callback registration and the ledger needed for its only
/// successful terminal operation.
#[derive(Debug)]
#[must_use = "a registered callback remains live until unregister and quiescence complete"]
pub struct InstalledReclaimableCallback<'deployment> {
    registration: ReclaimableOpaqueCallback<'deployment>,
    roots: &'deployment mut InstalledRootLedger,
}

impl<'deployment> InstalledReclaimableCallback<'deployment> {
    pub const fn registration(&self) -> &ReclaimableOpaqueCallback<'deployment> {
        &self.registration
    }

    pub const fn roots(&self) -> &InstalledRootLedger {
        self.roots
    }

    /// Complete provider unregister and exact root quiescence as one
    /// transactional transition. Every live value returns on rejection.
    pub fn unregister_and_quiesce(
        self,
        provider_receipt: OpaqueCallbackUnregistrationReceipt,
        root_removal_receipt: RootRemovalReceipt,
    ) -> Result<CompletedOpaqueCallbackUnregistration, Box<CallbackUnregistrationError<'deployment>>>
    {
        let Self {
            registration,
            roots,
        } = self;
        match registration.unregister_and_quiesce(roots, provider_receipt, root_removal_receipt) {
            Ok(completed) => Ok(completed),
            Err(error) => {
                let diagnostic = error.diagnostic().0.clone();
                let (registration, provider_receipt, root_removal_receipt) = (*error).into_parts();
                Err(Box::new(CallbackUnregistrationError {
                    installed: InstalledReclaimableCallback {
                        registration,
                        roots,
                    },
                    provider_receipt,
                    root_removal_receipt,
                    diagnostic,
                }))
            }
        }
    }
}

/// Root-installation rejection with complete retry custody.
#[derive(Debug)]
pub struct CallbackRootDeploymentError {
    input: ReclaimableCallbackRootDeployment,
    diagnostic: String,
}

impl CallbackRootDeploymentError {
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }

    pub fn into_input(self) -> ReclaimableCallbackRootDeployment {
        self.input
    }
}

impl std::fmt::Display for CallbackRootDeploymentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for CallbackRootDeploymentError {}

/// Provider-result rejection retaining the installed root and receipt.
#[derive(Debug)]
pub struct CallbackRegistrationAdmissionError<'deployment> {
    pending: PendingReclaimableCallbackRegistration<'deployment>,
    receipt: OpaqueCallbackRegistrationReceipt,
    diagnostic: String,
}

impl<'deployment> CallbackRegistrationAdmissionError<'deployment> {
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        PendingReclaimableCallbackRegistration<'deployment>,
        OpaqueCallbackRegistrationReceipt,
    ) {
        (self.pending, self.receipt)
    }
}

impl std::fmt::Display for CallbackRegistrationAdmissionError<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for CallbackRegistrationAdmissionError<'_> {}

/// Failed cleanup of a callback root for which registration was not admitted.
#[derive(Debug)]
pub struct PendingCallbackRootRemovalError<'deployment> {
    pending: PendingReclaimableCallbackRegistration<'deployment>,
    receipt: RootRemovalReceipt,
    diagnostic: String,
}

impl<'deployment> PendingCallbackRootRemovalError<'deployment> {
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        PendingReclaimableCallbackRegistration<'deployment>,
        RootRemovalReceipt,
    ) {
        (self.pending, self.receipt)
    }
}

impl std::fmt::Display for PendingCallbackRootRemovalError<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for PendingCallbackRootRemovalError<'_> {}

/// Unregister/quiescence rejection retaining registration and both receipts.
#[derive(Debug)]
pub struct CallbackUnregistrationError<'deployment> {
    installed: InstalledReclaimableCallback<'deployment>,
    provider_receipt: OpaqueCallbackUnregistrationReceipt,
    root_removal_receipt: RootRemovalReceipt,
    diagnostic: String,
}

impl<'deployment> CallbackUnregistrationError<'deployment> {
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        InstalledReclaimableCallback<'deployment>,
        OpaqueCallbackUnregistrationReceipt,
        RootRemovalReceipt,
    ) {
        (
            self.installed,
            self.provider_receipt,
            self.root_removal_receipt,
        )
    }
}

impl std::fmt::Display for CallbackUnregistrationError<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for CallbackUnregistrationError<'_> {}
