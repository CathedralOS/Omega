use super::*;
use crate::SourceIntegerTheory;
use crate::mathematical_core::{Declaration, check_signature, check_type};

struct OrderPremises {
    arena: TermArena,
    declarations: Vec<Declaration>,
    theory: SourceIntegerTheory,
    values: Vec<TermHandle>,
    premises: Vec<(TermHandle, TermHandle)>,
}

impl OrderPremises {
    fn new(values: usize) -> Self {
        let mut arena = TermArena::new();
        let mut declarations = crate::indexed_scheme(&mut arena);
        let theory = SourceIntegerTheory::new(&mut arena, &mut declarations).unwrap();
        let integer = arena.insert(Term::Constant {
            declaration: theory.vocabulary().carrier,
            levels: vec![],
        });
        let values = (0..values)
            .map(|_| {
                let declaration = declarations.len() as u32;
                declarations.push(Declaration::assumption(0, integer));
                arena.insert(Term::Constant {
                    declaration,
                    levels: vec![],
                })
            })
            .collect();
        Self {
            arena,
            declarations,
            theory,
            values,
            premises: Vec::new(),
        }
    }

    fn edge(&mut self, left: usize, right: usize) {
        self.edge_terms(self.values[left], self.values[right]);
    }

    fn edge_terms(&mut self, left: TermHandle, right: TermHandle) {
        let proposition = self
            .theory
            .with(&mut self.arena, &mut self.declarations, |state| {
                state.order_term_application(false, left, right)
            })
            .unwrap();
        let declaration = self.declarations.len() as u32;
        self.declarations
            .push(Declaration::assumption(0, proposition));
        let evidence = self.arena.insert(Term::Constant {
            declaration,
            levels: vec![],
        });
        self.premises.push((proposition, evidence));
    }

