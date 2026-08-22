//! Normalized ledger for entry points invoked from outside Omega's call graph.
//!
//! Installing code does not make any of its entries analysis roots. A slot
//! owner separately installs one admitted entry under a validated boundary
//! plan. That operation records the root's effects, trust receipts, stack and
//! nesting policy, WCSU demand, and component/version pins. The returned
//! handle borrows the installed code, preventing retirement while the root is
//! reachable.

use std::collections::{BTreeMap, BTreeSet};

use omega_calling_conventions::{BoundaryEntryPlan, EntryControl, ValuePlacement};
#[cfg(test)]
use omega_calling_conventions::{
    EntryStack, MachineRegister, ProviderExitRealization, StateFootprintEvidence,
    ValidatedBoundaryEntryPlan,
};
pub use omega_executable_installation::{ArtifactId, InstalledCodeId};
use omega_executable_installation::{InstalledCode, InstalledCodeContext};
pub use omega_terminal_installation_evidence::{
    TerminalObjectEvidence, TerminalStackDemandEvidence,
};
pub use psi_core::FuelScheduleIdentity;
use psi_layout_plans::EntryStubId;

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
normalized_id!(ProviderFuelSummaryId, "fixed-fuel provider summary");
normalized_id!(FuelProvisionId, "logical-fuel provision");
normalized_id!(
    ProviderFuelValidationReceiptId,
    "fixed-fuel provider validation receipt"
);
normalized_id!(FuelValidationReceiptId, "logical-fuel validation receipt");
normalized_id!(
    FuelSuspensionValidationReceiptId,
    "fuel-suspension validation receipt"
);
normalized_id!(NativeFuelMeterPlanId, "native fuel-meter plan");
normalized_id!(SponsorContextTransportId, "sponsor-context transport");
normalized_id!(
    FuelExhaustionTransferPlanId,
    "fuel-exhaustion transfer plan"
);
normalized_id!(
    DynamicFuelMeterValidationReceiptId,
    "dynamic fuel-meter validation receipt"
);
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

mod fixed_fuel;
pub use fixed_fuel::*;
mod native_fuel;
pub use native_fuel::*;
mod opaque_callback_replacement;
pub use opaque_callback_replacement::*;
mod provider_execution;
pub use provider_execution::*;
mod root_validation;
pub use root_validation::*;
mod stack_demand;
pub use stack_demand::*;

fn bind_terminal_function<TerminalArtifact: TerminalObjectEvidence>(
    terminal_artifact: &TerminalArtifact,
    installed_code: &InstalledCode,
    entry: EntryStubId,
    text_offset: usize,
) -> Result<(), ExternalRootDiagnostic> {
    if terminal_artifact.architecture() != installed_code.architecture() {
        return Err(ExternalRootDiagnostic(
            "terminal artifact architecture does not match the installed executable".into(),
        ));
    }
    if !installed_code.binds_exact_unrelocated_artifact_bytes(terminal_artifact.text_bytes()) {
        return Err(ExternalRootDiagnostic(
            "installed executable does not retain the exact relocation-free terminal artifact bytes"
                .into(),
        ));
    }
    let text_offset = u64::try_from(text_offset).map_err(|_| {
        ExternalRootDiagnostic(
            "terminal function offset cannot be represented by installation metadata".into(),
        )
    })?;
    if !installed_code.binds_entry_offset(entry, text_offset) {
        return Err(ExternalRootDiagnostic(
            "selected installed entry does not name the certified terminal function offset".into(),
        ));
    }
    Ok(())
}

/// Invocation-specific evidence that one runtime subject entered through an
/// accepted source qualification on the installed root's exact requirement.
#[derive(Debug, PartialEq, Eq)]
pub struct AdmittedEntryQualification {
    provider_plan: ProviderPlanId,
    requirement_identity: String,
    parameter_index: usize,
    abi_placement: ValuePlacement,
    domain: String,
    effective_carry: psi_language_semantics::CarryPolicy,
    entry_receipt: InterruptEntryReceiptId,
    invocation: InterruptInvocationId,
    subject: AdmittedEntrySubject,
}

impl AdmittedEntryQualification {
    /// Match this unforgeable occurrence against the compiler-owned static
    /// parameter contract. The receipt/invocation/subject remain bound inside
    /// the value; callers can inspect but cannot construct or restate them.
    pub fn matches_contract(
        &self,
        provider_plan: ProviderPlanId,
        requirement_identity: &str,
        parameter_index: usize,
        domain: &str,
        effective_carry: psi_language_semantics::CarryPolicy,
    ) -> bool {
        self.provider_plan == provider_plan
            && self.requirement_identity == requirement_identity
            && self.parameter_index == parameter_index
            && self.domain == domain
            && self.effective_carry == effective_carry
    }

    pub const fn provider_plan(&self) -> ProviderPlanId {
        self.provider_plan
    }

    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    pub const fn parameter_index(&self) -> usize {
        self.parameter_index
    }

    /// Exact inbound ABI placement selected for this semantic parameter.
    ///
    /// The semantic index remains authoritative until this occurrence is
    /// admitted. Entry lowering may then consume this placement without
    /// rediscovering a parameter by source name or physical register.
    pub const fn abi_placement(&self) -> &ValuePlacement {
        &self.abi_placement
    }

    /// Match the semantic subject and exact normalized placement consumed by
    /// one generated entry-prologue parameter capture.
    pub fn matches_parameter_placement(
        &self,
        parameter_index: usize,
        placement: &ValuePlacement,
    ) -> bool {
        self.parameter_index == parameter_index && self.abi_placement == *placement
    }

    pub fn domain(&self) -> &str {
        &self.domain
    }

    pub const fn effective_carry(&self) -> psi_language_semantics::CarryPolicy {
        self.effective_carry
    }

    pub const fn entry_receipt(&self) -> InterruptEntryReceiptId {
        self.entry_receipt
    }

