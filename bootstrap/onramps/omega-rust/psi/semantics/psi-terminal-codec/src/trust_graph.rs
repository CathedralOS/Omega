//! Exact migration trust graph for terminal-Psi artifact verification.
//!
//! This is deliberately an honest description of the current deployment
//! boundary, not the final canonical-ledger implementation. Rust decoding,
//! semantic reconstruction, every sufficient-form reducer, the current ledger
//! framework, every unproved leaf schema, and every unproved call-composition
//! row remain explicit trusted judgments until low-rung derivations replace
//! them.

mod current;
mod identity;
mod validation;

use std::sync::OnceLock;

use current::build_current_terminal_trust_graph;
#[cfg(test)]
use current::dependencies;
use identity::{dependency_digest, graph_identity};
use psi_terminal_semantics::operation_semantic_row;
#[cfg(test)]
use psi_terminal_semantics::{
    OperationSemanticCustody, OperationSemanticRow, OperationSemanticTag,
};
pub use validation::validate_terminal_trust_graph;

const CURRENT_ENTRY: &str = "closure:terminal-pcc-current";
const MIGRATION_POLICY_DESCRIPTOR: &[u8] = b"PCC-CANONICAL-SEMANTIC-LEDGER-v1\0canonical bytes -> exhaustive local ledger -> unchanged canonical goals\0Rust decoder/verifier/reducers remain explicit trusted judgments until low-rung derivations replace them\0unknown and cyclic leaves reject\0portable terminal semantics only";
static CURRENT_TRUST_GRAPH: OnceLock<Result<ValidatedTerminalTrustGraph, TrustGraphError>> =
    OnceLock::new();

const CODEC_SOURCE: &[u8] = include_bytes!("lib.rs");
const VERIFIER_LIB_SOURCE: &[u8] = include_bytes!("../../psi-terminal-verifier/src/lib.rs");
const VERIFIER_VALIDATION_SOURCE: &[u8] =
    include_bytes!("../../psi-terminal-verifier/src/validation.rs");
const VERIFIER_SOURCE: &[u8] = include_bytes!("../../psi-terminal-verifier/src/verification.rs");
const VERIFIER_SOURCE_CLOSURE_BUILD_SOURCE: &[u8] = include_bytes!("../build.rs");
const VERIFIER_SOURCE_CLOSURE: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/psi-terminal-verifier-source-closure.bin"
));
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
const PROOF_KERNEL_LIB_SOURCE: &[u8] = include_bytes!("../../psi-proof-admission/src/lib.rs");
const PROOF_KERNEL_EVIDENCE_SOURCE: &[u8] =
    include_bytes!("../../psi-proof-admission/src/evidence.rs");
const PROOF_KERNEL_KERNEL_SOURCE: &[u8] = include_bytes!("../../psi-proof-admission/src/kernel.rs");
const PROOF_KERNEL_PROOF_SOURCE: &[u8] = include_bytes!("../../psi-proof-admission/src/proof.rs");
const PROOF_KERNEL_INTEGER_AFFINE_SOURCE: &[u8] =
    include_bytes!("../../psi-proof-admission/src/integer_affine.rs");
const PROOF_KERNEL_INTEGER_CAST_SOURCE: &[u8] =
    include_bytes!("../../psi-proof-admission/src/integer_cast.rs");
const PROOF_CODEC_SOURCE: &[u8] = include_bytes!("proof_bundle.rs");
const OBLIGATION_LEDGER_CODEC_SOURCE: &[u8] = include_bytes!("obligation_ledger.rs");
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
        let proof_calculus_identity = format!(
            "root:canonical-proof-calculus-format-{}",
            crate::proof_bundle::FORMAT_MARKER
        );
        let proof_calculus = graph
            .nodes()
            .iter()
            .find(|node| node.identity() == proof_calculus_identity)
            .expect("current proof-calculus root");
        assert_eq!(
            proof_calculus.version(),
            format!("proof-bundle-format-{}", crate::proof_bundle::FORMAT_MARKER)
        );
        let rust_kernel = graph
            .nodes()
            .iter()
            .find(|node| node.identity() == "implementation:rust-proof-kernel")
            .expect("current Rust proof kernel");
        assert_eq!(rust_kernel.version(), "rust-proof-kernel-v7");
        assert!(
            rust_kernel
                .dependencies()
                .contains(&proof_calculus_identity)
        );
        let terminal_bytes_identity = format!(
            "root:canonical-terminal-bytes-format-{}-vocabulary-{}",
            crate::FORMAT_MARKER,
            psi_terminal::VocabularyMarker::CURRENT.get()
        );
        let terminal_bytes = graph
            .nodes()
            .iter()
            .find(|node| node.identity() == terminal_bytes_identity)
            .expect("current canonical-terminal-bytes root");
        assert_eq!(
            terminal_bytes.version(),
            format!(
                "PSITERM-format-{}-vocabulary-{}",
                crate::FORMAT_MARKER,
                psi_terminal::VocabularyMarker::CURRENT.get()
            )
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
            4
        );
        assert_eq!(
            graph
                .nodes()
                .iter()
                .filter(|node| node.kind() == TrustDependencyKind::CallComposition)
                .count(),
            5
        );
        assert_eq!(OperationSemanticRow::ALL.len(), 41);
        assert_eq!(
            OperationSemanticRow::ALL
                .iter()
                .filter(|row| row.custody() == OperationSemanticCustody::LeafDenotation)
                .count(),
            36
        );
        assert_eq!(
            OperationSemanticRow::ALL
                .iter()
                .filter(|row| row.custody() == OperationSemanticCustody::CallComposition)
                .count(),
            5
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
