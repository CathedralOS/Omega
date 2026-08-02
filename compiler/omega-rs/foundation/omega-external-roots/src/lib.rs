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
use omega_executable_installation::{InstalledCode, InstalledCodeContext};
use omega_layout_plans::EntryStubId;

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

/// Identity of the deterministic logical-cost schedule used to denominate a
/// fuel summary or provision. Schedule versioning is independent from portable
/// IR semantics: changing this value changes accounting, not program meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FuelScheduleIdentity(u32);

impl FuelScheduleIdentity {
    pub fn from_schedule_version(version: u32) -> Result<Self, ExternalRootDiagnostic> {
        if version == 0 {
            return Err(ExternalRootDiagnostic(
                "logical-fuel schedule version cannot be zero".into(),
            ));
        }
        Ok(Self(version))
    }

    pub const fn schedule_version(self) -> u32 {
        self.0
    }
}

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

/// Exact canonical inputs retained behind every stack-composition result.
///
/// Compact fingerprints remain useful report keys, but are not admission
/// evidence: two distinct nesting graphs or provider summaries must remain
/// distinguishable even if their compact fingerprints collide.
#[derive(Debug, Clone, PartialEq, Eq)]
struct StackCompositionEvidence {
    relation: StackNestingRelation,
    summaries: BTreeMap<ExternalRootId, ProviderStackSummary>,
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
    composition_evidence: StackCompositionEvidence,
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

    let composition_evidence = StackCompositionEvidence {
        relation: relation.clone(),
        summaries: by_root
            .iter()
            .map(|(root, summary)| (*root, **summary))
            .collect(),
    };
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
                composition_evidence: composition_evidence.clone(),
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

/// One bounded call edge in a fixed-fuel provider summary. Multiplicity is
/// explicit: a set of callees alone cannot distinguish one invocation from a
/// bounded repeated use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FixedFuelCall {
    pub callee: ProviderFuelSummaryId,
    pub maximum_invocations: u64,
}

/// Public fixed-fuel summary supplied by a selected provider. Units are
/// deterministic logical cost under the named schedule, not native
/// instructions, cycles, elapsed time, or WCET. Absence of a summary fails
/// closed; recursive summary graphs are rejected by composition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedFuelProviderSummary {
    pub identity: ProviderFuelSummaryId,
    pub provider: RootProviderId,
    pub schedule: FuelScheduleIdentity,
    pub local_units: u64,
    pub calls: BTreeSet<FixedFuelCall>,
    pub validation_receipt: ProviderFuelValidationReceiptId,
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
    let schedule = root_summary.schedule;
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
        .map(|identity| {
            by_identity
                .get(identity)
                .expect("used fixed-fuel summary exists")
                .validation_receipt
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
    if summary.schedule != schedule {
        return Err(ExternalRootDiagnostic(format!(
            "fixed-fuel summary 0x{:016x} uses schedule version {}, but the root uses version {}",
            identity.normalized_identity(),
            summary.schedule.schedule_version(),
            schedule.schedule_version()
        )));
    }
    let mut units = summary.local_units;
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
    hash.u64(u64::from(schedule.schedule_version()));
    hash.u64(used.len() as u64);
    for identity in used {
        let summary = summaries
            .get(identity)
            .expect("used fixed-fuel summary exists");
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
    pub effective_carry: omega_core::semantics::CarryPolicy,
}

/// Invocation-specific evidence that one runtime subject entered through an
/// accepted source qualification on the installed root's exact requirement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedEntryQualification {
    pub provider_plan: ProviderPlanId,
    pub requirement_identity: String,
    pub parameter_index: usize,
    pub domain: String,
    pub effective_carry: omega_core::semantics::CarryPolicy,
    pub entry_receipt: InterruptEntryReceiptId,
    pub invocation: InterruptInvocationId,
    pub subject: AdmittedEntrySubject,
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
    pub effective_carry: omega_core::semantics::CarryPolicy,
}

/// Concrete result evidence minted only after the provider's exact transition
/// receipt has changed the interrupt-mask state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedResultQualification {
    pub provider_plan: ProviderPlanId,
    pub requirement_identity: String,
    pub domain: String,
    pub effective_carry: omega_core::semantics::CarryPolicy,
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
            boundary: root.boundary,
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
    hash.u64(u64::from(
        candidate.logical_fuel.schedule.schedule_version(),
    ));
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

    const fn finish(self) -> u64 {
        self.0
    }
}

