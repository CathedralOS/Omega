//! Pure composition of validated external-entry stack epochs.
//!
//! This module computes context-, phase-, and domain-aware stack demand. It
//! deliberately does not turn structurally validated plan data into admission
//! evidence: callers must separately bind each realization to sealed target
//! facts, emitted adapter bytes, or an admitted opaque-provider receipt.

use std::collections::{BTreeMap, BTreeSet};

use omega_calling_conventions::{
    ArrivalContextId, ArrivalContextRealization, ArrivalContextStackDomain, EntryStack,
    EntryStackEpoch, EntryStackRealization, EntryStackStage, Preemption, StackDomainRef,
    ValidatedBoundaryEntryPlan, ValidatedEntryStackDomainClosure, ValidatedEntryStackRealization,
    validate_entry_stack_domain_closure, validate_entry_stack_realization,
};
use omega_executable_installation::{
    ArtifactId, InstalledCode, InstalledCodeContext, InstalledCodeId,
};
use psi_layout_plans::EntryStubId;

use super::{
    ExternalRootDiagnostic, ExternalRootId, Fnv1a, ProviderStackSummary, RootProviderId,
    StackDomain, StackLocalEvidence, StackNestingRelation, StackValidationReceiptId,
};

/// Structurally closed input to epoch composition.
///
/// This is not root-admission evidence. `body_wcsu_bytes` and the realization
/// still need provenance before the resulting demand can enter a resource
/// ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpochStackCompositionInput {
    pub root: ExternalRootId,
    pub provider: RootProviderId,
    pub realization: ValidatedEntryStackRealization,
    pub body_wcsu_bytes: u64,
    pub body_wcsu_alignment: u64,
}

/// Exact admitted evidence for an opaque external-entry adapter's epoch plan.
///
/// The receipt attests the complete context/epoch set. Exact installed-code and
/// boundary-plan identities keep that admission from being replayed for a
/// different adapter, entry, target, or public stack contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpaqueAdapterStackRealizationEvidence {
    root: ExternalRootId,
    provider: RootProviderId,
    architecture: omega_target::Architecture,
    installed_code: InstalledCodeId,
    installed_code_context: InstalledCodeContext,
    artifact: ArtifactId,
    entry: EntryStubId,
    boundary_contract_fingerprint: u64,
    body_domains: ValidatedEntryStackDomainClosure,
    realization: ValidatedEntryStackRealization,
    validation_receipt: StackValidationReceiptId,
}

/// Exact compiler-derived evidence for a direct generated entry with no
/// adapter epochs outside the emitted Terminal body itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectGeneratedEntryStackRealizationEvidence {
    root: ExternalRootId,
    provider: RootProviderId,
    architecture: omega_target::Architecture,
    installed_code: InstalledCodeId,
    installed_code_context: InstalledCodeContext,
    artifact: ArtifactId,
    entry: EntryStubId,
    boundary_contract_fingerprint: u64,
    body_domains: ValidatedEntryStackDomainClosure,
    realization: ValidatedEntryStackRealization,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterStackRealizationOrigin {
    DirectGenerated,
    OpaqueProvider,
}

/// Auditable origin of one bound adapter realization. Generated direct entries
/// derive their one body epoch from exact emitted Terminal evidence; opaque
/// adapters require an identity-bound provider receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterStackRealizationEvidence {
    DirectGenerated(DirectGeneratedEntryStackRealizationEvidence),
    OpaqueProvider(OpaqueAdapterStackRealizationEvidence),
}

impl AdapterStackRealizationEvidence {
    pub const fn origin(&self) -> AdapterStackRealizationOrigin {
        match self {
            Self::DirectGenerated(_) => AdapterStackRealizationOrigin::DirectGenerated,
            Self::OpaqueProvider(_) => AdapterStackRealizationOrigin::OpaqueProvider,
        }
    }

    pub const fn root(&self) -> ExternalRootId {
        match self {
            Self::DirectGenerated(evidence) => evidence.root,
            Self::OpaqueProvider(evidence) => evidence.root,
        }
    }

    pub const fn provider(&self) -> RootProviderId {
        match self {
            Self::DirectGenerated(evidence) => evidence.provider,
            Self::OpaqueProvider(evidence) => evidence.provider,
        }
    }

    pub const fn architecture(&self) -> omega_target::Architecture {
        match self {
            Self::DirectGenerated(evidence) => evidence.architecture,
            Self::OpaqueProvider(evidence) => evidence.architecture,
        }
    }

    pub const fn installed_code(&self) -> InstalledCodeId {
        match self {
            Self::DirectGenerated(evidence) => evidence.installed_code,
            Self::OpaqueProvider(evidence) => evidence.installed_code,
        }
    }

    pub const fn artifact(&self) -> ArtifactId {
        match self {
            Self::DirectGenerated(evidence) => evidence.artifact,
            Self::OpaqueProvider(evidence) => evidence.artifact,
        }
    }

    pub const fn entry(&self) -> EntryStubId {
        match self {
            Self::DirectGenerated(evidence) => evidence.entry,
            Self::OpaqueProvider(evidence) => evidence.entry,
        }
    }

