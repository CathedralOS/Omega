//! One-field mutation coverage for the bounded PCC `.proof` sidecar
//! envelope — the receipt certifying one published canonical Terminal
//! artifact.
//!
//! The sidecar carries eight representable claim fields: the product kind,
//! the artifact content commitment, the semantic and checker profile
//! identities, the offered guarantee roster with each guarantee's premise
//! roster, the out-of-band evidence bytes, the assumption closure, and the
//! omitted-dependency inventory with each dependency's content commitment.
//! The envelope holds no self-identity: its binding to the certified artifact
//! is the artifact commitment, which independent replay recomputes from the
//! offered artifact bytes, and every offered claim field is re-established
//! from the decoded artifact under the receiver's pinned policy. A one-field
//! substitution therefore either fails to form a canonical envelope at
//! construction or decoding, or decodes to a different offer the replay joins
//! reject — even when the artifact commitment is honestly recomputed over
//! substituted artifact bytes.

use super::{canonical_artifact, kernel_bundle, operation_id, semantic_module};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{BoundaryMachineId, ServiceId};
use std::ops::Range;
use terminal_codec::{
    CanonicalTerminalArtifact, CodecError, PSI_TERMINAL_VERIFIED_GUARANTEE, PccDependency,
    PccGuarantee, PccProductKind, PccProofSidecar, PccReceiverPolicy, PccVerificationOutcome,
    admission_profile_identity, build_psi_proof_sidecar, pcc_artifact_commitment,
    psi_semantic_profile_identity, terminal_assumption_closure, verify_psi_proof_sidecar,
};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, InstallationReachDependency, Operation,
    OperationKind, OperationResult, ServiceDeclaration,
};

/// A read cursor that records the byte span of every field it walks.
struct Cursor<'bytes> {
    bytes: &'bytes [u8],
    offset: usize,
}

impl<'bytes> Cursor<'bytes> {
    const fn new(bytes: &'bytes [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, len: usize) -> Range<usize> {
        let span = self.offset..self.offset + len;
        self.offset += len;
        span
    }

    fn u32(&mut self) -> (Range<usize>, usize) {
        let span = self.take(4);
        let value = u32::from_le_bytes(self.bytes[span.clone()].try_into().expect("u32 field"));
        (span, usize::try_from(value).expect("u32 length fits usize"))
    }

    fn u64(&mut self) -> (Range<usize>, usize) {
        let span = self.take(8);
        let value = u64::from_le_bytes(self.bytes[span.clone()].try_into().expect("u64 field"));
        (span, usize::try_from(value).expect("u64 length fits usize"))
    }

    fn string(&mut self) -> StringSpan {
        let (len, text_len) = self.u32();
        StringSpan {
            len,
            text: self.take(text_len),
        }
    }
}

/// The wire span of one length-prefixed UTF-8 field.
struct StringSpan {
    len: Range<usize>,
    text: Range<usize>,
}

impl StringSpan {
    fn whole(&self) -> Range<usize> {
        self.len.start..self.text.end
    }
}

/// The wire span of one guarantee row.
struct GuaranteeSpan {
    whole: Range<usize>,
    identity: StringSpan,
    premise_count: Range<usize>,
    premises: Vec<StringSpan>,
}

/// The wire span of one dependency row.
struct DependencySpan {
    whole: Range<usize>,
    identity: StringSpan,
    commitment: Range<usize>,
}

/// Byte offsets of every wire field inside one canonical sidecar encoding.
struct SidecarSpans {
    magic: Range<usize>,
    format_marker: Range<usize>,
    product: Range<usize>,
    commitment: Range<usize>,
    semantic_profile: StringSpan,
    checker_profile: StringSpan,
    guarantee_count: Range<usize>,
    guarantees: Vec<GuaranteeSpan>,
    evidence_len: Range<usize>,
    evidence: Range<usize>,
    assumption_count: Range<usize>,
    assumptions: Vec<StringSpan>,
    dependency_count: Range<usize>,
    dependencies: Vec<DependencySpan>,
    end: usize,
}

