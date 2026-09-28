//! Optimizer module role: test leaf. Proven field-value specialization admission, realization, and custody evidence through the rule.
//!
//! Every fixture is driven through the one live route: the
//! `RepresentationSpecialization` pass runs `FieldValueSpecializationRule`
//! to its fixed point, the committed candidate carries the proposed plan,
//! the run's session carries the folded unit, and forged rows are replayed
//! against `validate_field_value_specialization_candidate` — the independent
//! validator the pass manager itself consults.

use crate::rules::FieldValueSpecializationRule;
use crate::{OptimizationRun, PsiOptimizationCommit, run_psi_pipeline};
use abstract_operations::AbstractOperation;
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use optimization_core::{
    Optimization, OptimizationSelections, OptimizationUnitIdentity, OptimizationWorkBudget,
};
use optimization_unit::{
    FieldValueResolution, FieldValueSpecializationRewrite, FoldedFieldValue, NodeLocation,
    PsiOptimizationUnit, PsiProvenance, PsiRewriteCandidate, PsiRewriteCandidateError,
    PsiRewritePatch,
};
use optimization_unit_semantics::{
    OptimizationUnitValidationError, validate_field_value_specialization_candidate,
};
use semantic_vocabulary::{MachineId, PlaceId, StructuralPlaceKind};
use terminal_psi_to_abstract_operations::VerifiedPsiOptimizationUnit;

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

