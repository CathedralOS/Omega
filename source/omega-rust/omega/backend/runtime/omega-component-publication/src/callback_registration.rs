use omega_executable_installation::InstalledCode;
use omega_external_roots::{
    CompletedOpaqueCallbackUnregistration, ExternalRootDiagnostic, InstalledExternalRoot,
    InstalledRootLedger, OpaqueCallbackRegistrationCapacityOccurrence,
    OpaqueCallbackRegistrationReceipt, OpaqueCallbackUnregistrationReceipt,
    ReclaimableOpaqueCallback, RootAdmission, RootInstallError, RootRemovalReceipt,
    RootSlotAuthority, ValidatedExternalRoot, admit_reclaimable_opaque_callback,
};
use omega_image_emission::InstalledCompilerPrivateFunctionEntry;

use crate::RunnableComponentEraLedger;
use omega_effects::{
    ComponentEraEntryLedger, ProgramLocalRootEpochLease,
    ProgramLocalRootEpochLeaseAcquisitionError, ProgramLocalRootEpochLeaseId,
};

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
        admit_compiler_private_callback(self.installed, attribution, root, receipt, capacity)
    }
}

fn admit_compiler_private_callback<'code>(
    installed: &'code InstalledCode,
    attribution: InstalledCompilerPrivateFunctionEntry,
    root: InstalledExternalRoot<'code>,
    receipt: OpaqueCallbackRegistrationReceipt,
    capacity: OpaqueCallbackRegistrationCapacityOccurrence,
) -> Result<
    RegisteredCompilerPrivateCallback<'code>,
    Box<RegisteredCompilerPrivateCallbackAdmissionError<'code>>,
> {
    if !attribution.binds_installed_code(installed)
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

/// Split custody for callback registration in one retained component era.
///
/// This is the only bridge that can lower a provider registration to the
/// package-visible linear [`Registration`]: it retains the exact installed
/// occurrence, its root ledger, and the lifecycle ledger that issued the
/// component-era lease.
#[derive(Debug)]
#[must_use = "component-era callback runtime custody must be retained while registrations are live"]
pub struct RunnableComponentCallbackRegistrationRuntime<'component> {
    era_identity: u64,
    installed: &'component InstalledCode,
    roots: &'component mut InstalledRootLedger,
    lifecycle: &'component mut ComponentEraEntryLedger,
}

impl RunnableComponentEraLedger {
    /// Borrow one retained component and its lifecycle as disjoint callback
    /// registration custody.
    pub fn callback_registration_runtime(
        &mut self,
        era_identity: u64,
    ) -> Option<RunnableComponentCallbackRegistrationRuntime<'_>> {
        let runnable = self.runnable.get_mut(&era_identity)?;
        Some(RunnableComponentCallbackRegistrationRuntime {
            era_identity,
            installed: runnable.artifact.installed(),
            roots: &mut runnable.roots,
            lifecycle: &mut self.lifecycle,
        })
    }
}

impl<'component> RunnableComponentCallbackRegistrationRuntime<'component> {
    pub const fn era_identity(&self) -> u64 {
        self.era_identity
    }

    pub const fn installed(&self) -> &InstalledCode {
        self.installed
    }

    pub fn component_era_lease_holds(&self) -> Option<usize> {
        self.lifecycle
            .program_local_root_authority_holds(self.era_identity)
    }

