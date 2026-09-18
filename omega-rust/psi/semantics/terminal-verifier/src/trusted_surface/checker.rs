//! Primitive-judgment, certificate-rule, and evidence-route entries.
//!
//! The three exhaustive maps below have no wildcard arm: adding an accepted
//! `PrimitiveJudgment`, `ProofRule`, or `EvidenceRoute` variant fails
//! compilation here before publication.

use proof_admission::{EvidenceRoute, PrimitiveJudgment, ProofRule};

use super::{CoveredSurface, EntryBinding, LedgerFamily, SoundnessStatus, TrustedSurfaceEntry};

const KERNEL: &str = "omega-rust/psi/semantics/proof-admission/src/kernel.rs";
const EVIDENCE: &str = "omega-rust/psi/semantics/proof-admission/src/admission/evidence.rs";
const PROOF: &str = "omega-rust/psi/semantics/proof-admission/src/proof.rs";
const PROPOSITIONAL: &str =
    "omega-rust/psi/semantics/proof-admission/src/proof/propositional_rules.rs";
const EQUALITY_RULES: &str = "omega-rust/psi/semantics/proof-admission/src/proof/equality_rules.rs";
const INTEGER_ORDER_RULES: &str =
    "omega-rust/psi/semantics/proof-admission/src/proof/integer_order_rules.rs";
const INTEGER_BOUND_RULES: &str =
    "omega-rust/psi/semantics/proof-admission/src/proof/integer_bound_rules.rs";
const INTEGER_MATH_NORMALIZATION: &str =
    "omega-rust/psi/semantics/proof-admission/src/proof/integer_math_normalization.rs";
const TRAVERSAL: &str = "omega-rust/psi/semantics/proof-admission/src/proof/traversal.rs";
const SUBTRACT_ORDER: &str = "omega-rust/psi/semantics/proof-admission/src/proof/subtract_order.rs";
const ORDER_DISCRETENESS: &str =
    "omega-rust/psi/semantics/proof-admission/src/proof/order_discreteness.rs";
const STRICT_ORDER: &str =
    "omega-rust/psi/semantics/proof-admission/src/proof/strict_order_transitivity.rs";
const INTEGER_AFFINE: &str =
    "omega-rust/psi/semantics/proof-admission/src/integer_rules/integer_affine.rs";
const AFFINE_BOUND_MAPPING: &str =
    "omega-rust/psi/semantics/proof-admission/src/integer_rules/integer_affine/bound_mapping.rs";
const AFFINE_TRUTH_BOUNDS: &str =
    "omega-rust/psi/semantics/proof-admission/src/integer_rules/integer_affine/truth_bounds.rs";
const AFFINE_WITNESS_CHECKING: &str =
    "omega-rust/psi/semantics/proof-admission/src/integer_rules/integer_affine/witness_checking.rs";
const INTEGER_CAST: &str =
    "omega-rust/psi/semantics/proof-admission/src/integer_rules/integer_cast.rs";
const INTEGER_FORBIDDEN: &str =
    "omega-rust/psi/semantics/proof-admission/src/integer_rules/integer_forbidden_root.rs";
const PREDICATE_DENOTATION: &str =
    "omega-rust/psi/semantics/proof-admission/src/predicate_denotation.rs";
const VALUE_EQUALITIES: &str =
    "omega-rust/psi/semantics/proof-admission/src/predicate_denotation/value_equalities.rs";
const NODES: &str =
    "omega-rust/psi/representations/terminal-psi/src/artifacts/proof_bundle/nodes.rs";
const ADMISSION: &str =
    "omega-rust/psi/representations/terminal-psi/src/artifacts/proof_bundle/admission.rs";
const WITNESSES: &str =
    "omega-rust/psi/representations/terminal-psi/src/artifacts/proof_bundle/witnesses.rs";

const TRUSTED: SoundnessStatus = SoundnessStatus::ExplicitlyTrusted {
    root: "root:rust-reference-verifier",
    rationale: "implemented rule witnessed by the crate's unit corpus and the verifier/canary rejection suites; no lower-rung derivation discharges it",
};

const fn dispatch(surface: CoveredSurface) -> EntryBinding {
    EntryBinding::DispatchOn(surface)
}

