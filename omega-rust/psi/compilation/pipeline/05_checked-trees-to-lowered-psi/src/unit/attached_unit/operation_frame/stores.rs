//! Store emission over checked places and evaluated values.
//!
//! `OperationFrame::store` lowers every checked `CheckedStorePlan`. The
//! plan's root resolves once in this frame — an authored parameter through
//! the authored-to-lowered parameter pairing, a local through its live home —
//! and the destination's leaf representation then selects the Terminal
//! operation that joins the destination place and the evaluated value:
//!
//! - `Primitive`: `WriteOnlyPrimitiveStore` over the complete projection, or
//!   one primitive store per footprint byte through a recast loan;
//! - `ScalarField`: `StructuralScalarFieldStore` with its declared range
//!   obligation, rejoined through a captured loan when the root is one;
//! - `ByteElement` and `ViewElement`: the runtime-indexed write of
//!   `emission::indexed_write`, whose carrier decides `ByteSequenceWrite`,
//!   `StructuralByteSequenceFieldByteStore` or `ElementViewWrite`;
//! - `RepresentationView`: the view's same-width field store or footprint
//!   byte stores (`emission::representation_view_store`);
//! - `ByteContents` and `ByteDescriptor`: contents replacement or one
//!   descriptor slot store (`emission::structural_byte_sequence_store`);
//! - `SumField`: a copied place or an established case literal restoring the
//!   field through its borrowed window;
//! - `WindowRepair`: the owned binding restoring an open borrowed window's
//!   hole, and `OwnedRebind`: an owned binding's home replaced by its new
//!   binding (`operation_frame::locals`).
//!
//! Every store evaluates its destination's runtime selectors before its
//! value, as the authored assignment orders them.

use super::{
    OperationFrame, StructuralTypeRoster, allocate_structural_result, last_operation_id,
    operation_result_declaration, require_parameter_window_root, structural_type_declaration,
    window_place,
};
use crate::emission::borrowed_window::BorrowedWindowRepairValue;
use crate::emission::indexed_write::IndexedWriteTarget;
use crate::expression_preparation::bindings::ScalarBindings;
use crate::expression_preparation::bindings::structural_paths::lower_structural_path;
use crate::unit::{
    CheckedScalarExpressionRole, LoweringError, OperationKind, PlaceId, StructuralAccess,
    StructuralParameterDeclaration, StructuralTypeId, ValueDeclaration, allocate_dense,
    terminal_scalar_type, unsupported,
};
use checked_trees::{
    CheckedScalarStoreValue, CheckedStoreLeaf, CheckedStorePlan, CheckedStoreValue,
};