    pub const fn boundary_contract_fingerprint(&self) -> u64 {
        match self {
            Self::DirectGenerated(evidence) => evidence.boundary_contract_fingerprint,
            Self::OpaqueProvider(evidence) => evidence.boundary_contract_fingerprint,
        }
    }

    /// Exact body stack domain in every admissible arrival context. This is
    /// derived from the retained realization rather than collapsed to one
    /// artifact-wide stack choice.
    pub fn body_domains(&self) -> Vec<(ArrivalContextId, StackDomainRef)> {
        let closure = match self {
            Self::DirectGenerated(evidence) => &evidence.body_domains,
            Self::OpaqueProvider(evidence) => &evidence.body_domains,
        };
        closure
            .contexts()
            .iter()
            .map(|context| (context.context, context.domain))
            .collect()
    }

    pub const fn realization(&self) -> &ValidatedEntryStackRealization {
        match self {
            Self::DirectGenerated(evidence) => &evidence.realization,
            Self::OpaqueProvider(evidence) => &evidence.realization,
        }
    }

    pub const fn validation_receipt(&self) -> Option<StackValidationReceiptId> {
        match self {
            Self::DirectGenerated(_) => None,
            Self::OpaqueProvider(evidence) => Some(evidence.validation_receipt),
        }
    }

    pub(super) fn matches_installed_code_entry(
        &self,
        installed_code: &InstalledCode,
        entry: EntryStubId,
    ) -> bool {
        match self {
            Self::DirectGenerated(evidence) => {
                evidence.architecture == installed_code.architecture()
                    && evidence.installed_code == installed_code.identity()
                    && evidence.installed_code_context == installed_code.receipt_context()
                    && evidence.artifact == installed_code.artifact()
                    && evidence.entry == entry
            }
            Self::OpaqueProvider(evidence) => {
                evidence.matches_installed_code_entry(installed_code, entry)
            }
        }
    }
}

impl OpaqueAdapterStackRealizationEvidence {
    pub const fn root(&self) -> ExternalRootId {
        self.root
    }

    pub const fn provider(&self) -> RootProviderId {
        self.provider
    }

    pub const fn architecture(&self) -> omega_target::Architecture {
        self.architecture
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

    pub const fn realization(&self) -> &ValidatedEntryStackRealization {
        &self.realization
    }

    pub const fn validation_receipt(&self) -> StackValidationReceiptId {
        self.validation_receipt
    }

    pub const fn boundary_contract_fingerprint(&self) -> u64 {
        self.boundary_contract_fingerprint
    }

    pub(super) fn matches_installed_code_entry(
        &self,
        installed_code: &InstalledCode,
        entry: EntryStubId,
    ) -> bool {
        self.architecture == installed_code.architecture()
            && self.installed_code == installed_code.identity()
            && self.installed_code_context == installed_code.receipt_context()
            && self.artifact == installed_code.artifact()
            && self.entry == entry
    }

    fn matches_installed_entry(
        &self,
        installed_code: &InstalledCode,
        entry: EntryStubId,
        boundary: &ValidatedBoundaryEntryPlan,
    ) -> bool {
        self.matches_installed_code_entry(installed_code, entry)
            && self.boundary_contract_fingerprint == boundary.contract_fingerprint()
    }
}

/// Epoch input whose body demand and adapter realization are both bound to the
/// same exact installed root. Private fields prevent structurally valid but
/// unaudited epoch data from entering admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundEpochStackCompositionInput {
    pure: EpochStackCompositionInput,
    body_evidence: StackLocalEvidence,
    realization_evidence: AdapterStackRealizationEvidence,
}

impl BoundEpochStackCompositionInput {
    pub const fn root(&self) -> ExternalRootId {
        self.pure.root
    }

    pub const fn provider(&self) -> RootProviderId {
        self.pure.provider
    }

    pub const fn pure(&self) -> &EpochStackCompositionInput {
        &self.pure
    }

    pub const fn body_evidence(&self) -> &StackLocalEvidence {
        &self.body_evidence
    }

    pub const fn realization_evidence(&self) -> &AdapterStackRealizationEvidence {
        &self.realization_evidence
    }
}

/// Bind an admitted opaque adapter realization to one exact provider summary,
/// installed entry, and public boundary plan.
pub fn bind_opaque_adapter_stack_realization(
    summary: &ProviderStackSummary,
    boundary: &ValidatedBoundaryEntryPlan,
    installed_code: &InstalledCode,
    entry: EntryStubId,
    realization: ValidatedEntryStackRealization,
    validation_receipt: StackValidationReceiptId,
) -> Result<BoundEpochStackCompositionInput, ExternalRootDiagnostic> {
    let body_domains = body_domain_closure(boundary.plan().state.stack, &realization)?;
    validate_bound_adapter_realization(
        summary,
        boundary,
        installed_code,
        entry,
        &body_domains,
        &realization,
    )?;

    let realization_evidence = OpaqueAdapterStackRealizationEvidence {
        root: summary.root,
        provider: summary.provider,
        architecture: installed_code.architecture(),
        installed_code: installed_code.identity(),
        installed_code_context: installed_code.receipt_context(),
        artifact: installed_code.artifact(),
        entry,
        boundary_contract_fingerprint: boundary.contract_fingerprint(),
        body_domains,
        realization: realization.clone(),
        validation_receipt,
    };
    debug_assert!(realization_evidence.matches_installed_entry(installed_code, entry, boundary));
    Ok(BoundEpochStackCompositionInput {
        pure: EpochStackCompositionInput {
            root: summary.root,
            provider: summary.provider,
            realization,
            body_wcsu_bytes: summary.local_wcsu_bytes(),
            body_wcsu_alignment: summary.wcsu_alignment(),
        },
        body_evidence: summary.local_evidence.clone(),
        realization_evidence: AdapterStackRealizationEvidence::OpaqueProvider(realization_evidence),
    })
}

