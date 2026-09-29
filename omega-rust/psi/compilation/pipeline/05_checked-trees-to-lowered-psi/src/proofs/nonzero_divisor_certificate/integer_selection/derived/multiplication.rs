//! Carry a prior product's tight endpoints into later arithmetic.
//!
//! For exact products, the four-corner rule bounds the mathematical product, not its
//! stored scalar result. Transport through the cited prior definition, then
//! use the kernel's existing scalar/math citation normalization under a
//! discharged hypothesis. Every step is ordinary published proof evidence;
//! neither a new checker rule nor a producer-asserted interval is required.
//! Wrapping products use the same four corners, but admission requires the
//! complete rectangle to fit the carrier and yields a scalar bound. Transport
//! that bound directly; wrapping expressions do not unconditionally equal the
//! unbounded mathematical product.

use proof_admission::{
    IntegerAffineWitness, PrimitiveJudgment, ProofNode, ProofRule, check_integer_affine_witness,
    map_integer_affine_bound,
};
use semantic_vocabulary::{
    IntegerMathTerm, IntegerType, Proposition, PropositionContext, ScalarTerm,
};

use super::{evidence_literal, oriented};

pub(super) fn definition_bounds(
    context: &PropositionContext,
    assumption_count: usize,
    semantic_axioms: &[Proposition],
    definition_axiom: usize,
    output: &ScalarTerm,
    expression: &ScalarTerm,
    proofs: &[ProofNode],
    discovered: &mut Vec<ProofNode>,
) {
    let (ScalarTerm::ExactIntegerMultiply {
        scalar_type,
        left,
        right,
    }
    | ScalarTerm::WrappingIntegerMultiply {
        scalar_type,
        left,
        right,
    }) = expression
    else {
        return;
    };
    let ScalarTerm::Value { id, .. } = output else {
        return;
    };
    let endpoints = [
        (&**left, true),
        (&**left, false),
        (&**right, true),
        (&**right, false),
    ]
    .map(|(operand, lower)| strongest_endpoint(*scalar_type, operand, lower, proofs));
    let [
        Some(left_lower),
        Some(left_upper),
        Some(right_lower),
        Some(right_upper),
    ] = endpoints
    else {
        return;
    };
    let witness = IntegerAffineWitness {
        root: (**left).clone(),
        target: expression.clone(),
        definition_axioms: Vec::new(),
        literal_axioms: Vec::new(),
    };
    let Ok(form) = check_integer_affine_witness(context, semantic_axioms, &witness) else {
        return;
    };
    let output_math = IntegerMathTerm::MathValue {
        source_type: *scalar_type,
        value: *id,
    };
    for lower in [true, false] {
        let endpoints = if lower {
            [&left_lower, &left_upper, &right_lower, &right_upper]
        } else {
            [&left_upper, &left_lower, &right_upper, &right_lower]
        };
        let evidence = Proposition::Conjunction(
            endpoints
                .iter()
                .map(|proof| proof.conclusion.clone())
                .collect(),
        );
        let Ok(mapped) = map_integer_affine_bound(&form, &evidence) else {
            continue;
        };
        let scalar_literal = match &mapped {
            Proposition::LessOrEqual(first, second) => {
                if lower {
                    first.clone()
                } else {
                    second.clone()
                }
            }
            Proposition::IntegerMathLessOrEqual(first, second) => {
                let IntegerMathTerm::IntegerLiteral(bound) = (if lower { first } else { second })
                else {
                    continue;
                };
                let Some(bound) = bound.as_integer_value(*scalar_type) else {
                    continue;
                };
                let Ok(literal) = ScalarTerm::integer(*scalar_type, bound) else {
                    continue;
                };
                literal
            }
            _ => continue,
        };
        let scalar_bound = if lower {
            Proposition::LessOrEqual(scalar_literal, output.clone())
        } else {
            Proposition::LessOrEqual(output.clone(), scalar_literal)
        };
        if proofs.iter().any(|proof| proof.conclusion == scalar_bound) {
            continue;
        }
        let mut definition = ProofNode {
            conclusion: semantic_axioms[definition_axiom].clone(),
            rule: ProofRule::SemanticAxiom {
                index: definition_axiom,
            },
        };
        if matches!(&definition.conclusion, Proposition::Equal(first, _) if first != output) {
            definition = ProofNode {
                conclusion: Proposition::Equal(output.clone(), expression.clone()),
                rule: ProofRule::EqualitySymmetry {
                    equality: Box::new(definition),
                },
            };
        }
        let mapped_proof = ProofNode {
            conclusion: mapped.clone(),
            rule: ProofRule::IntegerAffineBound {
                root_bound: Box::new(ProofNode {
                    conclusion: evidence,
                    rule: ProofRule::ConjunctionIntroduction(
                        endpoints.into_iter().cloned().collect(),
                    ),
                }),
                witness: witness.clone(),
            },
        };
        if matches!(mapped, Proposition::LessOrEqual(..)) {
            discovered.push(ProofNode {
                conclusion: scalar_bound,
                rule: ProofRule::IntegerOrderSubstitution {
                    relation: Box::new(mapped_proof),
                    equality: Box::new(definition),
                    endpoint: usize::from(lower),
                },
            });
            continue;
        }
        let Proposition::IntegerMathLessOrEqual(mapped_left, mapped_right) = &mapped else {
            continue;
        };
        let (literal, product) = if lower {
            (mapped_left, mapped_right)
        } else {
            (mapped_right, mapped_left)
        };
        let mathematical_bound = if lower {
            Proposition::IntegerMathLessOrEqual(literal.clone(), output_math.clone())
        } else {
            Proposition::IntegerMathLessOrEqual(output_math.clone(), literal.clone())
        };
        let equality = ProofNode {
            conclusion: Proposition::IntegerMathEqual(output_math.clone(), product.clone()),
            rule: ProofRule::ValueEqualityTransport {
                premise: Box::new(ProofNode {
                    conclusion: Proposition::IntegerMathEqual(product.clone(), product.clone()),
                    rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
                }),
                equalities: vec![definition],
            },
        };
        let transported = ProofNode {
            conclusion: mathematical_bound.clone(),
            rule: ProofRule::IntegerOrderSubstitution {
                relation: Box::new(mapped_proof),
                equality: Box::new(equality),
                endpoint: usize::from(lower),
            },
        };
        // Citation normalization equates these leaf relations. Discharge the
        // temporary hypothesis immediately; it adds no ambient assumption and
        // does not use the later operation's result to prove its own safety.
        discovered.push(ProofNode {
            conclusion: scalar_bound.clone(),
            rule: ProofRule::ImplicationElimination {
                implication: Box::new(ProofNode {
                    conclusion: Proposition::Implication {
                        premise: Box::new(mathematical_bound),
                        conclusion: Box::new(scalar_bound.clone()),
                    },
                    rule: ProofRule::ImplicationIntroduction {
                        body: Box::new(ProofNode {
                            conclusion: scalar_bound,
                            rule: ProofRule::Assumption {
                                index: assumption_count,
                            },
                        }),
                    },
                }),
                premise: Box::new(transported),
            },
        });
    }
}

fn strongest_endpoint(
    integer_type: IntegerType,
    operand: &ScalarTerm,
    lower: bool,
    proofs: &[ProofNode],
) -> Option<ProofNode> {
    if operand.integer_value().is_some() {
        return Some(ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
        });
    }
    // Four tight endpoints suffice; enumerating every weaker combination
    // multiplies proof construction without establishing a stronger interval.
    proofs
        .iter()
        .filter(|proof| oriented(proof, operand, integer_type, lower))
        .filter_map(|proof| {
            Some((
                evidence_literal(proof, operand, integer_type, lower)?,
                proof,
            ))
        })
        .reduce(|current, candidate| {
            if (lower && candidate.0 > current.0) || (!lower && candidate.0 < current.0) {
                candidate
            } else {
                current
            }
        })
        .map(|(_, proof)| proof.clone())
}
