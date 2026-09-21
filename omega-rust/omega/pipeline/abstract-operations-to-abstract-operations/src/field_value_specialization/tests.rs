//! Optimizer module role: test leaf. Proven field-value specialization proposal, replay, and custody evidence.

use super::super::VerifiedPsiOptimizationSession;
use crate::{
    FieldValueSpecializationCandidate, FieldValueSpecializationError,
    apply_field_value_specialization, propose_field_value_specializations,
    validate_field_value_specialization,
};
use abstract_operations::AbstractOperation;
use optimization_unit::{
    FoldedFieldValue, NodeLocation, ProvenanceDisposition, PsiOptimizationUnit, PsiProvenance,
    PsiRealizationSite, recompute_psi_optimization_unit_identity,
};
use semantic_vocabulary::{MachineId, PlaceId, StructuralPlaceKind};

/// A scalar machine establishes `Point` once with constant initializers and
/// reads both fields: `p.x` folds to `37` and `p.flag` folds to `true` in one
/// candidate under the `EstablishRecord` basis.
const ESTABLISHED_FIELDS_SOURCE: &str = r#"
    data Point { x: u32; flag: bool; }
    machine probe() -> bool {
        let p: Point = Point { x: 37, flag: true };
        (p.x == 37) == p.flag
    }
"#;

/// A field read on a machine parameter place over unbounded carriers carries
/// no establishment proof and no declared bound: the parameter arrives with
/// whatever value the caller supplied, so nothing may specialize.
const PARAMETER_FIELD_SOURCE: &str = r#"
    data Point { x: u32; flag: bool; }
    machine probe(p: Point) -> u32 {
        p.x
    }
"#;

/// A field read on a machine parameter place whose declared field bound
/// closes over exactly one value: the declared `BoundedInteger` singleton
/// proves the stored value with no producer at all.
const BOUNDED_PARAMETER_SOURCE: &str = r#"
    data Box { v: i32 [5..=5]; }
    machine probe(b: Box) -> i32 {
        b.v
    }
"#;

/// A nested path descends through a parameter's record field to a bounded
/// leaf: the declared singleton at the resolved position proves the read at
/// depth even though the root is a plain record.
const NESTED_BOUNDED_SOURCE: &str = r#"
    data Inner { v: i32 [7..=7]; }
    data Outer { inner: Inner; }
    machine probe(o: Outer) -> i32 {
        o.inner.v
    }
"#;

/// A local record establishment whose nested field is itself structural:
/// the `EstablishRecord` proves only its own level's initializers, so the
/// read below the structural child stays an observation.
const NESTED_UNPROVEN_SOURCE: &str = r#"
    data Inner { v: u32; }
    data Outer { inner: Inner; }
    machine probe() -> u32 {
        let o: Outer = Outer { inner: Inner { v: 7 } };
        o.inner.v
    }
"#;

/// No field read observes the established place at all: no candidate exists.
const NO_READS_SOURCE: &str = r#"
    data Point { x: u32; flag: bool; }
    machine probe() -> u32 {
        let p: Point = Point { x: 37, flag: true };
        3
    }
"#;

