//! Store emission: port writes and the whole store family — primitive,
//! indexed, representation-view, element-view, loaned-byte, scalar-field,
//! and structural case-field stores, with their destination and extent
//! helpers. Each store composes the same destination machinery: resolve the
//! carrier path to a place root, emit the operand value, and publish one
//! Terminal operation against the frame's namespaces.

use super::super::primitive_locals::{self};
use super::{
    OperationFrame, StructuralTypeRoster, allocate_structural_result, field_window_place,
    last_operation_id, operation_result_declaration, require_parameter_window_root,
    structural_type_declaration,
};
use crate::emission::borrowed_window::BorrowedWindowRepairValue;
use crate::emission::indexed_write::{IndexedWriteLeaf, IndexedWriteTarget};
use crate::emission::operation_emission::buffer::scalar_result_declaration;
use crate::expression_preparation::bindings::ScalarBindings;
use crate::expression_preparation::bindings::structural_paths::lower_structural_path;
use crate::unit::{
    CheckedScalarExpressionRole, LoweringError, Operation, OperationKind, PlaceId,
    ServiceReachSummary, StructuralAccess, StructuralTypeId, ValueDeclaration, allocate_dense,
    lookup_service_id, terminal_scalar_type, unsupported,
};

impl OperationFrame<'_, '_> {
    pub(super) fn port_write(
        &mut self,
        service_reach: &ServiceReachSummary,
        port: u16,
        value: u8,
    ) -> Result<(), LoweringError> {
        let checked = self.checked;
        let rows = &checked.facts.service_reaches.rows;
        let [port_service] = rows.services(service_reach.direct) else {
            return unsupported(
                "port output does not carry the unique exact checked PortIo service",
            );
        };
        if !rows
            .services(service_reach.transitive)
            .contains(port_service)
        {
            return unsupported(
                "port output does not carry the unique exact checked PortIo service",
            );
        }
        // `CheckedUnitEffectOperationPlan::PortWrite` is minted only for the
        // exact checked asm-port-out builtin. Its singleton direct row is
        // therefore the symbol-backed PortIo authority; no spelling lookup is
        // repeated here.
        let service = lookup_service_id(self.service_ids, *port_service)?;
        self.push_unit(OperationKind::PortWrite {
            service,
            port,
            value,
        });
        Ok(())
    }

    /// A primitive store names an exclusive borrowed parameter projection or
    /// an established primitive local.
    pub(super) fn write_only_primitive_store(
        &mut self,
        statement_index: u32,
        role: CheckedScalarExpressionRole,
        destination: &checked_trees::CheckedStoreRoot,
        path: &[checked_trees::CheckedUnitStructuralPathSegment],
        value: &checked_trees::CheckedCallScalarArgument,
    ) -> Result<(), LoweringError> {
        if let Some(loan) =
            crate::scalar_graph::scalar_computations::fields::loaned_primitive_store(
                self.checked,
                self.machine,
                self.state,
                destination,
                path,
                self.structural_types.declarations(),
            )?
        {
            let projection = loan
                .byte_overlay
                .as_ref()
                .ok_or(LoweringError::Unsupported(
                    "primitive recast store lost its byte projection",
                ))?;
            // Capture the destination before evaluating the RHS, just as for
            // field stores. A recast loan never invents another owned home.
            let destination = self.prepare_loaned_byte_store(statement_index, &loan)?;
            let value = self.evaluation.source_value(
                self.checked,
                self.machine,
                self.state,
                statement_index,
                role,
                value,
                self.source_value_count,
                self.values,
                self.next_value,
                self.next_block,
                self.next_edge,
                self.operations,
                self.calls,
            )?;
            for kind in destination.emit(
                value,
                projection.stored_type,
                projection.offset,
                self.next_value,
                self.operations,
                self.calls,
            )? {
                self.push_unit(kind);
            }
            return Ok(());
        }
        let destination = match destination {
            checked_trees::CheckedStoreRoot::Parameter { position } => {
                // The root names the authored parameter position;
                // `structural_parameters` pairs each checked plan's authored
                // position with the declaration that position lowered into.
                let (_, parameter) = self
                    .evaluation
                    .structural_parameters
                    .iter()
                    .find(|(authored, _)| authored == position)
                    .ok_or(LoweringError::Unsupported(
                        "primitive store parameter is absent",
                    ))?;
                crate::emission::primitive_store::parameter_destination(
                    parameter,
                    path,
                    self.structural_types.declarations(),
                )?
            }
            checked_trees::CheckedStoreRoot::Local { symbol } => {
                if path.is_empty() {
                    let local = primitive_locals::find(self.primitive_locals, *symbol)?;
                    crate::emission::primitive_store::Destination {
                        place: local.declaration.id,
                        path: Vec::new(),
                    }
                } else {
                    // A projected store writes through an established
                    // structural local's owned place: the declared carrier
                    // supplies the walk, the binding table supplies storage.
                    let source = self
                        .evaluation
                        .structural_locals
                        .iter()
                        .find(|local| local.root == facts::PlaceRoot::Symbol(*symbol))
                        .map(|local| local.argument.clone())
                        .ok_or(LoweringError::Unsupported(
                            "primitive store lost its structural local destination",
                        ))?;
                    if source.access != StructuralAccess::Owned {
                        return unsupported("primitive store local is not an owned place");
                    }
                    let (_, authored) =
                        crate::declarations::state_declaration(self.checked, self.state)?;
                    let local = crate::emission::structural_scalar_store_source::local_destination(
                        self.checked,
                        authored,
                        statement_index,
                        *symbol,
                    )?;
                    let identity = self.checked.normalized_type_identity(local.type_reference);
                    let declaration = self
                        .structural_types
                        .declarations()
                        .iter()
                        .find(|declaration| declaration.identity == identity.as_str())
                        .ok_or(LoweringError::Unsupported(
                            "primitive store local lost its declared carrier",
                        ))?;
                    let (steps, _) = crate::emission::primitive_store::lower_destination_path(
                        declaration.id,
                        path,
                        self.structural_types.declarations(),
                    )?;
                    // A projected owned alias contributes its own walk: the
                    // destination is the leaf under the place that actually
                    // holds the storage. The alias path is static by
                    // construction, so it composes as ordinary steps.
                    let mut composed = source
                        .path
                        .iter()
                        .cloned()
                        .map(crate::emission::runtime_elements::ProjectionStep::Static)
                        .collect::<Vec<_>>();
                    composed.extend(steps);
                    crate::emission::primitive_store::Destination {
                        place: source.place,
                        path: composed,
                    }
                }
            }
        };
        let kind = crate::emission::primitive_store::emit_assignment(
            self.checked,
            self.machine,
            self.state,
            statement_index,
            destination,
            role,
            value,
            self.evaluation,
            self.source_value_count,
            self.values,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
            self.calls,
        )?;
        self.push_unit(kind);
        Ok(())
    }

