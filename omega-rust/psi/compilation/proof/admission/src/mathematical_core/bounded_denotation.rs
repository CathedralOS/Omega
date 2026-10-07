//! The bounded certificate language denoted into the common mathematical
//! core.
//!
//! `crate::proof` re-decides the bounded `ProofNode`/`Proposition`
//! certificate language — the shape every shipped certificate producer
//! emits, from the contract-entailment assumption discharge in
//! `typed-trees-to-checked-trees` upward — directly. This module is the
//! second, independent route the board item names: the same certificate
//! is denoted proposition-by-proposition and rule-by-rule into the common
//! core's terms, and the resulting judgment is re-decided by
//! [`verify_mathematical_certificate`]. `accept_certificate` consults it
//! on every acceptance, so a shipped certificate discharging a machine's
//! `ensures` obligation — a theorem such as `requires a == b ensures
//! b == a` — is judged by the kernel at admission, and the acceptance
//! records `MathematicalCoreDecision::Judged` with the judgment's
//! measurements or `Refused` naming the uncovered family. Nothing producer-side is trusted:
//! each rule's premises and conclusion relation are re-derived
//! structurally during denotation, `decide_primitive` re-decides
//! primitive leaves, and the kernel re-decides the whole elaborated
//! judgment — the same judgment that survives the canonical certificate
//! wire in `terminal-codec`.
//!
//! The denotation is deliberately small and compositional: every
//! proposition denotes a relevant `Type 0` inhabitant.
//!
//! - `Truth` denotes `Id Two zero zero` and `Falsehood` denotes
//!   `Id Two zero one`.
//! - Boolean values denote `Two`; literals, negation and Boolean equality
//!   use its constructors and case eliminator. Proposition equality is
//!   `Id Two`, not the Boolean-valued comparison operation.
//! - Other non-integer `Equal(l, r)` denotes `Id S l' r'` over the
//!   proposition's scalar carrier `S`, with operands constant at `S`.
//! - `Equal` and the `LessThan`/`LessOrEqual` relations over fixed
//!   non-address integers — including compound scalar operands — denote
//!   into one shared mathematical-integer vocabulary: `Int : Type 0`
//!   with each `IntegerMathTerm` interned by exact evaluated value
//!   (opaque open terms intern by the term itself). Closed magnitudes in the
//!   fixed scalar literal vocabulary have shared signed binary definitions;
//!   larger closed mathematical values retain opaque `Int` constants.
//!   Open mathematical and unevaluated exact scalar addition and subtraction
//!   share their respective functions applied to their operands; every other
//!   fixed-integer scalar operation denotes an uninterpreted per-operation
//!   function applied to its operands, keeping equality transport's
//!   rewrites visible inside `op l r`. An already-
//!   admitted open mathematical
//!   arithmetic term stays opaque if a previously skipped child exceeds evaluation
//!   resources. Thus
//!   evaluated `2 + 0 = 2` remains reflexive without a decision assumption.
//!   `IntLt` and `IntLe` are the two relation constants `Π(_ : Int). Π(_ : Int).
//!   Type 0`, and `IntegerMathEqual`/lifted `Equal` the `Id Int`
//!   identity. A cited `Equal` and its `IntegerMathEqual` form share
//!   one `Id Int`, so the citation-level `Equal`↔`IntegerMathEqual`
//!   crossing the bounded matcher licenses denotes to the same type.
//! - `ContentConservation` — exact equality in one closed content
//!   algebra — denotes `Id C l' r'` where `C` is that algebra's own
//!   `Type 0` carrier assumption and `l'`, `r'` are element assumptions
//!   interned by the canonical content terms. Equalities in different
//!   algebras can never compose: their carriers are different
//!   constants. A reflexive conservation `c ≡ c` is then a `refl` the
//!   kernel proves outright, and the transitivity rule below is a `J`
//!   composition, not an axiom.
//! - Every other atomic proposition denotes an assumption constant
//!   `P : Type 0` — the only axiom shape the calculus admits.
//! - `Conjunction` denotes a right-nested `Σ`, `Disjunction` a tagged sum
//!   `Σ(t : Two). caseTwo(λ(_ : Two). Type 0, d₀, rest, t)`, and
//!   `Implication` a non-dependent `Π` — the connective rules become the
//!   kernel's own pair/projection, `caseTwo` and λ/application rules.
//!
//! The judgment is `Γ ⊢ t : ⟦goal⟧` where `Γ` is the certificate's
//! assumption roster followed by its semantic-axiom roster — the same
//! premises `accept_certificate` scopes the proof under. Discharged
//! hypotheses (implication premises, case branches) are λ-binders inside
//! the evidence term, never premises of the judgment. A `Primitive` leaf
//! that is not definitionally reflexive becomes a *decision assumption*
//! in the signature — the bounded kernel's decision is evidence the
//! judgment commits to — so `certificate_assumption_closure` names
//! exactly the atoms, scalar carriers, integer vocabulary, scalar terms
//! and bounded decisions the judgment depends on, and nothing else.
//!
//! The denotation covers every rule family the certificate language
//! defines. The propositional and identity fragment — primitive,
//! assumption and semantic-axiom leaves, conjunction, disjunction and
//! implication introduction and elimination, and equality
//! symmetry/transitivity over any `Id` carrier — `Id Int` chains and
//! per-algebra content identities alike — denotes the kernel's own
//! constructions: pairs and projections, `caseTwo`, λ/application and
//! `J` eliminations. The integer order rules whose instance lands
//! wholly in the `Int` vocabulary cite a fixed roster of named
//! assumptions — `eq_le`, `lt_le`, the three strict/mixed transitivity
//! laws and the four endpoint-substitution laws — each an assumption
//! constant with an exact `Π` statement authored through `scheme_dsl`,
//! applied to the denoted endpoints and premise evidence, with a `sym`
//! `J` wrapping whichever premise the canonical `IntegerMathEqual`
//! order flipped. Integer order discreteness derives its adjacent-literal
//! order through shared binary definitions and five fixed numeral laws,
//! then applies mixed transitivity to the cited inclusive bound. Its
//! numeral laws remain explicit arithmetic assumptions, not a derivation
//! of integer arithmetic or a claim of assumption consistency. Subtraction
//! order applies fixed subtraction-by-zero and right-antitonicity laws to
//! the cited premises. Evaluated scalar differences retain canonical numeral
//! identity: binary order proves their positive-decrement case, while a
//! contradictory positivity premise uses fixed strict irreflexivity and
//! checked empty elimination. The checked correlated unsigned subtraction bound
//! applies fixed self-subtraction and nonstrict right-antitonicity laws to
//! derive nonnegativity; closed instances use binary order or contradictory
//! premise elimination. Correlated addition bounds apply fixed addition
//! monotonicity and subtraction cancellation while the sum and difference
//! stay unreduced; exact SSA root and carrier-endpoint equalities use
//! endpoint transport. A conclusion whose endpoints both evaluate denotes
//! `IntLe` between canonical constants: the binary numeral laws decide a
//! strict relation, `refl` an equal one, and empty elimination discharges a
//! false relation under a checked false premise. An open sum over a
//! closed difference replaces the applicative cancellation step with the
//! checked numeral-operation equation `add n r = e`, interned once per
//! evaluated operand triple — an exact interned assumption in place of
//! the whole-rule implication.
//! Value-equation transport expands both statements through their closed
//! value definitions and uses J in the exact replacement contexts. This
//! composes multiple equations, Boolean operations, exact addition/subtraction
//! and connective binders. A normalized Truth premise establishes a reflexive
//! expanded result by refl. Two `Id Two` identities differing by open
//! `not`/`equal` compositions over neutral Boolean atoms — the pairs Boolean
//! normalization equates without a closed computation — are decided by
//! `caseTwo` case analysis on each atom: `refl` closes a branch whose goal
//! endpoints agree, and `J` eliminates the contradictory premise at a branch
//! whose premise endpoints disagree. The `Id Two`/`Int` carrier crossing the
//! normalized `Truth`/`Falsehood` goals need is also derived, not assumed:
//! a closed-true goal is inhabited outright — `refl` on a reflexive `Id`,
//! the binary numeral order on a decided `IntLt`, `eq_le`/`lt_le` on a
//! decided `IntLe`, and the pair, tag and λ constructions through the
//! connective denotations — while a closed-false premise refutes, either
//! through `J`'s discriminant landing on the goal for `Id Two` over
//! distinct constructors, or through the numeral-order derivation,
//! substitution and transitivity laws and strict irreflexivity into
//! `Empty` eliminated into the goal. Identities over other carriers beyond a
//! single swap, dependent codomains, unmatched tagged sums, open or
//! undecided relations, and endpoints
//! outside the denoted Boolean fragment can still require rule-instance
//! evidence; a bounded construction allowance also declines to that explicit
//! route rather than rejecting an otherwise supported proof.
//! Other witness-bearing bound rules and denotation-conversion instances outside the
//! supported `Int` vocabulary denote a *rule-instance decision*: an assumption constant whose type
//! is the checked implication `Π(_ : ⟦premise₁⟧). … . ⟦conclusion⟧`,
//! applied to the denoted premise evidence (ambient axiom and
//! assumption citations bind as further premises). Each rule's
//! premise/conclusion relation is re-decided during denotation by the
//! same shared function the bounded checker runs — the axiom records
//! the rule instance's arithmetic or conversion decision in the
//! judgment's assumption closure, never silently — and premises in the
//! `Int` vocabulary give that axiom an exact arithmetic statement.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use numerics::bignum::BigInt;
use semantic_vocabulary::{
    ContentAlgebra, ContentTerm, IntegerMathLiteral, IntegerMathTerm, IntegerType, IntegerValue,
    Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId,
};

