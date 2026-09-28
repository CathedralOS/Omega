//! Normalized identities: every nonzero `u64` coordinate an external root,
//! slot, receipt or policy is known by, and the compact FNV-1a report
//! fingerprint that names a slot from its text.

use crate::ExternalRootDiagnostic;

macro_rules! normalized_id {
    ($name:ident, $label:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

        impl $name {
            pub fn from_normalized_identity(identity: u64) -> Result<Self, ExternalRootDiagnostic> {
                if identity == 0 {
                    return Err(ExternalRootDiagnostic(format!(
                        "normalized {} identity cannot be zero",
                        $label
                    )));
                }
                Ok(Self(identity))
            }

            pub const fn normalized_identity(self) -> u64 {
                self.0
            }
        }
    };
}

normalized_id!(ExternalRootId, "external-root");
normalized_id!(RootSlotId, "external-root slot");
normalized_id!(RootSlotOwnerId, "external-root slot owner");
normalized_id!(RootProviderId, "external-root provider");
normalized_id!(ProviderPlanId, "provider plan");
normalized_id!(ProviderExecutionId, "provider execution");
normalized_id!(
    InstalledProviderOccurrenceId,
    "installed provider occurrence"
);
normalized_id!(
    ProviderOccurrenceInstallationReceiptId,
    "provider occurrence installation receipt"
);
normalized_id!(
    ProgressProfileEstablishmentReceiptId,
    "progress-profile establishment receipt"
);
normalized_id!(
    ProgressProfileGrantInvocationId,
    "progress-profile grant invocation"
);
normalized_id!(RootEffectId, "external-root effect");
normalized_id!(TrustReceiptId, "external-root trust receipt");
normalized_id!(NestingRelationId, "external-root nesting relation");
normalized_id!(
    AcknowledgementPolicyId,
    "external-root acknowledgement policy"
);
normalized_id!(ComponentContractId, "component contract");
normalized_id!(ComponentArtifactId, "component artifact");
normalized_id!(ComponentProviderId, "component provider");
normalized_id!(ComponentVersionPinId, "component version pin");
normalized_id!(RootAdmissionId, "external-root admission");
normalized_id!(RootRemovalReceiptId, "external-root removal receipt");
normalized_id!(StackValidationReceiptId, "stack validation receipt");
normalized_id!(
    X86_64GateProfileValidationReceiptId,
    "x86-64 installed gate/profile validation receipt"
);
normalized_id!(ProviderFuelSummaryId, "fixed-fuel provider summary");
normalized_id!(FuelProvisionId, "logical-fuel provision");
normalized_id!(
    ProviderFuelValidationReceiptId,
    "fixed-fuel provider validation receipt"
);
normalized_id!(FuelValidationReceiptId, "logical-fuel validation receipt");
normalized_id!(StateValidationReceiptId, "machine-state validation receipt");
normalized_id!(InterruptInvocationId, "interrupt invocation");
normalized_id!(InterruptEntryReceiptId, "interrupt entry receipt");
normalized_id!(InterruptMaskControlId, "interrupt-mask control");
normalized_id!(InterruptMaskStateId, "interrupt-mask state");
normalized_id!(InterruptMaskGuardId, "interrupt-mask guard");
normalized_id!(
    InterruptMaskTransitionReceiptId,
    "interrupt-mask transition receipt"
);
normalized_id!(InterruptAcknowledgementId, "interrupt acknowledgement");
normalized_id!(
    InterruptAcknowledgementReceiptId,
    "interrupt acknowledgement receipt"
);
normalized_id!(OpaqueCallbackRegistrationId, "opaque callback registration");
normalized_id!(OpaqueCallbackProviderId, "opaque callback provider");
normalized_id!(
    OpaqueCallbackRegistrationCapacityOccurrenceId,
    "opaque callback live-registration capacity occurrence"
);
normalized_id!(
    ProcessLifetimeGatewayId,
    "process-lifetime callback gateway"
);
normalized_id!(
    GatewayDispatchContractId,
    "callback gateway dispatch contract"
);
normalized_id!(
    GatewayAdmissionReceiptId,
    "callback gateway admission receipt"
);
normalized_id!(
    OpaqueCallbackUnregistrationContractId,
    "opaque callback unregistration contract"
);
normalized_id!(
    OpaqueCallbackRegistrationReceiptId,
    "opaque callback registration receipt"
);
normalized_id!(
    OpaqueCallbackUnregistrationReceiptId,
    "opaque callback unregistration receipt"
);
normalized_id!(
    UefiApplicationBootstrapLedgerId,
    "UEFI application bootstrap ledger"
);
normalized_id!(UefiFirmwareSessionId, "UEFI firmware session");
normalized_id!(UefiPhysicalInvocationId, "UEFI physical invocation");
normalized_id!(UefiImageHandleOccurrenceId, "UEFI image-handle occurrence");
normalized_id!(UefiSystemTableOccurrenceId, "UEFI system-table occurrence");
normalized_id!(
    UefiBootServicesTableOccurrenceId,
    "UEFI Boot Services table occurrence"
);
normalized_id!(
    UefiBootServicesPhaseLeaseId,
    "UEFI Boot Services phase lease"
);
normalized_id!(UefiOsHandoffId, "UEFI OS-handoff");
normalized_id!(
    UefiOsHandoffBootServicesId,
    "UEFI OS-handoff Boot Services occurrence"
);
normalized_id!(
    UefiOsHandoffAllocationRosterId,
    "UEFI OS-handoff allocation roster"
);
normalized_id!(
    UefiOsHandoffStackEvidenceId,
    "UEFI OS-handoff surviving stack evidence"
);
normalized_id!(
    UefiMemoryMapSnapshotId,
    "UEFI memory-map snapshot occurrence"
);
normalized_id!(UefiMemoryMapKeyId, "UEFI memory-map key occurrence");
normalized_id!(
    UefiExitBootServicesReceiptId,
    "UEFI ExitBootServices success receipt"
);
normalized_id!(
    SecondaryProcessorOccurrenceId,
    "secondary-processor occurrence"
);
normalized_id!(
    SecondaryProcessorStartupProfileId,
    "secondary-processor startup profile"
);
normalized_id!(
    SecondaryProcessorStartupInvocationId,
    "secondary-processor startup invocation"
);
normalized_id!(
    SecondaryProcessorStartupReceiptId,
    "secondary-processor startup receipt"
);
normalized_id!(
    SecondaryProcessorQuiescenceReceiptId,
    "secondary-processor quiescence receipt"
);
normalized_id!(
    SecondaryProcessorSettlementReceiptId,
    "secondary-processor settlement receipt"
);
normalized_id!(InterruptTableProfileId, "interrupt-table profile");
normalized_id!(
    InterruptTableEstablishmentId,
    "interrupt-table establishment"
);
normalized_id!(
    InterruptTablePublicationAuthorityId,
    "interrupt-table publication authority"
);
normalized_id!(InterruptTablePublicationId, "interrupt-table publication");
normalized_id!(
    InterruptTablePublicationReceiptId,
    "interrupt-table publication receipt"
);

