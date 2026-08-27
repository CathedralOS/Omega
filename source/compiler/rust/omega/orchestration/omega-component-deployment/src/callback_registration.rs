//! Deployment custody across reclaimable callback registration.
//!
//! The first phase installs an independently admitted external root and keeps
//! it pending. The registrar then remains an ordinary runtime boundary call
//! outside this module. The second phase consumes its provider result and
//! keeps registration and ledger custody together until explicit
//! unregister-and-quiesce completion. Compiler-emitted callbacks enter this
//! sequence only through an exact installed-entry attribution.

use crate::TerminalComponentDeploymentSession;
use omega_backend_plan::CallbackInstallationEntry;
use omega_executable_installation::{InstalledCode, InstalledCodeContext};
use omega_external_roots::{
    CompletedOpaqueCallbackUnregistration, InstalledExternalRoot, InstalledRootLedger,
    OpaqueCallbackRegistrationReceipt, OpaqueCallbackUnregistrationReceipt, ProviderExecution,
    ReclaimableOpaqueCallback, RootAdmission, RootAdmissionId, RootRemovalReceipt,
    RootSlotAuthority, ValidatedExternalRoot, admit_reclaimable_opaque_callback,
};

/// Project compiler-issued callback entries into canonical artifact decode
/// rows without exposing a resolved address. The manifest remains borrowed so
/// installation and later attribution consume the same retained identities.
pub fn callback_artifact_entries(
    manifest: &omega_backend_plan::CallbackInstallationManifest,
) -> Result<Vec<omega_executable_installation::ArtifactEntry>, CallbackArtifactEntryError> {
    manifest
        .entries()
        .iter()
        .map(|entry| {
            let offset = u64::try_from(entry.text_offset()).map_err(|_| {
                CallbackArtifactEntryError(
                    "callback text offset does not fit the artifact entry domain".into(),
                )
            })?;
            Ok(
                omega_executable_installation::ArtifactEntry::from_canonical_decode(
                    entry.entry(),
                    offset,
                ),
            )
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackArtifactEntryError(String);

impl std::fmt::Display for CallbackArtifactEntryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::error::Error for CallbackArtifactEntryError {}

/// Complete inputs for binding one compiler-retained callback entry to one
/// exact installed-code occurrence. The byte snapshots remain provider-side
/// equality inputs and are returned unchanged on rejection.
#[derive(Debug)]
#[must_use = "callback installation input retains exact compiler evidence and byte snapshots"]
pub struct CallbackEntryInstallationInput {
    entry: CallbackInstallationEntry,
    unrelocated_text: Vec<u8>,
    materialized_text: Vec<u8>,
}

impl CallbackEntryInstallationInput {
    pub fn new(
        entry: CallbackInstallationEntry,
        unrelocated_text: Vec<u8>,
        materialized_text: Vec<u8>,
    ) -> Self {
        Self {
            entry,
            unrelocated_text,
            materialized_text,
        }
    }

    pub const fn entry(&self) -> &CallbackInstallationEntry {
        &self.entry
    }

    pub fn into_parts(self) -> (CallbackInstallationEntry, Vec<u8>, Vec<u8>) {
        (self.entry, self.unrelocated_text, self.materialized_text)
    }
}

/// Sealed equality between one compiler-retained callback entry and one exact
/// installed code occurrence. It exposes no resolved address and grants no
/// registration, invocation, capacity, lease, or publication authority.
#[derive(Debug)]
pub struct InstalledCallbackEntryAttribution {
    installed: InstalledCodeContext,
    entry: CallbackInstallationEntry,
}

impl InstalledCallbackEntryAttribution {
    pub const fn entry(&self) -> psi_layout_plans::EntryStubId {
        self.entry.entry()
    }

    pub fn requirement_identity(&self) -> &str {
        self.entry.requirement_identity()
    }

    pub const fn function_identity(&self) -> omega_control_flow::MachineFunctionIdentity {
        self.entry.function_identity()
    }

    pub const fn placement_index(&self) -> usize {
        self.entry.placement_index()
    }

    pub const fn text_interval(&self) -> (usize, usize) {
        (self.entry.text_offset(), self.entry.text_byte_count())
    }

    fn matches_installed(&self, installed: &InstalledCode) -> bool {
        self.installed == installed.receipt_context()
    }
}

fn callback_root_attribution_matches(
    installed: &InstalledCode,
    entry: psi_layout_plans::EntryStubId,
    requirement_identity: &str,
    attribution: &InstalledCallbackEntryAttribution,
) -> bool {
    attribution.matches_installed(installed)
        && entry == attribution.entry()
        && requirement_identity == attribution.requirement_identity()
}

/// Bind exact compiler callback-entry evidence to one installed realization.
/// Rejection performs no installation or runtime operation and returns every
/// caller-owned input unchanged for correction and retry.
pub fn bind_installed_callback_entry(
    installed: &InstalledCode,
    input: CallbackEntryInstallationInput,
) -> Result<InstalledCallbackEntryAttribution, CallbackEntryInstallationError> {
    let architecture_matches = installed.architecture() == input.entry.target().architecture;
    let text_offset = u64::try_from(input.entry.text_offset()).ok();
    let accepted = architecture_matches
        && installed.binds_exact_materialized_artifact_bytes(
            &input.unrelocated_text,
            &input.materialized_text,
        )
        && text_offset
            .is_some_and(|offset| installed.binds_entry_offset(input.entry.entry(), offset))
        && installed.selected_entry_target(input.entry.entry()).is_ok();
    if !accepted {
        return Err(CallbackEntryInstallationError {
            input,
            diagnostic: "callback installation entry does not match the exact installed artifact, materialized bytes, architecture, and entry interval".into(),
        });
    }
    Ok(InstalledCallbackEntryAttribution {
        installed: installed.receipt_context(),
        entry: input.entry,
    })
}

#[derive(Debug)]
pub struct CallbackEntryInstallationError {
    input: CallbackEntryInstallationInput,
    diagnostic: String,
}

impl CallbackEntryInstallationError {
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }

    pub fn into_input(self) -> CallbackEntryInstallationInput {
        self.input
    }
}

impl std::fmt::Display for CallbackEntryInstallationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for CallbackEntryInstallationError {}

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
    attribution: InstalledCallbackEntryAttribution,
}

impl ReclaimableCallbackRootDeployment {
    pub fn new(
        admission_identity: RootAdmissionId,
        root: ValidatedExternalRoot,
        provider_execution: ProviderExecution,
        slot: RootSlotAuthority,
        attribution: InstalledCallbackEntryAttribution,
    ) -> Self {
        Self {
            admission_identity,
            root,
            provider_execution,
            slot,
            attribution,
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

    pub const fn attribution(&self) -> &InstalledCallbackEntryAttribution {
        &self.attribution
    }

    pub fn into_parts(
        self,
    ) -> (
        RootAdmissionId,
        ValidatedExternalRoot,
        ProviderExecution,
        RootSlotAuthority,
        InstalledCallbackEntryAttribution,
    ) {
        (
            self.admission_identity,
            self.root,
            self.provider_execution,
            self.slot,
            self.attribution,
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
        install_reclaimable_callback_root(&self.installed, &mut self.roots, input)
    }
}

fn install_reclaimable_callback_root<'deployment>(
    installed: &'deployment InstalledCode,
    roots: &'deployment mut InstalledRootLedger,
    input: ReclaimableCallbackRootDeployment,
) -> Result<PendingReclaimableCallbackRegistration<'deployment>, CallbackRootDeploymentError> {
    let ReclaimableCallbackRootDeployment {
        admission_identity,
        root,
        provider_execution,
        slot,
        attribution,
    } = input;

    if !callback_root_attribution_matches(
        installed,
        root.candidate().entry,
        &root.candidate().requirement_identity,
        &attribution,
    ) {
        return Err(CallbackRootDeploymentError {
            input: ReclaimableCallbackRootDeployment {
                admission_identity,
                root,
                provider_execution,
                slot,
                attribution,
            },
            diagnostic:
                "callback root does not match the exact installed callback entry and requirement"
                    .into(),
        });
    }

    let admission = match RootAdmission::from_admitted_provider(
        admission_identity,
        &root,
        &provider_execution,
        installed,
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
                    attribution,
                },
                diagnostic: diagnostic.0,
            });
        }
    };

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
                    attribution,
                },
                diagnostic,
            });
        }
    };

    Ok(PendingReclaimableCallbackRegistration {
        root,
        roots,
        attribution,
    })
}

