//! One-field mutation coverage for the semantic module's proof-only
//! float-meaning projection roster and the equality propositions joined to it.
//!
//! `float_meaning_projections` is the module-level custody record of each
//! proof-only `FloatMeaning` projection: the dense proof-value result
//! declaration, the reconstructible source, the catalog operation, and the
//! closed contract identity derived from that operation. The fixture carries
//! every source variant: dense-by-first-use transitional inputs, exact
//! binary32 and binary64 literals, one valid carrier for each
//! artifact-relative direct source — a machine parameter, a machine scalar
//! result, a non-call operation result, a call-operation result, a block
//! parameter, and a structural leaf — each rejoined by the module-bound
//! validation to the owner machine's declared tables — and a semantic
//! application spelling one FloatSemantics kernel application through the
//! catalog contract identity and its operand roster, discharged by the
//! module-bound validation when every meaning operand is a literal. `float_meaning_equalities`
//! is the joined proposition roster: dense proposition identities whose
//! operands name projection results by index and whose reconstructed carriers
//! must agree on source format, operation, and contract. Their wire fields —
//! both roster counts, each row's result id, value-type tag, source tag and
//! payload, operation tag, the five contract axes, and each equality's id and
//! operand indices — are each substituted independently. A substitution either
//! fails canonical decoding (an unknown tag, a zero direct-source identity, a
//! broken dense index or first-use source order, a reordered or duplicated
//! roster) or the module-bound validation (a duplicated projection key, an
//! inconsistent transitional source format, a contract, format, or
//! equality-carrier mismatch, an unknown equality operand, or a direct-source
//! rejoin), or it decodes to a different module whose honestly recomputed
//! semantic and artifact identities diverge and whose replay against the
//! retained manifest and sealed proof subject rejects.
//!
//! The fixture carries two transitional binary32 inputs, an exact binary32
//! literal, a direct machine parameter on the shared machine, one transitional
//! binary64 input, and an exact binary64 literal, then one row per remaining
//! direct-source variant bound to a second float-result machine and the shared
//! machine's float operations, a semantic application of `add` over two
//! literal operands, and an exact binary32 literal equal to its discharged
//! result. Four equalities join the binary32 rows, the last joining the
//! application result to the equal literal; the binary64 rows stay
//! unreferenced so operand retargets exercise the carrier-mismatch join,
//! while retargets onto the direct carriers stay representable.

use std::ops::Range;

use super::{
    block_id, canonical_artifact, contract_id, edge_id, kernel_bundle, machine_id, operation_id,
    place_id, semantic_module, structural_field_id, structural_type_id, value_id,
};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IeeeFloatFormat, IeeeFloatStructuralField, IeeeFloatValue,
    PropositionError, ScalarType, StructuralPlaceKind,
};
use terminal_codec::{
    ArtifactManifestError, CodecError, ProofCodecError, build_artifact_manifest,
    build_identity_optimization_execution_record, decode_module, decode_proof_section_for,
    encode_module, terminal_psi_identity, validate_artifact_manifest,
};
use terminal_psi::{
    BindingRelevance, Block, DirectBlockFloatParameter, DirectCallFloatResult,
    DirectMachineFloatParameter, DirectMachineFloatResult, DirectOperationFloatResult,
    DirectStructuralFloatLeaf, FloatMeaningEqualityProposition, FloatMeaningProjection,
    FloatMeaningProjectionOperation, FloatMeaningSource, FloatProjectionInput,
    FloatProjectionInputId, FloatSemanticApplication, FloatSemanticApplicationOperand,
    MachineContract, Operation, OperationKind, OperationResult, ProofOnlyValueType,
    ProofPropositionId, ProofValueDeclaration, ProofValueId, StructuralAccess,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalMachine, TerminalMachineResult, Terminator, ValueDeclaration,
};
use terminal_verifier::{FloatMeaningProjectionVerificationError, ModuleError, verify_module};

/// Byte offsets of every wire field inside the two float-meaning rosters.
/// Optional spans record the payload coordinates of the source variant the
/// fixture row actually carries, so a substitution can name the transitional
/// input id, the transitional or direct format byte, the literal bits, a
/// direct source's owner, producer/block, and value identities, or a
/// structural leaf's root place, path count, and field segment.
struct ModuleSpans {
    projection_count: Range<usize>,
    projections: Vec<ProjectionSpan>,
    equality_count: Range<usize>,
    equalities: Vec<EqualitySpan>,
}

struct ProjectionSpan {
    row: Range<usize>,
    result_id: Range<usize>,
    value_type: Range<usize>,
    source_tag: Range<usize>,
    source: Range<usize>,
    source_id: Option<Range<usize>>,
    source_format: Option<Range<usize>>,
    source_bits: Option<Range<usize>>,
    owner: Option<Range<usize>>,
    producer: Option<Range<usize>>,
    coordinate: Option<Range<usize>>,
    field_root: Option<Range<usize>>,
    field_path_count: Option<Range<usize>>,
    field_segment: Option<Range<usize>>,
    application_row: Option<Range<usize>>,
    application_version: Option<Range<usize>>,
    application_commitment: Option<Range<usize>>,
    application_operand_tag: Option<Range<usize>>,
    application_operand_format: Option<Range<usize>>,
    application_operand_id: Option<Range<usize>>,
    operation: Range<usize>,
    contract_format: Range<usize>,
    contract_operation: Range<usize>,
    contract_declaration: Range<usize>,
    contract_version: Range<usize>,
    contract_commitment: Range<usize>,
}

struct EqualitySpan {
    row: Range<usize>,
    id: Range<usize>,
    left: Range<usize>,
    right: Range<usize>,
}

