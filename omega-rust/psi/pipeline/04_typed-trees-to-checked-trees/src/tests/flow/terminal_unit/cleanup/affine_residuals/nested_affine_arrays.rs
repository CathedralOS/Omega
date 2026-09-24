use crate::tests::flow::terminal_unit::cleanup::{
    assert_token_cleanup_partition, fixed_cleanup_path,
};
use crate::tests::flow::terminal_unit::{
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralPathSegment, checked, machine_named,
};

#[test]
fn nested_affine_arrays_discard_each_live_complement_in_decreasing_index_order() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::nested(values: [[Token; 3]; 2]) {
            Sink::take(values[1][0]);
            Sink::take(values[0][1]);
        }
        machine Root::nested_four(values: [[Token; 4]; 2]) {
            Sink::take(values[1][3]);
            Sink::take(values[0][1]);
        }
        machine Root::same_outer(values: [[Token; 3]; 2]) {
            Sink::take(values[0][0]);
            Sink::take(values[0][1]);
        }
        machine Root::one(values: [[Token; 3]; 2]) {
            Sink::take(values[1][2]);
        }
        machine Root::same_outer_four(values: [[Token; 4]; 2]) {
            Sink::take(values[0][0]);
            Sink::take(values[0][3]);
        }
        machine Root::one_four(values: [[Token; 4]; 2]) {
            Sink::take(values[1][3]);
        }
        machine Root::nested_five(values: [[Token; 5]; 2]) {
            Sink::take(values[1][4]);
            Sink::take(values[0][1]);
        }
        machine Root::same_outer_five(values: [[Token; 5]; 2]) {
            Sink::take(values[0][0]);
            Sink::take(values[0][4]);
        }
        machine Root::one_five(values: [[Token; 5]; 2]) {
            Sink::take(values[1][4]);
        }
        machine Root::nested_six(values: [[Token; 6]; 2]) {
            Sink::take(values[1][5]);
            Sink::take(values[0][1]);
        }
        machine Root::same_outer_six(values: [[Token; 6]; 2]) {
            Sink::take(values[0][0]);
            Sink::take(values[0][5]);
        }
        machine Root::one_six(values: [[Token; 6]; 2]) {
            Sink::take(values[1][5]);
        }
        machine Root::nested_seven(values: [[Token; 7]; 2]) {
            Sink::take(values[1][6]);
            Sink::take(values[0][1]);
        }
        machine Root::same_outer_seven(values: [[Token; 7]; 2]) {
            Sink::take(values[0][0]);
            Sink::take(values[0][6]);
        }
        machine Root::one_seven(values: [[Token; 7]; 2]) {
            Sink::take(values[1][6]);
        }
        machine Root::nested_eight(values: [[Token; 8]; 2]) {
            Sink::take(values[1][7]);
            Sink::take(values[0][1]);
        }
        machine Root::same_outer_eight(values: [[Token; 8]; 2]) {
            Sink::take(values[0][0]);
            Sink::take(values[0][7]);
        }
        machine Root::one_eight(values: [[Token; 8]; 2]) {
            Sink::take(values[1][7]);
        }
        machine Root::nested_nine(values: [[Token; 9]; 2]) {
            Sink::take(values[1][8]);
            Sink::take(values[0][1]);
        }
        machine Root::same_outer_nine(values: [[Token; 9]; 2]) {
            Sink::take(values[0][0]);
            Sink::take(values[0][8]);
        }
        machine Root::one_nine(values: [[Token; 9]; 2]) {
            Sink::take(values[1][8]);
        }
        machine Root::nested_ten(values: [[Token; 10]; 2]) {
            Sink::take(values[1][9]);
            Sink::take(values[0][1]);
        }
        machine Root::same_outer_ten(values: [[Token; 10]; 2]) {
            Sink::take(values[0][0]);
            Sink::take(values[0][9]);
        }
        machine Root::one_ten(values: [[Token; 10]; 2]) {
            Sink::take(values[1][9]);
        }
        machine Root::nested_eleven(values: [[Token; 11]; 2]) {
            Sink::take(values[1][10]);
            Sink::take(values[0][1]);
        }
        machine Root::same_outer_eleven(values: [[Token; 11]; 2]) {
            Sink::take(values[0][0]);
            Sink::take(values[0][10]);
        }
        machine Root::one_eleven(values: [[Token; 11]; 2]) {
            Sink::take(values[1][10]);
        }
        machine Root::nested_twelve(values: [[Token; 12]; 2]) {
            Sink::take(values[1][11]);
            Sink::take(values[0][1]);
        }
        machine Root::same_outer_twelve(values: [[Token; 12]; 2]) {
            Sink::take(values[0][0]);
            Sink::take(values[0][11]);
        }
        machine Root::one_twelve(values: [[Token; 12]; 2]) {
            Sink::take(values[1][11]);
        }
        machine Root::nested_thirteen(values: [[Token; 13]; 2]) {
            Sink::take(values[1][12]);
            Sink::take(values[0][1]);
        }
        machine Root::same_outer_thirteen(values: [[Token; 13]; 2]) {
            Sink::take(values[0][0]);
            Sink::take(values[0][12]);
        }
        machine Root::one_thirteen(values: [[Token; 13]; 2]) {
            Sink::take(values[1][12]);
        }
        machine Root::nested_fourteen(values: [[Token; 14]; 2]) {
            Sink::take(values[1][13]);
            Sink::take(values[0][1]);
        }
        machine Root::same_outer_fourteen(values: [[Token; 14]; 2]) {
            Sink::take(values[0][0]);
            Sink::take(values[0][13]);
        }
        machine Root::one_fourteen(values: [[Token; 14]; 2]) {
            Sink::take(values[1][13]);
        }
        machine Root::nested_fifteen(values: [[Token; 15]; 2]) {
            Sink::take(values[1][14]);
            Sink::take(values[0][1]);
        }
        machine Root::same_outer_fifteen(values: [[Token; 15]; 2]) {
            Sink::take(values[0][0]);
            Sink::take(values[0][14]);
        }
        machine Root::one_fifteen(values: [[Token; 15]; 2]) {
            Sink::take(values[1][14]);
        }
        machine Root::nested_sixteen(values: [[Token; 16]; 2]) {
            Sink::take(values[1][15]);
            Sink::take(values[0][1]);
        }
        machine Root::same_outer_sixteen(values: [[Token; 16]; 2]) {
            Sink::take(values[0][0]);
            Sink::take(values[0][15]);
        }
        machine Root::one_sixteen(values: [[Token; 16]; 2]) {
            Sink::take(values[1][15]);
        }
        machine Root::too_wide(values: [[Token; 17]; 2]) {
            Sink::take(values[1][16]);
            Sink::take(values[0][1]);
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine_named(&checked, "nested"))
        .expect("one leaf move per outer array leaves four exact residual leaves");
    let path = |path: &[CheckedUnitStructuralPathSegment]| match path {
        [
            CheckedUnitStructuralPathSegment::FixedIndex(outer),
            CheckedUnitStructuralPathSegment::FixedIndex(inner),
        ] => (*outer, *inner),
        _ => panic!("nested array leaf has exactly two literal indices"),
    };
    assert_eq!(
        plan.machine.operations[..2]
            .iter()
            .map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } => path(&structural_arguments[0].path),
                _ => panic!("nested cleanup contains calls before return"),
            })
            .collect::<Vec<_>>(),
        vec![(1, 0), (0, 1)],
        "authored move order is retained",
    );
    assert_eq!(
        plan.residual_affine_discards
            .iter()
            .map(|discard| path(&discard.path))
            .collect::<Vec<_>>(),
        vec![(1, 2), (1, 1), (0, 2), (0, 0)],
        "outer and inner live complements both descend",
    );
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine_named(&checked, "nested_four"))
        .expect("one leaf move per outer quartet leaves six exact residual leaves");
    assert_eq!(
        plan.machine.operations[..2]
            .iter()
            .map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } => path(&structural_arguments[0].path),
                _ => panic!("nested quartet cleanup contains calls before return"),
            })
            .collect::<Vec<_>>(),
        vec![(1, 3), (0, 1)],
        "authored nested-quartet move order is retained",
    );
    assert_eq!(
        plan.residual_affine_discards
            .iter()
            .map(|discard| path(&discard.path))
            .collect::<Vec<_>>(),
        vec![(1, 2), (1, 1), (1, 0), (0, 3), (0, 2), (0, 0)],
        "quartet outer and inner live complements both descend",
    );
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine_named(&checked, "nested_five"))
        .expect("one leaf move per outer quintet leaves eight exact residual leaves");
    assert_eq!(
        plan.machine.operations[..2]
            .iter()
            .map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } => path(&structural_arguments[0].path),
                _ => panic!("nested quintet cleanup contains calls before return"),
            })
            .collect::<Vec<_>>(),
        vec![(1, 4), (0, 1)],
        "authored nested-quintet move order is retained",
    );
    assert_eq!(
        plan.residual_affine_discards
            .iter()
            .map(|discard| path(&discard.path))
            .collect::<Vec<_>>(),
        vec![
            (1, 3),
            (1, 2),
            (1, 1),
            (1, 0),
            (0, 4),
            (0, 3),
            (0, 2),
            (0, 0),
        ],
        "quintet outer and inner live complements both descend",
    );
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine_named(&checked, "nested_six"))
        .expect("one leaf move per outer sextet leaves ten exact residual leaves");
    assert_eq!(
        plan.machine.operations[..2]
            .iter()
            .map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } => path(&structural_arguments[0].path),
                _ => panic!("nested sextet cleanup contains calls before return"),
            })
            .collect::<Vec<_>>(),
        vec![(1, 5), (0, 1)],
        "authored nested-sextet move order is retained",
    );
    assert_eq!(
        plan.residual_affine_discards
            .iter()
            .map(|discard| path(&discard.path))
            .collect::<Vec<_>>(),
        vec![
            (1, 4),
            (1, 3),
            (1, 2),
            (1, 1),
            (1, 0),
            (0, 5),
            (0, 4),
            (0, 3),
            (0, 2),
            (0, 0),
        ],
        "sextet outer and inner live complements both descend",
    );
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine_named(&checked, "nested_seven"))
        .expect("one leaf move per outer septet leaves twelve exact residual leaves");
    assert_eq!(
        plan.machine.operations[..2]
            .iter()
            .map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } => path(&structural_arguments[0].path),
                _ => panic!("nested septet cleanup contains calls before return"),
            })
            .collect::<Vec<_>>(),
        vec![(1, 6), (0, 1)],
        "authored nested-septet move order is retained",
    );
    assert_eq!(
        plan.residual_affine_discards
            .iter()
            .map(|discard| path(&discard.path))
            .collect::<Vec<_>>(),
        vec![
            (1, 5),
            (1, 4),
            (1, 3),
            (1, 2),
            (1, 1),
            (1, 0),
            (0, 6),
            (0, 5),
            (0, 4),
            (0, 3),
            (0, 2),
            (0, 0),
        ],
        "septet outer and inner live complements both descend",
    );
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine_named(&checked, "nested_eight"))
        .expect("one leaf move per outer octet leaves fourteen exact residual leaves");
    assert_eq!(
        plan.machine.operations[..2]
            .iter()
            .map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } => path(&structural_arguments[0].path),
                _ => panic!("nested octet cleanup contains calls before return"),
            })
            .collect::<Vec<_>>(),
        vec![(1, 7), (0, 1)],
        "authored nested-octet move order is retained",
    );
    assert_eq!(
        plan.residual_affine_discards
            .iter()
            .map(|discard| path(&discard.path))
            .collect::<Vec<_>>(),
        vec![
            (1, 6),
            (1, 5),
            (1, 4),
            (1, 3),
            (1, 2),
            (1, 1),
            (1, 0),
            (0, 7),
            (0, 6),
            (0, 5),
            (0, 4),
            (0, 3),
            (0, 2),
            (0, 0),
        ],
        "octet outer and inner live complements both descend",
    );
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine_named(&checked, "nested_nine"))
        .expect("one leaf move per outer nonet leaves sixteen exact residual leaves");
    assert_eq!(
        plan.machine.operations[..2]
            .iter()
            .map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } => path(&structural_arguments[0].path),
                _ => panic!("nested nonet cleanup contains calls before return"),
            })
            .collect::<Vec<_>>(),
        vec![(1, 8), (0, 1)],
        "authored nested-nonet move order is retained",
    );
    assert_eq!(
        plan.residual_affine_discards
            .iter()
            .map(|discard| path(&discard.path))
            .collect::<Vec<_>>(),
        vec![
            (1, 7),
            (1, 6),
            (1, 5),
            (1, 4),
            (1, 3),
            (1, 2),
            (1, 1),
            (1, 0),
            (0, 8),
            (0, 7),
            (0, 6),
            (0, 5),
            (0, 4),
            (0, 3),
            (0, 2),
            (0, 0),
        ],
        "nonet outer and inner live complements both descend",
    );
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine_named(&checked, "nested_ten"))
        .expect("one leaf move per outer decet leaves eighteen exact residual leaves");
    assert_eq!(
        plan.machine.operations[..2]
            .iter()
            .map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } => path(&structural_arguments[0].path),
                _ => panic!("nested decet cleanup contains calls before return"),
            })
            .collect::<Vec<_>>(),
        vec![(1, 9), (0, 1)],
        "authored nested-decet move order is retained",
    );
    assert_eq!(
        plan.residual_affine_discards
            .iter()
            .map(|discard| path(&discard.path))
            .collect::<Vec<_>>(),
        vec![
            (1, 8),
            (1, 7),
            (1, 6),
            (1, 5),
            (1, 4),
            (1, 3),
            (1, 2),
            (1, 1),
            (1, 0),
            (0, 9),
            (0, 8),
            (0, 7),
            (0, 6),
            (0, 5),
            (0, 4),
            (0, 3),
            (0, 2),
            (0, 0),
        ],
        "decet outer and inner live complements both descend",
    );
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine_named(&checked, "nested_eleven"))
        .expect("one leaf move per outer length-eleven array leaves twenty exact residual leaves");
    assert_eq!(
        plan.machine.operations[..2]
            .iter()
            .map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } => path(&structural_arguments[0].path),
                _ => panic!("nested length-eleven cleanup contains calls before return"),
            })
            .collect::<Vec<_>>(),
        vec![(1, 10), (0, 1)],
        "authored nested length-eleven move order is retained",
    );
    assert_eq!(
        plan.residual_affine_discards
            .iter()
            .map(|discard| path(&discard.path))
            .collect::<Vec<_>>(),
        vec![
            (1, 9),
            (1, 8),
            (1, 7),
            (1, 6),
            (1, 5),
            (1, 4),
            (1, 3),
            (1, 2),
            (1, 1),
            (1, 0),
            (0, 10),
            (0, 9),
            (0, 8),
            (0, 7),
            (0, 6),
            (0, 5),
            (0, 4),
            (0, 3),
            (0, 2),
            (0, 0),
        ],
        "length-eleven outer and inner live complements both descend",
    );
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine_named(&checked, "nested_twelve"))
        .expect(
            "one leaf move per outer length-twelve array leaves twenty-two exact residual leaves",
        );
    assert_eq!(
        plan.machine.operations[..2]
            .iter()
            .map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } => path(&structural_arguments[0].path),
                _ => panic!("nested length-twelve cleanup contains calls before return"),
            })
            .collect::<Vec<_>>(),
        vec![(1, 11), (0, 1)],
        "authored nested length-twelve move order is retained",
    );
    assert_eq!(
        plan.residual_affine_discards
            .iter()
            .map(|discard| path(&discard.path))
            .collect::<Vec<_>>(),
        vec![
            (1, 10),
            (1, 9),
            (1, 8),
            (1, 7),
            (1, 6),
            (1, 5),
            (1, 4),
            (1, 3),
            (1, 2),
            (1, 1),
            (1, 0),
            (0, 11),
            (0, 10),
            (0, 9),
            (0, 8),
            (0, 7),
            (0, 6),
            (0, 5),
            (0, 4),
            (0, 3),
            (0, 2),
            (0, 0),
        ],
        "length-twelve outer and inner live complements both descend",
    );
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine_named(&checked, "nested_thirteen"))
        .expect("one leaf move per outer length-thirteen array leaves twenty-four exact residual leaves");
    assert_eq!(
        plan.machine.operations[..2]
            .iter()
            .map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } => path(&structural_arguments[0].path),
                _ => panic!("nested length-thirteen cleanup contains calls before return"),
            })
            .collect::<Vec<_>>(),
        vec![(1, 12), (0, 1)],
        "authored nested length-thirteen move order is retained",
    );
    assert_eq!(
        plan.residual_affine_discards
            .iter()
            .map(|discard| path(&discard.path))
            .collect::<Vec<_>>(),
        vec![
            (1, 11),
            (1, 10),
            (1, 9),
            (1, 8),
            (1, 7),
            (1, 6),
            (1, 5),
            (1, 4),
            (1, 3),
            (1, 2),
            (1, 1),
            (1, 0),
            (0, 12),
            (0, 11),
            (0, 10),
            (0, 9),
            (0, 8),
            (0, 7),
            (0, 6),
            (0, 5),
            (0, 4),
            (0, 3),
            (0, 2),
            (0, 0),
        ],
        "length-thirteen outer and inner live complements both descend",
    );
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine_named(&checked, "nested_fourteen"))
        .expect(
            "one leaf move per outer length-fourteen array leaves twenty-six exact residual leaves",
        );
    assert_eq!(
        plan.machine.operations[..2]
            .iter()
            .map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } => path(&structural_arguments[0].path),
                _ => panic!("nested length-fourteen cleanup contains calls before return"),
            })
            .collect::<Vec<_>>(),
        vec![(1, 13), (0, 1)],
        "authored nested length-fourteen move order is retained",
    );
    assert_eq!(
        plan.residual_affine_discards
            .iter()
            .map(|discard| path(&discard.path))
            .collect::<Vec<_>>(),
        vec![
            (1, 12),
            (1, 11),
            (1, 10),
            (1, 9),
            (1, 8),
            (1, 7),
            (1, 6),
            (1, 5),
            (1, 4),
            (1, 3),
            (1, 2),
            (1, 1),
            (1, 0),
            (0, 13),
            (0, 12),
            (0, 11),
            (0, 10),
            (0, 9),
            (0, 8),
            (0, 7),
            (0, 6),
            (0, 5),
            (0, 4),
            (0, 3),
            (0, 2),
            (0, 0),
        ],
        "length-fourteen outer and inner live complements both descend",
    );
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine_named(&checked, "nested_fifteen"))
        .expect(
            "one leaf move per outer length-fifteen array leaves twenty-eight exact residual leaves",
        );
    assert_eq!(
        plan.machine.operations[..2]
            .iter()
            .map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } => path(&structural_arguments[0].path),
                _ => panic!("nested length-fifteen cleanup contains calls before return"),
            })
            .collect::<Vec<_>>(),
        vec![(1, 14), (0, 1)],
        "authored nested length-fifteen move order is retained",
    );
    assert_eq!(
        plan.residual_affine_discards
            .iter()
            .map(|discard| path(&discard.path))
            .collect::<Vec<_>>(),
        vec![
            (1, 13),
            (1, 12),
            (1, 11),
            (1, 10),
            (1, 9),
            (1, 8),
            (1, 7),
            (1, 6),
            (1, 5),
            (1, 4),
            (1, 3),
            (1, 2),
            (1, 1),
            (1, 0),
            (0, 14),
            (0, 13),
            (0, 12),
            (0, 11),
            (0, 10),
            (0, 9),
            (0, 8),
            (0, 7),
            (0, 6),
            (0, 5),
            (0, 4),
            (0, 3),
            (0, 2),
            (0, 0),
        ],
        "length-fifteen outer and inner live complements both descend",
    );
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine_named(&checked, "nested_sixteen"))
        .expect("one leaf move per outer length-sixteen array leaves thirty exact residual leaves");
    assert_eq!(
        plan.machine.operations[..2]
            .iter()
            .map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } => path(&structural_arguments[0].path),
                _ => panic!("nested length-sixteen cleanup contains calls before return"),
            })
            .collect::<Vec<_>>(),
        vec![(1, 15), (0, 1)],
        "authored nested length-sixteen move order is retained",
    );
    assert_eq!(
        plan.residual_affine_discards
            .iter()
            .map(|discard| path(&discard.path))
            .collect::<Vec<_>>(),
        vec![
            (1, 14),
            (1, 13),
            (1, 12),
            (1, 11),
            (1, 10),
            (1, 9),
            (1, 8),
            (1, 7),
            (1, 6),
            (1, 5),
            (1, 4),
            (1, 3),
            (1, 2),
            (1, 1),
            (1, 0),
            (0, 15),
            (0, 14),
            (0, 13),
            (0, 12),
            (0, 11),
            (0, 10),
            (0, 9),
            (0, 8),
            (0, 7),
            (0, 6),
            (0, 5),
            (0, 4),
            (0, 3),
            (0, 2),
            (0, 0),
        ],
        "length-sixteen outer and inner live complements both descend",
    );
    for (same_outer, one_move, length, second_move) in [
        ("same_outer", "one", 3, 1),
        ("same_outer_four", "one_four", 4, 3),
        ("same_outer_five", "one_five", 5, 4),
        ("same_outer_six", "one_six", 6, 5),
        ("same_outer_seven", "one_seven", 7, 6),
        ("same_outer_eight", "one_eight", 8, 7),
        ("same_outer_nine", "one_nine", 9, 8),
        ("same_outer_ten", "one_ten", 10, 9),
        ("same_outer_eleven", "one_eleven", 11, 10),
        ("same_outer_twelve", "one_twelve", 12, 11),
        ("same_outer_thirteen", "one_thirteen", 13, 12),
        ("same_outer_fourteen", "one_fourteen", 14, 13),
        ("same_outer_fifteen", "one_fifteen", 15, 14),
        ("same_outer_sixteen", "one_sixteen", 16, 15),
    ] {
        let array_type = format!("array(named(name(Token)),literal({length}))");
        // Outer element 1 is untouched and remains one maximal array root.
        let mut residuals = vec![(fixed_cleanup_path(&[1]), array_type.clone())];
        residuals.extend(
            (1..length)
                .rev()
                .filter(|index| *index != second_move)
                .map(|index| {
                    (
                        fixed_cleanup_path(&[0, index]),
                        "named(name(Token))".to_owned(),
                    )
                }),
        );
        assert_token_cleanup_partition(
            &checked,
            same_outer,
            &[
                fixed_cleanup_path(&[0, 0]),
                fixed_cleanup_path(&[0, second_move]),
            ],
            &residuals,
        );

        // Outer element 1 cleans first; untouched element 0 stays whole and last.
        let mut residuals = (0..length - 1)
            .rev()
            .map(|index| {
                (
                    fixed_cleanup_path(&[1, index]),
                    "named(name(Token))".to_owned(),
                )
            })
            .collect::<Vec<_>>();
        residuals.push((fixed_cleanup_path(&[0]), array_type));
        assert_token_cleanup_partition(
            &checked,
            one_move,
            &[fixed_cleanup_path(&[1, length - 1])],
            &residuals,
        );
    }
    let mut residuals = (0..16)
        .rev()
        .map(|index| {
            (
                fixed_cleanup_path(&[1, index]),
                "named(name(Token))".to_owned(),
            )
        })
        .collect::<Vec<_>>();
    residuals.extend((0..17).rev().filter(|index| *index != 1).map(|index| {
        (
            fixed_cleanup_path(&[0, index]),
            "named(name(Token))".to_owned(),
        )
    }));
    assert_token_cleanup_partition(
        &checked,
        "too_wide",
        &[fixed_cleanup_path(&[1, 16]), fixed_cleanup_path(&[0, 1])],
        &residuals,
    );
}

#[test]
fn affine_triple_partial_cleanup_rejects_nominal_elements_and_qualification() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        data Sink {}
        machine Sink::take(token: Token) {}

        data NominalToken { value: u64; }
        machine NominalToken::drop(&mut self) {}
        machine Sink::take_nominal(token: NominalToken) {}

        domain [Token; 3]::Ready
        requires
            true;

        data Root {}
        machine Root::nominal(values: [NominalToken; 3]) {
            Sink::take_nominal(values[0]);
            Sink::take_nominal(values[1]);
        }
        machine Root::qualified(values: [Token; 3] in Ready) {
            Sink::take(values[0]);
            Sink::take(values[1]);
        }
        "#,
    );
    for machine in ["nominal", "qualified"] {
        assert!(
            checked
                .facts
                .flow
                .terminal_partial_affine_unit_cleanups
                .for_machine(machine_named(&checked, machine))
                .is_none(),
            "`{machine}` is outside the exact claim-free unqualified structural-affine carrier"
        );
    }
}
