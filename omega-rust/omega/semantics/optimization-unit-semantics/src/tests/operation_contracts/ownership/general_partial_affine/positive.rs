//! Maximal subtree and empty-complement acceptance, independent of producer fixtures.

use super::fixtures::*;
use crate::validate_psi_optimization_unit;

#[test]
fn general_partial_affine_accepts_wider_arrays_and_authored_move_order() {
    for length in [2, 3, 4, 5, 17, 257] {
        let moved = length / 2;
        let residuals = (0..length)
            .rev()
            .filter(|value| *value != moved)
            .map(|value| (vec![index(value)], 1))
            .collect::<Vec<_>>();
        validate_psi_optimization_unit(&unit(
            vec![record(1, &[]), array(2, 1, length)],
            2,
            &[(vec![index(moved)], 1)],
            &residuals,
        ))
        .unwrap_or_else(|error| panic!("length {length}: {error:?}"));
    }
    validate_psi_optimization_unit(&unit(
        vec![record(1, &[]), array(2, 1, 7)],
        2,
        &[
            (vec![index(5)], 1),
            (vec![index(0)], 1),
            (vec![index(3)], 1),
        ],
        &[
            (vec![index(6)], 1),
            (vec![index(4)], 1),
            (vec![index(2)], 1),
            (vec![index(1)], 1),
        ],
    ))
    .expect("move order stays authored while residuals descend");
}

#[test]
fn general_partial_affine_keeps_maximal_subtrees_across_mixed_paths() {
    validate_psi_optimization_unit(&mixed_unit())
        .expect("Field/FixedIndex/Field keeps the untouched row whole");
    validate_psi_optimization_unit(&unit(
        vec![
            record(1, &[]),
            array(2, 1, 2),
            array(3, 2, 3),
            array(4, 3, 2),
        ],
        4,
        &[(vec![index(0), index(1), index(0)], 1)],
        &[
            (vec![index(1)], 3),
            (vec![index(0), index(2)], 2),
            (vec![index(0), index(1), index(1)], 1),
            (vec![index(0), index(0)], 2),
        ],
    ))
    .expect("three dimensions retain maximal untouched planes and rows");
    validate_psi_optimization_unit(&unit(
        vec![
            record(1, &[]),
            array(2, 1, 2),
            record(3, &[("values", 2)]),
            array(4, 3, 2),
        ],
        4,
        &[(vec![index(1), field("values"), index(0)], 1)],
        &[
            (vec![index(1), field("values"), index(1)], 1),
            (vec![index(0)], 3),
        ],
    ))
    .expect("FixedIndex/Field/FixedIndex resolves every component");
}

#[test]
fn general_partial_affine_accepts_disjoint_moves_with_different_path_depths() {
    validate_psi_optimization_unit(&unit(
        vec![
            record(1, &[]),
            array(2, 1, 3),
            record(3, &[("rows", 2), ("tail", 1)]),
        ],
        3,
        &[(vec![field("tail")], 1), (vec![field("rows"), index(1)], 1)],
        &[
            (vec![field("rows"), index(2)], 1),
            (vec![field("rows"), index(0)], 1),
        ],
    ))
    .expect("different depths and source order share one complement");
    validate_psi_optimization_unit(&unit(
        vec![record(1, &[]), array(2, 1, 3), array(3, 2, 3)],
        3,
        &[(vec![index(2)], 2), (vec![index(0), index(1)], 1)],
        &[
            (vec![index(1)], 2),
            (vec![index(0), index(2)], 1),
            (vec![index(0), index(0)], 1),
        ],
    ))
    .expect("whole subtree and leaf moves coexist");
}

#[test]
fn general_partial_affine_accepts_all_disjoint_moves_with_empty_complement() {
    for length in [1, 2, 3, 4, 5, 17] {
        let moves = (0..length)
            .map(|value| (vec![index(value)], 1))
            .collect::<Vec<_>>();
        validate_psi_optimization_unit(&unit(
            vec![record(1, &[]), array(2, 1, length)],
            2,
            &moves,
            &[],
        ))
        .unwrap_or_else(|error| panic!("fully moved length {length}: {error:?}"));
    }
    validate_psi_optimization_unit(&unit(
        vec![
            record(1, &[]),
            record(2, &[("left", 1), ("right", 1)]),
            array(3, 2, 2),
        ],
        3,
        &[
            (vec![index(1)], 2),
            (vec![index(0), field("right")], 1),
            (vec![index(0), field("left")], 1),
        ],
        &[],
    ))
    .expect("mixed whole-child and descendant moves can exhaust an array");
    validate_psi_optimization_unit(&unit(
        vec![
            record(1, &[]),
            array(2, 1, 2),
            record(3, &[("row", 2), ("tail", 1)]),
        ],
        3,
        &[
            (vec![field("row"), index(1)], 1),
            (vec![field("tail")], 1),
            (vec![field("row"), index(0)], 1),
        ],
        &[],
    ))
    .expect("all record descendants move without a whole-root discard");
}

#[test]
fn general_partial_affine_does_not_expand_huge_untouched_subtrees() {
    validate_psi_optimization_unit(&unit(
        vec![record(1, &[]), array(2, 1, u64::MAX), array(3, 2, 2)],
        3,
        &[(vec![index(0)], 2)],
        &[(vec![index(1)], 2)],
    ))
    .expect("an untouched huge row needs one residual and no element scan");
    validate_psi_optimization_unit(&unit(
        vec![record(1, &[]), array(2, 1, u64::MAX), array(3, 2, 2)],
        3,
        &[(vec![index(0)], 2), (vec![index(1)], 2)],
        &[],
    ))
    .expect("whole-row moves exhaust even huge rows without expansion");
}

#[test]
fn general_partial_affine_scalar_record_fields_need_no_cleanup() {
    use semantic_vocabulary::{IeeeFloatFormat, ScalarType, StructuralFieldId};
    use terminal_psi::{
        BindingRelevance, StructuralFieldDeclaration, StructuralFieldType, StructuralTypeShape,
    };

    let mut carrier = record(3, &[("row", 2)]);
    let StructuralTypeShape::Record { fields } = &mut carrier.shape else {
        unreachable!()
    };
    for (position, field_type) in [
        StructuralFieldType::Scalar(ScalarType::Boolean),
        StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary64),
        StructuralFieldType::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned {
            capacity: 8,
        }),
    ]
    .into_iter()
    .enumerate()
    {
        fields.push(StructuralFieldDeclaration {
            id: crate::tests::id(position as u64 + 2, StructuralFieldId::new),
            identity: format!("metadata{position}"),
            relevance: BindingRelevance::Relevant,
            field_type,
        });
    }
    validate_psi_optimization_unit(&unit(
        vec![record(1, &[]), array(2, 1, 2), carrier.clone()],
        3,
        &[(vec![field("row"), index(1)], 1)],
        &[(vec![field("row"), index(0)], 1)],
    ))
    .expect("scalar, floating, and bounded byte fields add no residual cleanup");
    validate_psi_optimization_unit(&unit(
        vec![record(1, &[]), array(2, 1, 2), carrier],
        3,
        &[
            (vec![field("row"), index(1)], 1),
            (vec![field("row"), index(0)], 1),
        ],
        &[],
    ))
    .expect("scalar metadata does not prevent an empty structural complement");
}
