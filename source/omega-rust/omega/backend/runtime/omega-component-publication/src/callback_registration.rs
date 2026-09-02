use omega_executable_installation::InstalledCode;
use omega_external_roots::{
    CompletedOpaqueCallbackUnregistration, ExternalRootDiagnostic, InstalledExternalRoot,
    InstalledRootLedger, OpaqueCallbackRegistrationCapacityOccurrence,
    OpaqueCallbackRegistrationReceipt, OpaqueCallbackUnregistrationReceipt,
    ReclaimableOpaqueCallback, RootAdmission, RootInstallError, RootRemovalReceipt,
    RootSlotAuthority, ValidatedExternalRoot, admit_reclaimable_opaque_callback,
};
use omega_image_emission::InstalledCompilerPrivateFunctionEntry;

/// Split borrow of one runnable component's installed code and root ledger.
/// The split is required because an installed root pins the code while
/// unregister and quiescence must continue mutating the independent ledger.
#[derive(Debug)]
#[must_use = "runtime root custody must be retained while installed roots are live"]
pub struct InstalledRunnableExternalRootRuntime<'code> {
    pub(crate) installed: &'code InstalledCode,
    pub(crate) roots: &'code mut InstalledRootLedger,
}

impl<'code> InstalledRunnableExternalRootRuntime<'code> {
    pub fn install(
        &mut self,
        root: ValidatedExternalRoot,
        slot: RootSlotAuthority,
        admission: RootAdmission,
    ) -> Result<InstalledExternalRoot<'code>, Box<RootInstallError>> {
        self.roots.install(self.installed, root, slot, admission)
    }

    /// Join compiler-private entry attribution to an already-installed root
    /// and an exact successful provider registration. Rejection returns every
    /// consumed input for correction and retry.
    pub fn admit_compiler_private_callback(
        &self,
        attribution: InstalledCompilerPrivateFunctionEntry,
        root: InstalledExternalRoot<'code>,
        receipt: OpaqueCallbackRegistrationReceipt,
        capacity: OpaqueCallbackRegistrationCapacityOccurrence,
    ) -> Result<
        RegisteredCompilerPrivateCallback<'code>,
        Box<RegisteredCompilerPrivateCallbackAdmissionError<'code>>,
    > {
        if !attribution.binds_installed_code(self.installed)
            || !root.binds_installed_entry(attribution.installed_context(), attribution.entry())
        {
            return Err(Box::new(RegisteredCompilerPrivateCallbackAdmissionError {
                attribution,
                root,
                receipt,
                capacity,
                diagnostic: ExternalRootDiagnostic(
                    "compiler-private callback attribution does not bind the exact installed root entry and code occurrence"
                        .into(),
                ),
            }));
        }

        match admit_reclaimable_opaque_callback(root, receipt, capacity) {
            Ok(registration) => Ok(RegisteredCompilerPrivateCallback {
                attribution,
                registration,
            }),
            Err(error) => {
                let (root, receipt, capacity) = (*error).into_parts();
                Err(Box::new(RegisteredCompilerPrivateCallbackAdmissionError {
                    attribution,
                    root,
                    receipt,
                    capacity,
                    diagnostic: ExternalRootDiagnostic(
                        "provider callback result does not establish the exact registered compiler-private root"
                            .into(),
                    ),
                }))
            }
        }
    }
}

/// Linear runtime registration for one compiler-private installed entry.
/// Attribution remains beside the provider registration until unregister and
/// exact root quiescence both succeed.
#[derive(Debug)]
#[must_use = "registered callback custody must be retained through unregister and quiescence"]
pub struct RegisteredCompilerPrivateCallback<'code> {
    attribution: InstalledCompilerPrivateFunctionEntry,
    registration: ReclaimableOpaqueCallback<'code>,
}

impl RegisteredCompilerPrivateCallback<'_> {
    pub const fn attribution(&self) -> &InstalledCompilerPrivateFunctionEntry {
        &self.attribution
    }

    pub const fn registration(&self) -> &ReclaimableOpaqueCallback<'_> {
        &self.registration
    }
}