use super::certificate::{
    MathematicalCertificate, certificate_assumption_closure_on_current_thread,
    run_on_verification_stack, verify_mathematical_certificate_on_current_thread,
};
use super::conversion::Budget;
use super::scheme_dsl::{self, Syntax};
use super::signature::Declaration;
use super::term::{Level, Sort, Term, TermArena, TermHandle};
use super::typing::CoreError;
use crate::ClosedIntegerEvaluator;
use crate::kernel::{KernelError, decide_primitive};
use crate::proof::integer_math_normalization::{
    lift_fixed_integer_relation, propositions_match_under_integer_math_normalization,
};
use crate::proof::{
    AcceptedPremise, AcceptedProofRule, MathematicalJudgmentReceipt, PremiseRoster, ProofError,
    ProofNode, ProofRule, add_order, equality_rules, integer_bound_rules, integer_order_rules,
    order_discreteness, strict_order_transitivity, subtract_antitone, subtract_order,
};

/// A bound on the proof nodes one denotation walks — a resource refusal,
/// never a judgment. The canonical wire's own depth bound applies
/// separately to the denoted term.
const MAX_ELABORATION_NODES: u64 = 1 << 16;

mod addition;
mod binary_numerals;
mod booleans;
mod casts;
mod equality_transport;
mod elaboration;
use elaboration::Elaboration;
mod forbidden_roots;
mod integer_operations;
mod multiplication;
#[cfg(test)]
mod order_antisymmetry_tests;
mod order_translation;
mod subtraction;

/// One bounded certificate elaborated into the common mathematical core.
///
/// `arena` is where the certificate's handles resolve — a caller hands
/// both to `encode_mathematical_certificate` for the wire or re-decides
/// the judgment with [`verify_mathematical_certificate`]. `certificate`
/// is producer evidence even though this module produced it: the kernel
/// never trusts who built it. `rules`, `assumptions` and
/// `semantic_axioms` are the premise/rule record the denotation observed,
/// in the same shape `accept_certificate` reports — cited ambient
/// premises only, never discharged branch hypotheses.
pub struct BoundedDenotation {
    pub arena: TermArena,
    pub certificate: MathematicalCertificate,
    pub rules: Vec<AcceptedProofRule>,
    pub assumptions: Vec<AcceptedPremise>,
    pub semantic_axioms: Vec<AcceptedPremise>,
}

impl BoundedDenotation {
    /// Measure the elaborated judgment: signature size, exact assumption
    /// closure, context depth and the arena footprint at the time of the
    /// call — after `verify_bounded_certificate` that includes the
    /// kernel's working terms, which is the checking cost the board asks
    /// to be measured. The closure is computed over the stored signature
    /// by [`super::certificate::certificate_assumption_closure`], never by watching what
    /// conversion unfolded.
    pub fn receipt(&self) -> MathematicalJudgmentReceipt {
        run_on_verification_stack(|| self.receipt_on_current_thread())
    }

