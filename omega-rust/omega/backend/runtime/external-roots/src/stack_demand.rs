//! Installed terminal-entry stack binding and artifact-wide WCSU composition.
//!
//! This module owns exact local stack evidence, nesting relations, cycle and
//! dedicated-stack reentry rejection, transitive peak composition, and stable
//! composition fingerprints. It does not admit external roots, provision
//! runtime storage, or execute providers.

use std::collections::{BTreeMap, BTreeSet};

use calling_conventions::EntryStack;
use executable_installation::{ArtifactId, InstalledCode, InstalledCodeContext, InstalledCodeId};
use installation_evidence::{ObjectEvidence, StackDemandEvidence};
use layout_plans::EntryStubId;

use super::{
    BoundEpochStackComposition, ExternalRootDiagnostic, ExternalRootId, Fnv1a, NestingRelationId,
    RootProviderId, StackValidationReceiptId, bind_terminal_function,
};

/// A terminal-Psi stack closure bound to the exact installed bytes and entry
/// stub selected for one external root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledEntryStackDemand {
    psi: terminal_psi::TerminalPsiIdentity,
    architecture: target::Architecture,
    machine_entry: semantic_vocabulary::MachineId,
    ceiling_bytes: u64,
    stack_alignment: u64,
    contributing_machines: BTreeSet<semantic_vocabulary::MachineId>,
    admitted_stack_contribution_report_identities: BTreeSet<u64>,
    admitted_stack_contribution_commitments: BTreeSet<[u8; 32]>,
    installed_code: InstalledCodeId,
    installed_code_context: InstalledCodeContext,
    artifact: ArtifactId,
    entry: EntryStubId,
}

impl InstalledEntryStackDemand {
    pub const fn psi(&self) -> terminal_psi::TerminalPsiIdentity {
        self.psi
    }

    pub const fn machine_entry(&self) -> semantic_vocabulary::MachineId {
        self.machine_entry
    }

    pub const fn ceiling_bytes(&self) -> u64 {
        self.ceiling_bytes
    }

    pub const fn stack_alignment(&self) -> u64 {
        self.stack_alignment
    }

    pub const fn contributing_machines(&self) -> &BTreeSet<semantic_vocabulary::MachineId> {
        &self.contributing_machines
    }

    pub const fn admitted_stack_contribution_report_identities(&self) -> &BTreeSet<u64> {
        &self.admitted_stack_contribution_report_identities
    }

    pub const fn admitted_stack_contribution_commitments(&self) -> &BTreeSet<[u8; 32]> {
        &self.admitted_stack_contribution_commitments
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

    pub(super) fn matches_installed_entry(
        &self,
        installed_code: &InstalledCode,
        entry: EntryStubId,
    ) -> bool {
        self.entry == entry
            && self.installed_code == installed_code.identity()
            && self.installed_code_context == installed_code.receipt_context()
            && self.artifact == installed_code.artifact()
    }
}

/// Bind one emitter-derived terminal stack closure to exact installed bytes
/// and the selected external entry stub.
pub fn bind_installed_entry_stack<
    TerminalArtifact: ObjectEvidence,
    StackDemand: StackDemandEvidence,
>(
    demand: &StackDemand,
    artifact: &TerminalArtifact,
    installed_code: &InstalledCode,
    entry: EntryStubId,
) -> Result<InstalledEntryStackDemand, ExternalRootDiagnostic> {
    if demand.psi() != artifact.psi() {
        return Err(ExternalRootDiagnostic(
            "terminal stack demand does not name the terminal artifact's semantic identity".into(),
        ));
    }
    if demand.architecture() != artifact.architecture() {
        return Err(ExternalRootDiagnostic(
            "terminal stack demand target does not match the terminal artifact architecture".into(),
        ));
    }
    if demand.ceiling_bytes() == 0
        || demand.stack_alignment() == 0
        || !demand.stack_alignment().is_power_of_two()
    {
        return Err(ExternalRootDiagnostic(
            "terminal stack demand requires nonzero bytes and power-of-two alignment".into(),
        ));
    }
    let admitted_stack_contribution_report_identities =
        demand.admitted_stack_contribution_report_identities();
    let admitted_stack_contribution_commitments = demand.admitted_stack_contribution_commitments();
    if admitted_stack_contribution_report_identities.contains(&0)
        || admitted_stack_contribution_commitments.contains(&[0; 32])
        || admitted_stack_contribution_report_identities.is_empty()
            != admitted_stack_contribution_commitments.is_empty()
    {
        return Err(ExternalRootDiagnostic(
            "terminal stack demand has incomplete or zero admitted same-stack provenance".into(),
        ));
    }
    let function_offset = artifact
        .function_text_offset(demand.entry())
        .ok_or_else(|| {
            ExternalRootDiagnostic(
                "terminal stack-demand entry is not present in the emitted artifact".into(),
            )
        })?;
    bind_terminal_function(artifact, installed_code, entry, function_offset)?;
    Ok(InstalledEntryStackDemand {
        psi: demand.psi(),
        architecture: demand.architecture(),
        machine_entry: demand.entry(),
        ceiling_bytes: demand.ceiling_bytes(),
        stack_alignment: u64::from(demand.stack_alignment()),
        contributing_machines: demand.contributing_machines().clone(),
        admitted_stack_contribution_report_identities,
        admitted_stack_contribution_commitments,
        installed_code: installed_code.identity(),
        installed_code_context: installed_code.receipt_context(),
        artifact: installed_code.artifact(),
        entry,
    })
}

pub fn validate_installed_entry_stack(
    binding: &InstalledEntryStackDemand,
    installed_code: &InstalledCode,
    entry: EntryStubId,
) -> Result<(), ExternalRootDiagnostic> {
    if !binding.matches_installed_entry(installed_code, entry) {
        return Err(ExternalRootDiagnostic(
            "terminal stack demand does not bind the selected installed code and entry".into(),
        ));
    }
    Ok(())
}

/// Exact evidence for one provider's local stack demand. Checked terminal
/// code contributes a byte- and entry-bound closure; an opaque provider must
/// instead retain its explicit admission receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StackLocalEvidence {
    TerminalEntry(InstalledEntryStackDemand),
    AdmittedProvider {
        local_wcsu_bytes: u64,
        wcsu_alignment: u64,
        validation_receipt: StackValidationReceiptId,
    },
}

