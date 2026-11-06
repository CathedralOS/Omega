//! Close the installation's instruction roster before accepting its effects.
//!
//! Matching the required stores and guards does not account for an additional
//! port write, atomic operation, call, or CPU-state change. Only the operations
//! whose operands and ordering are checked by this audit may carry Scope's
//! source identity. Ordinary instruction and byte validation remains separate.

use super::Error;
use selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind as Kind,
    SelectedTerminator,
};
use semantic_vocabulary::OperationId;

pub(super) fn validate(
    selected: &SelectedFunction,
    expressions: &super::expressions::Expressions<'_>,
    operation: OperationId,
    validation: SelectedInstructionId,
    work: SelectedInstructionId,
) -> Result<(), Error> {
    if validation == work {
        return Err(Error::Mismatch(
            "installation validation and work share one instruction",
        ));
    }
    let mut counts = Counts::default();
    for block in &selected.blocks {
        for row in &block.instructions {
            if row.provenance.operations.contains(&operation) && !expressions.private_store(row) {
                body(row, validation, &mut counts)?;
            }
        }
        let row = selected_instructions::terminator_instruction(&block.terminator);
        if !row.provenance.operations.contains(&operation) {
            continue;
        }
        match (&block.terminator, &row.kind) {
            (SelectedTerminator::Jump { .. }, Kind::Jump)
            | (
                SelectedTerminator::ConditionalBranchU64LessThan { .. },
                Kind::ConditionalBranchU64LessThan,
            )
            | (SelectedTerminator::ConditionalBranch { .. }, Kind::ConditionalBranchNonZero) => {}
            (
                SelectedTerminator::OperationInvokeNoNormal {
                    operation: owner, ..
                },
                Kind::InvokeNoNormal { .. },
            ) if *owner == operation && row.id == work => counts.work += 1,
            _ => {
                return Err(Error::Mismatch(
                    "installation contains an unaccounted control transfer",
                ));
            }
        }
    }
    if counts.validation != 1
        || counts.work != 1
        || counts.cpu_check != 1
        || counts.task_commit != 1
        || counts.table_publication != 1
    {
        return Err(Error::Mismatch(
            "installation changed its exact helper or CPU-operation roster",
        ));
    }
    Ok(())
}

#[derive(Default)]
struct Counts {
    validation: usize,
    work: usize,
    cpu_check: usize,
    task_commit: usize,
    table_publication: usize,
}

fn body(
    row: &SelectedInstruction,
    validation: SelectedInstructionId,
    counts: &mut Counts,
) -> Result<(), Error> {
    match row.kind {
        // These forms compute words and addresses. Their relevant results are
        // independently reconstructed; they cannot introduce a storage write
        // or an additional invocation.
        Kind::MaterializeI64 { .. }
        | Kind::CopyI64
        | Kind::FrameAddress { .. }
        | Kind::ImageAddress { .. }
        | Kind::AddressOffset { .. }
        | Kind::ByteViewAddress
        | Kind::WrappingAddI64
        | Kind::WrappingSubtractI64
        | Kind::WrappingMultiplyI64
        | Kind::BitwiseAndI64
        | Kind::BitwiseOrI64
        | Kind::BitwiseXorI64
        | Kind::WrappingShiftLeftI64
        | Kind::WrappingShiftRightU64
        | Kind::CompareI64
        | Kind::CompareI64Zero
        | Kind::MaterializeBooleanEqual
        | Kind::MaterializeBooleanU64LessThan
        | Kind::MaterializeBooleanU64LessOrEqual => {}
        // Every load and store is checked by the backing and write auditors.
        Kind::Load8 { .. }
        | Kind::Load16 { .. }
        | Kind::Load32 { .. }
        | Kind::Load64 { .. }
        | Kind::Store { .. } => {}
        Kind::CallScalar { .. } if row.id == validation => counts.validation += 1,
        Kind::InstalledInterruptCpuCheck { .. } => counts.cpu_check += 1,
        Kind::InstalledInterruptTaskCommit { .. } => counts.task_commit += 1,
        Kind::InstalledInterruptTablePublication { .. } => counts.table_publication += 1,
        _ => {
            return Err(Error::Mismatch(
                "installation contains an unaccounted instruction or side effect",
            ));
        }
    }
    Ok(())
}