    /// Collect the same receipt inside an already-reserved verification stack.
    /// Admission calls this after elaboration and kernel checking have returned:
    /// their recursive frames are gone, so assumption traversal can reuse the
    /// full reservation. Public receipt collection still reserves its own stack;
    /// certificate size and producer identity never decide stack safety.
    pub(crate) fn receipt_on_current_thread(&self) -> MathematicalJudgmentReceipt {
        let saturating = |count: usize| u32::try_from(count).unwrap_or(u32::MAX);
        MathematicalJudgmentReceipt {
            declarations: saturating(self.certificate.signature.len()),
            assumption_closure: saturating(
                certificate_assumption_closure_on_current_thread(&self.arena, &self.certificate)
                    .len(),
            ),
            context_depth: saturating(self.certificate.context.len()),
            arena_slots: saturating(self.arena.len()),
        }
    }
}

/// Why a bounded certificate could not be denoted and re-decided.
///
/// The variants distinguish a malformed certificate from a valid one the
/// denotation does not cover and from a kernel rejection — a receiver can
/// route `Unsupported` back to the bounded checker without ever treating
/// it as a false judgment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundedDenotationError {
    /// The certificate is malformed as a bounded proof: a rule's
    /// premise/conclusion relation failed, a citation missed its roster,
    /// or a primitive judgment did not establish its conclusion. The
    /// payload is the same error `accept_certificate` reports.
    Certificate(ProofError),
    /// A valid bounded certificate construction the denotation does not
    /// cover. Unreachable for the families this route handles — the
    /// `Equal`/`IntegerMathEqual` crossing denotes identically — but kept
    /// so a future shape can refuse rather than decide false.
    Unsupported(&'static str),
    /// The elaborated judgment failed the kernel's re-decision. A
    /// well-formed elaboration never produces this; surfacing it keeps a
    /// bridge defect from ever becoming a silently accepted judgment.
    Kernel(CoreError),
    /// The proof tree exceeded the elaboration bound.
    DepthLimitExceeded,
}

impl std::fmt::Display for BoundedDenotationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for BoundedDenotationError {}

/// The premise citation relation: exactly
/// [`propositions_match_under_integer_math_normalization`] — the bounded
/// checker's own rule, shared so this route never invents a second
/// matching relation.
fn propositions_match(retained: &Proposition, requested: &Proposition) -> bool {
    propositions_match_under_integer_math_normalization(retained, requested)
}

/// The `IntegerValue` a canonical math literal denotes when its magnitude
/// fits the fixed numeral range — i128::MIN's unsigned magnitude
/// included. A literal outside it denotes to an opaque `Int` constant no
/// checked numeral-operation equation can name, so `None` callers keep
/// their explicit instance fallback.
/// Find a prior `Equal` landing `subject` and return the other side, its
/// proof handle, and the proposition.
fn affine_equality_for<'a>(
    definitions: &'a [(Proposition, TermHandle)],
    subject: &ScalarTerm,
) -> Option<(&'a ScalarTerm, TermHandle, &'a Proposition)> {
    definitions.iter().find_map(|(proposition, proof)| {
        let Proposition::Equal(first, second) = proposition else {
            return None;
        };
        if first == subject {
            Some((second, *proof, proposition))
        } else if second == subject {
            Some((first, *proof, proposition))
        } else {
            None
        }
    })
}

/// The witness's affine expression: its own root when the chain records no
/// axioms, else the expression the single recorded definition axiom lands.
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

/// The `LessOrEqual` premise the witness's root must match, oriented by the
/// checked bound's side.
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

/// The endpoint literal plus equality evidence: `Some((None, lit))` when
/// the endpoint already evaluates to it, `Some((Some(..), lit))` the
/// recorded equality landing `endpoint` there, `None` to give up.
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

/// Denote one bounded certificate — `goal`, `assumptions`,
/// `semantic_axioms` and `proof` exactly as `accept_certificate` takes
/// them — into the mathematical-core judgment `Γ ⊢ t : ⟦goal⟧`.
///
/// Denotation itself performs every structural check the bounded checker
/// performs: rule labels and node conclusions are re-derived, never
/// trusted. The returned certificate is still only evidence —
/// [`verify_bounded_certificate`] or an explicit
/// [`verify_mathematical_certificate`] call decides it.
pub fn denote_bounded_certificate(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    proof: &ProofNode,
) -> Result<BoundedDenotation, BoundedDenotationError> {
    run_on_verification_stack(|| {
        denote_bounded_certificate_on_current_thread(
            context,
            goal,
            assumptions,
            semantic_axioms,
            &BTreeSet::new(),
            proof,
        )
    })
}

/// [`denote_bounded_certificate`] on the caller's stack, for the
/// verification route that already reserves
/// [`run_on_verification_stack`]'s depth.
fn denote_bounded_certificate_on_current_thread(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
    proof: &ProofNode,
) -> Result<BoundedDenotation, BoundedDenotationError> {
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

/// Denote a bounded certificate and re-decide the resulting judgment with
/// the kernel. This is the route the board item names: the shipped
/// certificate language checked by the common mathematical core, not a
/// second proposition truth.
pub fn verify_bounded_certificate(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    proof: &ProofNode,
    budget: &mut Budget,
) -> Result<BoundedDenotation, BoundedDenotationError> {
    run_on_verification_stack(|| {
        verify_bounded_certificate_on_current_thread(
            context,
            goal,
            assumptions,
            semantic_axioms,
            &BTreeSet::new(),
            proof,
            budget,
        )
    })
}

/// [`verify_bounded_certificate`] on the caller's stack. Elaboration recurses over the certificate's proof tree and the
/// kernel recurses over the elaborated term, so the public entry runs the
/// whole route inside [`run_on_verification_stack`] — one deep stack for
/// both stages. `accept_certificate` calls this directly because it
/// already reserves the same stack for its bounded traversal.
pub(crate) fn verify_bounded_certificate_on_current_thread(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
    proof: &ProofNode,
    budget: &mut Budget,
) -> Result<BoundedDenotation, BoundedDenotationError> {
    let mut denoted = denote_bounded_certificate_on_current_thread(
        context,
        goal,
        assumptions,
        semantic_axioms,
        machine_parameter_values,
        proof,
    )?;
    verify_mathematical_certificate_on_current_thread(
        &mut denoted.arena,
        &denoted.certificate,
        budget,
    )
    .map_err(BoundedDenotationError::Kernel)?;
    Ok(denoted)
}

/// The fixed roster of integer laws the order rules cite. Each member
/// is one assumption constant with an exact `Π` statement over `Int`,
/// `IntLt` and `IntLe` — a receiver audits the roster once, instead of
/// one opaque `Π` implication per rule instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum IntegerLaw {
    /// `Π(x y : Int). Id Int x y → IntLe x y`.
    EqualityToLessOrEqual,
    /// `Π(x y : Int). IntLt x y → IntLe x y`.
    LessThanToLessOrEqual,
    /// `Π(x y z : Int). IntLe x y → IntLe y z → IntLe x z`.
    LessOrEqualTransitivity,
    /// `Π(x y z : Int). IntLt x y → IntLt y z → IntLt x z`.
    LessThanTransitivity,
    /// `Π(x y z : Int). IntLt x y → IntLe y z → IntLt x z`.
    LessThanLessOrEqualTransitivity,
    /// `Π(x y z : Int). IntLe x y → IntLt y z → IntLt x z`.
    LessOrEqualLessThanTransitivity,
    /// `Π(x y z : Int). Id Int x y → IntLt x z → IntLt y z`.
    LessThanSubstituteLeft,
    /// `Π(x y z : Int). Id Int y z → IntLt x y → IntLt x z`.
    LessThanSubstituteRight,
    /// `Π(x y z : Int). Id Int x y → IntLe x z → IntLe y z`.
    LessOrEqualSubstituteLeft,
    /// `Π(x y z : Int). Id Int y z → IntLe x y → IntLe x z`.
    LessOrEqualSubstituteRight,
}

