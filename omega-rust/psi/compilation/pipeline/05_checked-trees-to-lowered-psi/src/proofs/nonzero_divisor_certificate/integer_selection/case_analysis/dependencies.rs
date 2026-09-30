//! Select case assumptions through the goal's transitive value dependencies.
//!
//! The selection splits into a goal-independent half and a per-goal half.
//! Every projected fact's value dependencies, the literal identities excluded
//! from them, and the connected components those dependencies form are pure
//! functions of the `(assumptions, semantic_axioms)` scope. The implication
//! search asks for connected facts at every goal, premise probe and case
//! split under a handful of scopes, so the scope half is computed once per
//! scope content and only the goal's own dependencies are walked per call.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use semantic_vocabulary::{Proposition, ScalarTerm, ValueId};

use super::super::super::integer_evidence::{Citation, ProjectedFact, projected_facts};
use super::super::super::scope_identity::{self, ScopeDerived};

struct Dependencies {
    values: BTreeSet<ValueId>,
    complete: bool,
}

impl Dependencies {
    fn of(proposition: &Proposition, literals: &BTreeSet<ValueId>) -> Self {
        let mut values = BTreeSet::new();
        let complete = proposition.visit_value_ids(|value| {
            if !literals.contains(&value) {
                values.insert(value);
            }
        });
        Self { values, complete }
    }
}

pub(super) fn connected_cases<'a>(
    goal: &Proposition,
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> Vec<ProjectedFact<'a>> {
    let scope = scope_graph(assumptions, semantic_axioms);
    scope.connected(&scope.cases, goal, assumptions, semantic_axioms)
}

/// Implications the goal can reach through value dependencies. A conditional
/// fact about values the goal never touches cannot shorten its proof, and
/// trying each one recursively multiplies the search by every guard in scope.
pub(super) fn connected_implications<'a>(
    goal: &Proposition,
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> Vec<ProjectedFact<'a>> {
    let scope = scope_graph(assumptions, semantic_axioms);
    scope.connected(&scope.implications, goal, assumptions, semantic_axioms)
}

/// The goal-independent dependency graph of one scope's projected facts.
///
/// A fact is connected to a goal when a chain of facts sharing values joins
/// the fact's values to the goal's, or to any fact whose dependencies are
/// incomplete. That reachability is exactly membership in one connected
/// component of the graph whose hyperedges are the facts' value sets, so the
/// scope labels components once and each goal only looks up its own values'
/// labels.
struct ScopeGraph {
    literals: BTreeSet<ValueId>,
    /// The component of every value some projected fact depends on.
    components: BTreeMap<ValueId, usize>,
    /// Components every goal reaches: structural and opaque identities are
    /// outside this value-only graph, so facts with incomplete dependencies
    /// keep their links to ordinary values conservatively.
    incomplete: BTreeSet<usize>,
    /// First projected occurrence of each distinct implication, in citation
    /// order.
    implications: Vec<Candidate>,
    /// First projected occurrence of each distinct disjunction, in citation
    /// order.
    cases: Vec<Candidate>,
}

/// One selectable projected fact, located by citation so it resolves against
/// any content-equal scope.
struct Candidate {
    citation: Citation,
    projection: Rc<[usize]>,
    /// The fact's component, or `None` when its dependencies are incomplete
    /// or empty and every goal therefore keeps it.
    component: Option<usize>,
}

