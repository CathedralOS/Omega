//! Typed build behavior exclusions and the Terminal-closure absence checker.
//!
//! A build may forbid exact behavior — a crash cause or an abstract boundary
//! service — in the selected executable composition, independently of the
//! conservative public contracts the same declarations publish. This is what
//! separates a verified no-op assertion composition from a checking one under
//! an identical declared Trap ceiling, and an ordinary silent logger from an
//! actual Console invocation behind an equally broad published ceiling.
//!
//! The check walks the unoptimized Terminal closure because semantic absence
//! cannot be established by native instruction scanning, by a
//! producer-written summary row, or by a test run, and optional optimization
//! selection must not determine admissibility. For the same reason the walk
//! never consults an in-module machine's `contract.crash_routes`,
//! `published_service_ceiling`, or `declared_service_reach`: those publish the
//! callable's broad allowance, while the verified selected body is the
//! stronger fact this composition is entitled to. Bodyless boundary
//! declarations have no body evidence, so their declared crash routes and
//! fixed service reach count conservatively.
//!
//! The report distinguishes a witnessed prohibited site from an evidence gap:
//! both fail admission, but a conservative possible path is not labeled a
//! runtime execution. `establish_behavior_exclusions` consumes an
//! already-selected entry roster and the selected provider-plan facts that
//! decide which retained provider-candidate bodies a boundary call can
//! realize.
//!
//! A boundary call's own fixed service reach always counts: the requirement
//! invocation contributes it no matter which provider the installation picks.
//! Its declared crash routes count only while realization coverage is
//! incomplete — a retained candidate the selected plan names by exact
//! `CheckedAdapter` identity supplies a verified body, which is stronger
//! evidence than the published ceiling it refined. An unconstrained slot
//! admits every retained candidate plus the contract any external provider
//! could only conform to. Bounded dynamic dispatches rejoin the module's
//! dispatch catalog and cover their exact realization; calls through an
//! existential descriptor parameter have no retained target and remain gaps.
//!
//! The build authoring surface is `builder.exclude_crash(CrashCause::X)`, a
//! toolchain Build machine harvested statically from the root build entry's
//! checked call scope (see `declarations::harvest_behavior_exclusions`).
//! Authored selections retain their exact declaration symbol, selecting
//! machine, and source span so the product-admission join can reproduce and
//! audit the canonical union independently. `builder.exclude_service<Trait>()`
//! is the service counterpart: a build-declaration marker whose single type
//! path must resolve to an exact boundary trait; its Terminal service
//! identity is resolved per module at the join. The product-admission join
//! itself lives in `checked-compilation-to-terminal-artifact`, which replays
//! the unoptimized lowering before the artifact is admitted.

use semantic_vocabulary::{BlockId, BoundaryMachineId, MachineId, OperationId, ServiceId};
use std::collections::{BTreeMap, VecDeque};
use symbols::SymbolHandle;
use terminal_psi::{
    CrashCause, OperationKind, ProviderCandidateConformance, TerminalMachine, TerminalModule,
    Terminator,
};

/// One exact exclusion selected by the build root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BehaviorExclusion {
    CrashCause(CrashCause),
    Service(ServiceId),
}

/// Canonical union of the root's selected exclusions: sorted, deduplicated,
/// so reordered or repeated selections compare equal.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BehaviorExclusions {
    crash_causes: Vec<CrashCause>,
    services: Vec<ServiceId>,
}

impl BehaviorExclusions {
    pub fn from_selections(selections: impl IntoIterator<Item = BehaviorExclusion>) -> Self {
        let mut exclusions = Self::default();
        for selection in selections {
            match selection {
                BehaviorExclusion::CrashCause(cause) => exclusions.crash_causes.push(cause),
                BehaviorExclusion::Service(service) => exclusions.services.push(service),
            }
        }
        exclusions.sort_and_deduplicate();
        exclusions
    }

    /// Set union; a later selection never removes an earlier restriction.
    pub fn union(&mut self, other: &Self) {
        self.crash_causes.extend_from_slice(&other.crash_causes);
        self.services.extend_from_slice(&other.services);
        self.sort_and_deduplicate();
    }

    pub fn is_empty(&self) -> bool {
        self.crash_causes.is_empty() && self.services.is_empty()
    }

    pub fn excludes_crash_cause(&self, cause: CrashCause) -> bool {
        self.crash_causes.contains(&cause)
    }

    pub fn excludes_service(&self, service: ServiceId) -> bool {
        self.services.contains(&service)
    }

