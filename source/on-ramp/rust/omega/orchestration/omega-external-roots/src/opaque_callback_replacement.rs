use super::*;

/// Provider evidence that one opaque callback address is a process-lifetime
/// gateway rather than an entry embedded in replaceable component code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessLifetimeGatewayAdmissionReceipt {
    identity: GatewayAdmissionReceiptId,
    registration: OpaqueCallbackRegistrationId,
    provider: OpaqueCallbackProviderId,
    gateway: ProcessLifetimeGatewayId,
    dispatch_contract: GatewayDispatchContractId,
    installed_code: InstalledCodeContext,
    installed_code_identity: InstalledCodeId,
    entry: EntryStubId,
    foreign_target_is_gateway: bool,
    gateway_is_process_lifetime: bool,
    dispatches_current_era: bool,
}

impl ProcessLifetimeGatewayAdmissionReceipt {
    #[allow(clippy::too_many_arguments)]
    pub fn from_provider(
        identity: GatewayAdmissionReceiptId,
        registration: OpaqueCallbackRegistrationId,
        provider: OpaqueCallbackProviderId,
        gateway: ProcessLifetimeGatewayId,
        dispatch_contract: GatewayDispatchContractId,
        installed_code: &InstalledCode,
        entry: EntryStubId,
        foreign_target_is_gateway: bool,
        gateway_is_process_lifetime: bool,
        dispatches_current_era: bool,
    ) -> Self {
        Self {
            identity,
            registration,
            provider,
            gateway,
            dispatch_contract,
            installed_code: installed_code.receipt_context(),
            installed_code_identity: installed_code.identity(),
            entry,
            foreign_target_is_gateway,
            gateway_is_process_lifetime,
            dispatches_current_era,
        }
    }
}

/// Accepted opaque callback whose foreign address remains in process-lifetime
/// code. The installed-code borrow deliberately has no retirement operation.
#[derive(Debug)]
pub struct ProcessLifetimeOpaqueCallback<'code> {
    registration: OpaqueCallbackRegistrationId,
    provider: OpaqueCallbackProviderId,
    gateway: ProcessLifetimeGatewayId,
    dispatch_contract: GatewayDispatchContractId,
    admission: GatewayAdmissionReceiptId,
    entry: EntryStubId,
    installed_code: &'code InstalledCode,
}

impl ProcessLifetimeOpaqueCallback<'_> {
    pub const fn registration(&self) -> OpaqueCallbackRegistrationId {
        self.registration
    }

    pub const fn provider(&self) -> OpaqueCallbackProviderId {
        self.provider
    }

    pub const fn gateway(&self) -> ProcessLifetimeGatewayId {
        self.gateway
    }

    pub const fn dispatch_contract(&self) -> GatewayDispatchContractId {
        self.dispatch_contract
    }

    pub const fn admission(&self) -> GatewayAdmissionReceiptId {
        self.admission
    }

    pub const fn entry(&self) -> EntryStubId {
        self.entry
    }

    pub const fn installed_code(&self) -> InstalledCodeId {
        self.installed_code.identity()
    }
}

pub fn admit_process_lifetime_opaque_callback<'code>(
    installed_code: &'code InstalledCode,
    receipt: ProcessLifetimeGatewayAdmissionReceipt,
) -> Result<ProcessLifetimeOpaqueCallback<'code>, Box<ProcessLifetimeGatewayAdmissionError>> {
    let exact_code = receipt.installed_code == installed_code.receipt_context()
        && receipt.installed_code_identity == installed_code.identity();
    let entry_admitted = installed_code.selected_entry_target(receipt.entry).is_ok();
    let diagnostic = if !exact_code {
        Some("process-lifetime callback gateway receipt does not bind the exact installed code")
    } else if !entry_admitted {
        Some("process-lifetime callback gateway entry is not admitted by the installed artifact")
    } else if !receipt.foreign_target_is_gateway {
        Some("opaque callback foreign target is not the admitted gateway entry")
    } else if !receipt.gateway_is_process_lifetime {
        Some("opaque callback gateway is not retained for process lifetime")
    } else if !receipt.dispatches_current_era {
        Some("opaque callback gateway does not dispatch through the current-era contract")
    } else {
        None
    };
    if let Some(diagnostic) = diagnostic {
        return Err(Box::new(ProcessLifetimeGatewayAdmissionError {
            receipt,
            diagnostic: ExternalRootDiagnostic(diagnostic.into()),
        }));
    }
    Ok(ProcessLifetimeOpaqueCallback {
        registration: receipt.registration,
        provider: receipt.provider,
        gateway: receipt.gateway,
        dispatch_contract: receipt.dispatch_contract,
        admission: receipt.identity,
        entry: receipt.entry,
        installed_code,
    })
}