impl RootSlotId {
    /// Derive the stable slot identity shared by build selection,
    /// installation, and external-root verification for any target-required
    /// root schema.
    pub fn for_target_required_root_slot(
        declaration: target::TargetRequiredRootSlotDeclaration,
    ) -> Result<Self, ExternalRootDiagnostic> {
        if declaration
            .owner()
            .required_root_slot(declaration.slot_name())
            != Some(declaration)
        {
            return Err(ExternalRootDiagnostic(
                "target required-root declaration does not match its owning profile catalog".into(),
            ));
        }
        let canonical = format!(
            "target-root-slot\n{}::{}",
            declaration.owner().root_slot_owner_name(),
            declaration.slot_name()
        );
        Self::from_normalized_identity(fnv1a_identity(&canonical))
    }

    /// Derive the stable slot identity shared by build selection, installation,
    /// and external-root verification.
    pub fn for_target_program_entry(
        slot: target::ProgramEntrySlotDeclaration,
    ) -> Result<Self, ExternalRootDiagnostic> {
        Self::for_target_required_root_slot(
            target::TargetRequiredRootSlotDeclaration::ProgramEntry(slot),
        )
    }
}

impl RootSlotOwnerId {
    pub fn for_target_profile(
        profile: target::TargetProfile,
    ) -> Result<Self, ExternalRootDiagnostic> {
        let canonical = format!("target-root-slot-owner\n{}", profile.root_slot_owner_name());
        Self::from_normalized_identity(fnv1a_identity(&canonical))
    }
}

fn fnv1a_identity(value: &str) -> u64 {
    let mut identity = 0xcbf2_9ce4_8422_2325_u64;
    for byte in value.bytes() {
        identity ^= u64::from(byte);
        identity = identity.wrapping_mul(0x0000_0100_0000_01b3);
    }
    if identity == 0 {
        0xcbf2_9ce4_8422_2325
    } else {
        identity
    }
}

pub(crate) struct Fnv1a(u64);

impl Fnv1a {
    pub(crate) const fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    pub(crate) fn u64(&mut self, value: u64) {
        for byte in value.to_le_bytes() {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    pub(crate) fn string(&mut self, value: &str) {
        self.u64(value.len() as u64);
        for byte in value.bytes() {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    pub(crate) fn bytes(&mut self, value: &[u8]) {
        self.u64(value.len() as u64);
        for byte in value {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    pub(crate) const fn finish(self) -> u64 {
        self.0
    }
}
