//! Optimizer module role: test leaf. Proven field-value specialization admission, realization, and custody evidence through the rule.
//!
//! Every fixture is driven through the one live route: the
//! `RepresentationSpecialization` pass runs `FieldValueSpecializationRule`
//! to its fixed point, the committed candidate carries the proposed plan,
//! the run's session carries the folded unit, and forged rows are replayed
//! against `validate_field_value_specialization_candidate` — the independent
//! validator the pass manager itself consults.

use crate::rules::FieldValueSpecializationRule;
use crate::{
    OptimizationRun, PsiOptimizationCommit, VerifiedPsiOptimizationSession, run_psi_pipeline,
};
use abstract_operations::AbstractOperation;
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use optimization_core::{
    Optimization, OptimizationSelections, OptimizationUnitIdentity, OptimizationWorkBudget,
};
use optimization_unit::{
    FieldValueResolution, FieldValueSpecializationRewrite, FoldedFieldValue, NodeLocation,
    ProvenanceDisposition, PsiOptimizationUnit, PsiProvenance, PsiRealizationSite,
    PsiRewriteCandidate, PsiRewriteCandidateError, PsiRewritePatch,
    recompute_psi_optimization_unit_identity,
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

/// A nested path descends through a record field stored whole: the child's
/// own `EstablishRecord` establishes the deeper position, so `o.inner.v`
/// folds to the child's proven initializer. The row's producer is the
/// child's establishing operation while the patch's producer stays the
/// observed place's own.
const NESTED_ESTABLISHED_SOURCE: &str = r#"
    data Inner { v: u32; }
    data Outer { inner: Inner; }
    machine probe() -> u32 {
        let o: Outer = Outer { inner: Inner { v: 7 } };
        o.inner.v
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

#[test]
fn established_record_fields_fold_to_proven_values() {
    let unit = lowered_unit_entry(
        ESTABLISHED_FIELDS_SOURCE,
        "established field values",
        "probe",
    );
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    let (place, producer) = established_record_place(&input, machine);

    let run = specialize(unit);
    let (commit, patch) = single_commit(&run);
    assert_eq!(patch.machine, machine);
    assert_eq!(patch.place, place);
    assert_eq!(patch.producer, Some(producer));
    assert_eq!(commit.input, input.identity);
    assert_ne!(commit.output, input.identity);
    let [first, second] = patch.reads.as_slice() else {
        panic!("two folded field reads")
    };
    assert_eq!(first.source, place);
    assert_eq!(second.source, place);
    assert_eq!(first.producer, Some(producer));
    assert_eq!(second.producer, Some(producer));
    let mut values = [&first.resolution, &second.resolution];
    values.sort();
    assert_eq!(
        values,
        [
            &FieldValueResolution::Constant(FoldedFieldValue::Boolean(true)),
            &FieldValueResolution::Constant(FoldedFieldValue::Integer(
                semantic_vocabulary::IntegerValue::Unsigned(37),
            )),
        ]
    );

    // The proposal is deterministic: an independent run commits the same
    // candidate, custody, and output revision.
    let replayed = specialize(lowered_unit_entry(
        ESTABLISHED_FIELDS_SOURCE,
        "established field values",
        "probe",
    ));
    assert_eq!(replayed.commits(), run.commits());

    let output = run.session().unit();
    let output_function = output
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine retained");
    for row in &patch.reads {
        let folded = &output_function
            .blocks
            .iter()
            .find(|block| block.id == row.site.block)
            .expect("block retained")
            .nodes[usize::try_from(row.site.node).expect("index")];
        match &row.resolution {
            FieldValueResolution::Constant(FoldedFieldValue::Boolean(constant)) => {
                let AbstractOperation::BooleanConstant {
                    psi_operation,
                    result,
                    value,
                } = &folded.operation
                else {
                    panic!("boolean read folds to BooleanConstant")
                };
                assert_eq!(*psi_operation, row.psi_operation);
                assert_eq!(*result, row.result);
                assert_eq!(value, constant);
            }
            FieldValueResolution::Constant(FoldedFieldValue::Integer(constant)) => {
                let AbstractOperation::IntegerConstant {
                    psi_operation,
                    result,
                    value,
                    ..
                } = &folded.operation
                else {
                    panic!("integer read folds to IntegerConstant")
                };
                assert_eq!(*psi_operation, row.psi_operation);
                assert_eq!(*result, row.result);
                assert_eq!(value, constant);
            }
            FieldValueResolution::Forward(_) => panic!("constant rows never forward"),
        }
        assert_eq!(
            folded.provenance,
            vec![PsiProvenance::Operation(row.psi_operation)]
        );
        let input_node = &input
            .functions
            .iter()
            .find(|function| function.machine == machine)
            .expect("input machine")
            .blocks
            .iter()
            .find(|block| block.id == row.site.block)
            .expect("input block")
            .nodes[usize::try_from(row.site.node).expect("index")];
        assert_eq!(folded.definitions, input_node.definitions);
        assert_eq!(folded.uses, input_node.uses);
        assert_eq!(folded.successors, input_node.successors);
    }

    // The ledger records each folded site's retained custody — exactly the
    // custody the validator accepted for the commit.
    let [record] = run.transformation_ledger().records() else {
        panic!("one transformation record")
    };
    assert_eq!(record.input, input.identity);
    assert_eq!(record.output, output.identity);
    assert_eq!(record.provenance, commit.provenance);
    assert_eq!(record.provenance.len(), 2);
    for row in &patch.reads {
        let site = PsiRealizationSite::Node(row.site);
        assert!(record.provenance.iter().any(|rewrite| {
            rewrite.input == site
                && rewrite.disposition == ProvenanceDisposition::RealizedAt(site)
                && rewrite.sources == vec![PsiProvenance::Operation(row.psi_operation)]
        }));
    }

    // The single commit is the pass's fixed point: no field read on the
    // place survives to draw a second candidate.
    assert!(field_read_on(output, machine, place).is_none());
}

#[test]
fn parameter_field_read_yields_no_candidate() {
    let unit = lowered_unit_entry(PARAMETER_FIELD_SOURCE, "parameter decline", "probe");
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    let place = parameter_place(&input);
    assert!(
        field_read_on(&input, machine, place).is_some(),
        "the fixture must actually contain a field read"
    );
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
fn nested_bounded_field_folds_at_path_depth() {
    let unit = lowered_unit_entry(NESTED_BOUNDED_SOURCE, "nested bounded field", "probe");
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    let place = parameter_place(&input);
    let (site, read) = field_read_on(&input, machine, place).expect("field read exists");

    let run = specialize(unit);
    let (_, patch) = single_commit(&run);
    let [row] = patch.reads.as_slice() else {
        panic!("one folded field read")
    };
    assert_eq!(row.producer, None);
    assert_eq!(row.path.len(), 1, "the read descends one record field");
    assert_eq!(row.field, read.2);
    assert_eq!(
        row.resolution,
        FieldValueResolution::Constant(FoldedFieldValue::Integer(
            semantic_vocabulary::IntegerValue::Signed(7)
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
            value: semantic_vocabulary::IntegerValue::Signed(7),
            ..
        }
    ));
}

#[test]
fn nonconstant_initializer_forwards_to_uses() {
    let unit = lowered_unit_entry(FORWARDED_FIELD_SOURCE, "forwarded field value", "probe");
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    let function = &input.functions[0];
    let (place, producer) = established_record_place(&input, machine);

    let run = specialize(unit);
    let (commit, patch) = single_commit(&run);
    assert_eq!(patch.machine, machine);
    assert_eq!(patch.place, place);
    assert_eq!(patch.producer, Some(producer));

    let forward_row = patch
        .reads
        .iter()
        .find(|row| matches!(row.resolution, FieldValueResolution::Forward(_)))
        .expect("one forwarded read");
    let FieldValueResolution::Forward(forwarded) = &forward_row.resolution else {
        unreachable!()
    };
    // The front end routes the `x` machine parameter through a block
    // parameter binding, so the proven initializer is parameter-sourced
    // rather than a computed node — that is exactly why it is nonconstant.
    let definition = function
        .parameters
        .iter()
        .chain(function.blocks.iter().flat_map(|block| &block.parameters))
        .find(|definition| definition.value == forwarded.initializer)
        .expect("the proven initializer is parameter-sourced");
    assert_eq!(forwarded.scalar_type, definition.scalar_type);
    assert_ne!(forwarded.initializer, forward_row.result);
    assert_eq!(forward_row.producer, Some(producer));
    assert!(
        !forwarded.uses.is_empty(),
        "the forwarded read has covered uses"
    );
    for site in &forwarded.uses {
        assert_eq!(site.machine, machine);
        let use_node = &function
            .blocks
            .iter()
            .find(|block| block.id == site.block)
            .expect("use block exists")
            .nodes[usize::try_from(site.node).expect("use index")];
        assert!(
            use_node
                .uses
                .iter()
                .any(|use_site| use_site.value == forward_row.result),
            "each listed use site references the read's result"
        );
    }
    // The candidate declares exactly the one `result → initializer`
    // substitution the forwarded row implies.
    let [substitution] = commit.declaration.substitutions() else {
        panic!("one scalar substitution")
    };
    assert_eq!(substitution.from, forward_row.result);
    assert_eq!(substitution.to, forwarded.initializer);
    let constant_row = patch
        .reads
        .iter()
        .find(|row| matches!(row.resolution, FieldValueResolution::Constant(_)))
        .expect("one folded read");
    assert_eq!(
        constant_row.resolution,
        FieldValueResolution::Constant(FoldedFieldValue::Boolean(true))
    );
    assert_eq!(constant_row.producer, Some(producer));

    // The proposal is deterministic.
    let replayed = specialize(lowered_unit_entry(
        FORWARDED_FIELD_SOURCE,
        "forwarded field value",
        "probe",
    ));
    assert_eq!(replayed.commits(), run.commits());

    let output_function = run
        .session()
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine retained");
    let input_block = function
        .blocks
        .iter()
        .find(|block| block.id == forward_row.site.block)
        .expect("input block");
    let output_block = output_function
        .blocks
        .iter()
        .find(|block| block.id == forward_row.site.block)
        .expect("block retained");
    assert_eq!(
        output_block.nodes.len(),
        input_block.nodes.len() - 1,
        "the forwarded read retires"
    );

    // The node inheriting the vacated index absorbs the read's custody.
    let receiver = &output_block.nodes[usize::try_from(forward_row.site.node).expect("index")];
    assert!(
        receiver
            .provenance
            .contains(&PsiProvenance::Operation(forward_row.psi_operation))
    );
    assert!(receiver.fuel.iter().any(|settlement| {
        settlement.site == PsiProvenance::Operation(forward_row.psi_operation)
    }));

    // No surviving node defines or uses the retired result; every listed use
    // now references the proven initializer at its shifted coordinate.
    for node in output_function.blocks.iter().flat_map(|block| &block.nodes) {
        assert!(
            !node
                .definitions
                .iter()
                .any(|definition| definition.value == forward_row.result)
        );
        assert!(
            !node
                .uses
                .iter()
                .any(|use_site| use_site.value == forward_row.result)
        );
    }
    for site in &forwarded.uses {
        let shift =
            usize::from(site.block == forward_row.site.block && site.node > forward_row.site.node);
        let output_index = usize::try_from(site.node).expect("index") - shift;
        let node = &output_function
            .blocks
            .iter()
            .find(|block| block.id == site.block)
            .expect("use block retained")
            .nodes[output_index];
        assert!(
            node.uses
                .iter()
                .any(|use_site| use_site.value == forwarded.initializer),
            "the substituted use references the initializer"
        );
    }

    // The folded sibling still lands at its (shifted) site as a constant.
    let folded_site = constant_row.site;
    let folded_shift = usize::from(
        folded_site.block == forward_row.site.block && folded_site.node > forward_row.site.node,
    );
    let folded = &output_function
        .blocks
        .iter()
        .find(|block| block.id == folded_site.block)
        .expect("fold block retained")
        .nodes[usize::try_from(folded_site.node).expect("index") - folded_shift];
    assert!(matches!(
        folded.operation,
        AbstractOperation::BooleanConstant { value: true, .. }
    ));

    // The ledger records the retired site's custody landing on the node that
    // inherited its index.
    let [record] = run.transformation_ledger().records() else {
        panic!("one transformation record")
    };
    let retired = PsiRealizationSite::Node(forward_row.site);
    assert!(record.provenance.iter().any(|rewrite| {
        rewrite.input == retired
            && rewrite.disposition == ProvenanceDisposition::RealizedAt(retired)
            && rewrite.sources == vec![PsiProvenance::Operation(forward_row.psi_operation)]
    }));

    // The single commit is the pass's fixed point.
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
        forged(&commit.declaration, |patch| {
            let result = patch.reads[forward_index].result;
            let FieldValueResolution::Forward(forwarded) =
                &mut patch.reads[forward_index].resolution
            else {
                unreachable!()
            };
            forwarded.initializer = result;
        }),
    );

    // A forged use roster — replay recomputes the complete covered set.
    assert_rejects_rows(
        &input,
        forged(&commit.declaration, |patch| {
            let FieldValueResolution::Forward(forwarded) =
                &mut patch.reads[forward_index].resolution
            else {
                unreachable!()
            };
            forwarded.uses.pop();
        }),
    );

    // A forged constant claim on the forwarded read.
    assert_rejects_rows(
        &input,
        forged(&commit.declaration, |patch| {
            patch.reads[forward_index].resolution = FieldValueResolution::Constant(
                FoldedFieldValue::Integer(semantic_vocabulary::IntegerValue::Unsigned(7)),
            );
        }),
    );

    // The untampered declaration still validates to the committed output.
    let validated = validate_field_value_specialization_candidate(&input, &commit.declaration)
        .expect("the exact candidate still validates");
    assert_eq!(validated.unit().identity, commit.output);
}

#[test]
fn nested_structural_child_folds_from_child_establishment() {
    let unit = lowered_unit_entry(
        NESTED_ESTABLISHED_SOURCE,
        "nested established field",
        "probe",
    );
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    // The read observes the outer place while the leaf's proof is the inner
    // record's establishment — two `EstablishRecord` nodes exist, and the
    // child's is the one whose result place the read does not observe.
    let (outer_place, outer_producer) = outer_establishment(&input, machine);
    let child_producer = input.functions[0]
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
    let (site, read) = field_read_on(&input, machine, outer_place).expect("field read exists");

    let run = specialize(unit);
    let (_, patch) = single_commit(&run);
    assert_eq!(patch.machine, machine);
    assert_eq!(patch.place, outer_place);
    assert_eq!(patch.producer, Some(outer_producer));
    let [row] = patch.reads.as_slice() else {
        panic!("one folded field read")
    };
    assert_eq!(row.site, site);
    assert_eq!(row.source, outer_place);
    assert_eq!(
        row.path.len(),
        1,
        "the read descends one structural field into the child"
    );
    assert_eq!(row.field, read.2);
    assert_eq!(
        row.producer,
        Some(child_producer),
        "the row's proof witness is the child's establishing operation"
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
    assert!(field_read_on(run.session().unit(), machine, outer_place).is_none());
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
    let [substitution] = commit.declaration.substitutions() else {
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
fn replay_rejects_forged_nested_row_witness() {
    let unit = lowered_unit_entry(
        NESTED_ESTABLISHED_SOURCE,
        "nested established field",
        "probe",
    );
    let input = unit.unit().clone();
    let run = specialize(unit);
    let (commit, _) = single_commit(&run);

    // A forged row witness — claiming the observed place's own producer
    // instead of the child's establishing operation — is refused: replay
    // re-derives the row from the descent chain rather than trusting it.
    assert_rejects_rows(
        &input,
        forged(&commit.declaration, |patch| {
            patch.reads[0].producer = patch.producer;
        }),
    );

    // A forged folded value on the nested read.
    assert_rejects_rows(
        &input,
        forged(&commit.declaration, |patch| {
            patch.reads[0].resolution = FieldValueResolution::Constant(FoldedFieldValue::Integer(
                semantic_vocabulary::IntegerValue::Unsigned(41),
            ));
        }),
    );

    // A forged path that skips the descent.
    assert_rejects_rows(
        &input,
        forged(&commit.declaration, |patch| {
            patch.reads[0].path.clear();
        }),
    );

    // The untampered declaration still validates to the committed output.
    let validated = validate_field_value_specialization_candidate(&input, &commit.declaration)
        .expect("the exact candidate still validates");
    assert_eq!(validated.unit().identity, commit.output);
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
        forged(&commit.declaration, |patch| {
            patch.reads[0].resolution = FieldValueResolution::Constant(FoldedFieldValue::Integer(
                semantic_vocabulary::IntegerValue::Unsigned(41),
            ));
        }),
    );

    // A forged field identity — a field no source operation carries.
    assert_rejects_rows(
        &input,
        forged(&commit.declaration, |patch| {
            patch.reads[0].field =
                semantic_vocabulary::StructuralFieldId::new(patch.reads[0].field.get() + 7)
                    .expect("forged field identity");
        }),
    );

    // A forged path segment.
    assert_rejects_rows(
        &input,
        forged(&commit.declaration, |patch| {
            patch.reads[0].path =
                vec![semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(0)];
        }),
    );

    // A forged site coordinate.
    assert_rejects_rows(
        &input,
        forged(&commit.declaration, |patch| {
            patch.reads[0].site.node += 1;
        }),
    );

    // A forged producer identity.
    assert_rejects_rows(
        &input,
        forged(&commit.declaration, |patch| {
            patch.producer = Some(patch.reads[0].psi_operation);
        }),
    );

    // The untampered declaration still validates to the committed output.
    let validated = validate_field_value_specialization_candidate(&input, &commit.declaration)
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
        &commit.declaration,
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
        validate_field_value_specialization_candidate(run.session().unit(), &commit.declaration)
            .err(),
        Some(OptimizationUnitValidationError::CandidateInputMismatch)
    );
}

/// The committed unit revalidates independently, while a unit whose folded
/// node drops its fuel settlement — settling fewer sources than the custody
/// it names — is refused by transformed validation. A forged commit output
/// identity is refused by publication replay in
/// `pass_manager::tests::evidence_matrix::representation_specialization::forged_field_value_run_axes_fail_publication_replay`.
#[test]
fn transformed_unit_rejects_forged_folded_custody() {
    let unit = lowered_unit_entry(
        ESTABLISHED_FIELDS_SOURCE,
        "established field values",
        "probe",
    );
    let run = specialize(unit);
    let (_, patch) = single_commit(&run);
    let row = &patch.reads[0];
    let machine = patch.machine;
    let verified_input = run.session().input().clone();

    assert!(
        VerifiedPsiOptimizationSession::from_transformed(
            verified_input.clone(),
            run.session().unit().clone(),
        )
        .is_ok(),
        "the committed folded revision revalidates independently"
    );

    let mut malformed = run.session().unit().clone();
    let folded = malformed
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .flat_map(|block| &mut block.nodes)
        .find(|node| {
            matches!(
                node.operation,
                AbstractOperation::BooleanConstant { psi_operation, .. }
                    | AbstractOperation::IntegerConstant { psi_operation, .. }
                    if psi_operation == row.psi_operation
            )
        })
        .expect("folded node exists");
    assert_eq!(
        folded.provenance,
        vec![PsiProvenance::Operation(row.psi_operation)]
    );
    folded.fuel.pop();
    malformed.identity = recompute_psi_optimization_unit_identity(&malformed);
    assert!(matches!(
        VerifiedPsiOptimizationSession::from_transformed(verified_input, malformed),
        Err(
            OptimizationUnitValidationError::FuelDoesNotMatchProvenance { machine: rejected, .. }
        ) if rejected == machine
    ));
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
    let PsiRewritePatch::SpecializeFieldValue(patch) = commit.declaration.patch() else {
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
