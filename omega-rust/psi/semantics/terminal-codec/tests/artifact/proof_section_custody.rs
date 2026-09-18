//! One-field mutation coverage for the sealed canonical proof section.
//!
//! The `PSIPSC` section binds a canonical `PSIPRF` proof bundle to the exact
//! reconstructed semantic subject it was admitted for; the artifact manifest
//! in turn binds the bundle fingerprint. Its wire fields — the section magic,
//! format marker, vocabulary marker, subject fingerprint, bundle framing, the
//! counted obligation-evidence roster (obligation identity plus each evidence
//! route payload), the grouped recursive-component certificates (component
//! identity, certificate identity, ranking relation, well-foundedness route,
//! and each decrease-edge route), the counted control-cycle certificates, and
//! the counted evidence-producer provenance roster (dense identity, term,
//! conformance and trait identities, and each realization row) — are each
//! substituted independently. A substitution either fails canonical decoding
//! or encoding, or decodes to a different bundle whose honestly recomputed
//! artifact identity diverges and whose replay against the retained manifest
//! and the terminal verifier rejects.
//!
//! The fixture module declares no control cycle, so every control-cycle
//! certificate is surplus authority: the roster is representable and identity
//! binding is exercised per field while replay rejects the unearned row.
//! The same holds for `Admitted` evidence routes inside a derivable
//! obligation: the substitution is representable and identity-bound while the
//! verifier refuses an admission where a derivation exists.

use std::ops::Range;

use super::{
    canonical_artifact, evidence_id, evidence_term_id, i32_type, machine_id, obligation_id,
    proof_recursive_component, proof_recursive_evidence, proposition_id, semantic_module,
};
use proof_admission::{
    AdmissionEvidence, AdmissionKind, AdmissionProfile, CertificateEnvelope, EvidenceRoute,
    PrimitiveJudgment, ProofNode, ProofRule, ProofSystemMarker, RecursiveComponentCertificate,
    RecursiveEdgeCertificate,
};
use semantic_vocabulary::{
    AdmissionSiteId, CycleComponentId, IntegerValue, ProfileDecisionId, Proposition,
    PropositionError, RankingRelationId, RecursiveComponentId, ScalarTerm,
};
use terminal_codec::{
    ArtifactManifestError, CanonicalTerminalArtifact, CanonicalTerminalArtifactError,
    ProofCodecError, build_artifact_manifest, build_identity_optimization_execution_record,
    decode_proof_section, decode_proof_section_for, encode_proof_section, proof_bundle_fingerprint,
    validate_artifact_manifest, verify_terminal_artifact_proof,
};
use terminal_psi::{
    ContractClause, EvidenceContractLane, EvidenceContractLaneKind, EvidenceInterfaceIdentity,
    EvidenceRequirementIdentity, EvidenceTermDeclaration, PropositionApplicationIdentity,
    PropositionDeclaration, PropositionEvidence, TerminalModule,
};
use terminal_verifier::{
    ControlCycleEvidence, EvidenceProducerProvenance, EvidenceProducerRealization,
    EvidenceProducerRowSource, ObligationEvidence, ProofBundle, RecursiveComponentEvidence,
    verify_module,
};

/// Byte offsets of every wire field inside the canonical sealed section.
struct SectionSpans {
    magic: Range<usize>,
    format_marker: Range<usize>,
    vocabulary: Range<usize>,
    fingerprint: Range<usize>,
    bundle_magic: Range<usize>,
    bundle_marker: Range<usize>,
    evidence_count: Range<usize>,
    evidence: Vec<EvidenceSpan>,
    component_count: Range<usize>,
    components: Vec<ComponentSpan>,
    cycle_count: Range<usize>,
    producer_count: Range<usize>,
    producers: Vec<ProducerSpan>,
    end: usize,
}

struct EvidenceSpan {
    row: Range<usize>,
    obligation: Range<usize>,
    route_tag: Range<usize>,
    /// `KernelDerived` judgment byte.
    judgment: Option<Range<usize>>,
    /// `CertificateDerived` envelope fields and proof tree.
    certificate: Option<CertificateSpan>,
}

struct CertificateSpan {
    identity: Range<usize>,
    marker: Range<usize>,
    node: NodeSpan,
}

struct NodeSpan {
    /// The proposition tag byte alone.
    conclusion_tag: Range<usize>,
    rule_tag: Range<usize>,
}

struct ComponentSpan {
    row: Range<usize>,
    component: Range<usize>,
    edge_count: Range<usize>,
}

struct ProducerSpan {
    row: Range<usize>,
    id: Range<usize>,
    term: Range<usize>,
    conformance_len: Range<usize>,
    conformance: Range<usize>,
    row_count: Range<usize>,
    rows: Vec<ProducerRowSpan>,
}

struct ProducerRowSpan {
    row: Range<usize>,
    source: Range<usize>,
}

/// A cursor that walks the sealed section exactly as the canonical decoder
/// does, recording the byte span of every field the matrix substitutes. The
/// fixture only exercises the proposition, term, rule and route tags handled
/// below; an unexpected tag fails the walk rather than silently spanning
/// unknown bytes.
struct SpanWalker<'bytes> {
    bytes: &'bytes [u8],
    offset: usize,
}

impl<'bytes> SpanWalker<'bytes> {
    const fn new(bytes: &'bytes [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, len: usize) -> Range<usize> {
        let start = self.offset;
        self.offset += len;
        start..self.offset
    }

    fn u32_at(&self, span: Range<usize>) -> u32 {
        u32::from_le_bytes(
            self.bytes[span]
                .try_into()
                .expect("u32 field inside the section"),
        )
    }

    fn take_count(&mut self) -> (Range<usize>, u32) {
        let span = self.take(4);
        let count = self.u32_at(span.clone());
        (span, count)
    }

    /// Skip one encoded scalar term. The fixture carries `Value` and
    /// `Integer` leaves only.
    fn skip_scalar_term(&mut self) {
        match self.bytes[self.offset] {
            // Value: tag, u64 identity, scalar type.
            1 => {
                self.take(9);
                self.skip_scalar_type();
            }
            // Integer literal: tag, integer type, integer value.
            3 => {
                self.take(1);
                self.skip_integer_type();
                match self.bytes[self.offset] {
                    1 | 2 => {
                        self.take(1 + 16);
                    }
                    tag => panic!("unexpected integer value tag {tag}"),
                }
            }
            tag => panic!("unexpected scalar term tag {tag}"),
        }
    }

    fn skip_integer_type(&mut self) {
        self.take(3);
    }

    fn skip_scalar_type(&mut self) {
        match self.bytes[self.offset] {
            1 => {
                self.take(1);
            }
            2 => {
                self.take(1);
                self.skip_integer_type();
            }
            tag => panic!("unexpected scalar type tag {tag}"),
        }
    }