/// Derive the complete stack realization for a compiler-emitted direct entry.
/// The only epoch is the exact Terminal body's epoch; no caller-authored epoch
/// or provider receipt is accepted on this path.
pub fn bind_direct_generated_entry_stack_realization(
    summary: &ProviderStackSummary,
    boundary: &ValidatedBoundaryEntryPlan,
    installed_code: &InstalledCode,
    entry: EntryStubId,
    body_domains: ValidatedEntryStackDomainClosure,
) -> Result<BoundEpochStackCompositionInput, ExternalRootDiagnostic> {
    let StackLocalEvidence::TerminalEntry(binding) = &summary.local_evidence else {
        return Err(ExternalRootDiagnostic(
            "direct generated entry stack realization requires emitter-derived Terminal body evidence"
                .into(),
        ));
    };
    if !binding.matches_installed_entry(installed_code, entry) {
        return Err(ExternalRootDiagnostic(
            "direct generated entry body evidence names a different installed entry".into(),
        ));
    }
    if body_domains.boundary_stack() != boundary.plan().state.stack {
        return Err(ExternalRootDiagnostic(
            "direct generated entry stack-domain closure drifted from the boundary stack disposition"
                .into(),
        ));
    }
    let realization = validate_entry_stack_realization(EntryStackRealization {
        contexts: body_domains
            .contexts()
            .iter()
            .map(|context| ArrivalContextRealization {
                context: context.context,
                epochs: vec![EntryStackEpoch {
                    stage: EntryStackStage::Body,
                    active_domain: context.domain,
                    occupancy_by_domain: Vec::new(),
                    nesting: boundary.plan().state.preemption,
                }],
            })
            .collect(),
    })
    .map_err(|error| {
        ExternalRootDiagnostic(format!(
            "direct generated entry stack realization is invalid: {}",
            error.0
        ))
    })?;
    validate_bound_adapter_realization(
        summary,
        boundary,
        installed_code,
        entry,
        &body_domains,
        &realization,
    )?;
    Ok(BoundEpochStackCompositionInput {
        pure: EpochStackCompositionInput {
            root: summary.root,
            provider: summary.provider,
            realization: realization.clone(),
            body_wcsu_bytes: summary.local_wcsu_bytes(),
            body_wcsu_alignment: summary.wcsu_alignment(),
        },
        body_evidence: summary.local_evidence.clone(),
        realization_evidence: AdapterStackRealizationEvidence::DirectGenerated(
            DirectGeneratedEntryStackRealizationEvidence {
                root: summary.root,
                provider: summary.provider,
                architecture: installed_code.architecture(),
                installed_code: installed_code.identity(),
                installed_code_context: installed_code.receipt_context(),
                artifact: installed_code.artifact(),
                entry,
                boundary_contract_fingerprint: boundary.contract_fingerprint(),
                body_domains,
                realization,
            },
        ),
    })
}