    pub fn crash_causes(&self) -> &[CrashCause] {
        &self.crash_causes
    }

    pub fn services(&self) -> &[ServiceId] {
        &self.services
    }

    fn sort_and_deduplicate(&mut self) {
        self.crash_causes.sort_unstable();
        self.crash_causes.dedup();
        self.services.sort_unstable();
        self.services.dedup();
    }
}

/// One authored exclusion kind with its exact typed-stage declaration
/// identity. The terminal-facing [`BehaviorExclusion`] set derives from these
/// rows at the product-admission join, once selected service identities exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoredBehaviorExclusionKind {
    /// `builder.exclude_crash(CrashCause::X)`: the terminal crash cause plus
    /// the exact toolchain `CrashCause` case symbol the authored value named.
    CrashCause {
        cause: CrashCause,
        case_symbol: SymbolHandle,
    },
    /// `builder.exclude_service<Trait>()`: the exact boundary-trait symbol
    /// the authored type path resolved to. The Terminal service identity it
    /// names exists only per module, so the row resolves at the
    /// product-admission join against that module's service catalog.
    Service { trait_symbol: SymbolHandle },
}

/// One authored exclusion selection retained from the root build machine's
/// checked call scope. Rows keep their authored occurrence provenance;
/// repeated selections of the same kind union idempotently at the join, so
/// authored order cannot change the resulting admission requirement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoredBehaviorExclusion {
    pub kind: AuthoredBehaviorExclusionKind,
    /// The typed machine that spelled the selection. A specialization
    /// template and its instances retain the same authored span under their
    /// own machine symbols.
    pub selecting_machine: SymbolHandle,
    pub source_span: source::SourceSpan,
}

impl AuthoredBehaviorExclusion {
    /// The canonical exclusion this authored row selects in `module`.
    /// `trait_identity` maps a checked boundary-trait symbol to the identity
    /// Terminal service declarations carry. A service exclusion whose
    /// identity `module` never declares resolves to `None`: no boundary in
    /// that composition can invoke it, so it imposes no requirement there.
    pub fn resolve(
        &self,
        module: &TerminalModule,
        trait_identity: &dyn Fn(SymbolHandle) -> Option<String>,
    ) -> Option<BehaviorExclusion> {
        match self.kind {
            AuthoredBehaviorExclusionKind::CrashCause { cause, .. } => {
                Some(BehaviorExclusion::CrashCause(cause))
            }
            AuthoredBehaviorExclusionKind::Service { trait_symbol } => {
                let identity = trait_identity(trait_symbol)?;
                module
                    .services
                    .iter()
                    .find(|service| service.identity == identity)
                    .map(|service| BehaviorExclusion::Service(service.id))
            }
        }
    }
}

/// The canonical crash-cause union a set of authored rows selects. Service
/// rows need a module to resolve their identity and are not part of this
/// set; product admission uses [`authored_behavior_exclusion_set_in`].
pub fn authored_behavior_exclusion_set(
    exclusions: &[AuthoredBehaviorExclusion],
) -> BehaviorExclusions {
    BehaviorExclusions::from_selections(exclusions.iter().filter_map(|row| match row.kind {
        AuthoredBehaviorExclusionKind::CrashCause { cause, .. } => {
            Some(BehaviorExclusion::CrashCause(cause))
        }
        AuthoredBehaviorExclusionKind::Service { .. } => None,
    }))
}

/// The canonical union a set of authored rows selects in `module`, service
/// rows resolved against that module's service catalog. Duplicates and
/// authored order collapse under the same deduplication the terminal-side
/// checker consumes.
pub fn authored_behavior_exclusion_set_in(
    exclusions: &[AuthoredBehaviorExclusion],
    module: &TerminalModule,
    trait_identity: &dyn Fn(SymbolHandle) -> Option<String>,
) -> BehaviorExclusions {
    BehaviorExclusions::from_selections(
        exclusions
            .iter()
            .filter_map(|row| row.resolve(module, trait_identity)),
    )
}

/// The executable coordinate where a possible excluded behavior is retained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProhibitedSite {
    CrashTerminator {
        block: BlockId,
    },
    BoundaryCall {
        block: BlockId,
        operation: OperationId,
        boundary: BoundaryMachineId,
    },
    PortWrite {
        block: BlockId,
        operation: OperationId,
    },
}

/// One possible excluded behavior attributable to an entry, machine, and site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProhibitedBehavior {
    pub exclusion: BehaviorExclusion,
    pub entry: MachineId,
    pub machine: MachineId,
    pub site: ProhibitedSite,
}

