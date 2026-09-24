//! Fixtures shared by the runtime spill control flow tests: successors and
//! the CFG fixtures.

mod cyclic_parameters_and_replays;
mod edge_snapshots_and_reloads;
mod slot_reuse;

use crate::ValidatedRuntimeSpill;
use crate::rewrites::runtime_spill::admission;
use crate::rewrites::runtime_spill::tests::{
    Arc, BlockId, EdgeId, NativeTarget, SelectedBlock, SelectedBlockId, SelectedInstructionId,
    SelectedInstructionKind, SelectedTerminator, ValueDefinitionSite, ValueId, VirtualRegister,
    VirtualRegisterId, VirtualRegisterOrigin, baseline_target_register_environment, fixture,
    selected_instruction_plan_identity,
};
use selected_instructions::{
    SelectedBlockOrigin, SelectedBoundarySettlement, SelectedBoundarySettlementPayload,
    SelectedSuccessor, SelectedSuccessorRole, SelectedValueBinding, SelectedValueTransport,
};
use semantic_vocabulary::{BoundaryMachineId, OperationId};

pub(super) fn successor(destination: u32) -> SelectedSuccessor {
    SelectedSuccessor {
        role: SelectedSuccessorRole::EdgeTransferContinuation,
        psi_edge: EdgeId::new(2).unwrap(),
        block: SelectedBlockId(destination),
        source_target: BlockId::new(3).unwrap(),
        bindings: Vec::new(),
        structural_bindings: Vec::new(),
        structural_case: None,
        fuel: Vec::new(),
    }
}

pub(super) fn cfg_fixture(target: NativeTarget) -> ValidatedRuntimeSpill {
    let mut source = fixture(target);
    let environment = baseline_target_register_environment(target).unwrap();
    let jump_row = environment
        .constraint(environment.selected_keys().jump)
        .unwrap();
    let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
    let mut body = function.blocks.remove(0);
    body.id = SelectedBlockId(1);
    body.origin = SelectedBlockOrigin::EdgeTransfer {
        edge: EdgeId::new(2).unwrap(),
        target: BlockId::new(3).unwrap(),
    };
    let mut exit = SelectedBlock {
        id: SelectedBlockId(2),
        origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
        instructions: Vec::new(),
        terminator: body.terminator.clone(),
    };
    let SelectedTerminator::Return { instruction, .. } = &mut exit.terminator else {
        unreachable!();
    };
    instruction.id = SelectedInstructionId(1000);
    body.terminator = SelectedTerminator::Jump {
        instruction: admission::instruction(
            SelectedInstructionId(101),
            SelectedInstructionKind::Jump,
            jump_row,
            &[],
        ),
        successor: successor(2),
    };
    function.blocks = vec![
        SelectedBlock {
            id: SelectedBlockId(0),
            origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::Jump {
                instruction: admission::instruction(
                    SelectedInstructionId(100),
                    SelectedInstructionKind::Jump,
                    jump_row,
                    &[],
                ),
                successor: successor(1),
            },
        },
        body,
        exit,
    ];
    function.virtual_registers[1].definition_site = Some(ValueDefinitionSite::BlockParameter {
        block: BlockId::new(3).unwrap(),
        position: 0,
    });
    for (block, instruction_index) in [(0, 0), (1, 0), (1, 1), (1, 4), (2, 0)] {
        function
            .boundary_settlements
            .push(SelectedBoundarySettlement {
                block: SelectedBlockId(block),
                instruction_index,
                settlement: SelectedBoundarySettlementPayload::HostedWriteByteI32 {
                    operation: OperationId::new(1).unwrap(),
                    boundary: BoundaryMachineId::new(1).unwrap(),
                    source: ValueId::new(1).unwrap(),
                },
            });
    }
    let identity = selected_instruction_plan_identity(source.transformed());
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    source
}

/// A loop-carried parameter: the destination's terminator gains a back edge
/// through a dedicated edge-transfer block that rebinds the parameter from a
/// loop-body value (or, when `passthrough`, from the parameter itself). The
/// destination still dominates every use and every arrival stores its bound
/// argument before the destination executes.
fn cyclic_parameter_fixture(target: NativeTarget, passthrough: bool) -> ValidatedRuntimeSpill {
    let mut source = super::parameters::parameter_fixture(target);
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let copy = environment.constraint(keys.copy_i64).unwrap();
    let jump = environment.constraint(keys.jump).unwrap();
    let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
    let scalar_type = function.virtual_registers[1].scalar_type;
    let class = function.virtual_registers[1].class;
    let terminal = function.blocks[2].terminator.clone();
    // Register 7 is the back-edge arrival's bound argument: one CopyI64 in the
    // edge-transfer block produces it, exactly like the entry-side arrivals.
    function.virtual_registers.push(VirtualRegister {
        id: VirtualRegisterId(7),
        scalar_type,
        class,
        origin: VirtualRegisterOrigin::InstructionResult {
            instruction: SelectedInstructionId(501),
            source_value: ValueId::new(2).unwrap(),
        },
        definition_site: function.virtual_registers[5].definition_site,
        entry_fixed_view: None,
    });
    let mut back = successor(2);
    back.bindings.push(SelectedValueBinding {
        semantic: abstract_operations::ValueBinding {
            parameter: ValueId::new(2).unwrap(),
            argument: ValueId::new(2).unwrap(),
            scalar_type,
        },
        transport: SelectedValueTransport::Registers {
            argument: VirtualRegisterId(7),
            parameter: VirtualRegisterId(1),
        },
    });
    function.blocks[2].terminator = SelectedTerminator::ConditionalBranch {
        instruction: admission::instruction(
            SelectedInstructionId(2000),
            SelectedInstructionKind::Jump,
            jump,
            &[],
        ),
        when_nonzero: successor(5),
        when_zero: successor(4),
    };
    function.blocks.push(SelectedBlock {
        id: SelectedBlockId(4),
        origin: SelectedBlockOrigin::EdgeTransfer {
            edge: EdgeId::new(2).unwrap(),
            target: BlockId::new(3).unwrap(),
        },
        instructions: vec![admission::instruction(
            SelectedInstructionId(501),
            SelectedInstructionKind::CopyI64,
            copy,
            &[
                VirtualRegisterId(if passthrough { 1 } else { 5 }),
                VirtualRegisterId(7),
            ],
        )],
        terminator: SelectedTerminator::Jump {
            instruction: admission::instruction(
                SelectedInstructionId(500),
                SelectedInstructionKind::Jump,
                jump,
                &[],
            ),
            successor: back,
        },
    });
    function.blocks.push(SelectedBlock {
        id: SelectedBlockId(5),
        origin: SelectedBlockOrigin::Source(BlockId::new(4).unwrap()),
        instructions: Vec::new(),
        terminator: terminal,
    });
    let identity = selected_instruction_plan_identity(source.transformed());
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    source
}
