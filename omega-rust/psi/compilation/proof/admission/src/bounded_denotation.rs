/*
    Denotation and elaboration of bounded `ProofNode` certificates into the
    common mathematical core, including propositional connectives, Boolean case
    analysis, value-equality transport, signed binary numerals, and arithmetic
    and integer-order rules.

    We elaborate bounded propositions and proof rules into `Term` judgments and
    re-decide every covered certificate through `verify_mathematical_certificate`.
*/

use std::collections::{BTreeMap, BTreeSet, HashMap};

use numerics::bignum::BigInt;
use semantic_vocabulary::{
    ContentAlgebra, ContentTerm, IntegerMathLiteral, IntegerMathTerm, IntegerType, IntegerValue,
    Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId,
};

use crate::ClosedIntegerEvaluator;
use crate::kernel::{KernelError, decide_primitive};
use crate::mathematical_core::certificate::{
    MathematicalCertificate, certificate_assumption_closure, verify_mathematical_certificate,
};
use crate::mathematical_core::conversion::Budget;
use crate::mathematical_core::scheme_dsl::{self, Syntax};
use crate::mathematical_core::signature::Declaration;
use crate::mathematical_core::term::{Level, Sort, Term, TermArena, TermHandle};
use crate::mathematical_core::typing::CoreError;
use crate::proof::integer_math_normalization::{
    lift_fixed_integer_relation, propositions_match_under_integer_math_normalization,
};
use crate::proof::{
    AcceptedPremise, AcceptedProofRule, MathematicalJudgmentReceipt, PremiseRoster, ProofError,
    ProofNode, ProofRule, add_order, equality_rules, integer_bound_rules, integer_order_rules,
    order_discreteness, strict_order_transitivity, subtract_antitone, subtract_order,
};

const MAX_ELABORATION_NODES: u64 = 1 << 16;

mod addition {
    use std::cmp::Ordering;
    use std::collections::BTreeMap;

    use numerics::bignum::BigInt;
    use semantic_vocabulary::{
        IntegerCarrier, IntegerMathTerm, IntegerType, IntegerValue, Proposition, ScalarTerm,
        ScalarType,
    };

    use super::binary_numerals;
    use super::denotation::{BinaryBoundOperation, math_less_or_equal_sides};
    use super::{
        BoundedDenotationError, Denotation, IntegerLaw, Term, TermHandle,
        affine_carrier_endpoint_equality, affine_root_premise, affine_witness_root_expression,
        unbounded_integer,
    };
    use crate::mathematical_core::scheme_dsl::{self, apps, id, pi, pis, scheme_at, v};
    use crate::term_matchers::is_math_add;
    use crate::{ClosedIntegerEvaluator, kernel::KernelError, proof::ProofError};

    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    enum Law {
        Monotone,
        CancelSubtract,
        MonotoneBoth,
    }

    #[derive(Default)]
    pub(super) struct Addition {
        operation: Option<u32>,
        laws: BTreeMap<Law, u32>,
        pub(super) numeral_sums: BTreeMap<(IntegerValue, IntegerValue, IntegerValue), u32>,
    }

    impl Denotation {
        pub(super) fn add_order_both(
            &mut self,
            a: TermHandle,
            b: TermHandle,
            c: TermHandle,
            d: TermHandle,
            first: TermHandle,
            second: TermHandle,
        ) -> Result<TermHandle, BoundedDenotationError> {
            self.add_law_application(Law::MonotoneBoth, &[a, b, c, d, first, second])
        }
        pub(super) fn add_order_translation(
            &mut self,
            left: TermHandle,
            right: TermHandle,
            amount: TermHandle,
            evidence: TermHandle,
        ) -> Result<TermHandle, BoundedDenotationError> {
            self.add_law_application(Law::Monotone, &[left, right, amount, evidence])
        }

        pub(super) fn add_operation(&mut self) -> Result<u32, BoundedDenotationError> {
            self.binary_integer_operation(|this| &mut this.addition.operation)
        }

        pub(super) fn add_terms(
            &mut self,
            left: TermHandle,
            right: TermHandle,
        ) -> Result<TermHandle, BoundedDenotationError> {
            self.binary_terms(Self::add_operation, left, right)
        }

        fn add_law_application(
            &mut self,
            law: Law,
            arguments: &[TermHandle],
        ) -> Result<TermHandle, BoundedDenotationError> {
            self.roster_law_application(
                |this| &mut this.addition.laws,
                law,
                |this| -> Result<scheme_dsl::Syntax, BoundedDenotationError> {
                    let integer = this.integer()?;
                    let operation = this.add_operation()?;
                    let constant = |position| scheme_at(position, Vec::new());
                    let carrier = || constant(integer);
                    let binders = |names: &[&'static str], body| {
                        pis(names.iter().map(|name| (*name, carrier())), body)
                    };
                    let add = |left, right| apps(constant(operation), [left, right]);
                    let statement = match law {
                        Law::Monotone => {
                            let relation = this.integer_less_or_equal()?;
                            let le = |left, right| apps(constant(relation), [left, right]);
                            binders(
                                &["a", "b", "c"],
                                pi(
                                    "_",
                                    le(v("a"), v("b")),
                                    le(add(v("a"), v("c")), add(v("b"), v("c"))),
                                ),
                            )
                        }
                        Law::MonotoneBoth => {
                            let relation = this.integer_less_or_equal()?;
                            let le = |left, right| apps(constant(relation), [left, right]);
                            binders(
                                &["a", "b", "c", "d"],
                                pi(
                                    "_",
                                    le(v("a"), v("b")),
                                    pi(
                                        "_",
                                        le(v("c"), v("d")),
                                        le(add(v("a"), v("c")), add(v("b"), v("d"))),
                                    ),
                                ),
                            )
                        }
                        Law::CancelSubtract => {
                            let subtract = this.subtract_operation()?;
                            binders(
                                &["b", "c"],
                                id(
                                    carrier(),
                                    add(apps(constant(subtract), [v("b"), v("c")]), v("c")),
                                    v("b"),
                                ),
                            )
                        }
                    };
                    Ok(statement)
                },
                arguments,
            )
        }

        pub(super) fn numeral_sum(
            &mut self,
            addend: IntegerValue,
            augend: IntegerValue,
            sum: IntegerValue,
            addend_term: TermHandle,
            augend_term: TermHandle,
            sum_term: TermHandle,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            self.numeral_binary_operation(
                addend,
                augend,
                sum,
                addend_term,
                augend_term,
                sum_term,
                |left, right| left.add(right),
                Self::add_terms,
                |denotation| &mut denotation.addition.numeral_sums,
            )
        }

        pub(super) fn correlated_add_bound_evidence(
            &mut self,
            premise: &Proposition,
            mut evidence: TermHandle,
            witness: &crate::IntegerAffineWitness,
            conclusion: &Proposition,
            definitions: &[(Proposition, TermHandle)],
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            let ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } = &witness.target
            else {
                return Ok(None);
            };
            if scalar_type.carrier() != IntegerCarrier::Fixed {
                return Ok(None);
            }
            let Some((lower, bound, sum)) = math_less_or_equal_sides(conclusion, is_math_add)
            else {
                return Ok(None);
            };

            let evaluate = |term: &IntegerMathTerm| {
                ClosedIntegerEvaluator::default()
                    .evaluate_closed(term)
                    .map_err(|error| {
                        BoundedDenotationError::Certificate(ProofError::PrimitiveJudgment(
                            KernelError::ClosedIntegerEvaluation(error),
                        ))
                    })
            };
            let bound_value = evaluate(bound)?;
            let sum_value = evaluate(sum)?;
            if let (Some(bound_value), Some(sum_value)) = (&bound_value, &sum_value) {
                let (goal_left, goal_right, left_value, right_value) = if lower {
                    (bound, sum, bound_value, sum_value)
                } else {
                    (sum, bound, sum_value, bound_value)
                };
                return self.closed_order_bound_evidence(
                    premise,
                    evidence,
                    conclusion,
                    goal_left,
                    goal_right,
                    left_value,
                    right_value,
                );
            }
            if sum_value.is_some() {
                return Ok(None);
            }
            let Some((difference, equality)) = affine_witness_root_expression(witness, definitions)
            else {
                return Ok(None);
            };
            let ScalarTerm::ExactIntegerSubtract {
                scalar_type: subtract_type,
                left: endpoint,
                right: decrement,
            } = difference
            else {
                return Ok(None);
            };
            let closed_difference = difference.integer_value().is_some();
            let expected_premise = affine_root_premise(witness, lower, left.as_ref());
            if subtract_type != scalar_type || decrement != right || premise != &expected_premise {
                return Ok(None);
            }
            let Some((endpoint_equality, endpoint_literal)) =
                affine_carrier_endpoint_equality(*scalar_type, lower, endpoint, definitions)
            else {
                return Ok(None);
            };
            let left_term = self.fixed_scalar_term(left)?;
            let right_term = self.fixed_scalar_term(right)?;
            let endpoint_term = self.fixed_scalar_term(endpoint)?;
            let difference_term = self.fixed_scalar_term(difference)?;
            let Some(stepped) = self.affine_root_equality_step(
                lower,
                witness,
                left_term,
                difference_term,
                equality,
                evidence,
            )?
            else {
                return Ok(None);
            };
            evidence = stepped;
            let sum = self.add_terms(left_term, right_term)?;
            let cancelled_sum = self.add_terms(difference_term, right_term)?;
            let ordered_terms = if lower {
                [difference_term, left_term, right_term, evidence]
            } else {
                [left_term, difference_term, right_term, evidence]
            };
            let order = self.add_law_application(Law::Monotone, &ordered_terms)?;
            let equality = if closed_difference {
                let (
                    Some((_, difference_value)),
                    Some((_, decrement_value)),
                    Some((_, endpoint_value)),
                ) = (
                    difference.integer_value(),
                    decrement.integer_value(),
                    endpoint.integer_value(),
                )
                else {
                    return Ok(None);
                };
                let Some(equality) = self.numeral_sum(
                    difference_value,
                    decrement_value,
                    endpoint_value,
                    difference_term,
                    right_term,
                    endpoint_term,
                )?
                else {
                    return Ok(None);
                };
                equality
            } else {
                self.add_law_application(Law::CancelSubtract, &[endpoint_term, right_term])?
            };
            let order = if lower {
                self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteLeft,
                    &[cancelled_sum, endpoint_term, sum, equality, order],
                )?
            } else {
                self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteRight,
                    &[sum, cancelled_sum, endpoint_term, equality, order],
                )?
            };
            self.affine_endpoint_equality_order(
                lower,
                endpoint_equality,
                &endpoint_literal,
                endpoint_term,
                order,
                |_| Ok(sum),
            )
        }

        pub(super) fn direct_add_bound_evidence(
            &mut self,
            premise: &Proposition,
            evidence: TermHandle,
            witness: &crate::IntegerAffineWitness,
            conclusion: &Proposition,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            self.direct_binary_bound_evidence(
                &Addition::bound_operation(),
                premise,
                evidence,
                witness,
                conclusion,
            )
        }

        pub(super) fn exact_add_definition_bound_evidence(
            &mut self,
            left_bound: &Proposition,
            left_evidence: TermHandle,
            right_bound: &Proposition,
            right_evidence: TermHandle,
            definition: &Proposition,
            definition_evidence: TermHandle,
            conclusion: &Proposition,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            self.exact_binary_definition_bound_evidence(
                &Addition::bound_operation(),
                left_bound,
                left_evidence,
                right_bound,
                right_evidence,
                definition,
                definition_evidence,
                conclusion,
            )
        }

        pub(super) fn carrier_bound(
            &mut self,
            operand: &ScalarTerm,
            lower: bool,
            integer_type: &IntegerType,
        ) -> Result<(ScalarTerm, TermHandle), BoundedDenotationError> {
            let value = if lower {
                integer_type.minimum_value()
            } else {
                integer_type.maximum_value()
            };
            let endpoint = ScalarTerm::integer(*integer_type, value)
                .expect("carrier endpoint is representable");
            let position = self.interned_keyed_assumption(
                |this| &mut this.carrier_bounds,
                (operand.clone(), lower),
                |this| {
                    let proposition = if lower {
                        Proposition::LessOrEqual(endpoint.clone(), operand.clone())
                    } else {
                        Proposition::LessOrEqual(operand.clone(), endpoint.clone())
                    };
                    this.denote(&proposition)
                },
            )?;
            Ok((endpoint, self.constant(position)))
        }

        pub(super) fn add_bound_endpoint(
            &mut self,
            operand: &ScalarTerm,
            proposition: &Proposition,
            evidence: TermHandle,
            lower: bool,
            integer_type: &IntegerType,
        ) -> Result<Option<(ScalarTerm, TermHandle)>, BoundedDenotationError> {
            if operand.integer_value().is_some() {
                if proposition != &Proposition::Truth {
                    return Ok(None);
                }
                let term = self.fixed_scalar_term(operand)?;
                let integer = self.integer_constant()?;
                let reflexive = self.arena.insert(Term::Refl {
                    ty: integer,
                    value: term,
                });
                let equality = self.integer_law_application(
                    IntegerLaw::EqualityToLessOrEqual,
                    &[term, term, reflexive],
                )?;
                return Ok(Some((operand.clone(), equality)));
            }
            match proposition {
                Proposition::Truth
                    if operand.scalar_type() == ScalarType::Integer(*integer_type) =>
                {
                    self.carrier_bound(operand, lower, integer_type).map(Some)
                }
                Proposition::Truth => Ok(None),
                Proposition::Equal(first, second) => {
                    let literal = if first == operand {
                        second
                    } else if second == operand {
                        first
                    } else {
                        return Ok(None);
                    };
                    let Some((actual, _)) = literal.integer_value() else {
                        return Ok(None);
                    };
                    if actual != *integer_type {
                        return Ok(None);
                    }
                    let operand_term = self.fixed_scalar_term(operand)?;
                    let literal_term = self.fixed_scalar_term(literal)?;
                    let denoted = self.denote(proposition)?;
                    let Some((_, denoted_from, denoted_to)) = self.identity_parts(denoted) else {
                        return Ok(None);
                    };
                    let (want_from, want_to) = if lower {
                        (literal_term, operand_term)
                    } else {
                        (operand_term, literal_term)
                    };
                    let Some(equality) = self.directed_equality(
                        denoted_from,
                        denoted_to,
                        want_from,
                        want_to,
                        evidence,
                    ) else {
                        return Ok(None);
                    };
                    self.integer_law_application(
                        IntegerLaw::EqualityToLessOrEqual,
                        &[want_from, want_to, equality],
                    )
                    .map(|order| Some((literal.clone(), order)))
                }
                Proposition::LessOrEqual(first, second) => {
                    if first == second {
                        return Ok(None);
                    }
                    let bound = if second == operand && lower {
                        first
                    } else if first == operand && !lower {
                        second
                    } else {
                        return Ok(None);
                    };
                    let Some((actual, _)) = bound.integer_value() else {
                        return Ok(None);
                    };
                    if actual != *integer_type {
                        return Ok(None);
                    }
                    Ok(Some((bound.clone(), evidence)))
                }
                _ => Ok(None),
            }
        }

        pub(super) fn closed_order_bound_evidence(
            &mut self,
            premise: &Proposition,
            evidence: TermHandle,
            conclusion: &Proposition,
            left: &IntegerMathTerm,
            right: &IntegerMathTerm,
            left_value: &BigInt,
            right_value: &BigInt,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            let numeral = |value: &BigInt| {
                let (negative, magnitude) = binary_numerals::fixed_magnitude(value)?;
                if !negative {
                    Some(IntegerValue::Unsigned(magnitude))
                } else if magnitude == 1_u128 << 127 {
                    Some(IntegerValue::Signed(i128::MIN))
                } else {
                    i128::try_from(magnitude)
                        .ok()
                        .map(|magnitude| IntegerValue::Signed(-magnitude))
                }
            };
            match left_value.cmp(right_value) {
                Ordering::Less => {
                    let (Some(lower), Some(upper)) = (numeral(left_value), numeral(right_value))
                    else {
                        return Ok(None);
                    };
                    let strict = self.literal_order(lower, upper)?;
                    let left = self.math_term(left)?;
                    let right = self.math_term(right)?;
                    self.integer_law_application(
                        IntegerLaw::LessThanToLessOrEqual,
                        &[left, right, strict],
                    )
                    .map(Some)
                }
                Ordering::Equal => {
                    let left = self.math_term(left)?;
                    let right = self.math_term(right)?;
                    let integer = self.integer_constant()?;
                    let reflexive = self.arena.insert(Term::Refl {
                        ty: integer,
                        value: left,
                    });
                    self.integer_law_application(
                        IntegerLaw::EqualityToLessOrEqual,
                        &[left, right, reflexive],
                    )
                    .map(Some)
                }
                Ordering::Greater => {
                    let Proposition::LessOrEqual(premise_left, premise_right) = premise else {
                        return Ok(None);
                    };
                    let (Some((_, left_value)), Some((_, right_value))) =
                        (premise_left.integer_value(), premise_right.integer_value())
                    else {
                        return Ok(None);
                    };
                    if unbounded_integer(left_value) <= unbounded_integer(right_value) {
                        return Ok(None);
                    }
                    let strict = self.literal_order(right_value, left_value)?;
                    let premise_left = self.fixed_scalar_term(premise_left)?;
                    let premise_right = self.fixed_scalar_term(premise_right)?;
                    let reflexive = self.integer_law_application(
                        IntegerLaw::LessThanLessOrEqualTransitivity,
                        &[premise_right, premise_left, premise_right, strict, evidence],
                    )?;
                    let scrutinee = self.irreflexive(premise_right, reflexive)?;
                    let ty = self.denote(conclusion)?;
                    Ok(Some(self.arena.insert(Term::EmptyElim { ty, scrutinee })))
                }
            }
        }
    }

    impl Addition {
        fn bound_operation() -> BinaryBoundOperation {
            BinaryBoundOperation {
                target: crate::term_matchers::exact_integer_add,
                is_target_operation: is_math_add,
                right_antitone: false,
                terms: Denotation::add_terms,
                monotone: |denotation, arguments| {
                    denotation.add_law_application(Law::MonotoneBoth, arguments)
                },
                numeral_equation: Denotation::numeral_sum,
            }
        }

        pub(super) fn source_safety_roster(&self) -> Vec<u32> {
            self.operation
                .into_iter()
                .chain(self.laws.values().copied())
                .chain(self.numeral_sums.values().copied())
                .collect()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::super::test_utilities::open_operation_preserves_skipped_numeric_resource_refusals;
        use super::super::test_utilities::{
            assert_refused, direct_affine_proof, direct_fixture, elaborate_checked, mathematical,
            verify_then_elaborate,
        };
        use crate::proof::assumption_proof;
        use crate::{
            Budget, IntegerAffineWitness, ProofNode, ProofRule, value_context,
            verify_bounded_certificate,
        };
        use semantic_vocabulary::IntegerMathLiteral;
        use semantic_vocabulary::{
            IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, Proposition,
            PropositionContext, ScalarTerm, ScalarType, ValueId,
        };
        use std::collections::BTreeSet;
        use std::slice::from_ref;

        #[test]
        fn open_addition_preserves_previously_skipped_numeric_resource_refusals() {
            open_operation_preserves_skipped_numeric_resource_refusals(IntegerMathTerm::Add);
        }

        #[test]
        fn correlated_add_bounds_transport_ssa_roots_and_carrier_endpoints() {
            for (sign, width, lower) in [
                (IntegerSign::Unsigned, 8, false),
                (IntegerSign::Unsigned, 64, false),
                (IntegerSign::Unsigned, 128, false),
                (IntegerSign::Signed, 128, false),
                (IntegerSign::Signed, 32, true),
                (IntegerSign::Signed, 64, true),
                (IntegerSign::Signed, 128, true),
            ] {
                let integer = IntegerType::new(sign, width).unwrap();
                let scalar = ScalarType::Integer(integer);
                let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
                let context = value_context(scalar, 4);
                let left = if width == 8 {
                    ScalarTerm::integer(integer, IntegerValue::Unsigned(0)).unwrap()
                } else {
                    value(1)
                };
                let premise = if lower {
                    Proposition::LessOrEqual(value(3), left.clone())
                } else {
                    Proposition::LessOrEqual(left.clone(), value(3))
                };
                let endpoint = if lower {
                    integer.minimum_value()
                } else {
                    integer.maximum_value()
                };
                let endpoint = ScalarTerm::integer(integer, endpoint).unwrap();
                let difference =
                    ScalarTerm::exact_integer_subtract(integer, value(4), value(2)).unwrap();
                let sum = IntegerMathTerm::Add(
                    Box::new(mathematical(&left)),
                    Box::new(mathematical(&value(2))),
                );
                let goal = if lower {
                    Proposition::IntegerMathLessOrEqual(mathematical(&endpoint), sum)
                } else {
                    Proposition::IntegerMathLessOrEqual(sum, mathematical(&endpoint))
                };
                for reversed in [false, true] {
                    let equality = |left, right| {
                        if reversed {
                            Proposition::Equal(right, left)
                        } else {
                            Proposition::Equal(left, right)
                        }
                    };
                    let axioms = [
                        equality(value(4), endpoint.clone()),
                        equality(value(3), difference.clone()),
                    ];
                    let proof = ProofNode {
                        conclusion: goal.clone(),
                        rule: ProofRule::IntegerAffineBound {
                            root_bound: Box::new(assumption_proof(premise.clone(), 0)),
                            witness: IntegerAffineWitness {
                                root: value(3),
                                target: ScalarTerm::exact_integer_add(
                                    integer,
                                    left.clone(),
                                    value(2),
                                )
                                .unwrap(),
                                definition_axioms: vec![1],
                                literal_axioms: vec![Some(0)],
                            },
                        },
                    };
                    let denoted = verify_bounded_certificate(
                        &context,
                        &goal,
                        from_ref(&premise),
                        &axioms,
                        &proof,
                        &mut Budget::default(),
                    )
                    .unwrap();
                    assert!(denoted.certificate.signature.len() < 300);
                    let parameters = BTreeSet::new();
                    let assumptions = [premise.clone()];
                    let elaboration = elaborate_checked(
                        &context,
                        &goal,
                        &assumptions,
                        &axioms,
                        &parameters,
                        &proof,
                    );
                    assert!(elaboration.denotation.rule_axioms.is_empty());
                }
            }
        }

        fn closed_addend_rule_axioms(
            integer: IntegerType,
            lower: bool,
            left: IntegerValue,
            right: IntegerValue,
        ) -> usize {
            let literal = |value| ScalarTerm::integer(integer, value).unwrap();
            let endpoint = if lower {
                integer.minimum_value()
            } else {
                integer.maximum_value()
            };
            let left = literal(left);
            let right = literal(right);
            let root =
                ScalarTerm::exact_integer_subtract(integer, literal(endpoint), right.clone())
                    .unwrap();
            let premise = if lower {
                Proposition::LessOrEqual(root.clone(), left.clone())
            } else {
                Proposition::LessOrEqual(left.clone(), root.clone())
            };
            let sum = IntegerMathTerm::Add(
                Box::new(mathematical(&left)),
                Box::new(mathematical(&right)),
            );
            let goal = if lower {
                Proposition::IntegerMathLessOrEqual(IntegerMathTerm::literal(endpoint), sum)
            } else {
                Proposition::IntegerMathLessOrEqual(sum, IntegerMathTerm::literal(endpoint))
            };
            let proof = ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::IntegerAffineBound {
                    root_bound: Box::new(assumption_proof(premise.clone(), 0)),
                    witness: IntegerAffineWitness {
                        root,
                        target: ScalarTerm::exact_integer_add(integer, left, right).unwrap(),
                        definition_axioms: Vec::new(),
                        literal_axioms: Vec::new(),
                    },
                },
            };
            let context = PropositionContext::from_value_types([]).unwrap();
            let assumptions = [premise];
            let parameters = BTreeSet::new();
            let elaboration =
                verify_then_elaborate(&context, &goal, &assumptions, &parameters, &proof);
            assert!(
                elaboration.denotation.addition.laws.is_empty(),
                "closed goals derive from numeral laws, not the law chain"
            );
            elaboration.denotation.rule_axioms.len()
        }

        #[test]
        fn closed_addend_bounds_derive_from_numeral_laws() {
            let wide = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
            for (left, right) in [(0, 0), (1, 2), (u128::MAX, 0), (0, u128::MAX)] {
                assert_eq!(
                    closed_addend_rule_axioms(
                        wide,
                        false,
                        IntegerValue::Unsigned(left),
                        IntegerValue::Unsigned(right),
                    ),
                    0
                );
            }
            let signed = IntegerType::new(IntegerSign::Signed, 8).unwrap();
            for (left, right) in [
                (IntegerValue::Signed(-100), IntegerValue::Signed(10)),
                (IntegerValue::Signed(-118), IntegerValue::Signed(-10)),
            ] {
                assert_eq!(closed_addend_rule_axioms(signed, true, left, right), 0);
            }
        }

        #[test]
        fn closed_addend_false_goal_empties_the_checked_premise() {
            let wide = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
            assert_eq!(
                closed_addend_rule_axioms(
                    wide,
                    false,
                    IntegerValue::Unsigned(u128::MAX),
                    IntegerValue::Unsigned(u128::MAX),
                ),
                0
            );
            let narrow = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
            assert_eq!(
                closed_addend_rule_axioms(
                    narrow,
                    false,
                    IntegerValue::Unsigned(200),
                    IntegerValue::Unsigned(100),
                ),
                0
            );
        }

        #[test]
        fn open_sum_over_closed_difference_uses_the_numeral_equation() {
            let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
            let scalar = ScalarType::Integer(integer);
            let left = ScalarTerm::value(ValueId::new(1).unwrap(), scalar);
            let context =
                PropositionContext::from_value_types([(ValueId::new(1).unwrap(), scalar)]).unwrap();
            let literal =
                |value| ScalarTerm::integer(integer, IntegerValue::Unsigned(value)).unwrap();
            let right = literal(7);
            let root =
                ScalarTerm::exact_integer_subtract(integer, literal(255), right.clone()).unwrap();
            let premise = Proposition::LessOrEqual(left.clone(), root.clone());
            let goal = Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::Add(
                    Box::new(mathematical(&left)),
                    Box::new(mathematical(&right)),
                ),
                mathematical(&literal(255)),
            );
            let proof = ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::IntegerAffineBound {
                    root_bound: Box::new(assumption_proof(premise.clone(), 0)),
                    witness: IntegerAffineWitness {
                        root,
                        target: ScalarTerm::exact_integer_add(integer, left, right).unwrap(),
                        definition_axioms: Vec::new(),
                        literal_axioms: Vec::new(),
                    },
                },
            };
            let assumptions = [premise];
            let parameters = BTreeSet::new();
            let elaboration =
                verify_then_elaborate(&context, &goal, &assumptions, &parameters, &proof);
            assert!(elaboration.denotation.rule_axioms.is_empty());
            assert_eq!(elaboration.denotation.addition.laws.len(), 1);
            assert!(
                elaboration
                    .denotation
                    .addition
                    .laws
                    .contains_key(&super::Law::Monotone)
            );
            assert_eq!(elaboration.denotation.addition.numeral_sums.len(), 1);
            assert_eq!(
                elaboration
                    .denotation
                    .addition
                    .numeral_sums
                    .keys()
                    .next()
                    .unwrap(),
                &(
                    IntegerValue::Unsigned(248),
                    IntegerValue::Unsigned(7),
                    IntegerValue::Unsigned(255)
                )
            );
            let mut wrong_premise = proof.clone();
            let ProofRule::IntegerAffineBound { root_bound, .. } = &mut wrong_premise.rule else {
                unreachable!()
            };
            root_bound.conclusion = Proposition::LessOrEqual(
                ScalarTerm::value(ValueId::new(1).unwrap(), scalar),
                literal(249),
            );
            assert_refused(&context, &goal, &assumptions, &[], &wrong_premise);
            let mut wrong_goal = proof.clone();
            wrong_goal.conclusion = Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::Add(
                    Box::new(mathematical(&ScalarTerm::value(
                        ValueId::new(1).unwrap(),
                        scalar,
                    ))),
                    Box::new(mathematical(&literal(7))),
                ),
                mathematical(&literal(254)),
            );
            assert_refused(&context, &goal, &assumptions, &[], &wrong_goal);
        }

        fn exact_add_definition_fixture(
            context: &PropositionContext,
            goal: &Proposition,
            assumptions: &[Proposition],
            axioms: &[Proposition],
            proof: &ProofNode,
        ) -> (crate::BoundedDenotation, super::super::Denotation) {
            let denoted = verify_bounded_certificate(
                context,
                goal,
                assumptions,
                axioms,
                proof,
                &mut Budget::default(),
            )
            .unwrap();
            let parameters = BTreeSet::new();
            let elaboration =
                elaborate_checked(context, goal, assumptions, axioms, &parameters, proof);
            (denoted, elaboration.denotation)
        }

        fn exact_add_definition_proof(
            left_premise: &Proposition,
            right_premise: &Proposition,
            definition_axiom: usize,
            goal: &Proposition,
        ) -> ProofNode {
            ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::IntegerExactAddDefinitionBound {
                    left_bound: Box::new(assumption_proof(left_premise.clone(), 0)),
                    right_bound: Box::new(assumption_proof(right_premise.clone(), 1)),
                    definition_axiom,
                },
            }
        }

        fn check_exact_add_definition_bound(integer: IntegerType, lower: bool, reversed: bool) {
            let scalar = ScalarType::Integer(integer);
            let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
            let context = value_context(scalar, 3);
            let (left_endpoint, right_endpoint, bound) = match (integer.sign(), lower) {
                (IntegerSign::Signed, true) => (
                    IntegerValue::Signed(-5),
                    IntegerValue::Signed(-3),
                    IntegerValue::Signed(-8),
                ),
                (IntegerSign::Signed, false) => (
                    IntegerValue::Signed(7),
                    IntegerValue::Signed(9),
                    IntegerValue::Signed(16),
                ),
                (IntegerSign::Unsigned, true) => (
                    IntegerValue::Unsigned(2),
                    IntegerValue::Unsigned(3),
                    IntegerValue::Unsigned(5),
                ),
                (IntegerSign::Unsigned, false) => (
                    IntegerValue::Unsigned(100),
                    IntegerValue::Unsigned(50),
                    IntegerValue::Unsigned(150),
                ),
            };
            let literal = |value| ScalarTerm::integer(integer, value).unwrap();
            let output = value(3);
            let expression = ScalarTerm::exact_integer_add(integer, value(1), value(2)).unwrap();
            let (left_premise, right_premise) = if lower {
                (
                    Proposition::LessOrEqual(literal(left_endpoint), value(1)),
                    Proposition::LessOrEqual(literal(right_endpoint), value(2)),
                )
            } else {
                (
                    Proposition::LessOrEqual(value(1), literal(left_endpoint)),
                    Proposition::LessOrEqual(value(2), literal(right_endpoint)),
                )
            };
            let definition = if reversed {
                Proposition::Equal(expression.clone(), output.clone())
            } else {
                Proposition::Equal(output.clone(), expression.clone())
            };
            let goal = if lower {
                Proposition::LessOrEqual(literal(bound), output.clone())
            } else {
                Proposition::LessOrEqual(output.clone(), literal(bound))
            };
            let proof = exact_add_definition_proof(&left_premise, &right_premise, 0, &goal);
            let assumptions = [left_premise.clone(), right_premise.clone()];
            let axioms = [definition.clone()];
            let (denoted, denotation) =
                exact_add_definition_fixture(&context, &goal, &assumptions, &axioms, &proof);
            assert!(denoted.certificate.signature.len() < 400);
            assert!(
                denotation.rule_axioms.is_empty(),
                "exact-add definition bounds derive from the fixed monotone law"
            );
            assert!(denotation.decisions.is_empty());
            assert_eq!(denotation.addition.laws.len(), 1);
            assert_eq!(
                denotation.addition.numeral_sums.keys().collect::<Vec<_>>(),
                [&(left_endpoint, right_endpoint, bound)]
            );
        }

        #[test]
        fn exact_add_definition_bounds_use_the_two_sided_monotone_law() {
            for (sign, width) in [
                (IntegerSign::Unsigned, 8),
                (IntegerSign::Unsigned, 64),
                (IntegerSign::Signed, 32),
                (IntegerSign::Signed, 128),
            ] {
                for lower in [true, false] {
                    for reversed in [false, true] {
                        check_exact_add_definition_bound(
                            IntegerType::new(sign, width).unwrap(),
                            lower,
                            reversed,
                        );
                    }
                }
            }
        }

        #[test]
        fn exact_add_definition_bound_transports_operand_equalities() {
            let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
            let scalar = ScalarType::Integer(integer);
            let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
            let literal = |magnitude| {
                ScalarTerm::integer(integer, IntegerValue::Unsigned(magnitude)).unwrap()
            };
            let context = value_context(scalar, 3);
            let output = value(3);
            let expression = ScalarTerm::exact_integer_add(integer, value(1), value(2)).unwrap();
            for reversed_equality in [false, true] {
                for lower in [true, false] {
                    let (left_premise, right_premise, bound) = if lower {
                        let equality = if reversed_equality {
                            Proposition::Equal(literal(4), value(1))
                        } else {
                            Proposition::Equal(value(1), literal(4))
                        };
                        (
                            equality,
                            Proposition::LessOrEqual(literal(3), value(2)),
                            literal(7),
                        )
                    } else {
                        let equality = if reversed_equality {
                            Proposition::Equal(literal(4), value(1))
                        } else {
                            Proposition::Equal(value(1), literal(4))
                        };
                        (
                            equality,
                            Proposition::LessOrEqual(value(2), literal(9)),
                            literal(13),
                        )
                    };
                    let definition = Proposition::Equal(output.clone(), expression.clone());
                    let goal = if lower {
                        Proposition::LessOrEqual(bound.clone(), output.clone())
                    } else {
                        Proposition::LessOrEqual(output.clone(), bound.clone())
                    };
                    let proof = exact_add_definition_proof(&left_premise, &right_premise, 0, &goal);
                    let assumptions = [left_premise, right_premise];
                    let axioms = [definition];
                    let (_, denotation) = exact_add_definition_fixture(
                        &context,
                        &goal,
                        &assumptions,
                        &axioms,
                        &proof,
                    );
                    assert!(denotation.rule_axioms.is_empty());
                    assert_eq!(denotation.addition.laws.len(), 1);
                }
            }
        }

        #[test]
        fn exact_add_definition_bound_rejects_mismatched_certificates() {
            let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
            let scalar = ScalarType::Integer(integer);
            let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
            let literal = |magnitude| {
                ScalarTerm::integer(integer, IntegerValue::Unsigned(magnitude)).unwrap()
            };
            let context = value_context(scalar, 4);
            let output = value(3);
            let expression = ScalarTerm::exact_integer_add(integer, value(1), value(2)).unwrap();
            let left_premise = Proposition::LessOrEqual(literal(2), value(1));
            let right_premise = Proposition::LessOrEqual(literal(3), value(2));
            let definition = Proposition::Equal(output.clone(), expression.clone());
            let goal = Proposition::LessOrEqual(literal(5), output.clone());
            let proof = exact_add_definition_proof(&left_premise, &right_premise, 0, &goal);
            let assumptions = [left_premise, right_premise];
            let axioms = [definition];
            for wrong_goal in [
                Proposition::LessOrEqual(literal(6), output.clone()),
                Proposition::LessOrEqual(literal(5), value(4)),
                Proposition::LessOrEqual(literal(5), expression.clone()),
            ] {
                let mut invalid = proof.clone();
                invalid.conclusion = wrong_goal.clone();
                assert_refused(&context, &wrong_goal, &assumptions, &axioms, &invalid);
            }
            for wrong_premise in [
                Proposition::LessOrEqual(literal(2), value(2)),
                Proposition::LessOrEqual(literal(3), value(1)),
                Proposition::LessOrEqual(value(1), literal(2)),
                Proposition::Equal(value(1), value(2)),
            ] {
                let mut invalid = proof.clone();
                let ProofRule::IntegerExactAddDefinitionBound { left_bound, .. } =
                    &mut invalid.rule
                else {
                    unreachable!()
                };
                left_bound.conclusion = wrong_premise.clone();
                let wrong_assumptions = [wrong_premise, assumptions[1].clone()];
                assert_refused(&context, &goal, &wrong_assumptions, &axioms, &invalid);
            }
            for wrong_axiom in [
                Proposition::Equal(value(4), expression.clone()),
                Proposition::Equal(
                    output.clone(),
                    ScalarTerm::exact_integer_add(integer, value(1), value(1)).unwrap(),
                ),
                Proposition::LessOrEqual(output.clone(), expression.clone()),
            ] {
                assert_refused(
                    &context,
                    &goal,
                    &assumptions,
                    from_ref(&wrong_axiom),
                    &proof,
                );
            }
            let mut invalid = proof.clone();
            let ProofRule::IntegerExactAddDefinitionBound {
                definition_axiom, ..
            } = &mut invalid.rule
            else {
                unreachable!()
            };
            *definition_axiom = 7;
            assert_refused(&context, &goal, &assumptions, &axioms, &invalid);
        }

        #[test]
        fn direct_add_bound_out_of_numeral_range_keeps_instance_fallback() {
            let integer = IntegerType::new(IntegerSign::Signed, 128).unwrap();
            let scalar = ScalarType::Integer(integer);
            let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
            let literal =
                |value| ScalarTerm::integer(integer, IntegerValue::Signed(value)).unwrap();
            let minimum = literal(i128::MIN);
            let context = value_context(scalar, 2);
            let premise = Proposition::Conjunction(vec![
                Proposition::Equal(value(1), literal(i128::MIN + 1)),
                Proposition::LessOrEqual(minimum, value(2)),
            ]);
            let bound =
                IntegerMathTerm::IntegerLiteral(IntegerMathLiteral::new(true, u128::MAX).unwrap());
            let goal = Proposition::IntegerMathLessOrEqual(
                bound,
                IntegerMathTerm::Add(
                    Box::new(mathematical(&value(1))),
                    Box::new(mathematical(&value(2))),
                ),
            );
            let proof = direct_affine_proof(
                integer,
                value(1),
                value(2),
                &premise,
                &goal,
                ScalarTerm::exact_integer_add,
            );
            let denotation = direct_fixture(&context, &goal, &premise, &proof);
            assert_eq!(
                denotation.rule_axioms.len(),
                1,
                "an unrepresentable endpoint sum keeps the explicit instance assumption"
            );
        }

        #[test]
        fn direct_add_bound_rejects_mismatched_certificates() {
            let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
            let scalar = ScalarType::Integer(integer);
            let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
            let literal = |magnitude| {
                ScalarTerm::integer(integer, IntegerValue::Unsigned(magnitude)).unwrap()
            };
            let context = value_context(scalar, 3);
            let sum = IntegerMathTerm::Add(
                Box::new(mathematical(&value(1))),
                Box::new(mathematical(&value(2))),
            );
            let premise = Proposition::Conjunction(vec![
                Proposition::LessOrEqual(literal(2), value(1)),
                Proposition::LessOrEqual(literal(3), value(2)),
            ]);
            let goal = Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::literal(IntegerValue::Unsigned(5)),
                sum,
            );
            let proof = direct_affine_proof(
                integer,
                value(1),
                value(2),
                &premise,
                &goal,
                ScalarTerm::exact_integer_add,
            );
            let assumptions = [premise];
            for wrong_goal in [
                Proposition::IntegerMathLessOrEqual(
                    IntegerMathTerm::literal(IntegerValue::Unsigned(6)),
                    IntegerMathTerm::Add(
                        Box::new(mathematical(&value(1))),
                        Box::new(mathematical(&value(2))),
                    ),
                ),
                Proposition::IntegerMathLessOrEqual(
                    IntegerMathTerm::literal(IntegerValue::Unsigned(5)),
                    IntegerMathTerm::Add(
                        Box::new(mathematical(&value(1))),
                        Box::new(mathematical(&value(3))),
                    ),
                ),
                Proposition::IntegerMathLessOrEqual(
                    IntegerMathTerm::Add(
                        Box::new(mathematical(&value(1))),
                        Box::new(mathematical(&value(2))),
                    ),
                    IntegerMathTerm::literal(IntegerValue::Unsigned(5)),
                ),
            ] {
                let mut invalid = proof.clone();
                invalid.conclusion = wrong_goal.clone();
                assert_refused(&context, &wrong_goal, &assumptions, &[], &invalid);
            }
            for wrong_premise in [
                Proposition::LessOrEqual(literal(2), value(1)),
                Proposition::Conjunction(vec![
                    Proposition::LessOrEqual(literal(2), value(1)),
                    Proposition::LessOrEqual(literal(3), value(1)),
                ]),
                Proposition::Conjunction(vec![
                    Proposition::LessOrEqual(value(1), literal(2)),
                    Proposition::LessOrEqual(literal(3), value(2)),
                ]),
            ] {
                let mut invalid = proof.clone();
                let ProofRule::IntegerAffineBound { root_bound, .. } = &mut invalid.rule else {
                    unreachable!()
                };
                root_bound.conclusion = wrong_premise.clone();
                assert_refused(&context, &goal, from_ref(&wrong_premise), &[], &invalid);
            }
        }
    }
}
mod binary_numerals {
    use std::collections::BTreeMap;

    use numerics::bignum::BigInt;
    use semantic_vocabulary::{IntegerValue, Proposition, ScalarTerm};

    use super::{BoundedDenotationError, Declaration, Denotation, IntegerLaw, Term, TermHandle};
    use crate::mathematical_core::scheme_dsl::{Syntax, apps, id, pi, scheme_at, v};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    enum Constructor {
        Double,
        Odd,
        Negate,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    enum Law {
        DoubleZero,
        NegateZero,
        EvenBeforeOdd,
        OddBeforeNextEven,
        NegateOrder,
    }

    #[derive(Default)]
    pub(super) struct BinaryNumerals {
        zero: Option<u32>,
        constructors: BTreeMap<Constructor, u32>,
        prefixes: BTreeMap<(Constructor, u32), u32>,
        laws: BTreeMap<Law, u32>,
    }

    pub(super) fn fixed_magnitude(value: &BigInt) -> Option<(bool, u128)> {
        if value.bit_length() > u128::BITS as usize {
            return None;
        }
        let radix = BigInt::from_u128(1_u128 << 64);
        let (high, low) = value.abs().div_rem(&radix)?;
        Some((
            value.is_negative(),
            (u128::from(high.to_u64()?) << 64) | u128::from(low.to_u64()?),
        ))
    }

    impl Denotation {
        fn numeral_zero(&mut self) -> Result<u32, BoundedDenotationError> {
            self.interned_slot_assumption(
                |this| &mut this.binary_numerals.zero,
                |this| this.integer_constant(),
            )
        }

        fn numeral_constructor(
            &mut self,
            constructor: Constructor,
        ) -> Result<u32, BoundedDenotationError> {
            self.interned_keyed_assumption(
                |this| &mut this.binary_numerals.constructors,
                constructor,
                |this| {
                    let integer = this.integer_constant()?;
                    Ok(this.arena.insert(Term::Pi {
                        domain: integer,
                        codomain: integer,
                    }))
                },
            )
        }

        fn numeral_prefix(
            &mut self,
            constructor: Constructor,
            prefix: u32,
        ) -> Result<u32, BoundedDenotationError> {
            if let Some(&position) = self.binary_numerals.prefixes.get(&(constructor, prefix)) {
                return Ok(position);
            }
            let function_position = self.numeral_constructor(constructor)?;
            let function = self.constant(function_position);
            let argument = self.constant(prefix);
            let body = self.arena.insert(Term::Apply { function, argument });
            let integer = self.integer_constant()?;
            let position = self.position()?;
            self.declarations
                .push(Declaration::definition(0, integer, body));
            self.binary_numerals
                .prefixes
                .insert((constructor, prefix), position);
            Ok(position)
        }

        pub(super) fn binary_integer(
            &mut self,
            negative: bool,
            magnitude: u128,
        ) -> Result<u32, BoundedDenotationError> {
            let mut position = self.numeral_zero()?;
            for bit in (0..(u128::BITS - magnitude.leading_zeros())).rev() {
                let constructor = if magnitude & (1_u128 << bit) == 0 {
                    Constructor::Double
                } else {
                    Constructor::Odd
                };
                position = self.numeral_prefix(constructor, position)?;
            }
            if negative && magnitude != 0 {
                position = self.numeral_prefix(Constructor::Negate, position)?;
            }
            Ok(position)
        }

        fn binary_term(
            &mut self,
            negative: bool,
            magnitude: u128,
        ) -> Result<TermHandle, BoundedDenotationError> {
            let position = self.binary_integer(negative, magnitude)?;
            Ok(self.constant(position))
        }

        fn numeral_law(&mut self, law: Law) -> Result<u32, BoundedDenotationError> {
            self.roster_law_position(
                |this| &mut this.binary_numerals.laws,
                law,
                |this| -> Result<Syntax, BoundedDenotationError> {
                    let integer = this.integer()?;
                    let less_than = this.integer_less_than()?;
                    let zero = this.numeral_zero()?;
                    let double = this.numeral_constructor(Constructor::Double)?;
                    let odd = this.numeral_constructor(Constructor::Odd)?;
                    let negate = this.numeral_constructor(Constructor::Negate)?;
                    let constant = |position| scheme_at(position, Vec::new());
                    let apply = |position, value: Syntax| apps(constant(position), [value]);
                    let lt = |left, right| apps(constant(less_than), [left, right]);
                    let carrier = || constant(integer);
                    Ok(match law {
                        Law::DoubleZero => {
                            id(carrier(), apply(double, constant(zero)), constant(zero))
                        }
                        Law::NegateZero => {
                            id(carrier(), apply(negate, constant(zero)), constant(zero))
                        }
                        Law::EvenBeforeOdd => pi(
                            "x",
                            carrier(),
                            lt(apply(double, v("x")), apply(odd, v("x"))),
                        ),
                        Law::OddBeforeNextEven => pi(
                            "x",
                            carrier(),
                            pi(
                                "y",
                                carrier(),
                                pi(
                                    "_",
                                    lt(v("x"), v("y")),
                                    lt(apply(odd, v("x")), apply(double, v("y"))),
                                ),
                            ),
                        ),
                        Law::NegateOrder => pi(
                            "x",
                            carrier(),
                            pi(
                                "y",
                                carrier(),
                                pi(
                                    "_",
                                    lt(v("x"), v("y")),
                                    lt(apply(negate, v("y")), apply(negate, v("x"))),
                                ),
                            ),
                        ),
                    })
                },
            )
        }

        fn numeral_law_application(
            &mut self,
            law: Law,
            arguments: &[TermHandle],
        ) -> Result<TermHandle, BoundedDenotationError> {
            let position = self.numeral_law(law)?;
            Ok(self.constant_application(position, arguments))
        }

        fn positive_adjacent(&mut self, lower: u128) -> Result<TermHandle, BoundedDenotationError> {
            let half = self.binary_term(false, lower / 2)?;
            if lower & 1 != 0 {
                let evidence = self.positive_adjacent(lower / 2)?;
                let next_half = self.binary_term(false, lower / 2 + 1)?;
                return self
                    .numeral_law_application(Law::OddBeforeNextEven, &[half, next_half, evidence]);
            }
            let evidence = self.numeral_law_application(Law::EvenBeforeOdd, &[half])?;
            if lower != 0 {
                return Ok(evidence);
            }
            let zero_position = self.numeral_zero()?;
            let doubled_position = self.numeral_prefix(Constructor::Double, zero_position)?;
            let doubled = self.constant(doubled_position);
            let one = self.binary_term(false, 1)?;
            let equality = self.numeral_law_application(Law::DoubleZero, &[])?;
            self.integer_law_application(
                IntegerLaw::LessThanSubstituteLeft,
                &[doubled, half, one, equality, evidence],
            )
        }

        fn literal_adjacent(
            &mut self,
            lower: &ScalarTerm,
        ) -> Result<TermHandle, BoundedDenotationError> {
            let (_, value) = lower
                .integer_value()
                .ok_or(BoundedDenotationError::Unsupported(
                    "discreteness needs a fixed literal",
                ))?;
            let (negative, magnitude) = match value {
                IntegerValue::Signed(value) => (value < 0, value.unsigned_abs()),
                IntegerValue::Unsigned(value) => (false, value),
            };
            if !negative {
                return self.positive_adjacent(magnitude);
            }
            let evidence = self.positive_adjacent(magnitude - 1)?;
            let left = self.binary_term(false, magnitude - 1)?;
            let right = self.binary_term(false, magnitude)?;
            let evidence =
                self.numeral_law_application(Law::NegateOrder, &[left, right, evidence])?;
            if magnitude != 1 {
                return Ok(evidence);
            }
            let zero_position = self.numeral_zero()?;
            let negative_zero_position = self.numeral_prefix(Constructor::Negate, zero_position)?;
            let negative_zero = self.constant(negative_zero_position);
            let negative_one = self.binary_term(true, 1)?;
            let equality = self.numeral_law_application(Law::NegateZero, &[])?;
            self.integer_law_application(
                IntegerLaw::LessThanSubstituteRight,
                &[negative_one, negative_zero, left, equality, evidence],
            )
        }

        fn positive_order(
            &mut self,
            lower: u128,
            upper: u128,
        ) -> Result<TermHandle, BoundedDenotationError> {
            debug_assert!(lower < upper);
            let lower_half = self.binary_term(false, lower / 2)?;
            let upper_half = self.binary_term(false, upper / 2)?;
            let evidence = if lower / 2 == upper / 2 {
                self.numeral_law_application(Law::EvenBeforeOdd, &[lower_half])?
            } else {
                let prefix_order = self.positive_order(lower / 2, upper / 2)?;
                let mut evidence = self.numeral_law_application(
                    Law::OddBeforeNextEven,
                    &[lower_half, upper_half, prefix_order],
                )?;
                let lower_odd = self.binary_term(false, lower / 2 * 2 + 1)?;
                let upper_even = self.binary_term(false, upper / 2 * 2)?;
                if lower & 1 == 0 {
                    let prefix = self.binary_integer(false, lower / 2)?;
                    let doubled = self.numeral_prefix(Constructor::Double, prefix)?;
                    let doubled = self.constant(doubled);
                    let first = self.numeral_law_application(Law::EvenBeforeOdd, &[lower_half])?;
                    evidence = self.integer_law_application(
                        IntegerLaw::LessThanTransitivity,
                        &[doubled, lower_odd, upper_even, first, evidence],
                    )?;
                }
                if upper & 1 != 0 {
                    let last = self.numeral_law_application(Law::EvenBeforeOdd, &[upper_half])?;
                    let lower_position = self.binary_integer(false, lower / 2)?;
                    let lower_position = self.numeral_prefix(
                        if lower & 1 == 0 {
                            Constructor::Double
                        } else {
                            Constructor::Odd
                        },
                        lower_position,
                    )?;
                    let lower_raw = self.constant(lower_position);
                    let upper_term = self.binary_term(false, upper)?;
                    evidence = self.integer_law_application(
                        IntegerLaw::LessThanTransitivity,
                        &[lower_raw, upper_even, upper_term, evidence, last],
                    )?;
                }
                evidence
            };
            if lower != 0 {
                return Ok(evidence);
            }
            let zero = self.numeral_zero()?;
            let doubled = self.numeral_prefix(Constructor::Double, zero)?;
            let doubled = self.constant(doubled);
            let zero = self.constant(zero);
            let upper = self.binary_term(false, upper)?;
            let equality = self.numeral_law_application(Law::DoubleZero, &[])?;
            self.integer_law_application(
                IntegerLaw::LessThanSubstituteLeft,
                &[doubled, zero, upper, equality, evidence],
            )
        }

        pub(super) fn literal_order(
            &mut self,
            lower: IntegerValue,
            upper: IntegerValue,
        ) -> Result<TermHandle, BoundedDenotationError> {
            let parts = |value| match value {
                IntegerValue::Signed(value) => (value < 0, value.unsigned_abs()),
                IntegerValue::Unsigned(value) => (false, value),
            };
            let (negative_lower, lower) = parts(lower);
            let (negative_upper, upper) = parts(upper);
            if !negative_lower {
                debug_assert!(!negative_upper && lower < upper);
                return self.positive_order(lower, upper);
            }
            if negative_upper || upper == 0 {
                let order = self.positive_order(upper, lower)?;
                let positive_left = self.binary_term(false, upper)?;
                let positive_right = self.binary_term(false, lower)?;
                let order = self.numeral_law_application(
                    Law::NegateOrder,
                    &[positive_left, positive_right, order],
                )?;
                if upper != 0 {
                    return Ok(order);
                }
                let zero = self.numeral_zero()?;
                let negative_zero = self.numeral_prefix(Constructor::Negate, zero)?;
                let negative_zero = self.constant(negative_zero);
                let left = self.binary_term(true, lower)?;
                let equality = self.numeral_law_application(Law::NegateZero, &[])?;
                return self.integer_law_application(
                    IntegerLaw::LessThanSubstituteRight,
                    &[left, negative_zero, positive_left, equality, order],
                );
            }
            let left_value = if lower == 1_u128 << 127 {
                i128::MIN
            } else {
                -(lower as i128)
            };
            let first =
                self.literal_order(IntegerValue::Signed(left_value), IntegerValue::Signed(0))?;
            let second = self.positive_order(0, upper)?;
            let left = self.binary_term(true, lower)?;
            let zero = self.binary_term(false, 0)?;
            let right = self.binary_term(false, upper)?;
            self.integer_law_application(
                IntegerLaw::LessThanTransitivity,
                &[left, zero, right, first, second],
            )
        }

        pub(super) fn discreteness_evidence(
            &mut self,
            premise: &Proposition,
            evidence: TermHandle,
            goal: &Proposition,
        ) -> Result<TermHandle, BoundedDenotationError> {
            let (
                Proposition::LessOrEqual(left, right),
                Proposition::LessThan(strict_left, strict_right),
            ) = (premise, goal)
            else {
                return Err(BoundedDenotationError::Unsupported(
                    "checked discreteness has integer order endpoints",
                ));
            };
            let premise_type = self.denote(premise)?;
            let goal_type = self.denote(goal)?;
            let (
                Some(super::IntegerRelation::LessOrEqual { left: a, right: b }),
                Some(super::IntegerRelation::LessThan { left: c, right: d }),
            ) = (
                self.integer_relation(premise_type),
                self.integer_relation(goal_type),
            )
            else {
                return Err(BoundedDenotationError::Unsupported(
                    "checked discreteness denotes in Int",
                ));
            };
            if right == strict_right {
                let adjacent = self.literal_adjacent(strict_left)?;
                self.integer_law_application(
                    IntegerLaw::LessThanLessOrEqualTransitivity,
                    &[c, a, b, adjacent, evidence],
                )
            } else {
                debug_assert_eq!(left, strict_left);
                let adjacent = self.literal_adjacent(right)?;
                self.integer_law_application(
                    IntegerLaw::LessOrEqualLessThanTransitivity,
                    &[a, b, d, evidence, adjacent],
                )
            }
        }
    }

    impl BinaryNumerals {
        pub(super) fn source_safety_roster(&self) -> Vec<u32> {
            self.zero
                .into_iter()
                .chain(self.constructors.values().copied())
                .chain(self.laws.values().copied())
                .collect()
        }
    }

    #[cfg(test)]
    mod tests {
        use std::collections::BTreeSet;

        use semantic_vocabulary::{
            IntegerMathTerm, IntegerSign, IntegerType, PropositionContext, ScalarType, ValueId,
        };

        use super::super::{Elaboration, MathTermKey};
        use super::{Constructor, IntegerValue, Law, Proposition, ScalarTerm, Term};
        use crate::proof::assumption_proof;
        use crate::{
            Budget, PrimitiveJudgment, ProofNode, ProofRule, certificate_assumption_closure,
            proof_node, verify_bounded_certificate, verify_mathematical_certificate,
        };

        fn fixture(
            sign: IntegerSign,
            lower: IntegerValue,
            upper: IntegerValue,
            lower_bound: bool,
        ) -> (PropositionContext, Proposition, ProofNode) {
            let integer = IntegerType::new(sign, 128).unwrap();
            let scalar = ScalarType::Integer(integer);
            let identity = ValueId::new(1).unwrap();
            let value = ScalarTerm::value(identity, scalar);
            let lower = ScalarTerm::integer(integer, lower).unwrap();
            let upper = ScalarTerm::integer(integer, upper).unwrap();
            let (premise, conclusion) = if lower_bound {
                (
                    Proposition::LessOrEqual(upper, value.clone()),
                    Proposition::LessThan(lower, value),
                )
            } else {
                (
                    Proposition::LessOrEqual(value.clone(), lower),
                    Proposition::LessThan(value, upper),
                )
            };
            let proof = ProofNode {
                conclusion,
                rule: ProofRule::IntegerOrderDiscreteness {
                    relation: Box::new(assumption_proof(premise.clone(), 0)),
                },
            };
            (
                PropositionContext::from_value_types([(identity, scalar)]).unwrap(),
                premise,
                proof,
            )
        }

        #[test]
        fn binary_carry_and_signed_zero_use_exact_shared_law_closures() {
            for (lower, upper, expected_laws) in [
                (0, 1, vec![Law::DoubleZero, Law::EvenBeforeOdd]),
                (2, 3, vec![Law::EvenBeforeOdd]),
                (
                    7,
                    8,
                    vec![Law::DoubleZero, Law::EvenBeforeOdd, Law::OddBeforeNextEven],
                ),
                (
                    -2,
                    -1,
                    vec![
                        Law::DoubleZero,
                        Law::EvenBeforeOdd,
                        Law::OddBeforeNextEven,
                        Law::NegateOrder,
                    ],
                ),
                (
                    -1,
                    0,
                    vec![
                        Law::DoubleZero,
                        Law::NegateZero,
                        Law::EvenBeforeOdd,
                        Law::NegateOrder,
                    ],
                ),
            ] {
                for lower_bound in [false, true] {
                    let (context, premise, proof) = fixture(
                        IntegerSign::Signed,
                        IntegerValue::Signed(lower),
                        IntegerValue::Signed(upper),
                        lower_bound,
                    );
                    let premises = [premise];
                    let parameters = BTreeSet::new();
                    let mut elaboration =
                        Elaboration::new(&context, &proof.conclusion, &premises, &[], &parameters)
                            .unwrap();
                    let evidence = elaboration.node(&proof).unwrap();
                    let denotation = &elaboration.denotation;
                    assert!(
                        denotation.rule_axioms.is_empty(),
                        "discreteness must not assume its instance"
                    );
                    assert!(
                        denotation.decisions.is_empty(),
                        "literal adjacency must not become a primitive decision assumption"
                    );
                    assert_eq!(
                        denotation
                            .binary_numerals
                            .laws
                            .keys()
                            .copied()
                            .collect::<Vec<_>>(),
                        expected_laws
                    );

                    let mut expected_closure = BTreeSet::from([
                        denotation.integer.unwrap(),
                        denotation.integer_less_than.unwrap(),
                        denotation.integer_less_or_equal.unwrap(),
                        denotation.binary_numerals.zero.unwrap(),
                        denotation.binary_numerals.constructors[&Constructor::Double],
                        denotation.binary_numerals.constructors[&Constructor::Odd],
                    ]);
                    if lower < 0 {
                        expected_closure
                            .insert(denotation.binary_numerals.constructors[&Constructor::Negate]);
                    }
                    expected_closure.extend(denotation.binary_numerals.laws.values());
                    expected_closure.extend(denotation.integer_laws.values());
                    expected_closure.extend(denotation.math_terms.iter().filter_map(
                        |(term, &position)| {
                            matches!(term, MathTermKey::Open(_)).then_some(position)
                        },
                    ));
                    let mut denoted = elaboration.finish(evidence);
                    verify_mathematical_certificate(
                        &mut denoted.arena,
                        &denoted.certificate,
                        &mut Budget::default(),
                    )
                    .unwrap();
                    assert_eq!(
                        certificate_assumption_closure(&denoted.arena, &denoted.certificate),
                        expected_closure
                    );
                    let mut malformed = denoted.certificate.clone();
                    malformed.term = denoted.arena.insert(Term::Variable(0));
                    assert!(
                        verify_mathematical_certificate(
                            &mut denoted.arena,
                            &malformed,
                            &mut Budget::default()
                        )
                        .is_err(),
                        "the kernel must reject the inclusive premise as strict evidence"
                    );
                }
            }
        }

        #[test]
        fn large_closed_values_keep_exact_opaque_identity_without_a_new_capacity_limit() {
            let literal = |value| IntegerMathTerm::literal(IntegerValue::Unsigned(value));
            let huge = IntegerMathTerm::ShiftLeft {
                value: Box::new(literal(1)),
                count: Box::new(literal(65_535)),
            };
            let alternate = IntegerMathTerm::ShiftLeft {
                value: Box::new(literal(2)),
                count: Box::new(literal(65_534)),
            };
            let mut terms = [huge, alternate];
            terms.sort();
            let goal = Proposition::IntegerMathEqual(terms[0].clone(), terms[1].clone());
            let proof = proof_node!(goal.clone() => ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation));
            let denoted = verify_bounded_certificate(
                &PropositionContext::default(),
                &goal,
                &[],
                &[],
                &proof,
                &mut Budget::default(),
            )
            .unwrap();
            assert_eq!(
                denoted.certificate.signature.len(),
                2,
                "carrier and one exact-value constant, as before binary numeral denotation"
            );
            assert_eq!(
                certificate_assumption_closure(&denoted.arena, &denoted.certificate),
                BTreeSet::from([0, 1])
            );
            assert!(matches!(
                denoted.arena.get(denoted.certificate.term),
                Term::Refl { .. }
            ));
        }
    }
}
mod booleans {
    use super::{
        BoundedDenotationError, Denotation, MAX_ELABORATION_NODES, ScalarTerm, Term, TermArena,
        TermHandle,
    };
    use crate::mathematical_core::substitution::{shift, substitute};

    pub(super) mod scalar_denotation {
        use super::*;

        enum Pending<'source> {
            Not,
            EqualLeft { right: &'source ScalarTerm },
            EqualRight { left: TermHandle },
        }

        pub(crate) fn denote(
            denotation: &mut Denotation,
            term: &ScalarTerm,
        ) -> Result<TermHandle, BoundedDenotationError> {
            let mut pending = Vec::new();
            let mut current = term;
            'enter: loop {
                let mut value = match current {
                    ScalarTerm::Boolean(false) => denotation.two_zero,
                    ScalarTerm::Boolean(true) => denotation.two_one,
                    ScalarTerm::BooleanNot { operand } => {
                        pending.push(Pending::Not);
                        current = operand;
                        continue 'enter;
                    }
                    ScalarTerm::BooleanEqual { left, right } => {
                        pending.push(Pending::EqualLeft { right });
                        current = left;
                        continue 'enter;
                    }
                    _ => denotation.scalar_leaf(current)?,
                };
                loop {
                    match pending.pop() {
                        Some(Pending::Not) => {
                            value = denotation.boolean_case(
                                value,
                                denotation.two_one,
                                denotation.two_zero,
                            );
                        }
                        Some(Pending::EqualLeft { right }) => {
                            pending.push(Pending::EqualRight { left: value });
                            current = right;
                            continue 'enter;
                        }
                        Some(Pending::EqualRight { left }) => {
                            if !denotation.boolean_equality.is_valid() {
                                denotation.boolean_equality = equality_function(
                                    &mut denotation.arena,
                                    denotation.two,
                                    denotation.two_zero,
                                    denotation.two_one,
                                );
                            }
                            value = denotation.arena.apply_binary(
                                denotation.boolean_equality,
                                left,
                                value,
                            );
                        }
                        None => return Ok(value),
                    }
                }
            }
        }
    }

    macro_rules! id {
        ($ty:expr => $left:expr => $right:expr) => {
            Term::Id {
                ty: $ty,
                left: $left,
                right: $right,
            }
        };
    }

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

        pub(super) fn boolean_identity_transport(
            &mut self,
            premise: TermHandle,
            goal: TermHandle,
            evidence: TermHandle,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            let Some((goal_carrier, _, _)) = self.identity_parts(goal) else {
                return Ok(None);
            };
            if !self.arena.structurally_equal(goal_carrier, self.two) {
                return Ok(None);
            }
            self.boolean_goal_transport(premise, goal, evidence)
        }

        pub(super) fn boolean_goal_transport(
            &mut self,
            premise: TermHandle,
            goal: TermHandle,
            evidence: TermHandle,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            let Some((carrier, premise_left, premise_right)) = self.identity_parts(premise) else {
                return Ok(None);
            };
            if !self.arena.structurally_equal(carrier, self.two) {
                return Ok(None);
            }
            let mut remaining = MAX_ELABORATION_NODES;
            let Some(function) =
                self.boolean_goal_case(premise_left, premise_right, goal, &mut remaining)?
            else {
                return Ok(None);
            };
            Ok(Some(self.arena.insert(Term::Apply {
                function,
                argument: evidence,
            })))
        }

        fn boolean_goal_case(
            &mut self,
            premise_left: TermHandle,
            premise_right: TermHandle,
            goal: TermHandle,
            remaining: &mut u64,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            let premise_evaluations =
                [premise_left, premise_right].map(|term| self.boolean_eval(term, remaining));
            if premise_evaluations
                .iter()
                .any(|evaluation| matches!(evaluation, BooleanEval::Unsupported))
            {
                return Ok(None);
            }
            let mut atom = premise_evaluations
                .iter()
                .find_map(|evaluation| match evaluation {
                    BooleanEval::Blocked(atom) => Some(*atom),
                    _ => None,
                });
            if atom.is_none() {
                atom = self.first_boolean_atom(goal, remaining);
            }
            if let Some(atom) = atom {
                let Some(abstracted_premise_left) =
                    self.abstract_atom(premise_left, atom, 0, remaining)
                else {
                    return Ok(None);
                };
                let Some(abstracted_premise_right) =
                    self.abstract_atom(premise_right, atom, 0, remaining)
                else {
                    return Ok(None);
                };
                let Some(abstracted_goal) = self.abstract_atom(goal, atom, 1, remaining) else {
                    return Ok(None);
                };
                let premise_type = self.arena.insert(Term::Id {
                    ty: self.two,
                    left: abstracted_premise_left,
                    right: abstracted_premise_right,
                });
                let body = self.arena.insert(Term::Pi {
                    domain: premise_type,
                    codomain: abstracted_goal,
                });
                let motive = self.arena.insert(Term::Lambda {
                    domain: self.two,
                    body,
                });
                let Some(zero_branch) = self.instantiated_goal_case(
                    premise_left,
                    premise_right,
                    goal,
                    atom,
                    self.two_zero,
                    remaining,
                )?
                else {
                    return Ok(None);
                };
                let Some(one_branch) = self.instantiated_goal_case(
                    premise_left,
                    premise_right,
                    goal,
                    atom,
                    self.two_one,
                    remaining,
                )?
                else {
                    return Ok(None);
                };
                return Ok(Some(self.arena.insert(Term::CaseTwo {
                    motive,
                    zero_branch,
                    one_branch,
                    scrutinee: atom,
                })));
            }
            let (
                BooleanEval::Literal(premise_holds_left),
                BooleanEval::Literal(premise_holds_right),
            ) = (premise_evaluations[0], premise_evaluations[1])
            else {
                return Ok(None);
            };
            let domain = self
                .arena
                .insert(id!(self.two => premise_left => premise_right));
            let body = match self.boolean_leaf_inhabitant(goal, remaining)? {
                Some(inhabitant) => inhabitant,
                None => {
                    if premise_holds_left == premise_holds_right {
                        return Ok(None);
                    }
                    let proof = self.arena.insert(Term::Variable(0));
                    self.two_identity_discriminant(
                        premise_left,
                        premise_right,
                        goal,
                        premise_holds_left,
                        proof,
                    )
                }
            };
            Ok(Some(self.arena.insert(Term::Lambda { domain, body })))
        }

        fn boolean_leaf_inhabitant(
            &mut self,
            goal: TermHandle,
            remaining: &mut u64,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            if let Some((ty, left, right)) = self.identity_parts(goal)
                && self.arena.structurally_equal(ty, self.two)
            {
                let (BooleanEval::Literal(left_literal), BooleanEval::Literal(right_literal)) = (
                    self.boolean_eval(left, remaining),
                    self.boolean_eval(right, remaining),
                ) else {
                    return Ok(None);
                };
                return Ok(if left_literal == right_literal {
                    Some(self.arena.insert(Term::Refl {
                        ty: self.two,
                        value: left,
                    }))
                } else {
                    None
                });
            }
            self.closed_inhabitant(goal)
        }

        fn instantiated_goal_case(
            &mut self,
            premise_left: TermHandle,
            premise_right: TermHandle,
            goal: TermHandle,
            atom: TermHandle,
            literal: TermHandle,
            remaining: &mut u64,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            let Some(premise_left) = self.instantiate_atom(premise_left, atom, literal, remaining)
            else {
                return Ok(None);
            };
            let Some(premise_right) =
                self.instantiate_atom(premise_right, atom, literal, remaining)
            else {
                return Ok(None);
            };
            let Some(goal) = self.instantiate_atom(goal, atom, literal, remaining) else {
                return Ok(None);
            };
            self.boolean_goal_case(premise_left, premise_right, goal, remaining)
        }

        fn first_boolean_atom(
            &mut self,
            term: TermHandle,
            remaining: &mut u64,
        ) -> Option<TermHandle> {
            if let BooleanEval::Blocked(atom) = self.boolean_eval(term, remaining)
                && self.neutral_atom_is_two(atom)
            {
                return Some(atom);
            }
            match self.arena.get(term) {
                Term::Apply { function, argument } => {
                    if let Some(atom) = self.first_boolean_atom(function, remaining) {
                        return Some(atom);
                    }
                    self.first_boolean_atom(argument, remaining)
                }
                Term::Id { ty, left, right } => {
                    if let Some(atom) = self.first_boolean_atom(left, remaining) {
                        return Some(atom);
                    }
                    if let Some(atom) = self.first_boolean_atom(right, remaining) {
                        return Some(atom);
                    }
                    self.first_boolean_atom(ty, remaining)
                }
                Term::Pi { domain, codomain }
                | Term::Sigma { domain, codomain }
                | Term::Lambda {
                    domain,
                    body: codomain,
                } => {
                    if let Some(atom) = self.first_boolean_atom(domain, remaining) {
                        return Some(atom);
                    }
                    self.first_boolean_atom(codomain, remaining)
                }
                Term::CaseTwo {
                    motive,
                    zero_branch,
                    one_branch,
                    scrutinee,
                } => {
                    if let Some(atom) = self.first_boolean_atom(scrutinee, remaining) {
                        return Some(atom);
                    }
                    if let Some(atom) = self.first_boolean_atom(zero_branch, remaining) {
                        return Some(atom);
                    }
                    if let Some(atom) = self.first_boolean_atom(one_branch, remaining) {
                        return Some(atom);
                    }
                    self.first_boolean_atom(motive, remaining)
                }
                _ => None,
            }
        }

        fn neutral_atom_is_two(&self, mut atom: TermHandle) -> bool {
            let mut applications = 0u32;
            let head = loop {
                match self.arena.get(atom) {
                    Term::Apply { function, .. } => {
                        let Some(next) = applications.checked_add(1) else {
                            return false;
                        };
                        applications = next;
                        atom = function;
                    }
                    Term::Constant { declaration, .. } => break declaration,
                    _ => return false,
                }
            };
            let Some(declaration) = self
                .declarations
                .get(usize::try_from(head).unwrap_or(usize::MAX))
            else {
                return false;
            };
            if !declaration.is_assumption() {
                return false;
            }
            let mut ty = declaration.ty;
            for _ in 0..applications {
                let Term::Pi { codomain, .. } = self.arena.get(ty) else {
                    return false;
                };
                ty = codomain;
            }
            self.arena.structurally_equal(ty, self.two)
        }

        pub(super) fn two_identity_discriminant(
            &mut self,
            premise_left: TermHandle,
            premise_right: TermHandle,
            at_right: TermHandle,
            premise_left_literal: bool,
            proof: TermHandle,
        ) -> TermHandle {
            let shifted_premise_left = shift(&mut self.arena, premise_left, 0, 1);
            let bound = self.arena.insert(Term::Variable(0));
            let proof_domain = self
                .arena
                .insert(id!(self.two => shifted_premise_left => bound));
            let shifted_left = shift(&mut self.arena, premise_left, 0, 2);
            let shifted_at_right = shift(&mut self.arena, at_right, 0, 2);
            let self_identity = self
                .arena
                .insert(id!(self.two => shifted_left => shifted_left));
            let discriminant_motive = self.arena.insert(Term::Lambda {
                domain: self.two,
                body: self.type_zero,
            });
            let scrutinee = self.arena.insert(Term::Variable(1));
            let (zero_branch, one_branch) = if premise_left_literal {
                (shifted_at_right, self_identity)
            } else {
                (self_identity, shifted_at_right)
            };
            let discriminant = self.arena.insert(Term::CaseTwo {
                motive: discriminant_motive,
                zero_branch,
                one_branch,
                scrutinee,
            });
            let proof_lambda = self.arena.insert(Term::Lambda {
                domain: proof_domain,
                body: discriminant,
            });
            let motive = self.arena.insert(Term::Lambda {
                domain: self.two,
                body: proof_lambda,
            });
            let base = self.arena.insert(Term::Refl {
                ty: self.two,
                value: premise_left,
            });
            self.arena.insert(Term::IdElim {
                motive,
                base,
                endpoint: premise_right,
                proof,
            })
        }

        fn boolean_eval(&mut self, mut term: TermHandle, remaining: &mut u64) -> BooleanEval {
            loop {
                let Some(rest) = remaining.checked_sub(1) else {
                    return BooleanEval::Unsupported;
                };
                *remaining = rest;
                match self.arena.get(term) {
                    Term::TwoZero => return BooleanEval::Literal(false),
                    Term::TwoOne => return BooleanEval::Literal(true),
                    Term::Constant { declaration, .. } => {
                        let neutral = self
                            .declarations
                            .get(usize::try_from(declaration).unwrap_or(usize::MAX))
                            .is_some_and(|declaration| {
                                declaration.is_assumption()
                                    && self.arena.structurally_equal(declaration.ty, self.two)
                            });
                        return if neutral {
                            BooleanEval::Blocked(term)
                        } else {
                            BooleanEval::Unsupported
                        };
                    }
                    Term::CaseTwo {
                        zero_branch,
                        one_branch,
                        scrutinee,
                        ..
                    } => match self.boolean_eval(scrutinee, remaining) {
                        BooleanEval::Literal(false) => term = zero_branch,
                        BooleanEval::Literal(true) => term = one_branch,
                        other => return other,
                    },
                    Term::Apply { function, argument } => {
                        match self.boolean_function_head(function, remaining) {
                            BooleanHead::Lambda(body) => {
                                term = substitute(&mut self.arena, body, argument);
                            }
                            BooleanHead::Neutral => return BooleanEval::Blocked(term),
                            BooleanHead::Blocked(atom) => return BooleanEval::Blocked(atom),
                            BooleanHead::Unsupported => return BooleanEval::Unsupported,
                        }
                    }
                    _ => return BooleanEval::Unsupported,
                }
            }
        }

        fn boolean_function_head(
            &mut self,
            mut function: TermHandle,
            remaining: &mut u64,
        ) -> BooleanHead {
            loop {
                let Some(rest) = remaining.checked_sub(1) else {
                    return BooleanHead::Unsupported;
                };
                *remaining = rest;
                match self.arena.get(function) {
                    Term::Lambda { domain, body } => {
                        return if self.arena.structurally_equal(domain, self.two) {
                            BooleanHead::Lambda(body)
                        } else {
                            BooleanHead::Unsupported
                        };
                    }
                    Term::Apply {
                        function: inner,
                        argument,
                    } => match self.boolean_function_head(inner, remaining) {
                        BooleanHead::Lambda(body) => {
                            function = substitute(&mut self.arena, body, argument);
                        }
                        other => return other,
                    },
                    Term::Constant { declaration, .. } => {
                        return if self
                            .declarations
                            .get(usize::try_from(declaration).unwrap_or(usize::MAX))
                            .is_some_and(|declaration| declaration.is_assumption())
                        {
                            BooleanHead::Neutral
                        } else {
                            BooleanHead::Unsupported
                        };
                    }
                    Term::CaseTwo {
                        zero_branch,
                        one_branch,
                        scrutinee,
                        ..
                    } => match self.boolean_eval(scrutinee, remaining) {
                        BooleanEval::Literal(false) => function = zero_branch,
                        BooleanEval::Literal(true) => function = one_branch,
                        BooleanEval::Blocked(atom) => return BooleanHead::Blocked(atom),
                        BooleanEval::Unsupported => return BooleanHead::Unsupported,
                    },
                    _ => return BooleanHead::Unsupported,
                }
            }
        }

        fn abstract_atom(
            &mut self,
            term: TermHandle,
            atom: TermHandle,
            depth: u32,
            remaining: &mut u64,
        ) -> Option<TermHandle> {
            self.rewrite_atom(term, atom, depth, AtomRewrite::Binder, remaining)
        }

        fn instantiate_atom(
            &mut self,
            term: TermHandle,
            atom: TermHandle,
            literal: TermHandle,
            remaining: &mut u64,
        ) -> Option<TermHandle> {
            self.rewrite_atom(term, atom, 0, AtomRewrite::Literal(literal), remaining)
        }

        fn rewrite_atom(
            &mut self,
            term: TermHandle,
            atom: TermHandle,
            depth: u32,
            rewrite: AtomRewrite,
            remaining: &mut u64,
        ) -> Option<TermHandle> {
            *remaining = remaining.checked_sub(1)?;
            if self.arena.structurally_equal(term, atom) {
                let inserted = match rewrite {
                    AtomRewrite::Binder => self.arena.insert(Term::Variable(depth)),
                    AtomRewrite::Literal(literal) => literal,
                };
                return Some(inserted);
            }
            match self.arena.get(term) {
                Term::Variable(_)
                | Term::Constant { .. }
                | Term::Sort(_)
                | Term::Two
                | Term::TwoZero
                | Term::TwoOne => Some(term),
                Term::Apply { function, argument } => {
                    let function = self.rewrite_atom(function, atom, depth, rewrite, remaining)?;
                    let argument = self.rewrite_atom(argument, atom, depth, rewrite, remaining)?;
                    Some(self.arena.insert(Term::Apply { function, argument }))
                }
                Term::Id { ty, left, right } => {
                    let ty = self.rewrite_atom(ty, atom, depth, rewrite, remaining)?;
                    let left = self.rewrite_atom(left, atom, depth, rewrite, remaining)?;
                    let right = self.rewrite_atom(right, atom, depth, rewrite, remaining)?;
                    Some(self.arena.insert(Term::Id { ty, left, right }))
                }
                Term::Pi { domain, codomain } => {
                    let domain = self.rewrite_atom(domain, atom, depth, rewrite, remaining)?;
                    let codomain =
                        self.rewrite_atom(codomain, atom, depth + 1, rewrite, remaining)?;
                    Some(self.arena.insert(Term::Pi { domain, codomain }))
                }
                Term::Sigma { domain, codomain } => {
                    let domain = self.rewrite_atom(domain, atom, depth, rewrite, remaining)?;
                    let codomain =
                        self.rewrite_atom(codomain, atom, depth + 1, rewrite, remaining)?;
                    Some(self.arena.insert(Term::Sigma { domain, codomain }))
                }
                Term::Lambda { domain, body } => {
                    let domain = self.rewrite_atom(domain, atom, depth, rewrite, remaining)?;
                    let body = self.rewrite_atom(body, atom, depth + 1, rewrite, remaining)?;
                    Some(self.arena.insert(Term::Lambda { domain, body }))
                }
                Term::CaseTwo {
                    motive,
                    zero_branch,
                    one_branch,
                    scrutinee,
                } => {
                    let motive = self.rewrite_atom(motive, atom, depth, rewrite, remaining)?;
                    let zero_branch =
                        self.rewrite_atom(zero_branch, atom, depth, rewrite, remaining)?;
                    let one_branch =
                        self.rewrite_atom(one_branch, atom, depth, rewrite, remaining)?;
                    let scrutinee =
                        self.rewrite_atom(scrutinee, atom, depth, rewrite, remaining)?;
                    Some(self.arena.insert(Term::CaseTwo {
                        motive,
                        zero_branch,
                        one_branch,
                        scrutinee,
                    }))
                }
                _ => None,
            }
        }
    }

    #[derive(Clone, Copy)]
    enum BooleanEval {
        Literal(bool),
        Blocked(TermHandle),
        Unsupported,
    }

    enum BooleanHead {
        Lambda(TermHandle),
        Neutral,
        Blocked(TermHandle),
        Unsupported,
    }

    #[derive(Clone, Copy)]
    enum AtomRewrite {
        Binder,
        Literal(TermHandle),
    }

    #[cfg(test)]
    mod tests {
        use super::super::test_utilities::denote_via_assumption_premise;
        use super::super::verify_bounded_certificate;
        use super::{Denotation, ScalarTerm, Term};
        use crate::proof::assumption_proof;
        use crate::{
            Budget, MathematicalCertificate, ProofNode, ProofRule, certificate_assumption_closure,
            verify_mathematical_certificate,
        };
        use semantic_vocabulary::{
            IntegerSign, IntegerType, Proposition, PropositionContext, PropositionId, ScalarType,
            ValueId,
        };

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
            let expected_type = denotation
                .arena
                .insert(id!(denotation.two => value => expected_value));
            let term = denotation.arena.insert(Term::Refl {
                ty: denotation.two,
                value,
            });
            let mut certificate = MathematicalCertificate::with_signature(
                denotation.declarations,
                Vec::new(),
                term,
                expected_type,
            );
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
            certificate.expected = denotation
                .arena
                .insert(id!(denotation.two => value => wrong));
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
            let expected = denotation
                .arena
                .insert(id!(denotation.two => value => value));
            let certificate = MathematicalCertificate::with_signature(
                denotation.declarations,
                Vec::new(),
                term,
                expected,
            );
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

        fn boolean(id: u64) -> (ValueId, ScalarTerm) {
            let id = ValueId::new(id).expect("value id");
            (id, ScalarTerm::value(id, ScalarType::Boolean))
        }

        #[test]
        fn boolean_double_negation_identity_denotes_by_case_analysis() {
            let (b_id, b) = boolean(1);
            let context = PropositionContext::from_value_types([(b_id, ScalarType::Boolean)])
                .expect("context");
            let premise = Proposition::Equal(b.clone(), ScalarTerm::Boolean(true));
            let goal = Proposition::Equal(
                ScalarTerm::boolean_not(ScalarTerm::boolean_not(b).expect("inner not"))
                    .expect("outer not"),
                ScalarTerm::Boolean(true),
            );
            let denoted = denote_via_assumption_premise(
                &context,
                &goal,
                &premise,
                "the Boolean identity verifies through case analysis",
            );
            assert_eq!(denoted.certificate.signature.len(), 1);
            let Term::Apply { function, argument } = denoted.arena.get(denoted.certificate.term)
            else {
                panic!("the transport applies its case-analysis function to the premise");
            };
            assert_eq!(denoted.arena.get(argument), Term::Variable(0));
            assert!(matches!(denoted.arena.get(function), Term::CaseTwo { .. }));
        }

        #[test]
        fn boolean_equal_identity_denotes_by_nested_case_analysis() {
            let (b_id, b) = boolean(1);
            let (c_id, c) = boolean(2);
            let context = PropositionContext::from_value_types([
                (b_id, ScalarType::Boolean),
                (c_id, ScalarType::Boolean),
            ])
            .expect("context");
            let premise = Proposition::Equal(b.clone(), c.clone());
            let goal = Proposition::Equal(
                ScalarTerm::boolean_equal(b, c).expect("boolean equal"),
                ScalarTerm::Boolean(true),
            );
            let denoted = denote_via_assumption_premise(
                &context,
                &goal,
                &premise,
                "the two-atom Boolean identity verifies through case analysis",
            );
            assert_eq!(denoted.certificate.signature.len(), 2);
            let Term::Apply { function, .. } = denoted.arena.get(denoted.certificate.term) else {
                panic!("the transport applies its case-analysis function to the premise");
            };
            assert!(matches!(denoted.arena.get(function), Term::CaseTwo { .. }));
        }

        #[test]
        fn crossed_boolean_negations_denoate_by_nested_case_analysis() {
            let (a_id, a) = boolean(1);
            let (b_id, b) = boolean(2);
            let context = PropositionContext::from_value_types([
                (a_id, ScalarType::Boolean),
                (b_id, ScalarType::Boolean),
            ])
            .expect("context");
            let premise = Proposition::Equal(
                ScalarTerm::boolean_not(a.clone()).expect("not a"),
                b.clone(),
            );
            let goal = Proposition::Equal(a, ScalarTerm::boolean_not(b).expect("not b"));
            let denoted = denote_via_assumption_premise(
                &context,
                &goal,
                &premise,
                "crossed negations verify through nested case analysis",
            );
            assert_eq!(denoted.certificate.signature.len(), 2);
            let Term::Apply { function, .. } = denoted.arena.get(denoted.certificate.term) else {
                panic!("the transport applies its case-analysis function to the premise");
            };
            assert!(matches!(denoted.arena.get(function), Term::CaseTwo { .. }));
        }

        #[test]
        fn boolean_identity_nested_in_implication_domain_denotes_by_case_analysis() {
            let (b_id, b) = boolean(1);
            let context = PropositionContext::from_value_types([(b_id, ScalarType::Boolean)])
                .expect("context");
            let atom = Proposition::Atom(PropositionId::new(1).expect("atom"));
            let premise = Proposition::Implication {
                premise: Box::new(Proposition::Equal(
                    ScalarTerm::boolean_not(ScalarTerm::boolean_not(b.clone()).expect("inner not"))
                        .expect("outer not"),
                    ScalarTerm::Boolean(true),
                )),
                conclusion: Box::new(atom.clone()),
            };
            let goal = Proposition::Implication {
                premise: Box::new(Proposition::Equal(b, ScalarTerm::Boolean(true))),
                conclusion: Box::new(atom),
            };
            let denoted = denote_via_assumption_premise(
                &context,
                &goal,
                &premise,
                "the nested Boolean identity verifies through case analysis",
            );
            assert_eq!(denoted.certificate.signature.len(), 2);
            assert!(matches!(
                denoted.arena.get(denoted.certificate.term),
                Term::Lambda { .. }
            ));
        }

        #[test]
        fn value_equality_transport_through_boolean_endpoints_denotes_by_case_analysis() {
            let (x_id, x) = boolean(1);
            let (y_id, y) = boolean(2);
            let context = PropositionContext::from_value_types([
                (x_id, ScalarType::Boolean),
                (y_id, ScalarType::Boolean),
            ])
            .expect("context");
            let premise = Proposition::Equal(x.clone(), y.clone());
            let equation = Proposition::Equal(x.clone(), ScalarTerm::Boolean(false));
            let goal = Proposition::Equal(
                ScalarTerm::boolean_not(x).expect("not x"),
                ScalarTerm::boolean_not(y).expect("not y"),
            );
            let proof = ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::ValueEqualityTransport {
                    premise: Box::new(assumption_proof(premise.clone(), 0)),
                    equalities: vec![assumption_proof(equation.clone(), 1)],
                },
            };
            let denoted = verify_bounded_certificate(
                &context,
                &goal,
                &[premise, equation],
                &[],
                &proof,
                &mut Budget::default(),
            )
            .expect("transport through Boolean endpoints verifies through J and case analysis");
            assert_eq!(denoted.certificate.signature.len(), 2);
        }

        #[test]
        fn boolean_identity_transport_declines_a_non_identity() {
            let mut denotation = Denotation::new();
            let b = denotation
                .scalar_term(&ScalarTerm::value(
                    ValueId::new(1).unwrap(),
                    ScalarType::Boolean,
                ))
                .unwrap();
            let not_b = denotation
                .scalar_term(
                    &ScalarTerm::boolean_not(ScalarTerm::value(
                        ValueId::new(1).unwrap(),
                        ScalarType::Boolean,
                    ))
                    .unwrap(),
                )
                .unwrap();
            let one = denotation.two_one;
            let premise = denotation.arena.insert(id!(denotation.two => b => one));
            let goal = denotation.arena.insert(id!(denotation.two => not_b => one));
            let evidence = denotation.arena.insert(Term::Variable(0));
            assert_eq!(
                denotation
                    .boolean_identity_transport(premise, goal, evidence)
                    .expect("the declined transport reports no error"),
                None,
                "a premise-true, goal-false valuation must decline"
            );
            let premise = denotation
                .arena
                .insert(id!(denotation.two => b => denotation.two_zero));
            let term = denotation
                .boolean_identity_transport(premise, goal, evidence)
                .expect("the transport runs")
                .expect("the contrapositive identity is a case-analysis derivation");
            let certificate = MathematicalCertificate::with_signature(
                denotation.declarations.clone(),
                vec![premise],
                term,
                goal,
            );
            verify_mathematical_certificate(
                &mut denotation.arena,
                &certificate,
                &mut Budget::default(),
            )
            .expect("the case-analysis derivation is a kernel judgment");
        }

        #[test]
        fn boolean_goal_transport_declines_an_open_leaf_the_premise_does_not_cover() {
            let mut denotation = Denotation::new();
            let a = denotation
                .scalar_term(&ScalarTerm::value(
                    ValueId::new(1).unwrap(),
                    ScalarType::Boolean,
                ))
                .unwrap();
            let x = denotation
                .scalar_term(&ScalarTerm::value(
                    ValueId::new(2).unwrap(),
                    ScalarType::Boolean,
                ))
                .unwrap();
            let premise = denotation.arena.insert(id!(denotation.two => a => a));
            let failing_goal = denotation
                .arena
                .insert(id!(denotation.two => x => denotation.two_zero));
            let evidence = denotation.arena.insert(Term::Variable(0));
            assert!(
                denotation
                    .boolean_goal_transport(premise, failing_goal, evidence)
                    .expect("the declined transport reports no error")
                    .is_none(),
                "a valuation holding the premise but not the goal must decline"
            );
            let reflexive_goal = denotation.arena.insert(id!(denotation.two => x => x));
            let term = denotation
                .boolean_goal_transport(premise, reflexive_goal, evidence)
                .expect("the transport runs")
                .expect("an always-inhabited goal derives");
            let certificate = MathematicalCertificate::with_signature(
                denotation.declarations.clone(),
                vec![premise],
                term,
                reflexive_goal,
            );
            verify_mathematical_certificate(
                &mut denotation.arena,
                &certificate,
                &mut Budget::default(),
            )
            .expect("the goal-side case split is a kernel judgment");
        }

        #[test]
        fn boolean_goal_transport_skips_non_boolean_neutrals() {
            let mut denotation = Denotation::new();
            let a = denotation
                .scalar_term(&ScalarTerm::value(
                    ValueId::new(1).unwrap(),
                    ScalarType::Boolean,
                ))
                .unwrap();
            let less_than = denotation.integer_less_than().unwrap();
            let less_than = denotation.constant(less_than);
            let x = denotation
                .fixed_scalar_term(&ScalarTerm::value(
                    ValueId::new(2).unwrap(),
                    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
                ))
                .unwrap();
            let y = denotation
                .fixed_scalar_term(&ScalarTerm::value(
                    ValueId::new(3).unwrap(),
                    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
                ))
                .unwrap();
            let applied = denotation.arena.insert(Term::Apply {
                function: less_than,
                argument: x,
            });
            let relation = denotation.arena.insert(Term::Apply {
                function: applied,
                argument: y,
            });
            let identity = denotation.arena.insert(id!(denotation.two => a => a));
            let premise = denotation.arena.insert(id!(denotation.two => a => a));
            let type_motive = denotation.arena.insert(Term::Lambda {
                domain: denotation.two,
                body: denotation.type_zero,
            });
            let tag = denotation.arena.insert(Term::Variable(0));
            let family = denotation.arena.insert(Term::CaseTwo {
                motive: type_motive,
                zero_branch: relation,
                one_branch: identity,
                scrutinee: tag,
            });
            let goal = denotation.arena.insert(Term::Sigma {
                domain: denotation.two,
                codomain: family,
            });
            let evidence = denotation.arena.insert(Term::Variable(0));
            let term = denotation
                .boolean_goal_transport(premise, goal, evidence)
                .expect("the transport runs")
                .expect("the disjunction's second branch inhabits at each `a` valuation");
            let certificate = MathematicalCertificate::with_signature(
                denotation.declarations.clone(),
                vec![premise],
                term,
                goal,
            );
            verify_mathematical_certificate(
                &mut denotation.arena,
                &certificate,
                &mut Budget::default(),
            )
            .expect("the derivation never splits the `IntLt` spine");
        }
    }
}
mod casts {
    use semantic_vocabulary::{Proposition, ScalarTerm};

    use crate::CheckedIntegerCastChain;
    use crate::integer_rules::fixed_integer::fixed_native_interval;

    use super::integer_operations::IntegerOperation;
    use super::{BoundedDenotationError, Denotation, Term, TermHandle};

    struct CastEdge {
        operation: IntegerOperation,
        previous: TermHandle,
        next: TermHandle,
        application: TermHandle,
        definition: TermHandle,
    }

    impl Denotation {
        fn cast_identity_law(
            &mut self,
            operation: IntegerOperation,
        ) -> Result<u32, BoundedDenotationError> {
            self.interned_keyed_assumption(
                |this| &mut this.cast_identities,
                operation,
                |this| {
                    let integer = this.integer_constant()?;
                    let operation_position = this.integer_operation(operation)?;
                    let function = this.constant(operation_position);
                    let operand = this.arena.insert(Term::Variable(0));
                    let applied = this.arena.insert(Term::Apply {
                        function,
                        argument: operand,
                    });
                    let identity = this.arena.insert(Term::Id {
                        ty: integer,
                        left: applied,
                        right: operand,
                    });
                    Ok(this.arena.insert(Term::Pi {
                        domain: integer,
                        codomain: identity,
                    }))
                },
            )
        }

        fn cast_identity_application(
            &mut self,
            operation: IntegerOperation,
            operand: TermHandle,
        ) -> Result<TermHandle, BoundedDenotationError> {
            let position = self.cast_identity_law(operation)?;
            let law = self.constant(position);
            Ok(self.arena.insert(Term::Apply {
                function: law,
                argument: operand,
            }))
        }

        fn cast_order_term(
            &mut self,
            left: TermHandle,
            right: TermHandle,
            strict: bool,
        ) -> Result<TermHandle, BoundedDenotationError> {
            self.order_term_application(strict, left, right)
        }

        fn cast_edge_definition(
            proposition: &Proposition,
        ) -> Option<(&ScalarTerm, &ScalarTerm, IntegerOperation)> {
            let (next, definition) = proposition.as_equal()?;
            let operation = match definition {
                ScalarTerm::IntegerWiden {
                    source_type,
                    target_type,
                    ..
                } => IntegerOperation::Widen {
                    source: *source_type,
                    target: *target_type,
                },
                ScalarTerm::IntegerExactCast {
                    source_type,
                    target_type,
                    ..
                } => IntegerOperation::ExactCast {
                    source: *source_type,
                    target: *target_type,
                },
                _ => return None,
            };
            Some((next, definition, operation))
        }

        pub(super) fn cast_bound_evidence(
            &mut self,
            root_bound: &Proposition,
            root_evidence: TermHandle,
            chain: &CheckedIntegerCastChain,
            axiom_evidence: &[TermHandle],
            semantic_axioms: &[Proposition],
            conclusion: &Proposition,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            let goal = self.denote(conclusion)?;
            let (strict, goal_left, goal_right) = match self.integer_relation(goal) {
                Some(super::IntegerRelation::LessThan { left, right }) => (true, left, right),
                Some(super::IntegerRelation::LessOrEqual { left, right }) => (false, left, right),
                _ => return Ok(None),
            };

            let mut edges = Vec::with_capacity(chain.definition_axioms().len());
            let mut values = Vec::with_capacity(chain.definition_axioms().len() + 1);
            values.push(chain.root().clone());
            let mut previous = self.fixed_scalar_term(chain.root())?;
            for (&index, &axiom) in chain.definition_axioms().iter().zip(axiom_evidence.iter()) {
                let Some(proposition) = semantic_axioms.get(index) else {
                    return Ok(None);
                };
                let Some((next, definition, operation)) = Self::cast_edge_definition(proposition)
                else {
                    return Ok(None);
                };
                let application = self.fixed_scalar_term(definition)?;
                let next_denoted = self.fixed_scalar_term(next)?;
                edges.push(CastEdge {
                    operation,
                    previous,
                    next: next_denoted,
                    application,
                    definition: axiom,
                });
                previous = next_denoted;
                values.push(next.clone());
            }
            let target = self.fixed_scalar_term(chain.target())?;

            let step_equality =
                |this: &mut Self,
                 edge: &CastEdge|
                 -> Result<(TermHandle, TermHandle), BoundedDenotationError> {
                    let integer = this.integer_constant()?;
                    let identity = this.cast_identity_application(edge.operation, edge.previous)?;
                    let forward = this.transitivity(
                        integer,
                        edge.next,
                        edge.application,
                        edge.previous,
                        edge.definition,
                        identity,
                    );
                    let ty = this.arena.insert(Term::Id {
                        ty: integer,
                        left: edge.next,
                        right: edge.previous,
                    });
                    Ok((ty, forward))
                };

            let (mut bound, mut bound_ty, endpoint, moving_endpoint, start) = if *root_bound
                == Proposition::Truth
            {
                if strict {
                    return Ok(None);
                }
                let lower = self.arena.structurally_equal(goal_right, target);
                let upper = self.arena.structurally_equal(goal_left, target);
                if lower == upper {
                    return Ok(None);
                }
                if values.len() != chain.carriers().len() {
                    return Ok(None);
                }
                let mut selected: Option<usize> = None;
                let mut selected_bound = 0i128;
                for (index, carrier) in chain.carriers().iter().enumerate() {
                    let Some((minimum, maximum)) = fixed_native_interval(*carrier) else {
                        return Ok(None);
                    };
                    let candidate = if lower { minimum } else { maximum };
                    let better = match selected {
                        None => true,
                        Some(_) => {
                            if lower {
                                candidate > selected_bound
                            } else {
                                candidate < selected_bound
                            }
                        }
                    };
                    if better {
                        selected = Some(index);
                        selected_bound = candidate;
                    }
                }
                let Some(origin) = selected else {
                    return Ok(None);
                };
                let (endpoint_term, evidence) =
                    self.carrier_bound(&values[origin], lower, &chain.carriers()[origin])?;
                let endpoint = self.fixed_scalar_term(&endpoint_term)?;
                let value_term = self.fixed_scalar_term(&values[origin])?;
                let bound_ty = if lower {
                    self.cast_order_term(endpoint, value_term, false)?
                } else {
                    self.cast_order_term(value_term, endpoint, false)?
                };
                (evidence, bound_ty, endpoint, usize::from(lower), origin)
            } else {
                let root_ty = self.denote(root_bound)?;
                let (left, right) = match self.integer_relation(root_ty) {
                    Some(super::IntegerRelation::LessThan { left, right }) if strict => {
                        (left, right)
                    }
                    Some(super::IntegerRelation::LessOrEqual { left, right }) if !strict => {
                        (left, right)
                    }
                    _ => return Ok(None),
                };
                let root = self.fixed_scalar_term(chain.root())?;
                let (endpoint, moving_endpoint) = if self.arena.structurally_equal(right, root) {
                    (left, 1usize)
                } else if self.arena.structurally_equal(left, root) {
                    (right, 0usize)
                } else {
                    return Ok(None);
                };
                (root_evidence, root_ty, endpoint, moving_endpoint, 0)
            };

            for edge in &edges[start..] {
                let (equality_ty, equality) = step_equality(self, edge)?;
                let step_goal = if moving_endpoint == 1 {
                    self.cast_order_term(endpoint, edge.next, strict)?
                } else {
                    self.cast_order_term(edge.next, endpoint, strict)?
                };
                let Some(next_bound) = self.order_substitution_evidence(
                    bound_ty,
                    bound,
                    equality_ty,
                    equality,
                    moving_endpoint,
                    step_goal,
                )?
                else {
                    return Ok(None);
                };
                bound = next_bound;
                bound_ty = step_goal;
            }

            if !self.arena.structurally_equal(bound_ty, goal) {
                return Ok(None);
            }
            Ok(Some(bound))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::super::test_utilities::{assert_refused, elaborate_checked};
        use crate::proof::assumption_proof;
        use crate::{
            Budget, IntegerCastChainWitness, PrimitiveJudgment, ProofNode, ProofRule, proof_node,
            verify_bounded_certificate,
        };
        use semantic_vocabulary::{
            IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
            ScalarType, ValueId,
        };
        use std::collections::BTreeSet;
        use std::slice::from_ref;

        fn integer(sign: IntegerSign, bits: u16) -> IntegerType {
            IntegerType::new(sign, bits).expect("fixed integer type")
        }

        fn value(id: u64, integer: IntegerType) -> ScalarTerm {
            ScalarTerm::value(
                ValueId::new(id).expect("value id"),
                ScalarType::Integer(integer),
            )
        }

        fn context_for(roots: &[(u64, IntegerType)]) -> PropositionContext {
            PropositionContext::from_value_types(
                roots
                    .iter()
                    .map(|(id, ty)| (ValueId::new(*id).unwrap(), ScalarType::Integer(*ty))),
            )
            .expect("context")
        }

        fn denotation_counts(
            context: &PropositionContext,
            goal: &Proposition,
            assumptions: &[Proposition],
            axioms: &[Proposition],
            proof: &ProofNode,
        ) -> (usize, usize) {
            let parameters = BTreeSet::new();
            let elaboration =
                elaborate_checked(context, goal, assumptions, axioms, &parameters, proof);
            (
                elaboration.denotation.rule_axioms.len(),
                elaboration.denotation.cast_identities.len(),
            )
        }

        #[test]
        fn cast_bound_transports_through_one_exact_cast_edge() {
            let i16 = integer(IntegerSign::Signed, 16);
            let i8 = integer(IntegerSign::Signed, 8);
            let root = value(1, i16);
            let target = value(2, i8);
            let context = context_for(&[(1, i16), (2, i8)]);
            let root_bound = Proposition::LessOrEqual(
                ScalarTerm::integer(i16, IntegerValue::Signed(1)).unwrap(),
                root.clone(),
            );
            let definition = Proposition::Equal(
                target.clone(),
                ScalarTerm::integer_exact_cast(i16, i8, root.clone()).unwrap(),
            );
            let conclusion = Proposition::LessOrEqual(
                ScalarTerm::integer(i8, IntegerValue::Signed(1)).unwrap(),
                target.clone(),
            );
            let proof = ProofNode {
                conclusion: conclusion.clone(),
                rule: ProofRule::IntegerCastBound {
                    root_bound: Box::new(assumption_proof(root_bound.clone(), 0)),
                    witness: IntegerCastChainWitness {
                        root: root.clone(),
                        target: target.clone(),
                        definition_axioms: vec![0],
                    },
                },
            };
            verify_bounded_certificate(
                &context,
                &conclusion,
                from_ref(&root_bound),
                from_ref(&definition),
                &proof,
                &mut Budget::default(),
            )
            .expect("the transported bound verifies in the kernel");
            let (rule_axioms, cast_identities) = denotation_counts(
                &context,
                &conclusion,
                from_ref(&root_bound),
                from_ref(&definition),
                &proof,
            );
            assert_eq!(rule_axioms, 0);
            assert_eq!(cast_identities, 1);
        }

        #[test]
        fn cast_bound_walks_a_multi_edge_word_in_either_direction() {
            let i16 = integer(IntegerSign::Signed, 16);
            let u16 = integer(IntegerSign::Unsigned, 16);
            let i8 = integer(IntegerSign::Signed, 8);
            let root = value(1, i16);
            let middle = value(3, u16);
            let target = value(2, i8);
            let context = context_for(&[(1, i16), (2, i8), (3, u16)]);
            let first_definition = Proposition::Equal(
                middle.clone(),
                ScalarTerm::integer_exact_cast(i16, u16, root.clone()).unwrap(),
            );
            let second_definition = Proposition::Equal(
                target.clone(),
                ScalarTerm::integer_exact_cast(u16, i8, middle.clone()).unwrap(),
            );
            let axioms = [first_definition, second_definition];
            for root_is_left in [false, true] {
                let literal = |ty, v| ScalarTerm::integer(ty, IntegerValue::Signed(v)).unwrap();
                let root_bound = if root_is_left {
                    Proposition::LessOrEqual(root.clone(), literal(i16, 42))
                } else {
                    Proposition::LessOrEqual(literal(i16, 7), root.clone())
                };
                let conclusion = if root_is_left {
                    Proposition::LessOrEqual(target.clone(), literal(i8, 42))
                } else {
                    Proposition::LessOrEqual(literal(i8, 7), target.clone())
                };
                let proof = ProofNode {
                    conclusion: conclusion.clone(),
                    rule: ProofRule::IntegerCastBound {
                        root_bound: Box::new(assumption_proof(root_bound.clone(), 0)),
                        witness: IntegerCastChainWitness {
                            root: root.clone(),
                            target: target.clone(),
                            definition_axioms: vec![0, 1],
                        },
                    },
                };
                verify_bounded_certificate(
                    &context,
                    &conclusion,
                    from_ref(&root_bound),
                    &axioms,
                    &proof,
                    &mut Budget::default(),
                )
                .expect("the chained transport verifies in the kernel");
                let (rule_axioms, cast_identities) = denotation_counts(
                    &context,
                    &conclusion,
                    from_ref(&root_bound),
                    &axioms,
                    &proof,
                );
                assert_eq!(rule_axioms, 0);
                assert_eq!(cast_identities, 2);
            }
        }

        #[test]
        fn truth_root_cast_bound_uses_the_tightest_carrier_membership() {
            let u8 = integer(IntegerSign::Unsigned, 8);
            let u64 = integer(IntegerSign::Unsigned, 64);
            let root = value(1, u8);
            let target = value(2, u64);
            let context = context_for(&[(1, u8), (2, u64)]);
            let definition = Proposition::Equal(
                target.clone(),
                ScalarTerm::integer_widen(u8, u64, root.clone()).unwrap(),
            );
            for (bound, maximum) in [(true, 0u128), (false, 255)] {
                let endpoint = ScalarTerm::integer(u64, IntegerValue::Unsigned(maximum)).unwrap();
                let conclusion = if bound {
                    Proposition::LessOrEqual(endpoint, target.clone())
                } else {
                    Proposition::LessOrEqual(target.clone(), endpoint)
                };
                let proof = ProofNode {
                    conclusion: conclusion.clone(),
                    rule: ProofRule::IntegerCastBound {
                        root_bound: Box::new(
                            proof_node!(Proposition::Truth => ProofRule::Primitive(PrimitiveJudgment::Truth)),
                        ),
                        witness: IntegerCastChainWitness {
                            root: root.clone(),
                            target: target.clone(),
                            definition_axioms: vec![0],
                        },
                    },
                };
                verify_bounded_certificate(
                    &context,
                    &conclusion,
                    &[],
                    from_ref(&definition),
                    &proof,
                    &mut Budget::default(),
                )
                .expect("the carrier-membership bound transports to the target");
                let (rule_axioms, cast_identities) =
                    denotation_counts(&context, &conclusion, &[], from_ref(&definition), &proof);
                assert_eq!(rule_axioms, 0);
                assert_eq!(cast_identities, 1);
            }
        }

        #[test]
        fn live_cast_endpoint_preserves_strictness_and_derives_without_rule_axioms() {
            let narrow = integer(IntegerSign::Unsigned, 32);
            let wide = integer(IntegerSign::Unsigned, 64);
            let root = value(1, narrow);
            let target = value(2, wide);
            let other = value(3, wide);
            let context = context_for(&[(1, narrow), (2, wide), (3, wide), (4, wide)]);
            let axioms = [Proposition::Equal(
                target.clone(),
                ScalarTerm::integer_widen(narrow, wide, root.clone()).unwrap(),
            )];
            for strict in [false, true] {
                for root_is_left in [false, true] {
                    let relation = |moving: ScalarTerm, fixed: ScalarTerm, strict| {
                        let (left, right) = if root_is_left {
                            (moving, fixed)
                        } else {
                            (fixed, moving)
                        };
                        if strict {
                            Proposition::LessThan(left, right)
                        } else {
                            Proposition::LessOrEqual(left, right)
                        }
                    };
                    let assumption = crate::lift_fixed_integer_relation(&relation(
                        root.clone(),
                        other.clone(),
                        strict,
                    ))
                    .unwrap();
                    let goal = relation(target.clone(), other.clone(), strict);
                    let proof = ProofNode {
                        conclusion: goal.clone(),
                        rule: ProofRule::IntegerCastBound {
                            root_bound: Box::new(assumption_proof(assumption.clone(), 0)),
                            witness: IntegerCastChainWitness {
                                root: root.clone(),
                                target: target.clone(),
                                definition_axioms: vec![0],
                            },
                        },
                    };
                    let assumptions = [assumption];
                    verify_bounded_certificate(
                        &context,
                        &goal,
                        &assumptions,
                        &axioms,
                        &proof,
                        &mut Budget::default(),
                    )
                    .unwrap();
                    assert_eq!(
                        denotation_counts(&context, &goal, &assumptions, &axioms, &proof),
                        (0, 1)
                    );
                    for changed in [
                        relation(target.clone(), value(4, wide), strict),
                        relation(target.clone(), other.clone(), !strict),
                        relation(other.clone(), target.clone(), strict),
                    ] {
                        let mut tampered = proof.clone();
                        tampered.conclusion = changed.clone();
                        assert_refused(&context, &changed, &assumptions, &axioms, &tampered);
                    }
                    assert_refused(&context, &goal, &assumptions, &[], &proof);
                }
            }
        }
    }
}
mod denotation {
    use super::*;
    use crate::term_matchers::TermMatcher;
    use scheme_dsl::build;

    mod logical {
        use super::*;

        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        struct SyntaxKey(usize);

        #[derive(PartialEq, Eq, PartialOrd, Ord)]
        enum LogicalSyntax {
            Conjunction(Vec<SyntaxKey>),
            Disjunction(Vec<SyntaxKey>),
            Implication {
                premise: SyntaxKey,
                conclusion: SyntaxKey,
            },
        }

        #[derive(Default)]
        pub(crate) struct Cache {
            atoms: BTreeMap<Proposition, SyntaxKey>,
            logical: BTreeMap<LogicalSyntax, SyntaxKey>,
            terms: Vec<Option<TermHandle>>,
        }

        impl Cache {
            pub(crate) fn cached_atom(&self, proposition: &Proposition) -> Option<TermHandle> {
                self.atoms
                    .get(proposition)
                    .and_then(|key| self.terms[key.0])
            }

            fn new_key(&mut self) -> SyntaxKey {
                let key = SyntaxKey(self.terms.len());
                self.terms.push(None);
                key
            }

            fn intern_atom(&mut self, proposition: &Proposition) -> SyntaxKey {
                if let Some(&key) = self.atoms.get(proposition) {
                    return key;
                }
                let key = self.new_key();
                self.atoms.insert(proposition.clone(), key);
                key
            }

            fn intern_logical(&mut self, syntax: LogicalSyntax) -> SyntaxKey {
                if let Some(&key) = self.logical.get(&syntax) {
                    return key;
                }
                let key = self.new_key();
                self.logical.insert(syntax, key);
                key
            }

            fn get(&self, key: Option<SyntaxKey>) -> Option<TermHandle> {
                key.and_then(|key| self.terms[key.0])
            }

            fn publish(&mut self, key: Option<SyntaxKey>, term: TermHandle) {
                let key = key.expect("an invalid logical arity cannot finish denotation");
                let slot = &mut self.terms[key.0];
                debug_assert!(slot.is_none() || *slot == Some(term));
                *slot = Some(term);
            }
        }

        struct Node<'source> {
            proposition: &'source Proposition,
            key: Option<SyntaxKey>,
            children: Vec<usize>,
        }

        struct Prepared<'source> {
            nodes: Vec<Node<'source>>,
            root: usize,
        }

        enum Prepare<'source> {
            Enter(&'source Proposition),
            Close(&'source Proposition, usize),
        }

        impl<'source> Prepared<'source> {
            fn new(cache: &mut Cache, proposition: &'source Proposition) -> Self {
                let mut nodes: Vec<Node<'source>> = Vec::new();
                let mut finished: Vec<usize> = Vec::new();
                let mut pending = vec![Prepare::Enter(proposition)];
                while let Some(work) = pending.pop() {
                    let node = match work {
                        Prepare::Enter(proposition) => match proposition {
                            Proposition::Conjunction(parts) | Proposition::Disjunction(parts) => {
                                if parts.len() >= 2 {
                                    pending.push(Prepare::Close(proposition, parts.len()));
                                    pending.extend(parts.iter().rev().map(Prepare::Enter));
                                    continue;
                                }
                                Node {
                                    proposition,
                                    key: None,
                                    children: Vec::new(),
                                }
                            }
                            Proposition::Implication {
                                premise,
                                conclusion,
                            } => {
                                pending.push(Prepare::Close(proposition, 2));
                                pending.push(Prepare::Enter(conclusion));
                                pending.push(Prepare::Enter(premise));
                                continue;
                            }
                            _ => Node {
                                proposition,
                                key: Some(cache.intern_atom(proposition)),
                                children: Vec::new(),
                            },
                        },
                        Prepare::Close(proposition, count) => {
                            let children = finished.split_off(finished.len() - count);
                            let keys: Option<Vec<_>> =
                                children.iter().map(|&child| nodes[child].key).collect();
                            let key = keys.map(|keys| {
                                let syntax = match proposition {
                                    Proposition::Conjunction(_) => LogicalSyntax::Conjunction(keys),
                                    Proposition::Disjunction(_) => LogicalSyntax::Disjunction(keys),
                                    Proposition::Implication { .. } => LogicalSyntax::Implication {
                                        premise: keys[0],
                                        conclusion: keys[1],
                                    },
                                    _ => unreachable!("only logical nodes have pending children"),
                                };
                                cache.intern_logical(syntax)
                            });
                            Node {
                                proposition,
                                key,
                                children,
                            }
                        }
                    };
                    finished.push(nodes.len());
                    nodes.push(node);
                }
                assert_eq!(finished.len(), 1);
                Self {
                    nodes,
                    root: finished[0],
                }
            }
        }

        struct Pending {
            node: usize,
            next: usize,
            terms: Vec<TermHandle>,
        }

        pub(super) fn denote(
            denotation: &mut Denotation,
            proposition: &Proposition,
        ) -> Result<TermHandle, BoundedDenotationError> {
            if let Some(term) = denotation.denotations.cached_atom(proposition) {
                return Ok(term);
            }
            let prepared = Prepared::new(&mut denotation.denotations, proposition);
            let mut pending: Vec<Pending> = Vec::new();
            let mut current = prepared.root;
            'enter: loop {
                let node = &prepared.nodes[current];
                let mut value = if let Some(term) = denotation.denotations.get(node.key) {
                    term
                } else {
                    match node.proposition {
                        Proposition::Conjunction(parts) if parts.len() < 2 => {
                            return Err(BoundedDenotationError::Certificate(
                                ProofError::RuleConclusionMismatch("conjunction arity"),
                            ));
                        }
                        Proposition::Disjunction(parts) if parts.len() < 2 => {
                            return Err(BoundedDenotationError::Certificate(
                                ProofError::RuleConclusionMismatch("disjunction arity"),
                            ));
                        }
                        Proposition::Conjunction(_)
                        | Proposition::Disjunction(_)
                        | Proposition::Implication { .. } => {
                            let first = node.children[0];
                            pending.push(Pending {
                                node: current,
                                next: 1,
                                terms: Vec::with_capacity(node.children.len()),
                            });
                            current = first;
                            continue 'enter;
                        }
                        _ => {
                            let term = denotation.denote_leaf(node.proposition)?;
                            denotation.denotations.publish(node.key, term);
                            term
                        }
                    }
                };
                loop {
                    let Some(mut frame) = pending.pop() else {
                        return Ok(value);
                    };
                    frame.terms.push(value);
                    let node = &prepared.nodes[frame.node];
                    if frame.next < node.children.len() {
                        current = node.children[frame.next];
                        frame.next += 1;
                        pending.push(frame);
                        continue 'enter;
                    }
                    value = match node.proposition {
                        Proposition::Conjunction(_) => denotation.conjunction_terms(frame.terms),
                        Proposition::Disjunction(_) => denotation.disjunction_terms(frame.terms),
                        Proposition::Implication { .. } => denotation.arena.insert(Term::Pi {
                            domain: frame.terms[0],
                            codomain: frame.terms[1],
                        }),
                        _ => unreachable!("only logical nodes have pending term children"),
                    };
                    denotation.denotations.publish(node.key, value);
                }
            }
        }
    }

    pub(super) struct Denotation {
        pub(super) arena: TermArena,
        pub(super) type_zero: TermHandle,
        pub(super) two: TermHandle,
        pub(super) two_zero: TermHandle,
        pub(super) two_one: TermHandle,
        pub(super) boolean_equality: TermHandle,
        pub(super) declarations: Vec<Declaration>,
        pub(super) atoms: BTreeMap<Proposition, u32>,
        pub(super) carriers: BTreeMap<ScalarType, u32>,
        pub(super) terms: BTreeMap<ScalarTerm, u32>,
        /// Content algebra → its `Type 0` carrier application. Each
        /// algebra's conservation equations denote `Id` over their own
        /// carrier, so identities in different algebras can never compose.
        pub(super) content_carriers: BTreeMap<ContentAlgebra, TermHandle>,
        content_families: [Option<u32>; 2],
        content_parameters: BTreeMap<String, u32>,
        /// `(algebra, content term)` → element assumption position at that
        /// algebra's carrier. The key is the term itself: `ContentTerm`'s
        /// constructors already normalize ordering and nesting, so
        /// syntactic identity is the algebra's own canonical form.
        pub(super) content_terms: BTreeMap<(ContentAlgebra, ContentTerm), u32>,
        pub(super) integer: Option<u32>,
        pub(super) integer_less_than: Option<u32>,
        pub(super) integer_less_or_equal: Option<u32>,
        pub(super) integer_laws: BTreeMap<IntegerLaw, u32>,
        pub(super) binary_numerals: binary_numerals::BinaryNumerals,
        pub(super) subtraction: subtraction::Subtraction,
        pub(super) addition: addition::Addition,
        pub(super) multiplication: multiplication::Multiplication,
        pub(super) forbidden_roots: forbidden_roots::ForbiddenRoots,
        pub(super) math_terms: BTreeMap<MathTermKey, u32>,
        pub(super) closed_math_values: BTreeMap<u32, BigInt>,
        pub(super) scalar_integer_terms: BTreeMap<ScalarTerm, u32>,
        pub(super) integer_operations: BTreeMap<integer_operations::IntegerOperation, u32>,
        pub(super) cast_identities: BTreeMap<integer_operations::IntegerOperation, u32>,
        pub(super) decisions: HashMap<TermHandle, u32>,
        pub(super) carrier_bounds: BTreeMap<(ScalarTerm, bool), u32>,
        pub(super) rule_axioms: BTreeMap<(Vec<Proposition>, Proposition), u32>,
        pub(super) constants: HashMap<u32, TermHandle>,
        pub(super) denotations: logical::Cache,
    }

    pub(super) struct ExactDefinitionBoundTarget<'a> {
        pub(super) literal: &'a ScalarTerm,
        pub(super) output: &'a ScalarTerm,
        pub(super) lower: bool,
        pub(super) scalar_type: &'a IntegerType,
        pub(super) expression: &'a ScalarTerm,
        pub(super) left: &'a ScalarTerm,
        pub(super) right: &'a ScalarTerm,
    }

    pub(super) fn math_less_or_equal_sides(
        conclusion: &Proposition,
        is_target_operation: fn(&IntegerMathTerm) -> bool,
    ) -> Option<(bool, &IntegerMathTerm, &IntegerMathTerm)> {
        match conclusion {
            Proposition::IntegerMathLessOrEqual(bound, operation)
                if is_target_operation(operation) =>
            {
                Some((true, bound, operation))
            }
            Proposition::IntegerMathLessOrEqual(operation, bound)
                if is_target_operation(operation) =>
            {
                Some((false, bound, operation))
            }
            _ => None,
        }
    }

    pub(super) struct OperandBound<'a> {
        pub(super) operand: &'a ScalarTerm,
        pub(super) bound: &'a Proposition,
        pub(super) evidence: TermHandle,
    }

    fn operand_bound<'a>(
        operand: &'a ScalarTerm,
        bound: &'a Proposition,
        evidence: TermHandle,
    ) -> OperandBound<'a> {
        OperandBound {
            operand,
            bound,
            evidence,
        }
    }

    pub(super) struct BinaryBoundChain {
        pub(super) left_endpoint: ScalarTerm,
        pub(super) right_endpoint: ScalarTerm,
        pub(super) left_endpoint_term: TermHandle,
        pub(super) right_endpoint_term: TermHandle,
        pub(super) left_term: TermHandle,
        pub(super) right_term: TermHandle,
        pub(super) bound_operation: TermHandle,
        pub(super) operand_operation: TermHandle,
        pub(super) left_order: TermHandle,
        pub(super) right_order: TermHandle,
    }

    pub(super) fn binary_bound_chain(
        denotation: &mut Denotation,
        left: OperandBound<'_>,
        right: OperandBound<'_>,
        lower: bool,
        right_antitone: bool,
        scalar_type: &IntegerType,
        terms: fn(
            &mut Denotation,
            TermHandle,
            TermHandle,
        ) -> Result<TermHandle, BoundedDenotationError>,
    ) -> Result<Option<BinaryBoundChain>, BoundedDenotationError> {
        let right_lower = if right_antitone { !lower } else { lower };
        let Some((left_endpoint, left_order)) = denotation.add_bound_endpoint(
            left.operand,
            left.bound,
            left.evidence,
            lower,
            scalar_type,
        )?
        else {
            return Ok(None);
        };
        let Some((right_endpoint, right_order)) = denotation.add_bound_endpoint(
            right.operand,
            right.bound,
            right.evidence,
            right_lower,
            scalar_type,
        )?
        else {
            return Ok(None);
        };
        let left_endpoint_term = denotation.fixed_scalar_term(&left_endpoint)?;
        let right_endpoint_term = denotation.fixed_scalar_term(&right_endpoint)?;
        let left_term = denotation.fixed_scalar_term(left.operand)?;
        let right_term = denotation.fixed_scalar_term(right.operand)?;
        let bound_operation = terms(denotation, left_endpoint_term, right_endpoint_term)?;
        let operand_operation = terms(denotation, left_term, right_term)?;
        Ok(Some(BinaryBoundChain {
            left_endpoint,
            right_endpoint,
            left_endpoint_term,
            right_endpoint_term,
            left_term,
            right_term,
            bound_operation,
            operand_operation,
            left_order,
            right_order,
        }))
    }

    pub(super) fn binary_bound_order(
        denotation: &mut Denotation,
        lower: bool,
        chain: &BinaryBoundChain,
        ordered_law: impl Fn(
            &mut Denotation,
            &[TermHandle],
        ) -> Result<TermHandle, BoundedDenotationError>,
    ) -> Result<TermHandle, BoundedDenotationError> {
        if lower {
            ordered_law(
                denotation,
                &[
                    chain.left_endpoint_term,
                    chain.left_term,
                    chain.right_endpoint_term,
                    chain.right_term,
                    chain.left_order,
                    chain.right_order,
                ],
            )
        } else {
            ordered_law(
                denotation,
                &[
                    chain.left_term,
                    chain.left_endpoint_term,
                    chain.right_term,
                    chain.right_endpoint_term,
                    chain.left_order,
                    chain.right_order,
                ],
            )
        }
    }

    pub(super) fn exact_definition_bound_target<'a>(
        conclusion: &'a Proposition,
        definition: &'a Proposition,
        matcher: TermMatcher,
    ) -> Option<ExactDefinitionBoundTarget<'a>> {
        let Proposition::LessOrEqual(conclusion_left, conclusion_right) = conclusion else {
            return None;
        };
        let (literal, output, lower) = if conclusion_left.integer_value().is_some() {
            (conclusion_left, conclusion_right, true)
        } else if conclusion_right.integer_value().is_some() {
            (conclusion_right, conclusion_left, false)
        } else {
            return None;
        };
        let (defined_output, expression) = match definition {
            Proposition::Equal(first, second) => match (first, second) {
                (output @ ScalarTerm::Value { .. }, term) if matcher(term).is_some() => {
                    (output, second)
                }
                (term, output @ ScalarTerm::Value { .. }) if matcher(term).is_some() => {
                    (output, first)
                }
                _ => return None,
            },
            _ => return None,
        };
        let (scalar_type, left, right) = matcher(expression)?;
        if scalar_type.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
            || defined_output != output
            || output.scalar_type() != ScalarType::Integer(*scalar_type)
        {
            return None;
        }
        Some(ExactDefinitionBoundTarget {
            literal,
            output,
            lower,
            scalar_type,
            expression,
            left,
            right,
        })
    }

    pub(super) struct BinaryBoundOperation {
        pub(super) target: TermMatcher,
        pub(super) is_target_operation: fn(&IntegerMathTerm) -> bool,
        pub(super) right_antitone: bool,
        pub(super) terms: fn(
            &mut Denotation,
            TermHandle,
            TermHandle,
        ) -> Result<TermHandle, BoundedDenotationError>,
        pub(super) monotone:
            fn(&mut Denotation, &[TermHandle]) -> Result<TermHandle, BoundedDenotationError>,
        pub(super) numeral_equation: fn(
            &mut Denotation,
            IntegerValue,
            IntegerValue,
            IntegerValue,
            TermHandle,
            TermHandle,
            TermHandle,
        )
            -> Result<Option<TermHandle>, BoundedDenotationError>,
    }

    pub(super) trait PositionRoster<K> {
        fn roster_position(&self, key: &K) -> Option<u32>;
        fn record_position(&mut self, key: K, position: u32);
    }

    impl<K: Ord> PositionRoster<K> for BTreeMap<K, u32> {
        fn roster_position(&self, key: &K) -> Option<u32> {
            self.get(key).copied()
        }

        fn record_position(&mut self, key: K, position: u32) {
            self.insert(key, position);
        }
    }

    impl<K: Eq + std::hash::Hash> PositionRoster<K> for HashMap<K, u32> {
        fn roster_position(&self, key: &K) -> Option<u32> {
            self.get(key).copied()
        }

        fn record_position(&mut self, key: K, position: u32) {
            self.insert(key, position);
        }
    }

    impl Denotation {
        pub(super) fn direct_binary_bound_evidence(
            &mut self,
            operation: &BinaryBoundOperation,
            premise: &Proposition,
            evidence: TermHandle,
            witness: &crate::IntegerAffineWitness,
            conclusion: &Proposition,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            let Proposition::Conjunction(bounds) = premise else {
                return Ok(None);
            };
            let [left_bound, right_bound] = bounds.as_slice() else {
                return Ok(None);
            };
            let Some((scalar_type, left, right)) = (operation.target)(&witness.target) else {
                return Ok(None);
            };
            if scalar_type.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
                || witness.root != *left
                || !witness.definition_axioms.is_empty()
                || !witness.literal_axioms.is_empty()
            {
                return Ok(None);
            }
            let Some((lower, bound, result)) =
                math_less_or_equal_sides(conclusion, operation.is_target_operation)
            else {
                return Ok(None);
            };
            let IntegerMathTerm::IntegerLiteral(bound_literal) = bound else {
                return Ok(None);
            };
            let Some(bound_value) = math_literal_value(*bound_literal) else {
                return Ok(None);
            };
            let left_evidence = self.arena.insert(Term::Fst { pair: evidence });
            let right_evidence = self.arena.insert(Term::Snd { pair: evidence });
            let Some(chain) = binary_bound_chain(
                self,
                operand_bound(left, left_bound, left_evidence),
                operand_bound(right, right_bound, right_evidence),
                lower,
                operation.right_antitone,
                scalar_type,
                operation.terms,
            )?
            else {
                return Ok(None);
            };
            let result_term = self.math_term(result)?;
            if !self
                .arena
                .structurally_equal(result_term, chain.operand_operation)
            {
                return Ok(None);
            }
            let order = binary_bound_order(self, lower, &chain, operation.monotone)?;
            let (Some((_, left_value)), Some((_, right_value))) = (
                chain.left_endpoint.integer_value(),
                chain.right_endpoint.integer_value(),
            ) else {
                return Ok(None);
            };
            let bound_term = self.math_term(bound)?;
            let Some(equality) = (operation.numeral_equation)(
                self,
                left_value,
                right_value,
                bound_value,
                chain.left_endpoint_term,
                chain.right_endpoint_term,
                bound_term,
            )?
            else {
                return Ok(None);
            };
            if lower {
                self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteLeft,
                    &[
                        chain.bound_operation,
                        bound_term,
                        chain.operand_operation,
                        equality,
                        order,
                    ],
                )
            } else {
                self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteRight,
                    &[
                        chain.operand_operation,
                        chain.bound_operation,
                        bound_term,
                        equality,
                        order,
                    ],
                )
            }
            .map(Some)
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn exact_binary_definition_bound_evidence(
            &mut self,
            operation: &BinaryBoundOperation,
            left_bound: &Proposition,
            left_evidence: TermHandle,
            right_bound: &Proposition,
            right_evidence: TermHandle,
            definition: &Proposition,
            definition_evidence: TermHandle,
            conclusion: &Proposition,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            let Some(ExactDefinitionBoundTarget {
                literal,
                output,
                lower,
                scalar_type,
                expression,
                left,
                right,
            }) = exact_definition_bound_target(conclusion, definition, operation.target)
            else {
                return Ok(None);
            };
            let Some(chain) = binary_bound_chain(
                self,
                operand_bound(left, left_bound, left_evidence),
                operand_bound(right, right_bound, right_evidence),
                lower,
                operation.right_antitone,
                scalar_type,
                operation.terms,
            )?
            else {
                return Ok(None);
            };
            let order = binary_bound_order(self, lower, &chain, operation.monotone)?;
            let output_term = self.fixed_scalar_term(output)?;
            let expression_term = self.fixed_scalar_term(expression)?;
            let denoted = self.denote(definition)?;
            let Some((_, from, to)) = self.identity_parts(denoted) else {
                return Ok(None);
            };
            let result_equality = if self
                .arena
                .structurally_equal(expression_term, chain.operand_operation)
            {
                let Some(equality) = self.directed_equality(
                    from,
                    to,
                    chain.operand_operation,
                    output_term,
                    definition_evidence,
                ) else {
                    return Ok(None);
                };
                equality
            } else {
                let (Some((_, left_value)), Some((_, right_value)), Some((_, result_value))) = (
                    left.integer_value(),
                    right.integer_value(),
                    expression.integer_value(),
                ) else {
                    return Ok(None);
                };
                let Some(bridge) = (operation.numeral_equation)(
                    self,
                    left_value,
                    right_value,
                    result_value,
                    chain.left_term,
                    chain.right_term,
                    expression_term,
                )?
                else {
                    return Ok(None);
                };
                let Some(forward) = self.directed_equality(
                    from,
                    to,
                    expression_term,
                    output_term,
                    definition_evidence,
                ) else {
                    return Ok(None);
                };
                let integer = self.integer_constant()?;
                self.transitivity(
                    integer,
                    chain.operand_operation,
                    expression_term,
                    output_term,
                    bridge,
                    forward,
                )
            };
            let order = if lower {
                self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteRight,
                    &[
                        chain.bound_operation,
                        chain.operand_operation,
                        output_term,
                        result_equality,
                        order,
                    ],
                )?
            } else {
                self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteLeft,
                    &[
                        chain.operand_operation,
                        output_term,
                        chain.bound_operation,
                        result_equality,
                        order,
                    ],
                )?
            };
            let (Some((_, left_value)), Some((_, right_value)), Some((_, bound_value))) = (
                chain.left_endpoint.integer_value(),
                chain.right_endpoint.integer_value(),
                literal.integer_value(),
            ) else {
                return Ok(None);
            };
            let literal_term = self.fixed_scalar_term(literal)?;
            let Some(equality) = (operation.numeral_equation)(
                self,
                left_value,
                right_value,
                bound_value,
                chain.left_endpoint_term,
                chain.right_endpoint_term,
                literal_term,
            )?
            else {
                return Ok(None);
            };
            if lower {
                self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteLeft,
                    &[
                        chain.bound_operation,
                        literal_term,
                        output_term,
                        equality,
                        order,
                    ],
                )
            } else {
                self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteRight,
                    &[
                        output_term,
                        chain.bound_operation,
                        literal_term,
                        equality,
                        order,
                    ],
                )
            }
            .map(Some)
        }

        pub(super) fn new() -> Self {
            Self::new_in(TermArena::new(), Vec::new())
        }

        pub(super) fn new_in(mut arena: TermArena, declarations: Vec<Declaration>) -> Self {
            let type_zero = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
            let two = arena.insert(Term::Two);
            let two_zero = arena.insert(Term::TwoZero);
            let two_one = arena.insert(Term::TwoOne);
            Self {
                arena,
                type_zero,
                two,
                two_zero,
                two_one,
                boolean_equality: TermHandle::invalid(),
                declarations,
                atoms: BTreeMap::new(),
                carriers: BTreeMap::new(),
                terms: BTreeMap::new(),
                content_carriers: BTreeMap::new(),
                content_families: [None, None],
                content_parameters: BTreeMap::new(),
                content_terms: BTreeMap::new(),
                integer: None,
                integer_less_than: None,
                integer_less_or_equal: None,
                integer_laws: BTreeMap::new(),
                binary_numerals: binary_numerals::BinaryNumerals::default(),
                subtraction: subtraction::Subtraction::default(),
                addition: addition::Addition::default(),
                multiplication: multiplication::Multiplication::default(),
                forbidden_roots: forbidden_roots::ForbiddenRoots::default(),
                math_terms: BTreeMap::new(),
                closed_math_values: BTreeMap::new(),
                scalar_integer_terms: BTreeMap::new(),
                integer_operations: BTreeMap::new(),
                cast_identities: BTreeMap::new(),
                decisions: HashMap::new(),
                carrier_bounds: BTreeMap::new(),
                rule_axioms: BTreeMap::new(),
                constants: HashMap::new(),
                denotations: logical::Cache::default(),
            }
        }

        pub(super) fn position(&self) -> Result<u32, BoundedDenotationError> {
            u32::try_from(self.declarations.len())
                .map_err(|_| BoundedDenotationError::DepthLimitExceeded)
        }

        pub(super) fn constant(&mut self, declaration: u32) -> TermHandle {
            if let Some(&handle) = self.constants.get(&declaration) {
                return handle;
            }
            let handle = self.arena.insert(Term::Constant {
                declaration,
                levels: Vec::new(),
            });
            self.constants.insert(declaration, handle);
            handle
        }

        pub(super) fn atom(
            &mut self,
            proposition: &Proposition,
        ) -> Result<TermHandle, BoundedDenotationError> {
            let key =
                lift_fixed_integer_relation(proposition).unwrap_or_else(|| proposition.clone());
            let position = self.interned_keyed_assumption(
                |this| &mut this.atoms,
                key,
                |this| Ok(this.type_zero),
            )?;
            Ok(self.constant(position))
        }

        pub(super) fn carrier(
            &mut self,
            scalar_type: ScalarType,
        ) -> Result<TermHandle, BoundedDenotationError> {
            if scalar_type == ScalarType::Boolean {
                return Ok(self.two);
            }
            let position = self.interned_keyed_assumption(
                |this| &mut this.carriers,
                scalar_type,
                |this| Ok(this.type_zero),
            )?;
            Ok(self.constant(position))
        }

        pub(super) fn scalar_term(
            &mut self,
            term: &ScalarTerm,
        ) -> Result<TermHandle, BoundedDenotationError> {
            super::booleans::scalar_denotation::denote(self, term)
        }

        pub(super) fn scalar_leaf(
            &mut self,
            term: &ScalarTerm,
        ) -> Result<TermHandle, BoundedDenotationError> {
            if let Some(relation) = self.integer_relation_term(term)? {
                return Ok(relation);
            }
            let key = match term.integer_value() {
                Some((integer_type, value)) => {
                    ScalarTerm::integer(integer_type, value).unwrap_or_else(|_| term.clone())
                }
                None => term.clone(),
            };
            let scalar_type = key.scalar_type();
            let position = self.interned_keyed_assumption(
                |this| &mut this.terms,
                key,
                |this| this.carrier(scalar_type),
            )?;
            Ok(self.constant(position))
        }

        /// The `Type 0` carrier one closed content algebra denotes: its canonical
        /// family applied to the exact parameter. Conservation equations are
        /// identities over this carrier, interned once per algebra.
        pub(super) fn content_carrier(
            &mut self,
            algebra: &ContentAlgebra,
        ) -> Result<TermHandle, BoundedDenotationError> {
            if let Some(carrier) = self.content_carriers.get(algebra) {
                return Ok(*carrier);
            }
            let position = self.interned_keyed_assumption(
                |this| &mut this.content_parameters,
                algebra.parameter.clone(),
                |this| Ok(this.type_zero),
            )?;
            let parameter = self.constant(position);
            let carrier = self.content_carrier_at(algebra.kind, parameter, Level::Constant(0))?;
            self.content_carriers.insert(algebra.clone(), carrier);
            Ok(carrier)
        }

        /// The canonical algebra family is small; its type parameter is an outer
        /// argument, rather than a stored literal field. Its exact checked level
        /// remains on the application, including under source Pi substitution.
        pub(super) fn content_carrier_at(
            &mut self,
            kind: semantic_vocabulary::ContentAlgebraKind,
            parameter: TermHandle,
            level: Level,
        ) -> Result<TermHandle, BoundedDenotationError> {
            let slot = match kind {
                semantic_vocabulary::ContentAlgebraKind::IntervalSet => 0,
                semantic_vocabulary::ContentAlgebraKind::CountedQuantity => 1,
            };
            let declaration = match self.content_families[slot] {
                Some(declaration) => declaration,
                None => {
                    let domain = self
                        .arena
                        .insert(Term::Sort(Sort::Type(Level::Parameter(0))));
                    let ty = self.arena.insert(Term::Pi {
                        domain,
                        codomain: self.type_zero,
                    });
                    let declaration = self.position()?;
                    self.declarations.push(Declaration::assumption(1, ty));
                    self.content_families[slot] = Some(declaration);
                    declaration
                }
            };
            let function = self.arena.insert(Term::Constant {
                declaration,
                levels: vec![level],
            });
            Ok(self.arena.insert(Term::Apply {
                function,
                argument: parameter,
            }))
        }

        pub(super) fn content_term(
            &mut self,
            algebra: &ContentAlgebra,
            term: &ContentTerm,
        ) -> Result<TermHandle, BoundedDenotationError> {
            let position = self.interned_keyed_assumption(
                |this| &mut this.content_terms,
                (algebra.clone(), term.clone()),
                |this| this.content_carrier(algebra),
            )?;
            Ok(self.constant(position))
        }

        pub(super) fn integer(&mut self) -> Result<u32, BoundedDenotationError> {
            self.interned_slot_assumption(|this| &mut this.integer, |this| Ok(this.type_zero))
        }

        pub(super) fn integer_constant(&mut self) -> Result<TermHandle, BoundedDenotationError> {
            let position = self.integer()?;
            Ok(self.constant(position))
        }

        pub(super) fn integer_order_relation(&mut self) -> Result<u32, BoundedDenotationError> {
            let integer = self.integer()?;
            let ty = build(
                &mut self.arena,
                &mut Vec::new(),
                &scheme_dsl::pi(
                    "_",
                    scheme_dsl::scheme_at(integer, Vec::new()),
                    scheme_dsl::pi(
                        "_",
                        scheme_dsl::scheme_at(integer, Vec::new()),
                        scheme_dsl::sort(Level::Constant(0)),
                    ),
                ),
            );
            let position = self.position()?;
            self.declarations.push(Declaration::assumption(0, ty));
            Ok(position)
        }

        pub(super) fn integer_less_than(&mut self) -> Result<u32, BoundedDenotationError> {
            self.interned_slot(
                |this| &mut this.integer_less_than,
                |this| this.integer_order_relation(),
            )
        }

        pub(super) fn integer_less_or_equal(&mut self) -> Result<u32, BoundedDenotationError> {
            self.interned_slot(
                |this| &mut this.integer_less_or_equal,
                |this| this.integer_order_relation(),
            )
        }

        pub(super) fn integer_law(
            &mut self,
            law: IntegerLaw,
        ) -> Result<u32, BoundedDenotationError> {
            self.roster_law_position(
                |this| &mut this.integer_laws,
                law,
                |this| -> Result<scheme_dsl::Syntax, BoundedDenotationError> {
                    let integer = this.integer()?;
                    let less_than = this.integer_less_than()?;
                    let less_or_equal = this.integer_less_or_equal()?;
                    Ok(integer_law_statement(
                        law,
                        integer,
                        less_than,
                        less_or_equal,
                    ))
                },
            )
        }

        pub(super) fn constant_application(
            &mut self,
            position: u32,
            arguments: &[TermHandle],
        ) -> TermHandle {
            let constant = self.constant(position);
            self.arena.apply_arguments(constant, arguments)
        }

        pub(super) fn integer_law_application(
            &mut self,
            law: IntegerLaw,
            arguments: &[TermHandle],
        ) -> Result<TermHandle, BoundedDenotationError> {
            let position = self.integer_law(law)?;
            Ok(self.constant_application(position, arguments))
        }

        pub(super) fn interned_slot_assumption(
            &mut self,
            slot: impl Fn(&mut Self) -> &mut Option<u32>,
            statement: impl FnOnce(&mut Self) -> Result<TermHandle, BoundedDenotationError>,
        ) -> Result<u32, BoundedDenotationError> {
            if let Some(position) = *slot(self) {
                return Ok(position);
            }
            let ty = statement(self)?;
            let position = self.position()?;
            self.declarations.push(Declaration::assumption(0, ty));
            *slot(self) = Some(position);
            Ok(position)
        }

        pub(super) fn interned_slot(
            &mut self,
            slot: impl Fn(&mut Self) -> &mut Option<u32>,
            produce: impl FnOnce(&mut Self) -> Result<u32, BoundedDenotationError>,
        ) -> Result<u32, BoundedDenotationError> {
            if let Some(position) = *slot(self) {
                return Ok(position);
            }
            let position = produce(self)?;
            *slot(self) = Some(position);
            Ok(position)
        }

        pub(super) fn interned_keyed_assumption<K, M: PositionRoster<K>>(
            &mut self,
            roster: impl Fn(&mut Self) -> &mut M,
            key: K,
            statement: impl FnOnce(&mut Self) -> Result<TermHandle, BoundedDenotationError>,
        ) -> Result<u32, BoundedDenotationError> {
            if let Some(position) = roster(self).roster_position(&key) {
                return Ok(position);
            }
            let ty = statement(self)?;
            let position = self.position()?;
            self.declarations.push(Declaration::assumption(0, ty));
            roster(self).record_position(key, position);
            Ok(position)
        }

        pub(super) fn roster_law_position<K>(
            &mut self,
            laws: impl Fn(&mut Self) -> &mut BTreeMap<K, u32>,
            law: K,
            statement: impl FnOnce(&mut Self) -> Result<scheme_dsl::Syntax, BoundedDenotationError>,
        ) -> Result<u32, BoundedDenotationError>
        where
            K: Ord + Copy,
        {
            self.interned_keyed_assumption(laws, law, |this| {
                let statement = statement(this)?;
                Ok(build(&mut this.arena, &mut Vec::new(), &statement))
            })
        }

        pub(super) fn roster_law_application<K>(
            &mut self,
            laws: impl Fn(&mut Self) -> &mut BTreeMap<K, u32>,
            law: K,
            statement: impl FnOnce(&mut Self) -> Result<scheme_dsl::Syntax, BoundedDenotationError>,
            arguments: &[TermHandle],
        ) -> Result<TermHandle, BoundedDenotationError>
        where
            K: Ord + Copy,
        {
            let position = self.roster_law_position(laws, law, statement)?;
            Ok(self.constant_application(position, arguments))
        }

        pub(super) fn math_term(
            &mut self,
            term: &IntegerMathTerm,
        ) -> Result<TermHandle, BoundedDenotationError> {
            if let IntegerMathTerm::Embed(term) = term {
                return self.fixed_scalar_term(term);
            }
            let key = match ClosedIntegerEvaluator::default().evaluate_closed(term) {
                Ok(Some(value)) => MathTermKey::Closed(value),
                Ok(None) => {
                    if let IntegerMathTerm::Subtract(left, right)
                    | IntegerMathTerm::Add(left, right)
                    | IntegerMathTerm::Multiply(left, right) = term
                        && ClosedIntegerEvaluator::default()
                            .evaluate_closed(left)
                            .is_ok()
                        && ClosedIntegerEvaluator::default()
                            .evaluate_closed(right)
                            .is_ok()
                    {
                        let left = self.math_term(left)?;
                        let right = self.math_term(right)?;
                        return if matches!(term, IntegerMathTerm::Add(..)) {
                            self.add_terms(left, right)
                        } else if matches!(term, IntegerMathTerm::Subtract(..)) {
                            self.subtract_terms(left, right)
                        } else {
                            self.multiply_terms(left, right)
                        };
                    }
                    MathTermKey::Open(term.clone())
                }
                Err(error) => {
                    return Err(BoundedDenotationError::Certificate(
                        ProofError::PrimitiveJudgment(KernelError::ClosedIntegerEvaluation(error)),
                    ));
                }
            };
            self.intern_math_term(key)
        }

        pub(super) fn math_literal(
            &mut self,
            value: BigInt,
        ) -> Result<TermHandle, BoundedDenotationError> {
            self.intern_math_term(MathTermKey::Closed(value))
        }

        fn intern_math_term(
            &mut self,
            key: MathTermKey,
        ) -> Result<TermHandle, BoundedDenotationError> {
            if let Some(&position) = self.math_terms.get(&key) {
                return Ok(self.constant(position));
            }
            if let MathTermKey::Closed(value) = &key
                && let Some((negative, magnitude)) = binary_numerals::fixed_magnitude(value)
            {
                let position = self.binary_integer(negative, magnitude)?;
                self.closed_math_values.insert(position, value.clone());
                self.math_terms.insert(key, position);
                return Ok(self.constant(position));
            }
            let ty = self.integer_constant()?;
            let position = self.position()?;
            self.declarations.push(Declaration::assumption(0, ty));
            if let MathTermKey::Closed(value) = &key {
                self.closed_math_values.insert(position, value.clone());
            }
            self.math_terms.insert(key, position);
            Ok(self.constant(position))
        }

        pub(super) fn closed_math_value(&self, term: TermHandle) -> Option<BigInt> {
            let Term::Constant { declaration, .. } = self.arena.get(term) else {
                return None;
            };
            self.closed_math_values.get(&declaration).cloned()
        }

        pub(super) fn fixed_scalar_term(
            &mut self,
            term: &ScalarTerm,
        ) -> Result<TermHandle, BoundedDenotationError> {
            super::integer_operations::fixed_scalar::denote(self, term)
        }

        pub(super) fn fixed_scalar_relation(
            &mut self,
            proposition: &Proposition,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            let (left, right) = match proposition {
                semantic_vocabulary::ordered_comparison_operands!(left, right) => (left, right),
                _ => return Ok(None),
            };
            let fixed_integer = |term: &ScalarTerm| matches!(term.scalar_type(), ScalarType::Integer(integer) if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed && !integer.is_address());
            if !fixed_integer(left) || !fixed_integer(right) {
                return Ok(None);
            }
            let left = self.fixed_scalar_term(left)?;
            let right = self.fixed_scalar_term(right)?;
            let denoted = if matches!(proposition, Proposition::Equal(..)) {
                let ty = self.integer_constant()?;
                self.arena.insert(Term::Id { ty, left, right })
            } else {
                let relation = if matches!(proposition, Proposition::LessThan(..)) {
                    self.integer_less_than()?
                } else {
                    self.integer_less_or_equal()?
                };
                let relation = self.constant(relation);
                self.arena.apply_binary(relation, left, right)
            };
            Ok(Some(denoted))
        }

        pub(super) fn order_term_application(
            &mut self,
            strict: bool,
            left: TermHandle,
            right: TermHandle,
        ) -> Result<TermHandle, BoundedDenotationError> {
            let position = if strict {
                self.integer_less_than()?
            } else {
                self.integer_less_or_equal()?
            };
            let relation = self.constant(position);
            Ok(self.arena.apply_binary(relation, left, right))
        }

        pub(super) fn integer_order_term(
            &mut self,
            strict: bool,
            left: &IntegerMathTerm,
            right: &IntegerMathTerm,
        ) -> Result<TermHandle, BoundedDenotationError> {
            let left = self.math_term(left)?;
            let right = self.math_term(right)?;
            self.order_term_application(strict, left, right)
        }

        pub(super) fn integer_relation(&self, denoted: TermHandle) -> Option<IntegerRelation> {
            match self.arena.get(denoted) {
                Term::Id { ty, left, right } => match self.arena.get(ty) {
                    Term::Constant { declaration, .. } if Some(declaration) == self.integer => {
                        Some(IntegerRelation::Equality { left, right })
                    }
                    _ => None,
                },
                Term::Apply {
                    function,
                    argument: right,
                } => match self.arena.get(function) {
                    Term::Apply {
                        function: relation,
                        argument: left,
                    } => match self.arena.get(relation) {
                        Term::Constant { declaration, .. }
                            if Some(declaration) == self.integer_less_than =>
                        {
                            Some(IntegerRelation::LessThan { left, right })
                        }
                        Term::Constant { declaration, .. }
                            if Some(declaration) == self.integer_less_or_equal =>
                        {
                            Some(IntegerRelation::LessOrEqual { left, right })
                        }
                        _ => None,
                    },
                    _ => None,
                },
                _ => None,
            }
        }

        pub(super) fn identity_parts(
            &self,
            denoted: TermHandle,
        ) -> Option<(TermHandle, TermHandle, TermHandle)> {
            if let Term::Id { ty, left, right } = self.arena.get(denoted) {
                Some((ty, left, right))
            } else {
                None
            }
        }

        pub(super) fn symmetry(
            &mut self,
            ty: TermHandle,
            from: TermHandle,
            to: TermHandle,
            proof: TermHandle,
        ) -> TermHandle {
            let bound = self.arena.insert(Term::Variable(0));
            let identity = self.arena.insert(Term::Id {
                ty,
                left: from,
                right: bound,
            });
            let flipped = self.arena.insert(Term::Variable(1));
            let body = self.arena.insert(Term::Id {
                ty,
                left: flipped,
                right: from,
            });
            let inner = self.arena.insert(Term::Lambda {
                domain: identity,
                body,
            });
            let motive = self.arena.insert(Term::Lambda {
                domain: ty,
                body: inner,
            });
            let base = self.arena.insert(Term::Refl { ty, value: from });
            self.arena.insert(Term::IdElim {
                motive,
                base,
                endpoint: to,
                proof,
            })
        }

        pub(super) fn transitivity(
            &mut self,
            ty: TermHandle,
            left: TermHandle,
            middle: TermHandle,
            right: TermHandle,
            first: TermHandle,
            second: TermHandle,
        ) -> TermHandle {
            let bound = self.arena.insert(Term::Variable(0));
            let identity = self.arena.insert(Term::Id {
                ty,
                left: middle,
                right: bound,
            });
            let endpoint = self.arena.insert(Term::Variable(1));
            let body = self.arena.insert(Term::Id {
                ty,
                left,
                right: endpoint,
            });
            let inner = self.arena.insert(Term::Lambda {
                domain: identity,
                body,
            });
            let motive = self.arena.insert(Term::Lambda {
                domain: ty,
                body: inner,
            });
            self.arena.insert(Term::IdElim {
                motive,
                base: first,
                endpoint: right,
                proof: second,
            })
        }

        pub(super) fn symmetry_evidence(
            &mut self,
            premise: TermHandle,
            goal: TermHandle,
            evidence: TermHandle,
        ) -> Option<TermHandle> {
            if self.arena.structurally_equal(premise, goal) {
                return Some(evidence);
            }
            let (carrier, left, right) = self.identity_parts(premise)?;
            let (goal_carrier, goal_left, goal_right) = self.identity_parts(goal)?;
            (self.arena.structurally_equal(carrier, goal_carrier)
                && self.arena.structurally_equal(goal_left, right)
                && self.arena.structurally_equal(goal_right, left))
            .then(|| self.symmetry(carrier, left, right, evidence))
        }

        pub(super) fn transitivity_evidence(
            &mut self,
            first_ty: TermHandle,
            first: TermHandle,
            second_ty: TermHandle,
            second: TermHandle,
            goal_ty: TermHandle,
        ) -> Option<TermHandle> {
            let (first_carrier, a, b) = self.identity_parts(first_ty)?;
            let (second_carrier, c, d) = self.identity_parts(second_ty)?;
            let (goal_carrier, e, f) = self.identity_parts(goal_ty)?;
            if !(self.arena.structurally_equal(first_carrier, second_carrier)
                && self.arena.structurally_equal(first_carrier, goal_carrier))
            {
                return None;
            }
            let (composed, left, right) = if self.arena.structurally_equal(b, c) {
                (
                    self.transitivity(first_carrier, a, b, d, first, second),
                    a,
                    d,
                )
            } else if self.arena.structurally_equal(b, d) {
                let second = self.symmetry(second_carrier, c, d, second);
                (
                    self.transitivity(first_carrier, a, b, c, first, second),
                    a,
                    c,
                )
            } else if self.arena.structurally_equal(a, c) {
                let first = self.symmetry(first_carrier, a, b, first);
                (
                    self.transitivity(first_carrier, b, a, d, first, second),
                    b,
                    d,
                )
            } else if self.arena.structurally_equal(a, d) {
                let first = self.symmetry(first_carrier, a, b, first);
                let second = self.symmetry(second_carrier, c, d, second);
                (
                    self.transitivity(first_carrier, b, a, c, first, second),
                    b,
                    c,
                )
            } else {
                return None;
            };
            if self.arena.structurally_equal(e, left) && self.arena.structurally_equal(f, right) {
                Some(composed)
            } else if self.arena.structurally_equal(e, right)
                && self.arena.structurally_equal(f, left)
            {
                Some(self.symmetry(goal_carrier, left, right, composed))
            } else {
                None
            }
        }

        pub(super) fn weakening_evidence(
            &mut self,
            premise_ty: TermHandle,
            evidence: TermHandle,
            goal_ty: TermHandle,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            let Some(IntegerRelation::LessOrEqual { left, right }) = self.integer_relation(goal_ty)
            else {
                return Ok(None);
            };
            match self.integer_relation(premise_ty) {
                Some(IntegerRelation::LessThan { left: a, right: b })
                    if self.arena.structurally_equal(a, left)
                        && self.arena.structurally_equal(b, right) =>
                {
                    self.integer_law_application(
                        IntegerLaw::LessThanToLessOrEqual,
                        &[a, b, evidence],
                    )
                    .map(Some)
                }
                Some(IntegerRelation::Equality { left: a, right: b })
                    if self.arena.structurally_equal(a, left)
                        && self.arena.structurally_equal(b, right) =>
                {
                    self.integer_law_application(
                        IntegerLaw::EqualityToLessOrEqual,
                        &[a, b, evidence],
                    )
                    .map(Some)
                }
                Some(IntegerRelation::Equality { left: a, right: b })
                    if self.arena.structurally_equal(a, right)
                        && self.arena.structurally_equal(b, left) =>
                {
                    let integer = self.integer_constant()?;
                    let evidence = self.symmetry(integer, a, b, evidence);
                    self.integer_law_application(
                        IntegerLaw::EqualityToLessOrEqual,
                        &[b, a, evidence],
                    )
                    .map(Some)
                }
                _ => Ok(None),
            }
        }

        pub(super) fn order_transitivity_evidence(
            &mut self,
            first_ty: TermHandle,
            first: TermHandle,
            second_ty: TermHandle,
            second: TermHandle,
            goal_ty: TermHandle,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            fn strictness(relation: IntegerRelation) -> Option<(bool, TermHandle, TermHandle)> {
                match relation {
                    IntegerRelation::LessThan { left, right } => Some((true, left, right)),
                    IntegerRelation::LessOrEqual { left, right } => Some((false, left, right)),
                    IntegerRelation::Equality { .. } => None,
                }
            }
            let Some((first_strict, a, b)) = self.integer_relation(first_ty).and_then(strictness)
            else {
                return Ok(None);
            };
            let Some((second_strict, middle, c)) =
                self.integer_relation(second_ty).and_then(strictness)
            else {
                return Ok(None);
            };
            let Some((goal_strict, goal_left, goal_right)) =
                self.integer_relation(goal_ty).and_then(strictness)
            else {
                return Ok(None);
            };
            let law = match (first_strict, second_strict, goal_strict) {
                (false, false, false) => IntegerLaw::LessOrEqualTransitivity,
                (true, true, true) => IntegerLaw::LessThanTransitivity,
                (true, false, true) => IntegerLaw::LessThanLessOrEqualTransitivity,
                (false, true, true) => IntegerLaw::LessOrEqualLessThanTransitivity,
                _ => return Ok(None),
            };
            if !(self.arena.structurally_equal(b, middle)
                && self.arena.structurally_equal(a, goal_left)
                && self.arena.structurally_equal(c, goal_right))
            {
                return Ok(None);
            }
            self.integer_law_application(law, &[a, b, c, first, second])
                .map(Some)
        }

        pub(super) fn order_substitution_evidence(
            &mut self,
            relation_ty: TermHandle,
            relation_evidence: TermHandle,
            equality_ty: TermHandle,
            equality_evidence: TermHandle,
            endpoint: usize,
            goal_ty: TermHandle,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            let (strict, relation_left, relation_right, substitute_left, substitute_right) =
                match self.integer_relation(relation_ty) {
                    Some(IntegerRelation::LessThan { left, right }) => (
                        true,
                        left,
                        right,
                        IntegerLaw::LessThanSubstituteLeft,
                        IntegerLaw::LessThanSubstituteRight,
                    ),
                    Some(IntegerRelation::LessOrEqual { left, right }) => (
                        false,
                        left,
                        right,
                        IntegerLaw::LessOrEqualSubstituteLeft,
                        IntegerLaw::LessOrEqualSubstituteRight,
                    ),
                    _ => return Ok(None),
                };
            let (goal_left, goal_right) = match self.integer_relation(goal_ty) {
                Some(IntegerRelation::LessThan { left, right }) if strict => (left, right),
                Some(IntegerRelation::LessOrEqual { left, right }) if !strict => (left, right),
                _ => return Ok(None),
            };
            let Some(IntegerRelation::Equality { left: s, right: t }) =
                self.integer_relation(equality_ty)
            else {
                return Ok(None);
            };
            let (law, old, new, endpoints) = match endpoint {
                0 if self.arena.structurally_equal(relation_right, goal_right) => (
                    substitute_left,
                    relation_left,
                    goal_left,
                    [relation_left, goal_left, relation_right],
                ),
                1 if self.arena.structurally_equal(relation_left, goal_left) => (
                    substitute_right,
                    relation_right,
                    goal_right,
                    [relation_left, relation_right, goal_right],
                ),
                _ => return Ok(None),
            };
            let equality_evidence = if self.arena.structurally_equal(s, old)
                && self.arena.structurally_equal(t, new)
            {
                equality_evidence
            } else if self.arena.structurally_equal(s, new) && self.arena.structurally_equal(t, old)
            {
                let integer = self.integer_constant()?;
                self.symmetry(integer, s, t, equality_evidence)
            } else {
                return Ok(None);
            };
            self.integer_law_application(
                law,
                &[
                    endpoints[0],
                    endpoints[1],
                    endpoints[2],
                    equality_evidence,
                    relation_evidence,
                ],
            )
            .map(Some)
        }

        pub(super) fn directed_equality(
            &mut self,
            s: TermHandle,
            t: TermHandle,
            old: TermHandle,
            new: TermHandle,
            evidence: TermHandle,
        ) -> Option<TermHandle> {
            if self.arena.structurally_equal(s, old) && self.arena.structurally_equal(t, new) {
                Some(evidence)
            } else if self.arena.structurally_equal(s, new) && self.arena.structurally_equal(t, old)
            {
                let integer = self.integer_constant().ok()?;
                Some(self.symmetry(integer, s, t, evidence))
            } else {
                None
            }
        }

        pub(super) fn rule_axiom(
            &mut self,
            premises: &[Proposition],
            conclusion: &Proposition,
        ) -> Result<TermHandle, BoundedDenotationError> {
            let position = self.interned_keyed_assumption(
                |this| &mut this.rule_axioms,
                (premises.to_vec(), conclusion.clone()),
                |this| {
                    let mut ty = this.denote(conclusion)?;
                    for premise in premises.iter().rev() {
                        let domain = this.denote(premise)?;
                        ty = this.arena.insert(Term::Pi {
                            domain,
                            codomain: ty,
                        });
                    }
                    Ok(ty)
                },
            )?;
            Ok(self.constant(position))
        }

        pub(super) fn decision(
            &mut self,
            ty: TermHandle,
        ) -> Result<TermHandle, BoundedDenotationError> {
            let position =
                self.interned_keyed_assumption(|this| &mut this.decisions, ty, |_| Ok(ty))?;
            Ok(self.constant(position))
        }

        pub(super) fn affine_root_equality_step(
            &mut self,
            lower: bool,
            witness: &crate::IntegerAffineWitness,
            left_term: TermHandle,
            expression_term: TermHandle,
            equality: Option<(TermHandle, &Proposition)>,
            evidence: TermHandle,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            let Some((equality, proposition)) = equality else {
                return Ok(Some(evidence));
            };
            let root_term = self.fixed_scalar_term(&witness.root)?;
            let denoted = self.denote(proposition)?;
            let Some((_, from, to)) = self.identity_parts(denoted) else {
                return Ok(None);
            };
            let Some(equality) =
                self.directed_equality(from, to, root_term, expression_term, equality)
            else {
                return Ok(None);
            };
            Ok(Some(if lower {
                self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteLeft,
                    &[root_term, expression_term, left_term, equality, evidence],
                )?
            } else {
                self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteRight,
                    &[left_term, root_term, expression_term, equality, evidence],
                )?
            }))
        }

        pub(super) fn affine_endpoint_equality_order(
            &mut self,
            lower: bool,
            endpoint_equality: Option<(TermHandle, &Proposition)>,
            endpoint_literal: &ScalarTerm,
            endpoint_term: TermHandle,
            order: TermHandle,
            swapped_term: impl Fn(&mut Self) -> Result<TermHandle, BoundedDenotationError>,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            let Some((equality, proposition)) = endpoint_equality else {
                return Ok(Some(order));
            };
            let literal = self.fixed_scalar_term(endpoint_literal)?;
            let swapped = swapped_term(self)?;
            let denoted = self.denote(proposition)?;
            let Some((_, from, to)) = self.identity_parts(denoted) else {
                return Ok(None);
            };
            let Some(equality) = self.directed_equality(from, to, endpoint_term, literal, equality)
            else {
                return Ok(None);
            };
            if lower {
                self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteLeft,
                    &[endpoint_term, literal, swapped, equality, order],
                )
            } else {
                self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteRight,
                    &[swapped, endpoint_term, literal, equality, order],
                )
            }
            .map(Some)
        }

        pub(super) fn denote(
            &mut self,
            proposition: &Proposition,
        ) -> Result<TermHandle, BoundedDenotationError> {
            logical::denote(self, proposition)
        }

        fn denote_leaf(
            &mut self,
            proposition: &Proposition,
        ) -> Result<TermHandle, BoundedDenotationError> {
            Ok(match proposition {
                Proposition::Truth => {
                    let ty = self.two;
                    let zero = self.two_zero;
                    self.arena.insert(Term::Id {
                        ty,
                        left: zero,
                        right: zero,
                    })
                }
                Proposition::Falsehood => {
                    let ty = self.two;
                    let left = self.two_zero;
                    let right = self.two_one;
                    self.arena.insert(Term::Id { ty, left, right })
                }
                Proposition::Equal(..)
                | Proposition::LessThan(..)
                | Proposition::LessOrEqual(..) => {
                    if let Some(lifted) = lift_fixed_integer_relation(proposition) {
                        self.denote(&lifted)?
                    } else if let Some(denoted) = self.fixed_scalar_relation(proposition)? {
                        denoted
                    } else if let Proposition::Equal(left, right) = proposition {
                        let ty = self.carrier(left.scalar_type())?;
                        let left = self.scalar_term(left)?;
                        let right = self.scalar_term(right)?;
                        self.arena.insert(Term::Id { ty, left, right })
                    } else {
                        self.atom(proposition)?
                    }
                }
                Proposition::IntegerMathEqual(left, right) => {
                    let left = self.math_term(left)?;
                    let right = self.math_term(right)?;
                    let ty = self.integer_constant()?;
                    self.arena.insert(Term::Id { ty, left, right })
                }
                Proposition::IntegerMathLessThan(left, right) => {
                    self.integer_order_term(true, left, right)?
                }
                Proposition::IntegerMathLessOrEqual(left, right) => {
                    self.integer_order_term(false, left, right)?
                }
                Proposition::Conjunction(_)
                | Proposition::Disjunction(_)
                | Proposition::Implication { .. } => {
                    unreachable!(
                        "the logical continuation owns every ordered child and arity check"
                    )
                }
                Proposition::ContentConservation(conservation) => {
                    let ty = self.content_carrier(conservation.algebra())?;
                    let left = self.content_term(conservation.algebra(), conservation.left())?;
                    let right = self.content_term(conservation.algebra(), conservation.right())?;
                    self.arena.insert(Term::Id { ty, left, right })
                }
                atomic => self.atom(atomic)?,
            })
        }

        fn conjunction_terms(&mut self, mut members: Vec<TermHandle>) -> TermHandle {
            let mut tail = members
                .pop()
                .expect("canonical connective has at least two members");
            while let Some(domain) = members.pop() {
                tail = self.arena.insert(Term::Sigma {
                    domain,
                    codomain: tail,
                });
            }
            tail
        }

        pub(super) fn disjunction(
            &mut self,
            disjuncts: &[Proposition],
        ) -> Result<TermHandle, BoundedDenotationError> {
            let members = self.connective_members(
                disjuncts,
                ProofError::RuleConclusionMismatch("disjunction arity"),
            )?;
            Ok(self.disjunction_terms(members))
        }

        fn disjunction_terms(&mut self, mut members: Vec<TermHandle>) -> TermHandle {
            let mut tail = members
                .pop()
                .expect("canonical connective has at least two members");
            while let Some(first) = members.pop() {
                tail = self.tagged_sum(first, tail);
            }
            tail
        }

        fn connective_members(
            &mut self,
            members: &[Proposition],
            arity_error: ProofError,
        ) -> Result<Vec<TermHandle>, BoundedDenotationError> {
            if members.len() < 2 {
                return Err(BoundedDenotationError::Certificate(arity_error));
            }
            let mut terms = Vec::with_capacity(members.len());
            for member in members {
                terms.push(self.denote(member)?);
            }
            Ok(terms)
        }

        pub(super) fn tagged_sum(
            &mut self,
            zero_case: TermHandle,
            one_case: TermHandle,
        ) -> TermHandle {
            let motive = self.arena.insert(Term::Lambda {
                domain: self.two,
                body: self.type_zero,
            });
            let bound = self.arena.insert(Term::Variable(0));
            let family = self.arena.insert(Term::CaseTwo {
                motive,
                zero_branch: zero_case,
                one_branch: one_case,
                scrutinee: bound,
            });
            self.arena.insert(Term::Sigma {
                domain: self.two,
                codomain: family,
            })
        }
    }
}
mod equality_transport {
    use super::{
        BoundedDenotationError, Denotation, IntegerLaw, IntegerRelation, MAX_ELABORATION_NODES,
        integer_value_of,
    };
    use super::{ProofNode, Proposition, Term, TermHandle};
    use crate::mathematical_core::substitution::shift;

    mod coercion {
        use super::*;

        #[derive(Clone, Copy)]
        struct Request {
            premise: TermHandle,
            goal: TermHandle,
            evidence: TermHandle,
        }

        impl Request {
            fn new(premise: TermHandle, goal: TermHandle, evidence: TermHandle) -> Self {
                Self {
                    premise,
                    goal,
                    evidence,
                }
            }
        }

        #[derive(Clone, Copy)]
        struct Cases {
            motive: TermHandle,
            zero: TermHandle,
            one: TermHandle,
            goal_zero: TermHandle,
            goal_one: TermHandle,
            evidence: TermHandle,
        }

        enum Work {
            Coerce(Request),
            Shape(Request),
            Refute(Request),
            Shuffle(Request),
            ShuffleRest(Request),
            ShuffleGoal(Request),
            Cases(Cases),
        }

        enum Pending {
            Fallback(Request),
            Refutation(Request),
            RefuteFirst {
                request: Request,
                codomain: TermHandle,
            },
            FunctionArgument {
                evidence: TermHandle,
                codomain: TermHandle,
                goal_domain: TermHandle,
                goal_codomain: TermHandle,
            },
            FunctionBody {
                domain: TermHandle,
            },
            ProductFirst {
                request: Request,
                domain: TermHandle,
                codomain: TermHandle,
                goal_domain: TermHandle,
                goal_codomain: TermHandle,
            },
            PairSecond {
                first: TermHandle,
            },
            CaseZero(Cases),
            CaseOne {
                cases: Cases,
                zero: TermHandle,
            },
            ShuffleFirst {
                request: Request,
                codomain: TermHandle,
            },
            ShuffleSecond(Request),
            GoalFirst {
                request: Request,
                codomain: TermHandle,
            },
            TagZero {
                request: Request,
                one: TermHandle,
            },
            TagOne,
        }

        pub(super) fn construct(
            d: &mut Denotation,
            premise: TermHandle,
            goal: TermHandle,
            evidence: TermHandle,
            remaining: &mut u64,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            let mut pending = Vec::new();
            let mut work = Work::Coerce(Request::new(premise, goal, evidence));
            'drive: loop {
                let mut value = match work {
                    Work::Coerce(request) => {
                        if let Some(rest) = remaining.checked_sub(1) {
                            *remaining = rest;
                            pending.push(Pending::Fallback(request));
                            if d.arena.structurally_equal(request.premise, request.goal) {
                                Some(request.evidence)
                            } else if let Some(inhabited) = d.closed_inhabitant(request.goal)? {
                                Some(inhabited)
                            } else {
                                pending.push(Pending::Refutation(request));
                                work = Work::Refute(request);
                                continue 'drive;
                            }
                        } else {
                            None
                        }
                    }
                    Work::Refute(request) => {
                        if let Term::Sigma { domain, codomain } = d.arena.get(request.premise)
                            && !d.occurs_free(codomain, 0)
                        {
                            let first = d.arena.insert(Term::Fst {
                                pair: request.evidence,
                            });
                            pending.push(Pending::RefuteFirst { request, codomain });
                            work = Work::Refute(Request::new(domain, request.goal, first));
                            continue 'drive;
                        }
                        d.closed_relation_refutation(
                            request.premise,
                            request.evidence,
                            request.goal,
                        )?
                    }
                    Work::Shape(request) => {
                        match (d.arena.get(request.premise), d.arena.get(request.goal)) {
                            (
                                Term::Id { ty, left, right },
                                Term::Id {
                                    ty: goal_ty,
                                    left: goal_left,
                                    right: goal_right,
                                },
                            ) => {
                                if d.arena.structurally_equal(ty, goal_ty)
                                    && d.arena.structurally_equal(goal_left, right)
                                    && d.arena.structurally_equal(goal_right, left)
                                {
                                    Some(d.symmetry(ty, left, right, request.evidence))
                                } else {
                                    d.boolean_identity_transport(
                                        request.premise,
                                        request.goal,
                                        request.evidence,
                                    )?
                                }
                            }
                            (Term::Id { ty, .. }, _) if d.arena.structurally_equal(ty, d.two) => d
                                .boolean_goal_transport(
                                    request.premise,
                                    request.goal,
                                    request.evidence,
                                )?,
                            (
                                Term::Pi { domain, codomain },
                                Term::Pi {
                                    domain: goal_domain,
                                    codomain: goal_codomain,
                                },
                            ) => {
                                if d.occurs_free(codomain, 0) || d.occurs_free(goal_codomain, 0) {
                                    None
                                } else {
                                    let hypothesis = d.arena.insert(Term::Variable(0));
                                    pending.push(Pending::FunctionArgument {
                                        evidence: request.evidence,
                                        codomain,
                                        goal_domain,
                                        goal_codomain,
                                    });
                                    work =
                                        Work::Coerce(Request::new(goal_domain, domain, hypothesis));
                                    continue 'drive;
                                }
                            }
                            (
                                Term::Sigma { domain, codomain },
                                Term::Sigma {
                                    domain: goal_domain,
                                    codomain: goal_codomain,
                                },
                            ) => {
                                let fst = d.arena.insert(Term::Fst {
                                    pair: request.evidence,
                                });
                                pending.push(Pending::ProductFirst {
                                    request,
                                    domain,
                                    codomain,
                                    goal_domain,
                                    goal_codomain,
                                });
                                work = Work::Coerce(Request::new(domain, goal_domain, fst));
                                continue 'drive;
                            }
                            _ => None,
                        }
                    }
                    Work::Shuffle(request) => {
                        if let Term::Sigma { domain, codomain } = d.arena.get(request.premise)
                            && !d.occurs_free(codomain, 0)
                        {
                            let projection = d.arena.insert(Term::Fst {
                                pair: request.evidence,
                            });
                            pending.push(Pending::ShuffleFirst { request, codomain });
                            work = Work::Coerce(Request::new(domain, request.goal, projection));
                        } else {
                            work = Work::ShuffleRest(request);
                        }
                        continue 'drive;
                    }
                    Work::ShuffleRest(request) => {
                        if let Some((motive, zero, one)) = d.tagged_sum_parts(request.premise) {
                            work = Work::Cases(elimination_cases(d, request, motive, zero, one));
                        } else {
                            work = Work::ShuffleGoal(request);
                        }
                        continue 'drive;
                    }
                    Work::ShuffleGoal(request) => {
                        if let Term::Sigma { domain, codomain } = d.arena.get(request.goal)
                            && !d.occurs_free(codomain, 0)
                        {
                            pending.push(Pending::GoalFirst { request, codomain });
                            work = Work::Coerce(Request::new(
                                request.premise,
                                domain,
                                request.evidence,
                            ));
                            continue 'drive;
                        }
                        if let Some((_, zero, one)) = d.tagged_sum_parts(request.goal) {
                            pending.push(Pending::TagZero { request, one });
                            work =
                                Work::Coerce(Request::new(request.premise, zero, request.evidence));
                            continue 'drive;
                        }
                        None
                    }
                    Work::Cases(cases) => {
                        let hypothesis = d.arena.insert(Term::Variable(0));
                        pending.push(Pending::CaseZero(cases));
                        work = Work::Coerce(Request::new(cases.zero, cases.goal_zero, hypothesis));
                        continue 'drive;
                    }
                };
                loop {
                    match pending.pop() {
                        Some(Pending::Fallback(request)) => {
                            if value.is_none() {
                                work = Work::Shuffle(request);
                                continue 'drive;
                            }
                        }
                        Some(Pending::Refutation(request)) => {
                            if value.is_none() {
                                work = Work::Shape(request);
                                continue 'drive;
                            }
                        }
                        Some(Pending::RefuteFirst { request, codomain }) => {
                            if value.is_none() {
                                let second = d.arena.insert(Term::Snd {
                                    pair: request.evidence,
                                });
                                work = Work::Refute(Request::new(codomain, request.goal, second));
                                continue 'drive;
                            }
                        }
                        Some(Pending::FunctionArgument {
                            evidence,
                            codomain,
                            goal_domain,
                            goal_codomain,
                        }) => {
                            if let Some(argument) = value {
                                let function = shift(&mut d.arena, evidence, 0, 1);
                                let applied = d.arena.insert(Term::Apply { function, argument });
                                pending.push(Pending::FunctionBody {
                                    domain: goal_domain,
                                });
                                work = Work::Coerce(Request::new(codomain, goal_codomain, applied));
                                continue 'drive;
                            }
                        }
                        Some(Pending::FunctionBody { domain }) => {
                            value = value.map(|body| d.arena.insert(Term::Lambda { domain, body }));
                        }
                        Some(Pending::ProductFirst {
                            request,
                            domain,
                            codomain,
                            goal_domain,
                            goal_codomain,
                        }) => {
                            if let Some(first) = value {
                                let cases =
                                    match (d.arena.get(codomain), d.arena.get(goal_codomain)) {
                                        (
                                            Term::CaseTwo {
                                                motive,
                                                zero_branch: zero,
                                                one_branch: one,
                                                scrutinee,
                                            },
                                            Term::CaseTwo {
                                                motive: goal_motive,
                                                zero_branch: goal_zero,
                                                one_branch: goal_one,
                                                scrutinee: goal_scrutinee,
                                            },
                                        ) if d.arena.structurally_equal(motive, goal_motive)
                                            && d.arena.structurally_equal(domain, goal_domain)
                                            && matches!(
                                                d.arena.get(scrutinee),
                                                Term::Variable(0)
                                            )
                                            && matches!(
                                                d.arena.get(goal_scrutinee),
                                                Term::Variable(0)
                                            )
                                            && [motive, zero, one, goal_zero, goal_one]
                                                .into_iter()
                                                .all(|payload| !d.occurs_free(payload, 0)) =>
                                        {
                                            Some(selected_cases(
                                                d,
                                                motive,
                                                zero,
                                                one,
                                                goal_zero,
                                                goal_one,
                                                request.evidence,
                                            ))
                                        }
                                        _ => None,
                                    };
                                if let Some(cases) = cases {
                                    pending.push(Pending::PairSecond { first });
                                    work = Work::Cases(cases);
                                    continue 'drive;
                                }
                                if !d.occurs_free(codomain, 0) && !d.occurs_free(goal_codomain, 0) {
                                    let snd = d.arena.insert(Term::Snd {
                                        pair: request.evidence,
                                    });
                                    pending.push(Pending::PairSecond { first });
                                    work = Work::Coerce(Request::new(codomain, goal_codomain, snd));
                                    continue 'drive;
                                }
                                value = None;
                            }
                        }
                        Some(Pending::PairSecond { first }) => {
                            value =
                                value.map(|second| d.arena.insert(Term::Pair { first, second }));
                        }
                        Some(Pending::CaseZero(cases)) => {
                            if let Some(coerced) = value {
                                let zero = d.arena.insert(Term::Lambda {
                                    domain: cases.zero,
                                    body: coerced,
                                });
                                let hypothesis = d.arena.insert(Term::Variable(0));
                                pending.push(Pending::CaseOne { cases, zero });
                                work = Work::Coerce(Request::new(
                                    cases.one,
                                    cases.goal_one,
                                    hypothesis,
                                ));
                                continue 'drive;
                            }
                        }
                        Some(Pending::CaseOne { cases, zero }) => {
                            if let Some(coerced) = value {
                                let one = d.arena.insert(Term::Lambda {
                                    domain: cases.one,
                                    body: coerced,
                                });
                                let scrutinee = d.arena.insert(Term::Fst {
                                    pair: cases.evidence,
                                });
                                let eliminator = d.arena.insert(Term::CaseTwo {
                                    motive: cases.motive,
                                    zero_branch: zero,
                                    one_branch: one,
                                    scrutinee,
                                });
                                let argument = d.arena.insert(Term::Snd {
                                    pair: cases.evidence,
                                });
                                value = Some(d.arena.insert(Term::Apply {
                                    function: eliminator,
                                    argument,
                                }));
                            }
                        }
                        Some(Pending::ShuffleFirst { request, codomain }) => {
                            if value.is_none() {
                                let projection = d.arena.insert(Term::Snd {
                                    pair: request.evidence,
                                });
                                pending.push(Pending::ShuffleSecond(request));
                                work =
                                    Work::Coerce(Request::new(codomain, request.goal, projection));
                                continue 'drive;
                            }
                        }
                        Some(Pending::ShuffleSecond(request)) => {
                            if value.is_none() {
                                work = Work::ShuffleRest(request);
                                continue 'drive;
                            }
                        }
                        Some(Pending::GoalFirst { request, codomain }) => {
                            if let Some(first) = value {
                                pending.push(Pending::PairSecond { first });
                                work = Work::Coerce(Request::new(
                                    request.premise,
                                    codomain,
                                    request.evidence,
                                ));
                                continue 'drive;
                            }
                        }
                        Some(Pending::TagZero { request, one }) => {
                            if let Some(payload) = value {
                                value = Some(d.arena.insert(Term::Pair {
                                    first: d.two_zero,
                                    second: payload,
                                }));
                            } else {
                                pending.push(Pending::TagOne);
                                work = Work::Coerce(Request::new(
                                    request.premise,
                                    one,
                                    request.evidence,
                                ));
                                continue 'drive;
                            }
                        }
                        Some(Pending::TagOne) => {
                            value = value.map(|payload| {
                                d.arena.insert(Term::Pair {
                                    first: d.two_one,
                                    second: payload,
                                })
                            });
                        }
                        None => return Ok(value),
                    }
                }
            }
        }

        fn selected_cases(
            d: &mut Denotation,
            motive: TermHandle,
            zero: TermHandle,
            one: TermHandle,
            goal_zero: TermHandle,
            goal_one: TermHandle,
            evidence: TermHandle,
        ) -> Cases {
            let tag = d.arena.insert(Term::Variable(0));
            let premise_family = d.arena.insert(Term::CaseTwo {
                motive,
                zero_branch: zero,
                one_branch: one,
                scrutinee: tag,
            });
            let shifted_tag = d.arena.insert(Term::Variable(1));
            let shifted_goal_zero = shift(&mut d.arena, goal_zero, 0, 1);
            let shifted_goal_one = shift(&mut d.arena, goal_one, 0, 1);
            let shifted_motive = shift(&mut d.arena, motive, 0, 1);
            let goal_family = d.arena.insert(Term::CaseTwo {
                motive: shifted_motive,
                zero_branch: shifted_goal_zero,
                one_branch: shifted_goal_one,
                scrutinee: shifted_tag,
            });
            let body = d.arena.insert(Term::Pi {
                domain: premise_family,
                codomain: goal_family,
            });
            let motive = d.arena.insert(Term::Lambda {
                domain: d.two,
                body,
            });
            Cases {
                motive,
                zero,
                one,
                goal_zero,
                goal_one,
                evidence,
            }
        }

        fn elimination_cases(
            d: &mut Denotation,
            request: Request,
            motive: TermHandle,
            zero: TermHandle,
            one: TermHandle,
        ) -> Cases {
            let tag = d.arena.insert(Term::Variable(0));
            let premise_family = d.arena.insert(Term::CaseTwo {
                motive,
                zero_branch: zero,
                one_branch: one,
                scrutinee: tag,
            });
            let shifted_goal = shift(&mut d.arena, request.goal, 0, 2);
            let body = d.arena.insert(Term::Pi {
                domain: premise_family,
                codomain: shifted_goal,
            });
            let motive = d.arena.insert(Term::Lambda {
                domain: d.two,
                body,
            });
            let goal = shift(&mut d.arena, request.goal, 0, 1);
            Cases {
                motive,
                zero,
                one,
                goal_zero: goal,
                goal_one: goal,
                evidence: request.evidence,
            }
        }
    }
    mod inhabitants {
        use super::*;

        enum Step {
            Ready(Option<TermHandle>),
            Product {
                domain: TermHandle,
                codomain: TermHandle,
            },
            Sum {
                zero: TermHandle,
                one: TermHandle,
            },
            Function {
                domain: TermHandle,
                codomain: TermHandle,
            },
        }

        enum Pending {
            First { second: TermHandle },
            Second { first: TermHandle },
            Zero { one: TermHandle },
            One,
            Body { domain: TermHandle },
        }

        pub(super) fn construct(
            denotation: &mut Denotation,
            goal: TermHandle,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            let mut pending = Vec::new();
            let mut current = goal;
            'descend: loop {
                let mut value = match step(denotation, current)? {
                    Step::Ready(value) => value,
                    Step::Product { domain, codomain } => {
                        pending.push(Pending::First { second: codomain });
                        current = domain;
                        continue 'descend;
                    }
                    Step::Sum { zero, one } => {
                        if !denotation.occurs_free(zero, 0) {
                            pending.push(Pending::Zero { one });
                            current = zero;
                            continue 'descend;
                        }
                        if !denotation.occurs_free(one, 0) {
                            pending.push(Pending::One);
                            current = one;
                            continue 'descend;
                        }
                        None
                    }
                    Step::Function { domain, codomain } => {
                        pending.push(Pending::Body { domain });
                        current = codomain;
                        continue 'descend;
                    }
                };
                loop {
                    match pending.pop() {
                        Some(Pending::First { second }) => {
                            if let Some(first) = value {
                                pending.push(Pending::Second { first });
                                current = second;
                                continue 'descend;
                            }
                        }
                        Some(Pending::Second { first }) => {
                            value = value.map(|second| {
                                denotation.arena.insert(Term::Pair { first, second })
                            });
                        }
                        Some(Pending::Zero { one }) => {
                            if let Some(payload) = value {
                                value = Some(denotation.arena.insert(Term::Pair {
                                    first: denotation.two_zero,
                                    second: payload,
                                }));
                            } else if !denotation.occurs_free(one, 0) {
                                pending.push(Pending::One);
                                current = one;
                                continue 'descend;
                            }
                        }
                        Some(Pending::One) => {
                            value = value.map(|payload| {
                                denotation.arena.insert(Term::Pair {
                                    first: denotation.two_one,
                                    second: payload,
                                })
                            });
                        }
                        Some(Pending::Body { domain }) => {
                            value = value
                                .map(|body| denotation.arena.insert(Term::Lambda { domain, body }));
                        }
                        None => return Ok(value),
                    }
                }
            }
        }

        fn step(
            denotation: &mut Denotation,
            goal: TermHandle,
        ) -> Result<Step, BoundedDenotationError> {
            if let Some((ty, left, right)) = denotation.identity_parts(goal) {
                return Ok(Step::Ready(
                    if denotation.arena.structurally_equal(left, right) {
                        Some(denotation.arena.insert(Term::Refl { ty, value: left }))
                    } else {
                        None
                    },
                ));
            }
            match denotation.integer_relation(goal) {
                Some(IntegerRelation::LessThan { left, right }) => {
                    let (Some(lower), Some(upper)) = (
                        denotation.closed_math_value(left),
                        denotation.closed_math_value(right),
                    ) else {
                        return Ok(Step::Ready(None));
                    };
                    if lower >= upper {
                        return Ok(Step::Ready(None));
                    }
                    let (Some(lower), Some(upper)) =
                        (integer_value_of(&lower), integer_value_of(&upper))
                    else {
                        return Ok(Step::Ready(None));
                    };
                    return denotation
                        .literal_order(lower, upper)
                        .map(|term| Step::Ready(Some(term)));
                }
                Some(IntegerRelation::LessOrEqual { left, right }) => {
                    let (Some(lower), Some(upper)) = (
                        denotation.closed_math_value(left),
                        denotation.closed_math_value(right),
                    ) else {
                        return Ok(Step::Ready(None));
                    };
                    if lower > upper {
                        return Ok(Step::Ready(None));
                    }
                    let evidence = if lower == upper {
                        let integer = denotation.integer_constant()?;
                        let reflexive = denotation.arena.insert(Term::Refl {
                            ty: integer,
                            value: left,
                        });
                        denotation.integer_law_application(
                            IntegerLaw::EqualityToLessOrEqual,
                            &[left, right, reflexive],
                        )?
                    } else {
                        let (Some(lower), Some(upper)) =
                            (integer_value_of(&lower), integer_value_of(&upper))
                        else {
                            return Ok(Step::Ready(None));
                        };
                        let strict = denotation.literal_order(lower, upper)?;
                        denotation.integer_law_application(
                            IntegerLaw::LessThanToLessOrEqual,
                            &[left, right, strict],
                        )?
                    };
                    return Ok(Step::Ready(Some(evidence)));
                }
                _ => {}
            }
            Ok(match denotation.arena.get(goal) {
                Term::Sigma { domain, codomain } => {
                    if !denotation.occurs_free(codomain, 0) {
                        Step::Product { domain, codomain }
                    } else if denotation.arena.structurally_equal(domain, denotation.two) {
                        match denotation.arena.get(codomain) {
                            Term::CaseTwo {
                                zero_branch,
                                one_branch,
                                scrutinee,
                                ..
                            } if matches!(denotation.arena.get(scrutinee), Term::Variable(0)) => {
                                Step::Sum {
                                    zero: zero_branch,
                                    one: one_branch,
                                }
                            }
                            _ => Step::Ready(None),
                        }
                    } else {
                        Step::Ready(None)
                    }
                }
                Term::Pi { domain, codomain } if !denotation.occurs_free(codomain, 0) => {
                    Step::Function { domain, codomain }
                }
                _ => Step::Ready(None),
            })
        }
    }

    #[derive(Clone, Copy)]
    struct Equation {
        carrier: TermHandle,
        from: TermHandle,
        to: TermHandle,
        evidence: TermHandle,
    }

    struct Replacement {
        equation: Equation,
        context: TermHandle,
    }

    impl Denotation {
        pub(super) fn source_rewrite_proof(
            &mut self,
            goal: TermHandle,
            identities: &[(TermHandle, TermHandle)],
            solve: impl FnOnce(
                &mut Self,
                TermHandle,
            ) -> Result<Option<TermHandle>, BoundedDenotationError>,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            let integer = self.integer_constant()?;
            let mut equations = Vec::new();
            for &(statement, evidence) in identities {
                let Some((carrier, left, right)) = self.identity_parts(statement) else {
                    continue;
                };
                if !self.arena.structurally_equal(carrier, integer) {
                    continue;
                }
                let parameter = |term| match self.arena.get(term) {
                    Term::Constant { declaration, .. } => self
                        .declarations
                        .get(declaration as usize)
                        .is_some_and(|declaration| declaration.body.is_none()),
                    _ => false,
                };
                let (from, to, evidence) = if parameter(left) {
                    (left, right, evidence)
                } else if parameter(right) {
                    (right, left, self.symmetry(carrier, left, right, evidence))
                } else {
                    continue;
                };
                if from != to
                    && !equations
                        .iter()
                        .any(|equation: &Equation| equation.from == from)
                {
                    equations.push(Equation {
                        carrier,
                        from,
                        to,
                        evidence,
                    });
                }
            }
            let mut remaining = MAX_ELABORATION_NODES;
            let Some((normal, replacements)) =
                self.expand_equations(goal, &equations, &mut remaining)?
            else {
                return Ok(None);
            };
            let Some(mut evidence) = solve(self, normal)? else {
                return Ok(None);
            };
            for replacement in replacements.into_iter().rev() {
                evidence = self.replace_evidence(replacement, evidence, true);
            }
            Ok(Some(evidence))
        }

        pub(super) fn compositional_transport(
            &mut self,
            premise: &Proposition,
            mut evidence: TermHandle,
            equalities: &[ProofNode],
            equality_evidence: &[TermHandle],
            conclusion: &Proposition,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            let truth_premise = matches!(premise, Proposition::Truth);
            let premise = self.denote(premise)?;
            let goal = self.denote(conclusion)?;
            if self.arena.structurally_equal(premise, goal) {
                return Ok(Some(evidence));
            }
            let mut equations = Vec::new();
            for (equality, &proof) in equalities.iter().zip(equality_evidence) {
                let Proposition::Equal(left, _) = &equality.conclusion else {
                    return Ok(None);
                };
                let reflexive = self.denote(&Proposition::Equal(left.clone(), left.clone()))?;
                let identity = self.denote(&equality.conclusion)?;
                let (Some((_, from, _)), Some((carrier, first, second))) = (
                    self.identity_parts(reflexive),
                    self.identity_parts(identity),
                ) else {
                    return Ok(None);
                };
                let (to, proof) = if self.arena.structurally_equal(first, from) {
                    (second, proof)
                } else if self.arena.structurally_equal(second, from) {
                    (first, self.symmetry(carrier, first, second, proof))
                } else {
                    return Ok(None);
                };
                if self.arena.structurally_equal(from, to)
                    || equations.iter().any(|prior: &Equation| prior.from == from)
                {
                    continue;
                }
                if !matches!(self.arena.get(from), Term::Constant { .. }) {
                    return Ok(None);
                }
                equations.push(Equation {
                    carrier,
                    from,
                    to,
                    evidence: proof,
                });
            }
            let mut remaining = MAX_ELABORATION_NODES;
            let Some((normal_premise, forward)) =
                self.expand_equations(premise, &equations, &mut remaining)?
            else {
                return Ok(None);
            };
            let Some((normal_goal, backward)) =
                self.expand_equations(goal, &equations, &mut remaining)?
            else {
                return Ok(None);
            };
            let reflexive_goal = self
                .identity_parts(normal_goal)
                .filter(|(_, left, right)| self.arena.structurally_equal(*left, *right));
            if truth_premise && let Some((ty, value, _)) = reflexive_goal {
                evidence = self.arena.insert(Term::Refl { ty, value });
            } else {
                for replacement in forward {
                    evidence = self.replace_evidence(replacement, evidence, false);
                }
                if !self.arena.structurally_equal(normal_premise, normal_goal) {
                    let Some(oriented) =
                        self.oriented_evidence(normal_premise, normal_goal, evidence)?
                    else {
                        return Ok(None);
                    };
                    evidence = oriented;
                }
            }
            for replacement in backward.into_iter().rev() {
                evidence = self.replace_evidence(replacement, evidence, true);
            }
            Ok(Some(evidence))
        }

        fn expand_equations(
            &mut self,
            mut statement: TermHandle,
            equations: &[Equation],
            remaining: &mut u64,
        ) -> Result<Option<(TermHandle, Vec<Replacement>)>, BoundedDenotationError> {
            let mut replacements = Vec::new();
            for _ in 0..=equations.len() {
                let mut changed = false;
                for &equation in equations {
                    let Some((next, context)) =
                        self.replacement_context(statement, equation, 0, remaining)?
                    else {
                        return Ok(None);
                    };
                    if next != statement {
                        replacements.push(Replacement { equation, context });
                        statement = next;
                        changed = true;
                    }
                }
                if !changed {
                    return Ok(Some((statement, replacements)));
                }
            }
            Ok(None)
        }

        fn replace_evidence(
            &mut self,
            replacement: Replacement,
            evidence: TermHandle,
            reverse: bool,
        ) -> TermHandle {
            let Equation {
                carrier,
                from,
                to,
                evidence: equation,
            } = replacement.equation;
            let (from, to, equation) = if reverse {
                (to, from, self.symmetry(carrier, from, to, equation))
            } else {
                (from, to, equation)
            };
            let endpoint = self.arena.insert(Term::Variable(0));
            let identity = self.arena.insert(Term::Id {
                ty: carrier,
                left: from,
                right: endpoint,
            });
            let body = shift(&mut self.arena, replacement.context, 0, 1);
            let body = self.arena.insert(Term::Lambda {
                domain: identity,
                body,
            });
            let motive = self.arena.insert(Term::Lambda {
                domain: carrier,
                body,
            });
            self.arena.insert(Term::IdElim {
                motive,
                base: evidence,
                endpoint: to,
                proof: equation,
            })
        }

        fn replacement_context(
            &mut self,
            original: TermHandle,
            equation: Equation,
            depth: u32,
            remaining: &mut u64,
        ) -> Result<Option<(TermHandle, TermHandle)>, BoundedDenotationError> {
            let Some(rest) = remaining.checked_sub(1) else {
                return Ok(None);
            };
            *remaining = rest;
            if original == equation.from {
                let variable = self.arena.insert(Term::Variable(depth));
                return Ok(Some((equation.to, variable)));
            }
            macro_rules! child {
                ($term:expr, $depth:expr) => {
                    match self.replacement_context($term, equation, $depth, remaining)? {
                        Some(pair) => pair,
                        None => return Ok(None),
                    }
                };
            }
            let term = self.arena.get(original);
            let (rewritten, context) = match term.clone() {
                Term::Variable(position) => {
                    let shifted = if position >= depth {
                        position
                            .checked_add(1)
                            .ok_or(BoundedDenotationError::DepthLimitExceeded)?
                    } else {
                        position
                    };
                    let context = if shifted == position {
                        original
                    } else {
                        self.arena.insert(Term::Variable(shifted))
                    };
                    return Ok(Some((original, context)));
                }
                Term::Constant { .. }
                | Term::Sort(_)
                | Term::Two
                | Term::TwoZero
                | Term::TwoOne => {
                    return Ok(Some((original, original)));
                }
                Term::Apply { function, argument } => {
                    let (function, context_function) = child!(function, depth);
                    let (argument, context_argument) = child!(argument, depth);
                    (
                        Term::Apply { function, argument },
                        Term::Apply {
                            function: context_function,
                            argument: context_argument,
                        },
                    )
                }
                Term::Id { ty, left, right } => {
                    let (ty, context_ty) = child!(ty, depth);
                    let (left, context_left) = child!(left, depth);
                    let (right, context_right) = child!(right, depth);
                    (
                        Term::Id { ty, left, right },
                        Term::Id {
                            ty: context_ty,
                            left: context_left,
                            right: context_right,
                        },
                    )
                }
                Term::Pi { domain, codomain } | Term::Sigma { domain, codomain } => {
                    let (domain, context_domain) = child!(domain, depth);
                    let nested = depth
                        .checked_add(1)
                        .ok_or(BoundedDenotationError::DepthLimitExceeded)?;
                    let (codomain, context_codomain) = child!(codomain, nested);
                    if matches!(term, Term::Pi { .. }) {
                        (
                            Term::Pi { domain, codomain },
                            Term::Pi {
                                domain: context_domain,
                                codomain: context_codomain,
                            },
                        )
                    } else {
                        (
                            Term::Sigma { domain, codomain },
                            Term::Sigma {
                                domain: context_domain,
                                codomain: context_codomain,
                            },
                        )
                    }
                }
                Term::Lambda { domain, body } => {
                    let (domain, context_domain) = child!(domain, depth);
                    let nested = depth
                        .checked_add(1)
                        .ok_or(BoundedDenotationError::DepthLimitExceeded)?;
                    let (body, context_body) = child!(body, nested);
                    (
                        Term::Lambda { domain, body },
                        Term::Lambda {
                            domain: context_domain,
                            body: context_body,
                        },
                    )
                }
                Term::CaseTwo {
                    motive,
                    zero_branch,
                    one_branch,
                    scrutinee,
                } => {
                    let (motive, context_motive) = child!(motive, depth);
                    let (zero_branch, context_zero) = child!(zero_branch, depth);
                    let (one_branch, context_one) = child!(one_branch, depth);
                    let (scrutinee, context_scrutinee) = child!(scrutinee, depth);
                    (
                        Term::CaseTwo {
                            motive,
                            zero_branch,
                            one_branch,
                            scrutinee,
                        },
                        Term::CaseTwo {
                            motive: context_motive,
                            zero_branch: context_zero,
                            one_branch: context_one,
                            scrutinee: context_scrutinee,
                        },
                    )
                }
                _ => return Ok(None),
            };
            let rewritten = if rewritten == term {
                original
            } else {
                self.arena.insert(rewritten)
            };
            let context = if context == term {
                original
            } else {
                self.arena.insert(context)
            };
            Ok(Some((rewritten, context)))
        }

        pub(super) fn oriented_evidence(
            &mut self,
            premise: TermHandle,
            goal: TermHandle,
            evidence: TermHandle,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            if self.arena.structurally_equal(premise, goal) {
                return Ok(Some(evidence));
            }
            let mut remaining = MAX_ELABORATION_NODES;
            self.oriented_coercion(premise, goal, evidence, &mut remaining)
        }

        pub(super) fn closed_inhabitant(
            &mut self,
            goal: TermHandle,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            inhabitants::construct(self, goal)
        }

        fn closed_relation_refutation(
            &mut self,
            premise: TermHandle,
            evidence: TermHandle,
            goal: TermHandle,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            if let Some((ty, left, right)) = self.identity_parts(premise) {
                if self.arena.structurally_equal(ty, self.two) {
                    let premise_left_literal = if self.arena.structurally_equal(left, self.two_one)
                    {
                        true
                    } else if self.arena.structurally_equal(left, self.two_zero) {
                        false
                    } else {
                        return Ok(None);
                    };
                    let right_is_other = if premise_left_literal {
                        self.arena.structurally_equal(right, self.two_zero)
                    } else {
                        self.arena.structurally_equal(right, self.two_one)
                    };
                    if !right_is_other {
                        return Ok(None);
                    }
                    return Ok(Some(self.two_identity_discriminant(
                        left,
                        right,
                        goal,
                        premise_left_literal,
                        evidence,
                    )));
                }
                let (Some(left_value), Some(right_value)) =
                    (self.closed_math_value(left), self.closed_math_value(right))
                else {
                    return Ok(None);
                };
                if left_value == right_value {
                    return Ok(None);
                }
                let (low, high, low_value, high_value, equality) = if left_value < right_value {
                    (left, right, left_value, right_value, evidence)
                } else {
                    (
                        right,
                        left,
                        right_value,
                        left_value,
                        self.symmetry(ty, left, right, evidence),
                    )
                };
                let (Some(low_value), Some(high_value)) =
                    (integer_value_of(&low_value), integer_value_of(&high_value))
                else {
                    return Ok(None);
                };
                let strict = self.literal_order(low_value, high_value)?;
                let looped = self.integer_law_application(
                    IntegerLaw::LessThanSubstituteLeft,
                    &[low, high, high, equality, strict],
                )?;
                let absurd = self.irreflexive(high, looped)?;
                return Ok(Some(self.arena.insert(Term::EmptyElim {
                    ty: goal,
                    scrutinee: absurd,
                })));
            }
            match self.integer_relation(premise) {
                Some(IntegerRelation::LessThan { left, right }) => {
                    let (Some(left_value), Some(right_value)) =
                        (self.closed_math_value(left), self.closed_math_value(right))
                    else {
                        return Ok(None);
                    };
                    if left_value < right_value {
                        return Ok(None);
                    }
                    let absurd = if left_value == right_value {
                        self.irreflexive(left, evidence)?
                    } else {
                        let (Some(right_value), Some(left_value)) = (
                            integer_value_of(&right_value),
                            integer_value_of(&left_value),
                        ) else {
                            return Ok(None);
                        };
                        let backwards = self.literal_order(right_value, left_value)?;
                        let looped = self.integer_law_application(
                            IntegerLaw::LessThanTransitivity,
                            &[left, right, left, evidence, backwards],
                        )?;
                        self.irreflexive(left, looped)?
                    };
                    Ok(Some(self.arena.insert(Term::EmptyElim {
                        ty: goal,
                        scrutinee: absurd,
                    })))
                }
                Some(IntegerRelation::LessOrEqual { left, right }) => {
                    let (Some(left_value), Some(right_value)) =
                        (self.closed_math_value(left), self.closed_math_value(right))
                    else {
                        return Ok(None);
                    };
                    if left_value <= right_value {
                        return Ok(None);
                    }
                    let (Some(right_value), Some(left_value)) = (
                        integer_value_of(&right_value),
                        integer_value_of(&left_value),
                    ) else {
                        return Ok(None);
                    };
                    let backwards = self.literal_order(right_value, left_value)?;
                    let looped = self.integer_law_application(
                        IntegerLaw::LessOrEqualLessThanTransitivity,
                        &[left, right, left, evidence, backwards],
                    )?;
                    let absurd = self.irreflexive(left, looped)?;
                    Ok(Some(self.arena.insert(Term::EmptyElim {
                        ty: goal,
                        scrutinee: absurd,
                    })))
                }
                _ => Ok(None),
            }
        }

        fn oriented_coercion(
            &mut self,
            premise: TermHandle,
            goal: TermHandle,
            evidence: TermHandle,
            remaining: &mut u64,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            coercion::construct(self, premise, goal, evidence, remaining)
        }

        fn tagged_sum_parts(
            &self,
            term: TermHandle,
        ) -> Option<(TermHandle, TermHandle, TermHandle)> {
            let Term::Sigma { domain, codomain } = self.arena.get(term) else {
                return None;
            };
            if !self.arena.structurally_equal(domain, self.two) {
                return None;
            }
            let Term::CaseTwo {
                motive,
                zero_branch,
                one_branch,
                scrutinee,
            } = self.arena.get(codomain)
            else {
                return None;
            };
            if !matches!(self.arena.get(scrutinee), Term::Variable(0)) {
                return None;
            }
            if [motive, zero_branch, one_branch]
                .into_iter()
                .any(|payload| self.occurs_free(payload, 0))
            {
                return None;
            }
            Some((motive, zero_branch, one_branch))
        }

        pub(super) fn occurs_free(&self, term: TermHandle, index: u32) -> bool {
            let mut pending = vec![(term, index)];
            while let Some((term, index)) = pending.pop() {
                let node = self.arena.node(term);
                if let Term::Variable(position) = node {
                    if *position == index {
                        return true;
                    }
                    continue;
                }
                for (child, binders) in node.subterms().iter().rev() {
                    if let Some(nested) = index.checked_add(binders) {
                        pending.push((child, nested));
                    }
                }
            }
            false
        }
    }

    #[cfg(test)]
    mod tests {
        use super::super::Elaboration;
        use crate::proof::assumption_proof;
        use crate::{
            Budget, ProofNode, ProofRule, proof_node, value_context,
            verify_mathematical_certificate,
        };
        use semantic_vocabulary::{
            IntegerSign, IntegerType, Proposition, ScalarTerm, ScalarType, ValueId,
        };
        use std::collections::BTreeSet;

        fn integer() -> IntegerType {
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
        }

        fn value(position: u64) -> ScalarTerm {
            ScalarTerm::value(
                ValueId::new(position).unwrap(),
                ScalarType::Integer(integer()),
            )
        }

        fn check(premise: Proposition, goal: Proposition, equations: &[(u64, u64)]) {
            let context = value_context(ScalarType::Integer(integer()), 8);
            let assumptions = [premise.clone()];
            let axioms: Vec<_> = equations
                .iter()
                .map(|&(left, right)| Proposition::Equal(value(left), value(right)))
                .collect();
            let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::ValueEqualityTransport {
            premise: Box::new(assumption_proof(premise, 0)),
            equalities: axioms
                .iter()
                .enumerate()
                .map(|(index, conclusion)| proof_node!(conclusion.clone() => ProofRule::SemanticAxiom { index }))
                .collect(),
        },
            };
            let parameters = BTreeSet::new();
            let mut elaboration =
                Elaboration::new(&context, &goal, &assumptions, &axioms, &parameters).unwrap();
            let evidence = elaboration.node(&proof).unwrap();
            assert!(
                elaboration.denotation.rule_axioms.is_empty(),
                "substitution must derive its conclusion"
            );
            let mut denoted = elaboration.finish(evidence);
            verify_mathematical_certificate(
                &mut denoted.arena,
                &denoted.certificate,
                &mut Budget::default(),
            )
            .unwrap();
            let mut missing = axioms.clone();
            missing[0] = Proposition::Truth;
            let mut invalid =
                Elaboration::new(&context, &goal, &assumptions, &missing, &parameters).unwrap();
            assert!(
                invalid.node(&proof).is_err(),
                "missing cited equation must reject"
            );
        }

        fn equal_value(left: u64, right: u64) -> Proposition {
            Proposition::Equal(value(left), value(right))
        }

        #[test]
        fn reversed_identity_nested_in_disjunction_orients_by_j() {
            check(
                Proposition::Disjunction(vec![equal_value(1, 2), equal_value(1, 3)]),
                Proposition::Disjunction(vec![equal_value(4, 2), equal_value(4, 3)]),
                &[(4, 1)],
            );
        }

        #[test]
        fn reversed_identity_nested_in_conjunction_orients_by_j() {
            let context = |identity: Proposition| {
                Proposition::Conjunction(vec![
                    Proposition::LessOrEqual(value(7), value(8)),
                    identity,
                ])
            };
            check(
                context(equal_value(1, 2)),
                context(equal_value(4, 2)),
                &[(4, 1)],
            );
        }

        #[test]
        fn nested_identity_requiring_unlicensed_reversal_rejects() {
            let premise = Proposition::Implication {
                premise: Box::new(equal_value(1, 2)),
                conclusion: Box::new(equal_value(1, 3)),
            };
            let goal = Proposition::Implication {
                premise: Box::new(equal_value(2, 4)),
                conclusion: Box::new(equal_value(2, 3)),
            };
            let axioms = [equal_value(4, 1)];
            let context = value_context(ScalarType::Integer(integer()), 4);
            let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::ValueEqualityTransport {
            premise: Box::new(assumption_proof(premise.clone(), 0)),
            equalities: axioms
                .iter()
                .enumerate()
                .map(|(index, conclusion)| proof_node!(conclusion.clone() => ProofRule::SemanticAxiom { index }))
                .collect(),
        },
            };
            let assumptions = [premise];
            let parameters = BTreeSet::new();
            let mut elaboration =
                Elaboration::new(&context, &goal, &assumptions, &axioms, &parameters).unwrap();
            assert!(elaboration.node(&proof).is_err());
        }

        #[test]
        fn deep_oriented_refutation_uses_default_stack_and_keeps_true_premises_open() {
            std::thread::spawn(|| {
        use super::Denotation;
        use crate::mathematical_core::{Context, Term, infer_sort};
        for (depth, refutable) in [(8192, true), (32, true), (32, false)] {
            let mut d = Denotation::new();
            let truth = d.denote(&Proposition::Truth).unwrap();
            let goal = d.denote(&Proposition::Falsehood).unwrap();
            let mut premise = if refutable { goal } else { truth };
            for _ in 0..depth {
                premise = d.arena.insert(Term::Sigma {
                    domain: premise,
                    codomain: truth,
                });
            }
            infer_sort(
                &mut d.arena,
                &Context::empty(),
                premise,
                &mut Budget::new(2_000_000),
            )
            .expect("the complete premise is independently formed");
            let evidence = d.arena.insert(Term::Variable(0));
            eprintln!(
                "{depth}-node premise formed; begin oriented refutation refutable={refutable}"
            );
            let result = d.oriented_evidence(premise, goal, evidence).unwrap();
            eprintln!("oriented refutation returned");
            if refutable {
                let term = result.expect("the exact false conjunct refutes under its premise");
                let mut pending = vec![term];
                let mut visited = std::collections::HashSet::new();
                let mut projections = 0;
                while let Some(handle) = pending.pop() {
                    if !visited.insert(handle) {
                        continue;
                    }
                    let node = d.arena.get(handle);
                    if matches!(node, Term::Fst { .. }) {
                        projections += 1;
                    }
                    pending.extend(node.subterms().iter().map(|(child, _)| child));
                }
                assert_eq!(
                    projections, depth,
                    "the evidence spends the actual deepest premise"
                );
                if depth == 32 {
                    let certificate =
                        crate::MathematicalCertificate::closed(vec![premise], term, goal);
                    verify_mathematical_certificate(
                        &mut d.arena,
                        &certificate,
                        &mut Budget::new(2_000_000),
                    )
                    .expect("the independent kernel checks the full bounded refutation");
                }
            } else {
                assert!(
                    result.is_none(),
                    "closed true premises do not prove Falsehood"
                );
            }
            assert!(
                d.declarations.is_empty(),
                "the search does not invent a decision axiom"
            );
        }
            })
            .join()
            .expect("oriented evidence construction stays on a default stack");
        }

        #[test]
        fn oriented_search_shares_allowance_across_failed_alternatives() {
            use super::Denotation;
            use crate::mathematical_core::Term;
            for allowance in 0..=3 {
                let mut d = Denotation::new();
                let first = equal_value(1, 2);
                let last = equal_value(1, 3);
                let premise = d
                    .denote(&Proposition::Conjunction(vec![first, last.clone()]))
                    .unwrap();
                let goal = d.denote(&last).unwrap();
                let evidence = d.arena.insert(Term::Variable(0));
                let declarations = d.declarations.len();
                let mut remaining = allowance;
                let result = d
                    .oriented_coercion(premise, goal, evidence, &mut remaining)
                    .unwrap();
                assert_eq!(
                    remaining, 0,
                    "the root and failed first alternative consume the same allowance"
                );
                if allowance == 3 {
                    let term = result.expect(
                        "the second projection is reached after the failed first alternative",
                    );
                    assert!(matches!(d.arena.get(term), Term::Snd { pair } if pair == evidence));
                    let certificate = crate::MathematicalCertificate::with_signature(
                        d.declarations.clone(),
                        vec![premise],
                        term,
                        goal,
                    );
                    verify_mathematical_certificate(
                        &mut d.arena,
                        &certificate,
                        &mut Budget::new(2_000_000),
                    )
                    .unwrap();
                } else {
                    assert!(
                        result.is_none(),
                        "a declined first alternative cannot refund its allowance"
                    );
                }
                assert_eq!(declarations, d.declarations.len());
                assert!(
                    d.rule_axioms.is_empty(),
                    "exhaustion cannot invent a decision axiom"
                );
            }
        }
    }
}
use denotation::Denotation;
mod elaboration {
    use super::*;
    use equality_rules::predicate_denotation_relation;
    use integer_order_rules::math_order_translation;
    use integer_order_rules::order_antisymmetry;

    pub(super) struct Elaboration<'a> {
        pub(super) denotation: Denotation,
        context: &'a PropositionContext,
        ambient_assumptions: &'a [Proposition],
        axioms: &'a [Proposition],
        machine_parameter_values: &'a BTreeSet<ValueId>,
        ambient: usize,
        context_bindings: Vec<TermHandle>,
        expected: TermHandle,
        roster: Vec<Proposition>,
        positions: Vec<u32>,
        depth: u32,
        steps: u64,
        rules: BTreeSet<AcceptedProofRule>,
        assumptions: PremiseRoster,
        semantic_axioms: PremiseRoster,
    }

    macro_rules! certificate {
        ($error:expr) => {
            BoundedDenotationError::Certificate($error)
        };
    }

    mod walk {
        use super::*;

        pub(super) struct ChildEvidence<'proof> {
            values: std::vec::IntoIter<ChildTerm<'proof>>,
            premises: Option<Vec<Proposition>>,
        }

        struct ChildTerm<'proof> {
            source: &'proof ProofNode,
            term: TermHandle,
        }

        impl ChildEvidence<'_> {
            pub(super) fn take(&mut self, expected: &ProofNode) -> TermHandle {
                let child = self
                    .values
                    .next()
                    .expect("one elaborated child in source order");
                assert!(
                    std::ptr::eq(child.source, expected),
                    "child evidence belongs to the exact scheduled premise"
                );
                child.term
            }

            pub(super) fn take_prepared_premises(&mut self) -> Vec<Proposition> {
                self.premises
                    .take()
                    .expect("transport premise statements were prepared before their children")
            }

            fn finish(mut self) {
                assert!(
                    self.values.next().is_none(),
                    "all scheduled child evidence was consumed"
                );
                assert!(
                    self.premises.is_none(),
                    "all prepared transport statements were consumed"
                );
            }
        }

        struct Children<'proof> {
            fixed: [Option<&'proof ProofNode>; 3],
            remaining: &'proof [ProofNode],
        }

        impl<'proof> Children<'proof> {
            fn fixed(nodes: [Option<&'proof ProofNode>; 3]) -> Self {
                Self {
                    fixed: nodes,
                    remaining: &[],
                }
            }

            fn slice(nodes: &'proof [ProofNode]) -> Self {
                Self {
                    fixed: [None; 3],
                    remaining: nodes,
                }
            }

            fn len(&self) -> usize {
                self.fixed.iter().filter(|node| node.is_some()).count() + self.remaining.len()
            }

            fn next(&mut self) -> Option<&'proof ProofNode> {
                for node in &mut self.fixed {
                    if let Some(node) = node.take() {
                        return Some(node);
                    }
                }
                let (next, remaining) = self.remaining.split_first()?;
                self.remaining = remaining;
                Some(next)
            }
        }

        struct PartialNode<'proof> {
            proof: &'proof ProofNode,
            children: Children<'proof>,
            awaiting: Option<&'proof ProofNode>,
            evidence: Vec<ChildTerm<'proof>>,
            premises: Option<Vec<Proposition>>,
        }

        impl<'proof> PartialNode<'proof> {
            fn new(proof: &'proof ProofNode) -> Result<Self, BoundedDenotationError> {
                use ProofRule::*;
                let children = match &proof.rule {
                    Primitive(_)
                    | Assumption { index: _ }
                    | SemanticAxiom { index: _ }
                    | IntegerCorrelatedForbiddenRoots { witness: _ } => Children::fixed([None; 3]),
                    ConjunctionIntroduction(children) => {
                        let Proposition::Conjunction(expected) = &proof.conclusion else {
                            return Err(certificate!(ProofError::RuleConclusionMismatch(
                                "conjunction introduction"
                            )));
                        };
                        if children.len() != expected.len() {
                            return Err(certificate!(ProofError::ConjunctionArityMismatch));
                        }
                        Children::slice(children)
                    }
                    ConjunctionElimination {
                        conjunction,
                        conjunct: _,
                    } => Children::fixed([Some(conjunction), None, None]),
                    DisjunctionIntroduction { disjunct, index } => {
                        let Proposition::Disjunction(disjuncts) = &proof.conclusion else {
                            return Err(certificate!(ProofError::RuleConclusionMismatch(
                                "disjunction introduction"
                            )));
                        };
                        if *index >= disjuncts.len() {
                            return Err(certificate!(ProofError::UnknownDisjunct(*index)));
                        }
                        Children::fixed([Some(disjunct), None, None])
                    }
                    ImplicationElimination {
                        implication,
                        premise,
                    } => Children::fixed([Some(implication), Some(premise), None]),
                    EqualitySymmetry { equality } => Children::fixed([Some(equality), None, None]),
                    EqualityTransitivity {
                        left_equals_middle,
                        middle_equals_right,
                    } => {
                        Children::fixed([Some(left_equals_middle), Some(middle_equals_right), None])
                    }
                    PredicateDenotation { premise } => Children::fixed([Some(premise), None, None]),
                    ValueEqualityTransport {
                        premise,
                        equalities,
                    } => Children {
                        fixed: [Some(premise), None, None],
                        remaining: equalities,
                    },
                    IntegerSubtractOrder {
                        difference,
                        positive,
                    } => Children::fixed([Some(difference), Some(positive), None]),
                    IntegerAddOrder { sum, positive } => {
                        Children::fixed([Some(sum), Some(positive), None])
                    }
                    IntegerSubtractAntitone {
                        smaller,
                        larger,
                        order,
                    } => Children::fixed([Some(smaller), Some(larger), Some(order)]),
                    IntegerOrderDiscreteness { relation } | IntegerOrderWeakening { relation } => {
                        Children::fixed([Some(relation), None, None])
                    }
                    IntegerLessOrEqualTransitivity {
                        left_less_or_equal_middle,
                        middle_less_or_equal_right,
                    } => Children::fixed([
                        Some(left_less_or_equal_middle),
                        Some(middle_less_or_equal_right),
                        None,
                    ]),
                    IntegerStrictOrderTransitivity {
                        left_to_middle,
                        middle_to_right,
                    } => Children::fixed([Some(left_to_middle), Some(middle_to_right), None]),
                    IntegerOrderAntisymmetry { forward, backward } => {
                        Children::fixed([Some(forward), Some(backward), None])
                    }
                    IntegerMathOrderTranslation {
                        relation,
                        addend: _,
                    } => Children::fixed([Some(relation), None, None]),
                    IntegerOrderSubstitution {
                        relation,
                        equality,
                        endpoint: _,
                    } => Children::fixed([Some(relation), Some(equality), None]),
                    IntegerAffineBound {
                        root_bound,
                        witness: _,
                    }
                    | IntegerCastBound {
                        root_bound,
                        witness: _,
                    } => Children::fixed([Some(root_bound), None, None]),
                    IntegerExactAddDefinitionBound {
                        left_bound,
                        right_bound,
                        definition_axiom: _,
                    }
                    | IntegerExactSubtractDefinitionBound {
                        left_bound,
                        right_bound,
                        definition_axiom: _,
                    } => Children::fixed([Some(left_bound), Some(right_bound), None]),
                    ImplicationIntroduction { body: _ }
                    | DisjunctionElimination {
                        disjunction: _,
                        branches: _,
                    } => {
                        unreachable!("scoped rules have their own continuation frames")
                    }
                };
                let count = children.len();
                let premises = matches!(&proof.rule, ValueEqualityTransport { .. })
                    .then(|| Vec::with_capacity(count));
                Ok(Self {
                    proof,
                    children,
                    awaiting: None,
                    evidence: Vec::with_capacity(count),
                    premises,
                })
            }

            fn next_child(&mut self) -> Option<&'proof ProofNode> {
                let child = self.children.next()?;
                if let Some(premises) = &mut self.premises {
                    premises.push(child.conclusion.clone());
                }
                self.awaiting = Some(child);
                Some(child)
            }

            fn finish(
                self,
                elaboration: &mut Elaboration<'_>,
            ) -> Result<TermHandle, BoundedDenotationError> {
                let mut operands = ChildEvidence {
                    values: self.evidence.into_iter(),
                    premises: self.premises,
                };
                let term = elaboration.finish_regular(self.proof, &mut operands)?;
                operands.finish();
                Ok(term)
            }
        }

        #[derive(Clone, Copy)]
        struct Scope {
            depth: u32,
            roster: usize,
            positions: usize,
        }

        impl Scope {
            fn current(elaboration: &Elaboration<'_>) -> Self {
                Self {
                    depth: elaboration.depth,
                    roster: elaboration.roster.len(),
                    positions: elaboration.positions.len(),
                }
            }

            fn restore(self, elaboration: &mut Elaboration<'_>) {
                elaboration.depth = self.depth;
                elaboration.positions.truncate(self.positions);
                elaboration.roster.truncate(self.roster);
            }

            fn close_hypothesis(self, elaboration: &mut Elaboration<'_>) {
                assert_eq!(elaboration.depth, self.depth + 1);
                assert_eq!(elaboration.roster.len(), self.roster + 1);
                assert_eq!(elaboration.positions.len(), self.positions + 1);
                self.restore(elaboration);
            }

            fn close_intermediate(self, elaboration: &mut Elaboration<'_>) {
                assert_eq!(elaboration.depth, self.depth + 1);
                assert_eq!(elaboration.roster.len(), self.roster);
                assert_eq!(elaboration.positions.len(), self.positions);
                self.restore(elaboration);
            }

            fn assert_current(self, elaboration: &Elaboration<'_>) {
                assert_eq!(elaboration.depth, self.depth);
                assert_eq!(elaboration.roster.len(), self.roster);
                assert_eq!(elaboration.positions.len(), self.positions);
            }
        }

        fn hypothesis(elaboration: &mut Elaboration<'_>, proposition: &Proposition) -> Scope {
            let prefix = Scope::current(elaboration);
            elaboration.roster.push(proposition.clone());
            elaboration.positions.push(elaboration.depth);
            elaboration.depth += 1;
            prefix
        }

        #[derive(Clone, Copy)]
        struct Case<'proof> {
            disjuncts: &'proof [Proposition],
            branches: &'proof [ProofNode],
            scrutinee: TermHandle,
            conclusion: TermHandle,
        }

        #[derive(Clone, Copy)]
        struct CaseParts<'proof> {
            case: Case<'proof>,
            first: TermHandle,
            rest: TermHandle,
            zero: TermHandle,
        }

        enum Work<'proof> {
            Node(&'proof ProofNode),
            Case(Case<'proof>),
        }

        enum Pending<'proof> {
            Children(PartialNode<'proof>),
            Implication {
                proof: &'proof ProofNode,
                body: &'proof ProofNode,
                domain: TermHandle,
                scope: Scope,
            },
            Disjunction {
                proof: &'proof ProofNode,
                disjunction: &'proof ProofNode,
                branches: &'proof [ProofNode],
            },
            DisjunctionDone {
                proof: &'proof ProofNode,
                branches: &'proof [ProofNode],
            },
            CaseZero {
                case: Case<'proof>,
                first: TermHandle,
                scope: Scope,
            },
            CaseOne {
                parts: CaseParts<'proof>,
                scope: Scope,
            },
            CaseNested {
                parts: CaseParts<'proof>,
                scope: Scope,
            },
        }

        pub(super) fn elaborate(
            elaboration: &mut Elaboration<'_>,
            proof: &ProofNode,
        ) -> Result<TermHandle, BoundedDenotationError> {
            let root = Scope::current(elaboration);
            let result = drive(elaboration, proof);
            match &result {
                Ok(_) => root.assert_current(elaboration),
                Err(_) => root.restore(elaboration),
            }
            result
        }

        fn enter(
            elaboration: &mut Elaboration<'_>,
            proof: &ProofNode,
        ) -> Result<(), BoundedDenotationError> {
            elaboration.steps += 1;
            if elaboration.steps > MAX_ELABORATION_NODES {
                return Err(BoundedDenotationError::DepthLimitExceeded);
            }
            elaboration
                .context
                .validate(&proof.conclusion)
                .map_err(|error| certificate!(ProofError::MalformedProposition(error)))
        }

        fn drive(
            elaboration: &mut Elaboration<'_>,
            proof: &ProofNode,
        ) -> Result<TermHandle, BoundedDenotationError> {
            let mut pending = Vec::new();
            let mut current = Work::Node(proof);
            'drive: loop {
                let mut value = match current {
                    Work::Node(proof) => {
                        enter(elaboration, proof)?;
                        match &proof.rule {
                            ProofRule::ImplicationIntroduction { body } => {
                                let Proposition::Implication {
                                    premise,
                                    conclusion: _,
                                } = &proof.conclusion
                                else {
                                    return Err(certificate!(ProofError::RuleConclusionMismatch(
                                        "implication introduction"
                                    )));
                                };
                                let domain = elaboration.denotation.denote(premise)?;
                                let scope = hypothesis(elaboration, premise);
                                pending.push(Pending::Implication {
                                    proof,
                                    body,
                                    domain,
                                    scope,
                                });
                                current = Work::Node(body);
                                continue 'drive;
                            }
                            ProofRule::DisjunctionElimination {
                                disjunction,
                                branches,
                            } => {
                                pending.push(Pending::Disjunction {
                                    proof,
                                    disjunction,
                                    branches,
                                });
                                current = Work::Node(disjunction);
                                continue 'drive;
                            }
                            _ => {
                                let mut node = PartialNode::new(proof)?;
                                if let Some(child) = node.next_child() {
                                    pending.push(Pending::Children(node));
                                    current = Work::Node(child);
                                    continue 'drive;
                                }
                                node.finish(elaboration)?
                            }
                        }
                    }
                    Work::Case(case) => {
                        assert!(case.disjuncts.len() >= 2);
                        assert_eq!(case.disjuncts.len(), case.branches.len());
                        let first = elaboration.denotation.denote(&case.disjuncts[0])?;
                        let scope = hypothesis(elaboration, &case.disjuncts[0]);
                        pending.push(Pending::CaseZero { case, first, scope });
                        current = Work::Node(&case.branches[0]);
                        continue 'drive;
                    }
                };
                loop {
                    match pending.pop() {
                        Some(Pending::Children(mut node)) => {
                            let source =
                                node.awaiting.take().expect("one scheduled child completed");
                            node.evidence.push(ChildTerm {
                                source,
                                term: value,
                            });
                            if let Some(child) = node.next_child() {
                                pending.push(Pending::Children(node));
                                current = Work::Node(child);
                                continue 'drive;
                            }
                            value = node.finish(elaboration)?;
                        }
                        Some(Pending::Implication {
                            proof,
                            body,
                            domain,
                            scope,
                        }) => {
                            scope.close_hypothesis(elaboration);
                            let term = elaboration.denotation.arena.insert(Term::Lambda {
                                domain,
                                body: value,
                            });
                            let Proposition::Implication {
                                premise: _,
                                conclusion,
                            } = &proof.conclusion
                            else {
                                unreachable!("the implication shape was admitted before the child")
                            };
                            if &body.conclusion != conclusion.as_ref() {
                                return Err(certificate!(
                                    ProofError::ImplicationConclusionMismatch
                                ));
                            }
                            elaboration
                                .rules
                                .insert(AcceptedProofRule::ImplicationIntroduction);
                            value = term;
                        }
                        Some(Pending::Disjunction {
                            proof,
                            disjunction,
                            branches,
                        }) => {
                            let Proposition::Disjunction(disjuncts) = &disjunction.conclusion
                            else {
                                return Err(certificate!(ProofError::RulePremiseMismatch(
                                    "disjunction elimination"
                                )));
                            };
                            if branches.len() != disjuncts.len() {
                                return Err(certificate!(ProofError::DisjunctionArityMismatch));
                            }
                            let conclusion = elaboration.denotation.denote(&proof.conclusion)?;
                            pending.push(Pending::DisjunctionDone { proof, branches });
                            current = Work::Case(Case {
                                disjuncts,
                                branches,
                                scrutinee: value,
                                conclusion,
                            });
                            continue 'drive;
                        }
                        Some(Pending::DisjunctionDone { proof, branches }) => {
                            for branch in branches {
                                if branch.conclusion != proof.conclusion {
                                    return Err(certificate!(
                                        ProofError::DisjunctionBranchConclusionMismatch
                                    ));
                                }
                            }
                            elaboration
                                .rules
                                .insert(AcceptedProofRule::DisjunctionElimination);
                        }
                        Some(Pending::CaseZero { case, first, scope }) => {
                            scope.close_hypothesis(elaboration);
                            let zero = elaboration.denotation.arena.insert(Term::Lambda {
                                domain: first,
                                body: value,
                            });
                            let rest = if case.disjuncts.len() == 2 {
                                elaboration.denotation.denote(&case.disjuncts[1])?
                            } else {
                                elaboration.denotation.disjunction(&case.disjuncts[1..])?
                            };
                            let parts = CaseParts {
                                case,
                                first,
                                rest,
                                zero,
                            };
                            if case.disjuncts.len() == 2 {
                                let scope = hypothesis(elaboration, &case.disjuncts[1]);
                                pending.push(Pending::CaseOne { parts, scope });
                                current = Work::Node(&case.branches[1]);
                            } else {
                                let scope = Scope::current(elaboration);
                                elaboration.depth += 1;
                                let bound = elaboration.denotation.arena.insert(Term::Variable(0));
                                pending.push(Pending::CaseNested { parts, scope });
                                current = Work::Case(Case {
                                    disjuncts: &case.disjuncts[1..],
                                    branches: &case.branches[1..],
                                    scrutinee: bound,
                                    conclusion: case.conclusion,
                                });
                            }
                            continue 'drive;
                        }
                        Some(Pending::CaseOne { parts, scope }) => {
                            scope.close_hypothesis(elaboration);
                            let one = elaboration.denotation.arena.insert(Term::Lambda {
                                domain: parts.rest,
                                body: value,
                            });
                            value = case_term(elaboration, parts, one);
                        }
                        Some(Pending::CaseNested { parts, scope }) => {
                            scope.close_intermediate(elaboration);
                            let one = elaboration.denotation.arena.insert(Term::Lambda {
                                domain: parts.rest,
                                body: value,
                            });
                            value = case_term(elaboration, parts, one);
                        }
                        None => return Ok(value),
                    }
                }
            }
        }

        fn case_term(
            elaboration: &mut Elaboration<'_>,
            parts: CaseParts<'_>,
            one: TermHandle,
        ) -> TermHandle {
            let inner_motive = elaboration.denotation.arena.insert(Term::Lambda {
                domain: elaboration.denotation.two,
                body: elaboration.denotation.type_zero,
            });
            let bound = elaboration.denotation.arena.insert(Term::Variable(0));
            let family = elaboration.denotation.arena.insert(Term::CaseTwo {
                motive: inner_motive,
                zero_branch: parts.first,
                one_branch: parts.rest,
                scrutinee: bound,
            });
            let body = elaboration.denotation.arena.insert(Term::Pi {
                domain: family,
                codomain: parts.case.conclusion,
            });
            let motive = elaboration.denotation.arena.insert(Term::Lambda {
                domain: elaboration.denotation.two,
                body,
            });
            let tag = elaboration.denotation.arena.insert(Term::Fst {
                pair: parts.case.scrutinee,
            });
            let elimination = elaboration.denotation.arena.insert(Term::CaseTwo {
                motive,
                zero_branch: parts.zero,
                one_branch: one,
                scrutinee: tag,
            });
            let payload = elaboration.denotation.arena.insert(Term::Snd {
                pair: parts.case.scrutinee,
            });
            elaboration.denotation.arena.insert(Term::Apply {
                function: elimination,
                argument: payload,
            })
        }
    }

    impl<'a> Elaboration<'a> {
        pub(super) fn new(
            context: &'a PropositionContext,
            goal: &Proposition,
            assumptions: &'a [Proposition],
            semantic_axioms: &'a [Proposition],
            machine_parameter_values: &'a BTreeSet<ValueId>,
        ) -> Result<Self, BoundedDenotationError> {
            let mut denotation = Denotation::new();
            let mut context_bindings =
                Vec::with_capacity(assumptions.len() + semantic_axioms.len());
            for proposition in assumptions.iter().chain(semantic_axioms) {
                context_bindings.push(denotation.denote(proposition)?);
            }
            let expected = denotation.denote(goal)?;
            let depth = u32::try_from(assumptions.len() + semantic_axioms.len())
                .map_err(|_| BoundedDenotationError::DepthLimitExceeded)?;
            let positions = (0..assumptions.len())
                .map(|index| u32::try_from(index).expect("roster position fits the context depth"))
                .collect();
            Ok(Elaboration {
                denotation,
                context,
                ambient_assumptions: assumptions,
                axioms: semantic_axioms,
                machine_parameter_values,
                ambient: assumptions.len(),
                context_bindings,
                expected,
                roster: assumptions.to_vec(),
                positions,
                depth,
                steps: 0,
                rules: BTreeSet::new(),
                assumptions: PremiseRoster::default(),
                semantic_axioms: PremiseRoster::default(),
            })
        }

        fn variable(&mut self, position: u32) -> TermHandle {
            let index = self.depth - 1 - position;
            self.denotation.arena.insert(Term::Variable(index))
        }

        pub(super) fn node(
            &mut self,
            proof: &ProofNode,
        ) -> Result<TermHandle, BoundedDenotationError> {
            walk::elaborate(self, proof)
        }

        fn finish_regular(
            &mut self,
            proof: &ProofNode,
            operands: &mut walk::ChildEvidence<'_>,
        ) -> Result<TermHandle, BoundedDenotationError> {
            match &proof.rule {
                ProofRule::Primitive(judgment) => {
                    decide_primitive(self.context, &proof.conclusion, *judgment)
                        .map_err(|error| certificate!(ProofError::PrimitiveJudgment(error)))?;
                    self.rules.insert(AcceptedProofRule::Primitive);
                    let denoted = self.denotation.denote(&proof.conclusion)?;
                    if let Some(evidence) = self.denotation.closed_inhabitant(denoted)? {
                        return Ok(evidence);
                    }
                    self.denotation.decision(denoted)
                }
                ProofRule::Assumption { index } => self.cited(*index, &proof.conclusion),
                ProofRule::SemanticAxiom { index } => {
                    let Some(axiom) = self.semantic_axiom(*index) else {
                        return Err(certificate!(ProofError::UnknownSemanticAxiom(*index)));
                    };
                    if !propositions_match(&axiom, &proof.conclusion) {
                        return Err(certificate!(ProofError::SemanticAxiomConclusionMismatch(
                            *index
                        )));
                    }
                    self.same_denotation(&axiom, &proof.conclusion)?;
                    self.rules.insert(AcceptedProofRule::SemanticAxiom);
                    self.semantic_axioms.record(*index, &axiom);
                    let position = self.ambient + *index;
                    let position = u32::try_from(position)
                        .map_err(|_| BoundedDenotationError::DepthLimitExceeded)?;
                    Ok(self.variable(position))
                }
                ProofRule::ConjunctionIntroduction(children) => {
                    let Proposition::Conjunction(expected) = &proof.conclusion else {
                        unreachable!("conjunction shape admitted before its children")
                    };
                    let mut terms = Vec::with_capacity(children.len());
                    for child in children {
                        terms.push(operands.take(child));
                    }
                    for (child, expected) in children.iter().zip(expected.iter()) {
                        if &child.conclusion != expected {
                            return Err(certificate!(ProofError::ConjunctConclusionMismatch));
                        }
                    }
                    self.rules
                        .insert(AcceptedProofRule::ConjunctionIntroduction);
                    let mut term = terms.pop().expect("canonical conjunction arity");
                    while let Some(first) = terms.pop() {
                        term = self.denotation.arena.insert(Term::Pair {
                            first,
                            second: term,
                        });
                    }
                    Ok(term)
                }
                ProofRule::ConjunctionElimination {
                    conjunction,
                    conjunct,
                } => {
                    let pair = operands.take(conjunction);
                    let Proposition::Conjunction(conjuncts) = &conjunction.conclusion else {
                        return Err(certificate!(ProofError::RulePremiseMismatch(
                            "conjunction elimination"
                        )));
                    };
                    let selected = conjuncts
                        .get(*conjunct)
                        .ok_or(certificate!(ProofError::UnknownConjunct(*conjunct)))?;
                    if *selected != proof.conclusion {
                        return Err(certificate!(ProofError::ConjunctConclusionMismatch));
                    }
                    self.rules.insert(AcceptedProofRule::ConjunctionElimination);
                    let last = conjuncts.len() - 1;
                    let mut term = pair;
                    for _ in 0..(*conjunct).min(last) {
                        term = self.denotation.arena.insert(Term::Snd { pair: term });
                    }
                    if *conjunct < last {
                        term = self.denotation.arena.insert(Term::Fst { pair: term });
                    }
                    Ok(term)
                }
                ProofRule::DisjunctionIntroduction { disjunct, index } => {
                    let Proposition::Disjunction(disjuncts) = &proof.conclusion else {
                        unreachable!("disjunction shape and index admitted before the child")
                    };
                    let disjunct_term = operands.take(disjunct);
                    if disjuncts[*index] != disjunct.conclusion {
                        return Err(certificate!(ProofError::DisjunctConclusionMismatch));
                    }
                    self.rules
                        .insert(AcceptedProofRule::DisjunctionIntroduction);
                    Ok(self.disjunction_intro(disjuncts.len(), *index, disjunct_term))
                }
                ProofRule::DisjunctionElimination {
                    disjunction: _,
                    branches: _,
                } => {
                    unreachable!(
                        "disjunction elimination is owned by scoped case continuation frames"
                    )
                }
                ProofRule::ImplicationIntroduction { body: _ } => {
                    unreachable!("implication introduction is owned by scoped continuation frames")
                }
                ProofRule::ImplicationElimination {
                    implication,
                    premise,
                } => {
                    let function = operands.take(implication);
                    let argument = operands.take(premise);
                    let Proposition::Implication {
                        premise: required,
                        conclusion,
                    } = &implication.conclusion
                    else {
                        return Err(certificate!(ProofError::RulePremiseMismatch(
                            "implication elimination"
                        )));
                    };
                    if premise.conclusion != **required {
                        return Err(certificate!(ProofError::ImplicationPremiseMismatch));
                    }
                    if &proof.conclusion != conclusion.as_ref() {
                        return Err(certificate!(ProofError::ImplicationConclusionMismatch));
                    }
                    self.rules.insert(AcceptedProofRule::ImplicationElimination);
                    Ok(self
                        .denotation
                        .arena
                        .insert(Term::Apply { function, argument }))
                }
                ProofRule::EqualitySymmetry { equality } => {
                    let proof_term = operands.take(equality);
                    let Proposition::Equal(left, right) = &equality.conclusion else {
                        return Err(certificate!(ProofError::RulePremiseMismatch(
                            "equality symmetry"
                        )));
                    };
                    if proof.conclusion != Proposition::Equal(right.clone(), left.clone()) {
                        return Err(certificate!(ProofError::EqualityConclusionMismatch));
                    }
                    self.rules.insert(AcceptedProofRule::EqualitySymmetry);
                    let premise_ty = self.denotation.denote(&equality.conclusion)?;
                    let goal_ty = self.denotation.denote(&proof.conclusion)?;
                    match self
                        .denotation
                        .symmetry_evidence(premise_ty, goal_ty, proof_term)
                    {
                        Some(term) => Ok(term),
                        None => self.rule_instance(
                            AcceptedProofRule::EqualitySymmetry,
                            vec![equality.conclusion.clone()],
                            vec![proof_term],
                            &proof.conclusion,
                        ),
                    }
                }
                ProofRule::EqualityTransitivity {
                    left_equals_middle,
                    middle_equals_right,
                } => {
                    let first = operands.take(left_equals_middle);
                    let second = operands.take(middle_equals_right);
                    equality_rules::equality_transitivity_relation(
                        &left_equals_middle.conclusion,
                        &middle_equals_right.conclusion,
                        &proof.conclusion,
                    )
                    .map_err(BoundedDenotationError::Certificate)?;
                    self.rules.insert(AcceptedProofRule::EqualityTransitivity);
                    let first_ty = self.denotation.denote(&left_equals_middle.conclusion)?;
                    let second_ty = self.denotation.denote(&middle_equals_right.conclusion)?;
                    let goal_ty = self.denotation.denote(&proof.conclusion)?;
                    match self
                        .denotation
                        .transitivity_evidence(first_ty, first, second_ty, second, goal_ty)
                    {
                        Some(term) => Ok(term),
                        None => self.rule_instance(
                            AcceptedProofRule::EqualityTransitivity,
                            vec![
                                left_equals_middle.conclusion.clone(),
                                middle_equals_right.conclusion.clone(),
                            ],
                            vec![first, second],
                            &proof.conclusion,
                        ),
                    }
                }
                ProofRule::PredicateDenotation { premise } => {
                    let evidence = operands.take(premise);
                    predicate_denotation_relation(
                        self.context,
                        &premise.conclusion,
                        &proof.conclusion,
                    )
                    .map_err(BoundedDenotationError::Certificate)?;
                    self.rules.insert(AcceptedProofRule::PredicateDenotation);
                    let premise_ty = self.denotation.denote(&premise.conclusion)?;
                    let goal_ty = self.denotation.denote(&proof.conclusion)?;
                    if let Some(term) = self
                        .denotation
                        .oriented_evidence(premise_ty, goal_ty, evidence)?
                    {
                        return Ok(term);
                    }
                    self.rule_instance(
                        AcceptedProofRule::PredicateDenotation,
                        vec![premise.conclusion.clone()],
                        vec![evidence],
                        &proof.conclusion,
                    )
                }
                ProofRule::ValueEqualityTransport {
                    premise,
                    equalities,
                } => {
                    let premises = operands.take_prepared_premises();
                    let mut evidence = Vec::with_capacity(equalities.len() + 1);
                    evidence.push(operands.take(premise));
                    for equality in equalities {
                        evidence.push(operands.take(equality));
                    }
                    equality_rules::value_equality_transport_relation(
                        self.context,
                        &premise.conclusion,
                        equalities.iter().map(|equality| &equality.conclusion),
                        &proof.conclusion,
                    )
                    .map_err(BoundedDenotationError::Certificate)?;
                    self.rules.insert(AcceptedProofRule::ValueEqualityTransport);
                    if let Some(term) = self.denotation.compositional_transport(
                        &premise.conclusion,
                        evidence[0],
                        equalities,
                        &evidence[1..],
                        &proof.conclusion,
                    )? {
                        return Ok(term);
                    }
                    self.rule_instance(
                        AcceptedProofRule::ValueEqualityTransport,
                        premises,
                        evidence,
                        &proof.conclusion,
                    )
                }
                ProofRule::IntegerSubtractOrder {
                    difference,
                    positive,
                } => {
                    let difference_evidence = operands.take(difference);
                    let positive_evidence = operands.take(positive);
                    subtract_order::check(
                        &difference.conclusion,
                        &positive.conclusion,
                        &proof.conclusion,
                    )
                    .map_err(BoundedDenotationError::Certificate)?;
                    self.rules.insert(AcceptedProofRule::IntegerSubtractOrder);
                    self.denotation.subtraction_evidence(
                        &difference.conclusion,
                        difference_evidence,
                        positive_evidence,
                        &proof.conclusion,
                    )
                }
                ProofRule::IntegerAddOrder { sum, positive } => {
                    let sum_evidence = operands.take(sum);
                    let positive_evidence = operands.take(positive);
                    add_order::check(&sum.conclusion, &positive.conclusion, &proof.conclusion)
                        .map_err(BoundedDenotationError::Certificate)?;
                    self.rules.insert(AcceptedProofRule::IntegerAddOrder);
                    match self.denotation.addition_order_evidence(
                        &sum.conclusion,
                        sum_evidence,
                        positive_evidence,
                    )? {
                        Some(term) => Ok(term),
                        None => self.rule_instance(
                            AcceptedProofRule::IntegerAddOrder,
                            vec![sum.conclusion.clone(), positive.conclusion.clone()],
                            vec![sum_evidence, positive_evidence],
                            &proof.conclusion,
                        ),
                    }
                }
                ProofRule::IntegerSubtractAntitone {
                    smaller,
                    larger,
                    order,
                } => {
                    let smaller_evidence = operands.take(smaller);
                    let larger_evidence = operands.take(larger);
                    let order_evidence = operands.take(order);
                    subtract_antitone::check(
                        &smaller.conclusion,
                        &larger.conclusion,
                        &order.conclusion,
                        &proof.conclusion,
                    )
                    .map_err(BoundedDenotationError::Certificate)?;
                    self.rules
                        .insert(AcceptedProofRule::IntegerSubtractAntitone);
                    match self.denotation.subtraction_antitone_evidence(
                        &smaller.conclusion,
                        smaller_evidence,
                        &larger.conclusion,
                        larger_evidence,
                        order_evidence,
                    )? {
                        Some(term) => Ok(term),
                        None => self.rule_instance(
                            AcceptedProofRule::IntegerSubtractAntitone,
                            vec![
                                smaller.conclusion.clone(),
                                larger.conclusion.clone(),
                                order.conclusion.clone(),
                            ],
                            vec![smaller_evidence, larger_evidence, order_evidence],
                            &proof.conclusion,
                        ),
                    }
                }
                ProofRule::IntegerOrderDiscreteness { relation } => {
                    let evidence = operands.take(relation);
                    order_discreteness::check(&relation.conclusion, &proof.conclusion)
                        .map_err(BoundedDenotationError::Certificate)?;
                    self.rules
                        .insert(AcceptedProofRule::IntegerOrderDiscreteness);
                    self.denotation.discreteness_evidence(
                        &relation.conclusion,
                        evidence,
                        &proof.conclusion,
                    )
                }
                ProofRule::IntegerOrderWeakening { relation } => {
                    let evidence = operands.take(relation);
                    integer_order_rules::order_weakening(&relation.conclusion, &proof.conclusion)
                        .map_err(BoundedDenotationError::Certificate)?;
                    self.rules.insert(AcceptedProofRule::IntegerOrderWeakening);
                    let premise_ty = self.denotation.denote(&relation.conclusion)?;
                    let goal_ty = self.denotation.denote(&proof.conclusion)?;
                    match self
                        .denotation
                        .weakening_evidence(premise_ty, evidence, goal_ty)?
                    {
                        Some(term) => Ok(term),
                        None => self.rule_instance(
                            AcceptedProofRule::IntegerOrderWeakening,
                            vec![relation.conclusion.clone()],
                            vec![evidence],
                            &proof.conclusion,
                        ),
                    }
                }
                ProofRule::IntegerLessOrEqualTransitivity {
                    left_less_or_equal_middle,
                    middle_less_or_equal_right,
                } => {
                    let first = operands.take(left_less_or_equal_middle);
                    let second = operands.take(middle_less_or_equal_right);
                    integer_order_rules::less_or_equal_transitivity(
                        &left_less_or_equal_middle.conclusion,
                        &middle_less_or_equal_right.conclusion,
                        &proof.conclusion,
                    )
                    .map_err(BoundedDenotationError::Certificate)?;
                    self.rules
                        .insert(AcceptedProofRule::IntegerLessOrEqualTransitivity);
                    let first_ty = self
                        .denotation
                        .denote(&left_less_or_equal_middle.conclusion)?;
                    let second_ty = self
                        .denotation
                        .denote(&middle_less_or_equal_right.conclusion)?;
                    let goal_ty = self.denotation.denote(&proof.conclusion)?;
                    match self
                        .denotation
                        .order_transitivity_evidence(first_ty, first, second_ty, second, goal_ty)?
                    {
                        Some(term) => Ok(term),
                        None => self.rule_instance(
                            AcceptedProofRule::IntegerLessOrEqualTransitivity,
                            vec![
                                left_less_or_equal_middle.conclusion.clone(),
                                middle_less_or_equal_right.conclusion.clone(),
                            ],
                            vec![first, second],
                            &proof.conclusion,
                        ),
                    }
                }
                ProofRule::IntegerStrictOrderTransitivity {
                    left_to_middle,
                    middle_to_right,
                } => {
                    let first = operands.take(left_to_middle);
                    let second = operands.take(middle_to_right);
                    strict_order_transitivity::check(
                        &left_to_middle.conclusion,
                        &middle_to_right.conclusion,
                        &proof.conclusion,
                    )
                    .map_err(BoundedDenotationError::Certificate)?;
                    self.rules
                        .insert(AcceptedProofRule::IntegerStrictOrderTransitivity);
                    let first_ty = self.denotation.denote(&left_to_middle.conclusion)?;
                    let second_ty = self.denotation.denote(&middle_to_right.conclusion)?;
                    let goal_ty = self.denotation.denote(&proof.conclusion)?;
                    match self
                        .denotation
                        .order_transitivity_evidence(first_ty, first, second_ty, second, goal_ty)?
                    {
                        Some(term) => Ok(term),
                        None => self.rule_instance(
                            AcceptedProofRule::IntegerStrictOrderTransitivity,
                            vec![
                                left_to_middle.conclusion.clone(),
                                middle_to_right.conclusion.clone(),
                            ],
                            vec![first, second],
                            &proof.conclusion,
                        ),
                    }
                }
                ProofRule::IntegerOrderAntisymmetry { forward, backward } => {
                    let forward_evidence = operands.take(forward);
                    let backward_evidence = operands.take(backward);
                    order_antisymmetry(
                        &forward.conclusion,
                        &backward.conclusion,
                        &proof.conclusion,
                    )
                    .map_err(BoundedDenotationError::Certificate)?;
                    self.rules
                        .insert(AcceptedProofRule::IntegerOrderAntisymmetry);
                    let Proposition::LessOrEqual(left, right) = &forward.conclusion else {
                        unreachable!("checked antisymmetry order");
                    };
                    let conclusion = self.denotation.denote(&proof.conclusion)?;
                    self.denotation.order_antisymmetry_evidence(
                        left,
                        right,
                        forward_evidence,
                        backward_evidence,
                        conclusion,
                    )
                }
                ProofRule::IntegerMathOrderTranslation { relation, addend } => {
                    let evidence = operands.take(relation);
                    math_order_translation(&relation.conclusion, addend, &proof.conclusion)
                        .map_err(BoundedDenotationError::Certificate)?;
                    self.rules
                        .insert(AcceptedProofRule::IntegerMathOrderTranslation);
                    self.denotation.mathematical_order_translation(
                        &relation.conclusion,
                        addend,
                        evidence,
                    )
                }
                ProofRule::IntegerOrderSubstitution {
                    relation,
                    equality,
                    endpoint,
                } => {
                    let relation_evidence = operands.take(relation);
                    let equality_evidence = operands.take(equality);
                    integer_order_rules::endpoint_substitution(
                        &relation.conclusion,
                        &equality.conclusion,
                        *endpoint,
                        &proof.conclusion,
                    )
                    .map_err(BoundedDenotationError::Certificate)?;
                    self.rules
                        .insert(AcceptedProofRule::IntegerOrderSubstitution);
                    let relation_ty = self.denotation.denote(&relation.conclusion)?;
                    let equality_ty = self.denotation.denote(&equality.conclusion)?;
                    let goal_ty = self.denotation.denote(&proof.conclusion)?;
                    match self.denotation.order_substitution_evidence(
                        relation_ty,
                        relation_evidence,
                        equality_ty,
                        equality_evidence,
                        *endpoint,
                        goal_ty,
                    )? {
                        Some(term) => Ok(term),
                        None => self.rule_instance(
                            AcceptedProofRule::IntegerOrderSubstitution,
                            vec![relation.conclusion.clone(), equality.conclusion.clone()],
                            vec![relation_evidence, equality_evidence],
                            &proof.conclusion,
                        ),
                    }
                }
                ProofRule::IntegerAffineBound {
                    root_bound,
                    witness,
                } => {
                    let root = operands.take(root_bound);
                    let cited = integer_bound_rules::affine_bound_relation(
                        self.context,
                        self.axioms,
                        &root_bound.conclusion,
                        witness,
                        &proof.conclusion,
                    )
                    .map_err(BoundedDenotationError::Certificate)?;
                    if let Some(evidence) = self.denotation.unsigned_subtract_bound_evidence(
                        &root_bound.conclusion,
                        root,
                        witness,
                        &proof.conclusion,
                    )? {
                        self.rules.insert(AcceptedProofRule::IntegerAffineBound);
                        return Ok(evidence);
                    }
                    let mut definitions = Vec::with_capacity(cited.len());
                    for &index in &cited {
                        definitions.push(self.cited_axiom(index)?);
                    }
                    if let Some(evidence) = self.denotation.correlated_subtract_bound_evidence(
                        &root_bound.conclusion,
                        root,
                        witness,
                        &proof.conclusion,
                        &definitions,
                    )? {
                        self.rules.insert(AcceptedProofRule::IntegerAffineBound);
                        return Ok(evidence);
                    }
                    if let Some(evidence) = self.denotation.correlated_add_bound_evidence(
                        &root_bound.conclusion,
                        root,
                        witness,
                        &proof.conclusion,
                        &definitions,
                    )? {
                        self.rules.insert(AcceptedProofRule::IntegerAffineBound);
                        return Ok(evidence);
                    }
                    if let Some(evidence) = self.denotation.correlated_multiply_bound_evidence(
                        &root_bound.conclusion,
                        root,
                        witness,
                        &proof.conclusion,
                        &definitions,
                    )? {
                        self.rules.insert(AcceptedProofRule::IntegerAffineBound);
                        return Ok(evidence);
                    }
                    if let Some(evidence) = self.denotation.direct_add_bound_evidence(
                        &root_bound.conclusion,
                        root,
                        witness,
                        &proof.conclusion,
                    )? {
                        self.rules.insert(AcceptedProofRule::IntegerAffineBound);
                        return Ok(evidence);
                    }
                    if let Some(evidence) = self.denotation.direct_subtract_bound_evidence(
                        &root_bound.conclusion,
                        root,
                        witness,
                        &proof.conclusion,
                    )? {
                        self.rules.insert(AcceptedProofRule::IntegerAffineBound);
                        return Ok(evidence);
                    }
                    let mut premises = Vec::with_capacity(cited.len() + 1);
                    let mut evidence = Vec::with_capacity(cited.len() + 1);
                    premises.push(root_bound.conclusion.clone());
                    evidence.push(root);
                    for (proposition, variable) in definitions {
                        premises.push(proposition);
                        evidence.push(variable);
                    }
                    self.rule_instance(
                        AcceptedProofRule::IntegerAffineBound,
                        premises,
                        evidence,
                        &proof.conclusion,
                    )
                }
                ProofRule::IntegerExactAddDefinitionBound {
                    left_bound,
                    right_bound,
                    definition_axiom,
                }
                | ProofRule::IntegerExactSubtractDefinitionBound {
                    left_bound,
                    right_bound,
                    definition_axiom,
                } => {
                    let (operation, rule) = match &proof.rule {
                        ProofRule::IntegerExactAddDefinitionBound { .. } => (
                            integer_bound_rules::ExactDefinitionOperation::Add,
                            AcceptedProofRule::IntegerExactAddDefinitionBound,
                        ),
                        _ => (
                            integer_bound_rules::ExactDefinitionOperation::Subtract,
                            AcceptedProofRule::IntegerExactSubtractDefinitionBound,
                        ),
                    };
                    let left = operands.take(left_bound);
                    let right = operands.take(right_bound);
                    integer_bound_rules::exact_definition_bound_relation(
                        self.context,
                        self.axioms,
                        &left_bound.conclusion,
                        &right_bound.conclusion,
                        *definition_axiom,
                        &proof.conclusion,
                        operation,
                    )
                    .map_err(BoundedDenotationError::Certificate)?;
                    let (definition, variable) = self.cited_axiom(*definition_axiom)?;
                    let derived = match operation {
                        integer_bound_rules::ExactDefinitionOperation::Add => {
                            self.denotation.exact_add_definition_bound_evidence(
                                &left_bound.conclusion,
                                left,
                                &right_bound.conclusion,
                                right,
                                &definition,
                                variable,
                                &proof.conclusion,
                            )?
                        }
                        integer_bound_rules::ExactDefinitionOperation::Subtract => {
                            self.denotation.exact_subtract_definition_bound_evidence(
                                &left_bound.conclusion,
                                left,
                                &right_bound.conclusion,
                                right,
                                &definition,
                                variable,
                                &proof.conclusion,
                            )?
                        }
                    };
                    if let Some(evidence) = derived {
                        self.rules.insert(rule);
                        return Ok(evidence);
                    }
                    self.rule_instance(
                        rule,
                        vec![
                            left_bound.conclusion.clone(),
                            right_bound.conclusion.clone(),
                            definition,
                        ],
                        vec![left, right, variable],
                        &proof.conclusion,
                    )
                }
                ProofRule::IntegerCastBound {
                    root_bound,
                    witness,
                } => {
                    let root = operands.take(root_bound);
                    let (citations, chain) = integer_bound_rules::cast_bound_relation(
                        self.context,
                        self.axioms,
                        &root_bound.conclusion,
                        witness,
                        &proof.conclusion,
                    )
                    .map_err(BoundedDenotationError::Certificate)?;
                    let mut premises = Vec::with_capacity(citations.len() + 1);
                    let mut evidence = Vec::with_capacity(citations.len() + 1);
                    premises.push(root_bound.conclusion.clone());
                    evidence.push(root);
                    for &index in &citations {
                        let (proposition, variable) = self.cited_axiom(index)?;
                        premises.push(proposition);
                        evidence.push(variable);
                    }
                    if let Some(derived) = self.denotation.cast_bound_evidence(
                        &root_bound.conclusion,
                        evidence[0],
                        &chain,
                        &evidence[1..],
                        self.axioms,
                        &proof.conclusion,
                    )? {
                        self.rules.insert(AcceptedProofRule::IntegerCastBound);
                        return Ok(derived);
                    }
                    self.rule_instance(
                        AcceptedProofRule::IntegerCastBound,
                        premises,
                        evidence,
                        &proof.conclusion,
                    )
                }
                ProofRule::IntegerCorrelatedForbiddenRoots { witness } => {
                    let (citations, checked) =
                        integer_bound_rules::correlated_forbidden_roots_relation(
                            self.context,
                            self.ambient_assumptions,
                            self.axioms,
                            self.machine_parameter_values,
                            witness,
                            &proof.conclusion,
                        )
                        .map_err(BoundedDenotationError::Certificate)?;
                    let mut premises = Vec::with_capacity(
                        citations.semantic_axioms.len() + citations.assumptions.len(),
                    );
                    let mut evidence = Vec::with_capacity(premises.capacity());
                    let mut axiom_evidence = Vec::with_capacity(citations.semantic_axioms.len());
                    for index in citations.semantic_axioms {
                        let (proposition, variable) = self.cited_axiom(index)?;
                        axiom_evidence.push((proposition.clone(), variable));
                        premises.push(proposition);
                        evidence.push(variable);
                    }
                    let mut bound_evidence = Vec::with_capacity(citations.assumptions.len());
                    for index in citations.assumptions {
                        let (proposition, variable) = self.ambient_premise(index)?;
                        bound_evidence.push((proposition.clone(), variable));
                        premises.push(proposition);
                        evidence.push(variable);
                    }
                    if let Some(derived) = self.denotation.correlated_forbidden_roots_evidence(
                        witness,
                        &checked,
                        &axiom_evidence,
                        &bound_evidence,
                        &proof.conclusion,
                    )? {
                        self.rules
                            .insert(AcceptedProofRule::IntegerCorrelatedForbiddenRoots);
                        return Ok(derived);
                    }
                    self.rule_instance(
                        AcceptedProofRule::IntegerCorrelatedForbiddenRoots,
                        premises,
                        evidence,
                        &proof.conclusion,
                    )
                }
            }
        }

        fn cited(
            &mut self,
            index: usize,
            conclusion: &Proposition,
        ) -> Result<TermHandle, BoundedDenotationError> {
            let Some(assumption) = self.roster.get(index).cloned() else {
                return Err(certificate!(ProofError::UnknownAssumption(index)));
            };
            if !propositions_match(&assumption, conclusion) {
                return Err(certificate!(ProofError::AssumptionConclusionMismatch(
                    index
                )));
            }
            self.same_denotation(&assumption, conclusion)?;
            self.rules.insert(AcceptedProofRule::Assumption);
            if index < self.ambient {
                self.assumptions.record(index, &assumption);
            }
            let position = self.positions[index];
            Ok(self.variable(position))
        }

        fn semantic_axiom(&self, index: usize) -> Option<Proposition> {
            self.axioms.get(index).cloned()
        }

        fn same_denotation(
            &mut self,
            retained: &Proposition,
            requested: &Proposition,
        ) -> Result<(), BoundedDenotationError> {
            if retained == requested {
                return Ok(());
            }
            let retained = self.denotation.denote(retained)?;
            let requested = self.denotation.denote(requested)?;
            if self
                .denotation
                .arena
                .structurally_equal(retained, requested)
            {
                Ok(())
            } else {
                Err(BoundedDenotationError::Unsupported(
                    "integer-math-normalized citation across denotation shapes",
                ))
            }
        }

        fn ambient_premise(
            &mut self,
            index: usize,
        ) -> Result<(Proposition, TermHandle), BoundedDenotationError> {
            let Some(proposition) = self.roster.get(index).cloned() else {
                return Err(certificate!(ProofError::UnknownAssumption(index)));
            };
            if index >= self.ambient {
                return Err(certificate!(ProofError::UnknownAssumption(index)));
            }
            self.assumptions.record(index, &proposition);
            let position = self.positions[index];
            let variable = self.variable(position);
            Ok((proposition, variable))
        }

        fn cited_axiom(
            &mut self,
            index: usize,
        ) -> Result<(Proposition, TermHandle), BoundedDenotationError> {
            let Some(proposition) = self.axioms.get(index).cloned() else {
                return Err(certificate!(ProofError::UnknownSemanticAxiom(index)));
            };
            self.semantic_axioms.record(index, &proposition);
            let position = u32::try_from(self.ambient + index)
                .map_err(|_| BoundedDenotationError::DepthLimitExceeded)?;
            let variable = self.variable(position);
            Ok((proposition, variable))
        }

        fn rule_instance(
            &mut self,
            rule: AcceptedProofRule,
            premises: Vec<Proposition>,
            evidence: Vec<TermHandle>,
            conclusion: &Proposition,
        ) -> Result<TermHandle, BoundedDenotationError> {
            debug_assert_eq!(premises.len(), evidence.len());
            let axiom = self.denotation.rule_axiom(&premises, conclusion)?;
            let term = self.denotation.arena.apply_arguments(axiom, &evidence);
            self.rules.insert(rule);
            Ok(term)
        }

        fn disjunction_intro(
            &mut self,
            mut remaining: usize,
            mut index: usize,
            mut term: TermHandle,
        ) -> TermHandle {
            let mut tags = Vec::new();
            loop {
                tags.push(self.denotation.arena.insert(if index == 0 {
                    Term::TwoZero
                } else {
                    Term::TwoOne
                }));
                if index == 0 || remaining == 2 {
                    break;
                }
                remaining -= 1;
                index -= 1;
            }
            while let Some(first) = tags.pop() {
                term = self.denotation.arena.insert(Term::Pair {
                    first,
                    second: term,
                });
            }
            term
        }

        pub(super) fn finish(self, term: TermHandle) -> BoundedDenotation {
            let Elaboration {
                denotation,
                context_bindings,
                expected,
                rules,
                assumptions,
                semantic_axioms,
                ..
            } = self;
            BoundedDenotation {
                arena: denotation.arena,
                certificate: MathematicalCertificate {
                    signature: denotation.declarations,
                    level_arity: 0,
                    context: context_bindings,
                    term,
                    expected,
                },
                rules: rules.into_iter().collect(),
                assumptions: assumptions.premises,
                semantic_axioms: semantic_axioms.premises,
            }
        }
    }
}
use elaboration::Elaboration;
mod forbidden_roots {
    use std::collections::BTreeMap;

    use semantic_vocabulary::{
        IntegerCarrier, IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm,
    };

    use super::integer_operations::IntegerOperation;
    use super::{
        BoundedDenotationError, Denotation, IntegerLaw, IntegerRelation, Term, TermHandle,
    };
    use crate::mathematical_core::scheme_dsl::{
        self, apps, case_two, id, lam, pi, pis, scheme_at, sigma, sort, two, v,
    };
    use crate::mathematical_core::substitution::shift;
    use crate::mathematical_core::term::Level;
    use crate::{
        CheckedIntegerCorrelatedForbiddenRoots, CorrelatedAffineBranchWitness,
        IntegerCorrelatedForbiddenRootWitness,
    };

    mod chains {
        use super::*;

        #[derive(Clone)]
        pub(super) struct ChainStep {
            pub(super) definition: TermHandle,
            pub(super) literal: Option<TermHandle>,
            pub(super) operation: ChainOperation,
            pub(super) next: TermHandle,
            pub(super) sibling: TermHandle,
            pub(super) literal_term: TermHandle,
            pub(super) literal_value: i128,
            pub(super) expression: TermHandle,
            pub(super) current: TermHandle,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(super) enum ChainOperation {
            Add,
            Subtract,
            Multiply,
        }

        impl Denotation {
            pub(super) fn chain_operation_terms(
                &mut self,
                operation: ChainOperation,
                left: TermHandle,
                right: TermHandle,
            ) -> Result<TermHandle, BoundedDenotationError> {
                match operation {
                    ChainOperation::Add => self.add_terms(left, right),
                    ChainOperation::Subtract => self.subtract_terms(left, right),
                    ChainOperation::Multiply => self.multiply_terms(left, right),
                }
            }

            pub(super) fn chain_operation_eval(
                operation: ChainOperation,
                left: i128,
                right: i128,
            ) -> Option<i128> {
                match operation {
                    ChainOperation::Add => left.checked_add(right),
                    ChainOperation::Subtract => left.checked_sub(right),
                    ChainOperation::Multiply => left.checked_mul(right),
                }
            }

            pub(super) fn chain_sibling_congruence(
                &mut self,
                integer: TermHandle,
                operation: ChainOperation,
                next: TermHandle,
                from: TermHandle,
                to: TermHandle,
                proof: TermHandle,
                expression: TermHandle,
            ) -> Result<TermHandle, BoundedDenotationError> {
                let base = self.arena.insert(Term::Refl {
                    ty: integer,
                    value: expression,
                });
                self.integer_id_elim(integer, from, to, proof, base, |this, bound| {
                    let rewritten = this.chain_operation_terms(operation, next, bound)?;
                    Ok(this.arena.insert(Term::Id {
                        ty: integer,
                        left: expression,
                        right: rewritten,
                    }))
                })
            }

            pub(super) fn chain_operand_congruence(
                &mut self,
                integer: TermHandle,
                operation: ChainOperation,
                sibling: TermHandle,
                from: TermHandle,
                to: TermHandle,
                proof: TermHandle,
            ) -> Result<TermHandle, BoundedDenotationError> {
                let source = self.chain_operation_terms(operation, from, sibling)?;
                let base = self.arena.insert(Term::Refl {
                    ty: integer,
                    value: source,
                });
                self.integer_id_elim(integer, from, to, proof, base, |this, bound| {
                    let rewritten = this.chain_operation_terms(operation, bound, sibling)?;
                    Ok(this.arena.insert(Term::Id {
                        ty: integer,
                        left: source,
                        right: rewritten,
                    }))
                })
            }

            pub(super) fn chain_steps(
                &mut self,
                branch: &CorrelatedAffineBranchWitness,
                integer_type: IntegerType,
                evidence: &[(Proposition, TermHandle)],
                cursor: &mut usize,
            ) -> Result<Option<Vec<ChainStep>>, BoundedDenotationError> {
                let mut steps = Vec::with_capacity(branch.steps.len());
                for step in &branch.steps {
                    let literal_evidence = if step.literal_axiom.is_some() {
                        let Some((proposition, evidence)) = evidence.get(*cursor) else {
                            return Ok(None);
                        };
                        *cursor += 1;
                        Some((proposition.clone(), *evidence))
                    } else {
                        None
                    };
                    let Some((definition_proposition, definition_evidence)) = evidence.get(*cursor)
                    else {
                        return Ok(None);
                    };
                    *cursor += 1;
                    let Some((current, expression)) = definition_proposition.as_equal() else {
                        return Ok(None);
                    };
                    let (operation, next, sibling) = match expression {
                        ScalarTerm::ExactIntegerAdd {
                            scalar_type,
                            left,
                            right,
                        } if *scalar_type == integer_type => {
                            (ChainOperation::Add, left.as_ref(), right.as_ref())
                        }
                        ScalarTerm::ExactIntegerSubtract {
                            scalar_type,
                            left,
                            right,
                        } if *scalar_type == integer_type => {
                            (ChainOperation::Subtract, left.as_ref(), right.as_ref())
                        }
                        ScalarTerm::ExactIntegerMultiply {
                            scalar_type,
                            left,
                            right,
                        } if *scalar_type == integer_type => {
                            (ChainOperation::Multiply, left.as_ref(), right.as_ref())
                        }
                        _ => return Ok(None),
                    };
                    let current_term = self.fixed_scalar_term(current)?;
                    let next_term = self.fixed_scalar_term(next)?;
                    let sibling_term = self.fixed_scalar_term(sibling)?;
                    let expression_term =
                        self.chain_operation_terms(operation, next_term, sibling_term)?;
                    let denoted = self.denote(definition_proposition)?;
                    let Some((_, from, to)) = self.identity_parts(denoted) else {
                        return Ok(None);
                    };
                    let Some(definition) = self.directed_equality(
                        from,
                        to,
                        current_term,
                        expression_term,
                        *definition_evidence,
                    ) else {
                        return Ok(None);
                    };
                    let (literal_value, literal_term, literal_evidence_term) =
                        match literal_evidence {
                            None => {
                                let Some(value) = signed_literal(sibling, integer_type) else {
                                    return Ok(None);
                                };
                                (value, sibling_term, None)
                            }
                            Some((proposition, evidence)) => {
                                let Proposition::Equal(left, right) = &proposition else {
                                    return Ok(None);
                                };
                                if left != sibling {
                                    return Ok(None);
                                }
                                let Some(value) = signed_literal(right, integer_type) else {
                                    return Ok(None);
                                };
                                let literal_term = self.integer_numeral(value)?;
                                let denoted = self.denote(&proposition)?;
                                let Some((_, from, to)) = self.identity_parts(denoted) else {
                                    return Ok(None);
                                };
                                let Some(evidence) = self.directed_equality(
                                    from,
                                    to,
                                    sibling_term,
                                    literal_term,
                                    evidence,
                                ) else {
                                    return Ok(None);
                                };
                                (value, literal_term, Some(evidence))
                            }
                        };
                    steps.push(ChainStep {
                        definition,
                        literal: literal_evidence_term,
                        operation,
                        next: next_term,
                        sibling: sibling_term,
                        literal_term,
                        literal_value,
                        expression: expression_term,
                        current: current_term,
                    });
                }
                Ok(Some(steps))
            }

            pub(super) fn chain_step_lift(&mut self, step: &ChainStep, amount: u32) -> ChainStep {
                ChainStep {
                    definition: shift(&mut self.arena, step.definition, 0, amount),
                    literal: step
                        .literal
                        .map(|literal| shift(&mut self.arena, literal, 0, amount)),
                    operation: step.operation,
                    next: shift(&mut self.arena, step.next, 0, amount),
                    sibling: shift(&mut self.arena, step.sibling, 0, amount),
                    literal_term: shift(&mut self.arena, step.literal_term, 0, amount),
                    literal_value: step.literal_value,
                    expression: shift(&mut self.arena, step.expression, 0, amount),
                    current: shift(&mut self.arena, step.current, 0, amount),
                }
            }
        }
    }
    use chains::{ChainOperation, ChainStep};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    enum Law {
        AddRightInverse,
        SubtractRightInverse,
        QuotientUpperPositive(IntegerType),
        QuotientLowerPositive(IntegerType),
        QuotientLowerNegative(IntegerType),
        QuotientUpperNegative(IntegerType),
        LessOrEqualAntisymmetry,
        DivisorCaseSplit,
    }

    enum SolveOutcome {
        Refuted(TermHandle),
        Root {
            equality: TermHandle,
            quotient: i128,
            quotient_term: TermHandle,
        },
    }

    #[derive(Default)]
    pub(super) struct ForbiddenRoots {
        laws: BTreeMap<Law, u32>,
    }

    fn signed_literal(term: &ScalarTerm, integer_type: IntegerType) -> Option<i128> {
        match term.integer_value()? {
            (actual, IntegerValue::Signed(value)) if actual == integer_type => Some(value),
            _ => None,
        }
    }

    impl Denotation {
        pub(super) fn order_antisymmetry_evidence(
            &mut self,
            left: &ScalarTerm,
            right: &ScalarTerm,
            forward: TermHandle,
            backward: TermHandle,
            conclusion: TermHandle,
        ) -> Result<TermHandle, BoundedDenotationError> {
            let left = self.fixed_scalar_term(left)?;
            let right = self.fixed_scalar_term(right)?;
            let evidence = self.roots_law_application(
                Law::LessOrEqualAntisymmetry,
                &[left, right, forward, backward],
            )?;
            let Some((carrier, actual_left, actual_right)) = self.identity_parts(conclusion) else {
                return Err(BoundedDenotationError::Unsupported(
                    "antisymmetry conclusion is not an identity",
                ));
            };
            if self.arena.structurally_equal(actual_left, left)
                && self.arena.structurally_equal(actual_right, right)
            {
                Ok(evidence)
            } else if self.arena.structurally_equal(actual_left, right)
                && self.arena.structurally_equal(actual_right, left)
            {
                Ok(self.symmetry(carrier, left, right, evidence))
            } else {
                Err(BoundedDenotationError::Unsupported(
                    "antisymmetry denotation changed endpoints",
                ))
            }
        }

        pub(super) fn add_right_inverse(
            &mut self,
            original: TermHandle,
            increment: TermHandle,
            sum: TermHandle,
            evidence: TermHandle,
        ) -> Result<TermHandle, BoundedDenotationError> {
            self.roots_law_application(Law::AddRightInverse, &[original, increment, sum, evidence])
        }

        fn roots_law_application(
            &mut self,
            law: Law,
            arguments: &[TermHandle],
        ) -> Result<TermHandle, BoundedDenotationError> {
            self.roster_law_application(
                |this| &mut this.forbidden_roots.laws,
                law,
                |this| -> Result<scheme_dsl::Syntax, BoundedDenotationError> {
                    let integer = this.integer()?;
                    let add = this.add_operation()?;
                    let sub = this.subtract_operation()?;
                    let mul = this.multiply_operation()?;
                    let less_or_equal = this.integer_less_or_equal()?;
                    let zero = this.binary_integer(false, 0)?;
                    let one = this.binary_integer(false, 1)?;
                    let negative_one = this.binary_integer(true, 1)?;
                    let negative_two = this.binary_integer(true, 2)?;
                    let constant = |position| scheme_at(position, Vec::new());
                    let carrier = || constant(integer);
                    let binders = |names: &[&'static str], body| {
                        pis(names.iter().map(|name| (*name, carrier())), body)
                    };
                    let le = |left, right| apps(constant(less_or_equal), [left, right]);
                    let eq = |left, right| id(carrier(), left, right);
                    let add = |left, right| apps(constant(add), [left, right]);
                    let sub = |left, right| apps(constant(sub), [left, right]);
                    let mul = |left, right| apps(constant(mul), [left, right]);
                    let statement = match law {
                        Law::AddRightInverse => binders(
                            &["a", "k", "m"],
                            pi(
                                "_",
                                eq(add(v("a"), v("k")), v("m")),
                                eq(v("a"), sub(v("m"), v("k"))),
                            ),
                        ),
                        Law::SubtractRightInverse => binders(
                            &["a", "k", "m"],
                            pi(
                                "_",
                                eq(sub(v("a"), v("k")), v("m")),
                                eq(v("a"), add(v("m"), v("k"))),
                            ),
                        ),
                        Law::QuotientUpperPositive(scalar_type)
                        | Law::QuotientLowerPositive(scalar_type)
                        | Law::QuotientLowerNegative(scalar_type)
                        | Law::QuotientUpperNegative(scalar_type) => {
                            let divide =
                                this.integer_operation(IntegerOperation::ExactDivide(scalar_type))?;
                            let quotient = |left, right| apps(constant(divide), [left, right]);
                            let positive = matches!(
                                law,
                                Law::QuotientUpperPositive(_) | Law::QuotientLowerPositive(_)
                            );
                            let sign = if positive {
                                le(constant(one), v("k"))
                            } else {
                                le(v("k"), constant(negative_one))
                            };
                            let bound = match law {
                                Law::QuotientUpperPositive(_) | Law::QuotientLowerNegative(_) => {
                                    le(mul(v("a"), v("k")), v("m"))
                                }
                                _ => le(v("m"), mul(v("a"), v("k"))),
                            };
                            let residual = match law {
                                Law::QuotientUpperPositive(_) | Law::QuotientUpperNegative(_) => {
                                    le(v("a"), quotient(v("m"), v("k")))
                                }
                                _ => le(quotient(v("m"), v("k")), v("a")),
                            };
                            binders(&["k", "m", "a"], pi("_", sign, pi("_", bound, residual)))
                        }
                        Law::LessOrEqualAntisymmetry => binders(
                            &["a", "b"],
                            pi(
                                "_",
                                le(v("a"), v("b")),
                                pi("_", le(v("b"), v("a")), eq(v("a"), v("b"))),
                            ),
                        ),
                        Law::DivisorCaseSplit => {
                            let motive = || lam("_", two(), sort(Level::Constant(0)));
                            let innermost = sigma(
                                "t3",
                                two(),
                                case_two(
                                    motive(),
                                    eq(v("d"), constant(zero)),
                                    le(constant(one), v("d")),
                                    v("t3"),
                                ),
                            );
                            let middle = sigma(
                                "t2",
                                two(),
                                case_two(
                                    motive(),
                                    eq(v("d"), constant(negative_one)),
                                    innermost,
                                    v("t2"),
                                ),
                            );
                            pi(
                                "d",
                                carrier(),
                                sigma(
                                    "t",
                                    two(),
                                    case_two(
                                        motive(),
                                        le(v("d"), constant(negative_two)),
                                        middle,
                                        v("t"),
                                    ),
                                ),
                            )
                        }
                    };
                    Ok(statement)
                },
                arguments,
            )
        }

        fn integer_le_term(
            &mut self,
            left: TermHandle,
            right: TermHandle,
        ) -> Result<TermHandle, BoundedDenotationError> {
            let position = self.integer_less_or_equal()?;
            let relation = self.constant(position);
            Ok(self.arena.apply_binary(relation, left, right))
        }

        fn integer_numeral(&mut self, value: i128) -> Result<TermHandle, BoundedDenotationError> {
            let position = self.binary_integer(value < 0, value.unsigned_abs())?;
            Ok(self.constant(position))
        }

        fn integer_id_elim(
            &mut self,
            integer: TermHandle,
            from: TermHandle,
            endpoint: TermHandle,
            proof: TermHandle,
            base: TermHandle,
            family: impl FnOnce(&mut Self, TermHandle) -> Result<TermHandle, BoundedDenotationError>,
        ) -> Result<TermHandle, BoundedDenotationError> {
            let bound = self.arena.insert(Term::Variable(1));
            let body = family(self, bound)?;
            let identity = {
                let bound = self.arena.insert(Term::Variable(0));
                self.arena.insert(Term::Id {
                    ty: integer,
                    left: from,
                    right: bound,
                })
            };
            let inner = self.arena.insert(Term::Lambda {
                domain: identity,
                body,
            });
            let motive = self.arena.insert(Term::Lambda {
                domain: integer,
                body: inner,
            });
            Ok(self.arena.insert(Term::IdElim {
                motive,
                base,
                endpoint,
                proof,
            }))
        }

        fn solve_divisor(
            &mut self,
            integer_type: IntegerType,
            steps: &[ChainStep],
            target: TermHandle,
            hypothesis: TermHandle,
            mut value: i128,
            root: TermHandle,
        ) -> Result<Option<SolveOutcome>, BoundedDenotationError> {
            let integer = self.integer_constant()?;
            let mut current = target;
            let mut equation = hypothesis;
            let mut value_term = self.integer_numeral(value)?;
            for step in steps {
                let flipped = self.symmetry(integer, current, step.expression, step.definition);
                let mut bound = self.transitivity(
                    integer,
                    step.expression,
                    current,
                    value_term,
                    flipped,
                    equation,
                );
                let mut expression = step.expression;
                if let Some(literal) = step.literal {
                    let rewritten =
                        self.chain_operation_terms(step.operation, step.next, step.literal_term)?;
                    let congruence = self.chain_sibling_congruence(
                        integer,
                        step.operation,
                        step.next,
                        step.sibling,
                        step.literal_term,
                        literal,
                        step.expression,
                    )?;
                    let congruence = self.symmetry(integer, step.expression, rewritten, congruence);
                    bound = self.transitivity(
                        integer,
                        rewritten,
                        step.expression,
                        value_term,
                        congruence,
                        bound,
                    );
                    expression = rewritten;
                }
                match step.operation {
                    ChainOperation::Add => {
                        let Some(difference) = value.checked_sub(step.literal_value) else {
                            return Ok(None);
                        };
                        let difference_term = self.integer_numeral(difference)?;
                        let residual = self.subtract_terms(value_term, step.literal_term)?;
                        let inverse = self.roots_law_application(
                            Law::AddRightInverse,
                            &[step.next, step.literal_term, value_term, bound],
                        )?;
                        let Some(equation_step) = self.numeral_difference(
                            IntegerValue::Signed(value),
                            IntegerValue::Signed(step.literal_value),
                            IntegerValue::Signed(difference),
                            value_term,
                            step.literal_term,
                            difference_term,
                        )?
                        else {
                            return Ok(None);
                        };
                        equation = self.transitivity(
                            integer,
                            step.next,
                            residual,
                            difference_term,
                            inverse,
                            equation_step,
                        );
                        value = difference;
                        value_term = difference_term;
                    }
                    ChainOperation::Subtract => {
                        let Some(sum) = value.checked_add(step.literal_value) else {
                            return Ok(None);
                        };
                        let sum_term = self.integer_numeral(sum)?;
                        let residual = self.add_terms(value_term, step.literal_term)?;
                        let inverse = self.roots_law_application(
                            Law::SubtractRightInverse,
                            &[step.next, step.literal_term, value_term, bound],
                        )?;
                        let Some(equation_step) = self.numeral_sum(
                            IntegerValue::Signed(value),
                            IntegerValue::Signed(step.literal_value),
                            IntegerValue::Signed(sum),
                            value_term,
                            step.literal_term,
                            sum_term,
                        )?
                        else {
                            return Ok(None);
                        };
                        equation = self.transitivity(
                            integer,
                            step.next,
                            residual,
                            sum_term,
                            inverse,
                            equation_step,
                        );
                        value = sum;
                        value_term = sum_term;
                    }
                    ChainOperation::Multiply => {
                        if step.literal_value == 0 {
                            return Ok(None);
                        }
                        let Some(quotient) = value.checked_div(step.literal_value) else {
                            return Ok(None);
                        };
                        let quotient_term = self.integer_numeral(quotient)?;
                        let le_product_value = self.integer_law_application(
                            IntegerLaw::EqualityToLessOrEqual,
                            &[expression, value_term, bound],
                        )?;
                        let le_value_product = {
                            let flipped = self.symmetry(integer, expression, value_term, bound);
                            self.integer_law_application(
                                IntegerLaw::EqualityToLessOrEqual,
                                &[value_term, expression, flipped],
                            )?
                        };
                        let positive = step.literal_value > 0;
                        let Some(sign) = (if positive {
                            self.integer_le_numeral(
                                IntegerValue::Signed(1),
                                IntegerValue::Signed(step.literal_value),
                            )?
                        } else {
                            self.integer_le_numeral(
                                IntegerValue::Signed(step.literal_value),
                                IntegerValue::Signed(-1),
                            )?
                        }) else {
                            return Ok(None);
                        };
                        let divide =
                            self.integer_operation(IntegerOperation::ExactDivide(integer_type))?;
                        let quotient_app = {
                            let function = self.constant(divide);
                            let function = self.arena.insert(Term::Apply {
                                function,
                                argument: value_term,
                            });
                            self.arena.insert(Term::Apply {
                                function,
                                argument: step.literal_term,
                            })
                        };
                        let (upper_law, upper_bound, lower_law, lower_bound) = if positive {
                            (
                                Law::QuotientUpperPositive(integer_type),
                                le_product_value,
                                Law::QuotientLowerPositive(integer_type),
                                le_value_product,
                            )
                        } else {
                            (
                                Law::QuotientUpperNegative(integer_type),
                                le_value_product,
                                Law::QuotientLowerNegative(integer_type),
                                le_product_value,
                            )
                        };
                        let upper = self.roots_law_application(
                            upper_law,
                            &[step.literal_term, value_term, step.next, sign, upper_bound],
                        )?;
                        let lower = self.roots_law_application(
                            lower_law,
                            &[step.literal_term, value_term, step.next, sign, lower_bound],
                        )?;
                        let to_quotient = self.roots_law_application(
                            Law::LessOrEqualAntisymmetry,
                            &[step.next, quotient_app, upper, lower],
                        )?;
                        let Some(numeral) = self.numeral_quotient(
                            integer_type,
                            IntegerValue::Signed(value),
                            IntegerValue::Signed(step.literal_value),
                            IntegerValue::Signed(quotient),
                            value_term,
                            step.literal_term,
                            quotient_term,
                        )?
                        else {
                            return Ok(None);
                        };
                        equation = self.transitivity(
                            integer,
                            step.next,
                            quotient_app,
                            quotient_term,
                            to_quotient,
                            numeral,
                        );
                        if quotient.checked_mul(step.literal_value) != Some(value) {
                            let Some(refuted) = self.nondivisible_contradiction(
                                integer,
                                step.next,
                                quotient_term,
                                step.literal_term,
                                value_term,
                                value,
                                quotient,
                                step.literal_value,
                                equation,
                                bound,
                            )?
                            else {
                                return Ok(None);
                            };
                            return Ok(Some(SolveOutcome::Refuted(refuted)));
                        }
                        value = quotient;
                        value_term = quotient_term;
                    }
                }
                current = step.next;
            }
            if !self.arena.structurally_equal(current, root) {
                return Ok(None);
            }
            Ok(Some(SolveOutcome::Root {
                equality: equation,
                quotient: value,
                quotient_term: value_term,
            }))
        }

        fn nondivisible_contradiction(
            &mut self,
            integer: TermHandle,
            next: TermHandle,
            quotient_term: TermHandle,
            literal_term: TermHandle,
            value_term: TermHandle,
            value: i128,
            quotient: i128,
            literal: i128,
            root_equality: TermHandle,
            bound: TermHandle,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            let Some(product_value) = quotient.checked_mul(literal) else {
                return Ok(None);
            };
            if product_value == value {
                return Ok(None);
            }
            let rewritten = self.integer_id_elim(
                integer,
                next,
                quotient_term,
                root_equality,
                bound,
                |this, bound| {
                    let product = this.multiply_terms(bound, literal_term)?;
                    Ok(this.arena.insert(Term::Id {
                        ty: integer,
                        left: product,
                        right: value_term,
                    }))
                },
            )?;
            let product_term = self.integer_numeral(product_value)?;
            let Some(product_equation) = self.numeral_product(
                IntegerValue::Signed(quotient),
                IntegerValue::Signed(literal),
                IntegerValue::Signed(product_value),
                quotient_term,
                literal_term,
                product_term,
            )?
            else {
                return Ok(None);
            };
            let to_product = {
                let product_app = self.multiply_terms(quotient_term, literal_term)?;
                let flipped = self.symmetry(integer, product_app, value_term, rewritten);
                self.transitivity(
                    integer,
                    value_term,
                    product_app,
                    product_term,
                    flipped,
                    product_equation,
                )
            };
            let diagonal = {
                let flipped = self.symmetry(integer, value_term, product_term, to_product);
                if product_value < value {
                    let strict = self.literal_order(
                        IntegerValue::Signed(product_value),
                        IntegerValue::Signed(value),
                    )?;
                    self.integer_law_application(
                        IntegerLaw::LessThanSubstituteLeft,
                        &[product_term, value_term, value_term, flipped, strict],
                    )?
                } else {
                    let strict = self.literal_order(
                        IntegerValue::Signed(value),
                        IntegerValue::Signed(product_value),
                    )?;
                    self.integer_law_application(
                        IntegerLaw::LessThanSubstituteRight,
                        &[value_term, product_term, value_term, flipped, strict],
                    )?
                }
            };
            self.irreflexive(value_term, diagonal).map(Some)
        }

        fn eval_dividend(
            &mut self,
            steps: &[ChainStep],
            equation: TermHandle,
            root: TermHandle,
            mut value: i128,
            dividend: TermHandle,
        ) -> Result<Option<(i128, TermHandle)>, BoundedDenotationError> {
            let integer = self.integer_constant()?;
            let mut equation = equation;
            let mut inner = root;
            let mut value_term = self.integer_numeral(value)?;
            for step in steps {
                let operand = self.chain_operand_congruence(
                    integer,
                    step.operation,
                    step.sibling,
                    inner,
                    value_term,
                    equation,
                )?;
                let evaluated_sibling =
                    self.chain_operation_terms(step.operation, value_term, step.sibling)?;
                let mut bound = self.transitivity(
                    integer,
                    step.current,
                    step.expression,
                    evaluated_sibling,
                    step.definition,
                    operand,
                );
                let mut expression = evaluated_sibling;
                if let Some(literal) = step.literal {
                    let congruence = self.chain_sibling_congruence(
                        integer,
                        step.operation,
                        value_term,
                        step.sibling,
                        step.literal_term,
                        literal,
                        evaluated_sibling,
                    )?;
                    let evaluated_literal =
                        self.chain_operation_terms(step.operation, value_term, step.literal_term)?;
                    bound = self.transitivity(
                        integer,
                        step.current,
                        evaluated_sibling,
                        evaluated_literal,
                        bound,
                        congruence,
                    );
                    expression = evaluated_literal;
                }
                let Some(evaluated) =
                    Self::chain_operation_eval(step.operation, value, step.literal_value)
                else {
                    return Ok(None);
                };
                let evaluated_term = self.integer_numeral(evaluated)?;
                let Some(equation_step) = (match step.operation {
                    ChainOperation::Add => self.numeral_sum(
                        IntegerValue::Signed(value),
                        IntegerValue::Signed(step.literal_value),
                        IntegerValue::Signed(evaluated),
                        value_term,
                        step.literal_term,
                        evaluated_term,
                    ),
                    ChainOperation::Subtract => self.numeral_difference(
                        IntegerValue::Signed(value),
                        IntegerValue::Signed(step.literal_value),
                        IntegerValue::Signed(evaluated),
                        value_term,
                        step.literal_term,
                        evaluated_term,
                    ),
                    ChainOperation::Multiply => self.numeral_product(
                        IntegerValue::Signed(value),
                        IntegerValue::Signed(step.literal_value),
                        IntegerValue::Signed(evaluated),
                        value_term,
                        step.literal_term,
                        evaluated_term,
                    ),
                })?
                else {
                    return Ok(None);
                };
                equation = self.transitivity(
                    integer,
                    step.current,
                    expression,
                    evaluated_term,
                    bound,
                    equation_step,
                );
                inner = step.current;
                value = evaluated;
                value_term = evaluated_term;
            }
            if !self.arena.structurally_equal(inner, dividend) {
                return Ok(None);
            }
            Ok(Some((value, equation)))
        }

        fn empty_elimination(&mut self, ty: TermHandle, scrutinee: TermHandle) -> TermHandle {
            self.arena.insert(Term::EmptyElim { ty, scrutinee })
        }

        fn bounds_contradiction(
            &mut self,
            quotient: i128,
            quotient_term: TermHandle,
            lower: i128,
            upper: i128,
            lower_term: TermHandle,
            upper_term: TermHandle,
            lower_evidence: TermHandle,
            upper_evidence: TermHandle,
            root: TermHandle,
            equality: TermHandle,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            if quotient < lower {
                let strict = self
                    .literal_order(IntegerValue::Signed(quotient), IntegerValue::Signed(lower))?;
                let transported = self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteRight,
                    &[lower_term, root, quotient_term, equality, lower_evidence],
                )?;
                let contradiction = self.integer_law_application(
                    IntegerLaw::LessOrEqualLessThanTransitivity,
                    &[lower_term, quotient_term, lower_term, transported, strict],
                )?;
                self.irreflexive(lower_term, contradiction).map(Some)
            } else if quotient > upper {
                let strict = self
                    .literal_order(IntegerValue::Signed(upper), IntegerValue::Signed(quotient))?;
                let transported = self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteLeft,
                    &[root, quotient_term, upper_term, equality, upper_evidence],
                )?;
                let contradiction = self.integer_law_application(
                    IntegerLaw::LessThanLessOrEqualTransitivity,
                    &[upper_term, quotient_term, upper_term, strict, transported],
                )?;
                self.irreflexive(upper_term, contradiction).map(Some)
            } else {
                Ok(None)
            }
        }

        fn tagged_sum_elim(
            &mut self,
            zero_case: TermHandle,
            rest_case: TermHandle,
            scrutinee: TermHandle,
            zero_branch: TermHandle,
            one_branch: TermHandle,
            goal: TermHandle,
        ) -> TermHandle {
            let inner_motive = self.arena.insert(Term::Lambda {
                domain: self.two,
                body: self.type_zero,
            });
            let bound = self.arena.insert(Term::Variable(0));
            let family = self.arena.insert(Term::CaseTwo {
                motive: inner_motive,
                zero_branch: zero_case,
                one_branch: rest_case,
                scrutinee: bound,
            });
            let body = self.arena.insert(Term::Pi {
                domain: family,
                codomain: goal,
            });
            let motive = self.arena.insert(Term::Lambda {
                domain: self.two,
                body,
            });
            let tag = self.arena.insert(Term::Fst { pair: scrutinee });
            let elimination = self.arena.insert(Term::CaseTwo {
                motive,
                zero_branch,
                one_branch,
                scrutinee: tag,
            });
            let payload = self.arena.insert(Term::Snd { pair: scrutinee });
            self.arena.insert(Term::Apply {
                function: elimination,
                argument: payload,
            })
        }

        fn disjunct(&mut self, index: usize, payload: TermHandle) -> TermHandle {
            let zero = self.arena.insert(Term::TwoZero);
            let one = self.arena.insert(Term::TwoOne);
            match index {
                0 => self.arena.insert(Term::Pair {
                    first: zero,
                    second: payload,
                }),
                1 => {
                    let inner = self.arena.insert(Term::Pair {
                        first: zero,
                        second: payload,
                    });
                    self.arena.insert(Term::Pair {
                        first: one,
                        second: inner,
                    })
                }
                _ => {
                    let inner = self.arena.insert(Term::Pair {
                        first: one,
                        second: payload,
                    });
                    self.arena.insert(Term::Pair {
                        first: one,
                        second: inner,
                    })
                }
            }
        }

        fn negative_one_bound(
            &mut self,
            checked: &CheckedIntegerCorrelatedForbiddenRoots,
            integer_type: IntegerType,
            dividend_steps: &[ChainStep],
            outcome: SolveOutcome,
            interval: (i128, i128),
            lower_term: TermHandle,
            upper_term: TermHandle,
            lower_evidence: TermHandle,
            upper_evidence: TermHandle,
            root_term: TermHandle,
            dividend_term: TermHandle,
            bound_type: TermHandle,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            let integer = self.integer_constant()?;
            let (lower, upper) = interval;
            let IntegerValue::Signed(minimum) = integer_type.minimum_value() else {
                return Ok(None);
            };
            let IntegerValue::Signed(maximum) = integer_type.maximum_value() else {
                return Ok(None);
            };
            match outcome {
                SolveOutcome::Refuted(empty) => Ok(Some(self.empty_elimination(bound_type, empty))),
                SolveOutcome::Root {
                    equality,
                    quotient,
                    quotient_term,
                } => {
                    if quotient < lower || quotient > upper {
                        let Some(empty) = self.bounds_contradiction(
                            quotient,
                            quotient_term,
                            lower,
                            upper,
                            lower_term,
                            upper_term,
                            lower_evidence,
                            upper_evidence,
                            root_term,
                            equality,
                        )?
                        else {
                            return Ok(None);
                        };
                        return Ok(Some(self.empty_elimination(bound_type, empty)));
                    }
                    let Some((evaluated, evaluation)) = self.eval_dividend(
                        dividend_steps,
                        equality,
                        root_term,
                        quotient,
                        dividend_term,
                    )?
                    else {
                        return Ok(None);
                    };
                    if evaluated == minimum {
                        return Ok(None);
                    }
                    let evaluated_term = self.integer_numeral(evaluated)?;
                    if evaluated > minimum && evaluated <= maximum {
                        let Some(minimum_plus_one) = minimum.checked_add(1) else {
                            return Ok(None);
                        };
                        let minimum_plus_one_term = self.integer_numeral(minimum_plus_one)?;
                        let Some(order) = self.integer_le_numeral(
                            IntegerValue::Signed(minimum_plus_one),
                            IntegerValue::Signed(evaluated),
                        )?
                        else {
                            return Ok(None);
                        };
                        let flipped =
                            self.symmetry(integer, dividend_term, evaluated_term, evaluation);
                        self.integer_law_application(
                            IntegerLaw::LessOrEqualSubstituteRight,
                            &[
                                minimum_plus_one_term,
                                evaluated_term,
                                dividend_term,
                                flipped,
                                order,
                            ],
                        )
                        .map(Some)
                    } else {
                        let empty = if evaluated < minimum {
                            let (endpoint, membership) =
                                self.carrier_bound(checked.dividend(), true, &integer_type)?;
                            let endpoint_term = self.fixed_scalar_term(&endpoint)?;
                            let transported = self.integer_law_application(
                                IntegerLaw::LessOrEqualSubstituteRight,
                                &[
                                    endpoint_term,
                                    dividend_term,
                                    evaluated_term,
                                    evaluation,
                                    membership,
                                ],
                            )?;
                            let strict = self.literal_order(
                                IntegerValue::Signed(evaluated),
                                IntegerValue::Signed(minimum),
                            )?;
                            let contradiction = self.integer_law_application(
                                IntegerLaw::LessOrEqualLessThanTransitivity,
                                &[
                                    endpoint_term,
                                    evaluated_term,
                                    endpoint_term,
                                    transported,
                                    strict,
                                ],
                            )?;
                            self.irreflexive(endpoint_term, contradiction)?
                        } else {
                            let (endpoint, membership) =
                                self.carrier_bound(checked.dividend(), false, &integer_type)?;
                            let endpoint_term = self.fixed_scalar_term(&endpoint)?;
                            let transported = self.integer_law_application(
                                IntegerLaw::LessOrEqualSubstituteLeft,
                                &[
                                    dividend_term,
                                    evaluated_term,
                                    endpoint_term,
                                    evaluation,
                                    membership,
                                ],
                            )?;
                            let strict = self.literal_order(
                                IntegerValue::Signed(maximum),
                                IntegerValue::Signed(evaluated),
                            )?;
                            let contradiction = self.integer_law_application(
                                IntegerLaw::LessThanLessOrEqualTransitivity,
                                &[
                                    endpoint_term,
                                    evaluated_term,
                                    endpoint_term,
                                    strict,
                                    transported,
                                ],
                            )?;
                            self.irreflexive(endpoint_term, contradiction)?
                        };
                        Ok(Some(self.empty_elimination(bound_type, empty)))
                    }
                }
            }
        }

        pub(super) fn correlated_forbidden_roots_evidence(
            &mut self,
            witness: &IntegerCorrelatedForbiddenRootWitness,
            checked: &CheckedIntegerCorrelatedForbiddenRoots,
            axiom_evidence: &[(Proposition, TermHandle)],
            bound_evidence: &[(Proposition, TermHandle)],
            conclusion: &Proposition,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            let integer_type = checked.integer_type();
            if integer_type.carrier() != IntegerCarrier::Fixed
                || integer_type.sign() != IntegerSign::Signed
            {
                return Ok(None);
            }
            let IntegerValue::Signed(minimum) = integer_type.minimum_value() else {
                return Ok(None);
            };
            let Some(minimum_plus_one) = minimum.checked_add(1) else {
                return Ok(None);
            };
            let integer = self.integer_constant()?;
            let root_term = self.fixed_scalar_term(checked.root())?;
            let dividend_term = self.fixed_scalar_term(checked.dividend())?;
            let divisor_term = self.fixed_scalar_term(checked.divisor())?;
            let (lower, upper) = checked.interval();

            let mut cursor = 0;
            let Some(dividend_steps) =
                self.chain_steps(&witness.dividend, integer_type, axiom_evidence, &mut cursor)?
            else {
                return Ok(None);
            };
            let Some(divisor_steps) =
                self.chain_steps(&witness.divisor, integer_type, axiom_evidence, &mut cursor)?
            else {
                return Ok(None);
            };
            if cursor != axiom_evidence.len()
                || dividend_steps.is_empty()
                || divisor_steps.is_empty()
            {
                return Ok(None);
            }
            let chain = |denotation: &Self, steps: &[ChainStep], target: TermHandle| {
                denotation
                    .arena
                    .structurally_equal(steps[0].next, root_term)
                    && steps.windows(2).all(|pair| {
                        denotation
                            .arena
                            .structurally_equal(pair[0].current, pair[1].next)
                    })
                    && denotation
                        .arena
                        .structurally_equal(steps.last().expect("nonempty").current, target)
            };
            if !chain(self, &dividend_steps, dividend_term)
                || !chain(self, &divisor_steps, divisor_term)
            {
                return Ok(None);
            }

            let [lower_bound, upper_bound] = bound_evidence else {
                return Ok(None);
            };
            let mut bound_side =
                |proposition: &Proposition,
                 interval_value: i128,
                 root_on_right: bool|
                 -> Result<Option<TermHandle>, BoundedDenotationError> {
                    let denoted = self.denote(proposition)?;
                    let Some(IntegerRelation::LessOrEqual { left, right }) =
                        self.integer_relation(denoted)
                    else {
                        return Ok(None);
                    };
                    let bound_term = self.integer_numeral(interval_value)?;
                    let (endpoint, operand) = if root_on_right {
                        (left, right)
                    } else {
                        (right, left)
                    };
                    Ok((self.arena.structurally_equal(endpoint, bound_term)
                        && self.arena.structurally_equal(operand, root_term))
                    .then_some(bound_term))
                };
            let Some(lower_term) = bound_side(&lower_bound.0, lower, true)? else {
                return Ok(None);
            };
            let Some(upper_term) = bound_side(&upper_bound.0, upper, false)? else {
                return Ok(None);
            };

            let zero_term = self.integer_numeral(0)?;
            let one_term = self.integer_numeral(1)?;
            let negative_one_term = self.integer_numeral(-1)?;
            let negative_two_term = self.integer_numeral(-2)?;
            let minimum_plus_one_term = self.integer_numeral(minimum_plus_one)?;
            let disjunct_zero = self.integer_le_term(divisor_term, negative_two_term)?;
            let disjunct_one = self.integer_le_term(one_term, divisor_term)?;
            let conjunct_bound = self.integer_le_term(minimum_plus_one_term, dividend_term)?;
            let divisor_le_one = self.integer_le_term(divisor_term, negative_one_term)?;
            let conjunction = self.arena.insert(Term::Sigma {
                domain: divisor_le_one,
                codomain: conjunct_bound,
            });
            let rest = self.tagged_sum(disjunct_one, conjunction);
            let expected = self.tagged_sum(disjunct_zero, rest);
            let goal = self.denote(conclusion)?;
            if !self.arena.structurally_equal(expected, goal) {
                return Ok(None);
            }

            let case_neg_one = self.arena.insert(Term::Id {
                ty: integer,
                left: divisor_term,
                right: negative_one_term,
            });
            let case_zero = self.arena.insert(Term::Id {
                ty: integer,
                left: divisor_term,
                right: zero_term,
            });

            let dividend_lifted: Vec<ChainStep> = dividend_steps
                .iter()
                .map(|step| self.chain_step_lift(step, 1))
                .collect();
            let divisor_lifted: Vec<ChainStep> = divisor_steps
                .iter()
                .map(|step| self.chain_step_lift(step, 1))
                .collect();
            let divisor_solve: Vec<ChainStep> = divisor_lifted.iter().rev().cloned().collect();
            let lower_evidence = shift(&mut self.arena, lower_bound.1, 0, 1);
            let upper_evidence = shift(&mut self.arena, upper_bound.1, 0, 1);
            let hypothesis = self.arena.insert(Term::Variable(0));

            let zero_body = match self.solve_divisor(
                integer_type,
                &divisor_solve,
                divisor_term,
                hypothesis,
                0,
                root_term,
            )? {
                None => return Ok(None),
                Some(SolveOutcome::Refuted(empty)) => self.empty_elimination(goal, empty),
                Some(SolveOutcome::Root {
                    equality,
                    quotient,
                    quotient_term,
                }) => {
                    let Some(empty) = self.bounds_contradiction(
                        quotient,
                        quotient_term,
                        lower,
                        upper,
                        lower_term,
                        upper_term,
                        lower_evidence,
                        upper_evidence,
                        root_term,
                        equality,
                    )?
                    else {
                        return Ok(None);
                    };
                    self.empty_elimination(goal, empty)
                }
            };
            let zero_branch = self.arena.insert(Term::Lambda {
                domain: case_zero,
                body: zero_body,
            });

            let Some(negative_one_outcome) = self.solve_divisor(
                integer_type,
                &divisor_solve,
                divisor_term,
                hypothesis,
                -1,
                root_term,
            )?
            else {
                return Ok(None);
            };
            let Some(bound_evidence) = self.negative_one_bound(
                checked,
                integer_type,
                &dividend_lifted,
                negative_one_outcome,
                (lower, upper),
                lower_term,
                upper_term,
                lower_evidence,
                upper_evidence,
                root_term,
                dividend_term,
                conjunct_bound,
            )?
            else {
                return Ok(None);
            };
            let divisor_le_one_evidence = self.integer_law_application(
                IntegerLaw::EqualityToLessOrEqual,
                &[divisor_term, negative_one_term, hypothesis],
            )?;
            let conjunction_payload = self.arena.insert(Term::Pair {
                first: divisor_le_one_evidence,
                second: bound_evidence,
            });
            let negative_one_body = self.disjunct(2, conjunction_payload);
            let negative_one_branch = self.arena.insert(Term::Lambda {
                domain: case_neg_one,
                body: negative_one_body,
            });

            let negative_body = self.disjunct(0, hypothesis);
            let negative_branch = self.arena.insert(Term::Lambda {
                domain: disjunct_zero,
                body: negative_body,
            });
            let positive_body = self.disjunct(1, hypothesis);
            let positive_branch = self.arena.insert(Term::Lambda {
                domain: disjunct_one,
                body: positive_body,
            });

            let split = self.roots_law_application(Law::DivisorCaseSplit, &[divisor_term])?;
            let inner_sum = self.tagged_sum(case_zero, disjunct_one);
            let middle_sum = self.tagged_sum(case_neg_one, inner_sum);
            let level_three = {
                let scrutinee = self.arena.insert(Term::Variable(0));
                let zero_lifted = shift(&mut self.arena, zero_branch, 0, 2);
                let positive_lifted = shift(&mut self.arena, positive_branch, 0, 2);
                self.tagged_sum_elim(
                    case_zero,
                    disjunct_one,
                    scrutinee,
                    zero_lifted,
                    positive_lifted,
                    goal,
                )
            };
            let middle_one = self.arena.insert(Term::Lambda {
                domain: inner_sum,
                body: level_three,
            });
            let level_two = {
                let scrutinee = self.arena.insert(Term::Variable(0));
                let negative_one_lifted = shift(&mut self.arena, negative_one_branch, 0, 1);
                self.tagged_sum_elim(
                    case_neg_one,
                    inner_sum,
                    scrutinee,
                    negative_one_lifted,
                    middle_one,
                    goal,
                )
            };
            let top_one = self.arena.insert(Term::Lambda {
                domain: middle_sum,
                body: level_two,
            });
            let tag = self.arena.insert(Term::Fst { pair: split });
            let elimination = {
                let inner_motive = self.arena.insert(Term::Lambda {
                    domain: self.two,
                    body: self.type_zero,
                });
                let bound = self.arena.insert(Term::Variable(0));
                let family = self.arena.insert(Term::CaseTwo {
                    motive: inner_motive,
                    zero_branch: disjunct_zero,
                    one_branch: middle_sum,
                    scrutinee: bound,
                });
                let body = self.arena.insert(Term::Pi {
                    domain: family,
                    codomain: goal,
                });
                let motive = self.arena.insert(Term::Lambda {
                    domain: self.two,
                    body,
                });
                self.arena.insert(Term::CaseTwo {
                    motive,
                    zero_branch: negative_branch,
                    one_branch: top_one,
                    scrutinee: tag,
                })
            };
            let payload = self.arena.insert(Term::Snd { pair: split });
            Ok(Some(self.arena.insert(Term::Apply {
                function: elimination,
                argument: payload,
            })))
        }
    }

    #[cfg(test)]
    mod tests {
        use std::collections::BTreeSet;

        use super::super::{Elaboration, verify_bounded_certificate_with_machine_parameters};
        use super::Law;
        use crate::mathematical_core::Term;
        use crate::{
            Budget, CorrelatedAffineBranchWitness, IntegerCorrelatedForbiddenRootWitness,
            ProofError, ProofNode, ProofRule, correlated_step, proof_node, value_context,
        };
        use semantic_vocabulary::{
            IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
            ScalarType, ValueId,
        };

        fn i8() -> IntegerType {
            IntegerType::new(IntegerSign::Signed, 8).expect("i8")
        }

        fn value(index: u64, integer_type: IntegerType) -> ScalarTerm {
            ScalarTerm::value(
                ValueId::new(index).expect("value id"),
                ScalarType::Integer(integer_type),
            )
        }

        fn integer(integer_type: IntegerType, value: i128) -> ScalarTerm {
            ScalarTerm::integer(integer_type, IntegerValue::Signed(value)).expect("integer")
        }

        fn equal(target: ScalarTerm, expression: ScalarTerm) -> Proposition {
            Proposition::Equal(target, expression)
        }

        fn add(
            target: ScalarTerm,
            integer_type: IntegerType,
            left: ScalarTerm,
            right: ScalarTerm,
        ) -> Proposition {
            equal(
                target,
                ScalarTerm::exact_integer_add(integer_type, left, right).expect("add"),
            )
        }

        fn subtract(
            target: ScalarTerm,
            integer_type: IntegerType,
            left: ScalarTerm,
            right: ScalarTerm,
        ) -> Proposition {
            equal(
                target,
                ScalarTerm::exact_integer_subtract(integer_type, left, right).expect("subtract"),
            )
        }

        fn multiply(
            target: ScalarTerm,
            integer_type: IntegerType,
            left: ScalarTerm,
            right: ScalarTerm,
        ) -> Proposition {
            equal(
                target,
                ScalarTerm::exact_integer_multiply(integer_type, left, right).expect("multiply"),
            )
        }

        fn definedness(
            integer_type: IntegerType,
            dividend: &ScalarTerm,
            divisor: &ScalarTerm,
        ) -> Proposition {
            let minimum_plus_one = match integer_type.minimum_value() {
                IntegerValue::Signed(minimum) => minimum + 1,
                _ => unreachable!("signed fixed carrier"),
            };
            Proposition::Disjunction(vec![
                Proposition::LessOrEqual(divisor.clone(), integer(integer_type, -2)),
                Proposition::LessOrEqual(integer(integer_type, 1), divisor.clone()),
                Proposition::Conjunction(vec![
                    Proposition::LessOrEqual(divisor.clone(), integer(integer_type, -1)),
                    Proposition::LessOrEqual(
                        integer(integer_type, minimum_plus_one),
                        dividend.clone(),
                    ),
                ]),
            ])
        }

        struct Fixture {
            context: PropositionContext,
            axioms: Vec<Proposition>,
            premises: Vec<Proposition>,
            parameters: BTreeSet<ValueId>,
            goal: Proposition,
            proof: ProofNode,
        }

        fn safe_fixture() -> Fixture {
            let integer_type = i8();
            let root = value(1, integer_type);
            let sixty_four = value(2, integer_type);
            let left_offset = value(3, integer_type);
            let negative_two = value(4, integer_type);
            let dividend = value(5, integer_type);
            let two = value(6, integer_type);
            let right_product = value(7, integer_type);
            let divisor = value(8, integer_type);
            let lower = Proposition::LessOrEqual(integer(integer_type, -1), root.clone());
            let upper = Proposition::LessOrEqual(root.clone(), integer(integer_type, 0));
            let axioms = vec![
                equal(sixty_four.clone(), integer(integer_type, 64)),
                add(left_offset.clone(), integer_type, root.clone(), sixty_four),
                equal(negative_two.clone(), integer(integer_type, -2)),
                multiply(dividend.clone(), integer_type, left_offset, negative_two),
                equal(two.clone(), integer(integer_type, 2)),
                multiply(right_product.clone(), integer_type, root.clone(), two),
                add(
                    divisor.clone(),
                    integer_type,
                    right_product,
                    integer(integer_type, 1),
                ),
            ];
            let context = value_context(ScalarType::Integer(integer_type), 8);
            let witness = IntegerCorrelatedForbiddenRootWitness {
                dividend: CorrelatedAffineBranchWitness {
                    root: root.clone(),
                    target: dividend.clone(),
                    steps: vec![correlated_step(1, Some(0)), correlated_step(3, Some(2))],
                },
                divisor: CorrelatedAffineBranchWitness {
                    root,
                    target: divisor.clone(),
                    steps: vec![correlated_step(5, Some(4)), correlated_step(6, None)],
                },
                definition_axiom_count: 7,
                lower_bound_axiom: 7,
                upper_bound_axiom: 8,
                conclusion: Proposition::Conjunction(vec![lower.clone(), upper.clone()]),
            };
            let goal = definedness(integer_type, &dividend, &divisor);
            Fixture {
                context,
                axioms,
                premises: vec![lower, upper],
                parameters: BTreeSet::from([ValueId::new(1).expect("root")]),
                goal: goal.clone(),
                proof: proof_node!(goal => ProofRule::IntegerCorrelatedForbiddenRoots { witness }),
            }
        }

        fn negative_multiplier_fixture() -> Fixture {
            let integer_type = i8();
            let root = value(1, integer_type);
            let dividend = value(2, integer_type);
            let negative_two = value(3, integer_type);
            let right_product = value(4, integer_type);
            let divisor = value(5, integer_type);
            let lower = Proposition::LessOrEqual(integer(integer_type, -2), root.clone());
            let upper = Proposition::LessOrEqual(root.clone(), integer(integer_type, -2));
            let axioms = vec![
                subtract(
                    dividend.clone(),
                    integer_type,
                    root.clone(),
                    integer(integer_type, 100),
                ),
                equal(negative_two.clone(), integer(integer_type, -2)),
                multiply(
                    right_product.clone(),
                    integer_type,
                    root.clone(),
                    negative_two,
                ),
                subtract(
                    divisor.clone(),
                    integer_type,
                    right_product,
                    integer(integer_type, 5),
                ),
            ];
            let context = value_context(ScalarType::Integer(integer_type), 5);
            let witness = IntegerCorrelatedForbiddenRootWitness {
                dividend: CorrelatedAffineBranchWitness {
                    root: root.clone(),
                    target: dividend.clone(),
                    steps: vec![correlated_step(0, None)],
                },
                divisor: CorrelatedAffineBranchWitness {
                    root,
                    target: divisor.clone(),
                    steps: vec![correlated_step(2, Some(1)), correlated_step(3, None)],
                },
                definition_axiom_count: 4,
                lower_bound_axiom: 4,
                upper_bound_axiom: 5,
                conclusion: Proposition::Conjunction(vec![lower.clone(), upper.clone()]),
            };
            let goal = definedness(integer_type, &dividend, &divisor);
            Fixture {
                context,
                axioms,
                premises: vec![lower, upper],
                parameters: BTreeSet::from([ValueId::new(1).expect("root")]),
                goal: goal.clone(),
                proof: proof_node!(goal => ProofRule::IntegerCorrelatedForbiddenRoots { witness }),
            }
        }

        fn check(fixture: &Fixture, laws: &[Law]) {
            verify_bounded_certificate_with_machine_parameters(
                &fixture.context,
                &fixture.goal,
                &fixture.premises,
                &fixture.axioms,
                &fixture.parameters,
                &fixture.proof,
                &mut Budget::default(),
            )
            .unwrap();
            let mut elaboration = Elaboration::new(
                &fixture.context,
                &fixture.goal,
                &fixture.premises,
                &fixture.axioms,
                &fixture.parameters,
            )
            .unwrap();
            let evidence = elaboration.node(&fixture.proof).unwrap();
            assert!(elaboration.denotation.rule_axioms.is_empty());
            assert!(elaboration.denotation.decisions.is_empty());
            for law in laws {
                assert!(
                    elaboration
                        .denotation
                        .forbidden_roots
                        .laws
                        .contains_key(law),
                    "missing {law:?}"
                );
            }
            let term = elaboration.denotation.arena.get(evidence);
            assert!(!matches!(term, Term::Constant { .. }));
        }

        #[test]
        fn forbidden_roots_positive_multiplier_in_interval_root() {
            let integer = i8();
            let fixture = safe_fixture();
            check(
                &fixture,
                &[
                    Law::AddRightInverse,
                    Law::QuotientUpperPositive(integer),
                    Law::QuotientLowerPositive(integer),
                    Law::LessOrEqualAntisymmetry,
                    Law::DivisorCaseSplit,
                ],
            );
        }

        #[test]
        fn forbidden_roots_negative_multiplier_in_interval_root() {
            let integer = i8();
            let fixture = negative_multiplier_fixture();
            check(
                &fixture,
                &[
                    Law::SubtractRightInverse,
                    Law::QuotientUpperNegative(integer),
                    Law::QuotientLowerNegative(integer),
                    Law::LessOrEqualAntisymmetry,
                    Law::DivisorCaseSplit,
                ],
            );
        }

        #[test]
        fn forbidden_roots_out_of_interval_root_refutes_case() {
            let integer_type = i8();
            let root = value(1, integer_type);
            let dividend = value(2, integer_type);
            let negative_two = value(3, integer_type);
            let right_product = value(4, integer_type);
            let divisor = value(5, integer_type);
            let lower = Proposition::LessOrEqual(integer(integer_type, 2), root.clone());
            let upper = Proposition::LessOrEqual(root.clone(), integer(integer_type, 2));
            let axioms = vec![
                add(
                    dividend.clone(),
                    integer_type,
                    root.clone(),
                    integer(integer_type, 1),
                ),
                equal(negative_two.clone(), integer(integer_type, -2)),
                multiply(
                    right_product.clone(),
                    integer_type,
                    root.clone(),
                    negative_two,
                ),
                add(
                    divisor.clone(),
                    integer_type,
                    right_product,
                    integer(integer_type, 1),
                ),
            ];
            let context = value_context(ScalarType::Integer(integer_type), 5);
            let witness = IntegerCorrelatedForbiddenRootWitness {
                dividend: CorrelatedAffineBranchWitness {
                    root: root.clone(),
                    target: dividend.clone(),
                    steps: vec![correlated_step(0, None)],
                },
                divisor: CorrelatedAffineBranchWitness {
                    root,
                    target: divisor.clone(),
                    steps: vec![correlated_step(2, Some(1)), correlated_step(3, None)],
                },
                definition_axiom_count: 4,
                lower_bound_axiom: 4,
                upper_bound_axiom: 5,
                conclusion: Proposition::Conjunction(vec![lower.clone(), upper.clone()]),
            };
            let goal = definedness(integer_type, &dividend, &divisor);
            let fixture = Fixture {
                context,
                axioms,
                premises: vec![lower, upper],
                parameters: BTreeSet::from([ValueId::new(1).expect("root")]),
                goal: goal.clone(),
                proof: proof_node!(goal => ProofRule::IntegerCorrelatedForbiddenRoots { witness }),
            };
            check(
                &fixture,
                &[
                    Law::AddRightInverse,
                    Law::QuotientUpperNegative(integer_type),
                    Law::QuotientLowerNegative(integer_type),
                    Law::LessOrEqualAntisymmetry,
                    Law::DivisorCaseSplit,
                ],
            );
        }

        #[test]
        fn forbidden_roots_rejects_mutated_conclusion() {
            let fixture = safe_fixture();
            let integer_type = i8();
            let dividend = value(5, integer_type);
            let divisor = value(8, integer_type);
            let drifted = Proposition::Disjunction(vec![
                Proposition::LessOrEqual(divisor.clone(), integer(integer_type, -2)),
                Proposition::LessOrEqual(integer(integer_type, 1), divisor.clone()),
                Proposition::Conjunction(vec![
                    Proposition::LessOrEqual(divisor, integer(integer_type, -1)),
                    Proposition::LessOrEqual(integer(integer_type, -126), dividend),
                ]),
            ]);
            let ProofRule::IntegerCorrelatedForbiddenRoots { witness } = &fixture.proof.rule else {
                unreachable!()
            };
            let proof = ProofNode {
                conclusion: drifted,
                rule: ProofRule::IntegerCorrelatedForbiddenRoots {
                    witness: witness.clone(),
                },
            };
            assert!(matches!(
                verify_bounded_certificate_with_machine_parameters(
                    &fixture.context,
                    &proof.conclusion,
                    &fixture.premises,
                    &fixture.axioms,
                    &fixture.parameters,
                    &proof,
                    &mut Budget::default(),
                ),
                Err(crate::BoundedDenotationError::Certificate(
                    ProofError::IntegerCorrelatedForbiddenRootConversion(
                        crate::IntegerCorrelatedForbiddenRootConversionError::ConclusionMismatch,
                    )
                )),
            ));
        }

        #[test]
        fn forbidden_roots_rejects_mutated_witness() {
            let fixture = safe_fixture();
            let ProofRule::IntegerCorrelatedForbiddenRoots { witness } = &fixture.proof.rule else {
                unreachable!()
            };
            let mut drifted = witness.clone();
            drifted.dividend.steps[0].definition_axiom = 3;
            let proof = proof_node!(fixture.goal.clone() => ProofRule::IntegerCorrelatedForbiddenRoots { witness: drifted });
            assert!(
                verify_bounded_certificate_with_machine_parameters(
                    &fixture.context,
                    &fixture.goal,
                    &fixture.premises,
                    &fixture.axioms,
                    &fixture.parameters,
                    &proof,
                    &mut Budget::default(),
                )
                .is_err()
            );
        }
    }
}
mod integer_operations {
    use std::collections::BTreeMap;

    use numerics::bignum::BigInt;
    use semantic_vocabulary::{IntegerType, IntegerValue};

    use super::{
        BoundedDenotationError, Denotation, ScalarTerm, Term, TermHandle, unbounded_integer,
    };

    pub(super) mod fixed_scalar {
        use semantic_vocabulary::{IntegerMathTerm, ScalarType};

        use super::{
            BoundedDenotationError, Denotation, IntegerOperation, ScalarTerm, TermHandle,
            application,
        };

        #[derive(Clone, Copy)]
        enum BinaryOperation {
            Add,
            Subtract,
            Multiply,
            Primitive(IntegerOperation),
        }

        enum Pending<'source> {
            Unary(IntegerOperation),
            Left {
                operation: BinaryOperation,
                right: &'source ScalarTerm,
            },
            Right {
                operation: BinaryOperation,
                left: TermHandle,
            },
        }

        pub(crate) fn denote(
            denotation: &mut Denotation,
            term: &ScalarTerm,
        ) -> Result<TermHandle, BoundedDenotationError> {
            let mut pending = Vec::new();
            let mut current = term;
            'enter: loop {
                let mut value = if let Some((_, value)) = current.integer_value() {
                    denotation.math_term(&IntegerMathTerm::literal(value))?
                } else {
                    match current {
                        ScalarTerm::Value {
                            id,
                            scalar_type: ScalarType::Integer(source_type),
                        } => denotation.math_term(&IntegerMathTerm::MathValue {
                            source_type: *source_type,
                            value: *id,
                        })?,
                        ScalarTerm::ExactIntegerAdd { left, right, .. }
                        | ScalarTerm::ExactIntegerSubtract { left, right, .. }
                        | ScalarTerm::ExactIntegerMultiply { left, right, .. } => {
                            let operation = match current {
                                ScalarTerm::ExactIntegerAdd { .. } => BinaryOperation::Add,
                                ScalarTerm::ExactIntegerSubtract { .. } => {
                                    BinaryOperation::Subtract
                                }
                                _ => BinaryOperation::Multiply,
                            };
                            pending.push(Pending::Left { operation, right });
                            current = left;
                            continue 'enter;
                        }
                        _ => {
                            if let Some(application) = application(current) {
                                match application.right {
                                    Some(right) => pending.push(Pending::Left {
                                        operation: BinaryOperation::Primitive(
                                            application.operation,
                                        ),
                                        right,
                                    }),
                                    None => pending.push(Pending::Unary(application.operation)),
                                }
                                current = application.left;
                                continue 'enter;
                            }
                            let position = denotation.interned_keyed_assumption(
                                |this| &mut this.scalar_integer_terms,
                                current.clone(),
                                |this| this.integer_constant(),
                            )?;
                            denotation.constant(position)
                        }
                    }
                };
                loop {
                    match pending.pop() {
                        Some(Pending::Unary(operation)) => {
                            value = denotation.apply_integer_operation(operation, value, None)?;
                        }
                        Some(Pending::Left { operation, right }) => {
                            pending.push(Pending::Right {
                                operation,
                                left: value,
                            });
                            current = right;
                            continue 'enter;
                        }
                        Some(Pending::Right { operation, left }) => {
                            value = match operation {
                                BinaryOperation::Add => denotation.add_terms(left, value)?,
                                BinaryOperation::Subtract => {
                                    denotation.subtract_terms(left, value)?
                                }
                                BinaryOperation::Multiply => {
                                    denotation.multiply_terms(left, value)?
                                }
                                BinaryOperation::Primitive(operation) => denotation
                                    .apply_integer_operation(operation, left, Some(value))?,
                            };
                        }
                        None => return Ok(value),
                    }
                }
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use crate::{Budget, MathematicalCertificate, Term, verify_mathematical_certificate};
            use semantic_vocabulary::{
                IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ValueId,
            };

            #[test]
            fn policy_and_carrier_distinctions_are_not_definitional_equalities() {
                let integer = |width| IntegerType::new(IntegerSign::Unsigned, width).unwrap();
                let u8 = integer(8);
                let u16 = integer(16);
                let u32 = integer(32);
                let id = ValueId::new(1).unwrap();
                let value = ScalarTerm::value(id, ScalarType::Integer(u8));
                let one = |ty| ScalarTerm::integer(ty, IntegerValue::Unsigned(1)).unwrap();
                let context =
                    PropositionContext::from_value_types([(id, ScalarType::Integer(u8))]).unwrap();
                let pairs = [
                    (
                        ScalarTerm::WrappingIntegerAdd {
                            scalar_type: u8,
                            left: Box::new(value.clone()),
                            right: Box::new(one(u8)),
                        },
                        ScalarTerm::SaturatingIntegerAdd {
                            scalar_type: u8,
                            left: Box::new(value.clone()),
                            right: Box::new(one(u8)),
                        },
                    ),
                    (
                        ScalarTerm::IntegerWiden {
                            source_type: u8,
                            target_type: u16,
                            operand: Box::new(value.clone()),
                        },
                        ScalarTerm::IntegerWiden {
                            source_type: u8,
                            target_type: u32,
                            operand: Box::new(value.clone()),
                        },
                    ),
                    (
                        ScalarTerm::WrappingIntegerShiftLeft {
                            value_type: u8,
                            count_type: u8,
                            value: Box::new(value.clone()),
                            count: Box::new(one(u8)),
                        },
                        ScalarTerm::WrappingIntegerShiftLeft {
                            value_type: u8,
                            count_type: u16,
                            value: Box::new(value),
                            count: Box::new(one(u16)),
                        },
                    ),
                ];
                for (left, right) in pairs {
                    for source in [&left, &right] {
                        context
                            .validate(&Proposition::Equal(source.clone(), source.clone()))
                            .unwrap();
                    }
                    let mut denotation = Denotation::new();
                    let left = denote(&mut denotation, &left).unwrap();
                    let right = denote(&mut denotation, &right).unwrap();
                    let ty = denotation.integer_constant().unwrap();
                    for value in [left, right] {
                        let term = denotation.arena.insert(Term::Refl { ty, value });
                        let expected = denotation.arena.insert(Term::Id {
                            ty,
                            left: value,
                            right: value,
                        });
                        let certificate = MathematicalCertificate::with_signature(
                            denotation.declarations.clone(),
                            Vec::new(),
                            term,
                            expected,
                        );
                        verify_mathematical_certificate(
                            &mut denotation.arena,
                            &certificate,
                            &mut Budget::default(),
                        )
                        .unwrap();
                    }
                    let term = denotation.arena.insert(Term::Refl { ty, value: left });
                    let expected = denotation.arena.insert(Term::Id { ty, left, right });
                    let certificate = MathematicalCertificate::with_signature(
                        denotation.declarations,
                        Vec::new(),
                        term,
                        expected,
                    );
                    assert!(
                        verify_mathematical_certificate(
                            &mut denotation.arena,
                            &certificate,
                            &mut Budget::default()
                        )
                        .is_err(),
                        "different policies, conversion carriers and shift-count carriers need their own evidence"
                    );
                }
            }
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub(super) enum IntegerOperation {
        BitwiseNot(IntegerType),
        Widen {
            source: IntegerType,
            target: IntegerType,
        },
        ExactCast {
            source: IntegerType,
            target: IntegerType,
        },
        BitwiseAnd(IntegerType),
        BitwiseOr(IntegerType),
        BitwiseXor(IntegerType),
        WrappingShiftLeft {
            value: IntegerType,
            count: IntegerType,
        },
        WrappingShiftRight {
            value: IntegerType,
            count: IntegerType,
        },
        ExactShiftLeft {
            value: IntegerType,
            count: IntegerType,
        },
        ExactShiftRight {
            value: IntegerType,
            count: IntegerType,
        },
        ExactDivide(IntegerType),
        ExactRemainder(IntegerType),
        WrappingAdd(IntegerType),
        WrappingSubtract(IntegerType),
        WrappingMultiply(IntegerType),
        WrappingDivide(IntegerType),
        WrappingRemainder(IntegerType),
        SaturatingAdd(IntegerType),
        SaturatingSubtract(IntegerType),
        SaturatingMultiply(IntegerType),
        SaturatingDivide(IntegerType),
        SaturatingRemainder(IntegerType),
        Equal(IntegerType),
        LessThan(IntegerType),
        LessOrEqual(IntegerType),
    }

    impl IntegerOperation {
        const fn arity(self) -> usize {
            match self {
                Self::BitwiseNot(_) | Self::Widen { .. } | Self::ExactCast { .. } => 1,
                _ => 2,
            }
        }
    }

    macro_rules! integer_operation_table {
    ($term:expr; $( $variant:ident { $($field:ident),+ $(,)? } => ($operation:expr, $left:expr, $right:expr) ),+ $(,)?) => {
        match $term {
            $(
                ScalarTerm::$variant { $($field),+ } => ($operation, $left, $right),
            )+
            _ => return None,
        }
    };
    }

    struct IntegerApplication<'source> {
        operation: IntegerOperation,
        left: &'source ScalarTerm,
        right: Option<&'source ScalarTerm>,
    }

    fn application(term: &ScalarTerm) -> Option<IntegerApplication<'_>> {
        let (operation, left, right) = integer_operation_table! { term;
            IntegerBitwiseNot { scalar_type, operand } => (IntegerOperation::BitwiseNot(*scalar_type), operand, None),
            IntegerWiden { source_type, target_type, operand } => (IntegerOperation::Widen { source: *source_type, target: *target_type }, operand, None),
            IntegerExactCast { source_type, target_type, operand } => (IntegerOperation::ExactCast { source: *source_type, target: *target_type }, operand, None),
            IntegerBitwiseAnd { scalar_type, left, right } => (IntegerOperation::BitwiseAnd(*scalar_type), left, Some(right)),
            IntegerBitwiseOr { scalar_type, left, right } => (IntegerOperation::BitwiseOr(*scalar_type), left, Some(right)),
            IntegerBitwiseXor { scalar_type, left, right } => (IntegerOperation::BitwiseXor(*scalar_type), left, Some(right)),
            WrappingIntegerShiftLeft { value_type, count_type, value, count } => (IntegerOperation::WrappingShiftLeft { value: *value_type, count: *count_type }, value, Some(count)),
            WrappingIntegerShiftRight { value_type, count_type, value, count } => (IntegerOperation::WrappingShiftRight { value: *value_type, count: *count_type }, value, Some(count)),
            ExactIntegerShiftLeft { value_type, count_type, value, count } => (IntegerOperation::ExactShiftLeft { value: *value_type, count: *count_type }, value, Some(count)),
            ExactIntegerShiftRight { value_type, count_type, value, count } => (IntegerOperation::ExactShiftRight { value: *value_type, count: *count_type }, value, Some(count)),
            ExactIntegerDivide { scalar_type, left, right } => (IntegerOperation::ExactDivide(*scalar_type), left, Some(right)),
            ExactIntegerRemainder { scalar_type, left, right } => (IntegerOperation::ExactRemainder(*scalar_type), left, Some(right)),
            WrappingIntegerAdd { scalar_type, left, right } => (IntegerOperation::WrappingAdd(*scalar_type), left, Some(right)),
            WrappingIntegerSubtract { scalar_type, left, right } => (IntegerOperation::WrappingSubtract(*scalar_type), left, Some(right)),
            WrappingIntegerMultiply { scalar_type, left, right } => (IntegerOperation::WrappingMultiply(*scalar_type), left, Some(right)),
            WrappingIntegerDivide { scalar_type, left, right } => (IntegerOperation::WrappingDivide(*scalar_type), left, Some(right)),
            WrappingIntegerRemainder { scalar_type, left, right } => (IntegerOperation::WrappingRemainder(*scalar_type), left, Some(right)),
            SaturatingIntegerAdd { scalar_type, left, right } => (IntegerOperation::SaturatingAdd(*scalar_type), left, Some(right)),
            SaturatingIntegerSubtract { scalar_type, left, right } => (IntegerOperation::SaturatingSubtract(*scalar_type), left, Some(right)),
            SaturatingIntegerMultiply { scalar_type, left, right } => (IntegerOperation::SaturatingMultiply(*scalar_type), left, Some(right)),
            SaturatingIntegerDivide { scalar_type, left, right } => (IntegerOperation::SaturatingDivide(*scalar_type), left, Some(right)),
            SaturatingIntegerRemainder { scalar_type, left, right } => (IntegerOperation::SaturatingRemainder(*scalar_type), left, Some(right)),
        };
        Some(IntegerApplication {
            operation,
            left,
            right: right.map(|term| term.as_ref()),
        })
    }

    impl Denotation {
        pub(super) fn integer_operation(
            &mut self,
            operation: IntegerOperation,
        ) -> Result<u32, BoundedDenotationError> {
            self.interned_keyed_assumption(
                |this| &mut this.integer_operations,
                operation,
                |this| {
                    let result = if matches!(
                        operation,
                        IntegerOperation::Equal(_)
                            | IntegerOperation::LessThan(_)
                            | IntegerOperation::LessOrEqual(_)
                    ) {
                        this.two
                    } else {
                        this.integer_constant()?
                    };
                    let domain = this.integer_constant()?;
                    let mut ty = result;
                    for _ in 0..operation.arity() {
                        ty = this.arena.insert(Term::Pi {
                            domain,
                            codomain: ty,
                        });
                    }
                    Ok(ty)
                },
            )
        }

        pub(super) fn apply_integer_operation(
            &mut self,
            operation: IntegerOperation,
            left: TermHandle,
            right: Option<TermHandle>,
        ) -> Result<TermHandle, BoundedDenotationError> {
            let position = self.integer_operation(operation)?;
            let function = self.constant(position);
            Ok(match right {
                Some(right) => self.arena.apply_arguments(function, &[left, right]),
                None => self.arena.apply_arguments(function, &[left]),
            })
        }

        pub(super) fn integer_relation_term(
            &mut self,
            term: &ScalarTerm,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            let (operation, left, right) = match term {
                ScalarTerm::IntegerEqual {
                    scalar_type,
                    left,
                    right,
                } => (IntegerOperation::Equal(*scalar_type), left, right),
                ScalarTerm::IntegerLessThan {
                    scalar_type,
                    left,
                    right,
                } => (IntegerOperation::LessThan(*scalar_type), left, right),
                ScalarTerm::IntegerLessOrEqual {
                    scalar_type,
                    left,
                    right,
                } => (IntegerOperation::LessOrEqual(*scalar_type), left, right),
                _ => return Ok(None),
            };
            let left = self.fixed_scalar_term(left)?;
            let right = self.fixed_scalar_term(right)?;
            let position = self.integer_operation(operation)?;
            let relation = self.constant(position);
            Ok(Some(self.arena.apply_binary(relation, left, right)))
        }

        pub(super) fn binary_integer_operation(
            &mut self,
            cache: impl Fn(&mut Self) -> &mut Option<u32>,
        ) -> Result<u32, BoundedDenotationError> {
            self.interned_slot_assumption(cache, |this| {
                let integer = this.integer_constant()?;
                let result = this.arena.insert(Term::Pi {
                    domain: integer,
                    codomain: integer,
                });
                Ok(this.arena.insert(Term::Pi {
                    domain: integer,
                    codomain: result,
                }))
            })
        }

        pub(super) fn binary_terms(
            &mut self,
            operation: fn(&mut Self) -> Result<u32, BoundedDenotationError>,
            left: TermHandle,
            right: TermHandle,
        ) -> Result<TermHandle, BoundedDenotationError> {
            let position = operation(self)?;
            let operation = self.constant(position);
            Ok(self.arena.apply_binary(operation, left, right))
        }

        pub(super) fn numeral_binary_operation(
            &mut self,
            left: IntegerValue,
            right: IntegerValue,
            result: IntegerValue,
            left_term: TermHandle,
            right_term: TermHandle,
            result_term: TermHandle,
            compute: impl Fn(&BigInt, &BigInt) -> BigInt,
            terms: fn(
                &mut Self,
                TermHandle,
                TermHandle,
            ) -> Result<TermHandle, BoundedDenotationError>,
            cache: impl Fn(&mut Self) -> &mut BTreeMap<(IntegerValue, IntegerValue, IntegerValue), u32>,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            if compute(&unbounded_integer(left), &unbounded_integer(right))
                != unbounded_integer(result)
            {
                return Ok(None);
            }
            let position =
                self.interned_keyed_assumption(cache, (left, right, result), |this| {
                    let integer = this.integer_constant()?;
                    let application = terms(this, left_term, right_term)?;
                    Ok(this.arena.insert(Term::Id {
                        ty: integer,
                        left: application,
                        right: result_term,
                    }))
                })?;
            Ok(Some(self.constant(position)))
        }
    }
}
mod multiplication {
    use std::collections::BTreeMap;

    use semantic_vocabulary::{
        IntegerCarrier, IntegerMathTerm, IntegerType, IntegerValue, Proposition, ScalarTerm,
    };

    use super::denotation::math_less_or_equal_sides;
    use super::integer_operations::IntegerOperation;
    use super::{
        BoundedDenotationError, Denotation, IntegerLaw, Term, TermHandle,
        affine_witness_root_expression, unbounded_integer,
    };
    use crate::mathematical_core::scheme_dsl::{self, apps, id, pi, pis, scheme_at, v};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    enum Law {
        DistributeAdd,
        MonotonePositive,
        AntitoneNegative,
        DivideLowerPositive(IntegerType),
        DivideLowerNegative(IntegerType),
        DivideUpperPositive(IntegerType),
        DivideUpperNegative(IntegerType),
    }

    #[derive(Default)]
    pub(super) struct Multiplication {
        operation: Option<u32>,
        laws: BTreeMap<Law, u32>,
        pub(super) numeral_products: BTreeMap<(IntegerValue, IntegerValue, IntegerValue), u32>,
        pub(super) numeral_quotients:
            BTreeMap<(IntegerType, IntegerValue, IntegerValue, IntegerValue), u32>,
    }

    impl Denotation {
        pub(super) fn distribute_add(
            &mut self,
            scale: TermHandle,
            left: TermHandle,
            right: TermHandle,
        ) -> Result<TermHandle, BoundedDenotationError> {
            self.multiply_law_application(Law::DistributeAdd, &[scale, left, right])
        }
        pub(super) fn multiply_operation(&mut self) -> Result<u32, BoundedDenotationError> {
            self.binary_integer_operation(|this| &mut this.multiplication.operation)
        }

        pub(super) fn multiply_terms(
            &mut self,
            left: TermHandle,
            right: TermHandle,
        ) -> Result<TermHandle, BoundedDenotationError> {
            self.binary_terms(Self::multiply_operation, left, right)
        }

        fn multiply_law_application(
            &mut self,
            law: Law,
            arguments: &[TermHandle],
        ) -> Result<TermHandle, BoundedDenotationError> {
            self.roster_law_application(
                |this| &mut this.multiplication.laws,
                law,
                |this| -> Result<scheme_dsl::Syntax, BoundedDenotationError> {
                    let integer = this.integer()?;
                    let operation = this.multiply_operation()?;
                    let less_or_equal = this.integer_less_or_equal()?;
                    let zero = this.binary_integer(false, 0)?;
                    let one = this.binary_integer(false, 1)?;
                    let negative_one = this.binary_integer(true, 1)?;
                    let constant = |position| scheme_at(position, Vec::new());
                    let carrier = || constant(integer);
                    let binders = |names: &[&'static str], body| {
                        pis(names.iter().map(|name| (*name, carrier())), body)
                    };
                    let le = |left, right| apps(constant(less_or_equal), [left, right]);
                    let mul = |left, right| apps(constant(operation), [left, right]);
                    let statement = match law {
                        Law::DistributeAdd => {
                            let addition = this.add_operation()?;
                            let add = |left, right| apps(constant(addition), [left, right]);
                            binders(
                                &["scale", "left", "right"],
                                id(
                                    carrier(),
                                    mul(v("scale"), add(v("left"), v("right"))),
                                    add(mul(v("scale"), v("left")), mul(v("scale"), v("right"))),
                                ),
                            )
                        }
                        Law::MonotonePositive => binders(
                            &["a", "b", "r"],
                            pi(
                                "_",
                                le(v("a"), v("b")),
                                pi(
                                    "_",
                                    le(constant(one), v("r")),
                                    le(mul(v("a"), v("r")), mul(v("b"), v("r"))),
                                ),
                            ),
                        ),
                        Law::AntitoneNegative => binders(
                            &["a", "b", "r"],
                            pi(
                                "_",
                                le(v("a"), v("b")),
                                pi(
                                    "_",
                                    le(v("r"), constant(negative_one)),
                                    le(mul(v("b"), v("r")), mul(v("a"), v("r"))),
                                ),
                            ),
                        ),
                        Law::DivideLowerPositive(scalar_type)
                        | Law::DivideLowerNegative(scalar_type)
                        | Law::DivideUpperPositive(scalar_type)
                        | Law::DivideUpperNegative(scalar_type) => {
                            let divide =
                                this.integer_operation(IntegerOperation::ExactDivide(scalar_type))?;
                            let quotient = |left, right| apps(constant(divide), [left, right]);
                            let lower_endpoint = matches!(
                                law,
                                Law::DivideLowerPositive(_) | Law::DivideLowerNegative(_)
                            );
                            let positive_divisor = matches!(
                                law,
                                Law::DivideLowerPositive(_) | Law::DivideUpperPositive(_)
                            );
                            let dividend = if lower_endpoint {
                                le(v("n"), constant(zero))
                            } else {
                                le(constant(zero), v("n"))
                            };
                            let divisor = if positive_divisor {
                                le(constant(one), v("d"))
                            } else {
                                le(v("d"), constant(negative_one))
                            };
                            let product = mul(quotient(v("n"), v("d")), v("d"));
                            let residual = if lower_endpoint {
                                le(v("n"), product)
                            } else {
                                le(product, v("n"))
                            };
                            binders(&["n", "d"], pi("_", dividend, pi("_", divisor, residual)))
                        }
                    };
                    Ok(statement)
                },
                arguments,
            )
        }

        pub(super) fn integer_le_numeral(
            &mut self,
            lower: IntegerValue,
            upper: IntegerValue,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            let parts = |value| match value {
                IntegerValue::Signed(value) => (value < 0, value.unsigned_abs()),
                IntegerValue::Unsigned(value) => (false, value),
            };
            let (negative_lower, lower_magnitude) = parts(lower);
            let (negative_upper, upper_magnitude) = parts(upper);
            let lower_term = {
                let position = self.binary_integer(negative_lower, lower_magnitude)?;
                self.constant(position)
            };
            let upper_term = {
                let position = self.binary_integer(negative_upper, upper_magnitude)?;
                self.constant(position)
            };
            if lower == upper {
                let integer = self.integer_constant()?;
                let reflexive = self.arena.insert(Term::Refl {
                    ty: integer,
                    value: lower_term,
                });
                return self
                    .integer_law_application(
                        IntegerLaw::EqualityToLessOrEqual,
                        &[lower_term, upper_term, reflexive],
                    )
                    .map(Some);
            }
            if unbounded_integer(lower) >= unbounded_integer(upper) {
                return Ok(None);
            }
            let strict = self.literal_order(lower, upper)?;
            self.integer_law_application(
                IntegerLaw::LessThanToLessOrEqual,
                &[lower_term, upper_term, strict],
            )
            .map(Some)
        }

        pub(super) fn numeral_product(
            &mut self,
            left: IntegerValue,
            right: IntegerValue,
            product: IntegerValue,
            left_term: TermHandle,
            right_term: TermHandle,
            product_term: TermHandle,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            self.numeral_binary_operation(
                left,
                right,
                product,
                left_term,
                right_term,
                product_term,
                |left, right| left.mul(right),
                Self::multiply_terms,
                |denotation| &mut denotation.multiplication.numeral_products,
            )
        }

        pub(super) fn numeral_quotient(
            &mut self,
            integer_type: IntegerType,
            dividend: IntegerValue,
            divisor: IntegerValue,
            quotient: IntegerValue,
            dividend_term: TermHandle,
            divisor_term: TermHandle,
            quotient_term: TermHandle,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            let Some((exact, _)) = unbounded_integer(dividend).div_rem(&unbounded_integer(divisor))
            else {
                return Ok(None);
            };
            if exact != unbounded_integer(quotient) {
                return Ok(None);
            }
            let key = (integer_type, dividend, divisor, quotient);
            let position = self.interned_keyed_assumption(
                |this| &mut this.multiplication.numeral_quotients,
                key,
                |this| {
                    let integer = this.integer_constant()?;
                    let divide =
                        this.integer_operation(IntegerOperation::ExactDivide(integer_type))?;
                    let application = {
                        let function = this.constant(divide);
                        this.arena
                            .apply_binary(function, dividend_term, divisor_term)
                    };
                    Ok(this.arena.insert(Term::Id {
                        ty: integer,
                        left: application,
                        right: quotient_term,
                    }))
                },
            )?;
            Ok(Some(self.constant(position)))
        }

        pub(super) fn correlated_multiply_bound_evidence(
            &mut self,
            premise: &Proposition,
            evidence: TermHandle,
            witness: &crate::IntegerAffineWitness,
            conclusion: &Proposition,
            definitions: &[(Proposition, TermHandle)],
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            let ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } = &witness.target
            else {
                return Ok(None);
            };
            if scalar_type.carrier() != IntegerCarrier::Fixed {
                return Ok(None);
            }
            let Some((lower, bound, product)) =
                math_less_or_equal_sides(conclusion, crate::term_matchers::is_math_multiply)
            else {
                return Ok(None);
            };

            let IntegerMathTerm::IntegerLiteral(bound_literal) = bound else {
                return Ok(None);
            };
            let endpoint_value = if lower {
                scalar_type.minimum_value()
            } else {
                scalar_type.maximum_value()
            };
            if super::math_literal_value(*bound_literal).map(unbounded_integer)
                != Some(unbounded_integer(endpoint_value))
            {
                return Ok(None);
            }
            let Proposition::Conjunction(parts) = premise else {
                return Ok(None);
            };
            let [sign_evidence, bound_evidence] = parts.as_slice() else {
                return Ok(None);
            };
            let one = ScalarTerm::integer(*scalar_type, scalar_type.one())
                .expect("one is admitted by every fixed carrier");
            let negative_two = ScalarTerm::integer(*scalar_type, IntegerValue::Signed(-2)).ok();
            let positive = sign_evidence == &Proposition::LessOrEqual(one, right.as_ref().clone());
            let negative = negative_two.is_some_and(|negative_two| {
                sign_evidence == &Proposition::LessOrEqual(right.as_ref().clone(), negative_two)
            });
            if !positive && !negative {
                return Ok(None);
            }
            let expected_bound = if lower == positive {
                Proposition::LessOrEqual(witness.root.clone(), left.as_ref().clone())
            } else {
                Proposition::LessOrEqual(left.as_ref().clone(), witness.root.clone())
            };
            if bound_evidence != &expected_bound {
                return Ok(None);
            }
            let Some((expression, definition)) =
                affine_witness_root_expression(witness, definitions)
            else {
                return Ok(None);
            };
            let ScalarTerm::ExactIntegerDivide {
                scalar_type: divide_type,
                left: endpoint,
                right: divisor,
            } = expression
            else {
                return Ok(None);
            };
            if divide_type != scalar_type || divisor.as_ref() != right.as_ref() {
                return Ok(None);
            }
            let Some((endpoint_equality, endpoint_literal)) =
                super::affine_carrier_endpoint_equality(*scalar_type, lower, endpoint, definitions)
            else {
                return Ok(None);
            };
            let left_term = self.fixed_scalar_term(left)?;
            let right_term = self.fixed_scalar_term(right)?;
            let endpoint_term = self.fixed_scalar_term(endpoint)?;
            let expression_term = self.fixed_scalar_term(expression)?;
            let bound_term = self.math_term(bound)?;
            let endpoint_bound_term = self.fixed_scalar_term(&endpoint_literal)?;
            if !self
                .arena
                .structurally_equal(bound_term, endpoint_bound_term)
            {
                return Ok(None);
            }
            let product_term = self.math_term(product)?;
            let operand_product = self.multiply_terms(left_term, right_term)?;
            if !self.arena.structurally_equal(product_term, operand_product) {
                return Ok(None);
            }
            let sign = self.arena.insert(Term::Fst { pair: evidence });
            let mut bound = self.arena.insert(Term::Snd { pair: evidence });
            let Some(stepped) = self.affine_root_equality_step(
                lower == positive,
                witness,
                left_term,
                expression_term,
                definition,
                bound,
            )?
            else {
                return Ok(None);
            };
            bound = stepped;
            let zero = {
                let position = self.binary_integer(false, 0)?;
                self.constant(position)
            };
            let sign_term = if positive {
                sign
            } else {
                let negative_one_term = {
                    let position = self.binary_integer(true, 1)?;
                    self.constant(position)
                };
                let negative_two_term = {
                    let position = self.binary_integer(true, 2)?;
                    self.constant(position)
                };
                let strict =
                    self.literal_order(IntegerValue::Signed(-2), IntegerValue::Signed(-1))?;
                let step = self.integer_law_application(
                    IntegerLaw::LessThanToLessOrEqual,
                    &[negative_two_term, negative_one_term, strict],
                )?;
                self.integer_law_application(
                    IntegerLaw::LessOrEqualTransitivity,
                    &[right_term, negative_two_term, negative_one_term, sign, step],
                )?
            };
            let Some(numeral_side) = (if lower {
                self.integer_le_numeral(endpoint_value, IntegerValue::Unsigned(0))
            } else {
                self.integer_le_numeral(IntegerValue::Unsigned(0), endpoint_value)
            })?
            else {
                return Ok(None);
            };
            let endpoint_side = if let Some((equality, proposition)) = endpoint_equality {
                let literal_term = self.fixed_scalar_term(&endpoint_literal)?;
                let denoted = self.denote(proposition)?;
                let Some((_, from, to)) = self.identity_parts(denoted) else {
                    return Ok(None);
                };
                let Some(equality) =
                    self.directed_equality(from, to, literal_term, endpoint_term, equality)
                else {
                    return Ok(None);
                };
                if lower {
                    self.integer_law_application(
                        IntegerLaw::LessOrEqualSubstituteLeft,
                        &[literal_term, endpoint_term, zero, equality, numeral_side],
                    )?
                } else {
                    self.integer_law_application(
                        IntegerLaw::LessOrEqualSubstituteRight,
                        &[zero, literal_term, endpoint_term, equality, numeral_side],
                    )?
                }
            } else {
                numeral_side
            };
            let residual = self.multiply_law_application(
                match (lower, positive) {
                    (true, true) => Law::DivideLowerPositive(*scalar_type),
                    (true, false) => Law::DivideLowerNegative(*scalar_type),
                    (false, true) => Law::DivideUpperPositive(*scalar_type),
                    (false, false) => Law::DivideUpperNegative(*scalar_type),
                },
                &[endpoint_term, right_term, endpoint_side, sign_term],
            )?;
            let (a, b) = if lower == positive {
                (expression_term, left_term)
            } else {
                (left_term, expression_term)
            };
            let monotone = self.multiply_law_application(
                if positive {
                    Law::MonotonePositive
                } else {
                    Law::AntitoneNegative
                },
                &[a, b, right_term, bound, sign_term],
            )?;
            let endpoint_product = self.multiply_terms(expression_term, right_term)?;
            let combined = if lower {
                self.integer_law_application(
                    IntegerLaw::LessOrEqualTransitivity,
                    &[
                        endpoint_term,
                        endpoint_product,
                        operand_product,
                        residual,
                        monotone,
                    ],
                )?
            } else {
                self.integer_law_application(
                    IntegerLaw::LessOrEqualTransitivity,
                    &[
                        operand_product,
                        endpoint_product,
                        endpoint_term,
                        monotone,
                        residual,
                    ],
                )?
            };
            if let Some((equality, proposition)) = endpoint_equality {
                let literal_term = self.fixed_scalar_term(&endpoint_literal)?;
                let denoted = self.denote(proposition)?;
                let Some((_, from, to)) = self.identity_parts(denoted) else {
                    return Ok(None);
                };
                let Some(equality) =
                    self.directed_equality(from, to, endpoint_term, literal_term, equality)
                else {
                    return Ok(None);
                };
                if lower {
                    self.integer_law_application(
                        IntegerLaw::LessOrEqualSubstituteLeft,
                        &[
                            endpoint_term,
                            literal_term,
                            operand_product,
                            equality,
                            combined,
                        ],
                    )
                } else {
                    self.integer_law_application(
                        IntegerLaw::LessOrEqualSubstituteRight,
                        &[
                            operand_product,
                            endpoint_term,
                            literal_term,
                            equality,
                            combined,
                        ],
                    )
                }
                .map(Some)
            } else {
                Ok(Some(combined))
            }
        }
    }

    impl Multiplication {
        pub(super) fn source_safety_roster(&self) -> Vec<u32> {
            self.operation
                .into_iter()
                .chain(self.laws.values().copied())
                .chain(self.numeral_products.values().copied())
                .chain(self.numeral_quotients.values().copied())
                .collect()
        }
    }

    #[cfg(test)]
    mod tests {
        use std::collections::BTreeSet;

        use super::super::Elaboration;
        use super::super::test_utilities::{assert_refused, mathematical};
        use super::Law;
        use crate::mathematical_core::Term;
        use crate::proof::assumption_proof;
        use crate::{
            Budget, IntegerAffineWitness, ProofNode, ProofRule, value_context,
            verify_bounded_certificate,
        };
        use semantic_vocabulary::{
            IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, Proposition,
            PropositionContext, ScalarTerm, ScalarType, ValueId,
        };

        struct Fixture {
            context: PropositionContext,
            axioms: Vec<Proposition>,
            premise: Proposition,
            goal: Proposition,
            proof: ProofNode,
        }

        fn fixture(
            integer: IntegerType,
            endpoint: IntegerValue,
            lower: bool,
            positive: bool,
            landed: bool,
            divide_root: bool,
        ) -> Fixture {
            let scalar = ScalarType::Integer(integer);
            let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
            let literal = |value| ScalarTerm::integer(integer, value).unwrap();
            let left = value(1);
            let right = value(2);
            let quotient = value(3);
            let endpoint_term = if landed { value(4) } else { literal(endpoint) };
            let context = value_context(scalar, 4);
            let divide =
                ScalarTerm::exact_integer_divide(integer, endpoint_term.clone(), right.clone())
                    .unwrap();
            let mut axioms = Vec::new();
            let (root, definition_axioms, literal_axioms) = if divide_root {
                (divide.clone(), Vec::new(), Vec::new())
            } else {
                if landed {
                    axioms.push(Proposition::Equal(endpoint_term.clone(), literal(endpoint)));
                }
                let definition_index = axioms.len();
                axioms.push(Proposition::Equal(quotient.clone(), divide));
                (quotient, vec![definition_index], vec![landed.then_some(0)])
            };
            let one = literal(integer.one());
            let sign = if positive {
                Proposition::LessOrEqual(one, right.clone())
            } else {
                Proposition::LessOrEqual(right.clone(), literal(IntegerValue::Signed(-2)))
            };
            let bound = if lower == positive {
                Proposition::LessOrEqual(root.clone(), left.clone())
            } else {
                Proposition::LessOrEqual(left.clone(), root.clone())
            };
            let premise = Proposition::Conjunction(vec![sign, bound]);
            let product = IntegerMathTerm::Multiply(
                Box::new(mathematical(&left)),
                Box::new(mathematical(&right)),
            );
            let bound_literal = IntegerMathTerm::literal(endpoint);
            let goal = if lower {
                Proposition::IntegerMathLessOrEqual(bound_literal, product)
            } else {
                Proposition::IntegerMathLessOrEqual(product, bound_literal)
            };
            let proof = ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::IntegerAffineBound {
                    root_bound: Box::new(assumption_proof(premise.clone(), 0)),
                    witness: IntegerAffineWitness {
                        root,
                        target: ScalarTerm::exact_integer_multiply(integer, left, right).unwrap(),
                        definition_axioms,
                        literal_axioms,
                    },
                },
            };
            Fixture {
                context,
                axioms,
                premise,
                goal,
                proof,
            }
        }

        macro_rules! row {
            ($a:expr => $b:expr => $c:expr => $d:expr => $e:expr => $f:expr) => {
                fixture($a, $b, $c, $d, $e, $f)
            };
        }

        fn check(fixture: &Fixture, laws: &[Law]) {
            verify_bounded_certificate(
                &fixture.context,
                &fixture.goal,
                std::slice::from_ref(&fixture.premise),
                &fixture.axioms,
                &fixture.proof,
                &mut Budget::default(),
            )
            .unwrap();
            let parameters = BTreeSet::new();
            let mut elaboration = Elaboration::new(
                &fixture.context,
                &fixture.goal,
                std::slice::from_ref(&fixture.premise),
                &fixture.axioms,
                &parameters,
            )
            .unwrap();
            let evidence = elaboration.node(&fixture.proof).unwrap();
            assert!(elaboration.denotation.rule_axioms.is_empty());
            assert!(elaboration.denotation.decisions.is_empty());
            for law in laws {
                assert!(
                    elaboration.denotation.multiplication.laws.contains_key(law),
                    "missing {law:?}"
                );
            }
            let term = elaboration.denotation.arena.get(evidence);
            assert!(!matches!(term, Term::Constant { .. }));
        }

        #[test]
        fn correlated_multiply_negative_minimum() {
            let integer = IntegerType::new(IntegerSign::Signed, 8).unwrap();
            let fixture =
                row!(integer => IntegerValue::Signed(-128) => true => false => false => false);
            check(
                &fixture,
                &[Law::AntitoneNegative, Law::DivideLowerNegative(integer)],
            );
        }

        #[test]
        fn correlated_multiply_positive_maximum() {
            let integer = IntegerType::new(IntegerSign::Signed, 8).unwrap();
            let fixture =
                row!(integer => IntegerValue::Signed(127) => false => true => false => false);
            check(
                &fixture,
                &[Law::MonotonePositive, Law::DivideUpperPositive(integer)],
            );
        }

        #[test]
        fn correlated_multiply_negative_maximum() {
            let integer = IntegerType::new(IntegerSign::Signed, 8).unwrap();
            let fixture =
                row!(integer => IntegerValue::Signed(127) => false => false => false => false);
            check(
                &fixture,
                &[Law::AntitoneNegative, Law::DivideUpperNegative(integer)],
            );
        }

        #[test]
        fn correlated_multiply_divide_root_needs_no_definition() {
            let integer = IntegerType::new(IntegerSign::Signed, 8).unwrap();
            let fixture = fixture(integer, IntegerValue::Signed(-128), true, true, false, true);
            check(
                &fixture,
                &[Law::MonotonePositive, Law::DivideLowerPositive(integer)],
            );
        }

        #[test]
        fn correlated_multiply_landed_endpoint_substitutes_both_sites() {
            let integer = IntegerType::new(IntegerSign::Signed, 8).unwrap();
            let fixture = fixture(integer, IntegerValue::Signed(-128), true, true, true, false);
            check(
                &fixture,
                &[Law::MonotonePositive, Law::DivideLowerPositive(integer)],
            );
        }

        #[test]
        fn correlated_multiply_unsigned_zero_endpoint() {
            let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
            let fixture = fixture(integer, IntegerValue::Unsigned(0), true, true, false, false);
            check(
                &fixture,
                &[Law::MonotonePositive, Law::DivideLowerPositive(integer)],
            );
        }

        #[test]
        fn correlated_multiply_rejects_mutated_premise_and_goal() {
            let integer = IntegerType::new(IntegerSign::Signed, 8).unwrap();
            let scalar = ScalarType::Integer(integer);
            let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
            let fixture =
                row!(integer => IntegerValue::Signed(-128) => true => true => false => false);
            let mut flipped = fixture.proof.clone();
            let ProofRule::IntegerAffineBound { root_bound, .. } = &mut flipped.rule else {
                unreachable!()
            };
            root_bound.conclusion = Proposition::Conjunction(vec![
                Proposition::LessOrEqual(
                    ScalarTerm::integer(integer, IntegerValue::Signed(1)).unwrap(),
                    value(2),
                ),
                Proposition::LessOrEqual(value(1), value(3)),
            ]);
            let flipped_premise = root_bound.conclusion.clone();
            assert_refused(
                &fixture.context,
                &fixture.goal,
                std::slice::from_ref(&flipped_premise),
                &fixture.axioms,
                &flipped,
            );
            let mut wrong_goal = fixture.proof.clone();
            wrong_goal.conclusion = Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::literal(IntegerValue::Signed(-127)),
                IntegerMathTerm::Multiply(
                    Box::new(mathematical(&value(1))),
                    Box::new(mathematical(&value(2))),
                ),
            );
            assert_refused(
                &fixture.context,
                &wrong_goal.conclusion,
                std::slice::from_ref(&fixture.premise),
                &fixture.axioms,
                &wrong_goal,
            );
            let mut missing = fixture.proof.clone();
            let ProofRule::IntegerAffineBound { root_bound, .. } = &mut missing.rule else {
                unreachable!()
            };
            root_bound.conclusion = Proposition::LessOrEqual(value(3), value(1));
            let missing_premise = root_bound.conclusion.clone();
            assert_refused(
                &fixture.context,
                &fixture.goal,
                std::slice::from_ref(&missing_premise),
                &fixture.axioms,
                &missing,
            );
        }
    }
}
mod order_translation {
    use super::{BoundedDenotationError, Denotation, IntegerLaw, TermHandle, integer_value_of};
    use semantic_vocabulary::{IntegerMathTerm, Proposition};

    impl Denotation {
        pub(super) fn mathematical_order_translation(
            &mut self,
            relation: &Proposition,
            addend: &IntegerMathTerm,
            evidence: TermHandle,
        ) -> Result<TermHandle, BoundedDenotationError> {
            let Proposition::IntegerMathLessOrEqual(left, right) = relation else {
                return Err(BoundedDenotationError::Certificate(
                    crate::ProofError::RulePremiseMismatch(
                        "mathematical integer order translation",
                    ),
                ));
            };
            let left_term = self.math_term(left)?;
            let right_term = self.math_term(right)?;
            let addend_term = self.math_term(addend)?;
            let mut result =
                self.add_order_translation(left_term, right_term, addend_term, evidence)?;
            let left_sum = self.add_terms(left_term, addend_term)?;
            let right_sum = self.add_terms(right_term, addend_term)?;
            let left_result = self.math_term(&IntegerMathTerm::Add(
                Box::new(left.clone()),
                Box::new(addend.clone()),
            ))?;
            let right_result = self.math_term(&IntegerMathTerm::Add(
                Box::new(right.clone()),
                Box::new(addend.clone()),
            ))?;
            if !self.arena.structurally_equal(left_sum, left_result) {
                let equality =
                    self.translated_numeral_equality(left_term, addend_term, left_result)?;
                result = self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteLeft,
                    &[left_sum, left_result, right_sum, equality, result],
                )?;
            }
            if !self.arena.structurally_equal(right_sum, right_result) {
                let equality =
                    self.translated_numeral_equality(right_term, addend_term, right_result)?;
                result = self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteRight,
                    &[left_result, right_sum, right_result, equality, result],
                )?;
            }
            Ok(result)
        }

        fn translated_numeral_equality(
            &mut self,
            left: TermHandle,
            right: TermHandle,
            result: TermHandle,
        ) -> Result<TermHandle, BoundedDenotationError> {
            let value = |term| {
                self.closed_math_value(term)
                    .as_ref()
                    .and_then(integer_value_of)
            };
            let (Some(left_value), Some(right_value), Some(result_value)) =
                (value(left), value(right), value(result))
            else {
                return Err(BoundedDenotationError::Unsupported(
                    "translated numeral exceeds existing numeral evidence",
                ));
            };
            self.numeral_sum(left_value, right_value, result_value, left, right, result)?
                .ok_or(BoundedDenotationError::Unsupported(
                    "translated numeral lacks exact addition evidence",
                ))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::super::test_utilities::{elaborate_checked, verify_and_assert_forged};
        use std::collections::BTreeSet;

        use crate::proof::assumption_proof;
        use crate::{ProofNode, ProofRule, check_certificate};
        use semantic_vocabulary::{
            IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, Proposition,
            PropositionContext, ScalarType, ValueId,
        };

        fn integer() -> IntegerType {
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
        }
        fn value(raw: u64) -> IntegerMathTerm {
            IntegerMathTerm::MathValue {
                source_type: integer(),
                value: ValueId::new(raw).unwrap(),
            }
        }
        fn context() -> PropositionContext {
            PropositionContext::from_value_types(
                (1..=3).map(|raw| (ValueId::new(raw).unwrap(), ScalarType::Integer(integer()))),
            )
            .unwrap()
        }
        fn translated(
            left: IntegerMathTerm,
            right: IntegerMathTerm,
            addend: IntegerMathTerm,
        ) -> (Proposition, ProofNode) {
            let premise = Proposition::IntegerMathLessOrEqual(left.clone(), right.clone());
            let conclusion = Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::Add(Box::new(left), Box::new(addend.clone())),
                IntegerMathTerm::Add(Box::new(right), Box::new(addend.clone())),
            );
            (
                premise.clone(),
                ProofNode {
                    conclusion,
                    rule: ProofRule::IntegerMathOrderTranslation {
                        relation: Box::new(assumption_proof(premise, 0)),
                        addend,
                    },
                },
            )
        }

        #[test]
        fn mathematical_order_translation_derives_from_existing_core_law() {
            let addend = IntegerMathTerm::Subtract(Box::new(value(3)), Box::new(value(2)));
            let (premise, proof) = translated(value(1), value(2), addend);
            let context = context();
            check_certificate(
                &context,
                &proof.conclusion,
                std::slice::from_ref(&premise),
                &[],
                &proof,
            )
            .unwrap();
            let parameters = BTreeSet::new();
            let premises = [premise];
            let elaboration = elaborate_checked(
                &context,
                &proof.conclusion,
                &premises,
                &[],
                &parameters,
                &proof,
            );
            assert!(
                elaboration.denotation.rule_axioms.is_empty(),
                "translation must not introduce a rule-instance assumption"
            );
            verify_and_assert_forged(&context, &proof.conclusion, &premises, &[], &proof);
            assert!(check_certificate(&context, &proof.conclusion, &[], &[], &proof).is_err());
        }

        #[test]
        fn mathematical_order_translation_retains_closed_numeral_denotation() {
            let literal = |value| IntegerMathTerm::literal(IntegerValue::Signed(value));
            let (premise, proof) = translated(literal(-4), literal(2), literal(-3));
            let context = context();
            check_certificate(
                &context,
                &proof.conclusion,
                std::slice::from_ref(&premise),
                &[],
                &proof,
            )
            .unwrap();
            let parameters = BTreeSet::new();
            let premises = [premise];
            let elaboration = elaborate_checked(
                &context,
                &proof.conclusion,
                &premises,
                &[],
                &parameters,
                &proof,
            );
            assert!(elaboration.denotation.rule_axioms.is_empty());
        }

        #[test]
        fn mathematical_order_translation_rejects_strictness_addend_and_scope_drift() {
            let (premise, proof) = translated(value(1), value(2), value(3));
            let context = context();
            let mut stale = proof.clone();
            let ProofRule::IntegerMathOrderTranslation { addend, .. } = &mut stale.rule else {
                unreachable!()
            };
            *addend = value(2);
            assert!(
                check_certificate(
                    &context,
                    &stale.conclusion,
                    std::slice::from_ref(&premise),
                    &[],
                    &stale
                )
                .is_err()
            );

            let strict = Proposition::IntegerMathLessThan(value(1), value(2));
            let mut changed = proof;
            let ProofRule::IntegerMathOrderTranslation { relation, .. } = &mut changed.rule else {
                unreachable!()
            };
            relation.conclusion = strict.clone();
            assert!(
                check_certificate(&context, &changed.conclusion, &[strict], &[], &changed).is_err()
            );

            let (premise, unknown) = translated(value(1), value(2), value(99));
            assert!(
                check_certificate(&context, &unknown.conclusion, &[premise], &[], &unknown)
                    .is_err()
            );
            let wrong_type = IntegerMathTerm::MathValue {
                source_type: IntegerType::new(IntegerSign::Signed, 64).unwrap(),
                value: ValueId::new(3).unwrap(),
            };
            let (premise, wrong_carrier) = translated(value(1), value(2), wrong_type);
            assert!(
                check_certificate(
                    &context,
                    &wrong_carrier.conclusion,
                    &[premise],
                    &[],
                    &wrong_carrier
                )
                .is_err()
            );
        }

        #[test]
        fn mathematical_order_substitution_rejoins_only_exact_finite_rings() {
            use semantic_vocabulary::ScalarTerm;
            let scalar =
                |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), ScalarType::Integer(integer()));
            let sum = IntegerMathTerm::Add(Box::new(value(2)), Box::new(value(3)));
            let exact = ScalarTerm::exact_integer_add(integer(), scalar(2), scalar(3)).unwrap();
            let equality = Proposition::IntegerMathEqual(value(1), value(3));
            let context = context();
            for strict in [false, true] {
                let relation = if strict {
                    Proposition::IntegerMathLessThan(value(1), sum.clone())
                } else {
                    Proposition::IntegerMathLessOrEqual(value(1), sum.clone())
                };
                let conclusion = if strict {
                    Proposition::LessThan(scalar(3), exact.clone())
                } else {
                    Proposition::LessOrEqual(scalar(3), exact.clone())
                };
                let proof = ProofNode {
                    conclusion,
                    rule: ProofRule::IntegerOrderSubstitution {
                        relation: Box::new(assumption_proof(relation.clone(), 0)),
                        equality: Box::new(assumption_proof(equality.clone(), 1)),
                        endpoint: 0,
                    },
                };
                let premises = [relation, equality.clone()];
                check_certificate(&context, &proof.conclusion, &premises, &[], &proof).unwrap();
                let parameters = BTreeSet::new();
                let elaboration = elaborate_checked(
                    &context,
                    &proof.conclusion,
                    &premises,
                    &[],
                    &parameters,
                    &proof,
                );
                assert!(elaboration.denotation.rule_axioms.is_empty());
                for changed in [
                    ScalarTerm::WrappingIntegerAdd {
                        scalar_type: integer(),
                        left: Box::new(scalar(2)),
                        right: Box::new(scalar(3)),
                    },
                    ScalarTerm::SaturatingIntegerAdd {
                        scalar_type: integer(),
                        left: Box::new(scalar(2)),
                        right: Box::new(scalar(3)),
                    },
                ] {
                    let mut wrong = proof.clone();
                    wrong.conclusion = if strict {
                        Proposition::LessThan(scalar(3), changed)
                    } else {
                        Proposition::LessOrEqual(scalar(3), changed)
                    };
                    assert!(crate::lift_exact_integer_order(&wrong.conclusion).is_none());
                    assert!(
                        check_certificate(&context, &wrong.conclusion, &premises, &[], &wrong)
                            .is_err()
                    );
                }
            }
        }

        #[test]
        fn mathematical_order_translation_rejects_a_hidden_deep_addend_without_cloning_it() {
            let premise = Proposition::IntegerMathLessOrEqual(value(1), value(2));
            let mut addend = value(3);
            for _ in 0..16_384 {
                addend = IntegerMathTerm::Add(
                    Box::new(addend),
                    Box::new(IntegerMathTerm::literal(IntegerValue::Unsigned(0))),
                );
            }
            let proof = ProofNode {
                conclusion: premise.clone(),
                rule: ProofRule::IntegerMathOrderTranslation {
                    relation: Box::new(assumption_proof(premise.clone(), 0)),
                    addend,
                },
            };
            assert_eq!(
                check_certificate(&context(), &proof.conclusion, &[premise], &[], &proof),
                Err(crate::ProofError::IntegerOrderConclusionMismatch)
            );
            let ProofRule::IntegerMathOrderTranslation { mut addend, .. } = proof.rule else {
                unreachable!()
            };
            while let IntegerMathTerm::Add(left, _) = addend {
                addend = *left;
            }
        }
    }
}
mod source_content {
    /*
    Source content values use the same opaque canonical algebra carriers as
    checked content derivations. A carrier declaration forms values and Id
    statements; it grants no conservation theorem or resource authority.
    */

    use super::{BoundedDenotationError, Denotation};
    use crate::mathematical_core::{Declaration, Level, TermArena, TermHandle};
    use semantic_vocabulary::ContentAlgebraKind;

    /// One arena's append-only content vocabulary. Stored handles belong to that
    /// arena/signature pair, just as they do in SourceIntegerTheory.
    pub struct SourceContentTheory {
        state: Denotation,
    }

    impl SourceContentTheory {
        pub fn new(arena: &mut TermArena, declarations: &mut Vec<Declaration>) -> Self {
            let mut state = Denotation::new_in(std::mem::take(arena), std::mem::take(declarations));
            std::mem::swap(arena, &mut state.arena);
            std::mem::swap(declarations, &mut state.declarations);
            Self { state }
        }

        pub fn carrier(
            &mut self,
            arena: &mut TermArena,
            declarations: &mut Vec<Declaration>,
            kind: ContentAlgebraKind,
            parameter: TermHandle,
            level: Level,
        ) -> Result<TermHandle, BoundedDenotationError> {
            std::mem::swap(arena, &mut self.state.arena);
            std::mem::swap(declarations, &mut self.state.declarations);
            let result = self.state.content_carrier_at(kind, parameter, level);
            std::mem::swap(arena, &mut self.state.arena);
            std::mem::swap(declarations, &mut self.state.declarations);
            result
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::{
            Budget, Context, DEFAULT_CONVERSION_STEPS, Sort, Term, check_signature, check_type,
            convertible,
        };

        #[test]
        fn canonical_content_family_substitutes_its_outer_type_without_storing_it() {
            let mut arena = TermArena::default();
            let mut declarations = Vec::new();
            let mut content = SourceContentTheory::new(&mut arena, &mut declarations);
            let type_zero = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
            let parameter = arena.insert(Term::Variable(0));
            let generic = content
                .carrier(
                    &mut arena,
                    &mut declarations,
                    ContentAlgebraKind::CountedQuantity,
                    parameter,
                    Level::Constant(0),
                )
                .unwrap();
            let family = arena.insert(Term::Lambda {
                domain: type_zero,
                body: generic,
            });
            let two = arena.insert(Term::Two);
            let applied = arena.insert(Term::Apply {
                function: family,
                argument: two,
            });
            let concrete = content
                .carrier(
                    &mut arena,
                    &mut declarations,
                    ContentAlgebraKind::CountedQuantity,
                    two,
                    Level::Constant(0),
                )
                .unwrap();
            let higher = content
                .carrier(
                    &mut arena,
                    &mut declarations,
                    ContentAlgebraKind::CountedQuantity,
                    type_zero,
                    Level::Constant(1),
                )
                .unwrap();
            let intervals = content
                .carrier(
                    &mut arena,
                    &mut declarations,
                    ContentAlgebraKind::IntervalSet,
                    two,
                    Level::Constant(0),
                )
                .unwrap();
            let signature = check_signature(
                &mut arena,
                &declarations,
                &mut Budget::new(DEFAULT_CONVERSION_STEPS),
            )
            .unwrap();
            let context = Context::empty().with_signature(signature);
            for value in [applied, concrete, higher, intervals] {
                check_type(
                    &mut arena,
                    &context,
                    value,
                    type_zero,
                    &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                )
                .unwrap();
            }
            assert!(
                convertible(
                    &mut arena,
                    &context,
                    applied,
                    concrete,
                    type_zero,
                    &mut Budget::new(DEFAULT_CONVERSION_STEPS)
                )
                .unwrap()
            );
            assert!(
                !convertible(
                    &mut arena,
                    &context,
                    concrete,
                    intervals,
                    type_zero,
                    &mut Budget::new(DEFAULT_CONVERSION_STEPS)
                )
                .unwrap()
            );
        }
    }
}
mod source_integer {
    use super::{BoundedDenotationError, Denotation};
    use crate::mathematical_core::{Declaration, TermArena, TermHandle};
    use numerics::bignum::BigInt;

    mod closed {
        use super::super::{
            BoundedDenotationError, Denotation, IntegerLaw, IntegerRelation, integer_value_of,
        };
        use crate::mathematical_core::{
            Budget, DEFAULT_CONVERSION_STEPS, Signature, Term, TermHandle, substitute,
            weak_head_normalize_retaining,
        };
        use numerics::bignum::BigInt;

        impl Denotation {
            pub(super) fn source_head(
                &mut self,
                term: TermHandle,
            ) -> Result<TermHandle, BoundedDenotationError> {
                let signature = Signature::from_declarations(self.declarations.clone());
                let values = &self.closed_math_values;
                weak_head_normalize_retaining(
                    &mut self.arena,
                    &signature,
                    term,
                    &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                    &|position| values.contains_key(&position),
                )
                .map_err(BoundedDenotationError::Kernel)
            }

            pub(super) fn source_apply(
                &mut self,
                function: TermHandle,
                argument: TermHandle,
            ) -> TermHandle {
                self.arena.insert(Term::Apply { function, argument })
            }

            pub(super) fn source_congruence(
                &mut self,
                function: TermHandle,
                from: TermHandle,
                to: TermHandle,
                proof: TermHandle,
            ) -> Result<TermHandle, BoundedDenotationError> {
                let integer = self.integer_constant()?;
                let start = self.source_apply(function, from);
                let y = self.arena.insert(Term::Variable(0));
                let identity = self.arena.insert(Term::Id {
                    ty: integer,
                    left: from,
                    right: y,
                });
                let y = self.arena.insert(Term::Variable(1));
                let end = self.source_apply(function, y);
                let result = self.arena.insert(Term::Id {
                    ty: integer,
                    left: start,
                    right: end,
                });
                let inner = self.arena.insert(Term::Lambda {
                    domain: identity,
                    body: result,
                });
                let motive = self.arena.insert(Term::Lambda {
                    domain: integer,
                    body: inner,
                });
                let base = self.arena.insert(Term::Refl {
                    ty: integer,
                    value: start,
                });
                Ok(self.arena.insert(Term::IdElim {
                    motive,
                    base,
                    endpoint: to,
                    proof,
                }))
            }

            pub(super) fn source_integer_value(
                &mut self,
                original: TermHandle,
                depth: usize,
            ) -> Result<Option<(BigInt, TermHandle, TermHandle)>, BoundedDenotationError>
            {
                if depth > 128 {
                    return Err(BoundedDenotationError::DepthLimitExceeded);
                }
                let term = self.source_head(original)?;
                let integer = self.integer_constant()?;
                if let Some(value) = self.closed_math_value(term) {
                    let proof = self.arena.insert(Term::Refl {
                        ty: integer,
                        value: term,
                    });
                    return Ok(Some((value, term, proof)));
                }
                let Term::Apply {
                    function,
                    argument: right,
                } = self.arena.get(term)
                else {
                    return Ok(None);
                };
                let function = self.source_head(function)?;
                let Term::Apply {
                    function: operation,
                    argument: left,
                } = self.arena.get(function)
                else {
                    return Ok(None);
                };
                let Term::Constant { declaration, .. } = self.arena.get(operation) else {
                    return Ok(None);
                };
                let kind = if declaration == self.add_operation()? {
                    0
                } else if declaration == self.subtract_operation()? {
                    1
                } else if declaration == self.multiply_operation()? {
                    2
                } else {
                    return Ok(None);
                };
                let Some((left_value, left_term, left_proof)) =
                    self.source_integer_value(left, depth + 1)?
                else {
                    return Ok(None);
                };
                let Some((right_value, right_term, right_proof)) =
                    self.source_integer_value(right, depth + 1)?
                else {
                    return Ok(None);
                };
                let value = match kind {
                    0 => left_value.add(&right_value),
                    1 => left_value.sub(&right_value),
                    _ => left_value.mul(&right_value),
                };
                let (Some(l), Some(r), Some(v)) = (
                    integer_value_of(&left_value),
                    integer_value_of(&right_value),
                    integer_value_of(&value),
                ) else {
                    return Ok(None);
                };
                let result = self.math_literal(value.clone())?;
                let applied = self.source_apply(operation, left_term);
                let normalized = self.source_apply(applied, right_term);
                let equation = match kind {
                    0 => self.numeral_sum(l, r, v, left_term, right_term, result)?,
                    1 => self.numeral_difference(l, r, v, left_term, right_term, result)?,
                    _ => self.numeral_product(l, r, v, left_term, right_term, result)?,
                };
                let Some(equation) = equation else {
                    return Ok(None);
                };
                let variable = self.arena.insert(Term::Variable(0));
                let applied = self.source_apply(operation, variable);
                let body = self.source_apply(applied, right);
                let function = self.arena.insert(Term::Lambda {
                    domain: integer,
                    body,
                });
                let first = self.source_congruence(function, left, left_term, left_proof)?;
                let applied = self.source_apply(operation, left_term);
                let middle = self.source_apply(applied, right);
                let variable = self.arena.insert(Term::Variable(0));
                let body = self.source_apply(applied, variable);
                let function = self.arena.insert(Term::Lambda {
                    domain: integer,
                    body,
                });
                let second = self.source_congruence(function, right, right_term, right_proof)?;
                let combined = self.transitivity(integer, term, middle, normalized, first, second);
                let proof =
                    self.transitivity(integer, term, normalized, result, combined, equation);
                Ok(Some((value, result, proof)))
            }

            pub(crate) fn closed_source_predicate(
                &mut self,
                original: TermHandle,
                depth: usize,
            ) -> Result<Option<TermHandle>, BoundedDenotationError> {
                if depth > 128 {
                    return Err(BoundedDenotationError::DepthLimitExceeded);
                }
                let goal = self.source_head(original)?;
                if let Some((ty, left, right)) = self.identity_parts(goal) {
                    if self.arena.structurally_equal(left, right) {
                        return Ok(Some(self.arena.insert(Term::Refl { ty, value: left })));
                    }
                    let Some((lv, lc, lp)) = self.source_integer_value(left, depth + 1)? else {
                        return Ok(None);
                    };
                    let Some((rv, rc, rp)) = self.source_integer_value(right, depth + 1)? else {
                        return Ok(None);
                    };
                    if lv != rv {
                        return Ok(None);
                    }
                    let integer = self.integer_constant()?;
                    let reverse = self.symmetry(integer, right, rc, rp);
                    return Ok(Some(
                        self.transitivity(integer, left, lc, right, lp, reverse),
                    ));
                }
                if let Some(relation) = self.integer_relation(goal) {
                    let (strict, left, right) = match relation {
                        IntegerRelation::LessThan { left, right } => (true, left, right),
                        IntegerRelation::LessOrEqual { left, right } => (false, left, right),
                        _ => return Ok(None),
                    };
                    let Some((_, lc, lp)) = self.source_integer_value(left, depth + 1)? else {
                        return Ok(None);
                    };
                    let Some((_, rc, rp)) = self.source_integer_value(right, depth + 1)? else {
                        return Ok(None);
                    };
                    let normalized = self.order_term_application(strict, lc, rc)?;
                    let Some(evidence) = self.closed_inhabitant(normalized)? else {
                        return Ok(None);
                    };
                    let integer = self.integer_constant()?;
                    let left_reverse = self.symmetry(integer, left, lc, lp);
                    let right_reverse = self.symmetry(integer, right, rc, rp);
                    let first = self.integer_law_application(
                        if strict {
                            IntegerLaw::LessThanSubstituteLeft
                        } else {
                            IntegerLaw::LessOrEqualSubstituteLeft
                        },
                        &[lc, left, rc, left_reverse, evidence],
                    )?;
                    return self
                        .integer_law_application(
                            if strict {
                                IntegerLaw::LessThanSubstituteRight
                            } else {
                                IntegerLaw::LessOrEqualSubstituteRight
                            },
                            &[left, rc, right, right_reverse, first],
                        )
                        .map(Some);
                }
                match self.arena.get(goal) {
                    Term::Sigma { domain, codomain } => {
                        let Some(first) = self.closed_source_predicate(domain, depth + 1)? else {
                            return Ok(None);
                        };
                        let rest = substitute(&mut self.arena, codomain, first);
                        let Some(second) = self.closed_source_predicate(rest, depth + 1)? else {
                            return Ok(None);
                        };
                        Ok(Some(self.arena.insert(Term::Pair { first, second })))
                    }
                    Term::Box { ty } => {
                        let Term::Pi { domain, codomain } = self.arena.get(ty) else {
                            return Ok(None);
                        };
                        if !matches!(self.arena.get(domain), Term::Empty)
                            || !matches!(self.arena.get(codomain), Term::Empty)
                        {
                            return Ok(None);
                        }
                        let body = self.arena.insert(Term::Variable(0));
                        let value = self.arena.insert(Term::Lambda { domain, body });
                        Ok(Some(self.arena.insert(Term::BoxIntro { ty, value })))
                    }
                    _ => Ok(None),
                }
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use crate::SourceIntegerTheory;
            use crate::mathematical_core::{Context, Level, Sort, check_signature, check_type};

            #[test]
            fn source_integer_closed_arithmetic_is_evidence_for_the_exact_goal() {
                let mut arena = crate::TermArena::new();
                let mut declarations = crate::indexed_scheme(&mut arena);
                let mut theory = SourceIntegerTheory::new(&mut arena, &mut declarations).unwrap();
                let vocabulary = theory.vocabulary();
                let seven = theory
                    .literal(&mut arena, &mut declarations, BigInt::from_u64(7))
                    .unwrap();
                let type_zero = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
                declarations.push(crate::Declaration::assumption(0, type_zero));
                let three = theory
                    .literal(&mut arena, &mut declarations, BigInt::from_u64(3))
                    .unwrap();
                let ten = theory
                    .literal(&mut arena, &mut declarations, BigInt::from_u64(10))
                    .unwrap();
                let add = arena.insert(Term::Constant {
                    declaration: vocabulary.add,
                    levels: vec![],
                });
                let applied = arena.insert(Term::Apply {
                    function: add,
                    argument: seven,
                });
                let sum = arena.insert(Term::Apply {
                    function: applied,
                    argument: three,
                });
                let int = arena.insert(Term::Constant {
                    declaration: vocabulary.carrier,
                    levels: vec![],
                });
                let goal = arena.insert(Term::Id {
                    ty: int,
                    left: sum,
                    right: ten,
                });
                let evidence = theory
                    .closed_predicate(&mut arena, &mut declarations, goal)
                    .unwrap()
                    .unwrap();
                let wrong = arena.insert(Term::Id {
                    ty: int,
                    left: sum,
                    right: seven,
                });
                assert!(
                    theory
                        .closed_predicate(&mut arena, &mut declarations, wrong)
                        .unwrap()
                        .is_none()
                );
                let le = arena.insert(Term::Constant {
                    declaration: vocabulary.less_or_equal,
                    levels: vec![],
                });
                let function = arena.insert(Term::Apply {
                    function: le,
                    argument: sum,
                });
                let bound = arena.insert(Term::Apply {
                    function,
                    argument: ten,
                });
                let order = theory
                    .closed_predicate(&mut arena, &mut declarations, bound)
                    .unwrap()
                    .unwrap();
                let signature = check_signature(
                    &mut arena,
                    &declarations,
                    &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                )
                .unwrap();
                let context = Context::empty().with_signature(signature);
                check_type(
                    &mut arena,
                    &context,
                    evidence,
                    goal,
                    &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                )
                .unwrap();
                check_type(
                    &mut arena,
                    &context,
                    order,
                    bound,
                    &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                )
                .unwrap();
                assert!(
                    check_type(
                        &mut arena,
                        &context,
                        evidence,
                        wrong,
                        &mut Budget::new(DEFAULT_CONVERSION_STEPS)
                    )
                    .is_err()
                );
            }
        }
    }
    mod open {
        use super::super::{BoundedDenotationError, Denotation, IntegerLaw, IntegerRelation};
        use crate::mathematical_core::{
            Budget, Context, DEFAULT_CONVERSION_STEPS, Signature, Term, TermHandle, infer_sort,
            substitute,
        };

        #[derive(Clone, Copy)]
        struct NormalInteger {
            term: TermHandle,
            equation: TermHandle,
        }

        impl Denotation {
            fn source_binary(
                &mut self,
                term: TermHandle,
            ) -> Result<Option<(u32, TermHandle, TermHandle)>, BoundedDenotationError> {
                let term = self.source_head(term)?;
                let Term::Apply {
                    function,
                    argument: right,
                } = self.arena.get(term)
                else {
                    return Ok(None);
                };
                let function = self.source_head(function)?;
                let Term::Apply {
                    function,
                    argument: left,
                } = self.arena.get(function)
                else {
                    return Ok(None);
                };
                let Term::Constant { declaration, .. } = self.arena.get(function) else {
                    return Ok(None);
                };
                Ok(Some((declaration, left, right)))
            }

            fn source_normal_proposition(
                &mut self,
                term: TermHandle,
                depth: usize,
            ) -> Result<TermHandle, BoundedDenotationError> {
                if depth > 128 {
                    return Err(BoundedDenotationError::DepthLimitExceeded);
                }
                let head = self.source_head(term)?;
                let node = match self.arena.get(head) {
                    Term::Apply { function, argument } => Term::Apply {
                        function: self.source_normal_proposition(function, depth + 1)?,
                        argument: self.source_normal_proposition(argument, depth + 1)?,
                    },
                    Term::Id { ty, left, right } => Term::Id {
                        ty: self.source_normal_proposition(ty, depth + 1)?,
                        left: self.source_normal_proposition(left, depth + 1)?,
                        right: self.source_normal_proposition(right, depth + 1)?,
                    },
                    Term::Sigma { domain, codomain } => Term::Sigma {
                        domain: self.source_normal_proposition(domain, depth + 1)?,
                        codomain: self.source_normal_proposition(codomain, depth + 1)?,
                    },
                    _ => return Ok(head),
                };
                Ok(self.arena.insert(node))
            }

            fn source_normal_integer(
                &mut self,
                original: TermHandle,
                depth: usize,
            ) -> Result<NormalInteger, BoundedDenotationError> {
                if depth > 128 {
                    return Err(BoundedDenotationError::DepthLimitExceeded);
                }
                if let Some((_, term, equation)) = self.source_integer_value(original, depth + 1)? {
                    return Ok(NormalInteger { term, equation });
                }
                let term = self.source_head(original)?;
                let integer = self.integer_constant()?;
                let Some((operation, left, right)) = self.source_binary(term)? else {
                    let equation = self.arena.insert(Term::Refl {
                        ty: integer,
                        value: term,
                    });
                    return Ok(NormalInteger { term, equation });
                };
                if operation != self.add_operation()?
                    && operation != self.subtract_operation()?
                    && operation != self.multiply_operation()?
                {
                    let equation = self.arena.insert(Term::Refl {
                        ty: integer,
                        value: term,
                    });
                    return Ok(NormalInteger { term, equation });
                }
                let l = self.source_normal_integer(left, depth + 1)?;
                let r = self.source_normal_integer(right, depth + 1)?;
                let op = self.constant(operation);
                let variable = self.arena.insert(Term::Variable(0));
                let function = self.source_apply(op, variable);
                let body = self.source_apply(function, right);
                let function = self.arena.insert(Term::Lambda {
                    domain: integer,
                    body,
                });
                let first = self.source_congruence(function, left, l.term, l.equation)?;
                let function = self.source_apply(op, l.term);
                let middle = self.source_apply(function, right);
                let variable = self.arena.insert(Term::Variable(0));
                let body = self.source_apply(function, variable);
                let function = self.arena.insert(Term::Lambda {
                    domain: integer,
                    body,
                });
                let second = self.source_congruence(function, right, r.term, r.equation)?;
                let function = self.source_apply(op, l.term);
                let normalized = self.source_apply(function, r.term);
                let equation = self.transitivity(integer, term, middle, normalized, first, second);
                if operation == self.multiply_operation()?
                    && let Some((add, a, b)) = self.source_binary(r.term)?
                    && add == self.add_operation()?
                {
                    let expansion = self.distribute_add(l.term, a, b)?;
                    let first = self.multiply_terms(l.term, a)?;
                    let second = self.multiply_terms(l.term, b)?;
                    let expanded = self.add_terms(first, second)?;
                    let rest = self.source_normal_integer(expanded, depth + 1)?;
                    let combined =
                        self.transitivity(integer, term, normalized, expanded, equation, expansion);
                    let equation = self.transitivity(
                        integer,
                        term,
                        expanded,
                        rest.term,
                        combined,
                        rest.equation,
                    );
                    return Ok(NormalInteger {
                        term: rest.term,
                        equation,
                    });
                }
                Ok(NormalInteger {
                    term: normalized,
                    equation,
                })
            }

            fn source_flat_premises(
                &mut self,
                proposition: TermHandle,
                evidence: TermHandle,
                premises: &mut Vec<(TermHandle, TermHandle)>,
                depth: usize,
            ) -> Result<(), BoundedDenotationError> {
                if depth > 128 {
                    return Err(BoundedDenotationError::DepthLimitExceeded);
                }
                let proposition = self.source_normal_proposition(proposition, 0)?;
                if let Term::Sigma { domain, codomain } = self.arena.get(proposition) {
                    if self.occurs_free(codomain, 0) {
                        return Err(BoundedDenotationError::Unsupported(
                            "dependent source predicate premises require value abstraction",
                        ));
                    }
                    let first = self.arena.insert(Term::Fst { pair: evidence });
                    let second = self.arena.insert(Term::Snd { pair: evidence });
                    self.source_flat_premises(domain, first, premises, depth + 1)?;
                    let codomain = substitute(&mut self.arena, codomain, first);
                    self.source_flat_premises(codomain, second, premises, depth + 1)?;
                } else {
                    premises.push((proposition, evidence));
                }
                Ok(())
            }

            fn source_normal_order(
                &mut self,
                proposition: TermHandle,
                evidence: TermHandle,
            ) -> Result<Option<(TermHandle, TermHandle)>, BoundedDenotationError> {
                let Some(relation) = self.integer_relation(proposition) else {
                    return Ok(None);
                };
                let (strict, left, right) = match relation {
                    IntegerRelation::LessOrEqual { left, right } => (false, left, right),
                    IntegerRelation::LessThan { left, right } => (true, left, right),
                    _ => return Ok(None),
                };
                let l = self.source_normal_integer(left, 0)?;
                let r = self.source_normal_integer(right, 0)?;
                let first = self.integer_law_application(
                    if strict {
                        IntegerLaw::LessThanSubstituteLeft
                    } else {
                        IntegerLaw::LessOrEqualSubstituteLeft
                    },
                    &[left, l.term, right, l.equation, evidence],
                )?;
                let proof = self.integer_law_application(
                    if strict {
                        IntegerLaw::LessThanSubstituteRight
                    } else {
                        IntegerLaw::LessOrEqualSubstituteRight
                    },
                    &[l.term, right, r.term, r.equation, first],
                )?;
                Ok(Some((
                    self.order_term_application(strict, l.term, r.term)?,
                    proof,
                )))
            }

            fn source_order_goal(
                &mut self,
                goal: TermHandle,
                premises: &[(TermHandle, TermHandle)],
                depth: usize,
            ) -> Result<Option<TermHandle>, BoundedDenotationError> {
                if depth > 128 {
                    return Err(BoundedDenotationError::DepthLimitExceeded);
                }
                if let Some(proof) = self.closed_source_predicate(goal, 0)? {
                    return Ok(Some(proof));
                }
                for &(proposition, evidence) in premises {
                    if self.arena.structurally_equal(goal, proposition) {
                        return Ok(Some(evidence));
                    }
                }
                let Some(IntegerRelation::LessOrEqual { left, right }) =
                    self.integer_relation(goal)
                else {
                    return Ok(None);
                };
                let (Some((left_add, a, c)), Some((right_add, b, d))) =
                    (self.source_binary(left)?, self.source_binary(right)?)
                else {
                    return Ok(None);
                };
                if left_add != self.add_operation()? || right_add != self.add_operation()? {
                    return Ok(None);
                }
                let first_goal = self.order_term_application(false, a, b)?;
                let second_goal = self.order_term_application(false, c, d)?;
                let Some(first) = self.source_order_goal(first_goal, premises, depth + 1)? else {
                    return Ok(None);
                };
                let Some(second) = self.source_order_goal(second_goal, premises, depth + 1)? else {
                    return Ok(None);
                };
                self.add_order_both(a, b, c, d, first, second).map(Some)
            }

            fn source_symbolic_goal(
                &mut self,
                goal: TermHandle,
                premises: &[(TermHandle, TermHandle)],
            ) -> Result<Option<TermHandle>, BoundedDenotationError> {
                let Some(IntegerRelation::LessOrEqual { left, right }) =
                    self.integer_relation(goal)
                else {
                    return self.closed_source_predicate(goal, 0);
                };
                let l = self.source_normal_integer(left, 0)?;
                let r = self.source_normal_integer(right, 0)?;
                let normal = self.order_term_application(false, l.term, r.term)?;
                let Some(evidence) = self.source_order_goal(normal, premises, 0)? else {
                    return Ok(None);
                };
                let integer = self.integer_constant()?;
                let reverse_l = self.symmetry(integer, left, l.term, l.equation);
                let reverse_r = self.symmetry(integer, right, r.term, r.equation);
                let first = self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteLeft,
                    &[l.term, left, r.term, reverse_l, evidence],
                )?;
                self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteRight,
                    &[left, r.term, right, reverse_r, first],
                )
                .map(Some)
            }

            /// `level_arity` is the caller's universe-level binder count: a goal
            /// over a level-generic family (a type-parameterized source data
            /// constructor) mentions those level parameters, so sorting it in a
            /// level-free context would refuse a well-formed proposition.
            pub(super) fn source_assumption_predicate(
                &mut self,
                goal: TermHandle,
                assumptions: &[TermHandle],
                level_arity: u32,
            ) -> Result<Option<TermHandle>, BoundedDenotationError> {
                let context = Context::with_level_arity(level_arity)
                    .with_signature(Signature::from_declarations(self.declarations.clone()));
                for proposition in assumptions.iter().copied().chain(std::iter::once(goal)) {
                    infer_sort(
                        &mut self.arena,
                        &context,
                        proposition,
                        &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                    )
                    .map_err(BoundedDenotationError::Kernel)?;
                }
                let goal = self.source_normal_proposition(goal, 0)?;
                let mut premises = Vec::new();
                for (position, &assumption) in assumptions.iter().enumerate() {
                    let evidence = self
                        .arena
                        .insert(Term::Variable((assumptions.len() - 1 - position) as u32));
                    self.source_flat_premises(assumption, evidence, &mut premises, 0)?;
                }
                let identities = premises
                    .iter()
                    .copied()
                    .filter(|(proposition, _)| self.identity_parts(*proposition).is_some())
                    .collect::<Vec<_>>();
                let mut orders = Vec::new();
                for &(proposition, evidence) in &premises {
                    if let Some(order) = self.source_normal_order(proposition, evidence)? {
                        orders.push(order);
                    }
                }
                let Some(mut proof) =
                    self.source_rewrite_proof(goal, &identities, |this, rewritten| {
                        this.source_symbolic_goal(rewritten, &orders)
                    })?
                else {
                    return Ok(None);
                };
                for &domain in assumptions.iter().rev() {
                    proof = self.arena.insert(Term::Lambda {
                        domain,
                        body: proof,
                    });
                }
                Ok(Some(proof))
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use crate::mathematical_core::{Declaration, check_signature, check_type};
            use crate::{SourceIntegerTheory, TermArena};
            use numerics::bignum::BigInt;

            fn binary(
                arena: &mut TermArena,
                operation: u32,
                left: TermHandle,
                right: TermHandle,
            ) -> TermHandle {
                let function = arena.insert(Term::Constant {
                    declaration: operation,
                    levels: vec![],
                });
                let function = arena.insert(Term::Apply {
                    function,
                    argument: left,
                });
                arena.insert(Term::Apply {
                    function,
                    argument: right,
                })
            }

            #[test]
            fn dependent_value_premises_are_explicitly_unsupported() {
                let mut arena = TermArena::new();
                let mut declarations = crate::indexed_scheme(&mut arena);
                let mut theory = SourceIntegerTheory::new(&mut arena, &mut declarations).unwrap();
                let integer = arena.insert(Term::Constant {
                    declaration: theory.vocabulary().carrier,
                    levels: vec![],
                });
                let variable = arena.insert(Term::Variable(0));
                let predicate = arena.insert(Term::Id {
                    ty: integer,
                    left: variable,
                    right: variable,
                });
                let premise = arena.insert(Term::Sigma {
                    domain: integer,
                    codomain: predicate,
                });
                let zero = theory
                    .literal(&mut arena, &mut declarations, BigInt::from_u64(0))
                    .unwrap();
                let goal = arena.insert(Term::Id {
                    ty: integer,
                    left: zero,
                    right: zero,
                });
                assert!(matches!(
                    theory.predicate_from_assumptions(
                        &mut arena,
                        &mut declarations,
                        goal,
                        &[premise],
                        0
                    ),
                    Err(BoundedDenotationError::Unsupported(_))
                ));
            }

            #[test]
            fn symbolic_source_step_abstracts_its_predicates_and_induction_hypothesis() {
                let mut arena = TermArena::new();
                let mut declarations = crate::indexed_scheme(&mut arena);
                let mut theory = SourceIntegerTheory::new(&mut arena, &mut declarations).unwrap();
                let vocabulary = theory.vocabulary();
                let integer = arena.insert(Term::Constant {
                    declaration: vocabulary.carrier,
                    levels: vec![],
                });
                let mut value = || {
                    let declaration = declarations.len() as u32;
                    declarations.push(Declaration::assumption(0, integer));
                    arena.insert(Term::Constant {
                        declaration,
                        levels: vec![],
                    })
                };
                let length = value();
                let total = value();
                let previous = value();
                let previous_total = value();
                let head = value();
                let one = theory
                    .literal(&mut arena, &mut declarations, BigInt::from_u64(1))
                    .unwrap();
                let ten = theory
                    .literal(&mut arena, &mut declarations, BigInt::from_u64(10))
                    .unwrap();
                let head_bound = binary(&mut arena, vocabulary.less_or_equal, head, ten);
                let increment = binary(&mut arena, vocabulary.add, previous, one);
                let length_equation = arena.insert(Term::Id {
                    ty: integer,
                    left: increment,
                    right: length,
                });
                let sum = binary(&mut arena, vocabulary.add, previous_total, head);
                let total_equation = arena.insert(Term::Id {
                    ty: integer,
                    left: sum,
                    right: total,
                });
                let prior_limit = binary(&mut arena, vocabulary.multiply, ten, previous);
                let ih = binary(
                    &mut arena,
                    vocabulary.less_or_equal,
                    previous_total,
                    prior_limit,
                );
                let limit = binary(&mut arena, vocabulary.multiply, ten, length);
                let goal = binary(&mut arena, vocabulary.less_or_equal, total, limit);
                let assumptions = [head_bound, length_equation, total_equation, ih];
                let proof = theory
                    .predicate_from_assumptions(
                        &mut arena,
                        &mut declarations,
                        goal,
                        &assumptions,
                        0,
                    )
                    .unwrap()
                    .unwrap();
                assert!(
                    theory
                        .predicate_from_assumptions(
                            &mut arena,
                            &mut declarations,
                            goal,
                            &assumptions[..3],
                            0
                        )
                        .unwrap()
                        .is_none()
                );
                for missing in 0..3 {
                    let reduced = assumptions
                        .iter()
                        .enumerate()
                        .filter_map(|(position, &assumption)| {
                            (position != missing).then_some(assumption)
                        })
                        .collect::<Vec<_>>();
                    assert!(
                        theory
                            .predicate_from_assumptions(
                                &mut arena,
                                &mut declarations,
                                goal,
                                &reduced,
                                0
                            )
                            .unwrap()
                            .is_none(),
                        "missing source premise {missing}"
                    );
                }
                assert!(
                    !declarations
                        .iter()
                        .any(|declaration| declaration.body.is_none()
                            && arena.structurally_equal(declaration.ty, goal))
                );
                let mut expected = goal;
                for &domain in assumptions.iter().rev() {
                    expected = arena.insert(Term::Pi {
                        domain,
                        codomain: expected,
                    });
                }
                let signature = check_signature(
                    &mut arena,
                    &declarations,
                    &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                )
                .unwrap();
                check_type(
                    &mut arena,
                    &Context::empty().with_signature(signature),
                    proof,
                    expected,
                    &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                )
                .unwrap();
            }
        }
    }

    #[derive(Clone, Copy)]
    pub struct SourceIntegerVocabulary {
        pub carrier: u32,
        pub less_than: u32,
        pub less_or_equal: u32,
        pub add: u32,
        pub subtract: u32,
        pub multiply: u32,
    }

    pub struct SourceIntegerTheory {
        state: Denotation,
        vocabulary: SourceIntegerVocabulary,
    }

    impl SourceIntegerTheory {
        pub fn new(
            arena: &mut TermArena,
            declarations: &mut Vec<Declaration>,
        ) -> Result<Self, BoundedDenotationError> {
            let mut state = Denotation::new_in(std::mem::take(arena), std::mem::take(declarations));
            let vocabulary = (|| {
                Ok(SourceIntegerVocabulary {
                    carrier: state.integer()?,
                    less_than: state.integer_less_than()?,
                    less_or_equal: state.integer_less_or_equal()?,
                    add: state.add_operation()?,
                    subtract: state.subtract_operation()?,
                    multiply: state.multiply_operation()?,
                })
            })();
            std::mem::swap(arena, &mut state.arena);
            std::mem::swap(declarations, &mut state.declarations);
            Ok(Self {
                state,
                vocabulary: vocabulary?,
            })
        }

        pub fn source_safety_roster(&self) -> Vec<u32> {
            let state = &self.state;
            let mut positions = state
                .integer
                .into_iter()
                .chain(state.integer_less_than)
                .chain(state.integer_less_or_equal)
                .chain(state.integer_laws.values().copied())
                .collect::<Vec<_>>();
            positions.extend(state.binary_numerals.source_safety_roster());
            positions.extend(state.addition.source_safety_roster());
            positions.extend(state.subtraction.source_safety_roster());
            positions.extend(state.multiplication.source_safety_roster());
            positions.sort_unstable();
            positions.dedup();
            positions
        }

        pub fn vocabulary(&self) -> SourceIntegerVocabulary {
            self.vocabulary
        }

        pub fn literal(
            &mut self,
            arena: &mut TermArena,
            declarations: &mut Vec<Declaration>,
            value: BigInt,
        ) -> Result<TermHandle, BoundedDenotationError> {
            self.with(arena, declarations, |state| state.math_literal(value))
        }

        pub fn closed_predicate(
            &mut self,
            arena: &mut TermArena,
            declarations: &mut Vec<Declaration>,
            goal: TermHandle,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            self.with(arena, declarations, |state| {
                state.closed_source_predicate(goal, 0)
            })
        }

        pub fn predicate_from_assumptions(
            &mut self,
            arena: &mut TermArena,
            declarations: &mut Vec<Declaration>,
            goal: TermHandle,
            assumptions: &[TermHandle],
            level_arity: u32,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            self.with(arena, declarations, |state| {
                state.source_assumption_predicate(goal, assumptions, level_arity)
            })
        }

        fn with<T>(
            &mut self,
            arena: &mut TermArena,
            declarations: &mut Vec<Declaration>,
            produce: impl FnOnce(&mut Denotation) -> Result<T, BoundedDenotationError>,
        ) -> Result<T, BoundedDenotationError> {
            std::mem::swap(arena, &mut self.state.arena);
            std::mem::swap(declarations, &mut self.state.declarations);
            let result = produce(&mut self.state);
            std::mem::swap(arena, &mut self.state.arena);
            std::mem::swap(declarations, &mut self.state.declarations);
            result
        }
    }
}
pub use source_content::SourceContentTheory;
mod subtraction {
    use std::collections::BTreeMap;

    use semantic_vocabulary::{
        IntegerCarrier, IntegerMathTerm, IntegerSign, IntegerValue, Proposition, ScalarTerm,
    };

    use super::denotation::{BinaryBoundOperation, math_less_or_equal_sides};
    use super::{
        BoundedDenotationError, Denotation, IntegerLaw, Term, TermHandle,
        affine_carrier_endpoint_equality, affine_root_premise, affine_witness_root_expression,
    };
    use crate::mathematical_core::scheme_dsl::{self, apps, id, pi, pis, scheme_at, v};
    use crate::{ClosedIntegerEvaluator, kernel::KernelError, proof::ProofError};

    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    enum Law {
        Zero,
        Antitone,
        SelfZero,
        LessOrEqualAntitone,
        CancelAddLeft,
        CancelAddRight,
        MonotoneAntitone,
    }

    #[derive(Default)]
    pub(super) struct Subtraction {
        operation: Option<u32>,
        laws: BTreeMap<Law, u32>,
        irreflexivity: Option<u32>,
        pub(super) numeral_differences: BTreeMap<(IntegerValue, IntegerValue, IntegerValue), u32>,
    }

    impl Denotation {
        pub(super) fn subtract_operation(&mut self) -> Result<u32, BoundedDenotationError> {
            self.binary_integer_operation(|this| &mut this.subtraction.operation)
        }

        pub(super) fn subtract_terms(
            &mut self,
            left: TermHandle,
            right: TermHandle,
        ) -> Result<TermHandle, BoundedDenotationError> {
            self.binary_terms(Self::subtract_operation, left, right)
        }

        fn subtract_law_application(
            &mut self,
            law: Law,
            arguments: &[TermHandle],
        ) -> Result<TermHandle, BoundedDenotationError> {
            self.roster_law_application(
                |this| &mut this.subtraction.laws,
                law,
                |this| -> Result<scheme_dsl::Syntax, BoundedDenotationError> {
                    let integer = this.integer()?;
                    let operation = this.subtract_operation()?;
                    let less_than = this.integer_less_than()?;
                    let zero = this.binary_integer(false, 0)?;
                    let constant = |position| scheme_at(position, Vec::new());
                    let carrier = || constant(integer);
                    let binders = |names: &[&'static str], body| {
                        pis(names.iter().map(|name| (*name, carrier())), body)
                    };
                    let subtract = |left, right| apps(constant(operation), [left, right]);
                    let lt = |left, right| apps(constant(less_than), [left, right]);
                    let statement = match law {
                        Law::SelfZero => pi(
                            "x",
                            carrier(),
                            id(carrier(), subtract(v("x"), v("x")), constant(zero)),
                        ),
                        Law::LessOrEqualAntitone => {
                            let less_or_equal = this.integer_less_or_equal()?;
                            let le = |left, right| apps(constant(less_or_equal), [left, right]);
                            binders(
                                &["x", "a", "b"],
                                pi(
                                    "_",
                                    le(v("a"), v("b")),
                                    le(subtract(v("x"), v("b")), subtract(v("x"), v("a"))),
                                ),
                            )
                        }
                        Law::MonotoneAntitone => {
                            let less_or_equal = this.integer_less_or_equal()?;
                            let le = |left, right| apps(constant(less_or_equal), [left, right]);
                            binders(
                                &["a", "b", "c", "d"],
                                pi(
                                    "_",
                                    le(v("a"), v("b")),
                                    pi(
                                        "_",
                                        le(v("d"), v("c")),
                                        le(subtract(v("a"), v("c")), subtract(v("b"), v("d"))),
                                    ),
                                ),
                            )
                        }
                        Law::CancelAddLeft | Law::CancelAddRight => {
                            let less_or_equal = this.integer_less_or_equal()?;
                            let add = this.add_operation()?;
                            let le = |left, right| apps(constant(less_or_equal), [left, right]);
                            let plus = |left, right| apps(constant(add), [left, right]);
                            let (premise, conclusion) = if law == Law::CancelAddLeft {
                                (
                                    le(plus(v("a"), v("c")), v("b")),
                                    le(v("a"), subtract(v("b"), v("c"))),
                                )
                            } else {
                                (
                                    le(v("b"), plus(v("a"), v("c"))),
                                    le(subtract(v("b"), v("c")), v("a")),
                                )
                            };
                            binders(&["a", "b", "c"], pi("_", premise, conclusion))
                        }
                        Law::Zero => pi(
                            "x",
                            carrier(),
                            id(carrier(), subtract(v("x"), constant(zero)), v("x")),
                        ),
                        Law::Antitone => binders(
                            &["x", "a", "b"],
                            pi(
                                "_",
                                lt(v("a"), v("b")),
                                lt(subtract(v("x"), v("b")), subtract(v("x"), v("a"))),
                            ),
                        ),
                    };
                    Ok(statement)
                },
                arguments,
            )
        }

        pub(super) fn irreflexive(
            &mut self,
            value: TermHandle,
            evidence: TermHandle,
        ) -> Result<TermHandle, BoundedDenotationError> {
            let position = self.interned_slot_assumption(
                |this| &mut this.subtraction.irreflexivity,
                |this| {
                    let integer = this.integer_constant()?;
                    let relation_position = this.integer_less_than()?;
                    let relation = this.constant(relation_position);
                    let variable = this.arena.insert(Term::Variable(0));
                    let domain = this.arena.apply_binary(relation, variable, variable);
                    let empty = this.arena.insert(Term::Empty);
                    let codomain = this.arena.insert(Term::Pi {
                        domain,
                        codomain: empty,
                    });
                    let ty = this.arena.insert(Term::Pi {
                        domain: integer,
                        codomain,
                    });
                    Ok(ty)
                },
            )?;
            let function = self.constant(position);
            Ok(self.arena.apply_binary(function, value, evidence))
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
            let equality =
                self.symmetry(integer, result_term, subtraction_term, difference_evidence);
            self.integer_law_application(
                IntegerLaw::LessThanSubstituteLeft,
                &[subtraction_term, result_term, left_term, equality, decrease],
            )
        }

        pub(super) fn addition_order_evidence(
            &mut self,
            sum: &Proposition,
            sum_evidence: TermHandle,
            positive_evidence: TermHandle,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            let Proposition::Equal(
                result,
                addition @ ScalarTerm::ExactIntegerAdd { left, right, .. },
            ) = sum
            else {
                unreachable!("bounded addition order checker ran first")
            };
            if addition.integer_value().is_some() {
                return Ok(None);
            }
            let integer = self.integer_constant()?;
            let result_term = self.fixed_scalar_term(result)?;
            let original = self.fixed_scalar_term(left)?;
            let increment = self.fixed_scalar_term(right)?;
            let addition_term = self.fixed_scalar_term(addition)?;
            let zero = self.math_term(&IntegerMathTerm::literal(IntegerValue::Unsigned(0)))?;
            let forward = self.symmetry(integer, result_term, addition_term, sum_evidence);
            let inverse = self.add_right_inverse(original, increment, result_term, forward)?;
            let difference = self.subtract_terms(result_term, increment)?;
            let subtract_zero = self.subtract_terms(result_term, zero)?;
            let decrease = self.subtract_law_application(
                Law::Antitone,
                &[result_term, zero, increment, positive_evidence],
            )?;
            let equality = self.subtract_law_application(Law::Zero, &[result_term])?;
            let below = self.integer_law_application(
                IntegerLaw::LessThanSubstituteRight,
                &[difference, subtract_zero, result_term, equality, decrease],
            )?;
            let backwards = self.symmetry(integer, original, difference, inverse);
            self.integer_law_application(
                IntegerLaw::LessThanSubstituteLeft,
                &[difference, original, result_term, backwards, below],
            )
            .map(Some)
        }

        pub(super) fn subtraction_antitone_evidence(
            &mut self,
            smaller: &Proposition,
            smaller_evidence: TermHandle,
            larger: &Proposition,
            larger_evidence: TermHandle,
            order_evidence: TermHandle,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            let (
                Proposition::Equal(
                    smaller_result,
                    low @ ScalarTerm::ExactIntegerSubtract {
                        left: minuend,
                        right: large,
                        ..
                    },
                ),
                Proposition::Equal(
                    larger_result,
                    high @ ScalarTerm::ExactIntegerSubtract { right: small, .. },
                ),
            ) = (smaller, larger)
            else {
                unreachable!("bounded subtraction antitonicity checker ran first")
            };
            if low.integer_value().is_some() || high.integer_value().is_some() {
                return Ok(None);
            }
            let integer = self.integer_constant()?;
            let smaller_term = self.fixed_scalar_term(smaller_result)?;
            let larger_term = self.fixed_scalar_term(larger_result)?;
            let minuend = self.fixed_scalar_term(minuend)?;
            let large = self.fixed_scalar_term(large)?;
            let small = self.fixed_scalar_term(small)?;
            let low = self.fixed_scalar_term(low)?;
            let high = self.fixed_scalar_term(high)?;
            let decrease = self.subtract_law_application(
                Law::Antitone,
                &[minuend, small, large, order_evidence],
            )?;
            let low_back = self.symmetry(integer, smaller_term, low, smaller_evidence);
            let left = self.integer_law_application(
                IntegerLaw::LessThanSubstituteLeft,
                &[low, smaller_term, high, low_back, decrease],
            )?;
            let high_back = self.symmetry(integer, larger_term, high, larger_evidence);
            self.integer_law_application(
                IntegerLaw::LessThanSubstituteRight,
                &[smaller_term, high, larger_term, high_back, left],
            )
            .map(Some)
        }

        pub(super) fn unsigned_subtract_bound_evidence(
            &mut self,
            premise: &Proposition,
            evidence: TermHandle,
            witness: &crate::IntegerAffineWitness,
            goal: &Proposition,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            let ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } = &witness.target
            else {
                return Ok(None);
            };
            let direct = |term: &ScalarTerm| {
                matches!(term, ScalarTerm::Value { .. } | ScalarTerm::Integer { .. })
            };
            if scalar_type.carrier() != IntegerCarrier::Fixed
                || scalar_type.sign() != IntegerSign::Unsigned
                || !witness.definition_axioms.is_empty()
                || !witness.literal_axioms.is_empty()
                || &witness.root != right.as_ref()
                || left == right
                || !direct(left)
                || !direct(right)
                || premise
                    != &Proposition::LessOrEqual(right.as_ref().clone(), left.as_ref().clone())
            {
                return Ok(None);
            }
            let left_term = self.fixed_scalar_term(left)?;
            let right_term = self.fixed_scalar_term(right)?;
            let zero = self.math_term(&IntegerMathTerm::literal(IntegerValue::Unsigned(0)))?;
            let goal_type = self.denote(goal)?;
            let Some(super::IntegerRelation::LessOrEqual {
                right: difference, ..
            }) = self.integer_relation(goal_type)
            else {
                unreachable!("checked unsigned subtraction bound denotes IntLe")
            };
            if let (
                Some((_, IntegerValue::Unsigned(left_value))),
                Some((_, IntegerValue::Unsigned(right_value))),
            ) = (left.integer_value(), right.integer_value())
            {
                if left_value < right_value {
                    let order = self.literal_order(
                        IntegerValue::Unsigned(left_value),
                        IntegerValue::Unsigned(right_value),
                    )?;
                    let contradiction = self.integer_law_application(
                        super::IntegerLaw::LessThanLessOrEqualTransitivity,
                        &[left_term, right_term, left_term, order, evidence],
                    )?;
                    let scrutinee = self.irreflexive(left_term, contradiction)?;
                    return Ok(Some(self.arena.insert(Term::EmptyElim {
                        ty: goal_type,
                        scrutinee,
                    })));
                }
                let result = left_value - right_value;
                let strict =
                    self.literal_order(IntegerValue::Unsigned(0), IntegerValue::Unsigned(result))?;
                let order = self.integer_law_application(
                    super::IntegerLaw::LessThanToLessOrEqual,
                    &[zero, difference, strict],
                )?;
                return Ok(Some(order));
            }
            let self_difference = self.subtract_terms(left_term, left_term)?;
            let order = self.subtract_law_application(
                Law::LessOrEqualAntitone,
                &[left_term, right_term, left_term, evidence],
            )?;
            let equality = self.subtract_law_application(Law::SelfZero, &[left_term])?;
            self.integer_law_application(
                super::IntegerLaw::LessOrEqualSubstituteLeft,
                &[self_difference, zero, difference, equality, order],
            )
            .map(Some)
        }

        pub(super) fn correlated_subtract_bound_evidence(
            &mut self,
            premise: &Proposition,
            mut evidence: TermHandle,
            witness: &crate::IntegerAffineWitness,
            conclusion: &Proposition,
            definitions: &[(Proposition, TermHandle)],
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            let ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } = &witness.target
            else {
                return Ok(None);
            };
            if scalar_type.carrier() != IntegerCarrier::Fixed {
                return Ok(None);
            }
            let Some((lower, bound, difference)) =
                math_less_or_equal_sides(conclusion, crate::term_matchers::is_math_subtract)
            else {
                return Ok(None);
            };

            let evaluate = |term: &IntegerMathTerm| {
                ClosedIntegerEvaluator::default()
                    .evaluate_closed(term)
                    .map_err(|error| {
                        BoundedDenotationError::Certificate(ProofError::PrimitiveJudgment(
                            KernelError::ClosedIntegerEvaluation(error),
                        ))
                    })
            };
            let bound_value = evaluate(bound)?;
            let difference_value = evaluate(difference)?;
            if let (Some(bound_value), Some(difference_value)) = (&bound_value, &difference_value) {
                let (goal_left, goal_right, left_value, right_value) = if lower {
                    (bound, difference, bound_value, difference_value)
                } else {
                    (difference, bound, difference_value, bound_value)
                };
                return self.closed_order_bound_evidence(
                    premise,
                    evidence,
                    conclusion,
                    goal_left,
                    goal_right,
                    left_value,
                    right_value,
                );
            }
            if difference_value.is_some() {
                return Ok(None);
            }
            let Some((expression, equality)) = affine_witness_root_expression(witness, definitions)
            else {
                return Ok(None);
            };
            let ScalarTerm::ExactIntegerAdd {
                scalar_type: add_type,
                left: endpoint,
                right: addend,
            } = expression
            else {
                return Ok(None);
            };
            let expected_premise = affine_root_premise(witness, lower, left.as_ref());
            if add_type != scalar_type || addend != right || premise != &expected_premise {
                return Ok(None);
            }
            let Some((endpoint_equality, endpoint_literal)) =
                affine_carrier_endpoint_equality(*scalar_type, lower, endpoint, definitions)
            else {
                return Ok(None);
            };
            let left_term = self.fixed_scalar_term(left)?;
            let right_term = self.fixed_scalar_term(right)?;
            let endpoint_term = self.fixed_scalar_term(endpoint)?;
            let expression_term = self.fixed_scalar_term(expression)?;
            let Some(stepped) = self.affine_root_equality_step(
                lower,
                witness,
                left_term,
                expression_term,
                equality,
                evidence,
            )?
            else {
                return Ok(None);
            };
            evidence = stepped;
            let sum = self.add_terms(endpoint_term, right_term)?;
            if !self.arena.structurally_equal(expression_term, sum) {
                let (Some((_, endpoint)), Some((_, addend)), Some((_, total))) = (
                    endpoint.integer_value(),
                    right.integer_value(),
                    expression.integer_value(),
                ) else {
                    return Ok(None);
                };
                let Some(equation) = self.numeral_sum(
                    endpoint,
                    addend,
                    total,
                    endpoint_term,
                    right_term,
                    expression_term,
                )?
                else {
                    return Ok(None);
                };
                let integer = self.integer_constant()?;
                let equation = self.symmetry(integer, sum, expression_term, equation);
                evidence = if lower {
                    self.integer_law_application(
                        IntegerLaw::LessOrEqualSubstituteLeft,
                        &[expression_term, sum, left_term, equation, evidence],
                    )?
                } else {
                    self.integer_law_application(
                        IntegerLaw::LessOrEqualSubstituteRight,
                        &[left_term, expression_term, sum, equation, evidence],
                    )?
                };
            }
            let order = if lower {
                self.subtract_law_application(
                    Law::CancelAddLeft,
                    &[endpoint_term, left_term, right_term, evidence],
                )?
            } else {
                self.subtract_law_application(
                    Law::CancelAddRight,
                    &[endpoint_term, left_term, right_term, evidence],
                )?
            };
            self.affine_endpoint_equality_order(
                lower,
                endpoint_equality,
                &endpoint_literal,
                endpoint_term,
                order,
                |d| d.subtract_terms(left_term, right_term),
            )
        }

        pub(super) fn numeral_difference(
            &mut self,
            minuend: IntegerValue,
            subtrahend: IntegerValue,
            difference: IntegerValue,
            minuend_term: TermHandle,
            subtrahend_term: TermHandle,
            difference_term: TermHandle,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            self.numeral_binary_operation(
                minuend,
                subtrahend,
                difference,
                minuend_term,
                subtrahend_term,
                difference_term,
                |left, right| left.sub(right),
                Self::subtract_terms,
                |denotation| &mut denotation.subtraction.numeral_differences,
            )
        }

        pub(super) fn direct_subtract_bound_evidence(
            &mut self,
            premise: &Proposition,
            evidence: TermHandle,
            witness: &crate::IntegerAffineWitness,
            conclusion: &Proposition,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            self.direct_binary_bound_evidence(
                &Subtraction::bound_operation(),
                premise,
                evidence,
                witness,
                conclusion,
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn exact_subtract_definition_bound_evidence(
            &mut self,
            left_bound: &Proposition,
            left_evidence: TermHandle,
            right_bound: &Proposition,
            right_evidence: TermHandle,
            definition: &Proposition,
            definition_evidence: TermHandle,
            conclusion: &Proposition,
        ) -> Result<Option<TermHandle>, BoundedDenotationError> {
            self.exact_binary_definition_bound_evidence(
                &Subtraction::bound_operation(),
                left_bound,
                left_evidence,
                right_bound,
                right_evidence,
                definition,
                definition_evidence,
                conclusion,
            )
        }
    }

    impl Subtraction {
        fn bound_operation() -> BinaryBoundOperation {
            BinaryBoundOperation {
                target: crate::term_matchers::exact_integer_subtract,
                is_target_operation: crate::term_matchers::is_math_subtract,
                right_antitone: true,
                terms: Denotation::subtract_terms,
                monotone: |denotation, arguments| {
                    denotation.subtract_law_application(Law::MonotoneAntitone, arguments)
                },
                numeral_equation: Denotation::numeral_difference,
            }
        }

        pub(super) fn source_safety_roster(&self) -> Vec<u32> {
            self.operation
                .into_iter()
                .chain(self.laws.values().copied())
                .chain(self.irreflexivity)
                .chain(self.numeral_differences.values().copied())
                .collect()
        }
    }

    #[cfg(test)]
    mod bound_tests {
        use std::collections::BTreeSet;

        use super::super::test_utilities::open_operation_preserves_skipped_numeric_resource_refusals;
        use super::super::test_utilities::{
            assert_refused, direct_affine_proof, direct_fixture, elaborate_checked, mathematical,
            verify_then_elaborate,
        };
        use crate::proof::assumption_proof;
        use crate::{
            Budget, IntegerAffineWitness, ProofNode, ProofRule, value_context,
            verify_bounded_certificate,
        };
        use semantic_vocabulary::IntegerMathLiteral;
        use semantic_vocabulary::{
            IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, Proposition,
            PropositionContext, ScalarTerm, ScalarType, ValueId,
        };
        use std::slice::from_ref;

        macro_rules! sub {
            ($a:expr => $b:expr) => {
                IntegerMathTerm::Subtract($a, $b)
            };
        }

        fn check(left: ScalarTerm, right: ScalarTerm) {
            let ScalarType::Integer(integer) = left.scalar_type() else {
                unreachable!()
            };
            let context = value_context(left.scalar_type(), 2);
            let premise = Proposition::LessOrEqual(right.clone(), left.clone());
            let goal = Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::literal(IntegerValue::Unsigned(0)),
                sub!(Box::new(mathematical(&left)) => Box::new(mathematical(&right))),
            );
            let proof = ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::IntegerAffineBound {
                    root_bound: Box::new(assumption_proof(premise.clone(), 0)),
                    witness: IntegerAffineWitness {
                        root: right.clone(),
                        target: ScalarTerm::exact_integer_subtract(
                            integer,
                            left.clone(),
                            right.clone(),
                        )
                        .unwrap(),
                        definition_axioms: Vec::new(),
                        literal_axioms: Vec::new(),
                    },
                },
            };
            let assumptions = [premise];
            let parameters = BTreeSet::new();
            let elaboration =
                elaborate_checked(&context, &goal, &assumptions, &[], &parameters, &proof);
            assert!(elaboration.denotation.rule_axioms.is_empty());
            assert!(elaboration.denotation.decisions.is_empty());
            if left.integer_value().is_none() || right.integer_value().is_none() {
                assert_eq!(elaboration.denotation.subtraction.laws.len(), 2);
                assert!(
                    elaboration
                        .denotation
                        .subtraction
                        .laws
                        .contains_key(&super::Law::SelfZero)
                );
                assert!(
                    elaboration
                        .denotation
                        .subtraction
                        .laws
                        .contains_key(&super::Law::LessOrEqualAntitone)
                );
            }

            let mut budget = Budget::default();
            let denoted =
                verify_bounded_certificate(&context, &goal, &assumptions, &[], &proof, &mut budget)
                    .unwrap();
            eprintln!(
                "unsigned subtract bound {:?}, remaining={}",
                denoted.receipt(),
                budget.remaining()
            );
            assert!(denoted.certificate.signature.len() < 800);
            assert_refused(&context, &goal, &[], &[], &proof);
            let mut reversed = proof.clone();
            let ProofRule::IntegerAffineBound { root_bound, .. } = &mut reversed.rule else {
                unreachable!()
            };
            root_bound.conclusion = Proposition::LessOrEqual(left, right);
            let reversed_premise = root_bound.conclusion.clone();
            assert_refused(&context, &goal, from_ref(&reversed_premise), &[], &reversed);
        }

        #[test]
        fn unsigned_subtraction_bounds_preserve_open_and_closed_conditional_domain() {
            let integer = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
            let value = |index| {
                ScalarTerm::value(ValueId::new(index).unwrap(), ScalarType::Integer(integer))
            };
            let literal =
                |number| ScalarTerm::integer(integer, IntegerValue::Unsigned(number)).unwrap();
            check(value(1), value(2));
            check(value(1), literal(2));
            check(literal(2), value(2));
            for (left, right) in [
                (2, 1),
                (5, 2),
                (0, 1),
                (1, 2),
                (u128::MAX, 0),
                (u128::MAX, 1),
                (0, u128::MAX),
            ] {
                check(literal(left), literal(right));
            }
        }

        #[test]
        fn correlated_subtract_lower_uses_the_add_cancel_law() {
            let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
            let scalar = ScalarType::Integer(integer);
            let left = ScalarTerm::value(ValueId::new(1).unwrap(), scalar);
            let context =
                PropositionContext::from_value_types([(ValueId::new(1).unwrap(), scalar)]).unwrap();
            let literal =
                |number| ScalarTerm::integer(integer, IntegerValue::Unsigned(number)).unwrap();
            let right = literal(7);
            let root = ScalarTerm::exact_integer_add(integer, literal(0), right.clone()).unwrap();
            let premise = Proposition::LessOrEqual(root.clone(), left.clone());
            let goal = Proposition::IntegerMathLessOrEqual(
                mathematical(&literal(0)),
                sub!(Box::new(mathematical(&left)) => Box::new(mathematical(&right))),
            );
            let proof = ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::IntegerAffineBound {
                    root_bound: Box::new(assumption_proof(premise.clone(), 0)),
                    witness: IntegerAffineWitness {
                        root,
                        target: ScalarTerm::exact_integer_subtract(
                            integer,
                            left.clone(),
                            right.clone(),
                        )
                        .unwrap(),
                        definition_axioms: Vec::new(),
                        literal_axioms: Vec::new(),
                    },
                },
            };
            let assumptions = [premise];
            let parameters = BTreeSet::new();
            let elaboration =
                verify_then_elaborate(&context, &goal, &assumptions, &parameters, &proof);
            assert!(elaboration.denotation.rule_axioms.is_empty());
            assert_eq!(elaboration.denotation.subtraction.laws.len(), 1);
            assert!(
                elaboration
                    .denotation
                    .subtraction
                    .laws
                    .contains_key(&super::Law::CancelAddLeft)
            );
            assert_eq!(elaboration.denotation.addition.numeral_sums.len(), 1);
            assert_eq!(
                elaboration
                    .denotation
                    .addition
                    .numeral_sums
                    .keys()
                    .next()
                    .unwrap(),
                &(
                    IntegerValue::Unsigned(0),
                    IntegerValue::Unsigned(7),
                    IntegerValue::Unsigned(7)
                )
            );
            let mut wrong_premise = proof.clone();
            let ProofRule::IntegerAffineBound { root_bound, .. } = &mut wrong_premise.rule else {
                unreachable!()
            };
            root_bound.conclusion = Proposition::LessOrEqual(literal(8), left.clone());
            let wrong_assumption = root_bound.conclusion.clone();
            assert_refused(
                &context,
                &goal,
                from_ref(&wrong_assumption),
                &[],
                &wrong_premise,
            );
            let mut wrong_goal = proof.clone();
            wrong_goal.conclusion = Proposition::IntegerMathLessOrEqual(
                mathematical(&literal(1)),
                sub!(Box::new(mathematical(&left)) => Box::new(mathematical(&right))),
            );
            assert_refused(&context, &goal, &assumptions, &[], &wrong_goal);
        }

        #[test]
        fn correlated_subtract_upper_through_cited_definitions() {
            let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
            let scalar = ScalarType::Integer(integer);
            let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
            let context = value_context(scalar, 4);
            let endpoint = ScalarTerm::integer(integer, integer.maximum_value()).unwrap();
            let addend = ScalarTerm::integer(integer, IntegerValue::Unsigned(7)).unwrap();
            let expression =
                ScalarTerm::exact_integer_add(integer, value(4), addend.clone()).unwrap();
            let premise = Proposition::LessOrEqual(value(1), value(3));
            let difference =
                sub!(Box::new(mathematical(&value(1))) => Box::new(mathematical(&addend)));
            let goal = Proposition::IntegerMathLessOrEqual(difference, mathematical(&endpoint));
            for reversed in [false, true] {
                let equality = |left, right| {
                    if reversed {
                        Proposition::Equal(right, left)
                    } else {
                        Proposition::Equal(left, right)
                    }
                };
                let axioms = [
                    equality(value(4), endpoint.clone()),
                    equality(value(3), expression.clone()),
                ];
                let proof = ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::IntegerAffineBound {
                        root_bound: Box::new(assumption_proof(premise.clone(), 0)),
                        witness: IntegerAffineWitness {
                            root: value(3),
                            target: ScalarTerm::exact_integer_subtract(
                                integer,
                                value(1),
                                addend.clone(),
                            )
                            .unwrap(),
                            definition_axioms: vec![1],
                            literal_axioms: vec![Some(0)],
                        },
                    },
                };
                verify_bounded_certificate(
                    &context,
                    &goal,
                    from_ref(&premise),
                    &axioms,
                    &proof,
                    &mut Budget::default(),
                )
                .unwrap();
                let parameters = BTreeSet::new();
                let assumptions = [premise.clone()];
                let elaboration =
                    elaborate_checked(&context, &goal, &assumptions, &axioms, &parameters, &proof);
                assert!(elaboration.denotation.rule_axioms.is_empty());
                assert_eq!(elaboration.denotation.subtraction.laws.len(), 1);
                assert!(
                    elaboration
                        .denotation
                        .subtraction
                        .laws
                        .contains_key(&super::Law::CancelAddRight)
                );
                assert!(elaboration.denotation.addition.numeral_sums.is_empty());
            }
        }

        #[test]
        fn correlated_subtract_closed_goal_decides_by_numeral_laws() {
            let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
            let literal =
                |number| ScalarTerm::integer(integer, IntegerValue::Unsigned(number)).unwrap();
            let context = PropositionContext::from_value_types([]).unwrap();
            let left = literal(30);
            let right = literal(7);
            let root = ScalarTerm::exact_integer_add(integer, literal(0), right.clone()).unwrap();
            let premise = Proposition::LessOrEqual(root.clone(), left.clone());
            let goal = Proposition::IntegerMathLessOrEqual(
                mathematical(&literal(0)),
                sub!(Box::new(mathematical(&left)) => Box::new(mathematical(&right))),
            );
            let proof = ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::IntegerAffineBound {
                    root_bound: Box::new(assumption_proof(premise.clone(), 0)),
                    witness: IntegerAffineWitness {
                        root,
                        target: ScalarTerm::exact_integer_subtract(integer, left, right).unwrap(),
                        definition_axioms: Vec::new(),
                        literal_axioms: Vec::new(),
                    },
                },
            };
            let assumptions = [premise];
            let parameters = BTreeSet::new();
            let elaboration =
                verify_then_elaborate(&context, &goal, &assumptions, &parameters, &proof);
            assert!(elaboration.denotation.rule_axioms.is_empty());
            assert!(elaboration.denotation.subtraction.laws.is_empty());
        }

        #[test]
        fn subtraction_identity_transport_accepts_existing_leaf_math_citations() {
            let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
            let scalar = ScalarType::Integer(integer);
            let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
            let compound = ScalarTerm::exact_integer_subtract(integer, value(1), value(2)).unwrap();
            let first = Proposition::Equal(compound.clone(), value(1));
            let second = Proposition::Equal(value(1), value(2));
            let math_second =
                Proposition::IntegerMathEqual(mathematical(&value(1)), mathematical(&value(2)));
            let goal = Proposition::Equal(compound, value(2));
            let proof = ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::EqualityTransitivity {
                    left_equals_middle: Box::new(assumption_proof(first.clone(), 0)),
                    middle_equals_right: Box::new(assumption_proof(second, 1)),
                },
            };
            let context = value_context(scalar, 2);
            verify_bounded_certificate(
                &context,
                &goal,
                &[first, math_second],
                &[],
                &proof,
                &mut Budget::default(),
            )
            .unwrap();
        }

        #[test]
        fn open_subtraction_preserves_previously_skipped_numeric_resource_refusals() {
            open_operation_preserves_skipped_numeric_resource_refusals(IntegerMathTerm::Subtract);
        }

        #[test]
        fn direct_subtract_bound_exact_carrier_boundary_cases() {
            let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
            let scalar = ScalarType::Integer(integer);
            let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
            let literal = |magnitude| {
                ScalarTerm::integer(integer, IntegerValue::Unsigned(magnitude)).unwrap()
            };
            let context = value_context(scalar, 2);
            let difference =
                sub!(Box::new(mathematical(&value(1))) => Box::new(mathematical(&value(2))));
            for (lower, exact, bound) in [
                (true, 255, IntegerValue::Unsigned(0)),
                (false, 0, IntegerValue::Unsigned(0)),
            ] {
                let premise = Proposition::Conjunction(vec![
                    Proposition::Equal(value(1), literal(exact)),
                    Proposition::Truth,
                ]);
                let bound = IntegerMathTerm::literal(bound);
                let goal = if lower {
                    Proposition::IntegerMathLessOrEqual(bound, difference.clone())
                } else {
                    Proposition::IntegerMathLessOrEqual(difference.clone(), bound)
                };
                let proof = direct_affine_proof(
                    integer,
                    value(1),
                    value(2),
                    &premise,
                    &goal,
                    ScalarTerm::exact_integer_subtract,
                );
                let denotation = direct_fixture(&context, &goal, &premise, &proof);
                assert!(
                    denotation.rule_axioms.is_empty(),
                    "the boundary case derives through the same antitone law"
                );
                assert_eq!(denotation.subtraction.numeral_differences.len(), 1);
                assert_eq!(
                    denotation.carrier_bounds.keys().collect::<Vec<_>>(),
                    [&(value(2), !lower)]
                );
            }
        }

        #[test]
        fn direct_subtract_bound_out_of_numeral_range_keeps_instance_fallback() {
            let integer = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
            let scalar = ScalarType::Integer(integer);
            let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
            let literal = |magnitude| {
                ScalarTerm::integer(integer, IntegerValue::Unsigned(magnitude)).unwrap()
            };
            let context = value_context(scalar, 2);
            let premise = Proposition::Conjunction(vec![
                Proposition::Equal(value(1), literal(0)),
                Proposition::LessOrEqual(value(2), literal(u128::MAX)),
            ]);
            let goal = Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::IntegerLiteral(IntegerMathLiteral::new(true, u128::MAX).unwrap()),
                sub!(Box::new(mathematical(&value(1))) => Box::new(mathematical(&value(2)))),
            );
            let proof = direct_affine_proof(
                integer,
                value(1),
                value(2),
                &premise,
                &goal,
                ScalarTerm::exact_integer_subtract,
            );
            let denotation = direct_fixture(&context, &goal, &premise, &proof);
            assert_eq!(
                denotation.rule_axioms.len(),
                1,
                "an unrepresentable endpoint difference keeps the explicit instance assumption"
            );
        }

        #[test]
        fn direct_subtract_bound_rejects_mismatched_certificates() {
            let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
            let scalar = ScalarType::Integer(integer);
            let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
            let literal = |magnitude| {
                ScalarTerm::integer(integer, IntegerValue::Unsigned(magnitude)).unwrap()
            };
            let context = value_context(scalar, 3);
            let difference =
                sub!(Box::new(mathematical(&value(1))) => Box::new(mathematical(&value(2))));
            let premise = Proposition::Conjunction(vec![
                Proposition::LessOrEqual(literal(5), value(1)),
                Proposition::LessOrEqual(value(2), literal(3)),
            ]);
            let goal = Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::literal(IntegerValue::Unsigned(2)),
                difference.clone(),
            );
            let proof = direct_affine_proof(
                integer,
                value(1),
                value(2),
                &premise,
                &goal,
                ScalarTerm::exact_integer_subtract,
            );
            let assumptions = [premise];
            for wrong_goal in [
                Proposition::IntegerMathLessOrEqual(
                    IntegerMathTerm::literal(IntegerValue::Unsigned(3)),
                    difference.clone(),
                ),
                Proposition::IntegerMathLessOrEqual(
                    IntegerMathTerm::literal(IntegerValue::Unsigned(2)),
                    sub!(Box::new(mathematical(&value(1))) => Box::new(mathematical(&value(3)))),
                ),
                Proposition::IntegerMathLessOrEqual(
                    difference.clone(),
                    IntegerMathTerm::literal(IntegerValue::Unsigned(2)),
                ),
            ] {
                let mut invalid = proof.clone();
                invalid.conclusion = wrong_goal.clone();
                assert_refused(&context, &wrong_goal, &assumptions, &[], &invalid);
            }
            for wrong_premise in [
                Proposition::LessOrEqual(literal(5), value(1)),
                Proposition::Conjunction(vec![
                    Proposition::LessOrEqual(literal(5), value(1)),
                    Proposition::LessOrEqual(value(1), literal(3)),
                ]),
                Proposition::Conjunction(vec![
                    Proposition::LessOrEqual(literal(5), value(1)),
                    Proposition::LessOrEqual(literal(3), value(2)),
                ]),
            ] {
                let mut invalid = proof.clone();
                let ProofRule::IntegerAffineBound { root_bound, .. } = &mut invalid.rule else {
                    unreachable!()
                };
                root_bound.conclusion = wrong_premise.clone();
                assert_refused(&context, &goal, from_ref(&wrong_premise), &[], &invalid);
            }
            let mut invalid = proof.clone();
            let ProofRule::IntegerAffineBound { witness, .. } = &mut invalid.rule else {
                unreachable!()
            };
            witness.root = value(2);
            assert_refused(&context, &goal, &assumptions, &[], &invalid);
        }

        fn exact_subtract_definition_proof(
            left_premise: &Proposition,
            right_premise: &Proposition,
            goal: &Proposition,
        ) -> ProofNode {
            ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::IntegerExactSubtractDefinitionBound {
                    left_bound: Box::new(assumption_proof(left_premise.clone(), 0)),
                    right_bound: Box::new(assumption_proof(right_premise.clone(), 1)),
                    definition_axiom: 0,
                },
            }
        }

        struct ExactSubtractCase {
            context: PropositionContext,
            left_premise: Proposition,
            right_premise: Proposition,
            definition: Proposition,
            goal: Proposition,
        }

        fn exact_subtract_case(
            integer: IntegerType,
            lower: bool,
            reversed: bool,
            endpoints: (IntegerValue, IntegerValue, IntegerValue),
        ) -> ExactSubtractCase {
            let scalar = ScalarType::Integer(integer);
            let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
            let context = value_context(scalar, 3);
            let (left_endpoint, right_endpoint, bound) = endpoints;
            let literal = |value| ScalarTerm::integer(integer, value).unwrap();
            let output = value(3);
            let expression =
                ScalarTerm::exact_integer_subtract(integer, value(1), value(2)).unwrap();
            let (left_premise, right_premise) = if lower {
                (
                    Proposition::LessOrEqual(literal(left_endpoint), value(1)),
                    Proposition::LessOrEqual(value(2), literal(right_endpoint)),
                )
            } else {
                (
                    Proposition::LessOrEqual(value(1), literal(left_endpoint)),
                    Proposition::LessOrEqual(literal(right_endpoint), value(2)),
                )
            };
            let definition = if reversed {
                Proposition::Equal(expression, output.clone())
            } else {
                Proposition::Equal(output.clone(), expression)
            };
            let goal = if lower {
                Proposition::LessOrEqual(literal(bound), output)
            } else {
                Proposition::LessOrEqual(output, literal(bound))
            };
            ExactSubtractCase {
                context,
                left_premise,
                right_premise,
                definition,
                goal,
            }
        }

        #[test]
        fn exact_subtract_definition_bounds_use_the_two_sided_antitone_law() {
            for (sign, width) in [
                (IntegerSign::Unsigned, 8),
                (IntegerSign::Unsigned, 64),
                (IntegerSign::Signed, 32),
                (IntegerSign::Signed, 128),
            ] {
                let integer = IntegerType::new(sign, width).unwrap();
                for lower in [true, false] {
                    let endpoints = match (sign, lower) {
                        (IntegerSign::Signed, true) => (
                            IntegerValue::Signed(-5),
                            IntegerValue::Signed(3),
                            IntegerValue::Signed(-8),
                        ),
                        (IntegerSign::Signed, false) => (
                            IntegerValue::Signed(7),
                            IntegerValue::Signed(-9),
                            IntegerValue::Signed(16),
                        ),
                        (IntegerSign::Unsigned, true) => (
                            IntegerValue::Unsigned(10),
                            IntegerValue::Unsigned(4),
                            IntegerValue::Unsigned(6),
                        ),
                        (IntegerSign::Unsigned, false) => (
                            IntegerValue::Unsigned(100),
                            IntegerValue::Unsigned(40),
                            IntegerValue::Unsigned(60),
                        ),
                    };
                    for reversed in [false, true] {
                        let case = exact_subtract_case(integer, lower, reversed, endpoints);
                        let proof = exact_subtract_definition_proof(
                            &case.left_premise,
                            &case.right_premise,
                            &case.goal,
                        );
                        let assumptions = [case.left_premise.clone(), case.right_premise.clone()];
                        let axioms = [case.definition.clone()];
                        verify_bounded_certificate(
                            &case.context,
                            &case.goal,
                            &assumptions,
                            &axioms,
                            &proof,
                            &mut Budget::default(),
                        )
                        .unwrap();
                        let parameters = BTreeSet::new();
                        let elaboration = super::super::test_utilities::elaborate_checked(
                            &case.context,
                            &case.goal,
                            &assumptions,
                            &axioms,
                            &parameters,
                            &proof,
                        );
                        assert!(
                            elaboration.denotation.rule_axioms.is_empty(),
                            "exact-subtract definition bounds derive from the two-sided law"
                        );
                        assert_eq!(elaboration.denotation.subtraction.laws.len(), 1);
                        assert_eq!(
                            elaboration
                                .denotation
                                .subtraction
                                .numeral_differences
                                .keys()
                                .collect::<Vec<_>>(),
                            [&endpoints]
                        );
                    }
                }
            }
        }

        #[test]
        fn exact_subtract_definition_bound_rejects_mismatched_certificates() {
            let integer = IntegerType::new(IntegerSign::Signed, 32).unwrap();
            let endpoints = (
                IntegerValue::Signed(-5),
                IntegerValue::Signed(3),
                IntegerValue::Signed(-8),
            );
            let case = exact_subtract_case(integer, true, false, endpoints);
            let scalar = ScalarType::Integer(integer);
            let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
            let literal =
                |value| ScalarTerm::integer(integer, IntegerValue::Signed(value)).unwrap();
            let check = |left: &Proposition,
                         right: &Proposition,
                         definition: &Proposition,
                         goal: &Proposition| {
                let proof = exact_subtract_definition_proof(left, right, goal);
                verify_bounded_certificate(
                    &case.context,
                    goal,
                    &[left.clone(), right.clone()],
                    from_ref(definition),
                    &proof,
                    &mut Budget::default(),
                )
            };
            assert!(
                check(
                    &case.left_premise,
                    &case.right_premise,
                    &case.definition,
                    &case.goal
                )
                .is_ok()
            );
            let same_direction = Proposition::LessOrEqual(literal(3), value(2));
            assert!(
                check(
                    &case.left_premise,
                    &same_direction,
                    &case.definition,
                    &case.goal
                )
                .is_err()
            );
            let widened_goal = Proposition::LessOrEqual(literal(-7), value(3));
            assert!(
                check(
                    &case.left_premise,
                    &case.right_premise,
                    &case.definition,
                    &widened_goal
                )
                .is_err()
            );
            let add_definition = Proposition::Equal(
                value(3),
                ScalarTerm::exact_integer_add(integer, value(1), value(2)).unwrap(),
            );
            assert!(
                check(
                    &case.left_premise,
                    &case.right_premise,
                    &add_definition,
                    &case.goal
                )
                .is_err()
            );
        }
    }

    #[cfg(test)]
    mod order_tests {
        use super::super::test_utilities::{assert_refused, elaborate_checked};
        use crate::value_context;
        use std::collections::BTreeSet;

        use semantic_vocabulary::{
            IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
            ScalarType, ValueId,
        };

        use crate::{
            Budget, ProofNode, ProofRule, verify_bounded_certificate,
            verify_mathematical_certificate,
        };

        fn value(integer: IntegerType, identity: u64) -> ScalarTerm {
            ScalarTerm::value(
                ValueId::new(identity).unwrap(),
                ScalarType::Integer(integer),
            )
        }

        fn zero(integer: IntegerType) -> ScalarTerm {
            ScalarTerm::integer(integer, integer.zero()).unwrap()
        }

        fn context(integer: IntegerType) -> PropositionContext {
            value_context(ScalarType::Integer(integer), 6)
        }

        fn cited(conclusion: Proposition, index: usize) -> Box<ProofNode> {
            Box::new(ProofNode {
                conclusion,
                rule: ProofRule::Assumption { index },
            })
        }

        fn add_order(
            integer: IntegerType,
            original: ScalarTerm,
            increment: ScalarTerm,
        ) -> (Vec<Proposition>, ProofNode) {
            let result = value(integer, 1);
            let sum = Proposition::Equal(
                result.clone(),
                ScalarTerm::exact_integer_add(integer, original.clone(), increment.clone())
                    .unwrap(),
            );
            let positive = Proposition::LessThan(zero(integer), increment);
            let proof = ProofNode {
                conclusion: Proposition::LessThan(original, result),
                rule: ProofRule::IntegerAddOrder {
                    sum: cited(sum.clone(), 0),
                    positive: cited(positive.clone(), 1),
                },
            };
            (vec![sum, positive], proof)
        }

        fn antitone(
            integer: IntegerType,
            minuend: ScalarTerm,
            large: ScalarTerm,
            small: ScalarTerm,
        ) -> (Vec<Proposition>, ProofNode) {
            let (smaller, larger) = (value(integer, 1), value(integer, 2));
            let low = Proposition::Equal(
                smaller.clone(),
                ScalarTerm::exact_integer_subtract(integer, minuend.clone(), large.clone())
                    .unwrap(),
            );
            let high = Proposition::Equal(
                larger.clone(),
                ScalarTerm::exact_integer_subtract(integer, minuend, small.clone()).unwrap(),
            );
            let order = Proposition::LessThan(small, large);
            let proof = ProofNode {
                conclusion: Proposition::LessThan(smaller, larger),
                rule: ProofRule::IntegerSubtractAntitone {
                    smaller: cited(low.clone(), 0),
                    larger: cited(high.clone(), 1),
                    order: cited(order.clone(), 2),
                },
            };
            (vec![low, high, order], proof)
        }

        fn checked(integer: IntegerType, assumptions: &[Proposition], proof: &ProofNode) -> bool {
            let context = context(integer);
            let parameters = BTreeSet::new();
            let elaboration = elaborate_checked(
                &context,
                &proof.conclusion,
                assumptions,
                &[],
                &parameters,
                proof,
            );
            let rule_instance = !elaboration.denotation.rule_axioms.is_empty();
            let mut denoted = verify_bounded_certificate(
                &context,
                &proof.conclusion,
                assumptions,
                &[],
                proof,
                &mut Budget::default(),
            )
            .unwrap();
            let evidence = crate::forge_term_and_assert_rejected(&mut denoted);
            denoted.certificate.term = evidence;
            verify_mathematical_certificate(
                &mut denoted.arena,
                &denoted.certificate,
                &mut Budget::default(),
            )
            .unwrap();
            assert_refused(
                &context,
                &proof.conclusion,
                &assumptions[..assumptions.len() - 1],
                &[],
                proof,
            );
            rule_instance
        }

        fn rejects(integer: IntegerType, assumptions: &[Proposition], proof: &ProofNode) {
            assert_refused(
                &context(integer),
                &proof.conclusion,
                assumptions,
                &[],
                proof,
            );
        }

        #[test]
        fn closed_addition_order_keeps_the_checked_rule_instance() {
            let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
            let literal =
                |value| ScalarTerm::integer(integer, IntegerValue::Unsigned(value)).unwrap();
            let (assumptions, proof) = add_order(integer, literal(5), literal(3));
            assert!(checked(integer, &assumptions, &proof));
        }

        #[test]
        fn addition_order_rejects_reversal_nonstrict_positivity_and_another_increment() {
            let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
            let (assumptions, proof) = add_order(integer, value(integer, 2), value(integer, 3));
            let Proposition::LessThan(original, result) = &proof.conclusion else {
                unreachable!()
            };
            let mut reversed = proof.clone();
            reversed.conclusion = Proposition::LessThan(result.clone(), original.clone());
            rejects(integer, &assumptions, &reversed);

            let mut nonstrict = assumptions.clone();
            nonstrict[1] = Proposition::LessOrEqual(zero(integer), value(integer, 3));
            let mut weakened = proof.clone();
            let ProofRule::IntegerAddOrder { positive, .. } = &mut weakened.rule else {
                unreachable!()
            };
            positive.conclusion = nonstrict[1].clone();
            rejects(integer, &nonstrict, &weakened);

            let mut other = assumptions.clone();
            other[1] = Proposition::LessThan(zero(integer), value(integer, 4));
            let mut redirected = proof.clone();
            let ProofRule::IntegerAddOrder { positive, .. } = &mut redirected.rule else {
                unreachable!()
            };
            positive.conclusion = other[1].clone();
            rejects(integer, &other, &redirected);
        }

        #[test]
        fn closed_subtraction_antitonicity_keeps_the_checked_rule_instance() {
            let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
            let literal =
                |value| ScalarTerm::integer(integer, IntegerValue::Unsigned(value)).unwrap();
            let (assumptions, proof) = antitone(integer, literal(9), literal(4), literal(2));
            assert!(checked(integer, &assumptions, &proof));
        }

        #[test]
        fn subtraction_antitonicity_rejects_another_minuend_and_a_reversed_order() {
            let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
            let (assumptions, proof) = antitone(
                integer,
                value(integer, 3),
                value(integer, 4),
                value(integer, 5),
            );
            let mut foreign = assumptions.clone();
            foreign[1] = Proposition::Equal(
                value(integer, 2),
                ScalarTerm::exact_integer_subtract(integer, value(integer, 6), value(integer, 5))
                    .unwrap(),
            );
            let mut rerooted = proof.clone();
            let ProofRule::IntegerSubtractAntitone { larger, .. } = &mut rerooted.rule else {
                unreachable!()
            };
            larger.conclusion = foreign[1].clone();
            rejects(integer, &foreign, &rerooted);

            let mut backwards = assumptions.clone();
            backwards[2] = Proposition::LessThan(value(integer, 4), value(integer, 5));
            let mut flipped = proof.clone();
            let ProofRule::IntegerSubtractAntitone { order, .. } = &mut flipped.rule else {
                unreachable!()
            };
            order.conclusion = backwards[2].clone();
            rejects(integer, &backwards, &flipped);

            let Proposition::LessThan(smaller, larger) = &proof.conclusion else {
                unreachable!()
            };
            let mut reversed = proof.clone();
            reversed.conclusion = Proposition::LessThan(larger.clone(), smaller.clone());
            rejects(integer, &assumptions, &reversed);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::super::test_utilities::{assert_refused, elaborate_checked};
        use std::collections::BTreeSet;

        use semantic_vocabulary::{
            IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
            ScalarType, ValueId,
        };

        use crate::proof::assumption_proof;
        use crate::{
            Budget, ProofNode, ProofRule, proof_node, value_context, verify_bounded_certificate,
            verify_mathematical_certificate,
        };

        fn fixture(
            integer: IntegerType,
            left: ScalarTerm,
            right: ScalarTerm,
        ) -> (PropositionContext, Vec<Proposition>, ProofNode) {
            let scalar = ScalarType::Integer(integer);
            let result = ScalarTerm::value(ValueId::new(1).unwrap(), scalar);
            let difference = Proposition::Equal(
                result.clone(),
                ScalarTerm::exact_integer_subtract(integer, left.clone(), right.clone()).unwrap(),
            );
            let zero = integer.zero();
            let positive =
                Proposition::LessThan(ScalarTerm::integer(integer, zero).unwrap(), right);
            let assumptions = vec![difference.clone(), positive.clone()];
            let proof = ProofNode {
                conclusion: Proposition::LessThan(result, left),
                rule: ProofRule::IntegerSubtractOrder {
                    difference: Box::new(assumption_proof(difference, 0)),
                    positive: Box::new(assumption_proof(positive, 1)),
                },
            };
            let context = value_context(scalar, 3);
            (context, assumptions, proof)
        }

        fn checked(integer: IntegerType, left: ScalarTerm, right: ScalarTerm) {
            let (context, assumptions, proof) = fixture(integer, left, right);
            let parameters = BTreeSet::new();
            let elaboration = elaborate_checked(
                &context,
                &proof.conclusion,
                &assumptions,
                &[],
                &parameters,
                &proof,
            );
            assert!(elaboration.denotation.rule_axioms.is_empty());
            assert!(elaboration.denotation.decisions.is_empty());
            let mut budget = Budget::default();
            let mut denoted = verify_bounded_certificate(
                &context,
                &proof.conclusion,
                &assumptions,
                &[],
                &proof,
                &mut budget,
            )
            .unwrap();
            eprintln!(
                "conditional subtraction: {:?}, remaining={}",
                denoted.receipt(),
                budget.remaining()
            );
            assert!(denoted.certificate.signature.len() < 800);
            let evidence = crate::forge_term_and_assert_rejected(&mut denoted);
            denoted.certificate.term = evidence;
            verify_mathematical_certificate(
                &mut denoted.arena,
                &denoted.certificate,
                &mut Budget::default(),
            )
            .unwrap();
            assert_refused(&context, &proof.conclusion, &assumptions[..1], &[], &proof);
        }

        #[test]
        fn conditional_subtraction_preserves_open_closed_and_false_positive_premises() {
            let integer = IntegerType::new(IntegerSign::Signed, 8).unwrap();
            let literal =
                |value| ScalarTerm::integer(integer, IntegerValue::Signed(value)).unwrap();
            let value = |identity| {
                ScalarTerm::value(
                    ValueId::new(identity).unwrap(),
                    ScalarType::Integer(integer),
                )
            };
            checked(integer, value(2), value(3));
            for (left, right) in [
                (5, 1),
                (5, 7),
                (7, 4),
                (8, 6),
                (8, 5),
                (5, 0),
                (5, -2),
                (-128, 1),
                (127, -1),
                (-5, 7),
                (0, 127),
                (-128, 0),
            ] {
                checked(integer, literal(left), literal(right));
            }
            checked(
                integer,
                ScalarTerm::exact_integer_add(integer, value(2), literal(1)).unwrap(),
                value(3),
            );
        }

        #[test]
        fn maximal_width_and_nested_overflow_keep_the_original_conditional_domain() {
            let integer = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
            let literal =
                |value| ScalarTerm::integer(integer, IntegerValue::Unsigned(value)).unwrap();
            checked(integer, literal(u128::MAX), literal(u128::MAX));
            checked(integer, literal(0), literal(1));
            let negative_one =
                ScalarTerm::exact_integer_subtract(integer, literal(0), literal(1)).unwrap();
            checked(integer, negative_one, literal(u128::MAX));
            let signed = IntegerType::new(IntegerSign::Signed, 128).unwrap();
            let literal = |value| ScalarTerm::integer(signed, IntegerValue::Signed(value)).unwrap();
            checked(signed, literal(i128::MAX), literal(i128::MAX));
            checked(signed, literal(i128::MIN), literal(0));
            checked(signed, literal(0), literal(i128::MIN));
        }

        #[test]
        fn mismatched_decrement_and_reversed_goal_still_reject() {
            let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
            let literal =
                |value| ScalarTerm::integer(integer, IntegerValue::Unsigned(value)).unwrap();
            let (context, assumptions, mut proof) = fixture(integer, literal(5), literal(1));
            let Proposition::LessThan(left, right) = &proof.conclusion else {
                unreachable!()
            };
            let reversed = Proposition::LessThan(right.clone(), left.clone());
            assert_refused(
                &context,
                &reversed,
                &assumptions,
                &[],
                &proof_node!(reversed.clone() => proof.rule.clone()),
            );
            let ProofRule::IntegerSubtractOrder { positive, .. } = &mut proof.rule else {
                unreachable!()
            };
            positive.conclusion = Proposition::LessThan(literal(0), literal(2));
            let changed = [assumptions[0].clone(), positive.conclusion.clone()];
            assert_refused(&context, &proof.conclusion, &changed, &[], &proof);
        }
    }
}
pub use source_integer::{SourceIntegerTheory, SourceIntegerVocabulary};

pub struct BoundedDenotation {
    pub arena: TermArena,
    pub certificate: MathematicalCertificate,
    pub rules: Vec<AcceptedProofRule>,
    pub assumptions: Vec<AcceptedPremise>,
    pub semantic_axioms: Vec<AcceptedPremise>,
}

impl BoundedDenotation {
    pub fn receipt(&self) -> MathematicalJudgmentReceipt {
        let saturating = |count: usize| u32::try_from(count).unwrap_or(u32::MAX);
        MathematicalJudgmentReceipt {
            declarations: saturating(self.certificate.signature.len()),
            assumption_closure: saturating(
                certificate_assumption_closure(&self.arena, &self.certificate).len(),
            ),
            context_depth: saturating(self.certificate.context.len()),
            arena_slots: saturating(self.arena.len()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundedDenotationError {
    Certificate(ProofError),
    Unsupported(&'static str),
    Kernel(CoreError),
    DepthLimitExceeded,
}

crate::debug_display!(BoundedDenotationError);

impl std::error::Error for BoundedDenotationError {}

fn propositions_match(retained: &Proposition, requested: &Proposition) -> bool {
    propositions_match_under_integer_math_normalization(retained, requested)
}

fn affine_equality_for<'a>(
    definitions: &'a [(Proposition, TermHandle)],
    subject: &ScalarTerm,
) -> Option<(&'a ScalarTerm, TermHandle, &'a Proposition)> {
    definitions.iter().find_map(|(proposition, proof)| {
        let (first, second) = proposition.as_equal()?;
        if first == subject {
            Some((second, *proof, proposition))
        } else if second == subject {
            Some((first, *proof, proposition))
        } else {
            None
        }
    })
}

fn affine_witness_root_expression<'a>(
    witness: &'a crate::IntegerAffineWitness,
    definitions: &'a [(Proposition, TermHandle)],
) -> Option<(&'a ScalarTerm, Option<(TermHandle, &'a Proposition)>)> {
    if witness.definition_axioms.is_empty() && witness.literal_axioms.is_empty() {
        Some((&witness.root, None))
    } else if witness.definition_axioms.len() == 1 {
        let (expression, proof, proposition) = affine_equality_for(definitions, &witness.root)?;
        Some((expression, Some((proof, proposition))))
    } else {
        None
    }
}

fn affine_root_premise(
    witness: &crate::IntegerAffineWitness,
    lower: bool,
    left: &ScalarTerm,
) -> Proposition {
    if lower {
        Proposition::LessOrEqual(witness.root.clone(), left.clone())
    } else {
        Proposition::LessOrEqual(left.clone(), witness.root.clone())
    }
}

fn affine_carrier_endpoint_equality<'a>(
    scalar_type: IntegerType,
    lower: bool,
    endpoint: &'a ScalarTerm,
    definitions: &'a [(Proposition, TermHandle)],
) -> Option<(Option<(TermHandle, &'a Proposition)>, ScalarTerm)> {
    let endpoint_value = if lower {
        scalar_type.minimum_value()
    } else {
        scalar_type.maximum_value()
    };
    let endpoint_literal = ScalarTerm::integer(scalar_type, endpoint_value)
        .expect("carrier endpoint is representable");
    if endpoint.integer_value() == Some((scalar_type, endpoint_value)) {
        return Some((None, endpoint_literal));
    }
    let (literal, proof, proposition) = affine_equality_for(definitions, endpoint)?;
    if literal != &endpoint_literal {
        return None;
    }
    Some((Some((proof, proposition)), endpoint_literal))
}

fn math_literal_value(literal: IntegerMathLiteral) -> Option<IntegerValue> {
    if literal.negative() {
        if literal.magnitude() == (i128::MAX as u128) + 1 {
            Some(IntegerValue::Signed(i128::MIN))
        } else {
            i128::try_from(literal.magnitude())
                .ok()
                .map(|magnitude| IntegerValue::Signed(-magnitude))
        }
    } else {
        Some(IntegerValue::Unsigned(literal.magnitude()))
    }
}

fn unbounded_integer(value: IntegerValue) -> BigInt {
    match value {
        IntegerValue::Signed(value) => BigInt::from_i128(value),
        IntegerValue::Unsigned(value) => BigInt::from_u128(value),
    }
}

pub fn denote_bounded_certificate(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    proof: &ProofNode,
) -> Result<BoundedDenotation, BoundedDenotationError> {
    denote_bounded_certificate_with_machine_parameters(
        context,
        goal,
        assumptions,
        semantic_axioms,
        &BTreeSet::new(),
        proof,
    )
}

fn denote_bounded_certificate_with_machine_parameters(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
    proof: &ProofNode,
) -> Result<BoundedDenotation, BoundedDenotationError> {
    #[cfg(test)]
    crate::mathematical_core::bounded_observations::record_denotation();
    context.validate(goal).map_err(|error| {
        BoundedDenotationError::Certificate(ProofError::MalformedProposition(error))
    })?;
    for proposition in assumptions.iter().chain(semantic_axioms) {
        context.validate(proposition).map_err(|error| {
            BoundedDenotationError::Certificate(ProofError::MalformedProposition(error))
        })?;
    }
    let mut elaboration = Elaboration::new(
        context,
        goal,
        assumptions,
        semantic_axioms,
        machine_parameter_values,
    )?;
    let term = elaboration.node(proof)?;
    if proof.conclusion != *goal {
        return Err(BoundedDenotationError::Certificate(
            ProofError::CertificateConclusionMismatch,
        ));
    }
    Ok(elaboration.finish(term))
}

pub fn verify_bounded_certificate(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    proof: &ProofNode,
    budget: &mut Budget,
) -> Result<BoundedDenotation, BoundedDenotationError> {
    verify_bounded_certificate_with_machine_parameters(
        context,
        goal,
        assumptions,
        semantic_axioms,
        &BTreeSet::new(),
        proof,
        budget,
    )
}

pub(crate) fn verify_bounded_certificate_with_machine_parameters(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
    proof: &ProofNode,
    budget: &mut Budget,
) -> Result<BoundedDenotation, BoundedDenotationError> {
    let mut denoted = denote_bounded_certificate_with_machine_parameters(
        context,
        goal,
        assumptions,
        semantic_axioms,
        machine_parameter_values,
        proof,
    )?;
    verify_mathematical_certificate(&mut denoted.arena, &denoted.certificate, budget)
        .map_err(BoundedDenotationError::Kernel)?;
    Ok(denoted)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum IntegerLaw {
    EqualityToLessOrEqual,
    LessThanToLessOrEqual,
    LessOrEqualTransitivity,
    LessThanTransitivity,
    LessThanLessOrEqualTransitivity,
    LessOrEqualLessThanTransitivity,
    LessThanSubstituteLeft,
    LessThanSubstituteRight,
    LessOrEqualSubstituteLeft,
    LessOrEqualSubstituteRight,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum MathTermKey {
    Closed(BigInt),
    Open(IntegerMathTerm),
}

fn integer_value_of(value: &BigInt) -> Option<IntegerValue> {
    let (negative, magnitude) = binary_numerals::fixed_magnitude(value)?;
    Some(if negative && magnitude != 0 {
        IntegerValue::Signed(if magnitude == 1_u128 << 127 {
            i128::MIN
        } else {
            i128::try_from(magnitude).ok()?.checked_neg()?
        })
    } else {
        IntegerValue::Unsigned(magnitude)
    })
}

#[derive(Debug, Clone, Copy)]
enum IntegerRelation {
    Equality { left: TermHandle, right: TermHandle },
    LessThan { left: TermHandle, right: TermHandle },
    LessOrEqual { left: TermHandle, right: TermHandle },
}

fn integer_law_statement(
    law: IntegerLaw,
    integer: u32,
    less_than: u32,
    less_or_equal: u32,
) -> Syntax {
    use scheme_dsl::{apps, id, pi, scheme_at, v};
    let carrier = || scheme_at(integer, Vec::new());
    let lt = |left: Syntax, right: Syntax| apps(scheme_at(less_than, Vec::new()), [left, right]);
    let le =
        |left: Syntax, right: Syntax| apps(scheme_at(less_or_equal, Vec::new()), [left, right]);
    let eq = |left: Syntax, right: Syntax| id(carrier(), left, right);
    let implication = |premise: Syntax, conclusion: Syntax| {
        pi(
            "x",
            carrier(),
            pi("y", carrier(), pi("_", premise, conclusion)),
        )
    };
    let transitivity = |first: Syntax, second: Syntax, result: Syntax| {
        pi(
            "x",
            carrier(),
            pi(
                "y",
                carrier(),
                pi("z", carrier(), pi("_", first, pi("_", second, result))),
            ),
        )
    };
    match law {
        IntegerLaw::EqualityToLessOrEqual => implication(eq(v("x"), v("y")), le(v("x"), v("y"))),
        IntegerLaw::LessThanToLessOrEqual => implication(lt(v("x"), v("y")), le(v("x"), v("y"))),
        IntegerLaw::LessOrEqualTransitivity => {
            transitivity(le(v("x"), v("y")), le(v("y"), v("z")), le(v("x"), v("z")))
        }
        IntegerLaw::LessThanTransitivity => {
            transitivity(lt(v("x"), v("y")), lt(v("y"), v("z")), lt(v("x"), v("z")))
        }
        IntegerLaw::LessThanLessOrEqualTransitivity => {
            transitivity(lt(v("x"), v("y")), le(v("y"), v("z")), lt(v("x"), v("z")))
        }
        IntegerLaw::LessOrEqualLessThanTransitivity => {
            transitivity(le(v("x"), v("y")), lt(v("y"), v("z")), lt(v("x"), v("z")))
        }
        IntegerLaw::LessThanSubstituteLeft => {
            transitivity(eq(v("x"), v("y")), lt(v("x"), v("z")), lt(v("y"), v("z")))
        }
        IntegerLaw::LessThanSubstituteRight => {
            transitivity(eq(v("y"), v("z")), lt(v("x"), v("y")), lt(v("x"), v("z")))
        }
        IntegerLaw::LessOrEqualSubstituteLeft => {
            transitivity(eq(v("x"), v("y")), le(v("x"), v("z")), le(v("y"), v("z")))
        }
        IntegerLaw::LessOrEqualSubstituteRight => {
            transitivity(eq(v("y"), v("z")), le(v("x"), v("y")), le(v("x"), v("z")))
        }
    }
}

#[cfg(test)]
mod order_antisymmetry_tests {
    use super::test_utilities::{elaborate_checked, verify_and_assert_forged};
    use std::collections::BTreeSet;

    use crate::proof::assumption_proof;
    use crate::{
        AcceptedProofRule, ProofNode, ProofRule, accept_certificate, check_certificate,
        value_context,
    };
    use semantic_vocabulary::{
        IntegerSign, IntegerType, Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId,
    };

    fn value(carrier: IntegerType, identity: u64) -> ScalarTerm {
        ScalarTerm::value(
            ValueId::new(identity).unwrap(),
            ScalarType::Integer(carrier),
        )
    }
    fn context(carrier: IntegerType) -> PropositionContext {
        value_context(ScalarType::Integer(carrier), 3)
    }
    fn proof(left: ScalarTerm, right: ScalarTerm) -> (Vec<Proposition>, ProofNode) {
        let forward = Proposition::LessOrEqual(left.clone(), right.clone());
        let backward = Proposition::LessOrEqual(right.clone(), left.clone());
        let result = ProofNode {
            conclusion: Proposition::Equal(left, right),
            rule: ProofRule::IntegerOrderAntisymmetry {
                forward: Box::new(assumption_proof(forward.clone(), 0)),
                backward: Box::new(assumption_proof(backward.clone(), 1)),
            },
        };
        (vec![forward, backward], result)
    }

    #[test]
    fn integer_order_antisymmetry_certifies_both_orders_through_existing_core_law() {
        for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
            let carrier = IntegerType::new(sign, 64).unwrap();
            let context = context(carrier);
            let (premises, proof) = proof(value(carrier, 1), value(carrier, 2));
            let accepted =
                accept_certificate(&context, &proof.conclusion, &premises, &[], &proof).unwrap();
            assert!(
                accepted
                    .rules
                    .contains(&AcceptedProofRule::IntegerOrderAntisymmetry)
            );
            let parameters = BTreeSet::new();
            let elaboration = elaborate_checked(
                &context,
                &proof.conclusion,
                &premises,
                &[],
                &parameters,
                &proof,
            );
            assert!(elaboration.denotation.rule_axioms.is_empty());
            verify_and_assert_forged(&context, &proof.conclusion, &premises, &[], &proof);
            assert!(
                check_certificate(&context, &proof.conclusion, &premises[..1], &[], &proof)
                    .is_err()
            );
        }
    }

    #[test]
    fn integer_order_antisymmetry_rejects_strictness_endpoint_and_scope_drift() {
        let carrier = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let context = context(carrier);
        let (original, proof) = proof(value(carrier, 1), value(carrier, 2));
        for replacement in [
            Proposition::LessOrEqual(value(carrier, 1), value(carrier, 2)),
            Proposition::LessOrEqual(value(carrier, 2), value(carrier, 3)),
            Proposition::LessThan(value(carrier, 2), value(carrier, 1)),
            Proposition::LessOrEqual(value(carrier, 2), value(carrier, 99)),
        ] {
            let mut changed = proof.clone();
            let ProofRule::IntegerOrderAntisymmetry { backward, .. } = &mut changed.rule else {
                unreachable!()
            };
            backward.conclusion = replacement.clone();
            let premises = [original[0].clone(), replacement];
            assert!(
                check_certificate(&context, &changed.conclusion, &premises, &[], &changed).is_err()
            );
        }
        let mut changed = proof.clone();
        changed.conclusion = Proposition::Equal(value(carrier, 1), value(carrier, 3));
        assert!(
            check_certificate(&context, &changed.conclusion, &original, &[], &changed).is_err()
        );
        let mut changed = proof;
        let ProofRule::IntegerOrderAntisymmetry { forward, backward } = &mut changed.rule else {
            unreachable!()
        };
        std::mem::swap(forward, backward);
        assert!(
            check_certificate(&context, &changed.conclusion, &original, &[], &changed).is_err()
        );
    }

    #[test]
    fn integer_order_antisymmetry_does_not_relabel_carriers_or_arithmetic_policy() {
        let carrier = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        for other in [
            IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
            IntegerType::new(IntegerSign::Signed, 64).unwrap(),
            IntegerType::address(64).unwrap(),
        ] {
            let context = PropositionContext::from_value_types([
                (ValueId::new(1).unwrap(), ScalarType::Integer(carrier)),
                (ValueId::new(2).unwrap(), ScalarType::Integer(other)),
            ])
            .unwrap();
            let (premises, proof) = proof(value(carrier, 1), value(other, 2));
            assert!(
                check_certificate(&context, &proof.conclusion, &premises, &[], &proof).is_err()
            );
        }
        let wrapped = ScalarTerm::WrappingIntegerAdd {
            scalar_type: carrier,
            left: Box::new(value(carrier, 1)),
            right: Box::new(value(carrier, 2)),
        };
        let (premises, mut proof) = proof(wrapped.clone(), value(carrier, 3));
        check_certificate(&context(carrier), &proof.conclusion, &premises, &[], &proof).unwrap();
        let ScalarTerm::WrappingIntegerAdd {
            scalar_type,
            left,
            right,
        } = wrapped
        else {
            unreachable!()
        };
        proof.conclusion = Proposition::Equal(
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            },
            value(carrier, 3),
        );
        assert!(
            check_certificate(&context(carrier), &proof.conclusion, &premises, &[], &proof)
                .is_err()
        );
    }

    #[test]
    fn integer_order_antisymmetry_orients_literal_and_reversed_value_equalities() {
        use semantic_vocabulary::IntegerValue;
        let carrier = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let context = context(carrier);
        let zero = ScalarTerm::integer(carrier, IntegerValue::Unsigned(0)).unwrap();
        for (left, right) in [
            (value(carrier, 1), zero),
            (value(carrier, 2), value(carrier, 1)),
        ] {
            let (premises, proof) = proof(left, right);
            check_certificate(&context, &proof.conclusion, &premises, &[], &proof).unwrap();
            let parameters = BTreeSet::new();
            let elaboration = elaborate_checked(
                &context,
                &proof.conclusion,
                &premises,
                &[],
                &parameters,
                &proof,
            );
            assert!(elaboration.denotation.rule_axioms.is_empty());
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::proof::{assumption_proof, semantic_axiom_proof};
    use crate::proof_node;
    use std::collections::BTreeSet;

    use semantic_vocabulary::{
        ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentDomainId,
        ContentPlaceSegment, ContentPlaceVersion, ContentProjectionIdentity,
        ContentStructuralPlace, ContentTerm, IntegerSign, IntegerType, IntegerValue, PlaceId,
        PropositionId, ScalarType, StructuralPlaceKind, ValueId,
    };
    use terminal_psi::PrimitiveJudgment;

    use super::test_utilities::denote_via_assumption_premise;
    use super::{
        AcceptedPremise, AcceptedProofRule, BoundedDenotation, BoundedDenotationError, Elaboration,
        ProofError, ProofNode, ProofRule, Proposition, PropositionContext, ScalarTerm, Term,
        denote_bounded_certificate, verify_bounded_certificate,
    };
    use crate::mathematical_core::{
        Budget, DEFAULT_CONVERSION_STEPS, certificate_assumption_closure,
        verify_mathematical_certificate,
    };
    use std::slice::from_ref;

    macro_rules! v {
        ($a:expr => $b:expr => $c:expr => $d:expr => $e:expr => $f:expr) => {
            verify_bounded_certificate($a, $b, $c, $d, $e, $f)
        };
    }

    fn budget() -> Budget {
        Budget::new(DEFAULT_CONVERSION_STEPS)
    }

    fn unsigned64() -> IntegerType {
        IntegerType::new(IntegerSign::Unsigned, 64).expect("u64")
    }

    fn unsigned64_type() -> ScalarType {
        ScalarType::Integer(unsigned64())
    }

    fn value(id: u64) -> (ValueId, ScalarTerm) {
        let id = ValueId::new(id).expect("value id");
        (id, ScalarTerm::value(id, unsigned64_type()))
    }

    fn boolean_value(id: u64) -> (ValueId, ScalarTerm) {
        let id = ValueId::new(id).expect("value id");
        (id, ScalarTerm::value(id, ScalarType::Boolean))
    }

    fn literal(value: u128) -> ScalarTerm {
        ScalarTerm::integer(unsigned64(), IntegerValue::Unsigned(value)).expect("u64 literal")
    }

    fn atom(index: u64) -> Proposition {
        Proposition::Atom(PropositionId::new(index).expect("atom id"))
    }

    #[test]
    fn contract_entailment_discharge_shape_is_a_kernel_judgment() {
        let (x_id, x) = value(1);
        let (y_id, y) = value(2);
        let bound = Proposition::LessOrEqual(x, y);
        let context = PropositionContext::from_value_types([
            (x_id, unsigned64_type()),
            (y_id, unsigned64_type()),
        ])
        .expect("context");
        let proof = assumption_proof(bound.clone(), 0);
        let denoted = verify_bounded_certificate(
            &context,
            &bound,
            from_ref(&bound),
            &[],
            &proof,
            &mut budget(),
        )
        .expect("the discharge judgment verifies in the kernel");

        assert_eq!(denoted.certificate.signature.len(), 4);
        assert!(denoted.certificate.signature[0].is_assumption());
        assert_eq!(denoted.certificate.context.len(), 1);
        assert_eq!(
            denoted.arena.get(denoted.certificate.term),
            Term::Variable(0)
        );
        assert_eq!(
            certificate_assumption_closure(&denoted.arena, &denoted.certificate),
            BTreeSet::from([0, 1, 2, 3]),
            "the judgment commits to the carrier, the relation and both endpoints",
        );
        assert_eq!(
            denoted.assumptions,
            vec![AcceptedPremise {
                index: 0,
                proposition: bound,
            }],
        );
        assert_eq!(denoted.rules, vec![AcceptedProofRule::Assumption],);
    }

    #[test]
    fn discharged_hypotheses_stay_inside_the_evidence_term() {
        let p = atom(1);
        let q = atom(2);
        let conjunction = Proposition::Conjunction(vec![p.clone(), q.clone()]);
        let swapped = Proposition::Conjunction(vec![q.clone(), p.clone()]);
        let goal = Proposition::Implication {
            premise: Box::new(conjunction.clone()),
            conclusion: Box::new(swapped.clone()),
        };
        let hypothesis = |conclusion: Proposition| ProofNode {
            conclusion,
            rule: ProofRule::Assumption { index: 0 },
        };
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::ImplicationIntroduction {
                body: Box::new(ProofNode {
                    conclusion: swapped.clone(),
                    rule: ProofRule::ConjunctionIntroduction(vec![
                        ProofNode {
                            conclusion: q,
                            rule: ProofRule::ConjunctionElimination {
                                conjunction: Box::new(hypothesis(conjunction.clone())),
                                conjunct: 1,
                            },
                        },
                        ProofNode {
                            conclusion: p,
                            rule: ProofRule::ConjunctionElimination {
                                conjunction: Box::new(hypothesis(conjunction)),
                                conjunct: 0,
                            },
                        },
                    ]),
                }),
            },
        };
        let denoted = verify_bounded_certificate(
            &PropositionContext::default(),
            &goal,
            &[],
            &[],
            &proof,
            &mut budget(),
        )
        .expect("the swap proof verifies");
        assert_eq!(denoted.certificate.signature.len(), 2);
        assert!(
            denoted.certificate.context.is_empty(),
            "the pushed hypothesis is bound inside the term, not the context",
        );
        assert!(
            denoted.assumptions.is_empty(),
            "a discharged hypothesis is not an ambient premise",
        );
        assert_eq!(
            certificate_assumption_closure(&denoted.arena, &denoted.certificate),
            BTreeSet::from([0, 1]),
            "the judgment names both atoms the goal commits to",
        );
    }

    #[test]
    fn equality_rules_become_identity_eliminations() {
        let (x_id, x) = value(1);
        let (y_id, y) = value(2);
        let (z_id, z) = value(3);
        let context = PropositionContext::from_value_types([
            (x_id, unsigned64_type()),
            (y_id, unsigned64_type()),
            (z_id, unsigned64_type()),
        ])
        .expect("context");
        let first = Proposition::Equal(x.clone(), y.clone());
        let second = Proposition::Equal(y.clone(), z.clone());
        let goal = Proposition::Equal(x, z);
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::EqualityTransitivity {
                left_equals_middle: Box::new(assumption_proof(first.clone(), 0)),
                middle_equals_right: Box::new(assumption_proof(second.clone(), 1)),
            },
        };
        let denoted = v!(&context => &goal => &[first, second] => &[] => &proof => &mut budget())
            .expect("transitivity verifies as J");
        assert_eq!(denoted.certificate.signature.len(), 4);
        assert_eq!(denoted.certificate.context.len(), 2);
        assert_eq!(
            certificate_assumption_closure(&denoted.arena, &denoted.certificate),
            BTreeSet::from([0, 1, 2, 3]),
        );

        let flipped = Proposition::Equal(
            ScalarTerm::value(y_id, unsigned64_type()),
            ScalarTerm::value(x_id, unsigned64_type()),
        );
        let source = Proposition::Equal(
            ScalarTerm::value(x_id, unsigned64_type()),
            ScalarTerm::value(y_id, unsigned64_type()),
        );
        let proof = ProofNode {
            conclusion: flipped.clone(),
            rule: ProofRule::EqualitySymmetry {
                equality: Box::new(assumption_proof(source.clone(), 0)),
            },
        };
        let denoted =
            verify_bounded_certificate(&context, &flipped, &[source], &[], &proof, &mut budget())
                .expect("symmetry verifies");
        assert_eq!(denoted.certificate.signature.len(), 3);
        assert_eq!(
            denoted.arena.get(denoted.certificate.term),
            Term::Variable(0),
        );
        assert_eq!(
            certificate_assumption_closure(&denoted.arena, &denoted.certificate),
            BTreeSet::from([0, 1, 2]),
        );
    }

    #[test]
    fn order_rules_become_named_law_citations() {
        let (x_id, x) = value(1);
        let (y_id, y) = value(2);
        let context = PropositionContext::from_value_types([
            (x_id, unsigned64_type()),
            (y_id, unsigned64_type()),
        ])
        .expect("context");
        let strict = Proposition::LessThan(x.clone(), y.clone());
        let weakened = Proposition::LessOrEqual(x.clone(), y.clone());
        let proof = ProofNode {
            conclusion: weakened.clone(),
            rule: ProofRule::IntegerOrderWeakening {
                relation: Box::new(assumption_proof(strict.clone(), 0)),
            },
        };
        let denoted = verify_bounded_certificate(
            &context,
            &weakened,
            from_ref(&strict),
            &[],
            &proof,
            &mut budget(),
        )
        .expect("order weakening verifies as a roster citation");
        assert_eq!(denoted.certificate.signature.len(), 6);
        let mut head = denoted.certificate.term;
        for _ in 0..3 {
            let Term::Apply { function, .. } = denoted.arena.get(head) else {
                panic!("a roster citation applies endpoints then evidence");
            };
            head = function;
        }
        assert!(matches!(denoted.arena.get(head), Term::Constant { .. }));
        let Term::Apply { argument, .. } = denoted.arena.get(denoted.certificate.term) else {
            unreachable!("checked above");
        };
        assert_eq!(denoted.arena.get(argument), Term::Variable(0));
        assert_eq!(
            certificate_assumption_closure(&denoted.arena, &denoted.certificate),
            BTreeSet::from([0, 1, 2, 3, 4, 5]),
            "the judgment names the vocabulary, both endpoints and the law",
        );
        assert_eq!(
            denoted.rules,
            vec![
                AcceptedProofRule::Assumption,
                AcceptedProofRule::IntegerOrderWeakening
            ],
        );

        let proof = ProofNode {
            conclusion: weakened.clone(),
            rule: ProofRule::IntegerOrderWeakening {
                relation: Box::new(assumption_proof(weakened.clone(), 0)),
            },
        };
        assert!(matches!(
            denote_bounded_certificate(&context, &weakened, from_ref(&weakened), &[], &proof),
            Err(BoundedDenotationError::Certificate(
                ProofError::RulePremiseMismatch(_)
            )),
        ));
    }

    #[test]
    fn integer_math_transitivity_becomes_identity_eliminations() {
        let math = |value: u128| {
            semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Unsigned(value))
        };
        let first = Proposition::IntegerMathEqual(math(1), math(2));
        let second = Proposition::IntegerMathEqual(math(2), math(3));
        let goal = Proposition::IntegerMathEqual(math(1), math(3));
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::EqualityTransitivity {
                left_equals_middle: Box::new(assumption_proof(first.clone(), 0)),
                middle_equals_right: Box::new(assumption_proof(second.clone(), 1)),
            },
        };
        let denoted = verify_bounded_certificate(
            &PropositionContext::default(),
            &goal,
            &[first, second],
            &[],
            &proof,
            &mut budget(),
        )
        .expect("mathematical-integer transitivity verifies as J");
        assert!(matches!(
            denoted.arena.get(denoted.certificate.term),
            Term::IdElim { .. },
        ));
        assert_eq!(
            denoted.rules,
            vec![
                AcceptedProofRule::Assumption,
                AcceptedProofRule::EqualityTransitivity
            ],
        );

        let (x_id, x) = value(1);
        let (y_id, y) = value(2);
        let (z_id, z) = value(3);
        let context = PropositionContext::from_value_types([
            (x_id, unsigned64_type()),
            (y_id, unsigned64_type()),
            (z_id, unsigned64_type()),
        ])
        .expect("context");
        let first = Proposition::Equal(x.clone(), y.clone());
        let second = Proposition::Equal(y.clone(), z.clone());
        let goal = crate::lift_fixed_integer_relation(&Proposition::Equal(x, z)).expect("lifts");
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::EqualityTransitivity {
                left_equals_middle: Box::new(assumption_proof(first.clone(), 0)),
                middle_equals_right: Box::new(assumption_proof(second.clone(), 1)),
            },
        };
        let denoted = v!(&context => &goal => &[first, second] => &[] => &proof => &mut budget())
            .expect("the normalized conclusion verifies as J");
        assert!(matches!(
            denoted.arena.get(denoted.certificate.term),
            Term::IdElim { .. },
        ));
    }

    #[test]
    fn conversion_and_value_transport_use_existing_evidence_and_identity_elimination() {
        let (x_id, x) = value(1);
        let (y_id, y) = value(2);
        let (z_id, z) = value(3);
        let context = PropositionContext::from_value_types([
            (x_id, unsigned64_type()),
            (y_id, unsigned64_type()),
            (z_id, unsigned64_type()),
        ])
        .expect("context");

        let premise = Proposition::Equal(y.clone(), x.clone());
        let goal = Proposition::Equal(x, y);
        let denoted = denote_via_assumption_premise(
            &context,
            &goal,
            &premise,
            "predicate denotation verifies definitionally",
        );
        assert_eq!(
            denoted.arena.get(denoted.certificate.term),
            Term::Variable(0),
        );

        let premise = Proposition::LessOrEqual(
            ScalarTerm::value(x_id, unsigned64_type()),
            ScalarTerm::value(y_id, unsigned64_type()),
        );
        let equation = Proposition::Equal(ScalarTerm::value(x_id, unsigned64_type()), z.clone());
        let goal = Proposition::LessOrEqual(z, ScalarTerm::value(y_id, unsigned64_type()));
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::ValueEqualityTransport {
                premise: Box::new(assumption_proof(premise.clone(), 0)),
                equalities: vec![assumption_proof(equation.clone(), 1)],
            },
        };
        let denoted =
            v!(&context => &goal => &[premise, equation] => &[] => &proof => &mut budget())
                .expect("value equality transport verifies through J");
        assert!(matches!(
            denoted.arena.get(denoted.certificate.term),
            Term::IdElim { .. }
        ));
        assert_eq!(denoted.certificate.signature.len(), 5);
        assert_eq!(
            denoted.rules,
            vec![
                AcceptedProofRule::Assumption,
                AcceptedProofRule::ValueEqualityTransport
            ],
        );
    }

    #[test]
    fn order_substitution_becomes_a_named_law_citation() {
        let (x_id, x) = value(1);
        let (y_id, y) = value(2);
        let (z_id, z) = value(3);
        let context = PropositionContext::from_value_types([
            (x_id, unsigned64_type()),
            (y_id, unsigned64_type()),
            (z_id, unsigned64_type()),
        ])
        .expect("context");
        let relation = Proposition::LessOrEqual(x.clone(), y);
        let equality = Proposition::Equal(x, z.clone());
        let goal = Proposition::LessOrEqual(z.clone(), ScalarTerm::value(y_id, unsigned64_type()));
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::IntegerOrderSubstitution {
                relation: Box::new(assumption_proof(relation.clone(), 0)),
                equality: Box::new(assumption_proof(equality.clone(), 1)),
                endpoint: 0,
            },
        };
        let denoted =
            v!(&context => &goal => &[relation, equality] => &[] => &proof => &mut budget())
                .expect("order substitution verifies as a roster citation");
        let mut head = denoted.certificate.term;
        for _ in 0..5 {
            let Term::Apply { function, .. } = denoted.arena.get(head) else {
                panic!("a substitution citation applies endpoints then evidence");
            };
            head = function;
        }
        assert!(matches!(denoted.arena.get(head), Term::Constant { .. }));

        let wrong = Proposition::LessOrEqual(z.clone(), literal(9));
        let proof = ProofNode {
            conclusion: wrong.clone(),
            rule: ProofRule::IntegerOrderSubstitution {
                relation: Box::new(assumption_proof(
                    Proposition::LessOrEqual(
                        ScalarTerm::value(x_id, unsigned64_type()),
                        ScalarTerm::value(y_id, unsigned64_type()),
                    ),
                    0,
                )),
                equality: Box::new(assumption_proof(
                    Proposition::Equal(ScalarTerm::value(x_id, unsigned64_type()), z.clone()),
                    1,
                )),
                endpoint: 0,
            },
        };
        assert!(matches!(
            denote_bounded_certificate(
                &context,
                &wrong,
                &[
                    Proposition::LessOrEqual(
                        ScalarTerm::value(x_id, unsigned64_type()),
                        ScalarTerm::value(y_id, unsigned64_type()),
                    ),
                    Proposition::Equal(ScalarTerm::value(x_id, unsigned64_type()), z),
                ],
                &[],
                &proof,
            ),
            Err(BoundedDenotationError::Certificate(_)),
        ));
    }

    #[test]
    fn a_decided_closed_equality_is_refl_in_the_kernel() {
        let two = literal(2);
        let zero = literal(0);
        let sum = ScalarTerm::wrapping_integer_add(unsigned64(), two.clone(), zero)
            .expect("wrapping add");
        let goal = Proposition::Equal(sum, two);
        let proof = proof_node!(goal.clone() => ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation));
        let denoted = verify_bounded_certificate(
            &PropositionContext::default(),
            &goal,
            &[],
            &[],
            &proof,
            &mut budget(),
        )
        .expect("the decided equality denotes a refl-provable Id");
        assert_eq!(denoted.certificate.signature.len(), 6);
        assert!(denoted.certificate.context.is_empty());
        assert!(matches!(
            denoted.arena.get(denoted.certificate.term),
            Term::Refl { .. }
        ));
        assert_eq!(
            certificate_assumption_closure(&denoted.arena, &denoted.certificate),
            BTreeSet::from([0, 1, 2, 4]),
        );
    }

    #[test]
    fn a_closed_order_uses_only_the_fixed_numeral_law_closure() {
        let goal = Proposition::LessThan(literal(1), literal(2));
        let proof = proof_node!(goal.clone() => ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation));
        let mut denoted = verify_bounded_certificate(
            &PropositionContext::default(),
            &goal,
            &[],
            &[],
            &proof,
            &mut budget(),
        )
        .expect("the decided order verifies");
        assert_eq!(denoted.certificate.signature.len(), 14);
        assert_eq!(
            certificate_assumption_closure(&denoted.arena, &denoted.certificate),
            BTreeSet::from([0, 1, 2, 4, 6, 8, 10, 12, 13]),
            "exact fixed numeral vocabulary and order-law closure",
        );

        let mut missing_equation = denoted.certificate.clone();
        missing_equation.signature[3].body = None;
        assert!(
            verify_mathematical_certificate(&mut denoted.arena, &missing_equation, &mut budget(),)
                .is_err()
        );
        let zero = denoted.arena.insert(Term::Constant {
            declaration: 1,
            levels: Vec::new(),
        });
        let mut changed_operand = denoted.certificate.clone();
        changed_operand.signature[3].body = Some(zero);
        assert!(
            verify_mathematical_certificate(&mut denoted.arena, &changed_operand, &mut budget(),)
                .is_err()
        );

        let proof =
            proof_node!(Proposition::Truth => ProofRule::Primitive(PrimitiveJudgment::Truth));
        let denoted = verify_bounded_certificate(
            &PropositionContext::default(),
            &Proposition::Truth,
            &[],
            &[],
            &proof,
            &mut budget(),
        )
        .expect("truth verifies");
        assert!(denoted.certificate.signature.is_empty());
        assert!(certificate_assumption_closure(&denoted.arena, &denoted.certificate).is_empty());
    }

    #[test]
    fn disjunction_rules_become_tagged_sum_constructions() {
        let p = atom(1);
        let q = atom(2);
        let r = atom(3);
        let truth_node =
            || proof_node!(Proposition::Truth => ProofRule::Primitive(PrimitiveJudgment::Truth));

        let disjunction = Proposition::Disjunction(vec![p.clone(), q.clone()]);
        let proof = ProofNode {
            conclusion: disjunction.clone(),
            rule: ProofRule::DisjunctionIntroduction {
                disjunct: Box::new(assumption_proof(p.clone(), 0)),
                index: 0,
            },
        };
        let denoted = verify_bounded_certificate(
            &PropositionContext::default(),
            &disjunction,
            from_ref(&p),
            &[],
            &proof,
            &mut budget(),
        )
        .expect("disjunction introduction verifies as a tagged pair");
        assert!(matches!(
            denoted.arena.get(denoted.certificate.term),
            Term::Pair { .. }
        ));
        assert_eq!(
            certificate_assumption_closure(&denoted.arena, &denoted.certificate),
            BTreeSet::from([0, 1]),
            "the tagged sum names both disjunct atoms",
        );

        for (disjuncts, index) in [
            (vec![p.clone(), q.clone()], 1),
            (vec![p.clone(), q.clone(), r.clone()], 1),
            (vec![p.clone(), q.clone(), r.clone()], 2),
        ] {
            let selected = disjuncts[index].clone();
            let disjunction = Proposition::Disjunction(disjuncts);
            let proof = ProofNode {
                conclusion: disjunction.clone(),
                rule: ProofRule::DisjunctionIntroduction {
                    disjunct: Box::new(assumption_proof(selected.clone(), 0)),
                    index,
                },
            };
            let denoted = verify_bounded_certificate(
                &PropositionContext::default(),
                &disjunction,
                from_ref(&selected),
                &[],
                &proof,
                &mut budget(),
            )
            .unwrap_or_else(|error| {
                panic!("disjunct {index} of {disjunction:?} verifies as a tagged pair: {error}")
            });
            let Term::Pair { first, .. } = denoted.arena.get(denoted.certificate.term) else {
                panic!("a disjunction introduction denotes a pair");
            };
            assert!(matches!(denoted.arena.get(first), Term::TwoOne));
        }

        let proof = ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::DisjunctionElimination {
                disjunction: Box::new(assumption_proof(disjunction.clone(), 0)),
                branches: vec![truth_node(), truth_node()],
            },
        };
        let denoted = verify_bounded_certificate(
            &PropositionContext::default(),
            &Proposition::Truth,
            &[disjunction],
            &[],
            &proof,
            &mut budget(),
        )
        .expect("two-branch elimination verifies as dependent caseTwo");
        assert!(matches!(
            denoted.arena.get(denoted.certificate.term),
            Term::Apply { .. }
        ));

        let disjunction = Proposition::Disjunction(vec![p, q, r]);
        let proof = ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::DisjunctionElimination {
                disjunction: Box::new(assumption_proof(disjunction.clone(), 0)),
                branches: vec![truth_node(), truth_node(), truth_node()],
            },
        };
        let denoted = verify_bounded_certificate(
            &PropositionContext::default(),
            &Proposition::Truth,
            from_ref(&disjunction),
            &[],
            &proof,
            &mut budget(),
        )
        .expect("three-branch elimination verifies through the nested sum");
        assert_eq!(denoted.certificate.signature.len(), 3);
        assert_eq!(
            certificate_assumption_closure(&denoted.arena, &denoted.certificate),
            BTreeSet::from([0, 1, 2]),
        );
        assert_eq!(
            denoted.assumptions,
            vec![AcceptedPremise {
                index: 0,
                proposition: disjunction,
            }],
        );
    }

    #[test]
    fn semantic_axioms_bind_after_the_assumption_roster() {
        let p = atom(1);
        let implication = Proposition::Implication {
            premise: Box::new(p.clone()),
            conclusion: Box::new(p.clone()),
        };
        let goal = Proposition::Implication {
            premise: Box::new(implication.clone()),
            conclusion: Box::new(p.clone()),
        };
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::ImplicationIntroduction {
                body: Box::new(ProofNode {
                    conclusion: p.clone(),
                    rule: ProofRule::ImplicationElimination {
                        implication: Box::new(assumption_proof(implication, 0)),
                        premise: Box::new(semantic_axiom_proof(p.clone(), 0)),
                    },
                }),
            },
        };
        let denoted = verify_bounded_certificate(
            &PropositionContext::default(),
            &goal,
            &[],
            from_ref(&p),
            &proof,
            &mut budget(),
        )
        .expect("the axiom application verifies");
        assert_eq!(denoted.certificate.signature.len(), 1);
        assert_eq!(denoted.certificate.context.len(), 1);
        assert_eq!(
            denoted.semantic_axioms,
            vec![AcceptedPremise {
                index: 0,
                proposition: p,
            }],
        );
        assert_eq!(
            certificate_assumption_closure(&denoted.arena, &denoted.certificate),
            BTreeSet::from([0]),
        );
    }

    #[test]
    fn forged_and_malformed_certificates_reject() {
        let (x_id, x) = value(1);
        let (y_id, y) = value(2);
        let context = PropositionContext::from_value_types([
            (x_id, unsigned64_type()),
            (y_id, unsigned64_type()),
        ])
        .expect("context");
        let bound = Proposition::LessOrEqual(x, y.clone());
        let swapped = Proposition::LessOrEqual(y, ScalarTerm::value(x_id, unsigned64_type()));

        let proof = assumption_proof(bound.clone(), 1);
        assert!(matches!(
            denote_bounded_certificate(&context, &bound, from_ref(&bound), &[], &proof),
            Err(BoundedDenotationError::Certificate(
                ProofError::UnknownAssumption(1)
            )),
        ));

        let proof = assumption_proof(swapped.clone(), 0);
        assert!(matches!(
            denote_bounded_certificate(&context, &swapped, from_ref(&bound), &[], &proof),
            Err(BoundedDenotationError::Certificate(
                ProofError::AssumptionConclusionMismatch(0)
            )),
        ));

        let proof = assumption_proof(bound.clone(), 0);
        assert!(matches!(
            denote_bounded_certificate(
                &context,
                &Proposition::Truth,
                from_ref(&bound),
                &[],
                &proof
            ),
            Err(BoundedDenotationError::Certificate(
                ProofError::CertificateConclusionMismatch
            )),
        ));

        let falsehood = Proposition::LessThan(literal(2), literal(1));
        let proof = proof_node!(falsehood.clone() => ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation));
        assert!(matches!(
            denote_bounded_certificate(
                &PropositionContext::default(),
                &falsehood,
                &[],
                &[],
                &proof,
            ),
            Err(BoundedDenotationError::Certificate(
                ProofError::PrimitiveJudgment(_)
            )),
        ));

        let proof = assumption_proof(bound.clone(), 0);
        let mut denoted =
            denote_bounded_certificate(&context, &bound, from_ref(&bound), &[], &proof)
                .expect("denotation succeeds before tampering");
        denoted.certificate.term = denoted.arena.insert(Term::Variable(9));
        assert!(matches!(
            verify_mathematical_certificate(
                &mut denoted.arena,
                &denoted.certificate,
                &mut budget(),
            ),
            Err(crate::CoreError::UnboundVariable { .. }),
        ));
    }

    #[test]
    fn the_equal_integer_math_crossing_denotes_identically() {
        let (x_id, x) = value(1);
        let (y_id, y) = value(2);
        let context = PropositionContext::from_value_types([
            (x_id, unsigned64_type()),
            (y_id, unsigned64_type()),
        ])
        .expect("context");
        let fixed = Proposition::Equal(x, y);
        let lifted = crate::lift_fixed_integer_relation(&fixed).expect("lifts");
        let proof = assumption_proof(lifted.clone(), 0);
        assert_eq!(
            crate::check_certificate(&context, &lifted, from_ref(&fixed), &[], &proof),
            Ok(()),
            "the bounded checker accepts the normalized citation",
        );
        let denoted = verify_bounded_certificate(
            &context,
            &lifted,
            from_ref(&fixed),
            &[],
            &proof,
            &mut budget(),
        )
        .expect("the crossing denotes the same `Id Int` and verifies");
        assert_eq!(denoted.certificate.signature.len(), 3);
        assert_eq!(
            denoted.arena.get(denoted.certificate.term),
            Term::Variable(0),
        );
    }

    #[test]
    fn the_closure_names_every_judgment_premise() {
        let p = atom(1);
        let q = atom(2);
        let proof = assumption_proof(p.clone(), 0);
        let denoted = verify_bounded_certificate(
            &PropositionContext::default(),
            &p,
            &[p.clone(), q],
            &[],
            &proof,
            &mut budget(),
        )
        .expect("the citation verifies");
        assert_eq!(
            certificate_assumption_closure(&denoted.arena, &denoted.certificate),
            BTreeSet::from([0, 1]),
            "both context bindings enter the closure even though only the first is cited",
        );
        assert_eq!(
            denoted.assumptions,
            vec![AcceptedPremise {
                index: 0,
                proposition: p,
            }],
            "the acceptance projection still records only the cited premise",
        );
    }
    mod connectives {
        use super::*;
        use crate::proof_node;

        fn predicate_denotation_elaboration(
            premise: &Proposition,
            goal: &Proposition,
        ) -> Result<BoundedDenotation, BoundedDenotationError> {
            let (premise_proof, roster) = if matches!(premise, Proposition::Truth) {
                (
                    proof_node!(premise.clone() => ProofRule::Primitive(PrimitiveJudgment::Truth)),
                    Vec::new(),
                )
            } else {
                (assumption_proof(premise.clone(), 0), vec![premise.clone()])
            };
            let proof = ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::PredicateDenotation {
                    premise: Box::new(premise_proof),
                },
            };
            let context = PropositionContext::default();
            let machine_parameters = BTreeSet::new();
            let mut elaboration =
                Elaboration::new(&context, goal, &roster, &[], &machine_parameters)
                    .expect("the normalized relation licenses the conversion");
            let term = elaboration.node(&proof)?;
            assert_eq!(
                elaboration.denotation.rule_axioms.len(),
                0,
                "{premise:?} -> {goal:?} derives without a rule-instance axiom",
            );
            let mut denoted = elaboration.finish(term);
            verify_mathematical_certificate(
                &mut denoted.arena,
                &denoted.certificate,
                &mut budget(),
            )
            .expect("the derived judgment re-decides in the kernel");
            Ok(denoted)
        }

        #[test]
        fn closed_false_integer_relations_refute_to_empty_elimination() {
            for premise in [
                Proposition::LessThan(literal(3), literal(2)),
                Proposition::LessThan(literal(2), literal(2)),
                Proposition::LessOrEqual(literal(3), literal(2)),
                Proposition::Equal(literal(2), literal(3)),
                Proposition::Equal(literal(3), literal(2)),
            ] {
                let denoted = predicate_denotation_elaboration(&premise, &Proposition::Falsehood)
                    .expect("the closed-false relation denotes a refutation");
                assert!(
                    matches!(
                        denoted.arena.get(denoted.certificate.term),
                        Term::EmptyElim { .. }
                    ),
                    "{premise:?} eliminates its checked `Empty` into Falsehood",
                );
            }
        }

        #[test]
        fn the_falsehood_premise_transports_through_the_j_discriminant() {
            let goal = Proposition::Equal(literal(2), literal(3));
            let denoted = predicate_denotation_elaboration(&Proposition::Falsehood, &goal)
                .expect("the Falsehood premise denotes a discriminant elimination");
            assert!(
                matches!(
                    denoted.arena.get(denoted.certificate.term),
                    Term::IdElim { .. }
                ),
                "the evidence is the `J` discriminant landing on the goal",
            );
        }

        #[test]
        fn closed_true_goals_are_inhabited_outright() {
            let denoted = predicate_denotation_elaboration(
                &Proposition::Equal(literal(2), literal(2)),
                &Proposition::Truth,
            )
            .expect("a closed-true identity crosses to Truth");
            assert!(
                matches!(
                    denoted.arena.get(denoted.certificate.term),
                    Term::Refl { .. }
                ),
                "the `Id Two zero zero` goal is `refl`",
            );

            for goal in [
                Proposition::LessThan(literal(1), literal(2)),
                Proposition::LessOrEqual(literal(2), literal(2)),
                Proposition::Equal(literal(2), literal(2)),
            ] {
                predicate_denotation_elaboration(&Proposition::Truth, &goal)
                    .expect("a Truth premise inhabits the closed-true relation");
            }
        }

        #[test]
        fn the_crossing_composes_through_connective_denotations() {
            let premise = Proposition::Conjunction(vec![
                Proposition::LessThan(literal(3), literal(2)),
                Proposition::Equal(literal(2), literal(2)),
            ]);
            let denoted = predicate_denotation_elaboration(&premise, &Proposition::Falsehood)
                .expect("a conjunction with a refutable child denotes a projection");
            assert!(
                matches!(
                    denoted.arena.get(denoted.certificate.term),
                    Term::EmptyElim { .. }
                ),
                "the refuted component eliminates `Empty` into Falsehood",
            );

            let goal = Proposition::Conjunction(vec![
                Proposition::Equal(literal(2), literal(2)),
                Proposition::LessOrEqual(literal(2), literal(2)),
            ]);
            let denoted = predicate_denotation_elaboration(&Proposition::Truth, &goal)
                .expect("a Truth-normalized conjunction denotes a pair");
            assert!(
                matches!(
                    denoted.arena.get(denoted.certificate.term),
                    Term::Pair { .. }
                ),
                "the conjunction's components pair",
            );

            let goal = Proposition::Disjunction(vec![
                Proposition::Equal(literal(2), literal(3)),
                Proposition::Truth,
            ]);
            let denoted = predicate_denotation_elaboration(&Proposition::Truth, &goal)
                .expect("a Truth-normalized disjunction denotes a tagged pair");
            let Term::Pair { first, .. } = denoted.arena.get(denoted.certificate.term) else {
                panic!("the disjunction's inhabited branch pairs with a tag");
            };
            assert_eq!(denoted.arena.get(first), Term::TwoOne);
        }

        #[test]
        fn an_open_boolean_expansion_denotes_by_case_analysis() {
            let (a_id, a) = boolean_value(1);
            let (b_id, b) = boolean_value(2);
            let context = PropositionContext::from_value_types([
                (a_id, ScalarType::Boolean),
                (b_id, ScalarType::Boolean),
            ])
            .expect("context");
            let premise =
                Proposition::Equal(ScalarTerm::boolean_not(a.clone()).expect("not"), b.clone());
            let goal = Proposition::Disjunction(vec![
                Proposition::Conjunction(vec![
                    Proposition::Equal(a.clone(), ScalarTerm::Boolean(false)),
                    Proposition::Equal(b.clone(), ScalarTerm::Boolean(true)),
                ]),
                Proposition::Conjunction(vec![
                    Proposition::Equal(a.clone(), ScalarTerm::Boolean(true)),
                    Proposition::Equal(b, ScalarTerm::Boolean(false)),
                ]),
            ]);
            let proof = ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::PredicateDenotation {
                    premise: Box::new(assumption_proof(premise.clone(), 0)),
                },
            };
            let machine_parameters = BTreeSet::new();
            let mut elaboration = Elaboration::new(
                &context,
                &goal,
                std::slice::from_ref(&premise),
                &[],
                &machine_parameters,
            )
            .expect("the Boolean expansion licenses the conversion");
            let term = elaboration
                .node(&proof)
                .expect("the open Boolean expansion denotes a case analysis");
            assert!(
                elaboration.denotation.rule_axioms.is_empty(),
                "the crossing derives without naming a rule-instance axiom",
            );
            let mut denoted = elaboration.finish(term);
            let Term::Apply { function, .. } = denoted.arena.get(denoted.certificate.term) else {
                panic!("the expansion denotes an applied case-analysis function");
            };
            assert!(
                matches!(denoted.arena.get(function), Term::CaseTwo { .. }),
                "the transport function eliminates a Boolean atom",
            );
            assert_eq!(denoted.certificate.signature.len(), 2);
            verify_mathematical_certificate(
                &mut denoted.arena,
                &denoted.certificate,
                &mut budget(),
            )
            .expect("the derived judgment re-decides in the kernel");
        }

        #[test]
        fn a_permuted_conjunction_derives_through_its_projections() {
            let premise = Proposition::Conjunction(vec![atom(2), atom(1)]);
            let goal = Proposition::Conjunction(vec![atom(1), atom(2)]);
            let denoted = predicate_denotation_elaboration(&premise, &goal)
                .expect("the permuted conjunction derives");
            let Term::Pair { first, second } = denoted.arena.get(denoted.certificate.term) else {
                panic!("the permuted conjunction re-pairs its projections");
            };
            assert!(matches!(denoted.arena.get(first), Term::Snd { .. }));
            assert!(matches!(denoted.arena.get(second), Term::Fst { .. }));
        }

        #[test]
        fn a_deduplicated_conjunction_projects_and_re_pairs() {
            let premise = Proposition::Conjunction(vec![atom(1), atom(1)]);
            predicate_denotation_elaboration(&premise, &atom(1))
                .expect("the duplicated conjunction projects");

            let goal = Proposition::Conjunction(vec![atom(1), atom(1)]);
            predicate_denotation_elaboration(&atom(1), &goal)
                .expect("the duplicated conclusion re-pairs the premise");
        }

        #[test]
        fn a_flattened_conjunction_re_associates_through_projections() {
            let premise = Proposition::Conjunction(vec![
                Proposition::Conjunction(vec![atom(1), atom(2)]),
                atom(3),
            ]);
            let goal = Proposition::Conjunction(vec![atom(1), atom(2), atom(3)]);
            let denoted = predicate_denotation_elaboration(&premise, &goal)
                .expect("the flattened conjunction derives");
            let Term::Pair { second, .. } = denoted.arena.get(denoted.certificate.term) else {
                panic!("the flattened conjunction re-pairs its components");
            };
            assert!(
                matches!(denoted.arena.get(second), Term::Pair { .. }),
                "the re-associated tail pairs the nested projections",
            );
        }

        #[test]
        fn a_truth_conjunct_drops_through_the_projection() {
            let premise = Proposition::Conjunction(vec![atom(1), Proposition::Truth]);
            let denoted = predicate_denotation_elaboration(&premise, &atom(1))
                .expect("the Truth conjunct drops");
            assert!(
                matches!(
                    denoted.arena.get(denoted.certificate.term),
                    Term::Fst { .. }
                ),
                "the evidence is the surviving component's projection",
            );

            let goal = Proposition::Conjunction(vec![atom(1), Proposition::Truth]);
            let denoted = predicate_denotation_elaboration(&atom(1), &goal)
                .expect("the Truth conjunct pairs with its closed inhabitant");
            let Term::Pair { second, .. } = denoted.arena.get(denoted.certificate.term) else {
                panic!("the Truth-normalized goal pairs its components");
            };
            assert!(
                matches!(denoted.arena.get(second), Term::Refl { .. }),
                "the dropped Truth is refl at Id Two zero zero",
            );
        }

        #[test]
        fn a_permuted_disjunction_derives_through_retagging() {
            let premise = Proposition::Disjunction(vec![atom(2), atom(1)]);
            let goal = Proposition::Disjunction(vec![atom(1), atom(2)]);
            let denoted = predicate_denotation_elaboration(&premise, &goal)
                .expect("the permuted disjunction derives");
            let Term::Apply { function, .. } = denoted.arena.get(denoted.certificate.term) else {
                panic!("the disjunction eliminates the premise pair");
            };
            let Term::CaseTwo {
                zero_branch,
                one_branch,
                ..
            } = denoted.arena.get(function)
            else {
                panic!("the eliminator case-splits the tag");
            };
            let Term::Lambda { body, .. } = denoted.arena.get(zero_branch) else {
                panic!("the zero branch binds its payload");
            };
            let Term::Pair { first, .. } = denoted.arena.get(body) else {
                panic!("the zero branch re-tags into the goal");
            };
            assert_eq!(
                denoted.arena.get(first),
                Term::TwoOne,
                "the premise's first disjunct is the goal's rest",
            );
            let Term::Lambda { body, .. } = denoted.arena.get(one_branch) else {
                panic!("the one branch binds its payload");
            };
            let Term::Pair { first, .. } = denoted.arena.get(body) else {
                panic!("the one branch re-tags into the goal");
            };
            assert_eq!(
                denoted.arena.get(first),
                Term::TwoZero,
                "the premise's rest disjunct is the goal's first",
            );
        }

        #[test]
        fn a_falsehood_disjunct_drops_through_the_eliminator() {
            let premise = Proposition::Disjunction(vec![atom(1), Proposition::Falsehood]);
            let denoted = predicate_denotation_elaboration(&premise, &atom(1))
                .expect("the Falsehood disjunct drops");
            let Term::Apply { function, .. } = denoted.arena.get(denoted.certificate.term) else {
                panic!("the disjunction eliminates the premise pair");
            };
            let Term::CaseTwo { one_branch, .. } = denoted.arena.get(function) else {
                panic!("the eliminator case-splits the tag");
            };
            let Term::Lambda { body, .. } = denoted.arena.get(one_branch) else {
                panic!("the Falsehood branch binds its payload");
            };
            assert!(
                matches!(
                    denoted.arena.get(body),
                    Term::IdElim { .. } | Term::EmptyElim { .. }
                ),
                "the Falsehood payload refutes into the goal",
            );

            let goal = Proposition::Disjunction(vec![atom(1), Proposition::Falsehood]);
            let denoted = predicate_denotation_elaboration(&atom(1), &goal)
                .expect("the surviving disjunct tags zero");
            let Term::Pair { first, .. } = denoted.arena.get(denoted.certificate.term) else {
                panic!("the disjunction introduction pairs a tag");
            };
            assert_eq!(denoted.arena.get(first), Term::TwoZero);
        }

        #[test]
        fn an_implication_domain_derives_contravariantly() {
            let premise = Proposition::Implication {
                premise: Box::new(Proposition::Conjunction(vec![atom(2), atom(1)])),
                conclusion: Box::new(atom(3)),
            };
            let goal = Proposition::Implication {
                premise: Box::new(Proposition::Conjunction(vec![atom(1), atom(2)])),
                conclusion: Box::new(atom(3)),
            };
            let denoted = predicate_denotation_elaboration(&premise, &goal)
                .expect("the implication's normalized domain derives");
            assert!(
                matches!(
                    denoted.arena.get(denoted.certificate.term),
                    Term::Lambda { .. }
                ),
                "the crossing stays inside the function body",
            );
        }

        #[test]
        fn wide_connective_denotation_uses_default_stack_and_preserves_order() {
            std::thread::spawn(|| {
        use super::super::Denotation;
        use crate::mathematical_core::{Context, infer_sort};
        for disjunctive in [false, true] {
            let mut parts = vec![Proposition::Truth; 16_384];
            parts.push(Proposition::Falsehood);
            let proposition = if disjunctive {
                Proposition::Disjunction(parts)
            } else {
                Proposition::Conjunction(parts)
            };
            PropositionContext::default().validate(&proposition)
                .expect("every ordered member is a valid proposition");
            let parts = match &proposition {
                Proposition::Conjunction(parts) | Proposition::Disjunction(parts) => parts,
                _ => unreachable!(),
            };
            eprintln!("ordered proposition members admitted; begin disjunctive={disjunctive} denotation");
            let mut denotation = Denotation::new();
            let term = if disjunctive {
                denotation.disjunction(parts)
            } else {
                denotation.denote(&proposition)
            }.expect("every ordered member denotes");
            eprintln!("denotation complete; begin independent kernel formation");
            let Term::Sort(expected_sort) = denotation.arena.get(denotation.type_zero) else {
                panic!("the denotation owns Type 0");
            };
            let actual = infer_sort(&mut denotation.arena, &Context::empty(), term, &mut Budget::new(2_000_000))
                .expect("the complete connective denotes a type under its exact binders");
            assert!(crate::mathematical_core::term::sorts_equal(&actual, &expected_sort));
            let mut tail = term;
            for _ in 0..16_384 {
                let Term::Sigma { domain, codomain } = denotation.arena.get(tail) else {
                    panic!("one connective member remains before the final statement");
                };
                let first = if disjunctive {
                    assert!(matches!(denotation.arena.get(domain), Term::Two));
                    let Term::CaseTwo { zero_branch, one_branch, scrutinee, .. } = denotation.arena.get(codomain) else {
                        panic!("the tagged sum retains its exact branch family");
                    };
                    assert!(matches!(denotation.arena.get(scrutinee), Term::Variable(0)));
                    tail = one_branch;
                    zero_branch
                } else {
                    tail = codomain;
                    domain
                };
                let Term::Id { left, right, .. } = denotation.arena.get(first) else {
                    panic!("every preceding member is Truth");
                };
                assert!(denotation.arena.structurally_equal(left, right));
            }
            let Term::Id { left, right, .. } = denotation.arena.get(tail) else {
                panic!("the final member is Falsehood");
            };
            assert!(!denotation.arena.structurally_equal(left, right));
            let before = denotation.arena.len();
            let declarations = denotation.declarations.len();
            let fresh = Proposition::Atom(PropositionId::new(9_991).expect("fresh atom"));
            let invalid = if disjunctive {
                denotation.disjunction(&[fresh])
            } else {
                denotation.denote(&Proposition::Conjunction(vec![fresh]))
            };
            let expected = if disjunctive { "disjunction arity" } else { "conjunction arity" };
            assert!(matches!(invalid, Err(BoundedDenotationError::Certificate(ProofError::RuleConclusionMismatch(actual))) if actual == expected));
            assert_eq!(denotation.arena.len(), before, "invalid arity refuses before allocating a member");
            assert_eq!(denotation.declarations.len(), declarations);
        }
            }).join().expect("connective denotation and kernel formation use the default stack");
        }

        #[test]
        fn connective_denotation_preserves_member_failure_and_declaration_order() {
            use super::super::Denotation;
            for disjunctive in [false, true] {
                let first = Proposition::Atom(PropositionId::new(10_001).expect("first atom"));
                let last = Proposition::Atom(PropositionId::new(10_002).expect("later atom"));
                let malformed = Proposition::Conjunction(vec![Proposition::Truth]);
                let members = [first.clone(), malformed, last.clone()];
                let mut denotation = Denotation::new();
                let observed = if disjunctive {
                    denotation.disjunction(&members)
                } else {
                    denotation.denote(&Proposition::Conjunction(members.to_vec()))
                };
                assert!(matches!(
                    observed,
                    Err(BoundedDenotationError::Certificate(
                        ProofError::RuleConclusionMismatch("conjunction arity")
                    ))
                ));
                assert!(denotation.denotations.cached_atom(&first).is_some());
                assert!(denotation.denotations.cached_atom(&last).is_none());
                assert_eq!(
                    denotation.declarations.len(),
                    1,
                    "the earlier declaration remains and the later declaration is never admitted"
                );
            }
        }

        #[test]
        fn deep_proof_elaboration_visits_every_premise_on_default_stack() {
            std::thread::spawn(|| {
                for valid in [true, false] {
                    let mut proof = ProofNode {
                        conclusion: if valid {
                            Proposition::Truth
                        } else {
                            Proposition::Falsehood
                        },
                        rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
                    };
                    for _ in 0..4096 {
                        proof = ProofNode {
                            conclusion: Proposition::Truth,
                            rule: ProofRule::PredicateDenotation {
                                premise: Box::new(proof),
                            },
                        };
                    }
                    let context = PropositionContext::default();
                    context
                        .validate(&proof.conclusion)
                        .expect("outer goal is a valid statement");
                    let parameters = BTreeSet::new();
                    let mut elaboration =
                        Elaboration::new(&context, &proof.conclusion, &[], &[], &parameters)
                            .expect("outer statement denotes independently");
                    eprintln!("4097-node proof prepared; begin valid={valid} elaboration");
                    let observed = elaboration.node(&proof);
                    if valid {
                        let term = observed.expect("every primitive premise is admitted");
                        let mut denoted = elaboration.finish(term);
                        verify_mathematical_certificate(
                            &mut denoted.arena,
                            &denoted.certificate,
                            &mut budget(),
                        )
                        .expect("the independently reconstructed complete judgment re-decides");
                    } else {
                        assert!(matches!(
                            observed,
                            Err(BoundedDenotationError::Certificate(
                                ProofError::PrimitiveJudgment(_)
                            ))
                        ));
                    }
                    eprintln!("elaboration result checked; ordinary fixture teardown follows");
                }
            })
            .join()
            .expect("elaboration and its owned fixture stay on the default stack");
        }

        #[test]
        fn elaboration_restores_binders_after_scope_errors_and_siblings() {
            let truth = Proposition::Truth;
            let arrow = Proposition::Implication {
                premise: Box::new(truth.clone()),
                conclusion: Box::new(truth.clone()),
            };
            let goal = Proposition::Conjunction(vec![arrow.clone(), truth.clone()]);
            let assumptions = [truth.clone(), truth.clone()];
            let context = PropositionContext::default();
            let parameters = BTreeSet::new();
            let mut elaboration =
                Elaboration::new(&context, &goal, &assumptions, &[], &parameters).unwrap();
            let invalid = ProofNode {
                conclusion: arrow.clone(),
                rule: ProofRule::ImplicationIntroduction {
                    body: Box::new(assumption_proof(truth.clone(), 3)),
                },
            };
            assert!(matches!(
                elaboration.node(&invalid),
                Err(BoundedDenotationError::Certificate(
                    ProofError::UnknownAssumption(3)
                ))
            ));
            let proof = ProofNode {
                conclusion: goal,
                rule: ProofRule::ConjunctionIntroduction(vec![
                    ProofNode {
                        conclusion: arrow,
                        rule: ProofRule::ImplicationIntroduction {
                            body: Box::new(assumption_proof(truth.clone(), 2)),
                        },
                    },
                    assumption_proof(truth, 1),
                ]),
            };
            let term = elaboration
                .node(&proof)
                .expect("later proof keeps the initial roster after failure");
            let mut denoted = elaboration.finish(term);
            assert_eq!(
                denoted
                    .assumptions
                    .iter()
                    .map(|premise| premise.index)
                    .collect::<Vec<_>>(),
                [1]
            );
            let Term::Pair { first, second } = denoted.arena.get(term) else {
                panic!("conjunction retains both pieces")
            };
            assert!(
                matches!(denoted.arena.get(second), Term::Variable(0)),
                "ambient premise 1 stays the innermost original binding"
            );
            let Term::Lambda { body, .. } = denoted.arena.get(first) else {
                panic!("implication evidence is a lambda")
            };
            assert!(
                matches!(denoted.arena.get(body), Term::Variable(0)),
                "the body cites only its discharged hypothesis"
            );
            verify_mathematical_certificate(
                &mut denoted.arena,
                &denoted.certificate,
                &mut budget(),
            )
            .unwrap();
        }

        #[test]
        fn elaboration_tracks_ambient_evidence_through_nested_case_binders() {
            let truth = Proposition::Truth;
            let disjunction =
                Proposition::Disjunction(vec![truth.clone(), truth.clone(), truth.clone()]);
            let goal = Proposition::Conjunction(vec![truth.clone(), truth.clone()]);
            let assumptions = [truth.clone(), disjunction.clone()];
            let context = PropositionContext::default();
            let parameters = BTreeSet::new();
            let mut elaboration =
                Elaboration::new(&context, &goal, &assumptions, &[], &parameters).unwrap();
            let make = |middle| ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::ConjunctionIntroduction(vec![
                    ProofNode {
                        conclusion: truth.clone(),
                        rule: ProofRule::DisjunctionElimination {
                            disjunction: Box::new(assumption_proof(disjunction.clone(), 1)),
                            branches: vec![
                                assumption_proof(truth.clone(), 0),
                                assumption_proof(truth.clone(), middle),
                                assumption_proof(truth.clone(), 0),
                            ],
                        },
                    },
                    assumption_proof(truth.clone(), 0),
                ]),
            };
            assert!(matches!(
                elaboration.node(&make(3)),
                Err(BoundedDenotationError::Certificate(
                    ProofError::UnknownAssumption(3)
                ))
            ));
            let proof = make(0);
            let term = elaboration
                .node(&proof)
                .expect("all branches cite the exact ambient premise");
            let mut denoted = elaboration.finish(term);
            assert_eq!(
                denoted
                    .assumptions
                    .iter()
                    .map(|premise| premise.index)
                    .collect::<Vec<_>>(),
                [1, 0]
            );
            let Term::Pair { first, second } = denoted.arena.get(term) else {
                panic!("conjunction keeps its later sibling")
            };
            assert!(
                matches!(denoted.arena.get(second), Term::Variable(1)),
                "case scopes end before the ambient sibling"
            );
            let branches = |term| {
                let Term::Apply { function, .. } = denoted.arena.get(term) else {
                    panic!("case evidence applies its payload")
                };
                let Term::CaseTwo {
                    zero_branch,
                    one_branch,
                    ..
                } = denoted.arena.get(function)
                else {
                    panic!("the tag selects the admitted case branches")
                };
                (zero_branch, one_branch)
            };
            let (zero, one) = branches(first);
            let Term::Lambda { body, .. } = denoted.arena.get(zero) else {
                panic!("first branch discharges one hypothesis")
            };
            assert!(matches!(denoted.arena.get(body), Term::Variable(2)));
            let Term::Lambda { body, .. } = denoted.arena.get(one) else {
                panic!("remaining cases bind the tagged remainder")
            };
            let (zero, one) = branches(body);
            for branch in [zero, one] {
                let Term::Lambda { body, .. } = denoted.arena.get(branch) else {
                    panic!("each final branch discharges its own hypothesis")
                };
                assert!(
                    matches!(denoted.arena.get(body), Term::Variable(3)),
                    "intermediate binders shift the same ambient evidence without becoming premises"
                );
            }
            verify_mathematical_certificate(
                &mut denoted.arena,
                &denoted.certificate,
                &mut budget(),
            )
            .unwrap();
        }

        #[test]
        fn repeated_disjunction_tags_preserve_the_exact_selected_member() {
            let members = (0..64)
                .map(|index| {
                    Proposition::Atom(PropositionId::new(20_001 + index).expect("distinct member"))
                })
                .collect::<Vec<_>>();
            let goal = Proposition::Disjunction(members.clone());
            for index in [31, 63] {
                let context = PropositionContext::default();
                let parameters = BTreeSet::new();
                let assumptions = [members[index].clone()];
                let mut elaboration =
                    Elaboration::new(&context, &goal, &assumptions, &[], &parameters).unwrap();
                let make = |index| ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::DisjunctionIntroduction {
                        disjunct: Box::new(assumption_proof(assumptions[0].clone(), 0)),
                        index,
                    },
                };
                assert!(matches!(
                    elaboration.node(&make((index + 1) % members.len())),
                    Err(BoundedDenotationError::Certificate(
                        ProofError::DisjunctConclusionMismatch
                    ))
                ));
                let proof = make(index);
                let term = elaboration
                    .node(&proof)
                    .expect("the tag path selects this exact member");
                let mut denoted = elaboration.finish(term);
                assert_eq!(denoted.assumptions.len(), 1);
                assert_eq!(denoted.assumptions[0].index, 0);
                assert_eq!(denoted.assumptions[0].proposition, assumptions[0]);
                verify_mathematical_certificate(
                    &mut denoted.arena,
                    &denoted.certificate,
                    &mut budget(),
                )
                .expect("the full tagged proof checks against the original disjunction");
            }
        }

        #[test]
        fn deep_closed_inhabitants_use_default_stack_and_refuse_false_tail() {
            std::thread::spawn(|| {
                use super::super::Denotation;
                use crate::mathematical_core::{Context, infer_sort};
                for (depth, valid) in [(8192, false), (8192, true), (64, true)] {
                    let mut denotation = Denotation::new();
                    let truth = denotation.denote(&Proposition::Truth).unwrap();
                    let mut goal = if valid {
                        truth
                    } else {
                        denotation.denote(&Proposition::Falsehood).unwrap()
                    };
                    for _ in 0..depth {
                        goal = denotation.arena.insert(Term::Sigma {
                            domain: truth,
                            codomain: goal,
                        });
                    }
                    infer_sort(
                        &mut denotation.arena,
                        &Context::empty(),
                        goal,
                        &mut Budget::new(2_000_000),
                    )
                    .expect("the complete independently formed goal");
                    eprintln!(
                        "{depth}-node Sigma goal formed; begin closed inhabitant valid={valid}"
                    );
                    let evidence = denotation.closed_inhabitant(goal).unwrap();
                    eprintln!("closed inhabitant returned valid={valid}");
                    if valid {
                        let mut evidence =
                            evidence.expect("every closed true conjunct is inhabited");
                        if depth == 64 {
                            let certificate = crate::MathematicalCertificate::with_signature(
                                denotation.declarations.clone(),
                                Vec::new(),
                                evidence,
                                goal,
                            );
                            verify_mathematical_certificate(
                                &mut denotation.arena,
                                &certificate,
                                &mut Budget::new(2_000_000),
                            )
                            .expect("independent kernel checks the entire bounded control");
                        }
                        for _ in 0..depth {
                            let Term::Pair { first, second } = denotation.arena.get(evidence)
                            else {
                                panic!("one evidence pair per Sigma member");
                            };
                            let Term::Refl { ty, value } = denotation.arena.get(first) else {
                                panic!("every closed Truth member retains reflexive evidence");
                            };
                            assert_eq!(ty, denotation.two);
                            assert_eq!(value, denotation.two_zero);
                            evidence = second;
                        }
                        assert!(matches!(denotation.arena.get(evidence), Term::Refl { .. }));
                    } else {
                        assert!(
                            evidence.is_none(),
                            "the final false conjunct has no inhabitant"
                        );
                    }
                    assert!(
                        denotation.declarations.is_empty(),
                        "no opaque axiom closes the goal"
                    );
                }
            })
            .join()
            .expect("closed inhabitant construction stays on a default stack");
        }

        #[test]
        fn closed_inhabitants_keep_tag_preference_and_dependent_refusals() {
            use super::super::Denotation;
            let mut d = Denotation::new();
            let truth = d.denote(&Proposition::Truth).unwrap();
            let falsehood = d.denote(&Proposition::Falsehood).unwrap();
            let bound = d.arena.insert(Term::Variable(0));
            let dependent = d.arena.insert(Term::Id {
                ty: d.two,
                left: bound,
                right: bound,
            });
            for (zero, one, expected) in [
                (truth, truth, Some(false)),
                (falsehood, truth, Some(true)),
                (falsehood, falsehood, None),
                (dependent, truth, Some(true)),
                (truth, dependent, Some(false)),
                (dependent, dependent, None),
            ] {
                let goal = d.tagged_sum(zero, one);
                crate::mathematical_core::infer_sort(
                    &mut d.arena,
                    &crate::mathematical_core::Context::empty(),
                    goal,
                    &mut budget(),
                )
                .expect("each independent or dependent branch type forms under its tag binder");
                let value = d.closed_inhabitant(goal).unwrap();
                match expected {
                    Some(one) => {
                        let term = value.expect("an independent branch has a closed witness");
                        let Term::Pair { first, .. } = d.arena.get(term) else {
                            panic!("sum witness retains a tag");
                        };
                        assert_eq!(matches!(d.arena.get(first), Term::TwoOne), one);
                        let certificate =
                            crate::MathematicalCertificate::closed(Vec::new(), term, goal);
                        verify_mathematical_certificate(&mut d.arena, &certificate, &mut budget())
                            .expect("selected payload checks against the original sum type");
                    }
                    None => assert!(
                        value.is_none(),
                        "false or dependent branches are not closed by this producer"
                    ),
                }
            }
            for goal in [
                d.arena.insert(Term::Pi {
                    domain: d.two,
                    codomain: dependent,
                }),
                d.arena.insert(Term::Sigma {
                    domain: d.two,
                    codomain: dependent,
                }),
            ] {
                assert!(
                    d.closed_inhabitant(goal).unwrap().is_none(),
                    "genuinely dependent families keep their fallback"
                );
            }
            let goal = d.arena.insert(Term::Pi {
                domain: d.two,
                codomain: truth,
            });
            let term = d
                .closed_inhabitant(goal)
                .unwrap()
                .expect("nondependent function abstracts a closed body");
            let certificate = crate::MathematicalCertificate::closed(Vec::new(), term, goal);
            verify_mathematical_certificate(&mut d.arena, &certificate, &mut budget()).unwrap();
            assert!(d.declarations.is_empty());
        }

        #[test]
        fn free_variable_scan_preserves_binder_indices_and_overflow() {
            use super::super::Denotation;
            let mut d = Denotation::new();
            let bound = d.arena.insert(Term::Variable(0));
            let ambient = d.arena.insert(Term::Variable(1));
            let largest = d.arena.insert(Term::Variable(u32::MAX));
            for binder in [
                Term::Lambda {
                    domain: d.two,
                    body: bound,
                },
                Term::Pi {
                    domain: d.two,
                    codomain: bound,
                },
                Term::Sigma {
                    domain: d.two,
                    codomain: bound,
                },
            ] {
                let term = d.arena.insert(binder);
                assert!(
                    !d.occurs_free(term, 0),
                    "the innermost variable is locally bound"
                );
            }
            for binder in [
                Term::Lambda {
                    domain: d.two,
                    body: ambient,
                },
                Term::Pi {
                    domain: d.two,
                    codomain: ambient,
                },
                Term::Sigma {
                    domain: d.two,
                    codomain: ambient,
                },
            ] {
                let term = d.arena.insert(binder);
                assert!(
                    d.occurs_free(term, 0),
                    "an outer variable survives one binder"
                );
            }
            let term = d.arena.insert(Term::Lambda {
                domain: bound,
                body: d.two,
            });
            assert!(d.occurs_free(term, 0), "the domain is outside the binder");
            let term = d.arena.insert(Term::Lambda {
                domain: d.two,
                body: largest,
            });
            assert!(
                !d.occurs_free(term, u32::MAX),
                "overflowing body indices stay ineligible"
            );
            let term = d.arena.insert(Term::Lambda {
                domain: largest,
                body: d.two,
            });
            assert!(
                d.occurs_free(term, u32::MAX),
                "overflow does not skip the unshifted domain"
            );
        }

        #[test]
        fn nested_logical_denotation_and_exact_cache_identity_use_default_stack() {
            std::thread::spawn(|| {
                use super::super::Denotation;
                use crate::mathematical_core::{Context, infer_sort};
                fn nested(last: Proposition) -> Proposition {
                    let mut proposition = last;
                    for depth in 0..8192 {
                        proposition = match depth % 3 {
                            0 => Proposition::Implication {
                                premise: Box::new(Proposition::Truth),
                                conclusion: Box::new(proposition),
                            },
                            1 => Proposition::Conjunction(vec![Proposition::Truth, proposition]),
                            _ => {
                                Proposition::Disjunction(vec![Proposition::Falsehood, proposition])
                            }
                        };
                    }
                    proposition
                }
                let proposition = nested(Proposition::Truth);
                PropositionContext::default()
                    .validate(&proposition)
                    .expect("the whole logical proposition is admitted");
                eprintln!("8192 logical nodes admitted; begin denotation");
                let mut d = Denotation::new();
                let term = d
                    .denote(&proposition)
                    .expect("the complete logical tree denotes");
                eprintln!("logical denotation returned; begin formation and exact cache checks");
                infer_sort(
                    &mut d.arena,
                    &Context::empty(),
                    term,
                    &mut Budget::new(2_000_000),
                )
                .expect("the complete denoted logical type independently forms");
                let duplicate = nested(Proposition::Truth);
                let allocated = d.arena.len();
                assert_eq!(
                    d.denote(&duplicate).unwrap(),
                    term,
                    "equal syntax in distinct source storage reuses only its own denotation"
                );
                assert_eq!(
                    d.arena.len(),
                    allocated,
                    "a cache hit does not repeat term publication"
                );
                let different = nested(Proposition::Falsehood);
                let changed = d.denote(&different).unwrap();
                assert_ne!(
                    changed, term,
                    "one changed leaf cannot reuse the old complete result"
                );
                assert!(
                    d.declarations.is_empty(),
                    "logical structure adds no assumption authority"
                );
                eprintln!("exact cache checks returned; begin ordinary owned teardown");
            })
            .join()
            .expect("logical denotation, cache and owned teardown stay on a default stack");
        }

        #[test]
        fn logical_cache_keeps_exact_syntax_when_leaf_denotations_agree() {
            use super::super::Denotation;
            let fixed = Proposition::Equal(literal(2), literal(2));
            let lifted = crate::lift_fixed_integer_relation(&fixed)
                .expect("fixed integer equality has its mathematical spelling");
            let first = Proposition::Implication {
                premise: Box::new(fixed),
                conclusion: Box::new(Proposition::Truth),
            };
            let second = Proposition::Implication {
                premise: Box::new(lifted),
                conclusion: Box::new(Proposition::Truth),
            };
            let mut d = Denotation::new();
            let left = d.denote(&first).unwrap();
            let right = d.denote(&second).unwrap();
            assert_ne!(
                left, right,
                "different source syntax keeps distinct complete cache entries"
            );
            let Term::Pi {
                domain: left_domain,
                codomain: left_body,
            } = d.arena.get(left)
            else {
                panic!("the first implication denotes Pi");
            };
            let Term::Pi {
                domain: right_domain,
                codomain: right_body,
            } = d.arena.get(right)
            else {
                panic!("the second implication denotes Pi");
            };
            assert_eq!(
                left_domain, right_domain,
                "the leaves have the same denotation despite different syntax"
            );
            assert_eq!(left_body, right_body);
            for term in [left, right] {
                let certificate = crate::MathematicalCertificate::with_signature(
                    d.declarations.clone(),
                    Vec::new(),
                    term,
                    d.type_zero,
                );
                verify_mathematical_certificate(&mut d.arena, &certificate, &mut budget()).unwrap();
            }
            let allocated = d.arena.len();
            assert_eq!(d.denote(&first).unwrap(), left);
            assert_eq!(d.denote(&second).unwrap(), right);
            assert_eq!(d.arena.len(), allocated);
        }

        #[test]
        fn logical_cache_preserves_arity_and_first_failure_effects() {
            use super::super::Denotation;
            for proposition in [
                Proposition::Conjunction(Vec::new()),
                Proposition::Conjunction(vec![atom(900)]),
                Proposition::Disjunction(Vec::new()),
                Proposition::Disjunction(vec![atom(900)]),
            ] {
                let mut d = Denotation::new();
                let allocated = d.arena.len();
                assert!(d.denote(&proposition).is_err());
                assert_eq!(
                    d.arena.len(),
                    allocated,
                    "invalid arity refuses before any member term"
                );
                assert!(d.declarations.is_empty());
            }
            for disjunctive in [false, true] {
                let first = atom(901);
                let untouched = atom(902);
                let invalid_member = atom(903);
                let invalid = Proposition::Disjunction(vec![invalid_member.clone()]);
                let members = vec![first.clone(), invalid, untouched.clone()];
                let proposition = if disjunctive {
                    Proposition::Disjunction(members)
                } else {
                    Proposition::Conjunction(members)
                };
                let mut d = Denotation::new();
                assert!(d.denote(&proposition).is_err());
                assert_eq!(
                    d.declarations.len(),
                    1,
                    "only the first completed atomic member declares"
                );
                assert!(d.denotations.cached_atom(&first).is_some());
                assert!(
                    d.denotations.cached_atom(&untouched).is_none(),
                    "preparing syntax never publishes a later member"
                );
                assert!(
                    d.denotations.cached_atom(&invalid_member).is_none(),
                    "bad arity does not inspect its own members"
                );
            }
        }
    }
    mod content_algebra {
        use super::*;
        use crate::mathematical_core::{Level, Sort};
        use crate::proof_node;

        fn content_algebra() -> ContentAlgebra {
            ContentAlgebra {
                kind: ContentAlgebraKind::CountedQuantity,
                parameter: "Byte".to_owned(),
            }
        }

        fn content_root() -> PlaceId {
            PlaceId::new(1).expect("place")
        }

        fn content_context(root: PlaceId) -> PropositionContext {
            PropositionContext::from_value_types_and_places(
                [],
                [(root, StructuralPlaceKind::Parameter { position: 0 })],
            )
            .expect("content context")
        }

        fn content_term(root: PlaceId, field: &str) -> ContentTerm {
            ContentTerm::Projection {
                projection: ContentProjectionIdentity {
                    domain: ContentDomainId::new(2).expect("domain"),
                    projection_report_fingerprint: 3,
                },
                subject: ContentStructuralPlace {
                    version: ContentPlaceVersion::Current,
                    root,
                    segments: vec![ContentPlaceSegment::Field(field.to_owned())],
                },
            }
        }

        fn conservation(
            algebra: &ContentAlgebra,
            left: &ContentTerm,
            right: &ContentTerm,
        ) -> Proposition {
            Proposition::ContentConservation(ContentConservation::new(
                algebra.clone(),
                left.clone(),
                right.clone(),
            ))
        }

        fn assert_content_carrier_vocabulary(denoted: &BoundedDenotation) {
            let parameter = &denoted.certificate.signature[0];
            assert_eq!(parameter.level_arity, 0);
            assert!(parameter.is_assumption());
            assert_eq!(
                denoted.arena.get(parameter.ty),
                Term::Sort(Sort::Type(Level::Constant(0)))
            );
            let family = &denoted.certificate.signature[1];
            assert_eq!(family.level_arity, 1);
            assert!(family.is_assumption());
            let Term::Pi { domain, codomain } = denoted.arena.get(family.ty) else {
                panic!("the canonical carrier is an outer type family");
            };
            assert_eq!(
                denoted.arena.get(domain),
                Term::Sort(Sort::Type(Level::Parameter(0)))
            );
            assert_eq!(
                denoted.arena.get(codomain),
                Term::Sort(Sort::Type(Level::Constant(0)))
            );
        }

        /// `ContentConservation` denotes `Id` over its algebra's carrier, so
        /// its transitivity rule is a `J` composition the kernel re-decides
        /// in every shared-endpoint orientation the bounded relation check
        /// licenses — never an interned rule axiom.
        #[test]
        fn content_conservation_transitivity_is_a_j_composition() {
            let root = content_root();
            let algebra = content_algebra();
            let terms = |field: &str| content_term(root, field);
            let context = content_context(root);
            for (first_pair, second_pair, goal_pair) in [
                (("a", "b"), ("b", "c"), ("a", "c")),
                (("a", "c"), ("b", "c"), ("a", "b")),
                (("a", "c"), ("a", "b"), ("b", "c")),
                (("b", "c"), ("a", "b"), ("a", "c")),
            ] {
                let first = conservation(&algebra, &terms(first_pair.0), &terms(first_pair.1));
                let second = conservation(&algebra, &terms(second_pair.0), &terms(second_pair.1));
                let goal = conservation(&algebra, &terms(goal_pair.0), &terms(goal_pair.1));
                let proof = ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::EqualityTransitivity {
                        left_equals_middle: Box::new(semantic_axiom_proof(first.clone(), 0)),
                        middle_equals_right: Box::new(semantic_axiom_proof(second.clone(), 1)),
                    },
                };
                let denoted = verify_bounded_certificate(
                    &context,
                    &goal,
                    &[],
                    &[first, second],
                    &proof,
                    &mut budget(),
                )
                .unwrap_or_else(|error| {
                    panic!("content transitivity {goal_pair:?} verifies as J: {error}")
                });
                // `{P, C, a', b', c'}` — the parameter, carrier family and one element
                // constant per distinct content term; the rule cites no
                // axiom of its own.
                assert_content_carrier_vocabulary(&denoted);
                assert_eq!(denoted.certificate.signature.len(), 5);
                assert!(
                    denoted
                        .certificate
                        .signature
                        .iter()
                        .all(|declaration| declaration.is_assumption()),
                );
                assert!(
                    matches!(
                        denoted.arena.get(denoted.certificate.term),
                        Term::IdElim { .. }
                    ),
                    "content transitivity {goal_pair:?} composes by J",
                );
                assert_eq!(
                    certificate_assumption_closure(&denoted.arena, &denoted.certificate),
                    BTreeSet::from([0, 1, 2, 3, 4]),
                );
                assert_eq!(
                    denoted.rules,
                    vec![
                        AcceptedProofRule::SemanticAxiom,
                        AcceptedProofRule::EqualityTransitivity
                    ],
                );
                assert_eq!(denoted.semantic_axioms.len(), 2);
            }
        }

        #[test]
        fn reflexive_content_conservation_is_refl_in_the_kernel() {
            let root = content_root();
            let algebra = content_algebra();
            let context = content_context(root);
            let separate =
                ContentTerm::separate([content_term(root, "a"), content_term(root, "b")])
                    .expect("separation");
            let proposition = conservation(&algebra, &separate, &separate);
            let proof = proof_node!(proposition.clone() => ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality));
            let denoted =
                verify_bounded_certificate(&context, &proposition, &[], &[], &proof, &mut budget())
                    .expect("reflexive conservation verifies as refl");
            // `{P, C, s'}` — the parameter, carrier family and one endpoint;
            // the evidence is `refl (C P) s'` with no decision assumption.
            assert_content_carrier_vocabulary(&denoted);
            assert_eq!(denoted.certificate.signature.len(), 3);
            assert!(matches!(
                denoted.arena.get(denoted.certificate.term),
                Term::Refl { .. }
            ));
            assert_eq!(
                certificate_assumption_closure(&denoted.arena, &denoted.certificate),
                BTreeSet::from([0, 1, 2]),
            );
        }

        #[test]
        fn content_conservation_transitivity_rejects_at_the_shared_check() {
            let root = content_root();
            let algebra = content_algebra();
            let terms = |field: &str| content_term(root, field);
            let context = content_context(root);
            let transitivity =
                |first: Proposition, second: Proposition, goal: Proposition| ProofNode {
                    conclusion: goal,
                    rule: ProofRule::EqualityTransitivity {
                        left_equals_middle: Box::new(semantic_axiom_proof(first, 0)),
                        middle_equals_right: Box::new(semantic_axiom_proof(second, 1)),
                    },
                };

            let other = ContentAlgebra {
                kind: ContentAlgebraKind::CountedQuantity,
                parameter: "Gram".to_owned(),
            };
            let first = conservation(&algebra, &terms("a"), &terms("b"));
            let second = conservation(&other, &terms("b"), &terms("c"));
            let goal = conservation(&algebra, &terms("a"), &terms("c"));
            let proof = transitivity(first.clone(), second.clone(), goal.clone());
            assert!(matches!(
                denote_bounded_certificate(&context, &goal, &[], &[first, second], &proof),
                Err(BoundedDenotationError::Certificate(
                    ProofError::EqualityAlgebraMismatch
                )),
            ));

            let first = conservation(&algebra, &terms("a"), &terms("b"));
            let second = conservation(&algebra, &terms("c"), &terms("d"));
            let proof = transitivity(first.clone(), second.clone(), goal.clone());
            assert!(matches!(
                denote_bounded_certificate(&context, &goal, &[], &[first, second], &proof),
                Err(BoundedDenotationError::Certificate(
                    ProofError::EqualityMiddleMismatch
                )),
            ));

            let first = conservation(&algebra, &terms("a"), &terms("b"));
            let second = conservation(&algebra, &terms("b"), &terms("c"));
            let proof = transitivity(first.clone(), second.clone(), first.clone());
            assert!(matches!(
                denote_bounded_certificate(&context, &first.clone(), &[], &[first, second], &proof),
                Err(BoundedDenotationError::Certificate(
                    ProofError::EqualityConclusionMismatch
                )),
            ));
        }

        #[test]
        fn unchanged_content_transport_reuses_evidence_and_retains_its_context() {
            let root = content_root();
            let algebra = content_algebra();
            let premise =
                conservation(&algebra, &content_term(root, "a"), &content_term(root, "b"));
            let (x_id, x) = value(1);
            let (z_id, z) = value(3);
            let equation = Proposition::Equal(x, z);
            let context = PropositionContext::from_value_types_and_places(
                [(x_id, unsigned64_type()), (z_id, unsigned64_type())],
                [(root, StructuralPlaceKind::Parameter { position: 0 })],
            )
            .expect("context");
            let proof = ProofNode {
                conclusion: premise.clone(),
                rule: ProofRule::ValueEqualityTransport {
                    premise: Box::new(semantic_axiom_proof(premise.clone(), 0)),
                    equalities: vec![semantic_axiom_proof(equation.clone(), 1)],
                },
            };
            let denoted = verify_bounded_certificate(
                &context,
                &premise,
                &[],
                &[premise.clone(), equation],
                &proof,
                &mut budget(),
            )
            .expect("the original content evidence verifies");
            // P, C, a', b', Int, m_x and m_z remain; no new rule axiom.
            assert_content_carrier_vocabulary(&denoted);
            assert_eq!(denoted.certificate.signature.len(), 7);
            assert_eq!(
                denoted.arena.get(denoted.certificate.term),
                Term::Variable(1)
            );
            assert_eq!(
                certificate_assumption_closure(&denoted.arena, &denoted.certificate),
                (0..7).collect()
            );
            assert!(
                matches!(
                    denoted.arena.get(denoted.certificate.context[0]),
                    Term::Id { .. }
                ),
                "the first premise is the content identity",
            );
            assert_eq!(
                denoted.rules,
                vec![
                    AcceptedProofRule::SemanticAxiom,
                    AcceptedProofRule::ValueEqualityTransport
                ],
            );
        }
    }

    #[test]
    fn closed_primitive_order_does_not_assume_its_conclusion() {
        let goal = Proposition::LessOrEqual(literal(3), literal(5));
        let proof = proof_node!(goal.clone() => ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation));
        let context = PropositionContext::default();
        let parameters = BTreeSet::new();
        let mut elaboration = Elaboration::new(&context, &goal, &[], &[], &parameters).unwrap();
        let term = elaboration.node(&proof).unwrap();
        assert!(
            elaboration.denotation.decisions.is_empty(),
            "closed order must derive from numeral laws"
        );
        let mut denoted = elaboration.finish(term);
        verify_mathematical_certificate(&mut denoted.arena, &denoted.certificate, &mut budget())
            .unwrap();
    }

    #[test]
    fn false_closed_order_is_rejected_before_derivation() {
        for goal in [
            Proposition::LessThan(literal(2), literal(1)),
            Proposition::LessOrEqual(literal(6), literal(5)),
        ] {
            let proof = proof_node!(goal.clone() => ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation));
            assert!(matches!(
                denote_bounded_certificate(&PropositionContext::default(), &goal, &[], &[], &proof),
                Err(BoundedDenotationError::Certificate(_))
            ));
        }
    }
}

#[cfg(test)]
pub(crate) mod test_utilities {
    use super::{BoundedDenotation, Denotation, Elaboration};
    use crate::proof::assumption_proof;
    use crate::{Budget, ProofNode, ProofRule, affine_witness, verify_bounded_certificate};
    use semantic_vocabulary::{
        IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext,
        PropositionError, ScalarTerm, ScalarType, ValueId,
    };
    use std::collections::BTreeSet;
    use std::slice::from_ref;

    macro_rules! v {
        ($a:expr => $b:expr => $c:expr => $d:expr => $e:expr => $f:expr) => {
            verify_bounded_certificate($a, $b, $c, $d, $e, $f)
        };
    }

    pub(crate) fn denote_via_assumption_premise(
        context: &PropositionContext,
        goal: &Proposition,
        premise: &Proposition,
        message: &str,
    ) -> BoundedDenotation {
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::PredicateDenotation {
                premise: Box::new(assumption_proof(premise.clone(), 0)),
            },
        };
        v!(context => goal => from_ref(premise) => &[] => &proof => &mut Budget::default())
            .expect(message)
    }

    pub(crate) fn mathematical(term: &ScalarTerm) -> IntegerMathTerm {
        match term {
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(source_type),
            } => IntegerMathTerm::MathValue {
                source_type: *source_type,
                value: *id,
            },
            ScalarTerm::Integer { value, .. } => IntegerMathTerm::literal(*value),
            _ => unreachable!(),
        }
    }

    pub(crate) fn open_operation_preserves_skipped_numeric_resource_refusals(
        operation: fn(Box<IntegerMathTerm>, Box<IntegerMathTerm>) -> IntegerMathTerm,
    ) {
        let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let value = ValueId::new(1).unwrap();
        let context =
            PropositionContext::from_value_types([(value, ScalarType::Integer(integer))]).unwrap();
        let large = IntegerMathTerm::ShiftLeft {
            value: Box::new(IntegerMathTerm::literal(IntegerValue::Unsigned(1))),
            count: Box::new(IntegerMathTerm::literal(IntegerValue::Unsigned(65_536))),
        };
        let open = operation(
            Box::new(IntegerMathTerm::MathValue {
                source_type: integer,
                value,
            }),
            Box::new(large.clone()),
        );
        let goal = Proposition::IntegerMathEqual(open.clone(), open);
        let proof = assumption_proof(goal.clone(), 0);
        v!(&context => &goal => from_ref(&goal) => &[] => &proof => &mut Budget::default())
            .unwrap();
        let closed_goal = Proposition::IntegerMathEqual(large.clone(), large);
        let closed_proof = assumption_proof(closed_goal.clone(), 0);
        assert_refused(
            &context,
            &closed_goal,
            from_ref(&closed_goal),
            &[],
            &closed_proof,
        );
    }

    pub(super) fn direct_fixture(
        context: &PropositionContext,
        goal: &Proposition,
        premise: &Proposition,
        proof: &ProofNode,
    ) -> Denotation {
        v!(context => goal => from_ref(premise) => &[] => proof => &mut Budget::default()).unwrap();
        let parameters = BTreeSet::new();
        let assumptions = [premise.clone()];
        let mut elaboration =
            Elaboration::new(context, goal, &assumptions, &[], &parameters).unwrap();
        elaboration.node(proof).unwrap();
        elaboration.denotation
    }

    pub(crate) fn direct_affine_proof(
        integer: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
        premise: &Proposition,
        goal: &Proposition,
        operation: fn(IntegerType, ScalarTerm, ScalarTerm) -> Result<ScalarTerm, PropositionError>,
    ) -> ProofNode {
        ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::IntegerAffineBound {
                root_bound: Box::new(assumption_proof(premise.clone(), 0)),
                witness: affine_witness(
                    left.clone(),
                    operation(integer, left, right).unwrap(),
                    Vec::new(),
                    Vec::new(),
                ),
            },
        }
    }

    pub(super) fn verify_then_elaborate<'a>(
        context: &'a PropositionContext,
        goal: &Proposition,
        assumptions: &'a [Proposition],
        machine_parameter_values: &'a BTreeSet<ValueId>,
        proof: &ProofNode,
    ) -> Elaboration<'a> {
        v!(context => goal => assumptions => &[] => proof => &mut Budget::default()).unwrap();
        elaborate_checked(
            context,
            goal,
            assumptions,
            &[],
            machine_parameter_values,
            proof,
        )
    }

    pub(super) fn elaborate_checked<'a>(
        context: &'a PropositionContext,
        goal: &Proposition,
        assumptions: &'a [Proposition],
        semantic_axioms: &'a [Proposition],
        machine_parameter_values: &'a BTreeSet<ValueId>,
        proof: &ProofNode,
    ) -> Elaboration<'a> {
        let mut elaboration = Elaboration::new(
            context,
            goal,
            assumptions,
            semantic_axioms,
            machine_parameter_values,
        )
        .unwrap();
        elaboration.node(proof).unwrap();
        elaboration
    }

    pub(crate) fn assert_refused(
        context: &PropositionContext,
        goal: &Proposition,
        assumptions: &[Proposition],
        semantic_axioms: &[Proposition],
        proof: &ProofNode,
    ) {
        assert!(
        v!(context => goal => assumptions => semantic_axioms => proof => &mut Budget::default())
            .is_err()
    );
    }

    pub(crate) fn verify_and_assert_forged(
        context: &PropositionContext,
        goal: &Proposition,
        assumptions: &[Proposition],
        semantic_axioms: &[Proposition],
        proof: &ProofNode,
    ) -> BoundedDenotation {
        let mut denoted = verify_bounded_certificate(
            context,
            goal,
            assumptions,
            semantic_axioms,
            proof,
            &mut Budget::default(),
        )
        .unwrap();
        crate::forge_term_and_assert_rejected(&mut denoted);
        denoted
    }
}
