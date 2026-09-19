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
//! - `Equal(l, r)` denotes `Id S l' r'` over the proposition's scalar
//!   carrier `S` — a `Type 0` assumption — with each operand a constant
//!   of `S`. Closed integer terms are interned by *value*, so a bounded
//!   `ClosedIntegerRelation` equality like `2 + 0 = 2` denotes an
//!   identity the kernel's `refl` genuinely proves rather than an
//!   admitted atom.
//! - `Equal` and the `LessThan`/`LessOrEqual` relations over liftable
//!   integer operands — fixed and `IntegerMath*` alike — instead denote
//!   into one shared mathematical-integer vocabulary: `Int : Type 0`
//!   with each `IntegerMathTerm` interned by exact evaluated value
//!   (open terms intern by the term itself). Closed magnitudes in the
//!   fixed scalar literal vocabulary have shared signed binary definitions;
//!   larger closed values and open terms retain opaque `Int` constants. `IntLt`
//!   and `IntLe` the two relation constants `Π(_ : Int). Π(_ : Int).
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
//! of integer arithmetic or a claim of assumption consistency. The remaining
//! families — subtract-order, the witness-bearing bound rules, multiple-
//! equation or nested transports and denotation-conversion instances
//! outside the supported `Int` vocabulary —
//! denote a *rule-instance decision*: an assumption constant whose type
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
    ContentAlgebra, ContentTerm, IntegerMathTerm, Proposition, PropositionContext, ScalarTerm,
    ScalarType, ValueId,
};

use super::certificate::{
    MathematicalCertificate, certificate_assumption_closure, run_on_verification_stack,
    verify_mathematical_certificate_on_current_thread,
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
    AcceptedPremise, AcceptedProofRule, MathematicalJudgmentReceipt, ProofError, ProofNode,
    ProofRule, equality_rules, integer_bound_rules, integer_order_rules, order_discreteness,
    strict_order_transitivity, subtract_order,
};

/// A bound on the proof nodes one denotation walks — a resource refusal,
/// never a judgment. The canonical wire's own depth bound applies
/// separately to the denoted term.
const MAX_ELABORATION_NODES: u64 = 1 << 16;

mod binary_numerals;

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
    /// by [`certificate_assumption_closure`], never by watching what
    /// conversion unfolded.
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

