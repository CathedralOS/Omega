//! Find strongly connected call components once, with storage
//! proportional to machines and calls. Lowering borrows one ordered member
//! span instead of copying it per member, and outcome inference walks the
//! components callee-first, re-queuing a member's callers from the retained
//! reverse calls. The separate emitted graphs keep their existing ownership.

use semantic_vocabulary::MachineId;
use terminal_check_tables::{Checked, Construct};

use super::{CallGraph, index};
use crate::{Unserved, refuse};

struct Component {
    members: std::ops::Range<usize>,
    cyclic: bool,
    reaches_unranked_cycle: bool,
}

pub(crate) struct Components {
    machine_components: Vec<usize>,
    components: Vec<Component>,
    members: Vec<MachineId>,
    /// Every call site's caller, grouped by callee: the callers of machine
    /// index `m` are `callers[caller_offsets[m]..caller_offsets[m + 1]]`.
    caller_offsets: Vec<usize>,
    callers: Vec<(usize, usize)>,
    #[cfg(test)]
    discovery_edge_visits: usize,
}

impl Components {
    /// Call targets were checked against the complete body table before this
    /// operation. Both DFS passes are iterative, so a deep call chain needs no
    /// recursive host stack. Kosaraju's second pass numbers components in forward
    /// topological order, so a callee's component never precedes its caller's.
    pub(super) fn discover(calls: &CallGraph, machines: &[MachineId]) -> Self {
        let count = machines.len();
        #[cfg(test)]
        let mut discovery_edge_visits = 0;

        let mut reverse_offsets = vec![0usize; count + 1];
        for site in &calls.sites {
            #[cfg(test)]
            {
                discovery_edge_visits += 1;
            }
            reverse_offsets[index(site.callee) + 1] += 1;
        }
        for machine in 0..count {
            reverse_offsets[machine + 1] += reverse_offsets[machine];
        }
        let mut next_slot = reverse_offsets[..count].to_vec();
        let mut reverse_calls = vec![(0usize, 0usize); calls.sites.len()];
        for caller in 0..count {
            for (position, site) in calls.calls(caller).iter().enumerate() {
                #[cfg(test)]
                {
                    discovery_edge_visits += 1;
                }
                let callee = index(site.callee);
                reverse_calls[next_slot[callee]] = (caller, calls.offsets[caller] + position);
                next_slot[callee] += 1;
            }
        }

        let mut visited = vec![false; count];
        let mut finished = Vec::with_capacity(count);
        let mut traversal = Vec::with_capacity(count);
        for root in 0..count {
            if visited[root] {
                continue;
            }
            visited[root] = true;
            traversal.push((root, 0));
            while let Some((machine, next_call)) = traversal.last_mut() {
                let caller = *machine;
                if let Some(site) = calls.calls(caller).get(*next_call) {
                    #[cfg(test)]
                    {
                        discovery_edge_visits += 1;
                    }
                    *next_call += 1;
                    let callee = index(site.callee);
                    if !std::mem::replace(&mut visited[callee], true) {
                        traversal.push((callee, 0));
                    }
                } else {
                    traversal.pop();
                    finished.push(caller);
                }
            }
        }

        let mut machine_components = vec![usize::MAX; count];
        let mut component_sizes = Vec::with_capacity(count);
        let mut pending = Vec::with_capacity(count);
        for &root in finished.iter().rev() {
            if machine_components[root] != usize::MAX {
                continue;
            }
            let component = component_sizes.len();
            let mut size = 0;
            machine_components[root] = component;
            pending.push(root);
            while let Some(machine) = pending.pop() {
                size += 1;
                for &(caller, _) in
                    &reverse_calls[reverse_offsets[machine]..reverse_offsets[machine + 1]]
                {
                    #[cfg(test)]
                    {
                        discovery_edge_visits += 1;
                    }
                    if machine_components[caller] == usize::MAX {
                        machine_components[caller] = component;
                        pending.push(caller);
                    }
                }
            }
            component_sizes.push(size);
        }

        let mut components = Vec::with_capacity(component_sizes.len());
        let mut start = 0;
        next_slot.clear();
        for size in component_sizes {
            next_slot.push(start);
            components.push(Component {
                members: start..start + size,
                cyclic: size > 1,
                reaches_unranked_cycle: false,
            });
            start += size;
        }
        let mut members = machines.to_vec();
        // Scatter in MachineId order, not traversal order: head/block emission
        // and the first refusal must not depend on the DFS's choice of edges.
        for (machine, &identity) in machines.iter().enumerate() {
            let component = machine_components[machine];
            members[next_slot[component]] = identity;
            next_slot[component] += 1;
            if !components[component].cyclic {
                components[component].cyclic = calls.calls(machine).iter().any(|site| {
                    #[cfg(test)]
                    {
                        discovery_edge_visits += 1;
                    }
                    index(site.callee) == machine
                });
            }
        }
        Self {
            machine_components,
            components,
            members,
            caller_offsets: reverse_offsets,
            callers: reverse_calls,
            #[cfg(test)]
            discovery_edge_visits,
        }
    }