pub static ENTRIES: &[TrustedSurfaceEntry] = &[
    // -- Primitive judgments (PrimitiveJudgment variants) --
    TrustedSurfaceEntry {
        id: "primitive:truth",
        family: LedgerFamily::PrimitiveJudgment,
        binding: dispatch(CoveredSurface::PrimitiveJudgments),
        premises: "the goal is the well-formed Proposition::Truth under the checked context",
        conclusion: "Truth is established",
        dependencies: &["formation:proposition-context"],
        implementation: &[KERNEL, NODES],
        soundness: TRUSTED,
    },
    TrustedSurfaceEntry {
        id: "primitive:reflexive-equality",
        family: LedgerFamily::PrimitiveJudgment,
        binding: dispatch(CoveredSurface::PrimitiveJudgments),
        premises: "a well-formed Equal, IntegerMathEqual, or ContentConservation whose two sides are identical",
        conclusion: "the equality is established",
        dependencies: &["formation:proposition-context"],
        implementation: &[KERNEL, NODES],
        soundness: TRUSTED,
    },
    TrustedSurfaceEntry {
        id: "primitive:closed-integer-relation",
        family: LedgerFamily::PrimitiveJudgment,
        binding: dispatch(CoveredSurface::PrimitiveJudgments),
        premises: "an Equal, LessThan, or LessOrEqual over same-type fixed-integer literals, or the matching mathematical-integer relation whose terms fit the bounded evaluator",
        conclusion: "the closed relation holds by exact evaluation; malformed terms and resource exhaustion reject instead of judging",
        dependencies: &[
            "formation:proposition-context",
            "normalization:closed-integer-evaluator",
        ],
        implementation: &[KERNEL, NODES],
        soundness: TRUSTED,
    },
    TrustedSurfaceEntry {
        id: "primitive:integer-carrier-bound",
        family: LedgerFamily::PrimitiveJudgment,
        binding: dispatch(CoveredSurface::PrimitiveJudgments),
        premises: "a LessOrEqual between a value of one fixed integer type and a same-type literal equal to that type's exact minimum or maximum",
        conclusion: "the carrier bound holds for the value",
        dependencies: &["formation:proposition-context"],
        implementation: &[KERNEL, NODES],
        soundness: TRUSTED,
    },
    // -- Certificate checker rules (ProofRule variants) --
    TrustedSurfaceEntry {
        id: "rule:traversal",
        family: LedgerFamily::CheckerRule,
        binding: EntryBinding::Procedural,
        premises: "a certificate node whose children have already been checked under the scoped postorder traversal, with binder-discharged assumptions extending the roster locally",
        conclusion: "the node's local rule arm is re-checked and the acceptance records only ambient premises, axioms, and rules",
        dependencies: &["root:rust-reference-verifier"],
        implementation: &[TRAVERSAL, PROOF],
        soundness: TRUSTED,
    },
    TrustedSurfaceEntry {
        id: "rule:primitive",
        family: LedgerFamily::CheckerRule,
        binding: dispatch(CoveredSurface::ProofRules),
        premises: "a checked node whose conclusion is re-decided by the named PrimitiveJudgment",
        conclusion: "the conclusion is established by the kernel judgment",
        dependencies: &[
            "rule:traversal",
            "primitive:truth",
            "primitive:reflexive-equality",
            "primitive:closed-integer-relation",
            "primitive:integer-carrier-bound",
        ],
        implementation: &[PROOF, KERNEL, NODES],
        soundness: TRUSTED,
    },
    TrustedSurfaceEntry {
        id: "rule:semantic-axiom",
        family: LedgerFamily::CheckerRule,
        binding: dispatch(CoveredSurface::ProofRules),
        premises: "an index resolving inside the verifier-reconstructed semantic-axiom roster",
        conclusion: "the cited axiom is established; the node's conclusion must match it under integer-math normalization and the citation is recorded",
        dependencies: &[
            "rule:traversal",
            "fact:semantic-axiom-roster",
            "conversion:proposition-match",
        ],
        implementation: &[PROOF, PROPOSITIONAL, INTEGER_MATH_NORMALIZATION, NODES],
        soundness: TRUSTED,
    },
    TrustedSurfaceEntry {
        id: "rule:assumption",
        family: LedgerFamily::CheckerRule,
        binding: dispatch(CoveredSurface::ProofRules),
        premises: "an index resolving inside the ambient assumption roster supplied by reconstruction",
        conclusion: "the cited assumption is established; the node's conclusion must match it under integer-math normalization and the citation is recorded",
        dependencies: &["rule:traversal", "conversion:proposition-match"],
        implementation: &[PROOF, PROPOSITIONAL, INTEGER_MATH_NORMALIZATION, NODES],
        soundness: TRUSTED,
    },
    TrustedSurfaceEntry {
        id: "rule:conjunction-introduction",
        family: LedgerFamily::CheckerRule,
        binding: dispatch(CoveredSurface::ProofRules),
        premises: "children proving each conjunct in order",
        conclusion: "the conclusion is a conjunction of exactly the child conclusions, arity exact",
        dependencies: &["rule:traversal"],
        implementation: &[PROOF, PROPOSITIONAL, NODES],
        soundness: TRUSTED,
    },
    TrustedSurfaceEntry {
        id: "rule:conjunction-elimination",
        family: LedgerFamily::CheckerRule,
        binding: dispatch(CoveredSurface::ProofRules),
        premises: "a child concluding a conjunction that contains this conclusion at the cited index",
        conclusion: "the selected conjunct is established",
        dependencies: &["rule:traversal"],
        implementation: &[PROOF, PROPOSITIONAL, NODES],
        soundness: TRUSTED,
    },
    TrustedSurfaceEntry {
        id: "rule:disjunction-introduction",
        family: LedgerFamily::CheckerRule,
        binding: dispatch(CoveredSurface::ProofRules),
        premises: "a child proving the cited disjunct",
        conclusion: "the conclusion is a disjunction containing the child's conclusion at the cited index",
        dependencies: &["rule:traversal"],
        implementation: &[PROOF, PROPOSITIONAL, NODES],
        soundness: TRUSTED,
    },
    TrustedSurfaceEntry {
        id: "rule:disjunction-elimination",
        family: LedgerFamily::CheckerRule,
        binding: dispatch(CoveredSurface::ProofRules),
        premises: "a child concluding a disjunction and one branch per disjunct, each proving the common conclusion under only its own discharged assumption",
        conclusion: "the common conclusion is established; a branch may not reuse another case or an unproved disjunction",
        dependencies: &["rule:traversal"],
        implementation: &[PROOF, PROPOSITIONAL, NODES],
        soundness: TRUSTED,
    },
    TrustedSurfaceEntry {
        id: "rule:implication-introduction",
        family: LedgerFamily::CheckerRule,
        binding: dispatch(CoveredSurface::ProofRules),
        premises: "a body proving the implication's conclusion under its discharged premise",
        conclusion: "the implication is established with the exact recorded premise and conclusion",
        dependencies: &["rule:traversal"],
        implementation: &[PROOF, PROPOSITIONAL, NODES],
        soundness: TRUSTED,
    },
    TrustedSurfaceEntry {
        id: "rule:implication-elimination",
        family: LedgerFamily::CheckerRule,
        binding: dispatch(CoveredSurface::ProofRules),
        premises: "one child concluding premise->conclusion and another proving exactly that premise",
        conclusion: "the implication's conclusion is established",
        dependencies: &["rule:traversal"],
        implementation: &[PROOF, PROPOSITIONAL, NODES],
        soundness: TRUSTED,
    },
    TrustedSurfaceEntry {
        id: "rule:equality-transitivity",
        family: LedgerFamily::CheckerRule,
        binding: dispatch(CoveredSurface::ProofRules),
        premises: "two proved relations sharing one exact middle term: Equal, IntegerMathEqual (canonically ordered), or ContentConservation over one algebra",
        conclusion: "the composed equality of the outer terms; a shared-middle mismatch or algebra mismatch rejects",
        dependencies: &["rule:traversal", "conversion:proposition-match"],
        implementation: &[PROOF, EQUALITY_RULES, INTEGER_MATH_NORMALIZATION, NODES],
        soundness: TRUSTED,
    },
    TrustedSurfaceEntry {
        id: "rule:equality-symmetry",
        family: LedgerFamily::CheckerRule,
        binding: dispatch(CoveredSurface::ProofRules),
        premises: "a proved Equal",
        conclusion: "the same equality with sides exchanged",
        dependencies: &["rule:traversal"],
        implementation: &[PROOF, EQUALITY_RULES, NODES],
        soundness: TRUSTED,
    },
    TrustedSurfaceEntry {
        id: "rule:predicate-denotation",
        family: LedgerFamily::CheckerRule,
        binding: dispatch(CoveredSurface::ProofRules),
        premises: "a premise proved under the unchanged original roster whose bounded Boolean and closed fixed-integer literal denotation normal form exists",
        conclusion: "one proposition convertible to the premise's denotation; the outer conclusion still must equal the exact reconstructed obligation",
        dependencies: &["rule:traversal", "conversion:predicate-denotation"],
        implementation: &[PROOF, EQUALITY_RULES, PREDICATE_DENOTATION, NODES],
        soundness: TRUSTED,
    },
    TrustedSurfaceEntry {
        id: "rule:value-equality-transport",
        family: LedgerFamily::CheckerRule,
        binding: dispatch(CoveredSurface::ProofRules),
        premises: "a premise plus a nonempty ordered list of proved scalar equations, each oriented from an exact value identity to its term",
        conclusion: "the premise transported through only the carried equations when both sides' denotation normal forms match; constructor tags, types, and operand order are preserved",
        dependencies: &["rule:traversal", "conversion:value-equality-denotation"],
        implementation: &[
            PROOF,
            EQUALITY_RULES,
            PREDICATE_DENOTATION,
            VALUE_EQUALITIES,
            NODES,
        ],
        soundness: TRUSTED,
    },
    TrustedSurfaceEntry {
        id: "rule:integer-order-weakening",
        family: LedgerFamily::CheckerRule,
        binding: dispatch(CoveredSurface::ProofRules),
        premises: "a proved Equal or LessThan over fixed-integer terms of the same scalar type",
        conclusion: "the same endpoints under LessOrEqual, matched under integer-math normalization",
        dependencies: &["rule:traversal", "conversion:proposition-match"],
        implementation: &[
            PROOF,
            INTEGER_ORDER_RULES,
            INTEGER_MATH_NORMALIZATION,
            NODES,
        ],
        soundness: TRUSTED,
    },
    TrustedSurfaceEntry {
        id: "rule:integer-order-discreteness",
        family: LedgerFamily::CheckerRule,
        binding: dispatch(CoveredSurface::ProofRules),
        premises: "a proved nonstrict integer bound with a literal endpoint",
        conclusion: "the strict relation obtained by moving one literal endpoint outward by exactly one representable integer",
        dependencies: &["rule:traversal"],
        implementation: &[PROOF, INTEGER_ORDER_RULES, ORDER_DISCRETENESS, NODES],
        soundness: TRUSTED,
    },
    TrustedSurfaceEntry {
        id: "rule:integer-subtract-order",
        family: LedgerFamily::CheckerRule,
        binding: dispatch(CoveredSurface::ProofRules),
        premises: "proved result = original - decrement over exact fixed-integer subtraction and a proved 0 < decrement",
        conclusion: "result < original",
        dependencies: &["rule:traversal"],
        implementation: &[PROOF, INTEGER_ORDER_RULES, SUBTRACT_ORDER, NODES],
        soundness: TRUSTED,
    },
    TrustedSurfaceEntry {
        id: "rule:integer-less-or-equal-transitivity",
        family: LedgerFamily::CheckerRule,
        binding: dispatch(CoveredSurface::ProofRules),
        premises: "two proved LessOrEqual or IntegerMathLessOrEqual relations sharing one exact middle term",
        conclusion: "the composed nonstrict order of the outer terms",
        dependencies: &["rule:traversal", "conversion:proposition-match"],
        implementation: &[
            PROOF,
            INTEGER_ORDER_RULES,
            INTEGER_MATH_NORMALIZATION,
            NODES,
        ],
        soundness: TRUSTED,
    },
    TrustedSurfaceEntry {
        id: "rule:integer-strict-order-transitivity",
        family: LedgerFamily::CheckerRule,
        binding: dispatch(CoveredSurface::ProofRules),
        premises: "two proved integer orders sharing one exact middle term with at least one strict edge",
        conclusion: "the strict order of the outer terms",
        dependencies: &["rule:traversal"],
        implementation: &[PROOF, INTEGER_ORDER_RULES, STRICT_ORDER, NODES],
        soundness: TRUSTED,
    },
    TrustedSurfaceEntry {
        id: "rule:integer-order-substitution",
        family: LedgerFamily::CheckerRule,
        binding: dispatch(CoveredSurface::ProofRules),
        premises: "a proved strict or nonstrict integer order and a proved equality for exactly the cited endpoint",
        conclusion: "the same order with only that endpoint replaced; strictness and the unchanged endpoint are preserved",
        dependencies: &["rule:traversal"],
        implementation: &[
            PROOF,
            INTEGER_ORDER_RULES,
            INTEGER_MATH_NORMALIZATION,
            NODES,
        ],
        soundness: TRUSTED,
    },
    TrustedSurfaceEntry {
        id: "rule:integer-affine-bound",
        family: LedgerFamily::CheckerRule,
        binding: dispatch(CoveredSurface::ProofRules),
        premises: "a checked ordered affine witness from one proved root bound to its target through exact-add definitions and optional literal landings",
        conclusion: "the mapped affine bound equals the conclusion; a Truth root admits only the witness's own truth bounds; cited definition and literal axioms are recorded as dependencies",
        dependencies: &[
            "rule:traversal",
            "normalization:integer-affine-witness",
            "fact:semantic-axiom-roster",
        ],
        implementation: &[
            PROOF,
            INTEGER_BOUND_RULES,
            INTEGER_AFFINE,
            AFFINE_WITNESS_CHECKING,
            AFFINE_BOUND_MAPPING,
            AFFINE_TRUTH_BOUNDS,
            INTEGER_MATH_NORMALIZATION,
            WITNESSES,
            NODES,
        ],
        soundness: TRUSTED,
    },
    TrustedSurfaceEntry {
        id: "rule:integer-exact-add-definition-bound",
        family: LedgerFamily::CheckerRule,
        binding: dispatch(CoveredSurface::ProofRules),
        premises: "two proved scalar bounds and one cited prior exact-add definition axiom over a fixed-integer carrier",
        conclusion: "the two-endpoint bound obtained by mapping the conjunction through the definition's affine form; the cited axiom is recorded",
        dependencies: &[
            "rule:traversal",
            "normalization:integer-affine-witness",
            "fact:semantic-axiom-roster",
        ],
        implementation: &[
            PROOF,
            INTEGER_BOUND_RULES,
            INTEGER_AFFINE,
            AFFINE_WITNESS_CHECKING,
            AFFINE_BOUND_MAPPING,
            NODES,
        ],
        soundness: TRUSTED,
    },
    TrustedSurfaceEntry {
        id: "rule:integer-cast-bound",
        family: LedgerFamily::CheckerRule,
        binding: dispatch(CoveredSurface::ProofRules),
        premises: "a checked ordered word of partial fixed-integer exact casts and strict widening identities over one proved root bound",
        conclusion: "the mapped bound equals the conclusion; a Truth root admits only the chain's own truth bounds; cited definition axioms are recorded",
        dependencies: &[
            "rule:traversal",
            "normalization:integer-cast-chain-witness",
            "fact:semantic-axiom-roster",
        ],
        implementation: &[
            PROOF,
            INTEGER_BOUND_RULES,
            INTEGER_CAST,
            INTEGER_MATH_NORMALIZATION,
            WITNESSES,
            NODES,
        ],
        soundness: TRUSTED,
    },
    TrustedSurfaceEntry {
        id: "rule:integer-correlated-forbidden-roots",
        family: LedgerFamily::CheckerRule,
        binding: dispatch(CoveredSurface::ProofRules),
        premises: "checked correlated dividend/divisor affine branches over one machine signature parameter, the exact definition/assumption ledger boundary, and two bound axioms",
        conclusion: "the canonical signed exact-division definedness proposition reconstructed by the check; cited axioms and assumptions are recorded",
        dependencies: &[
            "rule:traversal",
            "normalization:correlated-forbidden-root-witness",
            "normalization:integer-affine-witness",
            "fact:semantic-axiom-roster",
        ],
        implementation: &[
            PROOF,
            INTEGER_BOUND_RULES,
            INTEGER_FORBIDDEN,
            WITNESSES,
            NODES,
        ],
        soundness: TRUSTED,
    },
    // -- Accepted evidence routes (EvidenceRoute variants) --
    TrustedSurfaceEntry {
        id: "route:kernel-derived",
        family: LedgerFamily::EvidenceRoute,
        binding: dispatch(CoveredSurface::EvidenceRoutes),
        premises: "the obligation's exact proposition re-decided by one named PrimitiveJudgment",
        conclusion: "the fact is kernel-derived for that obligation",
        dependencies: &["formation:proposition-context", "rule:primitive"],
        implementation: &[EVIDENCE, ADMISSION],
        soundness: TRUSTED,
    },
    TrustedSurfaceEntry {
        id: "route:certificate-derived",
        family: LedgerFamily::EvidenceRoute,
        binding: dispatch(CoveredSurface::EvidenceRoutes),
        premises: "a certificate envelope whose proof is checked against the exact obligation, assumptions, and reconstructed axioms under the current proof-system marker and, when the bounded denotation covers its rule families, re-decided by the mathematical-core kernel",
        conclusion: "the fact is certificate-derived and its rule/premise acceptance is recorded with the kernel's decision: a covered certificate the kernel rejects or decides differently rejects here, and a refused denotation leaves the bounded rules standing alone",
        dependencies: &["rule:traversal", "formation:mathematical-core"],
        implementation: &[EVIDENCE, ADMISSION],
        soundness: TRUSTED,
    },
    TrustedSurfaceEntry {
        id: "route:admitted",
        family: LedgerFamily::EvidenceRoute,
        binding: dispatch(CoveredSurface::EvidenceRoutes),
        premises: "an admission whose site, kind, and authority identity equal the obligation's authorized admission and whose evidence identity is accepted by the consuming profile",
        conclusion: "the fact is admitted; an admission never replaces a derivable obligation",
        dependencies: &["formation:proposition-context", "rule:primitive"],
        implementation: &[EVIDENCE, ADMISSION],
        soundness: TRUSTED,
    },
];