/// Installed external root held pending the provider's registrar-result
/// receipt. This is not yet a durable callback registration.
#[derive(Debug)]
#[must_use = "an installed callback root must be admitted or explicitly removed"]
pub struct PendingReclaimableCallbackRegistration<'deployment> {
    root: InstalledExternalRoot<'deployment>,
    roots: &'deployment mut InstalledRootLedger,
    attribution: InstalledCallbackEntryAttribution,
}

impl<'deployment> PendingReclaimableCallbackRegistration<'deployment> {
    pub const fn root(&self) -> &InstalledExternalRoot<'deployment> {
        &self.root
    }

    pub const fn roots(&self) -> &InstalledRootLedger {
        self.roots
    }

    pub const fn attribution(&self) -> &InstalledCallbackEntryAttribution {
        &self.attribution
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
        let Self {
            root,
            roots,
            attribution,
        } = self;
        match admit_reclaimable_opaque_callback(root, receipt) {
            Ok(registration) => Ok(InstalledReclaimableCallback {
                registration,
                roots,
                attribution,
            }),
            Err(error) => {
                let diagnostic = error.diagnostic().0.clone();
                let (root, receipt) = (*error).into_parts();
                Err(CallbackRegistrationAdmissionError {
                    pending: PendingReclaimableCallbackRegistration {
                        root,
                        roots,
                        attribution,
                    },
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
        InstalledCallbackEntryAttribution,
    ) {
        (self.roots, self.root, self.attribution)
    }

    /// Remove a pending root when the registrar rejects registration. A
    /// failed removal returns the complete pending carrier and receipt.
    pub fn remove(
        self,
        receipt: RootRemovalReceipt,
    ) -> Result<
        CompletedPendingCallbackRootRemoval,
        Box<PendingCallbackRootRemovalError<'deployment>>,
    > {
        let Self {
            root,
            roots,
            attribution,
        } = self;
        match roots.remove(root, receipt) {
            Ok(slot) => Ok(CompletedPendingCallbackRootRemoval { slot, attribution }),
            Err(error) => {
                let diagnostic = error.diagnostic().0.clone();
                let (root, receipt) = (*error).into_parts();
                Err(Box::new(PendingCallbackRootRemovalError {
                    pending: PendingReclaimableCallbackRegistration {
                        root,
                        roots,
                        attribution,
                    },
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
    attribution: InstalledCallbackEntryAttribution,
}

impl<'deployment> InstalledReclaimableCallback<'deployment> {
    pub const fn registration(&self) -> &ReclaimableOpaqueCallback<'deployment> {
        &self.registration
    }

    pub const fn roots(&self) -> &InstalledRootLedger {
        self.roots
    }

    pub const fn attribution(&self) -> &InstalledCallbackEntryAttribution {
        &self.attribution
    }

    /// Complete provider unregister and exact root quiescence as one
    /// transactional transition. Every live value returns on rejection.
    pub fn unregister_and_quiesce(
        self,
        provider_receipt: OpaqueCallbackUnregistrationReceipt,
        root_removal_receipt: RootRemovalReceipt,
    ) -> Result<
        CompletedAttributedCallbackUnregistration,
        Box<CallbackUnregistrationError<'deployment>>,
    > {
        let Self {
            registration,
            roots,
            attribution,
        } = self;
        match registration.unregister_and_quiesce(roots, provider_receipt, root_removal_receipt) {
            Ok(completed) => Ok(CompletedAttributedCallbackUnregistration {
                completed,
                attribution,
            }),
            Err(error) => {
                let diagnostic = error.diagnostic().0.clone();
                let (registration, provider_receipt, root_removal_receipt) = (*error).into_parts();
                Err(Box::new(CallbackUnregistrationError {
                    installed: InstalledReclaimableCallback {
                        registration,
                        roots,
                        attribution,
                    },
                    provider_receipt,
                    root_removal_receipt,
                    diagnostic,
                }))
            }
        }
    }
}

/// Successful cleanup of a pending, unregistered callback root. Both the
/// reclaimed slot and installed-entry attribution remain available to the
/// deployment owner.
#[derive(Debug)]
pub struct CompletedPendingCallbackRootRemoval {
    slot: RootSlotAuthority,
    attribution: InstalledCallbackEntryAttribution,
}

impl CompletedPendingCallbackRootRemoval {
    pub const fn slot(&self) -> &RootSlotAuthority {
        &self.slot
    }

    pub const fn attribution(&self) -> &InstalledCallbackEntryAttribution {
        &self.attribution
    }

    pub fn into_parts(self) -> (RootSlotAuthority, InstalledCallbackEntryAttribution) {
        (self.slot, self.attribution)
    }
}

/// Successful provider unregister and root quiescence with exact installed
/// callback attribution preserved beside the reclaimed slot receipt.
#[derive(Debug)]
pub struct CompletedAttributedCallbackUnregistration {
    completed: CompletedOpaqueCallbackUnregistration,
    attribution: InstalledCallbackEntryAttribution,
}

impl CompletedAttributedCallbackUnregistration {
    pub const fn completed(&self) -> &CompletedOpaqueCallbackUnregistration {
        &self.completed
    }

    pub const fn attribution(&self) -> &InstalledCallbackEntryAttribution {
        &self.attribution
    }

    pub fn into_parts(
        self,
    ) -> (
        CompletedOpaqueCallbackUnregistration,
        InstalledCallbackEntryAttribution,
    ) {
        (self.completed, self.attribution)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    use omega_calling_conventions::{
        ArrivalContextId, ArrivalContextRealization, CallSignature, CallingPolicy, EntryStack,
        EntryStackEpoch, EntryStackRealization, EntryStackStage, MachineRegister, MachineState,
        MachineStateSet, RegisterSet, StackDomainRef, StateFootprintEvidence, ValueShape,
        evaluate_ordinary_boundary_entry_plan, validate_entry_stack_realization,
    };
    use omega_executable_installation::{
        AdmissionReceiptId, Artifact, ArtifactAdmissionEvidence, ArtifactContentId, ArtifactEntry,
        ArtifactId, CodePlacementAuthority, CodePlacementId, EntrySetId,
        FinalValidationCertificate, FinalValidationId, InstallAuthority, InstallationAudience,
        InstallationReceipt, InstallationScopeId, InstalledCodeId, MachineContractSetId,
        MachineFootprintId, MaterializationReceipt, PlacementPlanId, RelocationSetId,
        WxEnforcement, admit_executable, install_validated, materialize_admitted_artifact,
        materialize_and_freeze, validate_final_placement,
    };
    use omega_external_roots::{
        AcknowledgementPolicyId, BoundEpochStackComposition, ComponentArtifactId,
        ComponentContractId, ComponentProviderId, ComponentVersionPin, ComponentVersionPinId,
        ComposedFuelDemand, ExternalRootCandidate, ExternalRootId, FixedFuelCall,
        FixedFuelProviderSummary, FuelProvisionId, FuelScheduleIdentity, FuelValidationReceiptId,
        LogicalFuelResourceColumn, MachineStateResourceColumn, NestingRelationId,
        OpaqueCallbackProviderId, OpaqueCallbackRegistrationId,
        OpaqueCallbackRegistrationReceiptId, OpaqueCallbackUnregistrationContractId,
        OpaqueCallbackUnregistrationReceiptId, OpaqueProviderExitAssurance, ProviderExecutionId,
        ProviderFuelSummaryId, ProviderFuelValidationReceiptId, ProviderPlanId,
        ResolvedRootServiceReach, RootAdmissionId, RootEffectId, RootProviderId,
        RootRemovalReceiptId, RootSlotId, RootSlotOwnerId, StackNestingRelation,
        StackResourceColumn, StackValidationReceiptId, StateValidationReceiptId, TrustReceiptId,
        admit_opaque_arrival_context_set, bind_opaque_adapter_stack_realization,
        compose_bound_entry_stack_epochs, compose_fixed_fuel, validate_external_root,
    };
    use omega_target::NativeTarget;
    use psi_extents::{
        AddressSpaceId, ExtentLineageId, ExtentProvenanceId, ExtentRightId, ExtentRights,
        ExtentRootGrant, MappingEraId,
    };
    use psi_layout_plans::{
        ArtifactInstallationScopeId, PlacementAddressRange, PlacementConstraints, PlacementPhase,
        PlacementSite,
    };

    fn external_id<T>(
        identity: u64,
        constructor: fn(u64) -> Result<T, omega_external_roots::ExternalRootDiagnostic>,
    ) -> T {
        constructor(identity).expect("normalized external-root identity")
    }

    fn callback_boundary() -> omega_calling_conventions::ValidatedBoundaryEntryPlan {
        evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: vec![ValueShape::integer(8, 8)],
                result: None,
            },
        )
        .expect("validated callback boundary")
    }

    fn fixed_fuel(provider: RootProviderId) -> ComposedFuelDemand {
        let schedule = FuelScheduleIdentity::new(1).expect("test fuel schedule");
        let leaf = FixedFuelProviderSummary::from_admitted_provider(
            external_id(31, ProviderFuelSummaryId::from_normalized_identity),
            external_id(12, RootProviderId::from_normalized_identity),
            schedule,
            5,
            BTreeSet::new(),
            external_id(
                41,
                ProviderFuelValidationReceiptId::from_normalized_identity,
            ),
        );
        let root = FixedFuelProviderSummary::from_admitted_provider(
            external_id(30, ProviderFuelSummaryId::from_normalized_identity),
            provider,
            schedule,
            2,
            BTreeSet::from([FixedFuelCall {
                callee: leaf.identity,
                maximum_invocations: 1,
            }]),
            external_id(
                40,
                ProviderFuelValidationReceiptId::from_normalized_identity,
            ),
        );
        compose_fixed_fuel(root.identity, [&root, &leaf]).expect("fixed-fuel composition")
    }

    fn stack_demand(
        root: ExternalRootId,
        provider: RootProviderId,
        relation: NestingRelationId,
        boundary: &omega_calling_conventions::ValidatedBoundaryEntryPlan,
        code: &InstalledCode,
        entry: psi_layout_plans::EntryStubId,
    ) -> BoundEpochStackComposition {
        let active_domain = StackDomainRef::from(EntryStack::Interrupted);
        let realization = validate_entry_stack_realization(EntryStackRealization {
            contexts: vec![ArrivalContextRealization {
                context: ArrivalContextId::new(1).expect("arrival context"),
                epochs: vec![EntryStackEpoch {
                    stage: EntryStackStage::Body,
                    active_domain,
                    occupancy_by_domain: Vec::new(),
                    nesting: boundary.plan().state.preemption,
                }],
            }],
        })
        .expect("stack realization");
        let summary = omega_external_roots::ProviderStackSummary::from_admitted_provider(
            root,
            provider,
            boundary.plan().state.stack,
            2048,
            16,
            external_id(49, StackValidationReceiptId::from_normalized_identity),
        );
        let contexts = admit_opaque_arrival_context_set(
            &summary,
            boundary,
            code,
            entry,
            vec![ArrivalContextId::new(1).expect("arrival context")],
            external_id(48, StackValidationReceiptId::from_normalized_identity),
        )
        .expect("arrival context closure");
        let bound = bind_opaque_adapter_stack_realization(
            &summary,
            boundary,
            code,
            entry,
            realization,
            contexts,
        )
        .expect("stack evidence binding");
        compose_bound_entry_stack_epochs(
            &StackNestingRelation {
                identity: relation,
                edges: BTreeSet::new(),
            },
            [&bound],
        )
        .expect("stack composition")
    }

    fn callback_root_candidate(
        installed: &InstalledCode,
        entry: psi_layout_plans::EntryStubId,
        requirement_identity: String,
        seed: u64,
    ) -> ExternalRootCandidate {
        let root = external_id(seed + 1, ExternalRootId::from_normalized_identity);
        let provider = external_id(seed + 2, RootProviderId::from_normalized_identity);
        let nesting_relation = external_id(seed + 3, NestingRelationId::from_normalized_identity);
        let boundary = callback_boundary();
        ExternalRootCandidate {
            identity: root,
            entry,
            provider,
            provider_plan: external_id(seed + 4, ProviderPlanId::from_normalized_identity),
            requirement_identity,
            entry_claims: Vec::new(),
            acknowledgement_parameter_index: None,
            interrupt_mask_guard_claim: None,
            service_reach: ResolvedRootServiceReach::from_selected_provider_closure(
                Vec::new(),
                Vec::new(),
                &omega_effects::SelectedProviderPlanFacts::default(),
            )
            .expect("empty root service reach"),
            effects: [external_id(
                seed + 5,
                RootEffectId::from_normalized_identity,
            )]
            .into_iter()
            .collect(),
            trust_receipts: [external_id(
                seed + 6,
                TrustReceiptId::from_normalized_identity,
            )]
            .into_iter()
            .collect(),
            nesting_relation,
            acknowledgement_policy: Some(external_id(
                seed + 7,
                AcknowledgementPolicyId::from_normalized_identity,
            )),
            stack: StackResourceColumn {
                ceiling_bytes: 8192,
                realization: stack_demand(
                    root,
                    provider,
                    nesting_relation,
                    &boundary,
                    installed,
                    entry,
                ),
                validation_receipt: external_id(
                    seed + 8,
                    StackValidationReceiptId::from_normalized_identity,
                ),
            },
            logical_fuel: LogicalFuelResourceColumn {
                schedule: FuelScheduleIdentity::new(1).expect("fuel schedule"),
                provision: external_id(seed + 9, FuelProvisionId::from_normalized_identity),
                ceiling_units: 64,
                realization: fixed_fuel(provider),
                validation_receipt: external_id(
                    seed + 10,
                    FuelValidationReceiptId::from_normalized_identity,
                ),
            },
            machine_state: MachineStateResourceColumn {
                realization: StateFootprintEvidence::new(
                    RegisterSet::new([MachineRegister::X86Rax]),
                    MachineStateSet::new([MachineState::Flags]),
                ),
                validation_receipt: external_id(
                    seed + 11,
                    StateValidationReceiptId::from_normalized_identity,
                ),
            },
            component_pins: [ComponentVersionPin {
                contract: external_id(seed + 12, ComponentContractId::from_normalized_identity),
                artifact: external_id(seed + 13, ComponentArtifactId::from_normalized_identity),
                provider: external_id(seed + 14, ComponentProviderId::from_normalized_identity),
                version: external_id(seed + 15, ComponentVersionPinId::from_normalized_identity),
            }]
            .into_iter()
            .collect(),
        }
    }

    fn installation_id<T>(
        identity: u64,
        constructor: fn(u64) -> Result<T, omega_executable_installation::InstallationDiagnostic>,
    ) -> T {
        constructor(identity).expect("normalized installation identity")
    }

    fn extent_id<T>(
        identity: u64,
        constructor: fn(u64) -> Result<T, psi_extents::ExtentDiagnostic>,
    ) -> T {
        constructor(identity).expect("normalized extent identity")
    }

    fn installed_code(
        seed: u64,
        target: NativeTarget,
        entry: psi_layout_plans::EntryStubId,
        entry_offset: u64,
        bytes: Vec<u8>,
    ) -> InstalledCode {
        let scope = ArtifactInstallationScopeId::from_normalized_identity(seed + 1).unwrap();
        let constraints = PlacementConstraints::new(
            Some(PlacementAddressRange::new(0x1000, 0x1_0000).unwrap()),
            4096,
            PlacementPhase::PostHandoff,
            None,
            Some(scope),
        )
        .unwrap();
        let artifact = Artifact::from_canonical_decode(
            installation_id(seed + 2, ArtifactId::from_normalized_identity),
            installation_id(seed + 3, ArtifactContentId::from_normalized_identity),
            target.architecture,
            bytes,
            installation_id(seed + 4, MachineContractSetId::from_normalized_identity),
            installation_id(seed + 5, MachineFootprintId::from_normalized_identity),
            installation_id(seed + 6, PlacementPlanId::from_normalized_identity),
            constraints.clone(),
            installation_id(seed + 7, EntrySetId::from_normalized_identity),
            vec![ArtifactEntry::from_canonical_decode(entry, entry_offset)],
            installation_id(seed + 8, RelocationSetId::from_normalized_identity),
            Vec::new(),
        )
        .unwrap();
        let admitted = admit_executable(
            &artifact,
            ArtifactAdmissionEvidence::from_validator(
                installation_id(seed + 9, AdmissionReceiptId::from_normalized_identity),
                &artifact,
                true,
            ),
        )
        .unwrap();
        let rights = ExtentRights::from_normalized_identities([extent_id(
            seed + 10,
            ExtentRightId::from_normalized_identity,
        )]);
        let issuance = psi_extents::ExtentProviderIssuance::from_normalized_identities([
            seed + 11,
            seed + 12,
            seed + 13,
            seed + 14,
            seed + 15,
            seed + 16,
            seed + 17,
            seed + 18,
            seed + 19,
            seed + 20,
            seed + 21,
            seed + 22,
            seed + 23,
        ])
        .unwrap();
        let extent = ExtentRootGrant::from_admitted_provider(
            issuance,
            extent_id(seed + 24, ExtentLineageId::from_normalized_identity),
            extent_id(seed + 25, AddressSpaceId::from_normalized_identity),
            rights.clone(),
            extent_id(seed + 26, ExtentProvenanceId::from_normalized_identity),
            extent_id(seed + 27, MappingEraId::from_normalized_identity),
        )
        .mint(0x1000, 4096)
        .unwrap();
        let placement = CodePlacementAuthority::from_admitted_provider(
            installation_id(seed + 28, CodePlacementId::from_normalized_identity),
            installation_id(seed + 1, InstallationScopeId::from_normalized_identity),
            InstallationAudience::FutureFetcher,
            &extent,
            rights,
            constraints,
            PlacementSite {
                base_address: 0x1000,
                phase: PlacementPhase::PostHandoff,
                machine_regime: None,
                installation_scope: Some(scope),
            },
        )
        .claim(extent)
        .unwrap();
        let materialized = materialize_admitted_artifact(&admitted, &placement, |_| None).unwrap();
        let frozen = materialize_and_freeze(
            &admitted,
            placement,
            materialized.clone(),
            MaterializationReceipt::from_materialized(
                &materialized,
                installation_id(seed + 30, MachineFootprintId::from_normalized_identity),
                true,
            ),
        )
        .unwrap();
        let certificate = FinalValidationCertificate::from_validator(
            installation_id(seed + 31, FinalValidationId::from_normalized_identity),
            &frozen,
            true,
        );
        let validated = validate_final_placement(frozen, &certificate).unwrap();
        let authority = InstallAuthority::from_admitted_provider(&validated);
        let receipt = InstallationReceipt::from_provider(
            installation_id(seed + 32, InstalledCodeId::from_normalized_identity),
            &validated,
            true,
            WxEnforcement::HardwareEnforced,
        );
        install_validated(validated, authority, receipt).unwrap()
    }

    #[test]
    fn installed_callback_entry_binding_is_transactional_and_exact_on_both_targets() {
        for (index, target) in [NativeTarget::windows_x64(), NativeTarget::linux_arm64()]
            .into_iter()
            .enumerate()
        {
            let (manifest, _, _, _) =
                omega_backend_plan::callback_installation_test_fixture(target);
            let entry = manifest.into_entries().remove(0);
            let entry_id = entry.entry();
            let requirement = entry.requirement_identity().to_owned();
            let bytes = vec![0u8; 128];
            let installed = installed_code(
                1000 + (index as u64) * 100,
                target,
                entry_id,
                u64::try_from(entry.text_offset()).unwrap(),
                bytes.clone(),
            );
            let rejected = bind_installed_callback_entry(
                &installed,
                CallbackEntryInstallationInput::new(entry, bytes.clone(), vec![1u8; 128]),
            )
            .expect_err("materialized-byte substitution must reject transactionally");
            let (entry, unrelocated, _) = rejected.into_input().into_parts();
            let attribution = bind_installed_callback_entry(
                &installed,
                CallbackEntryInstallationInput::new(entry, unrelocated, bytes.clone()),
            )
            .expect("returned callback entry and bytes retry exactly");
            assert!(callback_root_attribution_matches(
                &installed,
                entry_id,
                &requirement,
                &attribution,
            ));
            let foreign_entry = psi_layout_plans::EntryStubId::from_normalized_identity(
                entry_id.normalized_identity() ^ 1,
            )
            .unwrap();
            assert!(!callback_root_attribution_matches(
                &installed,
                foreign_entry,
                &requirement,
                &attribution,
            ));
            assert!(!callback_root_attribution_matches(
                &installed,
                entry_id,
                "Registrar::other",
                &attribution,
            ));
        }
    }

    #[test]
    fn attributed_callback_registration_retains_retry_cleanup_and_quiescence_custody() {
        let target = NativeTarget::windows_x64();
        let (manifest, _, _, _) = omega_backend_plan::callback_installation_test_fixture(target);
        let entry = manifest.into_entries().remove(0);
        let entry_id = entry.entry();
        let requirement = entry.requirement_identity().to_owned();
        let bytes = vec![0u8; 128];
        let mut installed = installed_code(
            5000,
            target,
            entry_id,
            u64::try_from(entry.text_offset()).unwrap(),
            bytes.clone(),
        );
        let attribution = bind_installed_callback_entry(
            &installed,
            CallbackEntryInstallationInput::new(entry, bytes.clone(), bytes.clone()),
        )
        .expect("exact callback installation attribution");
        let root = validate_external_root(
            callback_root_candidate(&installed, entry_id, requirement, 6000),
            &callback_boundary(),
        )
        .expect("callback root validates");
        let execution = ProviderExecution::from_admitted_provider(
            external_id(6020, ProviderExecutionId::from_normalized_identity),
            &root,
            Some(OpaqueProviderExitAssurance::HardwareIsolation {
                validation_receipt: external_id(6006, TrustReceiptId::from_normalized_identity),
            }),
        )
        .expect("callback provider execution");
        let slot = RootSlotAuthority::from_admitted_owner(
            external_id(6021, RootSlotId::from_normalized_identity),
            external_id(6022, RootSlotOwnerId::from_normalized_identity),
        );
        let input = ReclaimableCallbackRootDeployment::new(
            external_id(6023, RootAdmissionId::from_normalized_identity),
            root.clone(),
            execution.clone(),
            slot,
            attribution,
        );

        let mut substituted = installed_code(5100, target, entry_id, 64, bytes);
        let mut substituted_roots =
            InstalledRootLedger::claim(&mut substituted).expect("foreign root ledger");
        let rejected =
            install_reclaimable_callback_root(&substituted, &mut substituted_roots, input)
                .expect_err("installed-code occurrence substitution must reject");
        assert!(
            rejected
                .diagnostic()
                .contains("exact installed callback entry")
        );
        let input = rejected.into_input();
        let mut roots = InstalledRootLedger::claim(&mut installed).expect("callback root ledger");
        let pending = install_reclaimable_callback_root(&installed, &mut roots, input)
            .expect("rejected input retries against its exact installation");
        let root_identity = pending.root().root();

        let false_receipt = OpaqueCallbackRegistrationReceipt::from_provider(
            external_id(
                6024,
                OpaqueCallbackRegistrationReceiptId::from_normalized_identity,
            ),
            external_id(6025, OpaqueCallbackRegistrationId::from_normalized_identity),
            external_id(6026, OpaqueCallbackProviderId::from_normalized_identity),
            external_id(
                6027,
                OpaqueCallbackUnregistrationContractId::from_normalized_identity,
            ),
            pending.root(),
            false,
        );
        let rejected = pending
            .admit_registration(false_receipt)
            .expect_err("false registrar result establishes no registration");
        let (pending, _) = rejected.into_parts();
        assert!(pending.roots().record(root_identity).is_some());
        let nonquiescent = RootRemovalReceipt::from_provider(
            external_id(6028, RootRemovalReceiptId::from_normalized_identity),
            pending.root(),
            true,
            false,
        );
        let rejected = pending
            .remove(nonquiescent)
            .expect_err("pending cleanup still requires quiescence");
        let (pending, _) = (*rejected).into_parts();
        let quiescent = RootRemovalReceipt::from_provider(
            external_id(6029, RootRemovalReceiptId::from_normalized_identity),
            pending.root(),
            true,
            true,
        );
        let completed = pending
            .remove(quiescent)
            .expect("false registration can be explicitly cleaned up");
        let (slot, attribution) = completed.into_parts();
        let slot_identity = slot.slot();
        assert!(roots.record(root_identity).is_none());

        let pending = install_reclaimable_callback_root(
            &installed,
            &mut roots,
            ReclaimableCallbackRootDeployment::new(
                external_id(6030, RootAdmissionId::from_normalized_identity),
                root,
                execution,
                slot,
                attribution,
            ),
        )
        .expect("cleaned root and attribution remain reusable");
        let registration_receipt = OpaqueCallbackRegistrationReceipt::from_provider(
            external_id(
                6031,
                OpaqueCallbackRegistrationReceiptId::from_normalized_identity,
            ),
            external_id(6032, OpaqueCallbackRegistrationId::from_normalized_identity),
            external_id(6033, OpaqueCallbackProviderId::from_normalized_identity),
            external_id(
                6034,
                OpaqueCallbackUnregistrationContractId::from_normalized_identity,
            ),
            pending.root(),
            true,
        );
        let nonquiescent = RootRemovalReceipt::from_provider(
            external_id(6035, RootRemovalReceiptId::from_normalized_identity),
            pending.root(),
            true,
            false,
        );
        let quiescent = RootRemovalReceipt::from_provider(
            external_id(6037, RootRemovalReceiptId::from_normalized_identity),
            pending.root(),
            true,
            true,
        );
        let registered = pending
            .admit_registration(registration_receipt)
            .expect("exact true registrar result establishes registration custody");
        let incomplete = OpaqueCallbackUnregistrationReceipt::from_provider(
            external_id(
                6036,
                OpaqueCallbackUnregistrationReceiptId::from_normalized_identity,
            ),
            registered.registration(),
            false,
        );
        let rejected = registered
            .unregister_and_quiesce(incomplete, nonquiescent)
            .expect_err("failed provider unregister retains live custody");
        let (registered, _, _) = (*rejected).into_parts();
        let unregistered = OpaqueCallbackUnregistrationReceipt::from_provider(
            external_id(
                6038,
                OpaqueCallbackUnregistrationReceiptId::from_normalized_identity,
            ),
            registered.registration(),
            true,
        );
        let completed = registered
            .unregister_and_quiesce(unregistered, quiescent)
            .expect("unregister and quiescence return exact attribution and slot");
        let (completed, attribution) = completed.into_parts();
        assert_eq!(attribution.entry(), entry_id);
        assert_eq!(completed.into_slot_authority().slot(), slot_identity);
        assert!(roots.record(root_identity).is_none());
    }
}
