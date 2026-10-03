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
                    BooleanPredicateRole::UnknownEntryPathCondition => return Self::unknown(),
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
    program: &TypedTrees,
    operators: &checked_trees::CheckedOperatorFacts,
    semantic: &mut FactPlan,
    state: SymbolHandle,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
    value: bool,
    point: ProgramPoint,
    admit_subject: &impl Fn(&crate::flow::CanonicalPlace) -> bool,
) -> Option<FactContextHandle> {
    let (machine, _) = crate::semantic::calls::find_state_with_machine(program, state)?;
    let entry_index = crate::checks::termination::named_transition_target_state_index(
        program,
        machine,
        machine.symbol,
    )?;
    let entry = program.machine_states(machine).get(entry_index)?;
    let paths = crate::facts::entry_boolean_predicate(
        program,
        operators,
        machine.symbol,
        state,
        statement_index,
        expression,
        admit_subject,
    )
    .map(|expression| EntryPaths::from_expression(&expression, !value))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relational_entry_paths_union_arrivals_without_accumulating_loop_copies() {
        let flag = Boolean::Parameter { position: 0 };
        let other = Boolean::Parameter { position: 1 };
        let precise = EntryPaths::from_expression(&conjunction(flag.clone(), other), false);
        let broad = EntryPaths::from_expression(&flag, false);
        let mut paths = precise.clone();
        assert!(paths.union(&broad));
        assert_eq!(paths.expression(), Some(flag));
        for _ in 0..8 {
            assert!(!paths.union(&precise));
        }
        assert!(paths.union(&EntryPaths::unknown()));
        assert_eq!(paths.expression(), None);
        assert!(!paths.union(&EntryPaths::unconditional()));
        assert_eq!(
            paths.expression(),
            None,
            "an unknown arrival is not a proved unconditional path"
        );
    }

    #[test]
    fn relational_entry_path_survives_mutation_without_proving_current_value() {
        let program = crate::tests::front_end::typed_program("machine branch(flag: bool) {}");
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "branch")
            .unwrap()
            .symbol;
        let flag = Boolean::Parameter { position: 0 };
        let predicate = bind_entry(&program, machine, flag.clone()).unwrap();
        let place = predicate.bindings[0].place.clone();
        let point = ProgramPoint::State {
            machine_symbol: machine,
            state_symbol: predicate.entry_state,
        };
        let mut semantic = FactPlan::default();
        let history = seed_entry_paths(
            &mut semantic,
            machine,
            predicate.entry_state,
            point,
            &EntryPaths::from_expression(&flag, false),
        );
        let current = seed(
            &mut semantic,
            &predicate,
            point,
            FactOrigin::TransitionGuard,
        );
        let mut references = arena::Arena::default();
        let source = references.insert_many(
            [history, current].map(|context| checked_trees::FlowSemanticContextRef { context }),
        );
        assert_eq!(
            crate::checks::contracts::boolean_value_at_place(
                &program,
                &semantic,
                &[history, current],
                &place
            ),
            Some(true)
        );
        let surviving = crate::flow::filter_contexts_after_place_mutations(
            &program,
            &semantic,
            &Default::default(),
            &mut references,
            &mut Default::default(),
            &mut Default::default(),
            source,
            &[place.clone()],
            checked_trees::FlowInvalidationSource::Statement { statement_index: 0 },
        );
        let contexts = references
            .span_or_empty(surviving)
            .iter()
            .map(|reference| reference.context)
            .collect::<Vec<_>>();
        assert_eq!(
            entry_path_condition(&semantic, &contexts, machine),
            Some(flag)
        );
        assert_eq!(
            crate::checks::contracts::boolean_value_at_place(
                &program, &semantic, &contexts, &place
            ),
            None
        );
    }
}

#[cfg(test)]
mod evaluation_prefix_tests {
    use super::*;

    #[test]
    fn relational_entry_guard_uses_only_overlapping_prior_operand_writes() {
        let program = crate::tests::front_end::typed_program(
            "data Flags { ready: bool; other: bool; }
             machine inspect(mut flags: Flags) -> bool { flags.ready }",
        );
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "inspect")
            .unwrap();
        let state = &program.machine_states(machine)[0];
        let (statement_index, expression) = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .enumerate()
            .find_map(|(index, statement)| match statement {
                typed_trees::statement::StatementNode::Expression(expression) => {
                    Some((index, *expression))
                }
                _ => None,
            })
            .expect("field observation");
        let ready = Boolean::StructuralParameterField {
            parameter_position: 0,
            path: vec![facts::CheckedStructuralPredicatePathSegment::Field(
                "ready".into(),
            )],
        };
        let other = Boolean::StructuralParameterField {
            parameter_position: 0,
            path: vec![facts::CheckedStructuralPredicatePathSegment::Field(
                "other".into(),
            )],
        };
        let ready_place = bind_entry(&program, machine.symbol, ready.clone())
            .unwrap()
            .bindings[0]
            .place
            .clone();
        let other_place = bind_entry(&program, machine.symbol, other)
            .unwrap()
            .bindings[0]
            .place
            .clone();
        let mut control = checked_trees::FlowControlFacts::default();
        let mut writes = arena::HandleSpan::empty();
        let before_any_write = writes;
        for (place, expected) in [(other_place, Some(ready.clone())), (ready_place, None)] {
            let segments = control
                .operand_referent_segments
                .insert_many(place.segments);
            control.operand_origin_writes.append_to_span(
                &mut writes,
                checked_trees::FlowOperandReferent {
                    root: place.root,
                    segments,
                },
            );
            let mut semantic = FactPlan::default();
            let context = capture_entry_guard(
                &program,
                &Default::default(),
                &mut semantic,
                state.symbol,
                statement_index,
                expression,
                true,
                ProgramPoint::Statement {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                    statement_index,
                },
                &|place| {
                    crate::flow::entry_origin_is_stable(&program, &control, writes, false, place)
                },
            )
            .unwrap();
            assert_eq!(
                entry_path_condition(&semantic, &[context], machine.symbol),
                expected
            );
        }
        // Appending a later overlapping write cannot revise the earlier read.
        let mut semantic = FactPlan::default();
        let context = capture_entry_guard(
            &program,
            &Default::default(),
            &mut semantic,
            state.symbol,
            statement_index,
            expression,
            true,
            ProgramPoint::Statement {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
                statement_index,
            },
            &|place| {
                crate::flow::entry_origin_is_stable(
                    &program,
                    &control,
                    before_any_write,
                    false,
                    place,
                )
            },
        )
        .unwrap();
        assert_eq!(
            entry_path_condition(&semantic, &[context], machine.symbol),
            Some(ready)
        );
    }
}
