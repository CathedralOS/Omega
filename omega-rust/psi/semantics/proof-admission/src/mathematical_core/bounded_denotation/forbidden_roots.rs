//! Checked correlated forbidden-root safety in the shared `Int`
//! vocabulary.
//!
//! `IntegerCorrelatedForbiddenRoots` proves exact signed division's
//! definedness proposition — `d ≤ −2 ∨ 1 ≤ d ∨ (d ≤ −1 ∧ min+1 ≤ n)` —
//! from the two affine definition chains the witness replays over the
//! semantic-axiom ledger and the tight root bounds the signature's own
//! carrier range selects. The original checker runs first; this module
//! denotes the already-checked evidence into a real derivation instead
//! of a per-instance `rule_axiom`.
//!
//! The derivation is a four-way dependent `Two` elimination over one
//! fixed divisor trichotomy `d ≤ −2 ∨ d = −1 ∨ d = 0 ∨ 1 ≤ d` — each
//! case a `Σ`-tagged payload landing exactly where the goal's three-way
//! disjunction wants it:
//!
//! - `d ≤ −2` and `1 ≤ d` introduce the first and second disjuncts
//!   directly.
//! - `d = 0` is impossible over the tight interval: the divisor chain
//!   solves `d = 0` stepwise — `add`/`sub` steps invert through fixed
//!   right-inverse laws and checked numeral-operation equations, `mul`
//!   steps through per-type truncating-division squeeze laws and order
//!   antisymmetry — and either lands on a non-divisible product or on a
//!   root `r = q` outside the cited bounds. Both refute the case by
//!   strict-order irreflexivity and `sEmpty` elimination.
//! - `d = −1` solves the same way. A non-root or out-of-interval root
//!   refutes the bound side by `sEmpty` elimination; an in-interval
//!   root `r = q` transports the dividend chain forward — each step
//!   rewriting the inner operand by the just-derived identity and
//!   evaluating one checked `add`/`sub`/`mul` numeral equation — to
//!   `n = v`. The checker guarantees `v ≠ min` for this case, so
//!   `min < v ≤ max` gives `min+1 ≤ n` by numeral order and
//!   substitution, while `v < min` or `v > max` contradicts the
//!   dividend's interned carrier-membership bound and refutes the same
//!   way.
//!
//! Every leaf assumption is exact: the fixed `Π` laws are audited once,
//! and each `add`/`sub`/`mul`/`div_T` numeral equation is interned only
//! after its arithmetic is rechecked. Anything outside this vocabulary —
//! a malformed step, a shape the checker never produced, an overflow —
//! keeps the existing per-instance fallback.

use std::collections::BTreeMap;

use semantic_vocabulary::{
    IntegerCarrier, IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm,
};

use super::super::scheme_dsl::{self, apps, case_two, id, lam, pi, scheme_at, sigma, sort, two, v};
use super::super::term::Level;
use super::integer_operations::IntegerOperation;
use super::{
    BoundedDenotationError, Declaration, Denotation, IntegerLaw, IntegerRelation, Term, TermHandle,
};
use crate::mathematical_core::substitution::shift;
use crate::{
    CheckedIntegerCorrelatedForbiddenRoots, CorrelatedAffineBranchWitness,
    IntegerCorrelatedForbiddenRootWitness,
};

#[cfg(test)]
mod tests;

/// The fixed integer laws a correlated forbidden-root derivation cites —
/// each an assumption constant with an exact `Π` statement over `Int`,
/// `IntLe`, `Id` and `Two`, audited once like the order roster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Law {
    /// `Π a k m. Id Int (add a k) m → Id Int a (sub m k)` — an `add`
    /// definition step inverts by subtracting its literal.
    AddRightInverse,
    /// `Π a k m. Id Int (sub a k) m → Id Int a (add m k)` — a `sub`
    /// definition step inverts by adding its literal.
    SubtractRightInverse,
    /// `Π k m a. 1 ≤ k → mul a k ≤ m → a ≤ div_T m k` — a positive
    /// literal multiplier keeps the unknown at or under the truncating
    /// quotient.
    QuotientUpperPositive(IntegerType),
    /// `Π k m a. 1 ≤ k → m ≤ mul a k → div_T m k ≤ a` — the same
    /// product keeps the quotient at or under the unknown.
    QuotientLowerPositive(IntegerType),
    /// `Π k m a. k ≤ −1 → mul a k ≤ m → div_T m k ≤ a` — a negative
    /// literal multiplier reverses the bound at the quotient.
    QuotientLowerNegative(IntegerType),
    /// `Π k m a. k ≤ −1 → m ≤ mul a k → a ≤ div_T m k` — the same
    /// product bounds the unknown at or under the quotient.
    QuotientUpperNegative(IntegerType),
    /// `Π a b. a ≤ b → b ≤ a → Id Int a b` — order antisymmetry.
    LessOrEqualAntisymmetry,
    /// `Π d. d ≤ −2 ∨ d = −1 ∨ d = 0 ∨ 1 ≤ d` as the four-way tagged
    /// sum `Σ(t : Two). caseTwo(M, …)` — the integer trichotomy the
    /// conversion's three-way disjunction consumes.
    DivisorCaseSplit,
}

