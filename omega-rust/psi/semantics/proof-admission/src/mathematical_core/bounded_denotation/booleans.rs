//! The selected Boolean primitives have the existing Two denotation.
//! Keeping their operands visible lets J transport saved computations into
//! an authored result contract. Treating each expression as an unrelated
//! constant would instead require an assumed implication for every program.
//!
//! BooleanEqual computes a Boolean; proposition equality remains proof-relevant
//! Id. These are already-selected scalar primitives, not authored operators
//! recognized by spelling. No truth-table axiom or kernel rule is added.

use super::{BoundedDenotationError, Denotation, ScalarTerm, Term, TermArena, TermHandle};

/// One closed lambda, shared by the denotation. Supplying operands through
/// Apply keeps each operand once in the stored term: directly substituting
/// the right operand into both case branches creates exponential traversal
/// of a right-nested comparison even when its arena storage is shared.
fn equality_function(
    arena: &mut TermArena,
    two: TermHandle,
    zero: TermHandle,
    one: TermHandle,
) -> TermHandle {
    let right = arena.insert(Term::Variable(0));
    let left = arena.insert(Term::Variable(1));
    let motive = arena.insert(Term::Lambda {
        domain: two,
        body: two,
    });
    let opposite = arena.insert(Term::CaseTwo {
        motive,
        zero_branch: one,
        one_branch: zero,
        scrutinee: right,
    });
    let result = arena.insert(Term::CaseTwo {
        motive,
        zero_branch: opposite,
        one_branch: right,
        scrutinee: left,
    });
    let inner = arena.insert(Term::Lambda {
        domain: two,
        body: result,
    });
    arena.insert(Term::Lambda {
        domain: two,
        body: inner,
    })
}

impl Denotation {
    pub(super) fn boolean_term(
        &mut self,
        term: &ScalarTerm,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        Ok(Some(match term {
            ScalarTerm::Boolean(false) => self.two_zero,
            ScalarTerm::Boolean(true) => self.two_one,
            ScalarTerm::BooleanNot { operand } => {
                let operand = self.scalar_term(operand)?;
                self.boolean_case(operand, self.two_one, self.two_zero)
            }
            ScalarTerm::BooleanEqual { left, right } => {
                let left = self.scalar_term(left)?;
                let right = self.scalar_term(right)?;
                if !self.boolean_equality.is_valid() {
                    self.boolean_equality =
                        equality_function(&mut self.arena, self.two, self.two_zero, self.two_one);
                }
                let function = self.arena.insert(Term::Apply {
                    function: self.boolean_equality,
                    argument: left,
                });
                self.arena.insert(Term::Apply {
                    function,
                    argument: right,
                })
            }
            _ => return Ok(None),
        }))
    }

