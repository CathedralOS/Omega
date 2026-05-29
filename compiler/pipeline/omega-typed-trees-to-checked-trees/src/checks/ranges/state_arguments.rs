mod calls;
mod expressions;
mod statements;

use omega_core::symbols::SymbolHandle;
use omega_typed_trees::machine::Machine;
use omega_typed_trees::state::State;

use super::facts::RangeFacts;
use super::seed_machine_requires;

use self::statements::collect_state_argument_facts_from_statement;

#[derive(Clone, Debug, Default)]
pub(super) struct StateArgumentFacts {
    state: SymbolHandle,
    parameters: Vec<ParameterFacts>,
}

#[derive(Clone, Debug)]
struct ParameterFacts {
    symbol: SymbolHandle,
    name: String,
    is_self: bool,
    length: MergedFact<usize>,
    integer: MergedFact<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MergedFact<T> {
    Unseen,
    Known(T),
    Conflicting,
}

impl<T> Default for MergedFact<T> {
    fn default() -> Self {
        Self::Unseen
    }
}

impl<T: Copy + Eq> MergedFact<T> {
    fn merge(&mut self, value: Option<T>) {
        match (*self, value) {
            (Self::Unseen, Some(value)) => *self = Self::Known(value),
            (Self::Unseen | Self::Known(_), None) => *self = Self::Conflicting,
            (Self::Known(existing), Some(value)) if existing == value => {}
            (Self::Known(_), Some(_)) | (Self::Conflicting, _) => *self = Self::Conflicting,
        }
    }

    fn get(self) -> Option<T> {
        match self {
            Self::Known(value) => Some(value),
            Self::Unseen | Self::Conflicting => None,
        }
    }
}

pub(super) fn collect_state_argument_facts(
    program: &omega_typed_trees::TypedTrees,
    field_lengths: &[(SymbolHandle, String, usize)],
    machine: &Machine,
) -> Vec<StateArgumentFacts> {
    let mut collected = Vec::new();
    for state in program.machine_states(machine) {
        let mut facts = RangeFacts::new(field_lengths);
        seed_machine_requires(program, &mut facts, machine);
        for statement in program.statement_table.statements(state.statement_nodes) {
            collect_state_argument_facts_from_statement(
                program,
                machine,
                &mut facts,
                statement,
                &mut collected,
            );
        }
    }
    collected
}

pub(super) fn seed_state_argument_facts(
    facts: &mut RangeFacts<'_>,
    state: &State,
    collected: &[StateArgumentFacts],
) {
    let Some(state_facts) = collected.iter().find(|entry| entry.state == state.symbol) else {
        return;
    };

    for parameter in &state_facts.parameters {
        facts.define_local(
            parameter.symbol,
            parameter.name.clone(),
            parameter.length.get(),
            parameter.integer.get(),
        );
    }
}