/// One checked affine definition step, denoted: `Equal(current, op(next,
/// sibling))` with `sibling` the step's literal — closed directly or
/// landed by the cited literal axiom.
#[derive(Clone)]
struct ChainStep {
    /// `Id Int current' (op' next' sib')` — the cited definition.
    definition: TermHandle,
    /// `Id Int sib' lit'` — the cited literal equation when the sibling
    /// is a landed value rather than a literal.
    literal: Option<TermHandle>,
    /// The step's operation.
    operation: ChainOperation,
    /// `next'` — the step's inner operand.
    next: TermHandle,
    /// `sib'` — the step's right operand as denoted.
    sibling: TermHandle,
    /// `lit'` — the step's literal numeral.
    literal_term: TermHandle,
    /// The literal's exact checked value.
    literal_value: i128,
    /// `op' next' sib'` — the denoted right-hand side.
    expression: TermHandle,
    /// `current'` — the step's defined value.
    current: TermHandle,
}

/// The step's exact-arithmetic operation: the denoted shared function
/// and its exact `i128` evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChainOperation {
    Add,
    Subtract,
    Multiply,
}

/// The outcome of solving `divisor = c` stepwise through the checked
/// divisor chain.
enum SolveOutcome {
    /// No integer `r` satisfies the equation — an `sEmpty` scrutinee
    /// derived through strict-order irreflexivity.
    Refuted(TermHandle),
    /// `r = q` is the unique root — the transport equality and the
    /// root's exact value.
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

/// The signed literal a fixed integer scalar term carries, or `None` —
/// the checker's own `signed_literal` test on the checked type.
fn signed_literal(term: &ScalarTerm, integer_type: IntegerType) -> Option<i128> {
    match term.integer_value()? {
        (actual, IntegerValue::Signed(value)) if actual == integer_type => Some(value),
        _ => None,
    }
}

impl Denotation {
    /// `law a₁ … aₙ` — one forbidden-roots roster constant applied to
    /// the denoted endpoints and premise evidence, in statement order.
    fn roots_law_application(
        &mut self,
        law: Law,
        arguments: &[TermHandle],
    ) -> Result<TermHandle, BoundedDenotationError> {
        let position = if let Some(&position) = self.forbidden_roots.laws.get(&law) {
            position
        } else {
            let integer = self.integer()?;
            let add = self.add_operation()?;
            let sub = self.subtract_operation()?;
            let mul = self.multiply_operation()?;
            let less_or_equal = self.integer_less_or_equal()?;
            let zero = self.binary_integer(false, 0)?;
            let one = self.binary_integer(false, 1)?;
            let negative_one = self.binary_integer(true, 1)?;
            let negative_two = self.binary_integer(true, 2)?;
            let constant = |position| scheme_at(position, Vec::new());
            let carrier = || constant(integer);
            let le = |left, right| apps(constant(less_or_equal), [left, right]);
            let eq = |left, right| id(carrier(), left, right);
            let add = |left, right| apps(constant(add), [left, right]);
            let sub = |left, right| apps(constant(sub), [left, right]);
            let mul = |left, right| apps(constant(mul), [left, right]);
            let statement = match law {
                Law::AddRightInverse => pi(
                    "a",
                    carrier(),
                    pi(
                        "k",
                        carrier(),
                        pi(
                            "m",
                            carrier(),
                            pi(
                                "_",
                                eq(add(v("a"), v("k")), v("m")),
                                eq(v("a"), sub(v("m"), v("k"))),
                            ),
                        ),
                    ),
                ),
                Law::SubtractRightInverse => pi(
                    "a",
                    carrier(),
                    pi(
                        "k",
                        carrier(),
                        pi(
                            "m",
                            carrier(),
                            pi(
                                "_",
                                eq(sub(v("a"), v("k")), v("m")),
                                eq(v("a"), add(v("m"), v("k"))),
                            ),
                        ),
                    ),
                ),
                Law::QuotientUpperPositive(scalar_type)
                | Law::QuotientLowerPositive(scalar_type)
                | Law::QuotientLowerNegative(scalar_type)
                | Law::QuotientUpperNegative(scalar_type) => {
                    let divide =
                        self.integer_operation(IntegerOperation::ExactDivide(scalar_type))?;
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
                    // `mul a k ≤ m` for the `Quotient{Upper,Lower}…`
                    // variants that bound `a` through `mul a k ≤ m`;
                    // `m ≤ mul a k` for the others.
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
                    pi(
                        "k",
                        carrier(),
                        pi(
                            "m",
                            carrier(),
                            pi("a", carrier(), pi("_", sign, pi("_", bound, residual))),
                        ),
                    )
                }
                Law::LessOrEqualAntisymmetry => pi(
                    "a",
                    carrier(),
                    pi(
                        "b",
                        carrier(),
                        pi(
                            "_",
                            le(v("a"), v("b")),
                            pi("_", le(v("b"), v("a")), eq(v("a"), v("b"))),
                        ),
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
                            case_two(motive(), le(v("d"), constant(negative_two)), middle, v("t")),
                        ),
                    )
                }
            };
            let ty = scheme_dsl::build(&mut self.arena, &mut Vec::new(), &statement);
            let position = self.position()?;
            self.declarations.push(Declaration::assumption(0, ty));
            self.forbidden_roots.laws.insert(law, position);
            position
        };
        let mut function = self.constant(position);
        for &argument in arguments {
            function = self.arena.insert(Term::Apply { function, argument });
        }
        Ok(function)
    }

