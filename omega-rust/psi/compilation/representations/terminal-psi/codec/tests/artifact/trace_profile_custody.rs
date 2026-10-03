//! One-field mutation coverage for the standalone TerminalTraceV1 observation
//! profile.
//!
//! The profile binds the exact module commitment to the verifier-reconstructed
//! observation roster: the domain string, schema version, vocabulary marker
//! and module commitment; the mandatory root row (entry machine, scalar input
//! schemas, structural input schemas and the result schema); the sorted
//! terminator crash-site rows; the sorted boundary-call crash-route rows
//! (call-site coordinates, boundary identity, and the declared route bucket);
//! the sorted ordinary-event rows (call-site coordinates, event kind and its
//! bound identity, argument schemas and result schema); and the explicitly
//! empty external-termination roster. Each wire field is substituted
//! independently: a substitution either fails canonical encoding or decoding,
//! or decodes to a differently identified profile whose module-bound replay
//! against the same canonical module rejects.
//!
//! The fixture module carries three machines so every row group has enough
//! rows for ordering legs: the entry machine owns scalar and structural
//! parameters (including a projected qualification path), a port write, a
//! boundary call returning a structural result, and a scalar return, while
//! two further machines contribute terminator crash sites.
//!
//! The representable legs run through the shared
//! `run_one_field_substitution_matrix` driver over the declared
//! `TraceProfileCustodyFieldForTest` inventory in
//! `trace_profile_custody_fields.rs`: each leg mutates one
//! `TerminalTraceV1Profile` field, must still encode and round-trip
//! canonically, and is rejected by the module-bound acceptance replay — the
//! verifier's own reconstruction of the same canonical module — asserted
//! inside the matrix's checker. The raw wire axes (domain, version and
//! marker lies, count lies, closed tags, zero and order-breaking
//! identities, empty route buckets, roster-order violations, truncation)
//! and the producer-side encode rejections stay authored below.

use std::ops::Range;

use super::{
    block_id, contract_id, edge_id, machine_id, operation_id, place_id, structural_field_id,
    value_id,
};
use semantic_vocabulary::{
    BoundaryMachineId, DomainSemanticId, IntegerSign, IntegerType, IntegerValue, ScalarType,
    ServiceId, StructuralDomainId, StructuralPlaceKind, StructuralTypeId,
};
use terminal_codec::{
    CodecError, TerminalTraceV1ProfileAcceptanceError, TerminalTraceV1ProfileCodecError,
    accept_terminal_trace_v1_profile, decode_terminal_trace_v1_profile,
    encode_terminal_trace_v1_profile, reconstruct_canonical_terminal_trace_v1_profile,
    terminal_psi_identity,
};
use terminal_psi::{
    BindingRelevance, Block, BoundaryMachineDeclaration, BoundaryMachineResult,
    BoundaryStructuralResultDeclaration, CrashCause, CrashRouteBucket, CrashRouteGuard,
    MachineContract, Operation, OperationKind, OperationResult, ServiceDeclaration,
    StructuralAccess, StructuralArgument, StructuralDomainDeclaration, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPathQualification, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
    TerminalMachineResult, TerminalModule, TerminalTraceV1Profile, Terminator, ValueDeclaration,
    VocabularyMarker,
};

use mutation_matrix::{OneFieldSubstitutionMatrix, run_one_field_substitution_matrix};

#[path = "trace_profile_custody_fields.rs"]
mod trace_profile_custody_fields;
use trace_profile_custody_fields::{
    TraceProfileCustodyFieldForTest, substitute_trace_profile_for_test,
    trace_profile_custody_outcome,
};

const DOMAIN: &[u8] = b"omega.terminal.observation-profile.v2";
const ROOT_ROW_TAG: u8 = 1;
const CRASH_ROW_TAG: u8 = 2;
const ORDINARY_EVENT_ROW_TAG: u8 = 3;
const BOUNDARY_CRASH_ROW_TAG: u8 = 4;
const BOUNDARY_CALL_EVENT_TAG: u8 = 1;
const PORT_WRITE_EVENT_TAG: u8 = 2;

fn u8_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 test type"))
}

fn boundary_machine_id(raw: u64) -> BoundaryMachineId {
    BoundaryMachineId::new(raw).expect("nonzero boundary identity")
}

fn service_id(raw: u64) -> ServiceId {
    ServiceId::new(raw).expect("nonzero service identity")
}

fn structural_type_id(raw: u64) -> StructuralTypeId {
    StructuralTypeId::new(raw).expect("nonzero structural type identity")
}

fn structural_domain_id(raw: u64) -> StructuralDomainId {
    StructuralDomainId::new(raw).expect("nonzero domain identity")
}

/// Byte spans of every wire field inside the canonical profile bytes.
struct ProfileSpans {
    domain: Range<usize>,
    version: Range<usize>,
    vocabulary: Range<usize>,
    commitment: Range<usize>,
    root_count: Range<usize>,
    root: RootSpan,
    crash_count: Range<usize>,
    crashes: Vec<CrashSpan>,
    boundary_crash_count: Range<usize>,
    boundary_crashes: Vec<BoundaryCrashSpan>,
    operation_crash_count: Range<usize>,
    event_count: Range<usize>,
    events: Vec<EventSpan>,
    external_count: Range<usize>,
    end: usize,
}

struct ScalarSchemaSpan {
    comparison: Range<usize>,
    type_tag: Range<usize>,
    /// Integer carrier-sign byte and bit width, present only for `Integer`.
    integer: Option<(Range<usize>, Range<usize>)>,
}

struct PathSegmentSpan {
    tag: Range<usize>,
    /// Field-name length, present only for `Field` segments.
    field_len: Option<Range<usize>>,
    /// Field-name bytes or the fixed index payload.
    value: Option<Range<usize>>,
}

struct ProjectionSpan {
    path_count: Range<usize>,
    segments: Vec<PathSegmentSpan>,
    domain: Range<usize>,
}

struct StructuralSchemaSpan {
    comparison: Range<usize>,
    structural_type: Range<usize>,
    multiplicity: Range<usize>,
    access: Range<usize>,
    qualification_count: Range<usize>,
    qualifications: Vec<Range<usize>>,
    projected_count: Range<usize>,
    projected: Vec<ProjectionSpan>,
}

struct ResultSchemaSpan {
    tag: Range<usize>,
    scalar: Option<ScalarSchemaSpan>,
}

struct RootSpan {
    row_tag: Range<usize>,
    entry: Range<usize>,
    scalar_count: Range<usize>,
    scalars: Vec<ScalarSchemaSpan>,
    structural_count: Range<usize>,
    structurals: Vec<StructuralSchemaSpan>,
    result: ResultSchemaSpan,
}

struct CrashSpan {
    row: Range<usize>,
    row_tag: Range<usize>,
    machine: Range<usize>,
    block: Range<usize>,
    edge: Range<usize>,
    cause: Range<usize>,
}

struct BoundaryCrashSpan {
    row: Range<usize>,
    row_tag: Range<usize>,
    machine: Range<usize>,
    block: Range<usize>,
    operation: Range<usize>,
    boundary: Range<usize>,
    identity_len: Range<usize>,
    identity: Range<usize>,
    route_cause: Range<usize>,
    alternative_count: Range<usize>,
    guards: Vec<Range<usize>>,
}

struct EventSpan {
    row: Range<usize>,
    row_tag: Range<usize>,
    machine: Range<usize>,
    block: Range<usize>,
    operation: Range<usize>,
    kind_tag: Range<usize>,
    kind_target: Range<usize>,
    identity_len: Range<usize>,
    identity: Range<usize>,
    scalar_count: Range<usize>,
    scalars: Vec<ScalarSchemaSpan>,
    structural_count: Range<usize>,
    structurals: Vec<StructuralSchemaSpan>,
    result: ResultSchemaSpan,
}

