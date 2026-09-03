mod calls;
mod expressions;
mod statements;

use psi_symbols::SymbolHandle;
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;

use super::facts::RangeFacts;
use super::seed_machine_requires;

use self::statements::collect_state_argument_facts_from_statement;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct StateArgumentFacts {
    state: SymbolHandle,
    parameters: Vec<ParameterFacts>,
    index_proofs: MergedIndexProofs,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ParameterFacts {
    symbol: SymbolHandle,
    name: String,
    is_self: bool,
    length: MergedFact<usize>,
    integer: MergedFact<i64>,
    /// The tightest EXCLUSIVE upper bound every incoming edge proves for
    /// this parameter's argument (R4 transport: an ensures-bounded value
    /// passed as a transition argument carries its bound into the param).
    /// The meet is the MAX over edges -- the weakest bound all satisfy;
    /// one unbounded edge poisons it.
    upper_bound: MergedBound,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum MergedBound {
    #[default]
    Unseen,
    Known(i64),
    Unbounded,
}

impl MergedBound {
    fn merge(&mut self, value: Option<i64>) {
        *self = match (*self, value) {
            (Self::Unseen, Some(bound)) => Self::Known(bound),
            (Self::Known(existing), Some(bound)) => Self::Known(existing.max(bound)),
            (_, None) | (Self::Unbounded, _) => Self::Unbounded,
        };
    }

    fn get(self) -> Option<i64> {
        match self {
            Self::Known(bound) => Some(bound),
            Self::Unseen | Self::Unbounded => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum MergedFact<T> {
    #[default]
    Unseen,
    Known(T),
    Conflicting,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ParameterIndexProof {
    collection_parameter: usize,
    index_parameter: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct MergedIndexProofs {
    proofs: Option<Vec<ParameterIndexProof>>,
}

impl<T: Copy + Eq> MergedFact<T> {
    fn merge(&mut self, value: Option<T>) {
        match (*self, value) {
            (Self::Unseen, Some(value)) => *self = Self::Known(value),
            (Self::Unseen | Self::Known(_), None) => {}
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

impl MergedIndexProofs {
    fn merge(&mut self, incoming: Vec<ParameterIndexProof>) {
        if incoming.is_empty() {
            return;
        }

        let Some(existing) = &mut self.proofs else {
            self.proofs = Some(incoming);
            return;
        };

        existing.retain(|proof| incoming.contains(proof));
    }

    fn get(&self) -> &[ParameterIndexProof] {
        self.proofs.as_deref().unwrap_or(&[])
    }
}

/// Bound on the number of propagation passes. Each pass can only move a
/// `MergedFact` monotonically (`Unseen -> Known -> Conflicting`), so the
/// computation is guaranteed to reach a fixpoint; the cap is a defensive guard
/// against any future non-monotonic change introducing a cycle.
const MAX_PROPAGATION_PASSES: usize = 64;

pub(super) fn collect_state_argument_facts(
    program: &psi_typed_trees::TypedTrees,
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
        if let Some(bound) = parameter.upper_bound.get() {
            facts.prove_index_upper_bound(parameter.name.clone(), bound);
        }
    }

    for proof in state_facts.index_proofs.get() {
        let Some(collection) = state_facts.parameters.get(proof.collection_parameter) else {
            continue;
        };
        let Some(index) = state_facts.parameters.get(proof.index_parameter) else {
            continue;
        };
        facts.prove_index(collection.name.clone(), index.name.clone());
        facts.prove_range_bound(collection.name.clone(), index.name.clone());
    }
}
