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
//! - Every other atomic proposition denotes an assumption constant
//!   `P : Type 0` — the only axiom shape the calculus admits. Atoms are
//!   interned up to the lift/lower carrier normalization
//!   `accept_certificate` applies to cited premises, so a cited
//!   `LessOrEqual` and a claimed `IntegerMathLessOrEqual` share one
//!   constant.
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
//! exactly the atoms, scalar carriers, scalar terms and bounded decisions
//! the judgment depends on, and nothing else.
//!
//! The denotation covers the certificate language's propositional and
//! scalar-identity fragment: primitive, assumption and semantic-axiom
//! leaves, conjunction, disjunction and implication introduction and
//! elimination, and equality symmetry/transitivity over `Equal`. The
//! integer order, bound and correlated-root rules, the denotation-
//! conversion rules, and the `IntegerMath*`/`ContentConservation`
//! equality shapes are valid certificates this route refuses as
//! [`BoundedDenotationError::Unsupported`] — refused before their
//! children are denoted, never decided against their conclusions. The
//! bounded checker remains the route for those families.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm, ScalarType};

use super::certificate::{
    MathematicalCertificate, certificate_assumption_closure, verify_mathematical_certificate,
};
use super::conversion::Budget;
use super::signature::Declaration;
use super::term::{Level, Sort, Term, TermArena, TermHandle};
use super::typing::CoreError;
use crate::kernel::decide_primitive;
use crate::proof::integer_math_normalization::propositions_match_under_integer_math_normalization;
use crate::proof::{
    AcceptedPremise, AcceptedProofRule, MathematicalJudgmentReceipt, ProofError, ProofNode,
    ProofRule,
};

