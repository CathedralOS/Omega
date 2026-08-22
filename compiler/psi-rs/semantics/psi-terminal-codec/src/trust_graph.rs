//! Exact migration trust graph for terminal-Psi artifact verification.
//!
//! This is deliberately an honest description of the current deployment
//! boundary, not the final canonical-ledger implementation. Rust decoding,
//! semantic reconstruction, every sufficient-form reducer, the current ledger
//! framework, every unproved leaf schema, and every unproved call-composition
//! row remain explicit trusted judgments until low-rung derivations replace
//! them.

mod validation;

use std::sync::OnceLock;

#[cfg(test)]
use psi_terminal_semantics::OperationSemanticTag;
use psi_terminal_semantics::{
    CallCompositionSemanticRow, OperationSemanticCustody, OperationSemanticRow,
    ProofBearingScalarSemanticRow, StructuralEffectSemanticRow,
    exact_call_composition_semantic_row_in, exact_proof_bearing_scalar_semantic_row_in,
    exact_structural_effect_semantic_row_in, operation_semantic_row,
    validate_call_composition_semantic_rows, validate_proof_bearing_scalar_semantic_rows,
    validate_structural_effect_semantic_rows,
};
use sha2::{Digest, Sha256};
pub use validation::validate_terminal_trust_graph;

const NODE_DIGEST_DOMAIN: &[u8] = b"psi-terminal-trust-node\0";
const GRAPH_DIGEST_DOMAIN: &[u8] = b"psi-terminal-trust-graph\0";
const CURRENT_ENTRY: &str = "closure:terminal-pcc-current";
const MIGRATION_POLICY_DESCRIPTOR: &[u8] = b"PCC-CANONICAL-SEMANTIC-LEDGER-v1\0canonical bytes -> exhaustive local ledger -> unchanged canonical goals\0Rust decoder/verifier/reducers remain explicit trusted judgments until low-rung derivations replace them\0unknown and cyclic leaves reject\0portable terminal semantics only";
static CURRENT_TRUST_GRAPH: OnceLock<Result<ValidatedTerminalTrustGraph, TrustGraphError>> =
    OnceLock::new();

const CODEC_SOURCE: &[u8] = include_bytes!("lib.rs");
const VERIFIER_LIB_SOURCE: &[u8] = include_bytes!("../../psi-terminal-verifier/src/lib.rs");
const VERIFIER_VALIDATION_SOURCE: &[u8] =
    include_bytes!("../../psi-terminal-verifier/src/validation.rs");
const VERIFIER_SOURCE: &[u8] = include_bytes!("../../psi-terminal-verifier/src/verification.rs");
const EVIDENCE_PROVENANCE_SOURCE: &[u8] =
    include_bytes!("../../psi-terminal-verifier/src/verification/evidence_provenance.rs");
const VERIFIER_CALL_COMPOSITION_SOURCE: &[u8] =
    include_bytes!("../../psi-terminal-verifier/src/verification/call_composition.rs");
const INTEGER_FOUNDATION_SOURCE: &[u8] =
    include_bytes!("../../psi-terminal-verifier/src/verification/integer_foundation.rs");
const PROOF_BUNDLE_SOURCE: &[u8] =
    include_bytes!("../../psi-terminal-verifier/src/verification/proof_bundle.rs");
const RECONSTRUCTION_SOURCE: &[u8] =
    include_bytes!("../../psi-terminal-verifier/src/verification/reconstruction.rs");
const SUFFICIENT_REDUCTION_SOURCE: &[u8] =
    include_bytes!("../../psi-terminal-verifier/src/verification/sufficient_reduction.rs");
const SUBSTITUTION_SOURCE: &[u8] =
    include_bytes!("../../psi-terminal-verifier/src/verification/substitution.rs");
const AFFINE_JOINS_SOURCE: &[u8] =
    include_bytes!("../../psi-terminal-verifier/src/verification/affine_joins.rs");
const INTEGER_ADD_SUBTRACT_SOURCE: &[u8] =
    include_bytes!("../../psi-terminal-verifier/src/verification/integer_add_subtract.rs");
const INTEGER_AFFINE_SOURCE: &[u8] =
    include_bytes!("../../psi-terminal-verifier/src/verification/integer_affine.rs");
const INTEGER_CONVERSION_SOURCE: &[u8] =
    include_bytes!("../../psi-terminal-verifier/src/verification/integer_conversion.rs");
const INTEGER_CONVERSION_CHAINS_SOURCE: &[u8] =
    include_bytes!("../../psi-terminal-verifier/src/verification/integer_conversion/chains.rs");
const INTEGER_CONVERSION_COMPOSITION_SOURCE: &[u8] = include_bytes!(
    "../../psi-terminal-verifier/src/verification/integer_conversion/composition.rs"
);
const INTEGER_DIVIDE_REMAINDER_SOURCE: &[u8] =
    include_bytes!("../../psi-terminal-verifier/src/verification/integer_divide_remainder.rs");
const INTEGER_MULTIPLY_SOURCE: &[u8] =
    include_bytes!("../../psi-terminal-verifier/src/verification/integer_multiply.rs");