fn sidecar_spans(encoded: &[u8]) -> SidecarSpans {
    let mut cursor = Cursor::new(encoded);
    let magic = cursor.take(8);
    let format_marker = cursor.take(2);
    let product = cursor.take(1);
    let commitment = cursor.take(32);
    let semantic_profile = cursor.string();
    let checker_profile = cursor.string();
    let (guarantee_count, guarantees_len) = cursor.u32();
    let mut guarantees = Vec::with_capacity(guarantees_len);
    for _ in 0..guarantees_len {
        let start = cursor.offset;
        let identity = cursor.string();
        let (premise_count, premises_len) = cursor.u32();
        let mut premises = Vec::with_capacity(premises_len);
        for _ in 0..premises_len {
            premises.push(cursor.string());
        }
        guarantees.push(GuaranteeSpan {
            whole: start..cursor.offset,
            identity,
            premise_count,
            premises,
        });
    }
    let (evidence_len, evidence_size) = cursor.u64();
    let evidence = cursor.take(evidence_size);
    let (assumption_count, assumptions_len) = cursor.u32();
    let mut assumptions = Vec::with_capacity(assumptions_len);
    for _ in 0..assumptions_len {
        assumptions.push(cursor.string());
    }
    let (dependency_count, dependencies_len) = cursor.u32();
    let mut dependencies = Vec::with_capacity(dependencies_len);
    for _ in 0..dependencies_len {
        let start = cursor.offset;
        let identity = cursor.string();
        let commitment = cursor.take(32);
        dependencies.push(DependencySpan {
            whole: start..cursor.offset,
            identity,
            commitment,
        });
    }
    SidecarSpans {
        magic,
        format_marker,
        product,
        commitment,
        semantic_profile,
        checker_profile,
        guarantee_count,
        guarantees,
        evidence_len,
        evidence,
        assumption_count,
        assumptions,
        dependency_count,
        dependencies,
        end: cursor.offset,
    }
}

/// The eight representable claim fields of one sidecar, rebuilt through the
/// canonical constructor so every mutation re-encodes canonically.
#[derive(Clone)]
struct SidecarSpec {
    product: PccProductKind,
    commitment: [u8; 32],
    semantic_profile: String,
    checker_profile: String,
    guarantees: Vec<PccGuarantee>,
    evidence: Vec<u8>,
    assumptions: Vec<String>,
    dependencies: Vec<PccDependency>,
}

impl SidecarSpec {
    fn of(sidecar: &PccProofSidecar) -> Self {
        Self {
            product: sidecar.product(),
            commitment: *sidecar.artifact_commitment(),
            semantic_profile: sidecar.semantic_profile().to_owned(),
            checker_profile: sidecar.checker_profile().to_owned(),
            guarantees: sidecar.guarantees().to_vec(),
            evidence: sidecar.evidence().to_vec(),
            assumptions: sidecar.assumptions().to_vec(),
            dependencies: sidecar.dependencies().to_vec(),
        }
    }

    fn build(&self) -> Result<PccProofSidecar, CodecError> {
        PccProofSidecar::new(
            self.product,
            self.commitment,
            self.semantic_profile.clone(),
            self.checker_profile.clone(),
            self.guarantees.clone(),
            self.evidence.clone(),
            self.assumptions.clone(),
            self.dependencies.clone(),
        )
    }
}

