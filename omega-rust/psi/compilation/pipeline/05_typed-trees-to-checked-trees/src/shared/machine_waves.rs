//! Machine call-cycle components and the waves that check them in parallel.
//!
//! A whole-program fixed point over machine summaries (termination, mutation,
//! crash and reach summaries) only needs a machine's callees to be settled
//! before the machine itself is derived. Machines that call each other, directly
//! or around a longer cycle, form one *component*; a component's fixed point
//! needs nothing but its own members and components it calls. Ordering the
//! components so every callee comes first groups them into *waves*: every
//! component of a wave calls only earlier waves, so the components of one wave
//! are independent jobs.
//!
//! The caller supplies the read graph: for each machine (by roster position)
//! the machines whose results its step reads. An edge that a step never reads
//! only costs parallelism; a read without an edge would let the step see an
//! unsettled value, so each phase derives its edges from the same lookups its
//! step performs.
//!
//! Determinism: a step sees settled results of earlier waves and the initial
//! values of everything else, never a result from its own wave, and its result
//! is written back by roster position. The output therefore does not depend on
//! the thread count or on which worker finishes first.

use rayon::prelude::*;

pub(crate) struct MachineCallComponents {
    /// Each component's members in ascending roster order.
    members: Vec<Vec<usize>>,
    /// Component indices per wave, callees' waves first.
    callee_first_waves: Vec<Vec<usize>>,
    /// Component indices per wave, callers' waves first.
    caller_first_waves: Vec<Vec<usize>>,
}

impl MachineCallComponents {
    /// `reads[machine]` lists the roster positions whose results the step for
    /// `machine` reads. Self edges and duplicates are allowed.
    pub(crate) fn new(reads: &[Vec<usize>]) -> Self {
        let (component_of, count) = strongly_connected_components(reads);
        let mut members = vec![Vec::new(); count];
        for (machine, &component) in component_of.iter().enumerate() {
            members[component].push(machine);
        }
        // Tarjan emits a component only after every component it reaches, so
        // ascending component numbers are already a callee-first order.
        let mut callee_level = vec![0usize; count];
        for component in 0..count {
            let mut level = 0;
            for &machine in &members[component] {
                for &callee in &reads[machine] {
                    let callee = component_of[callee];
                    if callee != component {
                        level = level.max(callee_level[callee] + 1);
                    }
                }
            }
            callee_level[component] = level;
        }
        let mut caller_level = vec![0usize; count];
        for component in (0..count).rev() {
            for &machine in &members[component] {
                for &callee in &reads[machine] {
                    let callee = component_of[callee];
                    if callee != component {
                        caller_level[callee] =
                            caller_level[callee].max(caller_level[component] + 1);
                    }
                }
            }
        }
        Self {
            members,
            callee_first_waves: waves(&callee_level),
            caller_first_waves: waves(&caller_level),
        }
    }

    /// Run `step` once per component, callees' components first. `step`
    /// receives the component's members (ascending roster order) and the
    /// results so far, and returns one result per member in the same order.
    pub(crate) fn run_callee_first<T, F>(&self, results: &mut [T], step: F)
    where
        T: Send + Sync,
        F: Fn(&[usize], &[T]) -> Vec<T> + Sync,
    {
        self.run_waves(&self.callee_first_waves, results, step);
    }

    /// As `run_callee_first`, with callers' components first: for facts that
    /// flow from a caller into its private callee's entry. No phase uses this
    /// order yet; the flow inputs and private-entry passes are its consumers.
    #[allow(dead_code)]
    pub(crate) fn run_caller_first<T, F>(&self, results: &mut [T], step: F)
    where
        T: Send + Sync,
        F: Fn(&[usize], &[T]) -> Vec<T> + Sync,
    {
        self.run_waves(&self.caller_first_waves, results, step);
    }

