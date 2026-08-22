//! Normalized ledger for entry points invoked from outside Omega's call graph.
//!
//! Installing code does not make any of its entries analysis roots. A slot
//! owner separately installs one admitted entry under a validated boundary
//! plan. That operation records the root's effects, trust receipts, stack and
//! nesting policy, WCSU demand, and component/version pins. The returned
//! handle borrows the installed code, preventing retirement while the root is
//! reachable.

use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
use omega_calling_conventions::EntryStack;
use omega_calling_conventions::{
    BoundaryEntryPlan, EntryControl, MachineRegister, ProviderExitRealization,
    StateFootprintEvidence, ValidatedBoundaryEntryPlan, ValuePlacement,
    validate_provider_exit_realization, validate_state_footprint,
};
pub use omega_executable_installation::{ArtifactId, InstalledCodeId};
use omega_executable_installation::{
    InstalledCode, InstalledCodeContext, ResolvedPostHandoffEntryWriterContext,
};
pub use omega_terminal_installation_evidence::{
    TerminalObjectEvidence, TerminalStackDemandEvidence,
};
pub use psi_core::FuelScheduleIdentity;
use psi_layout_plans::{
    EntryStubId, PlacementSite, PostHandoffWriterInvocationPlan, PostHandoffWriterPlan,
    RelocationTarget,
};

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

mod opaque_callback_replacement;
pub use opaque_callback_replacement::*;
mod stack_demand;
pub use stack_demand::*;

use stack_demand::fingerprint_entry_stack;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentVersionPin {
    pub contract: ComponentContractId,
    pub artifact: ComponentArtifactId,
    pub provider: ComponentProviderId,
    pub version: ComponentVersionPinId,
}

/// One bounded call edge in a fixed-fuel provider summary. Multiplicity is
/// explicit: a set of callees alone cannot distinguish one invocation from a
/// bounded repeated use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FixedFuelCall {
    pub callee: ProviderFuelSummaryId,
    pub maximum_invocations: u64,
}

/// A recomputable terminal-Psi entry theorem bound to the exact
/// relocation-free bytes and selected entry of one installed realization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledTerminalEntryFuelCertificate {
    certificate: psi_terminal_fixed_fuel::FixedEntryFuelCertificate,
    installed_code: InstalledCodeId,
    installed_code_context: InstalledCodeContext,
    artifact: ArtifactId,
    entry: EntryStubId,
}

impl InstalledTerminalEntryFuelCertificate {
    pub const fn certificate(&self) -> &psi_terminal_fixed_fuel::FixedEntryFuelCertificate {
        &self.certificate
    }

    pub const fn installed_code(&self) -> InstalledCodeId {
        self.installed_code
    }

    pub const fn artifact(&self) -> ArtifactId {
        self.artifact
    }

    pub const fn entry(&self) -> EntryStubId {
        self.entry
    }

    fn matches_installed_entry(&self, installed_code: &InstalledCode, entry: EntryStubId) -> bool {
        self.entry == entry
            && self.installed_code == installed_code.identity()
            && self.installed_code_context == installed_code.receipt_context()
            && self.artifact == installed_code.artifact()
    }
}

/// A recomputable terminal-Psi path-segment theorem bound to the exact
/// relocation-free bytes and selected function entry of one installed
/// realization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledTerminalSegmentFuelCertificate {
    certificate: psi_terminal_fixed_fuel::FixedSegmentFuelCertificate,
    installed_code: InstalledCodeId,
    installed_code_context: InstalledCodeContext,
    artifact: ArtifactId,
    entry: EntryStubId,
}

impl InstalledTerminalSegmentFuelCertificate {
    pub const fn certificate(&self) -> &psi_terminal_fixed_fuel::FixedSegmentFuelCertificate {
        &self.certificate
    }

    pub const fn installed_code(&self) -> InstalledCodeId {
        self.installed_code
    }

    pub const fn artifact(&self) -> ArtifactId {
        self.artifact
    }

    pub const fn entry(&self) -> EntryStubId {
        self.entry
    }
}

/// Bind a checked terminal-Psi entry theorem to one exact installed function.
///
/// The terminal artifact is already the checked result of Omega lowering. The
/// generic installation ladder must contain byte-for-byte identical,
/// relocation-free text, and the selected stub must name the exact function
/// offset certified here.
pub fn bind_installed_terminal_entry_fuel<TerminalArtifact: TerminalObjectEvidence>(
    certificate: psi_terminal_fixed_fuel::FixedEntryFuelCertificate,
    terminal_artifact: &TerminalArtifact,
    installed_code: &InstalledCode,
    entry: EntryStubId,
) -> Result<InstalledTerminalEntryFuelCertificate, ExternalRootDiagnostic> {
    if certificate.terminal_psi() != terminal_artifact.terminal_psi() {
        return Err(ExternalRootDiagnostic(
            "terminal fixed-fuel certificate does not name the terminal artifact's semantic identity"
                .into(),
        ));
    }
    let function_offset = terminal_artifact
        .function_text_offset(certificate.entry())
        .ok_or_else(|| {
            ExternalRootDiagnostic(
                "terminal fixed-fuel entry is not present in the emitted artifact".into(),
            )
        })?;
    bind_terminal_function(terminal_artifact, installed_code, entry, function_offset)?;
    Ok(InstalledTerminalEntryFuelCertificate {
        certificate,
        installed_code: installed_code.identity(),
        installed_code_context: installed_code.receipt_context(),
        artifact: installed_code.artifact(),
        entry,
    })
}

/// Recheck a previously sealed whole-entry theorem against the exact code and
/// entry selected for an external root.
pub fn validate_installed_terminal_entry_fuel(
    binding: &InstalledTerminalEntryFuelCertificate,
    installed_code: &InstalledCode,
    entry: EntryStubId,
) -> Result<(), ExternalRootDiagnostic> {
    if !binding.matches_installed_entry(installed_code, entry) {
        return Err(ExternalRootDiagnostic(
            "terminal fixed-fuel entry does not bind the selected installed code and entry".into(),
        ));
    }
    Ok(())
}

/// Bind a checked terminal-Psi path-segment theorem to one exact installed
/// function. The stub identifies the function containing the segment; the
/// certificate retains its semantic block/edge endpoints.
pub fn bind_installed_terminal_segment_fuel<TerminalArtifact: TerminalObjectEvidence>(
    certificate: psi_terminal_fixed_fuel::FixedSegmentFuelCertificate,
    terminal_artifact: &TerminalArtifact,
    installed_code: &InstalledCode,
    entry: EntryStubId,
) -> Result<InstalledTerminalSegmentFuelCertificate, ExternalRootDiagnostic> {
    if certificate.terminal_psi() != terminal_artifact.terminal_psi() {
        return Err(ExternalRootDiagnostic(
            "terminal fixed-fuel certificate does not name the terminal artifact's semantic identity"
                .into(),
        ));
    }
    let function_offset = terminal_artifact
        .function_text_offset(certificate.machine())
        .ok_or_else(|| {
            ExternalRootDiagnostic(
                "terminal fixed-fuel segment machine is not present in the emitted artifact".into(),
            )
        })?;
    bind_terminal_function(terminal_artifact, installed_code, entry, function_offset)?;
    Ok(InstalledTerminalSegmentFuelCertificate {
        certificate,
        installed_code: installed_code.identity(),
        installed_code_context: installed_code.receipt_context(),
        artifact: installed_code.artifact(),
        entry,
    })
}

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

/// Exact evidence for the local part of one fixed-fuel summary.
///
/// Checked terminal Psi contributes a sealed recomputable entry or segment
/// certificate. An opaque provider may instead contribute an admitted summary
/// under its validation receipt. Both use the same Psi-owned schedule identity;
/// a provider-authored number can no longer masquerade as an IR certificate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedFuelLocalEvidence {
    TerminalEntry(InstalledTerminalEntryFuelCertificate),
    TerminalSegment(InstalledTerminalSegmentFuelCertificate),
    AdmittedProvider {
        schedule: FuelScheduleIdentity,
        units: u64,
        validation_receipt: ProviderFuelValidationReceiptId,
    },
}

impl FixedFuelLocalEvidence {
    pub const fn schedule(&self) -> FuelScheduleIdentity {
        match self {
            Self::TerminalEntry(binding) => binding.certificate.schedule(),
            Self::TerminalSegment(binding) => binding.certificate.schedule(),
            Self::AdmittedProvider { schedule, .. } => *schedule,
        }
    }

    pub const fn units(&self) -> u64 {
        match self {
            Self::TerminalEntry(binding) => binding.certificate.ceiling_units(),
            Self::TerminalSegment(binding) => binding.certificate.ceiling_units(),
            Self::AdmittedProvider { units, .. } => *units,
        }
    }

