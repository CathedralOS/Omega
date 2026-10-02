//! Snapshot selected successor operands before cleanup or parameter replacement.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{BlockId, PlaceId, StructuralPlaceKind, StructuralTypeId, ValueId};
use terminal_psi::{
    StructuralAccess, StructuralAffineDiscard, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralTypeShape,
};

use crate::byte_sequences::binding::ByteSequenceBinding;
use crate::custody::bind_affine_frontier;
use crate::custody::bind_arguments;
use crate::custody::bind_structural_arguments;
use crate::custody::remove_affine_root;
use crate::custody::resolve_structural_arguments;
use crate::errors::TerminalInterpretError;
use crate::execution::TerminalExecution;
use crate::scalar_array::TerminalScalarArrayValue;
use crate::values::{TerminalScalarCaseValue, TerminalScalarValue, TerminalStructuralValue};

#[cfg(test)]
mod tests;

pub(crate) struct BlockBindings {
    scalars: BTreeMap<ValueId, TerminalScalarValue>,
    structural: BTreeMap<PlaceId, TerminalStructuralValue>,
    scalar_cases: BTreeMap<PlaceId, TerminalScalarCaseValue>,
    scalar_arrays: BTreeMap<PlaceId, TerminalScalarArrayValue>,
    byte_sequences: BTreeMap<PlaceId, ByteSequenceBinding>,
    element_views: BTreeMap<PlaceId, crate::element_views::ElementView>,
    affine_sources: BTreeSet<PlaceId>,
    /// Roots whose projected children moved on this edge. Their whole-root
    /// frontier entries are split by the caller, not removed as transfers.
    projected_sources: BTreeSet<PlaceId>,
    affine_destinations: BTreeSet<StructuralAffineDiscard>,
    record_payload: Vec<(
        crate::values::StructuralScalarRuntimeField,
        TerminalScalarValue,
    )>,
    identity_cursor: Option<u64>,
}

impl BlockBindings {
    /// All operands have been captured, including descriptors whose source
    /// places are also destinations. Updating one destination cannot affect
    /// another binding. Affine and borrowed descriptors retain their backing;
    /// owned unrestricted records publish independent staged payloads.
    pub(crate) fn commit(self, execution: &mut TerminalExecution) {
        execution
            .local_structural_identities
            .commit_cursor(self.identity_cursor);
        execution
            .structural_scalar_fields
            .extend(self.record_payload);
        for source in self.affine_sources {
            execution.structural_values.remove(&source);
            execution.scalar_case_values.remove(&source);
            execution.scalar_array_values.remove(&source);
            execution.element_view_values.remove(&source);
            remove_affine_root(&mut execution.live_affine_frontier, source);
        }
        for destination in self.structural.keys() {
            execution.scalar_case_values.remove(destination);
            execution.scalar_array_values.remove(destination);
        }
        for destination in self.scalar_cases.keys() {
            execution.structural_values.remove(destination);
            execution.scalar_array_values.remove(destination);
            execution.element_view_values.remove(destination);
        }
        for destination in self.scalar_arrays.keys() {
            execution.structural_values.remove(destination);
            execution.scalar_case_values.remove(destination);
            execution.element_view_values.remove(destination);
        }
        execution.values.extend(self.scalars);
        execution.structural_values.extend(self.structural);
        execution.scalar_case_values.extend(self.scalar_cases);
        execution.scalar_array_values.extend(self.scalar_arrays);
        execution.byte_sequence_values.extend(self.byte_sequences);
        execution.element_view_values.extend(self.element_views);
        execution
            .live_affine_frontier
            .extend(self.affine_destinations);
    }

