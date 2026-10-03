//! Procedural entries: normalization/conversion, premise scope, write
//! invalidation, call and cycle composition, and shared formation rules.
//!
//! These rows are not dispatch variants; they bind through implementation
//! sites and file coverage. Every entry names the exact rule shape it trusts.

use super::{EntryBinding, LedgerFamily, SoundnessStatus, TrustedSurfaceEntry};

const TRUSTED: SoundnessStatus = SoundnessStatus::ExplicitlyTrusted {
    root: "root:rust-reference-verifier",
    rationale: "shared rule witnessed by the verifier's validation and rejection corpus; no lower-rung derivation discharges it",
};

const PROCEDURAL: EntryBinding = EntryBinding::Procedural;

macro_rules! rows {
    ($( $name:ident => ($id:literal, $family:ident, $premises:literal, $conclusion:literal, $deps:expr, $impls:expr) ;)+) => {
        $( static $name: TrustedSurfaceEntry = TrustedSurfaceEntry {
            id: $id,
            family: LedgerFamily::$family,
            binding: PROCEDURAL,
            premises: $premises,
            conclusion: $conclusion,
            dependencies: $deps,
            implementation: $impls,
            soundness: TRUSTED,
        }; )+
        pub static ENTRIES: &[TrustedSurfaceEntry] = &[ $( $name ),+ ];
    };
}

macro_rules! pa {
    ($file:literal) => {
        concat!("omega-rust/psi/compilation/proof/admission/src/", $file)
    };
}
macro_rules! ts {
    ($file:literal) => {
        concat!(
            "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/",
            $file
        )
    };
}
macro_rules! tv {
    ($file:literal) => {
        concat!(
            "omega-rust/psi/compilation/representations/terminal-psi/verification/src/",
            $file
        )
    };
}
macro_rules! tp {
    ($file:literal) => {
        concat!(
            "omega-rust/psi/compilation/representations/terminal-psi/src/",
            $file
        )
    };
}
macro_rules! sv {
    ($file:literal) => {
        concat!("omega-rust/psi/foundation/semantic-vocabulary/src/", $file)
    };
}