/// A cursor that walks the profile exactly as the canonical decoder does,
/// recording the byte span of every field the matrix substitutes. The fixture
/// only exercises the field shapes handled below; an unexpected tag fails the
/// walk rather than silently spanning unknown bytes.
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

    fn u32_at(&self, span: &Range<usize>) -> u32 {
        u32::from_le_bytes(
            self.bytes[span.clone()]
                .try_into()
                .expect("u32 field inside the profile"),
        )
    }

    fn take_count(&mut self) -> (Range<usize>, u32) {
        let span = self.take(4);
        let count = self.u32_at(&span);
        (span, count)
    }

    fn take_id(&mut self) -> Range<usize> {
        self.take(8)
    }

    fn take_string(&mut self) -> (Range<usize>, Range<usize>) {
        let (len_span, len) = self.take_count();
        let value = self.take(usize::try_from(len).expect("string length fits usize"));
        (len_span, value)
    }

    fn walk_scalar_schema(&mut self) -> ScalarSchemaSpan {
        let comparison = self.take(1);
        let type_tag = self.take(1);
        let integer = match self.bytes[type_tag.start] {
            1 => None,
            2 => Some((self.take(1), self.take(2))),
            3 => {
                self.take(1);
                None
            }
            tag => panic!("unexpected scalar type tag {tag}"),
        };
        ScalarSchemaSpan {
            comparison,
            type_tag,
            integer,
        }
    }

    fn walk_path(&mut self) -> Vec<PathSegmentSpan> {
        let (_, count) = self.take_count();
        let mut segments = Vec::with_capacity(usize::try_from(count).expect("path fits"));
        for _ in 0..count {
            let tag = self.take(1);
            let (field_len, value) = match self.bytes[tag.start] {
                1 => {
                    let (len, value) = self.take_string();
                    (Some(len), Some(value))
                }
                2 => (None, Some(self.take(8))),
                3 => (None, None),
                tag => panic!("unexpected structural path segment tag {tag}"),
            };
            segments.push(PathSegmentSpan {
                tag,
                field_len,
                value,
            });
        }
        segments
    }

    fn walk_structural_schema(&mut self) -> StructuralSchemaSpan {
        let comparison = self.take(1);
        let structural_type = self.take_id();
        let multiplicity = self.take(1);
        let access = self.take(1);
        let (qualification_count, qualifications) = self.take_count();
        let qualifications = (0..qualifications)
            .map(|_| self.take_id())
            .collect::<Vec<_>>();
        let (projected_count, projected) = self.take_count();
        let projected = (0..projected)
            .map(|_| {
                let path_count_start = self.offset;
                let segments = self.walk_path();
                let path_count = path_count_start..path_count_start + 4;
                let domain = self.take_id();
                ProjectionSpan {
                    path_count,
                    segments,
                    domain,
                }
            })
            .collect();
        StructuralSchemaSpan {
            comparison,
            structural_type,
            multiplicity,
            access,
            qualification_count,
            qualifications,
            projected_count,
            projected,
        }
    }

    fn walk_result_schema(&mut self) -> ResultSchemaSpan {
        let tag = self.take(1);
        let scalar = match self.bytes[tag.start] {
            1 => None,
            2 => Some(self.walk_scalar_schema()),
            3 => {
                self.walk_structural_schema();
                None
            }
            tag => panic!("unexpected result schema tag {tag}"),
        };
        ResultSchemaSpan { tag, scalar }
    }

    fn walk_route_bucket(&mut self) -> (Range<usize>, Range<usize>, Vec<Range<usize>>) {
        let route_cause = self.take(1);
        let (alternative_count, alternatives) = self.take_count();
        let mut guards = Vec::with_capacity(usize::try_from(alternatives).expect("guards fit"));
        for _ in 0..alternatives {
            let guard = self.take(1);
            match self.bytes[guard.start] {
                0 => {}
                tag => panic!("unexpected crash route guard tag {tag}"),
            }
            guards.push(guard);
        }
        (route_cause, alternative_count, guards)
    }
}

fn profile_spans(encoded: &[u8]) -> ProfileSpans {
    let mut walker = SpanWalker::new(encoded);
    let domain = walker.take(DOMAIN.len());
    let version = walker.take(2);
    let vocabulary = walker.take(2);
    let commitment = walker.take(32);

    let (root_count, roots) = walker.take_count();
    assert_eq!(roots, 1, "the profile always carries exactly one root row");
    let row_tag = walker.take(1);
    assert_eq!(encoded[row_tag.clone()], [ROOT_ROW_TAG]);
    let entry = walker.take_id();
    let (scalar_count, scalars) = walker.take_count();
    let scalars = (0..scalars).map(|_| walker.walk_scalar_schema()).collect();
    let (structural_count, structurals) = walker.take_count();
    let structurals = (0..structurals)
        .map(|_| walker.walk_structural_schema())
        .collect();
    let result = walker.walk_result_schema();
    let root = RootSpan {
        row_tag,
        entry,
        scalar_count,
        scalars,
        structural_count,
        structurals,
        result,
    };

    let (crash_count, crashes) = walker.take_count();
    let crashes = (0..crashes)
        .map(|_| {
            let row_start = walker.offset;
            let row_tag = walker.take(1);
            assert_eq!(encoded[row_tag.clone()], [CRASH_ROW_TAG]);
            let machine = walker.take_id();
            let block = walker.take_id();
            let edge = walker.take_id();
            let cause = walker.take(1);
            CrashSpan {
                row: row_start..walker.offset,
                row_tag,
                machine,
                block,
                edge,
                cause,
            }
        })
        .collect();

    let (boundary_crash_count, boundary_crashes) = walker.take_count();
    let boundary_crashes = (0..boundary_crashes)
        .map(|_| {
            let row_start = walker.offset;
            let row_tag = walker.take(1);
            assert_eq!(encoded[row_tag.clone()], [BOUNDARY_CRASH_ROW_TAG]);
            let machine = walker.take_id();
            let block = walker.take_id();
            let operation = walker.take_id();
            let boundary = walker.take_id();
            let (identity_len, identity) = walker.take_string();
            let (route_cause, alternative_count, guards) = walker.walk_route_bucket();
            BoundaryCrashSpan {
                row: row_start..walker.offset,
                row_tag,
                machine,
                block,
                operation,
                boundary,
                identity_len,
                identity,
                route_cause,
                alternative_count,
                guards,
            }
        })
        .collect();

    // These modules own no Trapping primitive, so revision 2's operation
    // crash group is present and empty.
    let (operation_crash_count, operation_crashes) = walker.take_count();
    assert_eq!(
        operation_crashes, 0,
        "no Trapping operation in these modules"
    );

    let (event_count, events) = walker.take_count();
    let events = (0..events)
        .map(|_| {
            let row_start = walker.offset;
            let row_tag = walker.take(1);
            assert_eq!(encoded[row_tag.clone()], [ORDINARY_EVENT_ROW_TAG]);
            let machine = walker.take_id();
            let block = walker.take_id();
            let operation = walker.take_id();
            let kind_tag = walker.take(1);
            assert!(
                matches!(
                    encoded[kind_tag.start],
                    BOUNDARY_CALL_EVENT_TAG | PORT_WRITE_EVENT_TAG
                ),
                "unexpected ordinary event kind {}",
                encoded[kind_tag.start],
            );
            let kind_target = walker.take_id();
            let (identity_len, identity) = walker.take_string();
            let (scalar_count, scalars) = walker.take_count();
            let scalars = (0..scalars).map(|_| walker.walk_scalar_schema()).collect();
            let (structural_count, structurals) = walker.take_count();
            let structurals = (0..structurals)
                .map(|_| walker.walk_structural_schema())
                .collect();
            let result = walker.walk_result_schema();
            EventSpan {
                row: row_start..walker.offset,
                row_tag,
                machine,
                block,
                operation,
                kind_tag,
                kind_target,
                identity_len,
                identity,
                scalar_count,
                scalars,
                structural_count,
                structurals,
                result,
            }
        })
        .collect();

    let (external_count, externals) = walker.take_count();
    assert_eq!(externals, 0, "the bounded rung fixes an empty roster");
    ProfileSpans {
        domain,
        version,
        vocabulary,
        commitment,
        root_count,
        root,
        crash_count,
        crashes,
        boundary_crash_count,
        boundary_crashes,
        operation_crash_count,
        event_count,
        events,
        external_count,
        end: walker.offset,
    }
}