#[test]
fn established_record_fields_fold_to_proven_values() {
    let session = lowered_session_entry(
        ESTABLISHED_FIELDS_SOURCE,
        "established field values",
        "probe",
    );
    let unit = session.unit().clone();
    let machine = unit.functions[0].machine;
    let (place, producer) = established_record_place(&unit, machine);

    let candidates = propose_field_value_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("exactly one specialization candidate")
    };
    assert_eq!(candidate.machine(), machine);
    assert_eq!(candidate.place(), place);
    assert_eq!(candidate.producer(), Some(producer));
    assert_eq!(candidate.input(), unit.identity);
    assert_ne!(candidate.output(), unit.identity);
    let [first, second] = candidate.reads() else {
        panic!("two folded field reads")
    };
    assert_eq!(first.source(), place);
    assert_eq!(second.source(), place);
    assert_eq!(first.producer(), Some(producer));
    assert_eq!(second.producer(), Some(producer));
    let mut values = [first.value(), second.value()];
    values.sort();
    assert_eq!(
        values,
        [
            FoldedFieldValue::Boolean(true),
            FoldedFieldValue::Integer(semantic_vocabulary::IntegerValue::Unsigned(37)),
        ]
    );

    // The proposal is deterministic and the folded site identity is bound
    // into the candidate identity.
    let replayed = propose_field_value_specializations(&session, 4).expect("replay runs");
    assert_eq!(replayed, candidates);

    let validated =
        validate_field_value_specialization(&session, candidate).expect("independent replay");
    let applied = apply_field_value_specialization(session, validated).expect("apply");
    let next = applied.session();
    let output_function = next
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine retained");

    for row in candidate.reads() {
        let folded = &output_function
            .blocks
            .iter()
            .find(|block| block.id == row.site().block)
            .expect("block retained")
            .nodes[usize::try_from(row.site().node).expect("index")];
        match row.value() {
            FoldedFieldValue::Boolean(constant) => {
                let AbstractOperation::BooleanConstant {
                    psi_operation,
                    result,
                    value,
                } = &folded.operation
                else {
                    panic!("boolean read folds to BooleanConstant")
                };
                assert_eq!(*psi_operation, row.psi_operation());
                assert_eq!(*result, row.result());
                assert_eq!(*value, constant);
            }
            FoldedFieldValue::Integer(constant) => {
                let AbstractOperation::IntegerConstant {
                    psi_operation,
                    result,
                    value,
                    ..
                } = &folded.operation
                else {
                    panic!("integer read folds to IntegerConstant")
                };
                assert_eq!(*psi_operation, row.psi_operation());
                assert_eq!(*result, row.result());
                assert_eq!(*value, constant);
            }
        }
        assert_eq!(
            folded.provenance,
            vec![PsiProvenance::Operation(row.psi_operation())]
        );
        let input_node = &unit
            .functions
            .iter()
            .find(|function| function.machine == machine)
            .expect("input machine")
            .blocks
            .iter()
            .find(|block| block.id == row.site().block)
            .expect("input block")
            .nodes[usize::try_from(row.site().node).expect("index")];
        assert_eq!(folded.definitions, input_node.definitions);
        assert_eq!(folded.uses, input_node.uses);
        assert_eq!(folded.successors, input_node.successors);
    }

    // The ledger records each folded site's retained custody.
    let [record] = applied.ledger().records() else {
        panic!("one transformation record")
    };
    assert_eq!(record.input, unit.identity);
    assert_eq!(record.output, next.unit().identity);
    assert_eq!(record.provenance.len(), 2);
    for row in candidate.reads() {
        let site = PsiRealizationSite::Node(row.site());
        assert!(record.provenance.iter().any(|rewrite| {
            rewrite.input == site
                && rewrite.disposition == ProvenanceDisposition::RealizedAt(site)
                && rewrite.sources == vec![PsiProvenance::Operation(row.psi_operation())]
        }));
    }

    // The applied session is an exact fixed point for this family.
    assert!(
        propose_field_value_specializations(applied.session(), 4)
            .expect("fixed-point proposal runs")
            .is_empty(),
        "the specialization reaches a fixed point"
    );
}

#[test]
fn parameter_field_read_yields_no_candidate() {
    let session = lowered_session_entry(PARAMETER_FIELD_SOURCE, "parameter decline", "probe");
    let unit = session.unit();
    let machine = unit.functions[0].machine;
    let function = &unit.functions[0];
    let place = function
        .structural_places
        .iter()
        .find(|declaration| matches!(declaration.kind, StructuralPlaceKind::Parameter { .. }))
        .expect("parameter place exists")
        .id;
    assert!(
        field_read_on(unit, machine, place).is_some(),
        "the fixture must actually contain a field read"
    );
    assert!(
        propose_field_value_specializations(&session, 4)
            .expect("proposal runs")
            .is_empty()
    );
}