impl ScopeGraph {
    fn new(assumptions: &[Proposition], semantic_axioms: &[Proposition]) -> Self {
        // A conjunction packages independent facts; it must not connect their
        // value dependencies. Only its exact projected leaves become graph rows.
        let facts = projected_facts(assumptions, semantic_axioms);
        let literals = literal_values(&facts);
        let dependencies = facts
            .iter()
            .map(|fact| Dependencies::of(fact.proposition, &literals))
            .collect::<Vec<_>>();
        let mut nodes = BTreeMap::<ValueId, usize>::new();
        let mut parents = Vec::<usize>::new();
        for dependency in &dependencies {
            let mut first = None;
            for &value in &dependency.values {
                let node = *nodes.entry(value).or_insert_with(|| {
                    parents.push(parents.len());
                    parents.len() - 1
                });
                match first {
                    None => first = Some(node),
                    Some(first) => union(&mut parents, first, node),
                }
            }
        }
        let components = nodes
            .into_iter()
            .map(|(value, node)| (value, root(&mut parents, node)))
            .collect::<BTreeMap<_, _>>();
        let component_of =
            |dependency: &Dependencies| dependency.values.first().map(|value| components[value]);
        let incomplete = dependencies
            .iter()
            .filter(|dependency| !dependency.complete)
            .filter_map(component_of)
            .collect::<BTreeSet<_>>();
        let mut implications = Vec::<(&Proposition, Candidate)>::new();
        let mut cases = Vec::<(&Proposition, Candidate)>::new();
        for (fact, dependency) in facts.iter().zip(&dependencies) {
            let selected = match fact.proposition {
                Proposition::Implication { .. } => &mut implications,
                Proposition::Disjunction(_) => &mut cases,
                _ => continue,
            };
            // Equal propositions have equal dependencies, so the first
            // occurrence is selected exactly when any later one would be.
            if selected
                .iter()
                .any(|(previous, _)| *previous == fact.proposition)
            {
                continue;
            }
            let component = if dependency.complete {
                component_of(dependency)
            } else {
                None
            };
            selected.push((
                fact.proposition,
                Candidate {
                    citation: fact.citation(),
                    projection: Rc::clone(&fact.projection),
                    component,
                },
            ));
        }
        Self {
            implications: implications
                .into_iter()
                .map(|(_, candidate)| candidate)
                .collect(),
            cases: cases.into_iter().map(|(_, candidate)| candidate).collect(),
            literals,
            components,
            incomplete,
        }
    }

    fn connected<'a>(
        &self,
        candidates: &[Candidate],
        goal: &Proposition,
        assumptions: &'a [Proposition],
        semantic_axioms: &'a [Proposition],
    ) -> Vec<ProjectedFact<'a>> {
        let goal = Dependencies::of(goal, &self.literals);
        // Structural and opaque identities are outside this value-only graph.
        // Keep their cases and their links to ordinary values conservatively.
        let retain_all = !goal.complete || goal.values.is_empty();
        let mut relevant = self.incomplete.clone();
        relevant.extend(
            goal.values
                .iter()
                .filter_map(|value| self.components.get(value).copied()),
        );
        candidates
            .iter()
            .filter(|candidate| {
                retain_all
                    || candidate
                        .component
                        .is_none_or(|component| relevant.contains(&component))
            })
            .map(|candidate| {
                ProjectedFact::resolve(
                    candidate.citation,
                    &candidate.projection,
                    assumptions,
                    semantic_axioms,
                )
            })
            .collect()
    }
}

fn root(parents: &mut [usize], mut node: usize) -> usize {
    while parents[node] != node {
        parents[node] = parents[parents[node]];
        node = parents[node];
    }
    node
}

fn union(parents: &mut [usize], left: usize, right: usize) {
    let left = root(parents, left);
    let right = root(parents, right);
    if left != right {
        parents[right] = left;
    }
}

thread_local! {
    /// Dependency graphs of recently searched scopes. The implication search
    /// nests a few assumption extensions under one axiom roster, and
    /// predicate conversion adds one converted roster beside it.
    static SCOPE_GRAPHS: RefCell<ScopeDerived<ScopeGraph>> =
        const { RefCell::new(ScopeDerived::new()) };
}

fn scope_graph(assumptions: &[Proposition], semantic_axioms: &[Proposition]) -> Rc<ScopeGraph> {
    scope_identity::derived(&SCOPE_GRAPHS, assumptions, semantic_axioms, || {
        ScopeGraph::new(assumptions, semantic_axioms)
    })
}

