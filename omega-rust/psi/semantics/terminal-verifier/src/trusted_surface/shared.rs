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
        concat!("omega-rust/psi/semantics/proof-admission/src/", $file)
    };
}
macro_rules! ts {
    ($file:literal) => {
        concat!("omega-rust/psi/semantics/terminal-semantics/src/", $file)
    };
}
macro_rules! tv {
    ($file:literal) => {
        concat!("omega-rust/psi/semantics/terminal-verifier/src/", $file)
    };
}
macro_rules! tp {
    ($file:literal) => {
        concat!("omega-rust/psi/representations/terminal-psi/src/", $file)
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
        "a proposition whose Boolean and same-carrier closed fixed-integer literal denotation is computed under a bounded recursion, substitution, and clone budget over the validated context",
        "the denotation normal form used by PredicateDenotation, including exact signed full-width literal comparisons; open arithmetic and IEEE relations remain unchanged, and unbounded or malformed propositions reject before evaluation",
        &["formation:proposition-context", "formation:mathematical-core", "primitive:closed-integer-relation"],
        &[pa!( "predicate_denotation.rs"), pa!( "predicate_denotation/budget.rs"), pa!( "kernel.rs")]
    );
    NORM_VALUE_EQUALITY_DENOTATION => (
        "conversion:value-equality-denotation",
        NormalizationConversion,
        "a proposition plus an ordered roster of proved value-to-term equations",
        "the denotation normal form under exactly those equations, preserving constructor tags, types, and operand order",
        &["conversion:predicate-denotation"],
        &[pa!( "predicate_denotation.rs"), pa!( "predicate_denotation/value_equalities.rs")]
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
        "an ordered affine witness of exact-add definition steps and literal landings over one carrier",
        "the mapped bound from root to target when every step checks; the witness is bounded and validated before interpretation",
        &["normalization:closed-integer-evaluator"],
        &[pa!( "integer_rules/integer_affine.rs"), pa!( "integer_rules/integer_affine/witness_checking.rs"), pa!( "integer_rules/integer_affine/bound_mapping.rs"), pa!( "integer_rules/integer_affine/truth_bounds.rs"), tp!( "artifacts/proof_bundle/witnesses.rs")]
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
        "block-invariant obligations are stated per arriving edge, never merged across edges",
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
        "the exact root/path store identities reaching the value, stopping at definitions and preserving only statically disjoint sibling writes; queries never evaluate source expressions or consult the requested contract",
        &["scope:dominance-order"],
        &[tv!( "verification/reconstruction/primitive_snapshots.rs")]
    );
    SCOPE_FIELD_SNAPSHOTS => (
        "scope:field-value-snapshots",
        PremiseScope,
        "live unconditional typed canonical field-to-SSA equalities before store or call invalidation, outside private crash reconstruction",
        "affected scalar facts are re-expressed through one saved value per field without splitting connectives or growing the fact roster; remaining affected observations still expire",
        &["scope:dominance-order", "fact:structural-effect-observation"],
        &[tv!( "verification/field_snapshots.rs"), tv!( "verification/reconstruction/operation_facts.rs"), tv!( "verification/call_composition.rs")]
    );

    // -- Write invalidation --
    INV_ESTABLISH_LOCAL => (
        "invalidation:establish-primitive-local",
        WriteInvalidation,
        "an EstablishPrimitiveLocal producing a structural result place",
        "every proposition still observing the result place after exact scalar capture is removed from the axiom set",
        &["fact:structural-effect-observation", "scope:field-value-snapshots"],
        &[tv!( "verification/reconstruction/operation_facts.rs"), tv!( "validation/propositions.rs")]
    );
    INV_PRIMITIVE_STORE => (
        "invalidation:write-only-primitive-store",
        WriteInvalidation,
        "a WriteOnlyPrimitiveStore to a destination place",
        "every proposition still observing the destination after exact scalar capture is removed from the axiom set",
        &["fact:structural-effect-observation", "scope:field-value-snapshots"],
        &[tv!( "verification/reconstruction/operation_facts.rs"), tv!( "validation/propositions.rs")]
    );
    INV_INDEXED_PRIMITIVE_STORE => (
        "invalidation:write-only-indexed-primitive-store",
        WriteInvalidation,
        "a WriteOnlyIndexedPrimitiveStore to a destination place",
        "every proposition still observing the destination root after exact scalar capture is removed from the axiom set; the runtime index cannot name the written leaf",
        &["fact:structural-effect-observation", "scope:field-value-snapshots"],
        &[tv!( "verification/reconstruction/operation_facts.rs"), tv!( "validation/propositions.rs")]
    );
    INV_FIELD_STORE => (
        "invalidation:structural-field-store",
        WriteInvalidation,
        "a structural field store whose canonical write path may resolve",
        "propositions still observing the exact write after exact scalar capture are removed; an unresolvable path forgets the entire root",
        &["scope:place-substitution", "scope:field-value-snapshots"],
        &[tv!( "verification/reconstruction/operation_facts.rs"), tv!( "validation/structural_operations/structural_paths.rs")]
    );
    INV_BYTE_WRITE => (
        "invalidation:byte-sequence-write",
        WriteInvalidation,
        "a byte-sequence write over a destination extent",
        "propositions observing the destination extent end their validity at the write",
        &["fact:byte-extent-length"],
        &[tv!( "verification/reconstruction/operation_facts.rs"), tv!( "validation/byte_sequence_write.rs")]
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
    COMP_CALL_INSTANTIATION => (
        "composition:call-instantiation",
        CallComposition,
        "a call operation's callee signature, argument substitution, and the callee's verified contract",
        "parameter equalities, instantiated requires obligations, and imported ensures propositions composed into the caller frame",
        &["scope:call-frame-rewrite", "formation:contract-validation", "scope:field-value-snapshots"],
        &[tv!( "verification/call_composition.rs"), ts!( "call_composition.rs")]
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
        &[tv!( "validation/suspension_call_plan.rs"), ts!( "call_composition/fixed_byte_view.rs")]
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
    FORM_MODULE_STRUCTURE => (
        "formation:module-structure",
        SharedFormation,
        "the terminal module's canonical representation: machines, blocks, structural types, and declared surfaces, including leaf-typed structural fields resolved through their canonical leaf shape",
        "structural validation accepting exactly well-formed modules before any reconstruction; malformed representations reject with a ModuleError",
        &["root:verification-contract"],
        &[tv!( "validation.rs"), tv!( "validation/error.rs"), tv!( "validation/foundation.rs"), tv!( "validation/foundation/boundary_machines.rs"), tv!( "validation/foundation/domains.rs"), tv!( "validation/foundation/machine_foundations.rs"), tv!( "validation/foundation/provider_candidates.rs"), tv!( "validation/foundation/provider_result.rs"), tv!( "validation/foundation/services.rs"), tv!( "validation/foundation/structural_types.rs"), tv!( "lib.rs"), pa!( "lib.rs")]
    );
    FORM_MACHINE => (
        "formation:machine-validation",
        SharedFormation,
        "one machine's signature, blocks, parameters (including shared-borrow record views joined at block parameters), contract, and declared invariants",
        "the machine validates as a whole before its obligations are reconstructed",
        &["formation:module-structure"],
        &[tv!( "validation/machine.rs"), tv!( "validation/machine/blocks.rs"), tv!( "validation/machine/contract_clauses.rs"), tv!( "validation/machine/custody_operations.rs"), tv!( "validation/machine/scalar_result_operations.rs"), tv!( "validation/machine/structural_places.rs"), tv!( "validation/block_views.rs")]
    );
    FORM_PROPOSITION_CONTEXT => (
        "formation:proposition-context",
        SharedFormation,
        "a proposition and the value/place context it is checked under",
        "only well-formed propositions over known values, places, and types are judged; malformed propositions reject before any rule runs",
        &["formation:module-structure"],
        &[tv!( "validation/propositions.rs"), sv!( "proposition.rs")]
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
        "validated control flow: successors, dominance, and cycle declarations agree before reconstruction relies on them",
        &["formation:module-structure"],
        &[tv!( "validation/control_flow.rs"), tv!( "validation/control_flow/block_checks.rs"), tv!( "validation/control_flow/block_graph.rs"), tv!( "validation/control_flow/definitions.rs"), tv!( "validation/control_flow/unranked_cycles.rs"), tv!( "control_graph.rs")]
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
        &[tv!( "validation/scalar_qualifications.rs")]
    );
    FORM_STRUCTURAL_ROSTERS => (
        "formation:structural-qualification-rosters",
        SharedFormation,
        "the declared structural qualification rosters",
        "rosters validate before reconstruction consults them for field and case facts",
        &["formation:machine-validation"],
        &[tv!( "validation/structural_qualification_rosters.rs")]
    );
    FORM_STRUCTURAL_RESULT_CONTRACTS => (
        "formation:structural-result-contracts",
        SharedFormation,
        "the declared structural result contract clauses",
        "structural result contracts validate before call composition imports them",
        &["formation:contract-validation"],
        &[tv!( "validation/structural_result_contracts.rs")]
    );
    FORM_CONTENT => (
        "formation:content-conservation",
        SharedFormation,
        "the module's content-identity claims, partitions, and reshuffles",
        "content conservation validates as an algebra over declared claims before reconstruction infers its propositions",
        &["formation:module-structure"],
        &[tv!( "validation/content.rs"), sv!( "content.rs")]
    );
    FORM_AFFINE_CLEANUP => (
        "formation:affine-cleanup",
        SharedFormation,
        "the machine's affine cleanup actions and continuations, including Jump edges whose owned arguments move one projected affine child while the residual complement dies on the edge and member `drop<T>` specializations whose nominal return carries the same exact cleanup shape",
        "cleanup plans validate before terminators reconstruct their obligations",
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
        "the machine's claim frontier and the referent roots pinned by shared-borrow join parameters at each block and traversal point",
        "the reconstructed frontier used by crash guards and cleanup checks is the validated one",
        &["scope:dominance-order"],
        &[tv!( "validation/frontier.rs"), tv!( "validation/frontier/block_entry.rs"), tv!( "validation/frontier/block_parameters.rs"), tv!( "validation/frontier/operations.rs"), tv!( "validation/frontier/terminators.rs"), tv!( "validation/frontier/traversal.rs")]
    );
    FORM_CRASH => (
        "formation:crash-validation",
        SharedFormation,
        "a crash site's declared guard terms and outcome, plus each operation's declared crash contracts checked against its owning machine's crash-route coverage",
        "crash sites and operation-level crash contracts validate before their private fact bundles are reconstructed",
        &["formation:machine-validation", "formation:frontier"],
        &[tv!( "validation/crash.rs"), tv!( "validation/crash/operation_contracts.rs"), tv!( "validation/crash/outcome.rs"), tv!( "validation/crash/site_truth.rs")]
    );
    FORM_CRASH_REQUIREMENTS => (
        "formation:crash-entry-requirements",
        SharedFormation,
        "a machine's declared entry requirements: integer-order and order-chain requirements",
        "entry requirements validate independently so a ranked machine's crash guards can be proved without all-path invariants; each found requirement certificate is accepted by the certificate checker, whose covered decisions the mathematical-core kernel re-decides",
        &["formation:crash-validation", "formation:mathematical-core"],
        &[tv!( "validation/crash/entry_requirements.rs"), tv!( "validation/crash/entry_requirements/integer_order.rs"), tv!( "validation/crash/entry_requirements/order_chain.rs")]
    );
    FORM_BLOCK_INVARIANTS => (
        "formation:scalar-block-invariants",
        SharedFormation,
        "a header block's declared scalar invariants over the destination telescope, immutable invocation formals, and storage observations rooted at places alive for the whole invocation",
        "invariants validate as well-formed propositions before per-edge obligations are reconstructed",
        &["formation:proposition-context", "scope:header-edge-arrival"],
        &[tv!( "validation/scalar_block_invariants.rs")]
    );
    FORM_EVIDENCE => (
        "formation:evidence-orchestration",
        SharedFormation,
        "the module's evidence bundle: envelopes, provenance, and producer identity",
        "evidence validates for provenance and coverage: every obligation has evidence, no duplicate, unknown, noncanonical, or unused evidence is accepted",
        &["formation:module-structure", "route:certificate-derived"],
        &[tv!( "verification.rs"), tv!( "verification/evidence_provenance.rs"), tv!( "verification/proof_bundle.rs"), tv!( "validation/evidence.rs"), tv!( "validation/evidence/contract_lanes.rs"), tv!( "validation/evidence/guarded_call_outputs.rs"), tv!( "validation/evidence/proof_output_calls.rs")]
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
    FORM_REFERENCES => (
        "formation:references",
        SharedFormation,
        "the machine's establish/release reference discipline",
        "reference operations validate against declared custody before observations are reconstructed",
        &["formation:operation-validation"],
        &[tv!( "validation/references.rs")]
    );
    FORM_PARTIAL_AFFINE => (
        "formation:partial-affine",
        SharedFormation,
        "the machine's partial and trivial affine locals and discards, plus projected affine roots reached through locals, call products, constructed records, and join block parameters",
        "partial affine structure validates before affine facts or obligations are reconstructed",
        &["formation:machine-validation"],
        &[tv!( "validation/partial_affine.rs")]
    );
    FORM_PRIMITIVE_STORAGE => (
        "formation:primitive-storage",
        SharedFormation,
        "the machine's primitive locals, stores, and reads",
        "primitive storage operations validate exact relevant-field/fixed-index paths, root access and initialization before facts are reconstructed",
        &["formation:operation-validation"],
        &[tv!( "validation/primitive_storage.rs"), ts!( "primitive_place.rs")]
    );
    FORM_SCALAR_ARRAY => (
        "formation:scalar-array",
        SharedFormation,
        "a scalar array's declared element type and complete initialization",
        "the array validates totally: reconstruction asserts no additional proof authority for it",
        &["formation:operation-validation"],
        &[tv!( "validation/scalar_array.rs"), ts!( "scalar_array.rs")]
    );
    FORM_SCALAR_CASE => (
        "formation:scalar-case",
        SharedFormation,
        "a scalar case's declared sum type and payload",
        "the case validates before its membership and payload facts are reconstructed",
        &["formation:operation-validation"],
        &[tv!( "validation/scalar_case.rs")]
    );
    FORM_CASE_MEMBERSHIP => (
        "formation:structural-case-membership",
        SharedFormation,
        "a structural case membership test with an exact root/path and declared sum case",
        "readable access, projection identity, dominating establishment and whole live ownership validate before a Boolean observation; no storage refinement is reconstructed",
        &["formation:operation-validation"],
        &[tv!( "validation/structural_case_membership.rs")]
    );
    FORM_STRUCTURAL_OPS => (
        "formation:structural-operations",
        SharedFormation,
        "structural, boundary, and effect operation custody: structural field stores and their canonical write paths, structural arguments and claim transfers, unit operations, payloadless and primitive structural calls, contract places, crash continuations, and boundary requirements",
        "these operations validate and resolve their paths and custody before invalidation, leaf equations, or call composition are reconstructed",
        &["formation:operation-validation"],
        &[tv!( "validation/structural_operations.rs"), tv!( "validation/structural_operations/boundary_requirements.rs"), tv!( "validation/structural_operations/claim_transfers.rs"), tv!( "validation/structural_operations/claim_transfers/argument_claims.rs"), tv!( "validation/structural_operations/claim_transfers/transfer_roster.rs"), tv!( "validation/structural_operations/contract_places.rs"), tv!( "validation/structural_operations/crash_continuations.rs"), tv!( "validation/structural_operations/payloadless_calls.rs"), tv!( "validation/structural_operations/primitive_calls.rs"), tv!( "validation/structural_operations/structural_arguments.rs"), tv!( "validation/structural_operations/structural_arguments/argument_checks.rs"), tv!( "validation/structural_operations/structural_arguments/argument_pairs.rs"), tv!( "validation/structural_operations/structural_paths.rs"), tv!( "validation/structural_operations/unit_operation.rs"), tv!( "validation/structural_operations/unit_operation/boundary_calls.rs"), tv!( "validation/structural_operations/unit_operation/local_establishments.rs"), tv!( "validation/structural_operations/unit_operation/structural_calls.rs"), tv!( "validation/structural_operations/unit_operation/unit_calls.rs")]
    );
    FORM_STRUCTURAL_SCALAR => (
        "formation:structural-scalar-fields",
        SharedFormation,
        "structural scalar field reads and bounded-integer field stores with their declared intervals",
        "field reads and stores validate and resolve their referents — a store's declared range obligation resolves against the declaration, independently of the stored value — before equations and bounds are reconstructed",
        &["formation:operation-validation"],
        &[tv!( "validation/structural_scalar_fields.rs")]
    );
    FORM_STRUCTURAL_BYTES => (
        "formation:structural-byte-sequence-fields",
        SharedFormation,
        "structural byte-sequence fields, their extents, and freshness discipline",
        "byte-sequence fields validate before extent equations and capacity obligations are reconstructed",
        &["formation:operation-validation"],
        &[tv!( "validation/structural_byte_sequence_fields.rs"), tv!( "validation/structural_byte_sequence_fields/freshness.rs"), tv!( "validation/structural_byte_sequence_store.rs")]
    );
    FORM_BYTE_OPS => (
        "formation:byte-sequence-operations",
        SharedFormation,
        "byte-sequence length, read, write, and subslice operations",
        "byte-sequence operations validate their extents and bounds before observations are reconstructed",
        &["formation:operation-validation"],
        &[tv!( "validation/byte_sequence_length.rs"), tv!( "validation/byte_sequence_read.rs"), tv!( "validation/byte_sequence_write.rs"), tv!( "validation/byte_sequence_subslice.rs")]
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
        "mathematical terms are well-typed, bounded, level-scope-checked, and canonically formed before any judgment or normalization evaluates them",
        &["formation:proposition-context"],
        &[pa!( "mathematical_core.rs"), pa!( "mathematical_core/term.rs"), pa!( "mathematical_core/typing.rs"), pa!( "mathematical_core/substitution.rs"), pa!( "mathematical_core/conversion.rs"), pa!( "mathematical_core/certificate.rs"), pa!( "mathematical_core/bounded_denotation.rs"), pa!( "mathematical_core/indexed.rs"), pa!( "mathematical_core/quotient.rs"), pa!( "mathematical_core/scheme_dsl.rs"), pa!( "mathematical_core/theorems.rs")]
    );
    FORM_REWRITE => (
        "formation:optimization-rewrite-validation",
        SharedFormation,
        "a producer's target-neutral rewrite claim over a verified module",
        "the rewrite is independently checked: surviving structure, operations, block parameters, edge arguments, and the proof question must be unchanged; the producer's liveness result is never trusted",
        &["formation:evidence-orchestration"],
        &[tv!( "optimization.rs"), tv!( "optimization/dead_scalar_elimination.rs"), tv!( "optimization/copy_propagation.rs"), tv!( "optimization/global_value_numbering.rs"), tv!( "optimization/sparse_conditional_constant_propagation.rs"), tv!( "optimization/control_flow_cleanup.rs"), tv!( "optimization/proof_check_elision.rs")]
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
            "omega-rust/psi/semantics/terminal-verifier/src/trusted_surface.rs",
            "omega-rust/psi/semantics/terminal-verifier/src/trusted_surface/sites.rs",
            "omega-rust/psi/semantics/terminal-verifier/src/trusted_surface/checker.rs",
            "omega-rust/psi/semantics/terminal-verifier/src/trusted_surface/reconstruction.rs",
            "omega-rust/psi/semantics/terminal-verifier/src/trusted_surface/operations.rs",
            "omega-rust/psi/semantics/terminal-verifier/src/trusted_surface/shared.rs",
        ]
    );
}