#[test]
fn bounded_parameter_field_folds_without_producer() {
    let session = lowered_session_entry(BOUNDED_PARAMETER_SOURCE, "bounded parameter", "probe");
    let unit = session.unit().clone();
    let machine = unit.functions[0].machine;
    let function = &unit.functions[0];
    let place = function
        .structural_places
        .iter()
        .find(|declaration| matches!(declaration.kind, StructuralPlaceKind::Parameter { .. }))
        .expect("parameter place exists")
        .id;
    let (site, read) = field_read_on(&unit, machine, place).expect("field read exists");

    let candidates = propose_field_value_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("exactly one specialization candidate")
    };
    assert_eq!(candidate.machine(), machine);
    assert_eq!(candidate.place(), place);
    assert_eq!(candidate.producer(), None);
    let [row] = candidate.reads() else {
        panic!("one folded field read")
    };
    assert_eq!(row.site(), site);
    assert_eq!(row.psi_operation(), read.0);
    assert_eq!(row.source(), place);
    assert_eq!(row.producer(), None);
    assert_eq!(
        row.value(),
        FoldedFieldValue::Integer(semantic_vocabulary::IntegerValue::Signed(5))
    );

    let validated =
        validate_field_value_specialization(&session, candidate).expect("independent replay");
    let applied = apply_field_value_specialization(session, validated).expect("apply");
    let folded = &applied.session().unit().functions[0]
        .blocks
        .iter()
        .find(|block| block.id == site.block)
        .expect("block retained")
        .nodes[usize::try_from(site.node).expect("index")];
    assert!(matches!(
        folded.operation,
        AbstractOperation::IntegerConstant {
            value: semantic_vocabulary::IntegerValue::Signed(5),
            ..
        }
    ));
    assert!(
        propose_field_value_specializations(applied.session(), 4)
            .expect("fixed-point proposal runs")
            .is_empty(),
        "the specialization reaches a fixed point"
    );
}

#[test]
fn nested_bounded_field_folds_at_path_depth() {
    let session = lowered_session_entry(NESTED_BOUNDED_SOURCE, "nested bounded field", "probe");
    let unit = session.unit().clone();
    let machine = unit.functions[0].machine;
    let function = &unit.functions[0];
    let place = function
        .structural_places
        .iter()
        .find(|declaration| matches!(declaration.kind, StructuralPlaceKind::Parameter { .. }))
        .expect("parameter place exists")
        .id;
    let (_, read) = field_read_on(&unit, machine, place).expect("field read exists");

    let candidates = propose_field_value_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("exactly one specialization candidate")
    };
    let [row] = candidate.reads() else {
        panic!("one folded field read")
    };
    assert_eq!(row.producer(), None);
    assert_eq!(row.path().len(), 1, "the read descends one record field");
    assert_eq!(row.field(), read.2);
    assert_eq!(
        row.value(),
        FoldedFieldValue::Integer(semantic_vocabulary::IntegerValue::Signed(7))
    );
}

#[test]
fn nested_structural_child_stays_unproven() {
    let session = lowered_session_entry(NESTED_UNPROVEN_SOURCE, "nested unproven field", "probe");
    let unit = session.unit();
    let machine = unit.functions[0].machine;
    let field_read_count = unit
        .functions
        .iter()
        .filter(|function| function.machine == machine)
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.nodes)
        .filter(|node| {
            matches!(
                node.operation,
                AbstractOperation::IntegerStructuralField { .. }
                    | AbstractOperation::BooleanStructuralField { .. }
            )
        })
        .count();
    assert!(
        field_read_count > 0,
        "the fixture must actually contain a field read"
    );
    assert!(
        propose_field_value_specializations(&session, 4)
            .expect("proposal runs")
            .is_empty(),
        "the EstablishRecord proves only its own level's initializers"
    );
}

#[test]
fn cyclic_machine_field_read_stays_frozen() {
    let session = cyclic_field_session();
    assert!(
        !session.cycle_components().components().is_empty(),
        "the fixture carries an authenticated cyclic component"
    );
    let field_read_count = session
        .unit()
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.nodes)
        .filter(|node| {
            matches!(
                node.operation,
                AbstractOperation::IntegerStructuralField { .. }
                    | AbstractOperation::BooleanStructuralField { .. }
            )
        })
        .count();
    assert!(
        field_read_count > 0,
        "the fixture must actually contain a field read to freeze"
    );
    assert!(
        propose_field_value_specializations(&session, 4)
            .expect("proposal runs")
            .is_empty(),
        "no field read inside frozen territory specializes"
    );
}