    pub const fn invocation(&self) -> InterruptInvocationId {
        self.invocation
    }

    pub const fn subject(&self) -> AdmittedEntrySubject {
        self.subject
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmittedEntrySubject {
    InterruptAcknowledgement(InterruptAcknowledgementId),
}

/// Concrete result evidence minted only after the provider's exact transition
/// receipt has changed the interrupt-mask state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedResultQualification {
    pub provider_plan: ProviderPlanId,
    pub requirement_identity: String,
    pub domain: String,
    pub effective_carry: psi_language_semantics::CarryPolicy,
    pub transition_receipt: InterruptMaskTransitionReceiptId,
    pub invocation: InterruptInvocationId,
    pub subject: AdmittedResultSubject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmittedResultSubject {
    InterruptMaskGuard(InterruptMaskGuardId),
}

/// Linear authority over one external-entry destination slot.
#[derive(Debug, PartialEq, Eq)]
pub struct RootSlotAuthority {
    slot: RootSlotId,
    owner: RootSlotOwnerId,
}

impl RootSlotAuthority {
    pub const fn from_admitted_owner(slot: RootSlotId, owner: RootSlotOwnerId) -> Self {
        Self { slot, owner }
    }

    pub const fn slot(&self) -> RootSlotId {
        self.slot
    }

    pub const fn owner(&self) -> RootSlotOwnerId {
        self.owner
    }
}

/// Admission commitment for one exact root, installed-code realization, and
/// owner-controlled destination slot. Construction represents provider
/// admission; ordinary callers cannot weaken its private bindings.
#[derive(Debug, PartialEq, Eq)]
pub struct RootAdmission {
    identity: RootAdmissionId,
    root_evidence: ValidatedExternalRoot,
    provider_execution_evidence: ProviderExecution,
    root_identity: u64,
    provider_execution: ProviderExecutionId,
    provider_execution_fingerprint: u64,
    provider_exit_assurance: OpaqueProviderExitAssurance,
    provider_exit_assurance_fingerprint: u64,
    provider_plan: ProviderPlanId,
    installed_code: InstalledCodeId,
    installed_code_context: InstalledCodeContext,
    artifact: ArtifactId,
    slot: RootSlotId,
    owner: RootSlotOwnerId,
    trust_receipts: BTreeSet<TrustReceiptId>,
}

impl RootAdmission {
    pub fn from_admitted_provider(
        identity: RootAdmissionId,
        root: &ValidatedExternalRoot,
        execution: &ProviderExecution,
        installed_code: &InstalledCode,
        slot: &RootSlotAuthority,
        trust_receipts: impl IntoIterator<Item = TrustReceiptId>,
    ) -> Result<Self, ExternalRootDiagnostic> {
        if !execution.matches_root(root) {
            return Err(ExternalRootDiagnostic(
                "root admission provider execution does not bind the exact validated root realization"
                    .into(),
            ));
        }
        Ok(Self {
            identity,
            root_evidence: root.clone(),
            provider_execution_evidence: execution.clone(),
            root_identity: root.normalized_identity,
            provider_execution: execution.identity,
            provider_execution_fingerprint: execution.normalized_identity,
            provider_exit_assurance: execution.exit_assurance,
            provider_exit_assurance_fingerprint: execution.exit_assurance_fingerprint,
            provider_plan: execution.provider_plan,
            installed_code: installed_code.identity(),
            installed_code_context: installed_code.receipt_context(),
            artifact: installed_code.artifact(),
            slot: slot.slot,
            owner: slot.owner,
            trust_receipts: trust_receipts.into_iter().collect(),
        })
    }

    pub const fn identity(&self) -> RootAdmissionId {
        self.identity
    }
}

/// Reportable root record. It contains normalized identities and the complete
/// boundary plan, never a numeric code address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledRootRecord {
    pub root: ExternalRootId,
    /// Normalizer-owned identity of the complete root candidate plus its
    /// validated boundary contract. This is distinct from the friendly/root
    /// slot identity and remains stable across installation placement.
    pub normalized_root_identity: u64,
    pub entry: EntryStubId,
    pub installed_code: InstalledCodeId,
    pub artifact: ArtifactId,
    pub slot: RootSlotId,
    pub owner: RootSlotOwnerId,
    pub admission: RootAdmissionId,
    pub provider_execution: ProviderExecutionId,
    pub provider_execution_fingerprint: u64,
    pub provider_exit_assurance: OpaqueProviderExitAssurance,
    pub provider_exit_assurance_fingerprint: u64,
    pub provider_plan: ProviderPlanId,
    pub requirement_identity: String,
    pub entry_claims: Vec<ExternalRootEntryClaim>,
    pub acknowledgement_parameter_index: Option<usize>,
    pub interrupt_mask_guard_claim: Option<ExternalRootResultClaim>,
    pub boundary_contract_fingerprint: u64,
    pub boundary: BoundaryEntryPlan,
    pub provider: RootProviderId,
    pub effects: BTreeSet<RootEffectId>,
    pub trust_receipts: BTreeSet<TrustReceiptId>,
    pub nesting_relation: NestingRelationId,
    pub acknowledgement_policy: Option<AcknowledgementPolicyId>,
    pub stack: StackResourceColumn,
    pub logical_fuel: LogicalFuelResourceColumn,
    pub machine_state: MachineStateResourceColumn,
    pub component_pins: BTreeSet<ComponentVersionPin>,
}

/// Linear liveness pin for one installed external root. Borrowing the code is
/// intentional: retirement needs ownership of `InstalledCode`, which cannot
/// be recovered until every root handle has been removed.
#[derive(Debug, Clone, PartialEq, Eq)]
struct InstalledRootEvidence {
    root: ValidatedExternalRoot,
    provider_execution: ProviderExecution,
    installed_code: InstalledCodeContext,
    slot: RootSlotId,
    owner: RootSlotOwnerId,
}

#[derive(Debug)]
pub struct InstalledExternalRoot<'code> {
    root: ExternalRootId,
    slot: RootSlotId,
    owner: RootSlotOwnerId,
    installed_code: &'code InstalledCode,
    evidence: InstalledRootEvidence,
}