/// A field read descending through a parameter's record field into an array
/// element: the indexed position resolves, but no basis can prove its stored
/// value — the parameter carries no establishment, and an `EstablishRecord`
/// descent never crosses a `FixedIndex` because an array element is not a
/// placed child. The read stays an observation.
const ARRAY_ELEMENT_FIELD_SOURCE: &str = r#"
    data Room { flag: bool; }
    data Level { rooms: [Room; 2]; }
    machine probe(l: Level) -> bool {
        l.rooms[1].flag
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

/// The same nested descent with a nonconstant leaf: the child's
/// `EstablishRecord` stores the `x` parameter into `v`, so `o.inner.v`
/// forwards the parameter to its compare use and retires.
const NESTED_FORWARDED_SOURCE: &str = r#"
    data Inner { v: u32; }
    data Outer { inner: Inner; }
    machine probe(x: u32) -> bool {
        let o: Outer = Outer { inner: Inner { v: x } };
        o.inner.v == 7
    }
"#;

/// A nested path whose child arrives as a call result: the record stores
/// the call's result place whole, but a call is not an establishing
/// producer — nothing proves the contents the callee returned, so the read
/// below it stays an observation.
const NESTED_UNESTABLISHED_CHILD_SOURCE: &str = r#"
    data Inner { v: u32; }
    data Outer { inner: Inner; }
    machine Inner::make() -> Inner {
        Inner { v: 9 }
    }
    machine probe() -> u32 {
        let o: Outer = Outer { inner: Inner::make() };
        o.inner.v
    }
"#;

/// A scalar machine establishes `Point` once with the nonconstant `x`
/// parameter and a constant flag: the `EstablishRecord` basis proves both
/// stored fields, so `p.x` forwards the parameter to its compare use and
/// retires while `p.flag` folds to `true` in one mixed candidate.
const FORWARDED_FIELD_SOURCE: &str = r#"
    data Point { x: u32; flag: bool; }
    machine probe(x: u32) -> bool {
        let p: Point = Point { x: x, flag: true };
        (p.x == 7) == p.flag
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

/// The bound parameter observed at a nested path: `o.inner.v` crosses the
/// edge binding, then descends the stored-whole `inner` child to the
/// child's own `EstablishRecord` — the same two-step proof a read on the
/// bound place itself would draw on.
const BOUND_NESTED_SOURCE: &str = r#"
    data Inner { v: u32; }
    data Outer { inner: Inner; }
    data Root {}
    machine Root::run() {
        let o: Outer = Outer { inner: Inner { v: 7 } };
        transition { _ -> read(o) }
        state read(o: Outer) {
            transition o.inner.v == 7 { true -> good() _ -> bad() }
        }
        state good() {}
        state bad() {}
    }
"#;

/// Both arms of the entry conditional deliver the same established place:
/// the binding is uniform across every incoming edge, so the read still
/// folds.
const UNIFORM_TWO_EDGE_SOURCE: &str = r#"
    data Point { x: u32; flag: bool; }
    data Root {}
    machine Root::run(flag: bool) {
        let p: Point = Point { x: 37, flag: true };
        transition flag { true -> read(p) _ -> read(p) }
        state read(p: Point) {
            transition p.x == 37 { true -> good() _ -> bad() }
        }
        state good() {}
        state bad() {}
    }
"#;

#[test]
fn array_element_field_read_yields_no_candidate() {
    let unit = lowered_unit_entry(
        ARRAY_ELEMENT_FIELD_SOURCE,
        "array element field read",
        "probe",
    );
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    let place = parameter_place(&input);
    assert!(
        field_read_on(&input, machine, place).is_some(),
        "the fixture must actually contain a field read"
    );
    // The read's path really crosses a fixed index — the decline must come
    // from the missing basis below an established child, not from a
    // trivially unresolvable path.
    let indexed = input.functions[0]
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .any(|node| {
            matches!(
                &node.operation,
                AbstractOperation::BooleanStructuralField { path, .. }
                    if matches!(
                        path.as_slice(),
                        [
                            semantic_vocabulary::CanonicalStructuralPathSegment::Field(_),
                            semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(1)
                        ]
                    )
            )
        });
    assert!(indexed, "the fixture's read crosses a fixed index");
    assert_declines(unit);
}

#[test]
fn bounded_parameter_field_folds_without_producer() {
    let unit = lowered_unit_entry(BOUNDED_PARAMETER_SOURCE, "bounded parameter", "probe");
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    let place = parameter_place(&input);
    let (site, read) = field_read_on(&input, machine, place).expect("field read exists");

    let run = specialize(unit);
    let (_, patch) = single_commit(&run);
    assert_eq!(patch.machine, machine);
    assert_eq!(patch.place, place);
    assert_eq!(patch.producer, None);
    let [row] = patch.reads.as_slice() else {
        panic!("one folded field read")
    };
    assert_eq!(row.site, site);
    assert_eq!(row.psi_operation, read.0);
    assert_eq!(row.source, place);
    assert_eq!(row.producer, None);
    assert_eq!(
        row.resolution,
        FieldValueResolution::Constant(FoldedFieldValue::Integer(
            semantic_vocabulary::IntegerValue::Signed(5)
        ))
    );

    let folded = &run.session().unit().functions[0]
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
    assert!(field_read_on(run.session().unit(), machine, place).is_none());
}

#[test]
fn replay_rejects_forged_forward_rows() {
    let unit = lowered_unit_entry(FORWARDED_FIELD_SOURCE, "forwarded field value", "probe");
    let input = unit.unit().clone();
    let run = specialize(unit);
    let (commit, patch) = single_commit(&run);
    let forward_index = patch
        .reads
        .iter()
        .position(|row| matches!(row.resolution, FieldValueResolution::Forward(_)))
        .expect("one forwarded read");

    // A forged initializer — replay re-derives the establishment's stored
    // scalar rather than trusting the claimed substitution.
    assert_rejects_rows(
        &input,
        forged(
            commit.declaration.rewrite().expect("rewrite declaration"),
            |patch| {
                let result = patch.reads[forward_index].result;
                let FieldValueResolution::Forward(forwarded) =
                    &mut patch.reads[forward_index].resolution
                else {
                    unreachable!()
                };
                forwarded.initializer = result;
            },
        ),
    );

    // A forged use roster — replay recomputes the complete covered set.
    assert_rejects_rows(
        &input,
        forged(
            commit.declaration.rewrite().expect("rewrite declaration"),
            |patch| {
                let FieldValueResolution::Forward(forwarded) =
                    &mut patch.reads[forward_index].resolution
                else {
                    unreachable!()
                };
                forwarded.uses.pop();
            },
        ),
    );

    // A forged constant claim on the forwarded read.
    assert_rejects_rows(
        &input,
        forged(
            commit.declaration.rewrite().expect("rewrite declaration"),
            |patch| {
                patch.reads[forward_index].resolution = FieldValueResolution::Constant(
                    FoldedFieldValue::Integer(semantic_vocabulary::IntegerValue::Unsigned(7)),
                );
            },
        ),
    );

    // The untampered declaration still validates to the committed output.
    let validated = validate_field_value_specialization_candidate(
        &input,
        commit.declaration.rewrite().expect("rewrite declaration"),
    )
    .expect("the exact candidate still validates");
    assert_eq!(validated.unit().identity, commit.output);
}

#[test]
fn nested_nonconstant_initializer_forwards_at_depth() {
    let unit = lowered_unit_entry(NESTED_FORWARDED_SOURCE, "nested forwarded field", "probe");
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    let function = &input.functions[0];
    let (outer_place, outer_producer) = outer_establishment(&input, machine);
    let child_producer = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            AbstractOperation::EstablishRecord {
                psi_operation,
                result,
                ..
            } if result.place != outer_place => Some(*psi_operation),
            _ => None,
        })
        .expect("the inner record's establishment exists");

    let run = specialize(unit);
    let (commit, patch) = single_commit(&run);
    assert_eq!(patch.machine, machine);
    assert_eq!(patch.place, outer_place);
    assert_eq!(patch.producer, Some(outer_producer));
    let [row] = patch.reads.as_slice() else {
        panic!("one forwarded field read")
    };
    assert_eq!(
        row.path.len(),
        1,
        "the read descends one structural field into the child"
    );
    assert_eq!(
        row.producer,
        Some(child_producer),
        "the row's proof witness is the child's establishing operation"
    );
    let FieldValueResolution::Forward(forwarded) = &row.resolution else {
        panic!("the nested read forwards")
    };
    let definition = function
        .parameters
        .iter()
        .chain(function.blocks.iter().flat_map(|block| &block.parameters))
        .find(|definition| definition.value == forwarded.initializer)
        .expect("the proven initializer is parameter-sourced");
    assert_eq!(forwarded.scalar_type, definition.scalar_type);
    assert_ne!(forwarded.initializer, row.result);
    assert!(!forwarded.uses.is_empty(), "the read has covered uses");
    let [substitution] = commit
        .declaration
        .rewrite()
        .expect("rewrite declaration")
        .substitutions()
    else {
        panic!("one scalar substitution")
    };
    assert_eq!(substitution.from, row.result);
    assert_eq!(substitution.to, forwarded.initializer);

    // The read's node retires and its custody lands on the receiver; no
    // surviving node defines or uses its result.
    let output_function = run
        .session()
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine retained");
    let output_block = output_function
        .blocks
        .iter()
        .find(|block| block.id == row.site.block)
        .expect("block retained");
    let input_block = function
        .blocks
        .iter()
        .find(|block| block.id == row.site.block)
        .expect("input block");
    assert_eq!(
        output_block.nodes.len(),
        input_block.nodes.len() - 1,
        "the forwarded read retires"
    );
    let receiver = &output_block.nodes[usize::try_from(row.site.node).expect("index")];
    assert!(
        receiver
            .provenance
            .contains(&PsiProvenance::Operation(row.psi_operation))
    );
    for node in output_function.blocks.iter().flat_map(|block| &block.nodes) {
        assert!(
            !node
                .uses
                .iter()
                .any(|use_site| use_site.value == row.result)
        );
    }
    assert!(field_read_on(run.session().unit(), machine, outer_place).is_none());
}

