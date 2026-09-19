//! One-field mutation coverage for the canonical Terminal obligation ledger.
//!
//! The ledger binds the exact Terminal semantic subject (vocabulary marker and
//! program fingerprint), the reconstruction trust graph, and the counted
//! roster of verifier-reconstructed obligations. Every representable field —
//! each owner variant's identities and positions, the obligation identity and
//! admission class, the proposition, the requirements and semantic-axiom
//! rosters, and the canonical-certificate flag — is substituted independently.
//! A substitution either fails to form a canonical ledger on the wire, or
//! decodes to a different ledger whose honestly recomputed fingerprint
//! diverges and whose independent reconstruction replay rejects it.

use std::ops::Range;

use super::{
    block_id, contract_id, edge_id, evidence_id, machine_id, obligation_id, operation_id, value_id,
};
use proof_admission::{AdmissionKind, AuthorizedAdmission, ObligationClass};
use semantic_vocabulary::{
    AdmissionSiteId, IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm, ScalarType,
};
use terminal_codec::{
    CodecError, build_terminal_obligation_ledger, canonical_proposition_order_key,
    current_terminal_trust_graph, decode_terminal_obligation_ledger,
    encode_terminal_obligation_ledger, terminal_obligation_ledger_fingerprint,
    validate_terminal_obligation_ledger,
};
use terminal_psi::{
    Block, ContractClause, MachineContract, Operation, OperationKind, OperationResult,
    TerminalMachine, TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
    VocabularyMarker,
};
use terminal_verifier::{ReconstructedTerminalObligation, ReconstructedTerminalObligationOwner};

const HEADER_LEN: usize = 8 + 2 + 2 + 32 + 32 + 4;

/// The caller/callee fixture: the caller runs an obligation-bearing exact
/// subtraction and a scalar call carrying two requirement obligations, and
/// each machine closes over a contract ensures clause. The produced roster
/// therefore carries `Operation`, `CallRequires`, and `ContractEnsures` owner
/// kinds with non-empty requirements, reconstructed axioms on the call rows,
/// and both canonical-certificate values.
fn ledger_fixture() -> TerminalModule {
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let integer = ScalarType::Integer(i8_type);
    let value = |id: u64| ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(id),
        scalar_type: integer,
    };
    let term = |id: u64| ScalarTerm::value(value_id(id), integer);
    let literal =
        |value| ScalarTerm::integer(i8_type, IntegerValue::Signed(value)).expect("i8 literal");
    let mut caller_requires = vec![
        Proposition::LessOrEqual(literal(-128), term(10)),
        Proposition::LessOrEqual(term(11), literal(127)),
    ];
    caller_requires.sort_by_key(|proposition| {
        canonical_proposition_order_key(proposition).expect("canonical requirement")
    });
    let mut callee_requires = vec![
        Proposition::LessOrEqual(literal(-128), term(200)),
        Proposition::LessOrEqual(term(201), literal(127)),
    ];
    callee_requires.sort_by_key(|proposition| {
        canonical_proposition_order_key(proposition).expect("canonical requirement")
    });

    let caller = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine_id(1),
        attachment: None,
        parameters: vec![value(10), value(11)],
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(value(13)),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(10),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(10),
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    static_reach_binding: None,
                    id: operation_id(10),
                    result: OperationResult::Scalar(value(20)),
                    kind: OperationKind::ExactIntegerSubtract {
                        left: value_id(10),
                        right: value_id(11),
                        obligation: obligation_id(100),
                    },
                },
                Operation {
                    static_reach_binding: None,
                    id: operation_id(11),
                    result: OperationResult::Scalar(value(27)),
                    kind: OperationKind::Call {
                        erased_arguments: Vec::new(),
                        callee: machine_id(2),
                        arguments: vec![value_id(10), value_id(11)],
                        requirement_obligations: vec![obligation_id(105), obligation_id(106)],
                        crash_continuations: Vec::new(),
                    },
                },
            ],
            terminator: Terminator::Return {
                cleanup_actions: Vec::new(),
                edge: edge_id(10),
                value: value_id(27),
            },
        }],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            id: contract_id(1),
            crash_routes: Vec::new(),
            requires: caller_requires,
            ensures: vec![ContractClause {
                obligation: obligation_id(1),
                proposition: Proposition::Equal(literal(7), literal(7)),
            }],
            outcome_specific_ensures: Vec::new(),
        },
    };

    let callee = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine_id(2),
        attachment: None,
        parameters: vec![value(200), value(201)],
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(value(202)),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(20),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(20),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Return {
                cleanup_actions: Vec::new(),
                edge: edge_id(20),
                value: value_id(200),
            },
        }],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            id: contract_id(2),
            crash_routes: Vec::new(),
            requires: callee_requires,
            ensures: vec![ContractClause {
                obligation: obligation_id(200),
                proposition: Proposition::LessOrEqual(term(200), literal(127)),
            }],
            outcome_specific_ensures: Vec::new(),
        },
    };

    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
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
        machines: vec![caller, callee],
    }
}

