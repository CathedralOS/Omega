//! Finite path conditions over original invocation values. Each clause contains
//! signed existing checked guard atoms; arrival joins union clauses and remove
//! subsumed clauses. Cycles revisit a finite set of atoms rather than unroll ASTs.
use super::*;
use facts::BooleanPredicateRole;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EntryPaths {
    known: bool,
    clauses: Vec<Vec<Boolean>>,
}

impl EntryPaths {
    pub(crate) fn unconditional() -> Self {
        Self {
            known: true,
            clauses: vec![Vec::new()],
        }
    }
    pub(crate) fn unknown() -> Self {
        Self {
            known: false,
            clauses: Vec::new(),
        }
    }
    fn impossible() -> Self {
        Self {
            known: true,
            clauses: Vec::new(),
        }
    }

    fn insert(&mut self, clause: Vec<Boolean>) -> bool {
        if self
            .clauses
            .iter()
            .any(|old| old.iter().all(|atom| clause.contains(atom)))
        {
            return false;
        }
        self.clauses
            .retain(|old| !clause.iter().all(|atom| old.contains(atom)));
        self.clauses.push(clause);
        true
    }

    pub(crate) fn union(&mut self, incoming: &Self) -> bool {
        if !self.known {
            return false;
        }
        if !incoming.known {
            *self = Self::unknown();
            return true;
        }
        let mut changed = false;
        for clause in &incoming.clauses {
            changed |= self.insert(clause.clone());
        }
        changed
    }

    fn intersect(&self, other: &Self) -> Self {
        if !self.known || !other.known {
            return Self::unknown();
        }
        let mut result = Self::impossible();
        for left in &self.clauses {
            for right in &other.clauses {
                let mut clause = left.clone();
                let mut consistent = true;
                for atom in right {
                    if clause.iter().any(|existing| opposite(existing, atom)) {
                        consistent = false;
                        break;
                    }
                    if !clause.contains(atom) {
                        clause.push(atom.clone());
                    }
                }
                if consistent {
                    result.insert(clause);
                }
            }
        }
        result
    }

    fn from_expression(expression: &Boolean, negated: bool) -> Self {
        match expression {
            Boolean::Constant(value) => {
                if *value != negated {
                    Self::unconditional()
                } else {
                    Self::impossible()
                }
            }
            Boolean::Not(operand) => Self::from_expression(operand, !negated),
            Boolean::And { left, right } | Boolean::Or { left, right } => {
                let mut left = Self::from_expression(left, negated);
                let right = Self::from_expression(right, negated);
                let conjunction = matches!(expression, Boolean::And { .. }) != negated;
                if conjunction {
                    left.intersect(&right)
                } else {
                    left.union(&right);
                    left
                }
            }
            atom => Self {
                known: true,
                clauses: vec![vec![if negated {
                    Boolean::Not(Box::new(atom.clone()))
                } else {
                    atom.clone()
                }]],
            },
        }
    }

    pub(crate) fn expression(&self) -> Option<Boolean> {
        if !self.known {
            return None;
        }
        Some(
            self.clauses
                .iter()
                .map(|clause| {
                    clause
                        .iter()
                        .cloned()
                        .reduce(|left, right| Boolean::And {
                            left: Box::new(left),
                            right: Box::new(right),
                        })
                        .unwrap_or(Boolean::Constant(true))
                })
                .reduce(|left, right| Boolean::Or {
                    left: Box::new(left),
                    right: Box::new(right),
                })
                .unwrap_or(Boolean::Constant(false)),
        )
    }

    pub(crate) fn retained(
        semantic: &FactPlan,
        contexts: &[FactContextHandle],
        machine: SymbolHandle,
    ) -> Self {
        Self::retained_with_knowledge(semantic, contexts, machine, PathKnowledge::Complete)
    }

    fn retained_with_knowledge(
        semantic: &FactPlan,
        contexts: &[FactContextHandle],
        machine: SymbolHandle,
        knowledge: PathKnowledge,
    ) -> Self {
        let mut result = Self::unconditional();
        for context in contexts {
            for fact in semantic
                .context_view(semantic.contexts.get(*context))
                .facts()
            {
                let FactPayload::BooleanPredicate { predicate } = fact.payload else {
                    continue;
                };
                if !semantic.boolean_predicates.is_valid(predicate) {
                    return Self::unknown();
                }
                let predicate = semantic.boolean_predicates.get(predicate);
                if predicate.machine != machine {
                    continue;
                }
                match predicate.role {
                    BooleanPredicateRole::CurrentObservation => {}
                    BooleanPredicateRole::UnknownEntryPathCondition => {
                        if matches!(knowledge, PathKnowledge::Complete) {
                            return Self::unknown();
                        }
                        // This row records an unprojectable conjunct, not a
                        // false condition. Independently retained conjuncts
                        // still hold on every execution of this same path.
                    }
                    BooleanPredicateRole::EntryPathCondition => {
                        result =
                            result.intersect(&Self::from_expression(&predicate.expression, false))
                    }
                }
            }
        }
        result
    }
}