impl InstalledExternalRoot<'_> {
    pub const fn root(&self) -> ExternalRootId {
        self.root
    }

    pub const fn slot(&self) -> RootSlotId {
        self.slot
    }

    pub const fn installed_code(&self) -> InstalledCodeId {
        self.installed_code.identity()
    }
}

/// Provider evidence for one concrete invocation of an installed interrupt
/// root. The exact installed realization and acknowledgement policy are bound
/// before the source-visible opaque obligations are minted.
#[derive(Debug, PartialEq, Eq)]
pub struct InterruptEntryReceipt {
    identity: InterruptEntryReceiptId,
    installed_root: InstalledRootEvidence,
    root: ExternalRootId,
    slot: RootSlotId,
    installed_code: InstalledCodeId,
    provider_execution: ProviderExecutionId,
    invocation: InterruptInvocationId,
    mask_control: InterruptMaskControlId,
    initial_mask_state: InterruptMaskStateId,
    acknowledgement_policy: Option<AcknowledgementPolicyId>,
    acknowledgement: Option<InterruptAcknowledgementId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InterruptInvocationEvidence {
    installed_root: InstalledRootEvidence,
    entry_receipt: InterruptEntryReceiptId,
    invocation: InterruptInvocationId,
    mask_control: InterruptMaskControlId,
    initial_mask_state: InterruptMaskStateId,
    acknowledgement_policy: Option<AcknowledgementPolicyId>,
    acknowledgement: Option<InterruptAcknowledgementId>,
}

impl InterruptInvocationEvidence {
    fn from_entry_receipt(receipt: &InterruptEntryReceipt) -> Self {
        Self {
            installed_root: receipt.installed_root.clone(),
            entry_receipt: receipt.identity,
            invocation: receipt.invocation,
            mask_control: receipt.mask_control,
            initial_mask_state: receipt.initial_mask_state,
            acknowledgement_policy: receipt.acknowledgement_policy,
            acknowledgement: receipt.acknowledgement,
        }
    }
}

impl InterruptEntryReceipt {
    pub fn from_provider(
        identity: InterruptEntryReceiptId,
        root: &InstalledExternalRoot<'_>,
        invocation: InterruptInvocationId,
        mask_control: InterruptMaskControlId,
        initial_mask_state: InterruptMaskStateId,
        acknowledgement_policy: Option<AcknowledgementPolicyId>,
        acknowledgement: Option<InterruptAcknowledgementId>,
    ) -> Self {
        Self {
            identity,
            installed_root: root.evidence.clone(),
            root: root.root,
            slot: root.slot,
            installed_code: root.installed_code.identity(),
            provider_execution: root.evidence.provider_execution.identity,
            invocation,
            mask_control,
            initial_mask_state,
            acknowledgement_policy,
            acknowledgement,
        }
    }

    pub const fn identity(&self) -> InterruptEntryReceiptId {
        self.identity
    }
}

/// The provider-owned half of an active interrupt. It must be reunited with
/// the exact restored mask control and completed acknowledgement before the
/// ledger accepts the deriver-owned exit.
#[derive(Debug, PartialEq, Eq)]
pub struct PendingInterruptExit {
    entry_receipt: InterruptEntryReceiptId,
    invocation_evidence: InterruptInvocationEvidence,
    root: ExternalRootId,
    installed_code: InstalledCodeId,
    provider_execution: ProviderExecutionId,
    invocation: InterruptInvocationId,
    mask_control: InterruptMaskControlId,
    initial_mask_state: InterruptMaskStateId,
    acknowledgement_policy: Option<AcknowledgementPolicyId>,
    acknowledgement: Option<InterruptAcknowledgementId>,
}

/// Provider-minted source obligations for one admitted interrupt invocation.
#[derive(Debug, PartialEq, Eq)]
pub struct InterruptEntryObligations {
    pending_exit: PendingInterruptExit,
    mask_control: InterruptMaskControl,
    acknowledgement: Option<InterruptAcknowledgement>,
}

impl InterruptEntryObligations {
    pub fn into_parts(
        self,
    ) -> (
        PendingInterruptExit,
        InterruptMaskControl,
        Option<InterruptAcknowledgement>,
    ) {
        (self.pending_exit, self.mask_control, self.acknowledgement)
    }
}

/// Opaque provider control corresponding to the source
/// `InterruptMaskControl`. The stack records exact prior-state guards so nested
/// save/restore operations must settle in LIFO order.
#[derive(Debug, PartialEq, Eq)]
pub struct InterruptMaskControl {
    invocation_evidence: InterruptInvocationEvidence,
    identity: InterruptMaskControlId,
    root: ExternalRootId,
    invocation: InterruptInvocationId,
    initial_state: InterruptMaskStateId,
    current_state: InterruptMaskStateId,
    live_guards: Vec<InterruptMaskGuardId>,
    used_guards: BTreeSet<InterruptMaskGuardId>,
    mask_guard_claim: Option<ExternalRootResultClaim>,
}

impl InterruptMaskControl {
    pub const fn identity(&self) -> InterruptMaskControlId {
        self.identity
    }

    pub const fn current_state(&self) -> InterruptMaskStateId {
        self.current_state
    }

