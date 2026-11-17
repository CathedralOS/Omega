/*
The tag tables in `wiki/spec/terminal-psi/encoding.md` are closed: a tag the
table omits must reject, and a tag it lists must decode. These tests read every
marked table straight from the page and feed every `u8` to the codec's decoder
for that space, so a codec tag the page does not list, or a page row the codec
no longer accepts, fails here instead of surviving as silent drift.

Each probe is the tag byte, after the fields the decoder reads before it (a
short valid prefix) and before any fields a decoder must read before it can
refuse the tag (a suffix). A listed tag then either decodes or fails on its own
fields; only an omitted tag returns `InvalidTag` for this space. The check
compares the set of accepted tags, not the row names or field lists.
*/

use crate::CodecError;
use crate::sections::debug_map;
use crate::wire_fields::Field;
use crate::wire_fields::wire::Reader;

const ENCODING_PAGE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../../wiki/spec/terminal-psi/encoding.md"
));

/// Marked tables this test cannot drive, each with the reason.
const UNCHECKED_TABLES: &[(&str, &str)] = &[(
    "admission-kind-tags",
    "admission kinds live only in the in-memory evidence route; the codec has no byte for them",
)];

/// A nonzero `u64` identity, as `Reader::id` requires.
const ID: [u8; 8] = [1, 0, 0, 0, 0, 0, 0, 0];
/// A `u32` count of one, framing a single row or segment.
const ONE: [u8; 4] = [1, 0, 0, 0];

/// The `(space, tag)` a decoder refused, if it refused a tag or bool byte.
type Refusal = Option<(&'static str, u8)>;
type Probe = fn(&mut Reader<'_>) -> Refusal;

fn refusal<T>(result: Result<T, CodecError>) -> Refusal {
    match result {
        Err(CodecError::InvalidTag(space, tag)) => Some((space, tag)),
        Err(CodecError::InvalidBoolean(byte)) => Some(("bool", byte)),
        _ => None,
    }
}

fn read<T, F: Field<T>>(reader: &mut Reader<'_>) -> Refusal {
    refusal(F::read(reader))
}

struct TagTable {
    marker: &'static str,
    spaces: &'static [&'static str],
    prefix: Vec<u8>,
    suffix: Vec<u8>,
    /// A byte the page's prose, not its table, gives as the absent form.
    absent: Option<u8>,
    probe: Probe,
}

impl TagTable {
    fn after(mut self, prefix: &[&[u8]]) -> Self {
        self.prefix = prefix.concat();
        self
    }

    fn padded(mut self, suffix: &[u8]) -> Self {
        self.suffix = suffix.to_vec();
        self
    }

    fn absent(mut self, tag: u8) -> Self {
        self.absent = Some(tag);
        self
    }

    /// Every disagreement between each occurrence of the marked table and the decoder.
    fn drift(&self) -> Vec<String> {
        let mut drift = Vec::new();
        for listed in marked_tables(self.marker) {
            for tag in 0..=u8::MAX {
                let bytes = [self.prefix.as_slice(), &[tag], &self.suffix].concat();
                let rejected = (self.probe)(&mut Reader::new(&bytes))
                    .is_some_and(|(space, refused)| self.spaces.contains(&space) && refused == tag);
                let listed = listed.contains(&tag) || self.absent == Some(tag);
                match (listed, rejected) {
                    (true, true) => drift.push(format!(
                        "`{}` lists {tag}, which the decoder rejects",
                        self.marker
                    )),
                    (false, false) => drift.push(format!(
                        "`{}` omits {tag}, which the decoder accepts",
                        self.marker
                    )),
                    _ => {}
                }
            }
        }
        drift
    }
}

fn table(marker: &'static str, spaces: &'static [&'static str], probe: Probe) -> TagTable {
    TagTable {
        marker,
        spaces,
        prefix: Vec::new(),
        suffix: Vec::new(),
        absent: None,
        probe,
    }
}

/// Returns the live tags of each table under `<!-- marker -->`, in page order.
/// A row named `retired` records a tag that must reject, so it is not returned.
fn marked_tables(marker: &str) -> Vec<Vec<u8>> {
    let anchor = format!("<!-- {marker} -->");
    let lines: Vec<&str> = ENCODING_PAGE.lines().collect();
    let tables: Vec<Vec<u8>> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.trim() == anchor)
        .map(|(at, _)| {
            let mut rows = lines[at + 1..]
                .iter()
                .skip_while(|line| line.trim().is_empty());
            let header = rows.next().copied().unwrap_or_default();
            assert!(
                header.starts_with('|'),
                "`{anchor}` is not followed by a table"
            );
            rows.skip(1)
                .take_while(|line| line.starts_with('|'))
                .filter(|row| {
                    row.split('|')
                        .nth(2)
                        .is_none_or(|name| name.trim() != "retired")
                })
                .map(|row| {
                    let cell = row.split('|').nth(1).unwrap_or_default().trim();
                    cell.parse()
                        .unwrap_or_else(|_| panic!("`{marker}` row has a non-`u8` tag: {row}"))
                })
                .collect()
        })
        .collect();
    assert!(!tables.is_empty(), "the page has no `{anchor}` table");
    for listed in &tables {
        assert!(!listed.is_empty(), "`{marker}` lists no tags");
    }
    tables
}

/// The live tags of the marked table, across every occurrence of its marker.
pub(crate) fn listed_tags(marker: &str) -> Vec<u8> {
    marked_tables(marker).concat()
}

fn page_markers() -> Vec<&'static str> {
    let mut markers: Vec<&str> = ENCODING_PAGE
        .lines()
        .filter_map(|line| line.trim().strip_prefix("<!-- ")?.strip_suffix(" -->"))
        .filter(|marker| marker.ends_with("-tags"))
        .collect();
    markers.sort_unstable();
    markers.dedup();
    markers
}

