//! Inspect the pre-optimization operations reachable under this target's selected
//! provider plans. Keep a prohibited behavior distinct from a gap in retained evidence.

use crate::{BehaviorExclusion, BehaviorExclusions};
use diagnostics::Diagnostic;
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, InvocationSite, MachineId, OperationId, ServiceId,
};
use std::collections::VecDeque;
use terminal_psi::{
    ProviderCandidateConformance, TerminalModule, UnoptimizedBehaviorOperation,
    UnoptimizedMachineBehavior,
};

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
                    terminal_psi::find_boundary_machine(&module.interfaces.boundary_machines, *boundary)
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
                    semantic_vocabulary::InvocationSite::Cleanup {
                        edge,
                        action_ordinal,
                    } => {
                        format!(
                            "cleanup invocation of boundary `{identity}` (block {block}, edge {edge}, action {action_ordinal})"
                        )
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
    ProviderRealization {
        boundary: BoundaryMachineId,
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
            EvidenceGapKind::ProviderRealization {
                boundary,
                target,
                reason,
            } => {
                format!(
                    "boundary {boundary} has incomplete provider evidence for deployment `{target}`: {reason}"
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

impl BehaviorExclusions {
    pub fn has_semantic_exclusions(&self) -> bool {
        !self.crash_causes().is_empty() || !self.services().is_empty()
    }

    /// Check every admitted machine, including dynamically reached private
    /// entries. Without that roster a clean whole module is sufficient evidence.
    pub fn validate_semantic_scope(
        &self,
        module: &TerminalModule,
        entry: MachineId,
        selected_machines: Option<&[MachineId]>,
        providers: &selected_provider_plans::SelectedProviderPlanFacts,
        deployment_target: &str,
    ) -> Result<(), Vec<Diagnostic>> {
        if !self.has_semantic_exclusions() {
            return Ok(());
        }
        let check = |entries: &[MachineId]| {
            establish_behavior_exclusions(module, entries, self, providers, deployment_target)
        };
        if let Some(machines) = selected_machines {
            if machines.binary_search(&entry).is_err() {
                return Err(vec![Diagnostic::error(
                    "selected behavior scope does not cover its entry",
                )]);
            }
            return check(machines).require_satisfied(module);
        }
        check(&[entry]).require_satisfied(module)?;
        let remaining = module
            .machines
            .iter()
            .map(|machine| machine.id)
            .filter(|machine| *machine != entry)
            .collect::<Vec<_>>();
        let report = check(&remaining);
        if report.verdict() != BehaviorExclusionVerdict::Satisfied {
            return Err(vec![Diagnostic::error(
                "behavior exclusions need the selected execution scope because the whole module cannot establish their absence",
            )]);
        }
        Ok(())
    }
}

impl BehaviorExclusionReport {
    fn require_satisfied(self, module: &TerminalModule) -> Result<(), Vec<Diagnostic>> {
        if self.verdict() == BehaviorExclusionVerdict::Satisfied {
            return Ok(());
        }
        let mut diagnostics = Vec::new();
        for finding in self.prohibited {
            let mut exclusion = finding.exclusion.describe(module);
            let mut qualified_service = String::new();
            if let BehaviorExclusion::Service(id) = finding.exclusion
                && let Some(identity) = module
                    .service(id)
                    .and_then(|row| module.identities.text(row.identity))
                && let Some((_, name)) = identity.rsplit_once("::")
            {
                // The short label is only for display. Always report the complete
                // declaration identity when two services could share that label.
                exclusion = format!("service `{name}`");
                qualified_service = format!(" (declaration `{identity}`)");
            }
            diagnostics.push(Diagnostic::error(format!(
                "behavior exclusion violated: {exclusion} is reachable in Terminal machine {} from entry {} at {}{qualified_service}",
                finding.machine, finding.entry, finding.site.describe(module)
            )));
        }
        for gap in self.gaps {
            diagnostics.push(Diagnostic::error(format!(
                "cannot establish behavior exclusions for entry {} in machine {}: {}",
                gap.entry,
                gap.machine,
                gap.kind.describe()
            )));
        }
        Err(diagnostics)
    }
}

pub fn establish_behavior_exclusions(
    module: &TerminalModule,
    entries: &[MachineId],
    exclusions: &BehaviorExclusions,
    selected_provider_plans: &selected_provider_plans::SelectedProviderPlanFacts,
    deployment_target: &str,
) -> BehaviorExclusionReport {
    let mut report = BehaviorExclusionReport::default();
    if !exclusions.has_semantic_exclusions() {
        return report;
    }
    let mut visited = std::collections::BTreeSet::new();
    for &entry in entries {
        if visited.contains(&entry) {
            continue;
        }
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
        let mut pending = VecDeque::from([machine]);
        while let Some(machine) = pending.pop_front() {
            if !visited.insert(machine.machine) {
                continue;
            }
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
    candidates: &[&ProviderCandidateConformance],
    selected_provider_plans: &selected_provider_plans::SelectedProviderPlanFacts,
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
                    provider_plans::ProviderBinding::CheckedAdapter {
                        machine_identity, ..
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
    selected_provider_plans: &selected_provider_plans::SelectedProviderPlanFacts,
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
    selected_provider_plans: &selected_provider_plans::SelectedProviderPlanFacts,
    deployment_target: &str,
    report: &mut BehaviorExclusionReport,
    pending: &mut VecDeque<&'module UnoptimizedMachineBehavior>,
) {
    let boundary = called_boundary;
    let Some(declaration) =
        terminal_psi::find_boundary_machine(&module.interfaces.boundary_machines, boundary)
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
        .interfaces.provider_candidates
        .iter()
        .filter(|candidate| candidate.boundary == boundary)
        .collect();
    let coverage = boundary_realization_coverage(
        module,
        &candidates,
        selected_provider_plans,
        deployment_target,
    );
    let (realizations, needs_contract) = match coverage {
        Ok(coverage) => coverage,
        Err(reason) => {
            report.gaps.push(EvidenceGap {
                entry,
                machine,
                block: Some(block),
                operation: invocation.operation(),
                kind: EvidenceGapKind::ProviderRealization {
                    boundary,
                    target: deployment_target.to_owned(),
                    reason,
                },
            });
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
        module.interfaces.services = vec![
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
        module.interfaces.boundary_machines.push(BoundaryMachineDeclaration {
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
            result_lifetimes: None,
            requires: Vec::new(),
            membership_ensures: Vec::new(),
            normal_qualifications: Vec::new(),
            scalar_domain_requirements: Vec::new(),
            scalar_requires: Vec::new(),
            scalar_ensures: Vec::new(),
            reference_outputs: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            fixed_service_reach: Vec::new(),
            service_reach_is_installation_bound: false,
            owning_service: Some(ServiceId::new(1).unwrap()),
            published_synchronous_entries: Vec::new(),
            published_service_ceiling: Vec::new(),
        });
        module
    }

    #[test]
    fn service_exclusions_require_one_exact_declaration() {
        let mut module = module();
        let identity = "package:other::Child";
        assert_eq!(
            crate::service_for_exclusion(&module, identity),
            Ok(ServiceId::new(3).unwrap())
        );
        assert!(crate::service_for_exclusion(&module, "Child").is_err());
        assert!(crate::service_for_exclusion(&module, "package:absent::Child").is_err());
        let mut duplicate = module.interfaces.services[2].clone();
        duplicate.id = ServiceId::new(4).unwrap();
        module.interfaces.services.push(duplicate);
        assert!(crate::service_for_exclusion(&module, identity).is_err());
    }

    #[test]
    fn native_admission_checks_selected_preoptimization_bodies_and_missing_evidence() {
        use semantic_vocabulary::{ContractId, CrashCause, EdgeId};
        use terminal_psi::{
            Block, MachineContract, TerminalMachine, Terminator, UnoptimizedMachineBehavior,
        };

        let entry = MachineId::new(1).unwrap();
        let private = MachineId::new(2).unwrap();
        let block = BlockId::new(1).unwrap();
        let edge = EdgeId::new(1).unwrap();
        let machine = |id, terminator| TerminalMachine {
            call_formations: Vec::new(),
            assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
            id,
            attachment: None,
            ranked_scc: None,
            structural_places: Vec::new(),
            block_entry_claims: Vec::new(),
            loan_completions: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block,
            blocks: vec![Block::plain(block, Vec::new(), terminator)],
            contract: MachineContract::empty(ContractId::new(1).unwrap()),
        };
        let mut module = TerminalModule::for_entry(entry);
        module.machines = vec![
            machine(
                entry,
                Terminator::ReturnUnit {
                    edge,
                    cleanup_actions: Vec::new(),
                },
            ),
            machine(
                private,
                Terminator::Crash {
                    edge,
                    cause: CrashCause::Trap,
                    site_guard: Vec::new(),
                    frontier_lower_bound: Vec::new(),
                },
            ),
        ];
        module.execution.unoptimized_behavior = module
            .machines
            .iter()
            .map(UnoptimizedMachineBehavior::capture)
            .collect();
        // Native optimization has removed the Trap. The selected composition must
        // still answer for the original operation retained in the published input.
        module.machines[1].blocks[0].terminator = Terminator::ReturnUnit {
            edge,
            cleanup_actions: Vec::new(),
        };
        let exclusions =
            BehaviorExclusions::from_selections([BehaviorExclusion::CrashCause(CrashCause::Trap)]);
        let providers = selected_provider_plans::SelectedProviderPlanFacts::default();
        let check = |module: &TerminalModule, scope: Option<&[MachineId]>| {
            exclusions.validate_semantic_scope(module, entry, scope, &providers, "linux_x86_64")
        };
        assert!(check(&module, Some(&[entry])).is_ok());
        let diagnostic = check(&module, Some(&[entry, private])).unwrap_err();
        assert!(
            diagnostic[0]
                .message
                .contains("behavior exclusion violated: crash cause Trap is reachable")
        );
        assert!(
            check(&module, None).unwrap_err()[0]
                .message
                .contains("need the selected execution scope")
        );

        // A direct invocation brings the private body into even the single-entry scope.
        module.execution.unoptimized_behavior[0]
            .operations
            .push(UnoptimizedBehaviorOperation::Invocation {
                block,
                site: InvocationSite::Operation(OperationId::new(1).unwrap()),
                callee: private,
            });
        assert!(
            check(&module, Some(&[entry])).unwrap_err()[0]
                .message
                .contains("behavior exclusion violated")
        );
        module.execution.unoptimized_behavior[1].operations.clear();
        assert!(check(&module, None).is_ok());
        module.execution.unoptimized_behavior.pop();
        assert!(
            check(&module, Some(&[entry])).unwrap_err()[0]
                .message
                .contains("cannot establish behavior exclusions")
        );
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
            &selected_provider_plans::SelectedProviderPlanFacts::default(),
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
        module.interfaces.boundary_machines[0].fixed_service_reach.push(owner);
        let reached = inspect(&module, owner);
        assert!(
            reached
                .prohibited
                .iter()
                .any(|row| row.exclusion == BehaviorExclusion::Service(owner)),
            "{reached:?}"
        );
        module.interfaces.boundary_machines[0].fixed_service_reach.clear();
        module.interfaces.boundary_machines[0]
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
        module.interfaces.boundary_machines[0].fixed_service_reach.clear();
        module.interfaces.boundary_machines[0]
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
