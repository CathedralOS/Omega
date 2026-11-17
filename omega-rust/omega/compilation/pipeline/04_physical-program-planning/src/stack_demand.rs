//! Stack composition over physical facts. No instruction bytes, object layout,
//! or earlier executable representation is needed to measure a call graph.

use function_identity::MachineFunctionIdentity;
use semantic_vocabulary::MachineId;
use std::collections::BTreeSet;
use target::{Architecture, NativeTarget};
use target_operations::CallSiteOwner;
use terminal_psi::TerminalPsiIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalStackExtent {
    Finite(u64),
    Conflicting,
    Unaccounted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalStackCall {
    pub target: MachineId,
    pub owner: CallSiteOwner,
    pub caller_live_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForeignStackExtent {
    None,
    Admitted(task_plans::AdmittedSameStackContribution),
    Hosted { receipt: Option<[u8; 32]> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalForeignStackCall {
    pub owner: CallSiteOwner,
    pub caller_live_bytes: u32,
    pub extent: ForeignStackExtent,
    pub invoked_callbacks: Vec<MachineFunctionIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalStackFunction {
    pub machine: MachineId,
    pub local: LocalStackExtent,
    pub calls: Vec<PhysicalStackCall>,
    pub foreign_calls: Vec<PhysicalForeignStackCall>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalStackProgram {
    pub psi: TerminalPsiIdentity,
    pub target: NativeTarget,
    pub functions: Vec<PhysicalStackFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StackDemandError {
    EntryFunctionMissing(MachineId),
    UnknownInternalCallTarget {
        caller: MachineId,
        target: MachineId,
    },
    TerminalStackCycle(MachineId),
    ConflictingTerminalStackEvidence(MachineId),
    UnaccountedTerminalStack(MachineId),
    TerminalStackCompositionOverflow {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    InvalidPrivateFunctionIdentity,
}

type Error = StackDemandError;

struct Composition {
    active: Vec<bool>,
    peaks: Vec<Option<u64>>,
    machines: BTreeSet<MachineId>,
    reports: BTreeSet<task_plans::AdmittedStackContributionReportId>,
    commitments: BTreeSet<task_plans::SameStackContributionCommitment>,
    hosted: BTreeSet<[u8; 32]>,
}

pub fn derive_physical_stack_demand(
    program: &PhysicalStackProgram,
    entry: MachineId,
    private_demands: Vec<(MachineFunctionIdentity, StackDemand)>,
) -> Result<StackDemand, Error> {
    let mut functions: Vec<_> = program.functions.iter().collect();
    functions.sort_unstable_by_key(|function| function.machine);
    if let Some(pair) = functions
        .windows(2)
        .find(|pair| pair[0].machine == pair[1].machine)
    {
        return Err(Error::ConflictingTerminalStackEvidence(pair[0].machine));
    }
    let root = functions
        .binary_search_by_key(&entry, |function| function.machine)
        .map_err(|_| Error::EntryFunctionMissing(entry))?;
    let mut composition = Composition {
        active: vec![false; functions.len()],
        peaks: vec![None; functions.len()],
        machines: BTreeSet::new(),
        reports: BTreeSet::new(),
        commitments: BTreeSet::new(),
        hosted: BTreeSet::new(),
    };
    let ceiling_bytes = visit(root, &functions, &private_demands, &mut composition)?;
    Ok(StackDemand {
        psi: program.psi,
        target: program.target,
        entry,
        ceiling_bytes,
        stack_alignment: 16,
        contributing_machines: composition.machines,
        admitted_contribution_report_identities: composition.reports,
        admitted_contribution_commitments: composition.commitments,
        hosted_native_bindings: composition.hosted,
        private_demands,
    })
}

fn visit(
    index: usize,
    functions: &[&PhysicalStackFunction],
    private_demands: &[(MachineFunctionIdentity, StackDemand)],
    composition: &mut Composition,
) -> Result<u64, Error> {
    let function = functions[index];
    let machine = function.machine;
    composition.machines.insert(machine);
    if let Some(peak) = composition.peaks[index] {
        return Ok(peak);
    }
    if std::mem::replace(&mut composition.active[index], true) {
        return Err(Error::TerminalStackCycle(machine));
    }
    let mut peak = match function.local {
        LocalStackExtent::Finite(bytes) => bytes,
        LocalStackExtent::Conflicting => {
            return Err(Error::ConflictingTerminalStackEvidence(machine));
        }
        LocalStackExtent::Unaccounted => return Err(Error::UnaccountedTerminalStack(machine)),
    };
    for call in &function.calls {
        let callee = functions
            .binary_search_by_key(&call.target, |function| function.machine)
            .map_err(|_| Error::UnknownInternalCallTarget {
                caller: machine,
                target: call.target,
            })?;
        let callee_peak = visit(callee, functions, private_demands, composition)?;
        let composed = u64::from(call.caller_live_bytes)
            .checked_add(callee_peak)
            .ok_or(Error::TerminalStackCompositionOverflow {
                caller: machine,
                owner: call.owner,
            })?;
        peak = peak.max(composed);
    }
    for call in &function.foreign_calls {
        let contribution = match &call.extent {
            ForeignStackExtent::Admitted(contribution) => contribution,
            extent => {
                if let ForeignStackExtent::Hosted { receipt } = extent {
                    composition
                        .hosted
                        .insert(receipt.ok_or(Error::InvalidPrivateFunctionIdentity)?);
                }
                peak = peak.max(u64::from(call.caller_live_bytes));
                continue;
            }
        };
        let mut callback_peak = 0;
        for callback in &call.invoked_callbacks {
            let (_, demand) = private_demands
                .iter()
                .find(|(identity, _)| identity == callback)
                .ok_or(Error::InvalidPrivateFunctionIdentity)?;
            composition
                .reports
                .extend(&demand.admitted_contribution_report_identities);
            composition
                .commitments
                .extend(&demand.admitted_contribution_commitments);
            composition.hosted.extend(&demand.hosted_native_bindings);
            callback_peak = callback_peak.max(demand.ceiling_bytes);
        }
        let composed = u64::from(call.caller_live_bytes)
            .checked_add(contribution.bytes())
            .and_then(|bytes| bytes.checked_add(callback_peak))
            .ok_or(Error::TerminalStackCompositionOverflow {
                caller: machine,
                owner: call.owner,
            })?;
        peak = peak.max(composed);
        composition.reports.insert(contribution.report_identity());
        composition.commitments.insert(contribution.commitment());
    }
    composition.active[index] = false;
    composition.peaks[index] = Some(peak);
    Ok(peak)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackDemand {
    psi: TerminalPsiIdentity,
    target: NativeTarget,
    entry: MachineId,
    ceiling_bytes: u64,
    stack_alignment: u32,
    contributing_machines: std::collections::BTreeSet<MachineId>,
    admitted_contribution_report_identities:
        std::collections::BTreeSet<task_plans::AdmittedStackContributionReportId>,
    admitted_contribution_commitments:
        std::collections::BTreeSet<task_plans::SameStackContributionCommitment>,
    hosted_native_bindings: std::collections::BTreeSet<[u8; 32]>,
    private_demands: Vec<(function_identity::MachineFunctionIdentity, StackDemand)>,
}

impl StackDemand {
    /// Account for a preplanned entry frame without changing the body's call graph.
    pub fn with_entry_cost(mut self, bytes: u64, alignment: u32) -> Option<Self> {
        self.ceiling_bytes = self.ceiling_bytes.checked_add(bytes)?;
        self.stack_alignment = self.stack_alignment.max(alignment);
        Some(self)
    }

    pub const fn psi(&self) -> TerminalPsiIdentity {
        self.psi
    }

    pub const fn target(&self) -> NativeTarget {
        self.target
    }

    pub const fn entry(&self) -> MachineId {
        self.entry
    }

    pub fn ceiling_bytes(&self) -> Option<u64> {
        (!self.requires_host_stack()).then_some(self.ceiling_bytes)
    }

    /// Known checked frames and admitted finite contributions, excluding
    /// the unbounded foreign frames of a hosted import.
    pub const fn known_stack_bytes(&self) -> u64 {
        self.ceiling_bytes
    }

    pub fn requires_host_stack(&self) -> bool {
        !self.hosted_native_bindings.is_empty()
    }

    pub fn hosted_native_bindings(&self) -> &std::collections::BTreeSet<[u8; 32]> {
        &self.hosted_native_bindings
    }

    pub const fn stack_alignment(&self) -> u32 {
        self.stack_alignment
    }

    pub const fn contributing_machines(&self) -> &std::collections::BTreeSet<MachineId> {
        &self.contributing_machines
    }

    pub const fn admitted_contribution_report_identities(
        &self,
    ) -> &std::collections::BTreeSet<task_plans::AdmittedStackContributionReportId> {
        &self.admitted_contribution_report_identities
    }

    pub const fn admitted_contribution_commitments(
        &self,
    ) -> &std::collections::BTreeSet<task_plans::SameStackContributionCommitment> {
        &self.admitted_contribution_commitments
    }
}

impl installation_evidence::StackDemandEvidence for StackDemand {
    fn psi(&self) -> TerminalPsiIdentity {
        self.psi
    }

    fn architecture(&self) -> Architecture {
        self.target.architecture
    }

    fn entry(&self) -> MachineId {
        self.entry
    }

    fn ceiling_bytes(&self) -> Option<u64> {
        self.ceiling_bytes()
    }

    fn stack_alignment(&self) -> u32 {
        self.stack_alignment
    }

    fn contributing_machines(&self) -> &std::collections::BTreeSet<MachineId> {
        &self.contributing_machines
    }

    fn admitted_stack_contribution_report_identities(&self) -> std::collections::BTreeSet<u64> {
        self.admitted_contribution_report_identities
            .iter()
            .map(|identity| identity.normalized_identity())
            .collect()
    }

    fn admitted_stack_contribution_commitments(&self) -> std::collections::BTreeSet<[u8; 32]> {
        self.admitted_contribution_commitments
            .iter()
            .map(|commitment| commitment.as_bytes())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::OperationId;
    use terminal_psi::{SemanticFingerprint, VocabularyMarker};

    fn machine(id: u64) -> MachineId {
        MachineId::new(id).unwrap()
    }

    fn program() -> PhysicalStackProgram {
        PhysicalStackProgram {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([5; 32]),
            },
            target: NativeTarget::linux_x64(),
            functions: vec![
                PhysicalStackFunction {
                    machine: machine(1),
                    local: LocalStackExtent::Finite(32),
                    calls: vec![PhysicalStackCall {
                        target: machine(2),
                        owner: CallSiteOwner::Operation(OperationId::new(3).unwrap()),
                        caller_live_bytes: 48,
                    }],
                    foreign_calls: vec![],
                },
                PhysicalStackFunction {
                    machine: machine(2),
                    local: LocalStackExtent::Finite(64),
                    calls: vec![],
                    foreign_calls: vec![],
                },
            ],
        }
    }

    #[test]
    fn physical_frames_compose_before_bytes_exist_and_hosted_demand_remains_unbounded() {
        let mut program = program();
        let demand = derive_physical_stack_demand(&program, machine(1), vec![]).unwrap();
        assert_eq!(demand.ceiling_bytes(), Some(112));
        assert_eq!(
            demand
                .contributing_machines()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            [machine(1), machine(2)]
        );
        let entry = demand.with_entry_cost(24, 32).unwrap();
        assert_eq!(entry.ceiling_bytes(), Some(136));
        assert_eq!(entry.stack_alignment(), 32);
        program.functions[1]
            .foreign_calls
            .push(PhysicalForeignStackCall {
                owner: CallSiteOwner::Operation(OperationId::new(4).unwrap()),
                caller_live_bytes: 80,
                extent: ForeignStackExtent::Hosted {
                    receipt: Some([7; 32]),
                },
                invoked_callbacks: vec![],
            });
        let hosted = derive_physical_stack_demand(&program, machine(1), vec![]).unwrap();
        assert_eq!(hosted.known_stack_bytes(), 128);
        assert_eq!(hosted.ceiling_bytes(), None);
        assert!(hosted.hosted_native_bindings().contains(&[7; 32]));
        program.functions[1].foreign_calls[0].extent = ForeignStackExtent::Hosted { receipt: None };
        assert_eq!(
            derive_physical_stack_demand(&program, machine(1), vec![]),
            Err(Error::InvalidPrivateFunctionIdentity)
        );
    }

    #[test]
    fn missing_cyclic_and_unaccounted_calls_never_become_zero_demand() {
        let original = program();
        let mut changed = original.clone();
        changed.functions[1].local = LocalStackExtent::Unaccounted;
        assert_eq!(
            derive_physical_stack_demand(&changed, machine(1), vec![]),
            Err(Error::UnaccountedTerminalStack(machine(2)))
        );
        changed = original.clone();
        changed.functions[1].calls = vec![PhysicalStackCall {
            target: machine(1),
            owner: CallSiteOwner::Operation(OperationId::new(5).unwrap()),
            caller_live_bytes: 64,
        }];
        assert_eq!(
            derive_physical_stack_demand(&changed, machine(1), vec![]),
            Err(Error::TerminalStackCycle(machine(1)))
        );
        changed = original;
        changed.functions.pop();
        assert_eq!(
            derive_physical_stack_demand(&changed, machine(1), vec![]),
            Err(Error::UnknownInternalCallTarget {
                caller: machine(1),
                target: machine(2)
            })
        );
    }
}

mod physical;
pub use physical::plan_physical_stack_program;
