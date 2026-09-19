//! One-field mutation coverage for the semantic module's proposition
//! vocabulary rosters.
//!
//! `proposition_declarations` is the nominal proof-formula vocabulary: each
//! row carries a dense `PropositionId`, a globally unique name, a counted
//! binder roster (name plus a `Type`, `Const { type_identity }`, or `Machine`
//! kind tag), a counted parameter-type string roster, and an evidence
//! classifier (`FactOnly` or `Witness { evidence_type }`).
//! `proposition_applications` is the normalized application roster: each row
//! carries a dense `PropositionId`, the joined declaration identity, a counted
//! binder-argument roster whose rows match the declaration binders
//! positionally (kind plus either an ordinary static-argument identity string
//! or a structured evidence projection carrying an `EvidenceTermId` and a
//! declaring-trait triple), a counted argument string roster, and an optional
//! evidence interface present exactly for witness-bearing applications.
//! `evidence_terms` retains the erased witness terms: a dense
//! `EvidenceTermId`, the proposition application identity it inhabits, and an
//! interface that must equal the application's. `evidence_contract_lanes`
//! retains each machine's `Requires`/`Ensures` custody: machine identity,
//! kind tag, per-(machine, kind) dense position, term identity, and the
//! output field present exactly on `ensures` lanes.
//!
//! The fixture keeps the fact-only declaration first — its emptier shape
//! sorts ahead of the witness declaration under the id-normalized semantic
//! identity order — so a name collision stays canonically ordered and reaches
//! `DuplicatePropositionName`. The witness application exercises all three
//! binder kinds with its machine binder arguments appearing once as an
//! evidence projection and once as an ordinary identity; the lone evidence
//! term inhabits that application with an identical interface, and a
//! requires/ensures lane pair on the entry machine keeps the term used and
//! the ensures lane matched so no producer provenance is owed. Every roster
//! count, identity, kind and classifier tag, length-framed string, and
//! counted sub-roster is substituted independently. A substitution either
//! fails canonical decoding or module-bound validation (a non-dense or
//! out-of-order identity, an unknown or misshaped join, a name that collides
//! or empties, a binder kind or interface that contradicts its declaration
//! classification, a term whose interface drifts from its application's, a
//! lane that orphans its term or breaks the output-field discipline), or it
//! decodes to a different module whose honestly recomputed semantic and
//! artifact identities diverge and whose replay against the retained
//! manifest and sealed proof subject rejects. Substitutions that decode to a
//! representationally sound but unverifiable module — an `ensures` lane left
//! without its matching `requires` — are named with the exact verification
//! error independent replay raises.

use std::ops::Range;

use super::{
    block_id, canonical_artifact, contract_id, edge_id, evidence_term_id, machine_id,
    proposition_id,
};
use proof_admission::AdmissionProfile;
use terminal_codec::{
    ArtifactManifestError, CodecError, ProofCodecError, build_artifact_manifest,
    build_identity_optimization_execution_record, decode_module, decode_proof_section_for,
    encode_module, terminal_psi_identity, validate_artifact_manifest,
};
use terminal_psi::{
    Block, EvidenceContractLane, EvidenceContractLaneKind, EvidenceInterfaceIdentity,
    EvidenceProjectionIdentity, EvidenceRequirementIdentity, EvidenceTermDeclaration,
    MachineContract, PropositionApplicationIdentity, PropositionBinderArgumentIdentity,
    PropositionBinderArgumentKind, PropositionBinderDeclaration, PropositionBinderKind,
    PropositionDeclaration, PropositionEvidence, TerminalMachine, TerminalMachineResult,
    TerminalModule, Terminator,
};
use terminal_verifier::{ModuleError, ProofBundle, VerificationError, verify_module};

/// Byte offsets of every wire field inside the four vocabulary rosters.
/// Every `string` field records its u32 length prefix and its content bytes
/// separately so a substitution can lie about either side, and every counted
/// roster records its own count span.
struct ModuleSpans {
    declaration_count: Range<usize>,
    declarations: Vec<DeclarationSpan>,
    application_count: Range<usize>,
    applications: Vec<ApplicationSpan>,
    term_count: Range<usize>,
    terms: Vec<TermSpan>,
    lane_count: Range<usize>,
    lanes: Vec<LaneSpan>,
}

/// A length-framed string field: the u32 length prefix and the value bytes.
struct StringSpan {
    len: Range<usize>,
    value: Range<usize>,
}

/// A proposition binder declaration row: name, kind tag, and the const
/// binder's type identity string when the kind is `Const`.
struct BinderSpan {
    row: Range<usize>,
    name: StringSpan,
    kind: Range<usize>,
    const_type: Option<StringSpan>,
}

struct DeclarationSpan {
    row: Range<usize>,
    id: Range<usize>,
    name: StringSpan,
    binder_count: Range<usize>,
    binders: Vec<BinderSpan>,
    parameter_count: Range<usize>,
    parameter_types: Vec<StringSpan>,
    evidence_tag: Range<usize>,
    evidence_type: Option<StringSpan>,
}

/// One evidence requirement row inside an interface: declaring trait, a
/// counted declaring-trait-argument roster, and the requirement identity.
struct RequirementSpan {
    row: Range<usize>,
    declaring_trait: StringSpan,
    argument_count: Range<usize>,
    arguments: Vec<StringSpan>,
    requirement: StringSpan,
}

struct InterfaceSpan {
    trait_identity: StringSpan,
    argument_count: Range<usize>,
    arguments: Vec<StringSpan>,
    requirement_count: Range<usize>,
    requirements: Vec<RequirementSpan>,
}

/// The structured evidence projection carried by a machine binder argument:
/// the projected term and the declaring-trait triple that must appear among
/// that term's interface requirements.
struct ProjectionSpan {
    term: Range<usize>,
    declaring_trait: StringSpan,
    argument_count: Range<usize>,
    arguments: Vec<StringSpan>,
    requirement: StringSpan,
}

/// A binder argument row: kind tag, the projection presence flag, and either
/// the ordinary identity string or the projection's fields.
struct BinderArgumentSpan {
    row: Range<usize>,
    kind: Range<usize>,
    projection_flag: Range<usize>,
    identity: Option<StringSpan>,
    projection: Option<ProjectionSpan>,
}

struct ApplicationSpan {
    row: Range<usize>,
    id: Range<usize>,
    declaration: Range<usize>,
    binder_count: Range<usize>,
    binder_arguments: Vec<BinderArgumentSpan>,
    argument_count: Range<usize>,
    arguments: Vec<StringSpan>,
    interface_flag: Range<usize>,
    interface: Option<InterfaceSpan>,
}

struct TermSpan {
    row: Range<usize>,
    id: Range<usize>,
    proposition: Range<usize>,
    interface: InterfaceSpan,
}

struct LaneSpan {
    row: Range<usize>,
    machine: Range<usize>,
    kind: Range<usize>,
    position: Range<usize>,
    term: Range<usize>,
    output_flag: Range<usize>,
    output_field: Option<StringSpan>,
}

/// A cursor that walks the canonical module encoding exactly as the decoder
/// does, recording the byte span of every vocabulary field the matrix
/// substitutes. Every section ahead of the declaration roster is empty in
/// the fixture, so the walk asserts each leading count rather than silently
/// spanning unknown row bytes.
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

    fn take_count(&mut self) -> (Range<usize>, u32) {
        let span = self.take(4);
        let count = u32::from_le_bytes(
            self.bytes[span.clone()]
                .try_into()
                .expect("u32 count inside the module"),
        );
        (span, count)
    }

    fn expect_empty_count(&mut self, label: &'static str) {
        let (_, count) = self.take_count();
        assert_eq!(count, 0, "the fixture leaves the {label} roster empty");
    }

    fn take_string(&mut self) -> StringSpan {
        let (len, value_len) = self.take_count();
        let value = self.take(usize::try_from(value_len).expect("string length fits usize"));
        StringSpan { len, value }
    }

    fn tag(&mut self) -> (Range<usize>, u8) {
        let span = self.take(1);
        (span.clone(), self.bytes[span.start])
    }

    fn walk_binder(&mut self) -> BinderSpan {
        let row_start = self.offset;
        let name = self.take_string();
        let (kind, value) = self.tag();
        let const_type = match value {
            1 | 3 => None,
            2 => Some(self.take_string()),
            other => panic!("fixture binder kinds are Type, Const, or Machine, not {other}"),
        };
        BinderSpan {
            row: row_start..self.offset,
            name,
            kind,
            const_type,
        }
    }

    fn walk_declaration(&mut self) -> DeclarationSpan {
        let row_start = self.offset;
        let id = self.take(8);
        let name = self.take_string();
        let (binder_count, binders) = self.take_count();
        let mut binder_spans = Vec::with_capacity(binders as usize);
        for _ in 0..binders {
            binder_spans.push(self.walk_binder());
        }
        let (parameter_count, parameters) = self.take_count();
        let mut parameter_types = Vec::with_capacity(parameters as usize);
        for _ in 0..parameters {
            parameter_types.push(self.take_string());
        }
        let (evidence_tag, evidence) = self.tag();
        let evidence_type = match evidence {
            1 => None,
            2 => Some(self.take_string()),
            other => panic!("fixture evidence is FactOnly or Witness, not tag {other}"),
        };
        DeclarationSpan {
            row: row_start..self.offset,
            id,
            name,
            binder_count,
            binders: binder_spans,
            parameter_count,
            parameter_types,
            evidence_tag,
            evidence_type,
        }
    }

    fn walk_projection(&mut self) -> ProjectionSpan {
        let term = self.take(8);
        let declaring_trait = self.take_string();
        let (argument_count, arguments) = self.take_count();
        let mut argument_spans = Vec::with_capacity(arguments as usize);
        for _ in 0..arguments {
            argument_spans.push(self.take_string());
        }
        let requirement = self.take_string();
        ProjectionSpan {
            term,
            declaring_trait,
            argument_count,
            arguments: argument_spans,
            requirement,
        }
    }

    fn walk_binder_argument(&mut self) -> BinderArgumentSpan {
        let row_start = self.offset;
        let (kind, _) = self.tag();
        let (projection_flag, flag) = self.tag();
        let (identity, projection) = match flag {
            0 => (Some(self.take_string()), None),
            1 => (None, Some(self.walk_projection())),
            other => panic!("fixture binder arguments carry a boolean flag, not {other}"),
        };
        BinderArgumentSpan {
            row: row_start..self.offset,
            kind,
            projection_flag,
            identity,
            projection,
        }
    }

    fn walk_requirement(&mut self) -> RequirementSpan {
        let row_start = self.offset;
        let declaring_trait = self.take_string();
        let (argument_count, arguments) = self.take_count();
        let mut argument_spans = Vec::with_capacity(arguments as usize);
        for _ in 0..arguments {
            argument_spans.push(self.take_string());
        }
        let requirement = self.take_string();
        RequirementSpan {
            row: row_start..self.offset,
            declaring_trait,
            argument_count,
            arguments: argument_spans,
            requirement,
        }
    }

    fn walk_interface(&mut self) -> InterfaceSpan {
        let trait_identity = self.take_string();
        let (argument_count, arguments) = self.take_count();
        let mut argument_spans = Vec::with_capacity(arguments as usize);
        for _ in 0..arguments {
            argument_spans.push(self.take_string());
        }
        let (requirement_count, requirements) = self.take_count();
        let mut requirement_spans = Vec::with_capacity(requirements as usize);
        for _ in 0..requirements {
            requirement_spans.push(self.walk_requirement());
        }
        InterfaceSpan {
            trait_identity,
            argument_count,
            arguments: argument_spans,
            requirement_count,
            requirements: requirement_spans,
        }
    }

    fn walk_application(&mut self) -> ApplicationSpan {
        let row_start = self.offset;
        let id = self.take(8);
        let declaration = self.take(8);
        let (binder_count, binder_arguments) = self.take_count();
        let mut argument_spans = Vec::with_capacity(binder_arguments as usize);
        for _ in 0..binder_arguments {
            argument_spans.push(self.walk_binder_argument());
        }
        let (argument_count, arguments) = self.take_count();
        let mut value_spans = Vec::with_capacity(arguments as usize);
        for _ in 0..arguments {
            value_spans.push(self.take_string());
        }
        let (interface_flag, flag) = self.tag();
        let interface = match flag {
            0 => None,
            1 => Some(self.walk_interface()),
            other => panic!("fixture interfaces carry a boolean flag, not {other}"),
        };
        ApplicationSpan {
            row: row_start..self.offset,
            id,
            declaration,
            binder_count,
            binder_arguments: argument_spans,
            argument_count,
            arguments: value_spans,
            interface_flag,
            interface,
        }
    }

    fn walk_term(&mut self) -> TermSpan {
        let row_start = self.offset;
        let id = self.take(8);
        let proposition = self.take(8);
        let interface = self.walk_interface();
        TermSpan {
            row: row_start..self.offset,
            id,
            proposition,
            interface,
        }
    }

    fn walk_lane(&mut self) -> LaneSpan {
        let row_start = self.offset;
        let machine = self.take(8);
        let (kind, _) = self.tag();
        let position = self.take(4);
        let term = self.take(8);
        let (output_flag, flag) = self.tag();
        let output_field = match flag {
            0 => None,
            1 => Some(self.take_string()),
            other => panic!("fixture lane outputs carry a boolean flag, not {other}"),
        };
        LaneSpan {
            row: row_start..self.offset,
            machine,
            kind,
            position,
            term,
            output_flag,
            output_field,
        }
    }
}