/// Why the closure evidence could not certify absence at one site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceGapKind {
    DynamicCall,
    UnknownCallee(MachineId),
    UnknownBoundary(BoundaryMachineId),
    UnknownEntry,
}

/// One site whose possible behavior the retained evidence does not cover.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceGap {
    pub entry: MachineId,
    pub machine: MachineId,
    pub block: Option<BlockId>,
    pub operation: Option<OperationId>,
    pub kind: EvidenceGapKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BehaviorExclusionVerdict {
    Satisfied,
    Prohibited,
    InsufficientEvidence,
}

/// Which abstract service each boundary machine in a module belongs to.
///
/// A boundary declaration retains only the service reach it spelled; the
/// trait that owns the requirement is not part of the Terminal row. Invoking
/// a boundary is an invocation of its owning service and of every parent in
/// the module's service closure, whatever the selected provider does, so the
/// join supplies this ownership from the checked trait declarations.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BoundaryServiceOwners {
    owners: BTreeMap<BoundaryMachineId, ServiceId>,
}

impl BoundaryServiceOwners {
    /// Join each boundary declaration to its owning service:
    /// `owning_service_identity` maps a boundary's canonical identity to the
    /// service identity its trait declares, resolved against the module's
    /// service catalog. Boundaries the map cannot name have no owner here.
    pub fn from_module(
        module: &TerminalModule,
        owning_service_identity: &dyn Fn(&str) -> Option<String>,
    ) -> Self {
        let mut owners = BTreeMap::new();
        for boundary in &module.boundary_machines {
            let Some(identity) = owning_service_identity(&boundary.identity) else {
                continue;
            };
            if let Some(service) = module
                .services
                .iter()
                .find(|service| service.identity == identity)
            {
                owners.insert(boundary.id, service.id);
            }
        }
        Self { owners }
    }

    pub fn insert(&mut self, boundary: BoundaryMachineId, service: ServiceId) {
        self.owners.insert(boundary, service);
    }

    pub fn owner(&self, boundary: BoundaryMachineId) -> Option<ServiceId> {
        self.owners.get(&boundary).copied()
    }

