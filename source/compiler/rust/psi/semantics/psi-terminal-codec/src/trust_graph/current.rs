//! Exact source-bound construction of the current migration trust graph.

use super::{
    AFFINE_JOINS_SOURCE, CODEC_SOURCE, CURRENT_ENTRY, EVIDENCE_PROVENANCE_SOURCE,
    INTEGER_ADD_SUBTRACT_SOURCE, INTEGER_AFFINE_SOURCE, INTEGER_CONVERSION_CHAINS_SOURCE,
    INTEGER_CONVERSION_COMPOSITION_SOURCE, INTEGER_CONVERSION_SOURCE,
    INTEGER_DIVIDE_REMAINDER_SOURCE, INTEGER_FOUNDATION_SOURCE, INTEGER_MULTIPLY_SOURCE,
    INTEGER_SHIFT_CHAINS_SOURCE, INTEGER_SHIFT_COMPOSITION_SOURCE, INTEGER_SHIFT_SOURCE,
    MIGRATION_POLICY_DESCRIPTOR, OBLIGATION_LEDGER_CODEC_SOURCE, PROOF_ADMISSION_EVIDENCE_SOURCE,
    PROOF_ADMISSION_INTEGER_AFFINE_SOURCE, PROOF_ADMISSION_INTEGER_CAST_SOURCE,
    PROOF_ADMISSION_JUDGMENT_SOURCE, PROOF_ADMISSION_LIB_SOURCE, PROOF_ADMISSION_PROOF_SOURCE,
    PROOF_BUNDLE_SOURCE, PROOF_CODEC_SOURCE, PROPOSITION_SOURCE, RECONSTRUCTION_SOURCE,
    SUBSTITUTION_SOURCE, SUFFICIENT_REDUCTION_SOURCE, TERMINAL_CALL_COMPOSITION_SOURCE,
    TERMINAL_MODEL_SOURCE, TERMINAL_PROOF_BEARING_SCALAR_SOURCE, TERMINAL_SEMANTICS_SOURCE,
    TERMINAL_STRUCTURAL_EFFECT_SOURCE, TrustAcceptingPolicy, TrustDependencyKind,
    TrustDependencyNode, TrustDependencyStatus, TrustGraphError, VERIFIER_CALL_COMPOSITION_SOURCE,
    VERIFIER_LIB_SOURCE, VERIFIER_SOURCE, VERIFIER_SOURCE_CLOSURE,
    VERIFIER_SOURCE_CLOSURE_BUILD_SOURCE, VERIFIER_VALIDATION_SOURCE, ValidatedTerminalTrustGraph,
    validate_terminal_trust_graph,
};
use crate::FORMAT_MARKER;
use psi_terminal_semantics::{
    CallCompositionSemanticRow, OperationSemanticCustody, OperationSemanticRow,
    ProofBearingScalarSemanticRow, StructuralEffectSemanticRow,
    exact_call_composition_semantic_row_in, exact_proof_bearing_scalar_semantic_row_in,
    exact_structural_effect_semantic_row_in, validate_call_composition_semantic_rows,
    validate_proof_bearing_scalar_semantic_rows, validate_structural_effect_semantic_rows,
};

fn terminal_vocabulary_version() -> String {
    format!(
        "terminal-vocabulary-{}",
        psi_terminal::VocabularyMarker::CURRENT.get()
    )
}

fn canonical_terminal_bytes_identity() -> &'static str {
    "root:canonical-terminal-bytes-format-30-vocabulary-32"
}

fn canonical_terminal_bytes_version() -> String {
    format!(
        "PSITERM-format-{FORMAT_MARKER}-vocabulary-{}",
        psi_terminal::VocabularyMarker::CURRENT.get()
    )
}

fn canonical_proof_calculus_identity() -> &'static str {
    "root:canonical-proof-calculus-format-19"
}

fn canonical_proof_calculus_version() -> String {
    format!("proof-bundle-format-{}", crate::proof_bundle::FORMAT_MARKER)
}

pub(super) fn build_current_terminal_trust_graph()
-> Result<ValidatedTerminalTrustGraph, TrustGraphError> {
    let mut nodes = Vec::new();
    nodes.extend(registered_roots());
    nodes.push(proof_admission_node());
    nodes.push(decoder_node());
    nodes.push(verifier_node());
    nodes.push(ledger_framework_node());
    nodes.extend(reduction_nodes());
    nodes.extend(operation_semantics_nodes());
    nodes.push(current_closure_node());
    nodes.sort_by(|left, right| left.identity.cmp(&right.identity));
    validate_terminal_trust_graph(CURRENT_ENTRY, nodes)
}