    pub const fn provider_validation_receipt(&self) -> Option<ProviderFuelValidationReceiptId> {
        match self {
            Self::TerminalEntry(_) | Self::TerminalSegment(_) => None,
            Self::AdmittedProvider {
                validation_receipt, ..
            } => Some(*validation_receipt),
        }
    }
}

/// Public fixed-fuel summary supplied by checked terminal Psi or a selected
/// opaque provider. Units are deterministic logical cost under the retained
/// evidence's schedule, not native instructions, cycles, elapsed time, or
/// WCET. Absence of a summary fails closed; recursive summary graphs are
/// rejected by composition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedFuelProviderSummary {
    pub identity: ProviderFuelSummaryId,
    pub provider: RootProviderId,
    pub local_evidence: FixedFuelLocalEvidence,
    pub calls: BTreeSet<FixedFuelCall>,
}

impl FixedFuelProviderSummary {
    pub fn from_admitted_provider(
        identity: ProviderFuelSummaryId,
        provider: RootProviderId,
        schedule: FuelScheduleIdentity,
        units: u64,
        calls: BTreeSet<FixedFuelCall>,
        validation_receipt: ProviderFuelValidationReceiptId,
    ) -> Self {
        Self {
            identity,
            provider,
            local_evidence: FixedFuelLocalEvidence::AdmittedProvider {
                schedule,
                units,
                validation_receipt,
            },
            calls,
        }
    }

    pub fn from_terminal_entry(
        identity: ProviderFuelSummaryId,
        provider: RootProviderId,
        certificate: InstalledTerminalEntryFuelCertificate,
        calls: BTreeSet<FixedFuelCall>,
    ) -> Self {
        Self {
            identity,
            provider,
            local_evidence: FixedFuelLocalEvidence::TerminalEntry(certificate),
            calls,
        }
    }

    pub fn from_terminal_segment(
        identity: ProviderFuelSummaryId,
        provider: RootProviderId,
        certificate: InstalledTerminalSegmentFuelCertificate,
        calls: BTreeSet<FixedFuelCall>,
    ) -> Self {
        Self {
            identity,
            provider,
            local_evidence: FixedFuelLocalEvidence::TerminalSegment(certificate),
            calls,
        }
    }
}

/// Exact canonical provider graph retained by a composed fuel demand.
///
/// The compact composition fingerprint is presentation identity only; root
/// admission compares this graph through the sealed demand value itself.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FixedFuelCompositionEvidence {
    summaries: BTreeMap<ProviderFuelSummaryId, FixedFuelProviderSummary>,
}

/// Canonical transitive result of a fixed-fuel provider graph. The private
/// fields ensure callers cannot hand-author a demand that skipped a callee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedFuelDemand {
    root: ProviderFuelSummaryId,
    root_provider: RootProviderId,
    schedule: FuelScheduleIdentity,
    units: u64,
    summaries: BTreeSet<ProviderFuelSummaryId>,
    provider_receipts: BTreeSet<ProviderFuelValidationReceiptId>,
    composition_evidence: FixedFuelCompositionEvidence,
    composition_fingerprint: u64,
}

impl ComposedFuelDemand {
    pub const fn root(&self) -> ProviderFuelSummaryId {
        self.root
    }

    pub const fn root_provider(&self) -> RootProviderId {
        self.root_provider
    }

    pub const fn schedule(&self) -> FuelScheduleIdentity {
        self.schedule
    }

    pub const fn units(&self) -> u64 {
        self.units
    }

    pub const fn composition_fingerprint(&self) -> u64 {
        self.composition_fingerprint
    }

    pub const fn summaries(&self) -> &BTreeSet<ProviderFuelSummaryId> {
        &self.summaries
    }

    pub const fn provider_receipts(&self) -> &BTreeSet<ProviderFuelValidationReceiptId> {
        &self.provider_receipts
    }

    /// Exact local evidence retained for every summary that participated in
    /// composition. This exposes terminal certificate identity separately
    /// from opaque-provider validation receipts without permitting callers to
    /// alter the sealed graph.
    pub fn summary_evidence(
        &self,
    ) -> impl Iterator<Item = (&ProviderFuelSummaryId, &FixedFuelProviderSummary)> {
        self.composition_evidence.summaries.iter()
    }
}

/// Compose an acyclic graph of admitted fixed-fuel summaries. Each edge's
/// maximum invocation count multiplies the callee's complete demand; missing
/// summaries, zero-count edges, cycles, duplicates, and arithmetic overflow
/// all fail closed.
pub fn compose_fixed_fuel<'a>(
    root: ProviderFuelSummaryId,
    summaries: impl IntoIterator<Item = &'a FixedFuelProviderSummary>,
) -> Result<ComposedFuelDemand, ExternalRootDiagnostic> {
    let mut by_identity = BTreeMap::new();
    for summary in summaries {
        if by_identity.insert(summary.identity, summary).is_some() {
            return Err(ExternalRootDiagnostic(format!(
                "fixed-fuel summary identity 0x{:016x} is duplicated",
                summary.identity.normalized_identity()
            )));
        }
    }
    let root_summary = by_identity.get(&root).ok_or_else(|| {
        ExternalRootDiagnostic(format!(
            "fixed-fuel root summary 0x{:016x} is missing",
            root.normalized_identity()
        ))
    })?;
    let mut visiting = BTreeSet::new();
    let mut memo = BTreeMap::new();
    let mut used = BTreeSet::new();
    let schedule = root_summary.local_evidence.schedule();
    let units = compose_fixed_fuel_summary(
        root,
        schedule,
        &by_identity,
        &mut visiting,
        &mut memo,
        &mut used,
    )?;
    let provider_receipts = used
        .iter()
        .filter_map(|identity| {
            by_identity
                .get(identity)
                .expect("used fixed-fuel summary exists")
                .local_evidence
                .provider_validation_receipt()
        })
        .collect();
    let composition_fingerprint = fingerprint_fixed_fuel_composition(schedule, &used, &by_identity);
    let composition_evidence = FixedFuelCompositionEvidence {
        summaries: used
            .iter()
            .map(|identity| {
                (
                    *identity,
                    (*by_identity
                        .get(identity)
                        .expect("used fixed-fuel summary exists"))
                    .clone(),
                )
            })
            .collect(),
    };
    Ok(ComposedFuelDemand {
        root,
        root_provider: root_summary.provider,
        schedule,
        units,
        summaries: used,
        provider_receipts,
        composition_evidence,
        composition_fingerprint,
    })
}

fn compose_fixed_fuel_summary(
    identity: ProviderFuelSummaryId,
    schedule: FuelScheduleIdentity,
    summaries: &BTreeMap<ProviderFuelSummaryId, &FixedFuelProviderSummary>,
    visiting: &mut BTreeSet<ProviderFuelSummaryId>,
    memo: &mut BTreeMap<ProviderFuelSummaryId, u64>,
    used: &mut BTreeSet<ProviderFuelSummaryId>,
) -> Result<u64, ExternalRootDiagnostic> {
    if let Some(units) = memo.get(&identity) {
        used.insert(identity);
        return Ok(*units);
    }
    if !visiting.insert(identity) {
        return Err(ExternalRootDiagnostic(format!(
            "fixed-fuel summary graph contains a cycle through 0x{:016x}",
            identity.normalized_identity()
        )));
    }
    let summary = summaries.get(&identity).ok_or_else(|| {
        ExternalRootDiagnostic(format!(
            "fixed-fuel callee summary 0x{:016x} is missing",
            identity.normalized_identity()
        ))
    })?;
    if summary.local_evidence.schedule() != schedule {
        return Err(ExternalRootDiagnostic(format!(
            "fixed-fuel summary 0x{:016x} uses schedule version {}, but the root uses version {}",
            identity.normalized_identity(),
            summary.local_evidence.schedule().marker(),
            schedule.marker()
        )));
    }
    let mut units = summary.local_evidence.units();
    for call in &summary.calls {
        if call.maximum_invocations == 0 {
            return Err(ExternalRootDiagnostic(format!(
                "fixed-fuel edge from 0x{:016x} to 0x{:016x} has zero maximum invocations",
                identity.normalized_identity(),
                call.callee.normalized_identity()
            )));
        }
        let callee_units =
            compose_fixed_fuel_summary(call.callee, schedule, summaries, visiting, memo, used)?;
        let edge_units = callee_units
            .checked_mul(call.maximum_invocations)
            .ok_or_else(|| {
                ExternalRootDiagnostic("fixed-fuel composition multiplication overflowed".into())
            })?;
        units = units.checked_add(edge_units).ok_or_else(|| {
            ExternalRootDiagnostic("fixed-fuel composition addition overflowed".into())
        })?;
    }
    visiting.remove(&identity);
    memo.insert(identity, units);
    used.insert(identity);
    Ok(units)
}