    pub fn install(
        &mut self,
        root: ValidatedExternalRoot,
        slot: RootSlotAuthority,
        admission: RootAdmission,
    ) -> Result<InstalledExternalRoot<'component>, Box<RootInstallError>> {
        self.roots.install(self.installed, root, slot, admission)
    }

    pub fn admit_compiler_private_callback(
        &self,
        attribution: InstalledCompilerPrivateFunctionEntry,
        root: InstalledExternalRoot<'component>,
        receipt: OpaqueCallbackRegistrationReceipt,
        capacity: OpaqueCallbackRegistrationCapacityOccurrence,
    ) -> Result<
        RegisteredCompilerPrivateCallback<'component>,
        Box<RegisteredCompilerPrivateCallbackAdmissionError<'component>>,
    > {
        admit_compiler_private_callback(self.installed, attribution, root, receipt, capacity)
    }

    /// Acquire the exact lifecycle hold which a subsequently lowered
    /// registration will own. The runtime supplies its retained era and entry
    /// contract; callers choose only the globally fresh linear identity.
    pub fn acquire_registration_lease(
        &mut self,
        identity: ProgramLocalRootEpochLeaseId,
    ) -> Result<ProgramLocalRootEpochLease, ProgramLocalRootEpochLeaseAcquisitionError> {
        let entry_contract_identity = self.lifecycle.entry_contract_identity().to_owned();
        self.lifecycle.acquire_program_local_root_epoch_lease(
            identity,
            self.era_identity,
            &entry_contract_identity,
        )
    }

    /// Join the live callback to the exact current component-era lease and
    /// lower the package-visible linear registration. Rejection returns both
    /// linear inputs unchanged.
    pub fn lower_registration(
        &self,
        registered: RegisteredCompilerPrivateCallback<'component>,
        lease: ProgramLocalRootEpochLease,
    ) -> Result<Registration<'component>, Box<RegistrationLoweringError<'component>>> {
        let exact = lease.era_identity() == self.era_identity
            && lease.artifact_occurrence_digest() == self.installed.occurrence_digest()
            && lease.artifact_instance_compatibility_report_identity()
                == self.installed.identity().normalized_identity()
            && registered.attribution.binds_installed_code(self.installed)
            && self
                .lifecycle
                .validate_program_local_root_epoch_lease(&lease)
                .is_ok();
        if !exact {
            return Err(Box::new(RegistrationLoweringError {
                registered,
                lease,
                diagnostic: ExternalRootDiagnostic(
                    "callback registration and component-era lease do not bind the exact retained installed occurrence"
                        .into(),
                ),
            }));
        }
        Ok(Registration { registered, lease })
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
        self.unregister_and_quiesce_in_ledger(runtime.roots, provider_receipt, root_removal_receipt)
    }

    fn unregister_and_quiesce_in_ledger(
        self,
        roots: &mut InstalledRootLedger,
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
        match registration.unregister_and_quiesce(roots, provider_receipt, root_removal_receipt) {
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

/// Package-visible linear callback registration.
///
/// The carrier owns both the live foreign registration and its exact
/// component-era hold. There is no operation that releases the hold while the
/// callback is live: successful provider unregister and root quiescence first
/// advance this value to [`UnregisteredRegistration`].
#[derive(Debug)]
#[must_use = "a live callback registration must be unregistered and its component-era lease released"]
pub struct Registration<'code> {
    registered: RegisteredCompilerPrivateCallback<'code>,
    lease: ProgramLocalRootEpochLease,
}

impl Registration<'_> {
    pub const fn attribution(&self) -> &InstalledCompilerPrivateFunctionEntry {
        self.registered.attribution()
    }

    pub const fn registration(&self) -> &ReclaimableOpaqueCallback<'_> {
        self.registered.registration()
    }

    pub const fn component_era_identity(&self) -> u64 {
        self.lease.era_identity()
    }

    pub const fn component_era_lease_identity(&self) -> ProgramLocalRootEpochLeaseId {
        self.lease.identity()
    }
}

impl<'code> Registration<'code> {
    /// End the foreign root while retaining the component-era hold and all
    /// returned provider/root authority in an opaque intermediate state.
    /// Failure reconstructs the complete live registration for retry.
    pub fn unregister_and_quiesce(
        self,
        runtime: &mut RunnableComponentCallbackRegistrationRuntime<'code>,
        provider_receipt: OpaqueCallbackUnregistrationReceipt,
        root_removal_receipt: RootRemovalReceipt,
    ) -> Result<UnregisteredRegistration, Box<RegistrationUnregistrationError<'code>>> {
        let Self { registered, lease } = self;
        match registered.unregister_and_quiesce_in_ledger(
            runtime.roots,
            provider_receipt,
            root_removal_receipt,
        ) {
            Ok(completed) => Ok(UnregisteredRegistration { completed, lease }),
            Err(error) => {
                let (registered, provider_receipt, root_removal_receipt) = (*error).into_parts();
                Err(Box::new(RegistrationUnregistrationError {
                    registration: Registration { registered, lease },
                    provider_receipt,
                    root_removal_receipt,
                    diagnostic: ExternalRootDiagnostic(
                        "linear registration retains its exact component-era lease until callback unregister and root quiescence succeed"
                            .into(),
                    ),
                }))
            }
        }
    }
}