impl StackLocalEvidence {
    pub const fn local_wcsu_bytes(&self) -> u64 {
        match self {
            Self::TerminalEntry(binding) => binding.ceiling_bytes,
            Self::AdmittedProvider {
                local_wcsu_bytes, ..
            } => *local_wcsu_bytes,
        }
    }

    pub const fn wcsu_alignment(&self) -> u64 {
        match self {
            Self::TerminalEntry(binding) => binding.stack_alignment,
            Self::AdmittedProvider { wcsu_alignment, .. } => *wcsu_alignment,
        }
    }

    pub const fn provider_validation_receipt(&self) -> Option<StackValidationReceiptId> {
        match self {
            Self::TerminalEntry(_) => None,
            Self::AdmittedProvider {
                validation_receipt, ..
            } => Some(*validation_receipt),
        }
    }
}

/// One provider's validated local stack demand for an external entry.
///
/// `stack` is copied from the entry's normalized `StatePlan`; composition and
/// final root admission verify that it has not drifted from that source fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderStackSummary {
    pub root: ExternalRootId,
    pub provider: RootProviderId,
    pub stack: EntryStack,
    pub local_evidence: StackLocalEvidence,
}

impl ProviderStackSummary {
    pub fn from_admitted_provider(
        root: ExternalRootId,
        provider: RootProviderId,
        stack: EntryStack,
        local_wcsu_bytes: u64,
        wcsu_alignment: u64,
        validation_receipt: StackValidationReceiptId,
    ) -> Self {
        Self {
            root,
            provider,
            stack,
            local_evidence: StackLocalEvidence::AdmittedProvider {
                local_wcsu_bytes,
                wcsu_alignment,
                validation_receipt,
            },
        }
    }

    pub fn from_entry(
        root: ExternalRootId,
        provider: RootProviderId,
        stack: EntryStack,
        demand: InstalledEntryStackDemand,
    ) -> Self {
        Self {
            root,
            provider,
            stack,
            local_evidence: StackLocalEvidence::TerminalEntry(demand),
        }
    }

    pub const fn local_wcsu_bytes(&self) -> u64 {
        self.local_evidence.local_wcsu_bytes()
    }

    pub const fn wcsu_alignment(&self) -> u64 {
        self.local_evidence.wcsu_alignment()
    }
}

/// One possible asynchronous preemption in an artifact-wide nesting relation.
/// `preemptor` may enter while `interrupted` is live.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct StackNestingEdge {
    pub interrupted: ExternalRootId,
    pub preemptor: ExternalRootId,
}