const INTEGER_SHIFT_SOURCE: &[u8] =
    include_bytes!("../../psi-terminal-verifier/src/verification/integer_shift.rs");
const INTEGER_SHIFT_CHAINS_SOURCE: &[u8] =
    include_bytes!("../../psi-terminal-verifier/src/verification/integer_shift/chains.rs");
const INTEGER_SHIFT_COMPOSITION_SOURCE: &[u8] =
    include_bytes!("../../psi-terminal-verifier/src/verification/integer_shift/composition.rs");
const PROOF_KERNEL_LIB_SOURCE: &[u8] = include_bytes!("../../psi-proof-kernel/src/lib.rs");
const PROOF_KERNEL_EVIDENCE_SOURCE: &[u8] =
    include_bytes!("../../psi-proof-kernel/src/evidence.rs");
const PROOF_KERNEL_KERNEL_SOURCE: &[u8] = include_bytes!("../../psi-proof-kernel/src/kernel.rs");
const PROOF_KERNEL_PROOF_SOURCE: &[u8] = include_bytes!("../../psi-proof-kernel/src/proof.rs");
const PROOF_KERNEL_INTEGER_AFFINE_SOURCE: &[u8] =
    include_bytes!("../../psi-proof-kernel/src/integer_affine.rs");
const PROOF_KERNEL_INTEGER_CAST_SOURCE: &[u8] =
    include_bytes!("../../psi-proof-kernel/src/integer_cast.rs");
const PROOF_CODEC_SOURCE: &[u8] = include_bytes!("proof_bundle.rs");
const PROPOSITION_SOURCE: &[u8] = include_bytes!("../../../foundation/psi-core/src/proposition.rs");
const TERMINAL_MODEL_SOURCE: &[u8] =
    include_bytes!("../../../representations/psi-terminal/src/module.rs");
const TERMINAL_SEMANTICS_SOURCE: &[u8] = include_bytes!("../../psi-terminal-semantics/src/lib.rs");
const TERMINAL_PROOF_BEARING_SCALAR_SOURCE: &[u8] =
    include_bytes!("../../psi-terminal-semantics/src/proof_bearing_scalar.rs");
const TERMINAL_CALL_COMPOSITION_SOURCE: &[u8] =
    include_bytes!("../../psi-terminal-semantics/src/call_composition.rs");
const TERMINAL_STRUCTURAL_EFFECT_SOURCE: &[u8] =
    include_bytes!("../../psi-terminal-semantics/src/structural_effect.rs");

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

fn build_current_terminal_trust_graph() -> Result<ValidatedTerminalTrustGraph, TrustGraphError> {
    let mut nodes = Vec::new();
    nodes.extend(registered_roots());
    nodes.push(proof_kernel_node());
    nodes.push(decoder_node());
    nodes.push(verifier_node());
    nodes.push(ledger_framework_node());
    nodes.extend(reduction_nodes());
    nodes.extend(operation_semantics_nodes());
    nodes.push(current_closure_node());
    nodes.sort_by(|left, right| left.identity.cmp(&right.identity));
    validate_terminal_trust_graph(CURRENT_ENTRY, nodes)
}

