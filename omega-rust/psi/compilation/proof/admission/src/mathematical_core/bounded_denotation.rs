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
mod denotation;
mod equality_transport;
use denotation::Denotation;
mod elaboration;
use elaboration::Elaboration;
mod forbidden_roots;
mod integer_operations;
mod multiplication;
#[cfg(test)]
mod order_antisymmetry_tests;
mod order_translation;
mod source_integer;
mod subtraction;
pub use source_integer::{SourceIntegerTheory, SourceIntegerVocabulary};

/// One bounded certificate elaborated into the common mathematical core.
///
/// `arena` is where the certificate's handles resolve — a caller hands
/// both to `encode_mathematical_certificate` for the wire or re-decides
/// the judgment with `verify_mathematical_certificate`. `certificate`
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

crate::debug_display!(BoundedDenotationError);

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
        let Some((first, second)) = proposition.as_equal() else {
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

/// One `IntegerValue` in the unbounded integer domain the numeral checks
/// compute in: signed and unsigned literals compare, subtract and divide in
/// one range, so a check never branches on the value's variant.
fn unbounded_integer(value: IntegerValue) -> BigInt {
    match value {
        IntegerValue::Signed(value) => BigInt::from_i128(value),
        IntegerValue::Unsigned(value) => BigInt::from_u128(value),
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
/// `verify_mathematical_certificate` call decides it.
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

#[cfg(test)]
mod tests;

/// Test-only helpers shared by the operation suites below — fixture,
/// affine-bound proof, and scalar-to-math-term conversion used to be
/// redefined verbatim under `addition/`, `multiplication/` and
/// `subtraction/`.
#[cfg(test)]
pub(crate) mod test_utilities;