/// A cursor that walks the canonical module encoding exactly as the decoder
/// does, recording the byte span of every float-meaning field the matrix
/// substitutes. Every section ahead of the projection roster is empty in the
/// fixture except the one structural type declaration, so the walk asserts
/// each remaining leading count is zero rather than silently spanning
/// unknown row bytes.
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

    fn walk_projection(&mut self) -> ProjectionSpan {
        let row_start = self.offset;
        let result_id = self.take(4);
        let value_type = self.take(1);
        let source_start = self.offset;
        let source_tag = self.take(1);
        let mut source_id = None;
        let mut source_format = None;
        let mut source_bits = None;
        let mut owner = None;
        let mut producer = None;
        let mut coordinate = None;
        let mut field_root = None;
        let mut field_path_count = None;
        let mut field_segment = None;
        let mut application_row = None;
        let mut application_version = None;
        let mut application_commitment = None;
        let mut application_operand_tag = None;
        let mut application_operand_format = None;
        let mut application_operand_id = None;
        match self.bytes[source_tag.clone()][0] {
            // TransitionalInput: u32 first-use id and a u8 IEEE format.
            1 => {
                source_id = Some(self.take(4));
                source_format = Some(self.take(1));
            }
            // ExactBinary32Literal / ExactBinary64Literal: raw landed bits.
            2 => source_bits = Some(self.take(4)),
            3 => source_bits = Some(self.take(8)),
            // DirectMachineParameter / DirectMachineResult: owner and value.
            4 | 5 => {
                owner = Some(self.take(8));
                coordinate = Some(self.take(8));
                source_format = Some(self.take(1));
            }
            // DirectOperationResult / DirectBlockParameter / DirectCallResult:
            // owner, producer or block, and value identities plus the format.
            6..=8 => {
                owner = Some(self.take(8));
                producer = Some(self.take(8));
                coordinate = Some(self.take(8));
                source_format = Some(self.take(1));
            }
            // DirectStructuralLeaf: owner plus a canonical structural field —
            // root place, counted path segments, then the format.
            9 => {
                owner = Some(self.take(8));
                field_root = Some(self.take(8));
                let (count, segments) = self.take_count();
                field_path_count = Some(count);
                for _ in 0..segments {
                    let tag = self.take(1);
                    let segment = self.take(8);
                    if field_segment.is_none() && self.bytes[tag][0] == 1 {
                        field_segment = Some(segment);
                    }
                }
                source_format = Some(self.take(1));
            }
            // SemanticApplication: catalog row ordinal, contract version and
            // commitment, the declared format, then a counted operand roster
            // of tagged IEEE-format / proof-value entries.
            10 => {
                application_row = Some(self.take(1));
                application_version = Some(self.take(2));
                application_commitment = Some(self.take(32));
                source_format = Some(self.take(1));
                let (_, operands) = self.take_count();
                for _ in 0..operands {
                    let tag = self.take(1);
                    if application_operand_tag.is_none() {
                        application_operand_tag = Some(tag.clone());
                    }
                    match self.bytes[tag][0] {
                        1 => {
                            let format = self.take(1);
                            if application_operand_format.is_none() {
                                application_operand_format = Some(format);
                            }
                        }
                        2 => {
                            let id = self.take(4);
                            if application_operand_id.is_none() {
                                application_operand_id = Some(id);
                            }
                        }
                        tag => {
                            panic!("the fixture carries no application operand tag {tag}")
                        }
                    }
                }
            }
            tag => panic!("the fixture carries no float-meaning source tag {tag}"),
        }
        let operation = self.take(1);
        let contract_format = self.take(2);
        let contract_operation = self.take(1);
        let contract_declaration = self.take(1);
        let contract_version = self.take(2);
        let contract_commitment = self.take(32);
        ProjectionSpan {
            row: row_start..self.offset,
            result_id,
            value_type,
            source_tag,
            source: source_start..operation.start,
            source_id,
            source_format,
            source_bits,
            owner,
            producer,
            coordinate,
            field_root,
            field_path_count,
            field_segment,
            application_row,
            application_version,
            application_commitment,
            application_operand_tag,
            application_operand_format,
            application_operand_id,
            operation,
            contract_format,
            contract_operation,
            contract_declaration,
            contract_version,
            contract_commitment,
        }
    }

    /// Skip one structural-type declaration row, mirroring
    /// `encode_structural_type`. The fixture's only declaration is a record of
    /// one IEEE float field, so other shapes and field types panic rather than
    /// silently spanning unknown row bytes.
    fn walk_structural_type(&mut self) {
        self.take(8); // declaration identity
        let (_, identity) = self.take_count();
        self.take(identity as usize); // identity text
        match self.bytes[self.take(1)][0] {
            // Record: a counted roster of field declarations.
            1 => {
                let (_, fields) = self.take_count();
                for _ in 0..fields {
                    self.take(8); // field identity
                    let (_, name) = self.take_count();
                    self.take(name as usize); // field identity text
                    self.take(1); // binding relevance
                    match self.bytes[self.take(1)][0] {
                        // IeeeFloat: one format byte.
                        4 => {
                            self.take(1);
                        }
                        tag => panic!("the fixture carries no structural field type tag {tag}"),
                    }
                }
            }
            tag => panic!("the fixture carries no structural type shape tag {tag}"),
        }
    }

    fn walk_equality(&mut self) -> EqualitySpan {
        let row_start = self.offset;
        let id = self.take(4);
        let left = self.take(4);
        let right = self.take(4);
        EqualitySpan {
            row: row_start..self.offset,
            id,
            left,
            right,
        }
    }
}

/// Locate the float-meaning rosters inside canonical module bytes by
/// mirroring `encode_raw`'s ordered section list. The fixture module keeps
/// every earlier roster empty except the one structural type declaration the
/// structural-leaf carrier needs, and the walk stops at the end of the
/// equality roster.
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
    // The fixture declares one structural type for the structural-leaf
    // carrier; the remaining leading rosters stay empty.
    let (_, structural_types) = walker.take_count();
    for _ in 0..structural_types {
        walker.walk_structural_type();
    }
    for label in [
        "structural domains",
        "services",
        "concrete root service reach",
        "installation reach dependencies",
        "placed-view inputs",
        "reborrow root handoffs",
        "reborrow restored call uses",
        "boundary machines",
        "provider candidates",
    ] {
        walker.expect_empty_count(label);
    }
    let (projection_count, projections) = walker.take_count();
    let mut projection_spans =
        Vec::with_capacity(usize::try_from(projections).expect("projection count fits usize"));
    for _ in 0..projections {
        projection_spans.push(walker.walk_projection());
    }
    let (equality_count, equalities) = walker.take_count();
    let mut equality_spans =
        Vec::with_capacity(usize::try_from(equalities).expect("equality count fits usize"));
    for _ in 0..equalities {
        equality_spans.push(walker.walk_equality());
    }
    ModuleSpans {
        projection_count,
        projections: projection_spans,
        equality_count,
        equalities: equality_spans,
    }
}

/// The closed contract identity the verifier reconstructs for one catalog
/// operation: any substitution of a single contract axis strands the join.
fn contract(
    operation: numerics::float_projection::FloatProjectionOperation,
) -> terminal_psi::FloatProjectionContractIdentity {
    let contract = operation.contract_identity();
    terminal_psi::FloatProjectionContractIdentity {
        format: contract.format,
        operation: contract.operation,
        declaration: contract.declaration,
        catalog_version: contract.catalog_version,
        commitment: contract.commitment,
    }
}

fn transitional(index: u32, source: u32, format: IeeeFloatFormat) -> FloatMeaningProjection {
    let operation = match format {
        IeeeFloatFormat::Binary32 => FloatMeaningProjectionOperation::Meaning32,
        IeeeFloatFormat::Binary64 => FloatMeaningProjectionOperation::Meaning64,
    };
    FloatMeaningProjection {
        result: ProofValueDeclaration {
            id: ProofValueId(index),
            value_type: ProofOnlyValueType::FloatMeaning,
        },
        source: FloatMeaningSource::TransitionalInput(FloatProjectionInput {
            id: FloatProjectionInputId(source),
            format,
        }),
        operation,
        contract: contract(match operation {
            FloatMeaningProjectionOperation::Meaning32 => {
                numerics::float_projection::FloatProjectionOperation::Meaning32
            }
            FloatMeaningProjectionOperation::Meaning64 => {
                numerics::float_projection::FloatProjectionOperation::Meaning64
            }
        }),
    }
}

fn literal32(index: u32, bits: u32) -> FloatMeaningProjection {
    FloatMeaningProjection {
        result: ProofValueDeclaration {
            id: ProofValueId(index),
            value_type: ProofOnlyValueType::FloatMeaning,
        },
        source: FloatMeaningSource::ExactBinary32Literal(bits),
        operation: FloatMeaningProjectionOperation::Meaning32,
        contract: contract(numerics::float_projection::FloatProjectionOperation::Meaning32),
    }
}

fn literal64(index: u32, bits: u64) -> FloatMeaningProjection {
    FloatMeaningProjection {
        result: ProofValueDeclaration {
            id: ProofValueId(index),
            value_type: ProofOnlyValueType::FloatMeaning,
        },
        source: FloatMeaningSource::ExactBinary64Literal(bits),
        operation: FloatMeaningProjectionOperation::Meaning64,
        contract: contract(numerics::float_projection::FloatProjectionOperation::Meaning64),
    }
}

/// The closed contract identity of one FloatSemantics catalog row: the
/// module-bound validation rejoins the ordinal, version, and commitment to
/// exactly this row, so any substitution strands the application.
fn semantic_application_contract(
    name: &'static str,
    parameters: &[numerics::float_semantics_catalog::FloatSemanticValueKind],
    result: numerics::float_semantics_catalog::FloatSemanticValueKind,
) -> terminal_psi::FloatSemanticContractIdentity {
    let contract = numerics::float_semantics_catalog::FloatSemanticOperation::from_source_identity(
        numerics::float_semantics_catalog::FLOAT_SEMANTICS_NAMESPACE,
        name,
        parameters,
        result,
    )
    .expect("catalog row")
    .contract_identity();
    terminal_psi::FloatSemanticContractIdentity {
        row: contract.row,
        catalog_version: contract.catalog_version,
        commitment: contract.commitment,
    }
}

fn application32(
    index: u32,
    identity: terminal_psi::FloatSemanticContractIdentity,
    operands: Vec<FloatSemanticApplicationOperand>,
) -> FloatMeaningProjection {
    meaning32(
        index,
        FloatMeaningSource::SemanticApplication(FloatSemanticApplication {
            contract: identity,
            format: IeeeFloatFormat::Binary32,
            operands,
        }),
    )
}

fn equality(index: u32, left: u32, right: u32) -> FloatMeaningEqualityProposition {
    FloatMeaningEqualityProposition {
        id: ProofPropositionId(index),
        left: ProofValueId(left),
        right: ProofValueId(right),
    }
}

fn float_scalar(id: u64) -> ValueDeclaration {
    ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(id),
        scalar_type: ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
    }
}