    pub fn save_and_mask(
        &mut self,
        receipt: InterruptMaskSaveReceipt,
    ) -> Result<InterruptMaskGuard, InterruptMaskSaveError> {
        let Some(mask_guard_claim) = self.mask_guard_claim.as_ref() else {
            return Err(InterruptMaskSaveError {
                receipt,
                diagnostic: ExternalRootDiagnostic(
                    "interrupt-mask save has no admitted routed result contract".into(),
                ),
            });
        };
        let matches = receipt.root == self.root
            && receipt.invocation_evidence == self.invocation_evidence
            && receipt.invocation == self.invocation
            && receipt.control == self.identity
            && receipt.prior_state == self.current_state
            && receipt.prior_state_saved_exactly
            && !self.used_guards.contains(&receipt.guard);
        if !matches {
            return Err(InterruptMaskSaveError {
                receipt,
                diagnostic: ExternalRootDiagnostic(
                    "interrupt-mask save receipt does not bind the exact control, invocation, fresh guard, and prior state"
                        .into(),
                ),
            });
        }
        self.current_state = receipt.masked_state;
        self.live_guards.push(receipt.guard);
        self.used_guards.insert(receipt.guard);
        Ok(InterruptMaskGuard {
            invocation_evidence: self.invocation_evidence.clone(),
            identity: receipt.guard,
            root: self.root,
            invocation: self.invocation,
            control: self.identity,
            prior_state: receipt.prior_state,
            masked_state: receipt.masked_state,
            qualification: AdmittedResultQualification {
                provider_plan: mask_guard_claim.provider_plan,
                requirement_identity: mask_guard_claim.requirement_identity.clone(),
                domain: mask_guard_claim.domain.clone(),
                effective_carry: mask_guard_claim.effective_carry,
                transition_receipt: receipt.identity,
                invocation: self.invocation,
                subject: AdmittedResultSubject::InterruptMaskGuard(receipt.guard),
            },
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct InterruptMaskSaveReceipt {
    identity: InterruptMaskTransitionReceiptId,
    invocation_evidence: InterruptInvocationEvidence,
    root: ExternalRootId,
    invocation: InterruptInvocationId,
    control: InterruptMaskControlId,
    guard: InterruptMaskGuardId,
    prior_state: InterruptMaskStateId,
    masked_state: InterruptMaskStateId,
    prior_state_saved_exactly: bool,
}

impl InterruptMaskSaveReceipt {
    pub fn from_provider(
        identity: InterruptMaskTransitionReceiptId,
        control: &InterruptMaskControl,
        guard: InterruptMaskGuardId,
        masked_state: InterruptMaskStateId,
        prior_state_saved_exactly: bool,
    ) -> Self {
        Self {
            identity,
            invocation_evidence: control.invocation_evidence.clone(),
            root: control.root,
            invocation: control.invocation,
            control: control.identity,
            guard,
            prior_state: control.current_state,
            masked_state,
            prior_state_saved_exactly,
        }
    }
}

/// Opaque linear guard corresponding to the source `InterruptMaskGuard`.
#[derive(Debug, PartialEq, Eq)]
pub struct InterruptMaskGuard {
    invocation_evidence: InterruptInvocationEvidence,
    identity: InterruptMaskGuardId,
    root: ExternalRootId,
    invocation: InterruptInvocationId,
    control: InterruptMaskControlId,
    prior_state: InterruptMaskStateId,
    masked_state: InterruptMaskStateId,
    qualification: AdmittedResultQualification,
}

impl InterruptMaskGuard {
    pub const fn identity(&self) -> InterruptMaskGuardId {
        self.identity
    }

    pub const fn qualification(&self) -> &AdmittedResultQualification {
        &self.qualification
    }

    pub fn restore(
        self,
        control: &mut InterruptMaskControl,
        receipt: InterruptMaskRestoreReceipt,
    ) -> Result<(), Box<InterruptMaskRestoreError>> {
        let top = control.live_guards.last().copied();
        let matches = self.root == control.root
            && self.invocation_evidence == control.invocation_evidence
            && receipt.invocation_evidence == self.invocation_evidence
            && self.invocation == control.invocation
            && self.control == control.identity
            && self.masked_state == control.current_state
            && top == Some(self.identity)
            && receipt.root == self.root
            && receipt.invocation == self.invocation
            && receipt.control == self.control
            && receipt.guard == self.identity
            && receipt.restored_state == self.prior_state
            && receipt.restored_exactly;
        if !matches {
            return Err(Box::new(InterruptMaskRestoreError {
                guard: self,
                receipt,
                diagnostic: ExternalRootDiagnostic(
                    "interrupt-mask restore receipt does not settle the newest exact saved state"
                        .into(),
                ),
            }));
        }
        control.live_guards.pop();
        control.current_state = self.prior_state;
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct InterruptMaskRestoreReceipt {
    identity: InterruptMaskTransitionReceiptId,
    invocation_evidence: InterruptInvocationEvidence,
    root: ExternalRootId,
    invocation: InterruptInvocationId,
    control: InterruptMaskControlId,
    guard: InterruptMaskGuardId,
    restored_state: InterruptMaskStateId,
    restored_exactly: bool,
}

impl InterruptMaskRestoreReceipt {
    pub fn from_provider(
        identity: InterruptMaskTransitionReceiptId,
        guard: &InterruptMaskGuard,
        restored_exactly: bool,
    ) -> Self {
        Self {
            identity,
            invocation_evidence: guard.invocation_evidence.clone(),
            root: guard.root,
            invocation: guard.invocation,
            control: guard.control,
            guard: guard.identity,
            restored_state: guard.prior_state,
            restored_exactly,
        }
    }
}

/// Opaque linear acknowledgement minted only by an admitted entry receipt.
#[derive(Debug, PartialEq, Eq)]
pub struct InterruptAcknowledgement {
    invocation_evidence: InterruptInvocationEvidence,
    identity: InterruptAcknowledgementId,
    root: ExternalRootId,
    provider_execution: ProviderExecutionId,
    invocation: InterruptInvocationId,
    policy: AcknowledgementPolicyId,
    qualifications: Vec<AdmittedEntryQualification>,
}

impl InterruptAcknowledgement {
    pub const fn identity(&self) -> InterruptAcknowledgementId {
        self.identity
    }

    /// Exact admitted source qualifications established for this concrete
    /// acknowledgement subject by the installed-root invocation receipt.
    pub fn qualifications(&self) -> &[AdmittedEntryQualification] {
        &self.qualifications
    }

    /// Resolve one exact static accepted-claim contract from this concrete
    /// linear occurrence. This never accepts a provider-plan receipt alone and
    /// never returns evidence detached from the acknowledgement carrier.
    pub fn qualification_for_contract(
        &self,
        provider_plan: ProviderPlanId,
        requirement_identity: &str,
        parameter_index: usize,
        domain: &str,
        effective_carry: psi_language_semantics::CarryPolicy,
    ) -> Result<&AdmittedEntryQualification, ExternalRootDiagnostic> {
        let matches = self
            .qualifications
            .iter()
            .filter(|qualification| {
                qualification.matches_contract(
                    provider_plan,
                    requirement_identity,
                    parameter_index,
                    domain,
                    effective_carry,
                )
            })
            .collect::<Vec<_>>();
        let [qualification] = matches.as_slice() else {
            return Err(ExternalRootDiagnostic(format!(
                "interrupt acknowledgement maps to {} qualifications for the exact accepted entry contract",
                matches.len()
            )));
        };
        Ok(*qualification)
    }

    pub fn complete(
        self,
        receipt: InterruptAcknowledgementReceipt,
    ) -> Result<CompletedInterruptAcknowledgement, Box<InterruptAcknowledgementError>> {
        let matches = receipt.root == self.root
            && receipt.invocation_evidence == self.invocation_evidence
            && receipt.provider_execution == self.provider_execution
            && receipt.invocation == self.invocation
            && receipt.policy == self.policy
            && receipt.acknowledgement == self.identity
            && receipt.source_acknowledged;
        if !matches {
            return Err(Box::new(InterruptAcknowledgementError {
                acknowledgement: self,
                receipt,
                diagnostic: ExternalRootDiagnostic(
                    "interrupt acknowledgement receipt does not complete the exact invocation and policy"
                        .into(),
                ),
            }));
        }
        Ok(CompletedInterruptAcknowledgement {
            invocation_evidence: self.invocation_evidence,
            root: self.root,
            provider_execution: self.provider_execution,
            invocation: self.invocation,
            policy: self.policy,
            acknowledgement: self.identity,
            receipt: receipt.identity,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct InterruptAcknowledgementReceipt {
    identity: InterruptAcknowledgementReceiptId,
    invocation_evidence: InterruptInvocationEvidence,
    root: ExternalRootId,
    provider_execution: ProviderExecutionId,
    invocation: InterruptInvocationId,
    policy: AcknowledgementPolicyId,
    acknowledgement: InterruptAcknowledgementId,
    source_acknowledged: bool,
}

impl InterruptAcknowledgementReceipt {
    pub fn from_provider(
        identity: InterruptAcknowledgementReceiptId,
        acknowledgement: &InterruptAcknowledgement,
        source_acknowledged: bool,
    ) -> Self {
        Self {
            identity,
            invocation_evidence: acknowledgement.invocation_evidence.clone(),
            root: acknowledgement.root,
            provider_execution: acknowledgement.provider_execution,
            invocation: acknowledgement.invocation,
            policy: acknowledgement.policy,
            acknowledgement: acknowledgement.identity,
            source_acknowledged,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct CompletedInterruptAcknowledgement {
    invocation_evidence: InterruptInvocationEvidence,
    root: ExternalRootId,
    provider_execution: ProviderExecutionId,
    invocation: InterruptInvocationId,
    policy: AcknowledgementPolicyId,
    acknowledgement: InterruptAcknowledgementId,
    receipt: InterruptAcknowledgementReceiptId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompletedInterruptEntry {
    pub entry_receipt: InterruptEntryReceiptId,
    pub root: ExternalRootId,
    pub invocation: InterruptInvocationId,
    pub acknowledgement_receipt: Option<InterruptAcknowledgementReceiptId>,
}

#[derive(Debug, Default)]
pub struct InstalledRootLedger {
    roots: BTreeMap<ExternalRootId, InstalledRootRecord>,
    root_evidence: BTreeMap<ExternalRootId, InstalledRootEvidence>,
    slots: BTreeSet<RootSlotId>,
    active_interrupts: BTreeSet<(ExternalRootId, InterruptInvocationId)>,
    entered_interrupts: BTreeSet<(ProviderExecutionId, InterruptInvocationId)>,
    minted_acknowledgements: BTreeSet<(ProviderExecutionId, InterruptAcknowledgementId)>,
}

impl InstalledRootLedger {
    pub fn records(&self) -> impl Iterator<Item = &InstalledRootRecord> {
        self.roots.values()
    }

    pub fn record(&self, root: ExternalRootId) -> Option<&InstalledRootRecord> {
        self.roots.get(&root)
    }

    /// Mint the opaque source obligations for one provider-reported interrupt
    /// invocation. Ordinary code has no constructor for these carriers; the
    /// receipt must match the exact installed root, selected execution, and
    /// acknowledgement policy. An invocation or acknowledgement identity can
    /// be admitted only once by a selected provider execution.
    pub fn begin_interrupt_entry(
        &mut self,
        root: &InstalledExternalRoot<'_>,
        receipt: InterruptEntryReceipt,
    ) -> Result<InterruptEntryObligations, InterruptEntryStartError> {
        let Some(record) = self.roots.get(&root.root) else {
            return Err(InterruptEntryStartError {
                receipt,
                diagnostic: ExternalRootDiagnostic(
                    "interrupt entry requires a currently installed external root".into(),
                ),
            });
        };
        let acknowledgement_shape_matches = match (
            record.acknowledgement_policy,
            receipt.acknowledgement_policy,
            receipt.acknowledgement,
        ) {
            (None, None, None) => true,
            (Some(expected), Some(actual), Some(_)) => expected == actual,
            _ => false,
        };
        let exact_root = root.slot == record.slot
            && root.installed_code.identity() == record.installed_code
            && self.root_evidence.get(&root.root).is_some_and(|evidence| {
                evidence == &root.evidence && evidence == &receipt.installed_root
            })
            && receipt.root == record.root
            && receipt.slot == record.slot
            && receipt.installed_code == record.installed_code
            && receipt.provider_execution == record.provider_execution
            && acknowledgement_shape_matches
            && (receipt.acknowledgement.is_none()
                || record.acknowledgement_parameter_index.is_some())
            && record.boundary.call.entry_control == EntryControl::InterruptReturn;
        if !exact_root {
            return Err(InterruptEntryStartError {
                receipt,
                diagnostic: ExternalRootDiagnostic(
                    "interrupt entry receipt does not bind the exact installed interrupt root, provider execution, and acknowledgement policy"
                        .into(),
                ),
            });
        }
        let entry_key = (record.provider_execution, receipt.invocation);
        let acknowledgement_key = receipt
            .acknowledgement
            .map(|identity| (record.provider_execution, identity));
        if self.entered_interrupts.contains(&entry_key)
            || acknowledgement_key.is_some_and(|key| self.minted_acknowledgements.contains(&key))
        {
            return Err(InterruptEntryStartError {
                receipt,
                diagnostic: ExternalRootDiagnostic(
                    "interrupt entry receipt replays an invocation or acknowledgement identity"
                        .into(),
                ),
            });
        }
        self.entered_interrupts.insert(entry_key);
        if let Some(key) = acknowledgement_key {
            self.minted_acknowledgements.insert(key);
        }
        self.active_interrupts
            .insert((record.root, receipt.invocation));

        let invocation_evidence = InterruptInvocationEvidence::from_entry_receipt(&receipt);
        let acknowledgement = receipt.acknowledgement.map(|identity| {
            let parameter_index = record
                .acknowledgement_parameter_index
                .expect("exact interrupt root validated the acknowledgement parameter");
            let qualifications = record
                .entry_claims
                .iter()
                .filter(|claim| claim.parameter_index == parameter_index)
                .map(|claim| AdmittedEntryQualification {
                    provider_plan: record.provider_plan,
                    requirement_identity: record.requirement_identity.clone(),
                    parameter_index,
                    abi_placement: record.boundary.call.parameters[parameter_index].clone(),
                    domain: claim.domain.clone(),
                    effective_carry: claim.effective_carry,
                    entry_receipt: receipt.identity,
                    invocation: receipt.invocation,
                    subject: AdmittedEntrySubject::InterruptAcknowledgement(identity),
                })
                .collect::<Vec<_>>();
            InterruptAcknowledgement {
                invocation_evidence: invocation_evidence.clone(),
                identity,
                root: record.root,
                provider_execution: record.provider_execution,
                invocation: receipt.invocation,
                policy: record
                    .acknowledgement_policy
                    .expect("validated acknowledgement shape has a policy"),
                qualifications,
            }
        });
        Ok(InterruptEntryObligations {
            pending_exit: PendingInterruptExit {
                entry_receipt: receipt.identity,
                invocation_evidence: invocation_evidence.clone(),
                root: record.root,
                installed_code: record.installed_code,
                provider_execution: record.provider_execution,
                invocation: receipt.invocation,
                mask_control: receipt.mask_control,
                initial_mask_state: receipt.initial_mask_state,
                acknowledgement_policy: record.acknowledgement_policy,
                acknowledgement: receipt.acknowledgement,
            },
            mask_control: InterruptMaskControl {
                invocation_evidence,
                identity: receipt.mask_control,
                root: record.root,
                invocation: receipt.invocation,
                initial_state: receipt.initial_mask_state,
                current_state: receipt.initial_mask_state,
                live_guards: Vec::new(),
                used_guards: BTreeSet::new(),
                mask_guard_claim: record.interrupt_mask_guard_claim.clone(),
            },
            acknowledgement,
        })
    }

    /// Admit the deriver-owned interrupt exit only after every source-visible
    /// obligation has returned to its exact provider state.
    pub fn finish_interrupt_entry(
        &mut self,
        pending: PendingInterruptExit,
        control: InterruptMaskControl,
        acknowledgement: Option<CompletedInterruptAcknowledgement>,
    ) -> Result<CompletedInterruptEntry, Box<InterruptEntryFinishError>> {
        let acknowledgement_matches = match (
            pending.acknowledgement_policy,
            pending.acknowledgement,
            acknowledgement.as_ref(),
        ) {
            (None, None, None) => true,
            (Some(policy), Some(identity), Some(completed)) => {
                completed.invocation_evidence == pending.invocation_evidence
                    && completed.root == pending.root
                    && completed.provider_execution == pending.provider_execution
                    && completed.invocation == pending.invocation
                    && completed.policy == policy
                    && completed.acknowledgement == identity
            }
            _ => false,
        };
        let record_matches = self.roots.get(&pending.root).is_some_and(|record| {
            record.installed_code == pending.installed_code
                && record.provider_execution == pending.provider_execution
                && record.acknowledgement_policy == pending.acknowledgement_policy
                && self
                    .root_evidence
                    .get(&pending.root)
                    .is_some_and(|evidence| evidence == &pending.invocation_evidence.installed_root)
        });
        let control_matches = control.invocation_evidence == pending.invocation_evidence
            && control.root == pending.root
            && control.invocation == pending.invocation
            && control.identity == pending.mask_control
            && control.initial_state == pending.initial_mask_state
            && control.current_state == pending.initial_mask_state
            && control.live_guards.is_empty();
        let active_key = (pending.root, pending.invocation);
        if !record_matches
            || !control_matches
            || !acknowledgement_matches
            || !self.active_interrupts.contains(&active_key)
        {
            return Err(Box::new(InterruptEntryFinishError {
                pending,
                control,
                acknowledgement,
                diagnostic: ExternalRootDiagnostic(
                    "interrupt exit requires the exact restored mask state and completed acknowledgement"
                        .into(),
                ),
            }));
        }
        self.active_interrupts.remove(&active_key);
        Ok(CompletedInterruptEntry {
            entry_receipt: pending.entry_receipt,
            root: pending.root,
            invocation: pending.invocation,
            acknowledgement_receipt: acknowledgement.map(|completed| completed.receipt),
        })
    }

    /// Deterministic identity of the currently installed root set.
    ///
    /// Candidate policy is already covered by each root's normalized identity;
    /// this layer binds it to the exact installed realization and owner-scoped
    /// destination. Presentation order cannot affect the result because the
    /// ledger is keyed by the normalized `ExternalRootId`.
    pub fn report_fingerprint(&self) -> u64 {
        let mut hash = Fnv1a::new();
        hash.u64(self.roots.len() as u64);
        for record in self.roots.values() {
            hash.u64(record.normalized_root_identity);
            hash.u64(record.installed_code.normalized_identity());
            hash.u64(record.artifact.normalized_identity());
            hash.u64(record.slot.normalized_identity());
            hash.u64(record.owner.normalized_identity());
            hash.u64(record.admission.normalized_identity());
            hash.u64(record.provider_execution.normalized_identity());
            hash.u64(record.provider_execution_fingerprint);
            hash.u64(record.provider_exit_assurance_fingerprint);
            hash.u64(record.provider_plan.normalized_identity());
        }
        hash.finish()
    }

    pub fn install<'code>(
        &mut self,
        installed_code: &'code InstalledCode,
        root: ValidatedExternalRoot,
        slot: RootSlotAuthority,
        admission: RootAdmission,
    ) -> Result<InstalledExternalRoot<'code>, Box<RootInstallError>> {
        let reject = |diagnostic: ExternalRootDiagnostic,
                      root: ValidatedExternalRoot,
                      slot: RootSlotAuthority,
                      admission: RootAdmission| {
            Err(Box::new(RootInstallError {
                root,
                slot,
                admission,
                diagnostic,
            }))
        };

        if self.roots.contains_key(&root.candidate.identity) {
            return reject(
                ExternalRootDiagnostic("external-root identity is already installed".into()),
                root,
                slot,
                admission,
            );
        }
        if self.slots.contains(&slot.slot) {
            return reject(
                ExternalRootDiagnostic("external-root slot is already occupied".into()),
                root,
                slot,
                admission,
            );
        }
        if let Some(existing) = self.roots.values().next()
            && (existing.nesting_relation != root.candidate.nesting_relation
                || existing.stack.realization.composition_evidence
                    != root.candidate.stack.realization.composition_evidence
                || existing
                    .stack
                    .realization
                    .artifact_composition_fingerprint()
                    != root
                        .candidate
                        .stack
                        .realization
                        .artifact_composition_fingerprint())
        {
            return reject(
                ExternalRootDiagnostic(
                    "external-root stack realization does not match the ledger's artifact-wide nesting composition"
                        .into(),
                ),
                root,
                slot,
                admission,
            );
        }
        if let Err(diagnostic) = admission
            .provider_execution_evidence
            .validate_installed_entry_binding(installed_code)
        {
            return reject(diagnostic, root, slot, admission);
        }
        if admission.root_evidence != root
            || admission.provider_execution_evidence.root_evidence != root
            || admission.provider_execution_evidence.identity != admission.provider_execution
            || admission.provider_execution_evidence.normalized_identity
                != admission.provider_execution_fingerprint
            || admission.root_identity != root.normalized_identity
            || admission.installed_code != installed_code.identity()
            || admission.installed_code_context != installed_code.receipt_context()
            || admission.artifact != installed_code.artifact()
            || admission.slot != slot.slot
            || admission.owner != slot.owner
            || admission.trust_receipts != root.candidate.trust_receipts
        {
            return reject(
                ExternalRootDiagnostic(
                    "external-root admission does not bind the exact root, code, slot, owner, and trust receipts"
                        .into(),
                ),
                root,
                slot,
                admission,
            );
        }

        let installed_root_evidence = InstalledRootEvidence {
            root: root.clone(),
            provider_execution: admission.provider_execution_evidence.clone(),
            installed_code: installed_code.receipt_context(),
            slot: slot.slot,
            owner: slot.owner,
        };
        let record = InstalledRootRecord {
            root: root.candidate.identity,
            normalized_root_identity: root.normalized_identity,
            entry: root.candidate.entry,
            installed_code: installed_code.identity(),
            artifact: installed_code.artifact(),
            slot: slot.slot,
            owner: slot.owner,
            admission: admission.identity,
            provider_execution: admission.provider_execution,
            provider_execution_fingerprint: admission.provider_execution_fingerprint,
            provider_exit_assurance: admission.provider_exit_assurance,
            provider_exit_assurance_fingerprint: admission.provider_exit_assurance_fingerprint,
            provider_plan: admission.provider_plan,
            requirement_identity: root.candidate.requirement_identity,
            entry_claims: root.candidate.entry_claims,
            acknowledgement_parameter_index: root.candidate.acknowledgement_parameter_index,
            interrupt_mask_guard_claim: root.candidate.interrupt_mask_guard_claim,
            boundary_contract_fingerprint: root.boundary_contract_fingerprint,
            boundary: root.boundary.plan().clone(),
            provider: root.candidate.provider,
            effects: root.candidate.effects,
            trust_receipts: root.candidate.trust_receipts,
            nesting_relation: root.candidate.nesting_relation,
            acknowledgement_policy: root.candidate.acknowledgement_policy,
            stack: root.candidate.stack,
            logical_fuel: root.candidate.logical_fuel,
            machine_state: root.candidate.machine_state,
            component_pins: root.candidate.component_pins,
        };
        let handle = InstalledExternalRoot {
            root: record.root,
            slot: record.slot,
            owner: record.owner,
            installed_code,
            evidence: installed_root_evidence.clone(),
        };
        self.slots.insert(record.slot);
        self.root_evidence
            .insert(record.root, installed_root_evidence);
        self.roots.insert(record.root, record);
        Ok(handle)
    }

    pub fn remove<'code>(
        &mut self,
        root: InstalledExternalRoot<'code>,
        receipt: RootRemovalReceipt,
    ) -> Result<RootSlotAuthority, Box<RootRemovalError<'code>>> {
        let matches = receipt.root == root.root
            && receipt.slot == root.slot
            && receipt.installed_code == root.installed_code.identity()
            && receipt.installed_root == root.evidence
            && self
                .root_evidence
                .get(&root.root)
                .is_some_and(|evidence| evidence == &root.evidence)
            && receipt.entry_unreachable
            && receipt.executions_quiesced
            && !self
                .active_interrupts
                .iter()
                .any(|(active_root, _)| *active_root == root.root);
        if !matches || !self.roots.contains_key(&root.root) {
            return Err(Box::new(RootRemovalError {
                root,
                receipt,
                diagnostic: ExternalRootDiagnostic(
                    "external-root removal receipt does not prove exact-slot unreachability and quiescence"
                        .into(),
                ),
            }));
        }
        self.roots.remove(&root.root);
        self.root_evidence.remove(&root.root);
        self.slots.remove(&root.slot);
        Ok(RootSlotAuthority {
            slot: root.slot,
            owner: root.owner,
        })
    }
}

#[derive(Debug)]
pub struct RootRemovalReceipt {
    identity: RootRemovalReceiptId,
    installed_root: InstalledRootEvidence,
    root: ExternalRootId,
    slot: RootSlotId,
    installed_code: InstalledCodeId,
    entry_unreachable: bool,
    executions_quiesced: bool,
}

impl RootRemovalReceipt {
    pub fn from_provider(
        identity: RootRemovalReceiptId,
        root: &InstalledExternalRoot<'_>,
        entry_unreachable: bool,
        executions_quiesced: bool,
    ) -> Self {
        Self {
            identity,
            installed_root: root.evidence.clone(),
            root: root.root,
            slot: root.slot,
            installed_code: root.installed_code.identity(),
            entry_unreachable,
            executions_quiesced,
        }
    }

    pub const fn identity(&self) -> RootRemovalReceiptId {
        self.identity
    }
}

#[derive(Debug)]
pub struct RootInstallError {
    root: ValidatedExternalRoot,
    slot: RootSlotAuthority,
    admission: RootAdmission,
    diagnostic: ExternalRootDiagnostic,
}

impl RootInstallError {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ValidatedExternalRoot, RootSlotAuthority, RootAdmission) {
        (self.root, self.slot, self.admission)
    }
}

#[derive(Debug)]
pub struct RootRemovalError<'code> {
    root: InstalledExternalRoot<'code>,
    receipt: RootRemovalReceipt,
    diagnostic: ExternalRootDiagnostic,
}

impl<'code> RootRemovalError<'code> {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (InstalledExternalRoot<'code>, RootRemovalReceipt) {
        (self.root, self.receipt)
    }
}

#[derive(Debug)]
pub struct InterruptEntryStartError {
    receipt: InterruptEntryReceipt,
    diagnostic: ExternalRootDiagnostic,
}

impl InterruptEntryStartError {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_receipt(self) -> InterruptEntryReceipt {
        self.receipt
    }
}

#[derive(Debug)]
pub struct InterruptMaskSaveError {
    receipt: InterruptMaskSaveReceipt,
    diagnostic: ExternalRootDiagnostic,
}

impl InterruptMaskSaveError {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_receipt(self) -> InterruptMaskSaveReceipt {
        self.receipt
    }
}

#[derive(Debug)]
pub struct InterruptMaskRestoreError {
    guard: InterruptMaskGuard,
    receipt: InterruptMaskRestoreReceipt,
    diagnostic: ExternalRootDiagnostic,
}

impl InterruptMaskRestoreError {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (InterruptMaskGuard, InterruptMaskRestoreReceipt) {
        (self.guard, self.receipt)
    }
}

#[derive(Debug)]
pub struct InterruptAcknowledgementError {
    acknowledgement: InterruptAcknowledgement,
    receipt: InterruptAcknowledgementReceipt,
    diagnostic: ExternalRootDiagnostic,
}

impl InterruptAcknowledgementError {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (InterruptAcknowledgement, InterruptAcknowledgementReceipt) {
        (self.acknowledgement, self.receipt)
    }
}

#[derive(Debug)]
pub struct InterruptEntryFinishError {
    pending: PendingInterruptExit,
    control: InterruptMaskControl,
    acknowledgement: Option<CompletedInterruptAcknowledgement>,
    diagnostic: ExternalRootDiagnostic,
}

impl InterruptEntryFinishError {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        PendingInterruptExit,
        InterruptMaskControl,
        Option<CompletedInterruptAcknowledgement>,
    ) {
        (self.pending, self.control, self.acknowledgement)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalRootDiagnostic(pub String);

impl std::fmt::Display for ExternalRootDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ExternalRootDiagnostic {}

struct Fnv1a(u64);

impl Fnv1a {
    const fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    fn u64(&mut self, value: u64) {
        for byte in value.to_le_bytes() {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn string(&mut self, value: &str) {
        self.u64(value.len() as u64);
        for byte in value.bytes() {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn bytes(&mut self, value: &[u8]) {
        self.u64(value.len() as u64);
        for byte in value {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    const fn finish(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests;