#[test]
fn nested_unestablished_child_stays_unproven() {
    let unit = lowered_unit_entry(
        NESTED_UNESTABLISHED_CHILD_SOURCE,
        "nested unestablished child",
        "probe",
    );
    let input = unit.unit().clone();
    let machine = input
        .functions
        .iter()
        .find(|function| {
            function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .any(|node| {
                    matches!(
                        node.operation,
                        AbstractOperation::IntegerStructuralField { .. }
                            | AbstractOperation::BooleanStructuralField { .. }
                    )
                })
        })
        .expect("the entry machine holds the nested field read")
        .machine;
    let (outer_place, _) = outer_establishment(&input, machine);
    assert!(
        field_read_on(&input, machine, outer_place).is_some(),
        "the fixture must actually contain a nested field read"
    );
    // The child place's producer is a call, not an establishment — the
    // record stores its result whole, but nothing proves the contents the
    // callee returned.
    assert_declines(unit);
}

#[test]
fn bound_parameter_nested_field_folds_through_stored_child() {
    let unit = lowered_unit_entry(BOUND_NESTED_SOURCE, "bound nested field", "Root::run");
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    // The read observes the bound parameter at `inner`: the proof crosses
    // the binding to the outer record's establishment, then descends the
    // stored-whole `inner` child to its own `EstablishRecord`.
    let (place, bound) = bound_observation(&input, machine);
    let outer_producer = producer_of(&input, machine, bound);
    let child_producer = stored_child_producer(&input, machine, outer_producer);
    let (site, read) = field_read_on(&input, machine, place).expect("field read exists");

    let run = specialize(unit);
    let (_, patch) = single_commit(&run);
    assert_eq!(patch.machine, machine);
    assert_eq!(patch.place, place);
    assert_eq!(patch.producer, None);
    let [row] = patch.reads.as_slice() else {
        panic!("one folded field read")
    };
    assert_eq!(row.site, site);
    assert_eq!(row.source, place);
    assert_eq!(
        row.path.len(),
        1,
        "the read descends one structural field into the bound record"
    );
    assert_eq!(row.field, read.2);
    assert_eq!(
        row.producer,
        Some(child_producer),
        "the row's proof witness is the stored child's establishment"
    );
    assert_eq!(
        row.resolution,
        FieldValueResolution::Constant(FoldedFieldValue::Integer(
            semantic_vocabulary::IntegerValue::Unsigned(7)
        ))
    );

    let folded = &run.session().unit().functions[0]
        .blocks
        .iter()
        .find(|block| block.id == site.block)
        .expect("block retained")
        .nodes[usize::try_from(site.node).expect("index")];
    assert!(matches!(
        folded.operation,
        AbstractOperation::IntegerConstant {
            value: semantic_vocabulary::IntegerValue::Unsigned(7),
            ..
        }
    ));
    assert!(field_read_on(run.session().unit(), machine, place).is_none());
}