fn literal_values(facts: &[ProjectedFact<'_>]) -> BTreeSet<ValueId> {
    let mut literals = BTreeSet::new();
    let mut aliases = Vec::new();
    let mut pending = facts
        .iter()
        .map(|fact| fact.proposition)
        .collect::<Vec<_>>();
    while let Some(fact) = pending.pop() {
        match fact {
            Proposition::Conjunction(parts) => pending.extend(parts),
            Proposition::Equal(left, right) => {
                for (value, definition) in [(left, right), (right, left)] {
                    let ScalarTerm::Value { id, scalar_type } = value else {
                        continue;
                    };
                    if *scalar_type != definition.scalar_type() {
                        continue;
                    }
                    match definition {
                        ScalarTerm::Boolean(_) => {
                            literals.insert(*id);
                        }
                        ScalarTerm::Integer { scalar_type, value }
                            if scalar_type.admits(*value) =>
                        {
                            literals.insert(*id);
                        }
                        ScalarTerm::Value { id: other, .. } => aliases.push((*id, *other)),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    // Only unconditional, exact same-type aliases inherit literal identity.
    // A shared zero SSA value must not connect every independent comparison.
    loop {
        let previous = literals.len();
        for (left, right) in &aliases {
            if literals.contains(right) {
                literals.insert(*left);
            }
        }
        if literals.len() == previous {
            break;
        }
    }
    literals
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use semantic_vocabulary::{
        IntegerSign, IntegerType, IntegerValue, Proposition, PropositionId, ScalarTerm, ScalarType,
        ValueId,
    };

    use super::super::super::super::integer_evidence::{Citation, ProjectedFact, projected_facts};
    use super::{Dependencies, connected_cases, connected_implications, literal_values};

    /// The fixpoint closure the component labelling replaces: grow the
    /// relevant values through every fact that meets them until nothing
    /// changes, then keep each selected fact that reaches them.
    fn closure_reference<'a>(
        goal: &Proposition,
        assumptions: &'a [Proposition],
        semantic_axioms: &'a [Proposition],
        selected: impl Fn(&Proposition) -> bool,
    ) -> Vec<ProjectedFact<'a>> {
        let facts = projected_facts(assumptions, semantic_axioms);
        let literals = literal_values(&facts);
        let goal = Dependencies::of(goal, &literals);
        let dependencies = facts
            .iter()
            .map(|fact| Dependencies::of(fact.proposition, &literals))
            .collect::<Vec<_>>();
        let mut relevant = goal.values;
        let retain_all = !goal.complete || relevant.is_empty();
        for dependency in &dependencies {
            if !dependency.complete {
                relevant.extend(&dependency.values);
            }
        }
        loop {
            let previous = relevant.len();
            for dependency in &dependencies {
                if !dependency.values.is_disjoint(&relevant) {
                    relevant.extend(&dependency.values);
                }
            }
            if relevant.len() == previous {
                break;
            }
        }
        let mut cases: Vec<ProjectedFact<'a>> = Vec::new();
        for (fact, dependency) in facts.into_iter().zip(dependencies) {
            let proposition = fact.proposition;
            if selected(proposition)
                && (retain_all
                    || !dependency.complete
                    || dependency.values.is_empty()
                    || !dependency.values.is_disjoint(&relevant))
                && !cases
                    .iter()
                    .any(|previous| previous.proposition == proposition)
            {
                cases.push(fact);
            }
        }
        cases
    }

    fn located(facts: &[ProjectedFact<'_>]) -> Vec<(bool, usize, Vec<usize>)> {
        facts
            .iter()
            .map(|fact| {
                let (assumed, index) = match fact.citation() {
                    Citation::Assumption(index) => (true, index),
                    Citation::SemanticAxiom(index) => (false, index),
                };
                (assumed, index, fact.projection.to_vec())
            })
            .collect()
    }

    fn integer_type() -> IntegerType {
        IntegerType::new(IntegerSign::Unsigned, 32).unwrap()
    }

    fn value(identity: u64) -> ScalarTerm {
        ScalarTerm::value(
            ValueId::new(identity).unwrap(),
            ScalarType::Integer(integer_type()),
        )
    }

    fn flag(identity: u64, polarity: bool) -> Proposition {
        Proposition::Equal(
            ScalarTerm::value(ValueId::new(identity).unwrap(), ScalarType::Boolean),
            ScalarTerm::Boolean(polarity),
        )
    }

    fn literal(number: u128) -> ScalarTerm {
        ScalarTerm::integer(integer_type(), IntegerValue::Unsigned(number)).unwrap()
    }

    fn implies(premise: Proposition, conclusion: Proposition) -> Proposition {
        Proposition::Implication {
            premise: Box::new(premise),
            conclusion: Box::new(conclusion),
        }
    }

    #[test]
    fn component_selection_matches_the_fixpoint_closure() {
        let opaque = Proposition::Atom(PropositionId::new(1).unwrap());
        let cases = Proposition::Disjunction(vec![flag(10, true), flag(10, false)]);
        let assumptions = vec![
            Proposition::Equal(value(1), literal(0)),
            Proposition::Equal(value(2), value(1)),
            Proposition::LessOrEqual(value(3), value(4)),
            implies(flag(9, true), Proposition::LessOrEqual(value(4), value(5))),
            cases.clone(),
        ];
        let axioms = vec![
            implies(
                flag(10, true),
                Proposition::LessOrEqual(value(6), literal(3)),
            ),
            Proposition::Conjunction(vec![
                Proposition::LessOrEqual(value(7), value(8)),
                implies(
                    opaque.clone(),
                    Proposition::LessOrEqual(value(8), literal(1)),
                ),
            ]),
            // Reaches only the opaque fact's component.
            implies(
                flag(15, true),
                Proposition::LessOrEqual(value(8), value(16)),
            ),
            cases,
            implies(Proposition::Equal(value(2), literal(0)), Proposition::Truth),
            implies(
                flag(11, true),
                Proposition::LessOrEqual(value(12), value(13)),
            ),
        ];
        let goals = [
            Proposition::LessOrEqual(value(3), literal(9)),
            Proposition::LessOrEqual(value(6), literal(9)),
            Proposition::LessOrEqual(value(12), literal(9)),
            flag(10, true),
            Proposition::LessOrEqual(value(7), literal(2)),
            Proposition::LessOrEqual(value(2), literal(0)),
            Proposition::LessOrEqual(value(14), literal(1)),
            Proposition::Truth,
            opaque,
        ];
        let is_implication = |fact: &Proposition| matches!(fact, Proposition::Implication { .. });
        let is_case = |fact: &Proposition| matches!(fact, Proposition::Disjunction(_));
        // With and without the opaque conjunct: an incomplete fact makes its
        // whole component relevant to every goal.
        for axioms in [
            axioms.clone(),
            axioms[..1].iter().chain(&axioms[2..]).cloned().collect(),
        ] {
            let mut selections = BTreeSet::new();
            for goal in &goals {
                let implications = connected_implications(goal, &assumptions, &axioms);
                assert_eq!(
                    located(&implications),
                    located(&closure_reference(
                        goal,
                        &assumptions,
                        &axioms,
                        is_implication
                    )),
                    "implications connected to {goal:?}"
                );
                let disjunctions = connected_cases(goal, &assumptions, &axioms);
                assert_eq!(
                    located(&disjunctions),
                    located(&closure_reference(goal, &assumptions, &axioms, is_case)),
                    "cases connected to {goal:?}"
                );
                selections.insert(located(&implications));
            }
            assert!(
                selections.len() > 1,
                "the goals must select different implication rosters"
            );
        }
    }
}