/// One verified pair: a canonical artifact with an installation-reach
/// dependency, and the receiver policy that pins the honest claim.
fn artifact_and_receiver() -> (CanonicalTerminalArtifact, PccReceiverPolicy) {
    let mut module = semantic_module();
    let boundary = BoundaryMachineId::new(1).expect("boundary identity");
    let service = ServiceId::new(1).expect("service identity");
    let dependency = InstallationReachDependency {
        requirement_identity: "test::observe".into(),
        upper_bound: vec![service],
    };
    module.services.push(ServiceDeclaration {
        id: service,
        identity: "test::Observation".into(),
        parents: Vec::new(),
    });
    module.boundary_machines.push(BoundaryMachineDeclaration {
        id: boundary,
        identity: dependency.requirement_identity.clone(),
        attachment: None,
        parameter_order: Vec::new(),
        scalar_parameters: Vec::new(),
        crash_routes: Vec::new(),
        structural_parameters: Vec::new(),
        result: BoundaryMachineResult::Unit,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        fixed_service_reach: Vec::new(),
        published_service_ceiling: vec![service],
    });
    module.machines[0].published_service_ceiling = vec![service];
    module.machines[0].blocks[0].operations.push(Operation {
        id: operation_id(2),
        static_reach_binding: None,
        result: OperationResult::Unit,
        kind: OperationKind::BoundaryCall {
            boundary,
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
        },
    });
    module.root_service_reach.installation_dependencies = vec![dependency.clone()];
    let artifact = canonical_artifact(&module, &kernel_bundle(), None);
    let profile = AdmissionProfile::default();
    // Receiver choices come from the test's independently fixed contract,
    // never from the sidecar the adversary offers.
    let policy = PccReceiverPolicy {
        policy_package_identity: "test::receiver".into(),
        configuration_identity: "terminal-verification".into(),
        required_guarantees: vec![PSI_TERMINAL_VERIFIED_GUARANTEE.into()],
        admitted_premises: Vec::new(),
        accepted_semantic_profiles: vec![psi_semantic_profile_identity(&artifact).unwrap()],
        accepted_checker_profiles: vec![admission_profile_identity(&profile)],
        admitted_assumptions: terminal_assumption_closure(),
        possessed_dependencies: vec![
            PccDependency::from_installation_reach(&dependency, &module.services).unwrap(),
        ],
        admission_profile: profile,
        max_artifact_bytes: u64::MAX,
        max_evidence_bytes: u64::MAX,
    };
    (artifact, policy)
}

fn rejecting_subject(outcome: PccVerificationOutcome) -> String {
    match outcome {
        PccVerificationOutcome::Reject(rejection) => rejection.subject,
        other => panic!("expected a named rejection, got {other:?}"),
    }
}

/// Splice `replacement` over `span` of the canonical wire encoding.
fn splice(encoded: &[u8], span: &Range<usize>, replacement: &[u8]) -> Vec<u8> {
    let mut mutated =
        Vec::with_capacity(encoded.len() - (span.end - span.start) + replacement.len());
    mutated.extend_from_slice(&encoded[..span.start]);
    mutated.extend_from_slice(replacement);
    mutated.extend_from_slice(&encoded[span.end..]);
    mutated
}

/// Swap two adjacent wire spans, producing a non-canonical roster order.
fn swap(encoded: &[u8], first: &Range<usize>, second: &Range<usize>) -> Vec<u8> {
    assert_eq!(
        first.end, second.start,
        "swapped wire fields must be adjacent"
    );
    let mut mutated = Vec::with_capacity(encoded.len());
    mutated.extend_from_slice(&encoded[..first.start]);
    mutated.extend_from_slice(&encoded[second.clone()]);
    mutated.extend_from_slice(&encoded[first.clone()]);
    mutated.extend_from_slice(&encoded[second.end..]);
    mutated
}