#[test]
fn uniform_multi_edge_binding_folds() {
    let unit = lowered_unit_entry(
        UNIFORM_TWO_EDGE_SOURCE,
        "uniform two-edge binding",
        "Root::run",
    );
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    let (place, bound) = bound_observation(&input, machine);
    let (established, producer) = established_record_place(&input, machine);
    // The fixture must actually deliver the parameter on two edges.
    let edges = input.functions[0]
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
        .flat_map(|edge| &edge.structural_bindings)
        .filter(|binding| binding.parameter == place)
        .count();
    assert_eq!(edges, 2, "two incoming edges bind the one place");

    let run = specialize(unit);
    let (_, patch) = single_commit(&run);
    assert_eq!(patch.place, place);
    let [row] = patch.reads.as_slice() else {
        panic!("one folded field read")
    };
    assert_eq!(row.producer, Some(producer));
    assert_eq!(
        row.resolution,
        FieldValueResolution::Constant(FoldedFieldValue::Integer(
            semantic_vocabulary::IntegerValue::Unsigned(37)
        ))
    );
    assert_eq!(bound, established);
}

#[test]
fn unobserved_establishment_yields_no_candidate() {
    let unit = lowered_unit_entry(NO_READS_SOURCE, "no-reads decline", "probe");
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    let (place, producer) = established_record_place(&input, machine);
    // The place is proven — its plan carries the establishment witness — but
    // no observation reads it, so the plan is empty and the rule proposes
    // nothing.
    let plan = super::propose::plan(&input, &input.functions[0], place).expect("proven place");
    assert_eq!(plan.producer, Some(producer));
    assert!(plan.reads.is_empty());
    // A candidate claiming the empty plan cannot even be constructed: a
    // field-value patch must fold at least one read.
    assert!(
        PsiRewriteCandidate::new_field_value_specialization(
            input.identity,
            FieldValueSpecializationRule::contract(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            0,
            plan,
        )
        .is_err()
    );
    assert_declines(unit);
}

#[test]
fn replay_rejects_forged_field_rows() {
    let unit = lowered_unit_entry(
        ESTABLISHED_FIELDS_SOURCE,
        "established field values",
        "probe",
    );
    let input = unit.unit().clone();
    let run = specialize(unit);
    let (commit, _) = single_commit(&run);

    // A forged folded value.
    assert_rejects_rows(
        &input,
        forged(
            commit.declaration.rewrite().expect("rewrite declaration"),
            |patch| {
                patch.reads[0].resolution = FieldValueResolution::Constant(
                    FoldedFieldValue::Integer(semantic_vocabulary::IntegerValue::Unsigned(41)),
                );
            },
        ),
    );

    // A forged field identity — a field no source operation carries.
    assert_rejects_rows(
        &input,
        forged(
            commit.declaration.rewrite().expect("rewrite declaration"),
            |patch| {
                patch.reads[0].field =
                    semantic_vocabulary::StructuralFieldId::new(patch.reads[0].field.get() + 7)
                        .expect("forged field identity");
            },
        ),
    );

    // A forged path segment.
    assert_rejects_rows(
        &input,
        forged(
            commit.declaration.rewrite().expect("rewrite declaration"),
            |patch| {
                patch.reads[0].path =
                    vec![semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(0)];
            },
        ),
    );

    // A forged site coordinate.
    assert_rejects_rows(
        &input,
        forged(
            commit.declaration.rewrite().expect("rewrite declaration"),
            |patch| {
                patch.reads[0].site.node += 1;
            },
        ),
    );

    // A forged producer identity.
    assert_rejects_rows(
        &input,
        forged(
            commit.declaration.rewrite().expect("rewrite declaration"),
            |patch| {
                patch.producer = Some(patch.reads[0].psi_operation);
            },
        ),
    );

    // The untampered declaration still validates to the committed output.
    let validated = validate_field_value_specialization_candidate(
        &input,
        commit.declaration.rewrite().expect("rewrite declaration"),
    )
    .expect("the exact candidate still validates");
    assert_eq!(validated.unit().identity, commit.output);
}

#[test]
fn replay_rejects_stale_candidate_revision() {
    let unit = lowered_unit_entry(
        ESTABLISHED_FIELDS_SOURCE,
        "established field values",
        "probe",
    );
    let input = unit.unit().clone();
    let run = specialize(unit);
    let (commit, _) = single_commit(&run);

    // A candidate pinned to a revision that is not the input.
    let stale = rebuild(
        commit.declaration.rewrite().expect("rewrite declaration"),
        OptimizationUnitIdentity::from_canonical_bytes(b"stale-input"),
        |_| {},
    )
    .expect("a stale input identity is still a well-formed declaration");
    assert_eq!(
        validate_field_value_specialization_candidate(&input, &stale).err(),
        Some(OptimizationUnitValidationError::CandidateInputMismatch)
    );

    // Committing moves the revision; the original declaration is stale
    // against the transformed unit afterward.
    assert_eq!(
        validate_field_value_specialization_candidate(
            run.session().unit(),
            commit.declaration.rewrite().expect("rewrite declaration")
        )
        .err(),
        Some(OptimizationUnitValidationError::CandidateInputMismatch)
    );
}

fn budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(96, 64, 64, 64, 64).expect("budget")
}