    /// Skip one encoded proposition. The fixture carries `Truth`, `Atom`,
    /// `Equal` and `Conjunction` shapes only.
    fn skip_proposition(&mut self) {
        match self.bytes[self.offset] {
            1 | 2 => {
                self.take(1);
            }
            3 => {
                self.take(9);
            }
            4..=6 => {
                self.take(1);
                self.skip_scalar_term();
                self.skip_scalar_term();
            }
            7 | 10 => {
                self.take(1);
                let (_, count) = self.take_count();
                for _ in 0..count {
                    self.skip_proposition();
                }
            }
            tag => panic!("unexpected proposition tag {tag}"),
        }
    }

    /// Walk one encoded proof node: conclusion proposition, rule tag,
    /// children, then the trailing rule suffix.
    fn walk_proof_node(&mut self) -> NodeSpan {
        let conclusion_tag = self.take(1);
        match self.bytes[conclusion_tag.start] {
            1 | 2 => {}
            3 => {
                self.take(8);
            }
            4..=6 => {
                self.skip_scalar_term();
                self.skip_scalar_term();
            }
            7 | 10 => {
                let (_, count) = self.take_count();
                for _ in 0..count {
                    self.skip_proposition();
                }
            }
            tag => panic!("unexpected proposition tag {tag}"),
        }
        let rule_tag = self.take(1);
        let (children, suffix_len): (usize, usize) = match self.bytes[rule_tag.start] {
            1..=3 => (
                0,
                match self.bytes[rule_tag.start] {
                    1 => 1,
                    _ => 4,
                },
            ),
            4 => {
                let (_, count) = self.take_count();
                (usize::try_from(count).expect("child count fits usize"), 0)
            }
            8 => (2, 0),
            tag => panic!("unexpected proof rule tag {tag}"),
        };
        for _ in 0..children {
            self.walk_proof_node();
        }
        self.take(suffix_len);
        NodeSpan {
            conclusion_tag,
            rule_tag,
        }
    }

    /// Walk one encoded evidence route payload (route tag plus payload). The
    /// well-foundedness leg of a component certificate and each decrease edge
    /// carry a bare route, not a full evidence row.
    fn walk_route(&mut self) -> (Range<usize>, Option<Range<usize>>, Option<CertificateSpan>) {
        let route_tag = self.take(1);
        let mut judgment = None;
        let mut certificate = None;
        match self.bytes[route_tag.start] {
            1 => {
                judgment = Some(self.take(1));
            }
            2 => {
                let identity = self.take(8);
                let marker = self.take(2);
                let node = self.walk_proof_node();
                certificate = Some(CertificateSpan {
                    identity,
                    marker,
                    node,
                });
            }
            3 => {
                // site, kind, authority, evidence identity, profile decision.
                self.take(8 + 1 + 8 + 8 + 8);
            }
            tag => panic!("unexpected evidence route tag {tag}"),
        }
        (route_tag, judgment, certificate)
    }

    /// Walk one encoded evidence row (obligation identity plus route).
    fn walk_evidence(&mut self) -> EvidenceSpan {
        let row_start = self.offset;
        let obligation = self.take(8);
        let (route_tag, judgment, certificate) = self.walk_route();
        EvidenceSpan {
            row: row_start..self.offset,
            obligation,
            route_tag,
            judgment,
            certificate,
        }
    }

    /// Walk one encoded component certificate body and return its edge-count
    /// span for the count-mutation legs.
    fn walk_certificate(&mut self) -> Range<usize> {
        self.take(8); // certificate identity
        self.take(8); // ranking relation
        self.walk_route(); // well-foundedness route
        let (edge_count, edges) = self.take_count();
        for _ in 0..edges {
            self.take(8); // edge obligation identity
            self.walk_route();
        }
        edge_count
    }

    fn take_string(&mut self) -> (Range<usize>, Range<usize>) {
        let (len_span, len) = self.take_count();
        let value = self.take(usize::try_from(len).expect("string length fits usize"));
        (len_span, value)
    }
}

fn section_spans(encoded: &[u8]) -> SectionSpans {
    let mut walker = SpanWalker::new(encoded);
    let magic = walker.take(8);
    let format_marker = walker.take(2);
    let vocabulary = walker.take(2);
    let fingerprint = walker.take(32);
    let bundle_magic = walker.take(8);
    let bundle_marker = walker.take(2);
    let (evidence_count, evidence_rows) = walker.take_count();
    let mut evidence = Vec::with_capacity(usize::try_from(evidence_rows).expect("evidence fits"));
    for _ in 0..evidence_rows {
        evidence.push(walker.walk_evidence());
    }
    let (component_count, component_rows) = walker.take_count();
    let mut components =
        Vec::with_capacity(usize::try_from(component_rows).expect("components fit"));
    for _ in 0..component_rows {
        let row_start = walker.offset;
        let component = walker.take(8);
        let edge_count = walker.walk_certificate();
        components.push(ComponentSpan {
            row: row_start..walker.offset,
            component,
            edge_count,
        });
    }
    let (cycle_count, cycle_rows) = walker.take_count();
    assert_eq!(cycle_rows, 0, "the fixture carries no control-cycle rows");
    let (producer_count, producer_rows) = walker.take_count();
    let mut producers = Vec::with_capacity(usize::try_from(producer_rows).expect("producers fit"));
    for _ in 0..producer_rows {
        let row_start = walker.offset;
        let id = walker.take(8);
        let term = walker.take(8);
        let (conformance_len, conformance) = walker.take_string();
        walker.take_string(); // evidence trait identity
        let (row_count, rows) = walker.take_count();
        let mut row_spans = Vec::with_capacity(usize::try_from(rows).expect("rows fit"));
        for _ in 0..rows {
            let producer_row_start = walker.offset;
            walker.take_string(); // declaring trait identity
            let (_argument_count, arguments) = walker.take_count();
            for _ in 0..arguments {
                walker.take_string();
            }
            walker.take_string(); // requirement identity
            walker.take_string(); // realization machine identity
            walker.take_string(); // realization state identity
            let source = walker.take(1);
            row_spans.push(ProducerRowSpan {
                row: producer_row_start..walker.offset,
                source,
            });
        }
        producers.push(ProducerSpan {
            row: row_start..walker.offset,
            id,
            term,
            conformance_len,
            conformance,
            row_count,
            rows: row_spans,
        });
    }
    SectionSpans {
        magic,
        format_marker,
        vocabulary,
        fingerprint,
        bundle_magic,
        bundle_marker,
        evidence_count,
        evidence,
        component_count,
        components,
        cycle_count,
        producer_count,
        producers,
        end: walker.offset,
    }
}

