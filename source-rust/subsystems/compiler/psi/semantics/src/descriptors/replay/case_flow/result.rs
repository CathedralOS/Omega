use super::*;

#[cfg(test)]
mod tests;
use crate::descriptors::descriptor_lifetimes::DescriptorPathSegment;

#[derive(Clone, Default)]
pub(crate) struct ResultCases {
    root_type: Option<StructuralTypeId>,
    facts: BTreeMap<Vec<DescriptorPathSegment>, CaseSet>,
}

impl ResultCases {
    pub(crate) fn capture(
        replay: &Replay<'_>,
        machine: &DescriptorMachine,
        state: &State,
        returned: &DescriptorLocation,
    ) -> Self {
        let Some(storage) = replay.canonical_storage(machine, state, returned, None) else {
            return Self::default();
        };
        Self {
            root_type: machine.place_types.get(&returned.root).copied(),
            facts: state
                .cases
                .facts
                .iter()
                .filter(|(subject, _)| {
                    subject.root == storage.root
                        && subject.path.starts_with(&storage.path)
                        && !subject.path[storage.path.len()..]
                            .contains(&DescriptorPathSegment::Referent)
                })
                .map(|(subject, cases)| {
                    (subject.path[storage.path.len()..].to_vec(), cases.clone())
                })
                .collect(),
        }
    }

    pub(crate) fn join(&mut self, other: &Self) {
        if self.root_type != other.root_type {
            self.root_type = None;
            self.facts.clear();
            return;
        }
        self.facts.retain(|path, cases| {
            let Some(other) = other.facts.get(path) else {
                return false;
            };
            *cases = cases.union(other);
            true
        });
    }

    pub(crate) fn excludes_payload(
        &self,
        root_type: StructuralTypeId,
        path: &[DescriptorPathSegment],
    ) -> bool {
        if self.root_type != Some(root_type) {
            return false;
        }
        path.iter().enumerate().any(|(position, segment)| {
            let DescriptorPathSegment::Case(case) = segment else {
                return false;
            };
            self.facts
                .get(&path[..position])
                .is_some_and(|active| !active.is_empty() && !active.contains(case))
        })
    }

    pub(crate) fn install(
        &self,
        replay: &Replay<'_>,
        machine: &DescriptorMachine,
        state: &mut State,
        result: &DescriptorLocation,
    ) -> Option<()> {
        if self.facts.is_empty() {
            return Some(());
        }
        if machine.place_types.get(&result.root).copied() != self.root_type {
            return None;
        }
        let storage = replay.canonical_storage(machine, state, result, None)?;
        for (path, active) in &self.facts {
            let mut subject = result.clone();
            subject.path.extend_from_slice(path);
            active.for_each_try(|case| {
                valid_subject(
                    replay,
                    machine,
                    &DescriptorCasePredicate {
                        subject: subject.clone().into(),
                        case,
                    },
                )
            })?;
            let mut destination = storage.clone();
            destination.path.extend_from_slice(path);
            Arc::make_mut(&mut state.cases)
                .facts
                .insert(destination, active.clone());
        }
        Some(())
    }
}