fn float32_constant(id: u64, result: u64, bits: u32) -> Operation {
    Operation {
        static_reach_binding: None,
        id: operation_id(id),
        result: OperationResult::Scalar(float_scalar(result)),
        kind: OperationKind::IeeeFloatConstant {
            value: IeeeFloatValue::Binary32(bits),
        },
    }
}

fn meaning32(index: u32, source: FloatMeaningSource) -> FloatMeaningProjection {
    FloatMeaningProjection {
        result: ProofValueDeclaration {
            id: ProofValueId(index),
            value_type: ProofOnlyValueType::FloatMeaning,
        },
        source,
        operation: FloatMeaningProjectionOperation::Meaning32,
        contract: contract(numerics::float_projection::FloatProjectionOperation::Meaning32),
    }
}

/// The shared semantic module plus one IEEE binary32 machine parameter and a
/// structural parameter carrying a binary32 leaf for the direct machine-
/// parameter and structural-leaf sources, a float-constant operation and a
/// call for the operation- and call-result sources, and a second float-result
/// machine owning a jumped-to block parameter for the machine-result and
/// block-parameter sources. Eleven dense projections cover every source
/// variant and three equalities join the binary32 carriers.
fn float_meaning_module() -> terminal_psi::TerminalModule {
    let mut module = semantic_module();
    // The structural-leaf source resolves one record field declared binary32.
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(1),
        identity: "example::FloatBox".to_owned(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: structural_field_id(1),
                identity: "payload".to_owned(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary32),
            }],
        },
    });
    let machine = &mut module.machines[0];
    machine.parameters.push(float_scalar(6));
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(1),
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    });
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: place_id(1),
            position: 0,
            is_self: false,
            structural_type: structural_type_id(1),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine.blocks[0].operations.extend([
        float32_constant(2, 11, 0x3f80_0000),
        Operation {
            static_reach_binding: None,
            id: operation_id(3),
            result: OperationResult::Scalar(float_scalar(12)),
            kind: OperationKind::Call {
                erased_arguments: Vec::new(),
                callee: machine_id(2),
                arguments: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        },
    ]);
    // A float-result callee: the entry block jumps its literal product into a
    // second block's binary32 parameter, which returns it as the scalar result.
    module.machines.push(TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine_id(2),
        attachment: None,
        structural_parameters: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(float_scalar(8)),
        structural_places: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(2),
        blocks: vec![
            Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: block_id(2),
                parameters: Vec::new(),
                operations: vec![float32_constant(4, 9, 0x4000_0000)],
                terminator: Terminator::Jump {
                    edge: edge_id(2),
                    target: block_id(3),
                    arguments: vec![value_id(9)],
                    erased_arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                    residual_affine_discards: Vec::new(),
                },
            },
            Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: block_id(3),
                parameters: vec![float_scalar(13)],
                operations: Vec::new(),
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: edge_id(3),
                    value: value_id(13),
                },
            },
        ],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            id: contract_id(2),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    });
    module.float_meaning_projections = vec![
        transitional(0, 0, IeeeFloatFormat::Binary32),
        transitional(1, 1, IeeeFloatFormat::Binary32),
        literal32(2, 0x3f80_0000),
        meaning32(
            3,
            FloatMeaningSource::DirectMachineParameter(DirectMachineFloatParameter {
                owner: machine_id(1),
                parameter: value_id(6),
                format: IeeeFloatFormat::Binary32,
            }),
        ),
        transitional(4, 2, IeeeFloatFormat::Binary64),
        literal64(5, 0x3ff0_0000_0000_0000),
        meaning32(
            6,
            FloatMeaningSource::DirectMachineResult(DirectMachineFloatResult {
                owner: machine_id(2),
                result: value_id(8),
                format: IeeeFloatFormat::Binary32,
            }),
        ),
        meaning32(
            7,
            FloatMeaningSource::DirectOperationResult(DirectOperationFloatResult {
                owner: machine_id(1),
                producer: operation_id(2),
                result: value_id(11),
                format: IeeeFloatFormat::Binary32,
            }),
        ),
        meaning32(
            8,
            FloatMeaningSource::DirectCallResult(DirectCallFloatResult {
                owner: machine_id(1),
                producer: operation_id(3),
                result: value_id(12),
                format: IeeeFloatFormat::Binary32,
            }),
        ),
        meaning32(
            9,
            FloatMeaningSource::DirectBlockParameter(DirectBlockFloatParameter {
                owner: machine_id(2),
                block: block_id(3),
                parameter: value_id(13),
                format: IeeeFloatFormat::Binary32,
            }),
        ),
        meaning32(
            10,
            FloatMeaningSource::DirectStructuralLeaf(DirectStructuralFloatLeaf {
                owner: machine_id(1),
                field: IeeeFloatStructuralField::new(
                    place_id(1),
                    vec![CanonicalStructuralPathSegment::Field(structural_field_id(
                        1,
                    ))],
                )
                .expect("a nonempty structural leaf path"),
                format: IeeeFloatFormat::Binary32,
            }),
        ),
        application32(
            11,
            semantic_application_contract(
                "add",
                &[
                    numerics::float_semantics_catalog::FloatSemanticValueKind::Format,
                    numerics::float_semantics_catalog::FloatSemanticValueKind::Meaning,
                    numerics::float_semantics_catalog::FloatSemanticValueKind::Meaning,
                ],
                numerics::float_semantics_catalog::FloatSemanticValueKind::Meaning,
            ),
            vec![
                FloatSemanticApplicationOperand::Format(IeeeFloatFormat::Binary32),
                FloatSemanticApplicationOperand::Meaning(ProofValueId(2)),
                FloatSemanticApplicationOperand::Meaning(ProofValueId(2)),
            ],
        ),
        literal32(12, 0x4000_0000),
    ];
    module.float_meaning_equalities = vec![
        equality(0, 0, 1),
        equality(1, 0, 2),
        equality(2, 1, 3),
        equality(3, 11, 12),
    ];
    module
}