/// Exact architecture/provider nesting graph consumed by stack composition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackNestingRelation {
    pub identity: NestingRelationId,
    pub edges: BTreeSet<StackNestingEdge>,
}

/// Exact canonical inputs retained behind every stack-composition result.
///
/// Compact fingerprints remain useful report keys, but are not admission
/// evidence: two distinct nesting graphs or provider summaries must remain
/// distinguishable even if their compact fingerprints collide.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StackCompositionEvidence {
    relation: StackNestingRelation,
    pub(super) summaries: BTreeMap<ExternalRootId, ProviderStackSummary>,
}

/// Provisioning domain produced from the one normalized `EntryStack` fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StackDomain {
    Interrupted,
    Dedicated { class: u16 },
    ProviderSelected,
}

/// Canonical transitive stack result for one external root. Private fields
/// prevent an unaudited caller-authored composed WCSU from entering the ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedStackDemand {
    root: ExternalRootId,
    root_provider: RootProviderId,
    relation: NestingRelationId,
    stack: EntryStack,
    local_wcsu_bytes: u64,
    pub(super) composed_wcsu_bytes: u64,
    wcsu_alignment: u64,
    pub(super) contributing_roots: BTreeSet<ExternalRootId>,
    validation_receipts: BTreeSet<StackValidationReceiptId>,
    pub(super) composition_evidence: StackCompositionEvidence,
    pub(super) non_authoritative_artifact_composition_report_fingerprint: u64,
    pub(super) non_authoritative_composition_report_fingerprint: u64,
}

impl ComposedStackDemand {
    pub const fn root(&self) -> ExternalRootId {
        self.root
    }

    pub const fn root_provider(&self) -> RootProviderId {
        self.root_provider
    }

    pub const fn relation(&self) -> NestingRelationId {
        self.relation
    }

    pub const fn stack(&self) -> EntryStack {
        self.stack
    }

    pub const fn local_wcsu_bytes(&self) -> u64 {
        self.local_wcsu_bytes
    }

    pub const fn composed_wcsu_bytes(&self) -> u64 {
        self.composed_wcsu_bytes
    }

    pub const fn wcsu_alignment(&self) -> u64 {
        self.wcsu_alignment
    }

    pub const fn contributing_roots(&self) -> &BTreeSet<ExternalRootId> {
        &self.contributing_roots
    }

    pub const fn validation_receipts(&self) -> &BTreeSet<StackValidationReceiptId> {
        &self.validation_receipts
    }

    pub const fn non_authoritative_composition_report_fingerprint(&self) -> u64 {
        self.non_authoritative_composition_report_fingerprint
    }

    pub const fn non_authoritative_artifact_composition_report_fingerprint(&self) -> u64 {
        self.non_authoritative_artifact_composition_report_fingerprint
    }

    pub fn summary_evidence(
        &self,
    ) -> impl Iterator<Item = (&ExternalRootId, &ProviderStackSummary)> {
        self.composition_evidence.summaries.iter()
    }
}

/// Canonical artifact-wide WCSU result. Per-domain provisioning takes the
/// maximum of roots that begin in that domain; sequential entries reuse the
/// same storage instead of being summed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactStackComposition {
    relation: NestingRelationId,
    demands: BTreeMap<ExternalRootId, ComposedStackDemand>,
    domain_wcsu_bytes: BTreeMap<StackDomain, u64>,
    domain_alignments: BTreeMap<StackDomain, u64>,
    non_authoritative_composition_report_fingerprint: u64,
}

impl ArtifactStackComposition {
    pub const fn relation(&self) -> NestingRelationId {
        self.relation
    }

    pub fn demand(&self, root: ExternalRootId) -> Option<&ComposedStackDemand> {
        self.demands.get(&root)
    }

    pub fn domain_wcsu_bytes(&self, domain: StackDomain) -> Option<u64> {
        self.domain_wcsu_bytes.get(&domain).copied()
    }

    pub fn domain_alignment(&self, domain: StackDomain) -> Option<u64> {
        self.domain_alignments.get(&domain).copied()
    }

    pub const fn non_authoritative_composition_report_fingerprint(&self) -> u64 {
        self.non_authoritative_composition_report_fingerprint
    }
}

