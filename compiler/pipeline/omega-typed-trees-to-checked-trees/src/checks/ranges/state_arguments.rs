mod calls;
mod expressions;
mod statements;

use omega_core::symbols::SymbolHandle;
use omega_typed_trees::machine::Machine;
use omega_typed_trees::state::State;

use super::facts::RangeFacts;
use super::seed_machine_requires;

use self::statements::collect_state_argument_facts_from_statement;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct StateArgumentFacts {
    state: SymbolHandle,
    parameters: Vec<ParameterFacts>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
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

/// Bound on the number of propagation passes. Each pass can only move a
/// `MergedFact` monotonically (`Unseen -> Known -> Conflicting`), so the
/// computation is guaranteed to reach a fixpoint; the cap is a defensive guard
/// against any future non-monotonic change introducing a cycle.
const MAX_PROPAGATION_PASSES: usize = 64;

pub(super) fn collect_state_argument_facts(
    program: &omega_typed_trees::TypedTrees,
    field_lengths: &[(SymbolHandle, String, usize)],
    machine: &Machine,
) -> Vec<StateArgumentFacts> {
    // Facts about a state's arguments are derived from the call/transition
    // sites that target it. On a recursive or cyclic control-flow path the
    // arguments handed to the *next* state are themselves built from the
    // current state's parameters, so a single forward pass cannot see facts
    // that only become available once the current state's own parameters have
    // been constrained by its incoming edges.
    //
    // To propagate facts around recursion and cycles we iterate to a fixpoint:
    // before analysing a state's outgoing arguments we seed the working facts
    // with everything already known about that state's own parameters (its
    // incoming-edge facts). Repeating until nothing changes lets a bound flow
    // forward across each cyclic edge.
    let mut collected: Vec<StateArgumentFacts> = Vec::new();

    for _ in 0..MAX_PROPAGATION_PASSES {
        let previous = collected.clone();

        for state in program.machine_states(machine) {
            let mut facts = RangeFacts::new(field_lengths);
            seed_machine_requires(program, &mut facts, machine);
            // Seed the source state's own parameter facts gathered so far so
            // that arguments derived from them (e.g. `remaining - 1`) carry the
            // refined bound into the cyclic callee on this pass.
            seed_state_argument_facts(&mut facts, state, &previous);

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

        if collected == previous {
            break;
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