#[derive(Debug)]
pub struct ProcessLifetimeGatewayAdmissionError {
    receipt: ProcessLifetimeGatewayAdmissionReceipt,
    diagnostic: ExternalRootDiagnostic,
}

impl ProcessLifetimeGatewayAdmissionError {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_receipt(self) -> ProcessLifetimeGatewayAdmissionReceipt {
        self.receipt
    }
}

/// Provider evidence that an opaque provider accepted an unregister contract
/// for one exact reclaimable external root using one exact live-registration
/// capacity occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpaqueCallbackRegistrationReceipt {
    identity: OpaqueCallbackRegistrationReceiptId,
    registration: OpaqueCallbackRegistrationId,
    provider: OpaqueCallbackProviderId,
    capacity: OpaqueCallbackRegistrationCapacityOccurrenceId,
    unregistration_contract: OpaqueCallbackUnregistrationContractId,
    installed_root: InstalledRootEvidence,
    callback_registered: bool,
}

impl OpaqueCallbackRegistrationReceipt {
    pub fn from_provider(
        identity: OpaqueCallbackRegistrationReceiptId,
        registration: OpaqueCallbackRegistrationId,
        provider: OpaqueCallbackProviderId,
        unregistration_contract: OpaqueCallbackUnregistrationContractId,
        root: &InstalledExternalRoot<'_>,
        capacity: &OpaqueCallbackRegistrationCapacityOccurrence,
        callback_registered: bool,
    ) -> Self {
        Self {
            identity,
            registration,
            provider,
            capacity: capacity.identity,
            unregistration_contract,
            installed_root: root.evidence.clone(),
            callback_registered,
        }
    }
}

/// One exact provider-owned unit of live callback-registration capacity.
///
/// This occurrence is neither a lifetime budget nor a count of emitted callback
/// thunks. It moves into one successful runtime registration and returns only
/// after that exact registration is unregistered and its external root is
/// quiescent. It is deliberately non-clonable.
#[derive(Debug)]
#[must_use = "live-registration capacity must be retained or transferred into a registration"]
pub struct OpaqueCallbackRegistrationCapacityOccurrence {
    identity: OpaqueCallbackRegistrationCapacityOccurrenceId,
    provider: OpaqueCallbackProviderId,
}

impl OpaqueCallbackRegistrationCapacityOccurrence {
    pub const fn from_provider(
        identity: OpaqueCallbackRegistrationCapacityOccurrenceId,
        provider: OpaqueCallbackProviderId,
    ) -> Self {
        Self { identity, provider }
    }

    pub const fn identity(&self) -> OpaqueCallbackRegistrationCapacityOccurrenceId {
        self.identity
    }

    pub const fn provider(&self) -> OpaqueCallbackProviderId {
        self.provider
    }
}

/// Linear registration path for an opaque callback that targets replaceable
/// code directly. Reclamation can occur only by consuming this value through
/// both provider unregistration and the existing exact-root quiescence gate.
#[derive(Debug)]
#[must_use = "a reclaimable callback retains registration capacity until unregister and quiescence complete"]
pub struct ReclaimableOpaqueCallback<'code> {
    registration: OpaqueCallbackRegistrationId,
    provider: OpaqueCallbackProviderId,
    unregistration_contract: OpaqueCallbackUnregistrationContractId,
    registration_receipt: OpaqueCallbackRegistrationReceiptId,
    capacity: OpaqueCallbackRegistrationCapacityOccurrence,
    root: InstalledExternalRoot<'code>,
}

