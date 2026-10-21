//! Physical loops refine one operation; they do not introduce source states.
//!
//! Runtime profile arrays need bounded iteration, not a cross-product of
//! unrolled comparisons. The operation supplies the bound and scratch state;
//! this owner supplies only control. All bodies use ordinary selected forms
//! and the existing allocator, branch relaxation, and target encoders. Internal
//! branches retain their operation but cannot name an authored edge, bind source
//! parameters, transfer custody, or settle additional logical fuel.

use super::{Builder, SelectedBlock, SelectedBlockId, SelectedInstructionKind, VirtualRegisterId};
use crate::SelectedInstructionError;
use legalized_operations::LegalizedScalarInstruction;
use selected_instructions::{
    SelectedBlockOrigin, SelectedSuccessor, SelectedSuccessorRole, SelectedTerminator,
};
use semantic_vocabulary::{BlockId, OperationId};

#[derive(Clone, Copy)]
pub(super) struct OpenBlock {
    pub(super) id: SelectedBlockId,
    pub(super) origin: SelectedBlockOrigin,
    pub(super) start: usize,
}

fn failure() -> SelectedInstructionError {
    SelectedInstructionError::custody_because(
        "operation control lost its physical block or source anchor",
    )
}

pub(super) fn close(
    builder: &mut Builder<'_>,
    terminator: SelectedTerminator,
    body_end: usize,
) -> Result<SelectedBlock, SelectedInstructionError> {
    let open = builder.open_block.take().ok_or_else(failure)?;
    let instructions = builder
        .instructions
        .get(open.start..body_end)
        .ok_or_else(failure)?
        .to_vec();
    Ok(SelectedBlock {
        id: open.id,
        origin: open.origin,
        instructions,
        terminator,
    })
}

fn allocate(builder: &mut Builder<'_>) -> Result<SelectedBlockId, SelectedInstructionError> {
    let id = SelectedBlockId(builder.next_block);
    builder.next_block = builder.next_block.checked_add(1).ok_or_else(failure)?;
    Ok(id)
}

fn settle_preceding_operations(builder: &mut Builder<'_>) -> Result<(), SelectedInstructionError> {
    if builder.pending_provenance.operations.is_empty() {
        return Ok(());
    }
    // A zero-payload producer still executes before this loop. Put its pending
    // source/fuel attribution on a preheader instruction, never on the repeated
    // condition or an internal edge. This register is not a new source value.
    let instruction = super::SelectedInstructionId(
        builder
            .instructions
            .len()
            .try_into()
            .map_err(|_| failure())?,
    );
    let register = VirtualRegisterId(builder.registers.len().try_into().map_err(|_| failure())?);
    builder.registers.push(super::VirtualRegister {
        id: register,
        scalar_type: semantic_vocabulary::ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
                .map_err(|_| failure())?,
        ),
        class: builder.class,
        origin: super::VirtualRegisterOrigin::InstructionScratch {
            instruction,
            operand: 0,
        },
        definition_site: None,
        entry_fixed_view: None,
    });
    builder.emit_materialize_i64(
        semantic_vocabulary::IntegerValue::Unsigned(0),
        register,
        Default::default(),
    )
}

fn successor(source: BlockId, operation: OperationId, block: SelectedBlockId) -> SelectedSuccessor {
    SelectedSuccessor {
        role: SelectedSuccessorRole::OperationContinuation { operation },
        psi_edge: None,
        block,
        source_target: source,
        bindings: Vec::new(),
        structural_bindings: Vec::new(),
        structural_case: None,
        fuel: Vec::new(),
    }
}

fn finish(
    builder: &mut Builder<'_>,
    terminator: SelectedTerminator,
) -> Result<(), SelectedInstructionError> {
    let body_end = builder
        .instructions
        .len()
        .checked_sub(1)
        .ok_or_else(failure)?;
    let block = close(builder, terminator, body_end)?;
    builder.implementation_blocks.push(block);
    Ok(())
}

fn jump(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    source: BlockId,
    destination: SelectedBlockId,
) -> Result<(), SelectedInstructionError> {
    builder.emit(
        SelectedInstructionKind::Jump,
        builder.constraints.keys.jump,
        &[],
        super::structural::provenance(row),
    )?;
    let instruction = builder.instructions.last().cloned().ok_or_else(failure)?;
    finish(
        builder,
        SelectedTerminator::Jump {
            instruction,
            successor: successor(source, row.operation, destination),
        },
    )
}