    /// Records which components lie on or call into an unranked cycle, in
    /// reverse topological order so every callee component is judged first.
    pub(super) fn rank(&mut self, calls: &CallGraph, witnessed: &[bool]) {
        debug_assert_eq!(witnessed.len(), self.machine_components.len());
        for component in (0..self.components.len()).rev() {
            // Every cyclic member must supply its own measure; lowering and
            // checking prove the comparisons on the component's actual edges.
            let members = &self.members[self.components[component].members.clone()];
            let ranked = members.iter().all(|member| witnessed[index(*member)]);
            let mut reaches_cycle = self.components[component].cyclic && !ranked;
            for &member in members {
                for site in calls.calls(index(member)) {
                    #[cfg(test)]
                    {
                        self.discovery_edge_visits += 1;
                    }
                    let callee_component = self.machine_components[index(site.callee)];
                    debug_assert!(callee_component >= component);
                    reaches_cycle |= self.components[callee_component].reaches_unranked_cycle;
                }
            }
            self.components[component].reaches_unranked_cycle = reaches_cycle;
        }
    }

    /// Every component's members in `MachineId` order, callees' components
    /// before their callers', with whether the component is a call cycle.
    pub(crate) fn callee_first(&self) -> impl Iterator<Item = (&[MachineId], bool)> {
        self.components
            .iter()
            .rev()
            .map(|component| (&self.members[component.members.clone()], component.cyclic))
    }

