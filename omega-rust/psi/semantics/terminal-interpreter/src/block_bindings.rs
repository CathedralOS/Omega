//! Snapshot selected successor operands before cleanup or parameter replacement.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{BlockId, PlaceId, StructuralPlaceKind, ValueId};
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralAffineDiscard, StructuralArgument,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralTypeShape,
};

use super::byte_sequence_binding::ByteSequenceBinding;
use super::{
    TerminalExecution, TerminalInterpretError, TerminalScalarValue, TerminalStructuralValue,
    bind_affine_frontier, bind_arguments, bind_structural_arguments, remove_affine_root,
};

#[cfg(test)]
mod tests;

pub(super) struct BlockBindings {
    scalars: BTreeMap<ValueId, TerminalScalarValue>,
    structural: BTreeMap<PlaceId, TerminalStructuralValue>,
    byte_sequences: BTreeMap<PlaceId, ByteSequenceBinding>,
    affine_sources: BTreeSet<PlaceId>,
    affine_destinations: BTreeSet<StructuralAffineDiscard>,
}

impl BlockBindings {
    /// All operands have been captured, including descriptors whose source
    /// places are also destinations. Updating one destination cannot affect
    /// another binding. Snapshotting descriptor metadata does not duplicate
    /// affine custody or copy field/primitive backing storage.
    pub(super) fn commit(self, execution: &mut TerminalExecution) {
        for source in self.affine_sources {
            execution.structural_values.remove(&source);
            remove_affine_root(&mut execution.live_affine_frontier, source);
        }
        execution.values.extend(self.scalars);
        execution.structural_values.extend(self.structural);
        execution.byte_sequence_values.extend(self.byte_sequences);
        execution
            .live_affine_frontier
            .extend(self.affine_destinations);
    }

