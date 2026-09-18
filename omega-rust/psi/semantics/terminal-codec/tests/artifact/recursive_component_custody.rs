//! One-field mutation coverage for the semantic module's proof recursive
//! component roster.
//!
//! `proof_recursive_components` is the proof-only side of the canonical
//! semantic module: the verifier, not the producer, reconstructs each
//! component's identity, ranking relation, well-foundedness obligation, and
//! per-edge decrease obligations from these exact rows, and the retained
//! proof bundle joins its recursive-component certificates to that
//! reconstruction. Its wire fields — the roster count, each component's
//! ranking-relation tag, rank-type identity, the counted proof-type roster
//! (type identity plus each field's identity and target type identity), the
//! counted member roster (contract, machine identity, and rank-parameter
//! identity), and the counted edge roster (caller, callee, the exact
//! statement/expression/transition call site with its lane, and the strict
//! member path) — are each substituted independently. A substitution either
//! fails canonical decoding or encoding, or decodes to a different module
//! whose honestly recomputed semantic and artifact identities diverge and
//! whose replay against the retained manifest, sealed proof subject, and
//! terminal verifier rejects.
//!
//! The fixture declares two components so the roster itself is reorderable,
//! and the first component carries an unused `package::Leaf` proof type plus
//! all three call-site variants so every representable field has both a
//! joined-rejection leg and a divergent-replay leg where one exists.

use std::ops::Range;

use super::{canonical_artifact, kernel_bundle, proof_recursive_evidence, semantic_module};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::ContractId;
use terminal_codec::{
    ArtifactManifestError, CodecError, ProofCodecError, build_artifact_manifest,
    build_identity_optimization_execution_record, decode_module, decode_proof_section_for,
    encode_module, terminal_psi_identity, validate_artifact_manifest,
};
use terminal_psi::{
    TerminalModule, TerminalProofRankingRelation, TerminalProofRecursiveCallSite,
    TerminalProofRecursiveComponent, TerminalProofRecursiveEdge, TerminalProofRecursiveField,
    TerminalProofRecursiveMember, TerminalProofRecursiveTransitionLane, TerminalProofRecursiveType,
};
use terminal_verifier::{ModuleError, VerificationError, verify_module};

/// Byte offsets of every wire field inside the recursive-component roster.
/// Every `string` field records its u32 length prefix and its content bytes
/// separately so a substitution can lie about either side.
struct ModuleSpans {
    component_count: Range<usize>,
    components: Vec<ComponentSpan>,
}

struct ComponentSpan {
    row: Range<usize>,
    ranking: Range<usize>,
    rank_type_len: Range<usize>,
    rank_type: Range<usize>,
    type_count: Range<usize>,
    types: Vec<TypeSpan>,
    member_count: Range<usize>,
    members: Vec<MemberSpan>,
    edge_count: Range<usize>,
    edges: Vec<EdgeSpan>,
}

struct TypeSpan {
    row: Range<usize>,
    identity_len: Range<usize>,
    identity: Range<usize>,
    field_count: Range<usize>,
    fields: Vec<FieldSpan>,
}

struct FieldSpan {
    row: Range<usize>,
    identity_len: Range<usize>,
    identity: Range<usize>,
    type_len: Range<usize>,
    type_identity: Range<usize>,
}

struct MemberSpan {
    row: Range<usize>,
    contract: Range<usize>,
    machine_len: Range<usize>,
    machine: Range<usize>,
    parameter_len: Range<usize>,
    parameter: Range<usize>,
}

struct EdgeSpan {
    row: Range<usize>,
    caller: Range<usize>,
    callee: Range<usize>,
    site_tag: Range<usize>,
    state_len: Range<usize>,
    state: Range<usize>,
    statement_index: Range<usize>,
    /// `Expression` sites only.
    expression_ordinal: Option<Range<usize>>,
    /// `Transition` sites only.
    lane: Option<Range<usize>>,
    path_count: Range<usize>,
    /// `(length prefix, content)` per strict member path element.
    path: Vec<(Range<usize>, Range<usize>)>,
}