/// One module whose trace profile exercises every wire field: the entry
/// machine binds scalar and structural parameters (including a projected
/// qualification path), observes one port write and one boundary call whose
/// declaration publishes two crash routes and a structural result, and a
/// second machine contributes a terminator crash site.
fn custody_module() -> TerminalModule {
    let machine = machine_id(11);
    let block = block_id(12);
    let crash_machine = machine_id(21);
    let crash_block = block_id(22);
    let service = service_id(1);
    let boundary = boundary_machine_id(1);
    let flag = value_id(1);
    let byte = value_id(2);
    let produced = value_id(4);
    let first_place = place_id(1);
    let second_place = place_id(2);
    let result_place = place_id(3);
    let first_type = structural_type_id(1);
    let second_type = structural_type_id(2);
    let domain = structural_domain_id(1);
    let u8_scalar = u8_type();
    let caller_routes = vec![
        CrashRouteBucket {
            cause: CrashCause::Trap,
            alternatives: vec![CrashRouteGuard::Truth],
        },
        CrashRouteBucket {
            cause: CrashCause::Abort,
            alternatives: vec![CrashRouteGuard::Truth],
        },
    ];
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine,
        structural_types: vec![
            StructuralTypeDeclaration {
                id: first_type,
                identity: "Console::Message".into(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
            StructuralTypeDeclaration {
                id: second_type,
                identity: "Console::Context".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        id: structural_field_id(1),
                        identity: "region".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(first_type),
                    }],
                },
            },
        ],
        structural_domains: vec![StructuralDomainDeclaration {
            byte_predicate: None,
            establishment_routes: Vec::new(),
            id: domain,
            semantic_domain: DomainSemanticId::new(1).expect("nonzero domain semantic identity"),
            identity: "Omega::Region".into(),
            carrier: first_type,
            content_projection: None,
        }],
        services: vec![ServiceDeclaration {
            id: service,
            identity: "PortIo::write-byte".into(),
            parents: Vec::new(),
        }],
        root_service_reach: terminal_psi::TerminalRootServiceReach {
            concrete: vec![service],
            installation_dependencies: Vec::new(),
        },
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: vec![BoundaryMachineDeclaration {
            unconditional_termination: false,
            completion: terminal_psi::BoundaryCompletion::NormalReturn,
            kind: terminal_psi::BoundaryRequirementKind::Trait,
            scalar_ensures: Vec::new(),
            scalar_requires: Vec::new(),
            fixed_service_reach: Vec::new(),
            id: boundary,
            identity: "Console::publish".into(),
            attachment: None,
            parameter_order: vec![
                terminal_psi::BoundaryParameterKind::Scalar,
                terminal_psi::BoundaryParameterKind::Scalar,
                terminal_psi::BoundaryParameterKind::Structural,
                terminal_psi::BoundaryParameterKind::Structural,
            ],
            scalar_parameters: vec![ScalarType::Boolean, u8_scalar],
            structural_parameters: vec![
                StructuralParameterDeclaration {
                    place: place_id(5),
                    position: 0,
                    is_self: false,
                    structural_type: first_type,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: StructuralAccess::Owned,
                    qualifications: vec![domain],
                    projected_qualifications: Vec::new(),
                },
                StructuralParameterDeclaration {
                    place: place_id(6),
                    position: 1,
                    is_self: false,
                    structural_type: second_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::SharedBorrow,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                },
            ],
            result: BoundaryMachineResult::Structural(BoundaryStructuralResultDeclaration {
                structural_type: first_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
            }),
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
            crash_routes: caller_routes.clone(),
        }],
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![
            TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: machine,
                attachment: None,
                parameters: vec![
                    ValueDeclaration {
                        qualifications: Default::default(),
                        id: flag,
                        scalar_type: ScalarType::Boolean,
                    },
                    ValueDeclaration {
                        qualifications: Default::default(),
                        id: byte,
                        scalar_type: u8_scalar,
                    },
                ],
                structural_parameters: vec![
                    StructuralParameterDeclaration {
                        place: first_place,
                        position: 0,
                        is_self: false,
                        structural_type: first_type,
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        access: StructuralAccess::Owned,
                        qualifications: vec![domain],
                        projected_qualifications: Vec::new(),
                    },
                    StructuralParameterDeclaration {
                        place: second_place,
                        position: 1,
                        is_self: false,
                        structural_type: second_type,
                        multiplicity: StructuralMultiplicity::Affine,
                        access: StructuralAccess::SharedBorrow,
                        qualifications: Vec::new(),
                        projected_qualifications: vec![StructuralPathQualification {
                            path: vec![StructuralPathSegment::Field("region".into())],
                            domain,
                        }],
                    },
                ],
                ranked_scc: None,
                result: TerminalMachineResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: value_id(5),
                    scalar_type: u8_scalar,
                }),
                structural_places: vec![
                    StructuralPlaceDeclaration {
                        id: first_place,
                        kind: StructuralPlaceKind::Parameter {
                            position: 0,
                            is_self: false,
                        },
                    },
                    StructuralPlaceDeclaration {
                        id: second_place,
                        kind: StructuralPlaceKind::Parameter {
                            position: 1,
                            is_self: false,
                        },
                    },
                    StructuralPlaceDeclaration {
                        id: result_place,
                        kind: StructuralPlaceKind::OperationResult {
                            producer: operation_id(2),
                            structural_type: first_type,
                        },
                    },
                ],
                entry_claims: Vec::new(),
                published_service_ceiling: vec![service],
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block,
                blocks: vec![Block {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block,
                    parameters: Vec::new(),
                    operations: vec![
                        Operation {
                            static_reach_binding: None,
                            suspension_crossing: None,
                            id: operation_id(1),
                            result: OperationResult::Unit,
                            kind: OperationKind::PortWrite {
                                service,
                                port: 0x3f8,
                                value: b'X',
                            },
                        },
                        Operation {
                            static_reach_binding: None,
                            suspension_crossing: None,
                            id: operation_id(2),
                            result: OperationResult::Structural(StructuralOperationResult {
                                qualification_establishments: Vec::new(),
                                place: result_place,
                                structural_type: first_type,
                                multiplicity: StructuralMultiplicity::Unrestricted,
                                qualifications: Vec::new(),
                                projected_qualifications: Vec::new(),
                                claims: Vec::new(),
                            }),
                            kind: OperationKind::BoundaryCall {
                                requirement_obligations: Vec::new(),
                                boundary,
                                arguments: vec![flag, byte],
                                structural_arguments: vec![
                                    StructuralArgument {
                                        place: first_place,
                                        path: Vec::new(),
                                        access: StructuralAccess::Owned,
                                    },
                                    StructuralArgument {
                                        place: second_place,
                                        path: Vec::new(),
                                        access: StructuralAccess::SharedBorrow,
                                    },
                                ],
                                completion_receipts: Vec::new(),
                            },
                        },
                        Operation {
                            static_reach_binding: None,
                            suspension_crossing: None,
                            id: operation_id(3),
                            result: OperationResult::Scalar(ValueDeclaration {
                                qualifications: Default::default(),
                                id: produced,
                                scalar_type: u8_scalar,
                            }),
                            kind: OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(7),
                            },
                        },
                    ],
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: edge_id(14),
                        value: produced,
                    },
                }],
                contract: MachineContract {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    id: contract_id(13),
                    crash_routes: caller_routes,
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            },
            TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: crash_machine,
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
                entry: crash_block,
                blocks: vec![Block {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: crash_block,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Crash {
                        edge: edge_id(25),
                        cause: CrashCause::Trap,
                        site_guard: Vec::new(),
                        frontier_lower_bound: Vec::new(),
                    },
                }],
                contract: MachineContract {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    id: contract_id(23),
                    crash_routes: vec![CrashRouteBucket {
                        cause: CrashCause::Trap,
                        alternatives: vec![CrashRouteGuard::Truth],
                    }],
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            },
            TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: machine_id(31),
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
                entry: block_id(32),
                blocks: vec![Block {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block_id(32),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Crash {
                        edge: edge_id(35),
                        cause: CrashCause::Abort,
                        site_guard: Vec::new(),
                        frontier_lower_bound: Vec::new(),
                    },
                }],
                contract: MachineContract {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    id: contract_id(33),
                    crash_routes: vec![CrashRouteBucket {
                        cause: CrashCause::Abort,
                        alternatives: vec![CrashRouteGuard::Truth],
                    }],
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            },
        ],
    }
}

