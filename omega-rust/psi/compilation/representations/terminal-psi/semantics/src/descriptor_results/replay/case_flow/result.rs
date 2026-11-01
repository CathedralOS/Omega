//! Completed calls transport positively established tags independently of
//! reference payload origins. An absent slot never establishes an inactive
//! case; every feasible return must supply the same subject constraint.

use super::*;

#[cfg(test)]
mod tests;
use crate::descriptor_lifetimes::DescriptorPathSegment;

#[derive(Clone, Default)]
pub(in crate::descriptor_results::replay) struct ResultCases {
    facts: BTreeMap<Vec<DescriptorPathSegment>, BTreeSet<StructuralCaseId>>,
}

impl ResultCases {
    pub(in crate::descriptor_results::replay) fn capture(
        replay: &Replay<'_>,
        machine: &DescriptorMachine,
        state: &State,
        returned: &DescriptorLocation,
    ) -> Self {
        let Some(storage) = replay.canonical_storage(machine, state, returned) else {
            return Self::default();
        };
        Self {
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

    /// A missing fact on any normal return leaves that subject unknown.
    /// Different known tags union their alternatives; they never intersect.
    pub(in crate::descriptor_results::replay) fn join(&mut self, other: &Self) {
        self.facts.retain(|path, cases| {
            let Some(other) = other.facts.get(path) else {
                return false;
            };
            cases.extend(other);
            true
        });
    }

    pub(in crate::descriptor_results::replay) fn install(
        &self,
        replay: &Replay<'_>,
        machine: &DescriptorMachine,
        state: &mut State,
        result: &DescriptorLocation,
    ) -> Option<()> {
        if self.facts.is_empty() {
            return Some(());
        }
        let storage = replay.canonical_storage(machine, state, result)?;
        for (path, active) in &self.facts {
            let mut subject = result.clone();
            subject.path.extend_from_slice(path);
            // Rejoin each summary subject to the receiving type. This checks
            // the tag declaration, not payload residency or pointer authority.
            for case in active {
                valid_subject(
                    replay,
                    machine,
                    &DescriptorCasePredicate {
                        subject: subject.clone(),
                        case: *case,
                    },
                )?;
            }
            let mut destination = storage.clone();
            destination.path.extend_from_slice(path);
            Arc::make_mut(&mut state.cases)
                .facts
                .insert(destination, active.clone());
        }
        Some(())
    }
}