#[test]
fn pcc_proof_sidecar_rejects_every_one_field_substitution() {
    let (artifact, policy) = artifact_and_receiver();
    let artifact_bytes = artifact.to_bytes();
    let honest = build_psi_proof_sidecar(&artifact, &policy.admission_profile, &artifact_bytes)
        .expect("honest sidecar");
    assert!(!honest.assumptions().is_empty());
    assert!(!honest.dependencies().is_empty());
    let encoded = honest.to_bytes();
    let spans = sidecar_spans(&encoded);
    assert_eq!(
        spans.end,
        encoded.len(),
        "the span walk covers the whole wire"
    );
    assert_eq!(spans.guarantees.len(), 1);
    assert_eq!(spans.dependencies.len(), 1);
    assert_eq!(
        PccProofSidecar::from_bytes(&encoded),
        Ok(honest.clone()),
        "the honest sidecar round-trips canonically"
    );
    assert!(
        matches!(
            verify_psi_proof_sidecar(&artifact_bytes, &encoded, &policy),
            PccVerificationOutcome::Complete(_)
        ),
        "the honest pair verifies"
    );
    let honest_spec = SidecarSpec::of(&honest);

    // A substituted claim field still forms a canonical envelope — it decodes
    // to the mutated offer — and independent replay rejects it with the named
    // subject under the receiver's pinned policy.
    let rejects_at_replay = |name: &'static str, spec: SidecarSpec, subject: &'static str| {
        let mutated = spec.build().unwrap_or_else(|error| {
            panic!("{name} must still form a canonical sidecar: {error:?}")
        });
        assert_ne!(mutated, honest, "{name} must change the sidecar");
        let mutated_bytes = mutated.to_bytes();
        assert_ne!(mutated_bytes, encoded, "{name} must change the wire");
        assert_eq!(
            PccProofSidecar::from_bytes(&mutated_bytes),
            Ok(mutated),
            "{name} must decode to the substituted offer"
        );
        assert_eq!(
            rejecting_subject(verify_psi_proof_sidecar(
                &artifact_bytes,
                &mutated_bytes,
                &policy
            )),
            subject,
            "{name}"
        );
    };

    // --- independently representable claim fields, each rejected by replay ---

    let mut spec = honest_spec.clone();
    spec.product = PccProductKind::Native;
    rejects_at_replay("the product kind", spec, "product kind");

    let mut spec = honest_spec.clone();
    spec.commitment = [0xAA; 32];
    rejects_at_replay("the artifact commitment", spec, "artifact bytes");

    let mut spec = honest_spec.clone();
    spec.semantic_profile = "foreign-semantics".into();
    rejects_at_replay("the semantic profile", spec, "semantic profile");

    let mut spec = honest_spec.clone();
    spec.checker_profile = "foreign-checker".into();
    rejects_at_replay("the checker profile", spec, "checker profile");

    let mut spec = honest_spec.clone();
    spec.guarantees[0].identity = "foreign-guarantee".into();
    rejects_at_replay(
        "a substituted guarantee identity",
        spec,
        "required guarantee",
    );

    let mut spec = honest_spec.clone();
    spec.guarantees.push(PccGuarantee {
        identity: "aaa-extra-guarantee".into(),
        premises: Vec::new(),
    });
    rejects_at_replay("an added guarantee", spec, "guarantees");

    let mut spec = honest_spec.clone();
    spec.guarantees[0]
        .premises
        .push("test::invented-premise".into());
    rejects_at_replay("an added premise", spec, "guarantee premise");

    let mut spec = honest_spec.clone();
    spec.evidence = vec![0x42];
    rejects_at_replay("non-empty evidence", spec, "psi evidence");

    let mut spec = honest_spec.clone();
    spec.assumptions[0] = "zz-foreign-assumption".into();
    rejects_at_replay("a substituted assumption", spec, "assumption");

    let mut spec = honest_spec.clone();
    spec.assumptions.push("zz-foreign-assumption".into());
    rejects_at_replay("an added assumption", spec, "assumption");

    let mut spec = honest_spec.clone();
    spec.assumptions.pop();
    rejects_at_replay("a dropped assumption", spec, "assumption closure");

    let mut spec = honest_spec.clone();
    spec.dependencies[0].identity = "foreign-dependency".into();
    rejects_at_replay("a substituted dependency identity", spec, "dependency");

    let mut spec = honest_spec.clone();
    spec.dependencies[0].content_commitment = [0xCC; 32];
    rejects_at_replay("a substituted dependency commitment", spec, "dependency");

    let mut spec = honest_spec.clone();
    spec.dependencies.push(PccDependency {
        identity: "zz-foreign-dependency".into(),
        content_commitment: [0xDD; 32],
    });
    rejects_at_replay("an added dependency", spec, "dependency");

    let mut spec = honest_spec.clone();
    spec.dependencies.pop();
    rejects_at_replay("a dropped dependency", spec, "dependency inventory");

    // --- the containing commitment honestly recomputed ---

    // Recomputing the artifact commitment over truncated artifact bytes does
    // not launder the substitution: the commitment join passes on the offered
    // bytes and the artifact leg rejects them.
    let truncated = &artifact_bytes[..artifact_bytes.len() - 1];
    let mut spec = honest_spec.clone();
    spec.commitment = pcc_artifact_commitment(truncated);
    let mutated = spec
        .build()
        .expect("a recomputed commitment still encodes canonically");
    assert_eq!(
        rejecting_subject(verify_psi_proof_sidecar(
            truncated,
            &mutated.to_bytes(),
            &policy
        )),
        "psi artifact",
        "a recomputed commitment over truncated bytes must still reject"
    );

    // A different canonical artifact under an honestly recomputed commitment
    // reaches claim reconstruction, where the offered dependency inventory
    // diverges from what the substituted artifact establishes.
    let foreign_artifact = canonical_artifact(&semantic_module(), &kernel_bundle(), None);
    let foreign_bytes = foreign_artifact.to_bytes();
    let mut spec = honest_spec.clone();
    spec.commitment = pcc_artifact_commitment(&foreign_bytes);
    let mutated = spec
        .build()
        .expect("a recomputed commitment still encodes canonically");
    assert_eq!(
        rejecting_subject(verify_psi_proof_sidecar(
            &foreign_bytes,
            &mutated.to_bytes(),
            &policy
        )),
        "dependency inventory",
        "a recomputed commitment over a foreign artifact must still reject"
    );

    // --- substitutions the receiver policy admits still reject at the
    // established-claim join ---

    let mut spec = honest_spec.clone();
    spec.guarantees[0]
        .premises
        .push("test::invented-premise".into());
    let mut admitted = policy.clone();
    admitted
        .admitted_premises
        .push("test::invented-premise".into());
    let mutated = spec.build().expect("an admitted premise encodes");
    assert_eq!(
        rejecting_subject(verify_psi_proof_sidecar(
            &artifact_bytes,
            &mutated.to_bytes(),
            &admitted
        )),
        "guarantees",
        "a policy-admitted premise must still reject against the established claim"
    );

    let mut spec = honest_spec.clone();
    spec.semantic_profile = "foreign-semantics".into();
    let mut admitted = policy.clone();
    admitted
        .accepted_semantic_profiles
        .push("foreign-semantics".into());
    let mutated = spec.build().expect("an admitted profile encodes");
    assert_eq!(
        rejecting_subject(verify_psi_proof_sidecar(
            &artifact_bytes,
            &mutated.to_bytes(),
            &admitted
        )),
        "semantic profile",
        "a policy-accepted foreign semantic profile must still reject"
    );

    let mut spec = honest_spec.clone();
    spec.checker_profile = "foreign-checker".into();
    let mut admitted = policy.clone();
    admitted
        .accepted_checker_profiles
        .push("foreign-checker".into());
    let mutated = spec.build().expect("an admitted profile encodes");
    assert_eq!(
        rejecting_subject(verify_psi_proof_sidecar(
            &artifact_bytes,
            &mutated.to_bytes(),
            &admitted
        )),
        "checker profile",
        "a policy-accepted foreign checker profile must still reject"
    );

    let foreign_dependency = PccDependency {
        identity: "test::foreign".into(),
        content_commitment: [0x5E; 32],
    };
    let mut spec = honest_spec.clone();
    spec.dependencies.push(foreign_dependency.clone());
    let mut admitted = policy.clone();
    admitted.possessed_dependencies.push(foreign_dependency);
    let mutated = spec.build().expect("a possessed dependency encodes");
    assert_eq!(
        rejecting_subject(verify_psi_proof_sidecar(
            &artifact_bytes,
            &mutated.to_bytes(),
            &admitted
        )),
        "dependency inventory",
        "a receiver-possessed foreign dependency must still reject"
    );

    // --- non-canonical claim fields rejected at construction ---

    for (name, spec, expected) in [
        (
            "an empty guarantee roster",
            SidecarSpec {
                guarantees: Vec::new(),
                ..honest_spec.clone()
            },
            CodecError::MalformedStructuralFoundation(
                "pcc sidecar must offer at least one guarantee",
            ),
        ),
        (
            "an empty semantic profile",
            SidecarSpec {
                semantic_profile: String::new(),
                ..honest_spec.clone()
            },
            CodecError::MalformedStructuralFoundation("pcc sidecar profiles must be non-empty"),
        ),
        (
            "an empty checker profile",
            SidecarSpec {
                checker_profile: String::new(),
                ..honest_spec.clone()
            },
            CodecError::MalformedStructuralFoundation("pcc sidecar profiles must be non-empty"),
        ),
        (
            "a duplicated guarantee",
            SidecarSpec {
                guarantees: vec![
                    honest.guarantees()[0].clone(),
                    honest.guarantees()[0].clone(),
                ],
                ..honest_spec.clone()
            },
            CodecError::NonCanonicalOrder("pcc guarantees"),
        ),
        (
            "an empty guarantee identity",
            SidecarSpec {
                guarantees: vec![PccGuarantee {
                    identity: String::new(),
                    premises: Vec::new(),
                }],
                ..honest_spec.clone()
            },
            CodecError::MalformedStructuralFoundation("pcc guarantee identity must be non-empty"),
        ),
        (
            "a duplicated premise",
            SidecarSpec {
                guarantees: vec![PccGuarantee {
                    identity: PSI_TERMINAL_VERIFIED_GUARANTEE.into(),
                    premises: vec!["a-premise".into(), "a-premise".into()],
                }],
                ..honest_spec.clone()
            },
            CodecError::NonCanonicalOrder("pcc premises"),
        ),
        (
            "an empty premise",
            SidecarSpec {
                guarantees: vec![PccGuarantee {
                    identity: PSI_TERMINAL_VERIFIED_GUARANTEE.into(),
                    premises: vec![String::new()],
                }],
                ..honest_spec.clone()
            },
            CodecError::MalformedStructuralFoundation("pcc premise must be non-empty"),
        ),
        (
            "a duplicated assumption",
            SidecarSpec {
                assumptions: vec![
                    honest.assumptions()[0].clone(),
                    honest.assumptions()[0].clone(),
                ],
                ..honest_spec.clone()
            },
            CodecError::NonCanonicalOrder("pcc assumptions"),
        ),
        (
            "an empty assumption",
            SidecarSpec {
                assumptions: vec![String::new()],
                ..honest_spec.clone()
            },
            CodecError::MalformedStructuralFoundation("pcc assumption must be non-empty"),
        ),
        (
            "a duplicated dependency identity",
            SidecarSpec {
                dependencies: vec![
                    honest.dependencies()[0].clone(),
                    PccDependency {
                        identity: honest.dependencies()[0].identity.clone(),
                        content_commitment: [0xEE; 32],
                    },
                ],
                ..honest_spec.clone()
            },
            CodecError::NonCanonicalOrder("pcc dependencies"),
        ),
        (
            "an empty dependency identity",
            SidecarSpec {
                dependencies: vec![PccDependency {
                    identity: String::new(),
                    content_commitment: [0xEE; 32],
                }],
                ..honest_spec.clone()
            },
            CodecError::MalformedStructuralFoundation("pcc dependency identity must be non-empty"),
        ),
    ] {
        assert_eq!(
            spec.build().map(|_| ()),
            Err(expected),
            "{name} must reject at construction"
        );
    }

    // --- wire-level substitutions rejected at canonical decoding ---

    let rejects_at_decode = |name: &'static str, mutated: Vec<u8>, expected: CodecError| {
        assert_eq!(
            PccProofSidecar::from_bytes(&mutated),
            Err(expected),
            "{name} must reject at canonical decoding"
        );
        assert_eq!(
            rejecting_subject(verify_psi_proof_sidecar(&artifact_bytes, &mutated, &policy)),
            "sidecar envelope",
            "{name} must reject the envelope before any claim join"
        );
    };

    let mut mutated = encoded.clone();
    mutated[spans.magic.start] ^= 0xFF;
    rejects_at_decode("the magic", mutated, CodecError::InvalidMagic);

    let mut mutated = encoded.clone();
    mutated[spans.format_marker.clone()].copy_from_slice(&u16::MAX.to_le_bytes());
    rejects_at_decode(
        "the format marker",
        mutated,
        CodecError::UnsupportedFormatMarker(u16::MAX),
    );

    let mut mutated = encoded.clone();
    mutated[spans.product.start] = 0x03;
    rejects_at_decode(
        "an unknown product tag",
        mutated,
        CodecError::InvalidTag("pcc product kind", 0x03),
    );

    let mut mutated = encoded.clone();
    mutated[spans.semantic_profile.len.clone()].copy_from_slice(&u32::MAX.to_le_bytes());
    rejects_at_decode(
        "an over-long semantic profile length",
        mutated,
        CodecError::StringTooLong("pcc semantic profile"),
    );

    rejects_at_decode(
        "an empty semantic profile",
        splice(
            &encoded,
            &spans.semantic_profile.whole(),
            &0_u32.to_le_bytes(),
        ),
        CodecError::MalformedStructuralFoundation("pcc sidecar profiles must be non-empty"),
    );

    rejects_at_decode(
        "an empty checker profile",
        splice(
            &encoded,
            &spans.checker_profile.whole(),
            &0_u32.to_le_bytes(),
        ),
        CodecError::MalformedStructuralFoundation("pcc sidecar profiles must be non-empty"),
    );

    let mut mutated = encoded.clone();
    mutated[spans.semantic_profile.text.start] = 0xFF;
    rejects_at_decode(
        "non-UTF-8 semantic profile bytes",
        mutated,
        CodecError::InvalidUtf8("pcc semantic profile"),
    );

    for (name, count) in [
        ("a zero guarantee count", 0_u32),
        ("an over-ceiling guarantee count", 4_097),
    ] {
        let mut mutated = encoded.clone();
        mutated[spans.guarantee_count.clone()].copy_from_slice(&count.to_le_bytes());
        rejects_at_decode(
            name,
            mutated,
            CodecError::CollectionTooLong("pcc guarantees"),
        );
    }

    let mut mutated = encoded.clone();
    mutated[spans.guarantees[0].premise_count.clone()].copy_from_slice(&4_097_u32.to_le_bytes());
    rejects_at_decode(
        "an over-ceiling premise count",
        mutated,
        CodecError::CollectionTooLong("pcc premises"),
    );

    rejects_at_decode(
        "an empty guarantee identity on the wire",
        splice(
            &encoded,
            &spans.guarantees[0].identity.whole(),
            &0_u32.to_le_bytes(),
        ),
        CodecError::MalformedStructuralFoundation("pcc guarantee identity must be non-empty"),
    );

    let mut mutated = encoded.clone();
    mutated[spans.evidence_len.clone()].copy_from_slice(&u64::MAX.to_le_bytes());
    rejects_at_decode(
        "an over-ceiling evidence length",
        mutated,
        CodecError::CollectionTooLong("pcc sidecar evidence"),
    );

    let mut mutated = encoded.clone();
    mutated[spans.assumption_count.clone()].copy_from_slice(&4_097_u32.to_le_bytes());
    rejects_at_decode(
        "an over-ceiling assumption count",
        mutated,
        CodecError::CollectionTooLong("pcc assumptions"),
    );

    let mut mutated = encoded.clone();
    mutated[spans.dependency_count.clone()].copy_from_slice(&4_097_u32.to_le_bytes());
    rejects_at_decode(
        "an over-ceiling dependency count",
        mutated,
        CodecError::CollectionTooLong("pcc dependencies"),
    );

    rejects_at_decode(
        "an empty dependency identity on the wire",
        splice(
            &encoded,
            &spans.dependencies[0].identity.whole(),
            &0_u32.to_le_bytes(),
        ),
        CodecError::MalformedStructuralFoundation("pcc dependency identity must be non-empty"),
    );

    // Evidence bytes and dependency commitments are opaque on the wire: a
    // substitution there still decodes canonically and rejects at the replay
    // join that owns it. The evidence length prefix must grow with the bytes
    // or the wire desynchronizes before any claim field is read.
    let mut evidence_field = Vec::with_capacity(9);
    evidence_field.extend_from_slice(&1_u64.to_le_bytes());
    evidence_field.push(0x42);
    let mutated = splice(
        &encoded,
        &(spans.evidence_len.start..spans.evidence.end),
        &evidence_field,
    );
    let decoded = PccProofSidecar::from_bytes(&mutated).expect("wire evidence bytes still decode");
    assert_ne!(decoded, honest);
    assert_eq!(
        rejecting_subject(verify_psi_proof_sidecar(&artifact_bytes, &mutated, &policy)),
        "psi evidence",
        "wire-level evidence substitution must reject at the Psi evidence join"
    );

    let mut mutated = encoded.clone();
    mutated[spans.dependencies[0].commitment.start] ^= 0xFF;
    let decoded =
        PccProofSidecar::from_bytes(&mutated).expect("a wire dependency commitment still decodes");
    assert_ne!(decoded, honest);
    assert_eq!(
        rejecting_subject(verify_psi_proof_sidecar(&artifact_bytes, &mutated, &policy)),
        "dependency",
        "a wire-level dependency commitment substitution must reject at possession"
    );

    // A roster reorder on the wire is non-canonical even when every row is
    // otherwise well-formed. The rich fixture carries two of each roster so
    // each order join is reachable.
    let rich = SidecarSpec {
        guarantees: vec![
            PccGuarantee {
                identity: "test::guarantee-a".into(),
                premises: vec!["test::premise-a".into(), "test::premise-b".into()],
            },
            PccGuarantee {
                identity: "test::guarantee-b".into(),
                premises: vec!["test::premise-c".into()],
            },
        ],
        assumptions: vec!["test::assumption-a".into(), "test::assumption-b".into()],
        dependencies: vec![
            PccDependency {
                identity: "test::dependency-a".into(),
                content_commitment: [0xA1; 32],
            },
            PccDependency {
                identity: "test::dependency-b".into(),
                content_commitment: [0xA2; 32],
            },
        ],
        ..honest_spec.clone()
    }
    .build()
    .expect("the rich sidecar constructs canonically");
    let rich_encoded = rich.to_bytes();
    let rich_spans = sidecar_spans(&rich_encoded);
    assert_eq!(rich_spans.end, rich_encoded.len());
    assert_eq!(rich_spans.guarantees.len(), 2);
    assert_eq!(rich_spans.guarantees[0].premises.len(), 2);
    assert_eq!(rich_spans.assumptions.len(), 2);
    assert_eq!(rich_spans.dependencies.len(), 2);

    rejects_at_decode(
        "a reordered guarantee roster",
        swap(
            &rich_encoded,
            &rich_spans.guarantees[0].whole,
            &rich_spans.guarantees[1].whole,
        ),
        CodecError::NonCanonicalOrder("pcc guarantees"),
    );

    rejects_at_decode(
        "a reordered premise roster",
        swap(
            &rich_encoded,
            &rich_spans.guarantees[0].premises[0].whole(),
            &rich_spans.guarantees[0].premises[1].whole(),
        ),
        CodecError::NonCanonicalOrder("pcc premises"),
    );

    rejects_at_decode(
        "a reordered assumption roster",
        swap(
            &rich_encoded,
            &rich_spans.assumptions[0].whole(),
            &rich_spans.assumptions[1].whole(),
        ),
        CodecError::NonCanonicalOrder("pcc assumptions"),
    );

    rejects_at_decode(
        "a reordered dependency roster",
        swap(
            &rich_encoded,
            &rich_spans.dependencies[0].whole,
            &rich_spans.dependencies[1].whole,
        ),
        CodecError::NonCanonicalOrder("pcc dependencies"),
    );

    // --- truncation and trailing bytes reject at decoding ---

    for cut in [
        spans.magic.end - 1,
        spans.format_marker.end - 1,
        spans.product.end,
        spans.commitment.end - 1,
        spans.semantic_profile.text.end - 1,
        spans.checker_profile.text.end,
        spans.guarantee_count.end,
        spans.guarantees[0].whole.end - 1,
        spans.evidence_len.end,
        spans.assumption_count.end,
        spans.dependency_count.end,
        encoded.len() - 1,
    ] {
        assert!(
            PccProofSidecar::from_bytes(&encoded[..cut]).is_err(),
            "truncation at byte {cut} must reject"
        );
    }

    let mut mutated = encoded.clone();
    mutated.push(0);
    rejects_at_decode("trailing bytes", mutated, CodecError::TrailingBytes(1));
}