/// A cursor that walks the canonical module encoding exactly as the decoder
/// does, recording the byte span of every roster field the matrix
/// substitutes. Every section ahead of the component roster is empty in the
/// fixture, so the walk asserts each leading count is zero rather than
/// silently spanning unknown row bytes.
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
                .expect("u32 field inside the module"),
        )
    }

    fn take_count(&mut self) -> (Range<usize>, u32) {
        let span = self.take(4);
        (span.clone(), self.u32_at(span))
    }

    fn expect_empty_count(&mut self, label: &'static str) {
        let (_, count) = self.take_count();
        assert_eq!(count, 0, "the fixture leaves the {label} roster empty");
    }

    fn take_string(&mut self) -> (Range<usize>, Range<usize>) {
        let (len_span, len) = self.take_count();
        let value = self.take(usize::try_from(len).expect("string length fits usize"));
        (len_span, value)
    }

    fn walk_component(&mut self) -> ComponentSpan {
        let row_start = self.offset;
        let ranking = self.take(1);
        let (rank_type_len, rank_type) = self.take_string();
        let (type_count, type_rows) = self.take_count();
        let mut types = Vec::with_capacity(usize::try_from(type_rows).expect("types fit"));
        for _ in 0..type_rows {
            let type_start = self.offset;
            let (identity_len, identity) = self.take_string();
            let (field_count, field_rows) = self.take_count();
            let mut fields = Vec::with_capacity(usize::try_from(field_rows).expect("fields fit"));
            for _ in 0..field_rows {
                let field_start = self.offset;
                let (identity_len, identity) = self.take_string();
                let (type_len, type_identity) = self.take_string();
                fields.push(FieldSpan {
                    row: field_start..self.offset,
                    identity_len,
                    identity,
                    type_len,
                    type_identity,
                });
            }
            types.push(TypeSpan {
                row: type_start..self.offset,
                identity_len,
                identity,
                field_count,
                fields,
            });
        }
        let (member_count, member_rows) = self.take_count();
        let mut members = Vec::with_capacity(usize::try_from(member_rows).expect("members fit"));
        for _ in 0..member_rows {
            let member_start = self.offset;
            let contract = self.take(8);
            let (machine_len, machine) = self.take_string();
            let (parameter_len, parameter) = self.take_string();
            members.push(MemberSpan {
                row: member_start..self.offset,
                contract,
                machine_len,
                machine,
                parameter_len,
                parameter,
            });
        }
        let (edge_count, edge_rows) = self.take_count();
        let mut edges = Vec::with_capacity(usize::try_from(edge_rows).expect("edges fit"));
        for _ in 0..edge_rows {
            let edge_start = self.offset;
            let caller = self.take(8);
            let callee = self.take(8);
            let site_tag = self.take(1);
            let (state_len, state) = self.take_string();
            let statement_index = self.take(8);
            let (expression_ordinal, lane) = match self.bytes[site_tag.start] {
                1 => (None, None),
                2 => (Some(self.take(8)), None),
                3 => (None, Some(self.take(1))),
                tag => panic!("unexpected recursive call-site tag {tag}"),
            };
            let (path_count, path_rows) = self.take_count();
            let mut path = Vec::with_capacity(usize::try_from(path_rows).expect("path fits"));
            for _ in 0..path_rows {
                path.push(self.take_string());
            }
            edges.push(EdgeSpan {
                row: edge_start..self.offset,
                caller,
                callee,
                site_tag,
                state_len,
                state,
                statement_index,
                expression_ordinal,
                lane,
                path_count,
                path,
            });
        }
        ComponentSpan {
            row: row_start..self.offset,
            ranking,
            rank_type_len,
            rank_type,
            type_count,
            types,
            member_count,
            members,
            edge_count,
            edges,
        }
    }
}

/// Locate the recursive-component roster inside canonical module bytes by
/// mirroring `encode_raw`'s ordered section list. The fixture module keeps
/// every earlier roster empty, so each leading section is a zero count.
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
        "proposition declarations",
        "proposition applications",
        "evidence terms",
        "evidence contract lanes",
        "proof-output invocations",
    ] {
        walker.expect_empty_count(label);
    }
    let (component_count, components) = walker.take_count();
    let mut spans = Vec::with_capacity(usize::try_from(components).expect("components fit"));
    for _ in 0..components {
        spans.push(walker.walk_component());
    }
    ModuleSpans {
        component_count,
        components: spans,
    }
}