fn record_premise(premises: &mut Vec<AcceptedPremise>, index: usize, proposition: &Proposition) {
    if !premises
        .iter()
        .any(|premise| premise.index == index && premise.proposition == *proposition)
    {
        premises.push(AcceptedPremise {
            index,
            proposition: proposition.clone(),
        });
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
    denote_bounded_certificate_with_machine_parameters(
        context,
        goal,
        assumptions,
        semantic_axioms,
        &BTreeSet::new(),
        proof,
    )
}

/// The same denotation with the verifier-reconstructed scalar signature
/// roots parameter-custody witness rules may name — exactly as
/// `accept_certificate_with_machine_parameters` scopes them.
pub fn denote_bounded_certificate_with_machine_parameters(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
    proof: &ProofNode,
) -> Result<BoundedDenotation, BoundedDenotationError> {
    run_on_verification_stack(|| {
        denote_bounded_certificate_on_current_thread(
            context,
            goal,
            assumptions,
            semantic_axioms,
            machine_parameter_values,
            proof,
        )
    })
}

/// [`denote_bounded_certificate_with_machine_parameters`] on the caller's
/// stack, for the verification route that already reserves
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

/// Denote and re-decide under the verifier-reconstructed machine
/// parameter values — the full route
/// `accept_certificate_with_machine_parameters` consults.
pub fn verify_bounded_certificate_with_machine_parameters(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
    proof: &ProofNode,
    budget: &mut Budget,
) -> Result<BoundedDenotation, BoundedDenotationError> {
    run_on_verification_stack(|| {
        verify_bounded_certificate_on_current_thread(
            context,
            goal,
            assumptions,
            semantic_axioms,
            machine_parameter_values,
            proof,
            budget,
        )
    })
}

/// [`verify_bounded_certificate_with_machine_parameters`] on the caller's
/// stack. Elaboration recurses over the certificate's proof tree and the
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
    match law {
        IntegerLaw::EqualityToLessOrEqual => pi(
            "x",
            carrier(),
            pi(
                "y",
                carrier(),
                pi("_", eq(v("x"), v("y")), le(v("x"), v("y"))),
            ),
        ),
        IntegerLaw::LessThanToLessOrEqual => pi(
            "x",
            carrier(),
            pi(
                "y",
                carrier(),
                pi("_", lt(v("x"), v("y")), le(v("x"), v("y"))),
            ),
        ),
        IntegerLaw::LessOrEqualTransitivity => pi(
            "x",
            carrier(),
            pi(
                "y",
                carrier(),
                pi(
                    "z",
                    carrier(),
                    pi(
                        "_",
                        le(v("x"), v("y")),
                        pi("_", le(v("y"), v("z")), le(v("x"), v("z"))),
                    ),
                ),
            ),
        ),
        IntegerLaw::LessThanTransitivity => pi(
            "x",
            carrier(),
            pi(
                "y",
                carrier(),
                pi(
                    "z",
                    carrier(),
                    pi(
                        "_",
                        lt(v("x"), v("y")),
                        pi("_", lt(v("y"), v("z")), lt(v("x"), v("z"))),
                    ),
                ),
            ),
        ),
        IntegerLaw::LessThanLessOrEqualTransitivity => pi(
            "x",
            carrier(),
            pi(
                "y",
                carrier(),
                pi(
                    "z",
                    carrier(),
                    pi(
                        "_",
                        lt(v("x"), v("y")),
                        pi("_", le(v("y"), v("z")), lt(v("x"), v("z"))),
                    ),
                ),
            ),
        ),
        IntegerLaw::LessOrEqualLessThanTransitivity => pi(
            "x",
            carrier(),
            pi(
                "y",
                carrier(),
                pi(
                    "z",
                    carrier(),
                    pi(
                        "_",
                        le(v("x"), v("y")),
                        pi("_", lt(v("y"), v("z")), lt(v("x"), v("z"))),
                    ),
                ),
            ),
        ),
        IntegerLaw::LessThanSubstituteLeft => pi(
            "x",
            carrier(),
            pi(
                "y",
                carrier(),
                pi(
                    "z",
                    carrier(),
                    pi(
                        "_",
                        eq(v("x"), v("y")),
                        pi("_", lt(v("x"), v("z")), lt(v("y"), v("z"))),
                    ),
                ),
            ),
        ),
        IntegerLaw::LessThanSubstituteRight => pi(
            "x",
            carrier(),
            pi(
                "y",
                carrier(),
                pi(
                    "z",
                    carrier(),
                    pi(
                        "_",
                        eq(v("y"), v("z")),
                        pi("_", lt(v("x"), v("y")), lt(v("x"), v("z"))),
                    ),
                ),
            ),
        ),
        IntegerLaw::LessOrEqualSubstituteLeft => pi(
            "x",
            carrier(),
            pi(
                "y",
                carrier(),
                pi(
                    "z",
                    carrier(),
                    pi(
                        "_",
                        eq(v("x"), v("y")),
                        pi("_", le(v("x"), v("z")), le(v("y"), v("z"))),
                    ),
                ),
            ),
        ),
        IntegerLaw::LessOrEqualSubstituteRight => pi(
            "x",
            carrier(),
            pi(
                "y",
                carrier(),
                pi(
                    "z",
                    carrier(),
                    pi(
                        "_",
                        eq(v("y"), v("z")),
                        pi("_", le(v("x"), v("y")), le(v("x"), v("z"))),
                    ),
                ),
            ),
        ),
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
    /// Canonical mathematical term → `Int`-typed declaration position.
    /// Closed terms intern by exact evaluated value — so a decided
    /// `IntegerMathEqual` on closed operands denotes `refl`-provable
    /// `Id Int c c` — and open terms intern by the term itself.
    math_terms: BTreeMap<MathTermKey, u32>,
    /// `Primitive` leaf statement → decision-assumption position.
    decisions: HashMap<TermHandle, u32>,
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
            math_terms: BTreeMap::new(),
            decisions: HashMap::new(),
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
        if let Some(&position) = self.carriers.get(&scalar_type) {
            return Ok(self.constant(position));
        }
        let position = self.position()?;
        self.declarations
            .push(Declaration::assumption(0, self.type_zero));
        self.carriers.insert(scalar_type, position);
        Ok(self.constant(position))
    }

    /// The assumption constant a scalar term denotes. A closed integer
    /// term is interned by its evaluated literal — the denotation is by
    /// value, so `2 + 0` and `2` name one constant and a decided
    /// `Equal(2 + 0, 2)` is `refl`-provable rather than admitted.
    fn scalar_term(&mut self, term: &ScalarTerm) -> Result<TermHandle, BoundedDenotationError> {
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
    /// `IntegerMathEqual` on them is `refl`-provable; open terms intern
    /// by the term itself. Fixed scalar magnitudes are binary definitions;
    /// larger closed values retain opaque assumptions so this focused
    /// discreteness encoding adds no numeric acceptance limit.
    fn math_term(&mut self, term: &IntegerMathTerm) -> Result<TermHandle, BoundedDenotationError> {
        let key = match ClosedIntegerEvaluator::default().evaluate_closed(term) {
            Ok(Some(value)) => MathTermKey::Closed(value),
            Ok(None) => MathTermKey::Open(term.clone()),
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
            self.math_terms.insert(key, position);
            return Ok(self.constant(position));
        }
        let ty = self.integer_constant()?;
        let position = self.position()?;
        self.declarations.push(Declaration::assumption(0, ty));
        self.math_terms.insert(key, position);
        Ok(self.constant(position))
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

    /// `ValueEqualityTransport` through the integer vocabulary: a single
    /// proved equation `s ≡ t` moving one `Int` endpoint cites the same
    /// substitution laws `IntegerOrderSubstitution` uses — an `IntLt`/
    /// `IntLe` premise matches the roster directly, and an `Id Int`
    /// premise transports by `J`-composed symmetry and transitivity.
    /// Every other shape (several equations, a non-integer carrier,
    /// transport inside a conjunction or bound) keeps the per-instance
    /// axiom — the roster only spans whole `Int` relations.
    fn transport_evidence(
        &mut self,
        premise_ty: TermHandle,
        premise_evidence: TermHandle,
        equality_ty: TermHandle,
        equality_evidence: TermHandle,
        goal_ty: TermHandle,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        match (
            self.integer_relation(premise_ty),
            self.integer_relation(goal_ty),
        ) {
            (
                Some(IntegerRelation::Equality { .. }),
                Some(IntegerRelation::Equality {
                    left: goal_left,
                    right: goal_right,
                }),
            ) => {
                let Some(IntegerRelation::Equality { left: a, right: b }) =
                    self.integer_relation(premise_ty)
                else {
                    unreachable!("matched above");
                };
                let Some(IntegerRelation::Equality { left: s, right: t }) =
                    self.integer_relation(equality_ty)
                else {
                    return Ok(None);
                };
                let integer = self.integer_constant()?;
                // Left endpoint moved `a → goal_left`, or right moved
                // `b → goal_right`; the equation reads the move in
                // either direction and `sym` reorients as needed.
                if self.arena.structurally_equal(b, goal_right) {
                    let Some(directed) =
                        self.directed_equality(s, t, a, goal_left, equality_evidence)
                    else {
                        return Ok(None);
                    };
                    // `Id gl a` then `Id a b` compose to `Id gl b`.
                    let flipped = self.symmetry(integer, a, goal_left, directed);
                    Ok(Some(self.transitivity(
                        integer,
                        goal_left,
                        a,
                        b,
                        flipped,
                        premise_evidence,
                    )))
                } else if self.arena.structurally_equal(a, goal_left) {
                    let Some(directed) =
                        self.directed_equality(s, t, b, goal_right, equality_evidence)
                    else {
                        return Ok(None);
                    };
                    Ok(Some(self.transitivity(
                        integer,
                        a,
                        b,
                        goal_right,
                        premise_evidence,
                        directed,
                    )))
                } else {
                    Ok(None)
                }
            }
            _ => {
                for endpoint in [0usize, 1usize] {
                    if let Some(term) = self.order_substitution_evidence(
                        premise_ty,
                        premise_evidence,
                        equality_ty,
                        equality_evidence,
                        endpoint,
                        goal_ty,
                    )? {
                        return Ok(Some(term));
                    }
                }
                Ok(None)
            }
        }
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
                // and `IntegerMath*` forms share one type. An unliftable
                // operand (a compound or non-integer carrier) stays in
                // the per-scalar vocabulary: `Id S` for equality, an
                // atom for the orders.
                if let Some(lifted) = lift_fixed_integer_relation(proposition) {
                    self.denote(&lifted)?
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

/// The denotation of one proof tree, tracking the scoped premise roster
/// exactly as `accept_certificate`'s traversal does: `roster` holds the
/// ambient assumptions plus the hypotheses implication and case-analysis
/// rules push and pop, `positions` records the de Bruijn binder each
/// roster entry's evidence binds at, and `depth` counts every enclosing
/// binder — including the intermediate ones a nested `caseTwo` adds that
/// are not themselves cited premises.
struct Elaboration<'a> {
    denotation: Denotation,
    context: &'a PropositionContext,
    /// The ambient assumption roster — the shared checkers read it for
    /// the citations the bound rules record.
    ambient_assumptions: &'a [Proposition],
    axioms: &'a [Proposition],
    /// The verifier-reconstructed scalar signature roots the
    /// parameter-custody witness rules check against.
    machine_parameter_values: &'a BTreeSet<ValueId>,
    /// Ambient assumption count — citations at or beyond it name
    /// discharged branch hypotheses, never recorded premises.
    ambient: usize,
    context_bindings: Vec<TermHandle>,
    expected: TermHandle,
    roster: Vec<Proposition>,
    positions: Vec<u32>,
    depth: u32,
    steps: u64,
    rules: BTreeSet<AcceptedProofRule>,
    assumptions: Vec<AcceptedPremise>,
    semantic_axioms: Vec<AcceptedPremise>,
}

impl<'a> Elaboration<'a> {
    fn new(
        context: &'a PropositionContext,
        goal: &Proposition,
        assumptions: &'a [Proposition],
        semantic_axioms: &'a [Proposition],
        machine_parameter_values: &'a BTreeSet<ValueId>,
    ) -> Result<Self, BoundedDenotationError> {
        let mut denotation = Denotation::new();
        let mut context_bindings = Vec::with_capacity(assumptions.len() + semantic_axioms.len());
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
            assumptions: Vec::new(),
            semantic_axioms: Vec::new(),
        })
    }

    /// The variable naming the premise bound at binder position
    /// `position` — de Bruijn `depth - 1 - position` at the current depth.
    fn variable(&mut self, position: u32) -> TermHandle {
        let index = self.depth - 1 - position;
        self.denotation.arena.insert(Term::Variable(index))
    }

    /// Bind `hypothesis`'s evidence at `domain` while `inside` runs — the
    /// discharged premise of an implication introduction or case branch.
    /// The roster entry lands at the current binder depth, then `depth`
    /// counts the new λ for the body's citations.
    fn with_hypothesis(
        &mut self,
        hypothesis: &Proposition,
        domain: TermHandle,
        inside: impl FnOnce(&mut Self) -> Result<TermHandle, BoundedDenotationError>,
    ) -> Result<TermHandle, BoundedDenotationError> {
        self.roster.push(hypothesis.clone());
        self.positions.push(self.depth);
        self.depth += 1;
        let body = inside(self);
        self.depth -= 1;
        self.positions.pop();
        self.roster.pop();
        let body = body?;
        Ok(self.denotation.arena.insert(Term::Lambda { domain, body }))
    }

    /// An intermediate binder that is not a cited premise — the nested
    /// tagged-sum hypothesis inside a multi-way disjunction elimination.
    /// `depth` still counts it, so outer citations stay aligned.
    fn with_intermediate(
        &mut self,
        domain: TermHandle,
        inside: impl FnOnce(&mut Self, TermHandle) -> Result<TermHandle, BoundedDenotationError>,
    ) -> Result<TermHandle, BoundedDenotationError> {
        self.depth += 1;
        let bound = self.denotation.arena.insert(Term::Variable(0));
        let body = inside(self, bound);
        self.depth -= 1;
        let body = body?;
        Ok(self.denotation.arena.insert(Term::Lambda { domain, body }))
    }

    fn node(&mut self, proof: &ProofNode) -> Result<TermHandle, BoundedDenotationError> {
        self.steps += 1;
        if self.steps > MAX_ELABORATION_NODES {
            return Err(BoundedDenotationError::DepthLimitExceeded);
        }
        self.context.validate(&proof.conclusion).map_err(|error| {
            BoundedDenotationError::Certificate(ProofError::MalformedProposition(error))
        })?;
        match &proof.rule {
            ProofRule::Primitive(judgment) => {
                decide_primitive(self.context, &proof.conclusion, *judgment).map_err(|error| {
                    BoundedDenotationError::Certificate(ProofError::PrimitiveJudgment(error))
                })?;
                self.rules.insert(AcceptedProofRule::Primitive);
                let denoted = self.denotation.denote(&proof.conclusion)?;
                // `refl` proves a definitionally reflexive denotation —
                // `Truth` and every decided `Equal`, whose operands the
                // value interning already unified. Any other decided leaf
                // is a named decision assumption in the signature.
                if let Term::Id { ty, left, right } = self.denotation.arena.get(denoted)
                    && self.denotation.arena.structurally_equal(left, right)
                {
                    return Ok(self.denotation.arena.insert(Term::Refl { ty, value: left }));
                }
                self.denotation.decision(denoted)
            }
            ProofRule::Assumption { index } => self.cited(*index, &proof.conclusion),
            ProofRule::SemanticAxiom { index } => {
                let Some(axiom) = self.semantic_axiom(*index) else {
                    return Err(BoundedDenotationError::Certificate(
                        ProofError::UnknownSemanticAxiom(*index),
                    ));
                };
                if !propositions_match(&axiom, &proof.conclusion) {
                    return Err(BoundedDenotationError::Certificate(
                        ProofError::SemanticAxiomConclusionMismatch(*index),
                    ));
                }
                self.same_denotation(&axiom, &proof.conclusion)?;
                self.rules.insert(AcceptedProofRule::SemanticAxiom);
                record_premise(&mut self.semantic_axioms, *index, &axiom);
                let position = self.ambient + *index;
                let position = u32::try_from(position)
                    .map_err(|_| BoundedDenotationError::DepthLimitExceeded)?;
                Ok(self.variable(position))
            }
            ProofRule::ConjunctionIntroduction(children) => {
                let Proposition::Conjunction(expected) = &proof.conclusion else {
                    return Err(BoundedDenotationError::Certificate(
                        ProofError::RuleConclusionMismatch("conjunction introduction"),
                    ));
                };
                if children.len() != expected.len() {
                    return Err(BoundedDenotationError::Certificate(
                        ProofError::ConjunctionArityMismatch,
                    ));
                }
                let mut terms = Vec::with_capacity(children.len());
                for child in children {
                    terms.push(self.node(child)?);
                }
                for (child, expected) in children.iter().zip(expected.iter()) {
                    if &child.conclusion != expected {
                        return Err(BoundedDenotationError::Certificate(
                            ProofError::ConjunctConclusionMismatch,
                        ));
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
                let pair = self.node(conjunction)?;
                let Proposition::Conjunction(conjuncts) = &conjunction.conclusion else {
                    return Err(BoundedDenotationError::Certificate(
                        ProofError::RulePremiseMismatch("conjunction elimination"),
                    ));
                };
                let selected =
                    conjuncts
                        .get(*conjunct)
                        .ok_or(BoundedDenotationError::Certificate(
                            ProofError::UnknownConjunct(*conjunct),
                        ))?;
                if *selected != proof.conclusion {
                    return Err(BoundedDenotationError::Certificate(
                        ProofError::ConjunctConclusionMismatch,
                    ));
                }
                self.rules.insert(AcceptedProofRule::ConjunctionElimination);
                // In `Σ(d₀, Σ(d₁, … dₙ₋₁))` conjunct `i` is `fst` after
                // `i` `snd`s — except the last, which is the bare
                // rightmost component.
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
                    return Err(BoundedDenotationError::Certificate(
                        ProofError::RuleConclusionMismatch("disjunction introduction"),
                    ));
                };
                if *index >= disjuncts.len() {
                    return Err(BoundedDenotationError::Certificate(
                        ProofError::UnknownDisjunct(*index),
                    ));
                }
                let disjunct_term = self.node(disjunct)?;
                if disjuncts[*index] != disjunct.conclusion {
                    return Err(BoundedDenotationError::Certificate(
                        ProofError::DisjunctConclusionMismatch,
                    ));
                }
                self.rules
                    .insert(AcceptedProofRule::DisjunctionIntroduction);
                Ok(self.disjunction_intro(disjuncts.len(), *index, disjunct_term))
            }
            ProofRule::DisjunctionElimination {
                disjunction,
                branches,
            } => {
                let scrutinee = self.node(disjunction)?;
                let Proposition::Disjunction(disjuncts) = &disjunction.conclusion else {
                    return Err(BoundedDenotationError::Certificate(
                        ProofError::RulePremiseMismatch("disjunction elimination"),
                    ));
                };
                if branches.len() != disjuncts.len() {
                    return Err(BoundedDenotationError::Certificate(
                        ProofError::DisjunctionArityMismatch,
                    ));
                }
                let conclusion = self.denotation.denote(&proof.conclusion)?;
                let term = self.disjunction_elim(disjuncts, branches, scrutinee, conclusion)?;
                for branch in branches {
                    if branch.conclusion != proof.conclusion {
                        return Err(BoundedDenotationError::Certificate(
                            ProofError::DisjunctionBranchConclusionMismatch,
                        ));
                    }
                }
                self.rules.insert(AcceptedProofRule::DisjunctionElimination);
                Ok(term)
            }
            ProofRule::ImplicationIntroduction { body } => {
                let Proposition::Implication {
                    premise,
                    conclusion,
                } = &proof.conclusion
                else {
                    return Err(BoundedDenotationError::Certificate(
                        ProofError::RuleConclusionMismatch("implication introduction"),
                    ));
                };
                let domain = self.denotation.denote(premise)?;
                let premise = premise.clone();
                let term = self.with_hypothesis(&premise, domain, |this| this.node(body))?;
                if &body.conclusion != conclusion.as_ref() {
                    return Err(BoundedDenotationError::Certificate(
                        ProofError::ImplicationConclusionMismatch,
                    ));
                }
                self.rules
                    .insert(AcceptedProofRule::ImplicationIntroduction);
                Ok(term)
            }
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => {
                let function = self.node(implication)?;
                let argument = self.node(premise)?;
                let Proposition::Implication {
                    premise: required,
                    conclusion,
                } = &implication.conclusion
                else {
                    return Err(BoundedDenotationError::Certificate(
                        ProofError::RulePremiseMismatch("implication elimination"),
                    ));
                };
                if premise.conclusion != **required {
                    return Err(BoundedDenotationError::Certificate(
                        ProofError::ImplicationPremiseMismatch,
                    ));
                }
                if &proof.conclusion != conclusion.as_ref() {
                    return Err(BoundedDenotationError::Certificate(
                        ProofError::ImplicationConclusionMismatch,
                    ));
                }
                self.rules.insert(AcceptedProofRule::ImplicationElimination);
                Ok(self
                    .denotation
                    .arena
                    .insert(Term::Apply { function, argument }))
            }
            ProofRule::EqualitySymmetry { equality } => {
                let proof_term = self.node(equality)?;
                let Proposition::Equal(left, right) = &equality.conclusion else {
                    return Err(BoundedDenotationError::Certificate(
                        ProofError::RulePremiseMismatch("equality symmetry"),
                    ));
                };
                if proof.conclusion != Proposition::Equal(right.clone(), left.clone()) {
                    return Err(BoundedDenotationError::Certificate(
                        ProofError::EqualityConclusionMismatch,
                    ));
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
                let first = self.node(left_equals_middle)?;
                let second = self.node(middle_equals_right)?;
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
                    // Every transitivity instance the shared check
                    // accepts denotes `Id` — `Equal`/`IntegerMathEqual`
                    // and `ContentConservation` alike — so `None` is a
                    // defensive fallback for a hypothetical denotation
                    // outside the identity vocabulary, not a live path.
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
                let evidence = self.node(premise)?;
                equality_rules::predicate_denotation_relation(
                    self.context,
                    &premise.conclusion,
                    &proof.conclusion,
                )
                .map_err(BoundedDenotationError::Certificate)?;
                self.rules.insert(AcceptedProofRule::PredicateDenotation);
                let premise_ty = self.denotation.denote(&premise.conclusion)?;
                let goal_ty = self.denotation.denote(&proof.conclusion)?;
                // Both propositions reach the same normalized denotation
                // goal; when their terms already agree — a canonical
                // `Equal`/`IntegerMathEqual` pair — the premise evidence
                // inhabits the goal with no axiom at all.
                if self
                    .denotation
                    .arena
                    .structurally_equal(premise_ty, goal_ty)
                {
                    return Ok(evidence);
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
                let mut premises = Vec::with_capacity(equalities.len() + 1);
                let mut evidence = Vec::with_capacity(equalities.len() + 1);
                premises.push(premise.conclusion.clone());
                evidence.push(self.node(premise)?);
                for equality in equalities {
                    premises.push(equality.conclusion.clone());
                    evidence.push(self.node(equality)?);
                }
                equality_rules::value_equality_transport_relation(
                    self.context,
                    &premise.conclusion,
                    equalities.iter().map(|equality| &equality.conclusion),
                    &proof.conclusion,
                )
                .map_err(BoundedDenotationError::Certificate)?;
                self.rules.insert(AcceptedProofRule::ValueEqualityTransport);
                if equalities.len() == 1 {
                    let premise_ty = self.denotation.denote(&premise.conclusion)?;
                    let equality_ty = self.denotation.denote(&equalities[0].conclusion)?;
                    let goal_ty = self.denotation.denote(&proof.conclusion)?;
                    if let Some(term) = self.denotation.transport_evidence(
                        premise_ty,
                        evidence[0],
                        equality_ty,
                        evidence[1],
                        goal_ty,
                    )? {
                        return Ok(term);
                    }
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
                let difference_evidence = self.node(difference)?;
                let positive_evidence = self.node(positive)?;
                subtract_order::check(
                    &difference.conclusion,
                    &positive.conclusion,
                    &proof.conclusion,
                )
                .map_err(BoundedDenotationError::Certificate)?;
                self.rule_instance(
                    AcceptedProofRule::IntegerSubtractOrder,
                    vec![difference.conclusion.clone(), positive.conclusion.clone()],
                    vec![difference_evidence, positive_evidence],
                    &proof.conclusion,
                )
            }
            ProofRule::IntegerOrderDiscreteness { relation } => {
                let evidence = self.node(relation)?;
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
                let evidence = self.node(relation)?;
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
                let first = self.node(left_less_or_equal_middle)?;
                let second = self.node(middle_less_or_equal_right)?;
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
                let first = self.node(left_to_middle)?;
                let second = self.node(middle_to_right)?;
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
            ProofRule::IntegerOrderSubstitution {
                relation,
                equality,
                endpoint,
            } => {
                let relation_evidence = self.node(relation)?;
                let equality_evidence = self.node(equality)?;
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
                let root = self.node(root_bound)?;
                let cited = integer_bound_rules::affine_bound_relation(
                    self.context,
                    self.axioms,
                    &root_bound.conclusion,
                    witness,
                    &proof.conclusion,
                )
                .map_err(BoundedDenotationError::Certificate)?;
                let mut premises = Vec::with_capacity(cited.len() + 1);
                let mut evidence = Vec::with_capacity(cited.len() + 1);
                premises.push(root_bound.conclusion.clone());
                evidence.push(root);
                for index in cited {
                    let (proposition, variable) = self.cited_axiom(index)?;
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
            } => {
                let left = self.node(left_bound)?;
                let right = self.node(right_bound)?;
                integer_bound_rules::exact_add_definition_bound_relation(
                    self.context,
                    self.axioms,
                    &left_bound.conclusion,
                    &right_bound.conclusion,
                    *definition_axiom,
                    &proof.conclusion,
                )
                .map_err(BoundedDenotationError::Certificate)?;
                let (definition, variable) = self.cited_axiom(*definition_axiom)?;
                self.rule_instance(
                    AcceptedProofRule::IntegerExactAddDefinitionBound,
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
                let root = self.node(root_bound)?;
                integer_bound_rules::cast_bound_relation(
                    self.context,
                    self.axioms,
                    &root_bound.conclusion,
                    witness,
                    &proof.conclusion,
                )
                .map_err(BoundedDenotationError::Certificate)?;
                let mut premises = Vec::with_capacity(witness.definition_axioms.len() + 1);
                let mut evidence = Vec::with_capacity(witness.definition_axioms.len() + 1);
                premises.push(root_bound.conclusion.clone());
                evidence.push(root);
                for &index in &witness.definition_axioms {
                    let (proposition, variable) = self.cited_axiom(index)?;
                    premises.push(proposition);
                    evidence.push(variable);
                }
                self.rule_instance(
                    AcceptedProofRule::IntegerCastBound,
                    premises,
                    evidence,
                    &proof.conclusion,
                )
            }
            ProofRule::IntegerCorrelatedForbiddenRoots { witness } => {
                let citations = integer_bound_rules::correlated_forbidden_roots_relation(
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
                for index in citations.semantic_axioms {
                    let (proposition, variable) = self.cited_axiom(index)?;
                    premises.push(proposition);
                    evidence.push(variable);
                }
                for index in citations.assumptions {
                    let (proposition, variable) = self.ambient_premise(index)?;
                    premises.push(proposition);
                    evidence.push(variable);
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

    /// A premise citation: `Assumption{index}` names `roster[index]`,
    /// which may be an ambient assumption or a discharged branch
    /// hypothesis. The bounded matcher's lift/lower normalization is
    /// honored exactly: `Equal`/`IntegerMathEqual` and the order
    /// relations denote the same `Int`-vocabulary term either side of
    /// the crossing, so a normalized citation binds its premise's own
    /// evidence.
    fn cited(
        &mut self,
        index: usize,
        conclusion: &Proposition,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let Some(assumption) = self.roster.get(index).cloned() else {
            return Err(BoundedDenotationError::Certificate(
                ProofError::UnknownAssumption(index),
            ));
        };
        if !propositions_match(&assumption, conclusion) {
            return Err(BoundedDenotationError::Certificate(
                ProofError::AssumptionConclusionMismatch(index),
            ));
        }
        self.same_denotation(&assumption, conclusion)?;
        self.rules.insert(AcceptedProofRule::Assumption);
        if index < self.ambient {
            record_premise(&mut self.assumptions, index, &assumption);
        }
        let position = self.positions[index];
        Ok(self.variable(position))
    }

    fn semantic_axiom(&self, index: usize) -> Option<Proposition> {
        self.axioms.get(index).cloned()
    }

    /// Citation pre-check for the normalized-but-not-equal case: the
    /// retained and requested propositions must denote the same term.
    /// Every normalized relation the matcher equates denotes identically
    /// now (`Id Int` either side of `Equal`/`IntegerMathEqual`, the same
    /// `IntLt`/`IntLe` applications across lifted orders), so the refusal
    /// below is defensive — the checked denotation fragment no longer
    /// contains a shape crossing it cannot express.
    /// `Err(Unsupported)` remains for a hypothetical normalized pair
    /// whose denotations diverge.
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

    /// One ambient assumption already bound in Γ as a rule-instance
    /// premise: the premise proposition and its context variable, with
    /// the citation recorded exactly as `accept_certificate` reports it.
    fn ambient_premise(
        &mut self,
        index: usize,
    ) -> Result<(Proposition, TermHandle), BoundedDenotationError> {
        let Some(proposition) = self.roster.get(index).cloned() else {
            return Err(BoundedDenotationError::Certificate(
                ProofError::UnknownAssumption(index),
            ));
        };
        if index >= self.ambient {
            return Err(BoundedDenotationError::Certificate(
                ProofError::UnknownAssumption(index),
            ));
        }
        record_premise(&mut self.assumptions, index, &proposition);
        let position = self.positions[index];
        let variable = self.variable(position);
        Ok((proposition, variable))
    }

    /// One ambient semantic axiom already bound in Γ as a rule-instance
    /// premise: axioms bind after the assumptions, so index `i` sits at
    /// binder position `ambient + i`.
    fn cited_axiom(
        &mut self,
        index: usize,
    ) -> Result<(Proposition, TermHandle), BoundedDenotationError> {
        let Some(proposition) = self.axioms.get(index).cloned() else {
            return Err(BoundedDenotationError::Certificate(
                ProofError::UnknownSemanticAxiom(index),
            ));
        };
        record_premise(&mut self.semantic_axioms, index, &proposition);
        let position = u32::try_from(self.ambient + index)
            .map_err(|_| BoundedDenotationError::DepthLimitExceeded)?;
        let variable = self.variable(position);
        Ok((proposition, variable))
    }

    /// The denoted evidence for a rule the kernel does not derive: the
    /// shared relation check has already re-decided the instance, so its
    /// arithmetic or conversion content is a named decision axiom
    /// `Π(_ : ⟦premise₁⟧). … . ⟦conclusion⟧` applied to the premise
    /// evidence in rule order. `premises` and `evidence` are aligned —
    /// child terms and cited ambient bindings alike.
    fn rule_instance(
        &mut self,
        rule: AcceptedProofRule,
        premises: Vec<Proposition>,
        evidence: Vec<TermHandle>,
        conclusion: &Proposition,
    ) -> Result<TermHandle, BoundedDenotationError> {
        debug_assert_eq!(premises.len(), evidence.len());
        let axiom = self.denotation.rule_axiom(&premises, conclusion)?;
        let mut term = axiom;
        for piece in evidence {
            term = self.denotation.arena.insert(Term::Apply {
                function: term,
                argument: piece,
            });
        }
        self.rules.insert(rule);
        Ok(term)
    }

    /// `⟨zero, t⟩` for disjunct 0, `⟨one, inner⟩` for a later one — the
    /// tagged-sum introduction shape `Σ(t : Two). caseTwo(M, d₀, rest, t)`.
    /// `remaining` is how many disjuncts the sum at this level still
    /// holds: the `one` branch of a binary sum is the bare last disjunct,
    /// so its payload is `t` itself, while a wider sum nests another
    /// tagged pair for the disjuncts after the first.
    fn disjunction_intro(
        &mut self,
        remaining: usize,
        index: usize,
        term: TermHandle,
    ) -> TermHandle {
        let tag = self.denotation.arena.insert(if index == 0 {
            Term::TwoZero
        } else {
            Term::TwoOne
        });
        let second = if index == 0 || remaining == 2 {
            term
        } else {
            self.disjunction_intro(remaining - 1, index - 1, term)
        };
        self.denotation
            .arena
            .insert(Term::Pair { first: tag, second })
    }

    /// `caseTwo(λ(t : Two). Π(_ : F t). ⟦C⟧, λ(h : ⟦d₀⟧). b₀, λ(h :
    /// rest). inner, fst s) (snd s)` — the dependent two-elimination a
    /// disjunction elimination denotes, where `F t = caseTwo(M, d₀, rest,
    /// t)` is the tagged sum's family and `inner` recurses into the
    /// nested disjunction when more than two disjuncts remain.
    fn disjunction_elim(
        &mut self,
        disjuncts: &[Proposition],
        branches: &[ProofNode],
        scrutinee: TermHandle,
        conclusion: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let first_denoted = self.denotation.denote(&disjuncts[0])?;
        let zero_branch =
            self.with_hypothesis(&disjuncts[0], first_denoted, |this| this.node(&branches[0]))?;
        let rest = if disjuncts.len() == 2 {
            self.denotation.denote(&disjuncts[1])?
        } else {
            self.denotation.disjunction(&disjuncts[1..])?
        };
        let one_branch = if disjuncts.len() == 2 {
            self.with_hypothesis(&disjuncts[1], rest, |this| this.node(&branches[1]))?
        } else {
            // λ(h : rest). caseTwo(…) — `h` is the nested tagged sum, an
            // intermediate binder that itself is never a cited premise.
            self.with_intermediate(rest, |this, bound| {
                this.disjunction_elim(&disjuncts[1..], &branches[1..], bound, conclusion)
            })?
        };
        let motive = {
            let inner_motive = self.denotation.arena.insert(Term::Lambda {
                domain: self.denotation.two,
                body: self.denotation.type_zero,
            });
            let bound = self.denotation.arena.insert(Term::Variable(0));
            let family = self.denotation.arena.insert(Term::CaseTwo {
                motive: inner_motive,
                zero_branch: first_denoted,
                one_branch: rest,
                scrutinee: bound,
            });
            let body = self.denotation.arena.insert(Term::Pi {
                domain: family,
                codomain: conclusion,
            });
            self.denotation.arena.insert(Term::Lambda {
                domain: self.denotation.two,
                body,
            })
        };
        let tag = self.denotation.arena.insert(Term::Fst { pair: scrutinee });
        let elimination = self.denotation.arena.insert(Term::CaseTwo {
            motive,
            zero_branch,
            one_branch,
            scrutinee: tag,
        });
        let payload = self.denotation.arena.insert(Term::Snd { pair: scrutinee });
        Ok(self.denotation.arena.insert(Term::Apply {
            function: elimination,
            argument: payload,
        }))
    }

    fn finish(self, term: TermHandle) -> BoundedDenotation {
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
            assumptions,
            semantic_axioms,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use semantic_vocabulary::{
        ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentDomainId,
        ContentPlaceSegment, ContentPlaceVersion, ContentProjectionIdentity,
        ContentStructuralPlace, ContentTerm, IntegerSign, IntegerType, IntegerValue, PlaceId,
        PropositionId, ScalarType, StructuralPlaceKind, ValueId,
    };
    use terminal_psi::PrimitiveJudgment;

    use super::{
        AcceptedPremise, AcceptedProofRule, BoundedDenotationError, ProofError, ProofNode,
        ProofRule, Proposition, PropositionContext, ScalarTerm, Term, denote_bounded_certificate,
        verify_bounded_certificate,
    };
    use crate::mathematical_core::{
        Budget, DEFAULT_CONVERSION_STEPS, certificate_assumption_closure,
        verify_mathematical_certificate,
    };

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

    fn literal(value: u128) -> ScalarTerm {
        ScalarTerm::integer(unsigned64(), IntegerValue::Unsigned(value)).expect("u64 literal")
    }

    fn atom(index: u64) -> Proposition {
        Proposition::Atom(PropositionId::new(index).expect("atom id"))
    }

    /// The contract-entailment discharge shape: `[x <= y] ⊢ x <= y` by
    /// `Assumption{0}` — the judgment every shipped producer emits for an
    /// assumption discharge, re-decided by the kernel here.
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
        let proof = ProofNode {
            conclusion: bound.clone(),
            rule: ProofRule::Assumption { index: 0 },
        };
        let denoted = verify_bounded_certificate(
            &context,
            &bound,
            std::slice::from_ref(&bound),
            &[],
            &proof,
            &mut budget(),
        )
        .expect("the discharge judgment verifies in the kernel");

        // `x <= y` over integers lifts to `IntLe m_x m_y`: the
        // signature is `{Int : Type 0, IntLe : Π(_:Int).Π(_:Int).Type 0,
        // m_x : Int, m_y : Int}` and the judgment
        // `Γ; [IntLe m_x m_y] ⊢ p : IntLe m_x m_y` — the term is the
        // bound variable.
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

    /// A discharged hypothesis is a λ binder inside the evidence, never a
    /// judgment premise: `(P ∧ Q) → (Q ∧ P)` is a closed proof whose kernel
    /// judgment has an empty context.
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

    /// `Equal` denotes the relevant `Id`, so equality symmetry and
    /// transitivity become `J` eliminations the kernel re-decides.
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
                left_equals_middle: Box::new(ProofNode {
                    conclusion: first.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                }),
                middle_equals_right: Box::new(ProofNode {
                    conclusion: second.clone(),
                    rule: ProofRule::Assumption { index: 1 },
                }),
            },
        };
        let denoted = verify_bounded_certificate(
            &context,
            &goal,
            &[first, second],
            &[],
            &proof,
            &mut budget(),
        )
        .expect("transitivity verifies as J");
        // `Int` plus one assumption constant per mathematical endpoint;
        // the lifted `Id Int` chain composes by `J`.
        assert_eq!(denoted.certificate.signature.len(), 4);
        assert_eq!(denoted.certificate.context.len(), 2);
        assert_eq!(
            certificate_assumption_closure(&denoted.arena, &denoted.certificate),
            BTreeSet::from([0, 1, 2, 3]),
        );

        // Symmetry: `x = y ⊢ y = x`.
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
                equality: Box::new(ProofNode {
                    conclusion: source.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                }),
            },
        };
        let denoted =
            verify_bounded_certificate(&context, &flipped, &[source], &[], &proof, &mut budget())
                .expect("symmetry verifies");
        // `{Int, m_x, m_y}` — canonical `IntegerMathEqual` order makes
        // the flipped goal's denotation the premise's own, so the cited
        // variable is the whole evidence.
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

    /// An integer order rule whose instance lands wholly in the `Int`
    /// vocabulary denotes a roster-law application: `x < y ⊢ x <= y`
    /// elaborates to `lt_le m_x m_y p`, the shared relation check
    /// re-decides the instance before the law is cited, and the closure
    /// names the law exactly.
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
                relation: Box::new(ProofNode {
                    conclusion: strict.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                }),
            },
        };
        let denoted = verify_bounded_certificate(
            &context,
            &weakened,
            std::slice::from_ref(&strict),
            &[],
            &proof,
            &mut budget(),
        )
        .expect("order weakening verifies as a roster citation");
        // The signature is `{Int, IntLt, m_x, m_y, IntLe, lt_le}`: the
        // evidence is the roster constant applied to the endpoints and
        // the cited premise variable — `lt_le m_x m_y p`.
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

        // A relation the rule does not license — a nonstrict premise —
        // is the same `RulePremiseMismatch` the bounded checker reports,
        // never an axiom.
        let proof = ProofNode {
            conclusion: weakened.clone(),
            rule: ProofRule::IntegerOrderWeakening {
                relation: Box::new(ProofNode {
                    conclusion: weakened.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                }),
            },
        };
        assert!(matches!(
            denote_bounded_certificate(
                &context,
                &weakened,
                std::slice::from_ref(&weakened),
                &[],
                &proof,
            ),
            Err(BoundedDenotationError::Certificate(
                ProofError::RulePremiseMismatch(_)
            )),
        ));
    }

    /// Mathematical-integer transitivity — `IntegerMathEqual` premises
    /// and the normalized `IntegerMathEqual` conclusion of an `Equal`
    /// chain alike — denotes `J` on the `Int` vocabulary; the
    /// orientation fixups ride on canonical endpoint order.
    #[test]
    fn integer_math_transitivity_becomes_identity_eliminations() {
        // `IntegerMathEqual` transitivity: `a = b`, `b = c` ⊢ `a = c`
        // over mathematical literals.
        let math = |value: u128| {
            semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Unsigned(value))
        };
        let first = Proposition::IntegerMathEqual(math(1), math(2));
        let second = Proposition::IntegerMathEqual(math(2), math(3));
        let goal = Proposition::IntegerMathEqual(math(1), math(3));
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::EqualityTransitivity {
                left_equals_middle: Box::new(ProofNode {
                    conclusion: first.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                }),
                middle_equals_right: Box::new(ProofNode {
                    conclusion: second.clone(),
                    rule: ProofRule::Assumption { index: 1 },
                }),
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

        // The normalized crossing: `Equal` premises composing to the
        // lifted `IntegerMathEqual` conclusion.
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
                left_equals_middle: Box::new(ProofNode {
                    conclusion: first.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                }),
                middle_equals_right: Box::new(ProofNode {
                    conclusion: second.clone(),
                    rule: ProofRule::Assumption { index: 1 },
                }),
            },
        };
        let denoted = verify_bounded_certificate(
            &context,
            &goal,
            &[first, second],
            &[],
            &proof,
            &mut budget(),
        )
        .expect("the normalized conclusion verifies as J");
        assert!(matches!(
            denoted.arena.get(denoted.certificate.term),
            Term::IdElim { .. },
        ));
    }

    /// The conversion rules — predicate denotation and value-equality
    /// transport — denote named rule-instance decisions whose premises
    /// are the cited child and each proved equation.
    #[test]
    fn conversion_rules_become_named_rule_instances() {
        let (x_id, x) = value(1);
        let (y_id, y) = value(2);
        let (z_id, z) = value(3);
        let context = PropositionContext::from_value_types([
            (x_id, unsigned64_type()),
            (y_id, unsigned64_type()),
            (z_id, unsigned64_type()),
        ])
        .expect("context");

        // `PredicateDenotation`: `y == x` converts to its canonical `x
        // == y` — different propositions, one canonical `Id Int`
        // denotation, so the cited premise variable is the whole
        // evidence with no axiom at all.
        let premise = Proposition::Equal(y.clone(), x.clone());
        let goal = Proposition::Equal(x, y);
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::PredicateDenotation {
                premise: Box::new(ProofNode {
                    conclusion: premise.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                }),
            },
        };
        let denoted = verify_bounded_certificate(
            &context,
            &goal,
            std::slice::from_ref(&premise),
            &[],
            &proof,
            &mut budget(),
        )
        .expect("predicate denotation verifies definitionally");
        assert_eq!(
            denoted.arena.get(denoted.certificate.term),
            Term::Variable(0),
        );

        // `ValueEqualityTransport`: `x <= y` under the proved `x == z`
        // transports to `z <= y` — a single integer-order move cites
        // the same substitution law the order rule uses.
        let premise = Proposition::LessOrEqual(
            ScalarTerm::value(x_id, unsigned64_type()),
            ScalarTerm::value(y_id, unsigned64_type()),
        );
        let equation = Proposition::Equal(ScalarTerm::value(x_id, unsigned64_type()), z.clone());
        let goal = Proposition::LessOrEqual(z, ScalarTerm::value(y_id, unsigned64_type()));
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::ValueEqualityTransport {
                premise: Box::new(ProofNode {
                    conclusion: premise.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                }),
                equalities: vec![ProofNode {
                    conclusion: equation.clone(),
                    rule: ProofRule::Assumption { index: 1 },
                }],
            },
        };
        let denoted = verify_bounded_certificate(
            &context,
            &goal,
            &[premise, equation],
            &[],
            &proof,
            &mut budget(),
        )
        .expect("value equality transport verifies as a roster citation");
        // `le_subst_left m_x m_z m_y eq p` — the roster law applied to
        // the three endpoints, the equation evidence, and the premise.
        let mut head = denoted.certificate.term;
        for _ in 0..5 {
            let Term::Apply { function, .. } = denoted.arena.get(head) else {
                panic!("transport cites the substitution law applied to endpoints and evidence");
            };
            head = function;
        }
        assert!(matches!(denoted.arena.get(head), Term::Constant { .. }));
        assert_eq!(
            denoted.rules,
            vec![
                AcceptedProofRule::Assumption,
                AcceptedProofRule::ValueEqualityTransport
            ],
        );
    }

    /// `IntegerOrderSubstitution` cites the roster's substitution law
    /// over the `Int` denotations of the relation and equality
    /// premises; the substituted endpoint is the shared check's
    /// decision, not a separate denotation.
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
                relation: Box::new(ProofNode {
                    conclusion: relation.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                }),
                equality: Box::new(ProofNode {
                    conclusion: equality.clone(),
                    rule: ProofRule::Assumption { index: 1 },
                }),
                endpoint: 0,
            },
        };
        let denoted = verify_bounded_certificate(
            &context,
            &goal,
            &[relation, equality],
            &[],
            &proof,
            &mut budget(),
        )
        .expect("order substitution verifies as a roster citation");
        // `le_subst_left m_x m_z m_y eq p` — five `Apply`s to the law
        // constant: three endpoints, the equation, the relation.
        let mut head = denoted.certificate.term;
        for _ in 0..5 {
            let Term::Apply { function, .. } = denoted.arena.get(head) else {
                panic!("a substitution citation applies endpoints then evidence");
            };
            head = function;
        }
        assert!(matches!(denoted.arena.get(head), Term::Constant { .. }));

        // Substituting an endpoint the equality does not license is the
        // shared checker's mismatch, never an axiom.
        let wrong = Proposition::LessOrEqual(z.clone(), literal(9));
        let proof = ProofNode {
            conclusion: wrong.clone(),
            rule: ProofRule::IntegerOrderSubstitution {
                relation: Box::new(ProofNode {
                    conclusion: Proposition::LessOrEqual(
                        ScalarTerm::value(x_id, unsigned64_type()),
                        ScalarTerm::value(y_id, unsigned64_type()),
                    ),
                    rule: ProofRule::Assumption { index: 0 },
                }),
                equality: Box::new(ProofNode {
                    conclusion: Proposition::Equal(
                        ScalarTerm::value(x_id, unsigned64_type()),
                        z.clone(),
                    ),
                    rule: ProofRule::Assumption { index: 1 },
                }),
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

    /// A decided closed equality denotes a reflexive identity: `2 + 0 = 2`
    /// is proved by the kernel's own `refl`, not by an admitted decision.
    #[test]
    fn a_decided_closed_equality_is_refl_in_the_kernel() {
        let two = literal(2);
        let zero = literal(0);
        let sum = ScalarTerm::wrapping_integer_add(unsigned64(), two.clone(), zero)
            .expect("wrapping add");
        let goal = Proposition::Equal(sum, two);
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
        };
        let denoted = verify_bounded_certificate(
            &PropositionContext::default(),
            &goal,
            &[],
            &[],
            &proof,
            &mut budget(),
        )
        .expect("the decided equality denotes a refl-provable Id");
        // Carrier `S` plus the single canonical literal `2 : S`; the
        // evidence is `refl S 2`.
        assert_eq!(denoted.certificate.signature.len(), 2);
        assert!(denoted.certificate.context.is_empty());
        assert!(matches!(
            denoted.arena.get(denoted.certificate.term),
            Term::Refl { .. }
        ));
        assert_eq!(
            certificate_assumption_closure(&denoted.arena, &denoted.certificate),
            BTreeSet::from([0, 1]),
        );
    }

    /// A `Primitive` leaf the calculus cannot prove definitionally — a
    /// decided closed order `1 < 2` — is a named decision assumption in the
    /// signature, so the closure names the bounded decision exactly.
    #[test]
    fn a_decision_assumption_is_named_in_the_closure() {
        let goal = Proposition::LessThan(literal(1), literal(2));
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
        };
        let denoted = verify_bounded_certificate(
            &PropositionContext::default(),
            &goal,
            &[],
            &[],
            &proof,
            &mut budget(),
        )
        .expect("the decided order verifies");
        // `IntLt` is a relation, not an `Id`, so the decided `1 < 2`
        // names a decision assumption of type `IntLt m₁ m₂` — the
        // signature is `{Int, IntLt, zero, odd, one:=odd zero,
        // double, two:=double one, decision}`. Literal definitions do
        // not appear as assumptions; their vocabulary does.
        assert_eq!(denoted.certificate.signature.len(), 8);
        assert_eq!(
            certificate_assumption_closure(&denoted.arena, &denoted.certificate),
            BTreeSet::from([0, 1, 2, 3, 5, 7]),
            "carrier, relation, numeral vocabulary and the named decision",
        );

        // `Truth` is definitional: no signature, no decision assumption.
        let proof = ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
        };
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

    /// A disjunction denotes the tagged sum `Σ(t : Two). caseTwo(M, d₀,
    /// rest, t)`: introduction pairs a `zero`/`one` tag with the nested
    /// payload, and elimination is the dependent `caseTwo` applied to the
    /// payload — for three disjuncts the one-branch's nested sum is an
    /// intermediate binder while each branch's hypothesis stays one cited
    /// premise.
    #[test]
    fn disjunction_rules_become_tagged_sum_constructions() {
        let p = atom(1);
        let q = atom(2);
        let r = atom(3);
        let truth_node = || ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
        };

        // Introduction: `[P] ⊢ P ∨ Q` pairs `zero` with the cited premise.
        let disjunction = Proposition::Disjunction(vec![p.clone(), q.clone()]);
        let proof = ProofNode {
            conclusion: disjunction.clone(),
            rule: ProofRule::DisjunctionIntroduction {
                disjunct: Box::new(ProofNode {
                    conclusion: p.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                }),
                index: 0,
            },
        };
        let denoted = verify_bounded_certificate(
            &PropositionContext::default(),
            &disjunction,
            std::slice::from_ref(&p),
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

        // Every other position pairs `one` with the payload the sum's
        // `one` branch actually holds: the bare last disjunct of a binary
        // sum, or the nested tagged pair of a wider one. The kernel checks
        // the payload at `caseTwo(M, d₀, rest, one) ≡ rest`, so a payload
        // nested one level too deep is a type error, not a shape quibble.
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
                    disjunct: Box::new(ProofNode {
                        conclusion: selected.clone(),
                        rule: ProofRule::Assumption { index: 0 },
                    }),
                    index,
                },
            };
            let denoted = verify_bounded_certificate(
                &PropositionContext::default(),
                &disjunction,
                std::slice::from_ref(&selected),
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

        // Two-branch elimination under one cited disjunction.
        let proof = ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::DisjunctionElimination {
                disjunction: Box::new(ProofNode {
                    conclusion: disjunction.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                }),
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

        // Three-branch elimination nests one intermediate tagged-sum
        // hypothesis the branches never cite.
        let disjunction = Proposition::Disjunction(vec![p, q, r]);
        let proof = ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::DisjunctionElimination {
                disjunction: Box::new(ProofNode {
                    conclusion: disjunction.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                }),
                branches: vec![truth_node(), truth_node(), truth_node()],
            },
        };
        let denoted = verify_bounded_certificate(
            &PropositionContext::default(),
            &Proposition::Truth,
            std::slice::from_ref(&disjunction),
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

    /// Semantic axioms bind after the ambient assumptions; a cited axiom is
    /// a context variable and a recorded premise.
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
                        implication: Box::new(ProofNode {
                            conclusion: implication,
                            rule: ProofRule::Assumption { index: 0 },
                        }),
                        premise: Box::new(ProofNode {
                            conclusion: p.clone(),
                            rule: ProofRule::SemanticAxiom { index: 0 },
                        }),
                    },
                }),
            },
        };
        let denoted = verify_bounded_certificate(
            &PropositionContext::default(),
            &goal,
            &[],
            std::slice::from_ref(&p),
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

    /// Forged and malformed certificates reject at the same points the
    /// bounded checker rejects them; a tampered denoted certificate rejects
    /// in the kernel.
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

        // A citation past the roster is an unknown assumption.
        let proof = ProofNode {
            conclusion: bound.clone(),
            rule: ProofRule::Assumption { index: 1 },
        };
        assert!(matches!(
            denote_bounded_certificate(&context, &bound, std::slice::from_ref(&bound), &[], &proof),
            Err(BoundedDenotationError::Certificate(
                ProofError::UnknownAssumption(1)
            )),
        ));

        // A citation whose conclusion is not the retained premise mismatches.
        let proof = ProofNode {
            conclusion: swapped.clone(),
            rule: ProofRule::Assumption { index: 0 },
        };
        assert!(matches!(
            denote_bounded_certificate(
                &context,
                &swapped,
                std::slice::from_ref(&bound),
                &[],
                &proof
            ),
            Err(BoundedDenotationError::Certificate(
                ProofError::AssumptionConclusionMismatch(0)
            )),
        ));

        // A proof of the obligation's sibling does not discharge it.
        let proof = ProofNode {
            conclusion: bound.clone(),
            rule: ProofRule::Assumption { index: 0 },
        };
        assert!(matches!(
            denote_bounded_certificate(
                &context,
                &Proposition::Truth,
                std::slice::from_ref(&bound),
                &[],
                &proof
            ),
            Err(BoundedDenotationError::Certificate(
                ProofError::CertificateConclusionMismatch
            )),
        ));

        // A primitive label on a false relation never decides it.
        let falsehood = Proposition::LessThan(literal(2), literal(1));
        let proof = ProofNode {
            conclusion: falsehood.clone(),
            rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
        };
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

        // A kernel-tampered term rejects: the variable is unbound.
        let proof = ProofNode {
            conclusion: bound.clone(),
            rule: ProofRule::Assumption { index: 0 },
        };
        let mut denoted =
            denote_bounded_certificate(&context, &bound, std::slice::from_ref(&bound), &[], &proof)
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

    /// The citation-level `Equal`↔`IntegerMathEqual` crossing denotes
    /// identically now: both sides name `Id Int` over the shared `Int`
    /// vocabulary, so the normalized citation binds the premise's own
    /// variable — the `Id`-versus-atom shape crossing is closed.
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
        let proof = ProofNode {
            conclusion: lifted.clone(),
            rule: ProofRule::Assumption { index: 0 },
        };
        assert_eq!(
            crate::check_certificate(&context, &lifted, std::slice::from_ref(&fixed), &[], &proof),
            Ok(()),
            "the bounded checker accepts the normalized citation",
        );
        let denoted = verify_bounded_certificate(
            &context,
            &lifted,
            std::slice::from_ref(&fixed),
            &[],
            &proof,
            &mut budget(),
        )
        .expect("the crossing denotes the same `Id Int` and verifies");
        // `{Int, m_x, m_y}`; the cited variable is the evidence.
        assert_eq!(denoted.certificate.signature.len(), 3);
        assert_eq!(
            denoted.arena.get(denoted.certificate.term),
            Term::Variable(0),
        );
    }

    /// An uncited ambient premise still belongs to the judgment: the
    /// closure is exact about what the *judgment* commits to, which is the
    /// full context — the same premise list `accept_certificate` scopes.
    #[test]
    fn the_closure_names_every_judgment_premise() {
        let p = atom(1);
        let q = atom(2);
        let proof = ProofNode {
            conclusion: p.clone(),
            rule: ProofRule::Assumption { index: 0 },
        };
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
            [(
                root,
                StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            )],
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
        // Every shared-endpoint arrangement the checker's
        // `ContentConservation` arm accepts: direct chaining, right-right
        // and left-left sharing, and the cross-side middle — the `J`
        // composition wraps the needed premises in `sym`.
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
                    left_equals_middle: Box::new(ProofNode {
                        conclusion: first.clone(),
                        rule: ProofRule::SemanticAxiom { index: 0 },
                    }),
                    middle_equals_right: Box::new(ProofNode {
                        conclusion: second.clone(),
                        rule: ProofRule::SemanticAxiom { index: 1 },
                    }),
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
            // `{C, a', b', c'}` — the algebra carrier and one element
            // constant per distinct content term; the rule cites no
            // axiom of its own.
            assert_eq!(denoted.certificate.signature.len(), 4);
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
                BTreeSet::from([0, 1, 2, 3]),
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

    /// A reflexive content conservation denotes `refl` on the algebra
    /// carrier — kernel-proved, not a named decision — over `Separate`
    /// endpoints as well as bare projections.
    #[test]
    fn reflexive_content_conservation_is_refl_in_the_kernel() {
        let root = content_root();
        let algebra = content_algebra();
        let context = content_context(root);
        let separate = ContentTerm::separate([content_term(root, "a"), content_term(root, "b")])
            .expect("separation");
        let proposition = conservation(&algebra, &separate, &separate);
        let proof = ProofNode {
            conclusion: proposition.clone(),
            rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
        };
        let denoted =
            verify_bounded_certificate(&context, &proposition, &[], &[], &proof, &mut budget())
                .expect("reflexive conservation verifies as refl");
        // `{C, s'}` — the carrier and the one endpoint; the evidence is
        // `refl C s'` with no decision assumption.
        assert_eq!(denoted.certificate.signature.len(), 2);
        assert!(matches!(
            denoted.arena.get(denoted.certificate.term),
            Term::Refl { .. }
        ));
        assert_eq!(
            certificate_assumption_closure(&denoted.arena, &denoted.certificate),
            BTreeSet::from([0, 1]),
        );
    }

    /// Content transitivity still rejects at the shared checker's own
    /// points: different algebras, no shared endpoint, or a conclusion
    /// that is not the composed equation.
    #[test]
    fn content_conservation_transitivity_rejects_at_the_shared_check() {
        let root = content_root();
        let algebra = content_algebra();
        let terms = |field: &str| content_term(root, field);
        let context = content_context(root);
        let transitivity = |first: Proposition, second: Proposition, goal: Proposition| ProofNode {
            conclusion: goal,
            rule: ProofRule::EqualityTransitivity {
                left_equals_middle: Box::new(ProofNode {
                    conclusion: first,
                    rule: ProofRule::SemanticAxiom { index: 0 },
                }),
                middle_equals_right: Box::new(ProofNode {
                    conclusion: second,
                    rule: ProofRule::SemanticAxiom { index: 1 },
                }),
            },
        };

        // A shared endpoint across different algebras is an algebra
        // mismatch — the denotation would put the equations over
        // different carriers anyway.
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

        // No shared endpoint at all.
        let first = conservation(&algebra, &terms("a"), &terms("b"));
        let second = conservation(&algebra, &terms("c"), &terms("d"));
        let proof = transitivity(first.clone(), second.clone(), goal.clone());
        assert!(matches!(
            denote_bounded_certificate(&context, &goal, &[], &[first, second], &proof),
            Err(BoundedDenotationError::Certificate(
                ProofError::EqualityMiddleMismatch
            )),
        ));

        // A shared endpoint but a conclusion that is not the composed
        // equation.
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

    /// A rule-instance axiom with a content premise states the exact
    /// `Id C` domain: a value-equality transport that cannot rewrite the
    /// content equation keeps it as an interned `Π` premise, auditable
    /// rather than atom-shaped.
    #[test]
    fn rule_instances_carry_content_identities_exactly() {
        let root = content_root();
        let algebra = content_algebra();
        let premise = conservation(&algebra, &content_term(root, "a"), &content_term(root, "b"));
        let (x_id, x) = value(1);
        let (z_id, z) = value(3);
        let equation = Proposition::Equal(x, z);
        let context = PropositionContext::from_value_types_and_places(
            [(x_id, unsigned64_type()), (z_id, unsigned64_type())],
            [(
                root,
                StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            )],
        )
        .expect("context");
        let proof = ProofNode {
            conclusion: premise.clone(),
            rule: ProofRule::ValueEqualityTransport {
                premise: Box::new(ProofNode {
                    conclusion: premise.clone(),
                    rule: ProofRule::SemanticAxiom { index: 0 },
                }),
                equalities: vec![ProofNode {
                    conclusion: equation.clone(),
                    rule: ProofRule::SemanticAxiom { index: 1 },
                }],
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
        .expect("the transport instance verifies");
        // `{C, a', b', Int, m_x, m_z, axiom}`: the axiom's statement is
        // `Π(_ : Id C a' b'). Π(_ : Id Int m_x m_z). Id C a' b'` — the
        // content identity appears exactly, not as an opaque atom.
        assert_eq!(denoted.certificate.signature.len(), 7);
        let mut head = denoted.certificate.term;
        for _ in 0..2 {
            let Term::Apply { function, .. } = denoted.arena.get(head) else {
                panic!("a rule instance applies the axiom to each premise");
            };
            head = function;
        }
        let Term::Constant {
            declaration: axiom_position,
            ..
        } = denoted.arena.get(head)
        else {
            panic!("the head is the axiom constant");
        };
        let Term::Pi { domain, .. } = denoted
            .arena
            .get(denoted.certificate.signature[axiom_position as usize].ty)
        else {
            panic!("the axiom's statement is a Π over its premises");
        };
        assert!(
            matches!(denoted.arena.get(domain), Term::Id { .. }),
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
