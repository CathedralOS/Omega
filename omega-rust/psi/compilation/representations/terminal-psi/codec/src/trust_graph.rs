//! Exact migration trust graph for terminal-Psi artifact verification.
//!
//! This is deliberately an honest description of the current deployment
//! boundary, not the final canonical-ledger implementation. Rust decoding,
//! semantic reconstruction, the current ledger framework, every unproved leaf
//! schema, and every unproved call-composition
//! row remain explicit trusted judgments until low-rung derivations replace
//! them.

mod current;
mod identity;
mod validation;

use std::sync::OnceLock;

use current::build_current_terminal_trust_graph;
use identity::{dependency_digest, graph_identity};
use terminal_semantics::operation_semantic_row;
pub use validation::validate_terminal_trust_graph;

const CURRENT_ENTRY: &str = "closure:terminal-pcc-current";
const MIGRATION_POLICY_DESCRIPTOR: &[u8] = b"PCC-CANONICAL-SEMANTIC-LEDGER-v1\0canonical bytes -> exhaustive local ledger -> unchanged canonical goals\0Rust decoder/verifier remain explicit trusted judgments until low-rung derivations replace them\0unknown and cyclic leaves reject\0portable terminal semantics only";
static CURRENT_TRUST_GRAPH: OnceLock<Result<ValidatedTerminalTrustGraph, TrustGraphError>> =
    OnceLock::new();

const CODEC_SOURCE: &[u8] = include_bytes!("lib.rs");
const SEMANTIC_MODULE_CODEC_SOURCE: &[u8] = include_bytes!("sections/semantic_module.rs");
// build.rs folds every Rust source under src/ into this closure, so the
// canonical-bytes and decoder commitments change when any codec production
// source changes; no file can join the codec surface unobserved. The manifest
// is bound too, since it is the manifest's [lib]/target paths that keep the
// compiled surface inside src/. The named sources below stay bound as the
// load-bearing entry points.
const CODEC_SOURCE_CLOSURE: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/psi-terminal-codec-source-closure.bin"
));
const CODEC_MANIFEST_SOURCE: &[u8] = include_bytes!("../Cargo.toml");
const MACHINE_WIRE_SOURCE: &[u8] = include_bytes!("sections/semantic_module/machine_wire.rs");
const BLOCK_WIRE_SOURCE: &[u8] = include_bytes!("sections/semantic_module/block_wire.rs");
const REACH_APPLICATION_WIRE_SOURCE: &[u8] =
    include_bytes!("sections/semantic_module/reach_application_wire.rs");
const STRUCTURAL_SIGNATURE_WIRE_SOURCE: &[u8] =
    include_bytes!("sections/semantic_module/structural_signature_wire.rs");
const DECLARATION_FOUNDATIONS_SOURCE: &[u8] = include_bytes!(
    "sections/semantic_module/module_foundation_validation/declaration_foundations.rs"
);
const PROOF_CODEC_VALIDATION_SOURCE: &[u8] = include_bytes!("sections/proof_bundle/validation.rs");
const PROOF_ADMISSION_RECURSION_SOURCE: &[u8] =
    include_bytes!("../../../../proof/admission/src/admission/recursion.rs");
const VERIFIER_LIB_SOURCE: &[u8] = include_bytes!("../../verification/src/lib.rs");
const VERIFIER_VALIDATION_SOURCE: &[u8] = include_bytes!("../../verification/src/validation.rs");
const VERIFIER_SOURCE: &[u8] = include_bytes!("../../verification/src/verification.rs");
const SOURCE_CLOSURE_BUILD_SOURCE: &[u8] = include_bytes!("../build.rs");
const VERIFIER_SOURCE_CLOSURE: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/psi-terminal-verifier-source-closure.bin"
));
const EVIDENCE_PROVENANCE_SOURCE: &[u8] =
    include_bytes!("../../verification/src/verification/evidence_provenance.rs");
const VERIFIER_CALL_COMPOSITION_SOURCE: &[u8] =
    include_bytes!("../../verification/src/verification/call_composition.rs");
