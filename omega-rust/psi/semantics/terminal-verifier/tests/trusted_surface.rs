//! Witnessed coverage for the trusted-surface inventory.
//!
//! These tests do not prove any rule sound. They witness that the inventory's
//! mechanical checks are wired to the real implementation: every dispatchable
//! enum variant is mapped exhaustively (compile-time, no wildcard arms), every
//! bound source file carries a digest that matches the working tree, every
//! non-test source under the trusted roots is claimed, and the coverage
//! checker itself fires on missing entries, shared entries, unclaimed files,
//! and changed digests.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use proof_admission::{
    AdmissionEvidence, AdmissionKind, CertificateEnvelope, CorrelatedAffineBranchWitness,
    EvidenceRoute, IntegerAffineWitness, IntegerCastChainWitness,
    IntegerCorrelatedForbiddenRootWitness, PrimitiveJudgment, ProofNode, ProofRule,
    ProofSystemMarker,
};
use semantic_vocabulary::{
    AdmissionSiteId, BlockId, ContractId, EdgeId, EvidenceIdentity, MachineId, OperationId,
    PlaceId, ProfileDecisionId, Proposition, ScalarTerm, StructuralCaseId, ValueId,
};
use terminal_psi::{StructuralCaseSuccessorEdge, SuccessorEdge, Terminator};
use terminal_semantics::OperationSemanticTag;
use terminal_verifier::ReconstructedTerminalObligationOwner;
use terminal_verifier::trusted_surface::{
    CoveredSurface, LedgerFailure, SoundnessStatus, TrustedSurfaceEntry, all_entries,
    check_dispatch_bijection, check_ledger_internals, check_source_coverage, checker_rule_entry,
    evidence_route_entry, obligation_owner_entry, operation_schema_entry, primitive_judgment_entry,
    terminator_fact_entry,
};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("crate lives four directories below the repository root")
        .to_path_buf()
}

