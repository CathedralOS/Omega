//! Installed terminal fuel binding and fixed-fuel graph composition.
//!
//! This module owns exact entry/segment certificate binding, provider-local
//! fixed-fuel evidence, acyclic graph composition, overflow checks, and stable
//! graph fingerprints. It does not provision runtime fuel, admit roots, or
//! execute providers.

use std::collections::{BTreeMap, BTreeSet};

use omega_executable_installation::{
    ArtifactId, InstalledCode, InstalledCodeContext, InstalledCodeId,
};
use omega_terminal_installation_evidence::TerminalObjectEvidence;
use psi_core::FuelScheduleIdentity;
use psi_layout_plans::EntryStubId;

use super::{
    ExternalRootDiagnostic, Fnv1a, FuelProvisionId, FuelValidationReceiptId, ProviderFuelSummaryId,
    ProviderFuelValidationReceiptId, RootProviderId, bind_terminal_function,
};

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
pub(super) struct FixedFuelCompositionEvidence {
    pub(super) summaries: BTreeMap<ProviderFuelSummaryId, FixedFuelProviderSummary>,
}

/// Canonical transitive result of a fixed-fuel provider graph. The private
/// fields ensure callers cannot hand-author a demand that skipped a callee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedFuelDemand {
    root: ProviderFuelSummaryId,
    root_provider: RootProviderId,
    schedule: FuelScheduleIdentity,
    pub(super) units: u64,
    pub(super) summaries: BTreeSet<ProviderFuelSummaryId>,
    pub(super) provider_receipts: BTreeSet<ProviderFuelValidationReceiptId>,
    pub(super) composition_evidence: FixedFuelCompositionEvidence,
    pub(super) composition_fingerprint: u64,
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