const PROOF_BUNDLE_SOURCE: &[u8] =
    include_bytes!("../../verification/src/verification/proof_bundle.rs");
const RECONSTRUCTION_SOURCE: &[u8] =
    include_bytes!("../../verification/src/verification/reconstruction.rs");
const PRIMITIVE_SNAPSHOT_RECONSTRUCTION_SOURCE: &[u8] =
    include_bytes!("../../verification/src/verification/reconstruction/primitive_snapshots.rs");
const PATH_FACTS_SOURCE: &[u8] =
    include_bytes!("../../verification/src/verification/reconstruction/path_facts.rs");
const DISCRETE_PATH_FACTS_SOURCE: &[u8] =
    include_bytes!("../../verification/src/verification/reconstruction/path_facts/discrete.rs");
const SUBSTITUTION_SOURCE: &[u8] =
    include_bytes!("../../semantics/src/substitution.rs");
const CRASH_OBLIGATION_RECONSTRUCTION_SOURCE: &[u8] =
    include_bytes!("../../verification/src/verification/reconstruction/crash_obligations.rs");
const PROOF_ADMISSION_LIB_SOURCE: &[u8] = include_bytes!("../../../../proof/admission/src/lib.rs");
const PROOF_ADMISSION_EVIDENCE_SOURCE: &[u8] =
    include_bytes!("../../../../proof/admission/src/admission/evidence.rs");
const PROOF_ADMISSION_JUDGMENT_SOURCE: &[u8] =
    include_bytes!("../../../../proof/admission/src/kernel.rs");
const PROOF_ADMISSION_CLOSED_INTEGER_SOURCE: &[u8] =
    include_bytes!("../../../../proof/admission/src/integer_rules/closed_integer.rs");
const PROOF_ADMISSION_PROOF_SOURCE: &[u8] =
    include_bytes!("../../../../proof/admission/src/proof.rs");
const PROOF_ADMISSION_CERTIFICATE_SEARCH_SOURCE: &[u8] =
    include_bytes!("../../../../proof/admission/src/certificate_search.rs");
const PROOF_ADMISSION_DENOTATION_CHECKER_SOURCE: &[u8] =
    include_bytes!("../../../../proof/admission/src/admission/denotation.rs");
const PROOF_ADMISSION_ORDER_DISCRETENESS_SOURCE: &[u8] =
    include_bytes!("../../../../proof/admission/src/proof/order_discreteness.rs");
const PROOF_ADMISSION_STRICT_ORDER_SOURCE: &[u8] =
    include_bytes!("../../../../proof/admission/src/proof/strict_order_transitivity.rs");
const PROOF_ADMISSION_TRAVERSAL_SOURCE: &[u8] =
    include_bytes!("../../../../proof/admission/src/proof/traversal.rs");
const PROOF_ADMISSION_INTEGER_AFFINE_SOURCE: &[u8] =
    include_bytes!("../../../../proof/admission/src/integer_rules/integer_affine.rs");
const PROOF_ADMISSION_INTEGER_CAST_SOURCE: &[u8] =
    include_bytes!("../../../../proof/admission/src/integer_rules/integer_cast.rs");
const PROOF_ADMISSION_INTEGER_FORBIDDEN_ROOT_SOURCE: &[u8] =
    include_bytes!("../../../../proof/admission/src/integer_rules/integer_forbidden_root.rs");
const PROOF_CODEC_SOURCE: &[u8] = include_bytes!("sections/proof_bundle.rs");
const OBLIGATION_LEDGER_CODEC_SOURCE: &[u8] = include_bytes!("sections/obligation_ledger.rs");
const PROPOSITION_SOURCE: &[u8] =
    include_bytes!("../../../../../foundation/semantic-vocabulary/src/proposition.rs");
const PROPOSITION_VALUE_IDS_SOURCE: &[u8] =
    include_bytes!("../../../../../foundation/semantic-vocabulary/src/proposition/value_ids.rs");