#[test]
fn terminal_trace_v1_profile_rejects_every_one_field_substitution() {
    let module = custody_module();
    let profile = reconstruct_canonical_terminal_trace_v1_profile(&module)
        .expect("the fixture module reconstructs a canonical profile");
    assert_eq!(profile.crash_sites.len(), 2);
    assert_eq!(profile.boundary_crash_sites.len(), 2);
    assert_eq!(profile.ordinary_events.len(), 2);
    let bytes = encode_terminal_trace_v1_profile(&profile).expect("canonical profile bytes");
    let spans = profile_spans(&bytes);
    assert_eq!(bytes.len(), spans.end, "span map must cover the profile");
    assert_eq!(
        decode_terminal_trace_v1_profile(&bytes).expect("canonical decode"),
        profile,
        "the canonical profile round-trips"
    );
    assert_eq!(
        accept_terminal_trace_v1_profile(&module, &bytes)
            .expect("module-bound acceptance replays the profile"),
        profile,
    );
    let identity = terminal_psi_identity(&module).expect("honest module identity");
    assert_eq!(profile.module_identity, identity);

    // Every representable substitution still forms a canonical profile that
    // re-encodes, honestly retains (or recomputes) the containing module
    // identity, decodes back to the substituted value, and is rejected by
    // module-bound replay: the verifier's own reconstruction of the same
    // module never yields the substituted row, identity or schema — all
    // asserted inside the matrix's checker. The donor is the authentic
    // profile of a sibling module whose reconstruction differs on every
    // row.
    let mut donor_module = module.clone();
    donor_module.machines[2].id = machine_id(41);
    let donor = reconstruct_canonical_terminal_trace_v1_profile(&donor_module)
        .expect("the foreign module reconstructs a canonical profile");
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "TerminalTraceV1 observation profile",
        fields: TraceProfileCustodyFieldForTest::INVENTORY,
        honest: &|| profile.clone(),
        donor,
        custody: &|record: &TerminalTraceV1Profile| record.clone(),
        substitute: &substitute_trace_profile_for_test,
        check: &|record: &TerminalTraceV1Profile| {
            let encoded = encode_terminal_trace_v1_profile(record)
                .expect("a declared leg must encode canonically");
            assert_eq!(
                decode_terminal_trace_v1_profile(&encoded),
                Ok(record.clone()),
                "a declared leg must decode canonically under the same subject"
            );
            accept_terminal_trace_v1_profile(&module, &encoded)
        },
        outcome: &trace_profile_custody_outcome,
        joined_replay: None,
    });
    // A substitution that cannot be represented canonically rejects inside the
    // encoder's canonical-order validation.
    let encode_rejected = |name: &'static str,
                           mutated: &TerminalTraceV1Profile,
                           expected: TerminalTraceV1ProfileCodecError| {
        assert_eq!(
            encode_terminal_trace_v1_profile(mutated),
            Err(expected),
            "{name} must reject at canonical encoding"
        );
    };
    // A substitution expressible only on the wire rejects at canonical decode.
    let rejected =
        |name: &'static str, mutated: &[u8], expected: TerminalTraceV1ProfileCodecError| {
            assert_eq!(
                decode_terminal_trace_v1_profile(mutated),
                Err(expected),
                "{name} must reject at canonical decoding"
            );
        };
    let rejected_unspecified = |name: &'static str, mutated: &[u8]| {
        assert!(
            decode_terminal_trace_v1_profile(mutated).is_err(),
            "{name} must reject at canonical decoding"
        );
    };

    // --- envelope: domain, schema version, vocabulary marker and the module
    // commitment bind the profile to this exact canonical module ---

    let mut mutated = bytes.clone();
    mutated[spans.domain.start] ^= 0xFF;
    rejected(
        "the observation-profile domain",
        &mutated,
        TerminalTraceV1ProfileCodecError::InvalidDomain,
    );

    let mut mutated = bytes.clone();
    // Revision 1 predates the operation crash group; its version number no
    // longer names a decodable row list.
    mutated[spans.version.clone()].copy_from_slice(&1_u16.to_le_bytes());
    rejected(
        "the schema version",
        &mutated,
        TerminalTraceV1ProfileCodecError::UnsupportedSchemaVersion(1),
    );

    // An extra operation crash row is not this module's observer.
    let mut mutated = bytes.clone();
    mutated[spans.operation_crash_count.clone()].copy_from_slice(&1_u32.to_le_bytes());
    rejected_unspecified("an operation crash row count", &mutated);

    let mut mutated = bytes.clone();
    mutated[spans.vocabulary.clone()].copy_from_slice(&u16::MAX.to_le_bytes());
    rejected(
        "the vocabulary marker",
        &mutated,
        TerminalTraceV1ProfileCodecError::UnsupportedVocabularyMarker(u16::MAX),
    );

    // A zero commitment is non-canonical outright; any other commitment still
    // decodes but names a foreign program identity, and replay against this
    // module's honest reconstruction rejects it.
    let mut mutated = bytes.clone();
    mutated[spans.commitment.clone()].copy_from_slice(&[0; 32]);
    rejected(
        "a zero module commitment",
        &mutated,
        TerminalTraceV1ProfileCodecError::ZeroModuleCommitment,
    );
    // The `ModuleIdentitySubstituted` inventory leg exercises the foreign
    // commitment through the matrix.

    // The module-side direction of the same join: replaying the retained bytes
    // against a module whose reconstruction differs also rejects. The
    // substitution keeps the machine roster canonical while changing the
    // identity the profile's crash rows were reconstructed from.
    assert_eq!(
        accept_terminal_trace_v1_profile(&donor_module, &bytes),
        Err(TerminalTraceV1ProfileAcceptanceError::ProfileMismatch),
        "a substituted module must reject at replay"
    );
    let mut malformed_module = module.clone();
    malformed_module.machines[1].id = machine_id(41);
    assert!(
        matches!(
            accept_terminal_trace_v1_profile(&malformed_module, &bytes),
            Err(TerminalTraceV1ProfileAcceptanceError::InvalidCanonicalModule(_))
        ),
        "a module substitution breaking canonical machine order must reject before replay"
    );

    let mut mutated = bytes.clone();
    mutated[spans.root_count.clone()].copy_from_slice(&0_u32.to_le_bytes());
    rejected(
        "a missing root row",
        &mutated,
        TerminalTraceV1ProfileCodecError::InvalidRootCount(0),
    );
    let mut mutated = bytes.clone();
    mutated[spans.root_count.clone()].copy_from_slice(&2_u32.to_le_bytes());
    rejected(
        "a second root row",
        &mutated,
        TerminalTraceV1ProfileCodecError::InvalidRootCount(2),
    );

    let mut mutated = bytes.clone();
    mutated[spans.root.row_tag.clone()].copy_from_slice(&[9]);
    rejected(
        "an unknown root row tag",
        &mutated,
        TerminalTraceV1ProfileCodecError::InvalidRowTag {
            group: "root",
            tag: 9,
        },
    );

    // --- root row: entry machine, scalar input schemas, structural input
    // schemas and the result schema ---

    let mut mutated = bytes.clone();
    mutated[spans.root.entry.clone()].copy_from_slice(&0_u64.to_le_bytes());
    rejected(
        "a zero root entry machine",
        &mutated,
        TerminalTraceV1ProfileCodecError::Wire(CodecError::ZeroIdentity("trace root entry")),
    );
    // The `RootEntryToModuleMachine` and `RootEntryForeign` inventory legs
    // exercise the entry machine — a real module machine that is not the
    // entry, and a foreign machine — through the matrix.

    // Scalar input roster shape and each schema's fields.
    let mut mutated = bytes.clone();
    mutated[spans.root.scalar_count.clone()].copy_from_slice(&1_u32.to_le_bytes());
    rejected_unspecified("a shrunken root scalar roster", &mutated);
    let mut mutated = bytes.clone();
    mutated[spans.root.scalar_count.clone()].copy_from_slice(&3_u32.to_le_bytes());
    rejected_unspecified("an over-counted root scalar roster", &mutated);

    // The `RootScalarInputDropped`, `RootScalarInputInserted`, and
    // `RootScalarInputTypeSubstituted` inventory legs exercise the scalar
    // input roster shape and the Boolean schema's free type axis through
    // the matrix; `RootScalarIntegerWidthSubstituted` and
    // `RootScalarIntegerSignSubstituted` exercise the u8 schema's integer
    // description.
    let mut mutated = bytes.clone();
    mutated[spans.root.scalars[0].comparison.clone()].copy_from_slice(&[9]);
    rejected(
        "a scalar input's comparison",
        &mutated,
        TerminalTraceV1ProfileCodecError::InvalidComparison(9),
    );
    let mut mutated = bytes.clone();
    mutated[spans.root.scalars[0].type_tag.clone()].copy_from_slice(&[9]);
    rejected(
        "a scalar input's type tag",
        &mutated,
        TerminalTraceV1ProfileCodecError::Wire(CodecError::InvalidTag("ScalarType", 9)),
    );
    let (carrier, _) = spans.root.scalars[1]
        .integer
        .clone()
        .expect("the u8 input encodes an integer description");
    let mut mutated = bytes.clone();
    mutated[carrier].copy_from_slice(&[9]);
    rejected(
        "a scalar input's integer carrier",
        &mutated,
        TerminalTraceV1ProfileCodecError::Wire(CodecError::InvalidTag("IntegerSign", 9)),
    );

    // Structural input roster shape and each schema's fields.
    let mut mutated = bytes.clone();
    mutated[spans.root.structural_count.clone()].copy_from_slice(&1_u32.to_le_bytes());
    rejected_unspecified("a shrunken root structural roster", &mutated);
    let mut mutated = bytes.clone();
    mutated[spans.root.structural_count.clone()].copy_from_slice(&3_u32.to_le_bytes());
    rejected_unspecified("an over-counted root structural roster", &mutated);

    // The `RootStructuralInputDropped` and `RootStructuralInputInserted`
    // inventory legs exercise the structural input roster shape through the
    // matrix.

    let first_input = &spans.root.structurals[0];
    let mut mutated = bytes.clone();
    mutated[first_input.comparison.clone()].copy_from_slice(&[9]);
    rejected(
        "a structural input's comparison",
        &mutated,
        TerminalTraceV1ProfileCodecError::InvalidComparison(9),
    );
    let mut mutated = bytes.clone();
    mutated[first_input.structural_type.clone()].copy_from_slice(&0_u64.to_le_bytes());
    rejected(
        "a zero structural input type",
        &mutated,
        TerminalTraceV1ProfileCodecError::Wire(CodecError::ZeroIdentity("trace structural type")),
    );
    // The `RootStructuralInputTypeSubstituted`,
    // `RootStructuralInputMultiplicitySubstituted`, and
    // `RootStructuralInputAccessSubstituted` inventory legs exercise each
    // free field through the matrix.
    let mut mutated = bytes.clone();
    mutated[first_input.multiplicity.clone()].copy_from_slice(&[9]);
    rejected(
        "a structural input's multiplicity",
        &mutated,
        TerminalTraceV1ProfileCodecError::InvalidStructuralMultiplicity(9),
    );
    let mut mutated = bytes.clone();
    mutated[first_input.access.clone()].copy_from_slice(&[9]);
    rejected(
        "a structural input's access",
        &mutated,
        TerminalTraceV1ProfileCodecError::InvalidStructuralAccess(9),
    );

    // Direct qualifications: the roster count is order-bound, each identity is
    // independently bound, and duplicates are non-canonical at encoding.
    let mut mutated = bytes.clone();
    mutated[first_input.qualification_count.clone()].copy_from_slice(&0_u32.to_le_bytes());
    rejected_unspecified("a cleared direct qualification roster", &mutated);
    let mut mutated = bytes.clone();
    mutated[first_input.qualifications[0].clone()].copy_from_slice(&0_u64.to_le_bytes());
    rejected(
        "a zero direct qualification",
        &mutated,
        TerminalTraceV1ProfileCodecError::Wire(CodecError::ZeroIdentity(
            "trace structural qualification",
        )),
    );
    // The `RootStructuralQualificationSubstituted` inventory leg exercises
    // the free qualification identity through the matrix.
    let mut changed = profile.clone();
    changed.root.structural_inputs[0]
        .qualifications
        .push(domain(&profile));
    encode_rejected(
        "a duplicated direct qualification",
        &changed,
        TerminalTraceV1ProfileCodecError::NonCanonicalStructuralQualifications,
    );

    // The projected qualification on the second structural input binds its
    // path segments and target domain.
    let second_input = &spans.root.structurals[1];
    assert_eq!(second_input.projected.len(), 1);
    let projection = &second_input.projected[0];
    assert_eq!(projection.segments.len(), 1);
    let mut mutated = bytes.clone();
    mutated[second_input.projected_count.clone()].copy_from_slice(&0_u32.to_le_bytes());
    rejected_unspecified("a cleared projected qualification roster", &mutated);
    let mut mutated = bytes.clone();
    mutated[second_input.projected_count.clone()].copy_from_slice(&2_u32.to_le_bytes());
    rejected_unspecified("an over-counted projected qualification roster", &mutated);
    let mut mutated = bytes.clone();
    mutated[projection.path_count.clone()].copy_from_slice(&0_u32.to_le_bytes());
    rejected_unspecified("a cleared projected qualification path", &mutated);
    let mut mutated = bytes.clone();
    mutated[projection.path_count.clone()].copy_from_slice(&2_u32.to_le_bytes());
    rejected_unspecified("an over-counted projected qualification path", &mutated);
    let mut mutated = bytes.clone();
    mutated[projection.segments[0].tag.clone()].copy_from_slice(&[9]);
    rejected(
        "a projected path's segment tag",
        &mutated,
        TerminalTraceV1ProfileCodecError::Wire(CodecError::InvalidTag("StructuralPathSegment", 9)),
    );
    let field_len = projection.segments[0]
        .field_len
        .clone()
        .expect("a field segment carries a length");
    let mut mutated = bytes.clone();
    mutated[field_len].copy_from_slice(&u32::MAX.to_le_bytes());
    rejected(
        "a projected path's field length",
        &mutated,
        TerminalTraceV1ProfileCodecError::Wire(CodecError::StringTooLong("structural path field")),
    );
    let field_value = projection.segments[0]
        .value
        .clone()
        .expect("a field segment carries its identity");
    let mut mutated = bytes.clone();
    mutated[field_value.clone()].copy_from_slice(&[0xFF; 6]);
    rejected_unspecified("a projected path's invalid field encoding", &mutated);
    // The `ProjectedPathFieldRenamed` inventory leg exercises the free
    // segment identity through the matrix.
    let mut mutated = bytes.clone();
    mutated[projection.domain.clone()].copy_from_slice(&0_u64.to_le_bytes());
    rejected(
        "a zero projected qualification domain",
        &mutated,
        TerminalTraceV1ProfileCodecError::Wire(CodecError::ZeroIdentity("StructuralDomainId")),
    );
    // The `ProjectedQualificationDomainSubstituted` inventory leg exercises
    // the free domain identity through the matrix.
    let mut changed = profile.clone();
    let duplicated_projection =
        changed.root.structural_inputs[1].projected_qualifications[0].clone();
    changed.root.structural_inputs[1]
        .projected_qualifications
        .push(duplicated_projection);
    encode_rejected(
        "a duplicated projected qualification",
        &changed,
        TerminalTraceV1ProfileCodecError::NonCanonicalStructuralQualifications,
    );

    // Root result schema: the tag is closed and the scalar payload is bound.
    let mut mutated = bytes.clone();
    mutated[spans.root.result.tag.clone()].copy_from_slice(&[9]);
    rejected(
        "the root result tag",
        &mutated,
        TerminalTraceV1ProfileCodecError::InvalidResultTag(9),
    );
    // The `RootResultUnit`, `RootResultScalarRetyped`, and
    // `RootResultStructural` inventory legs exercise the result tag's
    // representable variants through the matrix.

    // The scalar payload inside the root result binds its own fields.
    let result_scalar = spans.root.result.scalar.as_ref().expect("scalar result");
    let mut mutated = bytes.clone();
    mutated[result_scalar.comparison.clone()].copy_from_slice(&[9]);
    rejected(
        "the root result's comparison",
        &mutated,
        TerminalTraceV1ProfileCodecError::InvalidComparison(9),
    );
    let mut mutated = bytes.clone();
    mutated[result_scalar.type_tag.clone()].copy_from_slice(&[9]);
    rejected(
        "the root result's scalar type tag",
        &mutated,
        TerminalTraceV1ProfileCodecError::Wire(CodecError::InvalidTag("ScalarType", 9)),
    );
    let (carrier, _) = result_scalar
        .integer
        .clone()
        .expect("the u8 result encodes an integer description");
    let mut mutated = bytes.clone();
    mutated[carrier].copy_from_slice(&[9]);
    rejected(
        "the root result's integer carrier",
        &mutated,
        TerminalTraceV1ProfileCodecError::Wire(CodecError::InvalidTag("IntegerSign", 9)),
    );
    // The `RootResultIntegerWidthSubstituted` inventory leg exercises the
    // result's free integer width through the matrix.

    // --- crash-site roster: sorted (machine, block, edge) rows bind the exact
    // terminator crash site and cause ---

    let mut mutated = bytes.clone();
    mutated[spans.crash_count.clone()].copy_from_slice(&0_u32.to_le_bytes());
    rejected_unspecified("a cleared crash-site roster", &mutated);
    let mut mutated = bytes.clone();
    mutated[spans.crash_count.clone()].copy_from_slice(&3_u32.to_le_bytes());
    rejected_unspecified("an over-counted crash-site roster", &mutated);

    let crash = &spans.crashes[0];
    let mut mutated = bytes.clone();
    mutated[crash.row_tag.clone()].copy_from_slice(&[9]);
    rejected(
        "an unknown crash row tag",
        &mutated,
        TerminalTraceV1ProfileCodecError::InvalidRowTag {
            group: "crash",
            tag: 9,
        },
    );
    for (name, span, label) in [
        (
            "a zero crash machine",
            crash.machine.clone(),
            "trace crash machine",
        ),
        (
            "a zero crash block",
            crash.block.clone(),
            "trace crash block",
        ),
        ("a zero crash edge", crash.edge.clone(), "trace crash edge"),
    ] {
        let mut mutated = bytes.clone();
        mutated[span].copy_from_slice(&0_u64.to_le_bytes());
        rejected(
            name,
            &mutated,
            TerminalTraceV1ProfileCodecError::Wire(CodecError::ZeroIdentity(label)),
        );
    }
    // The `CrashMachineSubstituted`, `CrashBlockSubstituted`, and
    // `CrashEdgeSubstituted` inventory legs exercise each free coordinate of
    // the first row through the matrix.
    let mut mutated = bytes.clone();
    mutated[crash.cause.clone()].copy_from_slice(&[9]);
    rejected(
        "an unknown crash cause",
        &mutated,
        TerminalTraceV1ProfileCodecError::InvalidCrashCause(9),
    );
    // The `CrashCauseSubstituted` inventory leg exercises the cause through
    // the matrix.

    // The second crash row's fields bind independently; the cause stays
    // representable because it is not part of the site-order key. The
    // `SecondCrashMachineSubstituted`, `SecondCrashBlockSubstituted`,
    // `SecondCrashEdgeSubstituted`, and `SecondCrashCauseSubstituted`
    // inventory legs exercise all four through the matrix.

    // Roster shape: dropped, inserted, duplicated and reordered rows. The
    // `CrashRosterCleared`, `CrashRowDropped`, and `CrashRowInserted`
    // inventory legs exercise the representable shapes through the matrix.
    let mut changed = profile.clone();
    changed.crash_sites.swap(0, 1);
    encode_rejected(
        "a reordered crash site roster",
        &changed,
        TerminalTraceV1ProfileCodecError::NonCanonicalCrashSiteOrder,
    );
    let mut changed = profile.clone();
    changed.crash_sites.push(changed.crash_sites[0]);
    encode_rejected(
        "a duplicated crash site row",
        &changed,
        TerminalTraceV1ProfileCodecError::NonCanonicalCrashSiteOrder,
    );

    // --- boundary crash roster: operation-level call-site rows bind the
    // invoked boundary, its public identity and the declared route bucket ---

    let mut mutated = bytes.clone();
    mutated[spans.boundary_crash_count.clone()].copy_from_slice(&1_u32.to_le_bytes());
    rejected_unspecified("a shrunken boundary crash roster", &mutated);
    let mut mutated = bytes.clone();
    mutated[spans.boundary_crash_count.clone()].copy_from_slice(&3_u32.to_le_bytes());
    rejected_unspecified("an over-counted boundary crash roster", &mutated);

    let boundary_crash = &spans.boundary_crashes[0];
    let mut mutated = bytes.clone();
    mutated[boundary_crash.row_tag.clone()].copy_from_slice(&[9]);
    rejected(
        "an unknown boundary crash row tag",
        &mutated,
        TerminalTraceV1ProfileCodecError::InvalidRowTag {
            group: "boundary crash",
            tag: 9,
        },
    );
    for (name, span, label) in [
        (
            "a zero boundary crash machine",
            boundary_crash.machine.clone(),
            "trace boundary crash machine",
        ),
        (
            "a zero boundary crash block",
            boundary_crash.block.clone(),
            "trace boundary crash block",
        ),
        (
            "a zero boundary crash operation",
            boundary_crash.operation.clone(),
            "trace boundary crash operation",
        ),
        (
            "a zero boundary crash boundary",
            boundary_crash.boundary.clone(),
            "trace boundary crash boundary",
        ),
    ] {
        let mut mutated = bytes.clone();
        mutated[span].copy_from_slice(&0_u64.to_le_bytes());
        rejected(
            name,
            &mutated,
            TerminalTraceV1ProfileCodecError::Wire(CodecError::ZeroIdentity(label)),
        );
    }
    // The site coordinates are order-bound across the two rows: a larger
    // value on the first row breaks canonical order while a smaller one
    // stays representable and still rejects at replay — the
    // `BoundaryCrashBlockSubstituted`, `BoundaryCrashOperationSubstituted`,
    // and `BoundaryCrashBoundarySubstituted` inventory legs exercise the
    // representable directions through the matrix.
    for (name, span, raw) in [
        (
            "a boundary crash machine breaking roster order",
            boundary_crash.machine.clone(),
            31_u64,
        ),
        (
            "a boundary crash block breaking roster order",
            boundary_crash.block.clone(),
            31_u64,
        ),
        (
            "a boundary crash operation breaking roster order",
            boundary_crash.operation.clone(),
            31_u64,
        ),
    ] {
        let mut mutated = bytes.clone();
        mutated[span].copy_from_slice(&raw.to_le_bytes());
        rejected(
            name,
            &mutated,
            TerminalTraceV1ProfileCodecError::NonCanonicalBoundaryCrashSiteOrder,
        );
    }
    // The `BoundaryCrashMachineSubstituted` inventory leg exercises the
    // representable machine direction through the matrix.

    // The boundary identity string: length and bytes are independently bound.
    let mut mutated = bytes.clone();
    mutated[boundary_crash.identity_len.clone()].copy_from_slice(&u32::MAX.to_le_bytes());
    rejected_unspecified("an over-long boundary crash identity", &mutated);
    let mut mutated = bytes.clone();
    mutated[boundary_crash.identity.clone()].copy_from_slice(&[0xFF; 16]);
    rejected_unspecified("an invalid boundary crash identity encoding", &mutated);
    // The `BoundaryCrashIdentitySubstituted` and `BoundaryCrashIdentityLookalike`
    // inventory legs exercise the free identity string through the matrix.

    // The route bucket: the cause is order-bound between the two rows, the
    // alternatives cannot be empty, and the guard tag is closed.
    let mut mutated = bytes.clone();
    mutated[boundary_crash.route_cause.clone()].copy_from_slice(&[9]);
    rejected(
        "a boundary crash route's cause tag",
        &mutated,
        TerminalTraceV1ProfileCodecError::Wire(CodecError::InvalidTag("CrashCause", 9)),
    );
    let mut mutated = bytes.clone();
    mutated[boundary_crash.route_cause.clone()].copy_from_slice(&[2]);
    rejected(
        "a boundary crash route's cause colliding with its sibling",
        &mutated,
        TerminalTraceV1ProfileCodecError::NonCanonicalBoundaryCrashSiteOrder,
    );
    let mut mutated = bytes.clone();
    mutated[spans.boundary_crashes[1].route_cause.clone()].copy_from_slice(&[1]);
    rejected(
        "a boundary crash route's cause colliding with its predecessor",
        &mutated,
        TerminalTraceV1ProfileCodecError::NonCanonicalBoundaryCrashSiteOrder,
    );
    // Zeroing the wire count desynchronizes the sibling row; clearing the
    // roster itself is representable input that canonical validation refuses.
    let mut mutated = bytes.clone();
    mutated[boundary_crash.alternative_count.clone()].copy_from_slice(&0_u32.to_le_bytes());
    rejected_unspecified("a zeroed boundary crash alternative count", &mutated);
    let mut changed = profile.clone();
    changed.boundary_crash_sites[0].route.alternatives.clear();
    encode_rejected(
        "an empty boundary crash route bucket",
        &changed,
        TerminalTraceV1ProfileCodecError::Wire(CodecError::NonCanonicalOrder(
            "boundary crash route alternatives",
        )),
    );
    let mut mutated = bytes.clone();
    mutated[boundary_crash.alternative_count.clone()].copy_from_slice(&2_u32.to_le_bytes());
    rejected_unspecified("an over-counted boundary crash route bucket", &mutated);
    let mut mutated = bytes.clone();
    mutated[boundary_crash.guards[0].clone()].copy_from_slice(&[9]);
    rejected(
        "an unknown boundary crash route guard",
        &mutated,
        TerminalTraceV1ProfileCodecError::Wire(CodecError::InvalidTag("CrashRouteGuard", 9)),
    );
    let mut changed = profile.clone();
    changed.boundary_crash_sites.swap(0, 1);
    encode_rejected(
        "a reordered boundary crash roster",
        &changed,
        TerminalTraceV1ProfileCodecError::NonCanonicalBoundaryCrashSiteOrder,
    );
    let mut changed = profile.clone();
    changed
        .boundary_crash_sites
        .push(changed.boundary_crash_sites[0].clone());
    encode_rejected(
        "a duplicated boundary crash row",
        &changed,
        TerminalTraceV1ProfileCodecError::NonCanonicalBoundaryCrashSiteOrder,
    );
    // The `BoundaryCrashRowDropped` and `BoundaryCrashRowOnForeignOperation`
    // inventory legs exercise the representable roster shapes through the
    // matrix.
    let mut changed = profile.clone();
    changed.boundary_crash_sites[1]
        .route
        .alternatives
        .push(CrashRouteGuard::Predicate(
            terminal_psi::CrashPredicateTerm::new(semantic_vocabulary::Proposition::Equal(
                semantic_vocabulary::ScalarTerm::boolean(true),
                semantic_vocabulary::ScalarTerm::boolean(true),
            )),
        ));
    // A Truth guard cannot coexist with another alternative.
    encode_rejected(
        "a boundary crash route widening beyond Truth",
        &changed,
        TerminalTraceV1ProfileCodecError::Wire(CodecError::NonCanonicalOrder(
            "boundary crash route alternatives",
        )),
    );

    // --- ordinary event roster: sorted call-site rows bind kind, target,
    // argument schemas and result schema ---

    let mut mutated = bytes.clone();
    mutated[spans.event_count.clone()].copy_from_slice(&1_u32.to_le_bytes());
    rejected_unspecified("a shrunken ordinary event roster", &mutated);
    let mut mutated = bytes.clone();
    mutated[spans.event_count.clone()].copy_from_slice(&3_u32.to_le_bytes());
    rejected_unspecified("an over-counted ordinary event roster", &mutated);

    let port_write = &spans.events[0];
    let boundary_call = &spans.events[1];
    let mut mutated = bytes.clone();
    mutated[port_write.row_tag.clone()].copy_from_slice(&[9]);
    rejected(
        "an unknown ordinary event row tag",
        &mutated,
        TerminalTraceV1ProfileCodecError::InvalidRowTag {
            group: "ordinary event",
            tag: 9,
        },
    );
    for (name, span, label) in [
        (
            "a zero event machine",
            port_write.machine.clone(),
            "trace event machine",
        ),
        (
            "a zero event block",
            port_write.block.clone(),
            "trace event block",
        ),
        (
            "a zero event operation",
            port_write.operation.clone(),
            "trace event operation",
        ),
    ] {
        let mut mutated = bytes.clone();
        mutated[span].copy_from_slice(&0_u64.to_le_bytes());
        rejected(
            name,
            &mutated,
            TerminalTraceV1ProfileCodecError::Wire(CodecError::ZeroIdentity(label)),
        );
    }
    // The first event's coordinates are order-bound against the second row.
    let mut mutated = bytes.clone();
    mutated[port_write.operation.clone()].copy_from_slice(&2_u64.to_le_bytes());
    rejected(
        "an event operation colliding with its sibling",
        &mutated,
        TerminalTraceV1ProfileCodecError::NonCanonicalOrdinaryEventOrder,
    );
    let mut mutated = bytes.clone();
    mutated[port_write.operation.clone()].copy_from_slice(&0xFFFF_u64.to_le_bytes());
    rejected(
        "an event operation breaking roster order",
        &mutated,
        TerminalTraceV1ProfileCodecError::NonCanonicalOrdinaryEventOrder,
    );
    // The `EventOperationForeign`, `EventBlockForeign`, and
    // `EventMachineForeign` inventory legs exercise the last row's free
    // coordinates through the matrix.

    // The event kind tag is closed; a kind reinterpretation still decodes but
    // binds a different declaration, and each kind's target and identity are
    // bound.
    let mut mutated = bytes.clone();
    mutated[port_write.kind_tag.clone()].copy_from_slice(&[9]);
    rejected(
        "an unknown ordinary event kind",
        &mutated,
        TerminalTraceV1ProfileCodecError::InvalidOrdinaryEventKind(9),
    );
    // The `PortWriteRecastAsBoundaryCall`, `PortWriteRecastToBoundaryIdentity`,
    // and `BoundaryCallRecastAsPortWrite` inventory legs exercise the kind
    // reinterpretations through the matrix.
    let mut mutated = bytes.clone();
    mutated[port_write.kind_target.clone()].copy_from_slice(&0_u64.to_le_bytes());
    rejected(
        "a zero event service",
        &mutated,
        TerminalTraceV1ProfileCodecError::Wire(CodecError::ZeroIdentity("trace event service")),
    );
    // The `EventServiceForeign` and `EventBoundaryForeign` inventory legs
    // exercise the free kind targets through the matrix.
    let mut mutated = bytes.clone();
    mutated[port_write.identity_len.clone()].copy_from_slice(&u32::MAX.to_le_bytes());
    rejected_unspecified("an over-long event service identity", &mutated);
    let mut mutated = bytes.clone();
    mutated[port_write.identity.clone()].copy_from_slice(&[0xFF; 18]);
    rejected_unspecified("an invalid event service identity encoding", &mutated);
    // The `EventServiceIdentitySubstituted` and
    // `PortWriteServiceIdentityLookalike` inventory legs exercise the free
    // service identity through the matrix.

    // Port-write argument schemas: the synthesized (port, value) pair is bound
    // field by field.
    let mut mutated = bytes.clone();
    mutated[port_write.scalar_count.clone()].copy_from_slice(&1_u32.to_le_bytes());
    rejected_unspecified("a shrunken event scalar roster", &mutated);
    // The `EventScalarArgumentDropped` and `EventScalarArgumentInserted`
    // inventory legs exercise the argument roster shape through the matrix.
    let mut mutated = bytes.clone();
    mutated[port_write.scalars[0].comparison.clone()].copy_from_slice(&[9]);
    rejected(
        "an event scalar argument's comparison",
        &mutated,
        TerminalTraceV1ProfileCodecError::InvalidComparison(9),
    );
    let (carrier, _) = spans.events[0].scalars[0]
        .integer
        .clone()
        .expect("the port argument encodes an integer description");
    let mut mutated = bytes.clone();
    mutated[carrier].copy_from_slice(&[9]);
    rejected(
        "an event scalar argument's integer carrier",
        &mutated,
        TerminalTraceV1ProfileCodecError::Wire(CodecError::InvalidTag("IntegerSign", 9)),
    );
    // The `EventScalarIntegerWidthSubstituted` and
    // `EventScalarIntegerSignSubstituted` inventory legs exercise the port
    // argument's integer description through the matrix.

    // Boundary-call argument schemas: both scalar arguments and both
    // structural arguments are bound field by field.
    let mut mutated = bytes.clone();
    mutated[boundary_call.scalar_count.clone()].copy_from_slice(&1_u32.to_le_bytes());
    rejected_unspecified("a shrunken boundary-call scalar roster", &mutated);
    // The `BoundaryCallScalarArgumentDropped` and
    // `BoundaryCallScalarTypeSubstituted` inventory legs exercise the scalar
    // argument roster and each schema's type through the matrix.
    let mut mutated = bytes.clone();
    mutated[boundary_call.scalars[0].type_tag.clone()].copy_from_slice(&[9]);
    rejected(
        "a boundary-call scalar argument's type tag",
        &mutated,
        TerminalTraceV1ProfileCodecError::Wire(CodecError::InvalidTag("ScalarType", 9)),
    );

    let mut mutated = bytes.clone();
    mutated[boundary_call.structural_count.clone()].copy_from_slice(&1_u32.to_le_bytes());
    rejected_unspecified("a shrunken boundary-call structural roster", &mutated);
    // The `BoundaryCallStructuralArgumentDropped`,
    // `BoundaryCallStructuralTypeSubstituted`,
    // `BoundaryCallStructuralMultiplicitySubstituted`, and
    // `BoundaryCallStructuralAccessSubstituted` inventory legs exercise the
    // structural argument roster and each free schema field through the
    // matrix.
    let call_structural = &boundary_call.structurals[0];
    let mut mutated = bytes.clone();
    mutated[call_structural.multiplicity.clone()].copy_from_slice(&[9]);
    rejected(
        "a boundary-call structural argument's multiplicity",
        &mutated,
        TerminalTraceV1ProfileCodecError::InvalidStructuralMultiplicity(9),
    );
    let mut mutated = bytes.clone();
    mutated[call_structural.access.clone()].copy_from_slice(&[9]);
    rejected(
        "a boundary-call structural argument's access",
        &mutated,
        TerminalTraceV1ProfileCodecError::InvalidStructuralAccess(9),
    );
    // The first boundary structural argument declares a direct
    // qualification: the `BoundaryCallQualificationSubstituted` and
    // `BoundaryCallQualificationCleared` inventory legs exercise it through
    // the matrix.

    // Event result schemas: the port write observes Unit and the boundary call
    // observes a structural schema whose fields are bound.
    let mut mutated = bytes.clone();
    mutated[port_write.result.tag.clone()].copy_from_slice(&[9]);
    rejected(
        "an event result tag",
        &mutated,
        TerminalTraceV1ProfileCodecError::InvalidResultTag(9),
    );
    // The `PortWriteResultSubstituted`, `BoundaryCallResultUnit`,
    // `BoundaryCallResultTypeSubstituted`, `BoundaryCallResultAccessSubstituted`,
    // and `BoundaryCallResultMultiplicitySubstituted` inventory legs
    // exercise the event result schemas through the matrix.

    // Event roster shape: dropped, inserted, duplicated and reordered rows.
    // The `EventRowDropped` and `EventRowInserted` inventory legs exercise
    // the representable shapes through the matrix.
    let mut changed = profile.clone();
    changed.ordinary_events.swap(0, 1);
    encode_rejected(
        "a reordered ordinary event roster",
        &changed,
        TerminalTraceV1ProfileCodecError::NonCanonicalOrdinaryEventOrder,
    );
    let mut changed = profile.clone();
    changed
        .ordinary_events
        .insert(1, changed.ordinary_events[0].clone());
    encode_rejected(
        "a duplicated ordinary event row",
        &changed,
        TerminalTraceV1ProfileCodecError::NonCanonicalOrdinaryEventOrder,
    );

    // --- footer: the external-termination roster is fixed empty and the
    // profile ends exactly ---

    let mut mutated = bytes.clone();
    mutated[spans.external_count.clone()].copy_from_slice(&1_u32.to_le_bytes());
    rejected(
        "a populated external termination roster",
        &mutated,
        TerminalTraceV1ProfileCodecError::UnsupportedExternalTerminationRows(1),
    );

    let mut trailing = bytes.clone();
    trailing.push(0);
    rejected(
        "a trailing byte",
        &trailing,
        TerminalTraceV1ProfileCodecError::TrailingBytes(1),
    );

    for cut in [
        spans.domain.end - 1,
        spans.version.end - 1,
        spans.vocabulary.end - 1,
        spans.commitment.end - 1,
        spans.root_count.end - 1,
        spans.root.entry.end - 1,
        spans.crashes[0].row.end - 1,
        spans.crashes[1].row.end - 1,
        spans.boundary_crashes[0].row.end - 1,
        spans.boundary_crashes[1].row.end - 1,
        spans.events[0].row.end - 1,
        spans.events[1].row.end - 1,
        spans.external_count.end - 1,
        spans.end - 1,
    ] {
        rejected_unspecified("a truncated profile", &bytes[..cut]);
    }
}

fn domain(profile: &TerminalTraceV1Profile) -> StructuralDomainId {
    profile.root.structural_inputs[0].qualifications[0]
}