impl OperationFrame<'_, '_> {
    /// Lower one store. Its leaf representation decides the Terminal
    /// operation; see the module documentation.
    pub(super) fn store(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        store: &CheckedStorePlan,
    ) -> Result<(), LoweringError> {
        match &store.destination.leaf {
            CheckedStoreLeaf::Primitive {
                receiving_range, ..
            } => {
                if let Some(range) = receiving_range
                    && !crate::emission::structural_scalar_store::source_range_decided(
                        self.checked,
                        range.primitive,
                        range.decision,
                        Some(range.integer),
                    )?
                {
                    return unsupported("primitive store lost its receiving-range judgment");
                }
                self.primitive_store(identities, store)
            }
            CheckedStoreLeaf::ScalarField {
                primitive_type,
                range_decision,
            } => self.scalar_field_store(identities, store, *primitive_type, *range_decision),
            CheckedStoreLeaf::ByteElement { .. } => {
                let target = self.byte_element_target(store)?;
                self.indexed_write(identities, target)
            }
            CheckedStoreLeaf::ViewElement { .. } => {
                let target = self.view_element_target(identities, store)?;
                self.indexed_write(identities, target)
            }
            CheckedStoreLeaf::RepresentationView { view } => {
                self.representation_view_store(identities, store, *view)
            }
            CheckedStoreLeaf::ByteContents | CheckedStoreLeaf::ByteDescriptor => {
                self.byte_sequence_store(identities, store)
            }
            CheckedStoreLeaf::SumField => self.sum_field_store(identities, store),
            CheckedStoreLeaf::StructuralStore {
                type_identity,
                multiplicity,
                opening_binding,
            } => {
                let Some(value) = store.installed_value() else {
                    return unsupported("window repair installs no whole value");
                };
                let source = match store.destination.root {
                    checked_trees::CheckedStoreRoot::Parameter { .. } => {
                        self.window_destination(
                            identities,
                            store.destination.root,
                            &store.destination.path,
                        )?
                        .1
                    }
                    checked_trees::CheckedStoreRoot::Local { symbol } => {
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                            symbol,
                        }
                    }
                };
                let place = window_place(source, &store.destination.path, *type_identity);
                self.store_structural_field(
                    identities,
                    store.statement_index,
                    &place,
                    *opening_binding,
                    value,
                    *multiplicity,
                )
            }
            CheckedStoreLeaf::OwnedRebind { type_identity } => {
                let Some(value) = store.installed_value() else {
                    return unsupported("owned rebind installs no whole value");
                };
                if !store.destination.path.is_empty() {
                    return unsupported("owned rebind destination is not its whole binding");
                }
                self.rebind_structural_binding(
                    store.statement_index,
                    store.destination.root,
                    *type_identity,
                    value,
                )
            }
        }
    }

    /// The live home of a store rooted at a structural local, resolved
    /// through the structural-local roster. Scalar, byte and sum fields and
    /// view elements require identical root authority; only the
    /// replacement's value contract differs.
    fn local_home(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Result<(PlaceId, StructuralTypeId), LoweringError> {
        crate::emission::store_destination::unqualified_local_store_root(
            symbol,
            &self.evaluation.structural_locals,
            self.evaluation.structural_root_parameters(),
            self.operations,
        )
    }

    /// A primitive leaf names an exclusive borrowed parameter projection, an
    /// established primitive local, or a projection below an owned structural
    /// local; through a recast loan local it writes the original backing's
    /// bytes.
    fn primitive_store(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        store: &CheckedStorePlan,
    ) -> Result<(), LoweringError> {
        let Some((role, value)) = store.value.scalar() else {
            return unsupported("primitive store holds no scalar value");
        };
        let authored_value = match value {
            CheckedScalarStoreValue::Pure(expression) => Some(
                checked_trees::CheckedCallScalarArgument::Pure(expression.clone()),
            ),
            CheckedScalarStoreValue::Computation(root) => {
                Some(checked_trees::CheckedCallScalarArgument::Computation(*root))
            }
            CheckedScalarStoreValue::ScalarResult { .. } => None,
        };
        let statement_index = store.statement_index;
        let destination = &store.destination.root;
        let path = store.destination.path.as_slice();
        if let Some(loan) = crate::scalar_lowering::computations::fields::loaned_primitive_store(
            identities,
            self.checked,
            self.machine,
            self.state,
            destination,
            path,
            self.structural_types.declarations(),
        )? {
            let authored_value = authored_value.as_ref().ok_or(LoweringError::Unsupported(
                "primitive recast store has no call-result route",
            ))?;
            let projection = loan
                .byte_overlay
                .as_ref()
                .ok_or(LoweringError::Unsupported(
                    "primitive recast store lost its byte projection",
                ))?;
            // Capture the destination before evaluating the RHS, just as for
            // field stores. A recast loan never invents another owned home.
            let destination = self.prepare_loaned_byte_store(identities, statement_index, &loan)?;
            let value = self.evaluation.source_value(
                identities,
                self.checked,
                self.machine,
                self.state,
                statement_index,
                role,
                authored_value,
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
        let mut destination = match destination {
            checked_trees::CheckedStoreRoot::Parameter { position } => {
                let parameter = store_parameter(&self.evaluation.structural_parameters, *position)?;
                crate::emission::primitive_store::parameter_destination(
                    identities,
                    parameter,
                    path,
                    self.structural_types.declarations(),
                )?
            }
            checked_trees::CheckedStoreRoot::Local { symbol } => {
                // Whole and projected stores resolve one original home. An
                // empty projection still retains the declaration's receiving
                // restriction; the carrier scalar type alone grants no store.
                let mut homes = self
                    .evaluation
                    .structural_locals
                    .iter()
                    .filter(|home| home.root == facts::PlaceRoot::Symbol(*symbol));
                let home = homes.next().ok_or(LoweringError::Unsupported(
                    "primitive store lost its structural local destination",
                ))?;
                // A projected scalar store writes the captured backing, not
                // the local descriptor. Retain its actual exclusive access;
                // whole-home replacement still requires ownership.
                let projected = !home.argument.path.is_empty() || !path.is_empty();
                let writable = match home.argument.access {
                    StructuralAccess::Owned => true,
                    StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow => {
                        projected
                    }
                    StructuralAccess::SharedBorrow => false,
                };
                if homes.next().is_some() || !writable {
                    return unsupported("primitive store local has no unique writable home");
                }
                let source = home.argument.clone();
                let declared = home.structural_type;
                let (steps, scalar_type) =
                    crate::emission::primitive_store::lower_destination_path(
                        identities,
                        declared,
                        path,
                        self.structural_types.declarations(),
                    )?;
                // A captured alias contributes its existing static prefix before
                // this assignment's complete checked projection.
                let mut composed = source
                    .path
                    .iter()
                    .cloned()
                    .map(crate::emission::runtime_elements::ProjectionStep::Static)
                    .collect::<Vec<_>>();
                composed.extend(steps);
                let root = crate::emission::store_destination::local_store_place(
                    *symbol,
                    &self.evaluation.structural_locals,
                    self.evaluation.structural_root_parameters(),
                    self.operations,
                )?;
                crate::emission::primitive_store::Destination {
                    logical_receiving: None,
                    qualifications: root.qualifications.to_vec(),
                    projected_qualifications: root.projected_qualifications.to_vec(),
                    place: source.place,
                    root_type: root.structural_type,
                    path: composed,
                    scalar_type,
                }
            }
        };
        let (_, source) = self.assignment_source(store.destination.root)?;
        if let Some(captured) =
            self.windows
                .captured_assignment_place(identities, statement_index, &source, path)?
        {
            let logical = self.receiving_place(captured.original_source, captured.original_path)?;
            let physical = self.receiving_place(captured.source, captured.path)?;
            if physical.root_type != captured.root_type {
                return unsupported("captured primitive destination changed its root declaration");
            }
            destination.logical_receiving = Some(logical);
            destination.place = captured.source;
            destination.root_type = physical.root_type;
            destination.qualifications = physical.qualifications;
            destination.projected_qualifications = physical.projected_qualifications;
            destination.path = physical
                .path
                .into_iter()
                .map(crate::emission::runtime_elements::ProjectionStep::Static)
                .collect();
        }
        let preserves_byte_memberships = matches!(
            store.destination.leaf,
            CheckedStoreLeaf::Primitive {
                preserves_byte_memberships: true,
                ..
            }
        );
        let kind = crate::emission::primitive_store::emit_assignment(
            identities,
            self.checked,
            self.machine,
            self.state,
            statement_index,
            destination,
            self.structural_types.declarations(),
            preserves_byte_memberships,
            match &store.destination.leaf {
                CheckedStoreLeaf::Primitive {
                    receiving_range, ..
                } => receiving_range.as_ref(),
                _ => unreachable!("primitive store dispatch"),
            },
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

    /// Replace byte contents or one descriptor slot. A whole descriptor slot
    /// is the declared home itself; the home, not the referenced bytes'
    /// shared access, supplies the authority.
    fn byte_sequence_store(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        store: &CheckedStorePlan,
    ) -> Result<(), LoweringError> {
        use crate::emission::structural_byte_sequence_store::StoreRoot;
        let Some(value) = store.value.bytes() else {
            return unsupported("byte sequence store holds no byte value");
        };
        let descriptor = matches!(store.destination.leaf, CheckedStoreLeaf::ByteDescriptor);
        let root = match store.destination.root {
            checked_trees::CheckedStoreRoot::Parameter { position } => StoreRoot::Parameter(
                store_parameter(&self.evaluation.structural_parameters, position)?,
            ),
            checked_trees::CheckedStoreRoot::Local { symbol }
                if descriptor && store.destination.path.is_empty() =>
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
                let home = crate::emission::store_destination::local_store_root(
                    symbol,
                    &self.evaluation.structural_locals,
                    self.evaluation.structural_root_parameters(),
                    self.operations,
                )?;
                let whole_owned_bytes = store.destination.path.is_empty()
                    && self
                        .structural_types
                        .declarations()
                        .iter()
                        .any(|declaration| {
                            declaration.id == home.selected_type
                                && declaration.shape.is_bounded_owned_byte_sequence()
                        });
                if !whole_owned_bytes {
                    home.require_unqualified()?;
                }
                StoreRoot::Local {
                    place: home.argument.place,
                    structural_type: home.structural_type,
                    multiplicity: home.multiplicity,
                    access: home.argument.access,
                }
            }
        };
        // Only a roster this body owns may gain the generated literal-view
        // carrier; the store below rejects a published roster without it.
        if let StructuralTypeRoster::Owned(types) = &mut self.structural_types {
            crate::emission::structural_byte_sequence_store::prepare_types(
                identities, value, types,
            )?;
        }
        if descriptor {
            let mut destination =
                crate::emission::structural_byte_sequence_store::prepare_descriptor_store(
                    identities,
                    &root,
                    &store.destination.path,
                    self.structural_types.declarations(),
                )?;
            let indexes = crate::emission::runtime_elements::evaluate_selectors(
                identities,
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
            let path = crate::emission::runtime_elements::complete_path(
                std::mem::take(&mut destination.steps),
                indexes,
                self.calls,
                self.operations,
            )?;
            self.observe_store_element_extent(
                destination.destination,
                destination.structural_type,
                &path,
            )?;
            let source = self.byte_descriptor_operand(identities, store.statement_index, value)?;
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
            identities,
            &store.destination.path,
            value,
            root,
            &self.evaluation.structural_parameters,
            &self.evaluation.structural_locals,
            &self.evaluation.view_locals,
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
    fn indexed_write(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        target: IndexedWriteTarget<'_>,
    ) -> Result<(), LoweringError> {
        let leaf = target.leaf()?;
        let mut destination = target.destination(
            identities,
            &self.evaluation.structural_parameters,
            self.structural_types.declarations(),
        )?;
        let (_, source) = self.assignment_source(target.store().destination.root)?;
        let captured = self.windows.captured_indexed_destination(
            leaf.statement_index(),
            &source,
            &target.store().destination.path,
        )?;
        let captured_index = if let Some(captured) = captured {
            destination.capture_descriptor(captured.source, captured.path)?;
            Some(captured.coordinate)
        } else {
            None
        };
        if captured_index.is_none() {
            let descriptor_selectors = crate::emission::runtime_elements::evaluate_selectors(
                identities,
                &destination.descriptor_steps,
                self.checked,
                self.machine,
                self.state,
                leaf.statement_index(),
                self.evaluation,
                self.source_value_count,
                self.values,
                self.next_value,
                self.next_block,
                self.next_edge,
                self.operations,
                self.calls,
            )?;
            let descriptor_path = crate::emission::runtime_elements::complete_path(
                std::mem::take(&mut destination.descriptor_steps),
                descriptor_selectors,
                self.calls,
                self.operations,
            )?;
            destination.set_descriptor_path(descriptor_path);
        }
        // Scalar positions are stable, but earlier operations may have
        // established owned or borrowed storage since the namespace began.
        // Join the current local homes before lowering either indexed operand.
        let bindings = &self
            .evaluation
            .scalar_bindings
            .clone()
            .with_structural_locals(&self.evaluation.structural_locals)
            .with_view_locals(&self.evaluation.view_locals)
            .with_place_observations(&self.evaluation.place_observations);
        // The plan's index is the checked expression bound at the target's
        // selector, or a `&mut collection[index]` view local's coordinate.
        let index = if captured_index.is_none() {
            Some(bindings.expression(
                identities,
                &self.checked.facts.proven_scalar_indexes,
                leaf.index(),
            )?)
        } else {
            None
        };
        // A completed call-result source must be joined before adding private
        // staging slots to the scalar namespace. Pure source expressions name
        // that same authored prefix, even when the RHS later creates a join.
        let lowered = if matches!(
            leaf.value(),
            checked_trees::CheckedScalarStoreValue::Computation(_)
        ) {
            None
        } else {
            Some(leaf.lower_value(identities, bindings, self.checked, self.state, self.values)?)
        };
        let decisions = destination.source_decisions(self.checked, leaf)?;
        let selector_position = self.values.len();
        let coordinate_type = terminal_scalar_type(checked_trees::types::PrimitiveType::U64)?;
        let coordinate = if let Some(coordinate) = captured_index {
            coordinate
        } else {
            crate::emission::indexed_write::emit_index_coordinate(
                index.as_ref().ok_or(LoweringError::Unsupported(
                    "indexed write has neither a captured coordinate nor a selector",
                ))?,
                decisions.index,
                self.values,
                self.next_value,
                &mut self.calls.next_obligation_identity,
                self.operations,
            )?
        };
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
            checked_trees::CheckedScalarStoreValue::Computation(_)
        ) {
            // The shared scalar evaluator carries the captured coordinate
            // through any private blocks in the RHS's evaluation schedule.
            crate::emission::indexed_write::IndexedWriteValue::Established(
                self.evaluation.field_assignment_value(
                    identities,
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
    fn representation_view_store(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,

        store: &CheckedStorePlan,
        view: checked_trees::types::PrimitiveType,
    ) -> Result<(), LoweringError> {
        for kind in crate::emission::representation_view_store::emit(
            identities,
            store,
            view,
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

    /// Resolve the store's existing home, independently of how that home was
    /// established. Source checking owns its write authority; emission carries
    /// the retained root, type, index and predicate decisions into one write.
    fn byte_element_target<'s>(
        &self,
        store: &'s CheckedStorePlan,
    ) -> Result<IndexedWriteTarget<'s>, LoweringError> {
        if !store.destination.path.is_empty() {
            return match store.destination.root {
                checked_trees::CheckedStoreRoot::Parameter { .. } => {
                    Ok(IndexedWriteTarget::ByteField(store))
                }
                checked_trees::CheckedStoreRoot::Local { .. } => {
                    unsupported("a local byte field element has no store route")
                }
            };
        }
        let (place, structural_type) = match store.destination.root {
            checked_trees::CheckedStoreRoot::Parameter { position } => {
                let root = store_parameter(&self.evaluation.structural_parameters, position)?;
                (root.place, root.structural_type)
            }
            checked_trees::CheckedStoreRoot::Local { symbol } => {
                let root = crate::emission::store_destination::local_store_root(
                    symbol,
                    &self.evaluation.structural_locals,
                    self.evaluation.structural_root_parameters(),
                    self.operations,
                )?;
                (root.argument.place, root.structural_type)
            }
        };
        Ok(IndexedWriteTarget::Bytes {
            store,
            place,
            structural_type,
        })
    }

    /// A `&mut`-borrowed parameter names its view's place directly; a locally
    /// bound `&mut` view names the home place its establishment produced.
    fn view_element_target<'s>(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        store: &'s CheckedStorePlan,
    ) -> Result<IndexedWriteTarget<'s>, LoweringError> {
        let (place, structural_type) = match store.destination.root {
            checked_trees::CheckedStoreRoot::Parameter { position } => {
                let destination =
                    store_parameter(&self.evaluation.structural_parameters, position)?;
                if !matches!(
                    destination.access,
                    terminal_psi::mutating_access_pair_pattern!()
                ) || !matches!(
                    destination.multiplicity,
                    terminal_psi::nonlinear_multiplicity_pair_pattern!()
                ) {
                    return unsupported("element-view write parameter lost mutable custody");
                }
                let written =
                    self.view_element_write_region(identities, store, destination.structural_type)?;
                if !terminal_semantics::structural_memberships_disjoint_from_write(
                    &destination.qualifications,
                    &destination.projected_qualifications,
                    &written,
                ) {
                    return unsupported(
                        "element-view scalar write overlaps qualifications it does not restore",
                    );
                }
                (destination.place, destination.structural_type)
            }
            checked_trees::CheckedStoreRoot::Local { symbol } => {
                let root = crate::emission::store_destination::local_store_root(
                    symbol,
                    &self.evaluation.structural_locals,
                    self.evaluation.structural_root_parameters(),
                    self.operations,
                )?;
                let written =
                    self.view_element_write_region(identities, store, root.structural_type)?;
                root.require_disjoint_from_write(&written)?;
                (root.argument.place, root.structural_type)
            }
        };
        Ok(IndexedWriteTarget::ElementView {
            store,
            place,
            structural_type,
        })
    }

    /// A view's dynamic selector may choose any element. This is a
    /// conservative footprint for admission only, never an executable path;
    /// the indexed writer separately emits its real selector and bound.
    fn view_element_write_region(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        store: &CheckedStorePlan,
        root: StructuralTypeId,
    ) -> Result<Vec<terminal_psi::StructuralPathSegment>, LoweringError> {
        use crate::expression_preparation::bindings::structural_paths::{
            PreparedStructuralPathSegment, prepare_element_view_path,
        };
        let CheckedStoreLeaf::ViewElement { element_path, .. } = &store.destination.leaf else {
            return unsupported("element-view write has no element footprint");
        };
        let selected = prepare_element_view_path(
            identities,
            &store.destination.path,
            root,
            self.structural_types.declarations(),
        )?;
        // Dynamic descriptor selections may name any resident slot. Retain
        // the complete original owner coordinates for membership admission;
        // these universal steps never enter the executable store path.
        let mut written = Vec::new();
        for step in selected {
            match step {
                PreparedStructuralPathSegment::Static(step) => written.push(step),
                PreparedStructuralPathSegment::ViewExtent { .. } => {}
                PreparedStructuralPathSegment::RuntimeIndex(_)
                | PreparedStructuralPathSegment::ComputedIndex(_)
                | PreparedStructuralPathSegment::CapturedIndex(_) => {
                    written.push(terminal_psi::StructuralPathSegment::AllElements);
                }
            }
        }
        written.push(terminal_psi::StructuralPathSegment::AllElements);
        written.extend(lower_structural_path(identities, element_path)?);
        Ok(written)
    }

    /// Field and element stores both retain a recast loan's original backing.
    /// This prepares selectors only; callers evaluate their ordinary RHS next.
    fn prepare_loaned_byte_store(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,

        statement_index: u32,
        loan: &crate::scalar_lowering::computations::fields::LoanedStore,
    ) -> Result<crate::emission::representation_view_store::ByteStoreDestination, LoweringError>
    {
        use crate::scalar_lowering::computations::fields::LoanedSource;
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
            LoanedSource::Local(symbol) => self.local_home(symbol)?,
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
            identities,
            place,
            root_type,
            &loan.path,
            captured_index,
            loan.decided_byte_view_store,
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
                identities,
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

    /// Replace one declared scalar field. A captured loan local rejoins its
    /// original backing first: a byte overlay writes the field's footprint
    /// bytes, any other loan the rejoined field below the loan's root.
    fn scalar_field_store(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        store: &CheckedStorePlan,
        primitive_type: checked_trees::types::PrimitiveType,
        range_decision: arena::Handle<checked_trees::CheckedIntegerAssignmentDecision>,
    ) -> Result<(), LoweringError> {
        let Some((_, value)) = store.value.scalar() else {
            return unsupported("scalar field store holds no scalar value");
        };
        let Some((mut carrier_path, field_identity)) = store.destination.field() else {
            return unsupported("scalar field store names no field");
        };
        let loan = crate::scalar_lowering::computations::fields::loaned_store(
            identities,
            self.checked,
            self.machine,
            self.state,
            store.destination.root,
            carrier_path,
            field_identity,
            primitive_type,
            self.structural_types.declarations(),
        )?;
        let mut root = store.destination.root;
        let rejoined;
        if let Some(loan) = loan {
            use crate::scalar_lowering::computations::fields::LoanedSource;
            root = match loan.source {
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
                let destination =
                    self.prepare_loaned_byte_store(identities, store.statement_index, &loan)?;
                let value = self.scalar_store_value(
                    identities,
                    store.statement_index,
                    value,
                    primitive_type,
                    field_identity,
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
            rejoined = loan.path;
            carrier_path = &rejoined;
        }
        // A parameter destination roots the store at its borrowed place; an
        // owned structural local roots it at the local's live home place,
        // resolved through the structural-local roster.
        let mut destination_memberships;
        let (mut destination_place, mut destination_type, mut lowered) = match root {
            checked_trees::CheckedStoreRoot::Parameter { position } => {
                let destination =
                    store_parameter(&self.evaluation.structural_parameters, position)?;
                destination_memberships = Some((
                    destination.qualifications.clone(),
                    destination.projected_qualifications.clone(),
                ));
                let lowered =
                    crate::emission::structural_scalar_store::lower_structural_scalar_store_place(
                        identities,
                        primitive_type,
                        carrier_path,
                        field_identity,
                        destination,
                        self.structural_types.declarations(),
                    )?;
                (destination.place, destination.structural_type, lowered)
            }
            checked_trees::CheckedStoreRoot::Local { symbol } => {
                let root = crate::emission::store_destination::local_store_place(
                    symbol,
                    &self.evaluation.structural_locals,
                    self.evaluation.structural_root_parameters(),
                    self.operations,
                )?;
                destination_memberships = Some((
                    root.qualifications.to_vec(),
                    root.projected_qualifications.to_vec(),
                ));
                let mut lowered =
                    crate::emission::structural_scalar_store::lower_structural_scalar_store_local(
                        identities,
                        primitive_type,
                        carrier_path,
                        field_identity,
                        root.selected_type,
                        self.structural_types.declarations(),
                    )?;
                // A projected local retains its original storage, including
                // array selection and any descriptor crossing. Compose that
                // captured prefix before the assignment's checked field path.
                let mut steps = root
                    .argument
                    .path
                    .iter()
                    .cloned()
                    .map(crate::emission::runtime_elements::ProjectionStep::Static)
                    .collect::<Vec<_>>();
                steps.append(&mut lowered.steps);
                lowered.steps = steps;
                (root.argument.place, root.structural_type, lowered)
            }
        };
        lowered.bind_range_decision(self.checked, primitive_type, range_decision)?;
        let (_, source) = self.assignment_source(store.destination.root)?;
        if let Some(captured) = self.windows.captured_assignment_place(
            identities,
            store.statement_index,
            &source,
            &store.destination.path,
        )? {
            let logical = self.receiving_place(captured.original_source, captured.original_path)?;
            if !terminal_semantics::structural_memberships_disjoint_from_write(
                &logical.qualifications,
                &logical.projected_qualifications,
                &logical.path,
            ) {
                return unsupported(
                    "captured scalar store overlaps logical qualification evidence",
                );
            }
            let mut physical = self.receiving_place(captured.source, captured.path)?;
            let expected_field = terminal_psi::StructuralPathSegment::Field(
                identities.intern(&field_identity.spelling()),
            );
            if logical.path.last() != Some(&expected_field)
                || physical.path.pop() != Some(expected_field)
                || physical.root_type != captured.root_type
            {
                return unsupported("captured scalar field lost its exact declaration projection");
            }
            destination_place = captured.source;
            destination_type = physical.root_type;
            destination_memberships =
                Some((physical.qualifications, physical.projected_qualifications));
            lowered.steps = physical
                .path
                .into_iter()
                .map(crate::emission::runtime_elements::ProjectionStep::Static)
                .collect();
        }
        // The carrier's runtime elements are evaluated before the value.
        let indexes = crate::emission::runtime_elements::evaluate_selectors(
            identities,
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
        let value = self.scalar_store_value(
            identities,
            store.statement_index,
            value,
            primitive_type,
            field_identity,
        )?;
        let path = crate::emission::runtime_elements::complete_path(
            std::mem::take(&mut lowered.steps),
            indexes,
            self.calls,
            self.operations,
        )?;
        if let Some((whole, projected)) = destination_memberships {
            let field = terminal_semantics::projected_field_declaration(
                self.structural_types.declarations().iter(),
                destination_type,
                &path,
                lowered.field,
            )
            .ok_or(LoweringError::Unsupported(
                "scalar store has no field declaration",
            ))?;
            let mut written = path.clone();
            written.push(terminal_psi::StructuralPathSegment::Field(field.identity));
            if !terminal_semantics::structural_memberships_disjoint_from_write(
                &whole, &projected, &written,
            ) {
                return unsupported("scalar store overlaps qualification evidence");
            }
        }
        self.observe_store_element_extent(destination_place, destination_type, &path)?;
        let kind = lowered.into_operation(
            destination_place,
            path,
            value.id,
            self.calls,
            self.operations,
        )?;
        self.push_unit(kind);
        Ok(())
    }

    /// Resolve a complete receiving contract at the retained place. A type
    /// alone cannot reinterpret memberships expressed relative to another root.
    fn receiving_place(
        &self,
        place: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
    ) -> Result<crate::emission::primitive_store::ReceivingPlace, LoweringError> {
        let mut declarations = self
            .evaluation
            .structural_root_parameters()
            .filter(|parameter| parameter.place == place)
            .map(|parameter| {
                (
                    parameter.structural_type,
                    &parameter.qualifications,
                    &parameter.projected_qualifications,
                )
            })
            .chain(self.operations.operations.iter().filter_map(|operation| {
                operation
                    .result
                    .structural()
                    .filter(|result| result.place == place)
                    .map(|result| {
                        (
                            result.structural_type,
                            &result.qualifications,
                            &result.projected_qualifications,
                        )
                    })
            }));
        let selected = declarations.next();
        if declarations.next().is_some() {
            return unsupported("store receiving root declaration is ambiguous");
        }
        let (root_type, whole, projected) = if let Some(declaration) = selected {
            declaration
        } else {
            // Bindings can repeat the same operation home. Only an absent
            // authoritative declaration consults these views; every view must
            // agree on the complete contract, rather than mint another owner.
            let mut bindings =
                self.operations
                    .structural_values
                    .iter()
                    .filter_map(|(_, binding)| {
                        binding
                            .operation_result()
                            .ok()
                            .filter(|result| result.place == place)
                            .map(|result| {
                                (
                                    result.structural_type,
                                    &result.qualifications,
                                    &result.projected_qualifications,
                                )
                            })
                    });
            let Some(first) = bindings.next() else {
                return unsupported("store lost its retained receiving root declaration");
            };
            if bindings.any(|binding| binding != first) {
                return unsupported("store receiving binding contracts disagree");
            }
            first
        };
        Ok(crate::emission::primitive_store::ReceivingPlace {
            root_type,
            qualifications: whole.clone(),
            projected_qualifications: projected.clone(),
            path,
        })
    }

    /// A descriptor's live extent owns runtime bounds through either a scalar
    /// field store or a stored-view replacement.
    fn observe_store_element_extent(
        &mut self,
        destination: semantic_vocabulary::PlaceId,
        structural_type: StructuralTypeId,
        path: &[terminal_psi::StructuralPathSegment],
    ) -> Result<(), LoweringError> {
        crate::emission::runtime_elements::observe_element_extent(
            destination,
            structural_type,
            path,
            self.structural_types.declarations(),
            self.next_value,
            self.operations,
        )
    }

    /// A scalar field's value, completed in the field's declared carrier.
    fn scalar_store_value(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        statement_index: u32,
        value: &CheckedScalarStoreValue,
        primitive_type: checked_trees::types::PrimitiveType,
        field_identity: &checked_trees::CheckedMemberIdentity,
    ) -> Result<ValueDeclaration, LoweringError> {
        self.evaluation.field_assignment_value(
            identities,
            self.checked,
            self.machine,
            self.state,
            statement_index,
            value,
            primitive_type,
            Some(field_identity),
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
    fn window_destination(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        destination: checked_trees::CheckedStoreRoot,
        path: &[checked_trees::CheckedUnitStructuralPathSegment],
    ) -> Result<
        (
            StructuralTypeId,
            checked_trees::CheckedUnitStructuralArgumentSourcePlan,
        ),
        LoweringError,
    > {
        let selected = self.assignment_source(destination)?;
        if let checked_trees::CheckedStoreRoot::Local { symbol } = destination {
            let root = crate::emission::store_destination::local_store_place(
                symbol,
                &self.evaluation.structural_locals,
                self.evaluation.structural_root_parameters(),
                self.operations,
            )?;
            let (steps, leaf, _) = crate::emission::structural_scalar_store::lower_window_path(
                identities,
                root.selected_type,
                path,
                self.structural_types.declarations(),
            )?;
            let mut written = root.argument.path.clone();
            written.extend(crate::emission::runtime_elements::static_segments(steps)?);
            if let crate::emission::structural_scalar_store::WindowLeaf::Field(field) = leaf {
                written.push(terminal_psi::StructuralPathSegment::Field(field.identity));
            }
            root.require_disjoint_from_write(&written)?;
        }
        Ok(selected)
    }

    /// Resolve the authored destination namespace for captured coordinates.
    /// This grants no write permission: primitive, indexed and field writers
    /// retain their complete typed membership and custody checks separately.
    fn assignment_source(
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
                let authored = store_parameter(&self.evaluation.structural_parameters, position)?;
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
                // A projected local names its selected subtree; the window
                // resolver composes its captured prefix onto the home.
                let home = crate::emission::store_destination::local_store_place(
                    symbol,
                    &self.evaluation.structural_locals,
                    self.evaluation.structural_root_parameters(),
                    self.operations,
                )?;
                // Address capture retains predicates; each final store proves
                // its exact logical and physical receiving contracts separately.
                Ok((
                    home.selected_type,
                    CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
                ))
            }
        }
    }

    /// Replace one whole sum field. The stored value becomes a fresh owned
    /// place — a leaf copy of a whole place, or the case literal established
    /// in place — that vacates the authored field through the window ledger
    /// and restores the hole, exactly as any whole-place repair does. The
    /// moved-out leaf is unrestricted by the plan's own contract, so no
    /// custody action retires it.
    fn sum_field_store(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        store: &CheckedStorePlan,
    ) -> Result<(), LoweringError> {
        let Some((carrier_path, field_identity)) = store.destination.field() else {
            return unsupported("sum field store names no field");
        };
        // A parameter destination roots the window at its borrowed place; an
        // owned structural local roots it at the local's live home place.
        // The window pair below writes through either root identically.
        let (root_type, source) =
            self.window_destination(identities, store.destination.root, &store.destination.path)?;
        let (_, field) = crate::emission::structural_scalar_store::lower_structural_field_path(
            identities,
            root_type,
            carrier_path,
            field_identity,
            self.structural_types.declarations(),
        )?;
        let Some(hole_type) = field.field_type.as_structural() else {
            return unsupported("sum field store names a non-structural field");
        };
        let (replacement, type_identity) = match &store.value {
            CheckedStoreValue::Place(value) => (
                self.copied_sum_value(identities, value, hole_type)?,
                value.type_identity.clone(),
            ),
            CheckedStoreValue::CaseLiteral(literal) => (
                self.established_case_literal(
                    identities,
                    store.statement_index,
                    literal,
                    hole_type,
                )?,
                literal.type_identity.clone(),
            ),
            CheckedStoreValue::Scalar { .. } | CheckedStoreValue::Bytes(_) => {
                return unsupported("sum field store holds no sum value");
            }
        };
        let hole = window_place(source, &store.destination.path, type_identity);
        require_parameter_window_root(&self.parameters, &hole, self.evaluation)?;
        let displacement = self.windows.captured_displacement_expression(
            store.statement_index,
            &hole.source,
            &hole.path,
        );
        let moved = self.emit_window_move(
            identities,
            store.statement_index,
            &hole,
            &checked_trees::CheckedUnitStructuralResultBindingPlan {
                statement_index: store.statement_index,
                binding_ordinal: 0,
                type_identity,
                multiplicity: language_semantics::Multiplicity::Unrestricted,
                local: Default::default(),
            },
            displacement,
        )?;
        // The moved-out leaf is an internal temporary, not an authored
        // result — declare it as a private temporary so the verifier finds
        // its producing operation.
        let move_id =
            last_operation_id(self.operations, "sum field store move emitted no operation")?;
        self.private_places
            .push(operation_result_declaration(moved, move_id, hole_type));
        self.emit_window_store(
            identities,
            store.statement_index,
            &hole,
            Some(moved),
            BorrowedWindowRepairValue {
                place: replacement,
                structural_type: hole_type,
            },
        )
    }

    /// Leaf-copy the whole place a sum field store names into a fresh owned
    /// place. The plan supplies a parameter projection an owned or shared
    /// place still holds; the copy reads the source where it lives and moves
    /// nothing.
    fn copied_sum_value(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,

        value: &checked_trees::CheckedUnitStructuralArgumentPlan,
        hole_type: StructuralTypeId,
    ) -> Result<PlaceId, LoweringError> {
        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
            parameter_index: value_index,
        } = value.source
        else {
            return unsupported("structural case field store value is not a parameter");
        };
        if !value.path.iter().all(|segment| {
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
        let (copied_place, copy) = allocate_structural_result(
            self.operations,
            self.next_place,
            hole_type,
            terminal_psi::StructuralMultiplicity::Unrestricted,
            Vec::new(),
            OperationKind::StructuralLeafCopy {
                source: source_place,
                path: lower_structural_path(identities, &value.path)?,
            },
        )?;
        self.private_places
            .push(operation_result_declaration(copied_place, copy, hole_type));
        Ok(copied_place)
    }

    /// Establish an authored case literal into a fresh owned place. Every
    /// scalar operand evaluates in the literal's `StructuralValueField`
    /// namespace and an unnamed common field takes its declared zero.
    fn established_case_literal(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        statement_index: u32,
        literal: &checked_trees::CheckedCaseLiteralValuePlan,
        hole_type: StructuralTypeId,
    ) -> Result<PlaceId, LoweringError> {
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
        let case = terminal_psi::structural_case_by_identity(
            &cases,
            identities.intern(&literal.case_identity.spelling()),
        )
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
        // Case construction lowers against the body's scalar namespace, as
        // ordinary field stores do, even when this case has no scalar payload.
        let bindings = self.evaluation.scalar_bindings.clone();
        let mut initialized = Vec::new();
        for field_plan in &literal.fields {
            let (position, target) = shape_fields
                .iter()
                .enumerate()
                .find(|(_, candidate)| {
                    candidate.identity == identities.intern(&field_plan.field_identity.spelling())
                })
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
                identities,
                self.checked,
                self.state,
                statement_index,
                CheckedScalarExpressionRole::StructuralValueField {
                    expression: literal.literal,
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
            push_initialized_scalar_field(
                &mut initialized,
                position,
                target,
                value,
                &mut self.calls.next_obligation_identity,
            )?;
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
            push_initialized_scalar_field(
                &mut initialized,
                position,
                target,
                zero,
                &mut self.calls.next_obligation_identity,
            )?;
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
        Ok(copied_place)
    }
}

/// The lowered declaration of the authored parameter a store root names.
/// `structural_parameters` pairs each checked plan's authored position with
/// the declaration that position lowered into; a declaration's own `position`
/// can be dense, so it never identifies the root.
fn store_parameter(
    structural_parameters: &[(u32, StructuralParameterDeclaration)],
    position: u32,
) -> Result<&StructuralParameterDeclaration, LoweringError> {
    structural_parameters
        .iter()
        .find(|(authored, _)| *authored == position)
        .map(|(_, parameter)| parameter)
        .ok_or(LoweringError::Unsupported(
            "store names an unknown authored parameter",
        ))
}

/// Record one scalar field initializer, requesting the bounded-range
/// obligation the verifier reconstructs against the final SSA initializer.
fn push_initialized_scalar_field(
    initialized: &mut Vec<(usize, terminal_psi::RecordFieldInitializer)>,
    position: usize,
    target: &terminal_psi::StructuralFieldDeclaration,
    value: semantic_vocabulary::ValueId,
    next_obligation_identity: &mut u64,
) -> Result<(), LoweringError> {
    let range_obligation = if terminal_semantics::scalar_field_requires_evidence(&target.field_type)
    {
        Some(crate::terminal_identities::obligation_id(allocate_dense(
            next_obligation_identity,
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
    Ok(())
}