/// Locate the four vocabulary rosters inside canonical module bytes by
/// mirroring `encode_raw`'s ordered section list. The fixture module keeps
/// every earlier roster empty, so each leading section is an asserted zero
/// count.
fn module_spans(encoded: &[u8]) -> ModuleSpans {
    let mut walker = SpanWalker::new(encoded);
    walker.take(8); // module magic
    walker.take(2); // format marker
    walker.take(2); // vocabulary marker
    walker.take(8); // entry machine identity
    // The scalar-qualification catalog encodes four counted rosters even when
    // empty: domains, qualification sets, coercions, and float entry ranges.
    for label in [
        "scalar domains",
        "scalar qualification sets",
        "scalar qualification coercions",
        "scalar float entry ranges",
    ] {
        walker.expect_empty_count(label);
    }
    for label in [
        "structural types",
        "structural domains",
        "services",
        "concrete root service reach",
        "installation reach dependencies",
        "placed-view inputs",
        "reborrow root handoffs",
        "reborrow restored call uses",
        "boundary machines",
        "provider candidates",
        "float-meaning projections",
        "float-meaning equalities",
    ] {
        walker.expect_empty_count(label);
    }
    let (declaration_count, declarations) = walker.take_count();
    let mut declaration_spans = Vec::with_capacity(declarations as usize);
    for _ in 0..declarations {
        declaration_spans.push(walker.walk_declaration());
    }
    let (application_count, applications) = walker.take_count();
    let mut application_spans = Vec::with_capacity(applications as usize);
    for _ in 0..applications {
        application_spans.push(walker.walk_application());
    }
    let (term_count, terms) = walker.take_count();
    let mut term_spans = Vec::with_capacity(terms as usize);
    for _ in 0..terms {
        term_spans.push(walker.walk_term());
    }
    let (lane_count, lanes) = walker.take_count();
    let mut lane_spans = Vec::with_capacity(lanes as usize);
    for _ in 0..lanes {
        lane_spans.push(walker.walk_lane());
    }
    ModuleSpans {
        declaration_count,
        declarations: declaration_spans,
        application_count,
        applications: application_spans,
        term_count,
        terms: term_spans,
        lane_count,
        lanes: lane_spans,
    }
}

/// The interface the witness application, the evidence term, and the machine
/// binder argument's projection all share: one declaring trait argument and
/// one requirement. Every join key is exercised by a distinct substitution.
fn vocabulary_interface() -> EvidenceInterfaceIdentity {
    EvidenceInterfaceIdentity {
        trait_identity: "example::Provable".into(),
        arguments: vec!["example::Cell".into()],
        requirements: vec![EvidenceRequirementIdentity {
            declaring_trait_identity: "example::Provable".into(),
            declaring_trait_arguments: vec!["example::Cell".into()],
            requirement_identity: "example::provable::holds".into(),
        }],
    }
}

/// The fact-only declaration: no binders, no parameter types, no witness.
fn fact_declaration() -> PropositionDeclaration {
    PropositionDeclaration {
        id: proposition_id(1),
        name: "example::Bounded".into(),
        binders: Vec::new(),
        parameter_types: Vec::new(),
        evidence: PropositionEvidence::FactOnly,
    }
}

/// The witness declaration: every binder kind on the wire — one `Type`, one
/// `Const` carrying a type identity, and two `Machine` binders so the
/// application carries both a projected and an ordinary machine argument.
fn witness_declaration() -> PropositionDeclaration {
    PropositionDeclaration {
        id: proposition_id(2),
        name: "example::Closed".into(),
        binders: vec![
            PropositionBinderDeclaration {
                name: "T".into(),
                kind: PropositionBinderKind::Type,
            },
            PropositionBinderDeclaration {
                name: "n".into(),
                kind: PropositionBinderKind::Const {
                    type_identity: "example::Nat".into(),
                },
            },
            PropositionBinderDeclaration {
                name: "m".into(),
                kind: PropositionBinderKind::Machine,
            },
            PropositionBinderDeclaration {
                name: "w".into(),
                kind: PropositionBinderKind::Machine,
            },
        ],
        parameter_types: vec!["example::Cell".into(), "example::Nat".into()],
        evidence: PropositionEvidence::Witness {
            evidence_type: "example::ClosedProof".into(),
        },
    }
}

/// The shared projection: machine binder argument `m` is projected through
/// term 1's single requirement.
fn binder_projection() -> EvidenceProjectionIdentity {
    EvidenceProjectionIdentity {
        term: evidence_term_id(1),
        declaring_trait_identity: "example::Provable".into(),
        declaring_trait_arguments: vec!["example::Cell".into()],
        requirement_identity: "example::provable::holds".into(),
    }
}