fn selections() -> OptimizationSelections {
    OptimizationSelections::new([Optimization::RepresentationSpecialization])
        .expect("representation-specialization selection")
}

/// Runs the representation-specialization pass — the field-value rule beside
/// its case-membership sibling, which no fixture here exercises — to its
/// fixed point through the public pipeline entrance.
fn specialize(unit: VerifiedPsiOptimizationUnit) -> OptimizationRun {
    run_psi_pipeline(unit, &selections(), budget()).expect("the selected pass runs")
}

/// The run's single commit — the pass reached its fixed point after one
/// candidate — with its field-value patch.
fn single_commit(
    run: &OptimizationRun,
) -> (&PsiOptimizationCommit, FieldValueSpecializationRewrite) {
    let [commit] = run.commits() else {
        panic!("exactly one commit: one candidate covers the place")
    };
    assert_eq!(
        commit.rule,
        FieldValueSpecializationRule::contract().identity()
    );
    let PsiRewritePatch::SpecializeFieldValue(patch) = commit
        .declaration
        .rewrite()
        .expect("rewrite declaration")
        .patch()
    else {
        panic!("a field-value patch")
    };
    (commit, patch)
}

/// The whole selected pass declines the unit and leaves it byte-exact.
fn assert_declines(unit: VerifiedPsiOptimizationUnit) {
    let input_identity = unit.unit().identity;
    let run = specialize(unit);
    assert!(run.commits().is_empty(), "no candidate specializes");
    assert_eq!(run.session().unit().identity, input_identity);
}