    fn check(&mut self, proof: TermHandle, left: usize, right: usize) {
        let goal = self
            .theory
            .with(&mut self.arena, &mut self.declarations, |state| {
                state.order_term_application(false, self.values[left], self.values[right])
            })
            .unwrap();
        let signature = check_signature(
            &mut self.arena,
            &self.declarations,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .unwrap();
        check_type(
            &mut self.arena,
            &Context::empty().with_signature(signature),
            proof,
            goal,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .unwrap();
    }
}

#[test]
fn reconverging_unsuccessful_orders_share_completed_questions() {
    for cyclic in [false, true] {
        let layers = 24;
        let mut fixture = OrderPremises::new(layers * 3 + 2);
        for layer in 0..layers {
            let start = layer * 3;
            fixture.edge(start, start + 1);
            fixture.edge(start, start + 2);
            fixture.edge(start + 1, start + 3);
            fixture.edge(start + 2, start + 3);
            if cyclic {
                fixture.edge(start + 3, start + 1);
                fixture.edge(start + 3, start + 2);
            }
        }
        let unreachable = fixture.values[layers * 3 + 1];
        let start = fixture.values[0];
        fixture
            .theory
            .with(&mut fixture.arena, &mut fixture.declarations, |state| {
                let mut search = OrderSearch::new(&fixture.premises, &[]);
                assert!(
                    state
                        .source_order_search(start, unreachable, &mut search)?
                        .is_none()
                );
                assert!(
                    search.expansions <= layers * 3 + 1,
                    "a failed diamond suffix must be expanded once, not once per incoming path"
                );
                assert!(
                    search.comparisons <= layers * 8,
                    "goal lookup must not scan every earlier question for each dependency"
                );
                Ok(())
            })
            .unwrap();
    }
}

#[test]
fn transitive_orders_produce_checked_evidence_and_require_the_connecting_premise() {
    let mut fixture = OrderPremises::new(4);
    fixture.edge(0, 1);
    fixture.edge(1, 2);
    let values = &fixture.values;
    let proof = fixture
        .theory
        .with(&mut fixture.arena, &mut fixture.declarations, |state| {
            let mut search = OrderSearch::new(&fixture.premises, &[]);
            assert!(
                state
                    .source_order_search(values[0], values[3], &mut search)?
                    .is_none()
            );
            state.source_order_search(values[0], values[2], &mut search)
        })
        .unwrap()
        .expect("the two supplied orders compose");
    fixture.check(proof, 0, 2);
    fixture.premises.pop();
    fixture
        .theory
        .with(&mut fixture.arena, &mut fixture.declarations, |state| {
            let mut search = OrderSearch::new(&fixture.premises, &[]);
            assert!(
                state
                    .source_order_search(fixture.values[0], fixture.values[2], &mut search)?
                    .is_none()
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn cyclic_order_dependencies_complete_when_an_independent_path_proves_the_goal() {
    let mut fixture = OrderPremises::new(4);
    fixture.edge(0, 1);
    fixture.edge(1, 0);
    fixture.edge(0, 2);
    fixture.edge(2, 3);
    let values = &fixture.values;
    let (first, second) = fixture
        .theory
        .with(&mut fixture.arena, &mut fixture.declarations, |state| {
            let mut search = OrderSearch::new(&fixture.premises, &[]);
            let first = state
                .source_order_search(values[0], values[3], &mut search)?
                .unwrap();
            let blocked = search.question(&state.arena, values[1], values[3])?;
            assert!(search.questions[blocked].proof.is_none());
            let second = state
                .source_order_search(values[1], values[3], &mut search)?
                .expect("the earlier cycle cut cannot hide the completed ancestor's proof");
            Ok((first, second))
        })
        .unwrap();
    fixture.check(first, 0, 3);
    fixture.check(second, 1, 3);
}

#[test]
fn depth_limited_question_is_reopened_with_a_larger_remaining_allowance() {
    let mut fixture = OrderPremises::new(132);
    for value in 0..130 {
        fixture.edge(value, value + 1);
    }
    let values = &fixture.values;
    let proof = fixture
        .theory
        .with(&mut fixture.arena, &mut fixture.declarations, |state| {
            let mut search = OrderSearch::new(&fixture.premises, &[]);
            assert!(
                state
                    .source_order_search(values[0], values[130], &mut search)?
                    .is_none()
            );
            state.source_order_search(values[120], values[130], &mut search)
        })
        .unwrap()
        .expect("a failed deep query does not remove a shorter valid proof");
    fixture.check(proof, 120, 130);
}

#[test]
fn colliding_order_lookup_candidates_cannot_lend_another_goals_proof() {
    let mut fixture = OrderPremises::new(3);
    fixture.edge(0, 2);
    let values = &fixture.values;
    let proof = fixture
        .theory
        .with(&mut fixture.arena, &mut fixture.declarations, |state| {
            let mut search = OrderSearch::new(&fixture.premises, &[]);
            let proof = state
                .source_order_search(values[0], values[2], &mut search)?
                .unwrap();
            let original = search.question(&state.arena, values[0], values[2])?;
            let hashes = state
                .arena
                .structural_hashes(&[values[1], values[2]])
                .unwrap();
            // Force an unrelated, already proved goal into this lookup bucket.
            // The complete endpoint terms must still distinguish its meaning.
            search
                .by_structure
                .insert((hashes[0], hashes[1]), vec![original]);
            assert!(
                state
                    .source_order_search(values[1], values[2], &mut search)?
                    .is_none()
            );
            Ok(proof)
        })
        .unwrap();
    fixture.check(proof, 0, 2);
}

#[test]
fn a_pending_valid_path_survives_an_alternative_producer_refusal() {
    let mut fixture = OrderPremises::new(5);
    fixture.edge(0, 1);
    fixture.edge(1, 2);
    fixture.edge(2, 3);
    let deep = fixture
        .theory
        .with(&mut fixture.arena, &mut fixture.declarations, |state| {
            let mut sum = fixture.values[4];
            for _ in 0..140 {
                sum = state.add_terms(fixture.values[4], sum)?;
            }
            Ok(sum)
        })
        .unwrap();
    fixture.edge_terms(fixture.values[0], deep);
    let values = &fixture.values;
    let proof = fixture
        .theory
        .with(&mut fixture.arena, &mut fixture.declarations, |state| {
            let mut search = OrderSearch::new(&fixture.premises, &[]);
            let proof = state
                .source_order_search(values[0], values[3], &mut search)?
                .expect("the earlier transitive path supplies real proof evidence");
            assert!(search.questions.iter().any(|question| matches!(
                question.refusal,
                Some(BoundedDenotationError::DepthLimitExceeded)
            )));
            Ok(proof)
        })
        .unwrap();
    fixture.check(proof, 0, 3);

    fixture.premises.remove(2);
    fixture
        .theory
        .with(&mut fixture.arena, &mut fixture.declarations, |state| {
            let mut search = OrderSearch::new(&fixture.premises, &[]);
            assert!(matches!(
                state.source_order_search(fixture.values[0], fixture.values[3], &mut search),
                Err(BoundedDenotationError::DepthLimitExceeded)
            ));
            Ok(())
        })
        .unwrap();
}