/// The interning key for an `Int` element: the exact evaluated value of
/// a closed term — `IntegerMathLiteral` magnitudes cap at `u128`, so
/// values beyond that boundary intern by `BigInt`, not by literal —
/// and the term itself when an open leaf or an undefined shift leaves
/// it unevaluated.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum MathTermKey {
    Closed(BigInt),
    Open(IntegerMathTerm),
}

/// The `IntegerValue` a closed `Int` value denotes when it fits the
/// fixed scalar literal range the numeral-order derivations cover —
/// the same boundary `math_term` keeps between shared binary
/// definitions and opaque closed assumptions. `None` leaves a larger
/// closed value to the caller's fallback rather than approximating it.
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

/// The denoted head of a mathematical-integer proposition — the shape
/// the integer laws quantify over — with endpoints as interned `Int`
/// element handles.
#[derive(Debug, Clone, Copy)]
enum IntegerRelation {
    /// `Id Int left right`.
    Equality { left: TermHandle, right: TermHandle },
    /// `IntLt left right`.
    LessThan { left: TermHandle, right: TermHandle },
    /// `IntLe left right`.
    LessOrEqual { left: TermHandle, right: TermHandle },
}

/// One roster law's exact `Π` statement, written in `scheme_dsl`
/// notation over the `Int`, `IntLt` and `IntLe` declaration positions
/// so binders resolve to de Bruijn indices mechanically. The built term
/// is a declaration's `ty` — `check_signature` re-decides it under the
/// roster prefix like any other statement.
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
    // `Π (x y : Int). premise → conclusion`.
    let implication = |premise: Syntax, conclusion: Syntax| {
        pi(
            "x",
            carrier(),
            pi("y", carrier(), pi("_", premise, conclusion)),
        )
    };
    // `Π (x y z : Int). first → second → result`.
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

/// Proposition and scalar-term denotation state: the arena the judgment's
/// terms live in, the declaration signature every constant names, and the
/// interning tables that give each atom, carrier and scalar value exactly
/// one declaration.
struct Denotation {
    arena: TermArena,
    type_zero: TermHandle,
    two: TermHandle,
    two_zero: TermHandle,
    two_one: TermHandle,
    boolean_equality: TermHandle,
    declarations: Vec<Declaration>,
    /// Canonical atomic proposition → assumption position. The key is
    /// the proposition's lifted mathematical form when the fixed-width
    /// relation lifts, so lift/lower-equivalent citations share one atom.
    atoms: BTreeMap<Proposition, u32>,
    /// Scalar type → `Type 0` carrier assumption position.
    carriers: BTreeMap<ScalarType, u32>,
    /// Canonical scalar term → assumption position at its carrier. The
    /// key is the term's evaluated closed literal when it has one, so
    /// decided closed equalities denote reflexive identities.
    terms: BTreeMap<ScalarTerm, u32>,
    /// Content algebra → `Type 0` carrier assumption position. Each
    /// algebra's conservation equations denote `Id` over their own
    /// carrier, so identities in different algebras can never compose.
    content_carriers: BTreeMap<ContentAlgebra, u32>,
    /// `(algebra, content term)` → element assumption position at that
    /// algebra's carrier. The key is the term itself: `ContentTerm`'s
    /// constructors already normalize ordering and nesting, so
    /// syntactic identity is the algebra's own canonical form.
    content_terms: BTreeMap<(ContentAlgebra, ContentTerm), u32>,
    /// The `Int : Type 0` carrier position — one declaration every
    /// mathematical-integer constant and relation names.
    integer: Option<u32>,
    /// `IntLt : Π(_ : Int). Π(_ : Int). Type 0` — the strict order.
    integer_less_than: Option<u32>,
    /// `IntLe : Π(_ : Int). Π(_ : Int). Type 0` — the nonstrict order.
    integer_less_or_equal: Option<u32>,
    /// The fixed integer-law roster: law → assumption position.
    integer_laws: BTreeMap<IntegerLaw, u32>,
    /// Shared fixed-width numeral definitions and their fixed arithmetic laws.
    binary_numerals: binary_numerals::BinaryNumerals,
    subtraction: subtraction::Subtraction,
    addition: addition::Addition,
    /// Shared `mul` and the multiplication/division-residual law roster
    /// the correlated multiply bounds spend.
    multiplication: multiplication::Multiplication,
    /// The correlated forbidden-root law roster: chain-inversion,
    /// quotient-squeeze and divisor-case-split assumptions.
    forbidden_roots: forbidden_roots::ForbiddenRoots,
    /// Canonical mathematical term → `Int`-typed declaration position.
    /// Closed terms intern by exact evaluated value — so a decided
    /// `IntegerMathEqual` on closed operands denotes `refl`-provable
    /// `Id Int c c` — and open terms intern by the term itself.
    math_terms: BTreeMap<MathTermKey, u32>,
    /// `Int` declaration position → the exact closed value it interned
    /// by — the reverse of `math_terms`' `MathTermKey::Closed` entries.
    /// The closed Truth/Falsehood carrier crossing reads the decided
    /// value behind a denoted endpoint to pick the numeral-order
    /// derivation it needs without re-evaluating the term.
    closed_math_values: BTreeMap<u32, BigInt>,
    /// Sparse scalar terms outside the compositional Int denotation:
    /// field projections and any constructor `integer_operation_term`
    /// does not classify. Values/closed literals use math_terms; every
    /// classified operation denotes its uninterpreted function applied to
    /// the operand terms rather than a whole-term constant here.
    scalar_integer_terms: BTreeMap<ScalarTerm, u32>,
    /// Uninterpreted fixed-integer operation → `Π` assumption position,
    /// interned per operation constructor and machine types. Sharing one
    /// function per operation keeps transport's operand rewrites inside
    /// `op l r` applications visible; one constant per applied term
    /// would hide them.
    integer_operations: BTreeMap<integer_operations::IntegerOperation, u32>,
    /// Exact cast/widen operation → the position of its identity law
    /// `Π(x : Int). Id Int (op x) x` — the denoted content of the
    /// checked chain's "every edge preserves the mathematical integer"
    /// invariant, interned once per operation like the other fixed laws.
    cast_identities: BTreeMap<integer_operations::IntegerOperation, u32>,
    /// `Primitive` leaf statement → decision-assumption position.
    decisions: HashMap<TermHandle, u32>,
    /// `(operand, lower)` → assumption position of the operand's
    /// carrier-membership bound — `IntLe min' op'` or `IntLe op' max'`.
    /// A `Truth` bound over an open operand contributes exactly this
    /// fact; interning it per operand keeps the closure's named
    /// assumption the membership bound itself, not a rule implication.
    carrier_bounds: BTreeMap<(ScalarTerm, bool), u32>,
    /// Bounded rule instance → decision-assumption position. The key is
    /// the instance's premise propositions in rule order plus its
    /// conclusion — exactly what determines the axiom's `Π` type — so
    /// two nodes deriving the same implication share one constant.
    rule_axioms: BTreeMap<(Vec<Proposition>, Proposition), u32>,
    /// Declaration position → `Constant` node, so equal references share
    /// one handle.
    constants: HashMap<u32, TermHandle>,
    /// Proposition → denoted term cache; denotation is deterministic, so
    /// the same proposition always denotes to the same handle.
    denotations: BTreeMap<Proposition, TermHandle>,
}