fn vocabulary_module() -> TerminalModule {
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: vec![fact_declaration(), witness_declaration()],
        proposition_applications: vec![
            PropositionApplicationIdentity {
                id: proposition_id(1),
                declaration: proposition_id(1),
                binder_arguments: Vec::new(),
                arguments: Vec::new(),
                evidence_interface: None,
            },
            PropositionApplicationIdentity {
                id: proposition_id(2),
                declaration: proposition_id(2),
                binder_arguments: vec![
                    PropositionBinderArgumentIdentity {
                        kind: PropositionBinderArgumentKind::Type,
                        identity: "example::Cell".into(),
                        evidence_projection: None,
                    },
                    PropositionBinderArgumentIdentity {
                        kind: PropositionBinderArgumentKind::Const,
                        identity: "example::three".into(),
                        evidence_projection: None,
                    },
                    PropositionBinderArgumentIdentity {
                        kind: PropositionBinderArgumentKind::Machine,
                        identity: String::new(),
                        evidence_projection: Some(binder_projection()),
                    },
                    PropositionBinderArgumentIdentity {
                        kind: PropositionBinderArgumentKind::Machine,
                        identity: "example::main".into(),
                        evidence_projection: None,
                    },
                ],
                arguments: vec!["example::Cell".into(), "example::three".into()],
                evidence_interface: Some(vocabulary_interface()),
            },
        ],
        evidence_terms: vec![EvidenceTermDeclaration {
            id: evidence_term_id(1),
            proposition: proposition_id(2),
            interface: vocabulary_interface(),
        }],
        evidence_contract_lanes: vec![
            EvidenceContractLane {
                machine: machine_id(1),
                kind: EvidenceContractLaneKind::Requires,
                position: 0,
                term: evidence_term_id(1),
                output_field: None,
            },
            EvidenceContractLane {
                machine: machine_id(1),
                kind: EvidenceContractLaneKind::Ensures,
                position: 0,
                term: evidence_term_id(1),
                output_field: Some("verdict".into()),
            },
        ],
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: machine_id(1),
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(1),
            blocks: vec![Block {
                structural_parameters: Vec::new(),
                id: block_id(1),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit {
                    edge: edge_id(1),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: contract_id(1),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

/// The fixture carries no contract obligations, so the retained proof bundle
/// is the empty bundle.
fn vocabulary_bundle() -> ProofBundle {
    ProofBundle::default()
}

#[test]
fn terminal_proposition_vocabulary_rejects_every_one_field_substitution() {
    let module = vocabulary_module();
    let bundle = vocabulary_bundle();
    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("the fixture module verifies under its proof bundle");
    let encoded = encode_module(&module).expect("canonical module bytes");
    assert_eq!(
        decode_module(&encoded),
        Ok(module.clone()),
        "the canonical module round-trips"
    );
    let spans = module_spans(&encoded);
    assert_eq!(
        spans.declarations.len(),
        2,
        "fixture roster: a fact-only declaration and a witness declaration"
    );
    assert_eq!(
        spans.applications.len(),
        2,
        "fixture roster: one application per declaration"
    );
    assert_eq!(spans.terms.len(), 1, "fixture roster: one evidence term");
    assert_eq!(
        spans.lanes.len(),
        2,
        "fixture roster: a requires lane and a matched ensures lane"
    );

    // The retained artifact binds the semantic identity into its manifest
    // and the sealed proof section to this exact module; replaying either
    // against a substituted module is the independent-replay leg for every
    // representable field.
    let artifact = canonical_artifact(&module, &bundle, None);
    let retained = artifact.manifest();
    let semantic_identity = terminal_psi_identity(&module).expect("semantic identity");
    assert_eq!(retained.semantic(), semantic_identity);

    // A substitution that still forms a canonical module honestly
    // recomputes a divergent semantic and artifact identity: the
    // substituted module still verifies under the retained bundle (the
    // vocabulary carries no proof obligations), while the retained custody
    // replays — the manifest join and the sealed proof subject join —
    // reject it.
    let divergent = |name: &'static str, mutated: &[u8]| {
        let substituted = decode_module(mutated)
            .unwrap_or_else(|error| panic!("{name} must still decode: {error:?}"));
        assert_ne!(substituted, module, "{name} must change the module");
        assert_eq!(
            encode_module(&substituted).expect("re-encode the substitution"),
            mutated,
            "{name} must re-encode canonically"
        );
        assert_ne!(
            terminal_psi_identity(&substituted).expect("substituted semantic identity"),
            semantic_identity,
            "{name} must diverge the honestly recomputed semantic identity"
        );
        verify_module(&substituted, &bundle, &AdmissionProfile::default()).unwrap_or_else(
            |error| panic!("{name} must keep the substituted module verifiable: {error:?}"),
        );
        let recomputed_optimization =
            build_identity_optimization_execution_record(&substituted, &bundle)
                .expect("identity optimization over the substituted module");
        let recomputed =
            build_artifact_manifest(&substituted, &bundle, &recomputed_optimization, None, None)
                .expect("honest manifest over the substituted module");
        assert_ne!(
            recomputed.identity(),
            retained.identity(),
            "{name} must diverge the recomputed artifact identity"
        );
        assert_eq!(
            validate_artifact_manifest(
                &substituted,
                &bundle,
                &recomputed_optimization,
                None,
                None,
                retained,
            ),
            Err(ArtifactManifestError::ManifestMismatch),
            "{name} must reject at the retained-manifest replay"
        );
        assert!(
            matches!(
                decode_proof_section_for(&substituted, artifact.proof_bytes()),
                Err(ProofCodecError::ProofSubjectMismatch { .. })
            ),
            "{name} must reject at the sealed proof subject join"
        );
        substituted
    };
    // A substitution that decodes to a representationally sound module can
    // still fail independent proof replay: an ensures lane divorced from its
    // requires lane leaves the term unproduced. The canonical decode and the
    // retained custody replays are asserted alongside the exact verification
    // error.
    let verify_rejected = |name: &'static str, mutated: &[u8], expected: VerificationError| {
        let substituted = decode_module(mutated)
            .unwrap_or_else(|error| panic!("{name} must still decode: {error:?}"));
        assert_ne!(substituted, module, "{name} must change the module");
        assert_eq!(
            encode_module(&substituted).expect("re-encode the substitution"),
            mutated,
            "{name} must re-encode canonically"
        );
        assert_ne!(
            terminal_psi_identity(&substituted).expect("substituted semantic identity"),
            semantic_identity,
            "{name} must diverge the honestly recomputed semantic identity"
        );
        assert_eq!(
            verify_module(&substituted, &bundle, &AdmissionProfile::default())
                .err()
                .unwrap_or_else(|| panic!("{name} must reject at independent verification")),
            expected,
            "{name} must reject with the named verification error"
        );
        let recomputed_optimization =
            build_identity_optimization_execution_record(&substituted, &bundle)
                .expect("identity optimization over the substituted module");
        assert_eq!(
            validate_artifact_manifest(
                &substituted,
                &bundle,
                &recomputed_optimization,
                None,
                None,
                retained,
            ),
            Err(ArtifactManifestError::ManifestMismatch),
            "{name} must reject at the retained-manifest replay"
        );
        assert!(
            matches!(
                decode_proof_section_for(&substituted, artifact.proof_bytes()),
                Err(ProofCodecError::ProofSubjectMismatch { .. })
            ),
            "{name} must reject at the sealed proof subject join"
        );
    };
    // A substitution that cannot form a canonical module rejects inside the
    // canonical decoder with an exact error.
    let rejected = |name: &'static str, mutated: &[u8], expected: CodecError| {
        assert_eq!(
            decode_module(mutated),
            Err(expected),
            "{name} must reject at canonical decoding"
        );
    };
    // A module-level mutation the producer can express rejects inside the
    // canonical encoder's semantic validation.
    let encode_rejected =
        |name: &'static str, changed: &terminal_psi::TerminalModule, expected: CodecError| {
            assert_eq!(
                encode_module(changed),
                Err(expected.clone()),
                "{name} must reject at canonical encoding"
            );
        };
    let put_u8 = |range: Range<usize>, value: u8| -> Vec<u8> {
        let mut mutated = encoded.clone();
        mutated[range.start] = value;
        mutated
    };
    let put_u32 = |range: Range<usize>, value: u32| -> Vec<u8> {
        let mut mutated = encoded.clone();
        mutated[range].copy_from_slice(&value.to_le_bytes());
        mutated
    };
    let put_u64 = |range: Range<usize>, value: u64| -> Vec<u8> {
        let mut mutated = encoded.clone();
        mutated[range].copy_from_slice(&value.to_le_bytes());
        mutated
    };
    // Replace one encoded string, keeping its length prefix honest.
    let restring = |span: &StringSpan, value: &str| -> Vec<u8> {
        let mut mutated = encoded[..span.len.start].to_vec();
        mutated.extend_from_slice(
            &u32::try_from(value.len())
                .expect("string length fits u32")
                .to_le_bytes(),
        );
        mutated.extend_from_slice(value.as_bytes());
        mutated.extend_from_slice(&encoded[span.value.end..]);
        mutated
    };
    // Overwrite one byte inside an encoded string, keeping its length.
    let corrupt_utf8 = |span: &StringSpan| -> Vec<u8> {
        let mut mutated = encoded.clone();
        mutated[span.value.start] = 0xFF;
        mutated
    };
    // Set a counted roster to `remaining` rows and remove the row bytes in
    // `rows`, keeping the count honest.
    let excise = |count_span: Range<usize>, rows: Range<usize>, remaining: u32| -> Vec<u8> {
        let mut mutated = encoded[..count_span.start].to_vec();
        mutated.extend_from_slice(&remaining.to_le_bytes());
        mutated.extend_from_slice(&encoded[count_span.end..rows.start]);
        mutated.extend_from_slice(&encoded[rows.end..]);
        mutated
    };
    // Remove one roster row and decrement its roster count honestly.
    let drop_row = |count_span: Range<usize>, row: Range<usize>| -> Vec<u8> {
        let remaining = u32::from_le_bytes(
            encoded[count_span.clone()]
                .try_into()
                .expect("roster count"),
        ) - 1;
        excise(count_span, row, remaining)
    };
    // Duplicate one roster row directly behind itself with an honest count.
    let duplicate_row = |count_span: Range<usize>, row: Range<usize>| -> Vec<u8> {
        let grown = u32::from_le_bytes(
            encoded[count_span.clone()]
                .try_into()
                .expect("roster count"),
        ) + 1;
        let mut mutated = encoded[..count_span.start].to_vec();
        mutated.extend_from_slice(&grown.to_le_bytes());
        mutated.extend_from_slice(&encoded[count_span.end..row.end]);
        mutated.extend_from_slice(&encoded[row.clone()]);
        mutated.extend_from_slice(&encoded[row.end..]);
        mutated
    };
    // Swap two adjacent rows of the same roster, keeping the count honest.
    let swap_rows = |first: Range<usize>, second: Range<usize>| -> Vec<u8> {
        assert_eq!(first.end, second.start, "fixture rows are adjacent");
        let mut mutated = encoded[..first.start].to_vec();
        mutated.extend_from_slice(&encoded[second.clone()]);
        mutated.extend_from_slice(&encoded[first.clone()]);
        mutated.extend_from_slice(&encoded[second.end..]);
        mutated
    };

    let fact_declaration = &spans.declarations[0];
    let witness_declaration = &spans.declarations[1];
    let fact_application = &spans.applications[0];
    let witness_application = &spans.applications[1];
    let term = &spans.terms[0];
    let requires_lane = &spans.lanes[0];
    let ensures_lane = &spans.lanes[1];
    let interface = || {
        CodecError::InvalidModule(ModuleError::EvidenceTermInterfaceMismatch(
            evidence_term_id(1),
        ))
    };

    // --- declaration roster axes ------------------------------------------

    // A declaration count lying about its rows reads the application
    // section's bytes as a third declaration and dies inside it.
    rejected(
        "a declaration roster count one over",
        &put_u32(spans.declaration_count.clone(), 3),
        CodecError::InvalidTag("PropositionBinderKind", 0),
    );
    // Dropping either row desynchronizes the dense one-based identities or
    // strands the application that joined it.
    rejected(
        "a dropped fact-only declaration",
        &drop_row(
            spans.declaration_count.clone(),
            fact_declaration.row.clone(),
        ),
        CodecError::InvalidModule(ModuleError::NonDensePropositionDeclaration {
            expected: proposition_id(1),
            actual: proposition_id(2),
        }),
    );
    rejected(
        "a dropped witness declaration",
        &drop_row(
            spans.declaration_count.clone(),
            witness_declaration.row.clone(),
        ),
        CodecError::InvalidModule(ModuleError::UnknownPropositionDeclaration(proposition_id(
            2,
        ))),
    );
    rejected(
        "a cleared declaration roster",
        &excise(
            spans.declaration_count.clone(),
            fact_declaration.row.start..witness_declaration.row.end,
            0,
        ),
        CodecError::InvalidModule(ModuleError::UnknownPropositionDeclaration(proposition_id(
            1,
        ))),
    );
    // A duplicated row is not strictly increasing by semantic identity; a
    // swapped roster puts the witness declaration's name ahead of the
    // fact-only declaration's.
    rejected(
        "a duplicated declaration row",
        &duplicate_row(
            spans.declaration_count.clone(),
            fact_declaration.row.clone(),
        ),
        CodecError::NonCanonicalOrder("proposition declarations by semantic identity"),
    );
    rejected(
        "a reordered declaration roster",
        &swap_rows(
            fact_declaration.row.clone(),
            witness_declaration.row.clone(),
        ),
        CodecError::NonCanonicalOrder("proposition declarations by semantic identity"),
    );

    // --- declaration identities and names ---------------------------------

    rejected(
        "a zero declaration identity",
        &put_u64(fact_declaration.id.clone(), 0),
        CodecError::ZeroIdentity("PropositionId"),
    );
    rejected(
        "a declaration identity reordered against its peer",
        &put_u64(fact_declaration.id.clone(), 3),
        CodecError::NonCanonicalOrder("proposition declarations by PropositionId"),
    );
    rejected(
        "a declaration identity lifted off the dense sequence",
        &put_u64(witness_declaration.id.clone(), 9),
        CodecError::InvalidModule(ModuleError::NonDensePropositionDeclaration {
            expected: proposition_id(2),
            actual: proposition_id(9),
        }),
    );
    divergent(
        "a renamed fact-only declaration",
        &restring(&fact_declaration.name, "example::Arranged"),
    );
    divergent(
        "a renamed witness declaration",
        &restring(&witness_declaration.name, "example::Settled"),
    );
    // Equal names keep the id-normalized rows strictly increasing — the
    // fact-only row's emptier shape still sorts first — so the collision
    // reaches the unique-name check.
    rejected(
        "a declaration name colliding with its peer",
        &restring(&fact_declaration.name, "example::Closed"),
        CodecError::InvalidModule(ModuleError::DuplicatePropositionName(
            "example::Closed".into(),
        )),
    );
    rejected(
        "a witness declaration name colliding with its peer",
        &restring(&witness_declaration.name, "example::Bounded"),
        CodecError::InvalidModule(ModuleError::DuplicatePropositionName(
            "example::Bounded".into(),
        )),
    );
    rejected(
        "an emptied declaration name",
        &restring(&fact_declaration.name, ""),
        CodecError::InvalidModule(ModuleError::EmptyPropositionIdentity),
    );
    // An emptied witness-declaration name sorts the normalized row below its
    // peer — the empty string precedes "example::Bounded" — so the canonical
    // ordering check rejects before the unique/empty-name check can fire.
    rejected(
        "an emptied witness declaration name",
        &restring(&witness_declaration.name, ""),
        CodecError::NonCanonicalOrder("proposition declarations by semantic identity"),
    );
    rejected(
        "a declaration name with corrupted UTF-8",
        &corrupt_utf8(&fact_declaration.name),
        CodecError::InvalidUtf8("proposition name"),
    );
    rejected(
        "a declaration name length beyond the content bound",
        &put_u32(fact_declaration.name.len.clone(), u32::MAX),
        CodecError::StringTooLong("proposition name"),
    );

    // --- fact-only declaration rosters and evidence --------------------------

    // A binder count lying on the empty roster reads the parameter count as
    // a name, the evidence tag as a kind, and wanders into the next row.
    rejected(
        "a fact-only binder count one over",
        &put_u32(fact_declaration.binder_count.clone(), 1),
        CodecError::InvalidTag("PropositionEvidence", 4),
    );
    rejected(
        "a fact-only parameter count one over",
        &put_u32(fact_declaration.parameter_count.clone(), 1),
        CodecError::InvalidTag("PropositionEvidence", 101),
    );
    rejected(
        "an unknown fact-only evidence classifier",
        &put_u8(fact_declaration.evidence_tag.clone(), 0),
        CodecError::InvalidTag("PropositionEvidence", 0),
    );
    rejected(
        "an out-of-range fact-only evidence classifier",
        &put_u8(fact_declaration.evidence_tag.clone(), 3),
        CodecError::InvalidTag("PropositionEvidence", 3),
    );
    // A fact-only declaration recast as a witness reads the next
    // declaration's bytes as its evidence type and starves the roster.
    rejected(
        "a fact-only declaration recast as a witness",
        &put_u8(fact_declaration.evidence_tag.clone(), 2),
        CodecError::StringTooLong("proposition name"),
    );

    // --- witness declaration binders -----------------------------------------

    // One binder over reads the parameter-type count as a binder name and
    // dies on the kind tag.
    rejected(
        "a witness binder count one over",
        &put_u32(witness_declaration.binder_count.clone(), 5),
        CodecError::InvalidTag("PropositionBinderKind", 0),
    );
    rejected(
        "a dropped declaration binder",
        &drop_row(
            witness_declaration.binder_count.clone(),
            witness_declaration.binders[0].row.clone(),
        ),
        CodecError::InvalidModule(ModuleError::PropositionApplicationArityMismatch(
            proposition_id(2),
        )),
    );
    rejected(
        "a duplicated declaration binder",
        &duplicate_row(
            witness_declaration.binder_count.clone(),
            witness_declaration.binders[0].row.clone(),
        ),
        CodecError::InvalidModule(ModuleError::InvalidPropositionBinder(proposition_id(2))),
    );
    divergent(
        "a renamed declaration binder",
        &restring(&witness_declaration.binders[0].name, "U"),
    );
    rejected(
        "a declaration binder name colliding with its peer",
        &restring(&witness_declaration.binders[0].name, "n"),
        CodecError::InvalidModule(ModuleError::InvalidPropositionBinder(proposition_id(2))),
    );
    rejected(
        "an emptied declaration binder name",
        &restring(&witness_declaration.binders[0].name, ""),
        CodecError::InvalidModule(ModuleError::InvalidPropositionBinder(proposition_id(2))),
    );
    rejected(
        "a declaration binder name with corrupted UTF-8",
        &corrupt_utf8(&witness_declaration.binders[0].name),
        CodecError::InvalidUtf8("proposition binder name"),
    );
    // A `Type` binder recast as `Machine` decodes in place but no longer
    // matches the application's type argument; recast as `Const` it eats the
    // next row's bytes as its type identity and starves the stream.
    rejected(
        "a type binder recast as a machine binder",
        &put_u8(witness_declaration.binders[0].kind.clone(), 3),
        CodecError::InvalidModule(ModuleError::PropositionApplicationBinderMismatch(
            proposition_id(2),
        )),
    );
    rejected(
        "a machine binder recast as a type binder",
        &put_u8(witness_declaration.binders[3].kind.clone(), 1),
        CodecError::InvalidModule(ModuleError::PropositionApplicationBinderMismatch(
            proposition_id(2),
        )),
    );
    rejected(
        "a type binder recast as a const binder",
        &put_u8(witness_declaration.binders[0].kind.clone(), 2),
        CodecError::UnexpectedEnd,
    );
    rejected(
        "a const binder recast as a type binder",
        &put_u8(witness_declaration.binders[1].kind.clone(), 1),
        CodecError::StringTooLong("proposition binder name"),
    );
    rejected(
        "an unknown declaration binder kind",
        &put_u8(witness_declaration.binders[0].kind.clone(), 0),
        CodecError::InvalidTag("PropositionBinderKind", 0),
    );
    rejected(
        "a machine binder with an out-of-range kind tag",
        &put_u8(witness_declaration.binders[2].kind.clone(), 7),
        CodecError::InvalidTag("PropositionBinderKind", 7),
    );
    divergent(
        "a renamed const binder type",
        &restring(
            witness_declaration.binders[1].const_type.as_ref().unwrap(),
            "example::Natural",
        ),
    );
    rejected(
        "an emptied const binder type",
        &restring(
            witness_declaration.binders[1].const_type.as_ref().unwrap(),
            "",
        ),
        CodecError::InvalidModule(ModuleError::InvalidPropositionBinder(proposition_id(2))),
    );
    rejected(
        "a const binder type with corrupted UTF-8",
        &corrupt_utf8(witness_declaration.binders[1].const_type.as_ref().unwrap()),
        CodecError::InvalidUtf8("proposition const binder type"),
    );

    // --- witness declaration parameter types and evidence --------------------

    // One parameter type over reads the evidence tag and witness type length
    // as a string bound and starves the stream.
    rejected(
        "a parameter-type count one over",
        &put_u32(witness_declaration.parameter_count.clone(), 3),
        CodecError::UnexpectedEnd,
    );
    rejected(
        "a dropped declaration parameter type",
        &drop_row(
            witness_declaration.parameter_count.clone(),
            witness_declaration.parameter_types[0].len.start
                ..witness_declaration.parameter_types[0].value.end,
        ),
        CodecError::InvalidModule(ModuleError::PropositionApplicationArityMismatch(
            proposition_id(2),
        )),
    );
    divergent(
        "a renamed declaration parameter type",
        &restring(&witness_declaration.parameter_types[0], "example::Thing"),
    );
    rejected(
        "an emptied declaration parameter type",
        &restring(&witness_declaration.parameter_types[0], ""),
        CodecError::InvalidModule(ModuleError::EmptyPropositionIdentity),
    );
    rejected(
        "a declaration parameter type with corrupted UTF-8",
        &corrupt_utf8(&witness_declaration.parameter_types[0]),
        CodecError::InvalidUtf8("proposition parameter type"),
    );
    rejected(
        "an unknown witness evidence classifier",
        &put_u8(witness_declaration.evidence_tag.clone(), 0),
        CodecError::InvalidTag("PropositionEvidence", 0),
    );
    divergent(
        "a renamed witness evidence type",
        &restring(
            witness_declaration.evidence_type.as_ref().unwrap(),
            "example::HeldProof",
        ),
    );
    rejected(
        "an emptied witness evidence type",
        &restring(witness_declaration.evidence_type.as_ref().unwrap(), ""),
        CodecError::InvalidModule(ModuleError::EmptyPropositionIdentity),
    );
    rejected(
        "a witness evidence type with corrupted UTF-8",
        &corrupt_utf8(witness_declaration.evidence_type.as_ref().unwrap()),
        CodecError::InvalidUtf8("proposition evidence type"),
    );

    // --- application roster axes --------------------------------------------

    // One application over reads the evidence-term section as a third row
    // and dies inside it.
    rejected(
        "an application roster count one over",
        &put_u32(spans.application_count.clone(), 3),
        CodecError::StringTooLong("proposition argument"),
    );
    rejected(
        "a dropped fact-only application",
        &drop_row(
            spans.application_count.clone(),
            fact_application.row.clone(),
        ),
        CodecError::InvalidModule(ModuleError::NonDensePropositionApplication {
            expected: proposition_id(1),
            actual: proposition_id(2),
        }),
    );
    // Dropping the witness application strands the evidence term's
    // proposition join; clearing the roster strands it the same way.
    rejected(
        "a dropped witness application",
        &drop_row(
            spans.application_count.clone(),
            witness_application.row.clone(),
        ),
        CodecError::InvalidModule(ModuleError::UnknownEvidenceTermProposition(proposition_id(
            2,
        ))),
    );
    rejected(
        "a cleared application roster",
        &excise(
            spans.application_count.clone(),
            fact_application.row.start..witness_application.row.end,
            0,
        ),
        CodecError::InvalidModule(ModuleError::UnknownEvidenceTermProposition(proposition_id(
            2,
        ))),
    );
    rejected(
        "a duplicated application row",
        &duplicate_row(
            spans.application_count.clone(),
            fact_application.row.clone(),
        ),
        CodecError::NonCanonicalOrder("proposition applications by semantic identity"),
    );
    rejected(
        "a reordered application roster",
        &swap_rows(
            fact_application.row.clone(),
            witness_application.row.clone(),
        ),
        CodecError::NonCanonicalOrder("proposition applications by semantic identity"),
    );

    // --- application identities and declaration join --------------------------

    rejected(
        "a zero application identity",
        &put_u64(fact_application.id.clone(), 0),
        CodecError::ZeroIdentity("PropositionId"),
    );
    rejected(
        "an application identity reordered against its peer",
        &put_u64(fact_application.id.clone(), 3),
        CodecError::NonCanonicalOrder("proposition applications by PropositionId"),
    );
    rejected(
        "an application identity lifted off the dense sequence",
        &put_u64(witness_application.id.clone(), 9),
        CodecError::InvalidModule(ModuleError::NonDensePropositionApplication {
            expected: proposition_id(2),
            actual: proposition_id(9),
        }),
    );
    rejected(
        "a zero application declaration join",
        &put_u64(witness_application.declaration.clone(), 0),
        CodecError::ZeroIdentity("PropositionId"),
    );
    rejected(
        "an application joined to an unknown declaration",
        &put_u64(witness_application.declaration.clone(), 9),
        CodecError::InvalidModule(ModuleError::UnknownPropositionDeclaration(proposition_id(
            9,
        ))),
    );
    rejected(
        "an application rebound to the fact-only declaration",
        &put_u64(witness_application.declaration.clone(), 1),
        CodecError::InvalidModule(ModuleError::PropositionApplicationArityMismatch(
            proposition_id(2),
        )),
    );
    rejected(
        "a fact-only application rebound to the witness declaration",
        &put_u64(fact_application.declaration.clone(), 2),
        CodecError::InvalidModule(ModuleError::PropositionApplicationArityMismatch(
            proposition_id(1),
        )),
    );

    // --- fact-only application rosters -----------------------------------------

    // The empty binder-argument and argument rosters misread their
    // neighbours when the counts lie.
    rejected(
        "a fact-only binder-argument count one over",
        &put_u32(fact_application.binder_count.clone(), 1),
        CodecError::InvalidTag("PropositionBinderArgumentKind", 0),
    );
    rejected(
        "a fact-only application argument count one over",
        &put_u32(fact_application.argument_count.clone(), 1),
        CodecError::ZeroIdentity("PropositionId"),
    );
    rejected(
        "an unknown fact-only application interface flag",
        &put_u8(fact_application.interface_flag.clone(), 2),
        CodecError::InvalidTag("PropositionEvidenceInterface", 2),
    );
    // A fact-only application granted an interface reads the witness
    // application's bytes as its trait surface and starves the stream.
    rejected(
        "a fact-only application granted an interface",
        &put_u8(fact_application.interface_flag.clone(), 1),
        CodecError::UnexpectedEnd,
    );

    // --- witness application binder arguments ------------------------------------

    // One binder argument over reads the argument count's bytes as a kind
    // tag and an over-long identity.
    rejected(
        "a binder-argument count one over",
        &put_u32(witness_application.binder_count.clone(), 5),
        CodecError::UnexpectedEnd,
    );
    rejected(
        "a dropped binder argument",
        &drop_row(
            witness_application.binder_count.clone(),
            witness_application.binder_arguments[0].row.clone(),
        ),
        CodecError::InvalidModule(ModuleError::PropositionApplicationArityMismatch(
            proposition_id(2),
        )),
    );
    rejected(
        "a type binder argument recast as a const argument",
        &put_u8(witness_application.binder_arguments[0].kind.clone(), 2),
        CodecError::InvalidModule(ModuleError::PropositionApplicationBinderMismatch(
            proposition_id(2),
        )),
    );
    rejected(
        "a machine binder argument recast as a type argument",
        &put_u8(witness_application.binder_arguments[3].kind.clone(), 1),
        CodecError::InvalidModule(ModuleError::PropositionApplicationBinderMismatch(
            proposition_id(2),
        )),
    );
    rejected(
        "a projected binder argument recast as a type argument",
        &put_u8(witness_application.binder_arguments[2].kind.clone(), 1),
        CodecError::InvalidModule(ModuleError::PropositionApplicationBinderMismatch(
            proposition_id(2),
        )),
    );
    rejected(
        "an unknown binder argument kind",
        &put_u8(witness_application.binder_arguments[0].kind.clone(), 0),
        CodecError::InvalidTag("PropositionBinderArgumentKind", 0),
    );
    rejected(
        "an out-of-range binder argument kind",
        &put_u8(witness_application.binder_arguments[1].kind.clone(), 9),
        CodecError::InvalidTag("PropositionBinderArgumentKind", 9),
    );
    divergent(
        "a renamed type binder argument",
        &restring(
            witness_application.binder_arguments[0]
                .identity
                .as_ref()
                .unwrap(),
            "example::Box",
        ),
    );
    divergent(
        "a renamed const binder argument",
        &restring(
            witness_application.binder_arguments[1]
                .identity
                .as_ref()
                .unwrap(),
            "example::four",
        ),
    );
    divergent(
        "a renamed machine binder argument",
        &restring(
            witness_application.binder_arguments[3]
                .identity
                .as_ref()
                .unwrap(),
            "example::worker",
        ),
    );
    rejected(
        "an emptied binder argument identity",
        &restring(
            witness_application.binder_arguments[0]
                .identity
                .as_ref()
                .unwrap(),
            "",
        ),
        CodecError::InvalidModule(ModuleError::PropositionApplicationBinderMismatch(
            proposition_id(2),
        )),
    );
    rejected(
        "a binder argument identity with corrupted UTF-8",
        &corrupt_utf8(
            witness_application.binder_arguments[0]
                .identity
                .as_ref()
                .unwrap(),
        ),
        CodecError::InvalidUtf8("proposition binder argument"),
    );
    // An ordinary argument reclassified as a projection reads its identity
    // length and leading bytes as the term identity, then its tail as a
    // declaring-trait bound.
    rejected(
        "an ordinary binder argument reclassified as a projection",
        &put_u8(
            witness_application.binder_arguments[0]
                .projection_flag
                .clone(),
            1,
        ),
        CodecError::StringTooLong("evidence projection declaring trait"),
    );
    rejected(
        "a machine binder argument reclassified as a projection",
        &put_u8(
            witness_application.binder_arguments[3]
                .projection_flag
                .clone(),
            1,
        ),
        CodecError::StringTooLong("evidence projection declaring trait"),
    );
    rejected(
        "an unknown binder argument projection flag",
        &put_u8(
            witness_application.binder_arguments[0]
                .projection_flag
                .clone(),
            2,
        ),
        CodecError::InvalidTag("PropositionBinderArgument", 2),
    );
    // A projected argument reclassified as ordinary reads the term identity
    // as a one-byte string and the following bytes as the next argument's
    // kind.
    rejected(
        "a projected binder argument reclassified as ordinary",
        &put_u8(
            witness_application.binder_arguments[2]
                .projection_flag
                .clone(),
            0,
        ),
        CodecError::InvalidTag("PropositionBinderArgumentKind", 0),
    );

    // --- evidence projection fields -------------------------------------------

    let projection = witness_application.binder_arguments[2]
        .projection
        .as_ref()
        .unwrap();
    rejected(
        "a zero projection term identity",
        &put_u64(projection.term.clone(), 0),
        CodecError::ZeroIdentity("EvidenceTermId"),
    );
    rejected(
        "a projection joined to an unknown term",
        &put_u64(projection.term.clone(), 9),
        CodecError::InvalidModule(ModuleError::UnknownEvidenceProjectionTerm {
            proposition: proposition_id(2),
            term: evidence_term_id(9),
        }),
    );
    rejected(
        "a projection declaring trait emptied",
        &restring(&projection.declaring_trait, ""),
        CodecError::InvalidModule(ModuleError::PropositionApplicationBinderMismatch(
            proposition_id(2),
        )),
    );
    rejected(
        "a projection declaring trait off the term's requirement surface",
        &restring(&projection.declaring_trait, "example::Other"),
        CodecError::InvalidModule(ModuleError::EvidenceProjectionRequirementMismatch {
            proposition: proposition_id(2),
            term: evidence_term_id(1),
        }),
    );
    rejected(
        "a projection declaring trait with corrupted UTF-8",
        &corrupt_utf8(&projection.declaring_trait),
        CodecError::InvalidUtf8("evidence projection declaring trait"),
    );
    // Clearing the projection's declaring-trait arguments keeps the row
    // aligned — the requirement string slides into the vacated slot — but
    // the rewritten triple no longer matches the term's requirement.
    rejected(
        "a cleared projection declaring-trait argument roster",
        &excise(
            projection.argument_count.clone(),
            projection.arguments[0].len.start..projection.arguments[0].value.end,
            0,
        ),
        CodecError::InvalidModule(ModuleError::EvidenceProjectionRequirementMismatch {
            proposition: proposition_id(2),
            term: evidence_term_id(1),
        }),
    );
    rejected(
        "a projection declaring-trait argument off the term's surface",
        &restring(&projection.arguments[0], "example::Other"),
        CodecError::InvalidModule(ModuleError::EvidenceProjectionRequirementMismatch {
            proposition: proposition_id(2),
            term: evidence_term_id(1),
        }),
    );
    rejected(
        "a projection declaring-trait argument emptied",
        &restring(&projection.arguments[0], ""),
        CodecError::InvalidModule(ModuleError::PropositionApplicationBinderMismatch(
            proposition_id(2),
        )),
    );
    rejected(
        "a projection requirement off the term's surface",
        &restring(&projection.requirement, "example::provable::other"),
        CodecError::InvalidModule(ModuleError::EvidenceProjectionRequirementMismatch {
            proposition: proposition_id(2),
            term: evidence_term_id(1),
        }),
    );
    rejected(
        "a projection requirement emptied",
        &restring(&projection.requirement, ""),
        CodecError::InvalidModule(ModuleError::PropositionApplicationBinderMismatch(
            proposition_id(2),
        )),
    );
    rejected(
        "a projection requirement with corrupted UTF-8",
        &corrupt_utf8(&projection.requirement),
        CodecError::InvalidUtf8("evidence projection requirement"),
    );

    // --- witness application arguments -------------------------------------------

    // One argument over reads the interface flag and trait length as a
    // string bound and starves the stream.
    rejected(
        "an application argument count one over",
        &put_u32(witness_application.argument_count.clone(), 3),
        CodecError::UnexpectedEnd,
    );
    rejected(
        "a dropped application argument",
        &drop_row(
            witness_application.argument_count.clone(),
            witness_application.arguments[0].len.start..witness_application.arguments[0].value.end,
        ),
        CodecError::InvalidModule(ModuleError::PropositionApplicationArityMismatch(
            proposition_id(2),
        )),
    );
    divergent(
        "a renamed application argument",
        &restring(&witness_application.arguments[0], "example::Box"),
    );
    divergent(
        "a renamed second application argument",
        &restring(&witness_application.arguments[1], "example::four"),
    );
    rejected(
        "an emptied application argument",
        &restring(&witness_application.arguments[0], ""),
        CodecError::InvalidModule(ModuleError::EmptyPropositionIdentity),
    );
    rejected(
        "an application argument with corrupted UTF-8",
        &corrupt_utf8(&witness_application.arguments[0]),
        CodecError::InvalidUtf8("proposition argument"),
    );

    // --- witness application evidence interface -----------------------------------

    // Every interface field of the witness application is double-joined:
    // the declaration's classifier requires a valid interface, and the
    // evidence term requires an identical one. No single-field substitution
    // stays representable.
    let application_interface = witness_application.interface.as_ref().unwrap();
    // A witness application stripped of its interface flag leaves the
    // interface bytes behind: the evidence-term count lands on the trait
    // length and the desynchronized walk starves the stream before module
    // validation can name the missing interface.
    rejected(
        "a witness application stripped of its interface",
        &put_u8(witness_application.interface_flag.clone(), 0),
        CodecError::UnexpectedEnd,
    );
    rejected(
        "an unknown application interface flag",
        &put_u8(witness_application.interface_flag.clone(), 2),
        CodecError::InvalidTag("PropositionEvidenceInterface", 2),
    );
    rejected(
        "an emptied interface trait",
        &restring(&application_interface.trait_identity, ""),
        CodecError::InvalidModule(ModuleError::InvalidPropositionEvidenceInterface(
            proposition_id(2),
        )),
    );
    rejected(
        "a renamed interface trait off the term's surface",
        &restring(&application_interface.trait_identity, "example::Other"),
        interface(),
    );
    rejected(
        "an interface trait with corrupted UTF-8",
        &corrupt_utf8(&application_interface.trait_identity),
        CodecError::InvalidUtf8("evidence interface trait identity"),
    );
    rejected(
        "an interface argument count one over",
        &put_u32(application_interface.argument_count.clone(), 2),
        CodecError::UnexpectedEnd,
    );
    rejected(
        "a renamed interface argument off the term's surface",
        &restring(&application_interface.arguments[0], "example::Other"),
        interface(),
    );
    rejected(
        "an emptied interface argument",
        &restring(&application_interface.arguments[0], ""),
        CodecError::InvalidModule(ModuleError::InvalidPropositionEvidenceInterface(
            proposition_id(2),
        )),
    );
    // Clearing the requirements keeps the interface well-formed but breaks
    // the term join; one requirement over reads the evidence-term section
    // and starves the stream.
    rejected(
        "a cleared interface requirement roster",
        &excise(
            application_interface.requirement_count.clone(),
            application_interface.requirements[0].row.clone(),
            0,
        ),
        interface(),
    );
    rejected(
        "an interface requirement count one over",
        &put_u32(application_interface.requirement_count.clone(), 2),
        CodecError::StringTooLong("evidence requirement identity"),
    );
    let requirement = &application_interface.requirements[0];
    rejected(
        "a renamed interface requirement trait off the term's surface",
        &restring(&requirement.declaring_trait, "example::Other"),
        interface(),
    );
    rejected(
        "an emptied interface requirement trait",
        &restring(&requirement.declaring_trait, ""),
        CodecError::InvalidModule(ModuleError::InvalidPropositionEvidenceInterface(
            proposition_id(2),
        )),
    );
    rejected(
        "an interface requirement argument count one over",
        &put_u32(requirement.argument_count.clone(), 2),
        CodecError::UnexpectedEnd,
    );
    rejected(
        "a renamed interface requirement argument off the term's surface",
        &restring(&requirement.arguments[0], "example::Other"),
        interface(),
    );
    rejected(
        "an emptied interface requirement argument",
        &restring(&requirement.arguments[0], ""),
        CodecError::InvalidModule(ModuleError::InvalidPropositionEvidenceInterface(
            proposition_id(2),
        )),
    );
    rejected(
        "a renamed interface requirement identity off the term's surface",
        &restring(&requirement.requirement, "example::provable::other"),
        interface(),
    );
    rejected(
        "an emptied interface requirement identity",
        &restring(&requirement.requirement, ""),
        CodecError::InvalidModule(ModuleError::InvalidPropositionEvidenceInterface(
            proposition_id(2),
        )),
    );
    rejected(
        "an interface requirement identity with corrupted UTF-8",
        &corrupt_utf8(&requirement.requirement),
        CodecError::InvalidUtf8("evidence requirement identity"),
    );

    // --- evidence terms --------------------------------------------------------

    // One term over reads the lane roster as a second term and starves the
    // stream; a maximal count trips the counted-decoder capacity guard.
    rejected(
        "an evidence-term roster count one over",
        &put_u32(spans.term_count.clone(), 2),
        CodecError::UnexpectedEnd,
    );
    rejected(
        "a maximal evidence-term roster count",
        &put_u32(spans.term_count.clone(), u32::MAX),
        CodecError::UnexpectedEnd,
    );
    // Clearing the terms strands the projection's term join before the
    // lanes' — the projection is validated with its application.
    rejected(
        "a cleared evidence-term roster",
        &excise(spans.term_count.clone(), term.row.clone(), 0),
        CodecError::InvalidModule(ModuleError::UnknownEvidenceProjectionTerm {
            proposition: proposition_id(2),
            term: evidence_term_id(1),
        }),
    );
    rejected(
        "a duplicated evidence term",
        &duplicate_row(spans.term_count.clone(), term.row.clone()),
        CodecError::NonCanonicalOrder("evidence terms by proposition and EvidenceTermId"),
    );
    rejected(
        "a zero evidence-term identity",
        &put_u64(term.id.clone(), 0),
        CodecError::ZeroIdentity("EvidenceTermId"),
    );
    rejected(
        "an evidence-term identity lifted off the dense sequence",
        &put_u64(term.id.clone(), 9),
        CodecError::InvalidModule(ModuleError::NonDenseEvidenceTerm {
            expected: evidence_term_id(1),
            actual: evidence_term_id(9),
        }),
    );
    rejected(
        "a zero evidence-term proposition",
        &put_u64(term.proposition.clone(), 0),
        CodecError::ZeroIdentity("PropositionId"),
    );
    rejected(
        "an evidence term joined to an unknown proposition",
        &put_u64(term.proposition.clone(), 9),
        CodecError::InvalidModule(ModuleError::UnknownEvidenceTermProposition(proposition_id(
            9,
        ))),
    );
    rejected(
        "an evidence term joined to a fact-only proposition",
        &put_u64(term.proposition.clone(), 1),
        CodecError::InvalidModule(ModuleError::FactOnlyEvidenceTerm(proposition_id(1))),
    );
    rejected(
        "a term interface trait off the application's surface",
        &restring(&term.interface.trait_identity, "example::Other"),
        interface(),
    );
    rejected(
        "a term interface trait emptied",
        &restring(&term.interface.trait_identity, ""),
        CodecError::InvalidModule(ModuleError::InvalidEvidenceInterface(evidence_term_id(1))),
    );
    rejected(
        "a term interface trait with corrupted UTF-8",
        &corrupt_utf8(&term.interface.trait_identity),
        CodecError::InvalidUtf8("evidence interface trait identity"),
    );
    rejected(
        "a term interface argument off the application's surface",
        &restring(&term.interface.arguments[0], "example::Other"),
        interface(),
    );
    rejected(
        "a term interface argument emptied",
        &restring(&term.interface.arguments[0], ""),
        CodecError::InvalidModule(ModuleError::InvalidEvidenceInterface(evidence_term_id(1))),
    );
    rejected(
        "a cleared term requirement roster",
        &excise(
            term.interface.requirement_count.clone(),
            term.interface.requirements[0].row.clone(),
            0,
        ),
        interface(),
    );
    rejected(
        "a term requirement trait off the application's surface",
        &restring(
            &term.interface.requirements[0].declaring_trait,
            "example::Other",
        ),
        interface(),
    );
    rejected(
        "a term requirement argument off the application's surface",
        &restring(
            &term.interface.requirements[0].arguments[0],
            "example::Other",
        ),
        interface(),
    );
    rejected(
        "a term requirement identity emptied",
        &restring(&term.interface.requirements[0].requirement, ""),
        CodecError::InvalidModule(ModuleError::InvalidEvidenceInterface(evidence_term_id(1))),
    );

    // --- evidence contract lanes ------------------------------------------------

    // One lane over reads the proof-output section's zero counts as a zero
    // machine identity; a maximal count trips the counted-decoder capacity
    // guard.
    rejected(
        "a lane roster count one over",
        &put_u32(spans.lane_count.clone(), 3),
        CodecError::ZeroIdentity("MachineId"),
    );
    rejected(
        "a maximal lane roster count",
        &put_u32(spans.lane_count.clone(), u32::MAX),
        CodecError::UnexpectedEnd,
    );
    // Dropping the requires lane leaves the ensures lane unmatched: the
    // module is representationally sound, but independent verification
    // reports the term with no producer.
    verify_rejected(
        "a dropped requires lane leaving its ensures unmatched",
        &drop_row(spans.lane_count.clone(), requires_lane.row.clone()),
        VerificationError::MissingEvidenceProducer(evidence_term_id(1)),
    );
    divergent(
        "a dropped ensures lane",
        &drop_row(spans.lane_count.clone(), ensures_lane.row.clone()),
    );
    // Clearing the lanes alone orphans the term; clearing the vocabulary
    // wholesale stays representable.
    rejected(
        "a cleared lane roster",
        &excise(
            spans.lane_count.clone(),
            requires_lane.row.start..ensures_lane.row.end,
            0,
        ),
        CodecError::InvalidModule(ModuleError::OrphanEvidenceTerm(evidence_term_id(1))),
    );
    {
        let mut cleared = module.clone();
        cleared.proposition_declarations.clear();
        cleared.proposition_applications.clear();
        cleared.evidence_terms.clear();
        cleared.evidence_contract_lanes.clear();
        let cleared_bytes = encode_module(&cleared).expect("cleared vocabulary encodes");
        divergent("a cleared proposition vocabulary", &cleared_bytes);
    }
    rejected(
        "a duplicated lane row",
        &duplicate_row(spans.lane_count.clone(), requires_lane.row.clone()),
        CodecError::NonCanonicalOrder("evidence contract lanes by machine, kind, and position"),
    );
    rejected(
        "a reordered lane roster",
        &swap_rows(requires_lane.row.clone(), ensures_lane.row.clone()),
        CodecError::NonCanonicalOrder("evidence contract lanes by machine, kind, and position"),
    );
    rejected(
        "a zero lane machine identity",
        &put_u64(requires_lane.machine.clone(), 0),
        CodecError::ZeroIdentity("MachineId"),
    );
    // The ordering key leads with the machine: a larger machine on the
    // earlier lane reorders the roster, while a larger machine on the last
    // lane stays ordered and fails the join.
    rejected(
        "a lane machine reordered against its peer",
        &put_u64(requires_lane.machine.clone(), 9),
        CodecError::NonCanonicalOrder("evidence contract lanes by machine, kind, and position"),
    );
    rejected(
        "a lane joined to an unknown machine",
        &put_u64(ensures_lane.machine.clone(), 9),
        CodecError::InvalidModule(ModuleError::UnknownEvidenceContractMachine(machine_id(9))),
    );
    // A kind flip collides the (machine, kind, position) ordering key of
    // both rows.
    rejected(
        "a requires lane recast as ensures",
        &put_u8(requires_lane.kind.clone(), 2),
        CodecError::NonCanonicalOrder("evidence contract lanes by machine, kind, and position"),
    );
    rejected(
        "an ensures lane recast as requires",
        &put_u8(ensures_lane.kind.clone(), 1),
        CodecError::NonCanonicalOrder("evidence contract lanes by machine, kind, and position"),
    );
    rejected(
        "an unknown lane kind",
        &put_u8(requires_lane.kind.clone(), 0),
        CodecError::InvalidTag("EvidenceContractLaneKind", 0),
    );
    rejected(
        "a lane position lifted off its dense sequence",
        &put_u32(requires_lane.position.clone(), 7),
        CodecError::InvalidModule(ModuleError::NonDenseEvidenceContractLane {
            machine: machine_id(1),
            kind: EvidenceContractLaneKind::Requires,
            expected: 0,
            actual: 7,
        }),
    );
    rejected(
        "an ensures lane position lifted off its dense sequence",
        &put_u32(ensures_lane.position.clone(), 7),
        CodecError::InvalidModule(ModuleError::NonDenseEvidenceContractLane {
            machine: machine_id(1),
            kind: EvidenceContractLaneKind::Ensures,
            expected: 0,
            actual: 7,
        }),
    );
    rejected(
        "a zero lane term identity",
        &put_u64(requires_lane.term.clone(), 0),
        CodecError::ZeroIdentity("EvidenceTermId"),
    );
    rejected(
        "a lane joined to an unknown term",
        &put_u64(requires_lane.term.clone(), 9),
        CodecError::InvalidModule(ModuleError::UnknownEvidenceContractTerm(evidence_term_id(
            9,
        ))),
    );
    // A requires lane granted an output flag reads the next lane's machine
    // bytes as a one-byte field and desynchronizes the roster; an ensures
    // lane stripped of its flag leaves its field bytes behind for the
    // proof-output section to misread.
    rejected(
        "a requires lane granted an output flag",
        &put_u8(requires_lane.output_flag.clone(), 1),
        CodecError::InvalidBoolean(118),
    );
    rejected(
        "an ensures lane stripped of its output flag",
        &put_u8(ensures_lane.output_flag.clone(), 0),
        CodecError::ZeroIdentity("MachineId"),
    );
    rejected(
        "an unknown lane output flag",
        &put_u8(ensures_lane.output_flag.clone(), 2),
        CodecError::InvalidBoolean(2),
    );
    divergent(
        "a renamed ensures output field",
        &restring(ensures_lane.output_field.as_ref().unwrap(), "finding"),
    );
    rejected(
        "an emptied ensures output field",
        &restring(ensures_lane.output_field.as_ref().unwrap(), ""),
        CodecError::InvalidModule(ModuleError::InvalidEvidenceOutputField(machine_id(1))),
    );
    rejected(
        "an ensures output field named the reserved result carrier",
        &restring(ensures_lane.output_field.as_ref().unwrap(), "value"),
        CodecError::InvalidModule(ModuleError::ReservedEvidenceOutputField(machine_id(1))),
    );
    rejected(
        "an ensures output field with corrupted UTF-8",
        &corrupt_utf8(ensures_lane.output_field.as_ref().unwrap()),
        CodecError::InvalidUtf8("evidence output field"),
    );

    // --- producer-side rejections -------------------------------------------

    // Every validation failure above also fails closed on the way out: the
    // canonical encoder runs the same module validation before emitting
    // bytes.
    let mut changed = module.clone();
    changed.proposition_declarations.swap(0, 1);
    encode_rejected(
        "a producer-side reordered declaration roster",
        &changed,
        CodecError::NonCanonicalOrder("proposition declarations by semantic identity"),
    );
    let mut changed = module.clone();
    changed.proposition_declarations[0].id = proposition_id(3);
    encode_rejected(
        "a producer-side declaration identity reordered against its peer",
        &changed,
        CodecError::NonCanonicalOrder("proposition declarations by PropositionId"),
    );
    let mut changed = module.clone();
    changed
        .proposition_declarations
        .push(changed.proposition_declarations[0].clone());
    encode_rejected(
        "a producer-side duplicated declaration row",
        &changed,
        CodecError::NonCanonicalOrder("proposition declarations by semantic identity"),
    );
    let mut changed = module.clone();
    changed.proposition_declarations[1].id = proposition_id(9);
    encode_rejected(
        "a producer-side declaration identity off the dense sequence",
        &changed,
        CodecError::InvalidModule(ModuleError::NonDensePropositionDeclaration {
            expected: proposition_id(2),
            actual: proposition_id(9),
        }),
    );
    let mut changed = module.clone();
    changed.proposition_declarations[0].name.clear();
    encode_rejected(
        "a producer-side emptied declaration name",
        &changed,
        CodecError::InvalidModule(ModuleError::EmptyPropositionIdentity),
    );
    let mut changed = module.clone();
    changed.proposition_declarations[1].name = "example::Bounded".into();
    encode_rejected(
        "a producer-side declaration name colliding with its peer",
        &changed,
        CodecError::InvalidModule(ModuleError::DuplicatePropositionName(
            "example::Bounded".into(),
        )),
    );
    let mut changed = module.clone();
    changed.proposition_declarations[1].binders[0].name = "n".into();
    encode_rejected(
        "a producer-side binder name colliding with its peer",
        &changed,
        CodecError::InvalidModule(ModuleError::InvalidPropositionBinder(proposition_id(2))),
    );
    let mut changed = module.clone();
    changed.proposition_declarations[1].binders[0].kind = PropositionBinderKind::Machine;
    encode_rejected(
        "a producer-side type binder recast as a machine binder",
        &changed,
        CodecError::InvalidModule(ModuleError::PropositionApplicationBinderMismatch(
            proposition_id(2),
        )),
    );
    let mut changed = module.clone();
    changed.proposition_declarations[1].parameter_types.pop();
    encode_rejected(
        "a producer-side trimmed declaration parameter roster",
        &changed,
        CodecError::InvalidModule(ModuleError::PropositionApplicationArityMismatch(
            proposition_id(2),
        )),
    );
    let mut changed = module.clone();
    changed.proposition_declarations[0].evidence = PropositionEvidence::Witness {
        evidence_type: "example::BoundedProof".into(),
    };
    encode_rejected(
        "a producer-side fact-only declaration recast as a witness",
        &changed,
        CodecError::InvalidModule(ModuleError::InvalidPropositionEvidenceInterface(
            proposition_id(1),
        )),
    );
    let mut changed = module.clone();
    changed.proposition_declarations[1].evidence = PropositionEvidence::FactOnly;
    encode_rejected(
        "a producer-side witness declaration recast as fact-only",
        &changed,
        CodecError::InvalidModule(ModuleError::InvalidPropositionEvidenceInterface(
            proposition_id(2),
        )),
    );
    let mut changed = module.clone();
    changed.proposition_applications.swap(0, 1);
    encode_rejected(
        "a producer-side reordered application roster",
        &changed,
        CodecError::NonCanonicalOrder("proposition applications by semantic identity"),
    );
    let mut changed = module.clone();
    changed.proposition_applications[1].id = proposition_id(9);
    encode_rejected(
        "a producer-side application identity off the dense sequence",
        &changed,
        CodecError::InvalidModule(ModuleError::NonDensePropositionApplication {
            expected: proposition_id(2),
            actual: proposition_id(9),
        }),
    );
    let mut changed = module.clone();
    changed.proposition_applications[1].declaration = proposition_id(9);
    encode_rejected(
        "a producer-side application joined to an unknown declaration",
        &changed,
        CodecError::InvalidModule(ModuleError::UnknownPropositionDeclaration(proposition_id(
            9,
        ))),
    );
    let mut changed = module.clone();
    changed.proposition_applications[1].declaration = proposition_id(1);
    encode_rejected(
        "a producer-side application rebound to the fact-only declaration",
        &changed,
        CodecError::InvalidModule(ModuleError::PropositionApplicationArityMismatch(
            proposition_id(2),
        )),
    );
    let mut changed = module.clone();
    changed.proposition_applications[1].binder_arguments[0].kind =
        PropositionBinderArgumentKind::Const;
    encode_rejected(
        "a producer-side type binder argument recast as const",
        &changed,
        CodecError::InvalidModule(ModuleError::PropositionApplicationBinderMismatch(
            proposition_id(2),
        )),
    );
    let mut changed = module.clone();
    changed.proposition_applications[1].binder_arguments[0]
        .identity
        .clear();
    encode_rejected(
        "a producer-side emptied binder argument identity",
        &changed,
        CodecError::InvalidModule(ModuleError::PropositionApplicationBinderMismatch(
            proposition_id(2),
        )),
    );
    // A projection on a non-machine argument cannot satisfy the identity
    // rule; a projection onto an unknown term or a requirement off the
    // term's interface fails the join.
    let mut changed = module.clone();
    changed.proposition_applications[1].binder_arguments[0]
        .identity
        .clear();
    changed.proposition_applications[1].binder_arguments[0].evidence_projection =
        Some(binder_projection());
    encode_rejected(
        "a producer-side projection on a type binder argument",
        &changed,
        CodecError::InvalidModule(ModuleError::PropositionApplicationBinderMismatch(
            proposition_id(2),
        )),
    );
    let mut changed = module.clone();
    changed.proposition_applications[1].binder_arguments[2]
        .evidence_projection
        .as_mut()
        .unwrap()
        .term = evidence_term_id(9);
    encode_rejected(
        "a producer-side projection joined to an unknown term",
        &changed,
        CodecError::InvalidModule(ModuleError::UnknownEvidenceProjectionTerm {
            proposition: proposition_id(2),
            term: evidence_term_id(9),
        }),
    );
    let mut changed = module.clone();
    changed.proposition_applications[1].binder_arguments[2]
        .evidence_projection
        .as_mut()
        .unwrap()
        .requirement_identity = "example::provable::other".into();
    encode_rejected(
        "a producer-side projection requirement off the term's surface",
        &changed,
        CodecError::InvalidModule(ModuleError::EvidenceProjectionRequirementMismatch {
            proposition: proposition_id(2),
            term: evidence_term_id(1),
        }),
    );
    // The ordinary machine argument projected through the same term stays
    // representable end to end: it encodes, decodes, verifies, and diverges
    // the identity.
    let mut changed = module.clone();
    changed.proposition_applications[1].binder_arguments[3]
        .identity
        .clear();
    changed.proposition_applications[1].binder_arguments[3].evidence_projection =
        Some(binder_projection());
    let projected_bytes = encode_module(&changed).expect("projected argument encodes");
    divergent(
        "a producer-side machine binder argument projected through the term",
        &projected_bytes,
    );
    let mut changed = module.clone();
    changed.proposition_applications[1].arguments.pop();
    encode_rejected(
        "a producer-side trimmed application argument roster",
        &changed,
        CodecError::InvalidModule(ModuleError::PropositionApplicationArityMismatch(
            proposition_id(2),
        )),
    );
    let mut changed = module.clone();
    changed.proposition_applications[1].arguments[0].clear();
    encode_rejected(
        "a producer-side emptied application argument",
        &changed,
        CodecError::InvalidModule(ModuleError::EmptyPropositionIdentity),
    );
    let mut changed = module.clone();
    changed.proposition_applications[1].evidence_interface = None;
    encode_rejected(
        "a producer-side witness application stripped of its interface",
        &changed,
        CodecError::InvalidModule(ModuleError::InvalidPropositionEvidenceInterface(
            proposition_id(2),
        )),
    );
    let mut changed = module.clone();
    changed.proposition_applications[0].evidence_interface = Some(vocabulary_interface());
    encode_rejected(
        "a producer-side fact-only application granted an interface",
        &changed,
        CodecError::InvalidModule(ModuleError::InvalidPropositionEvidenceInterface(
            proposition_id(1),
        )),
    );
    let mut changed = module.clone();
    changed.proposition_applications[1]
        .evidence_interface
        .as_mut()
        .unwrap()
        .trait_identity
        .clear();
    encode_rejected(
        "a producer-side emptied interface trait",
        &changed,
        CodecError::InvalidModule(ModuleError::InvalidPropositionEvidenceInterface(
            proposition_id(2),
        )),
    );
    let mut changed = module.clone();
    changed.proposition_applications[1]
        .evidence_interface
        .as_mut()
        .unwrap()
        .requirements
        .push(EvidenceRequirementIdentity {
            declaring_trait_identity: "example::Provable".into(),
            declaring_trait_arguments: vec!["example::Cell".into()],
            requirement_identity: "example::provable::holds".into(),
        });
    encode_rejected(
        "a producer-side duplicated interface requirement",
        &changed,
        CodecError::InvalidModule(ModuleError::InvalidPropositionEvidenceInterface(
            proposition_id(2),
        )),
    );
    let mut changed = module.clone();
    changed.evidence_terms[0].proposition = proposition_id(9);
    encode_rejected(
        "a producer-side term joined to an unknown proposition",
        &changed,
        CodecError::InvalidModule(ModuleError::UnknownEvidenceTermProposition(proposition_id(
            9,
        ))),
    );
    let mut changed = module.clone();
    changed.evidence_terms[0].proposition = proposition_id(1);
    encode_rejected(
        "a producer-side term joined to a fact-only proposition",
        &changed,
        CodecError::InvalidModule(ModuleError::FactOnlyEvidenceTerm(proposition_id(1))),
    );
    let mut changed = module.clone();
    changed.evidence_terms[0].interface.trait_identity = "example::Other".into();
    encode_rejected(
        "a producer-side term interface off the application's surface",
        &changed,
        interface(),
    );
    let mut changed = module.clone();
    changed.evidence_terms[0].interface.trait_identity.clear();
    encode_rejected(
        "a producer-side emptied term interface trait",
        &changed,
        CodecError::InvalidModule(ModuleError::InvalidEvidenceInterface(evidence_term_id(1))),
    );
    // A second term inhabiting the same application but unused by any lane
    // is orphaned; a second term carrying a divergent interface fails the
    // application join.
    let mut changed = module.clone();
    changed.evidence_terms.push(EvidenceTermDeclaration {
        id: evidence_term_id(2),
        proposition: proposition_id(2),
        interface: vocabulary_interface(),
    });
    encode_rejected(
        "a producer-side evidence term no lane consumes",
        &changed,
        CodecError::InvalidModule(ModuleError::OrphanEvidenceTerm(evidence_term_id(2))),
    );
    let mut changed = module.clone();
    changed.evidence_terms.push(EvidenceTermDeclaration {
        id: evidence_term_id(2),
        proposition: proposition_id(2),
        interface: EvidenceInterfaceIdentity {
            trait_identity: "zz::Other".into(),
            arguments: Vec::new(),
            requirements: Vec::new(),
        },
    });
    encode_rejected(
        "a producer-side term interface diverging from its application",
        &changed,
        CodecError::InvalidModule(ModuleError::EvidenceTermInterfaceMismatch(
            evidence_term_id(2),
        )),
    );
    let mut changed = module.clone();
    changed.evidence_contract_lanes[0].machine = machine_id(9);
    encode_rejected(
        "a producer-side lane machine reordered against its peer",
        &changed,
        CodecError::NonCanonicalOrder("evidence contract lanes by machine, kind, and position"),
    );
    let mut changed = module.clone();
    changed.evidence_contract_lanes[1].machine = machine_id(9);
    encode_rejected(
        "a producer-side lane joined to an unknown machine",
        &changed,
        CodecError::InvalidModule(ModuleError::UnknownEvidenceContractMachine(machine_id(9))),
    );
    let mut changed = module.clone();
    changed.evidence_contract_lanes[0].term = evidence_term_id(9);
    encode_rejected(
        "a producer-side lane joined to an unknown term",
        &changed,
        CodecError::InvalidModule(ModuleError::UnknownEvidenceContractTerm(evidence_term_id(
            9,
        ))),
    );
    let mut changed = module.clone();
    changed.evidence_contract_lanes[0].kind = EvidenceContractLaneKind::Ensures;
    encode_rejected(
        "a producer-side requires lane recast as ensures",
        &changed,
        CodecError::NonCanonicalOrder("evidence contract lanes by machine, kind, and position"),
    );
    let mut changed = module.clone();
    changed.evidence_contract_lanes[0].position = u32::MAX;
    encode_rejected(
        "a producer-side lane position off its dense sequence",
        &changed,
        CodecError::InvalidModule(ModuleError::NonDenseEvidenceContractLane {
            machine: machine_id(1),
            kind: EvidenceContractLaneKind::Requires,
            expected: 0,
            actual: u32::MAX,
        }),
    );
    let mut changed = module.clone();
    changed.evidence_contract_lanes[0].output_field = Some("given".into());
    encode_rejected(
        "a producer-side requires lane granted an output field",
        &changed,
        CodecError::InvalidModule(ModuleError::EvidenceRequiresHasOutputField {
            machine: machine_id(1),
            position: 0,
        }),
    );
    let mut changed = module.clone();
    changed.evidence_contract_lanes[1].output_field = None;
    encode_rejected(
        "a producer-side ensures lane stripped of its output field",
        &changed,
        CodecError::InvalidModule(ModuleError::MissingEvidenceOutputField {
            machine: machine_id(1),
            position: 0,
        }),
    );
    let mut changed = module.clone();
    changed.evidence_contract_lanes[1].output_field = Some("value".into());
    encode_rejected(
        "a producer-side ensures output field on the reserved carrier",
        &changed,
        CodecError::InvalidModule(ModuleError::ReservedEvidenceOutputField(machine_id(1))),
    );
    let mut changed = module.clone();
    changed.evidence_contract_lanes.push(EvidenceContractLane {
        machine: machine_id(1),
        kind: EvidenceContractLaneKind::Ensures,
        position: 1,
        term: evidence_term_id(1),
        output_field: Some("verdict".into()),
    });
    encode_rejected(
        "a producer-side ensures lane duplicating an output field",
        &changed,
        CodecError::InvalidModule(ModuleError::DuplicateEvidenceOutputField(machine_id(1))),
    );
    // A second ensures lane on the same machine and term is matched by the
    // existing requires lane — producer provenance keys the pair, not the
    // lane — so the row stays representable end to end and only the honest
    // identities diverge.
    let mut changed = module.clone();
    changed.evidence_contract_lanes.push(EvidenceContractLane {
        machine: machine_id(1),
        kind: EvidenceContractLaneKind::Ensures,
        position: 1,
        term: evidence_term_id(1),
        output_field: Some("finding".into()),
    });
    let matched_bytes = encode_module(&changed).expect("matched ensures encodes");
    divergent(
        "a producer-side second ensures lane matched by the requires lane",
        &matched_bytes,
    );

    // --- module envelope boundaries ---------------------------------------

    // Truncation inside the rosters and a trailing byte reject at the
    // envelope, before semantic replay ever runs.
    for cut in [
        fact_declaration.row.end - 1,
        witness_application.row.end - 1,
        term.row.end - 1,
        ensures_lane.row.end - 1,
        encoded.len() - 1,
    ] {
        assert!(
            decode_module(&encoded[..cut]).is_err(),
            "truncation at byte {cut} must reject"
        );
    }
    let mut trailing = encoded.clone();
    trailing.push(0);
    rejected(
        "a trailing byte after the module",
        &trailing,
        CodecError::TrailingBytes(1),
    );
}