const BYTE_PREDICATE_VOCABULARY_SOURCE: &[u8] =
    include_bytes!("../../../../../foundation/semantic-vocabulary/src/byte_predicates.rs");
const SCALAR_TERM_VOCABULARY_SOURCE: &[u8] =
    include_bytes!("../../../../../foundation/semantic-vocabulary/src/proposition/scalar_terms.rs");
const PROPOSITION_CONTEXT_SOURCE: &[u8] =
    include_bytes!("../../../../../foundation/semantic-vocabulary/src/proposition/propositions.rs");
const TERMINAL_BYTE_PREDICATE_SOURCE: &[u8] =
    include_bytes!("../../semantics/src/structural_effect/byte_predicate.rs");
const TERMINAL_PRIMITIVE_PLACE_SOURCE: &[u8] =
    include_bytes!("../../semantics/src/primitive_place.rs");
const BYTE_PREDICATE_VALIDATION_SOURCE: &[u8] =
    include_bytes!("../../verification/src/validation/byte_sequence/predicate.rs");
const TERMINAL_REPRESENTATION_SOURCE_CLOSURE: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/psi-terminal-representation-source-closure.bin"
));
// Descriptor provenance and loan replay live in shared semantics. Bind that
// entire implementation alongside the verifier so extracting another helper
// cannot silently leave it outside the artifact's acceptance identity.
const TERMINAL_SEMANTICS_SOURCE_CLOSURE: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/psi-terminal-semantics-source-closure.bin"
));
const TERMINAL_SEMANTICS_SOURCE: &[u8] = include_bytes!("../../semantics/src/lib.rs");
const BOOLEAN_POLARITY_RECONSTRUCTION_SOURCE: &[u8] = include_bytes!(
    "../../verification/src/verification/reconstruction/operation_facts/boolean_polarity.rs"
);
const PREDICATE_DENOTATION_SOURCE: &[u8] =
    include_bytes!("../../../../proof/admission/src/predicate_denotation.rs");
const PREDICATE_DENOTATION_BUDGET_SOURCE: &[u8] =
    include_bytes!("../../../../proof/admission/src/predicate_denotation/budget.rs");
const PREDICATE_VALUE_EQUALITIES_SOURCE: &[u8] =
    include_bytes!("../../../../proof/admission/src/predicate_denotation/value_equalities.rs");
const TERMINAL_PROOF_BEARING_SCALAR_SOURCE: &[u8] =
    include_bytes!("../../semantics/src/proof_bearing_scalar.rs");
const TERMINAL_CANONICAL_SCALAR_GOAL_SOURCE: &[u8] =
    include_bytes!("../../semantics/src/proof_bearing_scalar/canonical_goal.rs");
const TERMINAL_CALL_COMPOSITION_SOURCE: &[u8] =
    include_bytes!("../../semantics/src/call_composition.rs");
const TERMINAL_FIXED_BYTE_VIEW_SOURCE: &[u8] =
    include_bytes!("../../semantics/src/call_composition/fixed_byte_view.rs");
const TERMINAL_STRUCTURAL_EFFECT_SOURCE: &[u8] =
    include_bytes!("../../semantics/src/structural_effect.rs");
const TERMINAL_SCALAR_ARRAY_SOURCE: &[u8] = include_bytes!("../../semantics/src/scalar_array.rs");
const TERMINAL_RECORD_FIELD_SOURCE: &[u8] = include_bytes!("../../semantics/src/record_field.rs");
const TERMINAL_BYTE_EXTENT_SOURCE: &[u8] =
    include_bytes!("../../semantics/src/structural_effect/byte_extent.rs");
const PROOF_ADMISSION_SUBTRACT_ORDER_SOURCE: &[u8] =
    include_bytes!("../../../../proof/admission/src/proof/subtract_order.rs");
const BYTE_EXTENT_RECONSTRUCTION_SOURCE: &[u8] = include_bytes!(
    "../../verification/src/verification/reconstruction/operation_facts/byte_extent.rs"
);
const BYTE_SUBSLICE_VALIDATION_SOURCE: &[u8] =
    include_bytes!("../../verification/src/validation/byte_sequence/subslice.rs");