impl ReclaimableOpaqueCallback<'_> {
    pub const fn registration(&self) -> OpaqueCallbackRegistrationId {
        self.registration
    }

    pub const fn provider(&self) -> OpaqueCallbackProviderId {
        self.provider
    }

    pub const fn unregistration_contract(&self) -> OpaqueCallbackUnregistrationContractId {
        self.unregistration_contract
    }

    pub const fn root(&self) -> ExternalRootId {
        self.root.root()
    }

    pub const fn installed_code(&self) -> InstalledCodeId {
        self.root.installed_code()
    }

    pub const fn capacity(&self) -> &OpaqueCallbackRegistrationCapacityOccurrence {
        &self.capacity
    }
}

pub fn admit_reclaimable_opaque_callback<'code>(
    root: InstalledExternalRoot<'code>,
    receipt: OpaqueCallbackRegistrationReceipt,
    capacity: OpaqueCallbackRegistrationCapacityOccurrence,
) -> Result<ReclaimableOpaqueCallback<'code>, Box<OpaqueCallbackRegistrationError<'code>>> {
    let exact_capacity =
        receipt.capacity == capacity.identity && receipt.provider == capacity.provider;
    if receipt.installed_root != root.evidence || !exact_capacity || !receipt.callback_registered {
        return Err(Box::new(OpaqueCallbackRegistrationError {
            root,
            receipt,
            capacity,
            diagnostic: ExternalRootDiagnostic(
                "opaque callback registration receipt does not bind the exact installed external root, provider capacity occurrence, and completed registration"
                    .into(),
            ),
        }));
    }
    Ok(ReclaimableOpaqueCallback {
        registration: receipt.registration,
        provider: receipt.provider,
        unregistration_contract: receipt.unregistration_contract,
        registration_receipt: receipt.identity,
        capacity,
        root,
    })
}

#[derive(Debug)]
pub struct OpaqueCallbackRegistrationError<'code> {
    root: InstalledExternalRoot<'code>,
    receipt: OpaqueCallbackRegistrationReceipt,
    capacity: OpaqueCallbackRegistrationCapacityOccurrence,
    diagnostic: ExternalRootDiagnostic,
}

impl<'code> OpaqueCallbackRegistrationError<'code> {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        InstalledExternalRoot<'code>,
        OpaqueCallbackRegistrationReceipt,
        OpaqueCallbackRegistrationCapacityOccurrence,
    ) {
        (self.root, self.receipt, self.capacity)
    }
}

/// Provider evidence that the opaque holder removed the exact foreign callback
/// registration. Root unreachability and execution quiescence remain separate
/// facts checked by `InstalledRootLedger::remove`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpaqueCallbackUnregistrationReceipt {
    identity: OpaqueCallbackUnregistrationReceiptId,
    registration: OpaqueCallbackRegistrationId,
    provider: OpaqueCallbackProviderId,
    unregistration_contract: OpaqueCallbackUnregistrationContractId,
    registration_receipt: OpaqueCallbackRegistrationReceiptId,
    capacity: OpaqueCallbackRegistrationCapacityOccurrenceId,
    installed_root: InstalledRootEvidence,
    callback_unregistered: bool,
}

impl OpaqueCallbackUnregistrationReceipt {
    pub fn from_provider(
        identity: OpaqueCallbackUnregistrationReceiptId,
        registration: &ReclaimableOpaqueCallback<'_>,
        callback_unregistered: bool,
    ) -> Self {
        Self {
            identity,
            registration: registration.registration,
            provider: registration.provider,
            unregistration_contract: registration.unregistration_contract,
            registration_receipt: registration.registration_receipt,
            capacity: registration.capacity.identity,
            installed_root: registration.root.evidence.clone(),
            callback_unregistered,
        }
    }
}

