//! Snapshot selected successor operands before cleanup or parameter replacement.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{BlockId, PlaceId, StructuralPlaceKind, StructuralTypeId, ValueId};
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralAffineDiscard, StructuralArgument,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralTypeShape,
};

use super::byte_sequence_binding::ByteSequenceBinding;
use super::{
    TerminalExecution, TerminalInterpretError, TerminalScalarCaseValue, TerminalScalarValue,
    TerminalStructuralValue,
};
use crate::terminal_interpreter::custody::bind_affine_frontier;
use crate::terminal_interpreter::custody::bind_arguments;
use crate::terminal_interpreter::custody::bind_structural_arguments;
use crate::terminal_interpreter::custody::remove_affine_root;
use crate::terminal_interpreter::custody::resolve_structural_arguments;

#[cfg(test)]
mod tests;

pub(super) struct BlockBindings {
    scalars: BTreeMap<ValueId, TerminalScalarValue>,
    structural: BTreeMap<PlaceId, TerminalStructuralValue>,
    scalar_cases: BTreeMap<PlaceId, TerminalScalarCaseValue>,
    byte_sequences: BTreeMap<PlaceId, ByteSequenceBinding>,
    affine_sources: BTreeSet<PlaceId>,
    /// Roots whose projected children moved on this edge. Their whole-root
    /// frontier entries are split by the caller, not removed as transfers.
    projected_sources: BTreeSet<PlaceId>,
    affine_destinations: BTreeSet<StructuralAffineDiscard>,
    record_payload: Vec<(
        crate::terminal_interpreter::values::StructuralScalarRuntimeField,
        TerminalScalarValue,
    )>,
    identity_cursor: Option<u64>,
}

impl BlockBindings {
    /// All operands have been captured, including descriptors whose source
    /// places are also destinations. Updating one destination cannot affect
    /// another binding. Affine and borrowed descriptors retain their backing;
    /// owned unrestricted records publish independent staged payloads.
    pub(super) fn commit(self, execution: &mut TerminalExecution) {
        execution
            .local_structural_identities
            .commit_cursor(self.identity_cursor);
        execution
            .structural_scalar_fields
            .extend(self.record_payload);
        for source in self.affine_sources {
            execution.structural_values.remove(&source);
            execution.scalar_case_values.remove(&source);
            remove_affine_root(&mut execution.live_affine_frontier, source);
        }
        for destination in self.structural.keys() {
            execution.scalar_case_values.remove(destination);
        }
        for destination in self.scalar_cases.keys() {
            execution.structural_values.remove(destination);
        }
        execution.values.extend(self.scalars);
        execution.structural_values.extend(self.structural);
        execution.scalar_case_values.extend(self.scalar_cases);
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
            let structural_type = execution
                .structural_values
                .get(place)
                .map(|value| value.structural_type)
                .or_else(|| {
                    execution
                        .scalar_case_values
                        .get(place)
                        .map(|value| value.structural_type)
                })
                .ok_or(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                    *place,
                ))?;
            if !discarded.insert(*place)
                || self.affine_sources.contains(place)
                || self.projected_sources.contains(place)
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
                        structural_type,
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
        if self
            .structural
            .keys()
            .chain(self.scalar_cases.keys())
            .any(|place| {
                execution
                    .live_affine_frontier
                    .iter()
                    .any(|entry| entry.place == *place)
                    && !self.affine_sources.contains(place)
                    && !self.projected_sources.contains(place)
                    && !discarded.contains(place)
            })
        {
            return Err(TerminalInterpretError::AffineFrontierMismatch);
        }
        Ok(())
    }
}