/// A bound on the proof nodes one denotation walks — a resource refusal,
/// never a judgment. The canonical wire's own depth bound applies
/// separately to the denoted term.
const MAX_ELABORATION_NODES: u64 = 1 << 16;

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
    /// A valid bounded certificate family the denotation does not cover —
    /// integer order/bound rules, denotation conversions, and non-`Equal`
    /// equality shapes. Refused, never decided false.
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
/// performs on the covered fragment: rule labels and node conclusions
/// are re-derived, never trusted. The returned certificate is still only
/// evidence — [`verify_bounded_certificate`] or an explicit
/// [`verify_mathematical_certificate`] call decides it.
pub fn denote_bounded_certificate(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
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
    let mut elaboration = Elaboration::new(context, goal, assumptions, semantic_axioms)?;
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
    let mut denoted =
        denote_bounded_certificate(context, goal, assumptions, semantic_axioms, proof)?;
    verify_mathematical_certificate(&mut denoted.arena, &denoted.certificate, budget)
        .map_err(BoundedDenotationError::Kernel)?;
    Ok(denoted)
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
    /// `Primitive` leaf statement → decision-assumption position.
    decisions: HashMap<TermHandle, u32>,
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
            decisions: HashMap::new(),
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

    /// The `Type 0` assumption constant an atomic proposition denotes.
    /// The interning key is the lifted mathematical form, so a retained
    /// `LessOrEqual` and a requested `IntegerMathLessOrEqual` — which the
    /// bounded citation matcher equates — denote the same constant.
    fn atom(&mut self, proposition: &Proposition) -> Result<TermHandle, BoundedDenotationError> {
        let key =
            crate::lift_fixed_integer_relation(proposition).unwrap_or_else(|| proposition.clone());
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
            Proposition::Equal(left, right) => {
                let ty = self.carrier(left.scalar_type())?;
                let left = self.scalar_term(left)?;
                let right = self.scalar_term(right)?;
                self.arena.insert(Term::Id { ty, left, right })
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
    axioms: &'a [Proposition],
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
            axioms: semantic_axioms,
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
                let carrier = self.denotation.carrier(left.scalar_type())?;
                let left = self.denotation.scalar_term(left)?;
                let right = self.denotation.scalar_term(right)?;
                // `J(λ(y : S). λ(_ : Id S l y). Id S y l, refl S l, r, p)`
                // — symmetry as the identity eliminator, re-decided.
                let motive = {
                    let bound = self.denotation.arena.insert(Term::Variable(0));
                    let identity = self.denotation.arena.insert(Term::Id {
                        ty: carrier,
                        left,
                        right: bound,
                    });
                    let bound = self.denotation.arena.insert(Term::Variable(1));
                    let body = self.denotation.arena.insert(Term::Id {
                        ty: carrier,
                        left: bound,
                        right: left,
                    });
                    let inner = self.denotation.arena.insert(Term::Lambda {
                        domain: identity,
                        body,
                    });
                    self.denotation.arena.insert(Term::Lambda {
                        domain: carrier,
                        body: inner,
                    })
                };
                let base = self.denotation.arena.insert(Term::Refl {
                    ty: carrier,
                    value: left,
                });
                Ok(self.denotation.arena.insert(Term::IdElim {
                    motive,
                    base,
                    endpoint: right,
                    proof: proof_term,
                }))
            }
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => {
                let first = self.node(left_equals_middle)?;
                let second = self.node(middle_equals_right)?;
                match (
                    &left_equals_middle.conclusion,
                    &middle_equals_right.conclusion,
                ) {
                    (
                        Proposition::Equal(left, first_middle),
                        Proposition::Equal(second_middle, right),
                    ) => {
                        if first_middle != second_middle {
                            return Err(BoundedDenotationError::Certificate(
                                ProofError::EqualityMiddleMismatch,
                            ));
                        }
                        let composed = Proposition::Equal(left.clone(), right.clone());
                        if composed != proof.conclusion {
                            if propositions_match(&composed, &proof.conclusion) {
                                return Err(BoundedDenotationError::Unsupported(
                                    "integer-math-normalized equality conclusion",
                                ));
                            }
                            return Err(BoundedDenotationError::Certificate(
                                ProofError::EqualityConclusionMismatch,
                            ));
                        }
                        self.rules.insert(AcceptedProofRule::EqualityTransitivity);
                        let carrier = self.denotation.carrier(left.scalar_type())?;
                        let left = self.denotation.scalar_term(left)?;
                        let middle = self.denotation.scalar_term(first_middle)?;
                        let right = self.denotation.scalar_term(right)?;
                        // `J(λ(y : S). λ(_ : Id S m y). Id S l y, p₁, r, p₂)`
                        // — transitivity as the identity eliminator.
                        let motive = {
                            let bound = self.denotation.arena.insert(Term::Variable(0));
                            let identity = self.denotation.arena.insert(Term::Id {
                                ty: carrier,
                                left: middle,
                                right: bound,
                            });
                            let bound = self.denotation.arena.insert(Term::Variable(1));
                            let body = self.denotation.arena.insert(Term::Id {
                                ty: carrier,
                                left,
                                right: bound,
                            });
                            let inner = self.denotation.arena.insert(Term::Lambda {
                                domain: identity,
                                body,
                            });
                            self.denotation.arena.insert(Term::Lambda {
                                domain: carrier,
                                body: inner,
                            })
                        };
                        Ok(self.denotation.arena.insert(Term::IdElim {
                            motive,
                            base: first,
                            endpoint: right,
                            proof: second,
                        }))
                    }
                    (Proposition::IntegerMathEqual(..), Proposition::IntegerMathEqual(..)) => {
                        Err(BoundedDenotationError::Unsupported(
                            "mathematical-integer equality transitivity",
                        ))
                    }
                    (
                        Proposition::ContentConservation(..),
                        Proposition::ContentConservation(..),
                    ) => Err(BoundedDenotationError::Unsupported(
                        "content-conservation equality transitivity",
                    )),
                    _ => Err(BoundedDenotationError::Certificate(
                        ProofError::RulePremiseMismatch("equality transitivity"),
                    )),
                }
            }
            ProofRule::PredicateDenotation { .. } => {
                Err(BoundedDenotationError::Unsupported("predicate denotation"))
            }
            ProofRule::ValueEqualityTransport { .. } => Err(BoundedDenotationError::Unsupported(
                "value equality transport",
            )),
            ProofRule::IntegerSubtractOrder { .. } => Err(BoundedDenotationError::Unsupported(
                "integer subtract order",
            )),
            ProofRule::IntegerOrderDiscreteness { .. } => Err(BoundedDenotationError::Unsupported(
                "integer order discreteness",
            )),
            ProofRule::IntegerOrderWeakening { .. } => Err(BoundedDenotationError::Unsupported(
                "integer order weakening",
            )),
            ProofRule::IntegerLessOrEqualTransitivity { .. } => Err(
                BoundedDenotationError::Unsupported("integer <= transitivity"),
            ),
            ProofRule::IntegerStrictOrderTransitivity { .. } => Err(
                BoundedDenotationError::Unsupported("integer strict order transitivity"),
            ),
            ProofRule::IntegerOrderSubstitution { .. } => Err(BoundedDenotationError::Unsupported(
                "integer order substitution",
            )),
            ProofRule::IntegerAffineBound { .. } => {
                Err(BoundedDenotationError::Unsupported("integer affine bound"))
            }
            ProofRule::IntegerExactAddDefinitionBound { .. } => Err(
                BoundedDenotationError::Unsupported("integer exact-add definition bound"),
            ),
            ProofRule::IntegerCastBound { .. } => {
                Err(BoundedDenotationError::Unsupported("integer cast bound"))
            }
            ProofRule::IntegerCorrelatedForbiddenRoots { .. } => Err(
                BoundedDenotationError::Unsupported("integer correlated forbidden roots"),
            ),
        }
    }

    /// A premise citation: `Assumption{index}` names `roster[index]`,
    /// which may be an ambient assumption or a discharged branch
    /// hypothesis. The bounded matcher's lift/lower normalization is
    /// honored only when it leaves the denotation unchanged — an
    /// `Equal`/`IntegerMathEqual` crossing changes denotation shape and
    /// stays outside this fragment.
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
        IntegerSign, IntegerType, IntegerValue, PropositionId, ScalarType, ValueId,
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

        // One `Type 0` assumption for the atom; the judgment is
        // `P : Type 0; [P] ⊢ p : P` — the term is the bound variable.
        assert_eq!(denoted.certificate.signature.len(), 1);
        assert!(denoted.certificate.signature[0].is_assumption());
        assert_eq!(denoted.certificate.context.len(), 1);
        assert_eq!(
            denoted.arena.get(denoted.certificate.term),
            Term::Variable(0)
        );
        assert_eq!(
            certificate_assumption_closure(&denoted.arena, &denoted.certificate),
            BTreeSet::from([0]),
            "the judgment commits to exactly the cited atom",
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
        // Carrier `S` plus one assumption constant per scalar endpoint.
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
                .expect("symmetry verifies as J");
        assert_eq!(denoted.certificate.signature.len(), 3);
        assert_eq!(
            certificate_assumption_closure(&denoted.arena, &denoted.certificate),
            BTreeSet::from([0, 1, 2]),
        );
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
        assert_eq!(denoted.certificate.signature.len(), 2);
        assert_eq!(
            certificate_assumption_closure(&denoted.arena, &denoted.certificate),
            BTreeSet::from([0, 1]),
            "atom assumption and the named decision assumption",
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

    /// The rule families outside the denoted fragment refuse as
    /// `Unsupported` before their children are denoted — never a wrong
    /// judgment, never a silent reinterpretation.
    #[test]
    fn uncovered_rule_families_refuse_unsupported() {
        let junk = ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
        };
        let proof = ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: Box::new(junk.clone()),
                middle_less_or_equal_right: Box::new(junk),
            },
        };
        assert!(matches!(
            denote_bounded_certificate(
                &PropositionContext::default(),
                &Proposition::Truth,
                &[],
                &[],
                &proof,
            ),
            Err(BoundedDenotationError::Unsupported(
                "integer <= transitivity"
            )),
        ));

        let junk = ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
        };
        let proof = ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::PredicateDenotation {
                premise: Box::new(junk),
            },
        };
        assert!(matches!(
            denote_bounded_certificate(
                &PropositionContext::default(),
                &Proposition::Truth,
                &[],
                &[],
                &proof,
            ),
            Err(BoundedDenotationError::Unsupported(_)),
        ));

        // The bounded citation matcher accepts `Equal` cited as its lifted
        // `IntegerMathEqual`; the denotation refuses the crossing because
        // `Id` and the atom are different types.
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
        assert!(matches!(
            denote_bounded_certificate(&context, &lifted, &[fixed], &[], &proof),
            Err(BoundedDenotationError::Unsupported(_)),
        ));
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
}
