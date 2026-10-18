//! Finiteness of state write equations follows storage-root substitutions, not
//! equal parameter counts. A transition can drop a root or acquire a projection
//! of stable `self` without growing paths on repeated traversal.
//!
//! Every suffix-preserving substitution contributes a root dependency. A
//! dependency that can add a nonempty prefix must not lie on a dependency cycle.
//! Collection-coarse actuals also count as projections: their string footprints
//! erase selectors, but structured mutation summaries retain those selectors.
//! Shared completeness must bound both consumers without an iteration limit;
//! ordinary frame inference still proves effects and storage origins.

use super::inference::FrameInference;
use super::local_aliases::rebase_local_alias_path;
use super::place_paths::{FramePathPrecision, split_place_root};
use super::stored_origins::canonical_write_origins;
use super::transition_equations::CycleFrameEquation;
use super::transition_topology::strongly_connected_components;
use super::transparent_results::transparent_place_expression_origins;
use super::type_capabilities::parameter_may_carry_write;
use crate::declarations::symbols::TopLevelSymbols;
use language_core::is_self_receiver;
use symbols::SymbolHandle;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Root {
    Receiver,
    Parameter(SymbolHandle),
}

struct StateRoot {
    state: SymbolHandle,
    root: Root,
}

struct Dependency {
    source: usize,
    target: usize,
    grows: bool,
}

#[derive(Default)]
pub(super) struct Substitutions {
    roots: Vec<StateRoot>,
    dependencies: Vec<Dependency>,
}

impl Substitutions {
    /// Storage-relation solvers use the same root-dependency finiteness law,
    /// including shared roots whose provenance (rather than writes) is carried.
    pub(super) fn parameter(&mut self, state: SymbolHandle, parameter: SymbolHandle) -> usize {
        self.root(state, Root::Parameter(parameter))
    }
    fn root(&mut self, state: SymbolHandle, root: Root) -> usize {
        if let Some(position) = self
            .roots
            .iter()
            .position(|candidate| candidate.state == state && candidate.root == root)
        {
            return position;
        }
        let position = self.roots.len();
        self.roots.push(StateRoot { state, root });
        position
    }

    pub(super) fn insert(&mut self, source: usize, target: usize, grows: bool) {
        self.dependencies.push(Dependency {
            source,
            target,
            grows,
        });
    }

    pub(super) fn is_finite(&self) -> bool {
        let mut successors = vec![Vec::new(); self.roots.len()];
        for dependency in &self.dependencies {
            successors[dependency.source].push(dependency.target);
        }
        let components = strongly_connected_components(&successors);
        self.dependencies.iter().all(|dependency| {
            !dependency.grows || components[dependency.source] != components[dependency.target]
        })
    }
}

/// Prove that the equation instantiator and its structured-place consumers
/// cannot create unbounded write paths. Opaque actuals remain opaque. Duplicate and
/// omitted roots are harmless for this may-write union; their borrow legality
/// is decided independently, not granted by this finiteness proof.
pub(super) fn substitutions_are_finite(
    program: &dyn crate::CheckingInput,
    equations: &[CycleFrameEquation<'_>],
    symbols: &TopLevelSymbols<'_>,
    outer_inference: &FrameInference,
) -> bool {
    let mut substitutions = Substitutions::default();
    for equation in equations {
        let source_state = equation.state.symbol;
        let mut inference = outer_inference.clone();
        for local in &equation.stored {
            inference.record_local(local);
        }
        for edge in &equation.edges {
            let Some(target) = equations
                .iter()
                .find(|candidate| candidate.state.symbol == edge.target)
            else {
                return false;
            };
            // An attached receiver has the same storage identity across every
            // internal state edge, regardless of its other parameters.
            let target_receiver = substitutions.root(edge.target, Root::Receiver);
            let source_receiver = substitutions.root(source_state, Root::Receiver);
            substitutions.insert(target_receiver, source_receiver, false);
            let parameters = program
                .state_parameters(target.state)
                .iter()
                .collect::<Vec<_>>();
            if parameters.len() != edge.arguments.len() {
                return false;
            }
            for (parameter, actual) in parameters.iter().zip(&edge.arguments) {
                if !parameter_may_carry_write(program, parameter) {
                    continue;
                }
                let target_root =
                    substitutions.root(edge.target, Root::Parameter(parameter.symbol));
                let Some(origins) =
                    transparent_place_expression_origins(program, *actual, symbols, &mut inference)
                else {
                    return false;
                };
                for origin in origins {
                    // Mirror expand_write_path's two contributions: the
                    // rebased spelling and every overlapping stored leaf.
                    // Looking at the base includes every leaf any appended
                    // suffix could select. Selection can remove suffix text;
                    // a nonempty destination prefix or erased collection
                    // selector can grow the structured path.
                    let mut spelling = origin.clone();
                    spelling.path = rebase_local_alias_path(
                        program,
                        &origin.path,
                        &equation.local_alias_origins,
                    );
                    let (original_root, _) = split_place_root(&origin.path);
                    if equation.local_alias_origins.iter().any(|(name, alias)| {
                        name == original_root
                            && alias.precision == FramePathPrecision::CollectionCoarse
                    }) {
                        spelling.precision = FramePathPrecision::CollectionCoarse;
                    }
                    let Some(mut sources) = canonical_write_origins(
                        program,
                        &origin,
                        &equation.local_alias_origins,
                        &equation.stored,
                    ) else {
                        return false;
                    };
                    sources.push(spelling);
                    for source in sources {
                        let (name, suffix) = split_place_root(&source.path);
                        let root = if is_self_receiver(name) {
                            Root::Receiver
                        } else if let Some(parameter) = program
                            .state_parameters(equation.state)
                            .iter()
                            .find(|parameter| parameter.name.as_str() == name)
                        {
                            Root::Parameter(parameter.symbol)
                        } else if equation.locals.iter().any(|local| local == name) {
                            // Private local writes do not enter caller frames;
                            // external leaves were included separately above.
                            continue;
                        } else {
                            return false;
                        };
                        let source_root = substitutions.root(source_state, root);
                        substitutions.insert(
                            target_root,
                            source_root,
                            !suffix.is_empty()
                                || !source.source.recasts.is_empty()
                                || origin.precision == FramePathPrecision::CollectionCoarse
                                || source.precision == FramePathPrecision::CollectionCoarse,
                        );
                    }
                }
            }
        }
    }
    substitutions.is_finite()
}