#[test]
fn unobserved_establishment_yields_no_candidate() {
    let session = lowered_session_entry(NO_READS_SOURCE, "no-reads decline", "probe");
    assert!(
        propose_field_value_specializations(&session, 4)
            .expect("proposal runs")
            .is_empty()
    );
    let unit = session.unit();
    let machine = unit.functions[0].machine;
    let (place, producer) = established_record_place(unit, machine);
    let candidate = FieldValueSpecializationCandidate {
        identity: optimization_core::OptimizationCandidateIdentity::from_canonical_bytes(
            b"forged-unobserved-candidate",
        ),
        input: unit.identity,
        output: unit.identity,
        machine,
        place,
        producer: Some(producer),
        reads: Vec::new(),
    };
    assert_eq!(
        validate_field_value_specialization(&session, &candidate).err(),
        Some(FieldValueSpecializationError::AlreadySpecialized)
    );
}

#[test]
fn replay_rejects_forged_field_rows() {
    let session = lowered_session_entry(
        ESTABLISHED_FIELDS_SOURCE,
        "established field values",
        "probe",
    );
    let candidates = propose_field_value_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("one specialization candidate")
    };

    // A forged folded value.
    let mut forged = candidate.clone();
    forged.reads[0].value =
        FoldedFieldValue::Integer(semantic_vocabulary::IntegerValue::Unsigned(41));
    assert_eq!(
        validate_field_value_specialization(&session, &forged).err(),
        Some(FieldValueSpecializationError::CandidateMismatch)
    );

    // A forged field identity — a field no source operation carries.
    let mut forged = candidate.clone();
    forged.reads[0].field =
        semantic_vocabulary::StructuralFieldId::new(forged.reads[0].field.get() + 7)
            .expect("forged field identity");
    assert_eq!(
        validate_field_value_specialization(&session, &forged).err(),
        Some(FieldValueSpecializationError::CandidateMismatch)
    );

    // A forged path segment.
    let mut forged = candidate.clone();
    forged.reads[0].path = vec![semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(0)];
    assert_eq!(
        validate_field_value_specialization(&session, &forged).err(),
        Some(FieldValueSpecializationError::CandidateMismatch)
    );

    // A forged site coordinate.
    let mut forged = candidate.clone();
    forged.reads[0].site.node += 1;
    assert_eq!(
        validate_field_value_specialization(&session, &forged).err(),
        Some(FieldValueSpecializationError::CandidateMismatch)
    );

    // A forged producer identity.
    let mut forged = candidate.clone();
    forged.producer = Some(forged.reads[0].psi_operation);
    assert_eq!(
        validate_field_value_specialization(&session, &forged).err(),
        Some(FieldValueSpecializationError::CandidateMismatch)
    );

    // A forged candidate identity.
    let mut forged = candidate.clone();
    forged.identity =
        optimization_core::OptimizationCandidateIdentity::from_canonical_bytes(b"forged-identity");
    assert_eq!(
        validate_field_value_specialization(&session, &forged).err(),
        Some(FieldValueSpecializationError::CandidateMismatch)
    );

    // A forged output revision.
    let mut forged = candidate.clone();
    forged.output =
        optimization_core::OptimizationUnitIdentity::from_canonical_bytes(b"forged-output");
    assert_eq!(
        validate_field_value_specialization(&session, &forged).err(),
        Some(FieldValueSpecializationError::CandidateMismatch)
    );

    // The untampered candidate still validates.
    assert!(
        validate_field_value_specialization(&session, candidate).is_ok(),
        "the exact candidate still validates"
    );
}