fn validate_bound_adapter_realization(
    summary: &ProviderStackSummary,
    boundary: &ValidatedBoundaryEntryPlan,
    installed_code: &InstalledCode,
    entry: EntryStubId,
    body_domains: &ValidatedEntryStackDomainClosure,
    realization: &ValidatedEntryStackRealization,
) -> Result<(), ExternalRootDiagnostic> {
    installed_code.selected_entry_target(entry).map_err(|_| {
        ExternalRootDiagnostic("entry stack realization names no exact installed entry".into())
    })?;
    if boundary.plan().state.initial_regime.architecture() != installed_code.architecture() {
        return Err(ExternalRootDiagnostic(
            "entry stack realization target differs from the installed artifact architecture"
                .into(),
        ));
    }
    if summary.stack != boundary.plan().state.stack {
        return Err(ExternalRootDiagnostic(
            "entry stack summary drifted from the boundary plan's stack disposition".into(),
        ));
    }
    if body_domains.boundary_stack() != boundary.plan().state.stack {
        return Err(ExternalRootDiagnostic(
            "entry stack domain closure drifted from the boundary stack disposition".into(),
        ));
    }
    if body_domains.contexts().len() != realization.realization().contexts.len() {
        return Err(ExternalRootDiagnostic(
            "entry stack domain closure and realization contain different arrival-context sets"
                .into(),
        ));
    }
    for context in &realization.realization().contexts {
        let body = context
            .epochs
            .iter()
            .find(|epoch| epoch.stage == EntryStackStage::Body)
            .expect("validated realization has exactly one body epoch");
        let Some(closed) = body_domains
            .contexts()
            .iter()
            .find(|closed| closed.context == context.context)
        else {
            return Err(ExternalRootDiagnostic(format!(
                "entry stack arrival context 0x{:016x} is absent from the domain closure",
                context.context.get()
            )));
        };
        if body.active_domain != closed.domain {
            return Err(ExternalRootDiagnostic(format!(
                "entry stack arrival context 0x{:016x} executes its body on a domain other than its exact context closure",
                context.context.get()
            )));
        }
        for epoch in &context.epochs {
            if epoch.nesting == Preemption::ProviderDefined {
                return Err(ExternalRootDiagnostic(format!(
                    "entry stack arrival context 0x{:016x} retains unresolved provider-defined nesting",
                    context.context.get()
                )));
            }
            if !preemption_refines(epoch.nesting, boundary.plan().state.preemption) {
                return Err(ExternalRootDiagnostic(format!(
                    "entry stack arrival context 0x{:016x} widens the boundary plan's nesting ceiling",
                    context.context.get()
                )));
            }
        }
    }
    if let StackLocalEvidence::TerminalEntry(binding) = &summary.local_evidence
        && !binding.matches_installed_entry(installed_code, entry)
    {
        return Err(ExternalRootDiagnostic(
            "terminal body WCSU and adapter realization name different installed entries".into(),
        ));
    }
    Ok(())
}

fn body_domain_closure(
    boundary_stack: EntryStack,
    realization: &ValidatedEntryStackRealization,
) -> Result<ValidatedEntryStackDomainClosure, ExternalRootDiagnostic> {
    validate_entry_stack_domain_closure(
        boundary_stack,
        realization
            .realization()
            .contexts
            .iter()
            .map(|context| {
                let body = context
                    .epochs
                    .iter()
                    .find(|epoch| epoch.stage == EntryStackStage::Body)
                    .expect("validated realization has exactly one body epoch");
                ArrivalContextStackDomain {
                    context: context.context,
                    domain: body.active_domain,
                }
            })
            .collect(),
    )
    .map_err(|error| {
        ExternalRootDiagnostic(format!(
            "entry stack body-domain closure is invalid: {}",
            error.0
        ))
    })
}

fn preemption_refines(actual: Preemption, ceiling: Preemption) -> bool {
    match (actual, ceiling) {
        (_, Preemption::ProviderDefined) => true,
        (Preemption::NotApplicable | Preemption::Masked, Preemption::Nestable { .. }) => true,
        (
            Preemption::Nestable {
                maximum_depth: actual,
            },
            Preemption::Nestable {
                maximum_depth: ceiling,
            },
        ) => actual <= ceiling,
        (actual, ceiling) => actual == ceiling,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DomainStackDemand {
    pub bytes: u64,
    pub alignment: u64,
}

/// Context-maximized result for one root occurrence at artifact entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedEpochStackDemand {
    root: ExternalRootId,
    provider: RootProviderId,
    by_domain: BTreeMap<StackDomain, DomainStackDemand>,
    contributing_roots: BTreeSet<ExternalRootId>,
}

impl ComposedEpochStackDemand {
    pub const fn root(&self) -> ExternalRootId {
        self.root
    }

    pub const fn provider(&self) -> RootProviderId {
        self.provider
    }

    pub fn domain(&self, domain: StackDomain) -> Option<DomainStackDemand> {
        self.by_domain.get(&domain).copied()
    }

    pub fn domains(&self) -> impl Iterator<Item = (StackDomain, DomainStackDemand)> + '_ {
        self.by_domain
            .iter()
            .map(|(domain, demand)| (*domain, *demand))
    }

    pub const fn contributing_roots(&self) -> &BTreeSet<ExternalRootId> {
        &self.contributing_roots
    }
}

/// Exact pure-composition result. The retained inputs prevent a compact
/// fingerprint collision from becoming authority or equality evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpochStackComposition {
    relation: StackNestingRelation,
    inputs: BTreeMap<ExternalRootId, EpochStackCompositionInput>,
    demands: BTreeMap<ExternalRootId, ComposedEpochStackDemand>,
    domain_wcsu: BTreeMap<StackDomain, DomainStackDemand>,
    fingerprint: u64,
}

impl EpochStackComposition {
    pub const fn relation(&self) -> &StackNestingRelation {
        &self.relation
    }

    pub fn demand(&self, root: ExternalRootId) -> Option<&ComposedEpochStackDemand> {
        self.demands.get(&root)
    }

    pub fn domain(&self, domain: StackDomain) -> Option<DomainStackDemand> {
        self.domain_wcsu.get(&domain).copied()
    }

    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
}

/// Admission-capable epoch composition retaining every exact body and adapter
/// evidence row behind the pure arithmetic result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundEpochStackComposition {
    composition: EpochStackComposition,
    inputs: BTreeMap<ExternalRootId, BoundEpochStackCompositionInput>,
    fingerprint: u64,
}