fn registered_roots() -> Vec<TrustDependencyNode> {
    vec![
        TrustDependencyNode::new(
            "root:abstract-terminal-execution-model",
            TrustDependencyKind::RegisteredRoot,
            TrustDependencyStatus::Registered,
            "terminal-Psi abstract operational semantics",
            terminal_vocabulary_version(),
            "Psi language architecture",
            "portable terminal-Psi execution before native refinement",
            "Portable PCC bottoms out in the abstract terminal execution model.",
            TrustAcceptingPolicy::RegisteredSemanticFoundation,
            Vec::new(),
            &[("psi-terminal/module.rs", TERMINAL_MODEL_SOURCE)],
        ),
        TrustDependencyNode::new(
            canonical_proof_calculus_identity(),
            TrustDependencyKind::RegisteredRoot,
            TrustDependencyStatus::Registered,
            "terminal-Psi proof bundle and primitive calculus",
            canonical_proof_calculus_version(),
            "Psi proof-admission architecture",
            "portable terminal-Psi proof checking",
            "The current small proof calculus is an explicit registered semantic root.",
            TrustAcceptingPolicy::RegisteredSemanticFoundation,
            Vec::new(),
            &[
                ("psi-core/proposition.rs", PROPOSITION_SOURCE),
                (
                    "psi-proof-admission/evidence.rs",
                    PROOF_ADMISSION_EVIDENCE_SOURCE,
                ),
                (
                    "psi-proof-admission/kernel.rs",
                    PROOF_ADMISSION_JUDGMENT_SOURCE,
                ),
                (
                    "psi-proof-admission/integer_affine.rs",
                    PROOF_ADMISSION_INTEGER_AFFINE_SOURCE,
                ),
                (
                    "psi-proof-admission/integer_cast.rs",
                    PROOF_ADMISSION_INTEGER_CAST_SOURCE,
                ),
                ("psi-proof-admission/lib.rs", PROOF_ADMISSION_LIB_SOURCE),
                ("psi-proof-admission/proof.rs", PROOF_ADMISSION_PROOF_SOURCE),
                ("psi-terminal-codec/proof_bundle.rs", PROOF_CODEC_SOURCE),
            ],
        ),
        TrustDependencyNode::new(
            canonical_terminal_bytes_identity(),
            TrustDependencyKind::RegisteredRoot,
            TrustDependencyStatus::Registered,
            "canonical terminal-Psi semantic bytes",
            canonical_terminal_bytes_version(),
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

fn proof_admission_node() -> TrustDependencyNode {
    TrustDependencyNode::new(
        "implementation:rust-proof-admission",
        TrustDependencyKind::TrustedImplementation,
        TrustDependencyStatus::TrustedJudgment,
        "Rust product-local proof admission and judgment checker",
        "rust-proof-admission-v7",
        "psi-proof-admission",
        "portable proof bundle acceptance",
        "The current Rust admission checker remains trusted until the independent low-rung checker closes the diamond.",
        TrustAcceptingPolicy::ExplicitMigrationTrust,
        dependencies(&[
            canonical_proof_calculus_identity(),
            "root:explicit-rust-migration-policy",
        ]),
        &[
            ("psi-proof-admission/lib.rs", PROOF_ADMISSION_LIB_SOURCE),
            (
                "psi-proof-admission/evidence.rs",
                PROOF_ADMISSION_EVIDENCE_SOURCE,
            ),
            (
                "psi-proof-admission/kernel.rs",
                PROOF_ADMISSION_JUDGMENT_SOURCE,
            ),
            (
                "psi-proof-admission/integer_affine.rs",
                PROOF_ADMISSION_INTEGER_AFFINE_SOURCE,
            ),
            (
                "psi-proof-admission/integer_cast.rs",
                PROOF_ADMISSION_INTEGER_CAST_SOURCE,
            ),
            ("psi-proof-admission/proof.rs", PROOF_ADMISSION_PROOF_SOURCE),
        ],
    )
}

fn decoder_node() -> TrustDependencyNode {
    TrustDependencyNode::new(
        "implementation:rust-terminal-decoder",
        TrustDependencyKind::TrustedImplementation,
        TrustDependencyStatus::TrustedJudgment,
        "Rust canonical terminal-Psi byte decoder and structural validation",
        format!("{}-rust-decoder-v1", canonical_terminal_bytes_version()),
        "psi-terminal-codec",
        "canonical bytes through validated TerminalModule",
        "The final low generator begins at bytes; the current Rust decoder must remain visible until then.",
        TrustAcceptingPolicy::ExplicitMigrationTrust,
        dependencies(&[
            canonical_terminal_bytes_identity(),
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
            "implementation:rust-proof-admission",
            "root:abstract-terminal-execution-model",
            "root:explicit-rust-migration-policy",
        ]),
        &[
            (
                "psi-terminal-codec/build.rs",
                VERIFIER_SOURCE_CLOSURE_BUILD_SOURCE,
            ),
            (
                "psi-terminal-verifier/source-closure",
                VERIFIER_SOURCE_CLOSURE,
            ),
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
                "psi-terminal-codec/obligation_ledger.rs",
                OBLIGATION_LEDGER_CODEC_SOURCE,
            ),
            (
                "psi-terminal-codec/build.rs",
                VERIFIER_SOURCE_CLOSURE_BUILD_SOURCE,
            ),
            (
                "psi-terminal-verifier/source-closure",
                VERIFIER_SOURCE_CLOSURE,
            ),
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

pub(super) fn dependencies(values: &[&str]) -> Vec<String> {
    let mut values = values
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    values.sort();
    values
}
