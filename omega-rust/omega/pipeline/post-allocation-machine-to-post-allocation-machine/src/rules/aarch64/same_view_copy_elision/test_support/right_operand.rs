//! Optimizer module role: test fixture leaf. Ordered-right CompareI64 variants.

use semantic_vocabulary::MachineId;

use super::fixture::{Fixture, compare_i64_left_operand_fixture};

pub(crate) fn compare_i64_right_operand_fixture() -> Fixture {
    let mut fixture = compare_i64_left_operand_fixture();

    let compare = &mut fixture.selected.functions[0].blocks[0].instructions[1];
    compare.operands.swap(0, 1);
    compare.operands[0].operand = 0;
    compare.operands[1].operand = 1;

    let machine_compare = &mut fixture.source.functions[0].blocks[0].instructions[1];
    machine_compare.operands.swap(0, 1);
    machine_compare.operands[0].operand = 0;
    machine_compare.operands[1].operand = 1;

    fixture
}

pub(crate) fn two_pair_compare_i64_right_operand_fixture() -> Fixture {
    let mut fixture = compare_i64_right_operand_fixture();
    let second_machine = MachineId::new(2).unwrap();

    let mut selected = fixture.selected.functions[0].clone();
    selected.machine = second_machine;
    fixture.selected.functions.push(selected);

    let mut liveness = fixture.liveness.functions[0].clone();
    liveness.machine = second_machine;
    fixture.liveness.functions.push(liveness);

    let mut source = fixture.source.functions[0].clone();
    source.machine = second_machine;
    fixture.source.functions.push(source);

    fixture
}
