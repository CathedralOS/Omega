//! A shared slice names existing backing; it does not create ownership or
//! access. Check that origin independently, then prove the two bounds against
//! the descriptor measured immediately before formation.

use super::{Shape, Storage};
use semantic_vocabulary::{
    IdTable, OperationId, PlaceId, Proposition, ScalarType, StructuralPlaceKind, ValueId,
};
use terminal_check_tables::{Checked, Construct, ProgramPoint};
use terminal_psi::{
    Operation, OperationKind, StructuralAccess, StructuralMultiplicity, StructuralPathSegment,
    StructuralTypeShape,
};

fn count_type() -> ScalarType {
    ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
            .expect("u64 is valid"),
    )
}

fn is_shared_bytes(shape: &StructuralTypeShape) -> bool {
    matches!(
        shape,
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::SharedBorrow),
        })
    )
}

impl Storage<'_> {
    fn readable_descriptor(
        &self,
        root: PlaceId,
        path: &[StructuralPathSegment],
        concerns: Construct,
    ) -> Checked<bool> {
        let (multiplicity, authority) =
            if let Some(parameter) = self.lowered.structural_parameter(root) {
                (parameter.multiplicity, parameter.access)
            } else {
                match self.root_kind(root, concerns)? {
                    StructuralPlaceKind::OperationResult { producer, .. } => {
                        let result = self
                            .operation(producer, concerns)?
                            .result
                            .as_structural()
                            .filter(|result| result.place == root)
                            .ok_or_else(|| {
                                self.foreign(concerns, "a descriptor source lost its exact result")
                            })?;
                        (result.multiplicity, StructuralAccess::Owned)
                    }
                    StructuralPlaceKind::ByteSequenceLiteral { .. } => (
                        StructuralMultiplicity::Unrestricted,
                        StructuralAccess::SharedBorrow,
                    ),
                    _ => {
                        return Err(self
                            .unsupported(concerns, "a descriptor source without storage access"));
                    }
                }
            };
        Ok(terminal_semantics::reference_source_projection_accessible(
            self.types.iter(),
            self.root_type(root, concerns)?,
            multiplicity,
            authority,
            path,
            StructuralAccess::SharedBorrow,
        ))
    }

    fn byte_shape(
        &self,
        root: PlaceId,
        path: &[StructuralPathSegment],
        concerns: Construct,
    ) -> Checked<StructuralTypeShape> {
        let root = self.root_type(root, concerns)?;
        terminal_semantics::runtime_structural_path_tip(self.types.iter(), root, path)
            .map(|declaration| declaration.shape.clone())
            .or_else(|| {
                terminal_semantics::runtime_structural_field(self.types.iter(), root, path)
                    .and_then(|field| field.field_type.byte_sequence_carrier())
                    .map(StructuralTypeShape::ByteSequence)
            })
            .ok_or_else(|| {
                self.unsupported(concerns, "a byte view without its exact descriptor type")
            })
    }

    pub(super) fn shared_subslice(&self, operation: &Operation) -> Checked<()> {
        use terminal_semantics::descriptor_results::DescriptorCallOrigin;
        let OperationKind::ByteSequenceSubslice { source, .. } = &operation.kind else {
            unreachable!()
        };
        let concerns = Construct::Operation(operation.id);
        let result = operation
            .result
            .as_structural()
            .ok_or_else(|| self.foreign(concerns, "a byte subslice has no descriptor result"))?;
        if result.multiplicity != StructuralMultiplicity::Unrestricted
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.qualification_establishments.is_empty()
            || !result.additional_input_qualifications.is_empty()
            || !result.claims.is_empty()
            || self.root_kind(result.place, concerns)?
                != (StructuralPlaceKind::OperationResult {
                    producer: operation.id,
                    structural_type: result.structural_type,
                })
        {
            return Err(self.foreign(
                concerns,
                "a shared subslice must produce its exact plain unrestricted descriptor",
            ));
        }
        let destination = &self.declaration(result.structural_type, concerns)?.shape;
        let selected = self.byte_shape(source.place, &source.path, concerns)?;
        if !is_shared_bytes(&selected)
            || !is_shared_bytes(destination)
            || source.access != StructuralAccess::SharedBorrow
            || !self.readable_descriptor(source.place, &source.path, concerns)?
            || !terminal_semantics::descriptor_reborrow_shapes_match(
                &selected,
                source.access,
                Some(StructuralAccess::SharedBorrow),
                destination,
                StructuralAccess::SharedBorrow,
            )
        {
            return Err(self.unsupported(
                concerns,
                "a subslice without shared source and result access",
            ));
        }
        let location = self
            .argument(source, concerns)?
            .descriptor_location()
            .ok_or_else(|| {
                self.unsupported(
                    concerns,
                    "a subslice source without an exact descriptor location",
                )
            })?;
        let (block, after) = self
            .graph
            .locate(ProgramPoint::AfterOperation(operation.id))?;
        let position = u32::try_from(after - 1).map_err(|_| {
            self.foreign(concerns, "a subslice position exceeds its descriptor graph")
        })?;
        let origins = self
            .references
            .borrow_mut()
            .source_origins(self.machine, (block, position), &location)
            .filter(|origins| !origins.is_empty())
            .ok_or_else(|| {
                self.unsupported(concerns, "a subslice without established shared backing")
            })?;
        for origin in origins {
            let shared = match origin {
                DescriptorCallOrigin::Argument { source, projection } => {
                    matches!(
                        self.root_kind(source, concerns)?,
                        StructuralPlaceKind::Parameter { .. }
                    ) && is_shared_bytes(&self.byte_shape(source, &projection.path, concerns)?)
                        && self.readable_descriptor(source, &projection.path, concerns)?
                }
                DescriptorCallOrigin::Literal { machine, place } => self
                    .module
                    .machine(machine)
                    .and_then(|machine| machine.structural_place(place))
                    .is_some_and(|place| {
                        matches!(place.kind, StructuralPlaceKind::ByteSequenceLiteral { .. })
                    }),
                _ => false,
            };
            if !shared {
                return Err(self.unsupported(
                    concerns,
                    "a subslice whose backing is not shared ingress or immutable literal bytes",
                ));
            }
        }
        // Storage::uses separately proves the selected descriptor is available
        // on every arrival; the descriptor graph above identifies its backing.
        if !matches!(
            self.shape(&self.argument(source, concerns)?, concerns)?,
            Shape::Typed(_) | Shape::View
        ) {
            return Err(self.foreign(concerns, "a subslice source is not a descriptor"));
        }
        Ok(())
    }

    pub(super) fn byte_subslice_goal(
        &self,
        operation: &Operation,
        types: &IdTable<ValueId, ScalarType>,
    ) -> Checked<Proposition> {
        let OperationKind::ByteSequenceSubslice {
            source,
            start,
            end,
            length,
            ..
        } = &operation.kind
        else {
            unreachable!()
        };
        let (start, end, length) = (*start, *end, *length);
        let concerns = Construct::Operation(operation.id);
        let (block, after) = self
            .graph
            .locate(ProgramPoint::AfterOperation(operation.id))?;
        let measured = after
            .checked_sub(2)
            .and_then(|position| self.block(block).ok()?.operations.get(position))
            .is_some_and(|measurement| {
                matches!(&measurement.kind,
                OperationKind::ByteSequenceLength { source: measured } if measured == source)
                    && measurement
                        .result
                        .scalar_ref()
                        .is_some_and(|result| result.id == length)
            });
        if !measured {
            return Err(self.unsupported(
                concerns,
                "a byte subslice whose length is not its view's measurement directly before it",
            ));
        }
        let count = count_type();
        if [start, end, length]
            .iter()
            .any(|value| types.get(*value).copied() != Some(count))
        {
            return Err(self.foreign(concerns, "a byte subslice endpoint or length is not a u64"));
        }
        terminal_semantics::operation_safety_goal(operation, types)
            .map_err(crate::semantics_refusal(self.machine, operation.id))?
            .map(|(_, goal)| goal)
            .ok_or_else(|| self.foreign(concerns, "a byte subslice has no canonical bounds goal"))
    }

    /// The reaching-storage walk has already followed copies at their capture
    /// points and required the same producer on every incoming path. Rebase
    /// this length observation onto that producer, not an equal-sized view.
    pub(super) fn extent_equation(
        &self,
        measurement: &Operation,
        producer: OperationId,
    ) -> Checked<Option<Proposition>> {
        let concerns = Construct::Operation(measurement.id);
        let producer = self.operation(producer, concerns)?;
        if !self.graph.dominates(
            ProgramPoint::AfterOperation(producer.id),
            ProgramPoint::AfterOperation(measurement.id),
        )? {
            return Ok(None);
        }
        if matches!(producer.kind, OperationKind::ByteSequenceSubslice { .. })
            && self.repeated_extent(producer.id)?
        {
            return Ok(None);
        }
        let mut read = measurement.clone();
        let OperationKind::ByteSequenceLength { source } = &mut read.kind else {
            return Ok(None);
        };
        source.path.clear();
        source.place = match &producer.kind {
            OperationKind::ByteSequenceSubslice { .. } => {
                producer
                    .result
                    .as_structural()
                    .ok_or_else(|| self.foreign(concerns, "a reaching subslice has no result"))?
                    .place
            }
            OperationKind::EstablishByteSequenceLiteral { destination, .. } => *destination,
            _ => return Ok(None),
        };
        let observation = terminal_semantics::structural_effect_leaf_observation(&read)
            .map_err(crate::semantics_refusal(self.machine, measurement.id))?;
        let Some(observation) = observation else {
            return Ok(None);
        };
        match &producer.kind {
            OperationKind::EstablishByteSequenceLiteral { .. } => {
                terminal_semantics::literal_length_equation(&observation, producer)
            }
            OperationKind::ByteSequenceSubslice { .. } => {
                let produced = terminal_semantics::structural_effect_leaf_observation(producer)
                    .map_err(crate::semantics_refusal(self.machine, producer.id))?;
                let Some(produced) = produced else {
                    return Ok(None);
                };
                terminal_semantics::subslice_length_equation(&observation, &produced)
            }
            _ => unreachable!(),
        }
        .map_err(crate::semantics_refusal(self.machine, measurement.id))
    }

    // An endpoint SSA name in a loop may now denote a later occurrence than
    // the descriptor captured. Until that occurrence correspondence is known,
    // keep the descriptor available but learn no equation from its old bounds.
    fn repeated_extent(&self, producer: OperationId) -> Checked<bool> {
        let (origin, _) = self.graph.locate(ProgramPoint::AfterOperation(producer))?;
        let mut seen = vec![false; self.graph.block_count()];
        let mut pending = self.graph.predecessors(origin)?.to_vec();
        while let Some(block) = pending.pop() {
            if block == origin {
                return Ok(true);
            }
            let at = super::index(block.get());
            if !std::mem::replace(&mut seen[at], true) {
                pending.extend_from_slice(self.graph.predecessors(block)?);
            }
        }
        Ok(false)
    }

    /// Substituting a call contract asks what this exact descriptor contains;
    /// it does not manufacture a length measurement or its bounds evidence.
    pub(super) fn extent_contents(
        &self,
        producer: OperationId,
        at: ProgramPoint,
        scalar: ScalarType,
    ) -> Checked<Option<semantic_vocabulary::ScalarTerm>> {
        use semantic_vocabulary::{IntegerValue, ScalarTerm};
        let concerns = Construct::Operation(producer);
        if scalar != count_type()
            || !self
                .graph
                .dominates(ProgramPoint::AfterOperation(producer), at)?
        {
            return Ok(None);
        }
        let ScalarType::Integer(integer) = scalar else {
            unreachable!()
        };
        let term = match &self.operation(producer, concerns)?.kind {
            OperationKind::EstablishByteSequenceLiteral { bytes, .. } => {
                ScalarTerm::integer(integer, IntegerValue::Unsigned(bytes.len() as u128))
            }
            OperationKind::ByteSequenceSubslice { start, end, .. } => {
                if self.repeated_extent(producer)? {
                    return Ok(None);
                }
                ScalarTerm::exact_integer_subtract(
                    integer,
                    ScalarTerm::value(*end, scalar),
                    ScalarTerm::value(*start, scalar),
                )
            }
            _ => return Ok(None),
        }
        .map_err(|error| {
            self.foreign(
                concerns,
                &format!("descriptor extent has invalid scalar semantics: {error:?}"),
            )
        })?;
        Ok(Some(term))
    }
}