fn fingerprint_fixed_fuel_composition(
    schedule: FuelScheduleIdentity,
    used: &BTreeSet<ProviderFuelSummaryId>,
    summaries: &BTreeMap<ProviderFuelSummaryId, &FixedFuelProviderSummary>,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.u64(u64::from(schedule.marker()));
    hash.u64(used.len() as u64);
    for identity in used {
        let summary = summaries
            .get(identity)
            .expect("used fixed-fuel summary exists");
        hash.u64(summary.identity.normalized_identity());
        hash.u64(summary.provider.normalized_identity());
        fingerprint_fixed_fuel_local_evidence(&mut hash, &summary.local_evidence);
        hash.u64(summary.calls.len() as u64);
        for call in &summary.calls {
            hash.u64(call.callee.normalized_identity());
            hash.u64(call.maximum_invocations);
        }
    }
    hash.finish()
}

fn fingerprint_fixed_fuel_local_evidence(hash: &mut Fnv1a, evidence: &FixedFuelLocalEvidence) {
    match evidence {
        FixedFuelLocalEvidence::TerminalEntry(binding) => {
            hash.u64(0);
            hash.u64(binding.installed_code.normalized_identity());
            hash.u64(binding.artifact.normalized_identity());
            hash.u64(binding.entry.normalized_identity());
            let certificate = &binding.certificate;
            let terminal_psi = certificate.terminal_psi();
            hash.u64(u64::from(terminal_psi.vocabulary_marker.get()));
            hash.bytes(terminal_psi.program_fingerprint.as_bytes());
            hash.u64(u64::from(certificate.schedule().marker()));
            hash.u64(certificate.entry().get());
            hash.u64(certificate.relevant_preconditions().len() as u64);
            hash.u64(certificate.ceiling_units());
        }
        FixedFuelLocalEvidence::TerminalSegment(binding) => {
            hash.u64(1);
            hash.u64(binding.installed_code.normalized_identity());
            hash.u64(binding.artifact.normalized_identity());
            hash.u64(binding.entry.normalized_identity());
            let certificate = &binding.certificate;
            let terminal_psi = certificate.terminal_psi();
            hash.u64(u64::from(terminal_psi.vocabulary_marker.get()));
            hash.bytes(terminal_psi.program_fingerprint.as_bytes());
            hash.u64(u64::from(certificate.schedule().marker()));
            hash.u64(certificate.machine().get());
            hash.u64(certificate.start_block().get());
            hash.u64(certificate.end_edge().get());
            hash.u64(certificate.relevant_preconditions().len() as u64);
            hash.u64(certificate.ceiling_units());
        }
        FixedFuelLocalEvidence::AdmittedProvider {
            schedule,
            units,
            validation_receipt,
        } => {
            hash.u64(2);
            hash.u64(u64::from(schedule.marker()));
            hash.u64(*units);
            hash.u64(validation_receipt.normalized_identity());
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalFuelResourceColumn {
    pub schedule: FuelScheduleIdentity,
    pub provision: FuelProvisionId,
    pub ceiling_units: u64,
    pub realization: ComposedFuelDemand,
    pub validation_receipt: FuelValidationReceiptId,
}

/// The `StatePlan` itself is the public ceiling. This column retains only the
/// final transitive footprint that refined it and the public validation
/// receipt; instruction-selection/allocation derivations stay private.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineStateResourceColumn {
    pub realization: StateFootprintEvidence,
    pub validation_receipt: StateValidationReceiptId,
}

/// One source qualification accepted by the exact external-root requirement.
///
/// The compiler constructs these rows from the selected provider schema. The
/// runtime ledger retains them structurally so an invocation receipt can bind
/// a concrete parameter subject without parsing a type-display string or
/// trusting the provider to restate the admitted contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalRootEntryClaim {
    pub parameter_index: usize,
    pub domain: String,
    pub effective_carry: psi_language_semantics::CarryPolicy,
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

/// One routed result qualification supplied by an independently selected
/// boundary provider used during an external-root invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalRootResultClaim {
    pub provider_plan: ProviderPlanId,
    pub requirement_identity: String,
    pub domain: String,
    pub effective_carry: psi_language_semantics::CarryPolicy,
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

/// Provider-independent facts required for one externally invoked entry.
///
/// Effects and receipts are normalized open sets. The concrete interrupt,
/// firmware, syscall, or callback package owns their vocabulary; the ledger
/// only requires that admission bind the exact sets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalRootCandidate {
    pub identity: ExternalRootId,
    pub entry: EntryStubId,
    pub provider: RootProviderId,
    /// Exact normalized compiler-selected provider plan that supplies this
    /// root. Validation binds it into the root identity before execution or
    /// slot admission can be constructed.
    pub provider_plan: ProviderPlanId,
    /// Stable identity of the exact boundary requirement implemented by this
    /// entry stub, not merely the containing provider schema.
    pub requirement_identity: String,
    /// Compiler-owned accepted qualification rows for that one requirement.
    /// Validation requires canonical ordering and rejects duplicate claims.
    pub entry_claims: Vec<ExternalRootEntryClaim>,
    /// Parameter whose concrete subject is the provider-minted interrupt
    /// acknowledgement. `None` is valid for roots without that obligation.
    pub acknowledgement_parameter_index: Option<usize>,
    /// Exact routed result contract used when this root's mask control saves
    /// and masks the current invocation. It belongs to the independently
    /// selected mask-control provider, not implicitly to the root provider.
    pub interrupt_mask_guard_claim: Option<ExternalRootResultClaim>,
    pub effects: BTreeSet<RootEffectId>,
    pub trust_receipts: BTreeSet<TrustReceiptId>,
    /// Identity of the artifact-wide relation that names which other roots may
    /// preempt this one. Stack class and maximum depth remain the one copy in
    /// `BoundaryEntryPlan::state`; they are not re-authored here.
    pub nesting_relation: NestingRelationId,
    pub acknowledgement_policy: Option<AcknowledgementPolicyId>,
    pub stack: StackResourceColumn,
    pub logical_fuel: LogicalFuelResourceColumn,
    pub machine_state: MachineStateResourceColumn,
    pub component_pins: BTreeSet<ComponentVersionPin>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedExternalRoot {
    candidate: ExternalRootCandidate,
    boundary: ValidatedBoundaryEntryPlan,
    boundary_contract_fingerprint: u64,
    normalized_identity: u64,
}

/// Evidence that an opaque provider cannot escape the boundary's admitted
/// exit contract. An accepted claim is checked against the exact normalized
/// `CallPlan + StatePlan`; adequate hardware isolation is the explicit
/// alternative when the provider's exit is not inspectable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpaqueProviderExitAssurance {
    AcceptedClaim {
        realization: ProviderExitRealization,
        validation_receipt: TrustReceiptId,
    },
    HardwareIsolation {
        validation_receipt: TrustReceiptId,
    },
}

impl OpaqueProviderExitAssurance {
    fn validate(self, root: &ValidatedExternalRoot) -> Result<Self, ExternalRootDiagnostic> {
        let validation_receipt = match self {
            Self::AcceptedClaim {
                validation_receipt, ..
            }
            | Self::HardwareIsolation { validation_receipt } => validation_receipt,
        };
        if !root.candidate.trust_receipts.contains(&validation_receipt) {
            return Err(ExternalRootDiagnostic(
                "opaque provider exit assurance is absent from the root's admitted trust receipts"
                    .into(),
            ));
        }
        if let Self::AcceptedClaim { realization, .. } = self {
            validate_provider_exit_realization(root.boundary.plan(), &realization).map_err(
                |error| {
                    ExternalRootDiagnostic(format!(
                        "opaque provider exit claim violates the admitted boundary: {error}"
                    ))
                },
            )?;
        }
        Ok(self)
    }

    fn fingerprint(self) -> u64 {
        let mut hash = Fnv1a::new();
        match self {
            Self::AcceptedClaim {
                validation_receipt, ..
            } => {
                hash.u64(0);
                hash.u64(validation_receipt.normalized_identity());
            }
            Self::HardwareIsolation { validation_receipt } => {
                hash.u64(1);
                hash.u64(validation_receipt.normalized_identity());
            }
        }
        hash.finish()
    }
}

impl ValidatedExternalRoot {
    pub const fn candidate(&self) -> &ExternalRootCandidate {
        &self.candidate
    }

    pub const fn boundary(&self) -> &BoundaryEntryPlan {
        self.boundary.plan()
    }

    pub const fn boundary_contract_fingerprint(&self) -> u64 {
        self.boundary_contract_fingerprint
    }

    pub const fn normalized_identity(&self) -> u64 {
        self.normalized_identity
    }
}

/// Admitted execution binding for one exact external-root realization.
///
/// This does not fuse the stack, logical-fuel, and machine-state algebras.
/// It binds their independently validated results, the selected normalized
/// provider plan, and the executable entry into one provider execution that a
/// root admission may publish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderExecution {
    identity: ProviderExecutionId,
    root_evidence: ValidatedExternalRoot,
    provider_plan: ProviderPlanId,
    root: ExternalRootId,
    normalized_root_identity: u64,
    provider: RootProviderId,
    entry: EntryStubId,
    boundary_contract_fingerprint: u64,
    stack_artifact_composition_fingerprint: u64,
    stack_demand_fingerprint: u64,
    logical_fuel_fingerprint: u64,
    machine_state_validation_receipt: StateValidationReceiptId,
    exit_assurance: OpaqueProviderExitAssurance,
    exit_assurance_fingerprint: u64,
    effects: BTreeSet<RootEffectId>,
    normalized_identity: u64,
}

