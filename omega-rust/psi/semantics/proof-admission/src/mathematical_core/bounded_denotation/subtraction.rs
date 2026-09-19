//! Exact scalar subtraction in the shared mathematical integer vocabulary.
//!
//! Two fixed assumptions describe subtraction by zero and strict antitonicity
//! in its right operand. Their applications, followed by endpoint transport,
//! prove conditional decrease for open and overflowing scalar expressions.
//! The bounded checker still owns admissible carriers and exact premises.
//!
//! Representable closed scalar expressions retain their evaluated numeral
//! identity. Their order is derived from the binary numeral laws. A supplied
//! false positivity premise remains a valid conditional hypothesis: strict
//! irreflexivity and the core's empty elimination derive the conclusion.
//! Nothing here evaluates arbitrary open arithmetic or changes premise matching.

use std::collections::BTreeMap;

use semantic_vocabulary::{IntegerMathTerm, IntegerValue, Proposition, ScalarTerm};

use super::super::scheme_dsl::{self, apps, id, pi, scheme_at, v};
use super::{BoundedDenotationError, Declaration, Denotation, IntegerLaw, Term, TermHandle};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Law {
    Zero,
    Antitone,
}

#[derive(Default)]
pub(super) struct Subtraction {
    operation: Option<u32>,
    laws: BTreeMap<Law, u32>,
    irreflexivity: Option<u32>,
}

impl Denotation {
    fn subtract_operation(&mut self) -> Result<u32, BoundedDenotationError> {
        if let Some(position) = self.subtraction.operation {
            return Ok(position);
        }
        let integer = self.integer_constant()?;
        let result = self.arena.insert(Term::Pi {
            domain: integer,
            codomain: integer,
        });
        let ty = self.arena.insert(Term::Pi {
            domain: integer,
            codomain: result,
        });
        let position = self.position()?;
        self.declarations.push(Declaration::assumption(0, ty));
        self.subtraction.operation = Some(position);
        Ok(position)
    }

    pub(super) fn subtract_terms(
        &mut self,
        left: TermHandle,
        right: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let position = self.subtract_operation()?;
        let operation = self.constant(position);
        let function = self.arena.insert(Term::Apply {
            function: operation,
            argument: left,
        });
        Ok(self.arena.insert(Term::Apply {
            function,
            argument: right,
        }))
    }

    fn subtract_law_application(
        &mut self,
        law: Law,
        arguments: &[TermHandle],
    ) -> Result<TermHandle, BoundedDenotationError> {
        let position = if let Some(&position) = self.subtraction.laws.get(&law) {
            position
        } else {
            let integer = self.integer()?;
            let operation = self.subtract_operation()?;
            let less_than = self.integer_less_than()?;
            let zero = self.binary_integer(false, 0)?;
            let constant = |position| scheme_at(position, Vec::new());
            let carrier = || constant(integer);
            let subtract = |left, right| apps(constant(operation), [left, right]);
            let lt = |left, right| apps(constant(less_than), [left, right]);
            let statement = match law {
                Law::Zero => pi(
                    "x",
                    carrier(),
                    id(carrier(), subtract(v("x"), constant(zero)), v("x")),
                ),
                Law::Antitone => pi(
                    "x",
                    carrier(),
                    pi(
                        "a",
                        carrier(),
                        pi(
                            "b",
                            carrier(),
                            pi(
                                "_",
                                lt(v("a"), v("b")),
                                lt(subtract(v("x"), v("b")), subtract(v("x"), v("a"))),
                            ),
                        ),
                    ),
                ),
            };
            let ty = scheme_dsl::build(&mut self.arena, &mut Vec::new(), &statement);
            let position = self.position()?;
            self.declarations.push(Declaration::assumption(0, ty));
            self.subtraction.laws.insert(law, position);
            position
        };
        let mut function = self.constant(position);
        for &argument in arguments {
            function = self.arena.insert(Term::Apply { function, argument });
        }
        Ok(function)
    }