/// Stack provisioning admitted for one external root. The stack domain itself
/// remains the single value in `BoundaryEntryPlan::state.stack`; this column
/// adds a ceiling and the sealed artifact-wide composition that refines it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackResourceColumn {
    pub ceiling_bytes: u64,
    pub realization: BoundEpochStackComposition,
    pub validation_receipt: StackValidationReceiptId,
}

/// Compose every provider stack summary under one exact artifact-wide nesting
/// relation. Interrupted-stack entries add (with alignment) to the active
/// domain. Dedicated entries switch domains. Re-entering a dedicated class
/// that is already active is rejected because provisioning cannot make such a
/// reset-style stack switch preserve the suspended frames.
pub fn compose_artifact_stacks<'a>(
    relation: &StackNestingRelation,
    summaries: impl IntoIterator<Item = &'a ProviderStackSummary>,
) -> Result<ArtifactStackComposition, ExternalRootDiagnostic> {
    let mut by_root = BTreeMap::new();
    for summary in summaries {
        if summary.local_wcsu_bytes() == 0 {
            return Err(ExternalRootDiagnostic(format!(
                "provider stack summary for root 0x{:016x} has zero local WCSU",
                summary.root.normalized_identity()
            )));
        }
        if summary.wcsu_alignment() == 0 || !summary.wcsu_alignment().is_power_of_two() {
            return Err(ExternalRootDiagnostic(format!(
                "provider stack summary for root 0x{:016x} has alignment {} instead of a nonzero power of two",
                summary.root.normalized_identity(),
                summary.wcsu_alignment()
            )));
        }
        if by_root.insert(summary.root, summary).is_some() {
            return Err(ExternalRootDiagnostic(format!(
                "provider stack summary for root 0x{:016x} is duplicated",
                summary.root.normalized_identity()
            )));
        }
    }
    if by_root.is_empty() {
        return Err(ExternalRootDiagnostic(
            "artifact stack composition requires at least one provider summary".into(),
        ));
    }

    let mut outgoing: BTreeMap<ExternalRootId, Vec<ExternalRootId>> = BTreeMap::new();
    for edge in &relation.edges {
        if !by_root.contains_key(&edge.interrupted) {
            return Err(ExternalRootDiagnostic(format!(
                "stack nesting relation references missing interrupted root 0x{:016x}",
                edge.interrupted.normalized_identity()
            )));
        }
        let preemptor = by_root.get(&edge.preemptor).ok_or_else(|| {
            ExternalRootDiagnostic(format!(
                "stack nesting relation references missing preemptor root 0x{:016x}",
                edge.preemptor.normalized_identity()
            ))
        })?;
        if preemptor.stack == EntryStack::ProviderSelected {
            return Err(ExternalRootDiagnostic(format!(
                "provider-selected stack for nested root 0x{:016x} does not determine whether the active stack is shared or switched",
                edge.preemptor.normalized_identity()
            )));
        }
        outgoing
            .entry(edge.interrupted)
            .or_default()
            .push(edge.preemptor);
    }

    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for root in by_root.keys().copied() {
        reject_stack_cycle(root, &outgoing, &mut visiting, &mut visited)?;
    }
    for root in by_root.keys().copied() {
        let mut active_classes = BTreeSet::new();
        reject_dedicated_stack_reentry(root, &outgoing, &by_root, &mut active_classes)?;
    }

    let composition_evidence = StackCompositionEvidence {
        relation: relation.clone(),
        summaries: by_root
            .iter()
            .map(|(root, summary)| (*root, (*summary).clone()))
            .collect(),
    };
    let non_authoritative_input_report_fingerprint =
        non_authoritative_stack_inputs_report_fingerprint(relation, &by_root);
    let mut demands = BTreeMap::new();
    let mut domain_wcsu_bytes = BTreeMap::new();
    let mut domain_alignments = BTreeMap::new();
    for (root, summary) in &by_root {
        let mut contributing_roots = BTreeSet::from([*root]);
        let mut validation_receipts = summary
            .local_evidence
            .provider_validation_receipt()
            .into_iter()
            .collect();
        let (composed_wcsu_bytes, wcsu_alignment) = compose_active_stack_peak(
            *root,
            summary.local_wcsu_bytes(),
            summary.wcsu_alignment(),
            &outgoing,
            &by_root,
            &mut contributing_roots,
            &mut validation_receipts,
        )?;
        let domain = stack_domain(summary.stack);
        domain_wcsu_bytes
            .entry(domain)
            .and_modify(|bytes: &mut u64| *bytes = (*bytes).max(composed_wcsu_bytes))
            .or_insert(composed_wcsu_bytes);
        domain_alignments
            .entry(domain)
            .and_modify(|alignment: &mut u64| *alignment = (*alignment).max(wcsu_alignment))
            .or_insert(wcsu_alignment);

        let mut report_fingerprint = Fnv1a::new();
        report_fingerprint.u64(non_authoritative_input_report_fingerprint);
        report_fingerprint.u64(root.normalized_identity());
        report_fingerprint.u64(composed_wcsu_bytes);
        report_fingerprint.u64(wcsu_alignment);
        for contributor in &contributing_roots {
            report_fingerprint.u64(contributor.normalized_identity());
        }
        demands.insert(
            *root,
            ComposedStackDemand {
                root: *root,
                root_provider: summary.provider,
                relation: relation.identity,
                stack: summary.stack,
                local_wcsu_bytes: summary.local_wcsu_bytes(),
                composed_wcsu_bytes,
                wcsu_alignment,
                contributing_roots,
                validation_receipts,
                composition_evidence: composition_evidence.clone(),
                non_authoritative_artifact_composition_report_fingerprint:
                    non_authoritative_input_report_fingerprint,
                non_authoritative_composition_report_fingerprint: report_fingerprint.finish(),
            },
        );
    }
    Ok(ArtifactStackComposition {
        relation: relation.identity,
        demands,
        domain_wcsu_bytes,
        domain_alignments,
        non_authoritative_composition_report_fingerprint:
            non_authoritative_input_report_fingerprint,
    })
}