/// Non-constructible evidence that the external-root ledger admitted one exact
/// provider execution. Terminal lowering may borrow or retain this value; wire
/// formats record its fields but cannot recreate executable authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AdmittedTerminalProviderExecution {
    provider_plan: u64,
    provider_execution_identity: u64,
    provider_execution_fingerprint: u64,
    normalized_root_identity: u64,
    boundary_contract_fingerprint: u64,
}

/// One exact provider execution joined to the installed artifact resolver and
/// the provider-private input for a post-handoff entry writer.
///
/// Construction matches the selected provider-plan identity, retains the
/// execution that already sealed stack/fuel/state root evidence, and rechecks
/// the installed entry, writer's exact symbolic target set, and concrete
/// destination placement. The packed numeric context remains opaque and
/// non-clonable.
#[derive(Debug, PartialEq, Eq)]
pub struct PreparedExternalRootPostHandoffWriterInvocation {
    provider_execution: AdmittedTerminalProviderExecution,
    provider_execution_evidence: ProviderExecution,
    root_evidence: ValidatedExternalRoot,
    selected_entry: EntryStubId,
    selected_entry_source_slot: usize,
    architecture: omega_target::Architecture,
    invocation: PostHandoffWriterInvocationPlan,
    writer: PostHandoffWriterPlan,
    context: ResolvedPostHandoffEntryWriterContext,
}

/// Still-unpublished destination retaining the exact selected external-root
/// execution and writer preparation that produced its bytes. The provider
/// evidence is not reduced to copied report identities, and the installation-
/// owned destination remains in its consuming validated typestate rather than
/// being downgraded after replay. This outer carrier still exposes no bytes and
/// does not establish consumer semantics or publication authority.
#[derive(Debug)]
#[must_use = "written external-root destination retains provider and mapping custody"]
pub struct WrittenExternalRootPostHandoffWriterDestination<'mapping, 'bytes> {
    provider_execution: AdmittedTerminalProviderExecution,
    provider_execution_evidence: ProviderExecution,
    root_evidence: ValidatedExternalRoot,
    selected_entry: EntryStubId,
    selected_entry_source_slot: usize,
    architecture: omega_target::Architecture,
    invocation: PostHandoffWriterInvocationPlan,
    writer: PostHandoffWriterPlan,
    written: omega_executable_installation::ValidatedWrittenPostHandoffWriterDestination<
        'mapping,
        'bytes,
    >,
}

/// A written external-root destination whose provider, root, invocation,
/// installation, mapping, and destination evidence has been replayed before
/// its still-unpublished bytes become observable.
#[derive(Debug)]
#[must_use = "validated written external-root destination retains provider and mapping custody"]
pub struct ValidatedWrittenExternalRootPostHandoffWriterDestination<'mapping, 'bytes> {
    provider_execution: AdmittedTerminalProviderExecution,
    provider_execution_evidence: ProviderExecution,
    root_evidence: ValidatedExternalRoot,
    selected_entry: EntryStubId,
    selected_entry_source_slot: usize,
    architecture: omega_target::Architecture,
    invocation: PostHandoffWriterInvocationPlan,
    writer: PostHandoffWriterPlan,
    written: omega_executable_installation::ValidatedWrittenPostHandoffWriterDestination<
        'mapping,
        'bytes,
    >,
}

/// Consumer-validation rejection returns the complete written carrier for a
/// corrected installed-realization retry without exposing destination bytes.
#[derive(Debug)]
pub struct WrittenExternalRootConsumerValidationError<'mapping, 'bytes> {
    written: WrittenExternalRootPostHandoffWriterDestination<'mapping, 'bytes>,
    diagnostic: psi_layout_plans::MaterializationDiagnostic,
}

impl<'mapping, 'bytes> WrittenExternalRootConsumerValidationError<'mapping, 'bytes> {
    pub const fn diagnostic(&self) -> &psi_layout_plans::MaterializationDiagnostic {
        &self.diagnostic
    }

    pub fn into_written(self) -> WrittenExternalRootPostHandoffWriterDestination<'mapping, 'bytes> {
        self.written
    }
}

/// Failed recovery of a still-unpublished external-root writer destination.
/// The complete written carrier is returned unchanged so the owning consumer
/// can correct the installed-code input or choose another recovery path.
#[derive(Debug)]
pub struct WrittenExternalRootWriterRecoveryError<'mapping, 'bytes> {
    written: WrittenExternalRootPostHandoffWriterDestination<'mapping, 'bytes>,
    diagnostic: psi_layout_plans::MaterializationDiagnostic,
}

impl<'mapping, 'bytes> WrittenExternalRootWriterRecoveryError<'mapping, 'bytes> {
    pub const fn diagnostic(&self) -> &psi_layout_plans::MaterializationDiagnostic {
        &self.diagnostic
    }

    pub fn into_written(self) -> WrittenExternalRootPostHandoffWriterDestination<'mapping, 'bytes> {
        self.written
    }
}

#[derive(Debug)]
pub struct PreparedExternalRootWriterExecutionError<'mapping, 'bytes> {
    prepared: PreparedExternalRootPostHandoffWriterInvocation,
    destination: omega_executable_installation::ValidatedPreparedPostHandoffWriterDestination<
        'mapping,
        'bytes,
    >,
    diagnostic: psi_layout_plans::MaterializationDiagnostic,
}

impl<'mapping, 'bytes> PreparedExternalRootWriterExecutionError<'mapping, 'bytes> {
    pub const fn diagnostic(&self) -> &psi_layout_plans::MaterializationDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        PreparedExternalRootPostHandoffWriterInvocation,
        omega_executable_installation::ValidatedPreparedPostHandoffWriterDestination<
            'mapping,
            'bytes,
        >,
    ) {
        (self.prepared, self.destination)
    }
}

impl PreparedExternalRootPostHandoffWriterInvocation {
    fn validate_execution(
        &self,
        installed_code: &InstalledCode,
    ) -> Result<(), psi_layout_plans::MaterializationDiagnostic> {
        self.invocation.validate_structure()?;
        let replayed_invocation = self.writer.lower_reusable_fragment()?;
        if replayed_invocation != self.invocation {
            return Err(psi_layout_plans::MaterializationDiagnostic(
                "prepared external-root writer no longer matches its retained invocation".into(),
            ));
        }
        validate_external_root_writer_source(
            &self.provider_execution_evidence,
            &self.root_evidence,
            self.provider_execution,
            &self.invocation,
            self.selected_entry,
            self.selected_entry_source_slot,
        )?;
        if !self.context.binds_invocation(&self.invocation) {
            return Err(psi_layout_plans::MaterializationDiagnostic(
                "prepared external-root writer context no longer binds its retained invocation"
                    .into(),
            ));
        }
        if installed_code.identity() != self.context.installed_code()
            || installed_code.artifact() != self.context.artifact()
        {
            return Err(psi_layout_plans::MaterializationDiagnostic(
                "prepared external-root writer does not bind the exact installed artifact".into(),
            ));
        }
        if installed_code.architecture() != self.architecture {
            return Err(psi_layout_plans::MaterializationDiagnostic(
                "prepared external-root writer architecture does not match the exact installed artifact"
                    .into(),
            ));
        }
        Ok(())
    }

    pub const fn provider_execution(&self) -> AdmittedTerminalProviderExecution {
        self.provider_execution
    }

    pub const fn selected_entry(&self) -> EntryStubId {
        self.selected_entry
    }

    pub const fn selected_entry_source_slot(&self) -> usize {
        self.selected_entry_source_slot
    }

    pub fn selected_requirement_identity(&self) -> &str {
        &self.root_evidence.candidate.requirement_identity
    }

    pub fn selected_boundary_parameter_count(&self) -> usize {
        self.root_evidence.boundary.plan().call.parameters.len()
    }

    pub const fn selected_boundary_contract_fingerprint(&self) -> u64 {
        self.root_evidence.boundary_contract_fingerprint
    }

    pub fn selected_entry_claims(&self) -> &[ExternalRootEntryClaim] {
        &self.root_evidence.candidate.entry_claims
    }

    pub const fn architecture(&self) -> omega_target::Architecture {
        self.architecture
    }

    pub const fn invocation(&self) -> &PostHandoffWriterInvocationPlan {
        &self.invocation
    }

    pub const fn context(&self) -> &ResolvedPostHandoffEntryWriterContext {
        &self.context
    }

