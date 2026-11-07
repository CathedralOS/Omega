//! Exercise exact snapshot source resolution with the admitted target operand rows.
use super::*;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlockId, SelectedBlockOrigin, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionProvenance, SelectedOperand, SelectedTerminator,
};
use semantic_vocabulary::{BlockId, EdgeId};

fn instruction(
    environment: &ValidatedTargetRegisterEnvironment,
    id: u32,
    kind: SelectedInstructionKind,
    registers: &[u32],
) -> SelectedInstruction {
    let keys = environment.selected_keys();
    let key = match kind {
        SelectedInstructionKind::MaterializeI64 { .. } => keys.materialize_i64,
        SelectedInstructionKind::AddressOffset { .. } => keys.address_offset.unwrap(),
        SelectedInstructionKind::CopyBytes => keys.copy_bytes.unwrap(),
        SelectedInstructionKind::ReturnUnit => keys.return_unit,
        _ => panic!("unsupported test instruction"),
    };
    let row = environment.constraints().constraint(key).unwrap();
    assert_eq!(row.operands.len(), registers.len());
    SelectedInstruction {
        id: SelectedInstructionId(id),
        kind,
        constraint: key,
        operands: row
            .operands
            .iter()
            .zip(registers)
            .map(|(operand, register)| SelectedOperand {
                operand: operand.operand,
                virtual_register: VirtualRegisterId(*register),
                access: operand.access,
                class: operand.class,
                fixed_view: operand.fixed_view,
                tied_to: operand.tied_to,
                early_clobber: operand.early_clobber,
            })
            .collect(),
        implicit_uses: row.implicit_uses.clone(),
        implicit_defs: row.implicit_defs.clone(),
        clobbers: row.clobbers.clone(),
        provenance: SelectedInstructionProvenance::default(),
    }
}

fn block(environment: &ValidatedTargetRegisterEnvironment, offset: u32) -> SelectedBlock {
    let mut instructions = vec![];
    if offset != 0 {
        instructions.push(instruction(
            environment,
            0,
            SelectedInstructionKind::AddressOffset {
                byte_offset: offset,
            },
            &[0, 5],
        ));
    }
    instructions.push(instruction(
        environment,
        1,
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(8192),
        },
        &[2],
    ));
    instructions.push(instruction(
        environment,
        2,
        SelectedInstructionKind::CopyBytes,
        &[if offset == 0 { 0 } else { 5 }, 1, 2, 3, 4],
    ));
    SelectedBlock {
        id: SelectedBlockId(0),
        origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
        instructions,
        terminator: SelectedTerminator::Return {
            instruction: instruction(environment, 99, SelectedInstructionKind::ReturnUnit, &[]),
            psi_return_edge: EdgeId::new(1).unwrap(),
        },
    }
}

fn resolve(block: &SelectedBlock, offset: u32) -> Result<Option<usize>, RuntimeSpillError> {
    source_use(
        block,
        block.instructions.len() - 1,
        VirtualRegisterId(0),
        offset,
        8192,
        &mut 0,
        100,
    )
}

#[test]
fn bulk_snapshot_exact_zero_and_nonzero_offsets_name_the_actual_pointer_use() {
    let environment = register_environment::baseline_target_register_environment(
        target::NativeTarget::linux_arm64(),
    )
    .unwrap();
    let direct = block(&environment, 0);
    assert_eq!(resolve(&direct, 0), Ok(Some(1)));
    let displaced = block(&environment, 17);
    assert_eq!(resolve(&displaced, 17), Ok(Some(0)));
}

#[test]
fn bulk_snapshot_wrong_redefined_and_unknown_counts_refuse() {
    let environment = register_environment::baseline_target_register_environment(
        target::NativeTarget::linux_arm64(),
    )
    .unwrap();
    let mut wrong = block(&environment, 0);
    wrong.instructions[0].kind = SelectedInstructionKind::MaterializeI64 {
        value: IntegerValue::Unsigned(8191),
    };
    assert_eq!(resolve(&wrong, 0), Err(RuntimeSpillError::UnsupportedUse));
    let mut redefined = block(&environment, 0);
    redefined.instructions.insert(
        1,
        instruction(
            &environment,
            3,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(8191),
            },
            &[2],
        ),
    );
    assert_eq!(
        resolve(&redefined, 0),
        Err(RuntimeSpillError::UnsupportedUse)
    );
    let mut unknown = block(&environment, 0);
    unknown.instructions.remove(0);
    assert_eq!(resolve(&unknown, 0), Err(RuntimeSpillError::UnsupportedUse));
}

#[test]
fn bulk_snapshot_wrong_or_redefined_address_never_names_the_binding() {
    let environment = register_environment::baseline_target_register_environment(
        target::NativeTarget::linux_arm64(),
    )
    .unwrap();
    let mut wrong_offset = block(&environment, 17);
    wrong_offset.instructions[0].kind = SelectedInstructionKind::AddressOffset { byte_offset: 18 };
    assert_eq!(
        resolve(&wrong_offset, 17),
        Err(RuntimeSpillError::UnsupportedUse)
    );
    let mut unrelated = block(&environment, 17);
    unrelated.instructions[0].operands[0].virtual_register = VirtualRegisterId(6);
    assert_eq!(resolve(&unrelated, 17), Ok(None));
    let mut redefined = block(&environment, 17);
    redefined.instructions.insert(
        2,
        instruction(
            &environment,
            3,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(17),
            },
            &[5],
        ),
    );
    assert_eq!(resolve(&redefined, 17), Ok(None));
    let mut unknown = block(&environment, 17);
    unknown.instructions.remove(0);
    assert_eq!(resolve(&unknown, 17), Ok(None));
}

#[test]
fn bulk_snapshot_definition_walk_keeps_the_existing_work_budget() {
    let environment = register_environment::baseline_target_register_environment(
        target::NativeTarget::linux_arm64(),
    )
    .unwrap();
    let block = block(&environment, 17);
    let mut steps = 0;
    assert_eq!(
        source_use(&block, 2, VirtualRegisterId(0), 17, 8192, &mut steps, 0),
        Err(RuntimeSpillError::WorkBudgetExceeded)
    );
    assert_eq!(steps, 1);
}