impl<'code> RegisteredCompilerPrivateCallback<'code> {
    pub fn unregister_and_quiesce(
        self,
        runtime: &mut InstalledRunnableExternalRootRuntime<'code>,
        provider_receipt: OpaqueCallbackUnregistrationReceipt,
        root_removal_receipt: RootRemovalReceipt,
    ) -> Result<
        CompletedRegisteredCompilerPrivateCallback,
        Box<RegisteredCompilerPrivateCallbackUnregistrationError<'code>>,
    > {
        let Self {
            attribution,
            registration,
        } = self;
        match registration.unregister_and_quiesce(
            runtime.roots,
            provider_receipt,
            root_removal_receipt,
        ) {
            Ok(completion) => Ok(CompletedRegisteredCompilerPrivateCallback {
                attribution,
                completion,
            }),
            Err(error) => {
                let (registration, provider_receipt, root_removal_receipt) = (*error).into_parts();
                Err(Box::new(
                    RegisteredCompilerPrivateCallbackUnregistrationError {
                        registration: RegisteredCompilerPrivateCallback {
                            attribution,
                            registration,
                        },
                        provider_receipt,
                        root_removal_receipt,
                        diagnostic: ExternalRootDiagnostic(
                            "compiler-private callback remains live until provider unregistration and exact root quiescence both succeed"
                                .into(),
                        ),
                    },
                ))
            }
        }
    }
}

#[derive(Debug)]
pub struct RegisteredCompilerPrivateCallbackAdmissionError<'code> {
    attribution: InstalledCompilerPrivateFunctionEntry,
    root: InstalledExternalRoot<'code>,
    receipt: OpaqueCallbackRegistrationReceipt,
    capacity: OpaqueCallbackRegistrationCapacityOccurrence,
    diagnostic: ExternalRootDiagnostic,
}

impl<'code> RegisteredCompilerPrivateCallbackAdmissionError<'code> {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        InstalledCompilerPrivateFunctionEntry,
        InstalledExternalRoot<'code>,
        OpaqueCallbackRegistrationReceipt,
        OpaqueCallbackRegistrationCapacityOccurrence,
    ) {
        (self.attribution, self.root, self.receipt, self.capacity)
    }
}

impl std::fmt::Display for RegisteredCompilerPrivateCallbackAdmissionError<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for RegisteredCompilerPrivateCallbackAdmissionError<'_> {}

#[derive(Debug)]
pub struct RegisteredCompilerPrivateCallbackUnregistrationError<'code> {
    registration: RegisteredCompilerPrivateCallback<'code>,
    provider_receipt: OpaqueCallbackUnregistrationReceipt,
    root_removal_receipt: RootRemovalReceipt,
    diagnostic: ExternalRootDiagnostic,
}

impl<'code> RegisteredCompilerPrivateCallbackUnregistrationError<'code> {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        RegisteredCompilerPrivateCallback<'code>,
        OpaqueCallbackUnregistrationReceipt,
        RootRemovalReceipt,
    ) {
        (
            self.registration,
            self.provider_receipt,
            self.root_removal_receipt,
        )
    }
}

impl std::fmt::Display for RegisteredCompilerPrivateCallbackUnregistrationError<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for RegisteredCompilerPrivateCallbackUnregistrationError<'_> {}

#[derive(Debug)]
#[must_use = "completed callback custody returns attribution and provider/root completion"]
pub struct CompletedRegisteredCompilerPrivateCallback {
    attribution: InstalledCompilerPrivateFunctionEntry,
    completion: CompletedOpaqueCallbackUnregistration,
}

impl CompletedRegisteredCompilerPrivateCallback {
    pub const fn attribution(&self) -> &InstalledCompilerPrivateFunctionEntry {
        &self.attribution
    }

    pub const fn completion(&self) -> &CompletedOpaqueCallbackUnregistration {
        &self.completion
    }

    pub fn into_parts(
        self,
    ) -> (
        InstalledCompilerPrivateFunctionEntry,
        CompletedOpaqueCallbackUnregistration,
    ) {
        (self.attribution, self.completion)
    }
}
