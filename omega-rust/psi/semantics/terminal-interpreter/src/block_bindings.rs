//! Snapshot selected successor operands before cleanup or parameter replacement.

use std::collections::BTreeMap;

use semantic_vocabulary::{BlockId, PlaceId, ValueId};
use terminal_psi::{ByteSequenceCarrier, StructuralArgument, StructuralTypeShape};

use super::byte_sequence_view::ByteSequenceView;
use super::{
    TerminalExecution, TerminalInterpretError, TerminalScalarValue, TerminalStructuralValue,
    bind_arguments, bind_structural_arguments,
};

#[cfg(test)]
mod tests;

pub(super) struct BlockBindings {
    scalars: BTreeMap<ValueId, TerminalScalarValue>,
    structural: BTreeMap<PlaceId, TerminalStructuralValue>,
    byte_sequences: BTreeMap<PlaceId, ByteSequenceView>,
}

impl BlockBindings {
    /// All operands have been captured, including descriptors whose source
    /// places are also destinations. Updating one destination cannot affect
    /// another binding, and borrowed descriptors carry no ownership transfer.
    pub(super) fn commit(self, execution: &mut TerminalExecution) {
        execution.values.extend(self.scalars);
        execution.structural_values.extend(self.structural);
        execution.byte_sequence_values.extend(self.byte_sequences);
    }
}

impl TerminalExecution {
    pub(super) fn prepare_block_bindings(
        &self,
        target: BlockId,
        arguments: &[ValueId],
        structural_arguments: &[StructuralArgument],
    ) -> Result<BlockBindings, TerminalInterpretError> {
        let block = self
            .blocks
            .get(&target)
            .ok_or(TerminalInterpretError::VerifiedBlockMissing)?;
        let scalar_arguments = arguments
            .iter()
            .map(|argument| {
                self.values
                    .get(argument)
                    .copied()
                    .ok_or(TerminalInterpretError::VerifiedValueMissing(*argument))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let scalars = bind_arguments(&block.parameters, &scalar_arguments)?;
        if block.structural_parameters.len() != structural_arguments.len() {
            return Err(TerminalInterpretError::StructuralArgumentCount {
                expected: block.structural_parameters.len(),
                actual: structural_arguments.len(),
            });
        }
        let mut resolved_arguments = Vec::with_capacity(structural_arguments.len());
        for (position, (parameter, argument)) in block
            .structural_parameters
            .iter()
            .zip(structural_arguments)
            .enumerate()
        {
            if usize::try_from(parameter.position).ok() != Some(position)
                || parameter.is_self
                || !matches!(
                    self.structural_types.get(&parameter.structural_type),
                    Some(declaration)
                        if declaration.shape
                            == StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView)
                )
                || self.live_claims.values().any(|claim| {
                    claim.place == Some(argument.place) || claim.place == Some(parameter.place)
                })
                || self
                    .live_affine_frontier
                    .iter()
                    .any(|entry| entry.place == argument.place || entry.place == parameter.place)
            {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
            resolved_arguments.push(self.structural_values.get(&argument.place).cloned().ok_or(
                TerminalInterpretError::VerifiedStructuralPlaceMissing(argument.place),
            )?);
        }
        let structural =
            bind_structural_arguments(&block.structural_parameters, &resolved_arguments)?;
        // This checks the exact unrestricted SharedBorrow signature, whole
        // arguments, and empty qualifications, then clones only Arc descriptors.
        let byte_sequences = self.bind_byte_sequence_arguments(
            &block.structural_parameters,
            structural_arguments,
            &resolved_arguments,
        )?;
        Ok(BlockBindings {
            scalars,
            structural,
            byte_sequences,
        })
    }
}