#[test]
fn replay_rejects_stale_candidate_revision() {
    let session = lowered_session_entry(
        ESTABLISHED_FIELDS_SOURCE,
        "established field values",
        "probe",
    );
    let candidates = propose_field_value_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("one specialization candidate")
    };

    let mut stale = candidate.clone();
    stale.input = optimization_core::OptimizationUnitIdentity::from_canonical_bytes(b"stale-input");
    assert_eq!(
        validate_field_value_specialization(&session, &stale).err(),
        Some(FieldValueSpecializationError::StaleCandidateRevision {
            candidate: stale.input,
            current: session.unit().identity,
        })
    );

    // Applying moves the revision; the original candidate is stale afterward.
    let validated =
        validate_field_value_specialization(&session, candidate).expect("independent replay");
    let applied = apply_field_value_specialization(session, validated).expect("apply");
    assert_eq!(
        validate_field_value_specialization(applied.session(), candidate).err(),
        Some(FieldValueSpecializationError::StaleCandidateRevision {
            candidate: candidate.input(),
            current: applied.session().unit().identity,
        })
    );
}

#[test]
fn candidate_budget_is_exact() {
    let session = lowered_session_entry(
        ESTABLISHED_FIELDS_SOURCE,
        "established field values",
        "probe",
    );
    assert_eq!(
        propose_field_value_specializations(&session, 0).err(),
        Some(FieldValueSpecializationError::CandidateBudgetExhausted {
            required: 1,
            limit: 0,
        })
    );
    assert_eq!(
        propose_field_value_specializations(&session, 1)
            .expect("proposal runs")
            .len(),
        1
    );
}

#[test]
fn transformed_replay_rejects_forged_folded_custody() {
    let session = lowered_session_entry(
        ESTABLISHED_FIELDS_SOURCE,
        "established field values",
        "probe",
    );
    let candidates = propose_field_value_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("one specialization candidate")
    };
    let row = &candidate.reads()[0];
    let verified_input = session.input().clone();
    let validated =
        validate_field_value_specialization(&session, candidate).expect("independent replay");

    // A unit whose folded node claims a different custody source is not the
    // specialization this candidate pins: replay rebuilds the plan's own
    // output and the forged revision identity mismatches.
    let mut corrupted = validated.output.clone();
    let folded = corrupted
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .flat_map(|block| &mut block.nodes)
        .find(|node| {
            matches!(
                node.operation,
                AbstractOperation::BooleanConstant { psi_operation, .. }
                    | AbstractOperation::IntegerConstant { psi_operation, .. }
                    if psi_operation == row.psi_operation()
            )
        })
        .expect("folded node exists");
    assert_eq!(
        folded.provenance,
        vec![PsiProvenance::Operation(row.psi_operation())]
    );
    folded.provenance[0] = PsiProvenance::Operation(row.producer().expect("establishment basis"));
    folded.fuel[0].site = PsiProvenance::Operation(row.producer().expect("establishment basis"));
    corrupted.identity = recompute_psi_optimization_unit_identity(&corrupted);
    let mut forged = candidate.clone();
    forged.output = corrupted.identity;
    assert_eq!(
        validate_field_value_specialization(&session, &forged).err(),
        Some(FieldValueSpecializationError::CandidateMismatch)
    );

    // Dropping the folded node's fuel settlement while keeping its custody
    // claim leaves a unit whose node settles fewer sources than it names:
    // transformed validation rejects the forged fuel/provenance pair.
    let mut malformed = validated.output.clone();
    let machine = candidate.machine();
    malformed
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .flat_map(|block| &mut block.nodes)
        .find(|node| {
            matches!(
                node.operation,
                AbstractOperation::BooleanConstant { psi_operation, .. }
                    | AbstractOperation::IntegerConstant { psi_operation, .. }
                    if psi_operation == row.psi_operation()
            )
        })
        .expect("folded node exists")
        .fuel
        .pop();
    malformed.identity = recompute_psi_optimization_unit_identity(&malformed);
    assert!(matches!(
        VerifiedPsiOptimizationSession::from_transformed(verified_input, malformed),
        Err(
            optimization_unit_semantics::OptimizationUnitValidationError::FuelDoesNotMatchProvenance { machine: rejected, .. }
        ) if rejected == machine
    ));
}