#[derive(Debug)]
#[must_use = "completed callback unregistration returns root-slot and live-registration capacity authority"]
pub struct CompletedOpaqueCallbackUnregistration {
    registration: OpaqueCallbackRegistrationId,
    provider_receipt: OpaqueCallbackUnregistrationReceiptId,
    root_removal_receipt: RootRemovalReceiptId,
    slot: RootSlotAuthority,
    capacity: OpaqueCallbackRegistrationCapacityOccurrence,
}

impl CompletedOpaqueCallbackUnregistration {
    pub const fn registration(&self) -> OpaqueCallbackRegistrationId {
        self.registration
    }

    pub const fn provider_receipt(&self) -> OpaqueCallbackUnregistrationReceiptId {
        self.provider_receipt
    }

    pub const fn root_removal_receipt(&self) -> RootRemovalReceiptId {
        self.root_removal_receipt
    }

    pub const fn capacity(&self) -> &OpaqueCallbackRegistrationCapacityOccurrence {
        &self.capacity
    }

    pub fn into_parts(
        self,
    ) -> (
        RootSlotAuthority,
        OpaqueCallbackRegistrationCapacityOccurrence,
    ) {
        (self.slot, self.capacity)
    }
}

impl<'code> ReclaimableOpaqueCallback<'code> {
    pub fn unregister_and_quiesce(
        self,
        ledger: &mut InstalledRootLedger,
        provider_receipt: OpaqueCallbackUnregistrationReceipt,
        root_removal_receipt: RootRemovalReceipt,
    ) -> Result<CompletedOpaqueCallbackUnregistration, Box<OpaqueCallbackUnregistrationError<'code>>>
    {
        let exact_provider_receipt = provider_receipt.registration == self.registration
            && provider_receipt.provider == self.provider
            && provider_receipt.unregistration_contract == self.unregistration_contract
            && provider_receipt.registration_receipt == self.registration_receipt
            && provider_receipt.capacity == self.capacity.identity
            && provider_receipt.installed_root == self.root.evidence;
        if !exact_provider_receipt || !provider_receipt.callback_unregistered {
            return Err(Box::new(OpaqueCallbackUnregistrationError {
                registration: self,
                provider_receipt,
                root_removal_receipt,
                diagnostic: ExternalRootDiagnostic(
                    "opaque callback unregistration receipt does not remove the exact registered foreign callback"
                        .into(),
                ),
            }));
        }

        let Self {
            registration,
            provider,
            unregistration_contract,
            registration_receipt,
            capacity,
            root,
        } = self;
        let root_removal_identity = root_removal_receipt.identity();
        let slot = match ledger.remove(root, root_removal_receipt) {
            Ok(slot) => slot,
            Err(error) => {
                let (root, root_removal_receipt) = (*error).into_parts();
                return Err(Box::new(OpaqueCallbackUnregistrationError {
                    registration: ReclaimableOpaqueCallback {
                        registration,
                        provider,
                        unregistration_contract,
                        registration_receipt,
                        capacity,
                        root,
                    },
                    provider_receipt,
                    root_removal_receipt,
                    diagnostic: ExternalRootDiagnostic(
                        "opaque callback was unregistered but exact external-root quiescence is not established"
                            .into(),
                    ),
                }));
            }
        };
        Ok(CompletedOpaqueCallbackUnregistration {
            registration,
            provider_receipt: provider_receipt.identity,
            root_removal_receipt: root_removal_identity,
            slot,
            capacity,
        })
    }
}

#[derive(Debug)]
pub struct OpaqueCallbackUnregistrationError<'code> {
    registration: ReclaimableOpaqueCallback<'code>,
    provider_receipt: OpaqueCallbackUnregistrationReceipt,
    root_removal_receipt: RootRemovalReceipt,
    diagnostic: ExternalRootDiagnostic,
}

impl<'code> OpaqueCallbackUnregistrationError<'code> {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        ReclaimableOpaqueCallback<'code>,
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