/// The first component: one unused leaf proof type beside the recursive node
/// type, two members, and edges covering all three call-site variants in both
/// directions so the component stays strongly connected when one edge moves.
fn node_component() -> TerminalProofRecursiveComponent {
    let contract = |raw| ContractId::new(raw).expect("nonzero proof contract identity");
    let field = |identity: &str| TerminalProofRecursiveField {
        identity: identity.into(),
        type_identity: "package::Node".into(),
    };
    let member = |raw, machine: &str, parameter: &str| TerminalProofRecursiveMember {
        contract: contract(raw),
        machine_identity: machine.into(),
        rank_parameter_identity: parameter.into(),
    };
    let edge = |caller, callee, site, path: &str| TerminalProofRecursiveEdge {
        caller: contract(caller),
        callee: contract(callee),
        site,
        strict_member_path: vec![path.into()],
    };
    TerminalProofRecursiveComponent {
        ranking_relation: TerminalProofRankingRelation::StructuralSubterm,
        rank_type_identity: "package::Node".into(),
        types: vec![
            TerminalProofRecursiveType {
                identity: "package::Leaf".into(),
                fields: Vec::new(),
            },
            TerminalProofRecursiveType {
                identity: "package::Node".into(),
                fields: vec![field("package::Node::left"), field("package::Node::right")],
            },
        ],
        members: vec![
            member(1001, "package::left", "package::left::node"),
            member(1002, "package::right", "package::right::node"),
        ],
        edges: vec![
            edge(
                1001,
                1002,
                TerminalProofRecursiveCallSite::Statement {
                    state_identity: "package::left::step".into(),
                    statement_index: 0,
                },
                "package::Node::left",
            ),
            edge(
                1001,
                1002,
                TerminalProofRecursiveCallSite::Statement {
                    state_identity: "package::left::step".into(),
                    statement_index: 1,
                },
                "package::Node::right",
            ),
            edge(
                1001,
                1002,
                TerminalProofRecursiveCallSite::Expression {
                    state_identity: "package::left::step".into(),
                    statement_index: 2,
                    expression_ordinal: 0,
                },
                "package::Node::left",
            ),
            edge(
                1002,
                1001,
                TerminalProofRecursiveCallSite::Statement {
                    state_identity: "package::right::step".into(),
                    statement_index: 0,
                },
                "package::Node::left",
            ),
            edge(
                1002,
                1001,
                TerminalProofRecursiveCallSite::Statement {
                    state_identity: "package::right::step".into(),
                    statement_index: 1,
                },
                "package::Node::right",
            ),
            edge(
                1002,
                1001,
                TerminalProofRecursiveCallSite::Transition {
                    state_identity: "package::right::step".into(),
                    statement_index: 2,
                    lane: TerminalProofRecursiveTransitionLane::Continuation,
                },
                "package::Node::right",
            ),
        ],
    }
}

/// The second component: an isomorphic statement-only roster on a disjoint
/// proof type so the module-level roster has an ordering to violate.
fn tree_component() -> TerminalProofRecursiveComponent {
    let contract = |raw| ContractId::new(raw).expect("nonzero proof contract identity");
    let field = |identity: &str| TerminalProofRecursiveField {
        identity: identity.into(),
        type_identity: "package::Tree".into(),
    };
    let member = |raw, machine: &str, parameter: &str| TerminalProofRecursiveMember {
        contract: contract(raw),
        machine_identity: machine.into(),
        rank_parameter_identity: parameter.into(),
    };
    let edge = |caller, callee, state: &str, index, path: &str| TerminalProofRecursiveEdge {
        caller: contract(caller),
        callee: contract(callee),
        site: TerminalProofRecursiveCallSite::Statement {
            state_identity: state.into(),
            statement_index: index,
        },
        strict_member_path: vec![path.into()],
    };
    TerminalProofRecursiveComponent {
        ranking_relation: TerminalProofRankingRelation::StructuralSubterm,
        rank_type_identity: "package::Tree".into(),
        types: vec![TerminalProofRecursiveType {
            identity: "package::Tree".into(),
            fields: vec![field("package::Tree::left"), field("package::Tree::right")],
        }],
        members: vec![
            member(2001, "package::tree_left", "package::tree_left::node"),
            member(2002, "package::tree_right", "package::tree_right::node"),
        ],
        edges: vec![
            edge(
                2001,
                2002,
                "package::tree_left::step",
                0,
                "package::Tree::left",
            ),
            edge(
                2001,
                2002,
                "package::tree_left::step",
                1,
                "package::Tree::right",
            ),
            edge(
                2002,
                2001,
                "package::tree_right::step",
                0,
                "package::Tree::left",
            ),
            edge(
                2002,
                2001,
                "package::tree_right::step",
                1,
                "package::Tree::right",
            ),
        ],
    }
}

fn recursive_module() -> TerminalModule {
    let mut module = semantic_module();
    module.proof_recursive_components = vec![node_component(), tree_component()];
    module
}