#[derive(Clone, Copy)]
enum PathKnowledge {
    Complete,
    Necessary,
}

fn opposite(left: &Boolean, right: &Boolean) -> bool {
    matches!(left, Boolean::Not(operand) if operand.as_ref() == right)
        || matches!(right, Boolean::Not(operand) if operand.as_ref() == left)
}

pub(crate) fn entry_path_condition(
    semantic: &FactPlan,
    contexts: &[FactContextHandle],
    machine: SymbolHandle,
) -> Option<Boolean> {
    EntryPaths::retained(semantic, contexts, machine).expression()
}

/// A necessary condition on the live path: execution implies the returned
/// formula. Crash possibilities need this overapproximation, not a complete
/// reconstruction of every guard. An unknown conjunct adds no restriction;
/// current-storage observations add none either. The complete-path query and
/// its union across alternative arrivals retain their original semantics.
pub(crate) fn entry_path_necessary_condition(
    semantic: &FactPlan,
    contexts: &[FactContextHandle],
    machine: SymbolHandle,
) -> Option<Boolean> {
    EntryPaths::retained_with_knowledge(semantic, contexts, machine, PathKnowledge::Necessary)
        .expression()
}

pub(crate) fn seed_entry_paths(
    semantic: &mut FactPlan,
    machine: SymbolHandle,
    entry_state: SymbolHandle,
    point: ProgramPoint,
    paths: &EntryPaths,
) -> FactContextHandle {
    let expression = paths.expression();
    let predicate = semantic.boolean_predicates.append(facts::BooleanPredicate {
        role: if expression.is_some() {
            BooleanPredicateRole::EntryPathCondition
        } else {
            BooleanPredicateRole::UnknownEntryPathCondition
        },
        machine,
        entry_state,
        expression: expression.unwrap_or(Boolean::Constant(false)),
        bindings: HandleSpan::empty(),
    });
    let fact = semantic.append_fact(Fact {
        place: FactPlace::Unknown,
        point,
        origin: FactOrigin::TransitionGuard,
        evidence: Default::default(),
        payload: FactPayload::BooleanPredicate { predicate },
    });
    let mut refs = HandleSpan::empty();
    semantic.append_ref(&mut refs, fact);
    semantic.append_context(point, refs)
}

/// Capture when the guard runs: later writes do not revise this historical
/// proposition, and it never supplies a present-storage observation.
pub(crate) fn capture_entry_guard(
    program: &dyn ::validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    semantic: &mut FactPlan,
    state_symbol: SymbolHandle,
    expression: typed_trees::expression::ExpressionHandle,
    value: bool,
    point: ProgramPoint,
    subjects: &mut impl FnMut(
        typed_trees::expression::ExpressionHandle,
    ) -> Option<(
        u32,
        Vec<checked_trees::CheckedStructuralPredicatePathSegment>,
    )>,
) -> Option<FactContextHandle> {
    let (machine, state) = validation::find_state(program, state_symbol)?;
    let expression = crate::values::lower_entry_predicate(
        program, operators, machine, state, expression, subjects,
    )
    .map(|expression| {
        if value {
            expression
        } else {
            Boolean::Not(Box::new(expression))
        }
    });
    capture_entry_expression(program, semantic, state_symbol, expression, point)
}

/// Publish an already authenticated original-entry proposition. A failed
/// projection stays explicitly unknown, never a false path. Callers that have
/// no new observation may omit this capture and keep their earlier facts.
pub(crate) fn capture_entry_expression(
    program: &dyn ::validation::CheckingInput,
    semantic: &mut FactPlan,
    state_symbol: SymbolHandle,
    expression: Option<Boolean>,
    point: ProgramPoint,
) -> Option<FactContextHandle> {
    let (machine, _) = validation::find_state(program, state_symbol)?;
    let entry_index = validation::activation_target_state_index(program, machine, machine.symbol)?;
    let entry = program.machine_states(machine).get(entry_index)?;
    let paths = expression
        .map(|expression| EntryPaths::from_expression(&expression, false))
        .unwrap_or_else(EntryPaths::unknown);
    Some(seed_entry_paths(
        semantic,
        machine.symbol,
        entry.symbol,
        point,
        &paths,
    ))
}

/// Compose path prerequisites using the same finite clause normalization as
/// arrival flow. Both arguments are known formulas, so no unknown is minted.
pub(crate) fn conjunction(left: Boolean, right: Boolean) -> Boolean {
    EntryPaths::from_expression(&left, false)
        .intersect(&EntryPaths::from_expression(&right, false))
        .expression()
        .expect("known checked path conjunction")
}