fn report(failures: &[LedgerFailure]) -> String {
    failures
        .iter()
        .map(|failure| failure.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn truth_leaf() -> ProofNode {
    ProofNode {
        conclusion: Proposition::Truth,
        rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
    }
}

fn proof_rule_exemplars() -> Vec<ProofRule> {
    let leaf = || Box::new(truth_leaf());
    let term = || ScalarTerm::Boolean(true);
    let branch = || CorrelatedAffineBranchWitness {
        root: term(),
        target: term(),
        steps: Vec::new(),
    };
    vec![
        ProofRule::ValueEqualityTransport {
            premise: leaf(),
            equalities: Vec::new(),
        },
        ProofRule::PredicateDenotation { premise: leaf() },
        ProofRule::Primitive(PrimitiveJudgment::Truth),
        ProofRule::SemanticAxiom { index: 0 },
        ProofRule::Assumption { index: 0 },
        ProofRule::ConjunctionIntroduction(Vec::new()),
        ProofRule::ConjunctionElimination {
            conjunction: leaf(),
            conjunct: 0,
        },
        ProofRule::DisjunctionIntroduction {
            disjunct: leaf(),
            index: 0,
        },
        ProofRule::DisjunctionElimination {
            disjunction: leaf(),
            branches: Vec::new(),
        },
        ProofRule::ImplicationIntroduction { body: leaf() },
        ProofRule::ImplicationElimination {
            implication: leaf(),
            premise: leaf(),
        },
        ProofRule::EqualityTransitivity {
            left_equals_middle: leaf(),
            middle_equals_right: leaf(),
        },
        ProofRule::EqualitySymmetry { equality: leaf() },
        ProofRule::IntegerOrderWeakening { relation: leaf() },
        ProofRule::IntegerOrderDiscreteness { relation: leaf() },
        ProofRule::IntegerSubtractOrder {
            difference: leaf(),
            positive: leaf(),
        },
        ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle: leaf(),
            middle_less_or_equal_right: leaf(),
        },
        ProofRule::IntegerStrictOrderTransitivity {
            left_to_middle: leaf(),
            middle_to_right: leaf(),
        },
        ProofRule::IntegerOrderSubstitution {
            relation: leaf(),
            equality: leaf(),
            endpoint: 0,
        },
        ProofRule::IntegerAffineBound {
            root_bound: leaf(),
            witness: IntegerAffineWitness {
                root: term(),
                target: term(),
                definition_axioms: Vec::new(),
                literal_axioms: Vec::new(),
            },
        },
        ProofRule::IntegerExactAddDefinitionBound {
            left_bound: leaf(),
            right_bound: leaf(),
            definition_axiom: 0,
        },
        ProofRule::IntegerCastBound {
            root_bound: leaf(),
            witness: IntegerCastChainWitness {
                root: term(),
                target: term(),
                definition_axioms: Vec::new(),
            },
        },
        ProofRule::IntegerCorrelatedForbiddenRoots {
            witness: IntegerCorrelatedForbiddenRootWitness {
                dividend: branch(),
                divisor: branch(),
                definition_axiom_count: 0,
                lower_bound_axiom: 0,
                upper_bound_axiom: 0,
                conclusion: Proposition::Truth,
            },
        },
    ]
}

fn evidence_route_exemplars() -> Vec<EvidenceRoute> {
    vec![
        EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
        EvidenceRoute::CertificateDerived(CertificateEnvelope {
            identity: EvidenceIdentity::new(1).expect("nonzero identity"),
            proof_system_marker: ProofSystemMarker::CURRENT,
            proof: truth_leaf(),
        }),
        EvidenceRoute::Admitted(AdmissionEvidence {
            site: AdmissionSiteId::new(2).expect("nonzero identity"),
            kind: AdmissionKind::ProviderFact,
            authority_identity: EvidenceIdentity::new(3).expect("nonzero identity"),
            evidence_identity: EvidenceIdentity::new(4).expect("nonzero identity"),
            profile_decision: ProfileDecisionId::new(5).expect("nonzero identity"),
        }),
    ]
}

fn obligation_owner_exemplars() -> Vec<ReconstructedTerminalObligationOwner> {
    let machine = MachineId::new(1).expect("nonzero identity");
    let block = BlockId::new(2).expect("nonzero identity");
    let edge = EdgeId::new(3).expect("nonzero identity");
    let operation = OperationId::new(4).expect("nonzero identity");
    let contract = ContractId::new(5).expect("nonzero identity");
    vec![
        ReconstructedTerminalObligationOwner::ScalarBlockInvariant {
            machine,
            header: block,
            edge,
        },
        ReconstructedTerminalObligationOwner::Operation { machine, operation },
        ReconstructedTerminalObligationOwner::CallRequires {
            machine,
            operation,
            requirement_position: 0,
        },
        ReconstructedTerminalObligationOwner::NominalCleanupRequires {
            machine,
            edge,
            cleanup_position: 0,
            requirement_position: 1,
        },
        ReconstructedTerminalObligationOwner::ContractEnsures {
            machine,
            contract,
            clause_position: 0,
        },
    ]
}

fn terminator_exemplars() -> Vec<Terminator> {
    let edge = EdgeId::new(1).expect("nonzero identity");
    let other_edge = EdgeId::new(2).expect("nonzero identity");
    let target = BlockId::new(3).expect("nonzero identity");
    let value = ValueId::new(4).expect("nonzero identity");
    let place = PlaceId::new(5).expect("nonzero identity");
    let successor = |edge: EdgeId| SuccessorEdge {
        edge,
        target,
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    vec![
        Terminator::Jump {
            edge,
            target,
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        },
        Terminator::Conditional {
            condition: value,
            when_true: successor(edge),
            when_false: successor(other_edge),
        },
        Terminator::StructuralCase {
            source: place,
            cases: vec![StructuralCaseSuccessorEdge {
                edge,
                target,
                case: StructuralCaseId::new(6).expect("nonzero identity"),
                payload_fields: Vec::new(),
                trivial_affine_discards: Vec::new(),
            }],
        },
        Terminator::Return {
            edge,
            value,
            cleanup_actions: Vec::new(),
        },
        Terminator::ReturnUnit {
            edge,
            trivial_affine_discards: Vec::new(),
        },
        Terminator::ReturnUnitPartialAffine {
            edge,
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        },
        Terminator::ReturnUnitNominalAffine {
            edge,
            cleanups: Vec::new(),
        },
        Terminator::ReturnStructural {
            edge,
            source: place,
            returned_claims: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
        Terminator::Crash {
            edge,
            cause: terminal_psi::CrashCause::Trap,
            site_guard: Vec::new(),
            frontier_lower_bound: Vec::new(),
        },
    ]
}

fn kebab(name: &str) -> String {
    let mut out = String::new();
    for character in name.chars() {
        if character.is_ascii_uppercase() && !out.is_empty() {
            out.push('-');
        }
        out.push(character.to_ascii_lowercase());
    }
    out
}

#[test]
fn ledger_is_well_formed() {
    let failures = check_ledger_internals();
    assert!(
        failures.is_empty(),
        "ledger well-formedness failures:\n{}",
        report(&failures)
    );
}

#[test]
fn every_dispatch_variant_maps_to_its_own_entry() {
    let surfaces: Vec<(CoveredSurface, Vec<&TrustedSurfaceEntry>)> = vec![
        (
            CoveredSurface::PrimitiveJudgments,
            [
                PrimitiveJudgment::Truth,
                PrimitiveJudgment::ReflexiveEquality,
                PrimitiveJudgment::ClosedIntegerRelation,
                PrimitiveJudgment::IntegerCarrierBound,
            ]
            .iter()
            .map(|judgment| primitive_judgment_entry(*judgment))
            .collect(),
        ),
        (
            CoveredSurface::ProofRules,
            proof_rule_exemplars()
                .iter()
                .map(checker_rule_entry)
                .collect(),
        ),
        (
            CoveredSurface::EvidenceRoutes,
            evidence_route_exemplars()
                .iter()
                .map(evidence_route_entry)
                .collect(),
        ),
        (
            CoveredSurface::ObligationOwners,
            obligation_owner_exemplars()
                .iter()
                .map(obligation_owner_entry)
                .collect(),
        ),
        (
            CoveredSurface::OperationTags,
            OperationSemanticTag::ALL
                .iter()
                .map(|tag| operation_schema_entry(*tag))
                .collect(),
        ),
        (
            CoveredSurface::Terminators,
            terminator_exemplars()
                .iter()
                .map(terminator_fact_entry)
                .collect(),
        ),
    ];
    for (surface, produced) in &surfaces {
        let failures = check_dispatch_bijection(*surface, produced);
        assert!(
            failures.is_empty(),
            "{surface:?} coverage failures:\n{}",
            report(&failures)
        );
    }
}

#[test]
fn operation_entries_name_their_tag() {
    for tag in OperationSemanticTag::ALL {
        let entry = operation_schema_entry(tag);
        let expected = format!("operation:{}", kebab(tag.name()));
        assert_eq!(
            entry.id,
            expected,
            "operation tag {} must bind entry {expected}",
            tag.name()
        );
    }
}

#[test]
fn recorded_digests_match_the_working_tree() {
    let failures = check_source_coverage(&repository_root());
    assert!(
        failures.is_empty(),
        "source coverage failures:\n{}",
        report(&failures)
    );
}

#[test]
fn coverage_check_fires_on_a_missing_variant_entry() {
    let mut produced: Vec<&TrustedSurfaceEntry> = OperationSemanticTag::ALL
        .iter()
        .map(|tag| operation_schema_entry(*tag))
        .collect();
    let dropped = produced.pop().expect("operation surface is nonempty");
    let failures = check_dispatch_bijection(CoveredSurface::OperationTags, &produced);
    assert!(
        failures.contains(&LedgerFailure::DispatchEntryNotProduced {
            entry: dropped.id,
            surface: CoveredSurface::OperationTags,
        }),
        "dropping a variant's entry must fail coverage, got:\n{}",
        report(&failures)
    );
}

#[test]
fn coverage_check_fires_on_a_shared_entry() {
    let produced: Vec<&TrustedSurfaceEntry> = vec![
        primitive_judgment_entry(PrimitiveJudgment::Truth),
        primitive_judgment_entry(PrimitiveJudgment::Truth),
        primitive_judgment_entry(PrimitiveJudgment::ReflexiveEquality),
        primitive_judgment_entry(PrimitiveJudgment::ClosedIntegerRelation),
    ];
    let failures = check_dispatch_bijection(CoveredSurface::PrimitiveJudgments, &produced);
    assert!(
        failures.iter().any(|failure| matches!(
            failure,
            LedgerFailure::DispatchVariantSharedEntry {
                surface: CoveredSurface::PrimitiveJudgments,
                ..
            }
        )),
        "two variants sharing one entry must fail coverage, got:\n{}",
        report(&failures)
    );
}

fn scratch_tree() -> (tempfile_guard::Guard, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "trusted-surface-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    let source = repository_root().join("omega-rust/psi/semantics/proof-admission/src");
    let destination = root.join("omega-rust/psi/semantics/proof-admission/src");
    copy_tree(&source, &destination);
    (tempfile_guard::Guard(root.clone()), root)
}

fn copy_tree(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).expect("create scratch tree");
    for entry in std::fs::read_dir(source).expect("read trusted root") {
        let entry = entry.expect("directory entry");
        let target = destination.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("copy site file");
        }
    }
}

mod tempfile_guard {
    use std::path::PathBuf;

    pub struct Guard(pub PathBuf);

    impl Drop for Guard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

#[test]
fn source_coverage_fires_on_an_unclaimed_source_file() {
    let (_guard, root) = scratch_tree();
    let rogue = root.join("omega-rust/psi/semantics/proof-admission/src/rogue_rule.rs");
    std::fs::write(&rogue, b"// unclaimed source\n").expect("write rogue source");
    let failures = check_source_coverage(&root);
    assert!(
        failures.iter().any(|failure| matches!(
            failure,
            LedgerFailure::UnclaimedSourceFile { path }
            if path == "omega-rust/psi/semantics/proof-admission/src/rogue_rule.rs"
        )),
        "an unclaimed source file must fail coverage, got:\n{}",
        report(&failures)
    );
}

#[test]
fn source_coverage_fires_on_a_changed_implementation() {
    let (_guard, root) = scratch_tree();
    let kernel = root.join("omega-rust/psi/semantics/proof-admission/src/kernel.rs");
    let mut contents = std::fs::read(&kernel).expect("read kernel site");
    contents.push(b'\n');
    std::fs::write(&kernel, contents).expect("perturb kernel site");
    let failures = check_source_coverage(&root);
    let mismatch = failures.iter().find(|failure| {
        matches!(
            failure,
            LedgerFailure::DigestMismatch { path, .. }
            if *path == "omega-rust/psi/semantics/proof-admission/src/kernel.rs"
        )
    });
    let Some(LedgerFailure::DigestMismatch { dependents, .. }) = mismatch else {
        panic!(
            "an edited implementation must fail coverage, got:\n{}",
            report(&failures)
        );
    };
    assert!(
        dependents.contains(&"primitive:truth"),
        "the digest failure must name entries needing revalidation, got {dependents:?}"
    );
}

#[test]
fn soundness_status_is_explicit_and_never_implied_by_coverage() {
    let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut unfinished = Vec::new();
    for entry in all_entries() {
        let key = match entry.soundness {
            SoundnessStatus::Proved { .. } => "Proved",
            SoundnessStatus::ExplicitlyTrusted { .. } => "ExplicitlyTrusted",
            SoundnessStatus::Unfinished { .. } => {
                unfinished.push(entry.id);
                "Unfinished"
            }
        };
        *counts.entry(key).or_default() += 1;
    }
    let total: usize = counts.values().sum();
    assert_eq!(total, all_entries().count());
    // Coverage is inventory, not soundness: this is the current trusted vs
    // proved balance, printed so a claim of "proved" must carry evidence.
    eprintln!(
        "trusted-surface ledger: {counts:?} over {total} entries; unfinished: {unfinished:?}"
    );
    assert!(counts.get("ExplicitlyTrusted").copied().unwrap_or(0) > 0);
}
