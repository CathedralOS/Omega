/*
We define execution scope vocabulary (`ExecutionScope`) and establish
source-free selected-composition behavior exclusions (`BehaviorExclusions`). We
walk the pre-optimization semantic operations retained in canonical Terminal
Psi across the selected entry roster and selected provider-plan realizations so
optional optimizations never alter admissibility, and we adjudicate physical
authority exclusions against the reviewed mechanism closure without consulting
receiver permissions.
*/

use crate::TerminalAuthorityClass;
use diagnostics::Diagnostic;
pub use language_semantics::declaration_selection::{
    AuthoredBehaviorExclusion, AuthoredBehaviorExclusionKind,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, InvocationSite, MachineId, OperationId, ServiceId,
};
use std::collections::VecDeque;
use terminal_psi::{
    CrashCause, ProviderCandidateConformance, TargetMachineCallable, TargetMachineSupply,
    TerminalModule, UnoptimizedBehaviorOperation, UnoptimizedMachineBehavior,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionScope {
    CallerAddressSpace,
    IsolatedProvider(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BehaviorExclusion {
    CrashCause(CrashCause),
    Service(ServiceId),
    PhysicalAuthorityClass(TerminalAuthorityClass),
}

impl BehaviorExclusion {
    pub fn describe(self, module: &terminal_psi::TerminalModule) -> String {
        match self {
            Self::CrashCause(cause) => {
                format!("crash cause {cause:?}")
            }
            Self::Service(service) => match module.service(service) {
                Some(declaration) => format!(
                    "service `{}`",
                    module
                        .identities
                        .text(declaration.identity)
                        .unwrap_or_default()
                ),
                None => format!("service {service}"),
            },
            Self::PhysicalAuthorityClass(class) => {
                format!("physical authority class {class:?}")
            }
        }
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProhibitedSite {
    CrashTerminator {
        block: BlockId,
    },
    BoundaryCall {
        block: BlockId,
        site: InvocationSite,
        boundary: BoundaryMachineId,
    },
    CheckedInstruction {
        block: BlockId,
        operation: OperationId,
    },
}

impl ProhibitedSite {
    pub fn describe(&self, module: &terminal_psi::TerminalModule) -> String {
        match self {
            ProhibitedSite::CrashTerminator { block } => {
                format!("crash terminator in block {block}")
            }
            ProhibitedSite::BoundaryCall {
                block,
                site,
                boundary,
            } => {
                let identity =
                    terminal_psi::find_boundary_machine(&module.boundary_machines, *boundary)
                        .map(|declaration| {
                            module
                                .identities
                                .text(declaration.identity)
                                .unwrap_or("<undeclared boundary>")
                        })
                        .unwrap_or("<undeclared boundary>");
                match site {
                    semantic_vocabulary::InvocationSite::Operation(operation) => format!(
                        "call to boundary `{identity}` (block {block}, operation {operation})"
                    ),
                    semantic_vocabulary::InvocationSite::ExternalCompletion(edge) => format!(
                        "external completion to boundary `{identity}` (block {block}, edge {edge})"
                    ),
                    semantic_vocabulary::InvocationSite::InvokeNoNormal(edge) => {
                        format!("invocation of boundary `{identity}` (block {block}, edge {edge})")
                    }
                }
            }
            ProhibitedSite::CheckedInstruction { block, operation } => {
                format!("checked instruction (block {block}, operation {operation})")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProhibitedBehavior {
    pub exclusion: BehaviorExclusion,
    pub entry: MachineId,
    pub machine: MachineId,
    pub site: ProhibitedSite,
}

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
    MissingUnoptimizedBehavior(MachineId),
}

impl EvidenceGapKind {
    pub fn describe(&self) -> String {
        match self {
            EvidenceGapKind::UnknownCallee(callee) => {
                format!("call target machine {callee} is not retained in the module")
            }
            EvidenceGapKind::UnknownBoundary(boundary) => {
                format!("boundary {boundary} has no retained declaration")
            }
            EvidenceGapKind::UnknownBoundaryService(boundary) => {
                format!("invoked trait requirement {boundary} has no retained service owner")
            }
            EvidenceGapKind::UnknownEntry => {
                "the selected entry is not retained in the module".to_owned()
            }
            EvidenceGapKind::MissingUnoptimizedBehavior(machine) => {
                format!("machine {machine} has no retained unoptimized semantic operations")
            }
            EvidenceGapKind::TargetFamilySelection {
                head,
                target,
                reason,
            } => {
                format!(
                    "target-family head {head} has incomplete member evidence for deployment `{target}`: {reason}"
                )
            }
        }
    }
}

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
        if module.machine(entry).is_none() {
            report.gaps.push(EvidenceGap {
                entry,
                machine: entry,
                block: None,
                operation: None,
                kind: EvidenceGapKind::UnknownEntry,
            });
            continue;
        }
        let Some(machine) = module.unoptimized_machine_behavior(entry) else {
            report.gaps.push(EvidenceGap {
                entry,
                machine: entry,
                block: None,
                operation: None,
                kind: EvidenceGapKind::MissingUnoptimizedBehavior(entry),
            });
            continue;
        };
        let mut visited = Vec::new();
        let mut pending = VecDeque::from([machine]);
        while let Some(machine) = pending.pop_front() {
            if visited.contains(&machine.machine) {
                continue;
            }
            visited.push(machine.machine);
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
                            needs_contract = true;
                        }
                    }
                    _ => needs_contract = true,
                }
            }
        }
        if !constrained {
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
    machine: &'module UnoptimizedMachineBehavior,
    exclusions: &BehaviorExclusions,
    selected_provider_plans: &crate::SelectedProviderPlanFacts,
    deployment_target: &str,
    report: &mut BehaviorExclusionReport,
    pending: &mut VecDeque<&'module UnoptimizedMachineBehavior>,
) {
    for operation in &machine.operations {
        match *operation {
            UnoptimizedBehaviorOperation::Invocation {
                block,
                site,
                callee,
            } => {
                inspect_callee(
                    module,
                    entry,
                    machine.machine,
                    block,
                    site,
                    callee,
                    report,
                    pending,
                );
            }
            UnoptimizedBehaviorOperation::BoundaryInvocation {
                block,
                site,
                boundary,
            } => {
                inspect_boundary(
                    module,
                    entry,
                    machine.machine,
                    block,
                    site,
                    boundary,
                    exclusions,
                    selected_provider_plans,
                    deployment_target,
                    report,
                    pending,
                );
            }
            UnoptimizedBehaviorOperation::InstructionService {
                block,
                operation,
                service,
            } => {
                if exclusions.excludes_service(service) {
                    report.prohibited.push(ProhibitedBehavior {
                        exclusion: BehaviorExclusion::Service(service),
                        entry,
                        machine: machine.machine,
                        site: ProhibitedSite::CheckedInstruction { block, operation },
                    });
                }
            }
            UnoptimizedBehaviorOperation::Crash { block, cause } => {
                if exclusions.excludes_crash_cause(cause) {
                    report.prohibited.push(ProhibitedBehavior {
                        exclusion: BehaviorExclusion::CrashCause(cause),
                        entry,
                        machine: machine.machine,
                        site: ProhibitedSite::CrashTerminator { block },
                    });
                }
            }
            UnoptimizedBehaviorOperation::NominalCleanup { block, callee } => {
                enqueue_callee(
                    module,
                    entry,
                    machine.machine,
                    block,
                    None,
                    callee,
                    report,
                    pending,
                );
            }
        }
    }
}

fn inspect_callee<'module>(
    module: &'module TerminalModule,
    entry: MachineId,
    machine: MachineId,
    block: BlockId,
    invocation: InvocationSite,
    callee: MachineId,
    report: &mut BehaviorExclusionReport,
    pending: &mut VecDeque<&'module UnoptimizedMachineBehavior>,
) {
    enqueue_callee(
        module,
        entry,
        machine,
        block,
        invocation.operation(),
        callee,
        report,
        pending,
    );
}

fn enqueue_callee<'module>(
    module: &'module TerminalModule,
    entry: MachineId,
    machine: MachineId,
    block: BlockId,
    operation: Option<OperationId>,
    callee: MachineId,
    report: &mut BehaviorExclusionReport,
    pending: &mut VecDeque<&'module UnoptimizedMachineBehavior>,
) {
    let kind = if let Some(target) = module.unoptimized_machine_behavior(callee) {
        pending.push_back(target);
        return;
    } else if module.machine(callee).is_none() {
        EvidenceGapKind::UnknownCallee(callee)
    } else {
        EvidenceGapKind::MissingUnoptimizedBehavior(callee)
    };
    report.gaps.push(EvidenceGap {
        entry,
        machine,
        block: Some(block),
        operation,
        kind,
    });
}

fn inspect_boundary<'module>(
    module: &'module TerminalModule,
    entry: MachineId,
    machine: MachineId,
    block: BlockId,
    invocation: InvocationSite,
    called_boundary: BoundaryMachineId,
    exclusions: &BehaviorExclusions,
    selected_provider_plans: &crate::SelectedProviderPlanFacts,
    deployment_target: &str,
    report: &mut BehaviorExclusionReport,
    pending: &mut VecDeque<&'module UnoptimizedMachineBehavior>,
) {
    let selected_member = terminal_installation::resolve_target_machine_member(
        module,
        TargetMachineCallable::Boundary(called_boundary),
        deployment_target,
    );
    let family_gap = |reason| EvidenceGap {
        entry,
        machine,
        block: Some(block),
        operation: invocation.operation(),
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
                inspect_callee(
                    module, entry, machine, block, invocation, callee, report, pending,
                );
                return;
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
                return;
            }
        },
        Ok(None) => called_boundary,
        Err(error) => {
            report.gaps.push(family_gap(error.to_string()));
            return;
        }
    };
    let Some(declaration) =
        terminal_psi::find_boundary_machine(&module.boundary_machines, boundary)
    else {
        report.gaps.push(EvidenceGap {
            entry,
            machine,
            block: Some(block),
            operation: invocation.operation(),
            kind: EvidenceGapKind::UnknownBoundary(boundary),
        });
        return;
    };
    let site = ProhibitedSite::BoundaryCall {
        block,
        site: invocation,
        boundary,
    };
    if declaration.kind == terminal_psi::BoundaryRequirementKind::Trait
        && declaration.owning_service.is_none()
    {
        report.gaps.push(EvidenceGap {
            entry,
            machine,
            block: Some(block),
            operation: invocation.operation(),
            kind: EvidenceGapKind::UnknownBoundaryService(boundary),
        });
    }
    // Catalogue ownership identifies the interface, not what it reaches.
    // The published requirement ceiling remains binding even when a fixed
    // contribution is narrower. Include both retained rows and their parent
    // closure; neither a selected provider nor ownership can erase a promise.
    for service in invoked_services(
        module,
        declaration
            .published_service_ceiling
            .iter()
            .copied()
            .chain(declaration.fixed_service_reach.iter().copied()),
    ) {
        if exclusions.excludes_service(service) {
            report.prohibited.push(ProhibitedBehavior {
                exclusion: BehaviorExclusion::Service(service),
                entry,
                machine,
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
            return;
        }
    };
    if needs_contract {
        for bucket in &declaration.crash_routes {
            if exclusions.excludes_crash_cause(bucket.cause) {
                report.prohibited.push(ProhibitedBehavior {
                    exclusion: BehaviorExclusion::CrashCause(bucket.cause),
                    entry,
                    machine,
                    site: site.clone(),
                });
            }
        }
    }
    for realization in realizations {
        inspect_callee(
            module,
            entry,
            machine,
            block,
            invocation,
            realization,
            report,
            pending,
        );
    }
}

fn invoked_services(
    module: &TerminalModule,
    roots: impl IntoIterator<Item = ServiceId>,
) -> Vec<ServiceId> {
    let mut invoked = Vec::new();
    let mut pending: Vec<_> = roots.into_iter().collect();
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

pub fn resolve_authored_behavior_exclusion(
    row: &AuthoredBehaviorExclusion,
    module: &TerminalModule,
) -> Option<BehaviorExclusion> {
    match &row.kind {
        AuthoredBehaviorExclusionKind::CrashCause { cause, .. } => {
            Some(BehaviorExclusion::CrashCause(*cause))
        }
        AuthoredBehaviorExclusionKind::PhysicalAuthorityClass { class, .. } => {
            Some(BehaviorExclusion::PhysicalAuthorityClass(*class))
        }
        AuthoredBehaviorExclusionKind::Service { identity, .. } => module
            .services
            .iter()
            .find(|service| module.identities.text(service.identity) == Some(identity.as_str()))
            .map(|service| BehaviorExclusion::Service(service.id)),
    }
}

pub fn authored_behavior_exclusion_set_in(
    exclusions: &[AuthoredBehaviorExclusion],
    module: &TerminalModule,
) -> BehaviorExclusions {
    BehaviorExclusions::from_selections(
        exclusions
            .iter()
            .filter_map(|row| resolve_authored_behavior_exclusion(row, module)),
    )
}

#[cfg(test)]
mod boundary_reach_tests {
    use super::*;
    use terminal_psi::{
        BoundaryCompletion, BoundaryDeclarationRole, BoundaryMachineDeclaration,
        BoundaryMachineResult, BoundaryRequirementKind, NominalDeclarationIdentity,
        ServiceDeclaration, TerminalIdentityTableBuilder,
    };

    fn module() -> TerminalModule {
        let mut module = TerminalModule::for_entry(MachineId::new(1).unwrap());
        let mut identities = TerminalIdentityTableBuilder::default();
        let owner = identities.intern("package:owner::Interface");
        let parent = identities.intern("package:other::Parent");
        let child = identities.intern("package:other::Child");
        let callable = identities.intern("package:owner::Interface::read()");
        module.identities = identities.build();
        module.services = vec![
            ServiceDeclaration {
                id: ServiceId::new(1).unwrap(),
                identity: owner,
                parents: Vec::new(),
            },
            ServiceDeclaration {
                id: ServiceId::new(2).unwrap(),
                identity: parent,
                parents: Vec::new(),
            },
            ServiceDeclaration {
                id: ServiceId::new(3).unwrap(),
                identity: child,
                parents: vec![ServiceId::new(2).unwrap()],
            },
        ];
        module.boundary_machines.push(BoundaryMachineDeclaration {
            id: BoundaryMachineId::new(1).unwrap(),
            declaration: None,
            numeric_intrinsic: None,
            role: BoundaryDeclarationRole::Callable,
            kind: BoundaryRequirementKind::Trait,
            completion: BoundaryCompletion::NormalReturn,
            unconditional_termination: false,
            identity: callable,
            owner: NominalDeclarationIdentity::new(owner),
            attachment: None,
            parameter_order: Vec::new(),
            scalar_parameters: Vec::new(),
            crash_routes: Vec::new(),
            structural_parameters: Vec::new(),
            result: BoundaryMachineResult::Unit,
            result_reference_regions: Vec::new(),
            requires: Vec::new(),
            normal_qualifications: Vec::new(),
            scalar_domain_requirements: Vec::new(),
            scalar_requires: Vec::new(),
            scalar_ensures: Vec::new(),
            reference_outputs: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            fixed_service_reach: Vec::new(),
            owning_service: Some(ServiceId::new(1).unwrap()),
            published_synchronous_entries: Vec::new(),
            published_service_ceiling: Vec::new(),
        });
        module
    }

    fn inspect(module: &TerminalModule, excluded: ServiceId) -> BehaviorExclusionReport {
        let mut report = BehaviorExclusionReport::default();
        inspect_boundary(
            module,
            MachineId::new(1).unwrap(),
            MachineId::new(1).unwrap(),
            BlockId::new(1).unwrap(),
            InvocationSite::Operation(OperationId::new(1).unwrap()),
            BoundaryMachineId::new(1).unwrap(),
            &BehaviorExclusions::from_selections([BehaviorExclusion::Service(excluded)]),
            &crate::SelectedProviderPlanFacts::default(),
            "linux_x86_64",
            &mut report,
            &mut VecDeque::new(),
        );
        report
    }

    #[test]
    fn boundary_exclusions_follow_declared_reach_without_implicit_ownership() {
        let mut module = module();
        let owner = ServiceId::new(1).unwrap();
        let empty = inspect(&module, owner);
        assert!(empty.prohibited.is_empty(), "{empty:?}");
        assert!(empty.gaps.is_empty(), "{empty:?}");
        module.boundary_machines[0].fixed_service_reach.push(owner);
        let reached = inspect(&module, owner);
        assert!(
            reached
                .prohibited
                .iter()
                .any(|row| row.exclusion == BehaviorExclusion::Service(owner)),
            "{reached:?}"
        );
        module.boundary_machines[0].fixed_service_reach.clear();
        module.boundary_machines[0]
            .fixed_service_reach
            .push(ServiceId::new(3).unwrap());
        let reached = inspect(&module, ServiceId::new(2).unwrap());
        assert!(
            reached
                .prohibited
                .iter()
                .any(|row| row.exclusion == BehaviorExclusion::Service(ServiceId::new(2).unwrap())),
            "{reached:?}"
        );
        assert!(
            inspect(&module, ServiceId::new(1).unwrap())
                .prohibited
                .is_empty()
        );
        module.boundary_machines[0].fixed_service_reach.clear();
        module.boundary_machines[0]
            .published_service_ceiling
            .push(owner);
        let report = inspect(&module, owner);
        assert!(
            report
                .prohibited
                .iter()
                .any(|row| row.exclusion == BehaviorExclusion::Service(owner)),
            "{report:?}"
        );
    }
}