/// One module whose proof surface exercises every bundle roster: three
/// contract obligations discharged by all three wire-level route kinds, one
/// verifier-reconstructed recursive component, and one evidence term whose
/// producer provenance is mandatory.
fn custody_module() -> TerminalModule {
    let mut module = semantic_module();
    let contract = &mut module.machines[0].contract;
    let integer = i32_type();
    let literal = ScalarTerm::integer(integer, IntegerValue::Signed(7)).expect("i32 literal");
    contract.ensures.push(ContractClause {
        obligation: obligation_id(2),
        proposition: Proposition::Conjunction(vec![
            Proposition::Truth,
            Proposition::Equal(literal.clone(), literal),
        ]),
    });
    contract.ensures.push(ContractClause {
        obligation: obligation_id(3),
        proposition: Proposition::Truth,
    });
    module.proof_recursive_components = vec![proof_recursive_component()];
    let interface = EvidenceInterfaceIdentity {
        trait_identity: "package::Evidence".to_owned(),
        arguments: Vec::new(),
        requirements: vec![EvidenceRequirementIdentity {
            declaring_trait_identity: "package::Evidence".to_owned(),
            declaring_trait_arguments: Vec::new(),
            requirement_identity: "package::Evidence::run".to_owned(),
        }],
    };
    module.proposition_declarations = vec![PropositionDeclaration {
        id: proposition_id(1),
        name: "package::Produces".to_owned(),
        binders: Vec::new(),
        parameter_types: Vec::new(),
        evidence: PropositionEvidence::Witness {
            evidence_type: "package::Evidence".to_owned(),
        },
    }];
    module.proposition_applications = vec![PropositionApplicationIdentity {
        id: proposition_id(1),
        declaration: proposition_id(1),
        binder_arguments: Vec::new(),
        arguments: Vec::new(),
        evidence_interface: Some(interface.clone()),
    }];
    module.evidence_terms = vec![EvidenceTermDeclaration {
        id: evidence_term_id(1),
        proposition: proposition_id(1),
        interface,
    }];
    module.evidence_contract_lanes = vec![EvidenceContractLane {
        machine: machine_id(1),
        kind: EvidenceContractLaneKind::Ensures,
        position: 0,
        term: evidence_term_id(1),
        output_field: Some("package::main::evidence".to_owned()),
    }];
    module
}

fn custody_bundle(module: &TerminalModule) -> ProofBundle {
    let integer = i32_type();
    let literal = ScalarTerm::integer(integer, IntegerValue::Signed(7)).expect("i32 literal");
    let goal = Proposition::Equal(literal.clone(), literal.clone());
    let leaf = |rule| ProofNode {
        conclusion: Proposition::Truth,
        rule,
    };
    ProofBundle {
        evidence: vec![
            ObligationEvidence {
                obligation: obligation_id(1),
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: evidence_id(11),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: ProofNode {
                        conclusion: goal.clone(),
                        rule: ProofRule::EqualityTransitivity {
                            left_equals_middle: Box::new(ProofNode {
                                conclusion: goal.clone(),
                                rule: ProofRule::Assumption { index: 0 },
                            }),
                            middle_equals_right: Box::new(ProofNode {
                                conclusion: goal,
                                rule: ProofRule::Assumption { index: 0 },
                            }),
                        },
                    },
                }),
            },
            ObligationEvidence {
                obligation: obligation_id(2),
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: evidence_id(12),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: ProofNode {
                        conclusion: Proposition::Conjunction(vec![
                            Proposition::Truth,
                            Proposition::Equal(literal.clone(), literal.clone()),
                        ]),
                        rule: ProofRule::ConjunctionIntroduction(vec![
                            leaf(ProofRule::Primitive(PrimitiveJudgment::Truth)),
                            ProofNode {
                                conclusion: Proposition::Equal(literal.clone(), literal.clone()),
                                rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
                            },
                        ]),
                    },
                }),
            },
            ObligationEvidence {
                obligation: obligation_id(3),
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
            },
        ],
        recursive_components: proof_recursive_evidence(module),
        control_cycles: Vec::new(),
        evidence_producers: vec![EvidenceProducerProvenance {
            id: evidence_id(1),
            term: evidence_term_id(1),
            conformance_identity: "package::conformance".to_owned(),
            evidence_trait_identity: "package::Evidence".to_owned(),
            rows: vec![EvidenceProducerRealization {
                declaring_trait_identity: "package::Evidence".to_owned(),
                declaring_trait_arguments: Vec::new(),
                requirement_identity: "package::Evidence::run".to_owned(),
                realization_machine_identity: "package::producer".to_owned(),
                realization_state_identity: "package::producer::entry".to_owned(),
                source: EvidenceProducerRowSource::Inline,
            }],
        }],
    }
}