fn reject_stack_cycle(
    root: ExternalRootId,
    outgoing: &BTreeMap<ExternalRootId, Vec<ExternalRootId>>,
    visiting: &mut BTreeSet<ExternalRootId>,
    visited: &mut BTreeSet<ExternalRootId>,
) -> Result<(), ExternalRootDiagnostic> {
    if visited.contains(&root) {
        return Ok(());
    }
    if !visiting.insert(root) {
        return Err(ExternalRootDiagnostic(format!(
            "stack nesting relation contains a cycle through root 0x{:016x}",
            root.normalized_identity()
        )));
    }
    if let Some(preemptors) = outgoing.get(&root) {
        for preemptor in preemptors {
            reject_stack_cycle(*preemptor, outgoing, visiting, visited)?;
        }
    }
    visiting.remove(&root);
    visited.insert(root);
    Ok(())
}

fn reject_dedicated_stack_reentry(
    root: ExternalRootId,
    outgoing: &BTreeMap<ExternalRootId, Vec<ExternalRootId>>,
    summaries: &BTreeMap<ExternalRootId, &ProviderStackSummary>,
    active_classes: &mut BTreeSet<u16>,
) -> Result<(), ExternalRootDiagnostic> {
    let stack = summaries
        .get(&root)
        .expect("nesting root has summary")
        .stack;
    let inserted = match stack {
        EntryStack::Dedicated { class } => {
            if !active_classes.insert(class) {
                return Err(ExternalRootDiagnostic(format!(
                    "stack nesting path re-enters active dedicated class {} at root 0x{:016x}",
                    class,
                    root.normalized_identity()
                )));
            }
            Some(class)
        }
        EntryStack::Interrupted | EntryStack::ProviderSelected => None,
    };
    if let Some(preemptors) = outgoing.get(&root) {
        for preemptor in preemptors {
            reject_dedicated_stack_reentry(*preemptor, outgoing, summaries, active_classes)?;
        }
    }
    if let Some(class) = inserted {
        active_classes.remove(&class);
    }
    Ok(())
}

