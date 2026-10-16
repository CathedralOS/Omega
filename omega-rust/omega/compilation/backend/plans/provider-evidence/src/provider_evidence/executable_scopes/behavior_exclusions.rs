//! Selected-composition exclusions and source-free absence evidence.
//!
//! A build may forbid exact behavior — a crash cause, an abstract boundary
//! service, or a physical terminal-authority class — in the selected
//! executable composition, independently of the conservative public contracts
//! the same declarations publish. This is what
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
//! realize. Exact service ownership is a Terminal declaration field; neither
//! source declarations nor parsing display identities supplies that relationship.
//!
//! A boundary call's own fixed service reach always counts: the requirement
//! invocation contributes it no matter which provider the installation picks.
//! Its declared crash routes count only while realization coverage is
//! incomplete — a retained candidate the selected plan names by exact
//! `CheckedAdapter` identity supplies a verified body, which is stronger
//! evidence than the published ceiling it refined. An unconstrained slot
//! admits every retained candidate plus the contract any external provider
//! could only conform to.
//!
//! Physical exclusions retain the backend's existing authority classes. The
//! semantic walk demands a bounded call closure; `BehaviorExclusions` then
//! checks settled mechanism evidence without consulting receiver permissions.
//! Build authoring and source-spanned selection provenance remain with build
//! evaluation. Neither semantic nor physical checking executes the Build.
//! `attribution` owns human descriptions of retained exclusion evidence.

mod attribution;

use crate::TerminalAuthorityClass;
use diagnostics::Diagnostic;
use semantic_vocabulary::{BlockId, BoundaryMachineId, MachineId, OperationId, ServiceId};
use std::collections::VecDeque;
use terminal_psi::{
    CrashCause, OperationKind, ProviderCandidateConformance, TargetMachineCallable,
    TargetMachineSupply, TerminalMachine, TerminalModule, Terminator,
};

/// One exact exclusion selected by the build root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BehaviorExclusion {
    CrashCause(CrashCause),
    Service(ServiceId),
    /// No admitted mechanism may exercise this class of D45's closed
    /// terminal-authority vocabulary. The semantic walk records the
    /// selection and its bounded-closure evidence but cannot adjudicate
    /// mechanisms; native realization checks it against the retained
    /// mechanism-closure review's exercised dispositions.
    PhysicalAuthorityClass(TerminalAuthorityClass),
}

/// Canonical union of the root's selected exclusions: sorted, deduplicated,
/// so reordered or repeated selections compare equal.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BehaviorExclusions {
    crash_causes: Vec<CrashCause>,
    services: Vec<ServiceId>,
    physical_authority_classes: Vec<TerminalAuthorityClass>,
}

impl BehaviorExclusions {
    pub(crate) fn from_selections(selections: impl IntoIterator<Item = BehaviorExclusion>) -> Self {
        let mut exclusions = Self::default();
        for selection in selections {
            match selection {
                BehaviorExclusion::CrashCause(cause) => exclusions.crash_causes.push(cause),
                BehaviorExclusion::Service(service) => exclusions.services.push(service),
                BehaviorExclusion::PhysicalAuthorityClass(class) => {
                    exclusions.physical_authority_classes.push(class)
                }
            }
        }
        exclusions.sort_and_deduplicate();
        exclusions
    }

    /// Set union; a later selection never removes an earlier restriction.
    pub fn union(&mut self, other: &Self) {
        self.crash_causes.extend_from_slice(&other.crash_causes);
        self.services.extend_from_slice(&other.services);
        self.physical_authority_classes
            .extend_from_slice(&other.physical_authority_classes);
        self.sort_and_deduplicate();
    }

    pub fn is_empty(&self) -> bool {
        self.crash_causes.is_empty()
            && self.services.is_empty()
            && self.physical_authority_classes.is_empty()
    }

    pub(crate) fn excludes_crash_cause(&self, cause: CrashCause) -> bool {
        self.crash_causes.contains(&cause)
    }

    pub(crate) fn excludes_service(&self, service: ServiceId) -> bool {
        self.services.contains(&service)
    }

    /// Whether this union excludes one physical authority class: the native
    /// mechanism-closure review then rejects every admitted mechanism whose
    /// exercised disposition contains it, independently of any receiver
    /// permission policy.
    pub(crate) fn excludes_physical_authority_class(&self, class: TerminalAuthorityClass) -> bool {
        self.physical_authority_classes.contains(&class)
    }

    pub fn crash_causes(&self) -> &[CrashCause] {
        &self.crash_causes
    }

    pub fn services(&self) -> &[ServiceId] {
        &self.services
    }

    pub(crate) fn physical_authority_classes(&self) -> &[TerminalAuthorityClass] {
        &self.physical_authority_classes
    }