    pub(crate) fn validate_discards(
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
            .chain(self.scalar_arrays.keys())
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
    pub(crate) fn prepare_block_bindings(
        &self,
        target: BlockId,
        arguments: &[ValueId],
        structural_arguments: &[StructuralArgument],
        allow_projected: bool,
    ) -> Result<BlockBindings, TerminalInterpretError> {
        let machine = self.program.machines.get(&self.current_machine).ok_or(
            TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
        )?;
        let block = machine
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
        let contains_payloads = structural_arguments.iter().any(|argument| {
            self.scalar_case_values.contains_key(&argument.place)
                || self.scalar_array_values.contains_key(&argument.place)
        });
        let mut descriptor_parameters = Vec::new();
        let mut descriptor_arguments = Vec::new();
        let mut scalar_cases = BTreeMap::new();
        let mut scalar_arrays = BTreeMap::new();
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
                    && !(matches!(
                        parameter.access,
                        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                    ) || (allow_projected && parameter.access == StructuralAccess::Owned)))
                || self.live_claims.values().any(|claim| {
                    claim.place == Some(argument.place) || claim.place == Some(parameter.place)
                })
            {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
            if self.scalar_array_values.contains_key(&argument.place) {
                // Arrays already have a complete value carrier. Capture it by
                // the same rules as call operands before replacing any slots.
                let value = self.prepare_scalar_array_argument(machine, parameter, argument)?;
                if !self.block_parameter_is_declared(target, parameter)
                    || self.live_affine_frontier.iter().any(|entry| {
                        entry.place == argument.place || entry.place == parameter.place
                    })
                    || scalar_arrays.insert(parameter.place, value).is_some()
                {
                    return Err(TerminalInterpretError::VerifiedOperationMalformed);
                }
                continue;
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
                    ) || !self.block_parameter_is_declared(target, parameter)
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
                            &self.program.structural_types,
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
                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                    if matches!(self.program.structural_types.get(&parameter.structural_type),
                        Some(declaration) if matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
                            || declaration.shape.is_borrowed_byte_view()) =>
                {
                    // A descriptor lends its window, not its affine storage.
                    // The common argument binder below preserves exact backing
                    // and attenuates access. Its source debt remains here until
                    // the independently verified cleanup frontier discharges it.
                    if parameter.multiplicity != StructuralMultiplicity::Unrestricted
                        || !argument.path.is_empty()
                        || self.live_affine_frontier.iter().any(|entry| {
                            entry.place == parameter.place
                                || (entry.place == argument.place
                                    && (!entry.path.is_empty()
                                        || entry.structural_type != parameter.structural_type))
                        })
                    {
                        return Err(TerminalInterpretError::VerifiedOperationMalformed);
                    }
                }
                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                    if self.plain_record_type(parameter.structural_type)
                        || matches!(
                            self.program
                                .structural_types
                                .get(&parameter.structural_type)
                                .map(|declaration| &declaration.shape),
                            Some(StructuralTypeShape::PrimitiveScalar(_))
                        ) =>
                {
                    if parameter.multiplicity != StructuralMultiplicity::Unrestricted
                        || self
                            .live_affine_frontier
                            .iter()
                            .any(|entry| entry.place == parameter.place)
                        || (parameter.access == StructuralAccess::MutableBorrow
                            && (!self.block_parameter_is_declared(target, parameter)
                                || !self.mutable_place_source(argument.place)))
                    {
                        return Err(TerminalInterpretError::VerifiedOperationMalformed);
                    }
                    // A borrowed place binds the exact projected view: the
                    // argument path extends the root's stored path and
                    // must resolve to the parameter's declared referent
                    // type — a plain record subtree or one canonical
                    // primitive-scalar leaf. The loan moves no custody —
                    // the root either stays untracked or is still held as
                    // one whole frontier entry.
                    let view = resolve_structural_arguments(
                        &self.program.structural_types,
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
                    if matches!(self.program.structural_types.get(&parameter.structural_type), Some(declaration)
                                            if declaration.shape.is_borrowed_byte_carrier())
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
                    } else {
                        return Err(TerminalInterpretError::VerifiedOperationMalformed);
                    }
                }
                StructuralAccess::MutableBorrow => {
                    if parameter.multiplicity != StructuralMultiplicity::Unrestricted
                        || !matches!(self.program.structural_types.get(&parameter.structural_type), Some(declaration)
                                                    if declaration.shape.is_borrowed_byte_carrier())
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
                if contains_payloads {
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
        let descriptor_parameters = if contains_payloads {
            descriptor_parameters.as_slice()
        } else {
            &block.structural_parameters
        };
        let descriptor_arguments = if contains_payloads {
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
        let element_views = self.bind_element_view_arguments(
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
            scalar_arrays,
            byte_sequences,
            element_views,
            affine_sources,
            projected_sources,
            affine_destinations,
            record_payload,
            identity_cursor,
        })
    }

    /// Mutable forwarding preserves the current referent and its source debt.
    /// Runtime identities alone carry no permission: recover it from the exact
    /// current parameter or independently admitted owned result declaration.
    fn mutable_place_source(&self, place: PlaceId) -> bool {
        let Some(value) = self.structural_values.get(&place) else {
            return false;
        };
        let multiplicity = if let Some(source) = self.owned_block_source(place) {
            if source.structural_type != value.structural_type
                || !source.qualifications.is_empty()
                || !source.projected_qualifications.is_empty()
                || !matches!(
                    source.access,
                    StructuralAccess::MutableBorrow | StructuralAccess::Owned
                )
                || (source.access == StructuralAccess::MutableBorrow
                    && source.multiplicity != StructuralMultiplicity::Unrestricted)
            {
                return false;
            }
            source.multiplicity
        } else if self.owned_result_source(
            place,
            value.structural_type,
            StructuralMultiplicity::Affine,
            false,
        ) {
            StructuralMultiplicity::Affine
        } else if self.owned_result_source(
            place,
            value.structural_type,
            StructuralMultiplicity::Unrestricted,
            false,
        ) {
            StructuralMultiplicity::Unrestricted
        } else {
            return false;
        };
        let root = StructuralAffineDiscard {
            place,
            path: Vec::new(),
            structural_type: value.structural_type,
        };
        let expected = match multiplicity {
            StructuralMultiplicity::Unrestricted => None,
            StructuralMultiplicity::Affine => Some(&root),
            _ => return false,
        };
        self.live_affine_frontier
            .iter()
            .filter(|entry| entry.place == place)
            .eq(expected)
    }

    fn block_parameter_is_declared(
        &self,
        target: BlockId,
        parameter: &StructuralParameterDeclaration,
    ) -> bool {
        self.owned_block_source(parameter.place) == Some(parameter)
            && self
                .program
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
    }

    pub(crate) fn owned_result_matches(
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
        let Some(machine) = self.program.machines.get(&self.current_machine) else {
            return false;
        };
        let Some(declaration) = machine
            .structural_places
            .iter()
            .find(|declaration| declaration.id == place)
        else {
            return false;
        };
        // A declared trivial affine local is an exact owned source for its
        // empty-record type: the establishment op carries custody and the
        // declaration carries the type, so no producer row exists.
        if let StructuralPlaceKind::TrivialAffineLocal {
            structural_type: declared,
            ..
        } = declaration.kind
        {
            return declared == structural_type && multiplicity == StructuralMultiplicity::Affine;
        }
        let StructuralPlaceKind::OperationResult {
            producer,
            structural_type: declared,
        } = declaration.kind
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
                            // Leaf copies publish fresh record storage, just
                            // like construction; a whole copy of a scalar
                            // case publishes a fresh scalar case instead. The
                            // exact producer, type and multiplicity checks
                            // below still govern transfer.
                            terminal_psi::OperationKind::StructuralLeafCopy {
                                ref path, ..
                            } => !scalar_case || path.is_empty(),
                            terminal_psi::OperationKind::EstablishRecord { .. }
                            | terminal_psi::OperationKind::StructuralCaseLeafCopy { .. } => {
                                !scalar_case
                            }
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

    pub(crate) fn owned_block_source(
        &self,
        place: PlaceId,
    ) -> Option<&StructuralParameterDeclaration> {
        let machine = self.program.machines.get(&self.current_machine)?;
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
            StructuralPlaceKind::BlockParameter { block, position } => self.program.machines
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
