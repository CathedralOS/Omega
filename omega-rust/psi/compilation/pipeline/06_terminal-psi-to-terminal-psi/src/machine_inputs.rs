//! Keep each final machine check's source rows and proof offer together after
//! inference settles. Source rows already belong to machines: borrow them in
//! place instead of flattening, rescanning and copying the complete branch list.

use semantic_vocabulary::{IdTable, MachineId};
use terminal_check_tables::program::{Branch, MachineSources, authored_machine_sources};
use terminal_check_tables::{Checked, Construct, MachineTables, RefusalReason};
use terminal_psi::TerminalModule;

use crate::{Carried, refusal};

pub(super) struct MachineInputs<'sources> {
    pub branches: &'sources [Branch],
    pub carried: Option<usize>,
    pub requirement: bool,
}

pub(super) struct PreparedMachineInputs<'sources> {
    machines: IdTable<MachineId, MachineInputs<'sources>>,
    #[cfg(test)]
    association_visits: usize,
}

impl<'sources> PreparedMachineInputs<'sources> {
    pub fn new(
        module: &TerminalModule,
        sources: &'sources IdTable<MachineId, MachineSources>,
        tables: &IdTable<MachineId, MachineTables>,
        requirements: &[MachineId],
        carried: &[Carried<'_>],
    ) -> Self {
        #[cfg(test)]
        let mut association_visits = 0;
        let mut machines = tables.map(|_, _| {
            #[cfg(test)]
            {
                association_visits += 1;
            }
            MachineInputs {
                branches: &[],
                carried: None,
                requirement: false,
            }
        });
        for (machine, sources) in authored_machine_sources(module, sources) {
            #[cfg(test)]
            {
                association_visits += 1;
            }
            if let Some(inputs) = machines.get_mut(machine) {
                inputs.branches = &sources.branches;
            }
        }
        for &requirement in requirements {
            #[cfg(test)]
            {
                association_visits += 1;
            }
            if let Some(inputs) = machines.get_mut(requirement) {
                inputs.requirement = true;
            }
        }
        for (position, offered) in carried.iter().enumerate() {
            #[cfg(test)]
            {
                association_visits += 1;
            }
            if let Some(inputs) = machines.get_mut(offered.specialization)
                && inputs.carried.is_none()
            {
                // The old scan chose the first offer for this exact machine.
                // Neither unmatched offers nor later duplicates change that
                // association; the carrier still checks its actual proof goals.
                inputs.carried = Some(position);
            }
        }
        Self {
            machines,
            #[cfg(test)]
            association_visits,
        }
    }

    pub fn for_machine(&self, machine: MachineId) -> Checked<&MachineInputs<'sources>> {
        self.machines.get(machine).ok_or_else(|| {
            refusal(
                RefusalReason::ForeignIdentity,
                machine,
                Construct::Machine,
                "machine check inputs are past the end",
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_check_tables::program::{SourceOrigin, Unpositioned};

    fn id(raw: u64) -> MachineId {
        MachineId::new(raw).unwrap()
    }

    fn branch(raw: u64) -> Branch {
        Branch {
            block: semantic_vocabulary::BlockId::new(raw).unwrap(),
            decided_in: semantic_vocabulary::BlockId::new(1).unwrap(),
            arm: SourceOrigin::Unpositioned(Unpositioned::SpanAbsent),
            condition: SourceOrigin::Unpositioned(Unpositioned::SpanAbsent),
        }
    }

    #[test]
    fn machine_inputs_borrow_their_own_ordered_branches_and_first_proof_offer() {
        let mut sources = IdTable::default();
        for branches in [vec![branch(3), branch(2)], Vec::new(), vec![branch(7)]] {
            sources.push(MachineSources {
                branches,
                ..Default::default()
            });
        }
        let tables = sources.map(|_, _| MachineTables::default());
        let carried = [3, 99, 1, 3].map(|specialization| Carried {
            specialization: id(specialization),
            forms: Vec::new(),
        });
        let inputs = PreparedMachineInputs::new(
            &TerminalModule::declarations(),
            &sources,
            &tables,
            &[id(2), id(99)],
            &carried,
        );
        assert!(std::ptr::eq(
            inputs.for_machine(id(1)).unwrap().branches,
            sources.get(id(1)).unwrap().branches.as_slice()
        ));
        assert_eq!(
            inputs.for_machine(id(1)).unwrap().branches,
            &[branch(3), branch(2)]
        );
        assert!(inputs.for_machine(id(2)).unwrap().branches.is_empty());
        assert_eq!(inputs.for_machine(id(3)).unwrap().branches, &[branch(7)]);
        assert_eq!(inputs.for_machine(id(1)).unwrap().carried, Some(2));
        assert_eq!(inputs.for_machine(id(2)).unwrap().carried, None);
        assert_eq!(inputs.for_machine(id(3)).unwrap().carried, Some(0));
        assert!(!inputs.for_machine(id(1)).unwrap().requirement);
        assert!(inputs.for_machine(id(2)).unwrap().requirement);
        assert!(!inputs.for_machine(id(3)).unwrap().requirement);
        assert!(inputs.for_machine(id(99)).is_err());
        assert_eq!(
            inputs.association_visits,
            2 * tables.len() + carried.len() + 2
        );
    }

    #[test]
    fn association_work_is_linear_and_does_not_copy_branch_payloads() {
        for count in [32, 8192] {
            let mut sources = IdTable::default();
            for _ in 0..count {
                sources.push(MachineSources {
                    branches: vec![branch(2); 16],
                    ..Default::default()
                });
            }
            let tables = sources.map(|_, _| MachineTables::default());
            let carried: Vec<_> = tables
                .keys()
                .rev()
                .map(|specialization| Carried {
                    specialization,
                    forms: Vec::new(),
                })
                .collect();
            let requirements: Vec<_> = tables.keys().step_by(2).collect();
            let inputs = PreparedMachineInputs::new(
                &TerminalModule::declarations(),
                &sources,
                &tables,
                &requirements,
                &carried,
            );
            assert_eq!(inputs.machines.len(), count);
            assert_eq!(inputs.association_visits, 3 * count + requirements.len());
            for machine in tables.keys() {
                let input = inputs.for_machine(machine).unwrap();
                assert!(std::ptr::eq(
                    input.branches,
                    sources.get(machine).unwrap().branches.as_slice()
                ));
                assert_eq!(carried[input.carried.unwrap()].specialization, machine);
            }
        }
    }
}
