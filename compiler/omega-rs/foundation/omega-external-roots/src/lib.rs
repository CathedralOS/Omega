//! Normalized ledger for entry points invoked from outside Omega's call graph.
//!
//! Installing code does not make any of its entries analysis roots. A slot
//! owner separately installs one admitted entry under a validated boundary
//! plan. That operation records the root's effects, trust receipts, stack and
//! nesting policy, WCSU demand, and component/version pins. The returned
//! handle borrows the installed code, preventing retirement while the root is
//! reachable.

use std::collections::{BTreeMap, BTreeSet};

use omega_calling_conventions::{
    BoundaryEntryPlan, EntryControl, EntryStack, MachineRegister, ProviderExitRealization,
    StateFootprintEvidence, ValidatedBoundaryEntryPlan, validate_provider_exit_realization,
    validate_state_footprint,
};
pub use omega_executable_installation::{ArtifactId, InstalledCodeId};
use omega_executable_installation::{InstalledCode, ResolvedPostHandoffEntryWriterContext};
use omega_layout_plans::{
    ByteOrder, EntryStubId, PlacementPhase, PlacementSite, PostHandoffWriterPlan,
    PostHandoffWriterSource, RelocationTarget,
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
normalized_id!(ProviderWorkSummaryId, "fixed-work provider summary");
normalized_id!(StructuralWorkProfileId, "structural-work profile");
normalized_id!(
    ProviderWorkValidationReceiptId,
    "fixed-work provider validation receipt"
);
normalized_id!(
    StructuralWorkValidationReceiptId,
    "structural-work validation receipt"
);
normalized_id!(StateValidationReceiptId, "machine-state validation receipt");
normalized_id!(MaterializedIdtId, "materialized IDT");
normalized_id!(IdtDestinationId, "IDT destination");
normalized_id!(IdtWriterPreparationId, "IDT writer preparation");
normalized_id!(IdtWriterContextId, "IDT writer context");
normalized_id!(IdtMaterializationReceiptId, "IDT materialization receipt");
normalized_id!(IdtControlId, "IDT control authority");
normalized_id!(IdtInstallationReceiptId, "IDT installation receipt");
normalized_id!(InstalledIdtId, "installed IDT");
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentVersionPin {
    pub contract: ComponentContractId,
    pub artifact: ComponentArtifactId,
    pub provider: ComponentProviderId,
    pub version: ComponentVersionPinId,
}

/// One provider's validated local stack demand for an external entry.
///
/// `stack` is copied from the entry's normalized `StatePlan`; composition and
/// final root admission verify that it has not drifted from that source fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderStackSummary {
    pub root: ExternalRootId,
    pub provider: RootProviderId,
    pub stack: EntryStack,
    pub local_wcsu_bytes: u64,
    pub wcsu_alignment: u64,
    pub validation_receipt: StackValidationReceiptId,
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
    composed_wcsu_bytes: u64,
    wcsu_alignment: u64,
    contributing_roots: BTreeSet<ExternalRootId>,
    validation_receipts: BTreeSet<StackValidationReceiptId>,
    artifact_composition_fingerprint: u64,
    composition_fingerprint: u64,
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

    pub const fn composition_fingerprint(&self) -> u64 {
        self.composition_fingerprint
    }

    pub const fn artifact_composition_fingerprint(&self) -> u64 {
        self.artifact_composition_fingerprint
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
    composition_fingerprint: u64,
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

    pub const fn composition_fingerprint(&self) -> u64 {
        self.composition_fingerprint
    }
}

/// Stack provisioning admitted for one external root. The stack domain itself
/// remains the single value in `BoundaryEntryPlan::state.stack`; this column
/// adds a ceiling and the sealed artifact-wide composition that refines it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackResourceColumn {
    pub ceiling_bytes: u64,
    pub realization: ComposedStackDemand,
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
        if summary.local_wcsu_bytes == 0 {
            return Err(ExternalRootDiagnostic(format!(
                "provider stack summary for root 0x{:016x} has zero local WCSU",
                summary.root.normalized_identity()
            )));
        }
        if summary.wcsu_alignment == 0 || !summary.wcsu_alignment.is_power_of_two() {
            return Err(ExternalRootDiagnostic(format!(
                "provider stack summary for root 0x{:016x} has alignment {} instead of a nonzero power of two",
                summary.root.normalized_identity(),
                summary.wcsu_alignment
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

    let input_fingerprint = fingerprint_stack_inputs(relation, &by_root);
    let mut demands = BTreeMap::new();
    let mut domain_wcsu_bytes = BTreeMap::new();
    let mut domain_alignments = BTreeMap::new();
    for (root, summary) in &by_root {
        let mut contributing_roots = BTreeSet::from([*root]);
        let mut validation_receipts = BTreeSet::from([summary.validation_receipt]);
        let (composed_wcsu_bytes, wcsu_alignment) = compose_active_stack_peak(
            *root,
            summary.local_wcsu_bytes,
            summary.wcsu_alignment,
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

        let mut fingerprint = Fnv1a::new();
        fingerprint.u64(input_fingerprint);
        fingerprint.u64(root.normalized_identity());
        fingerprint.u64(composed_wcsu_bytes);
        fingerprint.u64(wcsu_alignment);
        for contributor in &contributing_roots {
            fingerprint.u64(contributor.normalized_identity());
        }
        demands.insert(
            *root,
            ComposedStackDemand {
                root: *root,
                root_provider: summary.provider,
                relation: relation.identity,
                stack: summary.stack,
                local_wcsu_bytes: summary.local_wcsu_bytes,
                composed_wcsu_bytes,
                wcsu_alignment,
                contributing_roots,
                validation_receipts,
                artifact_composition_fingerprint: input_fingerprint,
                composition_fingerprint: fingerprint.finish(),
            },
        );
    }
    Ok(ArtifactStackComposition {
        relation: relation.identity,
        demands,
        domain_wcsu_bytes,
        domain_alignments,
        composition_fingerprint: input_fingerprint,
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
            validation_receipts.insert(summary.validation_receipt);
            let aligned = align_up_checked(current_bytes, summary.wcsu_alignment)?;
            let nested_bytes = aligned
                .checked_add(summary.local_wcsu_bytes)
                .ok_or_else(|| {
                    ExternalRootDiagnostic("stack WCSU composition addition overflowed".into())
                })?;
            let (nested_peak, nested_alignment) = compose_active_stack_peak(
                *preemptor,
                nested_bytes,
                current_alignment.max(summary.wcsu_alignment),
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

fn align_up_checked(value: u64, alignment: u64) -> Result<u64, ExternalRootDiagnostic> {
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

fn fingerprint_stack_inputs(
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
        hash.u64(summary.local_wcsu_bytes);
        hash.u64(summary.wcsu_alignment);
        hash.u64(summary.validation_receipt.normalized_identity());
    }
    hash.u64(relation.edges.len() as u64);
    for edge in &relation.edges {
        hash.u64(edge.interrupted.normalized_identity());
        hash.u64(edge.preemptor.normalized_identity());
    }
    hash.finish()
}

fn fingerprint_entry_stack(hash: &mut Fnv1a, stack: EntryStack) {
    match stack {
        EntryStack::Interrupted => hash.u64(0),
        EntryStack::Dedicated { class } => {
            hash.u64(1);
            hash.u64(u64::from(class));
        }
        EntryStack::ProviderSelected => hash.u64(2),
    }
}

/// One bounded call edge in a fixed-work provider summary. Multiplicity is
/// explicit: a set of callees alone cannot distinguish one invocation from a
/// bounded repeated use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FixedWorkCall {
    pub callee: ProviderWorkSummaryId,
    pub maximum_invocations: u64,
}

/// Public fixed-work summary supplied by a selected provider. This is a finite
/// structural demand, not cycles or WCET. Absence of a summary fails closed;
/// recursive summary graphs are rejected by composition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedWorkProviderSummary {
    pub identity: ProviderWorkSummaryId,
    pub provider: RootProviderId,
    pub local_units: u64,
    pub calls: BTreeSet<FixedWorkCall>,
    pub validation_receipt: ProviderWorkValidationReceiptId,
}

/// Canonical transitive result of a fixed-work provider graph. The private
/// fields ensure callers cannot hand-author a demand that skipped a callee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedFixedWorkDemand {
    root: ProviderWorkSummaryId,
    root_provider: RootProviderId,
    units: u64,
    summaries: BTreeSet<ProviderWorkSummaryId>,
    provider_receipts: BTreeSet<ProviderWorkValidationReceiptId>,
    composition_fingerprint: u64,
}

impl ComposedFixedWorkDemand {
    pub const fn root(&self) -> ProviderWorkSummaryId {
        self.root
    }

    pub const fn root_provider(&self) -> RootProviderId {
        self.root_provider
    }

    pub const fn units(&self) -> u64 {
        self.units
    }

    pub const fn composition_fingerprint(&self) -> u64 {
        self.composition_fingerprint
    }

    pub const fn summaries(&self) -> &BTreeSet<ProviderWorkSummaryId> {
        &self.summaries
    }

    pub const fn provider_receipts(&self) -> &BTreeSet<ProviderWorkValidationReceiptId> {
        &self.provider_receipts
    }
}

/// Compose an acyclic graph of admitted fixed-work summaries. Each edge's
/// maximum invocation count multiplies the callee's complete demand; missing
/// summaries, zero-count edges, cycles, duplicates, and arithmetic overflow
/// all fail closed.
pub fn compose_fixed_work<'a>(
    root: ProviderWorkSummaryId,
    summaries: impl IntoIterator<Item = &'a FixedWorkProviderSummary>,
) -> Result<ComposedFixedWorkDemand, ExternalRootDiagnostic> {
    let mut by_identity = BTreeMap::new();
    for summary in summaries {
        if by_identity.insert(summary.identity, summary).is_some() {
            return Err(ExternalRootDiagnostic(format!(
                "fixed-work summary identity 0x{:016x} is duplicated",
                summary.identity.normalized_identity()
            )));
        }
    }
    let root_summary = by_identity.get(&root).ok_or_else(|| {
        ExternalRootDiagnostic(format!(
            "fixed-work root summary 0x{:016x} is missing",
            root.normalized_identity()
        ))
    })?;
    let mut visiting = BTreeSet::new();
    let mut memo = BTreeMap::new();
    let mut used = BTreeSet::new();
    let units =
        compose_fixed_work_summary(root, &by_identity, &mut visiting, &mut memo, &mut used)?;
    let provider_receipts = used
        .iter()
        .map(|identity| {
            by_identity
                .get(identity)
                .expect("used fixed-work summary exists")
                .validation_receipt
        })
        .collect();
    let composition_fingerprint = fingerprint_fixed_work_composition(&used, &by_identity);
    Ok(ComposedFixedWorkDemand {
        root,
        root_provider: root_summary.provider,
        units,
        summaries: used,
        provider_receipts,
        composition_fingerprint,
    })
}

fn compose_fixed_work_summary(
    identity: ProviderWorkSummaryId,
    summaries: &BTreeMap<ProviderWorkSummaryId, &FixedWorkProviderSummary>,
    visiting: &mut BTreeSet<ProviderWorkSummaryId>,
    memo: &mut BTreeMap<ProviderWorkSummaryId, u64>,
    used: &mut BTreeSet<ProviderWorkSummaryId>,
) -> Result<u64, ExternalRootDiagnostic> {
    if let Some(units) = memo.get(&identity) {
        used.insert(identity);
        return Ok(*units);
    }
    if !visiting.insert(identity) {
        return Err(ExternalRootDiagnostic(format!(
            "fixed-work summary graph contains a cycle through 0x{:016x}",
            identity.normalized_identity()
        )));
    }
    let summary = summaries.get(&identity).ok_or_else(|| {
        ExternalRootDiagnostic(format!(
            "fixed-work callee summary 0x{:016x} is missing",
            identity.normalized_identity()
        ))
    })?;
    let mut units = summary.local_units;
    for call in &summary.calls {
        if call.maximum_invocations == 0 {
            return Err(ExternalRootDiagnostic(format!(
                "fixed-work edge from 0x{:016x} to 0x{:016x} has zero maximum invocations",
                identity.normalized_identity(),
                call.callee.normalized_identity()
            )));
        }
        let callee_units =
            compose_fixed_work_summary(call.callee, summaries, visiting, memo, used)?;
        let edge_units = callee_units
            .checked_mul(call.maximum_invocations)
            .ok_or_else(|| {
                ExternalRootDiagnostic("fixed-work composition multiplication overflowed".into())
            })?;
        units = units.checked_add(edge_units).ok_or_else(|| {
            ExternalRootDiagnostic("fixed-work composition addition overflowed".into())
        })?;
    }
    visiting.remove(&identity);
    memo.insert(identity, units);
    used.insert(identity);
    Ok(units)
}

fn fingerprint_fixed_work_composition(
    used: &BTreeSet<ProviderWorkSummaryId>,
    summaries: &BTreeMap<ProviderWorkSummaryId, &FixedWorkProviderSummary>,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.u64(used.len() as u64);
    for identity in used {
        let summary = summaries
            .get(identity)
            .expect("used fixed-work summary exists");
        hash.u64(summary.identity.normalized_identity());
        hash.u64(summary.provider.normalized_identity());
        hash.u64(summary.local_units);
        hash.u64(summary.validation_receipt.normalized_identity());
        hash.u64(summary.calls.len() as u64);
        for call in &summary.calls {
            hash.u64(call.callee.normalized_identity());
            hash.u64(call.maximum_invocations);
        }
    }
    hash.finish()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralWorkResourceColumn {
    pub profile: StructuralWorkProfileId,
    pub ceiling_units: u64,
    pub realization: ComposedFixedWorkDemand,
    pub validation_receipt: StructuralWorkValidationReceiptId,
}

/// The `StatePlan` itself is the public ceiling. This column retains only the
/// final transitive footprint that refined it and the public validation
/// receipt; instruction-selection/allocation derivations stay private.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineStateResourceColumn {
    pub realization: StateFootprintEvidence,
    pub validation_receipt: StateValidationReceiptId,
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
    pub effects: BTreeSet<RootEffectId>,
    pub trust_receipts: BTreeSet<TrustReceiptId>,
    /// Identity of the artifact-wide relation that names which other roots may
    /// preempt this one. Stack class and maximum depth remain the one copy in
    /// `BoundaryEntryPlan::state`; they are not re-authored here.
    pub nesting_relation: NestingRelationId,
    pub acknowledgement_policy: Option<AcknowledgementPolicyId>,
    pub stack: StackResourceColumn,
    pub structural_work: StructuralWorkResourceColumn,
    pub machine_state: MachineStateResourceColumn,
    pub component_pins: BTreeSet<ComponentVersionPin>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedExternalRoot {
    candidate: ExternalRootCandidate,
    boundary: BoundaryEntryPlan,
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
            validate_provider_exit_realization(&root.boundary, &realization).map_err(|error| {
                ExternalRootDiagnostic(format!(
                    "opaque provider exit claim violates the admitted boundary: {error}"
                ))
            })?;
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
        &self.boundary
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
/// This does not fuse the stack, structural-work, and machine-state algebras.
/// It binds their independently validated results, the selected normalized
/// provider plan, and the executable entry into one provider execution that a
/// root admission may publish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderExecution {
    identity: ProviderExecutionId,
    provider_plan: ProviderPlanId,
    root: ExternalRootId,
    normalized_root_identity: u64,
    provider: RootProviderId,
    entry: EntryStubId,
    boundary_contract_fingerprint: u64,
    stack_artifact_composition_fingerprint: u64,
    stack_demand_fingerprint: u64,
    structural_work_fingerprint: u64,
    machine_state_validation_receipt: StateValidationReceiptId,
    exit_assurance: OpaqueProviderExitAssurance,
    exit_assurance_fingerprint: u64,
    effects: BTreeSet<RootEffectId>,
    normalized_identity: u64,
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
        hash.u64(
            candidate
                .structural_work
                .realization
                .composition_fingerprint(),
        );
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
        Ok(Self {
            identity,
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
            structural_work_fingerprint: candidate
                .structural_work
                .realization
                .composition_fingerprint(),
            machine_state_validation_receipt: candidate.machine_state.validation_receipt,
            exit_assurance,
            exit_assurance_fingerprint,
            effects: candidate.effects.clone(),
            normalized_identity: hash.finish(),
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

    pub const fn exit_assurance(&self) -> OpaqueProviderExitAssurance {
        self.exit_assurance
    }

    pub const fn exit_assurance_fingerprint(&self) -> u64 {
        self.exit_assurance_fingerprint
    }

    fn matches_root(&self, root: &ValidatedExternalRoot) -> bool {
        let candidate = root.candidate();
        self.root == candidate.identity
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
            && self.structural_work_fingerprint
                == candidate
                    .structural_work
                    .realization
                    .composition_fingerprint()
            && self.machine_state_validation_receipt == candidate.machine_state.validation_receipt
            && self.effects == candidate.effects
    }
}

pub fn validate_external_root(
    candidate: ExternalRootCandidate,
    boundary: &ValidatedBoundaryEntryPlan,
) -> Result<ValidatedExternalRoot, ExternalRootDiagnostic> {
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
    if candidate.structural_work.ceiling_units == 0 {
        return Err(ExternalRootDiagnostic(
            "external-root structural-work ceiling must be nonzero".into(),
        ));
    }
    if candidate.structural_work.realization.units() > candidate.structural_work.ceiling_units {
        return Err(ExternalRootDiagnostic(
            "external-root composed structural work exceeds the admitted ceiling".into(),
        ));
    }
    if candidate.structural_work.realization.root_provider() != candidate.provider {
        return Err(ExternalRootDiagnostic(
            "external-root structural-work root provider does not match the selected provider"
                .into(),
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
        boundary: boundary.plan().clone(),
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
    root_identity: u64,
    provider_execution: ProviderExecutionId,
    provider_execution_fingerprint: u64,
    provider_exit_assurance: OpaqueProviderExitAssurance,
    provider_exit_assurance_fingerprint: u64,
    provider_plan: ProviderPlanId,
    installed_code: InstalledCodeId,
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
            root_identity: root.normalized_identity,
            provider_execution: execution.identity,
            provider_execution_fingerprint: execution.normalized_identity,
            provider_exit_assurance: execution.exit_assurance,
            provider_exit_assurance_fingerprint: execution.exit_assurance_fingerprint,
            provider_plan: execution.provider_plan,
            installed_code: installed_code.identity(),
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
    pub boundary_contract_fingerprint: u64,
    pub boundary: BoundaryEntryPlan,
    pub provider: RootProviderId,
    pub effects: BTreeSet<RootEffectId>,
    pub trust_receipts: BTreeSet<TrustReceiptId>,
    pub nesting_relation: NestingRelationId,
    pub acknowledgement_policy: Option<AcknowledgementPolicyId>,
    pub stack: StackResourceColumn,
    pub structural_work: StructuralWorkResourceColumn,
    pub machine_state: MachineStateResourceColumn,
    pub component_pins: BTreeSet<ComponentVersionPin>,
}

/// Linear liveness pin for one installed external root. Borrowing the code is
/// intentional: retirement needs ownership of `InstalledCode`, which cannot
/// be recovered until every root handle has been removed.
#[derive(Debug)]
pub struct InstalledExternalRoot<'code> {
    root: ExternalRootId,
    slot: RootSlotId,
    owner: RootSlotOwnerId,
    installed_code: &'code InstalledCode,
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

impl InterruptEntryReceipt {
    #[allow(clippy::too_many_arguments)]
    pub const fn from_provider(
        identity: InterruptEntryReceiptId,
        root: ExternalRootId,
        slot: RootSlotId,
        installed_code: InstalledCodeId,
        provider_execution: ProviderExecutionId,
        invocation: InterruptInvocationId,
        mask_control: InterruptMaskControlId,
        initial_mask_state: InterruptMaskStateId,
        acknowledgement_policy: Option<AcknowledgementPolicyId>,
        acknowledgement: Option<InterruptAcknowledgementId>,
    ) -> Self {
        Self {
            identity,
            root,
            slot,
            installed_code,
            provider_execution,
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
    identity: InterruptMaskControlId,
    root: ExternalRootId,
    invocation: InterruptInvocationId,
    initial_state: InterruptMaskStateId,
    current_state: InterruptMaskStateId,
    live_guards: Vec<InterruptMaskGuardId>,
    used_guards: BTreeSet<InterruptMaskGuardId>,
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
        let matches = receipt.root == self.root
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
            identity: receipt.guard,
            root: self.root,
            invocation: self.invocation,
            control: self.identity,
            prior_state: receipt.prior_state,
            masked_state: receipt.masked_state,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct InterruptMaskSaveReceipt {
    identity: InterruptMaskTransitionReceiptId,
    root: ExternalRootId,
    invocation: InterruptInvocationId,
    control: InterruptMaskControlId,
    guard: InterruptMaskGuardId,
    prior_state: InterruptMaskStateId,
    masked_state: InterruptMaskStateId,
    prior_state_saved_exactly: bool,
}

impl InterruptMaskSaveReceipt {
    #[allow(clippy::too_many_arguments)]
    pub const fn from_provider(
        identity: InterruptMaskTransitionReceiptId,
        root: ExternalRootId,
        invocation: InterruptInvocationId,
        control: InterruptMaskControlId,
        guard: InterruptMaskGuardId,
        prior_state: InterruptMaskStateId,
        masked_state: InterruptMaskStateId,
        prior_state_saved_exactly: bool,
    ) -> Self {
        Self {
            identity,
            root,
            invocation,
            control,
            guard,
            prior_state,
            masked_state,
            prior_state_saved_exactly,
        }
    }
}

/// Opaque linear guard corresponding to the source `InterruptMaskGuard`.
#[derive(Debug, PartialEq, Eq)]
pub struct InterruptMaskGuard {
    identity: InterruptMaskGuardId,
    root: ExternalRootId,
    invocation: InterruptInvocationId,
    control: InterruptMaskControlId,
    prior_state: InterruptMaskStateId,
    masked_state: InterruptMaskStateId,
}

impl InterruptMaskGuard {
    pub const fn identity(&self) -> InterruptMaskGuardId {
        self.identity
    }

    pub fn restore(
        self,
        control: &mut InterruptMaskControl,
        receipt: InterruptMaskRestoreReceipt,
    ) -> Result<(), Box<InterruptMaskRestoreError>> {
        let top = control.live_guards.last().copied();
        let matches = self.root == control.root
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
    root: ExternalRootId,
    invocation: InterruptInvocationId,
    control: InterruptMaskControlId,
    guard: InterruptMaskGuardId,
    restored_state: InterruptMaskStateId,
    restored_exactly: bool,
}

impl InterruptMaskRestoreReceipt {
    #[allow(clippy::too_many_arguments)]
    pub const fn from_provider(
        identity: InterruptMaskTransitionReceiptId,
        root: ExternalRootId,
        invocation: InterruptInvocationId,
        control: InterruptMaskControlId,
        guard: InterruptMaskGuardId,
        restored_state: InterruptMaskStateId,
        restored_exactly: bool,
    ) -> Self {
        Self {
            identity,
            root,
            invocation,
            control,
            guard,
            restored_state,
            restored_exactly,
        }
    }
}

/// Opaque linear acknowledgement minted only by an admitted entry receipt.
#[derive(Debug, PartialEq, Eq)]
pub struct InterruptAcknowledgement {
    identity: InterruptAcknowledgementId,
    root: ExternalRootId,
    provider_execution: ProviderExecutionId,
    invocation: InterruptInvocationId,
    policy: AcknowledgementPolicyId,
}

impl InterruptAcknowledgement {
    pub const fn identity(&self) -> InterruptAcknowledgementId {
        self.identity
    }

    pub fn complete(
        self,
        receipt: InterruptAcknowledgementReceipt,
    ) -> Result<CompletedInterruptAcknowledgement, Box<InterruptAcknowledgementError>> {
        let matches = receipt.root == self.root
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
    root: ExternalRootId,
    provider_execution: ProviderExecutionId,
    invocation: InterruptInvocationId,
    policy: AcknowledgementPolicyId,
    acknowledgement: InterruptAcknowledgementId,
    source_acknowledged: bool,
}

impl InterruptAcknowledgementReceipt {
    #[allow(clippy::too_many_arguments)]
    pub const fn from_provider(
        identity: InterruptAcknowledgementReceiptId,
        root: ExternalRootId,
        provider_execution: ProviderExecutionId,
        invocation: InterruptInvocationId,
        policy: AcknowledgementPolicyId,
        acknowledgement: InterruptAcknowledgementId,
        source_acknowledged: bool,
    ) -> Self {
        Self {
            identity,
            root,
            provider_execution,
            invocation,
            policy,
            acknowledgement,
            source_acknowledged,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct CompletedInterruptAcknowledgement {
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
            && receipt.root == record.root
            && receipt.slot == record.slot
            && receipt.installed_code == record.installed_code
            && receipt.provider_execution == record.provider_execution
            && acknowledgement_shape_matches
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

        let acknowledgement = receipt
            .acknowledgement
            .map(|identity| InterruptAcknowledgement {
                identity,
                root: record.root,
                provider_execution: record.provider_execution,
                invocation: receipt.invocation,
                policy: record
                    .acknowledgement_policy
                    .expect("validated acknowledgement shape has a policy"),
            });
        Ok(InterruptEntryObligations {
            pending_exit: PendingInterruptExit {
                entry_receipt: receipt.identity,
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
                identity: receipt.mask_control,
                root: record.root,
                invocation: receipt.invocation,
                initial_state: receipt.initial_mask_state,
                current_state: receipt.initial_mask_state,
                live_guards: Vec::new(),
                used_guards: BTreeSet::new(),
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
                completed.root == pending.root
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
        });
        let control_matches = control.root == pending.root
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
        if installed_code
            .selected_entry_target(root.candidate.entry)
            .is_err()
        {
            return reject(
                ExternalRootDiagnostic(
                    "external-root entry is not in the admitted installed artifact".into(),
                ),
                root,
                slot,
                admission,
            );
        }
        if admission.root_identity != root.normalized_identity
            || admission.installed_code != installed_code.identity()
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
            boundary_contract_fingerprint: root.boundary_contract_fingerprint,
            boundary: root.boundary,
            provider: root.candidate.provider,
            effects: root.candidate.effects,
            trust_receipts: root.candidate.trust_receipts,
            nesting_relation: root.candidate.nesting_relation,
            acknowledgement_policy: root.candidate.acknowledgement_policy,
            stack: root.candidate.stack,
            structural_work: root.candidate.structural_work,
            machine_state: root.candidate.machine_state,
            component_pins: root.candidate.component_pins,
        };
        let handle = InstalledExternalRoot {
            root: record.root,
            slot: record.slot,
            owner: record.owner,
            installed_code,
        };
        self.slots.insert(record.slot);
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
    root: ExternalRootId,
    slot: RootSlotId,
    installed_code: InstalledCodeId,
    entry_unreachable: bool,
    executions_quiesced: bool,
}

impl RootRemovalReceipt {
    pub const fn from_provider(
        identity: RootRemovalReceiptId,
        root: ExternalRootId,
        slot: RootSlotId,
        installed_code: InstalledCodeId,
        entry_unreachable: bool,
        executions_quiesced: bool,
    ) -> Self {
        Self {
            identity,
            root,
            slot,
            installed_code,
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

/// One x86 IDT vector's exact installed-root target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct IdtRootBinding {
    pub vector: u8,
    pub root: ExternalRootId,
    pub entry: EntryStubId,
}

/// Linear unpublished destination supplied by the platform provider. The
/// writer mutates these exact bytes; a failed transition returns this value and
/// it remains unpublishable.
#[derive(Debug, PartialEq, Eq)]
pub struct UnpublishedIdtDestination {
    identity: IdtDestinationId,
    bytes: Vec<u8>,
    site: PlacementSite,
    mapped: bool,
    pinned: bool,
    writable: bool,
}

impl UnpublishedIdtDestination {
    pub fn from_provider(
        identity: IdtDestinationId,
        bytes: Vec<u8>,
        site: PlacementSite,
        mapped: bool,
        pinned: bool,
        writable: bool,
    ) -> Self {
        Self {
            identity,
            bytes,
            site,
            mapped,
            pinned,
            writable,
        }
    }

    pub const fn identity(&self) -> IdtDestinationId {
        self.identity
    }
}

/// Sealed proof that one normalized writer has passed the exact installed
/// artifact, unpublished destination, placement, and root-binding gates. Only
/// this proof may enter compiler-generated IDT writer lowering or the final
/// materialization transition; numeric entry addresses remain private to
/// `InstalledCode`.
#[derive(PartialEq, Eq)]
pub struct PreparedIdtWriter {
    identity: IdtWriterPreparationId,
    installed_code: InstalledCodeId,
    artifact: ArtifactId,
    writer_fingerprint: u64,
    placement_fingerprint: u64,
    initial_content_fingerprint: u64,
    root_binding_fingerprint: u64,
    destination: UnpublishedIdtDestination,
    writer: PostHandoffWriterPlan,
    roots: BTreeMap<u8, IdtRootBinding>,
}

impl std::fmt::Debug for PreparedIdtWriter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedIdtWriter")
            .field("identity", &self.identity)
            .field("installed_code", &self.installed_code)
            .field("artifact", &self.artifact)
            .field("destination", &self.destination.identity)
            .field("writer_fingerprint", &self.writer_fingerprint)
            .field("placement_fingerprint", &self.placement_fingerprint)
            .field(
                "initial_content_fingerprint",
                &self.initial_content_fingerprint,
            )
            .field("root_binding_fingerprint", &self.root_binding_fingerprint)
            .field("byte_len", &self.writer.byte_len)
            .field("step_count", &self.writer.steps.len())
            .field("source_slot_count", &self.source_slot_count())
            .finish()
    }
}

/// Address-free fragment retained by compiler lowering. `source_slot` names a
/// provider-private context slot whose value is populated only through the
/// sealed installed-code resolver; it is not a source-visible address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedIdtWriterStep {
    pub container_byte_offset: u64,
    pub container_width_bits: u16,
    pub destination_lsb: u16,
    pub source_lsb: u16,
    pub width: u16,
    pub source_slot: usize,
}

impl PreparedIdtWriter {
    pub const fn identity(&self) -> IdtWriterPreparationId {
        self.identity
    }

    pub const fn installed_code(&self) -> InstalledCodeId {
        self.installed_code
    }

    pub const fn artifact(&self) -> ArtifactId {
        self.artifact
    }

    pub const fn destination(&self) -> IdtDestinationId {
        self.destination.identity
    }

    pub const fn writer_fingerprint(&self) -> u64 {
        self.writer_fingerprint
    }

    pub const fn placement_fingerprint(&self) -> u64 {
        self.placement_fingerprint
    }

    pub const fn initial_content_fingerprint(&self) -> u64 {
        self.initial_content_fingerprint
    }

    pub const fn root_binding_fingerprint(&self) -> u64 {
        self.root_binding_fingerprint
    }

    pub const fn byte_len(&self) -> usize {
        self.writer.byte_len
    }

    pub const fn step_count(&self) -> usize {
        self.writer.steps.len()
    }

    pub const fn little_endian(&self) -> bool {
        matches!(self.writer.byte_order, ByteOrder::LittleEndian)
    }

    pub fn lowering_steps(&self) -> Vec<PreparedIdtWriterStep> {
        let mut sources = Vec::<PostHandoffWriterSource>::new();
        self.writer
            .steps
            .iter()
            .map(|step| {
                let source_slot = sources
                    .iter()
                    .position(|source| *source == step.source)
                    .unwrap_or_else(|| {
                        let slot = sources.len();
                        sources.push(step.source);
                        slot
                    });
                PreparedIdtWriterStep {
                    container_byte_offset: step.write.container_byte_offset,
                    container_width_bits: step.write.container_width_bits,
                    destination_lsb: step.write.destination_lsb,
                    source_lsb: step.write.source_lsb,
                    width: step.write.width,
                    source_slot,
                }
            })
            .collect()
    }

    pub fn source_slot_count(&self) -> usize {
        let mut sources = Vec::<PostHandoffWriterSource>::new();
        for step in &self.writer.steps {
            if !sources.contains(&step.source) {
                sources.push(step.source);
            }
        }
        sources.len()
    }
}

/// Owning seal for one prepared writer whose dense private ABI words have been
/// populated exactly once by the bound installed-code resolver. Numeric words
/// remain inside the opaque executable-installation carrier and are omitted
/// from `Debug` and all public accessors.
#[derive(PartialEq, Eq)]
pub struct PopulatedIdtWriter {
    identity: IdtWriterContextId,
    prepared: PreparedIdtWriter,
    context: ResolvedPostHandoffEntryWriterContext,
}

impl std::fmt::Debug for PopulatedIdtWriter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PopulatedIdtWriter")
            .field("identity", &self.identity)
            .field("preparation", &self.prepared.identity)
            .field("installed_code", &self.prepared.installed_code)
            .field("artifact", &self.prepared.artifact)
            .field("destination", &self.prepared.destination.identity)
            .field(
                "context_fingerprint",
                &format_args!("{:016x}", self.context.fingerprint()),
            )
            .field("packed_byte_len", &self.context.packed_byte_len())
            .field("source_slot_count", &self.context.source_slot_count())
            .finish()
    }
}

impl PopulatedIdtWriter {
    pub const fn identity(&self) -> IdtWriterContextId {
        self.identity
    }

    pub const fn prepared(&self) -> &PreparedIdtWriter {
        &self.prepared
    }

    pub const fn context_fingerprint(&self) -> u64 {
        self.context.fingerprint()
    }

    pub const fn packed_context_byte_len(&self) -> usize {
        self.context.packed_byte_len()
    }

    pub const fn source_slot_count(&self) -> usize {
        self.context.source_slot_count()
    }
}

#[derive(Debug)]
pub struct IdtWriterPreparationError {
    destination: UnpublishedIdtDestination,
    writer: PostHandoffWriterPlan,
    bindings: Vec<IdtRootBinding>,
    diagnostic: ExternalRootDiagnostic,
}

impl IdtWriterPreparationError {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        UnpublishedIdtDestination,
        PostHandoffWriterPlan,
        Vec<IdtRootBinding>,
    ) {
        (self.destination, self.writer, self.bindings)
    }
}

/// Prepare one exact direct-destination writer without resolving an entry to
/// a public number or mutating the destination. The installed artifact checks
/// every symbolic source, while this gate binds the linear unpublished
/// destination and exact root set that the completed IDT must retain.
pub fn prepare_idt_writer(
    installed_code: &InstalledCode,
    destination: UnpublishedIdtDestination,
    writer: PostHandoffWriterPlan,
    bindings: impl IntoIterator<Item = IdtRootBinding>,
) -> Result<PreparedIdtWriter, Box<IdtWriterPreparationError>> {
    let bindings = bindings.into_iter().collect::<Vec<_>>();
    let reject = |diagnostic, destination, writer, bindings| {
        Err(Box::new(IdtWriterPreparationError {
            destination,
            writer,
            bindings,
            diagnostic,
        }))
    };
    let roots = match validate_idt_writer_bindings(&writer, &bindings) {
        Ok(roots) => roots,
        Err(diagnostic) => return reject(diagnostic, destination, writer, bindings),
    };
    if !destination.mapped || !destination.pinned || !destination.writable {
        return reject(
            ExternalRootDiagnostic(
                "IDT writer destination must be mapped, pinned, and writable".into(),
            ),
            destination,
            writer,
            bindings,
        );
    }
    if let Err(error) = installed_code.validate_post_handoff_entry_writer(
        &writer,
        destination.bytes.len(),
        destination.site,
    ) {
        return reject(
            ExternalRootDiagnostic(format!("IDT writer preparation failed: {error}")),
            destination,
            writer,
            bindings,
        );
    }

    let writer_fingerprint = fingerprint_idt_writer(&writer);
    let placement_fingerprint = fingerprint_placement_site(destination.site);
    let initial_content_fingerprint =
        nonzero_fingerprint(fingerprint_bytes(&destination.bytes[..writer.byte_len]));
    let root_binding_fingerprint = fingerprint_idt_bindings(&roots);
    let identity = fingerprint_idt_writer_preparation(
        installed_code.identity(),
        installed_code.artifact(),
        destination.identity,
        writer_fingerprint,
        placement_fingerprint,
        initial_content_fingerprint,
        root_binding_fingerprint,
        writer.byte_len,
        writer.steps.len(),
    );
    Ok(PreparedIdtWriter {
        identity: IdtWriterPreparationId(identity),
        installed_code: installed_code.identity(),
        artifact: installed_code.artifact(),
        writer_fingerprint,
        placement_fingerprint,
        initial_content_fingerprint,
        root_binding_fingerprint,
        destination,
        writer,
        roots,
    })
}

#[derive(Debug)]
pub struct IdtWriterContextError {
    prepared: PreparedIdtWriter,
    diagnostic: ExternalRootDiagnostic,
}

impl IdtWriterContextError {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_prepared(self) -> PreparedIdtWriter {
        self.prepared
    }
}

/// Resolve and seal the packed provider-private words for one exact prepared
/// writer. Failure returns the still-owning preparation unchanged.
pub fn populate_idt_writer_context(
    installed_code: &InstalledCode,
    prepared: PreparedIdtWriter,
) -> Result<PopulatedIdtWriter, Box<IdtWriterContextError>> {
    let reject = |diagnostic, prepared| {
        Err(Box::new(IdtWriterContextError {
            prepared,
            diagnostic,
        }))
    };
    if prepared.installed_code != installed_code.identity()
        || prepared.artifact != installed_code.artifact()
    {
        return reject(
            ExternalRootDiagnostic(
                "IDT writer context population requires the exact prepared installed code and artifact"
                    .into(),
            ),
            prepared,
        );
    }
    let context = match installed_code.populate_post_handoff_entry_writer_context(
        &prepared.writer,
        prepared.destination.bytes.len(),
        prepared.destination.site,
    ) {
        Ok(context) => context,
        Err(error) => {
            return reject(
                ExternalRootDiagnostic(format!("IDT writer context population failed: {error}")),
                prepared,
            );
        }
    };
    if context.source_slot_count() != prepared.source_slot_count()
        || context.packed_byte_len()
            != (prepared.source_slot_count() + 1) * std::mem::size_of::<u64>()
    {
        return reject(
            ExternalRootDiagnostic(
                "IDT writer context population did not produce the exact dense destination-plus-source word set"
                    .into(),
            ),
            prepared,
        );
    }
    let identity = fingerprint_idt_writer_context(prepared.identity, context.fingerprint());
    Ok(PopulatedIdtWriter {
        identity: IdtWriterContextId(identity),
        prepared,
        context,
    })
}

/// Provider certificate over the completed direct-destination writer and its
/// software-fault-free bootstrap checks.
#[derive(Debug, PartialEq, Eq)]
pub struct IdtMaterializationReceipt {
    identity: IdtMaterializationReceiptId,
    installed_code: InstalledCodeId,
    artifact: ArtifactId,
    destination: IdtDestinationId,
    content_fingerprint: u64,
    software_fault_free: bool,
    remains_unpublished: bool,
}

impl IdtMaterializationReceipt {
    #[allow(clippy::too_many_arguments)]
    pub const fn from_provider(
        identity: IdtMaterializationReceiptId,
        installed_code: InstalledCodeId,
        artifact: ArtifactId,
        destination: IdtDestinationId,
        content_fingerprint: u64,
        software_fault_free: bool,
        remains_unpublished: bool,
    ) -> Self {
        Self {
            identity,
            installed_code,
            artifact,
            destination,
            content_fingerprint,
            software_fault_free,
            remains_unpublished,
        }
    }
}

/// Content-bound IDT produced by executing the validated post-handoff writer.
/// Every symbolic entry target has one root binding; constant/data targets
/// cannot masquerade as interrupt entries.
#[derive(Debug, PartialEq, Eq)]
pub struct MaterializedIdt {
    identity: MaterializedIdtId,
    content_fingerprint: u64,
    writer: PostHandoffWriterPlan,
    roots: BTreeMap<u8, IdtRootBinding>,
    destination: UnpublishedIdtDestination,
    materialization_receipt: IdtMaterializationReceiptId,
}

impl MaterializedIdt {
    pub const fn identity(&self) -> MaterializedIdtId {
        self.identity
    }

    pub const fn content_fingerprint(&self) -> u64 {
        self.content_fingerprint
    }

    pub const fn writer(&self) -> &PostHandoffWriterPlan {
        &self.writer
    }

    pub fn roots(&self) -> impl Iterator<Item = &IdtRootBinding> {
        self.roots.values()
    }

    pub const fn destination(&self) -> IdtDestinationId {
        self.destination.identity
    }

    pub const fn materialization_receipt(&self) -> IdtMaterializationReceiptId {
        self.materialization_receipt
    }
}

#[derive(Debug)]
pub struct IdtMaterializationError {
    populated: PopulatedIdtWriter,
    receipt: IdtMaterializationReceipt,
    diagnostic: ExternalRootDiagnostic,
}

impl IdtMaterializationError {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (PopulatedIdtWriter, IdtMaterializationReceipt) {
        (self.populated, self.receipt)
    }
}

pub fn materialize_idt(
    identity: MaterializedIdtId,
    installed_code: &InstalledCode,
    mut populated: PopulatedIdtWriter,
    receipt: IdtMaterializationReceipt,
) -> Result<MaterializedIdt, Box<IdtMaterializationError>> {
    let reject = |diagnostic, populated, receipt| {
        Err(Box::new(IdtMaterializationError {
            populated,
            receipt,
            diagnostic,
        }))
    };
    if populated.prepared.installed_code != installed_code.identity()
        || populated.prepared.artifact != installed_code.artifact()
    {
        return reject(
            ExternalRootDiagnostic(
                "populated IDT writer does not bind the exact installed code and artifact".into(),
            ),
            populated,
            receipt,
        );
    }
    if receipt.installed_code != installed_code.identity()
        || receipt.artifact != installed_code.artifact()
        || receipt.destination != populated.prepared.destination.identity
        || !receipt.software_fault_free
        || !receipt.remains_unpublished
    {
        return reject(
            ExternalRootDiagnostic(
                "IDT materialization receipt does not bind the exact code/destination and software-fault-free unpublished result"
                    .into(),
            ),
            populated,
            receipt,
        );
    }
    let destination_site = populated.prepared.destination.site;
    if let Err(error) = installed_code.execute_populated_post_handoff_entry_writer(
        &populated.context,
        &populated.prepared.writer,
        &mut populated.prepared.destination.bytes,
        destination_site,
    ) {
        return reject(
            ExternalRootDiagnostic(format!("IDT writer execution failed: {error}")),
            populated,
            receipt,
        );
    }
    let content_fingerprint = fingerprint_bytes(
        &populated.prepared.destination.bytes[..populated.prepared.writer.byte_len],
    );
    if content_fingerprint == 0 || receipt.content_fingerprint != content_fingerprint {
        return reject(
            ExternalRootDiagnostic(
                "IDT materialization receipt does not bind the exact completed bytes".into(),
            ),
            populated,
            receipt,
        );
    }
    let PopulatedIdtWriter { prepared, .. } = populated;
    let PreparedIdtWriter {
        destination,
        writer,
        roots,
        ..
    } = prepared;
    Ok(MaterializedIdt {
        identity,
        content_fingerprint,
        writer,
        roots,
        destination,
        materialization_receipt: receipt.identity,
    })
}

fn validate_idt_writer_bindings(
    writer: &PostHandoffWriterPlan,
    bindings: &[IdtRootBinding],
) -> Result<BTreeMap<u8, IdtRootBinding>, ExternalRootDiagnostic> {
    let mut roots = BTreeMap::new();
    let mut bound_entries = BTreeSet::new();
    for binding in bindings {
        if roots.insert(binding.vector, *binding).is_some() {
            return Err(ExternalRootDiagnostic(format!(
                "materialized IDT vector {} is bound more than once",
                binding.vector
            )));
        }
        bound_entries.insert(binding.entry);
    }
    if roots.is_empty() {
        return Err(ExternalRootDiagnostic(
            "materialized IDT must bind at least one installed root".into(),
        ));
    }
    let writer_entries: BTreeSet<_> = writer
        .steps
        .iter()
        .filter_map(|step| match step.write.target {
            RelocationTarget::Entry(entry) => Some(entry),
            RelocationTarget::Data(_) => None,
        })
        .collect();
    if writer_entries != bound_entries {
        return Err(ExternalRootDiagnostic(
            "materialized IDT writer entry targets do not exactly match its installed-root bindings"
                .into(),
        ));
    }
    Ok(roots)
}

fn fingerprint_idt_writer(writer: &PostHandoffWriterPlan) -> u64 {
    let mut hash = Fnv1a::new();
    hash.u64(writer.byte_len as u64);
    hash.u64(match writer.byte_order {
        ByteOrder::LittleEndian => 1,
        ByteOrder::BigEndian => 2,
    });
    let placement = writer.placement;
    match placement.permitted_range() {
        Some(range) => {
            hash.u64(1);
            hash.u64(range.start_inclusive());
            hash.u64(range.end_exclusive());
        }
        None => hash.u64(0),
    }
    hash.u64(placement.alignment());
    hash.u64(placement_phase_identity(placement.phase()));
    hash.u64(
        placement
            .machine_regime()
            .map_or(0, |identity| identity.normalized_identity()),
    );
    hash.u64(
        placement
            .installation_scope()
            .map_or(0, |identity| identity.normalized_identity()),
    );
    hash.u64(writer.steps.len() as u64);
    for step in &writer.steps {
        fingerprint_text(&mut hash, &step.write.field);
        fingerprint_relocation_target(&mut hash, step.write.target);
        hash.u64(step.write.container_byte_offset);
        hash.u64(u64::from(step.write.container_width_bits));
        hash.u64(u64::from(step.write.destination_lsb));
        hash.u64(u64::from(step.write.source_lsb));
        hash.u64(u64::from(step.write.width));
        match step.source {
            PostHandoffWriterSource::Resolved(value) => {
                hash.u64(1);
                hash.u64(value);
            }
            PostHandoffWriterSource::Resolve(target) => {
                hash.u64(2);
                fingerprint_relocation_target(&mut hash, target);
            }
        }
    }
    nonzero_fingerprint(hash.finish())
}

fn fingerprint_placement_site(site: PlacementSite) -> u64 {
    let mut hash = Fnv1a::new();
    hash.u64(site.base_address);
    hash.u64(placement_phase_identity(site.phase));
    hash.u64(
        site.machine_regime
            .map_or(0, |identity| identity.normalized_identity()),
    );
    hash.u64(
        site.installation_scope
            .map_or(0, |identity| identity.normalized_identity()),
    );
    nonzero_fingerprint(hash.finish())
}

fn fingerprint_idt_bindings(roots: &BTreeMap<u8, IdtRootBinding>) -> u64 {
    let mut hash = Fnv1a::new();
    hash.u64(roots.len() as u64);
    for binding in roots.values() {
        hash.u64(u64::from(binding.vector));
        hash.u64(binding.root.normalized_identity());
        hash.u64(binding.entry.normalized_identity());
    }
    nonzero_fingerprint(hash.finish())
}

#[allow(clippy::too_many_arguments)]
fn fingerprint_idt_writer_preparation(
    installed_code: InstalledCodeId,
    artifact: ArtifactId,
    destination: IdtDestinationId,
    writer_fingerprint: u64,
    placement_fingerprint: u64,
    initial_content_fingerprint: u64,
    root_binding_fingerprint: u64,
    byte_len: usize,
    step_count: usize,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.u64(installed_code.normalized_identity());
    hash.u64(artifact.normalized_identity());
    hash.u64(destination.normalized_identity());
    hash.u64(writer_fingerprint);
    hash.u64(placement_fingerprint);
    hash.u64(initial_content_fingerprint);
    hash.u64(root_binding_fingerprint);
    hash.u64(byte_len as u64);
    hash.u64(step_count as u64);
    nonzero_fingerprint(hash.finish())
}

fn fingerprint_idt_writer_context(
    preparation: IdtWriterPreparationId,
    context_fingerprint: u64,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.u64(preparation.normalized_identity());
    hash.u64(context_fingerprint);
    nonzero_fingerprint(hash.finish())
}

fn fingerprint_idt_descriptor(
    destination: IdtDestinationId,
    content_fingerprint: u64,
    packed: &[u8; 10],
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.u64(destination.normalized_identity());
    hash.u64(content_fingerprint);
    hash.u64(fingerprint_bytes(packed));
    nonzero_fingerprint(hash.finish())
}

fn fingerprint_relocation_target(hash: &mut Fnv1a, target: RelocationTarget) {
    match target {
        RelocationTarget::Data(identity) => {
            hash.u64(1);
            hash.u64(identity.normalized_identity());
        }
        RelocationTarget::Entry(identity) => {
            hash.u64(2);
            hash.u64(identity.normalized_identity());
        }
    }
}

fn fingerprint_text(hash: &mut Fnv1a, text: &str) {
    hash.u64(text.len() as u64);
    for byte in text.bytes() {
        hash.u64(u64::from(byte));
    }
}

const fn placement_phase_identity(phase: PlacementPhase) -> u64 {
    match phase {
        PlacementPhase::Build => 1,
        PlacementPhase::Load => 2,
        PlacementPhase::PostHandoff => 3,
    }
}

const fn nonzero_fingerprint(fingerprint: u64) -> u64 {
    if fingerprint == 0 {
        0xcbf2_9ce4_8422_2325
    } else {
        fingerprint
    }
}

/// Linear authority for the provider-owned architectural IDT register.
#[derive(Debug, PartialEq, Eq)]
pub struct IdtControl {
    identity: IdtControlId,
}

impl IdtControl {
    pub const fn from_admitted_provider(identity: IdtControlId) -> Self {
        Self { identity }
    }

    pub const fn identity(&self) -> IdtControlId {
        self.identity
    }
}

/// Closed proof that the exact materialized table's roots are already live in
/// the supplied ledger and that the provider holds the architectural IDT
/// authority. Compiler-generated provider lowering requires this proof before
/// it may introduce the deriver-only `lidt [r10]` operation.
#[derive(PartialEq, Eq)]
pub struct PreparedIdtLoad {
    materialized: MaterializedIdtId,
    destination: IdtDestinationId,
    descriptor: PreparedX86IdtDescriptor,
    content_fingerprint: u64,
    root_ledger_fingerprint: u64,
    roots: BTreeSet<ExternalRootId>,
    control: IdtControlId,
}

/// Provider-private packed x86-64 IDTR operand. The base and bytes have no
/// public accessor; generated invocation may borrow this seal, while reports
/// and target operations retain only its deterministic fingerprint.
#[derive(PartialEq, Eq)]
struct PreparedX86IdtDescriptor {
    packed: [u8; 10],
    fingerprint: u64,
}

impl std::fmt::Debug for PreparedIdtLoad {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedIdtLoad")
            .field("materialized", &self.materialized)
            .field("destination", &self.destination)
            .field(
                "descriptor_fingerprint",
                &format_args!("{:016x}", self.descriptor.fingerprint),
            )
            .field("content_fingerprint", &self.content_fingerprint)
            .field("root_ledger_fingerprint", &self.root_ledger_fingerprint)
            .field("control", &self.control)
            .finish()
    }
}

impl PreparedIdtLoad {
    pub const fn materialized(&self) -> MaterializedIdtId {
        self.materialized
    }

    pub const fn destination(&self) -> IdtDestinationId {
        self.destination
    }

    pub const fn descriptor_fingerprint(&self) -> u64 {
        self.descriptor.fingerprint
    }

    pub const fn descriptor_byte_len(&self) -> usize {
        self.descriptor.packed.len()
    }

    pub const fn content_fingerprint(&self) -> u64 {
        self.content_fingerprint
    }

    pub const fn root_ledger_fingerprint(&self) -> u64 {
        self.root_ledger_fingerprint
    }

    pub const fn control(&self) -> IdtControlId {
        self.control
    }
}

/// Establish the record-before-reachability precondition for one checked IDT
/// load. The returned carrier has no public constructor: a generated provider
/// cannot acquire it from an unrecorded table, the wrong live-root set, or
/// different `IdtControl` authority.
pub fn prepare_idt_load(
    ledger: &InstalledRootLedger,
    materialized: &MaterializedIdt,
    roots: &[InstalledExternalRoot<'_>],
    control: &IdtControl,
) -> Result<PreparedIdtLoad, ExternalRootDiagnostic> {
    let expected_roots = validate_idt_publication_roots(ledger, materialized, roots)?;
    let table_byte_len = materialized.writer.byte_len;
    let limit = table_byte_len
        .checked_sub(1)
        .and_then(|limit| u16::try_from(limit).ok())
        .ok_or_else(|| {
            ExternalRootDiagnostic(
                "x86 IDT descriptor requires a non-empty table no larger than 65536 bytes".into(),
            )
        })?;
    let base = materialized.destination.site.base_address;
    let mut packed = [0_u8; 10];
    packed[..2].copy_from_slice(&limit.to_le_bytes());
    packed[2..].copy_from_slice(&base.to_le_bytes());
    let descriptor_fingerprint = fingerprint_idt_descriptor(
        materialized.destination.identity,
        materialized.content_fingerprint,
        &packed,
    );
    Ok(PreparedIdtLoad {
        materialized: materialized.identity,
        destination: materialized.destination.identity,
        descriptor: PreparedX86IdtDescriptor {
            packed,
            fingerprint: descriptor_fingerprint,
        },
        content_fingerprint: materialized.content_fingerprint,
        root_ledger_fingerprint: ledger.report_fingerprint(),
        roots: expected_roots,
        control: control.identity,
    })
}

/// Provider receipt for the exact record-before-`lidt` publication attempt.
#[derive(Debug, PartialEq, Eq)]
pub struct IdtInstallationReceipt {
    identity: IdtInstallationReceiptId,
    installed: InstalledIdtId,
    materialized: MaterializedIdtId,
    content_fingerprint: u64,
    root_ledger_fingerprint: u64,
    roots: BTreeSet<ExternalRootId>,
    control: IdtControlId,
    published: bool,
}

impl IdtInstallationReceipt {
    pub fn from_provider(
        identity: IdtInstallationReceiptId,
        installed: InstalledIdtId,
        prepared: &PreparedIdtLoad,
        published: bool,
    ) -> Self {
        Self {
            identity,
            installed,
            materialized: prepared.materialized,
            content_fingerprint: prepared.content_fingerprint,
            root_ledger_fingerprint: prepared.root_ledger_fingerprint,
            roots: prepared.roots.clone(),
            control: prepared.control,
            published,
        }
    }
}

/// Published IDT. Owning the root handles keeps every referenced code
/// realization live for as long as hardware can enter through the table.
#[derive(Debug)]
pub struct InstalledIdt<'code> {
    identity: InstalledIdtId,
    materialized: MaterializedIdt,
    roots: Vec<InstalledExternalRoot<'code>>,
    control: IdtControl,
    installation_receipt: IdtInstallationReceiptId,
}

impl InstalledIdt<'_> {
    pub const fn identity(&self) -> InstalledIdtId {
        self.identity
    }

    pub const fn materialized(&self) -> &MaterializedIdt {
        &self.materialized
    }

    pub fn roots(&self) -> impl Iterator<Item = ExternalRootId> + '_ {
        self.roots.iter().map(InstalledExternalRoot::root)
    }

    pub const fn control(&self) -> &IdtControl {
        &self.control
    }

    pub const fn installation_receipt(&self) -> IdtInstallationReceiptId {
        self.installation_receipt
    }
}

#[derive(Debug)]
pub struct IdtInstallError<'code> {
    materialized: MaterializedIdt,
    roots: Vec<InstalledExternalRoot<'code>>,
    control: IdtControl,
    receipt: IdtInstallationReceipt,
    diagnostic: ExternalRootDiagnostic,
}

impl<'code> IdtInstallError<'code> {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        MaterializedIdt,
        Vec<InstalledExternalRoot<'code>>,
        IdtControl,
        IdtInstallationReceipt,
    ) {
        (self.materialized, self.roots, self.control, self.receipt)
    }
}

/// Publish one materialized IDT only after every symbolic target is already an
/// exact record in the supplied ledger. The caller cannot obtain an
/// `InstalledIdt` from an unpublished receipt, a stale ledger snapshot, or a
/// root handle targeting different code.
pub fn install_materialized_idt<'code>(
    ledger: &InstalledRootLedger,
    materialized: MaterializedIdt,
    roots: Vec<InstalledExternalRoot<'code>>,
    control: IdtControl,
    receipt: IdtInstallationReceipt,
) -> Result<InstalledIdt<'code>, Box<IdtInstallError<'code>>> {
    let reject = |diagnostic, materialized, roots, control, receipt| {
        Err(Box::new(IdtInstallError {
            materialized,
            roots,
            control,
            receipt,
            diagnostic,
        }))
    };
    let expected_roots = match validate_idt_publication_roots(ledger, &materialized, &roots) {
        Ok(expected_roots) => expected_roots,
        Err(diagnostic) => {
            return reject(diagnostic, materialized, roots, control, receipt);
        }
    };
    if receipt.materialized != materialized.identity
        || receipt.content_fingerprint != materialized.content_fingerprint
        || receipt.root_ledger_fingerprint != ledger.report_fingerprint()
        || receipt.roots != expected_roots
        || receipt.control != control.identity
        || !receipt.published
    {
        return reject(
            ExternalRootDiagnostic(
                "IDT installation receipt does not prove exact record-before-publish completion"
                    .into(),
            ),
            materialized,
            roots,
            control,
            receipt,
        );
    }
    Ok(InstalledIdt {
        identity: receipt.installed,
        materialized,
        roots,
        control,
        installation_receipt: receipt.identity,
    })
}

fn validate_idt_publication_roots(
    ledger: &InstalledRootLedger,
    materialized: &MaterializedIdt,
    roots: &[InstalledExternalRoot<'_>],
) -> Result<BTreeSet<ExternalRootId>, ExternalRootDiagnostic> {
    let handle_roots: BTreeSet<_> = roots.iter().map(InstalledExternalRoot::root).collect();
    if handle_roots.len() != roots.len() {
        return Err(ExternalRootDiagnostic(
            "IDT installation repeats an installed-root handle".into(),
        ));
    }
    let expected_roots: BTreeSet<_> = materialized
        .roots
        .values()
        .map(|binding| binding.root)
        .collect();
    let recorded = materialized.roots.values().all(|binding| {
        ledger
            .record(binding.root)
            .is_some_and(|record| record.entry == binding.entry)
    });
    if handle_roots != expected_roots || !recorded {
        return Err(ExternalRootDiagnostic(
            "IDT publication requires exact installed handles and ledger records for every entry target"
                .into(),
        ));
    }
    Ok(expected_roots)
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
    hash.u64(candidate.structural_work.profile.normalized_identity());
    hash.u64(candidate.structural_work.ceiling_units);
    hash.u64(
        candidate
            .structural_work
            .realization
            .root()
            .normalized_identity(),
    );
    hash.u64(
        candidate
            .structural_work
            .realization
            .root_provider()
            .normalized_identity(),
    );
    hash.u64(candidate.structural_work.realization.units());
    hash.u64(
        candidate
            .structural_work
            .realization
            .composition_fingerprint(),
    );
    hash.u64(
        candidate
            .structural_work
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

fn fingerprint_bytes(bytes: &[u8]) -> u64 {
    let mut hash = Fnv1a::new();
    hash.u64(bytes.len() as u64);
    for chunk in bytes.chunks(8) {
        let mut word = [0_u8; 8];
        word[..chunk.len()].copy_from_slice(chunk);
        hash.u64(u64::from_le_bytes(word));
    }
    hash.finish()
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

    const fn finish(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_calling_conventions::{
        CallSignature, CallingPolicy, MachineRegime, MachineState, MachineStateSet, Preemption,
        RegisterSet, StatePlan, ValueShape, evaluate_ordinary_boundary_entry_plan,
        validate_boundary_entry_plan,
    };
    use omega_executable_installation::{
        AdmissionReceiptId, Artifact, ArtifactAdmissionEvidence, ArtifactContentId, ArtifactEntry,
        CodePlacementAuthority, CodePlacementId, EntrySetId, FinalBytesId,
        FinalValidationCertificate, FinalValidationId, InstallAuthority, InstallationAudience,
        InstallationReceipt, InstallationScopeId, MachineContractSetId, MachineFootprintId,
        MaterializationReceipt, PlacementPlanId, WxEnforcement, admit_executable,
        install_validated, materialize_and_freeze, validate_final_placement,
    };
    use omega_extents::{
        AddressSpaceId, ExtentDiagnostic, ExtentLineageId, ExtentProvenanceId, ExtentRightId,
        ExtentRights, ExtentRootGrant, MappingEraId,
    };
    use omega_layout_plans::{
        ArtifactInstallationScopeId, ByteOrder, MaterializationWrite, PlacementAddressRange,
        PlacementConstraints, PlacementPhase, PlacementSite, PostHandoffWriterSource,
        PostHandoffWriterStep,
    };

    fn root_id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExternalRootDiagnostic>) -> T {
        constructor(identity).expect("normalized external-root identity")
    }

    fn install_id<T>(
        identity: u64,
        constructor: fn(u64) -> Result<T, omega_executable_installation::InstallationDiagnostic>,
    ) -> T {
        constructor(identity).expect("normalized installation identity")
    }

    fn extent_id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExtentDiagnostic>) -> T {
        constructor(identity).expect("normalized extent identity")
    }

    fn entry_id(identity: u64) -> EntryStubId {
        EntryStubId::from_normalized_identity(identity).expect("normalized entry identity")
    }

    fn constraints() -> PlacementConstraints {
        PlacementConstraints::new(
            Some(PlacementAddressRange::new(0x1000, 0x1_0000).expect("placement range")),
            4096,
            PlacementPhase::PostHandoff,
            None,
            Some(
                ArtifactInstallationScopeId::from_normalized_identity(61)
                    .expect("installation scope"),
            ),
        )
        .expect("placement constraints")
    }

    fn installed_code(artifact_identity: u64, entry: EntryStubId) -> InstalledCode {
        let artifact = Artifact::from_canonical_decode(
            install_id(artifact_identity, ArtifactId::from_normalized_identity),
            install_id(
                artifact_identity + 10,
                ArtifactContentId::from_normalized_identity,
            ),
            omega_target::Architecture::X86_64,
            vec![0; 64],
            install_id(30, MachineContractSetId::from_normalized_identity),
            install_id(31, MachineFootprintId::from_normalized_identity),
            install_id(32, PlacementPlanId::from_normalized_identity),
            constraints(),
            install_id(33, EntrySetId::from_normalized_identity),
            vec![ArtifactEntry::from_canonical_decode(entry, 16)],
        )
        .expect("artifact");
        let admitted = admit_executable(
            &artifact,
            ArtifactAdmissionEvidence::from_validator(
                install_id(40, AdmissionReceiptId::from_normalized_identity),
                artifact.identity(),
                install_id(
                    artifact_identity + 10,
                    ArtifactContentId::from_normalized_identity,
                ),
                install_id(30, MachineContractSetId::from_normalized_identity),
                install_id(31, MachineFootprintId::from_normalized_identity),
                install_id(32, PlacementPlanId::from_normalized_identity),
                constraints(),
                install_id(33, EntrySetId::from_normalized_identity),
                true,
            ),
        )
        .expect("admitted artifact");

        let rights = ExtentRights::from_normalized_identities([extent_id(
            51,
            ExtentRightId::from_normalized_identity,
        )]);
        let extent = ExtentRootGrant::from_admitted_provider(
            extent_id(100, ExtentLineageId::from_normalized_identity),
            extent_id(50, AddressSpaceId::from_normalized_identity),
            rights.clone(),
            extent_id(52, ExtentProvenanceId::from_normalized_identity),
            extent_id(53, MappingEraId::from_normalized_identity),
        )
        .mint(0x1000, 4096)
        .expect("placement extent");
        let placement = CodePlacementAuthority::from_admitted_provider(
            install_id(100, CodePlacementId::from_normalized_identity),
            install_id(61, InstallationScopeId::from_normalized_identity),
            InstallationAudience::FutureFetcher,
            extent_id(50, AddressSpaceId::from_normalized_identity),
            extent_id(52, ExtentProvenanceId::from_normalized_identity),
            rights,
            constraints(),
            PlacementSite {
                base_address: 0x1000,
                phase: PlacementPhase::PostHandoff,
                machine_regime: None,
                installation_scope: Some(
                    ArtifactInstallationScopeId::from_normalized_identity(61)
                        .expect("installation scope"),
                ),
            },
        )
        .claim(extent)
        .expect("placement");
        let frozen = materialize_and_freeze(
            &admitted,
            placement,
            MaterializationReceipt::from_provider(
                artifact.identity(),
                admitted.admission(),
                install_id(100, CodePlacementId::from_normalized_identity),
                install_id(32, PlacementPlanId::from_normalized_identity),
                install_id(170, FinalBytesId::from_normalized_identity),
                install_id(71, MachineFootprintId::from_normalized_identity),
                true,
            ),
        )
        .expect("frozen placement");
        let validated = validate_final_placement(
            frozen,
            &FinalValidationCertificate::from_validator(
                install_id(180, FinalValidationId::from_normalized_identity),
                artifact.identity(),
                admitted.admission(),
                install_id(100, CodePlacementId::from_normalized_identity),
                install_id(170, FinalBytesId::from_normalized_identity),
                install_id(71, MachineFootprintId::from_normalized_identity),
                true,
            ),
        )
        .expect("validated placement");
        install_validated(
            validated,
            InstallAuthority::from_admitted_provider(
                artifact.identity(),
                admitted.admission(),
                install_id(100, CodePlacementId::from_normalized_identity),
                install_id(61, InstallationScopeId::from_normalized_identity),
                InstallationAudience::FutureFetcher,
            ),
            InstallationReceipt::from_provider(
                install_id(300, InstalledCodeId::from_normalized_identity),
                artifact.identity(),
                admitted.admission(),
                install_id(100, CodePlacementId::from_normalized_identity),
                install_id(61, InstallationScopeId::from_normalized_identity),
                install_id(180, FinalValidationId::from_normalized_identity),
                true,
                WxEnforcement::HardwareEnforced,
            ),
        )
        .expect("installed code")
    }

    fn boundary() -> ValidatedBoundaryEntryPlan {
        evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: vec![ValueShape::integer(8, 8)],
                result: None,
            },
        )
        .expect("validated boundary")
    }

    fn fixed_work() -> ComposedFixedWorkDemand {
        let leaf = FixedWorkProviderSummary {
            identity: root_id(31, ProviderWorkSummaryId::from_normalized_identity),
            provider: root_id(12, RootProviderId::from_normalized_identity),
            local_units: 5,
            calls: BTreeSet::new(),
            validation_receipt: root_id(
                41,
                ProviderWorkValidationReceiptId::from_normalized_identity,
            ),
        };
        let root = FixedWorkProviderSummary {
            identity: root_id(30, ProviderWorkSummaryId::from_normalized_identity),
            provider: root_id(2, RootProviderId::from_normalized_identity),
            local_units: 2,
            calls: BTreeSet::from([FixedWorkCall {
                callee: leaf.identity,
                maximum_invocations: 1,
            }]),
            validation_receipt: root_id(
                40,
                ProviderWorkValidationReceiptId::from_normalized_identity,
            ),
        };
        compose_fixed_work(root.identity, [&root, &leaf]).expect("fixed-work composition")
    }

    fn stack_demand(
        root: ExternalRootId,
        provider: RootProviderId,
        relation: NestingRelationId,
        stack: EntryStack,
        local_wcsu_bytes: u64,
    ) -> ComposedStackDemand {
        let summary = ProviderStackSummary {
            root,
            provider,
            stack,
            local_wcsu_bytes,
            wcsu_alignment: 16,
            validation_receipt: root_id(49, StackValidationReceiptId::from_normalized_identity),
        };
        compose_artifact_stacks(
            &StackNestingRelation {
                identity: relation,
                edges: BTreeSet::new(),
            },
            [&summary],
        )
        .expect("stack composition")
        .demand(root)
        .expect("root stack demand")
        .clone()
    }

    fn candidate(entry: EntryStubId) -> ExternalRootCandidate {
        let root = root_id(1, ExternalRootId::from_normalized_identity);
        let provider = root_id(2, RootProviderId::from_normalized_identity);
        let nesting_relation = root_id(6, NestingRelationId::from_normalized_identity);
        ExternalRootCandidate {
            identity: root,
            entry,
            provider,
            provider_plan: root_id(55, ProviderPlanId::from_normalized_identity),
            effects: [root_id(3, RootEffectId::from_normalized_identity)]
                .into_iter()
                .collect(),
            trust_receipts: [root_id(4, TrustReceiptId::from_normalized_identity)]
                .into_iter()
                .collect(),
            nesting_relation,
            acknowledgement_policy: Some(root_id(
                7,
                AcknowledgementPolicyId::from_normalized_identity,
            )),
            stack: StackResourceColumn {
                ceiling_bytes: 8192,
                realization: stack_demand(
                    root,
                    provider,
                    nesting_relation,
                    EntryStack::ProviderSelected,
                    2048,
                ),
                validation_receipt: root_id(50, StackValidationReceiptId::from_normalized_identity),
            },
            structural_work: StructuralWorkResourceColumn {
                profile: root_id(53, StructuralWorkProfileId::from_normalized_identity),
                ceiling_units: 64,
                realization: fixed_work(),
                validation_receipt: root_id(
                    51,
                    StructuralWorkValidationReceiptId::from_normalized_identity,
                ),
            },
            machine_state: MachineStateResourceColumn {
                realization: StateFootprintEvidence::new(
                    RegisterSet::new([MachineRegister::X86Rax]),
                    MachineStateSet::new([MachineState::Flags]),
                ),
                validation_receipt: root_id(52, StateValidationReceiptId::from_normalized_identity),
            },
            component_pins: [ComponentVersionPin {
                contract: root_id(8, ComponentContractId::from_normalized_identity),
                artifact: root_id(9, ComponentArtifactId::from_normalized_identity),
                provider: root_id(10, ComponentProviderId::from_normalized_identity),
                version: root_id(11, ComponentVersionPinId::from_normalized_identity),
            }]
            .into_iter()
            .collect(),
        }
    }

    fn slot() -> RootSlotAuthority {
        RootSlotAuthority::from_admitted_owner(
            root_id(20, RootSlotId::from_normalized_identity),
            root_id(21, RootSlotOwnerId::from_normalized_identity),
        )
    }

    fn provider_execution(root: &ValidatedExternalRoot) -> ProviderExecution {
        ProviderExecution::from_admitted_provider(
            root_id(54, ProviderExecutionId::from_normalized_identity),
            root,
            Some(OpaqueProviderExitAssurance::AcceptedClaim {
                realization: ProviderExitRealization {
                    control: root.boundary().call.entry_control,
                    restored_state: root.boundary().state.restored_state,
                },
                validation_receipt: root_id(4, TrustReceiptId::from_normalized_identity),
            }),
        )
        .expect("admitted provider exit")
    }

    fn interrupt_boundary() -> ValidatedBoundaryEntryPlan {
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: None,
        };
        let ordinary =
            evaluate_ordinary_boundary_entry_plan(CallingPolicy::SystemVAMD64, &signature)
                .expect("ordinary x86 plan");
        let mut call = ordinary.plan().call.clone();
        call.ordinary_clobbers = RegisterSet::new([
            MachineRegister::X86Rax,
            MachineRegister::X86Rcx,
            MachineRegister::X86Rdx,
            MachineRegister::X86Rsi,
            MachineRegister::X86Rdi,
            MachineRegister::X86R8,
            MachineRegister::X86R9,
            MachineRegister::X86R10,
            MachineRegister::X86R11,
        ]);
        call.entry_control = EntryControl::InterruptReturn;
        let interrupted_state = MachineStateSet::new([
            MachineState::GeneralRegisters,
            MachineState::Flags,
            MachineState::InstructionPointer,
            MachineState::StackPointer,
            MachineState::VectorRegisters,
        ]);
        let saved_state = MachineStateSet::new([
            MachineState::GeneralRegisters,
            MachineState::Flags,
            MachineState::InstructionPointer,
            MachineState::StackPointer,
        ]);
        validate_boundary_entry_plan(
            BoundaryEntryPlan {
                call,
                state: StatePlan {
                    initial_regime: MachineRegime::X86Long64,
                    interrupted_state,
                    saved_state,
                    restored_state: saved_state,
                    permitted_transitive_use: MachineStateSet::new([
                        MachineState::GeneralRegisters,
                        MachineState::Flags,
                    ]),
                    stack: EntryStack::Dedicated { class: 1 },
                    preemption: Preemption::Masked,
                },
            },
            &signature,
        )
        .expect("interrupt boundary")
    }

    fn interrupt_candidate(entry: EntryStubId) -> ExternalRootCandidate {
        let mut candidate = candidate(entry);
        candidate.stack.realization = stack_demand(
            candidate.identity,
            candidate.provider,
            candidate.nesting_relation,
            EntryStack::Dedicated { class: 1 },
            2048,
        );
        candidate
    }

    fn interrupt_entry_receipt(
        root: &InstalledExternalRoot<'_>,
        invocation: u64,
        acknowledgement_policy: Option<u64>,
        acknowledgement: Option<u64>,
    ) -> InterruptEntryReceipt {
        InterruptEntryReceipt::from_provider(
            root_id(
                60 + invocation,
                InterruptEntryReceiptId::from_normalized_identity,
            ),
            root.root(),
            root.slot(),
            root.installed_code(),
            root_id(54, ProviderExecutionId::from_normalized_identity),
            root_id(invocation, InterruptInvocationId::from_normalized_identity),
            root_id(
                70 + invocation,
                InterruptMaskControlId::from_normalized_identity,
            ),
            root_id(80, InterruptMaskStateId::from_normalized_identity),
            acknowledgement_policy.map(|identity| {
                root_id(identity, AcknowledgementPolicyId::from_normalized_identity)
            }),
            acknowledgement.map(|identity| {
                root_id(
                    identity,
                    InterruptAcknowledgementId::from_normalized_identity,
                )
            }),
        )
    }

    #[test]
    fn interrupt_entry_mints_exact_linear_obligations_and_requires_settlement() {
        let entry = entry_id(1001);
        let code = installed_code(1, entry);
        let boundary = interrupt_boundary();
        let validated = validate_external_root(interrupt_candidate(entry), &boundary)
            .expect("interrupt root plan");
        let authority = slot();
        let execution = provider_execution(&validated);
        let admission = RootAdmission::from_admitted_provider(
            root_id(22, RootAdmissionId::from_normalized_identity),
            &validated,
            &execution,
            &code,
            &authority,
            validated.candidate().trust_receipts.iter().copied(),
        )
        .expect("root admission");
        let mut ledger = InstalledRootLedger::default();
        let installed = ledger
            .install(&code, validated, authority, admission)
            .expect("installed interrupt root");

        let obligations = ledger
            .begin_interrupt_entry(
                &installed,
                interrupt_entry_receipt(&installed, 90, Some(7), Some(91)),
            )
            .expect("admitted interrupt entry");
        let (pending, mut control, acknowledgement) = obligations.into_parts();
        let initial = control.current_state();
        let control_id = control.identity();
        let masked = root_id(81, InterruptMaskStateId::from_normalized_identity);
        let nested_masked = root_id(82, InterruptMaskStateId::from_normalized_identity);
        let first_guard_id = root_id(92, InterruptMaskGuardId::from_normalized_identity);
        let second_guard_id = root_id(93, InterruptMaskGuardId::from_normalized_identity);
        let first = control
            .save_and_mask(InterruptMaskSaveReceipt::from_provider(
                root_id(
                    94,
                    InterruptMaskTransitionReceiptId::from_normalized_identity,
                ),
                installed.root(),
                root_id(90, InterruptInvocationId::from_normalized_identity),
                control_id,
                first_guard_id,
                initial,
                masked,
                true,
            ))
            .expect("first exact mask save");
        let second = control
            .save_and_mask(InterruptMaskSaveReceipt::from_provider(
                root_id(
                    95,
                    InterruptMaskTransitionReceiptId::from_normalized_identity,
                ),
                installed.root(),
                root_id(90, InterruptInvocationId::from_normalized_identity),
                control_id,
                second_guard_id,
                masked,
                nested_masked,
                true,
            ))
            .expect("nested exact mask save");

        let out_of_order = first
            .restore(
                &mut control,
                InterruptMaskRestoreReceipt::from_provider(
                    root_id(
                        96,
                        InterruptMaskTransitionReceiptId::from_normalized_identity,
                    ),
                    installed.root(),
                    root_id(90, InterruptInvocationId::from_normalized_identity),
                    control_id,
                    first_guard_id,
                    initial,
                    true,
                ),
            )
            .expect_err("nested masks must restore in LIFO order");
        assert!(
            out_of_order
                .diagnostic()
                .0
                .contains("newest exact saved state")
        );
        let (first, _) = out_of_order.into_parts();
        second
            .restore(
                &mut control,
                InterruptMaskRestoreReceipt::from_provider(
                    root_id(
                        97,
                        InterruptMaskTransitionReceiptId::from_normalized_identity,
                    ),
                    installed.root(),
                    root_id(90, InterruptInvocationId::from_normalized_identity),
                    control_id,
                    second_guard_id,
                    masked,
                    true,
                ),
            )
            .expect("nested restore");
        first
            .restore(
                &mut control,
                InterruptMaskRestoreReceipt::from_provider(
                    root_id(
                        98,
                        InterruptMaskTransitionReceiptId::from_normalized_identity,
                    ),
                    installed.root(),
                    root_id(90, InterruptInvocationId::from_normalized_identity),
                    control_id,
                    first_guard_id,
                    initial,
                    true,
                ),
            )
            .expect("outer restore");
        let replayed_guard = control
            .save_and_mask(InterruptMaskSaveReceipt::from_provider(
                root_id(
                    105,
                    InterruptMaskTransitionReceiptId::from_normalized_identity,
                ),
                installed.root(),
                root_id(90, InterruptInvocationId::from_normalized_identity),
                control_id,
                first_guard_id,
                initial,
                masked,
                true,
            ))
            .expect_err("a settled guard identity cannot be minted again");
        assert!(replayed_guard.diagnostic().0.contains("fresh guard"));

        let acknowledgement =
            acknowledgement.expect("policy-bearing interrupt mints acknowledgement");
        let acknowledgement_id = acknowledgement.identity();
        let completed_acknowledgement = acknowledgement
            .complete(InterruptAcknowledgementReceipt::from_provider(
                root_id(
                    99,
                    InterruptAcknowledgementReceiptId::from_normalized_identity,
                ),
                installed.root(),
                execution.identity(),
                root_id(90, InterruptInvocationId::from_normalized_identity),
                root_id(7, AcknowledgementPolicyId::from_normalized_identity),
                acknowledgement_id,
                true,
            ))
            .expect("exact acknowledgement completion");
        let completed = ledger
            .finish_interrupt_entry(pending, control, Some(completed_acknowledgement))
            .expect("settled interrupt exit");
        assert_eq!(completed.root, installed.root());
        assert_eq!(
            completed.entry_receipt,
            root_id(150, InterruptEntryReceiptId::from_normalized_identity)
        );
        assert_eq!(
            completed.acknowledgement_receipt,
            Some(root_id(
                99,
                InterruptAcknowledgementReceiptId::from_normalized_identity
            ))
        );
    }

    #[test]
    fn interrupt_entry_rejects_policy_drift_replay_and_unsettled_exit() {
        let entry = entry_id(1001);
        let code = installed_code(1, entry);
        let boundary = interrupt_boundary();
        let validated = validate_external_root(interrupt_candidate(entry), &boundary)
            .expect("interrupt root plan");
        let authority = slot();
        let execution = provider_execution(&validated);
        let admission = RootAdmission::from_admitted_provider(
            root_id(22, RootAdmissionId::from_normalized_identity),
            &validated,
            &execution,
            &code,
            &authority,
            validated.candidate().trust_receipts.iter().copied(),
        )
        .expect("root admission");
        let mut ledger = InstalledRootLedger::default();
        let installed = ledger
            .install(&code, validated, authority, admission)
            .expect("installed interrupt root");

        let drifted = ledger
            .begin_interrupt_entry(
                &installed,
                interrupt_entry_receipt(&installed, 100, Some(8), Some(101)),
            )
            .expect_err("a different acknowledgement policy cannot mint a token");
        assert!(drifted.diagnostic().0.contains("acknowledgement policy"));

        let obligations = ledger
            .begin_interrupt_entry(
                &installed,
                interrupt_entry_receipt(&installed, 100, Some(7), Some(101)),
            )
            .expect("admitted interrupt entry");
        let replay = ledger
            .begin_interrupt_entry(
                &installed,
                interrupt_entry_receipt(&installed, 100, Some(7), Some(102)),
            )
            .expect_err("an admitted invocation cannot be replayed");
        assert!(replay.diagnostic().0.contains("replays an invocation"));
        let removal = ledger
            .remove(
                installed,
                RootRemovalReceipt::from_provider(
                    root_id(104, RootRemovalReceiptId::from_normalized_identity),
                    root_id(1, ExternalRootId::from_normalized_identity),
                    root_id(20, RootSlotId::from_normalized_identity),
                    code.identity(),
                    true,
                    true,
                ),
            )
            .expect_err("an active interrupt pins root retirement");
        assert!(removal.diagnostic().0.contains("quiescence"));
        let (installed, _) = removal.into_parts();

        let (pending, control, acknowledgement) = obligations.into_parts();
        let unsettled = ledger
            .finish_interrupt_entry(pending, control, None)
            .expect_err("policy-bearing interrupt must return its completed acknowledgement");
        assert!(
            unsettled
                .diagnostic()
                .0
                .contains("completed acknowledgement")
        );
        let (pending, control, _) = unsettled.into_parts();
        let acknowledgement = acknowledgement.expect("minted acknowledgement");
        let acknowledgement_id = acknowledgement.identity();
        let completed = acknowledgement
            .complete(InterruptAcknowledgementReceipt::from_provider(
                root_id(
                    103,
                    InterruptAcknowledgementReceiptId::from_normalized_identity,
                ),
                installed.root(),
                execution.identity(),
                root_id(100, InterruptInvocationId::from_normalized_identity),
                root_id(7, AcknowledgementPolicyId::from_normalized_identity),
                acknowledgement_id,
                true,
            ))
            .expect("exact acknowledgement");
        ledger
            .finish_interrupt_entry(pending, control, Some(completed))
            .expect("settled retry");
        let completed_replay = ledger
            .begin_interrupt_entry(
                &installed,
                interrupt_entry_receipt(&installed, 100, Some(7), Some(104)),
            )
            .expect_err("a completed invocation cannot be replayed");
        assert!(
            completed_replay
                .diagnostic()
                .0
                .contains("replays an invocation")
        );
        ledger
            .remove(
                installed,
                RootRemovalReceipt::from_provider(
                    root_id(105, RootRemovalReceiptId::from_normalized_identity),
                    root_id(1, ExternalRootId::from_normalized_identity),
                    root_id(20, RootSlotId::from_normalized_identity),
                    code.identity(),
                    true,
                    true,
                ),
            )
            .expect("settled interrupt permits exact root retirement");
    }

    #[test]
    fn interrupt_entry_without_acknowledgement_policy_mints_no_acknowledgement() {
        let entry = entry_id(1001);
        let code = installed_code(1, entry);
        let boundary = interrupt_boundary();
        let mut candidate = interrupt_candidate(entry);
        candidate.acknowledgement_policy = None;
        let validated = validate_external_root(candidate, &boundary).expect("exception root plan");
        let authority = slot();
        let execution = provider_execution(&validated);
        let admission = RootAdmission::from_admitted_provider(
            root_id(22, RootAdmissionId::from_normalized_identity),
            &validated,
            &execution,
            &code,
            &authority,
            validated.candidate().trust_receipts.iter().copied(),
        )
        .expect("root admission");
        let mut ledger = InstalledRootLedger::default();
        let installed = ledger
            .install(&code, validated, authority, admission)
            .expect("installed exception root");

        let obligations = ledger
            .begin_interrupt_entry(
                &installed,
                interrupt_entry_receipt(&installed, 110, None, None),
            )
            .expect("entry without an acknowledgement protocol");
        let (pending, control, acknowledgement) = obligations.into_parts();
        assert!(acknowledgement.is_none());
        ledger
            .finish_interrupt_entry(pending, control, None)
            .expect("exception exit with restored mask and no acknowledgement debt");
    }

    #[test]
    fn opaque_provider_exit_admission_fails_closed_and_rejects_plan_drift() {
        let validated =
            validate_external_root(candidate(entry_id(1001)), &boundary()).expect("root plan");
        let identity = root_id(54, ProviderExecutionId::from_normalized_identity);

        let missing = ProviderExecution::from_admitted_provider(identity, &validated, None)
            .expect_err("opaque provider without exit evidence must reject");
        assert!(
            missing
                .0
                .contains("accepted exit claim or adequate hardware isolation")
        );

        let unreported_isolation = ProviderExecution::from_admitted_provider(
            identity,
            &validated,
            Some(OpaqueProviderExitAssurance::HardwareIsolation {
                validation_receipt: root_id(99, TrustReceiptId::from_normalized_identity),
            }),
        )
        .expect_err("unreported isolation cannot serve as adequate evidence");
        assert!(unreported_isolation.0.contains("admitted trust receipts"));

        let wrong_control = ProviderExecution::from_admitted_provider(
            identity,
            &validated,
            Some(OpaqueProviderExitAssurance::AcceptedClaim {
                realization: ProviderExitRealization {
                    control: omega_calling_conventions::EntryControl::InterruptReturn,
                    restored_state: validated.boundary().state.restored_state,
                },
                validation_receipt: root_id(4, TrustReceiptId::from_normalized_identity),
            }),
        )
        .expect_err("provider exit that violates the CallPlan must reject");
        assert!(wrong_control.0.contains("exit control"));

        let wrong_restore = ProviderExecution::from_admitted_provider(
            identity,
            &validated,
            Some(OpaqueProviderExitAssurance::AcceptedClaim {
                realization: ProviderExitRealization {
                    control: validated.boundary().call.entry_control,
                    restored_state: MachineStateSet::new([MachineState::Flags]),
                },
                validation_receipt: root_id(4, TrustReceiptId::from_normalized_identity),
            }),
        )
        .expect_err("provider exit that violates the StatePlan must reject");
        assert!(wrong_restore.0.contains("restored-state set"));

        let isolated = ProviderExecution::from_admitted_provider(
            identity,
            &validated,
            Some(OpaqueProviderExitAssurance::HardwareIsolation {
                validation_receipt: root_id(4, TrustReceiptId::from_normalized_identity),
            }),
        )
        .expect("adequate hardware isolation is the explicit alternative");
        assert!(matches!(
            isolated.exit_assurance(),
            OpaqueProviderExitAssurance::HardwareIsolation { .. }
        ));
    }

    #[test]
    fn installation_records_the_complete_external_root_and_pins_code_liveness() {
        let entry = entry_id(1001);
        let code = installed_code(1, entry);
        let validated = validate_external_root(candidate(entry), &boundary()).expect("root plan");
        let validated_identity = validated.normalized_identity();
        let authority = slot();
        let execution = provider_execution(&validated);
        let admission = RootAdmission::from_admitted_provider(
            root_id(22, RootAdmissionId::from_normalized_identity),
            &validated,
            &execution,
            &code,
            &authority,
            validated.candidate().trust_receipts.iter().copied(),
        )
        .expect("root admission");
        let mut ledger = InstalledRootLedger::default();
        let installed = ledger
            .install(&code, validated, authority, admission)
            .expect("installed external root");

        let record = ledger.record(installed.root()).expect("root record");
        assert_eq!(record.entry, entry);
        assert_eq!(record.normalized_root_identity, validated_identity);
        assert_eq!(record.installed_code, code.identity());
        assert_eq!(record.provider_execution, execution.identity());
        assert_eq!(record.provider_plan, execution.provider_plan());
        assert_eq!(
            record.provider_execution_fingerprint,
            execution.normalized_identity()
        );
        assert_eq!(record.effects.len(), 1);
        assert_eq!(record.trust_receipts.len(), 1);
        assert_eq!(record.stack.realization.composed_wcsu_bytes(), 2048);
        assert_eq!(record.structural_work.realization.units(), 7);
        assert_eq!(
            record.machine_state.realization.registers().as_slice(),
            &[MachineRegister::X86Rax]
        );
        assert_eq!(record.component_pins.len(), 1);
        assert_eq!(
            record.boundary_contract_fingerprint,
            boundary().contract_fingerprint()
        );
        let installed_report_fingerprint = ledger.report_fingerprint();
        assert_ne!(installed_report_fingerprint, 0);

        let root_identity = installed.root();
        let root_slot = installed.slot();
        let receipt = RootRemovalReceipt::from_provider(
            root_id(23, RootRemovalReceiptId::from_normalized_identity),
            root_identity,
            root_slot,
            installed.installed_code(),
            true,
            true,
        );
        let returned = ledger.remove(installed, receipt).expect("root removal");
        assert_eq!(returned.slot(), root_slot);
        assert!(ledger.record(root_identity).is_none());
        assert_ne!(ledger.report_fingerprint(), installed_report_fingerprint);
    }

    #[test]
    fn install_rejects_foreign_entries_and_returns_every_consumed_authority() {
        let admitted_entry = entry_id(1001);
        let code = installed_code(1, admitted_entry);
        let foreign_entry = entry_id(1002);
        let validated =
            validate_external_root(candidate(foreign_entry), &boundary()).expect("root plan");
        let authority = slot();
        let execution = provider_execution(&validated);
        let admission = RootAdmission::from_admitted_provider(
            root_id(22, RootAdmissionId::from_normalized_identity),
            &validated,
            &execution,
            &code,
            &authority,
            validated.candidate().trust_receipts.iter().copied(),
        )
        .expect("root admission");
        let mut ledger = InstalledRootLedger::default();
        let error = ledger
            .install(&code, validated, authority, admission)
            .expect_err("foreign entry must reject");

        assert!(error.diagnostic().0.contains("not in the admitted"));
        let (root, slot, admission) = error.into_parts();
        assert_eq!(root.candidate().entry, foreign_entry);
        assert_eq!(
            slot.slot(),
            root_id(20, RootSlotId::from_normalized_identity)
        );
        assert_eq!(
            admission.identity(),
            root_id(22, RootAdmissionId::from_normalized_identity)
        );
        assert_eq!(ledger.records().count(), 0);
    }

    #[test]
    fn root_admission_rejects_provider_execution_from_another_realization() {
        let first = validate_external_root(candidate(entry_id(1001)), &boundary())
            .expect("first root realization");
        let execution = provider_execution(&first);
        let second = validate_external_root(candidate(entry_id(1002)), &boundary())
            .expect("second root realization");
        let code = installed_code(2, entry_id(1002));
        let authority = slot();
        let error = RootAdmission::from_admitted_provider(
            root_id(22, RootAdmissionId::from_normalized_identity),
            &second,
            &execution,
            &code,
            &authority,
            second.candidate().trust_receipts.iter().copied(),
        )
        .expect_err("provider execution cannot be replayed for changed entry/resources");

        assert!(error.0.contains("exact validated root realization"));
    }

    #[test]
    fn root_admission_rejects_execution_after_selected_plan_drift() {
        let entry = entry_id(1001);
        let first = validate_external_root(candidate(entry), &boundary())
            .expect("first selected provider plan");
        let execution = provider_execution(&first);
        let mut drifted = candidate(entry);
        drifted.provider_plan = root_id(56, ProviderPlanId::from_normalized_identity);
        let second =
            validate_external_root(drifted, &boundary()).expect("second selected provider plan");
        assert_ne!(first.normalized_identity(), second.normalized_identity());

        let code = installed_code(2, entry);
        let authority = slot();
        let error = RootAdmission::from_admitted_provider(
            root_id(22, RootAdmissionId::from_normalized_identity),
            &second,
            &execution,
            &code,
            &authority,
            second.candidate().trust_receipts.iter().copied(),
        )
        .expect_err("provider execution cannot cross selected-plan drift");

        assert!(error.0.contains("exact validated root realization"));
    }

    #[test]
    fn idt_publication_requires_recorded_roots_and_exact_publish_receipt() {
        let entry = entry_id(1001);
        let code = installed_code(1, entry);
        let validated = validate_external_root(candidate(entry), &boundary()).expect("root plan");
        let authority = slot();
        let execution = provider_execution(&validated);
        let admission = RootAdmission::from_admitted_provider(
            root_id(22, RootAdmissionId::from_normalized_identity),
            &validated,
            &execution,
            &code,
            &authority,
            validated.candidate().trust_receipts.iter().copied(),
        )
        .expect("root admission");
        let mut ledger = InstalledRootLedger::default();
        let installed_root = ledger
            .install(&code, validated, authority, admission)
            .expect("recorded root");
        let writer = PostHandoffWriterPlan {
            byte_len: 16,
            byte_order: ByteOrder::LittleEndian,
            placement: constraints(),
            steps: vec![PostHandoffWriterStep {
                write: MaterializationWrite {
                    field: "offset".into(),
                    target: RelocationTarget::Entry(entry),
                    container_byte_offset: 0,
                    container_width_bits: 64,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 64,
                },
                source: PostHandoffWriterSource::Resolve(RelocationTarget::Entry(entry)),
            }],
        };
        let binding = IdtRootBinding {
            vector: 32,
            root: installed_root.root(),
            entry,
        };
        let site = PlacementSite {
            base_address: 0x1000,
            phase: PlacementPhase::PostHandoff,
            machine_regime: None,
            installation_scope: Some(
                ArtifactInstallationScopeId::from_normalized_identity(61)
                    .expect("installation scope"),
            ),
        };
        let destination_id = root_id(199, IdtDestinationId::from_normalized_identity);
        let mut foreign_writer = writer.clone();
        let foreign_entry = entry_id(1002);
        foreign_writer.steps[0].write.target = RelocationTarget::Entry(foreign_entry);
        foreign_writer.steps[0].source =
            PostHandoffWriterSource::Resolve(RelocationTarget::Entry(foreign_entry));
        let foreign_error = prepare_idt_writer(
            &code,
            UnpublishedIdtDestination::from_provider(
                destination_id,
                vec![0_u8; 16],
                site,
                true,
                true,
                true,
            ),
            foreign_writer,
            [IdtRootBinding {
                entry: foreign_entry,
                ..binding
            }],
        )
        .expect_err("foreign entry cannot enter checked writer lowering");
        assert!(
            foreign_error
                .diagnostic()
                .0
                .contains("exact installed artifact")
        );

        let wrong_phase_error = prepare_idt_writer(
            &code,
            UnpublishedIdtDestination::from_provider(
                destination_id,
                vec![0_u8; 16],
                PlacementSite {
                    phase: PlacementPhase::Load,
                    ..site
                },
                true,
                true,
                true,
            ),
            writer.clone(),
            [binding],
        )
        .expect_err("placement phase drift cannot enter checked writer lowering");
        assert!(wrong_phase_error.diagnostic().0.contains("placement phase"));

        let writable_error = prepare_idt_writer(
            &code,
            UnpublishedIdtDestination::from_provider(
                destination_id,
                vec![0_u8; 16],
                site,
                true,
                true,
                false,
            ),
            writer.clone(),
            [binding],
        )
        .expect_err("non-writable destination cannot enter checked writer lowering");
        assert!(
            writable_error
                .diagnostic()
                .0
                .contains("mapped, pinned, and writable")
        );

        let mut expected = vec![0_u8; 16];
        expected[..8].copy_from_slice(&0x1010_u64.to_le_bytes());
        let bad_materialization_receipt = IdtMaterializationReceipt::from_provider(
            root_id(201, IdtMaterializationReceiptId::from_normalized_identity),
            code.identity(),
            code.artifact(),
            destination_id,
            fingerprint_bytes(&expected) ^ 1,
            true,
            true,
        );
        let prepared = prepare_idt_writer(
            &code,
            UnpublishedIdtDestination::from_provider(
                destination_id,
                vec![0_u8; 16],
                site,
                true,
                true,
                true,
            ),
            writer.clone(),
            [binding],
        )
        .expect("exact writer inputs prepare");
        assert_eq!(prepared.installed_code(), code.identity());
        assert_eq!(prepared.artifact(), code.artifact());
        assert_eq!(prepared.destination(), destination_id);
        assert_ne!(prepared.writer_fingerprint(), 0);
        assert_ne!(prepared.placement_fingerprint(), 0);
        assert_ne!(prepared.initial_content_fingerprint(), 0);
        assert_ne!(prepared.root_binding_fingerprint(), 0);
        assert_eq!(prepared.source_slot_count(), 1);
        assert_eq!(
            prepared.lowering_steps(),
            vec![PreparedIdtWriterStep {
                container_byte_offset: 0,
                container_width_bits: 64,
                destination_lsb: 0,
                source_lsb: 0,
                width: 64,
                source_slot: 0,
            }]
        );
        let foreign_code = installed_code(2, entry);
        let context_error = populate_idt_writer_context(&foreign_code, prepared)
            .expect_err("another installed realization cannot populate the private context");
        assert!(context_error.diagnostic().0.contains("exact prepared"));
        let prepared = context_error.into_prepared();
        let populated = populate_idt_writer_context(&code, prepared)
            .expect("exact installed realization populates private context");
        assert_ne!(populated.identity().normalized_identity(), 0);
        assert_ne!(populated.context_fingerprint(), 0);
        assert_eq!(populated.source_slot_count(), 1);
        assert_eq!(populated.packed_context_byte_len(), 16);
        assert!(!format!("{populated:?}").contains("packed_words"));
        let error = materialize_idt(
            root_id(200, MaterializedIdtId::from_normalized_identity),
            &code,
            populated,
            bad_materialization_receipt,
        )
        .expect_err("receipt for different final bytes cannot mint MaterializedIdt");
        assert!(error.diagnostic().0.contains("exact completed bytes"));
        let (retry_populated, _) = error.into_parts();
        assert_eq!(retry_populated.prepared.destination.bytes, expected);
        let materialization_receipt = IdtMaterializationReceipt::from_provider(
            root_id(210, IdtMaterializationReceiptId::from_normalized_identity),
            code.identity(),
            code.artifact(),
            destination_id,
            fingerprint_bytes(&expected),
            true,
            true,
        );
        let materialized = materialize_idt(
            root_id(200, MaterializedIdtId::from_normalized_identity),
            &code,
            retry_populated,
            materialization_receipt,
        )
        .expect("materialized IDT");
        assert_eq!(
            materialized.content_fingerprint(),
            fingerprint_bytes(&expected)
        );
        let roots = vec![installed_root];
        let control = IdtControl::from_admitted_provider(root_id(
            202,
            IdtControlId::from_normalized_identity,
        ));
        let preparation_error = prepare_idt_load(
            &InstalledRootLedger::default(),
            &materialized,
            &roots,
            &control,
        )
        .expect_err("unrecorded roots cannot produce an IDT-load carrier");
        assert!(
            preparation_error
                .0
                .contains("exact installed handles and ledger records")
        );
        let prepared = prepare_idt_load(&ledger, &materialized, &roots, &control)
            .expect("recorded roots permit checked IDT-load preparation");
        assert_eq!(prepared.destination(), destination_id);
        assert_eq!(prepared.content_fingerprint(), fingerprint_bytes(&expected));
        assert_eq!(prepared.control(), control.identity());
        assert_eq!(prepared.descriptor_byte_len(), 10);
        let mut expected_descriptor = [0_u8; 10];
        expected_descriptor[..2]
            .copy_from_slice(&u16::try_from(expected.len() - 1).unwrap().to_le_bytes());
        expected_descriptor[2..].copy_from_slice(&site.base_address.to_le_bytes());
        assert_eq!(prepared.descriptor.packed, expected_descriptor);
        assert_ne!(prepared.descriptor_fingerprint(), 0);
        assert!(!format!("{prepared:?}").contains("packed"));
        let stale = IdtInstallationReceipt::from_provider(
            root_id(203, IdtInstallationReceiptId::from_normalized_identity),
            root_id(204, InstalledIdtId::from_normalized_identity),
            &prepared,
            false,
        );
        let error = install_materialized_idt(&ledger, materialized, roots, control, stale)
            .expect_err("unpublished receipt cannot install IDT");
        assert!(error.diagnostic().0.contains("record-before-publish"));
        let (materialized, roots, control, _) = error.into_parts();
        let prepared = prepare_idt_load(&ledger, &materialized, &roots, &control)
            .expect("recovered inputs permit checked IDT-load preparation");
        let wrong_control_receipt = IdtInstallationReceipt::from_provider(
            root_id(211, IdtInstallationReceiptId::from_normalized_identity),
            root_id(212, InstalledIdtId::from_normalized_identity),
            &prepared,
            true,
        );
        let wrong_control = IdtControl::from_admitted_provider(root_id(
            213,
            IdtControlId::from_normalized_identity,
        ));
        let error = install_materialized_idt(
            &ledger,
            materialized,
            roots,
            wrong_control,
            wrong_control_receipt,
        )
        .expect_err("prepared receipt cannot publish through different IDT authority");
        assert!(error.diagnostic().0.contains("record-before-publish"));
        let (materialized, roots, _, _) = error.into_parts();
        let prepared = prepare_idt_load(&ledger, &materialized, &roots, &control)
            .expect("original IDT authority remains the checked publication authority");
        let receipt = IdtInstallationReceipt::from_provider(
            root_id(205, IdtInstallationReceiptId::from_normalized_identity),
            root_id(206, InstalledIdtId::from_normalized_identity),
            &prepared,
            true,
        );
        let installed = install_materialized_idt(&ledger, materialized, roots, control, receipt)
            .expect("recorded IDT publication");
        assert_eq!(installed.roots().collect::<Vec<_>>(), vec![binding.root]);

        let mismatch_destination = root_id(209, IdtDestinationId::from_normalized_identity);
        let mismatch = prepare_idt_writer(
            &code,
            UnpublishedIdtDestination::from_provider(
                mismatch_destination,
                vec![0_u8; 16],
                site,
                true,
                true,
                true,
            ),
            writer,
            [IdtRootBinding {
                entry: entry_id(1002),
                ..binding
            }],
        )
        .expect_err("writer target cannot bypass root binding preparation");
        assert!(mismatch.diagnostic().0.contains("exactly match"));
    }

    #[test]
    fn removal_requires_both_unreachability_and_execution_quiescence() {
        let entry = entry_id(1001);
        let code = installed_code(1, entry);
        let validated = validate_external_root(candidate(entry), &boundary()).expect("root plan");
        let authority = slot();
        let execution = provider_execution(&validated);
        let admission = RootAdmission::from_admitted_provider(
            root_id(22, RootAdmissionId::from_normalized_identity),
            &validated,
            &execution,
            &code,
            &authority,
            validated.candidate().trust_receipts.iter().copied(),
        )
        .expect("root admission");
        let mut ledger = InstalledRootLedger::default();
        let installed = ledger
            .install(&code, validated, authority, admission)
            .expect("installed external root");
        let receipt = RootRemovalReceipt::from_provider(
            root_id(23, RootRemovalReceiptId::from_normalized_identity),
            installed.root(),
            installed.slot(),
            installed.installed_code(),
            true,
            false,
        );
        let error = ledger
            .remove(installed, receipt)
            .expect_err("live executions prevent slot reuse");
        assert!(error.diagnostic().0.contains("quiescence"));
        assert_eq!(ledger.records().count(), 1);
        let (installed, _) = error.into_parts();
        assert_eq!(installed.installed_code(), code.identity());
    }

    #[test]
    fn independent_resource_columns_are_validated_before_ledger_entry() {
        let invalid_summary = ProviderStackSummary {
            root: root_id(1, ExternalRootId::from_normalized_identity),
            provider: root_id(2, RootProviderId::from_normalized_identity),
            stack: EntryStack::ProviderSelected,
            local_wcsu_bytes: 2048,
            wcsu_alignment: 3,
            validation_receipt: root_id(49, StackValidationReceiptId::from_normalized_identity),
        };
        let error = compose_artifact_stacks(
            &StackNestingRelation {
                identity: root_id(6, NestingRelationId::from_normalized_identity),
                edges: BTreeSet::new(),
            },
            [&invalid_summary],
        )
        .expect_err("bad WCSU alignment");
        assert!(error.0.contains("power of two"));

        let mut over_stack = candidate(entry_id(1001));
        over_stack.stack.ceiling_bytes = 2047;
        let error = validate_external_root(over_stack, &boundary()).expect_err("stack ceiling");
        assert!(error.0.contains("stack ceiling"));

        let mut wrong_root = candidate(entry_id(1001));
        wrong_root.stack.realization = stack_demand(
            root_id(99, ExternalRootId::from_normalized_identity),
            root_id(2, RootProviderId::from_normalized_identity),
            root_id(6, NestingRelationId::from_normalized_identity),
            EntryStack::ProviderSelected,
            2048,
        );
        let error = validate_external_root(wrong_root, &boundary()).expect_err("wrong stack root");
        assert!(error.0.contains("candidate root"));

        let mut over_work = candidate(entry_id(1001));
        over_work.structural_work.ceiling_units = 6;
        let error = validate_external_root(over_work, &boundary()).expect_err("work ceiling");
        assert!(error.0.contains("structural work"));

        let mut wrong_state = candidate(entry_id(1001));
        wrong_state.machine_state.realization = StateFootprintEvidence::new(
            RegisterSet::new([MachineRegister::Aarch64X(0)]),
            MachineStateSet::empty(),
        );
        let error = validate_external_root(wrong_state, &boundary()).expect_err("state ceiling");
        assert!(error.0.contains("machine-state"));

        let mut conflicting = candidate(entry_id(1001));
        conflicting.component_pins.insert(ComponentVersionPin {
            contract: root_id(8, ComponentContractId::from_normalized_identity),
            artifact: root_id(90, ComponentArtifactId::from_normalized_identity),
            provider: root_id(91, ComponentProviderId::from_normalized_identity),
            version: root_id(92, ComponentVersionPinId::from_normalized_identity),
        });
        let error = validate_external_root(conflicting, &boundary())
            .expect_err("one contract cannot pin two component realizations");
        assert!(error.0.contains("more than one realization"));
    }

    #[test]
    fn cathedral_irq_stack_is_maximum_root_plus_current_stack_fault() {
        let timer = root_id(100, ExternalRootId::from_normalized_identity);
        let keyboard = root_id(101, ExternalRootId::from_normalized_identity);
        let fatal_fault = root_id(102, ExternalRootId::from_normalized_identity);
        let double_fault = root_id(103, ExternalRootId::from_normalized_identity);
        let relation_identity = root_id(110, NestingRelationId::from_normalized_identity);
        let irq_provider = root_id(120, RootProviderId::from_normalized_identity);
        let fault_provider = root_id(121, RootProviderId::from_normalized_identity);
        let receipt =
            |identity| root_id(identity, StackValidationReceiptId::from_normalized_identity);
        let timer_summary = ProviderStackSummary {
            root: timer,
            provider: irq_provider,
            stack: EntryStack::Dedicated { class: 4 },
            local_wcsu_bytes: 2048,
            wcsu_alignment: 16,
            validation_receipt: receipt(130),
        };
        let keyboard_summary = ProviderStackSummary {
            root: keyboard,
            provider: irq_provider,
            stack: EntryStack::Dedicated { class: 4 },
            local_wcsu_bytes: 1536,
            wcsu_alignment: 16,
            validation_receipt: receipt(131),
        };
        let fatal_fault_summary = ProviderStackSummary {
            root: fatal_fault,
            provider: fault_provider,
            stack: EntryStack::Interrupted,
            local_wcsu_bytes: 1024,
            wcsu_alignment: 16,
            validation_receipt: receipt(132),
        };
        let double_fault_summary = ProviderStackSummary {
            root: double_fault,
            provider: fault_provider,
            stack: EntryStack::Dedicated { class: 1 },
            local_wcsu_bytes: 4096,
            wcsu_alignment: 64,
            validation_receipt: receipt(133),
        };
        let relation = StackNestingRelation {
            identity: relation_identity,
            edges: BTreeSet::from([
                StackNestingEdge {
                    interrupted: timer,
                    preemptor: fatal_fault,
                },
                StackNestingEdge {
                    interrupted: timer,
                    preemptor: double_fault,
                },
                StackNestingEdge {
                    interrupted: keyboard,
                    preemptor: fatal_fault,
                },
            ]),
        };

        let forward = compose_artifact_stacks(
            &relation,
            [
                &timer_summary,
                &keyboard_summary,
                &fatal_fault_summary,
                &double_fault_summary,
            ],
        )
        .expect("Cathedral stack composition");
        let reverse = compose_artifact_stacks(
            &relation,
            [
                &double_fault_summary,
                &fatal_fault_summary,
                &keyboard_summary,
                &timer_summary,
            ],
        )
        .expect("order-independent Cathedral stack composition");

        assert_eq!(forward, reverse);
        assert_eq!(
            forward
                .demand(timer)
                .expect("timer WCSU")
                .composed_wcsu_bytes(),
            3072
        );
        assert_eq!(
            forward.domain_wcsu_bytes(StackDomain::Dedicated { class: 4 }),
            Some(3072)
        );
        assert_eq!(
            forward.domain_wcsu_bytes(StackDomain::Dedicated { class: 1 }),
            Some(4096)
        );
        assert_eq!(
            forward
                .demand(timer)
                .expect("timer WCSU")
                .contributing_roots(),
            &BTreeSet::from([timer, fatal_fault])
        );

        let nested_maskable = StackNestingRelation {
            identity: relation_identity,
            edges: BTreeSet::from([StackNestingEdge {
                interrupted: timer,
                preemptor: keyboard,
            }]),
        };
        let error = compose_artifact_stacks(&nested_maskable, [&timer_summary, &keyboard_summary])
            .expect_err("shared dedicated IRQ stack cannot be re-entered");
        assert!(error.0.contains("re-enters active dedicated class 4"));

        let missing = compose_artifact_stacks(&relation, [&timer_summary])
            .expect_err("every nesting endpoint needs a provider stack summary");
        assert!(missing.0.contains("missing"));

        let cyclic = StackNestingRelation {
            identity: relation_identity,
            edges: BTreeSet::from([
                StackNestingEdge {
                    interrupted: timer,
                    preemptor: fatal_fault,
                },
                StackNestingEdge {
                    interrupted: fatal_fault,
                    preemptor: timer,
                },
            ]),
        };
        let error = compose_artifact_stacks(&cyclic, [&timer_summary, &fatal_fault_summary])
            .expect_err("recursive nesting is not a finite WCSU");
        assert!(error.0.contains("cycle"));
    }

    #[test]
    fn fixed_work_composition_is_transitive_canonical_and_fails_closed() {
        let leaf_identity = root_id(61, ProviderWorkSummaryId::from_normalized_identity);
        let root_identity = root_id(60, ProviderWorkSummaryId::from_normalized_identity);
        let leaf = FixedWorkProviderSummary {
            identity: leaf_identity,
            provider: root_id(62, RootProviderId::from_normalized_identity),
            local_units: 4,
            calls: BTreeSet::new(),
            validation_receipt: root_id(
                63,
                ProviderWorkValidationReceiptId::from_normalized_identity,
            ),
        };
        let root = FixedWorkProviderSummary {
            identity: root_identity,
            provider: root_id(2, RootProviderId::from_normalized_identity),
            local_units: 3,
            calls: BTreeSet::from([FixedWorkCall {
                callee: leaf_identity,
                maximum_invocations: 2,
            }]),
            validation_receipt: root_id(
                64,
                ProviderWorkValidationReceiptId::from_normalized_identity,
            ),
        };

        let forward = compose_fixed_work(root_identity, [&root, &leaf]).expect("composition");
        let reverse = compose_fixed_work(root_identity, [&leaf, &root]).expect("composition");
        assert_eq!(forward.units(), 11);
        assert_eq!(forward, reverse);
        assert_eq!(forward.summaries().len(), 2);
        assert_eq!(forward.provider_receipts().len(), 2);

        let error = compose_fixed_work(root_identity, [&root]).expect_err("missing callee");
        assert!(error.0.contains("missing"));

        let cyclic_leaf = FixedWorkProviderSummary {
            calls: BTreeSet::from([FixedWorkCall {
                callee: root_identity,
                maximum_invocations: 1,
            }]),
            ..leaf
        };
        let error = compose_fixed_work(root_identity, [&root, &cyclic_leaf])
            .expect_err("cyclic work graph");
        assert!(error.0.contains("cycle"));
    }

    #[test]
    fn cathedral_first_timer_profile_is_five_fixed_one_shot_nodes() {
        // Cathedral's first hard timer root does exactly four provider-facing
        // operations before its deriver-owned return: acknowledge the source,
        // capture the clock, set one preallocated coalescing wake state, and
        // return. Every edge is one-shot; application timer draining remains
        // outside this hard-root graph.
        let root_identity = root_id(100, ProviderWorkSummaryId::from_normalized_identity);
        let acknowledge_identity = root_id(101, ProviderWorkSummaryId::from_normalized_identity);
        let clock_identity = root_id(102, ProviderWorkSummaryId::from_normalized_identity);
        let wake_identity = root_id(103, ProviderWorkSummaryId::from_normalized_identity);
        let return_identity = root_id(104, ProviderWorkSummaryId::from_normalized_identity);

        let leaf = |identity, provider_identity, receipt_identity| FixedWorkProviderSummary {
            identity,
            provider: root_id(provider_identity, RootProviderId::from_normalized_identity),
            local_units: 1,
            calls: BTreeSet::new(),
            validation_receipt: root_id(
                receipt_identity,
                ProviderWorkValidationReceiptId::from_normalized_identity,
            ),
        };
        let acknowledge = leaf(acknowledge_identity, 201, 301);
        let clock = leaf(clock_identity, 202, 302);
        let wake = leaf(wake_identity, 203, 303);
        let return_path = leaf(return_identity, 204, 304);
        let timer = FixedWorkProviderSummary {
            identity: root_identity,
            provider: root_id(200, RootProviderId::from_normalized_identity),
            local_units: 1,
            calls: BTreeSet::from([
                FixedWorkCall {
                    callee: acknowledge_identity,
                    maximum_invocations: 1,
                },
                FixedWorkCall {
                    callee: clock_identity,
                    maximum_invocations: 1,
                },
                FixedWorkCall {
                    callee: wake_identity,
                    maximum_invocations: 1,
                },
                FixedWorkCall {
                    callee: return_identity,
                    maximum_invocations: 1,
                },
            ]),
            validation_receipt: root_id(
                300,
                ProviderWorkValidationReceiptId::from_normalized_identity,
            ),
        };

        let forward = compose_fixed_work(
            root_identity,
            [&timer, &acknowledge, &clock, &wake, &return_path],
        )
        .expect("the first Cathedral timer profile is finite fixed work");
        let reverse = compose_fixed_work(
            root_identity,
            [&return_path, &wake, &clock, &acknowledge, &timer],
        )
        .expect("presentation order cannot change the timer profile");
        assert_eq!(forward, reverse);
        assert_eq!(forward.units(), 5);
        assert_eq!(
            forward.summaries(),
            &BTreeSet::from([
                root_identity,
                acknowledge_identity,
                clock_identity,
                wake_identity,
                return_identity,
            ])
        );
        assert_eq!(forward.provider_receipts().len(), 5);

        let recursive_acknowledge = FixedWorkProviderSummary {
            calls: BTreeSet::from([FixedWorkCall {
                callee: root_identity,
                maximum_invocations: 1,
            }]),
            ..acknowledge.clone()
        };
        let error = compose_fixed_work(
            root_identity,
            [&timer, &recursive_acknowledge, &clock, &wake, &return_path],
        )
        .expect_err("a recursive acknowledgement provider cannot hide behind the timer root");
        assert!(error.0.contains("cycle"));

        let error = compose_fixed_work(root_identity, [&timer, &acknowledge, &clock, &return_path])
            .expect_err("a timer provider cannot omit its wake summary");
        assert!(error.0.contains("missing"));
    }
}