    /// `forall x : Int, IntLt x x -> Empty`. This is an explicit fixed
    /// order assumption, not a new trusted inference or a numeral decision.
    fn irreflexive(
        &mut self,
        value: TermHandle,
        evidence: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let position = if let Some(position) = self.subtraction.irreflexivity {
            position
        } else {
            let integer = self.integer_constant()?;
            let relation_position = self.integer_less_than()?;
            let relation = self.constant(relation_position);
            let variable = self.arena.insert(Term::Variable(0));
            let function = self.arena.insert(Term::Apply {
                function: relation,
                argument: variable,
            });
            let domain = self.arena.insert(Term::Apply {
                function,
                argument: variable,
            });
            let empty = self.arena.insert(Term::Empty);
            let codomain = self.arena.insert(Term::Pi {
                domain,
                codomain: empty,
            });
            let ty = self.arena.insert(Term::Pi {
                domain: integer,
                codomain,
            });
            let position = self.position()?;
            self.declarations.push(Declaration::assumption(0, ty));
            self.subtraction.irreflexivity = Some(position);
            position
        };
        let function = self.constant(position);
        let function = self.arena.insert(Term::Apply {
            function,
            argument: value,
        });
        Ok(self.arena.insert(Term::Apply {
            function,
            argument: evidence,
        }))
    }

    pub(super) fn subtraction_evidence(
        &mut self,
        difference: &Proposition,
        difference_evidence: TermHandle,
        positive_evidence: TermHandle,
        goal: &Proposition,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let Proposition::Equal(
            result,
            subtraction @ ScalarTerm::ExactIntegerSubtract { left, right, .. },
        ) = difference
        else {
            unreachable!("bounded subtraction checker ran first")
        };
        let result_term = self.fixed_scalar_term(result)?;
        let left_term = self.fixed_scalar_term(left)?;
        let right_term = self.fixed_scalar_term(right)?;
        let subtraction_term = self.fixed_scalar_term(subtraction)?;
        let zero = self.math_term(&IntegerMathTerm::literal(IntegerValue::Unsigned(0)))?;
        let decrease = if let Some((_, value)) = subtraction.integer_value() {
            let (_, decrement) = right
                .integer_value()
                .expect("evaluated exact subtraction has evaluated operands");
            if matches!(decrement, IntegerValue::Signed(value) if value <= 0)
                || decrement == IntegerValue::Unsigned(0)
            {
                let contradiction = if decrement == IntegerValue::Signed(0)
                    || decrement == IntegerValue::Unsigned(0)
                {
                    positive_evidence
                } else {
                    let backwards = self.literal_order(decrement, IntegerValue::Signed(0))?;
                    self.integer_law_application(
                        IntegerLaw::LessThanTransitivity,
                        &[zero, right_term, zero, positive_evidence, backwards],
                    )?
                };
                let scrutinee = self.irreflexive(zero, contradiction)?;
                let ty = self.denote(goal)?;
                return Ok(self.arena.insert(Term::EmptyElim { ty, scrutinee }));
            }
            let (_, original) = left
                .integer_value()
                .expect("evaluated exact subtraction has evaluated operands");
            self.literal_order(value, original)?
        } else {
            let subtract_zero = self.subtract_terms(left_term, zero)?;
            let decrease = self.subtract_law_application(
                Law::Antitone,
                &[left_term, zero, right_term, positive_evidence],
            )?;
            let equality = self.subtract_law_application(Law::Zero, &[left_term])?;
            self.integer_law_application(
                IntegerLaw::LessThanSubstituteRight,
                &[
                    subtraction_term,
                    subtract_zero,
                    left_term,
                    equality,
                    decrease,
                ],
            )?
        };
        let integer = self.integer_constant()?;
        let equality = self.symmetry(integer, result_term, subtraction_term, difference_evidence);
        self.integer_law_application(
            IntegerLaw::LessThanSubstituteLeft,
            &[subtraction_term, result_term, left_term, equality, decrease],
        )
    }
}