/// Retry-safe state after the foreign root has ended but before the exact
/// component-era hold has been returned. Provider capacity and root-slot
/// authority remain inaccessible until release succeeds.
#[derive(Debug)]
#[must_use = "the component-era lease must be released to complete callback unregistration"]
pub struct UnregisteredRegistration {
    completed: CompletedRegisteredCompilerPrivateCallback,
    lease: ProgramLocalRootEpochLease,
}

impl UnregisteredRegistration {
    pub const fn attribution(&self) -> &InstalledCompilerPrivateFunctionEntry {
        self.completed.attribution()
    }

    pub const fn component_era_identity(&self) -> u64 {
        self.lease.era_identity()
    }

    pub const fn component_era_lease_identity(&self) -> ProgramLocalRootEpochLeaseId {
        self.lease.identity()
    }

    /// Release the exact component-era hold. Rejection keeps the completed
    /// callback, returned capacity, root slot, and lease together for retry.
    pub fn release_component_era(
        self,
        runtime: &mut RunnableComponentCallbackRegistrationRuntime<'_>,
    ) -> Result<CompletedRegistration, Box<RegistrationLeaseReleaseError>> {
        let Self { completed, lease } = self;
        if runtime.era_identity != lease.era_identity()
            || runtime.installed.occurrence_digest() != lease.artifact_occurrence_digest()
            || runtime.installed.identity().normalized_identity()
                != lease.artifact_instance_compatibility_report_identity()
        {
            return Err(Box::new(RegistrationLeaseReleaseError {
                registration: UnregisteredRegistration { completed, lease },
                diagnostic: ExternalRootDiagnostic(
                    "completed callback can release its lease only through the exact retained component era"
                        .into(),
                ),
            }));
        }
        match runtime
            .lifecycle
            .release_program_local_root_epoch_lease(lease)
        {
            Ok(()) => Ok(CompletedRegistration { completed }),
            Err(error) => Err(Box::new(RegistrationLeaseReleaseError {
                registration: UnregisteredRegistration {
                    completed,
                    lease: error.into_lease(),
                },
                diagnostic: ExternalRootDiagnostic(
                    "exact component-era lease release failed; completed callback custody remains intact for retry"
                        .into(),
                ),
            })),
        }
    }
}

/// Callback completion exposed only after the exact component-era lease has
/// been released. Decomposition returns the original attribution and the exact
/// root-slot/capacity completion.
#[derive(Debug)]
#[must_use = "completed registration custody returns exact attribution and provider/root authority"]
pub struct CompletedRegistration {
    completed: CompletedRegisteredCompilerPrivateCallback,
}

impl CompletedRegistration {
    pub const fn attribution(&self) -> &InstalledCompilerPrivateFunctionEntry {
        self.completed.attribution()
    }

    pub const fn completion(&self) -> &CompletedOpaqueCallbackUnregistration {
        self.completed.completion()
    }

    pub fn into_parts(
        self,
    ) -> (
        InstalledCompilerPrivateFunctionEntry,
        CompletedOpaqueCallbackUnregistration,
    ) {
        self.completed.into_parts()
    }
}

#[derive(Debug)]
pub struct RegistrationLoweringError<'code> {
    registered: RegisteredCompilerPrivateCallback<'code>,
    lease: ProgramLocalRootEpochLease,
    diagnostic: ExternalRootDiagnostic,
}

impl<'code> RegistrationLoweringError<'code> {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        RegisteredCompilerPrivateCallback<'code>,
        ProgramLocalRootEpochLease,
    ) {
        (self.registered, self.lease)
    }
}

impl std::fmt::Display for RegistrationLoweringError<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for RegistrationLoweringError<'_> {}

#[derive(Debug)]
pub struct RegistrationUnregistrationError<'code> {
    registration: Registration<'code>,
    provider_receipt: OpaqueCallbackUnregistrationReceipt,
    root_removal_receipt: RootRemovalReceipt,
    diagnostic: ExternalRootDiagnostic,
}

impl<'code> RegistrationUnregistrationError<'code> {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        Registration<'code>,
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

impl std::fmt::Display for RegistrationUnregistrationError<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for RegistrationUnregistrationError<'_> {}

#[derive(Debug)]
pub struct RegistrationLeaseReleaseError {
    registration: UnregisteredRegistration,
    diagnostic: ExternalRootDiagnostic,
}

impl RegistrationLeaseReleaseError {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_registration(self) -> UnregisteredRegistration {
        self.registration
    }
}

impl std::fmt::Display for RegistrationLeaseReleaseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for RegistrationLeaseReleaseError {}

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