    fn boolean_case(
        &mut self,
        scrutinee: TermHandle,
        zero_branch: TermHandle,
        one_branch: TermHandle,
    ) -> TermHandle {
        let motive = self.arena.insert(Term::Lambda {
            domain: self.two,
            body: self.two,
        });
        self.arena.insert(Term::CaseTwo {
            motive,
            zero_branch,
            one_branch,
            scrutinee,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Denotation, ScalarTerm, Term};
    use crate::{
        Budget, MathematicalCertificate, certificate_assumption_closure,
        verify_mathematical_certificate,
    };
    use semantic_vocabulary::{ScalarType, ValueId};

    #[test]
    fn primitive_boolean_truth_tables_are_kernel_computations() {
        for left in [false, true] {
            let not = ScalarTerm::boolean_not(ScalarTerm::Boolean(left)).unwrap();
            check_closed(not, !left);
            for right in [false, true] {
                let equal = ScalarTerm::boolean_equal(
                    ScalarTerm::Boolean(left),
                    ScalarTerm::Boolean(right),
                )
                .unwrap();
                check_closed(equal, left == right);
            }
        }
    }

    fn check_closed(expression: ScalarTerm, expected: bool) {
        let mut denotation = Denotation::new();
        let value = denotation.scalar_term(&expression).unwrap();
        let expected_value = denotation
            .scalar_term(&ScalarTerm::Boolean(expected))
            .unwrap();
        let expected_type = denotation.arena.insert(Term::Id {
            ty: denotation.two,
            left: value,
            right: expected_value,
        });
        let term = denotation.arena.insert(Term::Refl {
            ty: denotation.two,
            value,
        });
        let mut certificate = MathematicalCertificate {
            signature: denotation.declarations,
            level_arity: 0,
            context: Vec::new(),
            term,
            expected: expected_type,
        };
        assert!(
            certificate.signature.is_empty(),
            "truth tables need no assumptions"
        );
        verify_mathematical_certificate(
            &mut denotation.arena,
            &certificate,
            &mut Budget::default(),
        )
        .unwrap();
        let wrong = if expected {
            denotation.two_zero
        } else {
            denotation.two_one
        };
        certificate.expected = denotation.arena.insert(Term::Id {
            ty: denotation.two,
            left: value,
            right: wrong,
        });
        assert!(
            verify_mathematical_certificate(
                &mut denotation.arena,
                &certificate,
                &mut Budget::default()
            )
            .is_err()
        );
    }

    #[test]
    fn symbolic_boolean_operands_remain_distinct_in_the_exact_closure() {
        let mut denotation = Denotation::new();
        let left = ScalarTerm::value(ValueId::new(1).unwrap(), ScalarType::Boolean);
        let right = ScalarTerm::value(ValueId::new(2).unwrap(), ScalarType::Boolean);
        let expression = ScalarTerm::boolean_equal(left.clone(), right.clone()).unwrap();
        let value = denotation.scalar_term(&expression).unwrap();
        let left = denotation.scalar_term(&left).unwrap();
        let right = denotation.scalar_term(&right).unwrap();
        assert_ne!(left, right);
        assert_eq!(denotation.declarations.len(), 2);
        assert!(
            denotation
                .declarations
                .iter()
                .all(|declaration| declaration.ty == denotation.two)
        );
        let term = denotation.arena.insert(Term::Refl {
            ty: denotation.two,
            value,
        });
        let expected = denotation.arena.insert(Term::Id {
            ty: denotation.two,
            left: value,
            right: value,
        });
        let certificate = MathematicalCertificate {
            signature: denotation.declarations,
            level_arity: 0,
            context: Vec::new(),
            term,
            expected,
        };
        assert_eq!(
            certificate_assumption_closure(&denotation.arena, &certificate),
            [0, 1].into_iter().collect()
        );
        verify_mathematical_certificate(
            &mut denotation.arena,
            &certificate,
            &mut Budget::default(),
        )
        .unwrap();
    }

    #[test]
    fn nested_boolean_equality_keeps_linear_comparison_input() {
        for depth in [4, 8, 16, 32] {
            let mut denotation = Denotation::new();
            let input = ScalarTerm::value(ValueId::new(1).unwrap(), ScalarType::Boolean);
            let mut expression = input.clone();
            for _ in 0..depth {
                expression = ScalarTerm::boolean_equal(input.clone(), expression).unwrap();
            }
            let left = denotation.scalar_term(&expression).unwrap();
            let right = denotation.scalar_term(&expression).unwrap();
            assert!(denotation.arena.len() <= 6 * depth + 32);
            // Count occurrences, not distinct handles: a compact DAG can
            // still expand exponentially for equality and closure readers.
            let mut pending = vec![left];
            let mut occurrences = 0;
            while let Some(term) = pending.pop() {
                occurrences += 1;
                assert!(occurrences <= 32 * depth + 1);
                match denotation.arena.get(term) {
                    Term::Apply { function, argument } => pending.extend([function, argument]),
                    Term::Lambda { domain, body } => pending.extend([domain, body]),
                    Term::CaseTwo {
                        motive,
                        zero_branch,
                        one_branch,
                        scrutinee,
                    } => pending.extend([motive, zero_branch, one_branch, scrutinee]),
                    Term::Variable(_)
                    | Term::Two
                    | Term::TwoZero
                    | Term::TwoOne
                    | Term::Constant { .. } => {}
                    other => panic!("unexpected Boolean denotation: {other:?}"),
                }
            }
            assert!(denotation.arena.structurally_equal(left, right));
        }
    }
}