fn fingerprint_carry_policy(hash: &mut Fnv1a, policy: omega_core::semantics::CarryPolicy) {
    use omega_core::semantics::{CarryAddress, CarryCpu, CarryHostThread, CarrySuspension};

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
mod tests {
    use super::*;
    use omega_calling_conventions::{
        CallSignature, CallingPolicy, MachineRegime, MachineState, MachineStateSet, Preemption,
        RegisterSet, StatePlan, ValueShape, evaluate_ordinary_boundary_entry_plan,
        validate_boundary_entry_plan,
    };
    use omega_executable_installation::{
        AdmissionReceiptId, Artifact, ArtifactAdmissionEvidence, ArtifactContentId, ArtifactEntry,
        CodePlacementAuthority, CodePlacementId, EntrySetId, FinalValidationCertificate,
        FinalValidationId, InstallAuthority, InstallationAudience, InstallationReceipt,
        InstallationScopeId, MachineContractSetId, MachineFootprintId, MaterializationReceipt,
        PlacementPlanId, RelocationSetId, WxEnforcement, admit_executable, install_validated,
        materialize_admitted_artifact, materialize_and_freeze, validate_final_placement,
    };
    use omega_extents::{
        AddressSpaceId, ExtentDiagnostic, ExtentLineageId, ExtentProvenanceId, ExtentRightId,
        ExtentRights, ExtentRootGrant, MappingEraId,
    };
    use omega_layout_plans::{
        ArtifactInstallationScopeId, PlacementAddressRange, PlacementConstraints, PlacementPhase,
        PlacementSite,
    };

    fn root_id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExternalRootDiagnostic>) -> T {
        constructor(identity).expect("normalized external-root identity")
    }

    fn fuel_schedule() -> FuelScheduleIdentity {
        FuelScheduleIdentity::from_schedule_version(1).expect("canonical test fuel schedule")
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
        installed_code_with_fill(artifact_identity, entry, 0)
    }

    fn installed_code_with_fill(
        artifact_identity: u64,
        entry: EntryStubId,
        fill: u8,
    ) -> InstalledCode {
        let artifact = Artifact::from_canonical_decode(
            install_id(artifact_identity, ArtifactId::from_normalized_identity),
            install_id(
                artifact_identity + 10,
                ArtifactContentId::from_normalized_identity,
            ),
            omega_target::Architecture::X86_64,
            vec![fill; 64],
            install_id(30, MachineContractSetId::from_normalized_identity),
            install_id(31, MachineFootprintId::from_normalized_identity),
            install_id(32, PlacementPlanId::from_normalized_identity),
            constraints(),
            install_id(33, EntrySetId::from_normalized_identity),
            vec![ArtifactEntry::from_canonical_decode(entry, 16)],
            install_id(34, RelocationSetId::from_normalized_identity),
            Vec::new(),
        )
        .expect("artifact");
        let admitted = admit_executable(
            &artifact,
            ArtifactAdmissionEvidence::from_validator(
                install_id(40, AdmissionReceiptId::from_normalized_identity),
                &artifact,
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
            &extent,
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
        let materialized = materialize_admitted_artifact(&admitted, &placement, |_| None)
            .expect("artifact without relocations materializes");
        let frozen = materialize_and_freeze(
            &admitted,
            placement,
            materialized.clone(),
            MaterializationReceipt::from_materialized(
                &materialized,
                install_id(71, MachineFootprintId::from_normalized_identity),
                true,
            ),
        )
        .expect("frozen placement");
        let certificate = FinalValidationCertificate::from_validator(
            install_id(180, FinalValidationId::from_normalized_identity),
            &frozen,
            true,
        );
        let validated =
            validate_final_placement(frozen, &certificate).expect("validated placement");
        let install_authority = InstallAuthority::from_admitted_provider(&validated);
        let installation_receipt = InstallationReceipt::from_provider(
            install_id(300, InstalledCodeId::from_normalized_identity),
            &validated,
            true,
            WxEnforcement::HardwareEnforced,
        );
        install_validated(validated, install_authority, installation_receipt)
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

    fn fixed_fuel() -> ComposedFuelDemand {
        let leaf = FixedFuelProviderSummary {
            identity: root_id(31, ProviderFuelSummaryId::from_normalized_identity),
            provider: root_id(12, RootProviderId::from_normalized_identity),
            schedule: fuel_schedule(),
            local_units: 5,
            calls: BTreeSet::new(),
            validation_receipt: root_id(
                41,
                ProviderFuelValidationReceiptId::from_normalized_identity,
            ),
        };
        let root = FixedFuelProviderSummary {
            identity: root_id(30, ProviderFuelSummaryId::from_normalized_identity),
            provider: root_id(2, RootProviderId::from_normalized_identity),
            schedule: fuel_schedule(),
            local_units: 2,
            calls: BTreeSet::from([FixedFuelCall {
                callee: leaf.identity,
                maximum_invocations: 1,
            }]),
            validation_receipt: root_id(
                40,
                ProviderFuelValidationReceiptId::from_normalized_identity,
            ),
        };
        compose_fixed_fuel(root.identity, [&root, &leaf]).expect("fixed-fuel composition")
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
            requirement_identity: "TestRoot::entry".into(),
            entry_claims: Vec::new(),
            acknowledgement_parameter_index: None,
            interrupt_mask_guard_claim: None,
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
            logical_fuel: LogicalFuelResourceColumn {
                schedule: fuel_schedule(),
                provision: root_id(53, FuelProvisionId::from_normalized_identity),
                ceiling_units: 64,
                realization: fixed_fuel(),
                validation_receipt: root_id(51, FuelValidationReceiptId::from_normalized_identity),
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
        candidate.requirement_identity = "TimerRoot::tick".into();
        candidate.entry_claims = vec![ExternalRootEntryClaim {
            parameter_index: 0,
            domain: "InterruptAcknowledgement::Pending".into(),
            effective_carry: omega_core::semantics::CarryPolicy::STRICT,
        }];
        candidate.acknowledgement_parameter_index = Some(0);
        candidate.interrupt_mask_guard_claim = Some(ExternalRootResultClaim {
            provider_plan: root_id(56, ProviderPlanId::from_normalized_identity),
            requirement_identity: "InterruptMaskControl::save_and_mask".into(),
            domain: "InterruptMaskGuard::Active".into(),
            effective_carry: omega_core::semantics::CarryPolicy::STRICT,
        });
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
            root,
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
                &control,
                first_guard_id,
                masked,
                true,
            ))
            .expect("first exact mask save");
        assert_eq!(
            first.qualification(),
            &AdmittedResultQualification {
                provider_plan: root_id(56, ProviderPlanId::from_normalized_identity),
                requirement_identity: "InterruptMaskControl::save_and_mask".into(),
                domain: "InterruptMaskGuard::Active".into(),
                effective_carry: omega_core::semantics::CarryPolicy::STRICT,
                transition_receipt: root_id(
                    94,
                    InterruptMaskTransitionReceiptId::from_normalized_identity
                ),
                invocation: root_id(90, InterruptInvocationId::from_normalized_identity),
                subject: AdmittedResultSubject::InterruptMaskGuard(first_guard_id),
            }
        );
        let second = control
            .save_and_mask(InterruptMaskSaveReceipt::from_provider(
                root_id(
                    95,
                    InterruptMaskTransitionReceiptId::from_normalized_identity,
                ),
                &control,
                second_guard_id,
                nested_masked,
                true,
            ))
            .expect("nested exact mask save");

        let out_of_order_receipt = InterruptMaskRestoreReceipt::from_provider(
            root_id(
                96,
                InterruptMaskTransitionReceiptId::from_normalized_identity,
            ),
            &first,
            true,
        );
        let out_of_order = first
            .restore(&mut control, out_of_order_receipt)
            .expect_err("nested masks must restore in LIFO order");
        assert!(
            out_of_order
                .diagnostic()
                .0
                .contains("newest exact saved state")
        );
        let (first, _) = out_of_order.into_parts();
        let second_receipt = InterruptMaskRestoreReceipt::from_provider(
            root_id(
                97,
                InterruptMaskTransitionReceiptId::from_normalized_identity,
            ),
            &second,
            true,
        );
        second
            .restore(&mut control, second_receipt)
            .expect("nested restore");
        let first_receipt = InterruptMaskRestoreReceipt::from_provider(
            root_id(
                98,
                InterruptMaskTransitionReceiptId::from_normalized_identity,
            ),
            &first,
            true,
        );
        first
            .restore(&mut control, first_receipt)
            .expect("outer restore");
        let replayed_guard = control
            .save_and_mask(InterruptMaskSaveReceipt::from_provider(
                root_id(
                    105,
                    InterruptMaskTransitionReceiptId::from_normalized_identity,
                ),
                &control,
                first_guard_id,
                masked,
                true,
            ))
            .expect_err("a settled guard identity cannot be minted again");
        assert!(replayed_guard.diagnostic().0.contains("fresh guard"));

        let acknowledgement =
            acknowledgement.expect("policy-bearing interrupt mints acknowledgement");
        let [pending_qualification] = acknowledgement.qualifications() else {
            panic!("acknowledgement must retain its exact Pending entry qualification");
        };
        assert_eq!(
            pending_qualification.provider_plan,
            root_id(55, ProviderPlanId::from_normalized_identity)
        );
        assert_eq!(
            pending_qualification.requirement_identity,
            "TimerRoot::tick"
        );
        assert_eq!(pending_qualification.parameter_index, 0);
        assert_eq!(
            pending_qualification.domain,
            "InterruptAcknowledgement::Pending"
        );
        assert_eq!(
            pending_qualification.effective_carry,
            omega_core::semantics::CarryPolicy::STRICT
        );
        assert_eq!(
            pending_qualification.entry_receipt,
            root_id(150, InterruptEntryReceiptId::from_normalized_identity)
        );
        assert_eq!(
            pending_qualification.subject,
            AdmittedEntrySubject::InterruptAcknowledgement(root_id(
                91,
                InterruptAcknowledgementId::from_normalized_identity
            ))
        );
        let acknowledgement_receipt = InterruptAcknowledgementReceipt::from_provider(
            root_id(
                99,
                InterruptAcknowledgementReceiptId::from_normalized_identity,
            ),
            &acknowledgement,
            true,
        );
        let completed_acknowledgement = acknowledgement
            .complete(acknowledgement_receipt)
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
        let removal_receipt = RootRemovalReceipt::from_provider(
            root_id(104, RootRemovalReceiptId::from_normalized_identity),
            &installed,
            true,
            true,
        );
        let removal = ledger
            .remove(installed, removal_receipt)
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
        let acknowledgement_receipt = InterruptAcknowledgementReceipt::from_provider(
            root_id(
                103,
                InterruptAcknowledgementReceiptId::from_normalized_identity,
            ),
            &acknowledgement,
            true,
        );
        let completed = acknowledgement
            .complete(acknowledgement_receipt)
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
        let removal_receipt = RootRemovalReceipt::from_provider(
            root_id(105, RootRemovalReceiptId::from_normalized_identity),
            &installed,
            true,
            true,
        );
        ledger
            .remove(installed, removal_receipt)
            .expect("settled interrupt permits exact root retirement");
    }

    #[test]
    fn interrupt_entry_without_acknowledgement_policy_mints_no_acknowledgement() {
        let entry = entry_id(1001);
        let code = installed_code(1, entry);
        let boundary = interrupt_boundary();
        let mut candidate = interrupt_candidate(entry);
        candidate.acknowledgement_policy = None;
        candidate.interrupt_mask_guard_claim = None;
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
        let (pending, mut control, acknowledgement) = obligations.into_parts();
        assert!(acknowledgement.is_none());
        let rejected_mask = control
            .save_and_mask(InterruptMaskSaveReceipt::from_provider(
                root_id(
                    112,
                    InterruptMaskTransitionReceiptId::from_normalized_identity,
                ),
                &control,
                root_id(113, InterruptMaskGuardId::from_normalized_identity),
                root_id(114, InterruptMaskStateId::from_normalized_identity),
                true,
            ))
            .expect_err("a mask transition without a routed result contract must reject");
        assert!(
            rejected_mask
                .diagnostic()
                .0
                .contains("no admitted routed result contract")
        );
        ledger
            .finish_interrupt_entry(pending, control, None)
            .expect("exception exit with restored mask and no acknowledgement debt");
    }

    #[test]
    fn interrupt_entry_receipt_cannot_substitute_colliding_installed_root() {
        let entry = entry_id(1001);
        let first_code = installed_code_with_fill(1, entry, 0x90);
        let second_code = installed_code_with_fill(1, entry, 0xcc);
        let boundary = interrupt_boundary();
        let first_root = validate_external_root(interrupt_candidate(entry), &boundary)
            .expect("first interrupt root");
        let second_root = first_root.clone();
        let first_execution = provider_execution(&first_root);
        let second_execution = provider_execution(&second_root);
        let first_slot = slot();
        let second_slot = slot();
        let first_admission = RootAdmission::from_admitted_provider(
            root_id(22, RootAdmissionId::from_normalized_identity),
            &first_root,
            &first_execution,
            &first_code,
            &first_slot,
            first_root.candidate().trust_receipts.iter().copied(),
        )
        .expect("first admission");
        let second_admission = RootAdmission::from_admitted_provider(
            root_id(22, RootAdmissionId::from_normalized_identity),
            &second_root,
            &second_execution,
            &second_code,
            &second_slot,
            second_root.candidate().trust_receipts.iter().copied(),
        )
        .expect("second admission");

        let mut first_ledger = InstalledRootLedger::default();
        let first_installed = first_ledger
            .install(&first_code, first_root, first_slot, first_admission)
            .expect("first installed interrupt root");
        let mut second_ledger = InstalledRootLedger::default();
        let second_installed = second_ledger
            .install(&second_code, second_root, second_slot, second_admission)
            .expect("second installed interrupt root");
        let substituted_receipt =
            interrupt_entry_receipt(&second_installed, 120, Some(7), Some(121));

        let error = first_ledger
            .begin_interrupt_entry(&first_installed, substituted_receipt)
            .expect_err("entry receipt must bind exact installed-root evidence");
        assert!(error.diagnostic().0.contains("exact installed"));
    }

    #[test]
    fn interrupt_obligation_receipts_retain_exact_invocation_evidence() {
        let entry = entry_id(1001);
        let first_code = installed_code_with_fill(1, entry, 0x90);
        let second_code = installed_code_with_fill(1, entry, 0xcc);
        let boundary = interrupt_boundary();
        let first_root = validate_external_root(interrupt_candidate(entry), &boundary)
            .expect("first interrupt root");
        let second_root = first_root.clone();
        let first_execution = provider_execution(&first_root);
        let second_execution = provider_execution(&second_root);
        let first_slot = slot();
        let second_slot = slot();
        let first_admission = RootAdmission::from_admitted_provider(
            root_id(22, RootAdmissionId::from_normalized_identity),
            &first_root,
            &first_execution,
            &first_code,
            &first_slot,
            first_root.candidate().trust_receipts.iter().copied(),
        )
        .expect("first admission");
        let second_admission = RootAdmission::from_admitted_provider(
            root_id(22, RootAdmissionId::from_normalized_identity),
            &second_root,
            &second_execution,
            &second_code,
            &second_slot,
            second_root.candidate().trust_receipts.iter().copied(),
        )
        .expect("second admission");
        let mut first_ledger = InstalledRootLedger::default();
        let first_installed = first_ledger
            .install(&first_code, first_root, first_slot, first_admission)
            .expect("first installed root");
        let mut second_ledger = InstalledRootLedger::default();
        let second_installed = second_ledger
            .install(&second_code, second_root, second_slot, second_admission)
            .expect("second installed root");

        let first_obligations = first_ledger
            .begin_interrupt_entry(
                &first_installed,
                interrupt_entry_receipt(&first_installed, 130, Some(7), Some(131)),
            )
            .expect("first invocation");
        let second_obligations = second_ledger
            .begin_interrupt_entry(
                &second_installed,
                interrupt_entry_receipt(&second_installed, 130, Some(7), Some(131)),
            )
            .expect("second invocation");
        let (_, mut first_control, first_acknowledgement) = first_obligations.into_parts();
        let (_, second_control, second_acknowledgement) = second_obligations.into_parts();

        let substituted_mask_receipt = InterruptMaskSaveReceipt::from_provider(
            root_id(
                132,
                InterruptMaskTransitionReceiptId::from_normalized_identity,
            ),
            &second_control,
            root_id(133, InterruptMaskGuardId::from_normalized_identity),
            root_id(134, InterruptMaskStateId::from_normalized_identity),
            true,
        );
        let mask_error = first_control
            .save_and_mask(substituted_mask_receipt)
            .expect_err("mask receipt cannot cross exact invocation evidence");
        assert!(mask_error.diagnostic().0.contains("exact control"));

        let first_acknowledgement = first_acknowledgement.expect("first acknowledgement");
        let second_acknowledgement = second_acknowledgement.expect("second acknowledgement");
        let substituted_ack_receipt = InterruptAcknowledgementReceipt::from_provider(
            root_id(
                135,
                InterruptAcknowledgementReceiptId::from_normalized_identity,
            ),
            &second_acknowledgement,
            true,
        );
        let acknowledgement_error = first_acknowledgement
            .complete(substituted_ack_receipt)
            .expect_err("acknowledgement receipt cannot cross exact invocation evidence");
        assert!(
            acknowledgement_error
                .diagnostic()
                .0
                .contains("exact invocation")
        );
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
        assert_eq!(record.requirement_identity, "TestRoot::entry");
        assert!(record.entry_claims.is_empty());
        assert_eq!(record.acknowledgement_parameter_index, None);
        assert!(record.interrupt_mask_guard_claim.is_none());
        assert_eq!(
            record.provider_execution_fingerprint,
            execution.normalized_identity()
        );
        assert_eq!(record.effects.len(), 1);
        assert_eq!(record.trust_receipts.len(), 1);
        assert_eq!(record.stack.realization.composed_wcsu_bytes(), 2048);
        assert_eq!(record.logical_fuel.realization.units(), 7);
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
            &installed,
            true,
            true,
        );
        let returned = ledger.remove(installed, receipt).expect("root removal");
        assert_eq!(returned.slot(), root_slot);
        assert!(ledger.record(root_identity).is_none());
        assert_ne!(ledger.report_fingerprint(), installed_report_fingerprint);
    }

    #[test]
    fn external_root_identity_binds_canonical_entry_claims() {
        let entry = entry_id(1001);
        let boundary = interrupt_boundary();
        let baseline = validate_external_root(interrupt_candidate(entry), &boundary)
            .expect("canonical interrupt entry contract");

        let mut drifted = interrupt_candidate(entry);
        drifted.entry_claims[0].domain = "InterruptAcknowledgement::Forged".into();
        let drifted = validate_external_root(drifted, &boundary)
            .expect("a different admitted domain remains a structurally valid root");
        assert_ne!(
            baseline.normalized_identity(),
            drifted.normalized_identity()
        );

        let mut duplicate = interrupt_candidate(entry);
        duplicate
            .entry_claims
            .push(duplicate.entry_claims[0].clone());
        let duplicate = validate_external_root(duplicate, &boundary)
            .expect_err("duplicate accepted claims must fail closed");
        assert!(duplicate.0.contains("uniquely sorted"));

        let mut missing = interrupt_candidate(entry);
        missing.entry_claims.clear();
        let missing = validate_external_root(missing, &boundary)
            .expect_err("the acknowledgement parameter must name an admitted claim");
        assert!(missing.0.contains("acknowledgement parameter"));
    }

    #[test]
    fn root_admission_cannot_substitute_colliding_installed_code() {
        let entry = entry_id(1001);
        let admitted_code = installed_code_with_fill(1, entry, 0x90);
        let substituted_code = installed_code_with_fill(1, entry, 0xcc);
        assert_eq!(admitted_code.identity(), substituted_code.identity());
        assert_eq!(admitted_code.artifact(), substituted_code.artifact());

        let validated = validate_external_root(candidate(entry), &boundary()).expect("root plan");
        let authority = slot();
        let execution = provider_execution(&validated);
        let admission = RootAdmission::from_admitted_provider(
            root_id(22, RootAdmissionId::from_normalized_identity),
            &validated,
            &execution,
            &admitted_code,
            &authority,
            validated.candidate().trust_receipts.iter().copied(),
        )
        .expect("root admission");

        let error = InstalledRootLedger::default()
            .install(&substituted_code, validated, authority, admission)
            .expect_err("compact installed/artifact IDs cannot substitute exact code");
        assert!(error.diagnostic().0.contains("exact root, code"));
    }

    #[test]
    fn root_removal_receipt_cannot_substitute_colliding_installed_code() {
        let entry = entry_id(1001);
        let first_code = installed_code_with_fill(1, entry, 0x90);
        let second_code = installed_code_with_fill(1, entry, 0xcc);
        let first_root =
            validate_external_root(candidate(entry), &boundary()).expect("first root plan");
        let second_root = first_root.clone();
        let first_execution = provider_execution(&first_root);
        let second_execution = provider_execution(&second_root);
        let first_slot = slot();
        let second_slot = slot();
        let first_admission = RootAdmission::from_admitted_provider(
            root_id(22, RootAdmissionId::from_normalized_identity),
            &first_root,
            &first_execution,
            &first_code,
            &first_slot,
            first_root.candidate().trust_receipts.iter().copied(),
        )
        .expect("first admission");
        let second_admission = RootAdmission::from_admitted_provider(
            root_id(22, RootAdmissionId::from_normalized_identity),
            &second_root,
            &second_execution,
            &second_code,
            &second_slot,
            second_root.candidate().trust_receipts.iter().copied(),
        )
        .expect("second admission");

        let mut first_ledger = InstalledRootLedger::default();
        let first_installed = first_ledger
            .install(&first_code, first_root, first_slot, first_admission)
            .expect("first installed root");
        let mut second_ledger = InstalledRootLedger::default();
        let second_installed = second_ledger
            .install(&second_code, second_root, second_slot, second_admission)
            .expect("second installed root");
        let substituted_receipt = RootRemovalReceipt::from_provider(
            root_id(23, RootRemovalReceiptId::from_normalized_identity),
            &second_installed,
            true,
            true,
        );

        let error = first_ledger
            .remove(first_installed, substituted_receipt)
            .expect_err("root removal must bind exact installed code");
        assert!(error.diagnostic().0.contains("exact-slot"));
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
    fn provider_execution_retains_exact_root_facts_beyond_the_compact_identity() {
        let entry = entry_id(1001);
        let first =
            validate_external_root(candidate(entry), &boundary()).expect("first root realization");
        let execution = provider_execution(&first);
        let mut drifted = candidate(entry);
        drifted
            .trust_receipts
            .insert(root_id(44, TrustReceiptId::from_normalized_identity));
        let mut second =
            validate_external_root(drifted, &boundary()).expect("second root realization");
        second.normalized_identity = first.normalized_identity;

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
        .expect_err("equal compact identity cannot replay execution across exact-root drift");

        assert!(error.0.contains("exact validated root realization"));
    }

    #[test]
    fn slot_admission_retains_the_exact_validated_root() {
        let entry = entry_id(1001);
        let first =
            validate_external_root(candidate(entry), &boundary()).expect("first root realization");
        let code = installed_code(1, entry);
        let authority = slot();
        let execution = provider_execution(&first);
        let admission = RootAdmission::from_admitted_provider(
            root_id(22, RootAdmissionId::from_normalized_identity),
            &first,
            &execution,
            &code,
            &authority,
            first.candidate().trust_receipts.iter().copied(),
        )
        .expect("root admission");

        let mut drifted = candidate(entry);
        drifted.acknowledgement_policy = None;
        let mut second =
            validate_external_root(drifted, &boundary()).expect("second root realization");
        second.normalized_identity = first.normalized_identity;
        let mut ledger = InstalledRootLedger::default();
        let error = ledger
            .install(&code, second, authority, admission)
            .expect_err("equal compact identity cannot replay admission across root-policy drift");

        assert!(
            error
                .diagnostic()
                .0
                .contains("does not bind the exact root")
        );
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
            &installed,
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
        over_work.logical_fuel.ceiling_units = 6;
        let error =
            validate_external_root(over_work, &boundary()).expect_err("logical-fuel ceiling");
        assert!(error.0.contains("logical fuel"));

        let mut wrong_fuel_schedule = candidate(entry_id(1001));
        wrong_fuel_schedule.logical_fuel.schedule =
            FuelScheduleIdentity::from_schedule_version(2).expect("different fuel schedule");
        let error = validate_external_root(wrong_fuel_schedule, &boundary())
            .expect_err("fuel provision cannot reinterpret another schedule's units");
        assert!(error.0.contains("different schedule versions"));

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
    fn stack_composition_retains_exact_inputs_beyond_compact_fingerprints() {
        let root = root_id(140, ExternalRootId::from_normalized_identity);
        let nested = root_id(141, ExternalRootId::from_normalized_identity);
        let relation_identity = root_id(142, NestingRelationId::from_normalized_identity);
        let root_summary = ProviderStackSummary {
            root,
            provider: root_id(143, RootProviderId::from_normalized_identity),
            stack: EntryStack::Dedicated { class: 4 },
            local_wcsu_bytes: 1024,
            wcsu_alignment: 16,
            validation_receipt: root_id(144, StackValidationReceiptId::from_normalized_identity),
        };
        let nested_summary = ProviderStackSummary {
            root: nested,
            provider: root_id(145, RootProviderId::from_normalized_identity),
            stack: EntryStack::Dedicated { class: 1 },
            local_wcsu_bytes: 2048,
            wcsu_alignment: 16,
            validation_receipt: root_id(146, StackValidationReceiptId::from_normalized_identity),
        };
        let without_edge = compose_artifact_stacks(
            &StackNestingRelation {
                identity: relation_identity,
                edges: BTreeSet::new(),
            },
            [&root_summary, &nested_summary],
        )
        .expect("independent roots");
        let with_edge = compose_artifact_stacks(
            &StackNestingRelation {
                identity: relation_identity,
                edges: BTreeSet::from([StackNestingEdge {
                    interrupted: root,
                    preemptor: nested,
                }]),
            },
            [&root_summary, &nested_summary],
        )
        .expect("dedicated nested root");

        let exact = without_edge.demand(root).expect("root demand");
        let mut collided = with_edge.demand(root).expect("root demand").clone();
        collided.artifact_composition_fingerprint = exact.artifact_composition_fingerprint;
        collided.composition_fingerprint = exact.composition_fingerprint;

        assert_eq!(exact.composed_wcsu_bytes, collided.composed_wcsu_bytes);
        assert_eq!(exact.contributing_roots, collided.contributing_roots);
        assert_ne!(
            exact, &collided,
            "compact fingerprint collision cannot erase exact nesting evidence"
        );
    }

    #[test]
    fn fixed_fuel_composition_is_transitive_canonical_and_fails_closed() {
        let error = FuelScheduleIdentity::from_schedule_version(0)
            .expect_err("zero cannot identify a fuel schedule");
        assert!(error.0.contains("cannot be zero"));

        let leaf_identity = root_id(61, ProviderFuelSummaryId::from_normalized_identity);
        let root_identity = root_id(60, ProviderFuelSummaryId::from_normalized_identity);
        let leaf = FixedFuelProviderSummary {
            identity: leaf_identity,
            provider: root_id(62, RootProviderId::from_normalized_identity),
            schedule: fuel_schedule(),
            local_units: 4,
            calls: BTreeSet::new(),
            validation_receipt: root_id(
                63,
                ProviderFuelValidationReceiptId::from_normalized_identity,
            ),
        };
        let root = FixedFuelProviderSummary {
            identity: root_identity,
            provider: root_id(2, RootProviderId::from_normalized_identity),
            schedule: fuel_schedule(),
            local_units: 3,
            calls: BTreeSet::from([FixedFuelCall {
                callee: leaf_identity,
                maximum_invocations: 2,
            }]),
            validation_receipt: root_id(
                64,
                ProviderFuelValidationReceiptId::from_normalized_identity,
            ),
        };

        let forward = compose_fixed_fuel(root_identity, [&root, &leaf]).expect("composition");
        let reverse = compose_fixed_fuel(root_identity, [&leaf, &root]).expect("composition");
        assert_eq!(forward.units(), 11);
        assert_eq!(forward.schedule(), fuel_schedule());
        assert_eq!(forward, reverse);
        assert_eq!(forward.summaries().len(), 2);
        assert_eq!(forward.provider_receipts().len(), 2);

        let error = compose_fixed_fuel(root_identity, [&root]).expect_err("missing callee");
        assert!(error.0.contains("missing"));

        let mismatched_leaf = FixedFuelProviderSummary {
            schedule: FuelScheduleIdentity::from_schedule_version(2)
                .expect("different fuel schedule"),
            ..leaf.clone()
        };
        let error = compose_fixed_fuel(root_identity, [&root, &mismatched_leaf])
            .expect_err("mixed fuel schedules must not compose");
        assert!(error.0.contains("schedule version"));

        let cyclic_leaf = FixedFuelProviderSummary {
            calls: BTreeSet::from([FixedFuelCall {
                callee: root_identity,
                maximum_invocations: 1,
            }]),
            ..leaf
        };
        let error = compose_fixed_fuel(root_identity, [&root, &cyclic_leaf])
            .expect_err("cyclic fuel graph");
        assert!(error.0.contains("cycle"));
    }

    #[test]
    fn fixed_fuel_composition_retains_exact_graph_beyond_compact_fingerprint() {
        let leaf_identity = root_id(71, ProviderFuelSummaryId::from_normalized_identity);
        let root_identity = root_id(70, ProviderFuelSummaryId::from_normalized_identity);
        let leaf = FixedFuelProviderSummary {
            identity: leaf_identity,
            provider: root_id(72, RootProviderId::from_normalized_identity),
            schedule: fuel_schedule(),
            local_units: 4,
            calls: BTreeSet::new(),
            validation_receipt: root_id(
                73,
                ProviderFuelValidationReceiptId::from_normalized_identity,
            ),
        };
        let root = FixedFuelProviderSummary {
            identity: root_identity,
            provider: root_id(74, RootProviderId::from_normalized_identity),
            schedule: fuel_schedule(),
            local_units: 3,
            calls: BTreeSet::from([FixedFuelCall {
                callee: leaf_identity,
                maximum_invocations: 2,
            }]),
            validation_receipt: root_id(
                75,
                ProviderFuelValidationReceiptId::from_normalized_identity,
            ),
        };
        let exact = compose_fixed_fuel(root_identity, [&root, &leaf]).expect("original fuel graph");

        let drifted_leaf = FixedFuelProviderSummary {
            local_units: 2,
            ..leaf
        };
        let drifted_root = FixedFuelProviderSummary {
            calls: BTreeSet::from([FixedFuelCall {
                callee: leaf_identity,
                maximum_invocations: 4,
            }]),
            ..root
        };
        let mut collided = compose_fixed_fuel(root_identity, [&drifted_root, &drifted_leaf])
            .expect("equal-total drifted fuel graph");
        collided.composition_fingerprint = exact.composition_fingerprint;

        assert_eq!(exact.units, collided.units);
        assert_eq!(exact.summaries, collided.summaries);
        assert_eq!(exact.provider_receipts, collided.provider_receipts);
        assert_ne!(
            exact, collided,
            "compact fingerprint collision cannot erase exact fuel-graph evidence"
        );
    }

    #[test]
    fn cathedral_first_timer_profile_is_five_fixed_one_shot_nodes() {
        // Cathedral's first hard timer root does exactly four provider-facing
        // operations before its deriver-owned return: acknowledge the source,
        // capture the clock, set one preallocated coalescing wake state, and
        // return. Every edge is one-shot; application timer draining remains
        // outside this hard-root graph.
        let root_identity = root_id(100, ProviderFuelSummaryId::from_normalized_identity);
        let acknowledge_identity = root_id(101, ProviderFuelSummaryId::from_normalized_identity);
        let clock_identity = root_id(102, ProviderFuelSummaryId::from_normalized_identity);
        let wake_identity = root_id(103, ProviderFuelSummaryId::from_normalized_identity);
        let return_identity = root_id(104, ProviderFuelSummaryId::from_normalized_identity);

        let leaf = |identity, provider_identity, receipt_identity| FixedFuelProviderSummary {
            identity,
            provider: root_id(provider_identity, RootProviderId::from_normalized_identity),
            schedule: fuel_schedule(),
            local_units: 1,
            calls: BTreeSet::new(),
            validation_receipt: root_id(
                receipt_identity,
                ProviderFuelValidationReceiptId::from_normalized_identity,
            ),
        };
        let acknowledge = leaf(acknowledge_identity, 201, 301);
        let clock = leaf(clock_identity, 202, 302);
        let wake = leaf(wake_identity, 203, 303);
        let return_path = leaf(return_identity, 204, 304);
        let timer = FixedFuelProviderSummary {
            identity: root_identity,
            provider: root_id(200, RootProviderId::from_normalized_identity),
            schedule: fuel_schedule(),
            local_units: 1,
            calls: BTreeSet::from([
                FixedFuelCall {
                    callee: acknowledge_identity,
                    maximum_invocations: 1,
                },
                FixedFuelCall {
                    callee: clock_identity,
                    maximum_invocations: 1,
                },
                FixedFuelCall {
                    callee: wake_identity,
                    maximum_invocations: 1,
                },
                FixedFuelCall {
                    callee: return_identity,
                    maximum_invocations: 1,
                },
            ]),
            validation_receipt: root_id(
                300,
                ProviderFuelValidationReceiptId::from_normalized_identity,
            ),
        };

        let forward = compose_fixed_fuel(
            root_identity,
            [&timer, &acknowledge, &clock, &wake, &return_path],
        )
        .expect("the first Cathedral timer profile is finite fixed work");
        let reverse = compose_fixed_fuel(
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

        let recursive_acknowledge = FixedFuelProviderSummary {
            calls: BTreeSet::from([FixedFuelCall {
                callee: root_identity,
                maximum_invocations: 1,
            }]),
            ..acknowledge.clone()
        };
        let error = compose_fixed_fuel(
            root_identity,
            [&timer, &recursive_acknowledge, &clock, &wake, &return_path],
        )
        .expect_err("a recursive acknowledgement provider cannot hide behind the timer root");
        assert!(error.0.contains("cycle"));

        let error = compose_fixed_fuel(root_identity, [&timer, &acknowledge, &clock, &return_path])
            .expect_err("a timer provider cannot omit its wake summary");
        assert!(error.0.contains("missing"));
    }
}