    /// Adjudicate the physical axis of one retained exclusion union against the
    /// mechanism-closure review provider admission retained for this artifact.
    ///
    /// Every exercised class of every closure leaf must stay outside the
    /// requested exclusion set; a leaf admitted without a permission policy is
    /// adjudicated identically — classification, not receiver permission, answers
    /// the absence question. Crash-cause and service exclusions are semantic
    /// verdicts established upstream and do not consult mechanisms here.
    ///
    /// The caller must bind and verify the review for its exact artifact,
    /// target and provider closure first. This comparison does not establish
    /// the authority of a caller-constructed review receipt.
    pub fn validate_physical_authority_closure(
        &self,
        closure_review: &crate::TerminalAuthorityClosureReviewReceipt,
    ) -> Result<(), Vec<Diagnostic>> {
        if self.physical_authority_classes().is_empty() {
            return Ok(());
        }
        let mut violations = Vec::new();
        for leaf in closure_review.leaves() {
            let Some(exercised) = leaf.exercised() else {
                violations.push(format!(
                    "authority subject `{}` has no physical authority classification for mechanism {:?}; requested exclusions cannot be established",
                    closure_review
                        .identities()
                        .text(leaf.subject_identity())
                        .unwrap_or_default(),
                    leaf.mechanism(),
                ));
                continue;
            };
            for &class in exercised.classes() {
                if self.excludes_physical_authority_class(class) {
                    violations.push(format!(
                        "authority subject `{}` exercises excluded physical authority class {class:?} through mechanism {:?}",
                        closure_review
                            .identities()
                            .text(leaf.subject_identity())
                            .unwrap_or_default(),
                        leaf.mechanism(),
                    ));
                }
            }
        }
        if violations.is_empty() {
            return Ok(());
        }
        let mut diagnostics = vec![Diagnostic::error(
            "requested behavior exclusions could not be satisfied by the reviewed mechanism closure"
                .to_owned(),
        )];
        diagnostics.extend(violations.into_iter().map(Diagnostic::error));
        Err(diagnostics)
    }

