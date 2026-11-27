//! Stage 05 must say which sum each match arm selects a case of. These tests
//! assert that on its output, the lowered program stage 06 checks.
//!
//! The program in `support/sum_dispatch.rs` has two unrelated sums, `Left`
//! and `Right`, whose cases have the same names, the same positions and the
//! same field name. The spec keeps them apart by owner: "A case pattern
//! destructures only that case's payload, and case names keep their
//! owner-qualified paths" (`wiki/spec/language/patterns.md`, "Data and case
//! decomposition"), and "Cases are nominal, not foreign integers"
//! (`wiki/spec/language/data_and_literals.md`, "Construction and case
//! identity").
//!
//! The lowered program records the owner in one way. Every case of every sum
//! has its own `StructuralCaseId`, unique in the module, and a dispatch edge
//! or a `Case` path step names a case by that id
//! (`source-rust/subsystems/compiler/psi/artifact/wire/encoding.md`: a `StructuralCase` terminator
//! carries "edge id + target block id + case id + counted field ids" for
//! each successor). So the id an arm carries is the whole statement of which
//! sum it selects from, and the tests below require it to be an id of the
//! sum the match subject holds.
//!
//! What stage 06 does when that id belongs to the other sum is tested in
//! `06_lowered-program-to-checked-program/tests/variant_parent_identity.rs`.

#[path = "support/sum_dispatch.rs"]
mod program;

use program::{Sum, dispatch, machine_position, payload_loan, prepare, sum};
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};
use source_compiler::test_support::lowering::Elaboration;
use terminal_psi::{StructuralPathSegment, StructuralTypeShape, Terminator};

fn byte() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 is valid"))
}

/// `Left` and `Right` as stage 05 declared them, after checking what the
/// other tests rely on: nothing but a case's id and its payload type tells
/// a case of one sum from the case at the same position in the other.
fn both_sums(lowered: &Elaboration) -> (Sum, Sum) {
    let (left, right) = (sum(lowered, "Left"), sum(lowered, "Right"));
    assert_ne!(left.id, right.id);
    assert_eq!((left.cases.len(), right.cases.len()), (2, 2));
    for (ours, theirs) in left.cases.iter().zip(&right.cases) {
        // The same spelling is one interned identity.
        assert_eq!(ours.identity, theirs.identity);
        assert_ne!(ours.id, theirs.id);
        assert_eq!(ours.fields.len(), theirs.fields.len());
        for (ours, theirs) in ours.fields.iter().zip(&theirs.fields) {
            assert_eq!(ours.identity, theirs.identity);
            assert_ne!(ours.id, theirs.id);
        }
    }
    let payload = |sum: &Sum| {
        let [small, nothing] = sum.cases.as_slice() else {
            unreachable!()
        };
        assert!(nothing.fields.is_empty());
        let [value] = small.fields.as_slice() else {
            panic!("`Small` carries one field: {small:?}")
        };
        value.field_type.as_scalar()
    };
    assert_eq!(payload(&left), Some(byte()));
    assert_eq!(payload(&right), Some(ScalarType::Boolean));

    // A case id names one case of one sum in the whole module.
    let mut ids: Vec<_> = lowered
        .module
        .types
        .structural_types
        .iter()
        .filter(|declared| matches!(declared.shape, StructuralTypeShape::Sum { .. }))
        .flat_map(|declared| declared.shape.cases())
        .map(|case| case.id)
        .collect();
    ids.sort_unstable();
    let declared = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), declared, "two cases share an id");
    (left, right)
}

#[test]
fn a_dispatch_lists_the_cases_of_the_sum_its_subject_holds() {
    let prepared = prepare();
    let lowered = prepared.lower().closure.unwrap();
    let (left, right) = both_sums(&lowered);
    for (name, own, foreign) in [
        ("choose", &left, &right),
        ("other", &right, &left),
        ("lend", &left, &right),
    ] {
        let machine = &lowered.module.machines[machine_position(&prepared, &lowered, name)];
        let (_, subject, edges) = dispatch(machine);
        assert_eq!(subject, own.id, "{name}");

        // Every case of that sum once, and so no case of the other sum.
        let mut listed: Vec<_> = edges.iter().map(|edge| edge.case).collect();
        listed.sort_unstable();
        let mut declared: Vec<_> = own.cases.iter().map(|case| case.id).collect();
        declared.sort_unstable();
        assert_eq!(listed, declared, "{name}");

        // Each edge binds the payload fields of its own case, and its arm's
        // block receives them with the types that case declares.
        for edge in edges {
            let case = own
                .cases
                .iter()
                .find(|case| case.id == edge.case)
                .expect("listed above");
            let fields: Vec<_> = case.fields.iter().map(|field| field.id).collect();
            assert_eq!(edge.payload_fields, fields, "{name}");
            let received: Vec<_> = machine
                .block(&edge.target)
                .unwrap()
                .parameters
                .iter()
                .map(|parameter| Some(parameter.scalar_type))
                .collect();
            let declared: Vec<_> = case
                .fields
                .iter()
                .map(|field| field.field_type.as_scalar())
                .collect();
            assert_eq!(received, declared, "{name}");
            assert!(
                foreign
                    .cases
                    .iter()
                    .flat_map(|case| &case.fields)
                    .all(|field| !edge.payload_fields.contains(&field.id)),
                "{name}"
            );
        }
    }

    // `choose` returns the payload its `Left::Small` edge delivers.
    let choose = &lowered.module.machines[machine_position(&prepared, &lowered, "choose")];
    let (_, _, edges) = dispatch(choose);
    let small = edges
        .iter()
        .find(|edge| edge.case == left.cases[0].id)
        .unwrap();
    let arm = choose.block(&small.target).unwrap();
    assert!(
        matches!(
            (&arm.terminator, arm.parameters.as_slice()),
            (Terminator::Return { value, .. }, [payload]) if *value == payload.id
        ),
        "{arm:?}"
    );

    // A match on a Boolean takes nothing from either sum.
    let plain = &lowered.module.machines[machine_position(&prepared, &lowered, "plain")];
    assert!(plain.structural_places.is_empty());
    assert!(
        plain
            .blocks
            .iter()
            .all(|block| !matches!(block.terminator, Terminator::StructuralCase { .. }))
    );
}

/// `look(@held.value)` in the `Left::Small` arm lends storage inside that
/// case's payload. The lowered argument reaches it by a `Case` step and then
/// the field's name (`wiki/spec/terminal-psi/structural_access.md`: "An
/// ordinary call may borrow a structural payload through `Case` followed by
/// that case's exact `Field`"). `Right::Small` has a field of the same name,
/// so the `Case` step alone says the payload is `Left::Small`'s.
#[test]
fn a_payload_loan_names_the_case_of_the_sum_it_borrows_from() {
    let prepared = prepare();
    let lowered = prepared.lower().closure.unwrap();
    let (left, right) = both_sums(&lowered);
    let machine = &lowered.module.machines[machine_position(&prepared, &lowered, "lend")];
    let (dispatching, subject, _) = dispatch(machine);
    assert_eq!(subject, left.id);
    let Terminator::StructuralCase { source, .. } = &dispatching.terminator else {
        unreachable!()
    };
    let (_, loan) = payload_loan(machine);
    assert_eq!(loan.place, *source);
    let small = &left.cases[0];
    assert_eq!(
        loan.path,
        [
            StructuralPathSegment::Case(small.id),
            StructuralPathSegment::Field(small.fields[0].identity),
        ]
    );
    assert_eq!(small.fields[0].identity, right.cases[0].fields[0].identity);
}
