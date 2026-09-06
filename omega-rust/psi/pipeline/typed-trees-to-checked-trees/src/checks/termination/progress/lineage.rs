//! Finite entry-subject correspondence through exact local-state transitions.

use super::{FlowFacts, ProgressPremise, ProgressSubject, SymbolHandle};

mod transfers;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq)]
enum ParameterLineage {
    Unseen,
    Exact(Vec<ProgressSubject>),
    Ambiguous,
}

pub(super) struct StateParameterLineage {
    values: Vec<(SymbolHandle, ParameterLineage)>,
}

impl StateParameterLineage {
    pub(super) fn derive(
        program: &typed_trees::TypedTrees,
        flow: &FlowFacts,
        machine: &typed_trees::machine::Machine,
    ) -> Self {
        let states = program.machine_states(machine);
        let mut values = states
            .iter()
            .flat_map(|state| program.state_parameters(state))
            .map(|parameter| (parameter.symbol, ParameterLineage::Unseen))
            .collect::<Vec<_>>();
        if let Some(entry) = states.first() {
            for parameter in program.state_parameters(entry) {
                set_parameter_lineage(
                    &mut values,
                    parameter.symbol,
                    ParameterLineage::Exact(vec![ProgressSubject {
                        root: parameter.symbol,
                        projections: Vec::new(),
                    }]),
                );
            }
        }

        let transfers = transfers::collect(program, flow, machine);
        Self::close(values, transfers)
    }

    fn close(
        mut values: Vec<(SymbolHandle, ParameterLineage)>,
        transfers: Vec<transfers::ParameterTransfer>,
    ) -> Self {
        let growing = transfers
            .iter()
            .map(|transfer| transfers::grows_on_cycle(&transfers, transfer))
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
                merge_parameter_lineage(&mut values, transfer.destination, incoming);
            }
            // Every remaining cycle contributes an empty projection. All
            // finite predecessor alternatives therefore converge without a
            // path-length or iteration limit.
            if values == previous {
                return Self { values };
            }
        }
    }

    pub(super) fn resolve(&self, premise: ProgressPremise) -> Option<Vec<ProgressPremise>> {
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

fn resolve_subject_lineage(
    lineage: &[(SymbolHandle, ParameterLineage)],
    subject: ProgressSubject,
) -> ParameterLineage {
    let Some((_, root)) = lineage.iter().find(|(symbol, _)| *symbol == subject.root) else {
        return ParameterLineage::Ambiguous;
    };
    match root {
        ParameterLineage::Unseen => ParameterLineage::Unseen,
        ParameterLineage::Ambiguous => ParameterLineage::Ambiguous,
        ParameterLineage::Exact(roots) => ParameterLineage::Exact(
            roots
                .iter()
                .map(|root| {
                    let mut resolved = root.clone();
                    resolved
                        .projections
                        .extend(subject.projections.iter().copied());
                    resolved
                })
                .collect(),
        ),
    }
}

fn set_parameter_lineage(
    lineage: &mut [(SymbolHandle, ParameterLineage)],
    symbol: SymbolHandle,
    value: ParameterLineage,
) {
    if let Some((_, retained)) = lineage
        .iter_mut()
        .find(|(candidate, _)| *candidate == symbol)
    {
        *retained = value;
    }
}

fn merge_parameter_lineage(
    lineage: &mut [(SymbolHandle, ParameterLineage)],
    symbol: SymbolHandle,
    incoming: ParameterLineage,
) {
    let Some((_, retained)) = lineage
        .iter_mut()
        .find(|(candidate, _)| *candidate == symbol)
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