    fn sort_and_deduplicate(&mut self) {
        self.crash_causes.sort_unstable();
        self.crash_causes.dedup();
        self.services.sort_unstable();
        self.services.dedup();
        self.physical_authority_classes.sort_unstable();
        self.physical_authority_classes.dedup();
    }
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
    CheckedInstruction {
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
    UnknownCallee(MachineId),
    UnknownBoundary(BoundaryMachineId),
    UnknownBoundaryService(BoundaryMachineId),
    TargetFamilySelection {
        head: BoundaryMachineId,
        target: String,
        reason: String,
    },
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
/// `deployment_target` is the selected exact tag, not a host or ISA projection;
/// independent family calls contribute only that member's behavior.
/// A boundary call counts as invocation of its declared owning service and
/// that service's parents, beside its fixed reach. No source joins are needed.
/// Executable edge work joins the closure too:
/// a nominal affine cleanup commits a selected machine's body on the edge,
/// so the walk follows `cleanup_machine` targets like static calls and
/// treats an absent one as an evidence gap rather than trusted inertness.
pub fn establish_behavior_exclusions(
    module: &TerminalModule,
    entries: &[MachineId],
    exclusions: &BehaviorExclusions,
    selected_provider_plans: &crate::SelectedProviderPlanFacts,
    deployment_target: &str,
) -> BehaviorExclusionReport {
    let mut report = BehaviorExclusionReport::default();
    if exclusions.is_empty() {
        return report;
    }
    for &entry in entries {
        let Some(machine) = module.machine(entry) else {
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
                deployment_target,
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
    module: &TerminalModule,
    boundary: BoundaryMachineId,
    candidates: &[&ProviderCandidateConformance],
    selected_provider_plans: &crate::SelectedProviderPlanFacts,
    deployment_target: &str,
) -> Result<(Vec<MachineId>, bool), String> {
    let mut bodies = Vec::new();
    let mut needs_contract = candidates.is_empty();
    let requirement_identities = {
        let mut identities: Vec<&str> = candidates
            .iter()
            .map(|candidate| {
                module
                    .identities
                    .text(candidate.requirement_identity)
                    .unwrap_or_default()
            })
            .collect();
        identities.sort_unstable();
        identities.dedup();
        identities
    };
    for requirement_identity in requirement_identities {
        if let Some(family_row) =
            selected_provider_plans.family_realization_row(requirement_identity)
        {
            let head = module.boundary_machine(family_row.head);
            let member = module.boundary_machine(family_row.member);
            let selected_member = terminal_installation::resolve_target_machine_member(
                module,
                TargetMachineCallable::Boundary(family_row.head),
                deployment_target,
            )
            .map_err(|error| error.to_string())?;
            if family_row.target != deployment_target
                || head.is_none_or(|declaration| {
                    module.identities.text(declaration.identity)
                        != selected_provider_plans
                            .identities()
                            .text(family_row.head_identity)
                })
                || member.is_none_or(|declaration| {
                    module.identities.text(declaration.identity)
                        != selected_provider_plans
                            .identities()
                            .text(family_row.member_identity)
                })
                || selected_member.is_none_or(|selected| {
                    selected.callable != TargetMachineCallable::Boundary(family_row.member)
                        || selected.supply == TargetMachineSupply::CheckedBody
                        || family_row.member != boundary
                })
            {
                return Err("selected provider family row disagrees with the exact catalog member or deployment".to_owned());
            }
        }
        let mut constrained = false;
        // Terminal candidates name closed requirement identities and
        // specialized provider instances; a template-keyed selected row
        // reaches them through the specialization roster.
        let specialization =
            selected_provider_plans.specialized_requirement_row(requirement_identity);
        for plan in selected_provider_plans.plans() {
            if plan.target != deployment_target {
                continue;
            }
            if specialization
                .is_some_and(|row| row.provider_plan_report_identity != plan.report_fingerprint())
            {
                continue;
            }
            for row in &plan.rows {
                let row_matches = selected_provider_plans.selected_row_matches_requirement(
                    requirement_identity,
                    plan.identities.text(row.requirement_identity),
                    plan.report_fingerprint(),
                );
                if !row_matches {
                    continue;
                }
                constrained = true;
                match &row.binding {
                    crate::provider_plan::ProviderBinding::CheckedAdapter {
                        machine_identity,
                        ..
                    } => {
                        let realization_identity = specialization
                            .and_then(|row| {
                                selected_provider_plans
                                    .identities()
                                    .text(row.machine_identity)
                            })
                            .unwrap_or_else(|| plan.identity_text(*machine_identity));
                        let mut matched = false;
                        for candidate in candidates.iter().filter(|candidate| {
                            module.identities.text(candidate.requirement_identity)
                                == Some(requirement_identity)
                                && module.identities.text(candidate.candidate_identity)
                                    == Some(realization_identity)
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
            for candidate in candidates.iter().filter(|candidate| {
                module.identities.text(candidate.requirement_identity) == Some(requirement_identity)
            }) {
                bodies.push(candidate.candidate);
            }
        }
    }
    Ok((bodies, needs_contract))
}

fn inspect_machine<'module>(
    module: &'module TerminalModule,
    entry: MachineId,
    machine: &'module TerminalMachine,
    exclusions: &BehaviorExclusions,
    selected_provider_plans: &crate::SelectedProviderPlanFacts,
    deployment_target: &str,
    report: &mut BehaviorExclusionReport,
    pending: &mut VecDeque<&'module TerminalMachine>,
) {
    for block in &machine.blocks {
        for operation in &block.operations {
            if let Some(callee) = operation.kind.direct_callee() {
                match module.machine(callee) {
                    Some(target) => pending.push_back(target),
                    None => report.gaps.push(EvidenceGap {
                        entry,
                        machine: machine.id,
                        block: Some(block.id),
                        operation: Some(operation.id),
                        kind: EvidenceGapKind::UnknownCallee(callee),
                    }),
                }
            }
            match &operation.kind {
                OperationKind::BoundaryCall { boundary, .. } => {
                    // A generated head is the portable checking contract, not
                    // this deployment's executable behavior. Resolve before
                    // accounting ordinary opaque service/provider contracts.
                    let called_boundary = *boundary;
                    let selected_member = terminal_installation::resolve_target_machine_member(
                        module,
                        TargetMachineCallable::Boundary(called_boundary),
                        deployment_target,
                    );
                    let family_gap = |reason| EvidenceGap {
                        entry,
                        machine: machine.id,
                        block: Some(block.id),
                        operation: Some(operation.id),
                        kind: EvidenceGapKind::TargetFamilySelection {
                            head: called_boundary,
                            target: deployment_target.to_owned(),
                            reason,
                        },
                    };
                    let boundary = match selected_member {
                        Ok(Some(member)) => match (member.callable, member.supply) {
                            (
                                TargetMachineCallable::Machine(callee),
                                TargetMachineSupply::CheckedBody | TargetMachineSupply::Boundary,
                            ) => {
                                match module.machine(callee) {
                                    Some(target) => pending.push_back(target),
                                    None => report.gaps.push(EvidenceGap {
                                        entry,
                                        machine: machine.id,
                                        block: Some(block.id),
                                        operation: Some(operation.id),
                                        kind: EvidenceGapKind::UnknownCallee(callee),
                                    }),
                                }
                                continue;
                            }
                            (TargetMachineCallable::Boundary(member), supply)
                                if supply != TargetMachineSupply::CheckedBody =>
                            {
                                member
                            }
                            _ => {
                                report.gaps.push(family_gap(format!(
                                    "selected callable {:?} has unsupported supply {:?}",
                                    member.callable, member.supply,
                                )));
                                continue;
                            }
                        },
                        Ok(None) => called_boundary,
                        Err(error) => {
                            report.gaps.push(family_gap(error.to_string()));
                            continue;
                        }
                    };
                    let Some(declaration) =
                        terminal_psi::find_boundary_machine(&module.boundary_machines, boundary)
                    else {
                        report.gaps.push(EvidenceGap {
                            entry,
                            machine: machine.id,
                            block: Some(block.id),
                            operation: Some(operation.id),
                            kind: EvidenceGapKind::UnknownBoundary(boundary),
                        });
                        continue;
                    };
                    let site = ProhibitedSite::BoundaryCall {
                        block: block.id,
                        operation: operation.id,
                        boundary,
                    };
                    if declaration.kind == terminal_psi::BoundaryRequirementKind::Trait
                        && declaration.owning_service.is_none()
                    {
                        report.gaps.push(EvidenceGap {
                            entry,
                            machine: machine.id,
                            block: Some(block.id),
                            operation: Some(operation.id),
                            kind: EvidenceGapKind::UnknownBoundaryService(boundary),
                        });
                    }
                    // The declaration's fixed reach and the service that owns
                    // the requirement (with its parents) are both invoked
                    // here, whatever the selected provider does.
                    let mut invoked = declaration.fixed_service_reach.clone();
                    for service in invoked_services(module, declaration.owning_service) {
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
                        .filter(|candidate| candidate.boundary == boundary)
                        .collect();
                    let coverage = boundary_realization_coverage(
                        module,
                        boundary,
                        &candidates,
                        selected_provider_plans,
                        deployment_target,
                    );
                    let (realizations, needs_contract) = match coverage {
                        Ok(coverage) => coverage,
                        Err(reason) => {
                            report.gaps.push(family_gap(reason));
                            continue;
                        }
                    };
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
                        match module.machine(realization) {
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
                OperationKind::PortWrite { service, .. }
                | OperationKind::RegisterAccess { service, .. } => {
                    if exclusions.excludes_service(*service) {
                        report.prohibited.push(ProhibitedBehavior {
                            exclusion: BehaviorExclusion::Service(*service),
                            entry,
                            machine: machine.id,
                            site: ProhibitedSite::CheckedInstruction {
                                block: block.id,
                                operation: operation.id,
                            },
                        });
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
        // An executable nominal cleanup commits a selected machine's body
        // when the edge commits; it is a call edge carried on the terminator
        // rather than an operation, so its possible behavior joins the
        // closure exactly like a static call. Claim-free discards carry no
        // code and are not invocations.
        for cleanup_machine in nominal_cleanup_machines(&block.terminator) {
            match module.machine(cleanup_machine) {
                Some(target) => pending.push_back(target),
                None => report.gaps.push(EvidenceGap {
                    entry,
                    machine: machine.id,
                    block: Some(block.id),
                    operation: None,
                    kind: EvidenceGapKind::UnknownCallee(cleanup_machine),
                }),
            }
        }
    }
}

/// Cleanup dependencies on every possible edge join behavior reach. This
/// enumerates possible calls; it does not execute unselected successor actions.
fn nominal_cleanup_machines(terminator: &Terminator) -> impl Iterator<Item = MachineId> {
    terminator
        .cleanup_rosters()
        .flat_map(|(_, actions)| actions)
        .filter_map(|action| match action {
            terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                Some(cleanup.cleanup_machine)
            }
            terminal_psi::TerminalAffineCleanupAction::DiscardRoot(_)
            | terminal_psi::TerminalAffineCleanupAction::RetireHome(_)
            | terminal_psi::TerminalAffineCleanupAction::DiscardResidual(_) => None,
        })
}

/// Nominal invocation contributes the exact owner and its transitive parents.
fn invoked_services(module: &TerminalModule, owner: Option<ServiceId>) -> Vec<ServiceId> {
    let mut invoked = Vec::new();
    let mut pending: Vec<_> = owner.into_iter().collect();
    while let Some(service) = pending.pop() {
        if invoked.contains(&service) {
            continue;
        }
        invoked.push(service);
        if let Some(declaration) = module.service(service) {
            pending.extend(declaration.parents.iter().copied());
        }
    }
    invoked
}

mod authored;
pub use authored::{
    AuthoredBehaviorExclusion, AuthoredBehaviorExclusionKind, authored_behavior_exclusion_set_in,
};