    pub(super) fn structural_byte_sequence_field_store(
        &mut self,
        store: &checked_trees::CheckedByteSequenceStorePlan,
    ) -> Result<(), LoweringError> {
        use crate::emission::structural_byte_sequence_store::StoreRoot;
        let root = match store.root {
            checked_trees::CheckedStoreRoot::Parameter { position } => {
                let (_, parameter) = self
                    .evaluation
                    .structural_parameters
                    .iter()
                    .find(|(authored, _)| *authored == position)
                    .ok_or(LoweringError::Unsupported(
                        "byte-field store destination is absent",
                    ))?;
                StoreRoot::Parameter(parameter)
            }
            // A whole descriptor slot is the declared home itself; the home,
            // not the referenced bytes' shared access, supplies the authority.
            checked_trees::CheckedStoreRoot::Local { symbol }
                if matches!(
                    &store.destination,
                    checked_trees::CheckedByteSequenceStoreDestination::Descriptor {
                        field_identity: None,
                        ..
                    }
                ) =>
            {
                let (place, structural_type, qualified) =
                    crate::emission::store_destination::descriptor_home(
                        symbol,
                        &self.evaluation.structural_locals,
                        self.operations,
                    )?;
                StoreRoot::DescriptorHome {
                    place,
                    structural_type,
                    qualified,
                }
            }
            checked_trees::CheckedStoreRoot::Local { symbol } => {
                let (place, structural_type) = self.local_store_root(symbol)?;
                if self
                    .structural_types
                    .declarations()
                    .iter()
                    .any(|declaration| {
                        declaration.id == structural_type
                            && matches!(
                                declaration.shape,
                                terminal_psi::StructuralTypeShape::Record { .. }
                            )
                    })
                {
                    StoreRoot::Owned {
                        place,
                        structural_type,
                    }
                } else {
                    StoreRoot::Reference {
                        place,
                        structural_type,
                    }
                }
            }
        };
        // Only a roster this body owns may gain the generated literal-view
        // carrier; the store below rejects a published roster without it.
        if let StructuralTypeRoster::Owned(types) = &mut self.structural_types {
            crate::emission::structural_byte_sequence_store::prepare_types(&store.value, types)?;
        }
        if let checked_trees::CheckedByteSequenceStoreDestination::Descriptor {
            path,
            field_identity,
        } = &store.destination
        {
            let mut destination =
                crate::emission::structural_byte_sequence_store::prepare_descriptor_store(
                    store,
                    &root,
                    path,
                    field_identity.as_deref(),
                    self.structural_types.declarations(),
                )?;
            let indexes = crate::emission::runtime_elements::evaluate_selectors(
                &destination.steps,
                self.checked,
                self.machine,
                self.state,
                store.statement_index,
                self.evaluation,
                self.source_value_count,
                self.values,
                self.next_value,
                self.next_block,
                self.next_edge,
                self.operations,
                self.calls,
            )?;
            self.observe_store_element_extent(
                destination.destination,
                destination.structural_type,
                &destination.steps,
            )?;
            let path = crate::emission::runtime_elements::complete_path(
                std::mem::take(&mut destination.steps),
                indexes,
                self.calls,
                self.operations,
            )?;
            let source = self.byte_descriptor_operand(store)?;
            let kind = crate::emission::structural_byte_sequence_store::emit_descriptor_store(
                destination,
                path,
                source,
                &mut self.calls.next_obligation_identity,
                self.operations,
            )?;
            self.push_unit(kind);
            return Ok(());
        }
        let kind = crate::emission::structural_byte_sequence_store::emit_content_store(
            store,
            root,
            &self.evaluation.structural_parameters,
            &self.evaluation.structural_locals,
            self.structural_types.declarations(),
            self.private_places,
            self.next_place,
            self.next_value,
            &mut self.calls.next_obligation_identity,
            self.operations,
        )?;
        self.push_unit(kind);
        Ok(())
    }