// --- test-local canonical row encoder --------------------------------------
//
// The ledger's row framing is re-derived here from the decoded row so every
// mutation can be expressed as a field substitution on the produced value.
// `canonical_proposition_order_key` supplies the codec-owned canonical byte
// form of every proposition, so the local encoder covers the complete
// proposition grammar. The test asserts the composed bytes equal the real
// codec's output before relying on them.

fn push_count(bytes: &mut Vec<u8>, count: usize) {
    bytes.extend_from_slice(
        &u32::try_from(count)
            .expect("test rosters fit u32")
            .to_le_bytes(),
    );
}

fn encode_owner(bytes: &mut Vec<u8>, owner: &ReconstructedTerminalObligationOwner) {
    match *owner {
        ReconstructedTerminalObligationOwner::Operation { machine, operation } => {
            bytes.push(1);
            bytes.extend_from_slice(&machine.get().to_le_bytes());
            bytes.extend_from_slice(&operation.get().to_le_bytes());
        }
        ReconstructedTerminalObligationOwner::CallRequires {
            machine,
            operation,
            requirement_position,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&machine.get().to_le_bytes());
            bytes.extend_from_slice(&operation.get().to_le_bytes());
            bytes.extend_from_slice(&requirement_position.to_le_bytes());
        }
        ReconstructedTerminalObligationOwner::NominalCleanupRequires {
            machine,
            edge,
            cleanup_position,
            requirement_position,
        } => {
            bytes.push(3);
            bytes.extend_from_slice(&machine.get().to_le_bytes());
            bytes.extend_from_slice(&edge.get().to_le_bytes());
            bytes.extend_from_slice(&cleanup_position.to_le_bytes());
            bytes.extend_from_slice(&requirement_position.to_le_bytes());
        }
        ReconstructedTerminalObligationOwner::ContractEnsures {
            machine,
            contract,
            clause_position,
        } => {
            bytes.push(4);
            bytes.extend_from_slice(&machine.get().to_le_bytes());
            bytes.extend_from_slice(&contract.get().to_le_bytes());
            bytes.extend_from_slice(&clause_position.to_le_bytes());
        }
        ReconstructedTerminalObligationOwner::ScalarBlockInvariant {
            machine,
            header,
            edge,
        } => {
            bytes.push(5);
            bytes.extend_from_slice(&machine.get().to_le_bytes());
            bytes.extend_from_slice(&header.get().to_le_bytes());
            bytes.extend_from_slice(&edge.get().to_le_bytes());
        }
    }
}

fn encode_class(bytes: &mut Vec<u8>, class: &ObligationClass) {
    match *class {
        ObligationClass::Derivable => bytes.push(1),
        ObligationClass::AdmissionAuthorized(authorization) => {
            bytes.push(2);
            bytes.extend_from_slice(&authorization.site.get().to_le_bytes());
            bytes.push(match authorization.kind {
                AdmissionKind::ForeignBoundaryGuarantee => 1,
                AdmissionKind::ProviderFact => 2,
                AdmissionKind::CheckedAssemblyClaim => 3,
            });
            bytes.extend_from_slice(&authorization.authority_identity.get().to_le_bytes());
        }
    }
}

fn encode_row(row: &ReconstructedTerminalObligation) -> Vec<u8> {
    let mut bytes = Vec::new();
    encode_owner(&mut bytes, &row.owner);
    bytes.extend_from_slice(&row.obligation.id.get().to_le_bytes());
    encode_class(&mut bytes, &row.obligation.class);
    bytes.extend_from_slice(
        &canonical_proposition_order_key(&row.obligation.proposition)
            .expect("obligation proposition encodes canonically"),
    );
    push_count(&mut bytes, row.requirements.len());
    for requirement in &row.requirements {
        bytes.extend_from_slice(
            &canonical_proposition_order_key(requirement).expect("requirement encodes canonically"),
        );
    }
    push_count(&mut bytes, row.semantic_axioms.len());
    for axiom in &row.semantic_axioms {
        bytes.extend_from_slice(
            &canonical_proposition_order_key(axiom).expect("axiom encodes canonically"),
        );
    }
    bytes.push(u8::from(row.canonical_certificate));
    bytes
}

fn compose(header: &[u8], rows: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = header.to_vec();
    for row in rows {
        bytes.extend_from_slice(row);
    }
    bytes
}