const BYTE_VIEW_DOMINANCE_SOURCE: &[u8] =
    include_bytes!("../../verification/src/validation/control_flow.rs");
const CONTROL_GRAPH_SOURCE: &[u8] = include_bytes!("../../semantics/src/control_graph.rs");
const BYTE_CYCLIC_ELIGIBILITY_SOURCE: &[u8] =
    include_bytes!("../../verification/src/validation/control_flow/unranked_cycles.rs");
const BYTE_BLOCK_BINDINGS_SOURCE: &[u8] =
    include_bytes!("../../verification/src/validation/block_views.rs");
const BYTE_VIEW_FRONTIER_SOURCE: &[u8] =
    include_bytes!("../../verification/src/validation/frontier.rs");
const BYTE_VIEW_FRONTIER_TRAVERSAL_SOURCE: &[u8] =
    include_bytes!("../../verification/src/validation/frontier/traversal.rs");
const BYTE_VIEW_ARGUMENTS_SOURCE: &[u8] =
    include_bytes!("../../verification/src/validation/structural/operations.rs");
const BYTE_WRITE_VALIDATION_SOURCE: &[u8] =
    include_bytes!("../../verification/src/validation/byte_sequence/write.rs");
const BYTE_READ_VALIDATION_SOURCE: &[u8] =
    include_bytes!("../../verification/src/validation/byte_sequence/read.rs");
const BYTE_VIEW_VALIDATION_SOURCE: &[u8] =
    include_bytes!("../../verification/src/validation/byte_sequence/length.rs");
const BYTE_OWNER_STORE_VALIDATION_SOURCE: &[u8] =
    include_bytes!("../../verification/src/validation/byte_sequence/store.rs");
const BYTE_PREDICATE_SEMANTICS_SOURCE: &[u8] =
    include_bytes!("../../semantics/src/byte_sequence_predicates.rs");
const BYTE_OWNER_LOAN_SEMANTICS_SOURCE: &[u8] =
    include_bytes!("../../semantics/src/call_composition/bounded_byte_owner.rs");
const BYTE_FIELD_STORE_VALIDATION_SOURCE: &[u8] =
    include_bytes!("../../verification/src/validation/structural/byte_sequence_store.rs");
const BYTE_VIEW_FIELD_STORE_VALIDATION_SOURCE: &[u8] =
    include_bytes!("../../verification/src/validation/structural/byte_view_field_store.rs");
const BYTE_FIELD_ACCESS_VALIDATION_SOURCE: &[u8] =
    include_bytes!("../../verification/src/validation/structural/byte_sequence_fields.rs");
const BYTE_FIELD_FRESHNESS_SOURCE: &[u8] = include_bytes!(
    "../../verification/src/validation/structural/byte_sequence_fields/freshness.rs"
);
const LITERAL_FOUNDATION_SOURCE: &[u8] =
    include_bytes!("../../verification/src/validation/foundation.rs");
const OPERATION_FACTS_SOURCE: &[u8] =
    include_bytes!("../../verification/src/verification/reconstruction/operation_facts.rs");
const MACHINE_RECONSTRUCTION_CONTEXT_SOURCE: &[u8] =
    include_bytes!("../../verification/src/verification/reconstruction/machine_context.rs");