    /// Consume one exact provider-prepared destination through the installed
    /// artifact resolver used during preparation. The successful result is
    /// still unpublished; consumer-specific validation and publication remain
    /// separate transitions.
    pub fn execute<'mapping, 'bytes>(
        self,
        installed_code: &InstalledCode,
        destination: omega_executable_installation::ValidatedPreparedPostHandoffWriterDestination<
            'mapping,
            'bytes,
        >,
    ) -> Result<
        WrittenExternalRootPostHandoffWriterDestination<'mapping, 'bytes>,
        Box<PreparedExternalRootWriterExecutionError<'mapping, 'bytes>>,
    > {
        if let Err(diagnostic) = self.validate_execution(installed_code) {
            return Err(Box::new(PreparedExternalRootWriterExecutionError {
                prepared: self,
                destination,
                diagnostic,
            }));
        }
        if let Err(diagnostic) = self.context.validate_for_destination(
            installed_code,
            destination.site(),
            destination.len(),
        ) {
            return Err(Box::new(PreparedExternalRootWriterExecutionError {
                prepared: self,
                destination,
                diagnostic: psi_layout_plans::MaterializationDiagnostic(diagnostic.0),
            }));
        }
        let Self {
            provider_execution,
            provider_execution_evidence,
            root_evidence,
            selected_entry,
            selected_entry_source_slot,
            architecture,
            invocation,
            writer,
            context,
        } = self;
        match installed_code.write_prepared_post_handoff_destination(context, &writer, destination)
        {
            Ok(written) => {
                let written = match written.into_validated_for_consumer(installed_code) {
                    Ok(written) => written,
                    Err(error) => {
                        let diagnostic = psi_layout_plans::MaterializationDiagnostic(
                            error.diagnostic().0.clone(),
                        );
                        let (context, destination) = (*error).into_prepared_parts();
                        return Err(Box::new(PreparedExternalRootWriterExecutionError {
                            prepared: Self {
                                provider_execution,
                                provider_execution_evidence,
                                root_evidence,
                                selected_entry,
                                selected_entry_source_slot,
                                architecture,
                                invocation,
                                writer,
                                context,
                            },
                            destination,
                            diagnostic,
                        }));
                    }
                };
                Ok(WrittenExternalRootPostHandoffWriterDestination {
                    provider_execution,
                    provider_execution_evidence,
                    root_evidence,
                    selected_entry,
                    selected_entry_source_slot,
                    architecture,
                    invocation,
                    writer,
                    written,
                })
            }
            Err(destination_error) => {
                let diagnostic = destination_error.diagnostic().clone();
                let (context, destination) = (*destination_error).into_parts();
                Err(Box::new(PreparedExternalRootWriterExecutionError {
                    prepared: Self {
                        provider_execution,
                        provider_execution_evidence,
                        root_evidence,
                        selected_entry,
                        selected_entry_source_slot,
                        architecture,
                        invocation,
                        writer,
                        context,
                    },
                    destination,
                    diagnostic,
                }))
            }
        }
    }
}

impl<'mapping, 'bytes> WrittenExternalRootPostHandoffWriterDestination<'mapping, 'bytes> {
    pub const fn provider_execution(&self) -> AdmittedTerminalProviderExecution {
        self.provider_execution
    }

    pub const fn selected_entry(&self) -> EntryStubId {
        self.selected_entry
    }

    pub const fn selected_entry_source_slot(&self) -> usize {
        self.selected_entry_source_slot
    }

    pub fn selected_requirement_identity(&self) -> &str {
        &self.root_evidence.candidate.requirement_identity
    }

    pub fn selected_boundary_parameter_count(&self) -> usize {
        self.root_evidence.boundary.plan().call.parameters.len()
    }

    pub const fn selected_boundary_contract_fingerprint(&self) -> u64 {
        self.root_evidence.boundary_contract_fingerprint
    }

    pub fn selected_entry_claims(&self) -> &[ExternalRootEntryClaim] {
        &self.root_evidence.candidate.entry_claims
    }

    pub const fn architecture(&self) -> omega_target::Architecture {
        self.architecture
    }

    pub const fn invocation(&self) -> &PostHandoffWriterInvocationPlan {
        &self.invocation
    }

    /// Independently replay provider preparation, invocation structure, and
    /// the installation-owned context. Rejection only borrows this carrier so
    /// the exact provider and destination inputs remain available for retry.
    pub fn validate_for_consumer(
        &self,
        installed_code: &InstalledCode,
    ) -> Result<(), psi_layout_plans::MaterializationDiagnostic> {
        self.invocation.validate_structure()?;
        let replayed_invocation = self.writer.lower_reusable_fragment()?;
        if replayed_invocation != self.invocation {
            return Err(psi_layout_plans::MaterializationDiagnostic(
                "written external-root destination does not retain its exact provider preparation and invocation"
                    .into(),
            ));
        }
        validate_external_root_writer_source(
            &self.provider_execution_evidence,
            &self.root_evidence,
            self.provider_execution,
            &self.invocation,
            self.selected_entry,
            self.selected_entry_source_slot,
        )?;
        if self.architecture != installed_code.architecture()
            || !self.written.binds_invocation(&self.invocation)
            || self.written.normalized_fragment_fingerprint()
                != self.invocation.fragment().fingerprint()
        {
            return Err(psi_layout_plans::MaterializationDiagnostic(
                "written external-root destination does not retain its exact provider preparation and invocation"
                    .into(),
            ));
        }
        self.written
            .validate_for_consumer(installed_code)
            .map_err(|diagnostic| psi_layout_plans::MaterializationDiagnostic(diagnostic.0))
    }

    /// Consume this carrier only after replaying its complete retained context
    /// against the consumer's installed realization. Rejection returns the
    /// original carrier and exposes no destination bytes.
    pub fn into_validated_for_consumer(
        self,
        installed_code: &InstalledCode,
    ) -> Result<
        ValidatedWrittenExternalRootPostHandoffWriterDestination<'mapping, 'bytes>,
        Box<WrittenExternalRootConsumerValidationError<'mapping, 'bytes>>,
    > {
        if let Err(diagnostic) = self.validate_for_consumer(installed_code) {
            return Err(Box::new(WrittenExternalRootConsumerValidationError {
                written: self,
                diagnostic,
            }));
        }
        let Self {
            provider_execution,
            provider_execution_evidence,
            root_evidence,
            selected_entry,
            selected_entry_source_slot,
            architecture,
            invocation,
            writer,
            written,
        } = self;
        Ok(ValidatedWrittenExternalRootPostHandoffWriterDestination {
            provider_execution,
            provider_execution_evidence,
            root_evidence,
            selected_entry,
            selected_entry_source_slot,
            architecture,
            invocation,
            writer,
            written,
        })
    }

    /// Return this still-unpublished destination to the exact provider-writer
    /// preparation state from which it can be executed again. Recovery first
    /// replays the complete provider, invocation, installation, mapping, and
    /// destination binding; rejection preserves this written carrier intact.
    /// Success does not restore old bytes, validate consumer semantics, or
    /// publish the destination.
    pub fn recover_for_retry(
        self,
        installed_code: &InstalledCode,
    ) -> Result<
        (
            PreparedExternalRootPostHandoffWriterInvocation,
            omega_executable_installation::ValidatedPreparedPostHandoffWriterDestination<
                'mapping,
                'bytes,
            >,
        ),
        Box<WrittenExternalRootWriterRecoveryError<'mapping, 'bytes>>,
    > {
        match self.into_validated_for_consumer(installed_code) {
            Ok(written) => written.recover_for_retry(),
            Err(error) => {
                let diagnostic = error.diagnostic().clone();
                Err(Box::new(WrittenExternalRootWriterRecoveryError {
                    written: (*error).into_written(),
                    diagnostic,
                }))
            }
        }
    }
}

impl<'mapping, 'bytes> ValidatedWrittenExternalRootPostHandoffWriterDestination<'mapping, 'bytes> {
    /// Bytes remain unpublished; this is observation after exact replay, not
    /// consumer semantic validation or publication.
    pub fn bytes(&self) -> &[u8] {
        self.written.bytes()
    }

    pub const fn provider_execution(&self) -> AdmittedTerminalProviderExecution {
        self.provider_execution
    }

    pub const fn selected_entry(&self) -> EntryStubId {
        self.selected_entry
    }

    pub const fn selected_entry_source_slot(&self) -> usize {
        self.selected_entry_source_slot
    }

    pub fn selected_requirement_identity(&self) -> &str {
        &self.root_evidence.candidate.requirement_identity
    }

