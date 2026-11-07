//! A portable interface returns through its complete retained body roster.
//! The interface owns no flow captures. Each body's equation supplies actual
//! origins in the common telescope, and every body remains a dependency.

use super::{Captures, CheckedReturnedReferencePlan, State, completions::ReturnCoverage};

/// None inside Some means an ordinary state. A missing or incompatible family
/// is unavailable data, never an empty list of returned descriptors.
pub(super) fn entries<'a>(
    program: &'a dyn crate::CheckingInput,
    state: &State,
) -> Option<Option<Vec<&'a State>>> {
    let machine = program.machine_holding_state(state.symbol)?;
    if !machine.target_family_interface {
        return Some(None);
    }
    if program.machine_states(machine).first()?.symbol != state.symbol {
        return None;
    }
    let parameters = program.state_parameters(state);
    let mut entries = Vec::new();
    for member in crate::target_family_members(program, machine)? {
        let body = program.machine(member.machine)?;
        if !body.body_is_present || !body.supply_mode.is_checked_body() {
            return None;
        }
        let entry = program.machine_states(body).first()?;
        let actual = program.state_parameters(entry);
        if program.normalized_type_identity(entry.return_type)
            != program.normalized_type_identity(state.return_type)
            || actual.len() != parameters.len()
            || actual.iter().zip(parameters).any(|(actual, expected)| {
                actual.is_const != expected.is_const
                    || actual.is_mutable != expected.is_mutable
                    || actual.relevance != expected.relevance
                    || program.normalized_type_identity(actual.type_reference)
                        != program.normalized_type_identity(expected.type_reference)
            })
        {
            return None;
        }
        entries.push(entry);
    }
    Some(Some(entries))
}

impl Captures<'_> {
    pub(super) fn family_return(
        &self,
        entries: &[&State],
    ) -> Option<Vec<CheckedReturnedReferencePlan>> {
        let relations = self.relations?;
        let observed = self.observed?;
        let mut output = Vec::new();
        let mut returning = false;
        for entry in entries {
            returning |= crate::normal_completion_possible(self.program, entry)?;
            for source in relations.approximation(entry.symbol, observed) {
                if !output.contains(&source) {
                    output.push(source);
                    if output.len() > super::super::MAX_REFERENCE_ORIGIN_LEAVES {
                        return None;
                    }
                }
            }
        }
        if returning {
            observed.completed(ReturnCoverage::alternatives(
                entries.iter().map(|entry| entry.symbol),
            )?);
        } else {
            observed.nonreturning();
        }
        Some(output)
    }
}
