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
//! runtime execution. Dynamic calls carry no per-target coverage in this
//! slice and are reported as gaps. There is no authoring surface or admission
//! join yet; `establish_behavior_exclusions` consumes an already-selected
//! entry roster.

use semantic_vocabulary::{BlockId, BoundaryMachineId, MachineId, OperationId, ServiceId};
use std::collections::VecDeque;
use terminal_psi::{CrashCause, OperationKind, TerminalMachine, TerminalModule, Terminator};

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
/// entry roster's static call closure in `module`.
pub fn establish_behavior_exclusions(
    module: &TerminalModule,
    entries: &[MachineId],
    exclusions: &BehaviorExclusions,
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
                &mut report,
                &mut pending,
            );
        }
    }
    report
}

fn inspect_machine<'module>(
    module: &'module TerminalModule,
    entry: MachineId,
    machine: &'module TerminalMachine,
    exclusions: &BehaviorExclusions,
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
                    for &service in &declaration.fixed_service_reach {
                        if exclusions.excludes_service(service) {
                            report.prohibited.push(ProhibitedBehavior {
                                exclusion: BehaviorExclusion::Service(service),
                                entry,
                                machine: machine.id,
                                site: site.clone(),
                            });
                        }
                    }
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
                    report.gaps.push(EvidenceGap {
                        entry,
                        machine: machine.id,
                        block: Some(block.id),
                        operation: Some(operation.id),
                        kind: EvidenceGapKind::DynamicCall,
                    });
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