    fn run_waves<T, F>(&self, waves: &[Vec<usize>], results: &mut [T], step: F)
    where
        T: Send + Sync,
        F: Fn(&[usize], &[T]) -> Vec<T> + Sync,
    {
        for wave in waves {
            let settled: &[T] = results;
            let derived: Vec<Vec<T>> = if wave.len() == 1 {
                vec![step(&self.members[wave[0]], settled)]
            } else {
                validation::checking_thread_pool().install(|| {
                    wave.par_iter()
                        .map(|&component| step(&self.members[component], settled))
                        .collect()
                })
            };
            for (&component, values) in wave.iter().zip(derived) {
                let members = &self.members[component];
                assert_eq!(
                    members.len(),
                    values.len(),
                    "a component step returns one result per member"
                );
                for (&machine, value) in members.iter().zip(values) {
                    results[machine] = value;
                }
            }
        }
    }
}

fn waves(levels: &[usize]) -> Vec<Vec<usize>> {
    let depth = levels.iter().copied().max().map_or(0, |level| level + 1);
    let mut waves = vec![Vec::new(); depth];
    for (component, &level) in levels.iter().enumerate() {
        waves[level].push(component);
    }
    waves
}

/// Iterative Tarjan. A component's successors receive lower numbers.
fn strongly_connected_components(next: &[Vec<usize>]) -> (Vec<usize>, usize) {
    const UNVISITED: usize = usize::MAX;
    let count = next.len();
    let mut index = vec![UNVISITED; count];
    let mut low = vec![0; count];
    let mut on_stack = vec![false; count];
    let mut stack = Vec::new();
    let mut component = vec![UNVISITED; count];
    let (mut next_index, mut components) = (0usize, 0usize);
    let mut work: Vec<(usize, usize)> = Vec::new();
    for start in 0..count {
        if index[start] != UNVISITED {
            continue;
        }
        work.push((start, 0));
        while let Some(&mut (node, ref mut edge)) = work.last_mut() {
            if *edge == 0 && index[node] == UNVISITED {
                index[node] = next_index;
                low[node] = next_index;
                next_index += 1;
                stack.push(node);
                on_stack[node] = true;
            }
            if let Some(&to) = next[node].get(*edge) {
                *edge += 1;
                if index[to] == UNVISITED {
                    work.push((to, 0));
                } else if on_stack[to] {
                    low[node] = low[node].min(index[to]);
                }
                continue;
            }
            work.pop();
            if let Some(&(parent, _)) = work.last() {
                low[parent] = low[parent].min(low[node]);
            }
            if low[node] == index[node] {
                while let Some(member) = stack.pop() {
                    on_stack[member] = false;
                    component[member] = components;
                    if member == node {
                        break;
                    }
                }
                components += 1;
            }
        }
    }
    (component, components)
}

#[cfg(test)]
mod tests {
    use super::MachineCallComponents;

    #[test]
    fn cycles_share_a_component_and_callees_run_first() {
        // 0 -> 1 <-> 2 -> 3, and 4 alone.
        let reads = vec![vec![1], vec![2], vec![1, 3], vec![], vec![4]];
        let components = MachineCallComponents::new(&reads);
        assert_eq!(components.members.len(), 4);
        assert_eq!(components.callee_first_waves.len(), 3);
        let mut order = vec![usize::MAX; 5];
        let counter = std::sync::atomic::AtomicUsize::new(0);
        let mut seen = vec![0usize; 5];
        components.run_callee_first(&mut seen, |members, settled| {
            for &member in members {
                for &callee in &reads[member] {
                    if !members.contains(&callee) {
                        assert_ne!(settled[callee], 0, "callee settled before caller");
                    }
                }
            }
            members.iter().map(|_| 1).collect()
        });
        components.run_callee_first(&mut order, |members, _| {
            let at = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            members.iter().map(|_| at).collect()
        });
        assert_eq!(order[1], order[2]);
        assert!(order[3] < order[1] && order[1] < order[0]);

        let mut caller_order = vec![usize::MAX; 5];
        counter.store(0, std::sync::atomic::Ordering::SeqCst);
        components.run_caller_first(&mut caller_order, |members, _| {
            let at = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            members.iter().map(|_| at).collect()
        });
        assert!(caller_order[0] < caller_order[1] && caller_order[1] < caller_order[3]);
    }
}