fn tag_tables() -> Vec<TagTable> {
    use crate::block_wire;
    use crate::module_wire::{
        borrow_wire, carry_and_suspension_wire as carry, closed_conformance_wire, declaration_wire,
        evidence_wire, float_meaning_wire as float, recursive_component_wire as recursive,
    };
    use crate::semantic_module::{machine_wire, reach_application_wire};
    use crate::structural_wire::{
        structural_field_wire as field, structural_place_wire as place,
        structural_signature_wire as signature, structural_type_wire,
    };
    use crate::term_wire::{
        content_wire, contract_wire, integer_math_term_wire,
        mathematical_certificate_wire as certificate, proof_declaration_wire as proof_declaration,
        proof_term_wire, proposition_wire, scalar_term_wire, scalar_wire,
    };
    use crate::wire_fields;

    // A structural type declaration up to its shape tag: no quotient carrier,
    // type id, two identity indexes, four flags and no nominal cleanup.
    let structural_type_head: &[u8] = &[&[0][..], &ID, &[0; 4], &[0; 4], &[0; 4], &[0]].concat();
    // A ranked cycle up to an edge's comparison byte: Natural, one component
    // with no block ranks and one edge whose successor tuple holds one `u32` value.
    let rank_edge_head: &[u8] = &[
        &[3][..],
        &ONE,
        &[0; 4],
        &ONE,
        &ID,
        &ID,
        &ID,
        &ONE,
        &[2, 32, 0],
        &ID,
    ]
    .concat();

    vec![
        table(
            "domain-establishment-route-tags",
            &["StructuralEstablishmentRoute"],
            read::<_, declaration_wire::Route>,
        ),
        table("scalar-type-tags", &["ScalarType"], |reader| {
            refusal(scalar_wire::decode_scalar_type(reader))
        }),
        table("integer-type-tags", &["IntegerSign"], |reader| {
            refusal(scalar_wire::decode_integer_type(reader))
        })
        .padded(&[32, 0]),
        table("integer-value-tags", &["IntegerValue"], |reader| {
            refusal(scalar_wire::decode_integer_value(reader))
        }),
        table("ieee-float-value-tags", &["IeeeFloatValue"], |reader| {
            refusal(scalar_wire::decode_ieee_float_value(reader))
        }),
        table(
            "ieee-float-relation-tags",
            &["IeeeFloatComparisonOperation"],
            read::<_, wire_fields::FloatComparison>,
        ),
        table(
            "ieee-comparison-kind-tags",
            &["IeeeFloatComparisonKind"],
            |reader| refusal(field::decode_ieee_float_comparison_kind(reader)),
        ),
        table("boolean-tags", &["bool"], |reader| {
            refusal(reader.boolean())
        }),
        table(
            "optional-identity-tags",
            &["OptionalSemanticId"],
            |reader| {
                refusal(wire_fields::wire::decode_optional_id::<
                    semantic_vocabulary::ValueId,
                >(reader, "ValueId"))
            },
        )
        .absent(0),
        table(
            "scalar-field-carrier-path-tags",
            &["scalar field carrier path"],
            read::<_, wire_fields::FieldPath>,
        )
        .after(&[&ONE]),
        table("scalar-term-tags", &["ScalarTerm"], |reader| {
            refusal(scalar_term_wire::decode_scalar_term(reader))
        })
        .after(&[&ONE]),
        table("integer-math-term-tags", &["IntegerMathTerm"], |reader| {
            refusal(integer_math_term_wire::decode_integer_math_term(reader))
        })
        .after(&[&ONE]),
        table("proof-term-tags", &["proof term"], |reader| {
            refusal(proof_term_wire::decode_proof_terms(reader))
        })
        .after(&[&ONE, &ONE]),
        table("content-term-tags", &["ContentTerm"], |reader| {
            refusal(content_wire::decode_content_term(reader))
        }),
        table("proposition-tags", &["Proposition"], |reader| {
            refusal(proposition_wire::decode_proposition(reader))
        })
        .after(&[&ONE]),
        table("operation-tags", &["OperationKind"], |reader| {
            refusal(block_wire::decode_operation_kind(reader))
        }),
        table("operation-result-tags", &["OperationResult"], |reader| {
            refusal(block_wire::decode_operation(reader))
        })
        .after(&[&ID, &[0, 0]]),
        table(
            "record-field-value-tags",
            &["RecordFieldValue"],
            read::<_, wire_fields::RecordFields>,
        )
        .after(&[&ONE, &ID]),
        table(
            "outcome-result-substitution-tags",
            &["bool"],
            read::<_, wire_fields::Optional<wire_fields::ResultSubstitution>>,
        ),
        table(
            "terminator-tags",
            &["Terminator"],
            read::<_, block_wire::terminator_wire::TerminatorRow>,
        ),
        table(
            "affine-cleanup-action-tags",
            &["TerminalAffineCleanupAction"],
            |reader| refusal(place::decode_affine_cleanup_action(reader)),
        ),
        table("ranked-scc-tags", &["TerminalRankedScc"], |reader| {
            refusal(machine_wire::decode_ranked_scc(reader))
        })
        .absent(0),
        table(
            "rank-comparison-tags",
            &["TerminalNaturalRankComparison"],
            |reader| refusal(machine_wire::decode_ranked_scc(reader)),
        )
        .after(&[rank_edge_head]),
        table(
            "machine-result-tags",
            &["TerminalMachineResult"],
            read::<_, machine_wire::MachineResult>,
        ),
        table("place-kind-tags", &["StructuralPlaceKind"], |reader| {
            refusal(place::decode_structural_place_kind(reader))
        }),
        table(
            "content-structural-place-kind-tags",
            &["StructuralPlaceKind", "ContentStructuralPlaceKind"],
            read::<_, content_wire::ContentPlaceDeclaration>,
        )
        .after(&[&ID])
        .padded(&[1; 12]),
        table(
            "content-place-version-tags",
            &["ContentPlaceVersion"],
            read::<_, content_wire::PlaceVersion>,
        ),
        table(
            "content-place-segment-tags",
            &["ContentPlaceSegment"],
            read::<_, content_wire::PlaceSegment>,
        ),
        table(
            "content-algebra-kind-tags",
            &["ContentAlgebraKind"],
            |reader| refusal(content_wire::decode_content_algebra(reader)),
        ),
        table(
            "reach-parameter-tags",
            &["ClosedReachParameter"],
            |reader| refusal(reach_application_wire::decode_telescope(reader)),
        )
        .after(&[&ONE]),
        table("reach-argument-tags", &["ClosedReachArgument"], |reader| {
            refusal(reach_application_wire::decode_argument(reader))
        }),
        table(
            "structural-type-shape-tags",
            &["StructuralTypeShape"],
            |reader| refusal(structural_type_wire::decode_structural_type(reader)),
        )
        .after(&[structural_type_head]),
        table(
            "byte-sequence-carrier-tags",
            &["ByteSequenceCarrier"],
            |reader| refusal(field::decode_byte_sequence_carrier(reader)),
        ),
        table("binding-relevance-tags", &["BindingRelevance"], |reader| {
            refusal(field::decode_structural_field(reader))
        })
        .after(&[&ID, &[0; 8]]),
        table(
            "structural-field-type-tags",
            &["StructuralFieldType"],
            |reader| refusal(field::decode_structural_field_type(reader)),
        ),
        table("structural-access-tags", &["StructuralAccess"], |reader| {
            refusal(signature::decode_structural_access(reader))
        }),
        table(
            "structural-multiplicity-tags",
            &["StructuralMultiplicity"],
            |reader| {
                refusal(crate::structural_wire::structural_result_wire::decode_multiplicity(reader))
            },
        ),
        table(
            "structural-path-segment-tags",
            &["StructuralPathSegment"],
            |reader| refusal(place::decode_qualification_path(reader)),
        )
        .after(&[&ONE]),
        table(
            "canonical-path-segment-tags",
            &["CanonicalStructuralPathSegment"],
            |reader| refusal(field::decode_canonical_structural_field(reader)),
        )
        .after(&[&ID, &ONE]),
        table(
            "content-projection-expression-tags",
            &["ContentProjectionExpression"],
            |reader| refusal(signature::decode_content_projection_expression(reader)),
        ),
        table(
            "content-projection-scalar-tags",
            &["ContentProjectionScalar"],
            |reader| {
                refusal(crate::term_table::decode_term_table::<
                    semantic_vocabulary::ContentProjectionScalar,
                >(reader))
            },
        )
        .after(&[&ONE]),
        table(
            "borrow-boundary-tags",
            &["TerminalBorrowBoundarySource"],
            read::<_, borrow_wire::Boundary>,
        ),
        table(
            "borrow-owner-segment-tags",
            &["TerminalBorrowOwnerSegment"],
            read::<_, borrow_wire::OwnerSegment>,
        ),
        table(
            "borrow-place-segment-tags",
            &["TerminalBorrowPlaceSegment"],
            read::<_, borrow_wire::PlaceSegment>,
        ),
        table(
            "restoration-class-tags",
            &["TerminalReborrowRestorationClass"],
            read::<_, borrow_wire::RestorationClass>,
        ),
        table(
            "boundary-parameter-kind-tags",
            &["BoundaryParameterKind"],
            read::<_, signature::ParameterKind>,
        ),
        table(
            "boundary-result-tags",
            &["BoundaryMachineResult"],
            read::<_, signature::CallResult>,
        ),
        table(
            "boundary-content-guarantee-tags",
            &["BoundaryContentGuarantee"],
            read::<_, signature::ContentGuarantee>,
        ),
        table(
            "retained-borrow-root-tags",
            &["RetainedBorrowPlaceRoot"],
            read::<_, signature::PlaceRoot>,
        ),
        table(
            "retained-borrow-segment-tags",
            &["ContentPlaceSegment"],
            read::<_, signature::PlaceSegment>,
        ),
        table(
            "float-value-type-tags",
            &["ProofOnlyValueType"],
            read::<_, float::ValueType>,
        ),
        table(
            "float-meaning-source-tags",
            &["FloatMeaningSource"],
            read::<_, float::Source>,
        ),
        table("ieee-format-tags", &["IeeeFloatFormat"], |reader| {
            refusal(field::decode_ieee_float_format(reader))
        }),
        table(
            "float-operand-tags",
            &["FloatSemanticApplicationOperand"],
            read::<_, float::ApplicationOperand>,
        ),
        table(
            "float-projection-operation-tags",
            &["FloatMeaningProjectionOperation"],
            read::<_, float::Operation>,
        ),
        table(
            "proposition-binder-kind-tags",
            &["PropositionBinderKind"],
            read::<_, proof_declaration::BinderKind>,
        ),
        table(
            "proposition-evidence-tags",
            &["PropositionEvidence"],
            read::<_, proof_declaration::Evidence>,
        ),
        table(
            "binder-argument-kind-tags",
            &["PropositionBinderArgumentKind"],
            read::<_, proof_declaration::ArgumentKind>,
        ),
        table(
            "proposition-binder-argument-tags",
            &["PropositionBinderArgument"],
            read::<_, proof_declaration::BinderArgument>,
        )
        .after(&[&[1]]),
        table(
            "proposition-evidence-interface-tags",
            &["bool"],
            read::<_, wire_fields::Optional<proof_declaration::Interface>>,
        ),
        table(
            "evidence-lane-kind-tags",
            &["EvidenceContractLaneKind"],
            read::<_, evidence_wire::LaneKind>,
        ),
        table(
            "ranking-relation-tags",
            &["TerminalProofRankingRelation"],
            read::<_, recursive::Ranking>,
        ),
        table(
            "recursive-call-site-tags",
            &["TerminalProofRecursiveCallSite"],
            read::<_, recursive::CallSite>,
        ),
        table(
            "recursive-transition-lane-tags",
            &["TerminalProofRecursiveTransitionLane"],
            read::<_, recursive::Lane>,
        ),
        table(
            "conformance-parameter-kind-tags",
            &["ClosedConformanceParameterKind"],
            read::<_, closed_conformance_wire::ParameterKind>,
        ),
        table(
            "callable-result-tags",
            &["ClosedConformanceCallableResult"],
            read::<_, closed_conformance_wire::CallableResult>,
        ),
        table(
            "suspension-target-tags",
            &["TerminalSuspensionCallTarget"],
            read::<_, carry::CallTarget>,
        ),
        table(
            "carry-suspension-tags",
            &["CarrySuspension"],
            read::<_, carry::Suspension>,
        ),
        table("carry-cpu-tags", &["CarryCpu"], read::<_, carry::Cpu>),
        table(
            "carry-host-thread-tags",
            &["CarryHostThread"],
            read::<_, carry::HostThread>,
        ),
        table(
            "carry-address-tags",
            &["CarryAddress"],
            read::<_, carry::Address>,
        ),
        table(
            "suspension-place-tags",
            &["TerminalSuspensionPlace"],
            read::<_, carry::Place>,
        ),
        table(
            "suspension-value-type-tags",
            &["TerminalSuspensionValueType"],
            read::<_, carry::ValueType>,
        ),
        table(
            "suspension-storage-tags",
            &["TerminalSuspensionStorage"],
            read::<_, carry::Storage>,
        ),
        table(
            "crash-cause-tags",
            &["CrashCause"],
            read::<_, contract_wire::Cause>,
        ),
        table(
            "crash-route-guard-tags",
            &["CrashRouteGuard"],
            read::<_, contract_wire::RouteGuard>,
        ),
        table(
            "debug-source-origin-tags",
            &["DebugSourceOrigin"],
            |reader| debug_refusal(debug_map::decode_source_origin(reader)),
        ),
        table("debug-subject-tags", &["DebugSubject"], |reader| {
            debug_refusal(debug_map::decode_subject(reader))
        }),
        table("certificate-term-tags", &["MathematicalTerm"], |reader| {
            refusal(certificate::decode_term(reader, &[]))
        }),
        table("certificate-sort-tags", &["MathematicalSort"], |reader| {
            refusal(certificate::decode_sort(reader))
        }),
        table("certificate-level-tags", &["MathematicalLevel"], |reader| {
            refusal(certificate::decode_level(reader))
        }),
    ]
}