impl BoundEpochStackComposition {
    pub const fn composition(&self) -> &EpochStackComposition {
        &self.composition
    }

    pub fn input(&self, root: ExternalRootId) -> Option<&BoundEpochStackCompositionInput> {
        self.inputs.get(&root)
    }

    pub fn inputs(
        &self,
    ) -> impl Iterator<Item = (&ExternalRootId, &BoundEpochStackCompositionInput)> {
        self.inputs.iter()
    }

    pub fn demand(&self, root: ExternalRootId) -> Option<&ComposedEpochStackDemand> {
        self.composition.demand(root)
    }

    pub fn domain(&self, domain: StackDomain) -> Option<DomainStackDemand> {
        self.composition.domain(domain)
    }

    pub const fn relation(&self) -> &StackNestingRelation {
        self.composition.relation()
    }

    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
}

pub fn compose_bound_entry_stack_epochs<'a>(
    relation: &StackNestingRelation,
    inputs: impl IntoIterator<Item = &'a BoundEpochStackCompositionInput>,
) -> Result<BoundEpochStackComposition, ExternalRootDiagnostic> {
    let mut bound = BTreeMap::new();
    for input in inputs {
        if bound.insert(input.root(), input.clone()).is_some() {
            return Err(ExternalRootDiagnostic(format!(
                "bound epoch stack input for root 0x{:016x} is duplicated",
                input.root().normalized_identity()
            )));
        }
    }
    let composition = compose_entry_stack_epochs(
        relation,
        bound.values().map(BoundEpochStackCompositionInput::pure),
    )?;
    let mut fingerprint = Fnv1a::new();
    fingerprint.u64(composition.fingerprint());
    fingerprint.u64(bound.len() as u64);
    for input in bound.values() {
        let evidence = input.realization_evidence();
        fingerprint.u64(match evidence.origin() {
            AdapterStackRealizationOrigin::DirectGenerated => 0,
            AdapterStackRealizationOrigin::OpaqueProvider => 1,
        });
        fingerprint.u64(evidence.root().normalized_identity());
        fingerprint.u64(evidence.provider().normalized_identity());
        fingerprint.u64(match evidence.architecture() {
            omega_target::Architecture::X86_64 => 1,
            omega_target::Architecture::Aarch64 => 2,
        });
        fingerprint.u64(evidence.installed_code().normalized_identity());
        fingerprint.u64(evidence.artifact().normalized_identity());
        fingerprint.u64(evidence.entry().normalized_identity());
        fingerprint.u64(evidence.boundary_contract_fingerprint());
        let body_domains = match evidence {
            AdapterStackRealizationEvidence::DirectGenerated(evidence) => &evidence.body_domains,
            AdapterStackRealizationEvidence::OpaqueProvider(evidence) => &evidence.body_domains,
        };
        fingerprint.u64(body_domains.fingerprint());
        fingerprint.u64(evidence.realization().fingerprint());
        fingerprint.u64(
            evidence
                .validation_receipt()
                .map(StackValidationReceiptId::normalized_identity)
                .unwrap_or_default(),
        );
    }
    Ok(BoundEpochStackComposition {
        composition,
        inputs: bound,
        fingerprint: fingerprint.finish(),
    })
}