/// The `OperationResult` place whose producer is an `EstablishRecord`,
/// returning (place, producer operation).
fn established_record_place(
    unit: &PsiOptimizationUnit,
    machine: MachineId,
) -> (PlaceId, semantic_vocabulary::OperationId) {
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine exists");
    let (place, producer) = function
        .structural_places
        .iter()
        .find_map(|declaration| match declaration.kind {
            StructuralPlaceKind::OperationResult { producer, .. } => {
                Some((declaration.id, producer))
            }
            _ => None,
        })
        .expect("established place exists");
    function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find(|node| {
            matches!(
                node.operation,
                AbstractOperation::EstablishRecord { psi_operation, .. }
                    if psi_operation == producer
            )
        })
        .expect("producer is an established record");
    (place, producer)
}

/// The first scalar field read observing `place`, returning its node location
/// plus (custody identity, result value, field identity).
fn field_read_on(
    unit: &PsiOptimizationUnit,
    machine: MachineId,
    place: PlaceId,
) -> Option<(
    NodeLocation,
    (
        semantic_vocabulary::OperationId,
        semantic_vocabulary::ValueId,
        semantic_vocabulary::StructuralFieldId,
    ),
)> {
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)?;
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            let (psi_operation, result, source, field) = match &node.operation {
                AbstractOperation::BooleanStructuralField {
                    psi_operation,
                    result,
                    source,
                    field,
                    ..
                } => (*psi_operation, *result, *source, *field),
                AbstractOperation::IntegerStructuralField {
                    psi_operation,
                    result,
                    source,
                    field,
                    ..
                } => (*psi_operation, result.value, *source, *field),
                _ => continue,
            };
            if source == place {
                return Some((
                    NodeLocation {
                        machine,
                        block: block.id,
                        node: u32::try_from(node_index).expect("node index fits u32"),
                    },
                    (psi_operation, result, field),
                ));
            }
        }
    }
    None
}

fn lowered_session_entry(source: &str, label: &str, entry: &str) -> VerifiedPsiOptimizationSession {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap_or_else(|error| panic!("tokenize {label}: {error:?}"));
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens)
        .unwrap_or_else(|error| panic!("parse {label}: {error:?}"));
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap_or_else(|error| panic!("resolve {label}: {error:?}"));
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .unwrap_or_else(|error| panic!("type {label}: {error:?}"));
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .unwrap_or_else(|error| panic!("check {label}: {error:?}"));
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, entry)
        .unwrap_or_else(|error| panic!("lower {label}: {error:?}"));
    let input = terminal_psi_to_abstract_operations::lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &terminal_codec::encode_module(&lowered.semantic_module)
                .unwrap_or_else(|error| panic!("encode {label} semantics: {error:?}")),
            proof_bytes: &terminal_codec::encode_proof_section(
                &lowered.semantic_module,
                &lowered.proof_bundle,
            )
            .unwrap_or_else(|error| panic!("encode {label} proof: {error:?}")),
            obligation_ledger_bytes: None,
        },
        &proof_admission::AdmissionProfile::default(),
    )
    .map(|admitted| {
        admitted
            .into_optimization_artifact()
            .into_optimization_input()
    })
    .unwrap_or_else(|error| panic!("optimizer-only {label} admission: {error:?}"));
    let verified = terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap_or_else(|error| panic!("build {label} optimizer unit: {error:?}"));
    VerifiedPsiOptimizationSession::new(verified)
        .unwrap_or_else(|error| panic!("verified {label} session: {error:?}"))
}