#[test]
fn terminal_proof_recursive_components_reject_every_one_field_substitution() {
    let module = recursive_module();
    let mut bundle = kernel_bundle();
    bundle.recursive_components = proof_recursive_evidence(&module);
    // The bundle roster is ordered by the reconstructed component identity,
    // not by the module's canonical component order.
    bundle
        .recursive_components
        .sort_by_key(|entry| entry.component);
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
        spans.components.len(),
        2,
        "fixture roster: a node component and a tree component"
    );

    // The retained artifact binds the semantic identity into its manifest and
    // the sealed proof section to this exact module; replaying either against
    // a substituted module is the independent-replay leg for every
    // representable field.
    let artifact = canonical_artifact(&module, &bundle, None);
    let retained = artifact.manifest();
    let semantic_identity = terminal_psi_identity(&module).expect("semantic identity");
    assert_eq!(retained.semantic(), semantic_identity);

    // A substitution that still forms a canonical module honestly recomputes
    // a divergent semantic and artifact identity, and the retained custody
    // replays reject it: the manifest join, the verifier's reconstructed
    // recursive-component evidence join, and the sealed proof subject join.
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
        match verify_module(&substituted, &bundle, &AdmissionProfile::default()) {
            Err(
                VerificationError::MissingRecursiveComponentEvidence(_)
                | VerificationError::UnknownRecursiveComponentEvidence(_),
            ) => {}
            other => panic!("{name} must reject at the recursive-evidence replay: {other:?}"),
        }
        assert!(
            matches!(
                decode_proof_section_for(&substituted, artifact.proof_bytes()),
                Err(ProofCodecError::ProofSubjectMismatch { .. })
            ),
            "{name} must reject at the sealed proof subject join"
        );
        substituted
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
    let encode_rejected = |name: &'static str, changed: &TerminalModule, expected: CodecError| {
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
    let restring = |len_span: Range<usize>, value_span: Range<usize>, value: &str| -> Vec<u8> {
        let mut mutated = encoded[..len_span.start].to_vec();
        mutated.extend_from_slice(
            &u32::try_from(value.len())
                .expect("string length fits u32")
                .to_le_bytes(),
        );
        mutated.extend_from_slice(value.as_bytes());
        mutated.extend_from_slice(&encoded[value_span.end..]);
        mutated
    };
    // Remove one roster row and decrement its roster count honestly.
    let drop_row = |count_span: Range<usize>, row: Range<usize>| -> Vec<u8> {
        let remaining = u32::from_le_bytes(
            encoded[count_span.clone()]
                .try_into()
                .expect("roster count"),
        ) - 1;
        let mut mutated = encoded[..count_span.start].to_vec();
        mutated.extend_from_slice(&remaining.to_le_bytes());
        mutated.extend_from_slice(&encoded[count_span.end..row.start]);
        mutated.extend_from_slice(&encoded[row.end..]);
        mutated
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

    let node = &spans.components[0];
    let tree = &spans.components[1];
    // The `package::Node` proof type is index 1 in component zero's type
    // roster (the unused `package::Leaf` sorts first).
    let node_type = &node.types[1];

    // --- roster axes -----------------------------------------------------

    // A roster count lying about its rows starves the next section or
    // overruns the component tag.
    rejected(
        "a component roster count one over",
        &put_u32(spans.component_count.clone(), 3),
        CodecError::InvalidTag("TerminalProofRankingRelation", 0),
    );
    rejected(
        "a maximal component roster count",
        &put_u32(spans.component_count.clone(), u32::MAX),
        CodecError::UnexpectedEnd,
    );
    // Clearing the roster or dropping either component is representable: the
    // module stays canonical, the recomputed identity diverges, and the
    // retained bundle's surplus component evidence rejects on replay.
    let mut cleared = encoded[..spans.component_count.start].to_vec();
    cleared.extend_from_slice(&0_u32.to_le_bytes());
    cleared.extend_from_slice(&encoded[tree.row.end..]);
    divergent("a cleared component roster", &cleared);
    divergent(
        "a dropped node component",
        &drop_row(spans.component_count.clone(), node.row.clone()),
    );
    divergent(
        "a dropped tree component",
        &drop_row(spans.component_count.clone(), tree.row.clone()),
    );
    // Duplicated and reordered components stay well-formed rows but violate
    // the strict canonical ordering.
    rejected(
        "a duplicated component",
        &duplicate_row(spans.component_count.clone(), node.row.clone()),
        CodecError::InvalidModule(ModuleError::NonCanonicalProofRecursiveComponents),
    );
    let mut swapped = encoded[..node.row.start].to_vec();
    swapped.extend_from_slice(&encoded[tree.row.clone()]);
    swapped.extend_from_slice(&encoded[node.row.clone()]);
    swapped.extend_from_slice(&encoded[tree.row.end..]);
    rejected(
        "a reordered component roster",
        &swapped,
        CodecError::InvalidModule(ModuleError::NonCanonicalProofRecursiveComponents),
    );

    // --- component axes --------------------------------------------------

    // The ranking relation has one representable tag; every other byte is
    // unknown.
    for tag in [0, 2, u8::MAX] {
        rejected(
            "an unknown ranking-relation tag",
            &put_u8(node.ranking.clone(), tag),
            CodecError::InvalidTag("TerminalProofRankingRelation", tag),
        );
    }

    // The rank-type identity must name a type inside this component, and the
    // strict member paths must still resolve from it. A present but unusable
    // type moves the rejection to the edge join.
    rejected(
        "an empty rank-type identity",
        &restring(node.rank_type_len.clone(), node.rank_type.clone(), ""),
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveComponent),
    );
    rejected(
        "a rank type the component does not declare",
        &restring(
            node.rank_type_len.clone(),
            node.rank_type.clone(),
            "package::Tree",
        ),
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveComponent),
    );
    rejected(
        "a rank type without the path's fields",
        &restring(
            node.rank_type_len.clone(),
            node.rank_type.clone(),
            "package::Leaf",
        ),
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveEdge {
            caller: ContractId::new(1001).expect("caller"),
            callee: ContractId::new(1002).expect("callee"),
        }),
    );
    rejected(
        "a rank-type identity length lie",
        &put_u32(node.rank_type_len.clone(), 14),
        CodecError::UnexpectedEnd,
    );

    // --- proof-type roster -----------------------------------------------

    // The unused leaf type can be dropped or renamed without disturbing the
    // rank join; the recomputed identity still diverges.
    divergent(
        "a dropped unused proof type",
        &drop_row(node.type_count.clone(), node.types[0].row.clone()),
    );
    divergent(
        "a renamed unused proof type",
        &restring(
            node.types[0].identity_len.clone(),
            node.types[0].identity.clone(),
            "package::Leaa",
        ),
    );
    // Dropping the rank type, clearing the roster, duplicating a type, or
    // reordering the roster each break the type join.
    rejected(
        "a dropped rank proof type",
        &drop_row(node.type_count.clone(), node_type.row.clone()),
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveComponent),
    );
    let mut cleared_types = encoded[..node.type_count.start].to_vec();
    cleared_types.extend_from_slice(&0_u32.to_le_bytes());
    cleared_types.extend_from_slice(&encoded[node_type.row.end..]);
    rejected(
        "a cleared proof-type roster",
        &cleared_types,
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveComponent),
    );
    rejected(
        "a duplicated proof type",
        &duplicate_row(node.type_count.clone(), node_type.row.clone()),
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveComponent),
    );
    let mut reordered = encoded[..node.types[0].row.start].to_vec();
    reordered.extend_from_slice(&encoded[node_type.row.clone()]);
    reordered.extend_from_slice(&encoded[node.types[0].row.clone()]);
    reordered.extend_from_slice(&encoded[node_type.row.end..]);
    rejected(
        "a reordered proof-type roster",
        &reordered,
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveComponent),
    );
    // A third "type" decodes the member count as a two-byte identity whose
    // first byte is a non-UTF-8 contract byte.
    rejected(
        "an over-counted proof-type roster",
        &put_u32(node.type_count.clone(), 3),
        CodecError::InvalidUtf8("proof recursive type identity"),
    );

    // A type identity carries the whole component's path join: renaming the
    // rank type unbinds every strict member path.
    rejected(
        "an emptied type identity",
        &restring(
            node_type.identity_len.clone(),
            node_type.identity.clone(),
            "",
        ),
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveComponent),
    );
    rejected(
        "a renamed rank type",
        &restring(
            node_type.identity_len.clone(),
            node_type.identity.clone(),
            "package::None",
        ),
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveComponent),
    );

    // --- proof-field roster ----------------------------------------------

    // Every retained strict path resolves field-by-field; losing either
    // declared field strands the edges that traverse it.
    for (name, field) in [
        ("a dropped left proof field", &node_type.fields[0]),
        ("a dropped right proof field", &node_type.fields[1]),
    ] {
        rejected(
            name,
            &drop_row(node_type.field_count.clone(), field.row.clone()),
            CodecError::InvalidModule(ModuleError::InvalidProofRecursiveEdge {
                caller: ContractId::new(1001).expect("caller"),
                callee: ContractId::new(1002).expect("callee"),
            }),
        );
    }
    rejected(
        "a duplicated proof field",
        &duplicate_row(
            node_type.field_count.clone(),
            node_type.fields[1].row.clone(),
        ),
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveComponent),
    );
    let mut reordered = encoded[..node_type.fields[0].row.start].to_vec();
    reordered.extend_from_slice(&encoded[node_type.fields[1].row.clone()]);
    reordered.extend_from_slice(&encoded[node_type.fields[0].row.clone()]);
    reordered.extend_from_slice(&encoded[node_type.fields[1].row.end..]);
    rejected(
        "a reordered proof-field roster",
        &reordered,
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveComponent),
    );

    // A field identity or target-type identity is joined by every strict
    // path that traverses it; any substitution strands the path.
    rejected(
        "an emptied field identity",
        &restring(
            node_type.fields[0].identity_len.clone(),
            node_type.fields[0].identity.clone(),
            "",
        ),
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveComponent),
    );
    rejected(
        "a renamed field identity",
        &restring(
            node_type.fields[0].identity_len.clone(),
            node_type.fields[0].identity.clone(),
            "package::Node::lefp",
        ),
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveEdge {
            caller: ContractId::new(1001).expect("caller"),
            callee: ContractId::new(1002).expect("callee"),
        }),
    );
    rejected(
        "a field targeting a leaf type",
        &restring(
            node_type.fields[0].type_len.clone(),
            node_type.fields[0].type_identity.clone(),
            "package::Leaf",
        ),
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveEdge {
            caller: ContractId::new(1001).expect("caller"),
            callee: ContractId::new(1002).expect("callee"),
        }),
    );
    rejected(
        "a field targeting an undeclared type",
        &restring(
            node_type.fields[1].type_len.clone(),
            node_type.fields[1].type_identity.clone(),
            "package::Missing",
        ),
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveEdge {
            caller: ContractId::new(1001).expect("caller"),
            callee: ContractId::new(1002).expect("callee"),
        }),
    );

    // --- member roster ----------------------------------------------------

    // Members are ordered by contract and joined by every edge endpoint; a
    // contract substitution renames the member out from under its edges.
    for (name, index) in [
        ("a dropped first member", 0_usize),
        ("a dropped second member", 1_usize),
    ] {
        rejected(
            name,
            &drop_row(node.member_count.clone(), node.members[index].row.clone()),
            CodecError::InvalidModule(ModuleError::InvalidProofRecursiveEdge {
                caller: ContractId::new(1001).expect("caller"),
                callee: ContractId::new(1002).expect("callee"),
            }),
        );
    }
    rejected(
        "a duplicated member",
        &duplicate_row(node.member_count.clone(), node.members[1].row.clone()),
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveComponent),
    );
    let mut reordered = encoded[..node.members[0].row.start].to_vec();
    reordered.extend_from_slice(&encoded[node.members[1].row.clone()]);
    reordered.extend_from_slice(&encoded[node.members[0].row.clone()]);
    reordered.extend_from_slice(&encoded[node.members[1].row.end..]);
    rejected(
        "a reordered member roster",
        &reordered,
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveComponent),
    );

    rejected(
        "a zero member contract",
        &put_u64(node.members[0].contract.clone(), 0),
        CodecError::ZeroIdentity("ContractId"),
    );
    rejected(
        "a member contract its edges do not know",
        &put_u64(node.members[0].contract.clone(), 999),
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveEdge {
            caller: ContractId::new(1001).expect("caller"),
            callee: ContractId::new(1002).expect("callee"),
        }),
    );
    rejected(
        "a colliding member contract",
        &put_u64(node.members[0].contract.clone(), 1002),
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveComponent),
    );

    // Machine and rank-parameter identities are source-free custody strings:
    // nonempty substitutions stay representable but reidentify the member.
    divergent(
        "a renamed member machine identity",
        &restring(
            node.members[0].machine_len.clone(),
            node.members[0].machine.clone(),
            "package::meft",
        ),
    );
    divergent(
        "a renamed member rank parameter",
        &restring(
            node.members[0].parameter_len.clone(),
            node.members[0].parameter.clone(),
            "package::left::done",
        ),
    );
    divergent(
        "a renamed tree member machine identity",
        &restring(
            tree.members[0].machine_len.clone(),
            tree.members[0].machine.clone(),
            "package::tree_laft",
        ),
    );
    rejected(
        "an emptied member machine identity",
        &restring(
            node.members[0].machine_len.clone(),
            node.members[0].machine.clone(),
            "",
        ),
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveMember(
            ContractId::new(1001).expect("member"),
        )),
    );
    rejected(
        "an emptied member rank parameter",
        &restring(
            node.members[1].parameter_len.clone(),
            node.members[1].parameter.clone(),
            "",
        ),
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveMember(
            ContractId::new(1002).expect("member"),
        )),
    );

    // --- edge roster ------------------------------------------------------

    // The component stays strongly connected while one edge leaves: the
    // remaining statement and transition routes still join every member.
    divergent(
        "a dropped transition edge",
        &drop_row(node.edge_count.clone(), node.edges[5].row.clone()),
    );
    divergent(
        "a dropped expression edge",
        &drop_row(node.edge_count.clone(), node.edges[2].row.clone()),
    );
    // Dropping one direction's whole edge family breaks reachability.
    let mut disconnected = encoded[..node.edge_count.start].to_vec();
    disconnected.extend_from_slice(&3_u32.to_le_bytes());
    disconnected.extend_from_slice(&encoded[node.edge_count.end..node.edges[3].row.start]);
    disconnected.extend_from_slice(&encoded[node.edges[5].row.end..]);
    rejected(
        "a disconnected member",
        &disconnected,
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveComponent),
    );
    rejected(
        "a duplicated edge",
        &duplicate_row(node.edge_count.clone(), node.edges[0].row.clone()),
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveComponent),
    );
    let mut reordered = encoded[..node.edges[0].row.start].to_vec();
    reordered.extend_from_slice(&encoded[node.edges[1].row.clone()]);
    reordered.extend_from_slice(&encoded[node.edges[0].row.clone()]);
    reordered.extend_from_slice(&encoded[node.edges[1].row.end..]);
    rejected(
        "a reordered edge roster",
        &reordered,
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveComponent),
    );

    // Edge endpoints must name members. A self-loop keeps the caller/callee
    // join representable while diverging the committed obligation.
    rejected(
        "a zero edge caller",
        &put_u64(node.edges[0].caller.clone(), 0),
        CodecError::ZeroIdentity("ContractId"),
    );
    rejected(
        "an edge caller outside the component",
        &put_u64(node.edges[0].caller.clone(), 5),
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveEdge {
            caller: ContractId::new(5).expect("caller"),
            callee: ContractId::new(1002).expect("callee"),
        }),
    );
    rejected(
        "an edge caller that overtakes the roster",
        &put_u64(node.edges[0].caller.clone(), 1002),
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveComponent),
    );
    rejected(
        "a zero edge callee",
        &put_u64(node.edges[0].callee.clone(), 0),
        CodecError::ZeroIdentity("ContractId"),
    );
    rejected(
        "an edge callee outside the component",
        &put_u64(node.edges[0].callee.clone(), 5),
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveEdge {
            caller: ContractId::new(1001).expect("caller"),
            callee: ContractId::new(5).expect("callee"),
        }),
    );
    divergent(
        "a self-looping edge callee",
        &put_u64(node.edges[0].callee.clone(), 1001),
    );

    // --- call-site variants ----------------------------------------------

    // Unknown call-site and transition-lane tags are not representable.
    for tag in [0, 4, u8::MAX] {
        rejected(
            "an unknown call-site tag",
            &put_u8(node.edges[0].site_tag.clone(), tag),
            CodecError::InvalidTag("TerminalProofRecursiveCallSite", tag),
        );
    }
    // Retagging a Statement site as an Expression site consumes the strict
    // path count and length as the expression ordinal, then reads the path
    // element's first bytes as an over-long string.
    rejected(
        "a call-site tag that misaligns the site",
        &put_u8(node.edges[0].site_tag.clone(), 2),
        CodecError::StringTooLong("proof recursive strict member path"),
    );
    // A Statement site extended into an Expression site stays ordered before
    // the existing expression edge and remains a unique (caller, site) pair.
    let mut expression_site = encoded[node.edges[1].site_tag.start..node.edges[1].row.end].to_vec();
    expression_site[0] = 2;
    expression_site.splice(
        node.edges[1].statement_index.end - node.edges[1].site_tag.start
            ..node.edges[1].statement_index.end - node.edges[1].site_tag.start,
        9_u64.to_le_bytes(),
    );
    let mut mutated = encoded[..node.edges[1].site_tag.start].to_vec();
    mutated.extend_from_slice(&expression_site);
    mutated.extend_from_slice(&encoded[node.edges[1].row.end..]);
    divergent("a statement site promoted to an expression", &mutated);
    // The transition lane's second representable value still orders after the
    // statement sites.
    divergent(
        "a transition lane flipped to the target",
        &put_u8(node.edges[5].lane.clone().expect("transition lane"), 1),
    );
    rejected(
        "an unknown transition lane",
        &put_u8(node.edges[5].lane.clone().expect("transition lane"), 3),
        CodecError::InvalidTag("TerminalProofRecursiveTransitionLane", 3),
    );

    // Site payloads: the state identity only needs to stay nonempty and keep
    // the edge's canonical position; the statement index and expression
    // ordinal are free while the ordering holds.
    divergent(
        "a renamed statement state identity",
        &restring(
            node.edges[0].state_len.clone(),
            node.edges[0].state.clone(),
            "package::left::stel",
        ),
    );
    rejected(
        "an emptied statement state identity",
        &restring(
            node.edges[0].state_len.clone(),
            node.edges[0].state.clone(),
            "",
        ),
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveEdge {
            caller: ContractId::new(1001).expect("caller"),
            callee: ContractId::new(1002).expect("callee"),
        }),
    );
    divergent(
        "a moved statement index",
        &put_u64(node.edges[1].statement_index.clone(), 7),
    );
    rejected(
        "a statement index that overtakes the next site",
        &put_u64(node.edges[0].statement_index.clone(), 5),
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveComponent),
    );
    divergent(
        "a moved expression ordinal",
        &put_u64(
            node.edges[2]
                .expression_ordinal
                .clone()
                .expect("expression ordinal"),
            9,
        ),
    );

    // --- strict member paths ----------------------------------------------

    // The strict member path must resolve field-by-field back to the rank
    // type. Substituting another declared field keeps the path valid; a
    // missing or emptied element strands it, and a lengthened path that
    // still resolves stays representable.
    divergent(
        "a strict path through the sibling field",
        &restring(
            node.edges[0].path[0].0.clone(),
            node.edges[0].path[0].1.clone(),
            "package::Node::right",
        ),
    );
    divergent(
        "a strict path through the tree sibling field",
        &restring(
            tree.edges[0].path[0].0.clone(),
            tree.edges[0].path[0].1.clone(),
            "package::Tree::right",
        ),
    );
    // A lengthened path: prepend a `package::Node::left` element so the path
    // traverses left twice and still ends at the rank type.
    let element_len = 19_u32.to_le_bytes();
    let mut lengthened = encoded[..node.edges[0].path_count.start].to_vec();
    lengthened.extend_from_slice(&2_u32.to_le_bytes());
    lengthened.extend_from_slice(&element_len);
    lengthened.extend_from_slice(b"package::Node::left");
    lengthened.extend_from_slice(&encoded[node.edges[0].path_count.end..]);
    divergent("a lengthened strict member path", &lengthened);
    rejected(
        "a cleared strict member path",
        &drop_row(
            node.edges[0].path_count.clone(),
            node.edges[0].path[0].0.start..node.edges[0].path[0].1.end,
        ),
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveEdge {
            caller: ContractId::new(1001).expect("caller"),
            callee: ContractId::new(1002).expect("callee"),
        }),
    );
    rejected(
        "a strict path through an undeclared field",
        &restring(
            node.edges[0].path[0].0.clone(),
            node.edges[0].path[0].1.clone(),
            "package::Node::lefp",
        ),
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveEdge {
            caller: ContractId::new(1001).expect("caller"),
            callee: ContractId::new(1002).expect("callee"),
        }),
    );
    rejected(
        "an emptied strict path element",
        &restring(
            node.edges[0].path[0].0.clone(),
            node.edges[0].path[0].1.clone(),
            "",
        ),
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveEdge {
            caller: ContractId::new(1001).expect("caller"),
            callee: ContractId::new(1002).expect("callee"),
        }),
    );
    // A path element length lie consumes the next edge's non-UTF-8 contract
    // byte.
    rejected(
        "a strict path element length lie",
        &put_u32(node.edges[0].path[0].0.clone(), 20),
        CodecError::InvalidUtf8("proof recursive strict member path"),
    );

    // --- producer-side rejections ------------------------------------------

    // Every ordering violation above also fails closed on the way out: the
    // canonical encoder runs the same semantic validation before emitting
    // bytes.
    let mut changed = module.clone();
    changed.proof_recursive_components.swap(0, 1);
    encode_rejected(
        "a producer-side reordered component roster",
        &changed,
        CodecError::InvalidModule(ModuleError::NonCanonicalProofRecursiveComponents),
    );
    let mut changed = module.clone();
    changed.proof_recursive_components[0].edges.swap(0, 1);
    encode_rejected(
        "a producer-side reordered edge roster",
        &changed,
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveComponent),
    );
    let mut changed = module.clone();
    changed.proof_recursive_components[0].members.swap(0, 1);
    encode_rejected(
        "a producer-side reordered member roster",
        &changed,
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveComponent),
    );
    let mut changed = module.clone();
    changed.proof_recursive_components[0].types.swap(0, 1);
    encode_rejected(
        "a producer-side reordered proof-type roster",
        &changed,
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveComponent),
    );
    let mut changed = module.clone();
    changed.proof_recursive_components[0].types[1]
        .fields
        .swap(0, 1);
    encode_rejected(
        "a producer-side reordered proof-field roster",
        &changed,
        CodecError::InvalidModule(ModuleError::InvalidProofRecursiveComponent),
    );

    // --- module envelope boundaries ----------------------------------------

    // Truncation inside the roster and a trailing byte reject at the
    // envelope, before semantic replay ever runs.
    for cut in [node.row.end - 1, tree.row.end - 1, encoded.len() - 1] {
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
