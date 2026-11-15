//! A byte view borrows existing storage without creating ownership or access.
//! Authenticate that storage when lending an array or reading a descriptor;
//! check slice bounds against the descriptor measured immediately before use.

use super::{Shape, Storage};
use semantic_vocabulary::{
    IdTable, OperationId, PlaceId, Proposition, ScalarType, StructuralPlaceKind, ValueId,
};
use terminal_check_tables::{Checked, Construct, ProgramPoint};
use terminal_psi::{
    Operation, OperationKind, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    StructuralPathSegment, StructuralTypeShape,
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
    fn descriptor_access(
        &self,
        root: PlaceId,
        path: &[StructuralPathSegment],
        concerns: Construct,
        requested: StructuralAccess,
    ) -> Checked<bool> {
        let (multiplicity, authority) = if matches!(
            self.root_kind(root, concerns)?,
            StructuralPlaceKind::ByteSequenceLiteral { .. }
        ) {
            (
                StructuralMultiplicity::Unrestricted,
                StructuralAccess::SharedBorrow,
            )
        } else {
            let source = terminal_semantics::structural_argument_source(self.lowered, root, 0)
                .ok_or_else(|| {
                    self.foreign(
                        concerns,
                        "a descriptor source has no actual storage declaration",
                    )
                })?;
            (source.multiplicity, source.access)
        };
        Ok(terminal_semantics::reference_source_projection_accessible(
            self.types.iter(),
            self.root_type(root, concerns)?,
            multiplicity,
            authority,
            path,
            requested,
        ))
    }

    pub(super) fn byte_shape(
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

    /// A slice call borrows the array's real storage. Formation and normal
    /// return use the same rule, so the callee cannot establish a stronger
    /// array type or gain authority that its caller did not lend.
    pub(super) fn byte_array_presentation(
        &self,
        argument: &StructuralArgument,
        parameter: &terminal_psi::StructuralParameterDeclaration,
        concerns: Construct,
    ) -> Checked<bool> {
        let actual_type = self.root_type(argument.place, concerns)?;
        let Some(selected) = terminal_semantics::runtime_structural_path_tip(
            self.types.iter(),
            actual_type,
            &argument.path,
        ) else {
            return Ok(false);
        };
        let (
            terminal_psi::StructuralTypeShape::FixedArray { .. },
            terminal_psi::StructuralTypeShape::ByteSequence(_),
        ) = (
            &selected.shape,
            &self.declaration(parameter.structural_type, concerns)?.shape,
        )
        else {
            return Ok(false);
        };
        let Some(backing) = self.byte_array_backing(argument, parameter.position, concerns)? else {
            return Ok(false);
        };
        if terminal_semantics::fixed_byte_array_loan(
            self.types.iter(),
            self.module.structural_domains.iter(),
            &backing,
            argument,
            parameter,
        )
        .is_none()
        {
            return Err(self.unsupported(
                concerns,
                "an array loan without byte-view type, extent and qualification correspondence",
            ));
        }
        Ok(true)
    }

    /// A call and a new view can borrow the same array. Check its original
    /// input or result here before either route decides how to carry the view.
    fn byte_array_backing(
        &self,
        argument: &StructuralArgument,
        position: u32,
        concerns: Construct,
    ) -> Checked<Option<std::borrow::Cow<'_, terminal_psi::StructuralParameterDeclaration>>> {
        let selected = terminal_semantics::runtime_structural_path_tip(
            self.types.iter(),
            self.root_type(argument.place, concerns)?,
            &argument.path,
        )
        .ok_or_else(|| self.foreign(concerns, "array backing lost its typed storage path"))?;
        let StructuralTypeShape::FixedArray { element, .. } = selected.shape else {
            return Ok(None);
        };
        let backing =
            terminal_semantics::structural_argument_source(self.lowered, argument.place, position)
                .ok_or_else(|| {
                    self.foreign(concerns, "an array loan lost its actual root contract")
                })?;
        let unclaimed = !self
            .lowered
            .entry_claims
            .iter()
            .any(|claim| claim.input == argument.place)
            && !self
                .lowered
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == argument.place)
            && self
                .lowered
                .producing_operation(argument.place)
                .is_none_or(|(operation, _)| {
                    operation.result.as_structural().is_some_and(|result| {
                        result.claims.is_empty() && result.qualification_establishments.is_empty()
                    })
                });
        let plain_element = self.domains.get(selected.id).is_some_and(Vec::is_empty)
            && self.domains.get(element).is_some_and(Vec::is_empty)
            && self
                .declaration(element, concerns)?
                .scalar_restriction
                .is_none();
        if !unclaimed || !plain_element {
            return Err(self.unsupported(
                concerns,
                "an array view without plain element storage and unclaimed backing",
            ));
        }
        Ok(Some(backing))
    }

    /// Forming a view retains the array's storage and creates a descriptor;
    /// the separate loan judgment still checks authority and the view's lifetime.
    pub(super) fn array_view_formation(&self, operation: &Operation) -> Checked<u64> {
        use terminal_psi::{ByteSequenceCarrier, ElementViewRegion, StructuralPathQualification};
        let concerns = Construct::Operation(operation.id);
        let (destination, source, element, region, access, multiplicity) = match &operation.kind {
            OperationKind::EstablishElementView {
                destination,
                source,
                element,
                region,
            } => (
                *destination,
                source,
                *element,
                *region,
                StructuralAccess::SharedBorrow,
                StructuralMultiplicity::Unrestricted,
            ),
            OperationKind::EstablishMutableElementView {
                destination,
                source,
                element,
                region,
            } => (
                *destination,
                source,
                *element,
                *region,
                StructuralAccess::MutableBorrow,
                StructuralMultiplicity::Affine,
            ),
            _ => return Err(self.foreign(concerns, "array view has no formation operation")),
        };
        let result = operation
            .result
            .as_structural()
            .ok_or_else(|| self.foreign(concerns, "array view has no descriptor result"))?;
        if destination != result.place
            || source.access != access
            || region != ElementViewRegion::WholeCollection
            || result.multiplicity != multiplicity
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.qualification_establishments.is_empty()
            || !result.additional_input_qualifications.is_empty()
            || !result.claims.is_empty()
            || !self
                .domains
                .get(result.structural_type)
                .is_some_and(Vec::is_empty)
            || self
                .declaration(result.structural_type, concerns)?
                .scalar_restriction
                .is_some()
            || self.root_kind(result.place, concerns)?
                != (StructuralPlaceKind::OperationResult {
                    producer: operation.id,
                    structural_type: result.structural_type,
                })
            || self.declaration(result.structural_type, concerns)?.shape
                != StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: Some(access),
                })
        {
            return Err(self.foreign(
                concerns,
                "array view changes its descriptor type, access or ownership",
            ));
        }
        let selected = terminal_semantics::runtime_structural_path_tip(
            self.types.iter(),
            self.root_type(source.place, concerns)?,
            &source.path,
        )
        .ok_or_else(|| self.foreign(concerns, "array view lost its captured storage"))?;
        if !matches!(selected.shape, StructuralTypeShape::FixedArray { element: actual, .. } if actual == element)
        {
            return Err(self.foreign(concerns, "array view changes its element type"));
        }
        let actual = self
            .byte_array_backing(source, 0, concerns)?
            .ok_or_else(|| self.foreign(concerns, "array view lost its source declaration"))?;
        let window = terminal_semantics::fixed_byte_storage_window(
            self.types.iter(),
            actual.structural_type,
            &source.path,
        )
        .ok_or_else(|| {
            self.unsupported(concerns, "array view has no closed byte storage window")
        })?;
        let (whole, projected) = StructuralPathQualification::select(
            &actual.qualifications,
            &actual.projected_qualifications,
            window.backing_path,
        );
        if window.backing_path != source.path
            || window.offset != 0
            || window.length != window.backing_length
            || !whole.is_empty()
            || !projected.is_empty()
            || !actual.additional_input_qualifications.is_empty()
            || !window.backing_path.is_empty() && !actual.qualifications.is_empty()
        {
            return Err(self.unsupported(
                concerns,
                "array view needs its whole plain backing without qualification loss",
            ));
        }
        Ok(window.length)
    }

    /// `left == right` over two byte views, a `ByteSequenceEqual`.
    ///
    /// structural_predicates.md: "Byte-sequence equality compares live
    /// lengths and live byte prefixes." byte_views.md, of an evaluated
    /// content predicate: "The operand retains its own dominating length
    /// observation, readable access, and establishment." So each operand is
    /// a whole view this machine may read, and the length it names is that
    /// same view's measurement: the lowered form measures the left view,
    /// then the right one, then compares, with nothing else between, so no
    /// operation can have changed either extent. The result is a plain
    /// `bool` and the comparison stores nothing.
    pub(super) fn compared_byte_views(&self, operation: &Operation) -> Checked<()> {
        use terminal_psi::control_flow::ByteSequenceOperand;
        let concerns = Construct::Operation(operation.id);
        let OperationKind::ByteSequenceEqual { left, right } = &operation.kind else {
            return Err(self.foreign(
                concerns,
                "a byte comparison requested for another operation",
            ));
        };
        if operation.result.scalar_ref().is_none_or(|result| {
            result.scalar_type != ScalarType::Boolean || result.qualifications != Default::default()
        }) {
            return Err(self.foreign(concerns, "a byte comparison must produce a plain bool"));
        }
        let (block, after) = self
            .graph
            .locate(ProgramPoint::AfterOperation(operation.id))?;
        let operations = &self.block(block)?.operations;
        // `after` counts the comparison itself: the right view's
        // measurement is two back, the left view's three back.
        for (operand, back) in [(left, 3), (right, 2)] {
            let ByteSequenceOperand::Whole { source, length } = operand else {
                return Err(self.unsupported(concerns, "a comparison of a view stored in a field"));
            };
            if !source.path.is_empty() || source.access != StructuralAccess::SharedBorrow {
                return Err(self.unsupported(
                    concerns,
                    "a byte comparison of a view that is not read whole and shared",
                ));
            }
            self.readable_byte_view(operation, source.place, &source.path)?;
            let measured = after
                .checked_sub(back)
                .and_then(|position| operations.get(position))
                .is_some_and(|measurement| {
                    matches!(&measurement.kind, OperationKind::ByteSequenceLength { source: measured }
                        if measured.place == source.place && measured.path.is_empty())
                        && measurement
                            .result
                            .scalar_ref()
                            .is_some_and(|result| result.id == *length)
                });
            if !measured {
                return Err(self.unsupported(
                    concerns,
                    "a byte comparison whose length is not its view's measurement directly before it",
                ));
            }
        }
        Ok(())
    }

    /// A rank observes one descriptor binding at a specified block cut.
    /// Payload-only calls may keep that binding; replacement and unresolved
    /// overlapping writes may not reuse its earlier observation.
    pub(crate) fn rank_descriptor_kept(
        &self,
        source: &StructuralArgument,
        block: semantic_vocabulary::BlockId,
        from: usize,
        until: usize,
    ) -> Checked<bool> {
        let concerns = Construct::Block(block);
        let raw = self.argument(source, concerns)?;
        let descriptor = self.backing(&raw, block, from, concerns)?;
        // Every pointer binding crossed on the way to the descriptor must
        // remain frozen too; a stable old destination cannot authenticate a
        // later observation through a rebound reference.
        let mut prefixes = Vec::new();
        for (position, segment) in raw.segments.iter().enumerate() {
            if *segment == terminal_check_tables::bindings::Segment::Referent {
                let prefix = super::Lineage {
                    root: raw.root,
                    segments: raw.segments[..position].to_vec(),
                };
                prefixes.push(self.backing(&prefix, block, from, concerns)?);
            }
        }
        let mut leaf = descriptor.clone();
        leaf.segments
            .push(terminal_check_tables::bindings::Segment::Extent);
        for row in &self.log {
            if row.block != block || row.position <= from || row.position > until {
                continue;
            }
            if prefixes
                .iter()
                .any(|prefix| super::overlap(&row.lineage, prefix))
                || super::overlap(&row.lineage, &descriptor)
                    && !self.call_preserves_extent(super::Asked::Contents(&leaf), row, concerns)?
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// A field operation carries a local field ID beside its enclosing path.
    /// Resolve that ID only in the selected carrier before using the ordinary
    /// descriptor access and observation rules.
    pub(super) fn byte_field_argument(
        &self,
        root: PlaceId,
        path: &[StructuralPathSegment],
        field: semantic_vocabulary::StructuralFieldId,
        concerns: Construct,
    ) -> Checked<StructuralArgument> {
        let carrier = self.stored_at(root, path, concerns)?;
        let fields = match self.shape(&carrier, concerns)? {
            Shape::Typed(owner) => self.declaration(owner, concerns)?.shape.record_fields(),
            Shape::Payload(case) => Some(case.fields.as_slice()),
            _ => None,
        }
        .ok_or_else(|| self.foreign(concerns, "a byte field has no record or payload carrier"))?;
        let field = fields
            .iter()
            .find(|candidate| candidate.id == field)
            .ok_or_else(|| {
                self.foreign(
                    concerns,
                    "a byte field is not declared by its selected carrier",
                )
            })?;
        if field.field_type.byte_sequence_carrier().is_none() {
            return Err(self.foreign(concerns, "a byte field has another carrier"));
        }
        let mut path = path.to_vec();
        path.push(StructuralPathSegment::Field(field.identity));
        Ok(StructuralArgument {
            place: root,
            path,
            access: StructuralAccess::SharedBorrow,
        })
    }

    pub(super) fn readable_byte_view(
        &self,
        operation: &Operation,
        root: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Checked<()> {
        let concerns = Construct::Operation(operation.id);
        let shape = self.byte_shape(root, path, concerns)?;
        let readable = matches!(
            shape,
            StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
                access: None
                    | Some(StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow),
            })
        );
        if !readable
            || !self.descriptor_access(root, path, concerns, StructuralAccess::SharedBorrow)?
        {
            return Err(self.unsupported(
                concerns,
                "a byte read without a readable descriptor occurrence",
            ));
        }
        Ok(())
    }

    pub(super) fn writable_byte_view(&self, operation: &Operation, root: PlaceId) -> Checked<()> {
        let concerns = Construct::Operation(operation.id);
        let source = terminal_semantics::structural_argument_source(self.lowered, root, 0)
            .ok_or_else(|| {
                self.foreign(concerns, "a byte store lost its descriptor declaration")
            })?;
        let mutable = matches!(
            self.byte_shape(root, &[], concerns)?,
            StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
                access: None | Some(StructuralAccess::MutableBorrow),
            })
        );
        if !mutable
            || !self.descriptor_access(root, &[], concerns, StructuralAccess::MutableBorrow)?
            || source.access == StructuralAccess::Owned
                && source.multiplicity != StructuralMultiplicity::Affine
            || !source.qualifications.is_empty()
            || !source.projected_qualifications.is_empty()
            || !source.additional_input_qualifications.is_empty()
            || !self
                .domains
                .get(source.structural_type)
                .is_some_and(Vec::is_empty)
        {
            return Err(self.unsupported(
                concerns,
                "a byte store without an unqualified exclusive descriptor",
            ));
        }
        Ok(())
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
            || !self.descriptor_access(
                source.place,
                &source.path,
                concerns,
                StructuralAccess::SharedBorrow,
            )?
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
                    let shared_ingress = matches!(
                        self.root_kind(source, concerns)?,
                        StructuralPlaceKind::Parameter { .. }
                    ) && is_shared_bytes(&self.byte_shape(
                        source,
                        &projection.path,
                        concerns,
                    )?) && self.descriptor_access(
                        source,
                        &projection.path,
                        concerns,
                        StructuralAccess::SharedBorrow,
                    )?;
                    if shared_ingress {
                        true
                    } else {
                        let enclosing = projection.enclosing_storage().ok_or_else(|| {
                            self.unsupported(
                                concerns,
                                "a shared subslice has no captured backing path",
                            )
                        })?;
                        terminal_semantics::fixed_byte_storage_window(
                            self.types.iter(),
                            self.root_type(source, concerns)?,
                            &enclosing.path,
                        )
                        .is_some()
                            && self.descriptor_access(
                                source,
                                &enclosing.path,
                                concerns,
                                StructuralAccess::SharedBorrow,
                            )?
                    }
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
                return Err(
                    self.unsupported(concerns, "a subslice without authenticated shared backing")
                );
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
        trace: &super::Reaching,
    ) -> Checked<Option<Proposition>> {
        let concerns = Construct::Operation(measurement.id);
        let Some(result) = measurement.result.scalar_ref() else {
            return Err(self.foreign(concerns, "a length measurement has no scalar result"));
        };
        let Some(extent) = self.extent_contents(
            producer,
            ProgramPoint::AfterOperation(measurement.id),
            result.scalar_type,
            trace,
        )?
        else {
            return Ok(None);
        };
        Ok(Some(Proposition::Equal(
            semantic_vocabulary::ScalarTerm::value(result.id, result.scalar_type),
            extent,
        )))
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
        trace: &super::Reaching,
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
            OperationKind::EstablishElementView { .. }
            | OperationKind::EstablishMutableElementView { .. } => ScalarTerm::integer(
                integer,
                IntegerValue::Unsigned(
                    self.array_view_formation(self.operation(producer, concerns)?)? as u128,
                ),
            ),
            OperationKind::ByteSequenceSubslice { start, end, .. } => {
                if !self.current_extent(producer, trace)? && self.repeated_extent(producer)? {
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