/// Byte spans of every field inside one encoded ledger row, computed from the
/// decoded row so propositions keep their codec-owned canonical lengths.
struct RowSpans {
    row: Range<usize>,
    owner_tag: usize,
    owner_fields: Vec<Range<usize>>,
    id: Range<usize>,
    class_tag: usize,
    class_fields: Vec<Range<usize>>,
    proposition: Range<usize>,
    requirements_len: Range<usize>,
    certificate: usize,
}

fn row_spans(start: usize, row: &ReconstructedTerminalObligation) -> RowSpans {
    let owner_tag = start;
    let mut owner_fields = Vec::new();
    let mut cursor = start + 1;
    let field = |width: usize, cursor: &mut usize, fields: &mut Vec<Range<usize>>| {
        fields.push(*cursor..*cursor + width);
        *cursor += width;
    };
    match row.owner {
        ReconstructedTerminalObligationOwner::Operation { .. } => {
            field(8, &mut cursor, &mut owner_fields);
            field(8, &mut cursor, &mut owner_fields);
        }
        ReconstructedTerminalObligationOwner::CallRequires { .. } => {
            field(8, &mut cursor, &mut owner_fields);
            field(8, &mut cursor, &mut owner_fields);
            field(4, &mut cursor, &mut owner_fields);
        }
        ReconstructedTerminalObligationOwner::NominalCleanupRequires { .. } => {
            field(8, &mut cursor, &mut owner_fields);
            field(8, &mut cursor, &mut owner_fields);
            field(4, &mut cursor, &mut owner_fields);
            field(4, &mut cursor, &mut owner_fields);
        }
        ReconstructedTerminalObligationOwner::ContractEnsures { .. } => {
            field(8, &mut cursor, &mut owner_fields);
            field(8, &mut cursor, &mut owner_fields);
            field(4, &mut cursor, &mut owner_fields);
        }
        ReconstructedTerminalObligationOwner::ScalarBlockInvariant { .. } => {
            field(8, &mut cursor, &mut owner_fields);
            field(8, &mut cursor, &mut owner_fields);
            field(8, &mut cursor, &mut owner_fields);
        }
    }
    let id = cursor..cursor + 8;
    cursor += 8;
    let class_tag = cursor;
    cursor += 1;
    let mut class_fields = Vec::new();
    if let ObligationClass::AdmissionAuthorized(_) = row.obligation.class {
        field(8, &mut cursor, &mut class_fields);
        field(1, &mut cursor, &mut class_fields);
        field(8, &mut cursor, &mut class_fields);
    }
    let proposition_width = canonical_proposition_order_key(&row.obligation.proposition)
        .expect("obligation proposition encodes canonically")
        .len();
    let proposition = cursor..cursor + proposition_width;
    cursor += proposition_width;
    let requirements_len = cursor..cursor + 4;
    cursor += 4;
    for requirement in &row.requirements {
        cursor += canonical_proposition_order_key(requirement)
            .expect("requirement encodes canonically")
            .len();
    }
    cursor += 4;
    for axiom in &row.semantic_axioms {
        cursor += canonical_proposition_order_key(axiom)
            .expect("axiom encodes canonically")
            .len();
    }
    RowSpans {
        row: start..cursor + 1,
        owner_tag,
        owner_fields,
        id,
        class_tag,
        class_fields,
        proposition,
        requirements_len,
        certificate: cursor,
    }
}