rows! {
    // -- Normalization and conversion --
    NORM_CONFORMANCE => (
        "normalization:conformance-identity",
        NormalizationConversion,
        "a normalization block naming its conformance identity",
        "normalization applies only under the exact declared conformance identity; a mismatched block rejects",
        &["formation:proposition-context"],
        &[pa!( "admission/normalization.rs")]
    );
    NORM_ORDERED_LAWS => (
        "normalization:ordered-law-application",
        NormalizationConversion,
        "an ordered list of named normalization laws applied in the canonical order",
        "the normalized form is the result of the laws applied in order; a different order or unknown law rejects",
        &["normalization:conformance-identity"],
        &[pa!( "admission/normalization.rs")]
    );
    NORM_LAW_EVIDENCE => (
        "normalization:law-evidence-binding",
        NormalizationConversion,
        "each normalization law's proof obligation and the evidence supplied for it",
        "a law participates only when its obligation is discharged by the bound evidence; omitted or unknown law evidence rejects",
        &["normalization:ordered-law-application", "route:certificate-derived"],
        &[pa!( "admission/normalization.rs")]
    );
    NORM_CONCLUSION_CITES => (
        "normalization:conclusion-cites-laws",
        NormalizationConversion,
        "the final normalized conclusion and the law-derived assumptions it may cite",
        "the conclusion must cite exactly the retained law-derived premise set; extra or missing citations reject",
        &["normalization:law-evidence-binding"],
        &[pa!( "admission/normalization.rs")]
    );
    NORM_PROPOSITION_FORM => (
        "conversion:proposition-match",
        NormalizationConversion,
        "two propositions compared under integer-math normalization",
        "citation and conclusion equality is decided on the normalized form; structurally different spellings of one proposition match",
        &["formation:mathematical-core"],
        &[pa!( "proof/integer_math_normalization.rs"), pa!( "mathematical_core/conversion.rs")]
    );
    NORM_PREDICATE_DENOTATION => (
        "conversion:predicate-denotation",
        NormalizationConversion,
        "a proposition validated under its original context and bounded recursion, substitution, and clone budget; closed IEEE comparisons require two exact-bit literals of the same declared format",
        "the Boolean and exact fixed-integer denotation normal form, retaining exact subjects and carriers in mixed mathematical comparisons; closed same-format IEEE literals are decoded to exact FloatSemantics meanings before comparison, with equal signed zeros, unordered nonreflexive NaNs, and explicit logical complements; open IEEE relations remain atomic with no unknown-float reflexivity; byte-content observations retain their exact predicate, descriptor root and path as unknown Boolean atoms, and unsupported compound endpoints remain unchanged, and unbounded or malformed propositions reject before evaluation without carrier coercion",
        &["formation:proposition-context", "formation:mathematical-core", "primitive:closed-integer-relation"],
        &[pa!( "predicate_denotation.rs"), pa!( "predicate_denotation/budget.rs"), pa!( "kernel.rs"), "omega-rust/psi/foundation/numerics/src/float_semantics.rs", "omega-rust/psi/foundation/numerics/src/bignum/exact_float.rs"]
    );
    NORM_VALUE_EQUALITY_DENOTATION => (
        "conversion:value-equality-denotation",
        NormalizationConversion,
        "a proposition plus an ordered roster of unconditional typed equations supplied as current semantic axioms or independently proved children; each definition subject is an exact Value, IntegerField, BooleanField, or IeeeFloatField term, including its full root/path and carrier or format",
        "the denotation normal form under exactly those equations in their supplied orientation, using the first nonreflexive definition of each exact subject; active definition indexes reject cycles and the shared budget bounds traversal and expansion; no expired field observation is recovered from history and the original premise slots remain unchanged; mathematical constructors retain operand order through the exact fixed-integer lift, while IEEE substitution preserves payload bits, format, and ordered comparisons, canonicalizing only symmetric comparisons; payload identity never asserts IEEE reflexivity, opaque constructors gain no arithmetic identities; byte-content observations may be substituted only through the supplied exact current equations, without equating aliased descriptors; only representable closed integer literals may change carrier without changing value",
        &["conversion:predicate-denotation"],
        &[pa!( "predicate_denotation.rs"), pa!( "predicate_denotation/value_equalities.rs"), pa!( "proof/integer_math_normalization.rs")]
    );
    NORM_CLOSED_INTEGER_EVALUATOR => (
        "normalization:closed-integer-evaluator",
        NormalizationConversion,
        "a closed mathematical-integer term within the bounded evaluator's resource limits",
        "the exact integer value the term denotes; resource exhaustion rejects rather than approximates",
        &["formation:mathematical-core"],
        &[pa!( "integer_rules/closed_integer.rs"), pa!( "mathematical_core/term.rs"), pa!( "mathematical_core/typing.rs")]
    );
    NORM_AFFINE_WITNESS => (
        "normalization:integer-affine-witness",
        NormalizationConversion,
        "an ordered endpoint witness of supported arithmetic definitions and literal landings over one carrier, including nonzero wrapping remainder on either sign, nonzero wrapping division except signed divisor -1, and wrapping addition or subtraction of a literal, where subtracting `c` is adding `-c` modulo the width; a target that is itself one wrapping addition or subtraction of a direct literal over the chain's last value is its own definition and cites no row; or a supported direct arithmetic expression; a direct wrapping-product bound requires all four operand-interval corners to fit the carrier",
        "the mapped bound from root to target when every step checks; an unsigned fixed-carrier remainder rooted at its exact runtime divisor maps a separately proved `0 < divisor` or `1 <= divisor` to `result < divisor`, retaining the direct expression or one canonical cited definition and rejecting a Truth premise; a wrapping addend `c` maps the side it cannot cross unconditionally and the other side only when the root bound carries the exact no-overflow conjunct, `operand <= maximum - c` for positive `c` and `minimum - c <= operand` for negative `c`, so an unsigned `operand - c` needs `c <= operand`; a missing, different or extra conjunct rejects; the witness is bounded and validated before interpretation",
        &["normalization:closed-integer-evaluator"],
        &[pa!( "integer_rules/integer_affine.rs"), pa!( "integer_rules/integer_affine/witness_checking.rs"), pa!( "integer_rules/integer_affine/remainder.rs"), pa!( "integer_rules/integer_affine/bound_mapping.rs"), pa!( "integer_rules/integer_affine/bound_mapping/relative_translation.rs"), pa!( "integer_rules/integer_affine/truth_bounds.rs"), tp!( "artifacts/proof_bundle/witnesses.rs")]
    );
    NORM_CAST_CHAIN_WITNESS => (
        "normalization:integer-cast-chain-witness",
        NormalizationConversion,
        "an ordered word of partial fixed-integer exact casts and strict widening identities",
        "the mapped bound through the chain when every step checks",
        &["normalization:integer-affine-witness"],
        &[pa!( "integer_rules/integer_cast.rs"), pa!( "integer_rules/integer_shift.rs"), tp!( "artifacts/proof_bundle/witnesses.rs")]
    );
    NORM_CORRELATED_WITNESS => (
        "normalization:correlated-forbidden-root-witness",
        NormalizationConversion,
        "correlated dividend/divisor affine branches over one machine signature parameter within the exact definition/assumption ledger boundary",
        "the canonical signed exact-division definedness proposition; a branch outside the boundary rejects",
        &["normalization:integer-affine-witness"],
        &[pa!( "integer_rules/integer_forbidden_root.rs"), tp!( "artifacts/proof_bundle/witnesses.rs")]
    );

    // -- Premise scope --
    SCOPE_DOMINANCE => (
        "scope:dominance-order",
        PremiseScope,
        "the machine's blocks traversed in dominance order from the entry parameters",
        "facts reconstructed at a point are exactly those of operations and terminators dominating it; nothing downstream is assumed",
        &["formation:control-flow-validation"],
        &[tv!( "verification/reconstruction/machine_flow.rs"), tv!( "control_graph.rs")]
    );
    SCOPE_HEADER_EDGE => (
        "scope:header-edge-arrival",
        PremiseScope,
        "a header block's incoming edges, each with its own reconstructed arrival set",
        "block-invariant obligations are stated per arriving edge, never merged across edges; a declared-predicate member whose fixed-shape elimination certificate the checker rejects stays a licensed premise introduction under this row — every accepted member is discharged under fact:header-invariant-members",
        &["scope:dominance-order"],
        &[tv!( "verification/reconstruction/scalar_block_invariants.rs"), tv!( "verification/reconstruction/machine_flow.rs")]
    );
    SCOPE_SUCCESSOR_SUBSTITUTION => (
        "scope:successor-substitution",
        PremiseScope,
        "the edge's argument values and the target block's declared parameters",
        "established facts are rewritten through the exact parameter substitution before joining the successor; unbound parameters keep no stale identity",
        &["scope:place-substitution"],
        &[tv!( "verification/reconstruction/path_facts.rs"), tv!( "verification/substitution.rs")]
    );
    SCOPE_ITERATION_CUT => (
        "scope:iteration-cut",
        PremiseScope,
        "the back edges selected as iteration cuts by validated cycle structure",
        "a cut edge contributes no arrival set: first-arrival facts never become loop invariants",
        &["scope:dominance-order", "formation:control-flow-validation"],
        &[tv!( "verification/reconstruction/machine_flow.rs"), tv!( "verification/reconstruction/terminator_facts.rs")]
    );
    SCOPE_PLACE_SUBSTITUTION => (
        "scope:place-substitution",
        PremiseScope,
        "a proposition and an exact place or value substitution",
        "the proposition with every observing place or value rewritten; substitutions are total maps over the proposition vocabulary",
        &["formation:proposition-context"],
        &[tv!( "verification/substitution.rs")]
    );
    SCOPE_CALL_FRAME => (
        "scope:call-frame-rewrite",
        PremiseScope,
        "a callee proposition and the call's validated argument/receiver/result substitution",
        "the proposition rewritten into the caller frame; a callee fact enters the caller only through this substitution",
        &["scope:place-substitution"],
        &[tv!( "verification/call_composition.rs"), tv!( "verification/substitution.rs")]
    );
    SCOPE_CRASH_FILTER => (
        "scope:crash-origin-filter",
        PremiseScope,
        "the axiom set reconstructed before a crash site",
        "only propositions whose observed storage keeps entry meaning are retained; later-storage observations are dropped before crash proofs",
        &["scope:dominance-order"],
        &[tv!( "verification/reconstruction/crash_field_origins.rs"), tv!( "verification/reconstruction/crash_paths.rs")]
    );
    SCOPE_PRIMITIVE_SNAPSHOTS => (
        "scope:primitive-value-snapshots",
        PremiseScope,
        "a copied primitive SSA value's reaching-store query over the machine within the bounded work limit",
        "the exact root/path store identities reaching the value, stopping at definitions and preserving only statically disjoint sibling writes; a runtime-selected element is never an exact definition and overlaps every sibling; queries never evaluate source expressions or consult the requested contract",
        &["scope:dominance-order"],
        &[tv!( "verification/reconstruction/primitive_snapshots.rs")]
    );
    SCOPE_FIELD_SNAPSHOTS => (
        "scope:field-value-snapshots",
        PremiseScope,
        "live unconditional typed canonical Boolean, integer, or IEEE field-to-SSA equalities before store or call invalidation, outside private crash reconstruction; IEEE captures require exact format and bit identity, never IEEE comparison equality",
        "affected scalar facts, including IEEE comparisons, are re-expressed through one saved value per field without splitting connectives or growing the fact roster; remaining affected observations still expire, so old field facts cannot justify replacement bits; copying a byte-predicate verdict does not snapshot the observed byte contents or preserve a content atom across a write",
        &["scope:dominance-order", "fact:structural-effect-observation"],
        &[tv!( "verification/field_snapshots.rs"), tv!( "verification/reconstruction/operation_facts.rs"), tv!( "verification/call_composition.rs")]
    );

    SCOPE_BORROWED_STORAGE => (
        "scope:borrowed-storage-observations",
        PremiseScope,
        "the validated machine's complete borrowed-block origin sets and current observation paths",
        "writes and calls invalidate facts whenever any possible physical root and projection overlap; normal exits can restate a surviving fact in the invocation namespace only when every possible origin agrees on that exact parameter and projection; local or ambiguous backings grant no equality; byte-content atoms preserve their exact descriptor identity and extent under formal substitution, while backing aliases govern invalidation only and never equate parent and subslice predicates",
        &["formation:machine-validation", "scope:place-substitution"],
        &[tv!("validation/borrowed_windows.rs"), tv!("validation/propositions.rs"), tv!("verification/storage_observations.rs"), tv!("verification/reconstruction/machine_context.rs"), tv!("verification/reconstruction.rs")]
    );

    // -- Write invalidation --
    INV_ESTABLISH_LOCAL => (
        "invalidation:establish-primitive-local",
        WriteInvalidation,
        "an EstablishPrimitiveLocal producing a structural result place",
        "every proposition still observing the result place after exact scalar capture is removed from the axiom set",
        &["fact:structural-effect-observation", "scope:field-value-snapshots", "scope:borrowed-storage-observations"],
        &[tv!( "verification/reconstruction/operation_facts.rs"), tv!( "validation/propositions.rs")]
    );
    INV_PRIMITIVE_STORE => (
        "invalidation:write-only-primitive-store",
        WriteInvalidation,
        "a WriteOnlyPrimitiveStore to a destination place",
        "every proposition still observing the destination after exact scalar capture is removed from the axiom set",
        &["fact:structural-effect-observation", "scope:field-value-snapshots", "scope:borrowed-storage-observations"],
        &[tv!( "verification/reconstruction/operation_facts.rs"), tv!( "validation/propositions.rs")]
    );
    INV_ATOMIC_ACCESS => (
        "invalidation:atomic-access",
        WriteInvalidation,
        "an AtomicAccess whose event modifies its leaf (store, read-modify-write, swap, or compare-exchange)",
        "every proposition still observing the accessed root is removed from the axiom set; the event itself publishes no equation",
        &["scope:field-value-snapshots", "scope:borrowed-storage-observations"],
        &[tv!( "verification/reconstruction/operation_facts.rs"), tv!( "validation/propositions.rs")]
    );
    INV_FIELD_STORE => (
        "invalidation:structural-field-store",
        WriteInvalidation,
        "a structural field store whose canonical write path may resolve, a whole bounded-byte-owner replacement, or formation of a mutable record reference whose later stores use a different carrier identity",
        "propositions still observing the exact write after exact scalar capture are removed; an unresolvable path forgets the entire root; mutable record reference formation retires source-root observations before indirect mutation, and release restores permission rather than old contents",
        &["scope:place-substitution", "scope:field-value-snapshots", "scope:borrowed-storage-observations"],
        &[tv!( "verification/reconstruction/operation_facts.rs"), tv!( "validation/structural/operations/structural_paths.rs")]
    );
    INV_BYTE_WRITE => (
        "invalidation:byte-sequence-write",
        WriteInvalidation,
        "a byte-sequence write over a destination extent",
        "propositions observing the destination extent end their validity at the write",
        &["fact:byte-extent-length"],
        &[tv!( "verification/reconstruction/operation_facts.rs"), tv!( "validation/byte_sequence/write.rs")]
    );
    INV_ELEMENT_WRITE => (
        "invalidation:element-view-write",
        WriteInvalidation,
        "an element-view write over a destination extent",
        "propositions observing the destination extent end their validity at the write",
        &["fact:element-extent-length"],
        &[tv!( "verification/reconstruction/operation_facts.rs"), tv!( "validation/element_view/write.rs")]
    );
    INV_REFERENCE_RELEASE => (
        "invalidation:reference-release",
        WriteInvalidation,
        "a ReleaseReference ending a reference's custody",
        "facts predicated on the released reference's validity no longer hold after the release point",
        &["formation:references"],
        &[ts!( "structural_effect.rs"), tv!( "validation/references.rs")]
    );

    // -- Call composition --
    // A completed local array's validated byte window has no owned-field
    // spelling either. Like a parameter window, its binder may be omitted
    // only when no requires, ensures, outcome or crash predicate observes it.
    // This omission imports no proposition about the window or its backing.
    COMP_CALL_INSTANTIATION => (
        "composition:call-instantiation",
        CallComposition,
        "a call operation's callee signature, argument substitution, and the callee's verified contract",
        "parameter equalities, instantiated requires obligations, and imported ensures propositions composed into the caller frame; borrowed payload substitutions preserve the original root and each canonical Case/Field coordinate, with active-case admission before call invalidation; a lent stored view field has no owned-field spelling, so its binder is omitted only when the complete published contract never observes it",
        &["scope:call-frame-rewrite", "formation:contract-validation", "scope:field-value-snapshots", "scope:borrowed-storage-observations", "formation:structural-operations", "formation:active-case-projection"],
        &[tv!( "verification/call_composition.rs"), ts!( "call_composition.rs"), ts!( "static_path.rs")]
    );
    COMP_CALL_ROW_VALIDATION => (
        "composition:call-row-validation",
        CallComposition,
        "the call-composition semantic rows supplied for the module",
        "each call tag has exactly one row with a matching schema; missing, duplicate, or schema-mismatched rows reject",
        &["formation:operation-validation"],
        &[ts!( "call_composition.rs"), ts!( "semantic_rows.rs")]
    );
    COMP_DYNAMIC_DISPATCH => (
        "composition:dynamic-dispatch",
        CallComposition,
        "a vetted dynamic-dispatch candidate set and the call's shared argument frame",
        "per-candidate composition under one set of shared requires obligations; the candidate set is validated, not chosen by evidence",
        &["composition:call-instantiation", "formation:dynamic-dispatch"],
        &[tv!( "validation/dynamic_dispatch.rs"), tv!( "validation/dynamic_dispatch/consumption.rs"), tv!( "validation/dynamic_dispatch/direct_dispatches.rs"), tv!( "validation/dynamic_dispatch/indirect_dispatches.rs"), tv!( "validation/dynamic_dispatch/rebound_descriptors.rs"), tv!( "validation/dynamic_dispatch/selections.rs"), tv!( "verification/call_composition.rs")]
    );
    COMP_BOUNDARY_CALL => (
        "composition:boundary-call",
        CallComposition,
        "a boundary call's declared suspension plan and external contract",
        "the boundary requires obligations and ensured effects composed through the fixed byte view of the suspension semantics",
        &["composition:call-instantiation", "formation:suspension-call-plan"],
        &[tv!( "validation/suspension_call_plan.rs"), tv!( "verification/call_composition.rs"), ts!( "call_composition/fixed_byte_view.rs"), ts!( "static_path.rs")]
    );
    COMP_IMPORTED_ENSURES => (
        "composition:imported-ensures-trust",
        CallComposition,
        "a callee's ensures clauses verified under its own obligation set",
        "the ensures propositions enter the caller as facts only after the callee's verification and frame substitution; caller evidence cannot weaken them",
        &["composition:call-instantiation", "owner:contract-ensures"],
        &[tv!( "verification/call_composition.rs")]
    );

    // -- Cycle composition --
    COMP_RECURSIVE_COMPONENT => (
        "composition:recursive-component-obligation",
        CycleComposition,
        "the canonical module's recursive component data: members, call sites, ranking relation, and commitments",
        "component, relation, well-foundedness, and per-edge decrease obligations reconstructed from canonical data; a certificate cannot add members, remove call sites, or choose a different relation",
        &["formation:proof-recursion-validation"],
        &[tv!( "proof_recursion.rs"), tv!( "validation/proof_recursion.rs")]
    );
    COMP_RECURSIVE_ADMISSION => (
        "composition:recursive-component-admission",
        CycleComposition,
        "a reconstructed recursive component with its edge obligations and supplied evidence",
        "each component and edge obligation is verified against its reconstructed question; unmatched evidence rejects",
        &["composition:recursive-component-obligation", "route:certificate-derived"],
        &[pa!( "admission/recursion.rs")]
    );
    COMP_WELL_FOUNDED => (
        "composition:well-founded-relation",
        CycleComposition,
        "the component's declared ranking relation and measure",
        "obligations that the relation is well-founded and that every cycle edge strictly decreases the measure",
        &["composition:recursive-component-obligation"],
        &[tv!( "proof_recursion.rs")]
    );
    COMP_RUNTIME_CYCLE => (
        "composition:runtime-control-cycle",
        CycleComposition,
        "a machine's validated runtime control-cycle declarations and edges",
        "cycle membership, obligation identities, and accepted-cycle checks reconstructed from canonical control flow",
        &["formation:control-flow-validation", "scope:iteration-cut"],
        &[tv!( "control_cycles.rs"), tv!( "control_cycles/reconstruction.rs"), tv!( "control_cycles/validation.rs")]
    );
    COMP_RANKED_SCC => (
        "composition:ranked-scc-validation",
        CycleComposition,
        "a machine's natural-cycle ranking declaration over canonical cyclic components",
        "the ranking validates exact component and edge coverage, successor-rank substitution, and strict descent on every cycle; ranked machines keep separate authority for crash facts",
        &["formation:control-flow-validation", "terminator:crash"],
        &[tv!( "control_cycles/validation.rs"), tv!( "validation/control_flow/unranked_cycles.rs")]
    );
    COMP_CYCLE_QUESTION => (
        "composition:cycle-question",
        CycleComposition,
        "a recursive component or runtime cycle's reconstructed obligation set",
        "the composed question asked of each cycle obligation; evidence answers the reconstructed question, never a producer-selected one",
        &["composition:recursive-component-admission", "composition:runtime-control-cycle"],
        &[tv!( "verification.rs")]
    );

    // -- Shared formation and orchestration --
    // Completed unrestricted records may return owned byte fields with their
    // declared primitive predicates. Each return rejoins its exact completed
    // producer/type/multiplicity; this does not admit unchecked construction.
    FORM_MODULE_STRUCTURE => (
        "formation:module-structure",
        SharedFormation,
        "the terminal module's canonical representation, including leaf-typed structural fields, explicit leaf copies, bounded byte fields with nonempty primitive predicate sets, plain byte-predicate domains without content or establishment routes, and nominal attachment metadata independent of storage establishment; actual zero establishments require empty bytes to satisfy every field predicate; structural domain IDs and canonical identities stay unique, and a repeated semantic family is admitted only for the same primitive byte predicate on distinct exact carriers, never a duplicate family/carrier pair or a repeated non-byte family",
        "structural validation accepting exactly well-formed modules before any reconstruction; malformed representations reject with a ModuleError; a service marker is childless but not a constructible empty record: direct owned markers are Affine, owning containers cannot be Unrestricted, shared or mutable borrowed formals remain Unrestricted, and ownership traversal stops at reference and view boundaries; zero initialization cannot create a marker value; exact service-field correspondence accepts a retained relevant marker carrier or a legacy nonempty erased carrier identity without granting authority, which still requires independent requirement, selected-provider and occurrence-custody authentication; bounded owned byte normal results require an authenticated owned producer at every return, with exact type, multiplicity and qualifications; their capacity, predicates and dominance remain independently checked; shared descriptor returns preserve a descriptor value and independently reconstruct its backing from the current body: every possible return resolves to an incoming borrow, an owned incoming borrow-carrying leaf, or an exact module literal; local owned backing and unresolved or cyclic origins cannot establish escape authority",
        &["root:verification-contract"],
        &[tv!( "validation.rs"), tv!( "validation/byte_sequence.rs"), tv!( "validation/element_view.rs"), tv!( "validation/scalar.rs"), tv!( "validation/structural.rs"), tv!( "validation/error.rs"), tv!( "validation/foundation.rs"), tv!( "validation/foundation/boundary_machines.rs"), tv!( "validation/foundation/domains.rs"), tv!( "validation/foundation/machine_foundations.rs"), tv!( "validation/foundation/provider_candidates.rs"), tv!( "validation/foundation/provider_result.rs"), tv!( "validation/foundation/services.rs"), tv!( "validation/foundation/structural_types.rs"), tv!( "validation/foundation/service_binding_custody.rs"), ts!( "service_binding.rs"), ts!( "descriptor_results.rs"), tv!( "lib.rs"), pa!( "lib.rs"), "omega-rust/psi/foundation/semantic-vocabulary/src/byte_predicates.rs", "omega-rust/psi/compilation/representations/terminal-psi/src/terminal_module/types/structural.rs"]
    );
    // Exclusive primitive block loans use the same exact source, access,
    // availability and alias replay as aggregate address loans. The plain
    // referent rule applies at the root as well as at aggregate children.
    // A shared bounded byte view may have a different carrier identity from
    // its fixed-array backing. The common referent judgment requires unsigned
    // non-address u8 elements and the exact declared positive extent; it grants
    // no authority. Source authentication, dominance, qualification and frontier
    // checks still establish that the backing may be lent and remains live.
    FORM_MACHINE => (
        "formation:machine-validation",
        SharedFormation,
        "one machine's signature, blocks, parameters (including shared-borrow record and primitive-scalar views and exclusive primitive-scalar, plain-record, plain-sum and plain-array state transfers joined at block parameters, with fresh owned homes retaining their affine disposal obligations while backing exclusive state loans, and shared or mutable bounded byte owner loans whose plain byte qualifications join a block parameter only by whole forwarding from an identically declared signature or dominating block parameter; mutable transport consumes the prior name and every permitted write independently preserves its primitive predicates; a shared borrowed byte view keeps its plain byte qualifications at a block parameter only when every arrival forwards it whole from an identically declared signature or available block parameter, or names the one available literal establishment whose roster is exactly those qualifications, which literal validation checks against its octets), contract, and declared invariants",
        "the machine validates as a whole before its obligations are reconstructed; a primitive scalar, an established service marker, a plain sum or fixed array, or a record composed of ordinary plain leaves and marker-bearing record children may be received as an unrestricted shared or mutable state loan, with exact source type, access, qualification and path replay; mutable loans participate in the existing alias, availability and frontier checks, so transferred names cannot lend again; byte and element subslices retain their backing origins, and only an available authenticated descriptor derived from the exact current exclusive parameter may observe its reborrow, never an old descriptor sharing only an ancestor; simultaneous shared-child and mutable-backing successor arguments still reject; a lent record's ordinary plain leaves include a bounded OWNED byte field, which the plain-leaf classification already admits and whose payload the loan addresses in place; a borrowed or bounded byte VIEW field stays out, being a referent pointer the loan would relocate. Admitting the field in the parameter shape grants it nothing: every read and store of a structural byte field remains independently validated and still rejects what it rejected before (InvalidStructuralByteSequenceFieldAccess, InvalidStructuralByteSequenceFieldStore); this borrowing-only classification does not admit marker construction, plain payload copies or new owned homes; independently, unrestricted owned bounded-byte state arrivals forward a whole invocation or available block parameter with identical carrier, access and primitive qualification roster, never a loan descriptor or newly established predicate",
        &["formation:module-structure"],
        &[tv!( "validation/machine.rs"), tv!( "validation/machine/blocks.rs"), tv!( "validation/machine/contract_clauses.rs"), tv!( "validation/machine/custody_operations.rs"), tv!( "validation/machine/scalar_result_operations.rs"), tv!( "validation/machine/structural_places.rs"), tv!( "validation/block_views.rs"), tv!( "validation/block_views/mutable_records.rs"), tv!( "validation/byte_sequence/subslice.rs"), tv!( "validation/element_view/subslice.rs"), ts!( "call_composition/fixed_byte_view.rs")]
    );
    FORM_PROPOSITION_CONTEXT => (
        "formation:proposition-context",
        SharedFormation,
        "a proposition and the value/place context it is checked under",
        "only well-formed propositions over known values, places, and types are judged; Boolean, integer and IEEE field terms rejoin a nonempty relevant canonical leaf of the same declared type, either a record field or a static in-bounds fixed-array element whose element shape is that primitive scalar; IEEE literals retain exact bits and format, IEEE comparisons require one format, and only symmetric equality and inequality require canonical operand order; byte-predicate atoms require a readable byte carrier at their exact root and canonical path, with an empty path reserved for a whole declared carrier and no truth inferred from the term itself; malformed propositions reject before any rule runs",
        &["formation:module-structure"],
        &[tv!( "validation/propositions.rs"), tv!( "validation/contracts.rs"), sv!( "byte_predicates.rs"), sv!( "proposition.rs"), sv!( "proposition/scalar_terms.rs"), sv!( "proposition/propositions.rs"), sv!( "proposition/value_ids.rs")]
    );
    FORM_OPERATION_VALIDATION => (
        "formation:operation-validation",
        SharedFormation,
        "one operation's kind-specific fields, types, and declared identities",
        "the operation validates under its kind's exact formation rules before reconstruction assigns it any semantics",
        &["formation:module-structure", "formation:proposition-context"],
        &[tv!( "validation/operations.rs"), tv!( "validation/operations/arithmetic_operands.rs"), tv!( "validation/operations/call_operands.rs"), tv!( "validation/operations/scalar_operands.rs"), tv!( "validation/operations/storage_operands.rs")]
    );
    FORM_CONTROL_FLOW => (
        "formation:control-flow-validation",
        SharedFormation,
        "the machine's blocks, edges, terminators, and declared cycle structure",
        "validated control flow: successors, dominance, and cycle declarations agree before reconstruction relies on them; unrestricted constructors share a disposal-free lifetime but retain each operation's initialization and evidence checks, while records and affine cases retain ordinary frontier replay and their applicable disposal rules; a returned shared byte or element descriptor requires a body-derived origin for the exact available source, including ordinary and nested call results; nominal result shape alone does not discharge the original backing lifetime",
        &["formation:module-structure"],
        &[tv!( "validation/control_flow.rs"), tv!( "validation/control_flow/block_checks.rs"), tv!( "validation/control_flow/block_graph.rs"), tv!( "validation/control_flow/definitions.rs"), tv!( "validation/control_flow/unranked_cycles.rs"), tv!( "control_graph.rs"), ts!( "descriptor_results.rs")]
    );
    FORM_CONTRACT => (
        "formation:contract-validation",
        SharedFormation,
        "a machine's requires and ensures clauses over its signature frame",
        "contract clauses validate as well-formed propositions before reconstruction instantiates them",
        &["formation:proposition-context"],
        &[tv!( "validation/contracts.rs")]
    );
    FORM_SCALAR_QUALIFICATIONS => (
        "formation:scalar-qualifications",
        SharedFormation,
        "the machine's scalar qualification declarations",
        "scalar qualifications validate before they qualify any obligation or fact",
        &["formation:machine-validation"],
        &[tv!( "validation/scalar/qualifications.rs")]
    );
    FORM_STRUCTURAL_ROSTERS => (
        "formation:structural-qualification-rosters",
        SharedFormation,
        "the declared structural qualification rosters",
        "rosters validate before reconstruction consults them for field and case facts",
        &["formation:machine-validation"],
        &[tv!( "validation/structural/qualification_rosters.rs")]
    );
    FORM_STRUCTURAL_RESULT_CONTRACTS => (
        "formation:structural-result-contracts",
        SharedFormation,
        "the declared structural result contract clauses",
        "structural result contracts validate before call composition imports them; a service marker is excluded from ordinary plain-owned payload admission even when it has no runtime fields; bounded owned byte returns authenticate an exact concatenation producer or ordinary callee result, preserve nominal type, unrestricted multiplicity and whole qualification roster, and carry no reference origins, projected qualifications, establishment rows or claims; independent operation validation retains capacity and byte-predicate obligations",
        &["formation:contract-validation"],
        &[tv!( "validation/structural/result_contracts.rs")]
    );
    FORM_CONTENT => (
        "formation:content-conservation",
        SharedFormation,
        "the module's content-identity claims, partitions, and reshuffles",
        "content conservation validates as an algebra over declared claims before reconstruction infers its propositions; a borrowed Case projection supplies neither an entry content-claim binding nor a content-partition transfer",
        &["formation:module-structure"],
        &[tv!( "validation/content.rs"), sv!( "content.rs"), sv!( "structural_places.rs")]
    );
    FORM_AFFINE_CLEANUP => (
        "formation:affine-cleanup",
        SharedFormation,
        "the machine's affine cleanup actions and continuations, including Jump edges whose owned arguments move one projected affine child while the residual complement dies on the edge, owned projections out of a structural parameter whose untouched complement dies as residual rows on the machine's own return terminator — or whose covering moves consume the root completely — while continuation edges close only temporary and result roots, and member `drop<T>` specializations whose nominal return carries the same exact cleanup shape",
        "cleanup plans validate before terminators reconstruct their obligations; a childless service marker supplies neither record fields nor a nominal cleanup contract by shape alone",
        &["formation:machine-validation"],
        &[tv!( "validation/affine_cleanup.rs"), tv!( "validation/affine_cleanup/continuation.rs")]
    );
    FORM_CONFORMANCE => (
        "formation:conformance-applications",
        SharedFormation,
        "the module's declared conformance applications",
        "conformance applications validate before normalization binds them",
        &["formation:module-structure"],
        &[tv!( "validation/conformance_applications.rs")]
    );
    FORM_REACH => (
        "formation:reach-applications",
        SharedFormation,
        "the module's declared reach applications",
        "reach applications validate before service-reach and dynamic-dispatch checks use them",
        &["formation:module-structure"],
        &[tv!( "validation/reach_applications.rs")]
    );
    FORM_ROOT_REACH => (
        "formation:root-service-reach",
        SharedFormation,
        "the module's declared root service reach",
        "root service reach validates before boundary and dispatch checks consult it",
        &["formation:reach-applications"],
        &[tv!( "validation/root_service_reach.rs")]
    );
    FORM_FLOAT => (
        "formation:float-meaning",
        SharedFormation,
        "the declared IEEE float meanings used by float operations",
        "float meanings validate and project before float rows are accepted",
        &["formation:operation-validation"],
        &[tv!( "validation/float_meaning.rs"), tv!( "verification/float_meaning_projection.rs")]
    );
    FORM_FRONTIER => (
        "formation:frontier",
        SharedFormation,
        "the machine's claim frontier, possible projected storage regions pinned by shared-borrow join parameters, and live whole owned homes backing exclusive state loans at each block and traversal point, including locally captured descriptor backing and current descendant permissions, plus reference loans stored inside an owned record carrier whose carrier coordinate re-homes onto the bound name across a whole owned edge move while formation identity, roots, and referents stay fixed",
        "the reconstructed frontier used by crash guards and cleanup checks is the validated one; every structural result publication rejects replacement of a root pinned by a shared successor loan or live reference, including on cyclic re-entry; shared state loans reject any overlapping possible origin after forwarding, while disjoint field writes remain legal and uncertain projections never narrow interference; exclusive state loans retain the backing home's affine disposal obligation through every arrival until exact normal-return disposition, copy-payload sources the owned-read gate exempts are availability-checked at every consuming establishment, and owned byte concatenations the gate exempts carry no affine debt and are availability-checked by byte-view use validation at every loan; claim-free bounded owned byte returns require the exact authenticated result contract and independent control-flow dominance, while partial moves, live claims and cleanup obligations still reject; exact unrestricted bounded-owned call results likewise have no disposal entry and must dominate every byte observation or loan; copied bounded-byte record operands are unrestricted but must already be available before the complete record establishes, and each member independently checks source capacity, access and predicate implication; local descriptor cohorts remain live through transparent derivation, aggregate capture, simultaneous edge transport and unknown escape, with definitions retiring only their own value instance; descendant access exemptions require exact current names established on every predecessor, never a possible origin or aggregate capture; backing consumption and owned publication cannot use descendant exemptions; dynamic disjointness requires independently reconstructed all-path scalar ordering and unchanged captured endpoints and selectors; exact scalar entry requirements and current normal-return guarantees provide unconditional u64 order facts only in their authenticated formal/actual/result namespaces, with call guarantees introduced after result definition and every definition or incomplete join retiring stale facts; full proof admission still discharges caller requirements and callee guarantees, while cyclic ambiguity, reference crossings and nested borrowed backing remain conservative; descriptor metadata and physical slot replacement remain distinct from borrowed payload access; affine borrowed descriptors retain exact carrier disposal without acquiring ownership of backing; ordinary call effects are independently reconstructed from current callee reads and writes and substituted through exact actual projections, with callee scalar coordinates removed and recursive, opaque or authority-carrying calls retaining conservative fences; authenticated shared descriptor call results substitute every formal source position and projection through the exact actual argument and join caller loan cohorts, preserving the original backing across further derivation and nested calls; writes or backing consumption while any returned descendant remains live reject, while its last observation ends the loan absent other live descendants or escape; callee runtime coordinates cannot establish caller disjointness, and literal residence is independent of caller carrier storage",
        &["scope:dominance-order"],
        &[tv!( "validation/frontier.rs"), tv!( "validation/frontier/block_entry.rs"), tv!( "validation/frontier/block_parameters.rs"), tv!( "validation/frontier/operations.rs"), tv!( "validation/frontier/shared_loans.rs"), tv!( "validation/borrowed_windows.rs"), tv!( "validation/frontier/terminators.rs"), tv!( "validation/frontier/traversal.rs"), tv!( "validation/frontier/descriptor_loans.rs"), tv!( "validation/frontier/descriptor_loans/aliases.rs"), tv!( "validation/frontier/descriptor_loans/call_accesses.rs"), tv!( "validation/frontier/descriptor_loans/accesses.rs"), tv!( "validation/frontier/descriptor_loans/flow.rs"), tv!( "validation/frontier/descriptor_loans/scalar_ordering.rs"), ts!( "borrowed_storage.rs"), ts!( "scalar_ordering.rs"), ts!( "scalar_ordering/contracts.rs"), ts!( "descriptor_lifetimes.rs"), ts!( "descriptor_results.rs")]
    );
    FORM_CRASH => (
        "formation:crash-validation",
        SharedFormation,
        "a crash site's declared guard terms and outcome, plus each operation's declared crash contracts checked against its owning machine's crash-route coverage",
        "crash sites and operation-level crash contracts validate before their private fact bundles are reconstructed",
        &["formation:machine-validation", "formation:frontier"],
        &[tv!( "validation/crash.rs"), tv!( "validation/crash/operation_contracts.rs"), tv!( "validation/crash/outcome.rs"), tv!( "verification/reconstruction/crash_obligations.rs")]
    );
    FORM_CRASH_REQUIREMENTS => (
        "formation:crash-entry-requirements",
        SharedFormation,
        "a machine's declared entry requirements and independently reconstructed pre-operation paths, including integer-order chains, exact primitive failure predicates and complements of supported scalar crash predicates",
        "certificates independently prove the caller's invocation-entry ceiling or disprove every uncovered call-time alternative on every retained path without normal-return facts; refutation retains only temporally stable requirements, and direct predicate equality covers only unchanged entry meanings; cyclic coverage keeps entry-only facts without invariant custody; exact forwarding and predicate formation remain trusted Rust decisions, while producers emit certificate supplies and consumers replay them through the certificate checker and mathematical-core kernel",
        &["formation:crash-validation", "formation:mathematical-core"],
        &[pa!( "admission/denotation.rs"), pa!( "certificate_search.rs"), pa!( "certificate_search/implications.rs"), pa!( "certificate_search/original_citations.rs"), pa!( "certificate_search/integer_order.rs"), pa!( "certificate_search/order_chain.rs"), tv!( "verification/reconstruction/crash_obligations.rs"), tv!( "verification.rs")]
    );
    FORM_BLOCK_INVARIANTS => (
        "formation:scalar-block-invariants",
        SharedFormation,
        "a header block's declared scalar invariants over the destination telescope, immutable invocation formals, and storage observations rooted at places alive for the whole invocation; members are finite integer or Boolean relations, mathematical integer comparisons over exact typed scalar embeddings, or exact-format IEEE comparisons and their connectives",
        "invariants validate as well-formed propositions before per-edge obligations are reconstructed; mathematical embeddings retain the exact scoped value and finite carrier, never admitting another block's value or a substituted signedness or width",
        &["formation:proposition-context", "scope:header-edge-arrival"],
        &[tv!( "validation/scalar/block_invariants.rs")]
    );
    FORM_EVIDENCE => (
        "formation:evidence-orchestration",
        SharedFormation,
        "the module's evidence bundle: envelopes, provenance, and producer identity",
        "evidence validates for provenance and coverage: every obligation has evidence, no duplicate, unknown, noncanonical, or unused evidence is accepted",
        &["formation:module-structure", "route:certificate-derived"],
        &[tv!( "verification.rs"), tv!( "verification/evidence_provenance.rs"), tv!( "verification/proof_bundle.rs"), tv!( "validation/evidence.rs"), tv!( "validation/evidence/contract_lanes.rs"), tv!( "validation/evidence/guarded_call_outputs.rs"), tv!( "validation/evidence/proof_output_calls.rs"), tp!( "artifacts/proof_bundle/crash.rs")]
    );
    FORM_PROOF_RECURSION => (
        "formation:proof-recursion-validation",
        SharedFormation,
        "the module's declared recursive components and commitments",
        "recursion declarations validate before component obligations are reconstructed",
        &["formation:contract-validation", "composition:ranked-scc-validation"],
        &[tv!( "validation/proof_recursion.rs")]
    );
    FORM_QUOTIENT => (
        "formation:quotient-correspondence",
        SharedFormation,
        "the retained quotient-correspondence declaration and its canonical bridge",
        "the standalone correspondence is independently replayed: theorem roles, parameters, and hermetic identities must match exactly",
        &["formation:module-structure"],
        &[tv!( "quotient_correspondence.rs"), tv!( "validation/quotient_correspondence.rs")]
    );
    FORM_DYNAMIC_DISPATCH => (
        "formation:dynamic-dispatch",
        SharedFormation,
        "the module's dynamic-dispatch declarations and candidate sets",
        "candidate sets validate before dispatch calls compose them",
        &["formation:contract-validation", "formation:root-service-reach"],
        &[tv!( "validation/dynamic_dispatch.rs"), tv!( "validation/dynamic_dispatch/consumption.rs"), tv!( "validation/dynamic_dispatch/direct_dispatches.rs"), tv!( "validation/dynamic_dispatch/indirect_dispatches.rs"), tv!( "validation/dynamic_dispatch/rebound_descriptors.rs"), tv!( "validation/dynamic_dispatch/selections.rs")]
    );
    FORM_SUSPENSION => (
        "formation:suspension-call-plan",
        SharedFormation,
        "a boundary call's declared suspension plan",
        "the suspension plan validates before boundary composition interprets it",
        &["formation:contract-validation"],
        &[tv!( "validation/suspension_call_plan.rs")]
    );
    FORM_STORAGE_REGIONS => (
        "formation:storage-regions",
        SharedFormation,
        "authenticated structural declarations and a validated projected or canonical field path; live access authority remains the caller's independent obligation",
        "storage geometry conservatively encloses runtime selectors and unresolved fields; reference indirection remains separate from the enclosing direct path, so truncating at a runtime index cannot manufacture disjointness; canonical fields rejoin exact declaration identities; different Case namespaces may overlay, and unknown or active-case geometry grants no disjointness; this geometry grants no reference, ownership or proof authority",
        &["formation:contract-validation"],
        &[ts!( "storage_regions.rs")]
    );
    FORM_ACTIVE_CASE_PROJECTION => (
        "formation:active-case-projection",
        SharedFormation,
        "a validated projected operation and each canonical Case prefix beneath its exact original root, with the current incoming facts before this operation invalidates storage observations",
        "each selected case is kernel-certified from surviving membership facts, or all other declared cases are refuted by surviving same-place membership observations whose Boolean results are known false; merely resolving a Case and its following field grants no authority, and this admission introduces no membership fact, ownership, or reference permission",
        &["scope:dominance-order", "fact:semantic-axiom-roster", "fact:structural-case-arm", "conversion:value-equality-denotation", "rule:semantic-axiom", "rule:implication-elimination"],
        &[tv!( "verification/reconstruction/operation_facts.rs"), tv!( "verification/reconstruction/operation_facts/case_hops.rs"), tv!( "validation/structural/operations/structural_paths.rs"), ts!( "primitive_place.rs"), ts!( "static_path.rs")]
    );
    FORM_REFERENCES => (
        "formation:references",
        SharedFormation,
        "the machine's establish/release discipline over mutable primitive or plain-owned record referents and shared named referents, including exact relevant-field/fixed-index and Case/Field ingress projections",
        "reference operations validate against declared custody — a minted, projected, or returned reference never exceeds its source's access, and shared carriers never move an ingress leaf; field observations require a live whole shared or mutable carrier with no live child or loan rooted at that carrier, while ordinary owned storage access compares its canonical operation region with every live origin candidate and carrier region, permitting disjoint siblings but suspending any overlap until release; different Case namespaces alone never establish disjointness because payloads overlay; field writes through an explicit Referent require an available live affine mutable carrier with no live child; ordinary nonescaping calls may lend an exact relevant-field/fixed-index subtree below that crossing with attenuated access, preserving the full argument path while conservatively comparing the parent's origin for alias rejection; Case/Field ingress establishment retains the same source access and lifetime checks and proves every case active before formation; this does not admit new referent-crossing establishment classes or local escapes; a whole mutable record result may select static paths beneath one formal ingress, with one permission conservatively pinning every candidate, exact reachable-return coverage and acyclic reference-result dependencies rather than circular signature evidence; every reference-returning declaration retains its complete exact source roster before observations are reconstructed; whole affine owned record block parameters over constructible carriers may carry stored reference leaves that re-home their carrier coordinate onto the bound place across the edge, while borrowed, projected, qualified, or non-affine carriers still reject",
        &["formation:operation-validation", "formation:active-case-projection"],
        &[tv!( "validation/references.rs"), tv!( "validation/structural/access_regions.rs"), ts!( "primitive_place.rs"), ts!( "static_path.rs")]
    );
    FORM_PARTIAL_AFFINE => (
        "formation:partial-affine",
        SharedFormation,
        "the machine's partial and trivial affine locals and discards, plus projected affine roots reached through locals, call products, constructed records and structural cases, and join block parameters",
        "partial affine structure validates before affine facts or obligations are reconstructed",
        &["formation:machine-validation"],
        &[tv!( "validation/partial_affine.rs")]
    );
    FORM_PRIMITIVE_STORAGE => (
        "formation:primitive-storage",
        SharedFormation,
        "the machine's primitive locals, stores, and reads",
        "primitive storage operations validate exact relevant-field/fixed-index paths, root access and initialization before facts are reconstructed; direct primitive read/store paths continue to reject Case even though borrowed structural arguments can resolve case-qualified geometry; entry and block parameters share the exact declaration and access query, while mutable block availability and alias replay independently reject stale or conflicting loan names",
        &["formation:operation-validation"],
        &[tv!( "validation/primitive_storage.rs"), ts!( "primitive_place.rs"), ts!( "static_path.rs")]
    );
    // A completed array producer may also lend a fixed byte window: the
    // existing window relation checks the exact element, field path, extent,
    // access and qualifications. This does not grant a new owned-parameter
    // presentation; completed-result provenance remains required on this route.
    FORM_SCALAR_ARRAY => (
        "formation:scalar-array",
        SharedFormation,
        "a whole unrestricted primitive-array parameter, complete typed or checked zero initialization, validated exact leaf snapshot, or ordinary or boundary call result with the declared array shape",
        "the array payload source validates without additional proposition authority; zero initialization must satisfy the ordinary zero-establishment rules and cannot manufacture qualifications or claims; leaf snapshots preserve their readable source, and control-flow availability independently requires each producer to dominate its uses; owned transfers remain whole, while shared or exclusive call-scoped loans may select an exact checked subtree of an initialized parameter or producer without moving its backing; projection bounds, exact formal type and access, and exclusive overlap remain independent checks",
        &["formation:operation-validation"],
        &[tv!( "validation/scalar/array.rs"), tv!( "validation/operations.rs"), tv!( "validation/structural/operations/structural_arguments/argument_checks.rs"), tv!( "validation/structural/operations/claim_transfers.rs"), ts!( "scalar_array.rs")]
    );
    FORM_SCALAR_CASE => (
        "formation:scalar-case",
        SharedFormation,
        "a scalar case's declared sum type and payload",
        "the case validates before its membership and payload facts are reconstructed",
        &["formation:operation-validation"],
        &[tv!( "validation/scalar/case.rs")]
    );
    FORM_CASE_MEMBERSHIP => (
        "formation:structural-case-membership",
        SharedFormation,
        "a structural case membership test with an exact root/path and declared sum case",
        "readable access, projection identity, dominating establishment and whole live ownership validate before a Boolean observation; no storage refinement is reconstructed",
        &["formation:operation-validation"],
        &[tv!( "validation/structural/case_membership.rs")]
    );
    // An ordinary call also transports a completed unrestricted owned record.
    // The record's field declarations retain byte predicates; root/projected
    // qualifications, result establishments and claims cannot be invented by
    // the call. Both structural call encodings replay the exact callee result.
    // Completed local storage may also lend whole or projected call-scoped
    // loans to a boundary. The same source, access, qualification, dominance
    // and overlapping-argument checks apply; this grants neither owned
    // boundary transfer nor retention after return.
    // Shared bounded-owner presentation is independent of call kind, result
    // kind and entry/block placement. The exact available root retains its
    // access and qualifications; the view observes its live length and bytes.
    // Geometry alone supplies no predicate or loan authority: formal predicate
    // implication, access attenuation and overlapping arguments still validate.
    FORM_STRUCTURAL_OPS => (
        "formation:structural-operations",
        SharedFormation,
        "structural, boundary, and effect operation custody, including literal qualification rosters checked against their exact bytes and carrier, whole bounded-owner argument loans with exact capacity and predicate declarations, and an established claim-free unrestricted whole owned byte concatenation supplied by value to an ordinary same-carrier owned formal, lent shared to an ordinary same-carrier bounded-owner formal, or lent to an ordinary/boundary shared byte-view formal without replacing its source place or carrier: primitive predicates replay from initial operands only while untouched and from declared qualifications after possible mutation; no projected qualification or exclusive access is introduced; whole owned or borrowed unrestricted unqualified primitive-array boundary actuals retain exact producer and formal shapes: shared byte loans may weaken predicates, while exclusive byte loans preserve them in both directions; a stored shared view field lends its own descriptor to an ordinary or boundary shared byte-view formal through its owning root and field path, its declared field predicates implying every byte predicate the formal's qualifications name",
        "whole shared-byte call arguments can satisfy primitive domain membership from exact live content predicates; subslices retain only subslice-preserving predicates reconstructed through their sources, never consumer-requested result annotations; these operations validate paths and custody before reconstruction; an ordinary borrowed Case/Field projection retains the original backing and must prove each case active before use, while its geometry grants neither owned transfer nor new reference-result escape authority; literal annotations cannot invent byte predicates and an exclusive whole-owner loan cannot discard the caller field invariant; repeated whole owned array actuals supply independent copies, while passing a borrowed array by value needs an explicit owned snapshot; a whole unrestricted bounded-byte field snapshot may satisfy an ordinary same-carrier owned formal's primitive byte predicates only from the exact readable source field declaration, with no other clobber or escaping mutable alias of the snapshot; this transports neither arbitrary domains nor new result qualifications; whole local array loans preserve their established backing, require exact element and extent presentation, reject use before establishment and exclusive argument overlap, and retain the owner's cleanup debt; a completed local record lends an exact field subtree as an ordinary borrowed projection under the same establishment and overlap checks, moving no custody out of the local, whether the call returns nothing, a scalar, a plain payload that holds no reference or claim, or a shared descriptor whose returned backing relation independently reconstructs through its exact actual arguments; a block parameter that rebinds a borrowed loan at a state edge lends the same exact fixed-array window a machine parameter lends, under the same access, claim and overlap checks; an ordinary structural call may transport an unrestricted bounded-owned byte result with its exact callee qualification roster, but cannot add establishment evidence, projected qualifications, reference origins or claims; a lent stored view field transports neither the record nor exclusive access, and its field predicates hold at every point because every write of the field preserves them",
        &["formation:operation-validation", "formation:active-case-projection"],
        &[tv!( "validation/structural/operations.rs"), tv!( "validation/structural/operations/boundary_requirements.rs"), tv!( "validation/structural/operations/claim_transfers.rs"), tv!( "validation/structural/operations/claim_transfers/argument_claims.rs"), tv!( "validation/structural/operations/claim_transfers/transfer_roster.rs"), tv!( "validation/structural/operations/contract_places.rs"), tv!( "validation/structural/operations/crash_continuations.rs"), tv!( "validation/structural/operations/payloadless_calls.rs"), tv!( "validation/structural/operations/primitive_calls.rs"), tv!( "validation/structural/operations/structural_arguments.rs"), tv!( "validation/structural/operations/structural_arguments/argument_checks.rs"), tv!( "validation/structural/operations/structural_arguments/argument_pairs.rs"), tv!( "validation/structural/operations/structural_paths.rs"), tv!( "validation/structural/operations/unit_operation.rs"), tv!( "validation/structural/operations/unit_operation/boundary_calls.rs"), tv!( "validation/structural/operations/unit_operation/local_establishments.rs"), tv!( "validation/structural/operations/unit_operation/structural_calls.rs"), tv!( "validation/structural/operations/unit_operation/unit_calls.rs"), ts!( "byte_sequence_predicates.rs"), ts!( "call_composition/bounded_byte_owner.rs"), "omega-rust/psi/foundation/semantic-vocabulary/src/byte_predicates.rs"]
    );
    FORM_STRUCTURAL_SCALAR => (
        "formation:structural-scalar-fields",
        SharedFormation,
        "structural scalar field reads and bounded-integer field stores with their declared intervals; a store's carrier composes fields and literal or runtime elements and ends at a record or at a mixed shape whose common field it writes directly, and a read's carrier may end in one Case step naming a payload field of that exact declared case, or at a mixed shape whose common field it reads directly because that field sits beside the tag whatever case is active",
        "field reads and stores validate and resolve their referents — a store's carrier resolves through its runtime elements as some element of each array, and its declared range obligation resolves against the declaration, independently of the stored value; an explicit Referent store retains exact mutable record permission and separately validated live custody; a case-qualified read's carrier is type-only here, its active case proved separately from reconstructed membership facts — before equations and bounds are reconstructed",
        &["formation:operation-validation"],
        &[tv!( "validation/structural/scalar_fields.rs"), tv!( "validation/structural/field_stores.rs"), ts!( "record_field.rs")]
    );
    FORM_STRUCTURAL_BYTES => (
        "formation:structural-byte-sequence-fields",
        SharedFormation,
        "structural byte-sequence fields, exact current extents and unchanged-length observations; replacement requires a bounded-owned carrier and immutable completed source whose independently replayed predicates imply the field restrictions; descriptor replacement of a shared borrowed-view field requires the one establishment of a byte literal, whose image backing outlives every root, and literal octets replaying predicates that imply the field restrictions, and ends every earlier observation of the replaced field; a qualified shared view field admits the same length and byte observations as a plain one; an indexed store into a UTF-8 field not declared ASCII-only needs every path to reach an ASCII-proved replacement of the identical field with only same-field stores or disjoint operations after it",
        "byte-sequence fields validate before extent equations and capacity obligations are reconstructed; an explicit Referent replacement resolves capacity and predicates from the exact mutable record referent while reference validation independently checks establishment, live permission and child suspension; a whole-field replacement into a completed unrestricted owned record retains its actual local producer and declared field capacity and predicates, with record availability checked before each use; no borrowed-view field or indexed-write permission follows from this owned destination; a destination that is neither resolves through one shared arrival-storage answer, which names the machine's own parameter at its declared position or a block parameter in the block declaring it and nothing else, while field descriptor replacement additionally shares the scalar store destination owner for completed records and mutable element-view results, retaining independent live-loan replay and an operation-owned bound for every dynamic selector; byte literals and provider attachments have no writable home, and the access, multiplicity and qualification tests each validator already applied are unchanged and still applied to whichever declaration it returns; a fact about that storage carries across a state edge only by following the edge's own binding, and only where the binding forwards the whole storage, so a projected argument and a case edge binding payload fields leave the fact unproven on that path, while backward discovery retains candidate names through ordinary edges and ambiguous joins without granting equality; byte-field freshness separately follows every arrival and requires both the establishing operation and its exact producer-side storage root, including through backedges",
        &["formation:operation-validation"],
        &[tv!( "validation/structural/byte_sequence_fields.rs"), tv!( "validation/structural/byte_sequence_fields/freshness.rs"), tv!( "validation/structural/byte_sequence_store.rs"), tv!( "validation/structural/byte_view_field_store.rs"), tv!( "validation/structural/field_stores.rs"), tv!( "validation/structural/runtime_indexes.rs"), ts!( "arrival_storage.rs"), ts!( "byte_sequence_predicates.rs"), "omega-rust/psi/foundation/semantic-vocabulary/src/byte_predicates.rs", "omega-rust/psi/compilation/representations/terminal-psi/src/terminal_module/types/structural.rs"]
    );
    // Concatenation imports NonEmpty from either validated operand: a byte in
    // either source survives. UTF-8, ASCII and no-NUL still require both sides.
    FORM_BYTE_OPS => (
        "formation:byte-sequence-operations",
        SharedFormation,
        "byte-sequence length, read, write, subslice, owned concatenation and whole-owner replacement operations resolve entry and state parameters through the same canonical arrival contract; bounded-owner length is metadata, including through write-only loans, while payload observations require readable access and replacement sources remain immutable; primitive qualification and projected-qualification checks apply identically at both arrival sites; concatenation result primitive qualifications replay from its operands; indexed writes into a claim-free unrestricted owned concatenation retain its exact current length and declared predicates, with all-path reaching ASCII evidence required for UTF-8 not declared ASCII-only; replacement requires an exclusive bounded-owner parameter, a permitted completed source and its current length, with every declared primitive byte predicate preserved",
        "byte-sequence operations validate access, extents and bounds before reconstruction; exact unrestricted owned bounded-byte formals may supply their live bytes and declaration predicates, without granting owned views or write-only loans payload access; whole replacement resolves an exact entry or block parameter through the one shared arrival-storage answer, which admits those two and nothing else, and requires both source and destination availability, so a transferred mutable name cannot write again; indexed writes separately require the index bound and declared byte-class goals and do not preserve incidental initializer predicates; mutable borrowed replacement sources reject and the receiving capacity obligation is reconstructed independently of predicate preservation; a completed ordinary bounded-owned byte call result imports only its exact declared primitive predicates, never stronger initializer facts; exact producer identity and dominance govern each read or replacement source",
        &["formation:operation-validation"],
        &[tv!( "validation/byte_sequence/length.rs"), tv!( "validation/byte_sequence/read.rs"), tv!( "validation/byte_sequence/write.rs"), tv!( "validation/byte_sequence/subslice.rs"), tv!( "validation/byte_sequence/store.rs"), tv!( "validation/byte_sequence/concatenation.rs"), tv!( "validation/structural/byte_sequence_fields/freshness.rs"), ts!( "arrival_storage.rs"), ts!( "byte_sequence_predicates.rs")]
    );
    FORM_CALL_GRAPH => (
        "formation:call-graph",
        SharedFormation,
        "the module's call edges between machines",
        "the call graph validates before call composition and recursion reconstruction use it",
        &["formation:module-structure"],
        &[tv!( "validation/call_graph.rs")]
    );
    FORM_MATHEMATICAL_CORE => (
        "formation:mathematical-core",
        SharedFormation,
        "the mathematical-integer term language: universe-level expressions over the judgment's level parameters, terms, typing, substitution, conversion, universe-polymorphic certificates, inductive W-type formation with dependent elimination, indexed families derived from it, a set-quotient scheme whose lifts are authored as checked declarations, theorem declarations crossing the certificate wire, and the bounded denotation that re-decides a ProofNode certificate inside the core",
        "mathematical terms are well-typed, bounded, level-scope-checked, and canonically formed before any judgment or normalization evaluates them; opposite independently proved nonstrict bounds elaborate through the existing integer order antisymmetry law, preserving both exact endpoints and their checked scalar carrier",
        &["formation:proposition-context"],
        &[pa!( "mathematical_core.rs"), pa!( "mathematical_core/term.rs"), pa!( "mathematical_core/typing.rs"), pa!( "mathematical_core/substitution.rs"), pa!( "mathematical_core/conversion.rs"), pa!( "mathematical_core/certificate.rs"), pa!( "mathematical_core/bounded_denotation.rs"), pa!( "mathematical_core/bounded_denotation/addition.rs"), pa!( "mathematical_core/bounded_denotation/binary_numerals.rs"), pa!( "mathematical_core/bounded_denotation/subtraction.rs"), pa!( "mathematical_core/bounded_denotation/booleans.rs"), pa!( "mathematical_core/bounded_denotation/equality_transport.rs"), pa!( "mathematical_core/bounded_denotation/casts.rs"), pa!( "mathematical_core/bounded_denotation/forbidden_roots.rs"), pa!( "mathematical_core/bounded_denotation/integer_operations.rs"), pa!( "mathematical_core/bounded_denotation/multiplication.rs"), pa!( "mathematical_core/indexed.rs"), pa!( "mathematical_core/quotient.rs"), pa!( "mathematical_core/scheme_dsl.rs"), pa!( "mathematical_core/theorems.rs")]
    );
    FORM_REWRITE => (
        "formation:optimization-rewrite-validation",
        SharedFormation,
        "a producer's target-neutral rewrite claim over a verified module",
        "the rewrite is independently checked: surviving structure, operations, block parameters and edge arguments obey the exact rule; proof goals, classes, ordered requirements and arrivals survive exact copy substitution, axiom citations transport only to reconstructed facts, and the transported bundle replays independently; the producer's liveness result is never trusted",
        &["formation:evidence-orchestration"],
        &[tv!( "optimization.rs"), tv!( "optimization/dead_scalar_elimination.rs"), tv!( "optimization/copy_propagation.rs"), tv!( "optimization/global_value_numbering.rs"), tv!( "optimization/sparse_conditional_constant_propagation.rs"), tv!( "optimization/control_flow_cleanup.rs"), tv!( "optimization/proof_check_elision.rs"), tv!( "optimization/proof_context_transport.rs")]
    );
    FORM_TRACE => (
        "formation:trace-observation-profile",
        SharedFormation,
        "the module's validated representation and the D39 observation profile schema",
        "trace rows are reconstructed by the verifier, not asserted by a producer: classification, event rows, and crash-site rows derive from the canonical module",
        &["formation:module-structure"],
        &[tv!( "terminal_trace_v1.rs")]
    );

    // -- The inventory itself --
    INVENTORY_SELF => (
        "inventory:ledger",
        Inventory,
        "the ledger's entries, maps, implementation sites, and trust roots",
        "mechanical coverage of the accepted dispatch and reconstruction surface; coverage is completeness, not soundness",
        &["root:verification-contract"],
        &[
            "omega-rust/psi/compilation/representations/terminal-psi/verification/src/trusted_surface.rs",
            "omega-rust/psi/compilation/representations/terminal-psi/verification/src/trusted_surface/sites.rs",
            "omega-rust/psi/compilation/representations/terminal-psi/verification/src/trusted_surface/checker.rs",
            "omega-rust/psi/compilation/representations/terminal-psi/verification/src/trusted_surface/reconstruction.rs",
            "omega-rust/psi/compilation/representations/terminal-psi/verification/src/trusted_surface/operations.rs",
            "omega-rust/psi/compilation/representations/terminal-psi/verification/src/trusted_surface/procedural.rs",
        ]
    );
}