impl Denotation {
    fn new() -> Self {
        let mut arena = TermArena::new();
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
            declarations: Vec::new(),
            atoms: BTreeMap::new(),
            carriers: BTreeMap::new(),
            terms: BTreeMap::new(),
            content_carriers: BTreeMap::new(),
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
            denotations: BTreeMap::new(),
        }
    }

    fn position(&self) -> Result<u32, BoundedDenotationError> {
        u32::try_from(self.declarations.len())
            .map_err(|_| BoundedDenotationError::DepthLimitExceeded)
    }

    fn constant(&mut self, declaration: u32) -> TermHandle {
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

    /// The `Type 0` assumption constant an atomic proposition outside
    /// the integer vocabulary denotes — every atom the certificate's
    /// cited premises reach, interned once by the proposition itself.
    fn atom(&mut self, proposition: &Proposition) -> Result<TermHandle, BoundedDenotationError> {
        let key = lift_fixed_integer_relation(proposition).unwrap_or_else(|| proposition.clone());
        if let Some(&position) = self.atoms.get(&key) {
            return Ok(self.constant(position));
        }
        let position = self.position()?;
        self.declarations
            .push(Declaration::assumption(0, self.type_zero));
        self.atoms.insert(key, position);
        Ok(self.constant(position))
    }

    /// The `Type 0` assumption constant a scalar carrier denotes.
    fn carrier(&mut self, scalar_type: ScalarType) -> Result<TermHandle, BoundedDenotationError> {
        if scalar_type == ScalarType::Boolean {
            return Ok(self.two);
        }
        if let Some(&position) = self.carriers.get(&scalar_type) {
            return Ok(self.constant(position));
        }
        let position = self.position()?;
        self.declarations
            .push(Declaration::assumption(0, self.type_zero));
        self.carriers.insert(scalar_type, position);
        Ok(self.constant(position))
    }

    /// Selected Boolean computation, an applicative value-level integer
    /// comparison, or an opaque scalar constant. A closed integer
    /// term is interned by its evaluated literal — the denotation is by
    /// value, so `2 + 0` and `2` name one constant and a decided
    /// `Equal(2 + 0, 2)` is `refl`-provable rather than admitted.
    fn scalar_term(&mut self, term: &ScalarTerm) -> Result<TermHandle, BoundedDenotationError> {
        if let Some(boolean) = self.boolean_term(term)? {
            return Ok(boolean);
        }
        if let Some(relation) = self.integer_relation_term(term)? {
            return Ok(relation);
        }
        let key = match term.integer_value() {
            Some((integer_type, value)) => {
                ScalarTerm::integer(integer_type, value).unwrap_or_else(|_| term.clone())
            }
            None => term.clone(),
        };
        if let Some(&position) = self.terms.get(&key) {
            return Ok(self.constant(position));
        }
        let ty = self.carrier(key.scalar_type())?;
        let position = self.position()?;
        self.declarations.push(Declaration::assumption(0, ty));
        self.terms.insert(key, position);
        Ok(self.constant(position))
    }

    /// The `Type 0` carrier assumption one closed content algebra
    /// denotes — its conservation equations are identities over this
    /// carrier, interned once per algebra.
    fn content_carrier(
        &mut self,
        algebra: &ContentAlgebra,
    ) -> Result<TermHandle, BoundedDenotationError> {
        if let Some(&position) = self.content_carriers.get(algebra) {
            return Ok(self.constant(position));
        }
        let position = self.position()?;
        self.declarations
            .push(Declaration::assumption(0, self.type_zero));
        self.content_carriers.insert(algebra.clone(), position);
        Ok(self.constant(position))
    }

    /// The assumption constant one content term denotes — an element of
    /// its algebra's carrier, interned by the canonical term. Content
    /// terms are already normalized at construction, so `a ≡ a` denotes
    /// a reflexive `Id` the kernel's `refl` proves rather than an
    /// admitted atom.
    fn content_term(
        &mut self,
        algebra: &ContentAlgebra,
        term: &ContentTerm,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let key = (algebra.clone(), term.clone());
        if let Some(&position) = self.content_terms.get(&key) {
            return Ok(self.constant(position));
        }
        let ty = self.content_carrier(algebra)?;
        let position = self.position()?;
        self.declarations.push(Declaration::assumption(0, ty));
        self.content_terms.insert(key, position);
        Ok(self.constant(position))
    }

    /// `Int : Type 0` — the mathematical-integer carrier assumption
    /// every `Int` element and integer relation names, pushed once.
    fn integer(&mut self) -> Result<u32, BoundedDenotationError> {
        if let Some(position) = self.integer {
            return Ok(position);
        }
        let position = self.position()?;
        self.declarations
            .push(Declaration::assumption(0, self.type_zero));
        self.integer = Some(position);
        Ok(position)
    }

    /// The `Int` carrier as a term — the `Constant` node every integer
    /// identity types its endpoints against.
    fn integer_constant(&mut self) -> Result<TermHandle, BoundedDenotationError> {
        let position = self.integer()?;
        Ok(self.constant(position))
    }

    /// `IntLt`/`IntLe : Π(_ : Int). Π(_ : Int). Type 0` — one shared
    /// order-relation statement shape, two positions.
    fn integer_order_relation(&mut self) -> Result<u32, BoundedDenotationError> {
        let integer = self.integer()?;
        let ty = scheme_dsl::build(
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

    /// `IntLt`'s declaration position.
    fn integer_less_than(&mut self) -> Result<u32, BoundedDenotationError> {
        if let Some(position) = self.integer_less_than {
            return Ok(position);
        }
        let position = self.integer_order_relation()?;
        self.integer_less_than = Some(position);
        Ok(position)
    }

    /// `IntLe`'s declaration position.
    fn integer_less_or_equal(&mut self) -> Result<u32, BoundedDenotationError> {
        if let Some(position) = self.integer_less_or_equal {
            return Ok(position);
        }
        let position = self.integer_order_relation()?;
        self.integer_less_or_equal = Some(position);
        Ok(position)
    }

    /// One roster law's assumption position — pushed with the carrier
    /// and both relation constants before it, so `check_signature`
    /// decides its exact `Π` statement under the roster prefix.
    fn integer_law(&mut self, law: IntegerLaw) -> Result<u32, BoundedDenotationError> {
        if let Some(&position) = self.integer_laws.get(&law) {
            return Ok(position);
        }
        let integer = self.integer()?;
        let less_than = self.integer_less_than()?;
        let less_or_equal = self.integer_less_or_equal()?;
        let ty = scheme_dsl::build(
            &mut self.arena,
            &mut Vec::new(),
            &integer_law_statement(law, integer, less_than, less_or_equal),
        );
        let position = self.position()?;
        self.declarations.push(Declaration::assumption(0, ty));
        self.integer_laws.insert(law, position);
        Ok(position)
    }

    /// `law a₁ … aₙ` — the roster constant applied to the denoted
    /// endpoints, then the premise evidence, in statement order.
    fn integer_law_application(
        &mut self,
        law: IntegerLaw,
        arguments: &[TermHandle],
    ) -> Result<TermHandle, BoundedDenotationError> {
        let position = self.integer_law(law)?;
        let mut term = self.constant(position);
        for &argument in arguments {
            term = self.arena.insert(Term::Apply {
                function: term,
                argument,
            });
        }
        Ok(term)
    }

    /// The `Int`-typed declaration a mathematical term denotes —
    /// interned by its exact closed value when the shared evaluator has
    /// one, so `add(1, 1)` and `2` name one constant and a decided
    /// `IntegerMathEqual` on them is `refl`-provable. Open addition,
    /// subtraction and multiplication compose child denotations through
    /// their shared `Int` functions; other open terms intern by source
    /// structure.
    /// Fixed scalar magnitudes are binary definitions;
    /// larger closed values retain opaque assumptions so this focused
    /// discreteness encoding adds no numeric acceptance limit.
    fn math_term(&mut self, term: &IntegerMathTerm) -> Result<TermHandle, BoundedDenotationError> {
        let key = match ClosedIntegerEvaluator::default().evaluate_closed(term) {
            Ok(Some(value)) => MathTermKey::Closed(value),
            Ok(None) => {
                // Open addition, subtraction and multiplication share
                // their scalar functions. Retain shallow applications,
                // not cloned source-tree keys for every nested prefix.
                // Closed values still intern above.
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
                // Evaluation short-circuits on an open left child. If an
                // unvisited child would exceed its numeric work budget, keep
                // this already-admitted expression opaque as before. This
                // does not relax the whole-term preflight or a closed refusal.
                MathTermKey::Open(term.clone())
            }
            Err(error) => {
                // The same refusal the primitive judgment reports —
                // interning shares the evaluator's budget, so a term
                // too large to evaluate is refused, never approximated.
                return Err(BoundedDenotationError::Certificate(
                    ProofError::PrimitiveJudgment(KernelError::ClosedIntegerEvaluation(error)),
                ));
            }
        };
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

    /// The exact closed value an interned `Int` constant denotes — the
    /// reverse of `math_terms`' `MathTermKey::Closed` interning, so the
    /// decided value behind a denoted endpoint is readable without
    /// re-evaluating. An open leaf, an applicative composition or any
    /// non-`Int` constant has no decided value here, so the closed
    /// relation crossing never guesses at what it denotes.
    fn closed_math_value(&self, term: TermHandle) -> Option<BigInt> {
        let Term::Constant { declaration, .. } = self.arena.get(term) else {
            return None;
        };
        self.closed_math_values.get(&declaration).cloned()
    }

    /// Keep one carrier for fixed scalar equations and orders, including
    /// mixed symbolic/compound endpoints. Every fixed-integer operation
    /// denotes its uninterpreted function applied to the operand terms —
    /// exact addition, subtraction and multiplication additionally carry
    /// their fixed arithmetic laws — while field projections and
    /// unclassified constructors stay per-term opaque.
    pub(super) fn fixed_scalar_term(
        &mut self,
        term: &ScalarTerm,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let denoted = if let Some((_, value)) = term.integer_value() {
            self.math_term(&IntegerMathTerm::literal(value))?
        } else {
            match term {
                ScalarTerm::Value {
                    id,
                    scalar_type: ScalarType::Integer(source_type),
                } => self.math_term(&IntegerMathTerm::MathValue {
                    source_type: *source_type,
                    value: *id,
                })?,
                ScalarTerm::ExactIntegerSubtract { left, right, .. }
                | ScalarTerm::ExactIntegerAdd { left, right, .. }
                | ScalarTerm::ExactIntegerMultiply { left, right, .. } => {
                    let left = self.fixed_scalar_term(left)?;
                    let right = self.fixed_scalar_term(right)?;
                    if matches!(term, ScalarTerm::ExactIntegerAdd { .. }) {
                        self.add_terms(left, right)?
                    } else if matches!(term, ScalarTerm::ExactIntegerSubtract { .. }) {
                        self.subtract_terms(left, right)?
                    } else {
                        self.multiply_terms(left, right)?
                    }
                }
                _ => {
                    if let Some(applied) = self.integer_operation_term(term)? {
                        return Ok(applied);
                    }
                    if let Some(&position) = self.scalar_integer_terms.get(term) {
                        return Ok(self.constant(position));
                    }
                    let ty = self.integer_constant()?;
                    let position = self.position()?;
                    self.declarations.push(Declaration::assumption(0, ty));
                    self.scalar_integer_terms.insert(term.clone(), position);
                    self.constant(position)
                }
            }
        };
        Ok(denoted)
    }

    fn fixed_scalar_relation(
        &mut self,
        proposition: &Proposition,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let (left, right) = match proposition {
            Proposition::Equal(left, right)
            | Proposition::LessThan(left, right)
            | Proposition::LessOrEqual(left, right) => (left, right),
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
            let function = self.arena.insert(Term::Apply {
                function: relation,
                argument: left,
            });
            self.arena.insert(Term::Apply {
                function,
                argument: right,
            })
        };
        Ok(Some(denoted))
    }

    /// `IntLt l r` or `IntLe l r` — the relation constant applied to the
    /// two interned `Int` endpoints.
    fn integer_order_term(
        &mut self,
        strict: bool,
        left: &IntegerMathTerm,
        right: &IntegerMathTerm,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let position = if strict {
            self.integer_less_than()?
        } else {
            self.integer_less_or_equal()?
        };
        let left = self.math_term(left)?;
        let right = self.math_term(right)?;
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

    /// The denoted head and endpoints of a proposition in the integer
    /// vocabulary — `Id Int`, `IntLt` or `IntLe` — or `None` when
    /// `denoted` lives in another vocabulary (a scalar carrier's `Id`,
    /// an atom, a connective). Read-only: matching never creates a
    /// declaration, so a partially-`Int` instance leaves the vocabulary
    /// unused rather than pushed but uncited.
    fn integer_relation(&self, denoted: TermHandle) -> Option<IntegerRelation> {
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

    /// The `(carrier, left, right)` of an `Id`-denoted proposition, or
    /// `None` for any other denotation shape.
    fn identity_parts(&self, denoted: TermHandle) -> Option<(TermHandle, TermHandle, TermHandle)> {
        if let Term::Id { ty, left, right } = self.arena.get(denoted) {
            Some((ty, left, right))
        } else {
            None
        }
    }

    /// `sym p` — `J(λ(y : C). λ(_ : Id C a y). Id C y a, refl C a, b, p)`
    /// mapping `p : Id C a b` to `Id C b a`, over whichever carrier the
    /// denoted identity names.
    fn symmetry(
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

    /// `q ∘ p` — `J(λ(y : C). λ(_ : Id C m y). Id C l y, p, r, q)`
    /// composing `p : Id C l m` and `q : Id C m r` to `Id C l r`, over
    /// whichever carrier the denoted identities share.
    fn transitivity(
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

    /// Denoted symmetry evidence: the licensed premise and goal share
    /// one `Id` carrier — canonical `IntegerMathEqual` order may already
    /// identify them (the premise evidence stands alone), or read them
    /// swapped (a `sym` `J`); any other denotation stays a per-instance
    /// axiom.
    fn symmetry_evidence(
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

    /// `p₂ ∘ p₁` over the denoted identities: the shared transitivity
    /// check licensed the chain, so the denoted premises share a carrier
    /// and one endpoint — the canonical `IntegerMathEqual` order decides
    /// which side of each premise the shared endpoint sits on, and a
    /// `sym` `J` reorients whichever premise needs it. `None` keeps the
    /// per-instance axiom for non-`Id` denotations.
    fn transitivity_evidence(
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
        } else if self.arena.structurally_equal(e, right) && self.arena.structurally_equal(f, left)
        {
            Some(self.symmetry(goal_carrier, left, right, composed))
        } else {
            None
        }
    }

    /// `eq_le`/`lt_le` on the denoted relations for one
    /// `IntegerOrderWeakening` instance: `Id Int` evidence is sym'd into
    /// the conclusion's endpoint order when canonical order flipped it.
    /// `None` leaves the per-instance axiom.
    fn weakening_evidence(
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
                self.integer_law_application(IntegerLaw::LessThanToLessOrEqual, &[a, b, evidence])
                    .map(Some)
            }
            Some(IntegerRelation::Equality { left: a, right: b })
                if self.arena.structurally_equal(a, left)
                    && self.arena.structurally_equal(b, right) =>
            {
                self.integer_law_application(IntegerLaw::EqualityToLessOrEqual, &[a, b, evidence])
                    .map(Some)
            }
            Some(IntegerRelation::Equality { left: a, right: b })
                if self.arena.structurally_equal(a, right)
                    && self.arena.structurally_equal(b, left) =>
            {
                let integer = self.integer_constant()?;
                let evidence = self.symmetry(integer, a, b, evidence);
                self.integer_law_application(IntegerLaw::EqualityToLessOrEqual, &[b, a, evidence])
                    .map(Some)
            }
            _ => Ok(None),
        }
    }

    /// The transitivity law on the denoted `IntLt`/`IntLe` chain — the
    /// shared check already fixed the exact middle and endpoints, so
    /// matching denotations is a lookup, and `None` leaves the
    /// per-instance axiom for any instance outside the vocabulary.
    fn order_transitivity_evidence(
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

    /// The `R`-substitution law for one `IntegerOrderSubstitution`
    /// instance: the relation and conclusion agree on strictness and the
    /// unchanged endpoint, and the `Id` evidence reads old→new or its
    /// `sym` flip. `None` leaves the per-instance axiom.
    fn order_substitution_evidence(
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
        // The law's (x, y, z) quantifier order, the replaced endpoint's
        // old→new pair, and the endpoint arguments in statement order.
        // The other endpoint never moves — denoted equality is canonical,
        // so the `Id` evidence may read new→old and take a `sym` flip.
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
        } else if self.arena.structurally_equal(s, new) && self.arena.structurally_equal(t, old) {
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

    /// The equation evidence oriented as `Id Int old new`, or `None`
    /// when its denoted endpoints are not `{old, new}` in either order.
    fn directed_equality(
        &mut self,
        s: TermHandle,
        t: TermHandle,
        old: TermHandle,
        new: TermHandle,
        evidence: TermHandle,
    ) -> Option<TermHandle> {
        if self.arena.structurally_equal(s, old) && self.arena.structurally_equal(t, new) {
            Some(evidence)
        } else if self.arena.structurally_equal(s, new) && self.arena.structurally_equal(t, old) {
            let integer = self.integer_constant().ok()?;
            Some(self.symmetry(integer, s, t, evidence))
        } else {
            None
        }
    }

    /// The named decision axiom for one bounded rule instance: an
    /// assumption constant of type `Π(_ : ⟦premise₁⟧). … . ⟦conclusion⟧`.
    /// The premise/conclusion relation itself has already been re-decided
    /// by the shared check; the constant records the rule's arithmetic or
    /// conversion content so the judgment's closure names the instance
    /// exactly. Equal instances intern to one constant.
    fn rule_axiom(
        &mut self,
        premises: &[Proposition],
        conclusion: &Proposition,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let key = (premises.to_vec(), conclusion.clone());
        if let Some(&position) = self.rule_axioms.get(&key) {
            return Ok(self.constant(position));
        }
        let mut ty = self.denote(conclusion)?;
        for premise in premises.iter().rev() {
            let domain = self.denote(premise)?;
            ty = self.arena.insert(Term::Pi {
                domain,
                codomain: ty,
            });
        }
        let position = self.position()?;
        self.declarations.push(Declaration::assumption(0, ty));
        self.rule_axioms.insert(key, position);
        Ok(self.constant(position))
    }

    /// Record a `Primitive` decision the certificate commits to as a named
    /// assumption of the leaf's denoted statement, so the judgment's
    /// assumption closure names it exactly.
    fn decision(&mut self, ty: TermHandle) -> Result<TermHandle, BoundedDenotationError> {
        if let Some(&position) = self.decisions.get(&ty) {
            return Ok(self.constant(position));
        }
        let position = self.position()?;
        self.declarations.push(Declaration::assumption(0, ty));
        self.decisions.insert(ty, position);
        Ok(self.constant(position))
    }

    /// The correlated-affine root-equality step: direct the cited equality
    /// at the witness root's expression and substitute it into the bound.
    /// `None` means give up; `Some(term)` is the extended evidence.
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
        let Some(equality) = self.directed_equality(from, to, root_term, expression_term, equality)
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

    /// The correlated-affine endpoint-equality tail: direct recorded
    /// equality evidence at the endpoint literal, then substitute it
    /// through `swapped_term` (evaluated lazily inside the guard).
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

    fn denote(&mut self, proposition: &Proposition) -> Result<TermHandle, BoundedDenotationError> {
        if let Some(&cached) = self.denotations.get(proposition) {
            return Ok(cached);
        }
        let term = self.denote_uncached(proposition)?;
        self.denotations.insert(proposition.clone(), term);
        Ok(term)
    }

    fn denote_uncached(
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
            Proposition::Equal(..) | Proposition::LessThan(..) | Proposition::LessOrEqual(..) => {
                // A relation whose integer operands both lift denotes
                // the `Int` vocabulary — `Id Int` for `Equal`, the
                // `IntLt`/`IntLe` application for the orders — so fixed
                // and `IntegerMath*` forms share one type. Compound fixed
                // scalar operands also use Int, with law-carrying
                // addition/subtraction and applicative uninterpreted other
                // open operations. This does
                // not broaden the bounded premise matcher's lift. Other
                // carriers retain `Id S` equality and atomic orders.
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
            Proposition::Conjunction(conjuncts) => self.conjunction(conjuncts)?,
            Proposition::Disjunction(disjuncts) => self.disjunction(disjuncts)?,
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                let domain = self.denote(premise)?;
                let codomain = self.denote(conclusion)?;
                self.arena.insert(Term::Pi { domain, codomain })
            }
            Proposition::ContentConservation(conservation) => {
                // `l ≡ r` in one algebra denotes `Id` over that
                // algebra's carrier: conservation is the algebra's own
                // equality, so its transitivity and reflexivity are the
                // kernel's `J` and `refl`, not rule-instance axioms.
                let ty = self.content_carrier(conservation.algebra())?;
                let left = self.content_term(conservation.algebra(), conservation.left())?;
                let right = self.content_term(conservation.algebra(), conservation.right())?;
                self.arena.insert(Term::Id { ty, left, right })
            }
            atomic => self.atom(atomic)?,
        })
    }

    /// `⟦c₀ ∧ … ∧ cₙ₋₁⟧` — a right-nested `Σ`, so conjunction
    /// introduction is a nested pair and elimination a projection path.
    fn conjunction(
        &mut self,
        conjuncts: &[Proposition],
    ) -> Result<TermHandle, BoundedDenotationError> {
        if conjuncts.len() < 2 {
            // `Proposition::validate` rejects non-canonical arity before
            // denotation; this stays a refusal, never a judgment.
            return Err(BoundedDenotationError::Certificate(
                ProofError::RuleConclusionMismatch("conjunction arity"),
            ));
        }
        let domain = self.denote(&conjuncts[0])?;
        let codomain = if conjuncts.len() == 2 {
            self.denote(&conjuncts[1])?
        } else {
            self.conjunction(&conjuncts[1..])?
        };
        Ok(self.arena.insert(Term::Sigma { domain, codomain }))
    }

    /// `⟦d₀ ∨ … ∨ dₙ₋₁⟧` — the tagged sum `Σ(t : Two). caseTwo(M, d₀,
    /// rest, t)` with `rest` the nested disjunction, so a disjunction
    /// introduction is a `⟨zero, …⟩`/`⟨one, …⟩` pair and an elimination a
    /// dependent `caseTwo`.
    fn disjunction(
        &mut self,
        disjuncts: &[Proposition],
    ) -> Result<TermHandle, BoundedDenotationError> {
        if disjuncts.len() < 2 {
            return Err(BoundedDenotationError::Certificate(
                ProofError::RuleConclusionMismatch("disjunction arity"),
            ));
        }
        let zero_case = self.denote(&disjuncts[0])?;
        let one_case = if disjuncts.len() == 2 {
            self.denote(&disjuncts[1])?
        } else {
            self.disjunction(&disjuncts[1..])?
        };
        Ok(self.tagged_sum(zero_case, one_case))
    }

    /// `Σ(t : Two). caseTwo(λ(_ : Two). Type 0, zero_case, one_case, t)`
    /// — the binary coproduct shape the disjunction denotation nests.
    fn tagged_sum(&mut self, zero_case: TermHandle, one_case: TermHandle) -> TermHandle {
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

#[cfg(test)]
mod tests;

/// Test-only helpers shared by the operation suites below — fixture,
/// affine-bound proof, and scalar-to-math-term conversion used to be
/// redefined verbatim under `addition/`, `multiplication/` and
/// `subtraction/`.
#[cfg(test)]
pub(crate) mod test_utilities;
