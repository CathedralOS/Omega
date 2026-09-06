mod calls;
mod expressions;
mod statements;

use symbols::SymbolHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;

use super::facts::RangeFacts;
use super::seed_state_requires;

use self::statements::collect_state_argument_facts_from_statement;

struct StateArgumentContext<'program, 'frames> {
    program: &'program typed_trees::TypedTrees,
    machine: &'program Machine,
    state: &'program State,
    call_frames: Option<&'frames validation::CallFrameResolver<'program>>,
}

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
    /// A floor shared by all incoming collection values, not an exact extent.
    minimum_length: MergedBound,
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
    fn merge_lower(&mut self, value: Option<i64>) {
        *self = match (*self, value) {
            (Self::Unseen, Some(bound)) => Self::Known(bound),
            (Self::Known(existing), Some(bound)) => Self::Known(existing.min(bound)),
            (_, None) | (Self::Unbounded, _) => Self::Unbounded,
        };
    }

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

impl MergedIndexProofs {
    fn merge(&mut self, incoming: Vec<ParameterIndexProof>) {
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

/// A defensive bound, not proof of convergence. No inferred argument facts
/// are published if any contribution remains unsettled when it is reached.
const MAX_PROPAGATION_PASSES: usize = 64;

pub(super) fn collect_state_argument_facts<'program>(
    program: &'program typed_trees::TypedTrees,
    field_lengths: &[(SymbolHandle, String, usize)],
    machine: &'program Machine,
    call_frames: Option<&validation::CallFrameResolver<'program>>,
) -> Vec<StateArgumentFacts> {
    // Facts about a state's arguments are derived from the call/transition
    // sites that target it. On a recursive or cyclic control-flow path the
    // arguments handed to the *next* state are themselves built from the
    // current state's parameters, so a single forward pass cannot see facts
    // that only become available once the current state's own parameters have
    // been constrained by its incoming edges.
    //
    // Unreached states contribute no edge yet; a reached state with no proof
    // contributes an unknown value / empty proof set. Keeping those separate
    // allows a real entry edge to establish a recursive invariant without
    // ignoring an unbounded reachable predecessor. Each pass rebuilds all
    // contributions so provisional facts cannot survive later weakening.
    let Some(entry) = program.machine_states(machine).first() else {
        return Vec::new();
    };
    let mut collected: Vec<StateArgumentFacts> = Vec::new();

    for _ in 0..MAX_PROPAGATION_PASSES {
        let previous = std::mem::take(&mut collected);

        for state in program.machine_states(machine) {
            if state.symbol != entry.symbol
                && !previous
                    .iter()
                    .any(|incoming| incoming.state == state.symbol)
            {
                continue;
            }
            let context = StateArgumentContext {
                program,
                machine,
                state,
                call_frames,
            };
            let mut facts = RangeFacts::new(field_lengths);
            for parameter in program.state_parameters(state) {
                facts.define_local(
                    parameter.symbol,
                    parameter.name.to_string(),
                    super::arrays::fixed_array_type_length(program, parameter.type_reference),
                    None,
                );
            }
            seed_state_requires(program, &mut facts, machine, state);
            // Seed the source state's own parameter facts gathered so far so
            // that arguments derived from them (e.g. `remaining - 1`) carry the
            // refined bound into the cyclic callee on this pass.
            if state.symbol != entry.symbol {
                seed_state_argument_facts(&mut facts, state, &previous);
            }

            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                facts.statement_index = statement_index;
                collect_state_argument_facts_from_statement(
                    &context,
                    &mut facts,
                    statement,
                    &mut collected,
                );
            }
        }

        // The machine head is callable with every argument its declaration
        // permits. Internal recursion cannot narrow that external entry set.
        collected.retain(|incoming| incoming.state != entry.symbol);
        if collected == previous {
            return collected;
        }
    }

    Vec::new()
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
        if let Some(minimum) = parameter.minimum_length.get() {
            facts.prove_minimum_length(parameter.name.clone(), minimum);
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