/// The committed declaration rebuilt with `input` as its revision and
/// `mutate` applied to its patch, keeping every other declared axis.
fn rebuild(
    declaration: &PsiRewriteCandidate,
    input: OptimizationUnitIdentity,
    mutate: impl FnOnce(&mut FieldValueSpecializationRewrite),
) -> Result<PsiRewriteCandidate, PsiRewriteCandidateError> {
    let PsiRewritePatch::SpecializeFieldValue(mut patch) = declaration.patch() else {
        panic!("a field-value patch")
    };
    mutate(&mut patch);
    PsiRewriteCandidate::new_field_value_specialization(
        input,
        FieldValueSpecializationRule::contract(),
        declaration.affected_blocks().to_vec(),
        declaration.substitutions().to_vec(),
        declaration.provenance().to_vec(),
        declaration.predicted_cost_delta(),
        patch,
    )
}

fn forged(
    declaration: &PsiRewriteCandidate,
    mutate: impl FnOnce(&mut FieldValueSpecializationRewrite),
) -> Result<PsiRewriteCandidate, PsiRewriteCandidateError> {
    rebuild(declaration, declaration.input(), mutate)
}

/// A forged row is refused either at candidate construction — the patch
/// invariants already disagree — or by the independent replay, which
/// re-admits every row against `input` rather than trusting it.
fn assert_rejects_rows(
    input: &PsiOptimizationUnit,
    forged: Result<PsiRewriteCandidate, PsiRewriteCandidateError>,
) {
    let Ok(candidate) = forged else {
        return;
    };
    assert!(matches!(
        validate_field_value_specialization_candidate(input, &candidate),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch
            | OptimizationUnitValidationError::CandidateProvenanceMismatch
            | OptimizationUnitValidationError::CandidateLocationMissing
            | OptimizationUnitValidationError::CandidateOutsideRegionMismatch)
    ));
}

/// The machine's parameter place.
fn parameter_place(unit: &PsiOptimizationUnit) -> PlaceId {
    unit.functions[0]
        .structural_places
        .iter()
        .find(|declaration| matches!(declaration.kind, StructuralPlaceKind::Parameter { .. }))
        .expect("parameter place exists")
        .id
}