/// A bound-proven field read inside an authenticated cyclic machine. Source
/// cannot express this shape — multi-state machines refuse structural
/// formals, and the verifier's unranked-cycle fence admits only
/// parameter-sourced structural work — so the Terminal module is built
/// directly: an unranked self-loop header observes its owned `Token`
/// parameter, and `Token`'s record field carries a declared `BoundedInteger`
/// singleton bound that proves the stored value with no producer at all.
fn cyclic_field_session() -> VerifiedPsiOptimizationSession {
    use semantic_vocabulary::{
        BlockId, BoundedIntegerType, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue,
        OperationId, ScalarType, StructuralFieldId, StructuralTypeId, ValueId,
    };
    use terminal_psi::{
        Block, MachineContract, Operation, OperationKind, OperationResult, StructuralAccess,
        StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
        StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralTypeDeclaration,
        StructuralTypeShape, SuccessorEdge, TerminalMachine, TerminalMachineResult, TerminalModule,
        Terminator, ValueDeclaration, VocabularyMarker,
    };

    let value = |raw| ValueId::new(raw).unwrap();
    let edge = |raw| EdgeId::new(raw).unwrap();
    let block = |raw| BlockId::new(raw).unwrap();
    let successor = |edge, target, arguments| SuccessorEdge {
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        edge,
        target,
        arguments,
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let unsigned_32 = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    let token = StructuralTypeId::new(601).unwrap();
    let field = StructuralFieldId::new(603).unwrap();
    let token_place = PlaceId::new(520).unwrap();
    let bound = BoundedIntegerType::new(
        unsigned_32,
        IntegerValue::Unsigned(7),
        IntegerValue::Unsigned(7),
    )
    .unwrap();

    let module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(501).unwrap(),
        structural_types: vec![StructuralTypeDeclaration {
            id: token,
            identity: "Token".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: field,
                    identity: "value".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::BoundedInteger(bound),
                }],
            },
        }],
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
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(501).unwrap(),
            attachment: None,
            parameters: vec![ValueDeclaration {
                qualifications: Default::default(),
                id: value(502),
                scalar_type: ScalarType::Boolean,
            }],
            structural_parameters: vec![StructuralParameterDeclaration {
                place: token_place,
                position: 0,
                is_self: false,
                structural_type: token,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::Owned,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }],
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: vec![StructuralPlaceDeclaration {
                id: token_place,
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            }],
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block(503),
            blocks: vec![
                Block {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block(503),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Jump {
                        erased_arguments: Vec::new(),
                        erased_proof_arguments: Vec::new(),
                        edge: edge(504),
                        target: block(505),
                        arguments: vec![value(502)],
                        structural_arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                        residual_affine_discards: Vec::new(),
                    },
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block(505),
                    parameters: vec![ValueDeclaration {
                        qualifications: Default::default(),
                        id: value(506),
                        scalar_type: ScalarType::Boolean,
                    }],
                    operations: vec![
                        Operation {
                            static_reach_binding: None,
                            suspension_crossing: None,
                            id: OperationId::new(507).unwrap(),
                            result: OperationResult::Scalar(ValueDeclaration {
                                qualifications: Default::default(),
                                id: value(508),
                                scalar_type: ScalarType::Integer(unsigned_32),
                            }),
                            kind: OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(7),
                            },
                        },
                        Operation {
                            static_reach_binding: None,
                            suspension_crossing: None,
                            id: OperationId::new(521).unwrap(),
                            result: OperationResult::Scalar(ValueDeclaration {
                                qualifications: Default::default(),
                                id: value(522),
                                scalar_type: ScalarType::Integer(unsigned_32),
                            }),
                            kind: OperationKind::IntegerStructuralField {
                                source: token_place,
                                path: Vec::new(),
                                field,
                            },
                        },
                    ],
                    terminator: Terminator::Conditional {
                        condition: value(506),
                        when_true: successor(edge(509), block(505), vec![value(506)]),
                        when_false: successor(edge(510), block(511), Vec::new()),
                    },
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block(511),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: edge(512),
                        trivial_affine_discards: Vec::new(),
                    },
                },
            ],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                id: ContractId::new(513).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let semantic = terminal_codec::encode_module(&module).unwrap();
    let proof =
        terminal_codec::encode_proof_section(&module, &terminal_verifier::ProofBundle::default())
            .unwrap();
    let input = terminal_psi_to_abstract_operations::lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &proof_admission::AdmissionProfile::default(),
    )
    .map(|admitted| {
        admitted
            .into_optimization_artifact()
            .into_optimization_input()
    })
    .unwrap();
    let verified = terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap();
    VerifiedPsiOptimizationSession::new(verified).expect("verified cyclic session")
}