    /// The machine index of every caller of machine index `callee`, in
    /// ascending order, once per call site.
    pub(crate) fn callers(&self, callee: usize) -> impl Iterator<Item = usize> + '_ {
        self.call_sites(callee).iter().map(|(caller, _)| *caller)
    }

    /// The caller and retained call occurrence for every incoming edge.
    pub(super) fn call_sites(&self, callee: usize) -> &[(usize, usize)] {
        &self.callers[self.caller_offsets[callee]..self.caller_offsets[callee + 1]]
    }

    pub(crate) fn members(&self, machine: MachineId) -> Checked<&[MachineId]> {
        let component = self.machine_components.get(index(machine)).ok_or_else(|| {
            refuse(
                machine,
                Construct::Machine,
                Unserved::Call("machine outside the component graph"),
            )
        })?;
        let component = &self.components[*component];
        Ok(if component.cyclic {
            &self.members[component.members.clone()]
        } else {
            &[]
        })
    }

    pub(super) fn reaches_unranked_cycle(&self, machine: usize) -> bool {
        self.components[self.machine_components[machine]].reaches_unranked_cycle
    }

    pub(crate) fn same_component(&self, left: usize, right: usize) -> bool {
        self.machine_components[left] == self.machine_components[right]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::CallSite;

    fn discover(adjacencies: &[Vec<usize>]) -> Components {
        discover_with_rankings(adjacencies, &vec![false; adjacencies.len()])
    }

    fn discover_with_rankings(adjacencies: &[Vec<usize>], ranked: &[bool]) -> Components {
        let machines: Vec<_> = (1..=adjacencies.len())
            .map(|raw| MachineId::new(raw as u64).unwrap())
            .collect();
        let mut calls = CallGraph {
            sites: Vec::new(),
            offsets: vec![0],
        };
        for callees in adjacencies {
            calls.sites.extend(callees.iter().map(|&callee| CallSite {
                callee: machines[callee],
                terminal: true,
                required_contract: None,
                arguments: &[],
            }));
            calls.offsets.push(calls.sites.len());
        }
        let mut table = Components::discover(&calls, &machines);
        table.rank(&calls, ranked);
        table
    }

    #[test]
    fn components_and_cycle_reachability_match_every_three_machine_graph() {
        for edges in 0..512 {
            let adjacency: Vec<Vec<usize>> = (0..3)
                .map(|caller| {
                    (0..3)
                        .filter(|callee| edges & (1 << (caller * 3 + callee)) != 0)
                        .collect()
                })
                .collect();
            let actual = discover(&adjacency);
            let mut reachable = [[false; 3]; 3];
            for caller in 0..3 {
                let mut pending = adjacency[caller].clone();
                while let Some(callee) = pending.pop() {
                    if !std::mem::replace(&mut reachable[caller][callee], true) {
                        pending.extend(&adjacency[callee]);
                    }
                }
            }
            for caller in 0..3 {
                let expected: Vec<_> = (0..3)
                    .filter(|&callee| reachable[caller][callee] && reachable[callee][caller])
                    .map(|callee| MachineId::new(callee as u64 + 1).unwrap())
                    .collect();
                assert_eq!(
                    actual
                        .members(MachineId::new(caller as u64 + 1).unwrap())
                        .unwrap(),
                    expected,
                    "graph {edges}, caller {caller}"
                );
                assert_eq!(
                    actual.reaches_unranked_cycle(caller),
                    (0..3).any(|callee| reachable[caller][callee] && reachable[callee][callee]),
                    "graph {edges}, caller {caller}"
                );
            }
        }
    }

    #[test]
    fn missing_rankings_propagate_through_shared_callees_and_whole_components() {
        // 0 branches through 1 and 2 into the shared self-cycle 3. A second
        // route through 2 reaches the 4/5 cycle; 6 is an unrelated self-cycle.
        let adjacency = [
            vec![1, 2],
            vec![3],
            vec![3, 4],
            vec![3],
            vec![5],
            vec![4],
            vec![6],
        ];
        for (ranked, expected) in [
            (
                [true, true, true, false, true, true, false],
                [true, true, true, true, false, false, true],
            ),
            (
                [true, true, true, true, true, false, false],
                [true, false, true, false, true, true, true],
            ),
            ([false, false, false, true, true, true, true], [false; 7]),
        ] {
            let actual = discover_with_rankings(&adjacency, &ranked);
            for (machine, expected) in expected.into_iter().enumerate() {
                assert_eq!(
                    actual.reaches_unranked_cycle(machine),
                    expected,
                    "rankings {ranked:?}, machine {machine}"
                );
            }
        }
    }

    #[test]
    fn sparse_chains_and_large_cycles_keep_linear_storage_and_discovery_work() {
        assert!(discover(&[]).members.is_empty());
        for count in [32, 8192] {
            for cyclic in [false, true] {
                let adjacency: Vec<_> = (0..count)
                    .map(|machine| {
                        if machine + 1 < count {
                            vec![machine + 1]
                        } else if cyclic {
                            vec![0]
                        } else {
                            Vec::new()
                        }
                    })
                    .collect();
                let actual = discover(&adjacency);
                let edges: usize = adjacency.iter().map(Vec::len).sum();
                assert_eq!(actual.members.len(), count);
                assert_eq!(actual.machine_components.len(), count);
                assert_eq!(actual.components.len(), if cyclic { 1 } else { count });
                assert!(actual.discovery_edge_visits <= 6 * edges);
                assert_eq!(actual.reaches_unranked_cycle(0), cyclic);
                let ranked = discover_with_rankings(&adjacency, &vec![true; count]);
                assert!(!ranked.reaches_unranked_cycle(0));
                assert!(ranked.discovery_edge_visits <= 6 * edges);
                if cyclic {
                    assert!(std::ptr::eq(
                        actual.members(MachineId::new(1).unwrap()).unwrap(),
                        actual
                            .members(MachineId::new(count as u64).unwrap())
                            .unwrap()
                    ));
                }
            }
        }
    }
}