/// The place the machine's field read observes — the state's block
/// parameter in bound fixtures — plus the one place its structural
/// bindings name, which the fixture requires to be the same place across
/// every binding.
fn bound_observation(unit: &PsiOptimizationUnit, machine: MachineId) -> (PlaceId, PlaceId) {
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine exists");
    let place = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            AbstractOperation::IntegerStructuralField { source, .. }
            | AbstractOperation::BooleanStructuralField { source, .. } => Some(*source),
            _ => None,
        })
        .expect("a field read observes the parameter");
    let declaration = function
        .structural_places
        .iter()
        .find(|declaration| declaration.id == place)
        .expect("the observed place is rostered");
    assert!(
        matches!(declaration.kind, StructuralPlaceKind::BlockParameter { .. }),
        "the fixture's read observes a block parameter"
    );
    let mut bound = None;
    for edge in function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
    {
        for binding in &edge.structural_bindings {
            if binding.parameter != place {
                continue;
            }
            let argument = &binding.argument;
            assert!(
                argument.path.is_empty(),
                "the fixture binds the whole place"
            );
            match bound {
                None => bound = Some(argument.place),
                Some(seen) => assert_eq!(seen, argument.place, "the fixture binds uniformly"),
            }
        }
    }
    (place, bound.expect("the parameter is bound"))
}

/// The producer `place`'s operation-result kind names.
fn producer_of(
    unit: &PsiOptimizationUnit,
    machine: MachineId,
    place: PlaceId,
) -> semantic_vocabulary::OperationId {
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine exists");
    let declaration = function
        .structural_places
        .iter()
        .find(|declaration| declaration.id == place)
        .expect("the place is rostered");
    let StructuralPlaceKind::OperationResult { producer, .. } = declaration.kind else {
        panic!("the place is an operation result")
    };
    producer
}

/// The establishing producer of the structural child `producer` stores
/// whole into its record — the nested carrier a bound fixture's deeper
/// read resolves to.
fn stored_child_producer(
    unit: &PsiOptimizationUnit,
    machine: MachineId,
    producer: semantic_vocabulary::OperationId,
) -> semantic_vocabulary::OperationId {
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine exists");
    let child = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            AbstractOperation::EstablishRecord {
                psi_operation,
                fields,
                ..
            } if *psi_operation == producer => {
                fields
                    .iter()
                    .find_map(|initializer| match &initializer.value {
                        terminal_psi::RecordFieldValue::Structural(argument) => {
                            Some(argument.place)
                        }
                        _ => None,
                    })
            }
            _ => None,
        })
        .expect("the producer stores a structural child");
    producer_of(unit, machine, child)
}

/// The `OperationResult` place a structural field read observes — the outer
/// record in a nested fixture — with its producer operation. Distinct from
/// `established_record_place`, which finds the roster's first established
/// place: the read's `source` names the observed place directly, so nested
/// establishments do not confuse the lookup.
fn outer_establishment(
    unit: &PsiOptimizationUnit,
    machine: MachineId,
) -> (PlaceId, semantic_vocabulary::OperationId) {
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine exists");
    let place = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            AbstractOperation::IntegerStructuralField { source, .. }
            | AbstractOperation::BooleanStructuralField { source, .. } => Some(*source),
            _ => None,
        })
        .expect("a field read observes the outer place");
    let declaration = function
        .structural_places
        .iter()
        .find(|declaration| declaration.id == place)
        .expect("the observed place is rostered");
    let StructuralPlaceKind::OperationResult { producer, .. } = declaration.kind else {
        panic!("the observed place is an operation result")
    };
    (place, producer)
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

fn lowered_unit_entry(source: &str, label: &str, entry: &str) -> VerifiedPsiOptimizationUnit {
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
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name(entry),
    )
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
    terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap_or_else(|error| panic!("build {label} optimizer unit: {error:?}"))
}
