//! Admitting one validated root into a slot: the slot authority, the
//! admission record that pins the provider execution, and the entry and
//! result qualifications a runtime subject arrives under.

use crate::executable_installation::{
    ArtifactId, InstalledCode, InstalledCodeContext, InstalledCodeId,
};
use crate::external_roots::OpaqueProviderExitAssurance;
use crate::external_roots::ProviderExecution;
use crate::external_roots::ValidatedExternalRoot;
use crate::external_roots::{
    ExternalRootDiagnostic, InterruptAcknowledgementId, InterruptEntryReceiptId,
    InterruptInvocationId, InterruptMaskGuardId, InterruptMaskTransitionReceiptId,
    ProviderExecutionId, ProviderPlanId, RootAdmissionId, RootSlotId, RootSlotOwnerId,
    TrustReceiptId,
};
use abstract_operations_to_target_operations::calling_conventions::ValuePlacement;
use installation_evidence::ObjectEvidence;
use std::collections::BTreeSet;
use terminal_psi::layout_plans::EntryStubId;

pub(crate) fn bind_terminal_function<TerminalArtifact: ObjectEvidence>(
    artifact: &TerminalArtifact,
    installed_code: &InstalledCode,
    entry: EntryStubId,
    text_offset: usize,
) -> Result<(), ExternalRootDiagnostic> {
    if artifact.architecture() != installed_code.architecture() {
        return Err(ExternalRootDiagnostic(
            "terminal artifact architecture does not match the installed executable".into(),
        ));
    }
    if !installed_code.binds_exact_unrelocated_artifact_bytes(artifact.text_bytes()) {
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
    pub(crate) provider_plan: ProviderPlanId,
    pub(crate) requirement_identity: String,
    pub(crate) parameter_index: usize,
    pub(crate) abi_placement: ValuePlacement,
    pub(crate) domain: String,
    pub(crate) effective_carry: language_semantics::CarryPolicy,
    pub(crate) entry_receipt: InterruptEntryReceiptId,
    pub(crate) invocation: InterruptInvocationId,
    pub(crate) subject: AdmittedEntrySubject,
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
        effective_carry: language_semantics::CarryPolicy,
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

    pub const fn effective_carry(&self) -> language_semantics::CarryPolicy {
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
    pub effective_carry: language_semantics::CarryPolicy,
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
    pub(crate) slot: RootSlotId,
    pub(crate) owner: RootSlotOwnerId,
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

    /// Canonical authority coordinates for one member of the owning target's
    /// complete required-root catalog.
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
        Ok(Self {
            slot: RootSlotId::for_target_required_root_slot(declaration)?,
            owner: RootSlotOwnerId::for_target_profile(declaration.owner())?,
        })
    }

    /// Canonical authority coordinates for one target-owned `ProgramEntry`
    /// declaration. Target-slot identity is derived in one place; callers do
    /// not restate a numeric slot or owner identity.
    pub fn for_target_program_entry(
        slot: target::ProgramEntrySlotDeclaration,
    ) -> Result<Self, ExternalRootDiagnostic> {
        Self::for_target_required_root_slot(
            target::TargetRequiredRootSlotDeclaration::ProgramEntry(slot),
        )
    }
}

/// Admission commitment for one exact root, installed-code realization, and
/// owner-controlled destination slot. Construction represents provider
/// admission; ordinary callers cannot weaken its private bindings.
#[derive(Debug, PartialEq, Eq)]
pub struct RootAdmission {
    pub(crate) identity: RootAdmissionId,
    pub(crate) root_evidence: ValidatedExternalRoot,
    pub(crate) provider_execution_evidence: ProviderExecution,
    pub(crate) root_report_identity: u64,
    pub(crate) provider_execution: ProviderExecutionId,
    pub(crate) provider_execution_report_fingerprint: u64,
    pub(crate) provider_exit_assurance: OpaqueProviderExitAssurance,
    pub(crate) provider_exit_assurance_report_fingerprint: u64,
    pub(crate) provider_plan: ProviderPlanId,
    pub(crate) installed_code: InstalledCodeId,
    pub(crate) installed_code_context: InstalledCodeContext,
    pub(crate) artifact: ArtifactId,
    pub(crate) slot: RootSlotId,
    pub(crate) owner: RootSlotOwnerId,
    pub(crate) trust_receipts: BTreeSet<TrustReceiptId>,
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
            root_report_identity: root.normalized_report_identity,
            provider_execution: execution.identity,
            provider_execution_report_fingerprint: execution.normalized_report_identity,
            provider_exit_assurance: execution.exit_assurance,
            provider_exit_assurance_report_fingerprint: execution.exit_assurance_report_fingerprint,
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