/// Execute an operation-owned region conditionally. Inactive sum payloads must
/// not be read merely because their overlaid storage exists. Like a loop, this
/// branch refines an operation and creates no semantic edge or custody event.
pub(super) fn when_nonzero(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    predicate: VirtualRegisterId,
    body: impl FnOnce(&mut Builder<'_>) -> Result<(), SelectedInstructionError>,
) -> Result<(), SelectedInstructionError> {
    conditional_body(builder, row, predicate, true, body)
}

/// A failed implementation assertion has no source-level continuation or
/// cleanup disposition. It cannot be expressed as a fabricated Crash edge.
pub(super) fn require_nonzero(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    predicate: VirtualRegisterId,
) -> Result<(), SelectedInstructionError> {
    conditional_body(builder, row, predicate, false, |builder| {
        builder.emit(
            SelectedInstructionKind::Crash,
            builder.constraints.keys.crash,
            &[],
            super::structural::provenance(row),
        )?;
        let instruction = builder.instructions.last().cloned().ok_or_else(failure)?;
        finish(
            builder,
            SelectedTerminator::OperationTrap {
                instruction,
                operation: row.operation,
            },
        )
    })
}

fn conditional_body(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    predicate: VirtualRegisterId,
    on_nonzero: bool,
    body: impl FnOnce(&mut Builder<'_>) -> Result<(), SelectedInstructionError>,
) -> Result<(), SelectedInstructionError> {
    let open = builder.open_block.ok_or_else(failure)?;
    let source = match open.origin {
        SelectedBlockOrigin::Source(source) | SelectedBlockOrigin::Operation { source, .. } => {
            source
        }
        _ => return Err(failure()),
    };
    if builder.block_body_end.is_some() {
        return Err(failure());
    }
    settle_preceding_operations(builder)?;
    let selected = allocate(builder)?;
    let continuation = allocate(builder)?;
    builder.emit(
        SelectedInstructionKind::CompareI64Zero,
        builder.constraints.keys.compare_i64_zero,
        &[predicate],
        super::structural::provenance(row),
    )?;
    builder.emit(
        SelectedInstructionKind::ConditionalBranchNonZero,
        builder.constraints.keys.conditional_branch,
        &[],
        super::structural::provenance(row),
    )?;
    let instruction = builder.instructions.last().cloned().ok_or_else(failure)?;
    finish(
        builder,
        SelectedTerminator::ConditionalBranch {
            instruction,
            when_nonzero: successor(
                source,
                row.operation,
                if on_nonzero { selected } else { continuation },
            ),
            when_zero: successor(
                source,
                row.operation,
                if on_nonzero { continuation } else { selected },
            ),
        },
    )?;
    let origin = SelectedBlockOrigin::Operation {
        source,
        operation: row.operation,
    };
    builder.open_block = Some(OpenBlock {
        id: selected,
        origin,
        start: builder.instructions.len(),
    });
    body(builder)?;
    // A terminating assertion body closes its block. Only an open body has a
    // normal continuation; adding an unreachable jump would falsify its CFG.
    if builder.open_block.is_some() {
        jump(builder, row, source, continuation)?;
    }
    builder.open_block = Some(OpenBlock {
        id: continuation,
        origin,
        start: builder.instructions.len(),
    });
    Ok(())
}

/// Emit a test-before-body loop. Closures build code once; runtime execution
/// repeats it. Loop-carried physical state lives in operation-owned scratch,
/// never in fabricated source values or SSA definitions with multiple writers.
/// The caller owns a finite bound and must advance its scratch state in `body`.
pub(super) fn while_nonzero(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    condition: impl FnOnce(&mut Builder<'_>) -> Result<VirtualRegisterId, SelectedInstructionError>,
    body: impl FnOnce(&mut Builder<'_>) -> Result<(), SelectedInstructionError>,
) -> Result<(), SelectedInstructionError> {
    let open = builder.open_block.ok_or_else(failure)?;
    let source = match open.origin {
        SelectedBlockOrigin::Source(source) | SelectedBlockOrigin::Operation { source, .. } => {
            source
        }
        _ => return Err(failure()),
    };
    if builder.block_body_end.is_some() {
        return Err(failure());
    }
    settle_preceding_operations(builder)?;
    let test = allocate(builder)?;
    let repeated = allocate(builder)?;
    let continuation = allocate(builder)?;
    let origin = SelectedBlockOrigin::Operation {
        source,
        operation: row.operation,
    };
    jump(builder, row, source, test)?;
    builder.open_block = Some(OpenBlock {
        id: test,
        origin,
        start: builder.instructions.len(),
    });
    let predicate = condition(builder)?;
    builder.emit(
        SelectedInstructionKind::CompareI64Zero,
        builder.constraints.keys.compare_i64_zero,
        &[predicate],
        super::structural::provenance(row),
    )?;
    builder.emit(
        SelectedInstructionKind::ConditionalBranchNonZero,
        builder.constraints.keys.conditional_branch,
        &[],
        super::structural::provenance(row),
    )?;
    let instruction = builder.instructions.last().cloned().ok_or_else(failure)?;
    finish(
        builder,
        SelectedTerminator::ConditionalBranch {
            instruction,
            when_nonzero: successor(source, row.operation, repeated),
            when_zero: successor(source, row.operation, continuation),
        },
    )?;
    builder.open_block = Some(OpenBlock {
        id: repeated,
        origin,
        start: builder.instructions.len(),
    });
    body(builder)?;
    jump(builder, row, source, test)?;
    builder.open_block = Some(OpenBlock {
        id: continuation,
        origin,
        start: builder.instructions.len(),
    });
    Ok(())
}