/// Compose structurally validated epoch plans.
///
/// Epochs within one context are sequential alternatives and take their
/// per-domain maximum. Arrival contexts are alternatives and also take their
/// maximum. A permitted nested occurrence is concurrent: its per-domain demand
/// is appended with alignment to the parent epoch's live occupancy. Relative
/// `Interrupted` domains resolve to the parent epoch's active domain.
pub fn compose_entry_stack_epochs<'a>(
    relation: &StackNestingRelation,
    inputs: impl IntoIterator<Item = &'a EpochStackCompositionInput>,
) -> Result<EpochStackComposition, ExternalRootDiagnostic> {
    let mut by_root = BTreeMap::new();
    for input in inputs {
        validate_input(input)?;
        if by_root.insert(input.root, input.clone()).is_some() {
            return Err(ExternalRootDiagnostic(format!(
                "epoch stack input for root 0x{:016x} is duplicated",
                input.root.normalized_identity()
            )));
        }
    }
    if by_root.is_empty() {
        return Err(ExternalRootDiagnostic(
            "epoch stack composition requires at least one root input".into(),
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
        if !by_root.contains_key(&edge.preemptor) {
            return Err(ExternalRootDiagnostic(format!(
                "stack nesting relation references missing preemptor root 0x{:016x}",
                edge.preemptor.normalized_identity()
            )));
        }
        outgoing
            .entry(edge.interrupted)
            .or_default()
            .push(edge.preemptor);
    }

    let domains = stack_domains(&by_root);
    let maximum_depth = maximum_nesting_depth(&by_root);
    let mut memo = BTreeMap::new();
    for live_depth in (1..=maximum_depth).rev() {
        for interrupted_domain in &domains {
            for input in by_root.values() {
                let composed = compose_root_at_depth(
                    input.root,
                    *interrupted_domain,
                    live_depth,
                    &outgoing,
                    &by_root,
                    &memo,
                )?;
                memo.insert((input.root, *interrupted_domain, live_depth), composed);
            }
        }
    }

    let mut demands = BTreeMap::new();
    let mut domain_wcsu = BTreeMap::new();
    for input in by_root.values() {
        let composed = memo
            .get(&(input.root, StackDomain::Interrupted, 1))
            .expect("depth-one root state was composed")
            .clone();
        merge_alternative_map(&mut domain_wcsu, &composed.by_domain);
        demands.insert(input.root, composed);
    }

    let fingerprint = fingerprint_inputs(relation, &by_root);
    Ok(EpochStackComposition {
        relation: relation.clone(),
        inputs: by_root,
        demands,
        domain_wcsu,
        fingerprint,
    })
}

fn validate_input(input: &EpochStackCompositionInput) -> Result<(), ExternalRootDiagnostic> {
    if input.body_wcsu_bytes == 0 {
        return Err(ExternalRootDiagnostic(format!(
            "epoch stack input for root 0x{:016x} has zero body WCSU",
            input.root.normalized_identity()
        )));
    }
    if input.body_wcsu_alignment == 0 || !input.body_wcsu_alignment.is_power_of_two() {
        return Err(ExternalRootDiagnostic(format!(
            "epoch stack input for root 0x{:016x} has body alignment {} instead of a nonzero power of two",
            input.root.normalized_identity(),
            input.body_wcsu_alignment
        )));
    }
    Ok(())
}

fn maximum_nesting_depth(inputs: &BTreeMap<ExternalRootId, EpochStackCompositionInput>) -> u16 {
    inputs
        .values()
        .flat_map(|input| &input.realization.realization().contexts)
        .flat_map(|context| &context.epochs)
        .filter_map(|epoch| match epoch.nesting {
            Preemption::Nestable { maximum_depth } => Some(maximum_depth),
            Preemption::NotApplicable | Preemption::Masked | Preemption::ProviderDefined => None,
        })
        .max()
        .unwrap_or(1)
}

fn stack_domains(
    inputs: &BTreeMap<ExternalRootId, EpochStackCompositionInput>,
) -> BTreeSet<StackDomain> {
    let mut domains = BTreeSet::from([StackDomain::Interrupted]);
    for epoch in inputs
        .values()
        .flat_map(|input| &input.realization.realization().contexts)
        .flat_map(|context| &context.epochs)
    {
        if let StackDomainRef::Dedicated { class } = epoch.active_domain {
            domains.insert(StackDomain::Dedicated { class });
        }
        for occupancy in &epoch.occupancy_by_domain {
            if let StackDomainRef::Dedicated { class } = occupancy.domain {
                domains.insert(StackDomain::Dedicated { class });
            }
        }
    }
    domains
}

fn compose_root_at_depth(
    root: ExternalRootId,
    interrupted_domain: StackDomain,
    live_depth: u16,
    outgoing: &BTreeMap<ExternalRootId, Vec<ExternalRootId>>,
    inputs: &BTreeMap<ExternalRootId, EpochStackCompositionInput>,
    memo: &BTreeMap<(ExternalRootId, StackDomain, u16), ComposedEpochStackDemand>,
) -> Result<ComposedEpochStackDemand, ExternalRootDiagnostic> {
    let input = inputs.get(&root).expect("nesting endpoint was validated");
    let mut root_peak = BTreeMap::new();
    let mut contributing_roots = BTreeSet::from([root]);

    for context in &input.realization.realization().contexts {
        let mut context_peak = BTreeMap::new();
        for epoch in &context.epochs {
            let active_domain = resolve_domain(epoch.active_domain, interrupted_domain)?;
            let mut base = BTreeMap::new();
            for occupancy in &epoch.occupancy_by_domain {
                let domain = resolve_domain(occupancy.domain, interrupted_domain)?;
                append_demand(
                    &mut base,
                    domain,
                    DomainStackDemand {
                        bytes: occupancy.bytes,
                        alignment: occupancy.alignment,
                    },
                )?;
            }
            if epoch.stage == EntryStackStage::Body {
                append_demand(
                    &mut base,
                    active_domain,
                    DomainStackDemand {
                        bytes: input.body_wcsu_bytes,
                        alignment: input.body_wcsu_alignment,
                    },
                )?;
            }

            let mut epoch_peak = base.clone();
            if let Preemption::Nestable { maximum_depth } = epoch.nesting
                && live_depth < maximum_depth
                && let Some(preemptors) = outgoing.get(&root)
            {
                for preemptor in preemptors {
                    let nested = memo
                        .get(&(*preemptor, active_domain, live_depth + 1))
                        .expect("deeper nesting state was composed first");
                    contributing_roots.extend(nested.contributing_roots.iter().copied());
                    let concurrent = append_maps(&base, &nested.by_domain)?;
                    merge_alternative_map(&mut epoch_peak, &concurrent);
                }
            }
            merge_alternative_map(&mut context_peak, &epoch_peak);
        }
        merge_alternative_map(&mut root_peak, &context_peak);
    }

    Ok(ComposedEpochStackDemand {
        root,
        provider: input.provider,
        by_domain: root_peak,
        contributing_roots,
    })
}

fn resolve_domain(
    domain: StackDomainRef,
    interrupted_domain: StackDomain,
) -> Result<StackDomain, ExternalRootDiagnostic> {
    match domain {
        StackDomainRef::Interrupted => Ok(interrupted_domain),
        StackDomainRef::Dedicated { class } => Ok(StackDomain::Dedicated { class }),
        StackDomainRef::ProviderSelected => Err(ExternalRootDiagnostic(
            "validated epoch stack realization retained a provider-selected domain".into(),
        )),
    }
}

fn append_maps(
    parent: &BTreeMap<StackDomain, DomainStackDemand>,
    nested: &BTreeMap<StackDomain, DomainStackDemand>,
) -> Result<BTreeMap<StackDomain, DomainStackDemand>, ExternalRootDiagnostic> {
    let mut combined = parent.clone();
    for (domain, demand) in nested {
        append_demand(&mut combined, *domain, *demand)?;
    }
    Ok(combined)
}

fn append_demand(
    demands: &mut BTreeMap<StackDomain, DomainStackDemand>,
    domain: StackDomain,
    appended: DomainStackDemand,
) -> Result<(), ExternalRootDiagnostic> {
    match demands.get_mut(&domain) {
        Some(existing) => {
            existing.bytes =
                super::stack_demand::align_up_checked(existing.bytes, appended.alignment)?
                    .checked_add(appended.bytes)
                    .ok_or_else(|| {
                        ExternalRootDiagnostic("stack epoch demand addition overflowed".into())
                    })?;
            existing.alignment = existing.alignment.max(appended.alignment);
        }
        None => {
            demands.insert(domain, appended);
        }
    }
    Ok(())
}

fn merge_alternative_map(
    target: &mut BTreeMap<StackDomain, DomainStackDemand>,
    alternative: &BTreeMap<StackDomain, DomainStackDemand>,
) {
    for (domain, demand) in alternative {
        target
            .entry(*domain)
            .and_modify(|current| {
                current.bytes = current.bytes.max(demand.bytes);
                current.alignment = current.alignment.max(demand.alignment);
            })
            .or_insert(*demand);
    }
}

fn fingerprint_inputs(
    relation: &StackNestingRelation,
    inputs: &BTreeMap<ExternalRootId, EpochStackCompositionInput>,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.u64(relation.identity.normalized_identity());
    hash.u64(inputs.len() as u64);
    for input in inputs.values() {
        hash.u64(input.root.normalized_identity());
        hash.u64(input.provider.normalized_identity());
        hash.u64(input.realization.fingerprint());
        hash.u64(input.body_wcsu_bytes);
        hash.u64(input.body_wcsu_alignment);
    }
    hash.u64(relation.edges.len() as u64);
    for edge in &relation.edges {
        hash.u64(edge.interrupted.normalized_identity());
        hash.u64(edge.preemptor.normalized_identity());
    }
    hash.finish()
}

#[cfg(test)]
mod tests {
    use super::super::{NestingRelationId, StackNestingEdge};
    use super::*;
    use omega_calling_conventions::{
        ArrivalContextId, ArrivalContextRealization, EntryStackEpoch, EntryStackRealization,
        StackOccupancy, validate_entry_stack_realization,
    };

    fn id<T>(value: u64, make: impl FnOnce(u64) -> Result<T, ExternalRootDiagnostic>) -> T {
        make(value).expect("nonzero normalized identity")
    }

    fn realization(contexts: Vec<ArrivalContextRealization>) -> ValidatedEntryStackRealization {
        validate_entry_stack_realization(EntryStackRealization { contexts })
            .expect("valid stack realization")
    }

    fn epoch(
        stage: EntryStackStage,
        active_domain: StackDomainRef,
        occupancy_by_domain: Vec<StackOccupancy>,
        nesting: Preemption,
    ) -> EntryStackEpoch {
        EntryStackEpoch {
            stage,
            active_domain,
            occupancy_by_domain,
            nesting,
        }
    }

    fn context(value: u64, epochs: Vec<EntryStackEpoch>) -> ArrivalContextRealization {
        ArrivalContextRealization {
            context: ArrivalContextId::new(value).expect("nonzero context"),
            epochs,
        }
    }

    #[test]
    fn epochs_and_contexts_take_maxima_while_body_wcsu_joins_only_the_body_domain() {
        let root = id(1, ExternalRootId::from_normalized_identity);
        let input = EpochStackCompositionInput {
            root,
            provider: id(2, RootProviderId::from_normalized_identity),
            realization: realization(vec![
                context(
                    1,
                    vec![
                        epoch(
                            EntryStackStage::Enter,
                            StackDomainRef::Interrupted,
                            vec![StackOccupancy {
                                domain: StackDomainRef::Interrupted,
                                bytes: 120,
                                alignment: 8,
                            }],
                            Preemption::Masked,
                        ),
                        epoch(
                            EntryStackStage::Body,
                            StackDomainRef::Dedicated { class: 4 },
                            vec![StackOccupancy {
                                domain: StackDomainRef::Dedicated { class: 4 },
                                bytes: 8,
                                alignment: 8,
                            }],
                            Preemption::Masked,
                        ),
                    ],
                ),
                context(
                    2,
                    vec![epoch(
                        EntryStackStage::Body,
                        StackDomainRef::Interrupted,
                        vec![StackOccupancy {
                            domain: StackDomainRef::Interrupted,
                            bytes: 24,
                            alignment: 8,
                        }],
                        Preemption::Masked,
                    )],
                ),
            ]),
            body_wcsu_bytes: 64,
            body_wcsu_alignment: 16,
        };
        let composed = compose_entry_stack_epochs(
            &StackNestingRelation {
                identity: id(3, NestingRelationId::from_normalized_identity),
                edges: BTreeSet::new(),
            },
            [&input],
        )
        .expect("context-aware composition");

        assert_eq!(
            composed.domain(StackDomain::Interrupted),
            Some(DomainStackDemand {
                bytes: 120,
                alignment: 16,
            })
        );
        assert_eq!(
            composed.domain(StackDomain::Dedicated { class: 4 }),
            Some(DomainStackDemand {
                bytes: 80,
                alignment: 16,
            })
        );
    }

    #[test]
    fn nested_interrupted_is_path_relative_and_finite_depth_closes_cycles() {
        let parent = id(10, ExternalRootId::from_normalized_identity);
        let child = id(11, ExternalRootId::from_normalized_identity);
        let provider = id(12, RootProviderId::from_normalized_identity);
        let parent_input = EpochStackCompositionInput {
            root: parent,
            provider,
            realization: realization(vec![context(
                1,
                vec![epoch(
                    EntryStackStage::Body,
                    StackDomainRef::Dedicated { class: 4 },
                    vec![StackOccupancy {
                        domain: StackDomainRef::Dedicated { class: 4 },
                        bytes: 24,
                        alignment: 8,
                    }],
                    Preemption::Nestable { maximum_depth: 2 },
                )],
            )]),
            body_wcsu_bytes: 40,
            body_wcsu_alignment: 16,
        };
        let child_input = EpochStackCompositionInput {
            root: child,
            provider,
            realization: realization(vec![context(
                1,
                vec![epoch(
                    EntryStackStage::Body,
                    StackDomainRef::Interrupted,
                    vec![StackOccupancy {
                        domain: StackDomainRef::Interrupted,
                        bytes: 8,
                        alignment: 8,
                    }],
                    Preemption::Nestable { maximum_depth: 2 },
                )],
            )]),
            body_wcsu_bytes: 16,
            body_wcsu_alignment: 16,
        };
        let relation = StackNestingRelation {
            identity: id(13, NestingRelationId::from_normalized_identity),
            edges: BTreeSet::from([
                StackNestingEdge {
                    interrupted: parent,
                    preemptor: child,
                },
                StackNestingEdge {
                    interrupted: child,
                    preemptor: parent,
                },
            ]),
        };
        let composed = compose_entry_stack_epochs(&relation, [&parent_input, &child_input])
            .expect("finite epoch nesting");

        assert_eq!(
            composed
                .demand(parent)
                .expect("parent demand")
                .domain(StackDomain::Dedicated { class: 4 }),
            Some(DomainStackDemand {
                bytes: 112,
                alignment: 16,
            })
        );
        assert_eq!(
            composed.domain(StackDomain::Interrupted),
            Some(DomainStackDemand {
                bytes: 32,
                alignment: 16,
            })
        );
        assert_eq!(
            composed.domain(StackDomain::Dedicated { class: 4 }),
            Some(DomainStackDemand {
                bytes: 112,
                alignment: 16,
            })
        );
    }

    #[test]
    fn input_validation_and_missing_nesting_endpoints_fail_closed() {
        let root = id(20, ExternalRootId::from_normalized_identity);
        let mut input = EpochStackCompositionInput {
            root,
            provider: id(21, RootProviderId::from_normalized_identity),
            realization: realization(vec![context(
                1,
                vec![epoch(
                    EntryStackStage::Body,
                    StackDomainRef::Interrupted,
                    Vec::new(),
                    Preemption::Masked,
                )],
            )]),
            body_wcsu_bytes: 8,
            body_wcsu_alignment: 8,
        };
        let relation = StackNestingRelation {
            identity: id(22, NestingRelationId::from_normalized_identity),
            edges: BTreeSet::new(),
        };
        input.body_wcsu_alignment = 3;
        let error = compose_entry_stack_epochs(&relation, [&input])
            .expect_err("malformed body alignment must reject");
        assert!(error.0.contains("nonzero power of two"));

        input.body_wcsu_alignment = 8;
        let missing = id(23, ExternalRootId::from_normalized_identity);
        let error = compose_entry_stack_epochs(
            &StackNestingRelation {
                identity: relation.identity,
                edges: BTreeSet::from([StackNestingEdge {
                    interrupted: root,
                    preemptor: missing,
                }]),
            },
            [&input],
        )
        .expect_err("missing nested root must reject");
        assert!(error.0.contains("missing preemptor"));
    }
}
