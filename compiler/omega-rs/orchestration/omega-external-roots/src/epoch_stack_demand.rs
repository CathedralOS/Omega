//! Pure composition of validated external-entry stack epochs.
//!
//! This module computes context-, phase-, and domain-aware stack demand. It
//! deliberately does not turn structurally validated plan data into admission
//! evidence: callers must separately bind each realization to sealed target
//! facts, emitted adapter bytes, or an admitted opaque-provider receipt.

use std::collections::{BTreeMap, BTreeSet};

use omega_calling_conventions::{
    EntryStackStage, Preemption, StackDomainRef, ValidatedEntryStackRealization,
};

use super::{
    ExternalRootDiagnostic, ExternalRootId, Fnv1a, RootProviderId, StackDomain,
    StackNestingRelation,
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