    /// `IntLe l' r'` — the non-strict order applied to two denoted `Int`
    /// endpoints.
    fn integer_le_term(
        &mut self,
        left: TermHandle,
        right: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let position = self.integer_less_or_equal()?;
        let relation = self.constant(position);
        let function = self.arena.insert(Term::Apply {
            function: relation,
            argument: left,
        });
        Ok(self.arena.insert(Term::Apply {
            function,
            argument: right,
        }))
    }

    /// The canonical numeral for one exact `i128` — the shared signed
    /// binary definition every fixed literal denotes.
    fn integer_numeral(&mut self, value: i128) -> Result<TermHandle, BoundedDenotationError> {
        let position = self.binary_integer(value < 0, value.unsigned_abs())?;
        Ok(self.constant(position))
    }

    /// `op' l' r'` — the denoted shared operation over two `Int`
    /// endpoints.
    fn chain_operation_terms(
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

    /// The exact `i128` evaluation of one chain step's operation, or
    /// `None` on overflow — a defensive refusal the checked witness
    /// never produces.
    fn chain_operation_eval(operation: ChainOperation, left: i128, right: i128) -> Option<i128> {
        match operation {
            ChainOperation::Add => left.checked_add(right),
            ChainOperation::Subtract => left.checked_sub(right),
            ChainOperation::Multiply => left.checked_mul(right),
        }
    }

    /// `J(λ(y : Int). λ(_ : Id Int from y). F y, base, to, proof)` —
    /// dependent identity elimination over `Int` with the family built
    /// under the two binders: `family` receives `y` as a `Variable(1)`
    /// handle and returns the body's type-level shape.
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

    /// `p : Id Int sib' lit'` rewrites the right operand of `op' ·' sib'`
    /// — the congruence step carrying `Id expr' (op' next' sib')` to
    /// `Id expr' (op' next' lit')` through the cited literal equation.
    fn chain_sibling_congruence(
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

    /// `p : Id Int inner' v'` rewrites the left operand of `op' inner'
    /// sib'` — the congruence step carrying `Id u' (op' inner' sib')` to
    /// `Id u' (op' v' sib')` through the solved inner equation. Returns
    /// `Id Int (op' inner' sib') (op' v' sib')`.
    fn chain_operand_congruence(
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

    /// Parse one checked branch's steps into denoted chain data, in the
    /// witness's innermost-first order, consuming `evidence` in the
    /// relation's citation order — literal axiom then definition axiom
    /// per step. Every definition is verified against the chain shape:
    /// `Equal(current, op(next, sibling))`. `None` keeps the instance
    /// fallback for any other shape.
    fn chain_steps(
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
            let Some((definition_proposition, definition_evidence)) = evidence.get(*cursor) else {
                return Ok(None);
            };
            *cursor += 1;
            let Proposition::Equal(current, expression) = definition_proposition else {
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
            let expression_term = self.chain_operation_terms(operation, next_term, sibling_term)?;
            // The cited evidence's own denotation fixes the equation's
            // orientation — `directed_equality` re-syms whichever side
            // canonical `IntegerMathEqual` order flipped.
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
            let (literal_value, literal_term, literal_evidence_term) = match literal_evidence {
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
                    let Some(evidence) =
                        self.directed_equality(from, to, sibling_term, literal_term, evidence)
                    else {
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

    /// One step's evidence lifted under `amount` enclosing λ's: the
    /// variable evidence (definition and literal citations) shifts;
    /// every denoted endpoint is closed so `shift` returns it intact.
    fn chain_step_lift(&mut self, step: &ChainStep, amount: u32) -> ChainStep {
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

    /// Solve `target = c` stepwise through the divisor chain — steps in
    /// outermost-first order — to the root value or an `sEmpty`
    /// refutation. `add`/`sub` steps invert exactly through the
    /// right-inverse laws and a checked numeral-operation equation;
    /// `mul` steps invert through the per-type truncating-division
    /// squeeze and antisymmetry, or refute when the product cannot
    /// divide. `None` keeps the instance fallback on any shape drift.
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
            // `def : Id current' expr'` and `eq : Id current' v'` compose
            // to `Id expr' v'`; a landed sibling rewrites `expr'` to
            // `op' next' lit'` through the cited literal equation.
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
                // `congruence : Id expr' (op next' lit')` — sym before
                // transitivity lands `Id (op next' lit') v'`.
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
                    // `mul a k = m` squeezes `a` to `div_T m k`: both
                    // directions through the sign-selected laws, then
                    // antisymmetry to `Id next' (div_T m' k')`.
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
                        // The product cannot divide: `mul next' k' = m'`
                        // with `next' = q'` gives `mul q' k' = m'`, but
                        // `mul q' k' = q·k ≠ m` — the numeral equation
                        // collapses to `IntLt m' m'` and `sEmpty`.
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

    /// `mul q' k' = m'` with `q·k ≠ m` collapses to `IntLt m' m'`: the
    /// checked numeral product `mul q' k' = q·k'` and the transported
    /// chain equation name two different numerals, so strict order
    /// substitutes to the diagonal and `irreflexive` refutes it.
    /// Returns the `sEmpty` scrutinee.
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
        // Transport `bound : Id (mul next' k') m'` along `next' = q'`.
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
        // `Id m' product'` — the two numerals the equation identifies.
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

    /// Evaluate the dividend chain forward at the solved root `r = q`:
    /// each step rewrites the inner operand by the just-derived
    /// identity, then lands the step's `op` numeral equation. Returns
    /// the final `Id dividend' v'` with `v` the stepwise value — which
    /// the checked affine form's own evaluation confirms.
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
            // `def : Id u' (op' inner' sib')` and `eq : Id inner' v'`
            // rewrite the operand to `op' v' sib'`; a landed sibling
            // then rewrites `sib'` to `lit'` through its cited equation.
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

    /// `IntLt m' m'` evidence collapses the branch — `irreflexive`
    /// gives the `sEmpty` scrutinee and `EmptyElim` any target.
    fn empty_elimination(&mut self, ty: TermHandle, scrutinee: TermHandle) -> TermHandle {
        self.arena.insert(Term::EmptyElim { ty, scrutinee })
    }

    /// `sEmpty` evidence for `r = q` against the cited tight bounds when
    /// `q` falls outside `[l, u]`: the strict numeral order composes
    /// with the transported bound to `IntLt l' l'` or `IntLt u' u'`.
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
            let strict =
                self.literal_order(IntegerValue::Signed(quotient), IntegerValue::Signed(lower))?;
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
            let strict =
                self.literal_order(IntegerValue::Signed(upper), IntegerValue::Signed(quotient))?;
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

    /// `caseTwo(λ(t : Two). Π(_ : F t). G, g0, g1, fst s) (snd s)` —
    /// one dependent elimination level over a denoted tagged sum
    /// `Σ(t : Two). caseTwo(M, zero_case, rest_case, t)`. The branches
    /// are λ-terms valid at the caller's level; `scrutinee` is the
    /// Σ-value at that level.
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

    /// `⟨zero, p⟩`, `⟨one, ⟨zero, p⟩⟩` or `⟨one, ⟨one, p⟩⟩` — the
    /// tagged-sum introduction landing `p` at `index` of the goal's
    /// three-way disjunction.
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

    /// The `d = −1` case's bound evidence `IntLe min+1' n'`: `sEmpty`
    /// elimination when the divisor cannot hit `−1` inside the
    /// interval; the forward dividend evaluation `n = v` with
    /// `min < v ≤ max` landing the numeral bound when it does; carrier
    /// membership on `n'` refuting `v ∉ [min, max]`.
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
                // The checked conversion guarantees `v ≠ minimum` for
                // this case — equality would be a forbidden root.
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
                    let flipped = self.symmetry(integer, dividend_term, evaluated_term, evaluation);
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
                    // `v` outside the carrier contradicts the dividend's
                    // interned membership bound through the evaluation.
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

    /// Elaborate a checked `IntegerCorrelatedForbiddenRoots` instance
    /// into the four-way case split above. `axiom_evidence` is the
    /// relation's citation list — dividend branch then divisor branch,
    /// literal axiom before each definition axiom — and `bound_evidence`
    /// the `[lower, upper]` tight-bound premises it records. `None`
    /// keeps the per-instance `rule_axiom` fallback for any shape this
    /// fixed vocabulary does not name.
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

        // Parse both branches in citation order; the parse re-verifies
        // each cited equation's shape and orientation.
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
        if cursor != axiom_evidence.len() || dividend_steps.is_empty() || divisor_steps.is_empty() {
            return Ok(None);
        }
        // The chain structure the checker enforced: the innermost step
        // reads the root, each step's `next` is the previous step's
        // `current`, and the outermost `current` is the branch target.
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

        // The two cited bound premises: `IntLe l' r'` and `IntLe r' u'`
        // naming the exact interval the checker selected.
        let [lower_bound, upper_bound] = bound_evidence else {
            return Ok(None);
        };
        let mut bound_side = |proposition: &Proposition,
                              interval_value: i128,
                              root_on_right: bool|
         -> Result<Option<TermHandle>, BoundedDenotationError> {
            let denoted = self.denote(proposition)?;
            let Some(IntegerRelation::LessOrEqual { left, right }) = self.integer_relation(denoted)
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

        // The conversion's three-way disjunction — rebuilt exactly and
        // compared to the checked conclusion's own denotation so every
        // payload lands against the goal's real Σ shape.
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
        // The conversion's order: `d ≤ −2 ∨ 1 ≤ d ∨ (d ≤ −1 ∧ min+1 ≤ n)`
        // — `Or(D0, Or(D1, And))`, so the `1 ≤ d` disjunct is the inner
        // sum's zero branch and the conjunction its one branch.
        let rest = self.tagged_sum(disjunct_one, conjunction);
        let expected = self.tagged_sum(disjunct_zero, rest);
        let goal = self.denote(conclusion)?;
        if !self.arena.structurally_equal(expected, goal) {
            return Ok(None);
        }

        // The four-way split's disjunct types — the same denoted terms
        // the law's `Π d` statement quantifies.
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

        // Inside each case λ the hypothesis is `Variable(0)` and every
        // ambient evidence citation shifts one binder.
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

        // case `d = 0`: impossible over the tight interval — either
        // solve outcome refutes (an in-interval root is the checker's
        // own forbidden case, unreachable in this conversion).
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

        // case `d = −1`: the conjunction's second conjunct.
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

        // case `d ≤ −2` and case `1 ≤ d`: the disjunction's first and
        // second disjuncts directly.
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

        // The four-way dependent elimination over the split law's
        // tagged sum — nested `caseTwo` exactly as `disjunction_elim`.
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