    pub(super) fn validate_discards(
        &self,
        execution: &TerminalExecution,
        trivial: &[PlaceId],
        residual: &[StructuralAffineDiscard],
    ) -> Result<(), TerminalInterpretError> {
        let mut discarded = BTreeSet::new();
        for place in trivial {
            let value = execution.structural_values.get(place).ok_or(
                TerminalInterpretError::VerifiedStructuralPlaceMissing(*place),
            )?;
            if !discarded.insert(*place)
                || self.affine_sources.contains(place)
                || execution
                    .live_claims
                    .values()
                    .any(|claim| claim.place == Some(*place))
                || execution
                    .live_affine_frontier
                    .iter()
                    .filter(|entry| entry.place == *place)
                    .ne(std::iter::once(&StructuralAffineDiscard {
                        place: *place,
                        path: Vec::new(),
                        structural_type: value.structural_type,
                    }))
            {
                return Err(TerminalInterpretError::AffineFrontierMismatch);
            }
        }
        if residual
            .iter()
            .any(|discard| self.affine_sources.contains(&discard.place))
        {
            return Err(TerminalInterpretError::AffineFrontierMismatch);
        }
        // A swap may replace another transferred root. Replacing a different
        // live destination requires its explicit whole-root discard first.
        if self.structural.keys().any(|place| {
            execution
                .live_affine_frontier
                .iter()
                .any(|entry| entry.place == *place)
                && !self.affine_sources.contains(place)
                && !discarded.contains(place)
        }) {
            return Err(TerminalInterpretError::AffineFrontierMismatch);
        }
        Ok(())
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
        let mut affine_sources = BTreeSet::new();
        for (position, (parameter, argument)) in block
            .structural_parameters
            .iter()
            .zip(structural_arguments)
            .enumerate()
        {
            if usize::try_from(parameter.position).ok() != Some(position)
                || parameter.is_self
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
                || argument.access != parameter.access
                || !argument.path.is_empty()
                || self.live_claims.values().any(|claim| {
                    claim.place == Some(argument.place) || claim.place == Some(parameter.place)
                })
            {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
            let value = self.structural_values.get(&argument.place).ok_or(
                TerminalInterpretError::VerifiedStructuralPlaceMissing(argument.place),
            )?;
            if !value.qualifications.is_empty() || !value.path.is_empty() {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
            match parameter.access {
                StructuralAccess::Owned => {
                    let source = self
                        .owned_block_source(argument.place)
                        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                    if source.structural_type != parameter.structural_type
                        || source.multiplicity != parameter.multiplicity
                        || source.access != StructuralAccess::Owned
                        || !source.qualifications.is_empty()
                        || !source.projected_qualifications.is_empty()
                        || !matches!(
                            parameter.multiplicity,
                            StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
                        )
                        || self.owned_block_source(parameter.place) != Some(parameter)
                        || !self
                            .machines
                            .get(&self.current_machine)
                            .is_some_and(|machine| {
                                machine.structural_places.iter().any(|declaration| {
                                    declaration.id == parameter.place
                                        && declaration.kind
                                            == StructuralPlaceKind::BlockParameter {
                                                block: target,
                                                position: parameter.position,
                                            }
                                })
                            })
                    {
                        return Err(TerminalInterpretError::VerifiedOperationMalformed);
                    }
                    let root = StructuralAffineDiscard {
                        place: argument.place,
                        path: Vec::new(),
                        structural_type: parameter.structural_type,
                    };
                    let expected =
                        (parameter.multiplicity == StructuralMultiplicity::Affine).then_some(&root);
                    if self
                        .live_affine_frontier
                        .iter()
                        .filter(|entry| entry.place == argument.place)
                        .ne(expected)
                    {
                        return Err(TerminalInterpretError::AffineFrontierMismatch);
                    }
                    if parameter.multiplicity == StructuralMultiplicity::Affine
                        && !affine_sources.insert(argument.place)
                    {
                        return Err(TerminalInterpretError::AffineFrontierMismatch);
                    }
                }
                StructuralAccess::SharedBorrow => {
                    if parameter.multiplicity != StructuralMultiplicity::Unrestricted
                        || !matches!(self.structural_types.get(&parameter.structural_type), Some(declaration)
                            if declaration.shape == StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView))
                        || self.live_affine_frontier.iter().any(|entry| {
                            entry.place == argument.place || entry.place == parameter.place
                        })
                    {
                        return Err(TerminalInterpretError::VerifiedOperationMalformed);
                    }
                }
                _ => return Err(TerminalInterpretError::VerifiedOperationMalformed),
            }
            resolved_arguments.push(value.clone());
        }
        let structural =
            bind_structural_arguments(&block.structural_parameters, &resolved_arguments)?;
        let affine_destinations = bind_affine_frontier(&block.structural_parameters, &structural)?;
        // Existing borrowed-byte bindings retain their exact checks. Owned
        // record fields are keyed by runtime referent identity, not SSA place.
        let byte_sequences = self.bind_byte_sequence_arguments(
            &block.structural_parameters,
            structural_arguments,
            &resolved_arguments,
        )?;
        Ok(BlockBindings {
            scalars,
            structural,
            byte_sequences,
            affine_sources,
            affine_destinations,
        })
    }

    fn owned_block_source(&self, place: PlaceId) -> Option<&StructuralParameterDeclaration> {
        let machine = self.machines.get(&self.current_machine)?;
        let declaration = machine
            .structural_places
            .iter()
            .find(|declaration| declaration.id == place)?;
        match declaration.kind {
            StructuralPlaceKind::Parameter { position, is_self } => {
                machine.structural_parameters.iter().find(|parameter| {
                    parameter.place == place
                        && parameter.position == position
                        && parameter.is_self == is_self
                })
            }
            StructuralPlaceKind::BlockParameter { block, position } => self
                .blocks
                .get(&block)?
                .structural_parameters
                .get(position as usize)
                .filter(|parameter| {
                    parameter.place == place && parameter.position == position && !parameter.is_self
                }),
            _ => None,
        }
    }
}