    pub fn recover_for_retry(
        self,
    ) -> Result<
        (
            PreparedExternalRootPostHandoffWriterInvocation,
            omega_executable_installation::ValidatedPreparedPostHandoffWriterDestination<
                'mapping,
                'bytes,
            >,
        ),
        Box<WrittenExternalRootWriterRecoveryError<'mapping, 'bytes>>,
    > {
        let Self {
            provider_execution,
            provider_execution_evidence,
            root_evidence,
            selected_entry,
            selected_entry_source_slot,
            architecture,
            invocation,
            writer,
            written,
        } = self;
        let (context, destination) = written.into_prepared_parts();
        Ok((
            PreparedExternalRootPostHandoffWriterInvocation {
                provider_execution,
                provider_execution_evidence,
                root_evidence,
                selected_entry,
                selected_entry_source_slot,
                architecture,
                invocation,
                writer,
                context,
            },
            destination,
        ))
    }

    pub fn into_parts(
        self,
    ) -> (
        AdmittedTerminalProviderExecution,
        ProviderExecution,
        ValidatedExternalRoot,
        EntryStubId,
        usize,
        omega_target::Architecture,
        PostHandoffWriterInvocationPlan,
        PostHandoffWriterPlan,
        omega_executable_installation::ValidatedWrittenPostHandoffWriterDestination<
            'mapping,
            'bytes,
        >,
    ) {
        (
            self.provider_execution,
            self.provider_execution_evidence,
            self.root_evidence,
            self.selected_entry,
            self.selected_entry_source_slot,
            self.architecture,
            self.invocation,
            self.writer,
            self.written,
        )
    }
}

fn selected_entry_source_slot(
    invocation: &PostHandoffWriterInvocationPlan,
    selected_entry: EntryStubId,
) -> Result<usize, psi_layout_plans::MaterializationDiagnostic> {
    invocation.validate_structure()?;
    let target = RelocationTarget::Entry(selected_entry);
    let mut matches = invocation
        .sources()
        .iter()
        .enumerate()
        .filter(|(_, source)| source.target == target);
    let Some((source_slot, source)) = matches.next() else {
        return Err(psi_layout_plans::MaterializationDiagnostic(
            "post-handoff writer does not contain the admitted external-root entry".into(),
        ));
    };
    if matches.next().is_some() {
        return Err(psi_layout_plans::MaterializationDiagnostic(
            "post-handoff writer repeats the admitted external-root entry source".into(),
        ));
    }
    if source.source != psi_layout_plans::PostHandoffWriterSource::Resolve(target) {
        return Err(psi_layout_plans::MaterializationDiagnostic(
            "post-handoff writer must resolve the admitted external-root entry through its sealed provider context"
                .into(),
        ));
    }
    Ok(source_slot)
}

fn validate_selected_entry_source(
    invocation: &PostHandoffWriterInvocationPlan,
    selected_entry: EntryStubId,
    retained_source_slot: usize,
) -> Result<(), psi_layout_plans::MaterializationDiagnostic> {
    let replayed_source_slot = selected_entry_source_slot(invocation, selected_entry)?;
    if replayed_source_slot != retained_source_slot {
        return Err(psi_layout_plans::MaterializationDiagnostic(
            "post-handoff writer selected-entry source-slot correspondence does not match its retained preparation"
                .into(),
        ));
    }
    Ok(())
}

fn validate_external_root_writer_source(
    provider_execution_evidence: &ProviderExecution,
    root_evidence: &ValidatedExternalRoot,
    provider_execution: AdmittedTerminalProviderExecution,
    invocation: &PostHandoffWriterInvocationPlan,
    selected_entry: EntryStubId,
    retained_source_slot: usize,
) -> Result<(), psi_layout_plans::MaterializationDiagnostic> {
    if !provider_execution_evidence.matches_root(root_evidence)
        || provider_execution_evidence.terminal_binding() != provider_execution
        || root_evidence.candidate.entry != selected_entry
        || root_evidence.candidate.provider_plan.normalized_identity()
            != provider_execution.provider_plan
        || root_evidence.normalized_identity != provider_execution.normalized_root_identity
        || root_evidence.boundary_contract_fingerprint
            != provider_execution.boundary_contract_fingerprint
    {
        return Err(psi_layout_plans::MaterializationDiagnostic(
            "post-handoff writer source does not retain its exact validated external-root requirement and provider execution"
                .into(),
        ));
    }
    validate_selected_entry_source(invocation, selected_entry, retained_source_slot)
}

impl AdmittedTerminalProviderExecution {
    pub const fn provider_plan(&self) -> u64 {
        self.provider_plan
    }

    pub const fn provider_execution_identity(&self) -> u64 {
        self.provider_execution_identity
    }

    pub const fn provider_execution_fingerprint(&self) -> u64 {
        self.provider_execution_fingerprint
    }

    pub const fn normalized_root_identity(&self) -> u64 {
        self.normalized_root_identity
    }

    pub const fn boundary_contract_fingerprint(&self) -> u64 {
        self.boundary_contract_fingerprint
    }
}

impl omega_terminal_installation_evidence::TerminalProviderExecutionEvidence for ProviderExecution {
    fn provider_plan(&self) -> u64 {
        self.provider_plan.normalized_identity()
    }

    fn provider_execution_identity(&self) -> u64 {
        self.identity.normalized_identity()
    }

    fn provider_execution_fingerprint(&self) -> u64 {
        self.normalized_identity
    }

    fn normalized_root_identity(&self) -> u64 {
        self.normalized_root_identity
    }

    fn boundary_contract_fingerprint(&self) -> u64 {
        self.boundary_contract_fingerprint
    }
}

fn fingerprint_provider_execution(
    identity: ProviderExecutionId,
    root: &ValidatedExternalRoot,
    exit_assurance_fingerprint: u64,
) -> u64 {
    let candidate = root.candidate();
    let mut hash = Fnv1a::new();
    hash.u64(identity.normalized_identity());
    hash.u64(candidate.provider_plan.normalized_identity());
    hash.u64(candidate.identity.normalized_identity());
    hash.u64(root.normalized_identity());
    hash.u64(candidate.provider.normalized_identity());
    hash.u64(candidate.entry.normalized_identity());
    hash.u64(root.boundary_contract_fingerprint());
    hash.u64(
        candidate
            .stack
            .realization
            .artifact_composition_fingerprint(),
    );
    hash.u64(candidate.stack.realization.composition_fingerprint());
    hash.u64(candidate.logical_fuel.realization.composition_fingerprint());
    hash.u64(
        candidate
            .machine_state
            .validation_receipt
            .normalized_identity(),
    );
    hash.u64(exit_assurance_fingerprint);
    for effect in &candidate.effects {
        hash.u64(effect.normalized_identity());
    }
    hash.finish()
}

impl ProviderExecution {
    /// Provider/trust admission creates this binding only after selecting the
    /// exact provider plan that will execute the validated root.
    pub fn from_admitted_provider(
        identity: ProviderExecutionId,
        root: &ValidatedExternalRoot,
        exit_assurance: Option<OpaqueProviderExitAssurance>,
    ) -> Result<Self, ExternalRootDiagnostic> {
        let exit_assurance = exit_assurance
            .ok_or_else(|| {
                ExternalRootDiagnostic(
                    "opaque provider requires an accepted exit claim or adequate hardware isolation"
                        .into(),
                )
            })?
            .validate(root)?;
        let exit_assurance_fingerprint = exit_assurance.fingerprint();
        let candidate = root.candidate();
        let normalized_identity =
            fingerprint_provider_execution(identity, root, exit_assurance_fingerprint);
        Ok(Self {
            identity,
            root_evidence: root.clone(),
            provider_plan: candidate.provider_plan,
            root: candidate.identity,
            normalized_root_identity: root.normalized_identity(),
            provider: candidate.provider,
            entry: candidate.entry,
            boundary_contract_fingerprint: root.boundary_contract_fingerprint(),
            stack_artifact_composition_fingerprint: candidate
                .stack
                .realization
                .artifact_composition_fingerprint(),
            stack_demand_fingerprint: candidate.stack.realization.composition_fingerprint(),
            logical_fuel_fingerprint: candidate.logical_fuel.realization.composition_fingerprint(),
            machine_state_validation_receipt: candidate.machine_state.validation_receipt,
            exit_assurance,
            exit_assurance_fingerprint,
            effects: candidate.effects.clone(),
            normalized_identity,
        })
    }

    pub const fn identity(&self) -> ProviderExecutionId {
        self.identity
    }

    pub const fn provider_plan(&self) -> ProviderPlanId {
        self.provider_plan
    }

    pub const fn normalized_identity(&self) -> u64 {
        self.normalized_identity
    }

    pub fn selected_requirement_identity(&self) -> &str {
        &self.root_evidence.candidate.requirement_identity
    }

    pub fn selected_boundary_parameter_count(&self) -> usize {
        self.root_evidence.boundary.plan().call.parameters.len()
    }

    pub const fn selected_boundary_contract_fingerprint(&self) -> u64 {
        self.root_evidence.boundary_contract_fingerprint
    }

    pub fn selected_entry_claims(&self) -> &[ExternalRootEntryClaim] {
        &self.root_evidence.candidate.entry_claims
    }

