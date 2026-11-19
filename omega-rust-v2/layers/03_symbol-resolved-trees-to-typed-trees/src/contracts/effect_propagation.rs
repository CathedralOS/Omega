//! Service reach and synchronous invocations propagate along the same executing
//! machine calls. A newly discovered fact crosses each incoming call once.
use crate::{CallUse, TypeError, TypedCall, TypedStage};
use omega_resolver::DeclarationId;
use std::collections::{HashMap, HashSet, VecDeque};
use std::{fmt::Debug, hash::Hash};

/// Source-order indices keep graph edges valid while the effect passes append their results.
pub(crate) struct ExecutionCalls {
    pub direct: Vec<usize>,
    pub indirect: Vec<usize>,
    callers: HashMap<DeclarationId, Vec<usize>>,
}

impl ExecutionCalls {
    pub(crate) fn collect(stage: &TypedStage) -> Self {
        let mut direct = Vec::new();
        let mut callers = HashMap::<DeclarationId, Vec<usize>>::new();
        for (i, call) in stage
            .calls
            .iter()
            .enumerate()
            .filter(|(_, c)| c.use_site == CallUse::Execution)
        {
            direct.push(i);
            if call.caller.is_some() {
                callers.entry(call.machine.clone()).or_default().push(i);
            }
        }
        let indirect = stage
            .indirect_calls
            .iter()
            .enumerate()
            .filter_map(|(i, c)| (c.use_site == CallUse::Execution).then_some(i))
            .collect();
        Self { direct, indirect, callers }
    }

    pub(super) fn callers_of(
        &self,
        machine: &DeclarationId,
    ) -> &[usize] {
        self.callers.get(machine).map_or(&[], Vec::as_slice)
    }

    pub(super) fn propagate<T: Clone + Debug + Eq + Hash>(
        &self,
        calls: &[TypedCall],
        rows: &mut HashMap<&DeclarationId, HashSet<T>>,
        capacity_error: &str,
        apply: impl Fn(&TypedCall, &DeclarationId, &T) -> Result<Option<T>, TypeError>,
    ) -> Result<(), TypeError> {
        let mut pending = VecDeque::new();
        for &index in &self.direct {
            let machine = &calls[index].machine;
            if self.callers_of(machine).first() == Some(&index) {
                let mut seeds: Vec<_> = rows[machine].iter().collect();
                seeds.sort_by_cached_key(|fact| format!("{fact:?}"));
                pending.extend(seeds.into_iter().map(|fact| (machine, fact.clone())));
            }
        }
        while let Some((machine, fact)) = pending.pop_front() {
            for &index in self.callers_of(machine) {
                let call = &calls[index];
                let caller = call.caller.as_ref().expect("an incoming edge has a caller");
                let Some(inherited) = apply(call, caller, &fact)? else { continue };
                let row = rows.get_mut(caller).unwrap();
                if row.insert(inherited.clone()) {
                    if row.len() > 4096 {
                        return Err(TypeError::for_site(&call.site, capacity_error));
                    }
                    pending.push_back((caller, inherited));
                }
            }
        }
        Ok(())
    }
}