#[test]
fn terminal_float_meaning_rosters_reject_every_one_field_substitution() {
    let module = float_meaning_module();
    let bundle = kernel_bundle();
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
        spans.projections.len(),
        13,
        "fixture roster: transitional, literal, one row per direct source variant, \
         and one semantic application with its discharged literal"
    );
    assert_eq!(spans.equalities.len(), 4, "fixture equality roster");

    // The retained artifact binds the semantic identity into its manifest and
    // the sealed proof section to this exact module; replaying either against
    // a substituted module is the independent-replay leg for every
    // representable field.
    let artifact = canonical_artifact(&module, &bundle, None);
    let retained = artifact.manifest();
    let semantic_identity = terminal_psi_identity(&module).expect("semantic identity");
    assert_eq!(retained.semantic(), semantic_identity);

    // A substitution that still forms a canonical module honestly recomputes
    // a divergent semantic and artifact identity: the substituted module
    // still verifies under the retained bundle (the rosters carry no proof
    // obligations), while the retained custody replays — the manifest join
    // and the sealed proof subject join — reject it.
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
    // canonical encoder's own order and semantic validation.
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
    let put_u16 = |range: Range<usize>, value: u16| -> Vec<u8> {
        let mut mutated = encoded.clone();
        mutated[range].copy_from_slice(&value.to_le_bytes());
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
    // Splice replacement bytes over one wire span, keeping the surrounding
    // sections honest; used to retarget a source across payload widths.
    let splice = |range: Range<usize>, replacement: &[u8]| -> Vec<u8> {
        let mut mutated = encoded[..range.start].to_vec();
        mutated.extend_from_slice(replacement);
        mutated.extend_from_slice(&encoded[range.end..]);
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

    let projections = &spans.projections;
    let equalities = &spans.equalities;
    let invalid_projection = |index: u32, error: FloatMeaningProjectionVerificationError| {
        CodecError::InvalidModule(ModuleError::InvalidFloatMeaningProjection { index, error })
    };
    let dense_projection_order = || {
        CodecError::NonCanonicalOrder(
            "float-meaning projections by dense proof value and first-use source IDs",
        )
    };
    let transitional_source_order = || {
        CodecError::NonCanonicalOrder(
            "float-meaning projections by dense proof value and first-use transitional source IDs",
        )
    };
    let dense_equality_order = || {
        CodecError::NonCanonicalOrder(
            "float-meaning equalities by dense proposition ID and ordered operands",
        )
    };

    // --- projection roster axes --------------------------------------------

    // A roster count lying about its rows reads the equality roster's bytes
    // as a phantom row or starves the walk entirely.
    rejected(
        "a float-projection roster count one over",
        &put_u32(spans.projection_count.clone(), 14),
        CodecError::InvalidTag("ProofOnlyValueType", 0),
    );
    rejected(
        "a maximal float-projection roster count",
        &put_u32(spans.projection_count.clone(), u32::MAX),
        CodecError::UnexpectedEnd,
    );
    // Clearing both rosters stays representable: the recomputed identity
    // diverges and the retained custody replays reject.
    let mut cleared = encoded[..spans.projection_count.start].to_vec();
    cleared.extend_from_slice(&0_u32.to_le_bytes());
    cleared.extend_from_slice(&0_u32.to_le_bytes());
    cleared.extend_from_slice(&encoded[equalities[3].row.end..]);
    divergent("a cleared float-meaning section", &cleared);
    // Clearing only the projection roster strands the retained equality
    // operands at the module-bound join.
    let mut stranded = encoded[..spans.projection_count.start].to_vec();
    stranded.extend_from_slice(&0_u32.to_le_bytes());
    stranded.extend_from_slice(&encoded[spans.equality_count.start..]);
    rejected(
        "a cleared float-projection roster under retained equalities",
        &stranded,
        CodecError::InvalidModule(ModuleError::UnknownFloatMeaningEqualityOperand {
            proposition: 0,
            operand: 0,
        }),
    );
    // The last row is bound to the trailing equality: dropping it keeps the
    // roster dense but strands the equality operand. Dropping or duplicating
    // any earlier row strands the dense proof-value index of every following
    // row.
    rejected(
        "a dropped trailing float-projection row",
        &drop_row(spans.projection_count.clone(), projections[12].row.clone()),
        CodecError::InvalidModule(ModuleError::UnknownFloatMeaningEqualityOperand {
            proposition: 3,
            operand: 12,
        }),
    );
    rejected(
        "a dropped first float-projection row",
        &drop_row(spans.projection_count.clone(), projections[0].row.clone()),
        dense_projection_order(),
    );
    rejected(
        "a dropped interior float-projection row",
        &drop_row(spans.projection_count.clone(), projections[4].row.clone()),
        dense_projection_order(),
    );
    rejected(
        "a duplicated float-projection row",
        &duplicate_row(spans.projection_count.clone(), projections[0].row.clone()),
        dense_projection_order(),
    );
    let mut swapped = encoded[..projections[0].row.start].to_vec();
    swapped.extend_from_slice(&encoded[projections[1].row.clone()]);
    swapped.extend_from_slice(&encoded[projections[0].row.clone()]);
    swapped.extend_from_slice(&encoded[projections[1].row.end..]);
    rejected(
        "a reordered float-projection roster",
        &swapped,
        dense_projection_order(),
    );

    // --- result declaration --------------------------------------------------

    // The result identity is the dense roster index; the proof-only value
    // type admits exactly one tag.
    rejected(
        "a substituted float-projection result id",
        &put_u32(projections[0].result_id.clone(), 7),
        dense_projection_order(),
    );
    for tag in [0, 2, u8::MAX] {
        rejected(
            "an unknown proof-only value-type tag",
            &put_u8(projections[0].value_type.clone(), tag),
            CodecError::InvalidTag("ProofOnlyValueType", tag),
        );
    }

    // --- source variants ----------------------------------------------------

    // The source tag names one of the closed payload shapes; a same-format
    // retarget on the last transitional row stays representable.
    for tag in [0, 11, u8::MAX] {
        rejected(
            "an unknown float-meaning source tag",
            &put_u8(projections[0].source_tag.clone(), tag),
            CodecError::InvalidTag("FloatMeaningSource", tag),
        );
    }
    divergent(
        "a transitional source retargeted to an exact binary64 literal",
        &splice(
            projections[4].source.clone(),
            &[&[3_u8], &0x4000_0000_0000_0000_u64.to_le_bytes()[..]].concat(),
        ),
    );
    // Retargeting the first transitional user strands the first-use source
    // density of every later transitional row.
    rejected(
        "a leading transitional source retargeted to a literal",
        &splice(
            projections[0].source.clone(),
            &[&[2_u8], &0x3f80_0000_u32.to_le_bytes()[..]].concat(),
        ),
        transitional_source_order(),
    );

    // Transitional inputs are dense by first use: an unseen id strands the
    // coordinate, reusing an earlier input collapses the distinct-source
    // sequence so the last first use lands off its dense index, a reused
    // different-format id strands the recorded source format, and an exact
    // literal colliding with a later row's key rejects at the module join.
    rejected(
        "a non-dense transitional float source",
        &put_u32(projections[0].source_id.clone().unwrap(), 9),
        transitional_source_order(),
    );
    rejected(
        "a transitional source reusing an earlier input",
        &put_u32(projections[1].source_id.clone().unwrap(), 0),
        transitional_source_order(),
    );
    rejected(
        "a transitional source reusing an input under another format",
        &put_u32(projections[4].source_id.clone().unwrap(), 0),
        CodecError::InvalidModule(
            ModuleError::InconsistentFloatMeaningProjectionSourceFormat { source: 0 },
        ),
    );
    rejected(
        "a transitional source retargeted onto a later literal's key",
        &splice(
            projections[4].source.clone(),
            &[&[3_u8], &0x3ff0_0000_0000_0000_u64.to_le_bytes()[..]].concat(),
        ),
        CodecError::InvalidModule(ModuleError::DuplicateFloatMeaningProjection {
            first: 4,
            duplicate: 5,
        }),
    );
    for tag in [0, 3, u8::MAX] {
        rejected(
            "an unknown transitional float format tag",
            &put_u8(projections[0].source_format.clone().unwrap(), tag),
            CodecError::InvalidTag("IeeeFloatFormat", tag),
        );
    }
    rejected(
        "a binary32 source widened under a meaning32 projection",
        &put_u8(projections[0].source_format.clone().unwrap(), 2),
        invalid_projection(
            0,
            FloatMeaningProjectionVerificationError::SourceFormatMismatch,
        ),
    );
    rejected(
        "a binary64 source narrowed under a meaning64 projection",
        &put_u8(projections[4].source_format.clone().unwrap(), 1),
        invalid_projection(
            4,
            FloatMeaningProjectionVerificationError::SourceFormatMismatch,
        ),
    );

    // Exact literal payloads are the freely representable axis: any bit
    // pattern, including a NaN payload the projection law erases, reidentifies
    // the source and diverges.
    divergent(
        "a substituted exact binary32 literal",
        &put_u32(projections[2].source_bits.clone().unwrap(), 0xbf80_0000),
    );
    divergent(
        "an exact binary32 literal reidentified as NaN payload",
        &put_u32(projections[2].source_bits.clone().unwrap(), 0x7fc0_0001),
    );
    divergent(
        "a substituted exact binary64 literal",
        &put_u64(
            projections[5].source_bits.clone().unwrap(),
            0x4000_0000_0000_0000,
        ),
    );

    // --- direct parameter source ---------------------------------------------

    // The direct parameter rejoins all three coordinates to the owner's
    // declared scalar table at the exact IEEE format.
    rejected(
        "a zero direct-parameter owner",
        &put_u64(projections[3].owner.clone().unwrap(), 0),
        CodecError::ZeroIdentity("float-meaning direct parameter owner"),
    );
    rejected(
        "a direct-parameter owner outside the module",
        &put_u64(projections[3].owner.clone().unwrap(), 9),
        invalid_projection(
            3,
            FloatMeaningProjectionVerificationError::InvalidDirectParameterOwner(machine_id(9)),
        ),
    );
    rejected(
        "a zero direct-parameter value",
        &put_u64(projections[3].coordinate.clone().unwrap(), 0),
        CodecError::ZeroIdentity("float-meaning direct parameter value"),
    );
    rejected(
        "a direct parameter outside the owner's table",
        &put_u64(projections[3].coordinate.clone().unwrap(), 9),
        invalid_projection(
            3,
            FloatMeaningProjectionVerificationError::InvalidDirectParameter {
                owner: machine_id(1),
                parameter: value_id(9),
            },
        ),
    );
    // A format flip strands the catalog source-format join before the direct
    // rejoin ever inspects the owner's declaration.
    rejected(
        "a direct parameter under another IEEE format",
        &put_u8(projections[3].source_format.clone().unwrap(), 2),
        invalid_projection(
            3,
            FloatMeaningProjectionVerificationError::SourceFormatMismatch,
        ),
    );

    // --- direct machine result source -----------------------------------------

    // The direct machine result rejoins owner and result to the owner's
    // declared scalar result declaration at the exact IEEE format. The shared
    // machine's integer result and the callee's operation result are both
    // reachable identities that are not this coordinate.
    rejected(
        "a zero direct-result owner",
        &put_u64(projections[6].owner.clone().unwrap(), 0),
        CodecError::ZeroIdentity("float-meaning direct result owner"),
    );
    rejected(
        "a direct-result owner outside the module",
        &put_u64(projections[6].owner.clone().unwrap(), 9),
        invalid_projection(
            6,
            FloatMeaningProjectionVerificationError::InvalidDirectResultOwner(machine_id(9)),
        ),
    );
    rejected(
        "a direct result rehomed to the non-float owner",
        &put_u64(projections[6].owner.clone().unwrap(), 1),
        invalid_projection(
            6,
            FloatMeaningProjectionVerificationError::InvalidDirectResult {
                owner: machine_id(1),
                result: value_id(8),
            },
        ),
    );
    rejected(
        "a zero direct-result value",
        &put_u64(projections[6].coordinate.clone().unwrap(), 0),
        CodecError::ZeroIdentity("float-meaning direct result value"),
    );
    rejected(
        "a direct result outside the owner's declaration",
        &put_u64(projections[6].coordinate.clone().unwrap(), 9),
        invalid_projection(
            6,
            FloatMeaningProjectionVerificationError::InvalidDirectResult {
                owner: machine_id(2),
                result: value_id(9),
            },
        ),
    );
    rejected(
        "a direct result under another IEEE format",
        &put_u8(projections[6].source_format.clone().unwrap(), 2),
        invalid_projection(
            6,
            FloatMeaningProjectionVerificationError::SourceFormatMismatch,
        ),
    );

    // --- direct operation result source ---------------------------------------

    // The direct operation result rejoins owner, producer, and result to one
    // non-call operation's declared scalar result in the owner's blocks.
    rejected(
        "a zero direct-operation-result owner",
        &put_u64(projections[7].owner.clone().unwrap(), 0),
        CodecError::ZeroIdentity("float-meaning direct operation-result owner"),
    );
    rejected(
        "a direct-operation-result owner outside the module",
        &put_u64(projections[7].owner.clone().unwrap(), 9),
        invalid_projection(
            7,
            FloatMeaningProjectionVerificationError::InvalidDirectOperationResultOwner(machine_id(
                9,
            )),
        ),
    );
    rejected(
        "a direct operation result rehomed to the callee",
        &put_u64(projections[7].owner.clone().unwrap(), 2),
        invalid_projection(
            7,
            FloatMeaningProjectionVerificationError::InvalidDirectOperationResultProducer {
                owner: machine_id(2),
                producer: operation_id(2),
            },
        ),
    );
    rejected(
        "a zero direct-operation-result producer",
        &put_u64(projections[7].producer.clone().unwrap(), 0),
        CodecError::ZeroIdentity("float-meaning direct operation-result producer"),
    );
    rejected(
        "a direct operation result on a call producer",
        &put_u64(projections[7].producer.clone().unwrap(), 3),
        invalid_projection(
            7,
            FloatMeaningProjectionVerificationError::DirectOperationResultCallProducer {
                owner: machine_id(1),
                producer: operation_id(3),
            },
        ),
    );
    rejected(
        "a direct operation result on an unknown producer",
        &put_u64(projections[7].producer.clone().unwrap(), 9),
        invalid_projection(
            7,
            FloatMeaningProjectionVerificationError::InvalidDirectOperationResultProducer {
                owner: machine_id(1),
                producer: operation_id(9),
            },
        ),
    );
    rejected(
        "a zero direct-operation-result value",
        &put_u64(projections[7].coordinate.clone().unwrap(), 0),
        CodecError::ZeroIdentity("float-meaning direct operation-result value"),
    );
    rejected(
        "a direct operation result naming the call's result",
        &put_u64(projections[7].coordinate.clone().unwrap(), 12),
        invalid_projection(
            7,
            FloatMeaningProjectionVerificationError::InvalidDirectOperationResult {
                owner: machine_id(1),
                producer: operation_id(2),
                result: value_id(12),
            },
        ),
    );
    rejected(
        "a direct operation result under another IEEE format",
        &put_u8(projections[7].source_format.clone().unwrap(), 2),
        invalid_projection(
            7,
            FloatMeaningProjectionVerificationError::SourceFormatMismatch,
        ),
    );
    // Retargeting another row onto this row's exact source key collides at the
    // module join even though the substituted payload stays well-formed.
    rejected(
        "a direct operation-result source duplicated onto an earlier key",
        &splice(
            projections[7].source.clone(),
            &encoded[projections[6].source.clone()],
        ),
        CodecError::InvalidModule(ModuleError::DuplicateFloatMeaningProjection {
            first: 6,
            duplicate: 7,
        }),
    );

    // --- direct call result source --------------------------------------------

    // The direct call result rejoins owner, producer, and result to one
    // scalar-result call operation in the owner's blocks; non-call producers
    // and unknown producers take distinct rejection arms.
    rejected(
        "a zero direct-call-result owner",
        &put_u64(projections[8].owner.clone().unwrap(), 0),
        CodecError::ZeroIdentity("float-meaning direct call-result owner"),
    );
    rejected(
        "a direct-call-result owner outside the module",
        &put_u64(projections[8].owner.clone().unwrap(), 9),
        invalid_projection(
            8,
            FloatMeaningProjectionVerificationError::InvalidDirectCallResultOwner(machine_id(9)),
        ),
    );
    rejected(
        "a direct call result rehomed to the callee",
        &put_u64(projections[8].owner.clone().unwrap(), 2),
        invalid_projection(
            8,
            FloatMeaningProjectionVerificationError::InvalidDirectCallResultProducer {
                owner: machine_id(2),
                producer: operation_id(3),
            },
        ),
    );
    rejected(
        "a zero direct-call-result producer",
        &put_u64(projections[8].producer.clone().unwrap(), 0),
        CodecError::ZeroIdentity("float-meaning direct call-result producer"),
    );
    rejected(
        "a direct call result on a non-call producer",
        &put_u64(projections[8].producer.clone().unwrap(), 2),
        invalid_projection(
            8,
            FloatMeaningProjectionVerificationError::InvalidDirectCallResultProducerKind {
                owner: machine_id(1),
                producer: operation_id(2),
            },
        ),
    );
    rejected(
        "a direct call result on an unknown producer",
        &put_u64(projections[8].producer.clone().unwrap(), 9),
        invalid_projection(
            8,
            FloatMeaningProjectionVerificationError::InvalidDirectCallResultProducer {
                owner: machine_id(1),
                producer: operation_id(9),
            },
        ),
    );
    rejected(
        "a zero direct-call-result value",
        &put_u64(projections[8].coordinate.clone().unwrap(), 0),
        CodecError::ZeroIdentity("float-meaning direct call-result value"),
    );
    rejected(
        "a direct call result naming the literal's result",
        &put_u64(projections[8].coordinate.clone().unwrap(), 11),
        invalid_projection(
            8,
            FloatMeaningProjectionVerificationError::InvalidDirectCallResult {
                owner: machine_id(1),
                producer: operation_id(3),
                result: value_id(11),
            },
        ),
    );
    rejected(
        "a direct call result under another IEEE format",
        &put_u8(projections[8].source_format.clone().unwrap(), 2),
        invalid_projection(
            8,
            FloatMeaningProjectionVerificationError::SourceFormatMismatch,
        ),
    );

    // --- direct block parameter source -----------------------------------------

    // The direct block parameter rejoins owner, block, and parameter to one
    // block's declared parameter table inside the owner.
    rejected(
        "a zero direct-block-parameter owner",
        &put_u64(projections[9].owner.clone().unwrap(), 0),
        CodecError::ZeroIdentity("float-meaning direct block-parameter owner"),
    );
    rejected(
        "a direct-block-parameter owner outside the module",
        &put_u64(projections[9].owner.clone().unwrap(), 9),
        invalid_projection(
            9,
            FloatMeaningProjectionVerificationError::InvalidDirectBlockParameterOwner(machine_id(
                9,
            )),
        ),
    );
    rejected(
        "a direct block parameter rehomed to the caller",
        &put_u64(projections[9].owner.clone().unwrap(), 1),
        invalid_projection(
            9,
            FloatMeaningProjectionVerificationError::InvalidDirectBlockParameterBlock {
                owner: machine_id(1),
                block: block_id(3),
            },
        ),
    );
    rejected(
        "a zero direct-block-parameter block",
        &put_u64(projections[9].producer.clone().unwrap(), 0),
        CodecError::ZeroIdentity("float-meaning direct block-parameter block"),
    );
    rejected(
        "a direct block parameter on a parameterless block",
        &put_u64(projections[9].producer.clone().unwrap(), 2),
        invalid_projection(
            9,
            FloatMeaningProjectionVerificationError::InvalidDirectBlockParameter {
                owner: machine_id(2),
                block: block_id(2),
                parameter: value_id(13),
            },
        ),
    );
    rejected(
        "a direct block parameter on an unknown block",
        &put_u64(projections[9].producer.clone().unwrap(), 9),
        invalid_projection(
            9,
            FloatMeaningProjectionVerificationError::InvalidDirectBlockParameterBlock {
                owner: machine_id(2),
                block: block_id(9),
            },
        ),
    );
    rejected(
        "a zero direct-block-parameter value",
        &put_u64(projections[9].coordinate.clone().unwrap(), 0),
        CodecError::ZeroIdentity("float-meaning direct block-parameter value"),
    );
    rejected(
        "a direct block parameter outside the block's table",
        &put_u64(projections[9].coordinate.clone().unwrap(), 9),
        invalid_projection(
            9,
            FloatMeaningProjectionVerificationError::InvalidDirectBlockParameter {
                owner: machine_id(2),
                block: block_id(3),
                parameter: value_id(9),
            },
        ),
    );
    rejected(
        "a direct block parameter under another IEEE format",
        &put_u8(projections[9].source_format.clone().unwrap(), 2),
        invalid_projection(
            9,
            FloatMeaningProjectionVerificationError::SourceFormatMismatch,
        ),
    );

    // --- direct structural leaf source -----------------------------------------

    // The direct structural leaf rejoins owner and the canonical field path to
    // one structural parameter's declared leaf type at the exact IEEE format.
    rejected(
        "a zero direct-structural-leaf owner",
        &put_u64(projections[10].owner.clone().unwrap(), 0),
        CodecError::ZeroIdentity("float-meaning direct structural-leaf owner"),
    );
    rejected(
        "a direct-structural-leaf owner outside the module",
        &put_u64(projections[10].owner.clone().unwrap(), 9),
        invalid_projection(
            10,
            FloatMeaningProjectionVerificationError::InvalidDirectStructuralLeafOwner(machine_id(
                9,
            )),
        ),
    );
    rejected(
        "a direct structural leaf rehomed to the callee",
        &put_u64(projections[10].owner.clone().unwrap(), 2),
        invalid_projection(
            10,
            FloatMeaningProjectionVerificationError::InvalidDirectStructuralLeaf {
                owner: machine_id(2),
            },
        ),
    );
    rejected(
        "a zero direct-structural-leaf root",
        &put_u64(projections[10].field_root.clone().unwrap(), 0),
        CodecError::ZeroIdentity("PlaceId"),
    );
    rejected(
        "a direct structural leaf on an unknown root",
        &put_u64(projections[10].field_root.clone().unwrap(), 9),
        invalid_projection(
            10,
            FloatMeaningProjectionVerificationError::InvalidDirectStructuralLeaf {
                owner: machine_id(1),
            },
        ),
    );
    rejected(
        "a direct structural leaf on an unknown field",
        &put_u64(projections[10].field_segment.clone().unwrap(), 9),
        invalid_projection(
            10,
            FloatMeaningProjectionVerificationError::InvalidDirectStructuralLeaf {
                owner: machine_id(1),
            },
        ),
    );
    // An empty canonical field path is not a representable leaf at all.
    rejected(
        "a direct structural leaf with an empty path",
        &put_u32(projections[10].field_path_count.clone().unwrap(), 0),
        CodecError::MalformedProposition(PropositionError::EmptyIeeeFloatStructuralFieldPath),
    );
    rejected(
        "a direct structural leaf under another IEEE format",
        &put_u8(projections[10].source_format.clone().unwrap(), 2),
        invalid_projection(
            10,
            FloatMeaningProjectionVerificationError::SourceFormatMismatch,
        ),
    );

    // --- semantic application source ------------------------------------------

    // The application contract rejoins ordinal, version, and commitment to
    // exactly one FloatSemantics catalog row; each axis substitutes
    // independently into the same stranded join.
    rejected(
        "a substituted application contract row",
        &put_u8(projections[11].application_row.clone().unwrap(), u8::MAX),
        invalid_projection(
            11,
            FloatMeaningProjectionVerificationError::SemanticApplicationContractMismatch,
        ),
    );
    rejected(
        "a substituted application contract version",
        &put_u16(projections[11].application_version.clone().unwrap(), 0),
        invalid_projection(
            11,
            FloatMeaningProjectionVerificationError::SemanticApplicationContractMismatch,
        ),
    );
    rejected(
        "a substituted application contract commitment",
        &put_u8(
            projections[11].application_commitment.clone().unwrap(),
            0x5A,
        ),
        invalid_projection(
            11,
            FloatMeaningProjectionVerificationError::SemanticApplicationContractMismatch,
        ),
    );
    // The declared format rejoins to the row's catalog operation before the
    // signature operands are inspected at all.
    rejected(
        "a semantic application declared under another IEEE format",
        &put_u8(projections[11].source_format.clone().unwrap(), 2),
        invalid_projection(
            11,
            FloatMeaningProjectionVerificationError::SourceFormatMismatch,
        ),
    );
    // A format operand disagreeing with the declared format strands the
    // format rejoin; a meaning operand naming its own row or a later row
    // strands the operand-row join.
    rejected(
        "a semantic application format operand under another IEEE format",
        &put_u8(
            projections[11].application_operand_format.clone().unwrap(),
            2,
        ),
        invalid_projection(
            11,
            FloatMeaningProjectionVerificationError::SemanticApplicationFormatMismatch,
        ),
    );
    rejected(
        "a semantic application operand retargeted to a later row",
        &put_u32(projections[11].application_operand_id.clone().unwrap(), 12),
        invalid_projection(
            11,
            FloatMeaningProjectionVerificationError::SemanticApplicationOperandRow { operand: 1 },
        ),
    );
    for tag in [0, 3, u8::MAX] {
        rejected(
            "an unknown application operand tag",
            &put_u8(
                projections[11].application_operand_tag.clone().unwrap(),
                tag,
            ),
            CodecError::InvalidTag("FloatSemanticApplicationOperand", tag),
        );
    }
    // An operand retarget onto a transitional binary64 carrier stays
    // representable: the format-carrying row constrains only its Format
    // operand, and a non-literal operand leaves the application undischarged.
    divergent(
        "a semantic application operand retargeted to a transitional binary64 carrier",
        &put_u32(projections[11].application_operand_id.clone().unwrap(), 4),
    );
    // A retarget of the discharged literal onto the application source lands
    // on the identical (source, operation) key and rejects at the module
    // join; retargeting the application onto a fresh literal stays
    // representable.
    rejected(
        "a literal retargeted onto the semantic application source",
        &splice(
            projections[12].source.clone(),
            &encoded[projections[11].source.clone()],
        ),
        CodecError::InvalidModule(ModuleError::DuplicateFloatMeaningProjection {
            first: 11,
            duplicate: 12,
        }),
    );
    divergent(
        "a semantic application retargeted to an exact literal",
        &splice(
            projections[11].source.clone(),
            &[&[2_u8], &0x4080_0000_u32.to_le_bytes()[..]].concat(),
        ),
    );

    // --- operation and the closed contract identity ---------------------------

    // The catalog operation tag admits exactly two operations; a substituted
    // operation strands the contract join because the stored identity derives
    // from the original operation.
    for tag in [0, 3, u8::MAX] {
        rejected(
            "an unknown float-projection operation tag",
            &put_u8(projections[0].operation.clone(), tag),
            CodecError::InvalidTag("FloatMeaningProjectionOperation", tag),
        );
    }
    rejected(
        "a meaning32 projection remarked as meaning64",
        &put_u8(projections[0].operation.clone(), 2),
        invalid_projection(
            0,
            FloatMeaningProjectionVerificationError::ContractIdentityMismatch,
        ),
    );
    rejected(
        "a meaning64 projection remarked as meaning32",
        &put_u8(projections[4].operation.clone(), 1),
        invalid_projection(
            4,
            FloatMeaningProjectionVerificationError::ContractIdentityMismatch,
        ),
    );

    // Every contract axis is bound to the operation's closed catalog
    // identity: format, operation, declaration, catalog version, and the
    // commitment each substitute independently into the same stranded join.
    rejected(
        "a substituted float contract format",
        &put_u16(projections[0].contract_format.clone(), 64),
        invalid_projection(
            0,
            FloatMeaningProjectionVerificationError::ContractIdentityMismatch,
        ),
    );
    rejected(
        "a substituted float contract operation",
        &put_u8(projections[0].contract_operation.clone(), 2),
        invalid_projection(
            0,
            FloatMeaningProjectionVerificationError::ContractIdentityMismatch,
        ),
    );
    rejected(
        "a substituted float contract declaration",
        &put_u8(projections[0].contract_declaration.clone(), 2),
        invalid_projection(
            0,
            FloatMeaningProjectionVerificationError::ContractIdentityMismatch,
        ),
    );
    rejected(
        "a substituted float contract catalog version",
        &put_u16(projections[0].contract_version.clone(), 2),
        invalid_projection(
            0,
            FloatMeaningProjectionVerificationError::ContractIdentityMismatch,
        ),
    );
    rejected(
        "a substituted float contract commitment",
        &put_u8(projections[0].contract_commitment.clone(), 0x5A),
        invalid_projection(
            0,
            FloatMeaningProjectionVerificationError::ContractIdentityMismatch,
        ),
    );
    rejected(
        "a substituted meaning64 contract commitment",
        &put_u8(projections[4].contract_commitment.clone(), 0xA5),
        invalid_projection(
            4,
            FloatMeaningProjectionVerificationError::ContractIdentityMismatch,
        ),
    );

    // --- equality roster ------------------------------------------------------

    // A roster count lying about its rows reads the following zero-count
    // sections as a phantom row or starves the walk entirely. The phantom row
    // shifts every later count forward, landing a nonzero count on the
    // quotient-correspondence roster whose first "row" then reads machine
    // bytes as an over-long identity string.
    rejected(
        "a float-equality roster count one over",
        &put_u32(spans.equality_count.clone(), 5),
        CodecError::StringTooLong("quotient static application bindings"),
    );
    rejected(
        "a maximal float-equality roster count",
        &put_u32(spans.equality_count.clone(), u32::MAX),
        CodecError::UnexpectedEnd,
    );
    // The equalities carry no obligation the projections need: clearing or
    // truncating the roster tail keeps the module representable.
    let mut cleared_equalities = encoded[..spans.equality_count.start].to_vec();
    cleared_equalities.extend_from_slice(&0_u32.to_le_bytes());
    cleared_equalities.extend_from_slice(&encoded[equalities[3].row.end..]);
    divergent("a cleared float-equality roster", &cleared_equalities);
    divergent(
        "a dropped trailing float-equality row",
        &drop_row(spans.equality_count.clone(), equalities[3].row.clone()),
    );
    rejected(
        "a dropped first float-equality row",
        &drop_row(spans.equality_count.clone(), equalities[0].row.clone()),
        dense_equality_order(),
    );
    rejected(
        "a duplicated float-equality row",
        &duplicate_row(spans.equality_count.clone(), equalities[1].row.clone()),
        dense_equality_order(),
    );
    let mut swapped = encoded[..equalities[0].row.start].to_vec();
    swapped.extend_from_slice(&encoded[equalities[1].row.clone()]);
    swapped.extend_from_slice(&encoded[equalities[0].row.clone()]);
    swapped.extend_from_slice(&encoded[equalities[1].row.end..]);
    rejected(
        "a reordered float-equality roster",
        &swapped,
        dense_equality_order(),
    );

    // The proposition identity is the dense roster index, and operands stay
    // canonically ordered and inside the projection table.
    rejected(
        "a substituted float-equality id",
        &put_u32(equalities[0].id.clone(), 7),
        dense_equality_order(),
    );
    rejected(
        "a float-equality operand pair out of order",
        &put_u32(equalities[0].left.clone(), 2),
        dense_equality_order(),
    );
    rejected(
        "a float-equality operand outside the projections",
        &put_u32(equalities[0].right.clone(), 13),
        CodecError::InvalidModule(ModuleError::UnknownFloatMeaningEqualityOperand {
            proposition: 0,
            operand: 13,
        }),
    );
    // An operand retarget that crosses the format/operation/contract carrier
    // strands the reconstructed equality join.
    rejected(
        "a float-equality operand retargeted across carriers",
        &put_u32(equalities[0].right.clone(), 4),
        invalid_projection(
            4,
            FloatMeaningProjectionVerificationError::EqualityCarrierMismatch,
        ),
    );
    rejected(
        "a second float-equality operand retargeted across carriers",
        &put_u32(equalities[2].right.clone(), 5),
        invalid_projection(
            5,
            FloatMeaningProjectionVerificationError::EqualityCarrierMismatch,
        ),
    );
    // Inside one carrier an operand retarget stays representable, including a
    // self-equality and an operand pair coinciding with another row's.
    divergent(
        "a float-equality retargeted inside its carrier",
        &put_u32(equalities[0].right.clone(), 2),
    );
    divergent(
        "a float-equality collapsed to a self-equality",
        &put_u32(equalities[0].left.clone(), 1),
    );
    divergent(
        "a second float-equality retargeted inside its carrier",
        &put_u32(equalities[2].left.clone(), 0),
    );
    // Every direct carrier shares the binary32 meaning catalog operation, so
    // retargeting an operand onto any of them stays representable.
    divergent(
        "a float-equality retargeted onto a direct machine-result carrier",
        &put_u32(equalities[0].right.clone(), 6),
    );
    divergent(
        "a float-equality retargeted onto a direct operation-result carrier",
        &put_u32(equalities[0].right.clone(), 7),
    );
    divergent(
        "a float-equality retargeted onto a direct call-result carrier",
        &put_u32(equalities[2].right.clone(), 8),
    );
    divergent(
        "a float-equality retargeted onto a direct block-parameter carrier",
        &put_u32(equalities[2].right.clone(), 9),
    );
    divergent(
        "a float-equality retargeted onto a direct structural-leaf carrier",
        &put_u32(equalities[2].right.clone(), 10),
    );
    // The semantic application's result rides the same proof-value space: a
    // same-format operand retarget onto it stays representable, including a
    // retarget of the application's own discharged-literal equality.
    divergent(
        "a float-equality retargeted onto the semantic application carrier",
        &put_u32(equalities[0].right.clone(), 11),
    );
    divergent(
        "a second float-equality retargeted onto the application carrier",
        &put_u32(equalities[2].right.clone(), 11),
    );

    // --- producer-side rejections --------------------------------------------

    // Every validation failure above also fails closed on the way out: the
    // canonical encoder runs the same order and module validation before
    // emitting bytes.
    let mut changed = module.clone();
    changed.float_meaning_projections.swap(0, 1);
    encode_rejected(
        "a producer-side reordered float-projection roster",
        &changed,
        dense_projection_order(),
    );
    let mut changed = module.clone();
    changed.float_meaning_projections[0].result.id = ProofValueId(7);
    encode_rejected(
        "a producer-side non-dense float-projection result",
        &changed,
        dense_projection_order(),
    );
    let mut changed = module.clone();
    changed.float_meaning_projections[4].source =
        changed.float_meaning_projections[5].source.clone();
    encode_rejected(
        "a producer-side duplicated float-projection key",
        &changed,
        CodecError::InvalidModule(ModuleError::DuplicateFloatMeaningProjection {
            first: 4,
            duplicate: 5,
        }),
    );
    let mut changed = module.clone();
    changed.float_meaning_projections[0].source =
        FloatMeaningSource::TransitionalInput(FloatProjectionInput {
            id: FloatProjectionInputId(0),
            format: IeeeFloatFormat::Binary64,
        });
    encode_rejected(
        "a producer-side source-format mismatch",
        &changed,
        invalid_projection(
            0,
            FloatMeaningProjectionVerificationError::SourceFormatMismatch,
        ),
    );
    let mut changed = module.clone();
    changed.float_meaning_projections[0].contract =
        contract(numerics::float_projection::FloatProjectionOperation::Meaning64);
    encode_rejected(
        "a producer-side foreign contract identity",
        &changed,
        invalid_projection(
            0,
            FloatMeaningProjectionVerificationError::ContractIdentityMismatch,
        ),
    );
    let mut changed = module.clone();
    changed.float_meaning_projections[3].source =
        FloatMeaningSource::DirectMachineParameter(DirectMachineFloatParameter {
            owner: machine_id(9),
            parameter: value_id(6),
            format: IeeeFloatFormat::Binary32,
        });
    encode_rejected(
        "a producer-side direct parameter outside the module",
        &changed,
        invalid_projection(
            3,
            FloatMeaningProjectionVerificationError::InvalidDirectParameterOwner(machine_id(9)),
        ),
    );
    // A coherent binary64 row — operation, contract, and source format moved
    // together — reaches the owner-table rejoin, where the declared
    // parameter's binary32 scalar type strands the direct-source join.
    let mut changed = module.clone();
    changed.float_meaning_projections[3].source =
        FloatMeaningSource::DirectMachineParameter(DirectMachineFloatParameter {
            owner: machine_id(1),
            parameter: value_id(6),
            format: IeeeFloatFormat::Binary64,
        });
    changed.float_meaning_projections[3].operation = FloatMeaningProjectionOperation::Meaning64;
    changed.float_meaning_projections[3].contract =
        contract(numerics::float_projection::FloatProjectionOperation::Meaning64);
    encode_rejected(
        "a producer-side direct parameter at the wrong declared format",
        &changed,
        invalid_projection(
            3,
            FloatMeaningProjectionVerificationError::DirectParameterFormatMismatch,
        ),
    );
    // The same coherent binary64 carrier on every other direct source reaches
    // that source's declared-coordinate rejoin instead.
    let mut changed = module.clone();
    changed.float_meaning_projections[6].source =
        FloatMeaningSource::DirectMachineResult(DirectMachineFloatResult {
            owner: machine_id(2),
            result: value_id(8),
            format: IeeeFloatFormat::Binary64,
        });
    changed.float_meaning_projections[6].operation = FloatMeaningProjectionOperation::Meaning64;
    changed.float_meaning_projections[6].contract =
        contract(numerics::float_projection::FloatProjectionOperation::Meaning64);
    encode_rejected(
        "a producer-side direct result at the wrong declared format",
        &changed,
        invalid_projection(
            6,
            FloatMeaningProjectionVerificationError::DirectResultFormatMismatch,
        ),
    );
    let mut changed = module.clone();
    changed.float_meaning_projections[7].source =
        FloatMeaningSource::DirectOperationResult(DirectOperationFloatResult {
            owner: machine_id(1),
            producer: operation_id(2),
            result: value_id(11),
            format: IeeeFloatFormat::Binary64,
        });
    changed.float_meaning_projections[7].operation = FloatMeaningProjectionOperation::Meaning64;
    changed.float_meaning_projections[7].contract =
        contract(numerics::float_projection::FloatProjectionOperation::Meaning64);
    encode_rejected(
        "a producer-side direct operation result at the wrong declared format",
        &changed,
        invalid_projection(
            7,
            FloatMeaningProjectionVerificationError::DirectOperationResultFormatMismatch,
        ),
    );
    let mut changed = module.clone();
    changed.float_meaning_projections[8].source =
        FloatMeaningSource::DirectCallResult(DirectCallFloatResult {
            owner: machine_id(1),
            producer: operation_id(3),
            result: value_id(12),
            format: IeeeFloatFormat::Binary64,
        });
    changed.float_meaning_projections[8].operation = FloatMeaningProjectionOperation::Meaning64;
    changed.float_meaning_projections[8].contract =
        contract(numerics::float_projection::FloatProjectionOperation::Meaning64);
    encode_rejected(
        "a producer-side direct call result at the wrong declared format",
        &changed,
        invalid_projection(
            8,
            FloatMeaningProjectionVerificationError::DirectCallResultFormatMismatch,
        ),
    );
    let mut changed = module.clone();
    changed.float_meaning_projections[9].source =
        FloatMeaningSource::DirectBlockParameter(DirectBlockFloatParameter {
            owner: machine_id(2),
            block: block_id(3),
            parameter: value_id(13),
            format: IeeeFloatFormat::Binary64,
        });
    changed.float_meaning_projections[9].operation = FloatMeaningProjectionOperation::Meaning64;
    changed.float_meaning_projections[9].contract =
        contract(numerics::float_projection::FloatProjectionOperation::Meaning64);
    encode_rejected(
        "a producer-side direct block parameter at the wrong declared format",
        &changed,
        invalid_projection(
            9,
            FloatMeaningProjectionVerificationError::DirectBlockParameterFormatMismatch,
        ),
    );
    let mut changed = module.clone();
    changed.float_meaning_projections[10].source =
        FloatMeaningSource::DirectStructuralLeaf(DirectStructuralFloatLeaf {
            owner: machine_id(1),
            field: IeeeFloatStructuralField::new(
                place_id(1),
                vec![CanonicalStructuralPathSegment::Field(structural_field_id(
                    1,
                ))],
            )
            .expect("a nonempty structural leaf path"),
            format: IeeeFloatFormat::Binary64,
        });
    changed.float_meaning_projections[10].operation = FloatMeaningProjectionOperation::Meaning64;
    changed.float_meaning_projections[10].contract =
        contract(numerics::float_projection::FloatProjectionOperation::Meaning64);
    encode_rejected(
        "a producer-side direct structural leaf at the wrong declared format",
        &changed,
        invalid_projection(
            10,
            FloatMeaningProjectionVerificationError::DirectStructuralLeafFormatMismatch,
        ),
    );
    let mut changed = module.clone();
    changed.float_meaning_equalities.swap(0, 1);
    encode_rejected(
        "a producer-side reordered float-equality roster",
        &changed,
        dense_equality_order(),
    );
    let mut changed = module.clone();
    changed.float_meaning_equalities[0].id = ProofPropositionId(7);
    encode_rejected(
        "a producer-side non-dense float-equality id",
        &changed,
        dense_equality_order(),
    );
    let mut changed = module.clone();
    changed.float_meaning_equalities[0].left = ProofValueId(2);
    encode_rejected(
        "a producer-side out-of-order float-equality pair",
        &changed,
        dense_equality_order(),
    );
    let mut changed = module.clone();
    changed.float_meaning_equalities[0].right = ProofValueId(13);
    encode_rejected(
        "a producer-side unknown float-equality operand",
        &changed,
        CodecError::InvalidModule(ModuleError::UnknownFloatMeaningEqualityOperand {
            proposition: 0,
            operand: 13,
        }),
    );
    // A producer-spelled application operand that names no earlier row fails
    // the same module validation on the way out.
    let mut changed = module.clone();
    let mut application = match changed.float_meaning_projections[11].source.clone() {
        FloatMeaningSource::SemanticApplication(application) => application,
        _ => unreachable!("the fixture's twelfth row is a semantic application"),
    };
    application.operands[1] = FloatSemanticApplicationOperand::Meaning(ProofValueId(12));
    changed.float_meaning_projections[11].source =
        FloatMeaningSource::SemanticApplication(application);
    encode_rejected(
        "a producer-side forward application operand",
        &changed,
        invalid_projection(
            11,
            FloatMeaningProjectionVerificationError::SemanticApplicationOperandRow { operand: 1 },
        ),
    );
    let mut changed = module.clone();
    changed.float_meaning_equalities[0].right = ProofValueId(4);
    encode_rejected(
        "a producer-side float-equality carrier mismatch",
        &changed,
        invalid_projection(
            4,
            FloatMeaningProjectionVerificationError::EqualityCarrierMismatch,
        ),
    );

    // --- module envelope boundaries ------------------------------------------

    // Truncation inside either roster and a trailing byte reject at the
    // envelope, before semantic replay ever runs.
    for cut in [
        projections[0].row.end - 1,
        projections[10].row.end - 1,
        projections[11].row.end - 1,
        equalities[3].row.end - 1,
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