    /// The owning service and its transitive parents in `module`.
    fn invoked_services(
        &self,
        module: &TerminalModule,
        boundary: BoundaryMachineId,
    ) -> Vec<ServiceId> {
        let mut invoked = Vec::new();
        let Some(owner) = self.owner(boundary) else {
            return invoked;
        };
        let mut pending = vec![owner];
        while let Some(service) = pending.pop() {
            if invoked.contains(&service) {
                continue;
            }
            invoked.push(service);
            if let Some(declaration) = module.services.iter().find(|row| row.id == service) {
                pending.extend(declaration.parents.iter().copied());
            }
        }
        invoked
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BehaviorExclusionReport {
    pub prohibited: Vec<ProhibitedBehavior>,
    pub gaps: Vec<EvidenceGap>,
}

impl BehaviorExclusionReport {
    /// Prohibited wins when both lists are populated; both stay reported.
    pub fn verdict(&self) -> BehaviorExclusionVerdict {
        if !self.prohibited.is_empty() {
            BehaviorExclusionVerdict::Prohibited
        } else if !self.gaps.is_empty() {
            BehaviorExclusionVerdict::InsufficientEvidence
        } else {
            BehaviorExclusionVerdict::Satisfied
        }
    }
}

/// Reconstruct a conservative account of possible behavior for the exact
/// entry roster's static call closure in `module`, under the provider
/// realization `selected_provider_plans` admits for each reached boundary.
pub fn establish_behavior_exclusions(
    module: &TerminalModule,
    entries: &[MachineId],
    exclusions: &BehaviorExclusions,
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
) -> BehaviorExclusionReport {
    establish_behavior_exclusions_with_owners(
        module,
        entries,
        exclusions,
        selected_provider_plans,
        &BoundaryServiceOwners::default(),
    )
}

/// [`establish_behavior_exclusions`] with the boundary-to-service ownership
/// the join reconstructed: a boundary call then counts as an invocation of
/// its owning service and that service's parents, beside the fixed reach
/// the declaration spelled.
pub fn establish_behavior_exclusions_with_owners(
    module: &TerminalModule,
    entries: &[MachineId],
    exclusions: &BehaviorExclusions,
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
    boundary_owners: &BoundaryServiceOwners,
) -> BehaviorExclusionReport {
    let mut report = BehaviorExclusionReport::default();
    if exclusions.is_empty() {
        return report;
    }
    for &entry in entries {
        let Some(machine) = module.machines.iter().find(|machine| machine.id == entry) else {
            report.gaps.push(EvidenceGap {
                entry,
                machine: entry,
                block: None,
                operation: None,
                kind: EvidenceGapKind::UnknownEntry,
            });
            continue;
        };
        let mut visited = Vec::new();
        let mut pending = VecDeque::from([machine]);
        while let Some(machine) = pending.pop_front() {
            if visited.contains(&machine.id) {
                continue;
            }
            visited.push(machine.id);
            inspect_machine(
                module,
                entry,
                machine,
                exclusions,
                selected_provider_plans,
                boundary_owners,
                &mut report,
                &mut pending,
            );
        }
    }
    report
}

/// Which realization evidence covers one reached boundary: retained
/// candidate bodies to walk, and whether the declared contract must still
/// bound realizations the retained catalog cannot name.
fn boundary_realization_coverage(
    candidates: &[&ProviderCandidateConformance],
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
) -> (Vec<MachineId>, bool) {
    let mut bodies = Vec::new();
    let mut needs_contract = candidates.is_empty();
    let requirement_identities = {
        let mut identities: Vec<&str> = candidates
            .iter()
            .map(|candidate| candidate.requirement_identity.as_str())
            .collect();
        identities.sort_unstable();
        identities.dedup();
        identities
    };
    for requirement_identity in requirement_identities {
        let mut constrained = false;
        for plan in selected_provider_plans.plans() {
            for row in &plan.rows {
                if row.requirement_identity != requirement_identity {
                    continue;
                }
                constrained = true;
                match &row.binding {
                    effects::provider_plan::ProviderBinding::CheckedAdapter {
                        machine_identity,
                        ..
                    } => {
                        let mut matched = false;
                        for candidate in candidates.iter().filter(|candidate| {
                            candidate.requirement_identity == requirement_identity
                                && candidate.candidate_identity == *machine_identity
                        }) {
                            matched = true;
                            bodies.push(candidate.candidate);
                        }
                        if !matched {
                            // The selected adapter body is absent from the
                            // retained catalog; the contract is the only
                            // remaining bound on its possible behavior.
                            needs_contract = true;
                        }
                    }
                    _ => needs_contract = true,
                }
            }
        }
        if !constrained {
            // Nothing settles this requirement; the installation may still
            // admit any retained candidate or an external realization, so the
            // declared contract stays in force alongside every body.
            needs_contract = true;
            for candidate in candidates
                .iter()
                .filter(|candidate| candidate.requirement_identity == requirement_identity)
            {
                bodies.push(candidate.candidate);
            }
        }
    }
    (bodies, needs_contract)
}

/// The exact machine one bounded dynamic dispatch realizes, when the module's
/// dispatch custody pins it. Descriptor-parameter dispatches deliberately
/// carry no realization: their target arrives with the caller's table.
fn dynamic_dispatch_realization(
    module: &TerminalModule,
    owner: MachineId,
    operation: OperationId,
) -> Option<MachineId> {
    let dispatch = &module.dynamic_dispatch;
    dispatch
        .direct_dispatches
        .iter()
        .find(|row| row.owner == owner && row.operation == operation)
        .map(|row| row.realization)
        .or_else(|| {
            dispatch
                .indirect_dispatches
                .iter()
                .find(|row| row.owner == owner && row.operation == operation)
                .map(|row| row.realization)
        })
        .or_else(|| {
            dispatch
                .stored_dispatches
                .iter()
                .find(|row| row.owner == owner && row.operation == operation)
                .map(|row| row.realization)
        })
}

fn inspect_machine<'module>(
    module: &'module TerminalModule,
    entry: MachineId,
    machine: &'module TerminalMachine,
    exclusions: &BehaviorExclusions,
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
    boundary_owners: &BoundaryServiceOwners,
    report: &mut BehaviorExclusionReport,
    pending: &mut VecDeque<&'module TerminalMachine>,
) {
    for block in &machine.blocks {
        for operation in &block.operations {
            match &operation.kind {
                OperationKind::Call { callee, .. }
                | OperationKind::CallUnit { callee, .. }
                | OperationKind::CallStructuralScalar { callee, .. }
                | OperationKind::CallStructural { callee, .. }
                | OperationKind::CallStructuralWithScalarArguments { callee, .. } => {
                    match module.machines.iter().find(|target| target.id == *callee) {
                        Some(target) => pending.push_back(target),
                        None => report.gaps.push(EvidenceGap {
                            entry,
                            machine: machine.id,
                            block: Some(block.id),
                            operation: Some(operation.id),
                            kind: EvidenceGapKind::UnknownCallee(*callee),
                        }),
                    }
                }
                OperationKind::BoundaryCall { boundary, .. } => {
                    let Some(declaration) = module
                        .boundary_machines
                        .iter()
                        .find(|declaration| declaration.id == *boundary)
                    else {
                        report.gaps.push(EvidenceGap {
                            entry,
                            machine: machine.id,
                            block: Some(block.id),
                            operation: Some(operation.id),
                            kind: EvidenceGapKind::UnknownBoundary(*boundary),
                        });
                        continue;
                    };
                    let site = ProhibitedSite::BoundaryCall {
                        block: block.id,
                        operation: operation.id,
                        boundary: *boundary,
                    };
                    // The declaration's fixed reach and the service that owns
                    // the requirement (with its parents) are both invoked
                    // here, whatever the selected provider does.
                    let mut invoked = declaration.fixed_service_reach.clone();
                    for service in boundary_owners.invoked_services(module, *boundary) {
                        if !invoked.contains(&service) {
                            invoked.push(service);
                        }
                    }
                    for service in invoked {
                        if exclusions.excludes_service(service) {
                            report.prohibited.push(ProhibitedBehavior {
                                exclusion: BehaviorExclusion::Service(service),
                                entry,
                                machine: machine.id,
                                site: site.clone(),
                            });
                        }
                    }
                    let candidates: Vec<&ProviderCandidateConformance> = module
                        .provider_candidates
                        .iter()
                        .filter(|candidate| candidate.boundary == *boundary)
                        .collect();
                    let (realizations, needs_contract) =
                        boundary_realization_coverage(&candidates, selected_provider_plans);
                    if needs_contract {
                        for bucket in &declaration.crash_routes {
                            if exclusions.excludes_crash_cause(bucket.cause) {
                                report.prohibited.push(ProhibitedBehavior {
                                    exclusion: BehaviorExclusion::CrashCause(bucket.cause),
                                    entry,
                                    machine: machine.id,
                                    site: site.clone(),
                                });
                            }
                        }
                    }
                    for realization in realizations {
                        match module
                            .machines
                            .iter()
                            .find(|target| target.id == realization)
                        {
                            Some(target) => pending.push_back(target),
                            None => report.gaps.push(EvidenceGap {
                                entry,
                                machine: machine.id,
                                block: Some(block.id),
                                operation: Some(operation.id),
                                kind: EvidenceGapKind::UnknownCallee(realization),
                            }),
                        }
                    }
                }
                OperationKind::PortWrite { service, .. } => {
                    if exclusions.excludes_service(*service) {
                        report.prohibited.push(ProhibitedBehavior {
                            exclusion: BehaviorExclusion::Service(*service),
                            entry,
                            machine: machine.id,
                            site: ProhibitedSite::PortWrite {
                                block: block.id,
                                operation: operation.id,
                            },
                        });
                    }
                }
                OperationKind::CallDynamicScalar { .. }
                | OperationKind::CallDynamicParameterScalar { .. }
                | OperationKind::CallDynamicUnit { .. }
                | OperationKind::CallDynamicParameterUnit { .. } => {
                    match dynamic_dispatch_realization(module, machine.id, operation.id) {
                        Some(realization) => {
                            match module
                                .machines
                                .iter()
                                .find(|target| target.id == realization)
                            {
                                Some(target) => pending.push_back(target),
                                None => report.gaps.push(EvidenceGap {
                                    entry,
                                    machine: machine.id,
                                    block: Some(block.id),
                                    operation: Some(operation.id),
                                    kind: EvidenceGapKind::UnknownCallee(realization),
                                }),
                            }
                        }
                        None => report.gaps.push(EvidenceGap {
                            entry,
                            machine: machine.id,
                            block: Some(block.id),
                            operation: Some(operation.id),
                            kind: EvidenceGapKind::DynamicCall,
                        }),
                    }
                }
                _ => {}
            }
        }
        if let Terminator::Crash { cause, .. } = &block.terminator
            && exclusions.excludes_crash_cause(*cause)
        {
            report.prohibited.push(ProhibitedBehavior {
                exclusion: BehaviorExclusion::CrashCause(*cause),
                entry,
                machine: machine.id,
                site: ProhibitedSite::CrashTerminator { block: block.id },
            });
        }
    }
}

#[cfg(test)]
mod tests;