    /// Export the exact admitted execution evidence consumed by the clean
    /// terminal-Psi native lane. Lowering does not accept a second provider
    /// plan choice: this binding inherits the plan selected by root admission.
    pub const fn terminal_binding(&self) -> AdmittedTerminalProviderExecution {
        AdmittedTerminalProviderExecution {
            provider_plan: self.provider_plan.normalized_identity(),
            provider_execution_identity: self.identity.normalized_identity(),
            provider_execution_fingerprint: self.normalized_identity,
            normalized_root_identity: self.normalized_root_identity,
            boundary_contract_fingerprint: self.boundary_contract_fingerprint,
        }
    }

    /// Independently replay the complete admitted root, execution-to-root
    /// binding, exit assurance, and normalized execution identity before an
    /// installed resolver observes symbolic writer sources.
    pub fn validate_for_writer_preparation(&self) -> Result<(), ExternalRootDiagnostic> {
        let replayed_root = validate_external_root(
            self.root_evidence.candidate.clone(),
            &self.root_evidence.boundary,
        )?;
        if replayed_root != self.root_evidence || !self.matches_root(&replayed_root) {
            return Err(ExternalRootDiagnostic(
                "post-handoff writer provider execution does not retain its exact validated root evidence"
                    .into(),
            ));
        }
        self.exit_assurance.validate(&replayed_root)?;
        let exit_assurance_fingerprint = self.exit_assurance.fingerprint();
        if exit_assurance_fingerprint != self.exit_assurance_fingerprint
            || fingerprint_provider_execution(
                self.identity,
                &replayed_root,
                exit_assurance_fingerprint,
            ) != self.normalized_identity
        {
            return Err(ExternalRootDiagnostic(
                "post-handoff writer provider execution identity fails exact structural replay"
                    .into(),
            ));
        }
        Ok(())
    }

    /// Prepare one post-handoff writer invocation only when it contains this
    /// execution's exact selected entry and is resolved by the same installed
    /// artifact used by the root's terminal fixed-fuel evidence.
    ///
    /// The plan ID is an identity comparison, not authority. The compiler
    /// orchestration wrapper supplies its retained selection so a later stage
    /// cannot accidentally substitute a different closure after root
    /// admission.
    pub fn prepare_post_handoff_entry_writer(
        &self,
        selected_provider_plan: ProviderPlanId,
        installed_code: &InstalledCode,
        writer: &PostHandoffWriterPlan,
        destination_len: usize,
        destination_site: PlacementSite,
    ) -> Result<PreparedExternalRootPostHandoffWriterInvocation, ExternalRootDiagnostic> {
        if selected_provider_plan != self.provider_plan {
            return Err(ExternalRootDiagnostic(
                "post-handoff writer selected provider plan does not match the admitted provider execution"
                    .into(),
            ));
        }
        self.validate_for_writer_preparation()?;
        self.validate_installed_entry_binding(installed_code)?;

        let invocation = writer
            .lower_reusable_fragment()
            .map_err(|error| ExternalRootDiagnostic(error.0))?;
        let selected_entry_source_slot = selected_entry_source_slot(&invocation, self.entry)
            .map_err(|diagnostic| ExternalRootDiagnostic(diagnostic.0))?;

        let context = installed_code
            .populate_post_handoff_entry_writer_context(writer, destination_len, destination_site)
            .map_err(|error| ExternalRootDiagnostic(error.0))?;
        if !context.binds_invocation(&invocation) {
            return Err(ExternalRootDiagnostic(
                "installed artifact resolver context does not bind the exact post-handoff writer invocation"
                    .into(),
            ));
        }
        Ok(PreparedExternalRootPostHandoffWriterInvocation {
            provider_execution: self.terminal_binding(),
            provider_execution_evidence: self.clone(),
            root_evidence: self.root_evidence.clone(),
            selected_entry: self.entry,
            selected_entry_source_slot,
            architecture: installed_code.architecture(),
            invocation,
            writer: writer.clone(),
            context,
        })
    }

    pub const fn exit_assurance(&self) -> OpaqueProviderExitAssurance {
        self.exit_assurance
    }

    pub const fn exit_assurance_fingerprint(&self) -> u64 {
        self.exit_assurance_fingerprint
    }

    fn matches_root(&self, root: &ValidatedExternalRoot) -> bool {
        let candidate = root.candidate();
        self.root_evidence == *root
            && self.root == candidate.identity
            && self.normalized_root_identity == root.normalized_identity()
            && self.provider_plan == candidate.provider_plan
            && self.provider == candidate.provider
            && self.entry == candidate.entry
            && self.boundary_contract_fingerprint == root.boundary_contract_fingerprint()
            && self.stack_artifact_composition_fingerprint
                == candidate
                    .stack
                    .realization
                    .artifact_composition_fingerprint()
            && self.stack_demand_fingerprint
                == candidate.stack.realization.composition_fingerprint()
            && self.logical_fuel_fingerprint
                == candidate.logical_fuel.realization.composition_fingerprint()
            && self.machine_state_validation_receipt == candidate.machine_state.validation_receipt
            && self.effects == candidate.effects
    }

    fn validate_installed_entry_binding(
        &self,
        installed_code: &InstalledCode,
    ) -> Result<(), ExternalRootDiagnostic> {
        if installed_code.selected_entry_target(self.entry).is_err() {
            return Err(ExternalRootDiagnostic(
                "external-root entry is not in the admitted installed artifact".into(),
            ));
        }
        let root_stack_summary = self
            .root_evidence
            .candidate
            .stack
            .realization
            .composition_evidence
            .summaries
            .get(&self.root_evidence.candidate.stack.realization.root())
            .expect("stack composition retains its root summary");
        if let StackLocalEvidence::TerminalEntry(binding) = &root_stack_summary.local_evidence {
            validate_installed_terminal_entry_stack(binding, installed_code, self.entry).map_err(
                |_| {
                    ExternalRootDiagnostic(
                        "terminal stack root evidence is not bound to the exact installed code and selected entry"
                            .into(),
                    )
                },
            )?;
        }
        let root_fuel_summary = self
            .root_evidence
            .candidate
            .logical_fuel
            .realization
            .composition_evidence
            .summaries
            .get(&self.root_evidence.candidate.logical_fuel.realization.root)
            .expect("fixed-fuel composition retains its root summary");
        let fuel_binding_matches = match &root_fuel_summary.local_evidence {
            FixedFuelLocalEvidence::TerminalEntry(binding) => {
                validate_installed_terminal_entry_fuel(binding, installed_code, self.entry).is_ok()
            }
            FixedFuelLocalEvidence::TerminalSegment(_) => false,
            FixedFuelLocalEvidence::AdmittedProvider { .. } => true,
        };
        if !fuel_binding_matches {
            return Err(ExternalRootDiagnostic(
                "terminal fixed-fuel root evidence is not a whole-entry certificate bound to the exact installed code and selected entry"
                    .into(),
            ));
        }
        Ok(())
    }
}