impl TerminalExecution {
    /// `allow_projected` selects the Jump-edge lane: an owned argument may
    /// carry a projection path that moves one affine child out of a live
    /// root. Other successor kinds cannot express the residual complement,
    /// so they must keep rejecting path-bearing owned arguments.
    pub(super) fn prepare_block_bindings(
        &self,
        target: BlockId,
        arguments: &[ValueId],
        structural_arguments: &[StructuralArgument],
        allow_projected: bool,
    ) -> Result<BlockBindings, TerminalInterpretError> {
        let block = self
            .machines
            .get(&self.current_machine)
            .ok_or(TerminalInterpretError::VerifiedCallTargetMissing(
                self.current_machine,
            ))?
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
        let contains_cases = structural_arguments
            .iter()
            .any(|argument| self.scalar_case_values.contains_key(&argument.place));
        let mut descriptor_parameters = Vec::new();
        let mut descriptor_arguments = Vec::new();
        let mut scalar_cases = BTreeMap::new();
        let mut case_destinations = BTreeSet::new();
        let mut affine_sources = BTreeSet::new();
        let mut projected_sources = BTreeSet::new();
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
                || (!argument.path.is_empty()
                    && !(parameter.access == StructuralAccess::SharedBorrow
                        || (allow_projected && parameter.access == StructuralAccess::Owned)))
                || self.live_claims.values().any(|claim| {
                    claim.place == Some(argument.place) || claim.place == Some(parameter.place)
                })
            {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
            let value = self.structural_values.get(&argument.place);
            let case = self.scalar_case_values.get(&argument.place);
            match (value, case) {
                (Some(value), None)
                    if value.qualifications.is_empty()
                        && (matches!(
                            parameter.access,
                            StructuralAccess::MutableBorrow | StructuralAccess::SharedBorrow
                        ) || value.path.is_empty()) => {}
                (None, Some(case))
                    if parameter.access == StructuralAccess::Owned
                        && argument.path.is_empty()
                        && case.structural_type == parameter.structural_type => {}
                (None, None) => {
                    return Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                        argument.place,
                    ));
                }
                _ => return Err(TerminalInterpretError::VerifiedOperationMalformed),
            }
            // A projected owned argument binds the selected subtree, keeping
            // the root's opaque identity and the exact moved path. The caller
            // discharges the residual complement on the same edge.
            let mut projected_value = None;
            match parameter.access {
                StructuralAccess::Owned => {
                    if !matches!(
                        parameter.multiplicity,
                        StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
                    ) || self.owned_block_source(parameter.place) != Some(parameter)
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
                    if argument.path.is_empty() {
                        let source_matches =
                            self.owned_block_source(argument.place)
                                .is_some_and(|source| {
                                    source.structural_type == parameter.structural_type
                                        && source.multiplicity == parameter.multiplicity
                                        && source.access == StructuralAccess::Owned
                                        && source.qualifications.is_empty()
                                        && source.projected_qualifications.is_empty()
                                })
                                || self.owned_result_matches(
                                    argument.place,
                                    parameter,
                                    case.is_some(),
                                );
                        if !source_matches {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let root = StructuralAffineDiscard {
                            place: argument.place,
                            path: Vec::new(),
                            structural_type: parameter.structural_type,
                        };
                        let expected = (parameter.multiplicity == StructuralMultiplicity::Affine)
                            .then_some(&root);
                        if self
                            .live_affine_frontier
                            .iter()
                            .filter(|entry| entry.place == argument.place)
                            .ne(expected)
                            || projected_sources.contains(&argument.place)
                        {
                            return Err(TerminalInterpretError::AffineFrontierMismatch);
                        }
                        if parameter.multiplicity == StructuralMultiplicity::Affine
                            && !affine_sources.insert(argument.place)
                        {
                            return Err(TerminalInterpretError::AffineFrontierMismatch);
                        }
                    } else {
                        let root = value.expect("one structural carrier was validated");
                        let view = resolve_structural_arguments(
                            &self.structural_types,
                            &self.structural_values,
                            std::slice::from_ref(argument),
                        )?
                        .pop()
                        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        if view.structural_type != parameter.structural_type {
                            return Err(TerminalInterpretError::StructuralArgumentType {
                                place: parameter.place,
                                expected: parameter.structural_type,
                                actual: view.structural_type,
                            });
                        }
                        let source_matches =
                            self.owned_block_source(argument.place)
                                .is_some_and(|source| {
                                    source.structural_type == root.structural_type
                                        && source.multiplicity == StructuralMultiplicity::Affine
                                        && source.access == StructuralAccess::Owned
                                        && source.qualifications.is_empty()
                                        && source.projected_qualifications.is_empty()
                                })
                                || self.owned_result_source(
                                    argument.place,
                                    root.structural_type,
                                    StructuralMultiplicity::Affine,
                                    false,
                                );
                        if !source_matches
                            || parameter.multiplicity != StructuralMultiplicity::Affine
                        {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let Some(containing) = self.live_affine_frontier.iter().find(|entry| {
                            entry.place == argument.place
                                && argument.path.starts_with(entry.path.as_slice())
                        }) else {
                            return Err(TerminalInterpretError::AffineFrontierMismatch);
                        };
                        if affine_sources.contains(&argument.place)
                            || (containing.path.is_empty()
                                && containing.structural_type != root.structural_type)
                        {
                            return Err(TerminalInterpretError::AffineFrontierMismatch);
                        }
                        projected_sources.insert(argument.place);
                        projected_value = Some(view);
                    }
                }
                StructuralAccess::SharedBorrow => {
                    if parameter.multiplicity != StructuralMultiplicity::Unrestricted
                        || self
                            .live_affine_frontier
                            .iter()
                            .any(|entry| entry.place == parameter.place)
                    {
                        return Err(TerminalInterpretError::VerifiedOperationMalformed);
                    }
                    if matches!(self.structural_types.get(&parameter.structural_type), Some(declaration)
                        if declaration.shape == StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView))
                    {
                        // Borrowed byte views join whole: the argument names
                        // an untracked view root with no projection.
                        if !argument.path.is_empty()
                            || self
                                .live_affine_frontier
                                .iter()
                                .any(|entry| entry.place == argument.place)
                        {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                    } else if self.plain_record_type(parameter.structural_type)
                        || matches!(
                            self.structural_types
                                .get(&parameter.structural_type)
                                .map(|declaration| &declaration.shape),
                            Some(StructuralTypeShape::PrimitiveScalar(_))
                        )
                    {
                        // A shared loan binds the exact projected view: the
                        // argument path extends the root's stored path and
                        // must resolve to the parameter's declared referent
                        // type — a plain record subtree or one canonical
                        // primitive-scalar leaf. The loan moves no custody —
                        // the root either stays untracked or is still held as
                        // one whole frontier entry.
                        let view = resolve_structural_arguments(
                            &self.structural_types,
                            &self.structural_values,
                            std::slice::from_ref(argument),
                        )?
                        .pop()
                        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        if view.structural_type != parameter.structural_type {
                            return Err(TerminalInterpretError::StructuralArgumentType {
                                place: parameter.place,
                                expected: parameter.structural_type,
                                actual: view.structural_type,
                            });
                        }
                        if self
                            .live_affine_frontier
                            .iter()
                            .any(|entry| entry.place == argument.place && !entry.path.is_empty())
                        {
                            return Err(TerminalInterpretError::AffineFrontierMismatch);
                        }
                        projected_value = Some(view);
                    } else {
                        return Err(TerminalInterpretError::VerifiedOperationMalformed);
                    }
                }
                StructuralAccess::MutableBorrow => {
                    if parameter.multiplicity != StructuralMultiplicity::Unrestricted
                        || !matches!(self.structural_types.get(&parameter.structural_type), Some(declaration)
                            if declaration.shape == StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView))
                        || self.live_affine_frontier.iter().any(|entry| {
                            entry.place == argument.place || entry.place == parameter.place
                        })
                    {
                        return Err(TerminalInterpretError::VerifiedOperationMalformed);
                    }
                    self.mutable_byte_sequence_storage(argument.place)?;
                }
                _ => return Err(TerminalInterpretError::VerifiedOperationMalformed),
            }
            if let Some(case) = case {
                if scalar_cases.insert(parameter.place, case.clone()).is_some() {
                    return Err(TerminalInterpretError::VerifiedOperationMalformed);
                }
                if parameter.multiplicity == StructuralMultiplicity::Affine {
                    case_destinations.insert(StructuralAffineDiscard {
                        place: parameter.place,
                        path: Vec::new(),
                        structural_type: parameter.structural_type,
                    });
                }
            } else {
                if contains_cases {
                    descriptor_parameters.push(parameter.clone());
                    descriptor_arguments.push(argument.clone());
                }
                resolved_arguments.push(projected_value.unwrap_or_else(|| {
                    value.expect("one structural carrier was validated").clone()
                }));
            }
        }
        // Existing descriptor-only edges reuse their original declaration and
        // argument slices. Only mixed representations need a selected roster.
        let descriptor_parameters = if contains_cases {
            descriptor_parameters.as_slice()
        } else {
            &block.structural_parameters
        };
        let descriptor_arguments = if contains_cases {
            descriptor_arguments.as_slice()
        } else {
            structural_arguments
        };
        let mut structural = bind_structural_arguments(descriptor_parameters, &resolved_arguments)?;
        let mut affine_destinations = bind_affine_frontier(descriptor_parameters, &structural)?;
        affine_destinations.extend(case_destinations);
        // Existing borrowed-byte bindings retain their exact checks. Owned
        // record fields are keyed by runtime referent identity, not SSA place.
        let byte_sequences = self.bind_byte_sequence_arguments(
            descriptor_parameters,
            descriptor_arguments,
            &resolved_arguments,
        )?;
        let mut identity_cursor = self.local_structural_identities.cursor();
        let record_payload = self.stage_owned_record_arguments(
            descriptor_parameters,
            &mut structural,
            &mut identity_cursor,
        )?;
        Ok(BlockBindings {
            scalars,
            structural,
            scalar_cases,
            byte_sequences,
            affine_sources,
            projected_sources,
            affine_destinations,
            record_payload,
            identity_cursor,
        })
    }

    pub(super) fn owned_result_matches(
        &self,
        place: PlaceId,
        parameter: &StructuralParameterDeclaration,
        scalar_case: bool,
    ) -> bool {
        self.owned_result_source(
            place,
            parameter.structural_type,
            parameter.multiplicity,
            scalar_case,
        )
    }

    /// A projected argument's source is checked against the root's declared
    /// type and affine ownership, not the parameter's leaf type.
    fn owned_result_source(
        &self,
        place: PlaceId,
        structural_type: StructuralTypeId,
        multiplicity: StructuralMultiplicity,
        scalar_case: bool,
    ) -> bool {
        let Some(machine) = self.machines.get(&self.current_machine) else {
            return false;
        };
        let Some(StructuralPlaceKind::OperationResult {
            producer,
            structural_type: declared,
        }) = machine
            .structural_places
            .iter()
            .find(|declaration| declaration.id == place)
            .map(|declaration| declaration.kind)
        else {
            return false;
        };
        // The committed record descriptor retains its runtime referent. This
        // recognizes its exact producer, not a fresh payload or a copied loan.
        // Case payloads use their existing distinct runtime representation.
        declared == structural_type
            && (scalar_case || self.plain_record_type(structural_type))
            && machine
                .blocks
                .values()
                .flat_map(|block| &block.operations)
                .any(|operation| {
                    operation.id == producer
                        && match operation.kind {
                            terminal_psi::OperationKind::EstablishScalarCase { .. } => scalar_case,
                            terminal_psi::OperationKind::EstablishRecord { .. } => !scalar_case,
                            terminal_psi::OperationKind::CallStructural { .. }
                            | terminal_psi::OperationKind::CallStructuralWithScalarArguments {
                                ..
                            } => true,
                            _ => false,
                        }
                        && operation.result.structural().is_some_and(|result| {
                            result.place == place
                                && result.structural_type == structural_type
                                && result.multiplicity == multiplicity
                                && result.qualifications.is_empty()
                                && result.projected_qualifications.is_empty()
                                && result.claims.is_empty()
                        })
                })
    }

    pub(super) fn owned_block_source(
        &self,
        place: PlaceId,
    ) -> Option<&StructuralParameterDeclaration> {
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
            StructuralPlaceKind::BlockParameter { block, position } => self.machines
                [&self.current_machine]
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