fn compose_active_stack_peak(
    root: ExternalRootId,
    current_bytes: u64,
    current_alignment: u64,
    outgoing: &BTreeMap<ExternalRootId, Vec<ExternalRootId>>,
    summaries: &BTreeMap<ExternalRootId, &ProviderStackSummary>,
    contributing_roots: &mut BTreeSet<ExternalRootId>,
    validation_receipts: &mut BTreeSet<StackValidationReceiptId>,
) -> Result<(u64, u64), ExternalRootDiagnostic> {
    let mut peak = current_bytes;
    let mut alignment = current_alignment;
    if let Some(preemptors) = outgoing.get(&root) {
        for preemptor in preemptors {
            let summary = summaries
                .get(preemptor)
                .expect("nesting preemptor has summary");
            if summary.stack != EntryStack::Interrupted {
                continue;
            }
            contributing_roots.insert(*preemptor);
            validation_receipts.extend(summary.local_evidence.provider_validation_receipt());
            let aligned = align_up_checked(current_bytes, summary.wcsu_alignment())?;
            let nested_bytes =
                aligned
                    .checked_add(summary.local_wcsu_bytes())
                    .ok_or_else(|| {
                        ExternalRootDiagnostic("stack WCSU composition addition overflowed".into())
                    })?;
            let (nested_peak, nested_alignment) = compose_active_stack_peak(
                *preemptor,
                nested_bytes,
                current_alignment.max(summary.wcsu_alignment()),
                outgoing,
                summaries,
                contributing_roots,
                validation_receipts,
            )?;
            peak = peak.max(nested_peak);
            alignment = alignment.max(nested_alignment);
        }
    }
    Ok((peak, alignment))
}

pub(super) fn align_up_checked(value: u64, alignment: u64) -> Result<u64, ExternalRootDiagnostic> {
    value
        .checked_add(alignment - 1)
        .map(|sum| sum & !(alignment - 1))
        .ok_or_else(|| ExternalRootDiagnostic("stack WCSU alignment overflowed".into()))
}

const fn stack_domain(stack: EntryStack) -> StackDomain {
    match stack {
        EntryStack::Interrupted => StackDomain::Interrupted,
        EntryStack::Dedicated { class } => StackDomain::Dedicated { class },
        EntryStack::ProviderSelected => StackDomain::ProviderSelected,
    }
}

fn non_authoritative_stack_inputs_report_fingerprint(
    relation: &StackNestingRelation,
    summaries: &BTreeMap<ExternalRootId, &ProviderStackSummary>,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.u64(relation.identity.normalized_identity());
    hash.u64(summaries.len() as u64);
    for summary in summaries.values() {
        hash.u64(summary.root.normalized_identity());
        hash.u64(summary.provider.normalized_identity());
        fingerprint_entry_stack(&mut hash, summary.stack);
        fingerprint_stack_local_evidence(&mut hash, &summary.local_evidence);
    }
    hash.u64(relation.edges.len() as u64);
    for edge in &relation.edges {
        hash.u64(edge.interrupted.normalized_identity());
        hash.u64(edge.preemptor.normalized_identity());
    }
    hash.finish()
}

pub(super) fn fingerprint_stack_local_evidence(hash: &mut Fnv1a, evidence: &StackLocalEvidence) {
    match evidence {
        StackLocalEvidence::TerminalEntry(binding) => {
            hash.u64(0);
            hash.u64(u64::from(binding.psi.vocabulary_marker.get()));
            hash.bytes(binding.psi.program_fingerprint.as_bytes());
            hash.u64(match binding.architecture {
                target::Architecture::X86_64 => 1,
                target::Architecture::Aarch64 => 2,
            });
            hash.u64(binding.machine_entry.get());
            hash.u64(binding.ceiling_bytes);
            hash.u64(binding.stack_alignment);
            hash.u64(binding.contributing_machines.len() as u64);
            for machine in &binding.contributing_machines {
                hash.u64(machine.get());
            }
            hash.u64(binding.admitted_stack_contribution_report_identities.len() as u64);
            for report_identity in &binding.admitted_stack_contribution_report_identities {
                hash.u64(*report_identity);
            }
            hash.u64(binding.admitted_stack_contribution_commitments.len() as u64);
            for commitment in &binding.admitted_stack_contribution_commitments {
                hash.bytes(commitment);
            }
            hash.u64(binding.installed_code.normalized_identity());
            hash.u64(binding.artifact.normalized_identity());
            hash.u64(binding.entry.normalized_identity());
        }
        StackLocalEvidence::AdmittedProvider {
            local_wcsu_bytes,
            wcsu_alignment,
            validation_receipt,
        } => {
            hash.u64(1);
            hash.u64(*local_wcsu_bytes);
            hash.u64(*wcsu_alignment);
            hash.u64(validation_receipt.normalized_identity());
        }
    }
}

pub(super) fn fingerprint_entry_stack(hash: &mut Fnv1a, stack: EntryStack) {
    match stack {
        EntryStack::Interrupted => hash.u64(0),
        EntryStack::Dedicated { class } => {
            hash.u64(1);
            hash.u64(u64::from(class));
        }
        EntryStack::ProviderSelected => hash.u64(2),
    }
}