pub fn validate_external_root(
    candidate: ExternalRootCandidate,
    boundary: &ValidatedBoundaryEntryPlan,
) -> Result<ValidatedExternalRoot, ExternalRootDiagnostic> {
    if candidate.requirement_identity.is_empty() {
        return Err(ExternalRootDiagnostic(
            "external-root requirement identity cannot be empty".into(),
        ));
    }
    let mut prior_claim: Option<(usize, &str)> = None;
    for claim in &candidate.entry_claims {
        if claim.domain.is_empty() {
            return Err(ExternalRootDiagnostic(
                "external-root entry claim domain identity cannot be empty".into(),
            ));
        }
        let key = (claim.parameter_index, claim.domain.as_str());
        if prior_claim.is_some_and(|prior| prior >= key) {
            return Err(ExternalRootDiagnostic(
                "external-root entry claims must be uniquely sorted by parameter and domain".into(),
            ));
        }
        if boundary
            .plan()
            .call
            .parameters
            .get(claim.parameter_index)
            .is_none()
        {
            return Err(ExternalRootDiagnostic(format!(
                "external-root entry claim parameter {} has no exact ABI placement in the validated boundary plan",
                claim.parameter_index
            )));
        }
        prior_claim = Some(key);
    }
    if let Some(parameter_index) = candidate.acknowledgement_parameter_index
        && !candidate
            .entry_claims
            .iter()
            .any(|claim| claim.parameter_index == parameter_index)
    {
        return Err(ExternalRootDiagnostic(
            "external-root acknowledgement parameter has no accepted qualification claim".into(),
        ));
    }
    if candidate
        .interrupt_mask_guard_claim
        .as_ref()
        .is_some_and(|claim| claim.requirement_identity.is_empty() || claim.domain.is_empty())
    {
        return Err(ExternalRootDiagnostic(
            "interrupt-mask guard claim requires exact requirement and domain identities".into(),
        ));
    }
    if candidate.stack.ceiling_bytes == 0 {
        return Err(ExternalRootDiagnostic(
            "external-root stack ceiling must be nonzero".into(),
        ));
    }
    if candidate.stack.realization.root() != candidate.identity {
        return Err(ExternalRootDiagnostic(
            "external-root stack realization does not name the candidate root".into(),
        ));
    }
    if candidate.stack.realization.root_provider() != candidate.provider {
        return Err(ExternalRootDiagnostic(
            "external-root stack realization provider does not match the selected provider".into(),
        ));
    }
    if candidate.stack.realization.relation() != candidate.nesting_relation {
        return Err(ExternalRootDiagnostic(
            "external-root stack realization does not use the selected nesting relation".into(),
        ));
    }
    if candidate.stack.realization.stack() != boundary.plan().state.stack {
        return Err(ExternalRootDiagnostic(
            "external-root stack realization does not match the boundary StatePlan stack".into(),
        ));
    }
    if candidate.stack.realization.composed_wcsu_bytes() > candidate.stack.ceiling_bytes {
        return Err(ExternalRootDiagnostic(
            "external-root composed WCSU exceeds the admitted stack ceiling".into(),
        ));
    }
    if candidate.logical_fuel.ceiling_units == 0 {
        return Err(ExternalRootDiagnostic(
            "external-root logical-fuel ceiling must be nonzero".into(),
        ));
    }
    if candidate.logical_fuel.schedule != candidate.logical_fuel.realization.schedule() {
        return Err(ExternalRootDiagnostic(
            "external-root fuel provision and realization use different schedule versions".into(),
        ));
    }
    if candidate.logical_fuel.realization.units() > candidate.logical_fuel.ceiling_units {
        return Err(ExternalRootDiagnostic(
            "external-root composed logical fuel exceeds the admitted ceiling".into(),
        ));
    }
    if candidate.logical_fuel.realization.root_provider() != candidate.provider {
        return Err(ExternalRootDiagnostic(
            "external-root logical-fuel root provider does not match the selected provider".into(),
        ));
    }
    validate_state_footprint(boundary, &candidate.machine_state.realization).map_err(|error| {
        ExternalRootDiagnostic(format!(
            "external-root machine-state realization is invalid: {error}"
        ))
    })?;
    let mut component_contracts = BTreeSet::new();
    for pin in &candidate.component_pins {
        if !component_contracts.insert(pin.contract) {
            return Err(ExternalRootDiagnostic(
                "external root cannot pin more than one realization of one component contract"
                    .into(),
            ));
        }
    }

    let boundary_contract_fingerprint = boundary.contract_fingerprint();
    let normalized_identity = fingerprint_root(&candidate, boundary_contract_fingerprint);
    Ok(ValidatedExternalRoot {
        candidate,
        boundary: boundary.clone(),
        boundary_contract_fingerprint,
        normalized_identity,
    })
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

fn fingerprint_root(candidate: &ExternalRootCandidate, boundary: u64) -> u64 {
    let mut hash = Fnv1a::new();
    hash.u64(candidate.identity.normalized_identity());
    hash.u64(candidate.entry.normalized_identity());
    hash.u64(candidate.provider.normalized_identity());
    hash.u64(candidate.provider_plan.normalized_identity());
    hash.string(&candidate.requirement_identity);
    hash.u64(candidate.entry_claims.len() as u64);
    for claim in &candidate.entry_claims {
        hash.u64(claim.parameter_index as u64);
        hash.string(&claim.domain);
        fingerprint_carry_policy(&mut hash, claim.effective_carry);
    }
    hash.u64(
        candidate
            .acknowledgement_parameter_index
            .map(|index| index as u64 + 1)
            .unwrap_or_default(),
    );
    match &candidate.interrupt_mask_guard_claim {
        Some(claim) => {
            hash.u64(1);
            hash.u64(claim.provider_plan.normalized_identity());
            hash.string(&claim.requirement_identity);
            hash.string(&claim.domain);
            fingerprint_carry_policy(&mut hash, claim.effective_carry);
        }
        None => hash.u64(0),
    }
    hash.u64(boundary);
    hash.u64(candidate.nesting_relation.normalized_identity());
    hash.u64(
        candidate
            .acknowledgement_policy
            .map(AcknowledgementPolicyId::normalized_identity)
            .unwrap_or_default(),
    );
    hash.u64(candidate.stack.ceiling_bytes);
    hash.u64(candidate.stack.realization.root().normalized_identity());
    hash.u64(
        candidate
            .stack
            .realization
            .root_provider()
            .normalized_identity(),
    );
    hash.u64(candidate.stack.realization.relation().normalized_identity());
    fingerprint_entry_stack(&mut hash, candidate.stack.realization.stack());
    hash.u64(candidate.stack.realization.local_wcsu_bytes());
    hash.u64(candidate.stack.realization.composed_wcsu_bytes());
    hash.u64(candidate.stack.realization.wcsu_alignment());
    hash.u64(
        candidate
            .stack
            .realization
            .artifact_composition_fingerprint(),
    );
    hash.u64(candidate.stack.realization.composition_fingerprint());
    hash.u64(candidate.stack.validation_receipt.normalized_identity());
    hash.u64(u64::from(candidate.logical_fuel.schedule.marker()));
    hash.u64(candidate.logical_fuel.provision.normalized_identity());
    hash.u64(candidate.logical_fuel.ceiling_units);
    hash.u64(
        candidate
            .logical_fuel
            .realization
            .root()
            .normalized_identity(),
    );
    hash.u64(
        candidate
            .logical_fuel
            .realization
            .root_provider()
            .normalized_identity(),
    );
    hash.u64(candidate.logical_fuel.realization.units());
    hash.u64(candidate.logical_fuel.realization.composition_fingerprint());
    hash.u64(
        candidate
            .logical_fuel
            .validation_receipt
            .normalized_identity(),
    );
    hash.u64(
        candidate
            .machine_state
            .realization
            .machine_state()
            .bits()
            .into(),
    );
    hash.u64(
        candidate
            .machine_state
            .realization
            .registers()
            .as_slice()
            .len() as u64,
    );
    for register in candidate.machine_state.realization.registers().as_slice() {
        hash.u64(machine_register_identity(*register));
    }
    hash.u64(
        candidate
            .machine_state
            .validation_receipt
            .normalized_identity(),
    );
    hash.u64(candidate.effects.len() as u64);
    for effect in &candidate.effects {
        hash.u64(effect.normalized_identity());
    }
    hash.u64(0xff01);
    hash.u64(candidate.trust_receipts.len() as u64);
    for receipt in &candidate.trust_receipts {
        hash.u64(receipt.normalized_identity());
    }
    hash.u64(0xff02);
    hash.u64(candidate.component_pins.len() as u64);
    for pin in &candidate.component_pins {
        hash.u64(pin.contract.normalized_identity());
        hash.u64(pin.artifact.normalized_identity());
        hash.u64(pin.provider.normalized_identity());
        hash.u64(pin.version.normalized_identity());
    }
    hash.finish()
}

fn machine_register_identity(register: MachineRegister) -> u64 {
    match register {
        MachineRegister::X86Rax => 0,
        MachineRegister::X86Rcx => 1,
        MachineRegister::X86Rdx => 2,
        MachineRegister::X86Rbx => 3,
        MachineRegister::X86Rsp => 4,
        MachineRegister::X86Rbp => 5,
        MachineRegister::X86Rsi => 6,
        MachineRegister::X86Rdi => 7,
        MachineRegister::X86R8 => 8,
        MachineRegister::X86R9 => 9,
        MachineRegister::X86R10 => 10,
        MachineRegister::X86R11 => 11,
        MachineRegister::X86R12 => 12,
        MachineRegister::X86R13 => 13,
        MachineRegister::X86R14 => 14,
        MachineRegister::X86R15 => 15,
        MachineRegister::X86Xmm(index) => 0x100 + u64::from(index),
        MachineRegister::Aarch64X(index) => 0x200 + u64::from(index),
        MachineRegister::Aarch64V(index) => 0x300 + u64::from(index),
    }
}

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

fn fingerprint_carry_policy(hash: &mut Fnv1a, policy: psi_language_semantics::CarryPolicy) {
    use psi_language_semantics::{CarryAddress, CarryCpu, CarryHostThread, CarrySuspension};

    hash.u64(match policy.suspension {
        CarrySuspension::Forbidden => 0,
        CarrySuspension::Allowed => 1,
    });
    hash.u64(match policy.cpu {
        CarryCpu::Origin => 0,
        CarryCpu::Any => 1,
    });
    hash.u64(match policy.host_thread {
        CarryHostThread::Origin => 0,
        CarryHostThread::Any => 1,
    });
    hash.u64(match policy.address {
        CarryAddress::Stable => 0,
        CarryAddress::Movable => 1,
    });
}

#[cfg(test)]
mod tests;