#[test]
fn sealed_proof_section_rejects_every_one_field_substitution() {
    let module = custody_module();
    let bundle = custody_bundle(&module);
    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("the fixture module verifies under its proof bundle");
    let section = encode_proof_section(&module, &bundle).expect("sealed proof section");
    let spans = section_spans(&section);
    assert_eq!(section.len(), spans.end, "span map must cover the section");
    assert_eq!(
        decode_proof_section_for(&module, &section),
        Ok(bundle.clone()),
        "the canonical section round-trips under its own subject"
    );
    let (claimed, decoded) = decode_proof_section(&section).expect("subject-paired decode");
    assert_eq!(decoded, bundle);

    // The retained artifact binds the bundle fingerprint into its manifest:
    // replaying that manifest against a substituted section is the identity
    // join for every representable field, and `verify_module` is the semantic
    // replay leg.
    let artifact = canonical_artifact(&module, &bundle, None);
    let retained = artifact.manifest();
    assert_eq!(retained.proof(), artifact.manifest().proof());
    assert_eq!(
        retained.proof(),
        proof_bundle_fingerprint(&bundle).expect("honest bundle fingerprint"),
    );

    // A substitution that still forms a canonical bundle reseals under the
    // same subject, honestly recomputes a divergent artifact identity, and is
    // rejected by the retained-manifest replay. `verifies` records whether
    // the semantic replay also rejects the mutated bundle: provenance fields
    // such as certificate identities are admitted by the verifier but still
    // diverge the bound identity.
    let representable = |name: &'static str, mutated: &ProofBundle, verifies: bool| {
        let mutated_section = encode_proof_section(&module, mutated)
            .unwrap_or_else(|error| panic!("{name} must still encode: {error:?}"));
        assert_ne!(mutated_section, section, "{name} must change the section");
        assert_eq!(
            decode_proof_section_for(&module, &mutated_section),
            Ok(mutated.clone()),
            "{name} must still decode under the same subject"
        );
        let recomputed_optimization =
            build_identity_optimization_execution_record(&module, mutated)
                .expect("identity optimization over the substituted bundle");
        let recomputed =
            build_artifact_manifest(&module, mutated, &recomputed_optimization, None, None)
                .expect("honest manifest over the substituted bundle");
        assert_ne!(
            recomputed.identity(),
            retained.identity(),
            "{name} must diverge the recomputed artifact identity"
        );
        assert_eq!(
            validate_artifact_manifest(
                &module,
                mutated,
                &recomputed_optimization,
                None,
                None,
                retained,
            ),
            Err(ArtifactManifestError::ManifestMismatch),
            "{name} must reject at the retained-manifest replay"
        );
        assert_eq!(
            verify_module(&module, mutated, &AdmissionProfile::default()).is_ok(),
            verifies,
            "{name} has the wrong semantic-replay outcome"
        );
    };
    // A substitution that cannot be represented canonically rejects inside
    // the sealed-section encoder's canonical-order validation.
    let encode_rejected = |name: &'static str, mutated: &ProofBundle, expected: ProofCodecError| {
        assert_eq!(
            encode_proof_section(&module, mutated),
            Err(expected),
            "{name} must reject at canonical encoding"
        );
    };
    // A substitution expressible only on the wire rejects at the
    // subject-paired canonical decode.
    let rejected = |name: &'static str, mutated: &[u8], expected: ProofCodecError| {
        assert_eq!(
            decode_proof_section_for(&module, mutated),
            Err(expected),
            "{name} must reject at canonical decoding"
        );
    };
    let rejected_unspecified = |name: &'static str, mutated: &[u8]| {
        assert!(
            decode_proof_section_for(&module, mutated).is_err(),
            "{name} must reject at canonical decoding"
        );
    };

    // --- sealed subject envelope: magic, markers, vocabulary and the
    // program fingerprint bind the section to this exact module ---

    let mut mutated = section.clone();
    mutated[spans.magic.start] ^= 0xFF;
    rejected("the section magic", &mutated, ProofCodecError::InvalidMagic);

    let mut mutated = section.clone();
    mutated[spans.format_marker.clone()].copy_from_slice(&u16::MAX.to_le_bytes());
    rejected(
        "the section format marker",
        &mutated,
        ProofCodecError::UnsupportedFormatMarker(u16::MAX),
    );

    let mut mutated = section.clone();
    mutated[spans.vocabulary.clone()].copy_from_slice(&u16::MAX.to_le_bytes());
    rejected(
        "the subject vocabulary marker",
        &mutated,
        ProofCodecError::UnsupportedProofSectionVocabulary(u16::MAX),
    );

    let mut mutated = section.clone();
    mutated[spans.fingerprint.start] ^= 0xFF;
    assert!(
        matches!(
            decode_proof_section_for(&module, &mutated),
            Err(ProofCodecError::ProofSubjectMismatch { .. })
        ),
        "a foreign program fingerprint must reject at decoding"
    );

    // --- bundle framing ---

    let mut mutated = section.clone();
    mutated[spans.bundle_magic.start] ^= 0xFF;
    rejected("the bundle magic", &mutated, ProofCodecError::InvalidMagic);

    let mut mutated = section.clone();
    mutated[spans.bundle_marker.clone()].copy_from_slice(&u16::MAX.to_le_bytes());
    rejected(
        "the bundle format marker",
        &mutated,
        ProofCodecError::UnsupportedFormatMarker(u16::MAX),
    );

    let mut trailing = section.clone();
    trailing.push(0);
    rejected(
        "a trailing byte",
        &trailing,
        ProofCodecError::TrailingBytes(1),
    );

    for cut in [
        spans.magic.end - 1,
        spans.format_marker.end - 1,
        spans.vocabulary.end - 1,
        spans.fingerprint.end - 1,
        spans.bundle_magic.end - 1,
        spans.bundle_marker.end - 1,
        spans.evidence_count.end - 1,
        spans.evidence[0].row.end - 1,
        spans.component_count.end - 1,
        spans.components[0].row.end - 1,
        spans.cycle_count.end - 1,
        spans.producer_count.end - 1,
        spans.producers[0].row.end - 1,
        spans.end - 1,
    ] {
        rejected_unspecified("a truncated section", &section[..cut]);
    }

    // --- obligation evidence roster: obligation identities are order-bound;
    // every route payload is independently representable ---

    // An earlier row's obligation cannot grow without breaking roster order.
    let mut changed = bundle.clone();
    changed.evidence[0].obligation = obligation_id(4);
    encode_rejected(
        "an obligation identity breaking roster order",
        &changed,
        ProofCodecError::NonCanonicalEvidenceOrder,
    );
    let mut changed = bundle.clone();
    changed.evidence[2].obligation = obligation_id(4);
    representable("the last obligation's identity", &changed, false);

    // Route-kind substitutions are representable: a derivable obligation
    // still rejects an `Admitted` route, and a kernel judgment that cannot
    // prove the conclusion is refused by the checker. An equally valid
    // derivation keeps the semantic verdict but still diverges the identity.
    let mut changed = bundle.clone();
    changed.evidence[0].route =
        EvidenceRoute::KernelDerived(PrimitiveJudgment::ClosedIntegerRelation);
    representable(
        "a certificate-to-kernel route of equal strength",
        &changed,
        true,
    );
    let mut changed = bundle.clone();
    changed.evidence[0].route = EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth);
    representable(
        "a certificate-to-kernel route that cannot prove",
        &changed,
        false,
    );
    let mut changed = bundle.clone();
    changed.evidence[2].route = EvidenceRoute::KernelDerived(PrimitiveJudgment::ReflexiveEquality);
    representable("a kernel judgment that cannot prove", &changed, false);
    let admitted = AdmissionEvidence {
        site: AdmissionSiteId::new(21).expect("site identity"),
        kind: AdmissionKind::ProviderFact,
        authority_identity: evidence_id(22),
        evidence_identity: evidence_id(23),
        profile_decision: ProfileDecisionId::new(24).expect("decision identity"),
    };
    let mut changed = bundle.clone();
    changed.evidence[2].route = EvidenceRoute::Admitted(admitted);
    representable("a kernel-to-admitted route", &changed, false);
    let admitted_fields: [(&'static str, Box<dyn Fn(&mut AdmissionEvidence)>); 5] = [
        (
            "an admitted site",
            Box::new(|evidence| {
                evidence.site = AdmissionSiteId::new(25).expect("site identity");
            }),
        ),
        (
            "an admitted kind",
            Box::new(|evidence| {
                evidence.kind = AdmissionKind::CheckedAssemblyClaim;
            }),
        ),
        (
            "an admitted authority",
            Box::new(|evidence| {
                evidence.authority_identity = evidence_id(26);
            }),
        ),
        (
            "an admitted evidence identity",
            Box::new(|evidence| {
                evidence.evidence_identity = evidence_id(27);
            }),
        ),
        (
            "an admitted profile decision",
            Box::new(|evidence| {
                evidence.profile_decision = ProfileDecisionId::new(28).expect("decision identity");
            }),
        ),
    ];
    for (name, mutate) in admitted_fields {
        let mut changed = bundle.clone();
        let mut substitution = admitted;
        mutate(&mut substitution);
        changed.evidence[2].route = EvidenceRoute::Admitted(substitution);
        representable(name, &changed, false);
    }

    // Certificate envelope: the evidence identity is producer provenance
    // admitted by the verifier, while the marker has no second canonical
    // value and rejects on the wire.
    let mut changed = bundle.clone();
    let EvidenceRoute::CertificateDerived(envelope) = &mut changed.evidence[0].route else {
        unreachable!()
    };
    envelope.identity = evidence_id(91);
    representable("a certificate's evidence identity", &changed, true);

    let mut mutated = section.clone();
    mutated[spans.evidence[0]
        .certificate
        .as_ref()
        .unwrap()
        .marker
        .clone()]
    .copy_from_slice(&u16::MAX.to_le_bytes());
    rejected(
        "a certificate's proof-system marker",
        &mutated,
        ProofCodecError::UnsupportedProofSystemMarker(u16::MAX),
    );
    let mut mutated = section.clone();
    mutated[spans.evidence[0]
        .certificate
        .as_ref()
        .unwrap()
        .identity
        .clone()]
    .copy_from_slice(&0_u64.to_le_bytes());
    rejected(
        "a zero certificate identity",
        &mutated,
        ProofCodecError::ZeroIdentity("EvidenceIdentity"),
    );

    // Proof node fields: the conclusion proposition is checked against the
    // obligation, the rule must derive it, and every rule payload is bound.
    let mut changed = bundle.clone();
    let EvidenceRoute::CertificateDerived(envelope) = &mut changed.evidence[0].route else {
        unreachable!()
    };
    envelope.proof.conclusion = Proposition::Truth;
    representable("a certificate's conclusion proposition", &changed, false);
    let mut changed = bundle.clone();
    let EvidenceRoute::CertificateDerived(envelope) = &mut changed.evidence[0].route else {
        unreachable!()
    };
    let Proposition::Equal(_, right) = &mut envelope.proof.conclusion else {
        unreachable!()
    };
    *right = ScalarTerm::integer(i32_type(), IntegerValue::Signed(8)).expect("i32 literal");
    representable("a conclusion's equality operand", &changed, false);
    let mut changed = bundle.clone();
    let EvidenceRoute::CertificateDerived(envelope) = &mut changed.evidence[0].route else {
        unreachable!()
    };
    envelope.proof.rule = ProofRule::Primitive(PrimitiveJudgment::Truth);
    representable("a certificate's rule kind", &changed, false);
    let mut changed = bundle.clone();
    let EvidenceRoute::CertificateDerived(envelope) = &mut changed.evidence[0].route else {
        unreachable!()
    };
    let ProofRule::EqualityTransitivity {
        left_equals_middle, ..
    } = &mut envelope.proof.rule
    else {
        unreachable!()
    };
    left_equals_middle.rule = ProofRule::Assumption { index: 9 };
    representable("a child's assumption index", &changed, false);
    let mut changed = bundle.clone();
    let EvidenceRoute::CertificateDerived(envelope) = &mut changed.evidence[0].route else {
        unreachable!()
    };
    let ProofRule::EqualityTransitivity {
        left_equals_middle, ..
    } = &mut envelope.proof.rule
    else {
        unreachable!()
    };
    left_equals_middle.conclusion = Proposition::Falsehood;
    representable("a child's conclusion", &changed, false);

    let mut changed = bundle.clone();
    let EvidenceRoute::CertificateDerived(envelope) = &mut changed.evidence[1].route else {
        unreachable!()
    };
    let ProofRule::ConjunctionIntroduction(children) = &mut envelope.proof.rule else {
        unreachable!()
    };
    children.pop();
    representable("a dropped conjunction child", &changed, false);
    let mut changed = bundle.clone();
    let EvidenceRoute::CertificateDerived(envelope) = &mut changed.evidence[1].route else {
        unreachable!()
    };
    let ProofRule::ConjunctionIntroduction(children) = &mut envelope.proof.rule else {
        unreachable!()
    };
    children[0].rule = ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality);
    representable("a leaf's primitive judgment", &changed, false);
    let mut changed = bundle.clone();
    let EvidenceRoute::CertificateDerived(envelope) = &mut changed.evidence[1].route else {
        unreachable!()
    };
    envelope.proof.conclusion = Proposition::Conjunction(vec![Proposition::Truth]);
    encode_rejected(
        "a shrunken conjunction conclusion",
        &changed,
        ProofCodecError::MalformedProposition(PropositionError::NonCanonicalConjunctionArity(1)),
    );

    // Roster shape: dropping or inserting a row is representable but the
    // verifier's reconstructed obligation set refuses either direction;
    // reordered and duplicated rows reject at canonical encoding.
    let mut changed = bundle.clone();
    changed.evidence.pop();
    representable("a dropped evidence row", &changed, false);
    let mut changed = bundle.clone();
    changed.evidence.push(ObligationEvidence {
        obligation: obligation_id(9),
        route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
    });
    representable("an inserted evidence row", &changed, false);
    let mut changed = bundle.clone();
    changed.evidence.swap(1, 2);
    encode_rejected(
        "a reordered evidence roster",
        &changed,
        ProofCodecError::NonCanonicalEvidenceOrder,
    );
    let mut changed = bundle.clone();
    changed.evidence[1].obligation = obligation_id(1);
    encode_rejected(
        "a duplicated evidence obligation",
        &changed,
        ProofCodecError::NonCanonicalEvidenceOrder,
    );

    // Wire axes: identities cannot be zero, every tag is closed, and roster
    // counts cannot lie about their payload.
    let mut mutated = section.clone();
    mutated[spans.evidence[0].obligation.clone()].copy_from_slice(&0_u64.to_le_bytes());
    rejected(
        "a zero obligation identity",
        &mutated,
        ProofCodecError::ZeroIdentity("ObligationId"),
    );
    let mut mutated = section.clone();
    mutated[spans.evidence[0].route_tag.clone()].copy_from_slice(&[9]);
    rejected(
        "an unknown evidence route tag",
        &mutated,
        ProofCodecError::InvalidTag("EvidenceRoute", 9),
    );
    // Tag 3 reinterpretation lands on the certificate marker's low byte.
    let mut mutated = section.clone();
    mutated[spans.evidence[0].route_tag.clone()].copy_from_slice(&[3]);
    rejected(
        "an evidence route tag widening to Admitted",
        &mutated,
        ProofCodecError::InvalidTag("AdmissionKind", 5),
    );
    let mut mutated = section.clone();
    mutated[spans.evidence[2].judgment.clone().unwrap()].copy_from_slice(&[9]);
    rejected(
        "an unknown primitive judgment",
        &mutated,
        ProofCodecError::InvalidTag("PrimitiveJudgment", 9),
    );
    let mut mutated = section.clone();
    mutated[spans.evidence[0]
        .certificate
        .as_ref()
        .unwrap()
        .node
        .conclusion_tag
        .clone()]
    .copy_from_slice(&[0xEE]);
    rejected(
        "an unknown proposition tag",
        &mutated,
        ProofCodecError::InvalidTag("Proposition", 0xEE),
    );
    let mut mutated = section.clone();
    mutated[spans.evidence[0]
        .certificate
        .as_ref()
        .unwrap()
        .node
        .rule_tag
        .clone()]
    .copy_from_slice(&[0xEE]);
    rejected(
        "an unknown proof rule tag",
        &mutated,
        ProofCodecError::InvalidTag("ProofRule", 0xEE),
    );
    let mut mutated = section.clone();
    mutated[spans.evidence_count.clone()].copy_from_slice(&4_u32.to_le_bytes());
    rejected_unspecified("an over-counted evidence roster", &mutated);
    let mut mutated = section.clone();
    mutated[spans.evidence_count.clone()].copy_from_slice(&0_u32.to_le_bytes());
    rejected_unspecified("a cleared evidence roster", &mutated);

    // --- recursive component certificates: component identity joins the
    // reconstructed obligation; the certificate's ranking relation is
    // semantic, its identity is provenance, and every route is bound ---

    let mut changed = bundle.clone();
    changed.recursive_components[0].component =
        RecursiveComponentId::new(changed.recursive_components[0].component.get() + 1)
            .expect("nonzero component identity");
    representable("a recursive component identity", &changed, false);
    let mut changed = bundle.clone();
    changed.recursive_components[0].certificate.identity = evidence_id(9021);
    representable("a component certificate's identity", &changed, true);
    let mut changed = bundle.clone();
    changed.recursive_components[0].certificate.ranking_relation =
        RankingRelationId::new(9022).expect("nonzero ranking relation");
    representable(
        "a component certificate's ranking relation",
        &changed,
        false,
    );
    let mut changed = bundle.clone();
    changed.recursive_components[0].certificate.well_foundedness =
        EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth);
    representable("a well-foundedness route kind", &changed, false);
    let mut changed = bundle.clone();
    let EvidenceRoute::CertificateDerived(envelope) =
        &mut changed.recursive_components[0].certificate.well_foundedness
    else {
        unreachable!()
    };
    envelope.identity = evidence_id(9023);
    representable("a well-foundedness envelope identity", &changed, true);
    let mut changed = bundle.clone();
    let EvidenceRoute::CertificateDerived(envelope) =
        &mut changed.recursive_components[0].certificate.well_foundedness
    else {
        unreachable!()
    };
    envelope.proof.rule = ProofRule::SemanticAxiom { index: 9 };
    representable("a well-foundedness axiom index", &changed, false);
    let mut changed = bundle.clone();
    let EvidenceRoute::CertificateDerived(envelope) =
        &mut changed.recursive_components[0].certificate.well_foundedness
    else {
        unreachable!()
    };
    envelope.proof.conclusion = Proposition::Truth;
    representable("a well-foundedness conclusion", &changed, false);

    let mut changed = bundle.clone();
    changed.recursive_components[0].certificate.edges[0].obligation = obligation_id(9_000_001);
    representable("a decrease edge's obligation", &changed, false);
    let mut changed = bundle.clone();
    let EvidenceRoute::CertificateDerived(envelope) =
        &mut changed.recursive_components[0].certificate.edges[0].evidence
    else {
        unreachable!()
    };
    envelope.proof.rule = ProofRule::SemanticAxiom { index: 7 };
    representable("a decrease edge's axiom index", &changed, false);
    let mut changed = bundle.clone();
    let EvidenceRoute::CertificateDerived(envelope) =
        &mut changed.recursive_components[0].certificate.edges[0].evidence
    else {
        unreachable!()
    };
    envelope.identity = evidence_id(9024);
    representable("a decrease edge's envelope identity", &changed, true);
    let mut changed = bundle.clone();
    changed.recursive_components[0].certificate.edges[0].evidence =
        EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth);
    representable("a decrease edge's route kind", &changed, false);

    let mut changed = bundle.clone();
    changed.recursive_components.pop();
    representable("a dropped recursive component row", &changed, false);
    let mut changed = bundle.clone();
    let surplus = RecursiveComponentEvidence {
        component: RecursiveComponentId::new(changed.recursive_components[0].component.get() + 1)
            .expect("nonzero component identity"),
        certificate: changed.recursive_components[0].certificate.clone(),
    };
    changed.recursive_components.push(surplus);
    representable("an inserted recursive component row", &changed, false);
    let mut changed = bundle.clone();
    changed.recursive_components[0].certificate.edges.reverse();
    encode_rejected(
        "reordered component edges",
        &changed,
        ProofCodecError::NonCanonicalRecursiveComponentEvidence,
    );
    let mut changed = bundle.clone();
    changed.recursive_components[0].certificate.edges[1].obligation =
        changed.recursive_components[0].certificate.edges[0].obligation;
    encode_rejected(
        "a duplicated component edge",
        &changed,
        ProofCodecError::NonCanonicalRecursiveComponentEvidence,
    );
    let mut changed = bundle.clone();
    changed.recursive_components[0].certificate.edges.pop();
    representable("a dropped component edge", &changed, false);

    let mut mutated = section.clone();
    mutated[spans.components[0].component.clone()].copy_from_slice(&0_u64.to_le_bytes());
    rejected(
        "a zero component identity",
        &mutated,
        ProofCodecError::ZeroIdentity("RecursiveComponentId"),
    );
    let mut mutated = section.clone();
    mutated[spans.components[0].edge_count.clone()].copy_from_slice(&9_u32.to_le_bytes());
    rejected_unspecified("an over-counted component edge roster", &mutated);
    let mut mutated = section.clone();
    mutated[spans.component_count.clone()].copy_from_slice(&2_u32.to_le_bytes());
    rejected_unspecified("an over-counted component roster", &mutated);

    // --- control-cycle certificates: the module reconstructs no cycle
    // obligation, so a substituted roster is representable surplus authority
    // that replay refuses; each certificate field is still bound into the
    // recomputed identity ---

    let surplus_cycle = |component: u64| -> ControlCycleEvidence {
        ControlCycleEvidence {
            component: CycleComponentId::new(component).expect("nonzero cycle identity"),
            certificate: RecursiveComponentCertificate {
                identity: evidence_id(41),
                ranking_relation: RankingRelationId::new(51).expect("nonzero ranking relation"),
                well_foundedness: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
                edges: vec![
                    RecursiveEdgeCertificate {
                        obligation: obligation_id(61),
                        evidence: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                            identity: evidence_id(71),
                            proof_system_marker: ProofSystemMarker::CURRENT,
                            proof: ProofNode {
                                conclusion: Proposition::Truth,
                                rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
                            },
                        }),
                    },
                    RecursiveEdgeCertificate {
                        obligation: obligation_id(62),
                        evidence: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
                    },
                ],
            },
        }
    };
    let mut changed = bundle.clone();
    changed.control_cycles.push(surplus_cycle(31));
    representable("an inserted control-cycle row", &changed, false);
    let cycle_fields: [(&'static str, Box<dyn Fn(&mut ControlCycleEvidence)>); 6] = [
        (
            "a control-cycle component identity",
            Box::new(|row| {
                row.component = CycleComponentId::new(32).expect("nonzero cycle identity");
            }),
        ),
        (
            "a control-cycle certificate identity",
            Box::new(|row| {
                row.certificate.identity = evidence_id(42);
            }),
        ),
        (
            "a control-cycle ranking relation",
            Box::new(|row| {
                row.certificate.ranking_relation =
                    RankingRelationId::new(52).expect("nonzero ranking relation");
            }),
        ),
        (
            "a control-cycle well-foundedness route",
            Box::new(|row| {
                row.certificate.well_foundedness =
                    EvidenceRoute::CertificateDerived(CertificateEnvelope {
                        identity: evidence_id(72),
                        proof_system_marker: ProofSystemMarker::CURRENT,
                        proof: ProofNode {
                            conclusion: Proposition::Truth,
                            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
                        },
                    });
            }),
        ),
        (
            "a control-cycle edge obligation",
            Box::new(|row| {
                row.certificate.edges[0].obligation = obligation_id(60);
            }),
        ),
        (
            "a control-cycle edge route",
            Box::new(|row| {
                row.certificate.edges[1].evidence = EvidenceRoute::Admitted(AdmissionEvidence {
                    site: AdmissionSiteId::new(63).expect("site identity"),
                    kind: AdmissionKind::ForeignBoundaryGuarantee,
                    authority_identity: evidence_id(64),
                    evidence_identity: evidence_id(65),
                    profile_decision: ProfileDecisionId::new(66).expect("decision identity"),
                });
            }),
        ),
    ];
    for (name, mutate) in cycle_fields {
        let mut changed = bundle.clone();
        let mut row = surplus_cycle(31);
        mutate(&mut row);
        changed.control_cycles.push(row);
        representable(name, &changed, false);
    }
    let mut changed = bundle.clone();
    changed.control_cycles = vec![surplus_cycle(32), surplus_cycle(31)];
    encode_rejected(
        "a reordered control-cycle roster",
        &changed,
        ProofCodecError::NonCanonicalControlCycleEvidence,
    );

    // --- evidence producer provenance: the dense identity is canonicality-
    // bound, the term join and interface fields are semantic, and the
    // remaining provenance strings are identity-bound only ---

    let mut changed = bundle.clone();
    changed.evidence_producers[0].id = evidence_id(2);
    encode_rejected(
        "a non-dense producer identity",
        &changed,
        ProofCodecError::NonCanonicalEvidenceProducerOrder,
    );
    let mut changed = bundle.clone();
    changed.evidence_producers[0].term = evidence_term_id(2);
    representable("a producer's evidence term", &changed, false);
    let mut changed = bundle.clone();
    changed.evidence_producers[0].conformance_identity = "package::other-conformance".to_owned();
    representable("a producer's conformance identity", &changed, true);
    let mut changed = bundle.clone();
    changed.evidence_producers[0].evidence_trait_identity = "package::Other".to_owned();
    representable("a producer's evidence trait", &changed, false);
    let mut changed = bundle.clone();
    changed.evidence_producers[0].rows[0].declaring_trait_identity = "package::Other".to_owned();
    representable("a realization's declaring trait", &changed, false);
    let mut changed = bundle.clone();
    changed.evidence_producers[0].rows[0].declaring_trait_arguments =
        vec!["package::Argument".to_owned()];
    representable("a realization's trait arguments", &changed, false);
    let mut changed = bundle.clone();
    changed.evidence_producers[0].rows[0].requirement_identity =
        "package::Evidence::other".to_owned();
    representable("a realization's requirement", &changed, false);
    let mut changed = bundle.clone();
    changed.evidence_producers[0].rows[0].realization_machine_identity =
        "package::other-machine".to_owned();
    representable("a realization's machine identity", &changed, true);
    let mut changed = bundle.clone();
    changed.evidence_producers[0].rows[0].realization_state_identity =
        "package::producer::other".to_owned();
    representable("a realization's state identity", &changed, true);
    let mut changed = bundle.clone();
    changed.evidence_producers[0].rows[0].source = EvidenceProducerRowSource::Reference;
    representable("a realization's source kind", &changed, true);

    let mut changed = bundle.clone();
    changed.evidence_producers.pop();
    representable("a dropped producer row", &changed, false);
    let mut changed = bundle.clone();
    changed.evidence_producers.push(EvidenceProducerProvenance {
        id: evidence_id(2),
        term: evidence_term_id(2),
        conformance_identity: "package::conformance".to_owned(),
        evidence_trait_identity: "package::Evidence".to_owned(),
        rows: Vec::new(),
    });
    representable("an inserted producer row", &changed, false);
    let mut changed = bundle.clone();
    changed.evidence_producers[0].conformance_identity = String::new();
    encode_rejected(
        "an empty conformance identity",
        &changed,
        ProofCodecError::InvalidEvidenceProducer,
    );
    let mut changed = bundle.clone();
    changed.evidence_producers[0].rows[0].requirement_identity = String::new();
    encode_rejected(
        "an empty realization requirement",
        &changed,
        ProofCodecError::InvalidEvidenceProducer,
    );
    let mut changed = bundle.clone();
    let duplicated_row = changed.evidence_producers[0].rows[0].clone();
    changed.evidence_producers[0].rows.push(duplicated_row);
    encode_rejected(
        "a duplicated realization row",
        &changed,
        ProofCodecError::NonCanonicalEvidenceProducerRows,
    );
    let mut changed = bundle.clone();
    let mut second_row = changed.evidence_producers[0].rows[0].clone();
    second_row.requirement_identity = "package::Evidence::zzz".to_owned();
    changed.evidence_producers[0].rows.insert(0, second_row);
    encode_rejected(
        "a reordered realization roster",
        &changed,
        ProofCodecError::NonCanonicalEvidenceProducerRows,
    );

    let mut mutated = section.clone();
    mutated[spans.producers[0].id.clone()].copy_from_slice(&0_u64.to_le_bytes());
    rejected(
        "a zero producer identity",
        &mutated,
        ProofCodecError::ZeroIdentity("EvidenceIdentity"),
    );
    let mut mutated = section.clone();
    mutated[spans.producers[0].term.clone()].copy_from_slice(&0_u64.to_le_bytes());
    rejected(
        "a zero producer term",
        &mutated,
        ProofCodecError::ZeroIdentity("EvidenceTermId"),
    );
    let mut mutated = section.clone();
    mutated[spans.producers[0].conformance.clone()].fill(0xFF);
    rejected(
        "a non-UTF-8 conformance identity",
        &mutated,
        ProofCodecError::InvalidUtf8("evidence producer conformance"),
    );
    let mut mutated = section.clone();
    mutated[spans.producers[0].conformance_len.clone()]
        .copy_from_slice(&((1_u32 << 20) + 1).to_le_bytes());
    rejected(
        "an over-long conformance identity",
        &mutated,
        ProofCodecError::StringTooLong("evidence producer conformance"),
    );
    let mut mutated = section.clone();
    mutated[spans.producers[0].rows[0].source.clone()].copy_from_slice(&[9]);
    rejected(
        "an unknown realization source tag",
        &mutated,
        ProofCodecError::InvalidTag("EvidenceProducerRowSource", 9),
    );
    let mut mutated = section.clone();
    mutated[spans.producers[0].row_count.clone()].copy_from_slice(&0_u32.to_le_bytes());
    rejected(
        "a cleared producer row count",
        &mutated,
        ProofCodecError::TrailingBytes(spans.producers[0].rows[0].row.len()),
    );
    let mut mutated = section.clone();
    mutated[spans.producer_count.clone()].copy_from_slice(&0_u32.to_le_bytes());
    rejected(
        "a cleared producer roster",
        &mutated,
        ProofCodecError::TrailingBytes(spans.producers[0].row.len()),
    );
    let mut mutated = section.clone();
    mutated[spans.producer_count.clone()].copy_from_slice(&3_u32.to_le_bytes());
    rejected_unspecified("an over-counted producer roster", &mutated);
    let mut mutated = section.clone();
    mutated[spans.cycle_count.clone()].copy_from_slice(&2_u32.to_le_bytes());
    rejected_unspecified("an over-counted control-cycle roster", &mutated);

    // --- the foreign-subject seal and the transport envelope join the same
    // substitution at the artifact boundary ---

    let mut foreign = semantic_module();
    foreign.machines[0].id = machine_id(7);
    foreign.entry = machine_id(7);
    assert!(
        matches!(
            decode_proof_section_for(&foreign, &section),
            Err(ProofCodecError::ProofSubjectMismatch { .. })
        ),
        "the retained section must reject under a foreign module"
    );
    assert_eq!(
        claimed,
        terminal_codec::terminal_psi_identity(&module).unwrap()
    );

    // --- envelope replay: a representable substitution rides the transport
    // envelope, recomputes a divergent artifact identity, and meets the
    // verifier replay leg on the decoded artifact ---

    let envelope = artifact.to_bytes();
    let semantic_len =
        usize::try_from(u64::from_le_bytes(envelope[10..18].try_into().unwrap())).unwrap();
    let proof_len =
        usize::try_from(u64::from_le_bytes(envelope[18..26].try_into().unwrap())).unwrap();
    assert_eq!(proof_len, section.len());
    let proof_offset = 35 + semantic_len;
    // Rebuild the transport envelope around substituted proof and
    // optimization sections: each section's u64 length is repaired and the
    // optimization record must be the one honestly recomputed over the
    // substituted bundle, since the manifest validation joins them.
    let splice_envelope = |new_proof: &[u8], new_optimization: &[u8]| -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&envelope[..18]);
        out.extend_from_slice(
            &u64::try_from(new_proof.len())
                .expect("proof section fits u64")
                .to_le_bytes(),
        );
        out.extend_from_slice(
            &u64::try_from(new_optimization.len())
                .expect("optimization section fits u64")
                .to_le_bytes(),
        );
        out.extend_from_slice(&envelope[34..proof_offset]);
        out.extend_from_slice(new_proof);
        out.extend_from_slice(new_optimization);
        out
    };

    let mut proof_changed = bundle.clone();
    proof_changed.evidence[0].route =
        EvidenceRoute::KernelDerived(PrimitiveJudgment::ClosedIntegerRelation);
    let substituted_section =
        encode_proof_section(&module, &proof_changed).expect("substituted section");
    let substituted_optimization = terminal_codec::encode_psi_optimization_execution_record(
        &build_identity_optimization_execution_record(&module, &proof_changed)
            .expect("identity optimization over the substitution"),
    );
    let decoded = CanonicalTerminalArtifact::from_bytes(&splice_envelope(
        &substituted_section,
        &substituted_optimization,
    ))
    .expect("a representable substitution still forms an artifact");
    assert_ne!(
        decoded.manifest().identity(),
        retained.identity(),
        "the substituted envelope must recompute a divergent artifact identity"
    );
    assert_eq!(
        validate_artifact_manifest(
            &module,
            &proof_changed,
            decoded.optimization(),
            None,
            None,
            retained,
        ),
        Err(ArtifactManifestError::ManifestMismatch),
        "the substituted envelope must reject at the retained-manifest replay"
    );
    // The verifier replays the substituted route: the kernel judgment proves
    // the same obligation, so the semantic verdict is unchanged even though
    // custody diverged.
    verify_terminal_artifact_proof(&decoded, &AdmissionProfile::default())
        .expect("an equally valid derivation still replays semantically");

    let mut proof_changed = bundle.clone();
    // Obligation identities stay in canonical order; every row retargets to
    // an obligation the verifier never reconstructs.
    proof_changed.evidence[0].obligation = obligation_id(7);
    proof_changed.evidence[1].obligation = obligation_id(8);
    proof_changed.evidence[2].obligation = obligation_id(9);
    let substituted_section =
        encode_proof_section(&module, &proof_changed).expect("substituted section");
    let substituted_optimization = terminal_codec::encode_psi_optimization_execution_record(
        &build_identity_optimization_execution_record(&module, &proof_changed)
            .expect("identity optimization over the substitution"),
    );
    let decoded = CanonicalTerminalArtifact::from_bytes(&splice_envelope(
        &substituted_section,
        &substituted_optimization,
    ))
    .expect("a representable substitution still forms an artifact");
    assert!(
        verify_terminal_artifact_proof(&decoded, &AdmissionProfile::default()).is_err(),
        "a retargeted evidence roster must reject at the verifier replay"
    );

    let mut corrupt_envelope = envelope.clone();
    corrupt_envelope[proof_offset + spans.evidence[0].route_tag.start] = 9;
    assert!(matches!(
        CanonicalTerminalArtifact::from_bytes(&corrupt_envelope),
        Err(CanonicalTerminalArtifactError::Proof(
            ProofCodecError::InvalidTag("EvidenceRoute", 9)
        ))
    ));

    // The producing side binds the same canonicality rules at encoding for
    // the remaining roster axes exercised above.
    assert_eq!(
        encode_proof_section(&module, &bundle),
        Ok(section.clone()),
        "the honest fixture re-encodes byte-exact"
    );
}
