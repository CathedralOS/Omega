//! Exact copy-before-left-operand comparison fixtures.

use super::*;

pub(crate) fn compare_i64_left_operand_fixture() -> Fixture {
    let mut fixture = compare_fixture();
    let x0 = fixture.physical.model().view_named("x0").unwrap().clone();
    let x1 = fixture.physical.model().view_named("x1").unwrap().clone();
    let x30 = fixture.physical.model().view_named("x30").unwrap().clone();
    let nzcv = fixture.physical.model().view_named("nzcv").unwrap().clone();

    let compare = &mut fixture.selected.functions[0].blocks[0].instructions[1];
    compare.kind = SelectedInstructionKind::CompareI64;
    compare.operands.push(selected_operand(
        1,
        3,
        RegisterOperandAccess::Use,
        x1.class,
        None,
    ));
    compare.provenance.values = vec![
        ValueId::new(1).unwrap(),
        ValueId::new(2).unwrap(),
        ValueId::new(3).unwrap(),
    ];
    fixture.selected.functions[0].virtual_registers = vec![
        VirtualRegister {
            id: VirtualRegisterId(1),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            class: x0.class,
            origin: VirtualRegisterOrigin::EntryParameter {
                source_value: ValueId::new(1).unwrap(),
                parameter_index: 0,
            },
            definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
            entry_fixed_view: Some(x0.id),
        },
        VirtualRegister {
            id: VirtualRegisterId(2),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            class: x0.class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(1),
                source_value: ValueId::new(1).unwrap(),
            },
            definition_site: Some(ValueDefinitionSite::Node {
                block: fixture.selected.functions[0].blocks[0].source_block,
                node: 0,
            }),
            entry_fixed_view: None,
        },
        VirtualRegister {
            id: VirtualRegisterId(3),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            class: x1.class,
            origin: VirtualRegisterOrigin::EntryParameter {
                source_value: ValueId::new(2).unwrap(),
                parameter_index: 1,
            },
            definition_site: Some(ValueDefinitionSite::FunctionParameter(1)),
            entry_fixed_view: Some(x1.id),
        },
    ];

    let through =
        super::super::sorted_units(x0.units.iter().chain(&x1.units).chain(&x30.units).copied());
    let live_block = &mut fixture.liveness.functions[0].blocks[0];
    live_block.instructions[0].virtual_live_in = vec![VirtualRegisterId(1), VirtualRegisterId(3)];
    live_block.instructions[0].virtual_live_out = vec![VirtualRegisterId(2), VirtualRegisterId(3)];
    live_block.instructions[0].unit_live_in = through.clone();
    live_block.instructions[0].unit_live_out = through.clone();
    live_block.instructions[1].virtual_uses = vec![VirtualRegisterId(2), VirtualRegisterId(3)];
    live_block.instructions[1].virtual_live_in = vec![VirtualRegisterId(2), VirtualRegisterId(3)];
    live_block.instructions[1].unit_uses =
        super::super::sorted_units(x0.units.iter().chain(&x1.units).copied());
    live_block.instructions[1].unit_live_in = through;
    live_block.instructions[1].unit_defs = nzcv.write_units.clone();

    let machine_compare = &mut fixture.source.functions[0].blocks[0].instructions[1];
    machine_compare.alternative = alternative(
        MachineAlternativeFamily::CompareI64,
        MachineEncodedEffects {
            external_operand_reads: vec![0, 1],
            external_operand_writes: vec![],
            implicit_unit_uses: vec![],
            implicit_unit_defs: nzcv.units.clone(),
            implicit_unit_clobbers: vec![],
            memory: MachineEncodedMemoryEffect::NoneV1,
            stack: MachineEncodedStackEffect::UnchangedV1,
            trap: MachineEncodedTrapBehavior::NeverV1,
            control: MachineEncodedControlEffect::FallThroughV1,
        },
    );
    machine_compare
        .operands
        .push(physical_operand(1, 3, RegisterOperandAccess::Use, &x1));
    machine_compare.unit_uses =
        super::super::sorted_units(x0.units.iter().chain(&x1.units).copied());
    machine_compare.unit_defs = nzcv.write_units;
    fixture
}

pub(crate) fn two_pair_compare_i64_left_operand_fixture() -> Fixture {
    let mut fixture = compare_i64_left_operand_fixture();
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
