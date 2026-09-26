//! Pure composition of validated external-entry stack epochs.
//!
//! This module computes context-, phase-, and domain-aware stack demand. It
//! deliberately does not turn structurally validated plan data into admission
//! evidence: callers must separately bind each realization to sealed target
//! facts, emitted adapter bytes, or an admitted opaque-provider receipt.
//!
//! This file composes the epochs. `arrival_contexts.rs` admits opaque
//! arrival contexts and gate profile rosters, `stack_realization_evidence.rs`
//! binds entry and adapter stack realization evidence,
//! `x86_64_hardware_entries.rs` produces installed x86_64 hardware entry
//! facts and `tests.rs` holds the composition tests.

mod arrival_contexts;
mod stack_realization_evidence;
#[cfg(test)]
mod tests;
mod x86_64_hardware_entries;

pub use arrival_contexts::{
    AdmittedOpaqueArrivalContextSet, EpochStackCompositionInput,
    ValidatedX86_64InstalledGateProfileRoster, admit_opaque_arrival_context_set,
    validate_x86_64_installed_gate_profile_roster,
};
pub use stack_realization_evidence::{
    AdapterStackRealizationOrigin, ArrivalStackRealizationOrigin, BoundEpochStackCompositionInput,
    EntryStackRealizationEvidence, GeneratedProgramStorageAdapterLiveFrameDemand,
    GeneratedProgramStorageAdapterStackEvidence, X86_64GeneratedProgramStorageAdapterEmission,
    bind_direct_generated_entry_stack_realization, bind_opaque_adapter_stack_realization,
    derive_generated_program_storage_adapter_live_frame_demand,
};
pub use x86_64_hardware_entries::{
    InstalledX86_64TargetDerivedHardwareArrival,
    bind_x86_64_generated_program_storage_adapter_stack_realization,
    bind_x86_64_target_direct_entry_stack_realization,
    produce_x86_64_installed_hardware_entry_facts,
};

use std::collections::{BTreeMap, BTreeSet};

use abstract_operations_to_target_operations::calling_conventions::{
    EntryStackStage, Preemption, StackDomainRef,
};

use crate::external_roots::identities::Fnv1a;
use crate::external_roots::stack_and_fuel::stack_demand::fingerprint_stack_local_evidence;
use crate::external_roots::{
    ExternalRootDiagnostic, ExternalRootId, RootProviderId, StackDomain, StackNestingRelation,
    StackValidationReceiptId, X86_64GateProfileValidationReceiptId,
};

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
    non_authoritative_report_fingerprint: u64,
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

    /// Compatibility accessor for the non-authoritative report/cache
    /// fingerprint. Exact relation, inputs, and demands remain retained.
    pub const fn report_fingerprint(&self) -> u64 {
        self.non_authoritative_report_fingerprint
    }

    pub const fn non_authoritative_report_fingerprint(&self) -> u64 {
        self.non_authoritative_report_fingerprint
    }
}

/// Admission-capable epoch composition retaining every exact body and adapter
/// evidence row behind the pure arithmetic result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundEpochStackComposition {
    composition: EpochStackComposition,
    inputs: BTreeMap<ExternalRootId, BoundEpochStackCompositionInput>,
    non_authoritative_report_fingerprint: u64,
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

    /// Compatibility accessor for the non-authoritative report/cache
    /// fingerprint. Every exact bound body and realization row is retained.
    pub const fn report_fingerprint(&self) -> u64 {
        self.non_authoritative_report_fingerprint
    }

    pub const fn non_authoritative_report_fingerprint(&self) -> u64 {
        self.non_authoritative_report_fingerprint
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
    let mut report_fingerprint = Fnv1a::new();
    report_fingerprint.u64(composition.report_fingerprint());
    report_fingerprint.u64(bound.len() as u64);
    for input in bound.values() {
        fingerprint_stack_local_evidence(&mut report_fingerprint, input.body_evidence());
        let evidence = input.realization_evidence();
        report_fingerprint.u64(match evidence.arrival_origin() {
            ArrivalStackRealizationOrigin::NoHardwareArrival => 0,
            ArrivalStackRealizationOrigin::X86_64TargetRule => 1,
            ArrivalStackRealizationOrigin::OpaqueProvider => 2,
        });
        report_fingerprint.u64(match evidence.adapter_origin() {
            AdapterStackRealizationOrigin::None => 0,
            AdapterStackRealizationOrigin::GeneratedProgramStorageSemanticWrapper => 1,
            AdapterStackRealizationOrigin::OpaqueProvider => 2,
        });
        report_fingerprint.u64(evidence.root().normalized_identity());
        report_fingerprint.u64(evidence.provider().normalized_identity());
        report_fingerprint.u64(match evidence.architecture() {
            target::Architecture::X86_64 => 1,
            target::Architecture::Aarch64 => 2,
        });
        report_fingerprint.u64(evidence.installed_code().normalized_identity());
        report_fingerprint.u64(evidence.artifact().normalized_identity());
        report_fingerprint.u64(evidence.entry().normalized_identity());
        report_fingerprint.u64(evidence.boundary_contract_report_fingerprint());
        report_fingerprint.bytes(&evidence.boundary_contract_commitment());
        report_fingerprint.u64(evidence.body_domains.report_fingerprint());
        report_fingerprint.u64(evidence.realization().report_fingerprint());
        report_fingerprint.u64(
            evidence
                .target_rule_report_fingerprint()
                .unwrap_or_default(),
        );
        report_fingerprint.u64(
            evidence
                .target_installation_validation_receipt()
                .map(X86_64GateProfileValidationReceiptId::normalized_identity)
                .unwrap_or_default(),
        );
        report_fingerprint.u64(
            evidence
                .generated_adapter()
                .map(GeneratedProgramStorageAdapterStackEvidence::report_fingerprint)
                .unwrap_or_default(),
        );
        report_fingerprint.u64(
            evidence
                .validation_receipt()
                .map(StackValidationReceiptId::normalized_identity)
                .unwrap_or_default(),
        );
    }
    Ok(BoundEpochStackComposition {
        composition,
        inputs: bound,
        non_authoritative_report_fingerprint: report_fingerprint.finish(),
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

    let non_authoritative_report_fingerprint =
        non_authoritative_epoch_stack_inputs_report_fingerprint(relation, &by_root);
    Ok(EpochStackComposition {
        relation: relation.clone(),
        inputs: by_root,
        demands,
        domain_wcsu,
        non_authoritative_report_fingerprint,
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
            existing.bytes = crate::external_roots::stack_and_fuel::stack_demand::align_up_checked(
                existing.bytes,
                appended.alignment,
            )?
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

fn non_authoritative_epoch_stack_inputs_report_fingerprint(
    relation: &StackNestingRelation,
    inputs: &BTreeMap<ExternalRootId, EpochStackCompositionInput>,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.u64(relation.identity.normalized_identity());
    hash.u64(inputs.len() as u64);
    for input in inputs.values() {
        hash.u64(input.root.normalized_identity());
        hash.u64(input.provider.normalized_identity());
        hash.u64(input.realization.report_fingerprint());
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