    /// The scalar namespace an assignment's index and value resolve in: the
    /// composed state's own namespace when the evaluation carries one, and
    /// otherwise the ordinary dense source prefix over primitive storage,
    /// structural parameters, and established structural locals.
    pub(super) fn dense_assignment_namespace(&self) -> ScalarBindings {
        ScalarBindings::new(self.values.iter().map(|value| value.scalar_type))
            .with_primitive_storage(&self.evaluation.primitive_storage)
            .with_view_locals(&self.evaluation.view_locals)
            .with_place_observations(&self.evaluation.place_observations)
            .with_element_views(&self.evaluation.element_views)
            .with_structural_parameters(&self.evaluation.structural_parameters)
            .with_structural_locals(&self.evaluation.structural_locals)
    }

    /// One runtime-indexed write: the index, then the value, lower in the
    /// assignment's scalar namespace, and `indexed_write::emit` checks the
    /// leaf's destination and emits the coordinate, length and write. Only a
    /// byte field's dense namespace carries the resolved structural field and
    /// case observations. A byte leaf's authored root resolves in
    /// `structural_parameters`, the authored-to-lowered pairing the field,
    /// case-field and element-view stores use, not in `parameters`, whose
    /// positions are dense on the ordinary route.
    pub(super) fn indexed_write(
        &mut self,
        target: IndexedWriteTarget<'_>,
    ) -> Result<(), LoweringError> {
        let leaf = target.leaf();
        let destination = target.destination(
            &self.evaluation.structural_parameters,
            self.structural_types.declarations(),
        )?;
        let dense;
        let bindings = match &self.evaluation.scalar_bindings {
            Some(bindings) => {
                // State scalar positions are stable, but earlier operations
                // may have established owned or borrowed storage since that
                // namespace was created. Join the current local homes before
                // lowering either indexed operand.
                dense = bindings
                    .clone()
                    .with_structural_locals(&self.evaluation.structural_locals)
                    .with_view_locals(&self.evaluation.view_locals)
                    .with_place_observations(&self.evaluation.place_observations);
                &dense
            }
            None => {
                let namespace = self.dense_assignment_namespace();
                dense = match leaf {
                    IndexedWriteLeaf::ByteField(_) => namespace
                        .with_resolved_structural_observations(
                            &self.evaluation.structural_fields,
                            &self.evaluation.structural_cases,
                        ),
                    IndexedWriteLeaf::ByteView(_) | IndexedWriteLeaf::ElementView(_) => namespace,
                };
                &dense
            }
        };
        let index = match leaf.index_role(self.checked, self.state)? {
            Some(index_role) => bindings.expression_at(
                self.checked,
                self.state,
                leaf.statement_index(),
                index_role,
            )?,
            // A `&mut collection[index]` view local's index coordinate was
            // already checked against its establishment: it lowers directly.
            None => bindings.expression(&self.checked.facts.proven_scalar_indexes, leaf.index())?,
        };
        // A completed call-result source must be joined before adding private
        // staging slots to the scalar namespace. Pure source expressions name
        // that same authored prefix, even when the RHS later creates a join.
        let lowered = if matches!(
            leaf.value(),
            checked_trees::CheckedStoreValue::Computation(_)
        ) {
            None
        } else {
            Some(leaf.lower_value(bindings, self.checked, self.state, self.values)?)
        };
        let decisions =
            destination.source_decisions(self.checked, self.machine, self.state, leaf)?;
        let selector_position = self.values.len();
        let coordinate_type = terminal_scalar_type(checked_trees::types::PrimitiveType::U64)?;
        let coordinate = crate::emission::indexed_write::emit_index_coordinate(
            &index,
            decisions.index,
            self.values,
            self.next_value,
            &mut self.calls.next_obligation_identity,
            self.operations,
        )?;
        // Ordinary scalar continuations forward every retained value. Give the
        // selected coordinate a private slot before the RHS expands, then read
        // its rebound slot, not the destination expression, at the final store.
        self.values.push(ValueDeclaration {
            qualifications: Default::default(),
            id: coordinate,
            scalar_type: coordinate_type,
        });
        let value = if matches!(
            leaf.value(),
            checked_trees::CheckedStoreValue::Computation(_)
        ) {
            // The shared scalar evaluator carries the captured coordinate
            // through any private blocks in the RHS's evaluation schedule.
            crate::emission::indexed_write::IndexedWriteValue::Established(
                self.evaluation.field_assignment_value(
                    self.checked,
                    self.machine,
                    self.state,
                    leaf.statement_index(),
                    leaf.value(),
                    leaf.value_type(),
                    None,
                    self.values,
                    self.next_value,
                    self.next_block,
                    self.next_edge,
                    self.operations,
                    self.calls,
                )?,
            )
        } else {
            crate::emission::indexed_write::IndexedWriteValue::Expression(lowered.as_ref().ok_or(
                LoweringError::Unsupported("indexed write lost its retained scalar value"),
            )?)
        };
        let index =
            crate::emission::operation_emission::expressions::LoweredDirectExpression::Parameter {
                position: selector_position,
                scalar_type: coordinate_type,
            };
        let kind = crate::emission::indexed_write::emit(
            target,
            destination,
            decisions,
            &index,
            value,
            self.values,
            self.next_value,
            &mut self.calls.next_obligation_identity,
            self.operations,
        )?;
        self.push_unit(kind);
        self.values.truncate(selector_position);
        Ok(())
    }