/// `PrimitiveJudgment` -> ledger entry, total by construction.
pub fn primitive_judgment_entry(judgment: PrimitiveJudgment) -> &'static TrustedSurfaceEntry {
    match judgment {
        PrimitiveJudgment::Truth => &ENTRIES[0],
        PrimitiveJudgment::ReflexiveEquality => &ENTRIES[1],
        PrimitiveJudgment::ClosedIntegerRelation => &ENTRIES[2],
        PrimitiveJudgment::IntegerCarrierBound => &ENTRIES[3],
    }
}

/// `ProofRule` -> ledger entry, total by construction.
pub fn checker_rule_entry(rule: &ProofRule) -> &'static TrustedSurfaceEntry {
    match rule {
        ProofRule::Primitive(_) => &ENTRIES[5],
        ProofRule::SemanticAxiom { .. } => &ENTRIES[6],
        ProofRule::Assumption { .. } => &ENTRIES[7],
        ProofRule::ConjunctionIntroduction(_) => &ENTRIES[8],
        ProofRule::ConjunctionElimination { .. } => &ENTRIES[9],
        ProofRule::DisjunctionIntroduction { .. } => &ENTRIES[10],
        ProofRule::DisjunctionElimination { .. } => &ENTRIES[11],
        ProofRule::ImplicationIntroduction { .. } => &ENTRIES[12],
        ProofRule::ImplicationElimination { .. } => &ENTRIES[13],
        ProofRule::EqualityTransitivity { .. } => &ENTRIES[14],
        ProofRule::EqualitySymmetry { .. } => &ENTRIES[15],
        ProofRule::PredicateDenotation { .. } => &ENTRIES[16],
        ProofRule::ValueEqualityTransport { .. } => &ENTRIES[17],
        ProofRule::IntegerOrderWeakening { .. } => &ENTRIES[18],
        ProofRule::IntegerOrderDiscreteness { .. } => &ENTRIES[19],
        ProofRule::IntegerSubtractOrder { .. } => &ENTRIES[20],
        ProofRule::IntegerLessOrEqualTransitivity { .. } => &ENTRIES[21],
        ProofRule::IntegerStrictOrderTransitivity { .. } => &ENTRIES[22],
        ProofRule::IntegerOrderSubstitution { .. } => &ENTRIES[23],
        ProofRule::IntegerAffineBound { .. } => &ENTRIES[24],
        ProofRule::IntegerExactAddDefinitionBound { .. } => &ENTRIES[25],
        ProofRule::IntegerCastBound { .. } => &ENTRIES[26],
        ProofRule::IntegerCorrelatedForbiddenRoots { .. } => &ENTRIES[27],
    }
}

/// `EvidenceRoute` -> ledger entry, total by construction.
pub fn evidence_route_entry(route: &EvidenceRoute) -> &'static TrustedSurfaceEntry {
    match route {
        EvidenceRoute::KernelDerived(_) => &ENTRIES[28],
        EvidenceRoute::CertificateDerived(_) => &ENTRIES[29],
        EvidenceRoute::Admitted(_) => &ENTRIES[30],
    }
}