// A content observation's equation, canonical subject and readable/current
// operand are one denotation dependency. These semantic/vocabulary sources
// live outside the generated verifier/representation/codec source closures.
const BYTE_PREDICATE_DENOTATION_SOURCES: &[(&str, &[u8])] = &[
    (
        "terminal-semantics/structural_effect/byte_predicate.rs",
        TERMINAL_BYTE_PREDICATE_SOURCE,
    ),
    (
        "terminal-semantics/primitive_place.rs",
        TERMINAL_PRIMITIVE_PLACE_SOURCE,
    ),
    (
        "semantic-vocabulary/byte_predicates.rs",
        BYTE_PREDICATE_VOCABULARY_SOURCE,
    ),
    (
        "semantic-vocabulary/proposition/scalar_terms.rs",
        SCALAR_TERM_VOCABULARY_SOURCE,
    ),
    (
        "semantic-vocabulary/proposition/propositions.rs",
        PROPOSITION_CONTEXT_SOURCE,
    ),
    (
        "terminal-semantics/byte_sequence_predicates.rs",
        BYTE_PREDICATE_SEMANTICS_SOURCE,
    ),
    (
        "terminal-verifier/validation/byte_sequence/predicate.rs",
        BYTE_PREDICATE_VALIDATION_SOURCE,
    ),
    (
        "terminal-verifier/validation/byte_sequence/length.rs",
        BYTE_VIEW_VALIDATION_SOURCE,
    ),
    (
        "terminal-verifier/validation/byte_sequence/read.rs",
        BYTE_READ_VALIDATION_SOURCE,
    ),
    (
        "terminal-verifier/validation/structural/byte_sequence_fields.rs",
        BYTE_FIELD_ACCESS_VALIDATION_SOURCE,
    ),
    (
        "terminal-verifier/validation/structural/byte_sequence_fields/freshness.rs",
        BYTE_FIELD_FRESHNESS_SOURCE,
    ),
    (
        "terminal-verifier/verification/reconstruction/operation_facts.rs",
        OPERATION_FACTS_SOURCE,
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TrustDependencyKind {
    RegisteredRoot,
    AcceptanceClosure,
    TrustedImplementation,
    SufficientFormReduction,
    LedgerFramework,
    DenotationSchema,
    StructuralEffectSchema,
    CallComposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TrustDependencyStatus {
    Registered,
    TrustedJudgment,
    LocallyDerivedPendingComposition,
    FullyDerived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TrustAcceptingPolicy {
    RegisteredSemanticFoundation,
    ExplicitMigrationTrust,
    KernelCheckedDerivation,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TrustDependencyDigest([u8; 32]);

impl TrustDependencyDigest {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for TrustDependencyDigest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, formatter)
    }
}

impl std::fmt::Display for TrustDependencyDigest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustDependencyNode {
    identity: String,
    kind: TrustDependencyKind,
    status: TrustDependencyStatus,
    semantic_subject: String,
    digest: TrustDependencyDigest,
    version: String,
    owner: String,
    scope: String,
    rationale: String,
    accepting_policy: TrustAcceptingPolicy,
    dependencies: Vec<String>,
}

impl TrustDependencyNode {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        identity: impl Into<String>,
        kind: TrustDependencyKind,
        status: TrustDependencyStatus,
        semantic_subject: impl Into<String>,
        version: impl Into<String>,
        owner: impl Into<String>,
        scope: impl Into<String>,
        rationale: impl Into<String>,
        accepting_policy: TrustAcceptingPolicy,
        dependencies: Vec<String>,
        exact_sources: &[(&str, &[u8])],
    ) -> Self {
        let identity = identity.into();
        let semantic_subject = semantic_subject.into();
        let version = version.into();
        let owner = owner.into();
        let scope = scope.into();
        let rationale = rationale.into();
        let digest = dependency_digest(
            &identity,
            kind,
            status,
            &semantic_subject,
            &version,
            &owner,
            &scope,
            &rationale,
            accepting_policy,
            exact_sources,
        );
        Self {
            identity,
            kind,
            status,
            semantic_subject,
            digest,
            version,
            owner,
            scope,
            rationale,
            accepting_policy,
            dependencies,
        }
    }

    pub fn identity(&self) -> &str {
        &self.identity
    }

    pub const fn kind(&self) -> TrustDependencyKind {
        self.kind
    }

    pub const fn status(&self) -> TrustDependencyStatus {
        self.status
    }

    pub fn semantic_subject(&self) -> &str {
        &self.semantic_subject
    }

    pub const fn digest(&self) -> TrustDependencyDigest {
        self.digest
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }

    pub fn scope(&self) -> &str {
        &self.scope
    }

    pub fn rationale(&self) -> &str {
        &self.rationale
    }

    pub const fn accepting_policy(&self) -> TrustAcceptingPolicy {
        self.accepting_policy
    }

    pub fn dependencies(&self) -> &[String] {
        &self.dependencies
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalTrustGraphIdentity([u8; 32]);

impl TerminalTrustGraphIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for TerminalTrustGraphIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, formatter)
    }
}

impl std::fmt::Display for TerminalTrustGraphIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTerminalTrustGraph {
    entry: String,
    nodes: Vec<TrustDependencyNode>,
    identity: TerminalTrustGraphIdentity,
    fully_derived: bool,
}

impl ValidatedTerminalTrustGraph {
    pub fn entry(&self) -> &str {
        &self.entry
    }

    pub fn nodes(&self) -> &[TrustDependencyNode] {
        &self.nodes
    }

    pub const fn identity(&self) -> TerminalTrustGraphIdentity {
        self.identity
    }

    pub const fn is_fully_derived(&self) -> bool {
        self.fully_derived
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustGraphError {
    EmptyEntry,
    EmptyField { node: String, field: &'static str },
    NonCanonicalNodeOrder,
    NonCanonicalDependencyOrder { node: String },
    DuplicateNode(String),
    UnknownEntry(String),
    UnknownDependency { node: String, dependency: String },
    SelfDependency(String),
    DependencyCycle(String),
    UnreachableNode(String),
    RootHasDependencies(String),
    RootHasInvalidStatus(String),
    RootHasInvalidPolicy(String),
    NonRootHasNoDependencies(String),
    NonRootHasRegisteredStatus(String),
}

impl std::fmt::Display for TrustGraphError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TrustGraphError {}

/// Exact current migration closure. Source-backed nodes digest the Rust bytes
/// that presently decide the judgment. A code change therefore changes the
/// graph identity even when its explicit semantic version was not bumped.
pub fn current_terminal_trust_graph() -> Result<ValidatedTerminalTrustGraph, TrustGraphError> {
    CURRENT_TRUST_GRAPH
        .get_or_init(build_current_terminal_trust_graph)
        .clone()
}

/// Exact current trusted semantic row selected by one closed operation
/// variant. Leaf operations resolve to declarative-schema custody; calls
/// resolve to the separate coverage/substitution/composition algebra. This
/// exhaustive match is intentionally separate from execution: adding an
/// `OperationKind` cannot compile until its migration trust row is named too.
pub fn current_rust_operation_semantics_trust_identity(
    operation: &terminal_psi::OperationKind,
) -> &'static str {
    operation_semantic_row(operation)
        .expect("the closed terminal operation table has one exact semantic row")
        .identity()
}

pub fn render_terminal_trust_graph(
    graph: &ValidatedTerminalTrustGraph,
) -> Result<String, std::fmt::Error> {
    use std::fmt::Write;

    let mut output = String::new();
    writeln!(
        &mut output,
        "trust-graph {} entry {} fully-derived {}",
        graph.identity, graph.entry, graph.fully_derived
    )?;
    for node in &graph.nodes {
        writeln!(
            &mut output,
            "trust-node {} kind {:?} status {:?}",
            node.identity, node.kind, node.status
        )?;
        writeln!(
            &mut output,
            "  subject {:?} version {:?} digest {}",
            node.semantic_subject, node.version, node.digest
        )?;
        writeln!(
            &mut output,
            "  owner {:?} scope {:?} policy {:?}",
            node.owner, node.scope, node.accepting_policy
        )?;
        writeln!(&mut output, "  rationale {:?}", node.rationale)?;
        for dependency in &node.dependencies {
            writeln!(&mut output, "  depends {dependency}")?;
        }
    }
    Ok(output)
}

fn write_hex(formatter: &mut std::fmt::Formatter<'_>, bytes: &[u8; 32]) -> std::fmt::Result {
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    Ok(())
}