fn debug_refusal<T>(result: Result<T, debug_map::DebugMapError>) -> Refusal {
    match result {
        Err(debug_map::DebugMapError::InvalidTag(space, tag)) => Some((space, tag)),
        _ => None,
    }
}

#[test]
fn every_marked_tag_table_matches_its_decoder() {
    let drift: Vec<String> = tag_tables().iter().flat_map(TagTable::drift).collect();
    assert!(
        drift.is_empty(),
        "encoding.md disagrees with the codec:\n{}",
        drift.join("\n")
    );
}

#[test]
fn every_marked_tag_table_is_checked_or_explained() {
    let checked: Vec<&str> = tag_tables().iter().map(|table| table.marker).collect();
    let unexplained: Vec<&str> = page_markers()
        .into_iter()
        .filter(|marker| {
            !checked.contains(marker)
                && !UNCHECKED_TABLES
                    .iter()
                    .any(|(unchecked, _)| unchecked == marker)
        })
        .collect();
    assert!(
        unexplained.is_empty(),
        "marked tag tables with no decoder check: {unexplained:?}"
    );
    for (marker, _) in UNCHECKED_TABLES {
        assert!(
            page_markers().contains(marker),
            "`{marker}` is listed as unchecked but the page has no such table"
        );
    }
}
