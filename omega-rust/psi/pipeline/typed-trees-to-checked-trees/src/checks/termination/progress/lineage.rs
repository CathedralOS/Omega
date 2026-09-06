//! Finite entry-subject correspondence through exact local-state transitions.

use super::{FlowFacts, ProgressPremise, ProgressSubject};

mod places;
mod transfers;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq)]
enum ParameterLineage {
    Unseen,
    Exact(Vec<ProgressSubject>),
    Ambiguous,
}

struct StateParameterLineage {
    values: Vec<(ProgressSubject, ParameterLineage)>,
}

impl StateParameterLineage {
    fn derive(
        program: &typed_trees::TypedTrees,
        flow: &FlowFacts,
        machine: &typed_trees::machine::Machine,
        demand: &ProgressSubject,
    ) -> Self {
        let Some(demand) = places::partition(program, machine, demand) else {
            return Self { values: Vec::new() };
        };
        let mut subjects = vec![demand];
        let mut transfers = Vec::new();
        let mut position = 0;
        while position < subjects.len() {
            let mut incoming = transfers::collect(
                program,
                flow,
                machine,
                std::slice::from_ref(&subjects[position]),
            );
            for transfer in &mut incoming {
                if let Some(source) = &transfer.source {
                    if let Some(partition) = places::partition(program, machine, source) {
                        if !subjects.contains(&partition) {
                            subjects.push(partition);
                        }
                    } else {
                        transfer.source = None;
                    }
                }
            }
            transfers.extend(incoming);
            position += 1;
        }
        let states = program.machine_states(machine);
        let mut values = subjects
            .into_iter()
            .map(|subject| (subject, ParameterLineage::Unseen))
            .collect::<Vec<_>>();
        if let Some(entry) = states.first() {
            for (subject, value) in &mut values {
                if program
                    .state_parameters(entry)
                    .iter()
                    .any(|parameter| parameter.symbol == subject.root)
                {
                    *value = ParameterLineage::Exact(vec![subject.clone()]);
                }
            }
        }

        Self::close(values, transfers)
    }

    fn close(
        mut values: Vec<(ProgressSubject, ParameterLineage)>,
        transfers: Vec<transfers::ParameterTransfer>,
    ) -> Self {
        let subjects = values
            .iter()
            .map(|(subject, _)| subject.clone())
            .collect::<Vec<_>>();
        let growing = transfers
            .iter()
            .map(|transfer| transfers::grows_on_cycle(&transfers, transfer, &subjects))
            .collect::<Vec<_>>();
        loop {
            let previous = values.clone();
            for (transfer, grows) in transfers.iter().zip(&growing) {
                let mut incoming = transfer
                    .source
                    .as_ref()
                    .map(|subject| resolve_subject_lineage(&previous, subject.clone()))
                    .unwrap_or(ParameterLineage::Ambiguous);
                if *grows && incoming != ParameterLineage::Unseen {
                    // This transfer closure generates arbitrarily long paths;
                    // it cannot establish a finite caller premise. Unseeded
                    // cycles stay unseen, so an unreachable predecessor cannot
                    // poison an otherwise exact join. Only the growing
                    // parameter and its dependents lose exact lineage.
                    incoming = ParameterLineage::Ambiguous;
                }
                merge_parameter_lineage(&mut values, &transfer.destination, incoming);
            }
            // Every remaining cycle contributes an empty projection. All
            // finite predecessor alternatives therefore converge without a
            // path-length or iteration limit.
            if values == previous {
                return Self { values };
            }
        }
    }

    fn resolve(&self, premise: ProgressPremise) -> Option<Vec<ProgressPremise>> {
        let ParameterLineage::Exact(subjects) =
            resolve_subject_lineage(&self.values, premise.subject)
        else {
            return None;
        };
        Some(
            subjects
                .into_iter()
                .map(|subject| ProgressPremise {
                    profile: premise.profile,
                    subject,
                })
                .collect(),
        )
    }
}

/// Discovery and resolution share one demand. A partial catalogue must never
/// answer a different subject by falling back to an undiscovered ancestor.
pub(super) fn resolve(
    program: &typed_trees::TypedTrees,
    flow: &FlowFacts,
    machine: &typed_trees::machine::Machine,
    premise: ProgressPremise,
) -> Option<Vec<ProgressPremise>> {
    StateParameterLineage::derive(program, flow, machine, &premise.subject).resolve(premise)
}

fn resolve_subject_lineage(
    lineage: &[(ProgressSubject, ParameterLineage)],
    subject: ProgressSubject,
) -> ParameterLineage {
    let Some(prefix) = places::matching_prefix(lineage.iter().map(|(place, _)| place), &subject)
    else {
        return ParameterLineage::Ambiguous;
    };
    let (_, root) = lineage
        .iter()
        .find(|(place, _)| place == prefix)
        .expect("catalogued prefix");
    let suffix = &subject.projections[prefix.projections.len()..];
    match root {
        ParameterLineage::Unseen => ParameterLineage::Unseen,
        ParameterLineage::Ambiguous => ParameterLineage::Ambiguous,
        ParameterLineage::Exact(roots) => ParameterLineage::Exact(
            roots
                .iter()
                .map(|root| {
                    let mut resolved = root.clone();
                    resolved.projections.extend(suffix.iter().copied());
                    resolved
                })
                .collect(),
        ),
    }
}

fn merge_parameter_lineage(
    lineage: &mut [(ProgressSubject, ParameterLineage)],
    subject: &ProgressSubject,
    incoming: ParameterLineage,
) {
    let Some((_, retained)) = lineage
        .iter_mut()
        .find(|(candidate, _)| candidate == subject)
    else {
        return;
    };
    match (&*retained, incoming) {
        (_, ParameterLineage::Unseen) => {}
        (ParameterLineage::Unseen, value) => *retained = value,
        (ParameterLineage::Exact(_), ParameterLineage::Exact(right)) => {
            let ParameterLineage::Exact(retained) = retained else {
                unreachable!()
            };
            for subject in right {
                if !retained.contains(&subject) {
                    retained.push(subject);
                }
            }
        }
        (ParameterLineage::Exact(_), ParameterLineage::Ambiguous) => {
            *retained = ParameterLineage::Ambiguous;
        }
        (ParameterLineage::Ambiguous, _) => {}
    }
}