/// Exact current trusted semantic row selected by one closed operation
/// variant. Leaf operations resolve to declarative-schema custody; calls
/// resolve to the separate coverage/substitution/composition algebra. This
/// exhaustive match is intentionally separate from execution: adding an
/// `OperationKind` cannot compile until its migration trust row is named too.
pub fn current_rust_operation_semantics_trust_identity(
    operation: &psi_terminal::OperationKind,
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

fn registered_roots() -> Vec<TrustDependencyNode> {
    vec![
        TrustDependencyNode::new(
            "root:abstract-terminal-execution-model",
            TrustDependencyKind::RegisteredRoot,
            TrustDependencyStatus::Registered,
            "terminal-Psi abstract operational semantics",
            "terminal-vocabulary-1",
            "Psi language architecture",
            "portable terminal-Psi execution before native refinement",
            "Portable PCC bottoms out in the abstract terminal execution model.",
            TrustAcceptingPolicy::RegisteredSemanticFoundation,
            Vec::new(),
            &[("psi-terminal/module.rs", TERMINAL_MODEL_SOURCE)],
        ),
        TrustDependencyNode::new(
            "root:canonical-proof-calculus-v15",
            TrustDependencyKind::RegisteredRoot,
            TrustDependencyStatus::Registered,
            "terminal-Psi proof bundle and primitive calculus",
            "proof-bundle-format-18",
            "Psi proof-kernel architecture",
            "portable terminal-Psi proof checking",
            "The current small proof calculus is an explicit registered semantic root.",
            TrustAcceptingPolicy::RegisteredSemanticFoundation,
            Vec::new(),
            &[
                ("psi-core/proposition.rs", PROPOSITION_SOURCE),
                ("psi-proof-kernel/evidence.rs", PROOF_KERNEL_EVIDENCE_SOURCE),
                ("psi-proof-kernel/kernel.rs", PROOF_KERNEL_KERNEL_SOURCE),
                (
                    "psi-proof-kernel/integer_affine.rs",
                    PROOF_KERNEL_INTEGER_AFFINE_SOURCE,
                ),
                (
                    "psi-proof-kernel/integer_cast.rs",
                    PROOF_KERNEL_INTEGER_CAST_SOURCE,
                ),
                ("psi-proof-kernel/lib.rs", PROOF_KERNEL_LIB_SOURCE),
                ("psi-proof-kernel/proof.rs", PROOF_KERNEL_PROOF_SOURCE),
                ("psi-terminal-codec/proof_bundle.rs", PROOF_CODEC_SOURCE),
            ],
        ),
        TrustDependencyNode::new(
            "root:canonical-terminal-bytes-v15",
            TrustDependencyKind::RegisteredRoot,
            TrustDependencyStatus::Registered,
            "canonical terminal-Psi semantic bytes",
            "PSITERM-format-16",
            "Psi terminal codec architecture",
            "canonical terminal-Psi byte vocabulary",
            "Artifact identity and authoritative reconstruction begin at exact canonical bytes.",
            TrustAcceptingPolicy::RegisteredSemanticFoundation,
            Vec::new(),
            &[("psi-terminal-codec/lib.rs", CODEC_SOURCE)],
        ),
        TrustDependencyNode::new(
            "root:explicit-rust-migration-policy",
            TrustDependencyKind::RegisteredRoot,
            TrustDependencyStatus::Registered,
            "temporary trust in current Rust semantic reconstruction",
            "PCC-CANONICAL-SEMANTIC-LEDGER-v1",
            "Omega/Psi architecture owners",
            "migration only; never native ISA or hardware refinement",
            "Unconverted Rust judgments remain explicit until canonical-ledger certificates replace them.",
            TrustAcceptingPolicy::ExplicitMigrationTrust,
            Vec::new(),
            &[(
                "PCC-CANONICAL-SEMANTIC-LEDGER-v1",
                MIGRATION_POLICY_DESCRIPTOR,
            )],
        ),
    ]
}

fn proof_kernel_node() -> TrustDependencyNode {
    TrustDependencyNode::new(
        "implementation:rust-proof-kernel",
        TrustDependencyKind::TrustedImplementation,
        TrustDependencyStatus::TrustedJudgment,
        "Rust implementation of the current terminal proof calculus",
        "rust-proof-kernel-v7",
        "psi-proof-kernel",
        "portable proof bundle acceptance",
        "The current Rust kernel remains trusted until the independent low-rung checker closes the diamond.",
        TrustAcceptingPolicy::ExplicitMigrationTrust,
        dependencies(&[
            "root:canonical-proof-calculus-v15",
            "root:explicit-rust-migration-policy",
        ]),
        &[
            ("psi-proof-kernel/lib.rs", PROOF_KERNEL_LIB_SOURCE),
            ("psi-proof-kernel/evidence.rs", PROOF_KERNEL_EVIDENCE_SOURCE),
            ("psi-proof-kernel/kernel.rs", PROOF_KERNEL_KERNEL_SOURCE),
            (
                "psi-proof-kernel/integer_affine.rs",
                PROOF_KERNEL_INTEGER_AFFINE_SOURCE,
            ),
            (
                "psi-proof-kernel/integer_cast.rs",
                PROOF_KERNEL_INTEGER_CAST_SOURCE,
            ),
            ("psi-proof-kernel/proof.rs", PROOF_KERNEL_PROOF_SOURCE),
        ],
    )
}

fn decoder_node() -> TrustDependencyNode {
    TrustDependencyNode::new(
        "implementation:rust-terminal-decoder",
        TrustDependencyKind::TrustedImplementation,
        TrustDependencyStatus::TrustedJudgment,
        "Rust canonical terminal-Psi byte decoder and structural validation",
        "PSITERM-format-16-rust-decoder-v1",
        "psi-terminal-codec",
        "canonical bytes through validated TerminalModule",
        "The final low generator begins at bytes; the current Rust decoder must remain visible until then.",
        TrustAcceptingPolicy::ExplicitMigrationTrust,
        dependencies(&[
            "root:canonical-terminal-bytes-v15",
            "root:explicit-rust-migration-policy",
        ]),
        &[("psi-terminal-codec/lib.rs", CODEC_SOURCE)],
    )
}

fn verifier_node() -> TrustDependencyNode {
    TrustDependencyNode::new(
        "implementation:rust-terminal-verifier",
        TrustDependencyKind::TrustedImplementation,
        TrustDependencyStatus::TrustedJudgment,
        "Rust terminal-Psi artifact traversal and obligation reconstruction",
        "rust-terminal-verifier-v1",
        "psi-terminal-verifier",
        "portable semantic reconstruction and proof admission",
        "Rust remains the authoritative migration implementation until the canonical ledger is established independently.",
        TrustAcceptingPolicy::ExplicitMigrationTrust,
        dependencies(&[
            "implementation:rust-proof-kernel",
            "root:abstract-terminal-execution-model",
            "root:explicit-rust-migration-policy",
        ]),
        &[
            ("psi-terminal-verifier/lib.rs", VERIFIER_LIB_SOURCE),
            (
                "psi-terminal-verifier/validation.rs",
                VERIFIER_VALIDATION_SOURCE,
            ),
            ("psi-terminal-verifier/verification.rs", VERIFIER_SOURCE),
            (
                "psi-terminal-verifier/verification/call_composition.rs",
                VERIFIER_CALL_COMPOSITION_SOURCE,
            ),
            (
                "psi-terminal-verifier/verification/evidence_provenance.rs",
                EVIDENCE_PROVENANCE_SOURCE,
            ),
            (
                "psi-terminal-verifier/verification/integer_foundation.rs",
                INTEGER_FOUNDATION_SOURCE,
            ),
            (
                "psi-terminal-verifier/verification/proof_bundle.rs",
                PROOF_BUNDLE_SOURCE,
            ),
            (
                "psi-terminal-verifier/verification/reconstruction.rs",
                RECONSTRUCTION_SOURCE,
            ),
            (
                "psi-terminal-verifier/verification/sufficient_reduction.rs",
                SUFFICIENT_REDUCTION_SOURCE,
            ),
            (
                "psi-terminal-verifier/verification/substitution.rs",
                SUBSTITUTION_SOURCE,
            ),
        ],
    )
}

fn ledger_framework_node() -> TrustDependencyNode {
    TrustDependencyNode::new(
        "framework:canonical-semantic-ledger-v1",
        TrustDependencyKind::LedgerFramework,
        TrustDependencyStatus::TrustedJudgment,
        "current Rust control, validity, frontier, premise, and goal reconstruction framework",
        "canonical-semantic-ledger-framework-v1-unproved",
        "Psi proof architecture",
        "portable terminal-Psi ledger algebra before low-rung derivation",
        "The ledger framework and its composition bridges are specified but not yet low-rung proved.",
        TrustAcceptingPolicy::ExplicitMigrationTrust,
        dependencies(&[
            "implementation:rust-terminal-verifier",
            "root:abstract-terminal-execution-model",
            "root:explicit-rust-migration-policy",
        ]),
        &[
            (
                "psi-terminal-verifier/validation.rs",
                VERIFIER_VALIDATION_SOURCE,
            ),
            ("psi-terminal-verifier/verification.rs", VERIFIER_SOURCE),
            (
                "psi-terminal-verifier/verification/call_composition.rs",
                VERIFIER_CALL_COMPOSITION_SOURCE,
            ),
            (
                "psi-terminal-verifier/verification/proof_bundle.rs",
                PROOF_BUNDLE_SOURCE,
            ),
            (
                "psi-terminal-verifier/verification/reconstruction.rs",
                RECONSTRUCTION_SOURCE,
            ),
            (
                "psi-terminal-verifier/verification/substitution.rs",
                SUBSTITUTION_SOURCE,
            ),
            (
                "PCC-CANONICAL-SEMANTIC-LEDGER-v1",
                MIGRATION_POLICY_DESCRIPTOR,
            ),
        ],
    )
}

fn reduction_nodes() -> Vec<TrustDependencyNode> {
    [
        (
            "reduction:affine-joins",
            "affine fork/join, rectangle, product, quadratic, and correlated divide/remainder sufficient forms",
            "affine-joins-v1",
            "verification/affine_joins.rs",
            AFFINE_JOINS_SOURCE,
        ),
        (
            "reduction:integer-add-subtract",
            "exact integer addition and subtraction sufficient forms",
            "integer-add-subtract-v1",
            "verification/integer_add_subtract.rs",
            INTEGER_ADD_SUBTRACT_SOURCE,
        ),
        (
            "reduction:integer-affine",
            "integer affine-chain and affine-preimage sufficient forms",
            "integer-affine-v1",
            "verification/integer_affine.rs",
            INTEGER_AFFINE_SOURCE,
        ),
        (
            "reduction:integer-base-interval",
            "shared integer carrier, interval, path-fact, and dispatch sufficient forms",
            "integer-base-interval-v1",
            "verification/integer_foundation.rs",
            INTEGER_FOUNDATION_SOURCE,
        ),
        (
            "reduction:integer-conversion",
            "integer cast, widening, and conversion-spine sufficient forms",
            "integer-conversion-v1",
            "verification/integer_conversion.rs",
            INTEGER_CONVERSION_SOURCE,
        ),
        (
            "reduction:integer-divide-remainder",
            "exact divide and remainder sufficient forms",
            "integer-divide-remainder-v5",
            "verification/integer_divide_remainder.rs",
            INTEGER_DIVIDE_REMAINDER_SOURCE,
        ),
        (
            "reduction:integer-multiply",
            "integer multiply, product, and signed-product sufficient forms",
            "integer-multiply-v1",
            "verification/integer_multiply.rs",
            INTEGER_MULTIPLY_SOURCE,
        ),
        (
            "reduction:integer-shift",
            "exact integer shift and mixed shift-chain sufficient forms",
            "integer-shift-v1",
            "verification/integer_shift.rs",
            INTEGER_SHIFT_SOURCE,
        ),
    ]
    .into_iter()
    .map(|(identity, subject, version, source_name, source)| {
        let mut exact_sources = vec![
            (source_name, source),
            (
                "verification/sufficient_reduction.rs",
                SUFFICIENT_REDUCTION_SOURCE,
            ),
        ];
        if identity == "reduction:integer-conversion" {
            exact_sources.push((
                "verification/integer_conversion/chains.rs",
                INTEGER_CONVERSION_CHAINS_SOURCE,
            ));
            exact_sources.push((
                "verification/integer_conversion/composition.rs",
                INTEGER_CONVERSION_COMPOSITION_SOURCE,
            ));
        }
        if identity == "reduction:integer-shift" {
            exact_sources.push((
                "verification/integer_shift/chains.rs",
                INTEGER_SHIFT_CHAINS_SOURCE,
            ));
            exact_sources.push((
                "verification/integer_shift/composition.rs",
                INTEGER_SHIFT_COMPOSITION_SOURCE,
            ));
        }
        TrustDependencyNode::new(
            identity,
            TrustDependencyKind::SufficientFormReduction,
            TrustDependencyStatus::TrustedJudgment,
            subject,
            version,
            "psi-terminal-verifier",
            "sufficient-form proof-obligation reconstruction only",
            "This reducer may choose a sufficient proposition until it emits a checked derivation of the unchanged canonical goal.",
            TrustAcceptingPolicy::ExplicitMigrationTrust,
            dependencies(&[
                "implementation:rust-terminal-verifier",
                "root:abstract-terminal-execution-model",
                "root:explicit-rust-migration-policy",
            ]),
            &exact_sources,
        )
    })
    .collect()
}

fn operation_semantics_nodes() -> Vec<TrustDependencyNode> {
    validate_call_composition_semantic_rows(&CallCompositionSemanticRow::ALL)
        .expect("the closed call-composition table is exact, complete, and canonical");
    validate_proof_bearing_scalar_semantic_rows(&ProofBearingScalarSemanticRow::ALL)
        .expect("the closed proof-bearing scalar table is exact, complete, and canonical");
    validate_structural_effect_semantic_rows(&StructuralEffectSemanticRow::ALL)
        .expect("the closed structural/effect table is exact, complete, and canonical");
    OperationSemanticRow::ALL
        .iter()
        .map(|row| {
            let structural_effect = exact_structural_effect_semantic_row_in(
                row.tag(),
                &StructuralEffectSemanticRow::ALL,
            )
            .expect("the closed structural/effect table is exact and unique");
            let proof_bearing_scalar = exact_proof_bearing_scalar_semantic_row_in(
                row.tag(),
                &ProofBearingScalarSemanticRow::ALL,
            )
            .expect("the closed proof-bearing scalar table is exact and unique");
            let call_composition = exact_call_composition_semantic_row_in(
                row.tag(),
                &CallCompositionSemanticRow::ALL,
            )
            .expect("the closed call-composition table is exact and unique");
            let (kind, subject, version, scope, rationale) = match row.custody() {
                OperationSemanticCustody::LeafDenotation if structural_effect.is_some() => (
                    TrustDependencyKind::StructuralEffectSchema,
                    format!(
                        "terminal-Psi OperationKind::{} structural/effect leaf semantics",
                        row.name()
                    ),
                    "terminal-structural-effect-v1-unproved",
                    "one closed terminal-Psi structural/effect schema row",
                    "The Rust structural/effect row is trusted until its place custody, effect, fuel, and frontier theorem is accepted.",
                ),
                OperationSemanticCustody::LeafDenotation if proof_bearing_scalar.is_some() => (
                    TrustDependencyKind::DenotationSchema,
                    format!(
                        "terminal-Psi OperationKind::{} proof-bearing scalar denotation and canonical goal",
                        row.name()
                    ),
                    "terminal-proof-bearing-scalar-v1-unproved",
                    "one closed proof-bearing scalar denotation/goal schema row",
                    "The Rust row is trusted until its denotation and canonical-goal theorem plus the global composition bridge are accepted; sufficient-form reduction remains a separate dependency.",
                ),
                OperationSemanticCustody::LeafDenotation => (
                    TrustDependencyKind::DenotationSchema,
                    format!("terminal-Psi OperationKind::{} direct leaf denotation", row.name()),
                    "terminal-leaf-denotation-v1-unproved",
                    "one closed terminal-Psi leaf-operation schema row",
                    "The Rust leaf row is trusted until its universally quantified denotation theorem and composition bridge are accepted.",
                ),
                OperationSemanticCustody::CallComposition => (
                    TrustDependencyKind::CallComposition,
                    format!("terminal-Psi OperationKind::{} call composition", row.name()),
                    "terminal-call-composition-v1-unproved",
                    "one closed terminal-Psi call coverage/substitution/composition row",
                    "The Rust call row is trusted until exact clause coverage, capture-free substitution, outcomes, crash routes, and evidence lifetimes are established by the low ledger algebra.",
                ),
            };
            let mut exact_sources = vec![
                (
                    "psi-terminal-verifier/validation.rs",
                    VERIFIER_VALIDATION_SOURCE,
                ),
                ("psi-terminal-verifier/verification.rs", VERIFIER_SOURCE),
                (
                    "psi-terminal-verifier/verification/reconstruction.rs",
                    RECONSTRUCTION_SOURCE,
                ),
                (
                    "psi-terminal-semantics/lib.rs",
                    TERMINAL_SEMANTICS_SOURCE,
                ),
            ];
            if structural_effect.is_some() {
                exact_sources.push((
                    "psi-terminal-semantics/structural_effect.rs",
                    TERMINAL_STRUCTURAL_EFFECT_SOURCE,
                ));
            }
            if proof_bearing_scalar.is_some() {
                exact_sources.push((
                    "psi-terminal-semantics/proof_bearing_scalar.rs",
                    TERMINAL_PROOF_BEARING_SCALAR_SOURCE,
                ));
            }
            if call_composition.is_some() {
                exact_sources.push((
                    "psi-terminal-semantics/call_composition.rs",
                    TERMINAL_CALL_COMPOSITION_SOURCE,
                ));
                exact_sources.push((
                    "psi-terminal-verifier/verification/call_composition.rs",
                    VERIFIER_CALL_COMPOSITION_SOURCE,
                ));
            }
            TrustDependencyNode::new(
                row.identity(),
                kind,
                TrustDependencyStatus::TrustedJudgment,
                subject,
                version,
                "psi-terminal-verifier",
                scope,
                rationale,
                TrustAcceptingPolicy::ExplicitMigrationTrust,
                dependencies(&[
                    "framework:canonical-semantic-ledger-v1",
                    "root:abstract-terminal-execution-model",
                    "root:explicit-rust-migration-policy",
                ]),
                &exact_sources,
            )
        })
        .collect()
}

fn current_closure_node() -> TrustDependencyNode {
    let mut dependency_ids = vec![
        "framework:canonical-semantic-ledger-v1".to_owned(),
        "implementation:rust-terminal-decoder".to_owned(),
        "implementation:rust-terminal-verifier".to_owned(),
    ];
    dependency_ids.extend(reduction_nodes().into_iter().map(|node| node.identity));
    dependency_ids.extend(
        operation_semantics_nodes()
            .into_iter()
            .map(|node| node.identity),
    );
    dependency_ids.sort();
    TrustDependencyNode::new(
        CURRENT_ENTRY,
        TrustDependencyKind::AcceptanceClosure,
        TrustDependencyStatus::TrustedJudgment,
        "current deployable terminal-Psi semantic reconstruction closure",
        "terminal-pcc-current-rust-closure-v1",
        "Psi/Omega deployment pipeline",
        "portable terminal-Psi verification only",
        "The artifact is accepted through explicit migration trust and cannot be reported as fully derived.",
        TrustAcceptingPolicy::ExplicitMigrationTrust,
        dependency_ids,
        &[(
            "PCC-CANONICAL-SEMANTIC-LEDGER-v1",
            MIGRATION_POLICY_DESCRIPTOR,
        )],
    )
}

fn dependencies(values: &[&str]) -> Vec<String> {
    let mut values = values
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    values.sort();
    values
}

#[allow(clippy::too_many_arguments)]
fn dependency_digest(
    identity: &str,
    kind: TrustDependencyKind,
    status: TrustDependencyStatus,
    semantic_subject: &str,
    version: &str,
    owner: &str,
    scope: &str,
    rationale: &str,
    accepting_policy: TrustAcceptingPolicy,
    exact_sources: &[(&str, &[u8])],
) -> TrustDependencyDigest {
    let mut digest = Sha256::new();
    digest.update(NODE_DIGEST_DOMAIN);
    hash_string(&mut digest, identity);
    digest.update([kind as u8, status as u8, accepting_policy as u8]);
    for value in [semantic_subject, version, owner, scope, rationale] {
        hash_string(&mut digest, value);
    }
    hash_len(&mut digest, exact_sources.len());
    for (label, source) in exact_sources {
        hash_string(&mut digest, label);
        hash_bytes(&mut digest, source);
    }
    TrustDependencyDigest(digest.finalize().into())
}

fn graph_identity(entry: &str, nodes: &[TrustDependencyNode]) -> TerminalTrustGraphIdentity {
    let mut digest = Sha256::new();
    digest.update(GRAPH_DIGEST_DOMAIN);
    hash_string(&mut digest, entry);
    hash_len(&mut digest, nodes.len());
    for node in nodes {
        hash_string(&mut digest, &node.identity);
        digest.update([
            node.kind as u8,
            node.status as u8,
            node.accepting_policy as u8,
        ]);
        hash_string(&mut digest, &node.semantic_subject);
        digest.update(node.digest.as_bytes());
        hash_string(&mut digest, &node.version);
        hash_string(&mut digest, &node.owner);
        hash_string(&mut digest, &node.scope);
        hash_string(&mut digest, &node.rationale);
        hash_len(&mut digest, node.dependencies.len());
        for dependency in &node.dependencies {
            hash_string(&mut digest, dependency);
        }
    }
    TerminalTrustGraphIdentity(digest.finalize().into())
}

fn hash_string(digest: &mut Sha256, value: &str) {
    hash_bytes(digest, value.as_bytes());
}

fn hash_bytes(digest: &mut Sha256, bytes: &[u8]) {
    hash_len(digest, bytes.len());
    digest.update(bytes);
}

fn hash_len(digest: &mut Sha256, len: usize) {
    let len = u64::try_from(len).expect("trust-graph data fits u64");
    digest.update(len.to_le_bytes());
}

fn write_hex(formatter: &mut std::fmt::Formatter<'_>, bytes: &[u8; 32]) -> std::fmt::Result {
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_graph_is_closed_canonical_and_explicitly_not_fully_derived() {
        let graph = current_terminal_trust_graph().expect("built-in trust graph validates");
        assert_eq!(graph.entry(), CURRENT_ENTRY);
        assert!(!graph.is_fully_derived());
        assert_eq!(
            graph
                .nodes()
                .iter()
                .filter(|node| node.kind() == TrustDependencyKind::RegisteredRoot)
                .count(),
            4
        );
        let proof_calculus = graph
            .nodes()
            .iter()
            .find(|node| node.identity() == "root:canonical-proof-calculus-v15")
            .expect("current proof-calculus root");
        assert_eq!(proof_calculus.version(), "proof-bundle-format-18");
        let rust_kernel = graph
            .nodes()
            .iter()
            .find(|node| node.identity() == "implementation:rust-proof-kernel")
            .expect("current Rust proof kernel");
        assert_eq!(rust_kernel.version(), "rust-proof-kernel-v7");
        assert!(
            rust_kernel
                .dependencies()
                .contains(&"root:canonical-proof-calculus-v15".to_owned())
        );
        assert_eq!(
            graph
                .nodes()
                .iter()
                .filter(|node| node.kind() == TrustDependencyKind::SufficientFormReduction)
                .count(),
            8
        );
        assert_eq!(
            graph
                .nodes()
                .iter()
                .filter(|node| node.kind() == TrustDependencyKind::DenotationSchema)
                .count(),
            32
        );
        assert_eq!(
            graph
                .nodes()
                .iter()
                .filter(|node| node.version() == "terminal-proof-bearing-scalar-v1-unproved")
                .count(),
            12
        );
        assert_eq!(
            graph
                .nodes()
                .iter()
                .filter(|node| node.kind() == TrustDependencyKind::StructuralEffectSchema)
                .count(),
            3
        );
        assert_eq!(
            graph
                .nodes()
                .iter()
                .filter(|node| node.kind() == TrustDependencyKind::CallComposition)
                .count(),
            4
        );
        assert_eq!(OperationSemanticRow::ALL.len(), 39);
        assert_eq!(
            OperationSemanticRow::ALL
                .iter()
                .filter(|row| row.custody() == OperationSemanticCustody::LeafDenotation)
                .count(),
            35
        );
        assert_eq!(
            OperationSemanticRow::ALL
                .iter()
                .filter(|row| row.custody() == OperationSemanticCustody::CallComposition)
                .count(),
            4
        );
        assert_eq!(
            OperationSemanticRow::ALL
                .iter()
                .find(|row| row.tag() == OperationSemanticTag::Call)
                .expect("call row")
                .identity(),
            "algebra:call:call"
        );
        assert!(graph.nodes().iter().all(|node| {
            !node.identity().is_empty()
                && !node.semantic_subject().is_empty()
                && node.digest().as_bytes().iter().any(|byte| *byte != 0)
                && !node.version().is_empty()
                && !node.owner().is_empty()
                && !node.scope().is_empty()
                && !node.rationale().is_empty()
        }));
    }

    #[test]
    fn graph_rejects_unknown_cycles_unregistered_leaves_and_noncanonical_edges() {
        let root = test_root();
        let leaf = test_node("node:leaf", dependencies(&["root:test"]));
        let mut valid = vec![leaf.clone(), root.clone()];
        valid.sort_by(|left, right| left.identity.cmp(&right.identity));
        validate_terminal_trust_graph("node:leaf", valid).expect("closed graph");

        let mut unknown = vec![
            test_node("node:leaf", dependencies(&["root:missing"])),
            root.clone(),
        ];
        unknown.sort_by(|left, right| left.identity.cmp(&right.identity));
        assert!(matches!(
            validate_terminal_trust_graph("node:leaf", unknown),
            Err(TrustGraphError::UnknownDependency { .. })
        ));

        let mut cyclic = vec![
            test_node("node:left", dependencies(&["node:right"])),
            test_node("node:right", dependencies(&["node:left"])),
        ];
        cyclic.sort_by(|left, right| left.identity.cmp(&right.identity));
        assert!(matches!(
            validate_terminal_trust_graph("node:left", cyclic),
            Err(TrustGraphError::DependencyCycle(_))
        ));

        let no_root = vec![test_node("node:leaf", Vec::new())];
        assert!(matches!(
            validate_terminal_trust_graph("node:leaf", no_root),
            Err(TrustGraphError::NonRootHasNoDependencies(_))
        ));

        let reversed_dependencies = vec![
            test_node("node:leaf", vec!["root:z".to_owned(), "root:a".to_owned()]),
            test_root_named("root:a"),
            test_root_named("root:z"),
        ];
        assert!(matches!(
            validate_terminal_trust_graph("node:leaf", reversed_dependencies),
            Err(TrustGraphError::NonCanonicalDependencyOrder { .. })
        ));
    }

    #[test]
    fn graph_identity_binds_exact_dependency_edges_and_source_bytes() {
        let graph = current_terminal_trust_graph().expect("built-in graph");
        let mut nodes = graph.nodes().to_vec();
        let closure = nodes
            .iter_mut()
            .find(|node| node.identity == CURRENT_ENTRY)
            .expect("closure node");
        let framework = closure
            .dependencies
            .iter()
            .position(|dependency| dependency == "framework:canonical-semantic-ledger-v1")
            .expect("closure directly names the framework");
        closure.dependencies.remove(framework);
        let changed = validate_terminal_trust_graph(CURRENT_ENTRY, nodes)
            .expect("changed graph remains structurally closed");
        assert_ne!(graph.identity(), changed.identity());

        let first = TrustDependencyNode::new(
            "root:test",
            TrustDependencyKind::RegisteredRoot,
            TrustDependencyStatus::Registered,
            "test",
            "v1",
            "test",
            "test",
            "test",
            TrustAcceptingPolicy::RegisteredSemanticFoundation,
            Vec::new(),
            &[("source", b"first")],
        );
        let second = TrustDependencyNode::new(
            "root:test",
            TrustDependencyKind::RegisteredRoot,
            TrustDependencyStatus::Registered,
            "test",
            "v1",
            "test",
            "test",
            "test",
            TrustAcceptingPolicy::RegisteredSemanticFoundation,
            Vec::new(),
            &[("source", b"second")],
        );
        assert_ne!(first.digest(), second.digest());

        let shift = graph
            .nodes()
            .iter()
            .find(|node| node.identity == "reduction:integer-shift")
            .expect("integer-shift reduction node");
        let root_only_shift = TrustDependencyNode::new(
            shift.identity.clone(),
            shift.kind,
            shift.status,
            shift.semantic_subject.clone(),
            shift.version.clone(),
            shift.owner.clone(),
            shift.scope.clone(),
            shift.rationale.clone(),
            shift.accepting_policy,
            shift.dependencies.clone(),
            &[("verification/integer_shift.rs", INTEGER_SHIFT_SOURCE)],
        );
        assert_ne!(
            shift.digest(),
            root_only_shift.digest(),
            "shift custody must include both child implementation modules",
        );

        let conversion = graph
            .nodes()
            .iter()
            .find(|node| node.identity == "reduction:integer-conversion")
            .expect("integer-conversion reduction node");
        let root_only_conversion = TrustDependencyNode::new(
            conversion.identity.clone(),
            conversion.kind,
            conversion.status,
            conversion.semantic_subject.clone(),
            conversion.version.clone(),
            conversion.owner.clone(),
            conversion.scope.clone(),
            conversion.rationale.clone(),
            conversion.accepting_policy,
            conversion.dependencies.clone(),
            &[(
                "verification/integer_conversion.rs",
                INTEGER_CONVERSION_SOURCE,
            )],
        );
        assert_ne!(
            conversion.digest(),
            root_only_conversion.digest(),
            "conversion custody must include both child implementation modules",
        );

        let exact_add = graph
            .nodes()
            .iter()
            .find(|node| node.identity == "schema:operation:exact-integer-add")
            .expect("exact-add denotation node");
        let without_proof_bearing_schema = TrustDependencyNode::new(
            exact_add.identity.clone(),
            exact_add.kind,
            exact_add.status,
            exact_add.semantic_subject.clone(),
            exact_add.version.clone(),
            exact_add.owner.clone(),
            exact_add.scope.clone(),
            exact_add.rationale.clone(),
            exact_add.accepting_policy,
            exact_add.dependencies.clone(),
            &[
                (
                    "psi-terminal-verifier/validation.rs",
                    VERIFIER_VALIDATION_SOURCE,
                ),
                ("psi-terminal-verifier/verification.rs", VERIFIER_SOURCE),
                (
                    "psi-terminal-verifier/verification/reconstruction.rs",
                    RECONSTRUCTION_SOURCE,
                ),
                ("psi-terminal-semantics/lib.rs", TERMINAL_SEMANTICS_SOURCE),
            ],
        );
        assert_ne!(
            exact_add.digest(),
            without_proof_bearing_schema.digest(),
            "proof-bearing leaf custody must bind its exact canonical-goal table",
        );
    }

    fn test_root() -> TrustDependencyNode {
        test_root_named("root:test")
    }

    fn test_root_named(identity: &str) -> TrustDependencyNode {
        TrustDependencyNode::new(
            identity,
            TrustDependencyKind::RegisteredRoot,
            TrustDependencyStatus::Registered,
            "test root",
            "v1",
            "tests",
            "tests",
            "tests",
            TrustAcceptingPolicy::RegisteredSemanticFoundation,
            Vec::new(),
            &[("test", b"root")],
        )
    }

    fn test_node(identity: &str, dependencies: Vec<String>) -> TrustDependencyNode {
        TrustDependencyNode::new(
            identity,
            TrustDependencyKind::TrustedImplementation,
            TrustDependencyStatus::TrustedJudgment,
            "test node",
            "v1",
            "tests",
            "tests",
            "tests",
            TrustAcceptingPolicy::ExplicitMigrationTrust,
            dependencies,
            &[("test", b"node")],
        )
    }
}