#[test]
fn terminal_obligation_ledger_rejects_every_one_field_substitution() {
    let module = ledger_fixture();
    let trust_graph = current_terminal_trust_graph().expect("current trust graph");
    let ledger = build_terminal_obligation_ledger(&module, &trust_graph).expect("produced ledger");
    let encoded = encode_terminal_obligation_ledger(&ledger).expect("encode ledger");
    let fingerprint = terminal_obligation_ledger_fingerprint(&ledger).expect("fingerprint");

    assert_eq!(
        decode_terminal_obligation_ledger(&encoded),
        Ok(ledger.clone()),
        "the canonical ledger round-trips"
    );
    validate_terminal_obligation_ledger(&ledger, &module, &trust_graph)
        .expect("produced ledger replays");

    // The produced roster exercises three owner kinds, a non-empty requirement
    // roster on every row, reconstructed axioms on the call rows, and both
    // canonical-certificate values.
    let rows = ledger.obligations();
    assert_eq!(
        rows.len(),
        5,
        "fixture roster: operation, two call requirements, two ensures rows"
    );
    assert!(matches!(
        rows[0].owner,
        ReconstructedTerminalObligationOwner::Operation { .. }
    ));
    assert!(matches!(
        rows[1].owner,
        ReconstructedTerminalObligationOwner::CallRequires {
            requirement_position: 0,
            ..
        }
    ));
    assert!(matches!(
        rows[2].owner,
        ReconstructedTerminalObligationOwner::CallRequires {
            requirement_position: 1,
            ..
        }
    ));
    assert!(matches!(
        rows[3].owner,
        ReconstructedTerminalObligationOwner::ContractEnsures { .. }
    ));
    assert!(matches!(
        rows[4].owner,
        ReconstructedTerminalObligationOwner::ContractEnsures { .. }
    ));
    assert!(
        rows.iter().all(|row| !row.requirements.is_empty()),
        "every produced row carries the machine's requirements"
    );
    assert!(
        rows[1].semantic_axioms.len() > rows[0].semantic_axioms.len(),
        "the call row accumulates the exact-subtraction equation"
    );

    // Verify the local encoder reproduces the canonical bytes exactly, and
    // compute every row's span.
    let mut row_bytes: Vec<Vec<u8>> = Vec::new();
    let mut spans = Vec::new();
    let mut cursor = HEADER_LEN;
    for row in rows {
        let encoded_row = encode_row(row);
        spans.push(row_spans(cursor, row));
        cursor += encoded_row.len();
        row_bytes.push(encoded_row);
    }
    assert_eq!(cursor, encoded.len());
    assert_eq!(
        compose(&encoded[..HEADER_LEN], &row_bytes),
        encoded,
        "the test-local encoder reproduces the canonical ledger bytes"
    );
    let header = encoded[..HEADER_LEN].to_vec();
    let count = header.len() - 4..header.len();

    // Every substitution below lands in exactly one of two outcomes: the wire
    // form is no longer canonical and decoding rejects it outright, or it
    // decodes to a different ledger whose honestly recomputed fingerprint
    // diverges and whose independent reconstruction replay rejects it.
    let divergent = |name: &'static str, mutated: &[u8]| {
        let substituted = decode_terminal_obligation_ledger(mutated)
            .unwrap_or_else(|error| panic!("{name} must still decode: {error:?}"));
        assert_ne!(substituted, ledger, "{name} must change the ledger");
        assert_eq!(
            encode_terminal_obligation_ledger(&substituted).expect("re-encode"),
            mutated,
            "{name} must re-encode canonically"
        );
        assert_ne!(
            terminal_obligation_ledger_fingerprint(&substituted).expect("substituted fingerprint"),
            fingerprint,
            "{name} must diverge the honestly recomputed ledger fingerprint"
        );
        assert_eq!(
            validate_terminal_obligation_ledger(&substituted, &module, &trust_graph),
            Err(CodecError::ObligationLedgerMismatch),
            "{name} must reject at the reconstruction replay"
        );
        substituted
    };

    // Replace row `index` with `substitute` and keep the roster count honest.
    let with_row = |index: usize, substitute: &ReconstructedTerminalObligation| -> Vec<u8> {
        let mut spliced = row_bytes.clone();
        spliced[index] = encode_row(substitute);
        compose(&header, &spliced)
    };
    let mutated_row =
        |index: usize, patch: &dyn Fn(&mut ReconstructedTerminalObligation)| -> Vec<u8> {
            let mut row = rows[index].clone();
            patch(&mut row);
            with_row(index, &row)
        };

    // --- envelope axes ---------------------------------------------------

    let mut mutated = encoded.clone();
    mutated[0] ^= 0xFF;
    assert_eq!(
        decode_terminal_obligation_ledger(&mutated),
        Err(CodecError::InvalidMagic),
        "a corrupted magic must reject at decoding"
    );

    let mut mutated = encoded.clone();
    mutated[8..10].copy_from_slice(&u16::MAX.to_le_bytes());
    assert_eq!(
        decode_terminal_obligation_ledger(&mutated),
        Err(CodecError::UnsupportedFormatMarker(u16::MAX)),
        "a substituted format marker must reject at decoding"
    );

    let mut mutated = encoded.clone();
    mutated[10..12].copy_from_slice(&u16::MAX.to_le_bytes());
    assert_eq!(
        decode_terminal_obligation_ledger(&mutated),
        Err(CodecError::UnsupportedVocabularyMarker(u16::MAX)),
        "an unknown vocabulary marker must reject at decoding"
    );

    // The program fingerprint and trust graph identity are representable
    // fields: substitutions still decode but the recomputed fingerprint
    // diverges and replay rejects them.
    for (name, range) in [
        ("the program fingerprint", 12..44),
        ("the trust graph identity", 44..76),
    ] {
        let mut mutated = encoded.clone();
        mutated[range.start] ^= 0xFF;
        divergent(name, &mutated);
        let mut mutated = encoded.clone();
        mutated[range.clone()].copy_from_slice(&[0; 32]);
        divergent(name, &mutated);
    }

    // A roster count lying about its roster starves the tail or strands bytes.
    for (name, value, expected) in [
        (
            "a roster count one short",
            (rows.len() - 1) as u32,
            CodecError::TrailingBytes(row_bytes.last().expect("last row").len()),
        ),
        (
            "a roster count one over",
            (rows.len() + 1) as u32,
            CodecError::UnexpectedEnd,
        ),
        (
            "a maximal roster count",
            u32::MAX,
            CodecError::UnexpectedEnd,
        ),
    ] {
        let mut mutated = encoded.clone();
        mutated[count.clone()].copy_from_slice(&value.to_le_bytes());
        assert_eq!(
            decode_terminal_obligation_ledger(&mutated),
            Err(expected),
            "{name} must reject at decoding"
        );
    }

    // Truncation inside every envelope field and every row rejects.
    for cut in [
        7,
        9,
        11,
        43,
        75,
        HEADER_LEN - 1,
        spans[0].row.end - 1,
        spans[1].row.end - 1,
        spans[2].row.end - 1,
        spans[3].row.end - 1,
        encoded.len() - 1,
    ] {
        assert!(
            decode_terminal_obligation_ledger(&encoded[..cut]).is_err(),
            "truncation at byte {cut} must reject"
        );
    }
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(
        decode_terminal_obligation_ledger(&trailing),
        Err(CodecError::TrailingBytes(1)),
        "a trailing byte must reject at decoding"
    );

    // --- roster axes ------------------------------------------------------

    // Dropping a row with an honestly recomputed count still decodes to a
    // different ledger, and replay rejects the incomplete question.
    for (name, index) in [
        ("a dropped leading row", 0),
        ("a dropped trailing row", rows.len() - 1),
    ] {
        let mut spliced = row_bytes.clone();
        spliced.remove(index);
        let mut dropped_header = header.clone();
        dropped_header[count.clone()].copy_from_slice(&((rows.len() - 1) as u32).to_le_bytes());
        divergent(name, &compose(&dropped_header, &spliced));
    }

    // Reordering is representable (owners stay unique) but the roster order is
    // fingerprinted, so replay rejects the permuted ledger.
    let mut spliced = row_bytes.clone();
    spliced.swap(0, 1);
    divergent("a reordered roster", &compose(&header, &spliced));

    // An appended foreign row is representable; replay rejects the extended
    // question. A duplicated row or owner is not even canonical on the wire.
    let foreign = ReconstructedTerminalObligation {
        owner: ReconstructedTerminalObligationOwner::ScalarBlockInvariant {
            machine: machine_id(1),
            header: block_id(901),
            edge: edge_id(902),
        },
        obligation: proof_admission::Obligation {
            id: obligation_id(999),
            class: ObligationClass::Derivable,
            proposition: Proposition::Truth,
        },
        requirements: Vec::new(),
        semantic_axioms: Vec::new(),
        canonical_certificate: false,
    };
    let mut extended_header = header.clone();
    extended_header[count.clone()].copy_from_slice(&((rows.len() + 1) as u32).to_le_bytes());
    let mut spliced = row_bytes.clone();
    spliced.push(encode_row(&foreign));
    divergent(
        "an appended foreign obligation",
        &compose(&extended_header, &spliced),
    );

    let mut spliced = row_bytes.clone();
    spliced.push(row_bytes.last().expect("last row").clone());
    assert_eq!(
        decode_terminal_obligation_ledger(&compose(&extended_header, &spliced)),
        Err(CodecError::NonCanonicalOrder(
            "terminal obligation identities"
        )),
        "a duplicated obligation identity must reject at decoding"
    );
    let mut same_owner = rows.last().expect("last row").clone();
    same_owner.obligation.id = obligation_id(999);
    let mut spliced = row_bytes.clone();
    spliced.push(encode_row(&same_owner));
    assert_eq!(
        decode_terminal_obligation_ledger(&compose(&extended_header, &spliced)),
        Err(CodecError::NonCanonicalOrder("terminal obligation owners")),
        "a duplicated obligation owner must reject at decoding"
    );

    // --- owner fields ------------------------------------------------------

    // Operation owner: each field substitutes independently.
    divergent(
        "the operation owner machine",
        &mutated_row(0, &|row| {
            row.owner = ReconstructedTerminalObligationOwner::Operation {
                machine: machine_id(2),
                operation: operation_id(10),
            };
        }),
    );
    divergent(
        "the operation owner operation",
        &mutated_row(0, &|row| {
            row.owner = ReconstructedTerminalObligationOwner::Operation {
                machine: machine_id(1),
                operation: operation_id(77),
            };
        }),
    );

    // CallRequires owner: machine, operation, and requirement position.
    divergent(
        "the call-requirement owner machine",
        &mutated_row(1, &|row| {
            row.owner = ReconstructedTerminalObligationOwner::CallRequires {
                machine: machine_id(2),
                operation: operation_id(11),
                requirement_position: 0,
            };
        }),
    );
    divergent(
        "the call-requirement owner operation",
        &mutated_row(1, &|row| {
            row.owner = ReconstructedTerminalObligationOwner::CallRequires {
                machine: machine_id(1),
                operation: operation_id(77),
                requirement_position: 0,
            };
        }),
    );
    divergent(
        "the call-requirement requirement position",
        &mutated_row(1, &|row| {
            row.owner = ReconstructedTerminalObligationOwner::CallRequires {
                machine: machine_id(1),
                operation: operation_id(11),
                requirement_position: 7,
            };
        }),
    );

    // ContractEnsures owner: machine, contract, and clause position.
    divergent(
        "the ensures owner machine",
        &mutated_row(3, &|row| {
            row.owner = ReconstructedTerminalObligationOwner::ContractEnsures {
                machine: machine_id(2),
                contract: contract_id(1),
                clause_position: 0,
            };
        }),
    );
    divergent(
        "the ensures owner contract",
        &mutated_row(3, &|row| {
            row.owner = ReconstructedTerminalObligationOwner::ContractEnsures {
                machine: machine_id(1),
                contract: contract_id(77),
                clause_position: 0,
            };
        }),
    );
    divergent(
        "the ensures owner clause position",
        &mutated_row(3, &|row| {
            row.owner = ReconstructedTerminalObligationOwner::ContractEnsures {
                machine: machine_id(1),
                contract: contract_id(1),
                clause_position: 9,
            };
        }),
    );

    // Owner-kind substitutions: every remaining variant is representable on a
    // produced row, and each of its fields binds independently.
    for (name, owner) in [
        (
            "a call-requirement owner on the operation row",
            ReconstructedTerminalObligationOwner::CallRequires {
                machine: machine_id(1),
                operation: operation_id(77),
                requirement_position: 0,
            },
        ),
        (
            "a nominal-cleanup owner on the operation row",
            ReconstructedTerminalObligationOwner::NominalCleanupRequires {
                machine: machine_id(1),
                edge: edge_id(55),
                cleanup_position: 2,
                requirement_position: 3,
            },
        ),
        (
            "an ensures owner on the operation row",
            ReconstructedTerminalObligationOwner::ContractEnsures {
                machine: machine_id(1),
                contract: contract_id(77),
                clause_position: 4,
            },
        ),
        (
            "a block-invariant owner on the operation row",
            ReconstructedTerminalObligationOwner::ScalarBlockInvariant {
                machine: machine_id(1),
                header: block_id(88),
                edge: edge_id(89),
            },
        ),
    ] {
        divergent(
            name,
            &mutated_row(0, &|row| {
                row.owner = owner;
            }),
        );
    }

    // Each field of the unproduced owner kinds binds independently.
    for (name, owner) in [
        (
            "the block-invariant owner machine",
            ReconstructedTerminalObligationOwner::ScalarBlockInvariant {
                machine: machine_id(2),
                header: block_id(88),
                edge: edge_id(89),
            },
        ),
        (
            "the block-invariant owner header",
            ReconstructedTerminalObligationOwner::ScalarBlockInvariant {
                machine: machine_id(1),
                header: block_id(90),
                edge: edge_id(89),
            },
        ),
        (
            "the block-invariant owner edge",
            ReconstructedTerminalObligationOwner::ScalarBlockInvariant {
                machine: machine_id(1),
                header: block_id(88),
                edge: edge_id(91),
            },
        ),
        (
            "the cleanup owner machine",
            ReconstructedTerminalObligationOwner::NominalCleanupRequires {
                machine: machine_id(2),
                edge: edge_id(55),
                cleanup_position: 2,
                requirement_position: 3,
            },
        ),
        (
            "the cleanup owner edge",
            ReconstructedTerminalObligationOwner::NominalCleanupRequires {
                machine: machine_id(1),
                edge: edge_id(56),
                cleanup_position: 2,
                requirement_position: 3,
            },
        ),
        (
            "the cleanup owner cleanup position",
            ReconstructedTerminalObligationOwner::NominalCleanupRequires {
                machine: machine_id(1),
                edge: edge_id(55),
                cleanup_position: 7,
                requirement_position: 3,
            },
        ),
        (
            "the cleanup owner requirement position",
            ReconstructedTerminalObligationOwner::NominalCleanupRequires {
                machine: machine_id(1),
                edge: edge_id(55),
                cleanup_position: 2,
                requirement_position: 8,
            },
        ),
    ] {
        divergent(
            name,
            &mutated_row(0, &|row| {
                row.owner = owner;
            }),
        );
    }

    // Owner identity fields are nonzero and the owner tag is closed.
    for (name, offset, expected) in [
        (
            "a zero owner machine",
            spans[0].owner_fields[0].start,
            CodecError::ZeroIdentity("MachineId"),
        ),
        (
            "a zero owner operation",
            spans[0].owner_fields[1].start,
            CodecError::ZeroIdentity("OperationId"),
        ),
    ] {
        let mut mutated = encoded.clone();
        mutated[offset..offset + 8].copy_from_slice(&0_u64.to_le_bytes());
        assert_eq!(
            decode_terminal_obligation_ledger(&mutated),
            Err(expected),
            "{name} must reject at decoding"
        );
    }
    let mut mutated = encoded.clone();
    mutated[spans[0].owner_tag] = 9;
    assert_eq!(
        decode_terminal_obligation_ledger(&mutated),
        Err(CodecError::InvalidTag("TerminalObligationOwner", 9)),
        "an unknown owner tag must reject at decoding"
    );

    // --- obligation identity ------------------------------------------------

    divergent(
        "the obligation identity",
        &mutated_row(0, &|row| {
            row.obligation.id = obligation_id(999);
        }),
    );
    // A colliding identity is not canonical on the wire.
    let colliding = {
        let mut row = rows[0].clone();
        row.obligation.id = rows[1].obligation.id;
        row
    };
    assert_eq!(
        decode_terminal_obligation_ledger(&with_row(0, &colliding)),
        Err(CodecError::NonCanonicalOrder(
            "terminal obligation identities"
        )),
        "a colliding obligation identity must reject at decoding"
    );
    let mut mutated = encoded.clone();
    mutated[spans[0].id.clone()].copy_from_slice(&0_u64.to_le_bytes());
    assert_eq!(
        decode_terminal_obligation_ledger(&mutated),
        Err(CodecError::ZeroIdentity("ObligationId")),
        "a zero obligation identity must reject at decoding"
    );

    // --- obligation class -----------------------------------------------------

    // The produced roster is all derivable; an admission-authorized class is
    // representable and every authorized field binds independently.
    let authorized = |site, kind, authority| {
        ObligationClass::AdmissionAuthorized(AuthorizedAdmission {
            site,
            kind,
            authority_identity: authority,
        })
    };
    for (name, class) in [
        (
            "an admission-authorized class",
            authorized(
                AdmissionSiteId::new(77).expect("site"),
                AdmissionKind::ForeignBoundaryGuarantee,
                evidence_id(88),
            ),
        ),
        (
            "the admission site",
            authorized(
                AdmissionSiteId::new(78).expect("site"),
                AdmissionKind::ForeignBoundaryGuarantee,
                evidence_id(88),
            ),
        ),
        (
            "the admission kind",
            authorized(
                AdmissionSiteId::new(77).expect("site"),
                AdmissionKind::ProviderFact,
                evidence_id(88),
            ),
        ),
        (
            "the checked-assembly admission kind",
            authorized(
                AdmissionSiteId::new(77).expect("site"),
                AdmissionKind::CheckedAssemblyClaim,
                evidence_id(88),
            ),
        ),
        (
            "the admission authority",
            authorized(
                AdmissionSiteId::new(77).expect("site"),
                AdmissionKind::ForeignBoundaryGuarantee,
                evidence_id(89),
            ),
        ),
    ] {
        divergent(
            name,
            &mutated_row(0, &|row| {
                row.obligation.class = class;
            }),
        );
    }

    // Noncanonical class encodings reject at decoding.
    let mut mutated = encoded.clone();
    mutated[spans[0].class_tag] = 9;
    assert_eq!(
        decode_terminal_obligation_ledger(&mutated),
        Err(CodecError::InvalidTag("ObligationClass", 9)),
        "an unknown class tag must reject at decoding"
    );
    let mut authorized_row = rows[0].clone();
    authorized_row.obligation.class = authorized(
        AdmissionSiteId::new(77).expect("site"),
        AdmissionKind::ProviderFact,
        evidence_id(88),
    );
    let authorized_spans = row_spans(HEADER_LEN, &authorized_row);
    let authorized_bytes = with_row(0, &authorized_row);
    for (name, offset, expected) in [
        (
            "a zero admission site",
            authorized_spans.class_fields[0].start,
            CodecError::ZeroIdentity("AdmissionSiteId"),
        ),
        (
            "an unknown admission kind",
            authorized_spans.class_fields[1].start,
            CodecError::InvalidTag("AdmissionKind", 9),
        ),
        (
            "a zero admission authority",
            authorized_spans.class_fields[2].start,
            CodecError::ZeroIdentity("EvidenceIdentity"),
        ),
    ] {
        let mut mutated = authorized_bytes.clone();
        match expected {
            CodecError::InvalidTag(_, _) => mutated[offset] = 9,
            _ => mutated[offset..offset + 8].copy_from_slice(&0_u64.to_le_bytes()),
        }
        assert_eq!(
            decode_terminal_obligation_ledger(&mutated),
            Err(expected),
            "{name} must reject at decoding"
        );
    }

    // --- proposition ------------------------------------------------------------

    // Substituting the goal proposition is representable and replay-bound.
    for (name, index, proposition) in [
        ("the operation goal", 0, Proposition::Truth),
        (
            "the call-requirement goal",
            1,
            Proposition::LessOrEqual(
                ScalarTerm::value(
                    value_id(11),
                    ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 8).expect("i8")),
                ),
                ScalarTerm::integer(
                    IntegerType::new(IntegerSign::Signed, 8).expect("i8"),
                    IntegerValue::Signed(0),
                )
                .expect("literal"),
            ),
        ),
        (
            "the ensures goal",
            3,
            Proposition::Equal(
                ScalarTerm::integer(
                    IntegerType::new(IntegerSign::Signed, 8).expect("i8"),
                    IntegerValue::Signed(8),
                )
                .expect("literal"),
                ScalarTerm::integer(
                    IntegerType::new(IntegerSign::Signed, 8).expect("i8"),
                    IntegerValue::Signed(8),
                )
                .expect("literal"),
            ),
        ),
    ] {
        divergent(
            name,
            &mutated_row(index, &|row| {
                row.obligation.proposition = proposition.clone();
            }),
        );
    }
    // An unknown proposition tag is not canonical on the wire.
    let mut mutated = encoded.clone();
    mutated[spans[0].proposition.start] = 0xFE;
    assert_eq!(
        decode_terminal_obligation_ledger(&mutated),
        Err(CodecError::InvalidTag("Proposition", 0xFE)),
        "an unknown proposition tag must reject at decoding"
    );

    // --- requirements roster -------------------------------------------------

    // Every caller row carries two requirements; substitute, drop, extend, and
    // reorder that roster on the produced rows.
    divergent(
        "a substituted requirement member",
        &mutated_row(0, &|row| {
            row.requirements[1] = Proposition::Truth;
        }),
    );
    divergent(
        "a dropped requirement member",
        &mutated_row(0, &|row| {
            row.requirements.pop();
        }),
    );
    divergent(
        "an extended requirement roster",
        &mutated_row(0, &|row| {
            row.requirements.push(Proposition::Falsehood);
        }),
    );
    divergent(
        "a reordered requirement roster",
        &mutated_row(0, &|row| {
            row.requirements.swap(0, 1);
        }),
    );
    // A requirement count lying about its members rejects at decoding.
    for (name, value) in [
        (
            "an overcounted requirement roster",
            (rows[0].requirements.len() as u32) + 1,
        ),
        (
            "an undercounted requirement roster",
            (rows[0].requirements.len() as u32) - 1,
        ),
    ] {
        let mut mutated = encoded.clone();
        mutated[spans[0].requirements_len.clone()].copy_from_slice(&value.to_le_bytes());
        assert!(
            decode_terminal_obligation_ledger(&mutated).is_err(),
            "{name} must reject at decoding"
        );
    }

    // --- semantic axiom roster ------------------------------------------------

    // The call rows accumulate axioms from the earlier exact operation;
    // substitute and drop members there, and extend the operation row's empty
    // roster.
    assert!(
        !rows[1].semantic_axioms.is_empty(),
        "the first call row carries reconstructed axioms"
    );
    divergent(
        "a substituted axiom member",
        &mutated_row(1, &|row| {
            let last = row.semantic_axioms.len() - 1;
            row.semantic_axioms[last] = Proposition::Truth;
        }),
    );
    divergent(
        "a dropped axiom member",
        &mutated_row(1, &|row| {
            row.semantic_axioms.pop();
        }),
    );
    divergent(
        "an extended axiom roster",
        &mutated_row(0, &|row| {
            row.semantic_axioms.push(Proposition::Truth);
        }),
    );

    // --- canonical-certificate flag --------------------------------------------

    divergent(
        "the canonical-certificate flag",
        &mutated_row(0, &|row| {
            row.canonical_certificate = !row.canonical_certificate;
        }),
    );
    let mut mutated = encoded.clone();
    mutated[spans[0].certificate] = 2;
    assert_eq!(
        decode_terminal_obligation_ledger(&mutated),
        Err(CodecError::InvalidBoolean(2)),
        "a noncanonical certificate flag must reject at decoding"
    );
}