    /// A store through a recast loan local emits the value once, then one
    /// primitive store per emitted footprint byte — or one whole store for
    /// a same-width leaf — in the dense scalar namespace the indexed-write
    /// family uses.
    pub(super) fn representation_view_store(
        &mut self,
        write: &checked_trees::CheckedRepresentationViewStorePlan,
    ) -> Result<(), LoweringError> {
        for kind in crate::emission::representation_view_store::emit(
            write,
            self.checked,
            self.machine,
            self.state,
            &mut *self.evaluation,
            self.source_value_count,
            self.values,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
            self.calls,
            self.structural_types.declarations(),
        )? {
            self.push_unit(kind);
        }
        Ok(())
    }

    /// Resolve a structural-local store destination to the local's live
    /// storage and type. Scalar, byte and sum fields require identical root
    /// authority; only the replacement's value contract differs.
    pub(super) fn local_store_root(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Result<(PlaceId, StructuralTypeId), LoweringError> {
        crate::emission::store_destination::unqualified_local_store_root(
            symbol,
            &self.evaluation.structural_locals,
            self.operations,
        )
    }

    pub(super) fn element_view_write(
        &mut self,
        write: &checked_trees::CheckedElementViewWritePlan,
    ) -> Result<(), LoweringError> {
        // A `&mut`-borrowed parameter names its destination place directly; a
        // locally bound `&mut` view names the home place its establishment
        // produced, resolved through the structural-local roster like a whole
        // structural store's local root.
        let (destination_place, destination_type) = match write.destination {
            checked_trees::CheckedStoreRoot::Parameter { position } => {
                let (_, destination) = self
                    .evaluation
                    .structural_parameters
                    .iter()
                    .find(|(authored, _)| *authored == position)
                    .ok_or(LoweringError::Unsupported(
                        "element-view write names an unknown parameter",
                    ))?;
                if destination.access != StructuralAccess::MutableBorrow
                    || destination.multiplicity
                        != terminal_psi::StructuralMultiplicity::Unrestricted
                    || !destination.qualifications.is_empty()
                    || !destination.projected_qualifications.is_empty()
                {
                    return unsupported("element-view write parameter lost mutable custody");
                }
                (destination.place, destination.structural_type)
            }
            checked_trees::CheckedStoreRoot::Local { symbol } => self.local_store_root(symbol)?,
        };
        self.indexed_write(IndexedWriteTarget::ElementView {
            write,
            place: destination_place,
            structural_type: destination_type,
        })
    }

    /// Field and element stores both retain a recast loan's original backing.
    /// This prepares selectors only; callers evaluate their ordinary RHS next.
    pub(super) fn prepare_loaned_byte_store(
        &mut self,
        statement_index: u32,
        loan: &crate::scalar_graph::scalar_computations::fields::LoanedStore,
    ) -> Result<crate::emission::representation_view_store::ByteStoreDestination, LoweringError>
    {
        use crate::scalar_graph::scalar_computations::fields::LoanedSource;
        let (place, root_type) = match loan.source {
            LoanedSource::Parameter(position) => {
                let (_, parameter) = self
                    .evaluation
                    .structural_parameters
                    .get(position as usize)
                    .ok_or(LoweringError::Unsupported(
                        "record store lost its backing parameter",
                    ))?;
                crate::emission::structural_scalar_store::validate_root_access(parameter)?;
                (parameter.place, parameter.structural_type)
            }
            LoanedSource::Local(symbol) => self.local_store_root(symbol)?,
        };
        let captured_index = if loan.byte_overlay.is_some()
            && matches!(
                loan.path.last(),
                Some(checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(_))
            ) {
            let bindings = self.dense_assignment_namespace();
            let coordinate = bindings.observation_coordinate(loan.symbol)?;
            Some(crate::emission::operation_emission::emit_direct_expression(
                &coordinate,
                self.values,
                self.next_value,
                self.operations,
            ))
        } else {
            None
        };
        let mut destination = crate::emission::representation_view_store::prepare_byte_destination(
            place,
            root_type,
            &loan.path,
            captured_index,
            self.checked,
            self.machine,
            self.state,
            statement_index,
            self.evaluation,
            self.source_value_count,
            self.values,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
            self.calls,
            self.structural_types.declarations(),
        )?;
        if let Some(projection) = &loan.byte_overlay {
            let steps = projection
                .indices
                .iter()
                .map(|index| crate::emission::runtime_elements::selector_step(&index.selector))
                .collect::<Vec<_>>();
            let indexes = crate::emission::runtime_elements::evaluate_selectors(
                &steps,
                self.checked,
                self.machine,
                self.state,
                statement_index,
                self.evaluation,
                self.source_value_count,
                self.values,
                self.next_value,
                self.next_block,
                self.next_edge,
                self.operations,
                self.calls,
            )?;
            if indexes.len() != projection.indices.len() {
                return unsupported("record byte store lost a declared array coordinate");
            }
            for (index, dimension) in indexes.into_iter().zip(&projection.indices) {
                destination.project_index(
                    index.index,
                    dimension.stride,
                    dimension.extent,
                    self.next_value,
                    self.operations,
                )?;
            }
        }
        Ok(destination)
    }

    pub(super) fn structural_scalar_field_store(
        &mut self,
        store: &checked_trees::CheckedStructuralScalarFieldStorePlan,
    ) -> Result<(), LoweringError> {
        let loan = crate::scalar_graph::scalar_computations::fields::loaned_store(
            self.checked,
            self.machine,
            self.state,
            store,
            self.structural_types.declarations(),
        )?;
        let mut rejoined;
        let store = if let Some(loan) = loan {
            use crate::scalar_graph::scalar_computations::fields::LoanedSource;
            let destination = match loan.source {
                LoanedSource::Parameter(position) => {
                    let (authored, _) = self
                        .evaluation
                        .structural_parameters
                        .get(position as usize)
                        .ok_or(LoweringError::Unsupported(
                            "record store lost its captured parameter",
                        ))?;
                    checked_trees::CheckedStoreRoot::Parameter {
                        position: *authored,
                    }
                }
                LoanedSource::Local(symbol) => checked_trees::CheckedStoreRoot::Local { symbol },
            };
            if let Some(projection) = &loan.byte_overlay {
                let destination = self.prepare_loaned_byte_store(store.statement_index, &loan)?;
                let value = self.scalar_field_store_value(store)?;
                for kind in destination.emit(
                    value,
                    projection.stored_type,
                    projection.offset,
                    self.next_value,
                    self.operations,
                    self.calls,
                )? {
                    self.push_unit(kind);
                }
                return Ok(());
            }
            rejoined = store.clone();
            rejoined.destination = destination;
            rejoined.carrier_path = loan.path;
            &rejoined
        } else {
            store
        };
        // A parameter destination roots the store at its borrowed place; an
        // owned structural local roots it at the local's live home place,
        // resolved through the structural-local roster.
        let (destination_place, destination_type, mut lowered) = match store.destination {
            checked_trees::CheckedStoreRoot::Parameter { position } => {
                // The destination names the authored parameter position;
                // `structural_parameters` pairs each checked plan's authored
                // position with the declaration that position lowered into.
                let (_, destination) = self
                    .evaluation
                    .structural_parameters
                    .iter()
                    .find(|(authored, _)| *authored == position)
                    .ok_or(LoweringError::Unsupported(
                        "structural scalar store names an unknown parameter",
                    ))?;
                let lowered =
                    crate::emission::structural_scalar_store::lower_structural_scalar_store_place(
                        store,
                        store.statement_index,
                        destination,
                        self.structural_types.declarations(),
                    )?;
                (destination.place, destination.structural_type, lowered)
            }
            checked_trees::CheckedStoreRoot::Local { symbol } => {
                let (place, root_type) = self.local_store_root(symbol)?;
                let lowered =
                    crate::emission::structural_scalar_store::lower_structural_scalar_store_local(
                        store,
                        root_type,
                        self.structural_types.declarations(),
                    )?;
                (place, root_type, lowered)
            }
        };
        lowered.bind_range_decision(self.checked, self.machine, self.state, store)?;
        // The carrier's runtime elements are evaluated before the value.
        let indexes = crate::emission::runtime_elements::evaluate_selectors(
            &lowered.steps,
            self.checked,
            self.machine,
            self.state,
            store.statement_index,
            self.evaluation,
            self.source_value_count,
            self.values,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
            self.calls,
        )?;
        let value = self.scalar_field_store_value(store)?;
        self.observe_store_element_extent(destination_place, destination_type, &lowered.steps)?;
        let kind = lowered.into_operation(
            destination_place,
            indexes,
            value.id,
            self.calls,
            self.operations,
        )?;
        self.push_unit(kind);
        Ok(())
    }

    /// A descriptor's live extent owns runtime bounds through either a scalar
    /// field store or a stored-view replacement. References keep their own
    /// authority; following the type here only finds the extent representation.
    pub(super) fn observe_store_element_extent(
        &mut self,
        destination: semantic_vocabulary::PlaceId,
        mut structural_type: StructuralTypeId,
        steps: &[crate::emission::runtime_elements::ProjectionStep],
    ) -> Result<(), LoweringError> {
        use crate::emission::runtime_elements::ProjectionStep;
        let declarations = self.structural_types.declarations();
        if let Some(terminal_psi::StructuralTypeShape::Reference { referent, .. }) = declarations
            .iter()
            .find(|declaration| declaration.id == structural_type)
            .map(|declaration| &declaration.shape)
        {
            structural_type = *referent;
        }
        let is_view = declarations.iter().any(|declaration| {
            declaration.id == structural_type
                && matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::ElementView { .. }
                )
        });
        if !is_view
            || !steps
                .iter()
                .any(|step| !matches!(step, ProjectionStep::Static(_)))
        {
            return Ok(());
        }
        let length = crate::terminal_identities::value_id(allocate_dense(self.next_value)?);
        let observation = self.operations.allocate();
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: observation,
            result: scalar_result_declaration(
                length,
                terminal_scalar_type(crate::unit::PrimitiveType::U64)?,
            ),
            kind: OperationKind::ElementViewLength {
                source: destination,
            },
        });
        Ok(())
    }

    pub(super) fn scalar_field_store_value(
        &mut self,
        store: &checked_trees::CheckedStructuralScalarFieldStorePlan,
    ) -> Result<ValueDeclaration, LoweringError> {
        self.evaluation.field_assignment_value(
            self.checked,
            self.machine,
            self.state,
            store.statement_index,
            &store.value,
            store.primitive_type,
            Some(&store.field_identity),
            self.values,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
            self.calls,
        )
    }

    /// Map an authored store root to the window argument's structural lane.
    /// Composed source-reader declarations keep authored `position` values;
    /// roster order, not that field, is the checked dense parameter namespace.
    pub(super) fn window_destination(
        &self,
        destination: checked_trees::CheckedStoreRoot,
    ) -> Result<
        (
            StructuralTypeId,
            checked_trees::CheckedUnitStructuralArgumentSourcePlan,
        ),
        LoweringError,
    > {
        use checked_trees::CheckedUnitStructuralArgumentSourcePlan;
        match destination {
            checked_trees::CheckedStoreRoot::Parameter { position } => {
                let (_, authored) = self
                    .evaluation
                    .structural_parameters
                    .iter()
                    .find(|(source_position, _)| *source_position == position)
                    .ok_or(LoweringError::Unsupported(
                        "window store names an unknown authored parameter",
                    ))?;
                let (dense, parameter) = self
                    .parameters
                    .iter()
                    .enumerate()
                    .find(|(_, parameter)| parameter.place == authored.place)
                    .ok_or(LoweringError::Unsupported(
                        "window store parameter has no retained slot",
                    ))?;
                Ok((
                    parameter.structural_type,
                    CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index: u32::try_from(dense).map_err(|_| {
                            LoweringError::Unsupported(
                                "window parameter index exceeds its namespace",
                            )
                        })?,
                    },
                ))
            }
            checked_trees::CheckedStoreRoot::Local { symbol } => {
                let (_, structural_type) = self.local_store_root(symbol)?;
                Ok((
                    structural_type,
                    CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
                ))
            }
        }
    }

    /// Replace one unrestricted sum field beneath a mutable-borrowed
    /// parameter. The checked plan supplies the authored value as a parameter
    /// projection an owned or shared place still holds: leaf-copy it into a
    /// fresh owned place, vacate the authored field through the window
    /// ledger, restore the hole with the copy, and retire the moved-out leaf
    /// trivially on the following edge.
    pub(super) fn structural_case_field_store(
        &mut self,
        store: &checked_trees::CheckedStructuralCaseFieldStorePlan,
    ) -> Result<(), LoweringError> {
        // A parameter destination roots the window at its borrowed place; an
        // owned structural local roots it at the local's live home place.
        // The window pair below writes through either root identically.
        let (root_type, source) = self.window_destination(store.destination)?;
        let (_, field) = crate::emission::structural_scalar_store::lower_structural_field_path(
            root_type,
            &store.carrier_path,
            &store.field_identity,
            self.structural_types.declarations(),
        )?;
        let terminal_psi::StructuralFieldType::Structural(hole_type) = field.field_type else {
            return unsupported("structural case field store names a non-structural field");
        };
        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
            parameter_index: value_index,
        } = store.value.source
        else {
            return unsupported("structural case field store value is not a parameter");
        };
        if !store.value.path.iter().all(|segment| {
            matches!(
                segment,
                checked_trees::CheckedUnitStructuralPathSegment::Field(_)
            )
        }) {
            return unsupported("structural case field store value path leaves record fields");
        }
        let value_root =
            self.parameters
                .get(value_index as usize)
                .ok_or(LoweringError::Unsupported(
                    "structural case field store value names an unknown parameter",
                ))?;
        let source_place = self.evaluation.current_structural_place(value_root.place);
        // The copy reads the source where it lives: a shared loan or an owned
        // parameter both admit the leaf copy without moving anything.
        let (copied_place, copy) = allocate_structural_result(
            self.operations,
            self.next_place,
            hole_type,
            terminal_psi::StructuralMultiplicity::Unrestricted,
            Vec::new(),
            OperationKind::StructuralLeafCopy {
                source: source_place,
                path: lower_structural_path(&store.value.path)?,
            },
        )?;
        self.private_places
            .push(operation_result_declaration(copied_place, copy, hole_type));
        let window_place = field_window_place(
            source,
            &store.carrier_path,
            &store.field_identity,
            store.value.type_identity.clone(),
        );
        require_parameter_window_root(self.parameters, &window_place, self.evaluation)?;
        let moved = self.emit_window_move(
            store.statement_index,
            &window_place,
            &checked_trees::CheckedUnitStructuralResultBindingPlan {
                statement_index: store.statement_index,
                binding_ordinal: 0,
                type_identity: store.value.type_identity.clone(),
                multiplicity: language_semantics::Multiplicity::Unrestricted,
            },
        )?;
        // The moved-out leaf is an internal temporary, not an authored
        // result — declare it as a private temporary so the verifier finds
        // its producing operation, then the cleanup edge retires it.
        let move_id = last_operation_id(
            self.operations,
            "structural case field store move emitted no operation",
        )?;
        self.private_places
            .push(operation_result_declaration(moved, move_id, hole_type));
        // The moved-out leaf is `Unrestricted` by the plan's own contract, so
        // no custody action retires it: edge discards admit affine places
        // only, and an unrestricted owned place may simply be left behind.
        self.emit_window_store(
            store.statement_index,
            &window_place,
            BorrowedWindowRepairValue {
                place: copied_place,
                structural_type: hole_type,
            },
        )
    }

    /// Replace one sum field with an authored case literal: the store itself
    /// establishes the construction. Every scalar operand evaluates in the
    /// literal's `StructuralValueField` namespace, an unnamed common field
    /// takes its declared zero, and the established place repairs the vacated
    /// field through the same move/store window pair a whole-place store
    /// emits — the hole's custody story is identical; only the producer
    /// differs.
    pub(super) fn structural_case_literal_field_store(
        &mut self,
        store: &checked_trees::CheckedStructuralCaseLiteralFieldStorePlan,
    ) -> Result<(), LoweringError> {
        let (root_type, source) = self.window_destination(store.destination)?;
        let (_, field) = crate::emission::structural_scalar_store::lower_structural_field_path(
            root_type,
            &store.carrier_path,
            &store.field_identity,
            self.structural_types.declarations(),
        )?;
        let terminal_psi::StructuralFieldType::Structural(hole_type) = field.field_type else {
            return unsupported("structural case literal field store names a non-structural field");
        };
        let declaration = structural_type_declaration(
            self.structural_types.declarations(),
            hole_type,
            "structural case literal field store type missing",
        )?;
        let (common, cases) = match &declaration.shape {
            terminal_psi::StructuralTypeShape::Sum { cases } => (Vec::new(), cases.clone()),
            terminal_psi::StructuralTypeShape::Mixed { fields, cases } => (
                fields
                    .iter()
                    .filter(|field| {
                        !matches!(
                            field.field_type,
                            terminal_psi::StructuralFieldType::Erased { .. }
                        )
                    })
                    .cloned()
                    .collect::<Vec<_>>(),
                cases.clone(),
            ),
            _ => return unsupported("case literal field store has a non-sum shape"),
        };
        let case = cases
            .iter()
            .find(|case| case.identity == store.case_identity)
            .ok_or(LoweringError::Unsupported(
                "case literal field store case declaration missing",
            ))?;
        // A mixed shape's establishment roster is its runtime common fields in
        // declaration order, then the selected case's payload.
        let shape_fields = common
            .iter()
            .chain(case.fields.iter().filter(|field| {
                !matches!(
                    field.field_type,
                    terminal_psi::StructuralFieldType::Erased { .. }
                )
            }))
            .cloned()
            .collect::<Vec<_>>();
        // Case construction uses the same current assignment namespace as
        // ordinary field stores. A single-state body has no preinstalled
        // state namespace, including when this case has no scalar payload.
        let bindings = self.evaluation.scalar_bindings.clone().unwrap_or_else(|| {
            self.dense_assignment_namespace()
                .with_resolved_structural_observations(
                    &self.evaluation.structural_fields,
                    &self.evaluation.structural_cases,
                )
        });
        let mut initialized = Vec::new();
        for field_plan in &store.fields {
            let (position, target) = shape_fields
                .iter()
                .enumerate()
                .find(|(_, candidate)| candidate.identity == field_plan.field_identity)
                .ok_or(LoweringError::Unsupported(
                    "case literal field store field identity missing",
                ))?;
            if !matches!(
                target.field_type,
                terminal_psi::StructuralFieldType::Scalar(_)
                    | terminal_psi::StructuralFieldType::IeeeFloat(_)
                    | terminal_psi::StructuralFieldType::BoundedInteger(_)
                    | terminal_psi::StructuralFieldType::RefinedScalar(_)
            ) {
                return unsupported("case literal operand requires a scalar carrier");
            }
            let expression = bindings.expression_at(
                self.checked,
                self.state,
                store.statement_index,
                CheckedScalarExpressionRole::StructuralValueField {
                    expression: store.literal,
                    field_ordinal: field_plan.field_ordinal,
                },
            )?;
            if target.field_type.scalar_type() != Some(expression.scalar_type())
                || crate::emission::expression_validation::direct_expression_contains_short_circuit(
                    &expression,
                )
            {
                return unsupported(
                    "case literal operand needs an exact branch-free scalar operand",
                );
            }
            crate::emission::expression_validation::validate_direct_parameter_types(
                &expression,
                &self
                    .values
                    .iter()
                    .map(|value| value.scalar_type)
                    .collect::<Vec<_>>(),
            )?;
            let value = crate::emission::operation_emission::expressions::emit_direct_expression(
                &expression,
                self.values,
                self.next_value,
                self.operations,
            );
            // The verifier reconstructs the declared range against the final
            // SSA initializer; a bounded field requests that proof.
            let range_obligation =
                if terminal_semantics::scalar_field_requires_evidence(&target.field_type) {
                    Some(crate::terminal_identities::obligation_id(allocate_dense(
                        &mut self.calls.next_obligation_identity,
                    )?))
                } else {
                    None
                };
            initialized.push((
                position,
                terminal_psi::RecordFieldInitializer {
                    field: target.id,
                    value: terminal_psi::RecordFieldValue::Scalar {
                        value,
                        range_obligation,
                    },
                },
            ));
        }
        // Case construction zero-initializes every common field the literal
        // leaves unnamed.
        for (position, target) in common.iter().enumerate() {
            if initialized
                .iter()
                .any(|(initialized, _)| *initialized == position)
            {
                continue;
            }
            let scalar_type = target
                .field_type
                .scalar_type()
                .ok_or(LoweringError::Unsupported(
                    "an unnamed common field has no scalar zero",
                ))?;
            let zero = crate::emission::operation_emission::expressions::emit_scalar_leaf(
                crate::unit::attached_unit::structural_values::record::zero_constant(scalar_type),
                scalar_type,
                self.next_value,
                self.operations,
            );
            let range_obligation =
                if terminal_semantics::scalar_field_requires_evidence(&target.field_type) {
                    Some(crate::terminal_identities::obligation_id(allocate_dense(
                        &mut self.calls.next_obligation_identity,
                    )?))
                } else {
                    None
                };
            initialized.push((
                position,
                terminal_psi::RecordFieldInitializer {
                    field: target.id,
                    value: terminal_psi::RecordFieldValue::Scalar {
                        value: zero,
                        range_obligation,
                    },
                },
            ));
        }
        initialized.sort_by_key(|(position, _)| *position);
        let (copied_place, establishment) = allocate_structural_result(
            self.operations,
            self.next_place,
            hole_type,
            terminal_psi::StructuralMultiplicity::Unrestricted,
            Vec::new(),
            OperationKind::EstablishStructuralCase {
                result_case: case.id,
                fields: initialized.into_iter().map(|(_, field)| field).collect(),
            },
        )?;
        self.private_places.push(operation_result_declaration(
            copied_place,
            establishment,
            hole_type,
        ));
        let window_place = field_window_place(
            source,
            &store.carrier_path,
            &store.field_identity,
            store.type_identity.clone(),
        );
        require_parameter_window_root(self.parameters, &window_place, self.evaluation)?;
        let moved = self.emit_window_move(
            store.statement_index,
            &window_place,
            &checked_trees::CheckedUnitStructuralResultBindingPlan {
                statement_index: store.statement_index,
                binding_ordinal: 0,
                type_identity: store.type_identity.clone(),
                multiplicity: language_semantics::Multiplicity::Unrestricted,
            },
        )?;
        let move_id = last_operation_id(
            self.operations,
            "structural case literal field store move emitted no operation",
        )?;
        self.private_places
            .push(operation_result_declaration(moved, move_id, hole_type));
        // The moved-out leaf is `Unrestricted` by the plan's own contract, so
        // no custody action retires it.
        self.emit_window_store(
            store.statement_index,
            &window_place,
            BorrowedWindowRepairValue {
                place: copied_place,
                structural_type: hole_type,
            },
        )
    }
}
