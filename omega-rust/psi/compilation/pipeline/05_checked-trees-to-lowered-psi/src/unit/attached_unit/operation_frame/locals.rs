//! Local establishment: scalar locals and arrays, owned byte sequences,
//! view subslices, structural locals and their stores, owned structural
//! values and rebinds, and the continuation cleanup that retires them. Each
//! local binds in the body's dense value namespace beside its authored
//! source position; a composed state instead registers the place in the
//! operation buffer and the machine-wide catalog.

use super::super::view_ranges::ViewRangeSite;
use super::{
    OperationFrame, StructuralResults, StructuralTypeRoster, allocate_structural_result,
    operation_result_declaration, require_parameter_window_root,
};
use crate::emission::borrowed_window::{BorrowedWindowRepairValue, WindowEvaluation};
use crate::emission::operation_emission::buffer::StructuralBinding;
use crate::expression_preparation::bindings::structural_paths::lower_structural_path;
use crate::unit::{
    CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan, LoweringError, Operation,
    OperationKind, OperationResult, PlaceId, StructuralAccess, StructuralDomainId,
    StructuralPlaceDeclaration, StructuralPlaceKind, ValueDeclaration, allocate_dense,
    lookup_type_id, place_id, terminal_scalar_type, unsupported,
};
use checked_trees::CheckedUnitStructuralArgumentPlan;

impl OperationFrame<'_, '_> {
    /// Bind one scalar local. A composed state names it in its own scalar
    /// namespace at its dense position; without that namespace the source
    /// bindings are the dense prefix of formals followed by locals in ordinal
    /// order, so the ordinal must name the next position.
    pub(super) fn establish_scalar_local(
        &mut self,
        result: &checked_trees::CheckedUnitScalarResultBindingPlan,
        value: &checked_trees::CheckedCallScalarArgument,
    ) -> Result<(), LoweringError> {
        // The source value is keyed by the local's authored ordinal, not by
        // the dense binding ordinal the plan assigns it.
        let role = if self.scalar_result == Some(result) {
            CheckedScalarExpressionRole::Return
        } else {
            CheckedScalarExpressionRole::LocalInitializer {
                binding_ordinal:
                    crate::expression_preparation::source_custody::source_local_ordinal(
                        self.checked,
                        self.state,
                        result.statement_index,
                    )?,
            }
        };
        let lowered = self.evaluation.source_value(
            self.checked,
            self.machine,
            self.state,
            result.statement_index,
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
        if lowered.scalar_type != terminal_scalar_type(result.primitive_type)? {
            return unsupported("Unit scalar expression local type disagrees with its binding");
        }
        let lowered = self.refine_scalar_local_value(result.statement_index, lowered)?;
        if let Some(bindings) = self.evaluation.scalar_bindings.as_mut() {
            bindings.append(
                checked_trees::CheckedScalarBindingDestination::Immutable,
                lowered.scalar_type,
                self.values.len(),
            )?;
        }
        self.values.push(lowered);
        Ok(())
    }

    /// Establish the declared predicate on a completed local value before
    /// publishing its source binding. The continuation forwards the existing
    /// namespace and this value together, regardless of its producing operation.
    pub(super) fn refine_scalar_local_value(
        &mut self,
        statement_index: u32,
        value: ValueDeclaration,
    ) -> Result<ValueDeclaration, LoweringError> {
        let (_, state) = crate::declarations::state_declaration(self.checked, self.state)?;
        let Some(checked_trees::statement::StatementNode::LocalData(local)) = self
            .checked
            .statement_table
            .statements(state.statement_nodes)
            .get(statement_index as usize)
        else {
            return Ok(value);
        };
        if local.type_is_inferred {
            return Ok(value);
        }
        let primitive_type = self
            .checked
            .primitive_type_reference(local.type_reference)
            .ok_or(LoweringError::Unsupported(
                "scalar local lost its primitive type",
            ))?;
        if terminal_scalar_type(primitive_type)? != value.scalar_type {
            return unsupported("completed scalar local disagrees with its declared type");
        }
        // Numeric predicates do not stand in for declared membership:
        // retain the catalog's independent rejection of unsupported theories.
        crate::expression_preparation::qualifications::type_atoms(
            self.checked,
            local.type_reference,
        )?;
        let predicates = validation::scalar_type_refinement_predicates(
            &self.checked.typed,
            local.type_reference,
            checked_trees::CheckedScalarExpression::Parameter {
                position: self.values.len(),
                primitive_type,
            },
        )
        .ok_or(LoweringError::Unsupported(
            "Unit local refinement has no predicate owner",
        ))?;
        if predicates.is_empty() {
            return Ok(value);
        }
        if local.is_mutable {
            return unsupported(
                "mutable scalar value refinements require invariant-window custody",
            );
        }
        let machine = self
            .calls
            .machine_ids
            .iter()
            .find(|(symbol, _)| *symbol == self.machine)
            .map(|(_, identity)| *identity)
            .ok_or(LoweringError::Unsupported(
                "local refinement lost its machine identity",
            ))?;
        self.values.push(value);
        self.evaluation.require_value_refinements(
            machine,
            predicates,
            self.values,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
            self.calls,
        )?;
        self.values.pop().ok_or(LoweringError::Unsupported(
            "local refinement continuation lost its completed value",
        ))
    }

    /// One `let`-bound primitive array literal: stage each element operand as
    /// a private value tail, emit the constructor's structural result into
    /// the body's own binding namespace, then release the tail. A
    /// statement-source construction additionally binds the declared local.
    pub(super) fn establish_scalar_array(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<(), LoweringError> {
        let CheckedUnitEffectOperationPlan::EstablishScalarArray {
            source,
            result,
            elements,
        } = operation
        else {
            return unsupported("array producer missing");
        };
        self.results
            .require_next(result, "array result binding is not dense")?;
        let leaf_start = self.values.len();
        for (ordinal, element) in elements.iter().enumerate() {
            let element_ordinal = u32::try_from(ordinal)
                .map_err(|_| LoweringError::Unsupported("array element ordinal exceeds u32"))?;
            let value = self.evaluation.source_value(
                self.checked,
                self.machine,
                self.state,
                result.statement_index,
                CheckedScalarExpressionRole::ArrayElement {
                    source: *source,
                    element_ordinal,
                },
                element,
                self.source_value_count,
                self.values,
                self.next_value,
                self.next_block,
                self.next_edge,
                self.operations,
                self.calls,
            )?;
            self.values.push(value);
        }
        let declaration = super::super::scalar_arrays::emit(
            result,
            &self.values[leaf_start..],
            self.type_ids,
            self.next_place,
            self.operations,
        )?;
        self.values.truncate(leaf_start);
        if let StructuralResults::StateGraph { places, state } = &mut self.results {
            places.push(declaration);
            let producer = self
                .operations
                .operations
                .last()
                .ok_or(LoweringError::Unsupported(
                    "array establishment emitted no operation",
                ))?;
            let OperationResult::Structural(produced) = &producer.result else {
                return unsupported("array establishment produced no structural value");
            };
            let produced = produced.clone();
            self.evaluation.establish_structural_result(
                self.checked,
                state.state,
                result,
                *source == checked_trees::CheckedArrayConstructionSource::Statement,
                produced,
                self.structural_types.declarations(),
                self.operations,
            )?;
        } else {
            self.results.push(declaration, false);
            // The dense route registers no result row, so it binds the
            // statement's local here; the state-graph row's own registration
            // already named it.
            if *source == checked_trees::CheckedArrayConstructionSource::Statement {
                super::super::structural_values::bind_local(
                    self.checked,
                    self.state,
                    operation,
                    declaration,
                    self.structural_types.declarations(),
                    &mut self.evaluation.structural_locals,
                )?;
            }
        }
        Ok(())
    }

    /// A window move evaluated through this frame: the statement's scalar
    /// evaluation surface and the frame's namespaces and counters supply the
    /// ledger's parameters.
    pub(super) fn emit_window_move(
        &mut self,
        statement_index: u32,
        moved: &CheckedUnitStructuralArgumentPlan,
        result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
    ) -> Result<PlaceId, LoweringError> {
        let mut evaluation = WindowEvaluation {
            checked: self.checked,
            machine: self.machine,
            state: self.state,
            statement_index,
            evaluation: &mut *self.evaluation,
            source_value_count: self.source_value_count,
            calls: &mut *self.calls,
        };
        self.windows.emit_move(
            moved,
            result,
            self.parameters,
            self.structural_types.declarations(),
            self.type_ids,
            &mut evaluation,
            self.values,
            self.next_place,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
        )
    }

    /// A window store evaluated through this frame, pairing the move.
    pub(super) fn emit_window_store(
        &mut self,
        statement_index: u32,
        destination: &CheckedUnitStructuralArgumentPlan,
        value: BorrowedWindowRepairValue,
    ) -> Result<(), LoweringError> {
        let mut evaluation = WindowEvaluation {
            checked: self.checked,
            machine: self.machine,
            state: self.state,
            statement_index,
            evaluation: &mut *self.evaluation,
            source_value_count: self.source_value_count,
            calls: &mut *self.calls,
        };
        self.windows.emit_store(
            destination,
            value,
            self.parameters,
            self.structural_types.declarations(),
            self.type_ids,
            &mut evaluation,
            self.values,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
        )
    }

    /// Move one whole structural field out of borrowed storage into the
    /// authored result binding. The opened window stays in the ledger until
    /// the matching `StoreStructuralField`; each route requires the ledger
    /// closed before its sequence's exits.
    pub(super) fn move_structural_field(
        &mut self,
        result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
        source: &CheckedUnitStructuralArgumentPlan,
    ) -> Result<(), LoweringError> {
        self.results
            .require_next(result, "borrowed-window move result binding is not dense")?;
        require_parameter_window_root(self.parameters, source, self.evaluation)?;
        let moved = self.emit_window_move(result.statement_index, source, result)?;
        let producer = self
            .operations
            .operations
            .last()
            .ok_or(LoweringError::Unsupported(
                "borrowed-window move emitted no operation",
            ))?;
        let OperationResult::Structural(produced) = &producer.result else {
            return unsupported("borrowed-window move established no structural value");
        };
        let declaration = StructuralPlaceDeclaration {
            id: moved,
            kind: StructuralPlaceKind::OperationResult {
                producer: producer.id,
                structural_type: produced.structural_type,
            },
        };
        let produced = produced.clone();
        match &mut self.results {
            StructuralResults::Dense(roster) => roster.push((Some(declaration), false)),
            // The composed namespace resolves later operands through the
            // operation buffer's registry, so the moved value registers there.
            StructuralResults::StateGraph { places, .. } => {
                places.push(declaration);
            }
        }
        // The result roster's layout does not decide whether this move binds
        // an authored local. Publish the already admitted detached value to
        // the same current-binding namespace in ordinary and composed bodies.
        self.evaluation.establish_structural_binding(
            self.checked,
            self.state,
            result,
            true,
            produced.into(),
            self.structural_types.declarations(),
            self.operations,
        )
    }

    /// Establish the authored carrier at its declaration, then register the
    /// actual backing in the local observation namespaces. A shared view
    /// borrows immutable literal storage; a bounded owner constructs fresh
    /// storage with its own live length and declared qualifications.
    pub(super) fn establish_byte_sequence(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<(), LoweringError> {
        let CheckedUnitEffectOperationPlan::EstablishByteSequence {
            result,
            value,
            qualifications,
        } = operation
        else {
            return unsupported("literal local has no producer");
        };
        self.results
            .require_next(result, "literal local binding is not dense")?;
        super::super::byte_locals::validate(self.checked, self.state, operation)?;
        let structural_type = lookup_type_id(self.type_ids, &result.type_identity)?;
        let mut qualifications = qualifications
            .iter()
            .map(|domain| {
                crate::terminal_identities::lookup_domain_id_for_carrier(
                    self.callees.domain_ids,
                    *domain,
                    structural_type,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        qualifications.sort();
        qualifications.dedup();
        if self
            .structural_types
            .declarations()
            .iter()
            .any(|declaration| {
                declaration.id == structural_type
                    && matches!(
                        declaration.shape,
                        terminal_psi::StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                        )
                    )
            })
        {
            return self.establish_owned_byte_sequence(
                result,
                value,
                structural_type,
                qualifications,
            );
        }
        if !self
            .structural_types
            .declarations()
            .iter()
            .any(|declaration| {
                declaration.id == structural_type
                    && matches!(
                        declaration.shape,
                        terminal_psi::StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                        )
                    )
            })
        {
            return unsupported("literal local carrier is not a borrowed byte view");
        }
        let bytes = value.literal_bytes().ok_or(LoweringError::Unsupported(
            "borrowed byte local has no literal storage",
        ))?;
        let destination = place_id(allocate_dense(self.next_place)?);
        let declaration_ordinal =
            crate::emission::next_byte_sequence_literal_ordinal(self.private_places)?;
        let declaration = StructuralPlaceDeclaration {
            id: destination,
            kind: StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal,
                structural_type,
            },
        };
        self.private_places.push(declaration);
        let id = self.operations.allocate();
        self.operations.push(Operation {
            id,
            static_reach_binding: None,
            suspension_crossing: None,
            result: OperationResult::Unit,
            // A literal view's bytes are image storage: the predicate
            // obligation belongs to the consuming store, as in
            // `emit_literal_view`, not to this establishment.
            kind: OperationKind::EstablishByteSequenceLiteral {
                destination,
                bytes: bytes.to_vec(),
                qualifications: Vec::new(),
            },
        });
        // The literal roster owns its declaration. Dense bindings index that
        // same place; a graph's registry retains its explicit literal kind.
        if let StructuralResults::Dense(roster) = &mut self.results {
            roster.push((Some(declaration), false));
        }
        self.evaluation.establish_structural_binding(
            self.checked,
            self.state,
            result,
            true,
            StructuralBinding::ByteSequenceLiteral {
                place: destination,
                structural_type,
                declaration_ordinal,
            },
            self.structural_types.declarations(),
            self.operations,
        )
    }

    /// Bind an owned bounded byte local to fresh backing copied from its
    /// initializer. The concatenation result is the local's own place: later
    /// loans, lengths and reads name it exactly as they name any owned result.
    pub(super) fn establish_owned_byte_sequence(
        &mut self,
        result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
        value: &checked_trees::CheckedByteSequenceValuePlan,
        structural_type: semantic_vocabulary::StructuralTypeId,
        qualifications: Vec<StructuralDomainId>,
    ) -> Result<(), LoweringError> {
        // Only a roster this body owns may gain the generated literal-view
        // carrier; a published roster must already hold it.
        if let StructuralTypeRoster::Owned(types) = &mut self.structural_types {
            crate::emission::structural_byte_sequence_store::prepare_types(value, types)?;
        }
        let (declaration, produced) =
            crate::emission::structural_byte_sequence_store::emit_owned_value(
                value,
                structural_type,
                qualifications,
                &self.evaluation.structural_parameters,
                &self.evaluation.structural_locals,
                self.structural_types.declarations(),
                self.private_places,
                self.next_place,
                self.next_value,
                &mut self.calls.next_obligation_identity,
                self.operations,
            )?;
        self.register_structural_result(declaration, result, produced)
    }

    /// Bind one immutable view local to an exclusive range. The range
    /// replays at its `LocalBinding` site and emits through the same replay
    /// and Terminal operation a call argument or an edge transfer uses. The
    /// published place joins the local namespace exactly as an `as_slice`
    /// view local's does, so later edges, calls, lengths and element reads
    /// resolve either kind of view local the same way.
    ///
    /// A range over array backing has no view to narrow yet. It first
    /// establishes a whole element view of that collection
    /// under a shared loan -- the same `EstablishElementView` an `as_slice`
    /// local emits -- into a private temporary, and narrows that. The
    /// verifier then relates the whole view's length to the array's declared
    /// extent and checks the range against it like any other subslice.
    pub(super) fn establish_view_subslice(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<(), LoweringError> {
        let CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, source } = operation
        else {
            return unsupported("view subslice binding has no producer");
        };
        self.results
            .require_next(result, "view subslice result binding is not dense")?;
        let binding_site =
            super::super::view_ranges::binding_site(self.checked, self.state, operation)?;
        let (expression, start, end) = match &source.source {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
                root: _,
                expression,
                start,
                end,
                storage_type_identity: _,
            } => (*expression, start, end),
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
                root: _,
                expression,
                start,
                end,
                storage_type_identity: _,
            } => (*expression, start, end),
            _ => return unsupported("view subslice binding lost its range source"),
        };

        // Endpoints read the body's scalar namespace, including the view
        // locals and parameters a `len` endpoint may observe.
        let bindings = self
            .evaluation
            .scalar_bindings
            .clone()
            .unwrap_or_else(|| {
                self.dense_assignment_namespace()
                    .with_structural_observations(self.structural_types.declarations())
            })
            .with_structural_locals(&self.evaluation.structural_locals)
            .with_view_locals(&self.evaluation.view_locals)
            .with_place_observations(&self.evaluation.place_observations);
        let destination = place_id(allocate_dense(self.next_place)?);
        let site = ViewRangeSite {
            state: self.state,
            statement: result.statement_index,
            site: binding_site,
            expression,
            retained: Some((start, end)),
        };
        let result_type = lookup_type_id(self.type_ids, &result.type_identity)?;
        let storage = super::super::view_ranges::storage(
            self.checked,
            self.state,
            result.statement_index,
            source,
            self.structural_types.declarations(),
        )?;
        let whole = storage
            .requires_establishment()
            .then(|| allocate_dense(self.next_place).map(place_id))
            .transpose()?;
        let (range_source, establishment) = storage.bind(
            self.checked,
            self.state,
            &bindings,
            whole,
            self.structural_types.declarations(),
        )?;
        if let Some(establishment) = establishment {
            let declaration = super::super::view_ranges::establish(
                &establishment,
                self.values,
                self.next_value,
                self.operations,
            );
            self.private_places.push(declaration);
        }
        let declaration = super::super::view_ranges::emit(
            self.checked,
            site,
            range_source,
            result_type,
            destination,
            self.evaluation,
            self.values.len(),
            self.values,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
            self.calls,
        )?;
        let produced = self
            .operations
            .operations
            .last()
            .and_then(|producer| producer.result.structural())
            .filter(|produced| produced.place == destination)
            .cloned()
            .ok_or(LoweringError::Unsupported(
                "view subslice established no structural value",
            ))?;
        self.register_structural_result(declaration, result, produced)
    }

    /// Register `declaration` on this route's results roster and bind
    /// `result` to `produced`. Every structural producer emits the same
    /// registration-establishment pair once it holds a declaration: a dense
    /// roster wraps each row with its discard-on-return flag while the
    /// state-graph roster takes the place bare.
    pub(super) fn register_structural_result(
        &mut self,
        declaration: StructuralPlaceDeclaration,
        result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
        produced: terminal_psi::StructuralOperationResult,
    ) -> Result<(), LoweringError> {
        match &mut self.results {
            StructuralResults::Dense(roster) => roster.push((Some(declaration), false)),
            StructuralResults::StateGraph { places, .. } => places.push(declaration),
        }
        self.evaluation.establish_structural_result(
            self.checked,
            self.state,
            result,
            true,
            produced,
            self.structural_types.declarations(),
            self.operations,
        )
    }

    /// Establish one declared structural local's home as an operation result
    /// place. Member stores and borrows rejoin the local through the binding
    /// the result registration publishes; the home itself has no contents
    /// until member operations establish them.
    pub(super) fn establish_structural_local(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<(), LoweringError> {
        let CheckedUnitEffectOperationPlan::EstablishStructuralLocal {
            result,
            qualifications,
            discard_result_on_return,
            ..
        } = operation
        else {
            return unsupported("structural local establishment has no result");
        };
        self.results
            .require_next(result, "structural local result binding is not dense")?;
        let structural_type = lookup_type_id(self.type_ids, &result.type_identity)?;
        let multiplicity = crate::terminal_identities::structural_multiplicity(result.multiplicity);
        // A descriptor home's declared byte domains, on its own view carrier.
        let mut qualifications = qualifications
            .iter()
            .map(|domain| {
                crate::terminal_identities::lookup_domain_id_for_carrier(
                    self.callees.domain_ids,
                    *domain,
                    structural_type,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        qualifications.sort();
        qualifications.dedup();
        let (home, producer) = allocate_structural_result(
            self.operations,
            self.next_place,
            structural_type,
            multiplicity,
            qualifications,
            OperationKind::EstablishStructuralLocal {},
        )?;
        self.results.push(
            operation_result_declaration(home, producer, structural_type),
            *discard_result_on_return,
        );
        let produced = self
            .operations
            .operations
            .last()
            .and_then(|produced| produced.result.structural().cloned())
            .ok_or(LoweringError::Unsupported(
                "structural local established no result",
            ))?;
        self.evaluation.establish_structural_result(
            self.checked,
            self.state,
            result,
            true,
            produced,
            self.structural_types.declarations(),
            self.operations,
        )
    }

    /// Store a whole owned result back into its open window. The ledger
    /// closes the hole; the repaired place reads through any selection that
    /// transported the result.
    pub(super) fn store_structural_field(
        &mut self,
        statement_index: u32,
        destination: &CheckedUnitStructuralArgumentPlan,
        value: &CheckedUnitStructuralArgumentPlan,
    ) -> Result<(), LoweringError> {
        let repair = self.owned_structural_value(value)?;
        require_parameter_window_root(self.parameters, destination, self.evaluation)?;
        self.emit_window_store(statement_index, destination, repair)
    }

    /// Resolve the completed owned occurrence independently of whether its
    /// consumer binds a local or repairs another owner's borrowed storage.
    pub(super) fn owned_structural_value(
        &self,
        value: &CheckedUnitStructuralArgumentPlan,
    ) -> Result<BorrowedWindowRepairValue, LoweringError> {
        if !value.path.is_empty() || value.access != checked_trees::CheckedStructuralAccess::Owned {
            return unsupported("replacement value is not a whole owned result");
        }
        let repair = match value.source {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal,
            } => {
                let repair = self
                    .results
                    .repair_value(binding_ordinal, self.operations)?;
                BorrowedWindowRepairValue {
                    place: self.evaluation.current_structural_place(repair.place),
                    structural_type: repair.structural_type,
                }
            }
            // An owned parameter's own storage supplies the repair whole.
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index,
            } => {
                let parameter = self.parameters.get(parameter_index as usize);
                let Some(parameter) = parameter else {
                    return unsupported("borrowed-window repair value's parameter is not declared");
                };
                if parameter.access != terminal_psi::StructuralAccess::Owned {
                    return unsupported("borrowed-window repair value is not an owned parameter");
                }
                BorrowedWindowRepairValue {
                    place: parameter.place,
                    structural_type: parameter.structural_type,
                }
            }
            // An owned local's established home supplies the repair whole.
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
                let local = self
                    .evaluation
                    .structural_locals
                    .iter()
                    .find(|local| local.root == facts::PlaceRoot::Symbol(symbol));
                let Some(crate::expression_preparation::bindings::StructuralLocalBinding {
                    argument,
                    structural_type,
                    ..
                }) = local
                else {
                    return unsupported("borrowed-window repair value's local is not established");
                };
                if argument.access != StructuralAccess::Owned || !argument.path.is_empty() {
                    return unsupported("replacement value's local does not own the whole home");
                }
                BorrowedWindowRepairValue {
                    place: argument.place,
                    structural_type: *structural_type,
                }
            }
            _ => {
                return unsupported("borrowed-window repair value is not a whole owned result");
            }
        };
        if repair.structural_type != lookup_type_id(self.type_ids, &value.type_identity)? {
            return unsupported("replacement value changed its declared structural type");
        }
        Ok(repair)
    }

    /// Owned bindings can change value identity after the prior occurrence's
    /// ordered disposition. Reuse the completed producer's home; manufacturing
    /// a borrowed-storage window here would give an ordinary local a second
    /// owner's restoration debt and require a spurious extraction.
    pub(super) fn rebind_structural_local(
        &mut self,
        _statement_index: u32,
        destination: &CheckedUnitStructuralArgumentPlan,
        value: &CheckedUnitStructuralArgumentPlan,
    ) -> Result<(), LoweringError> {
        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } =
            destination.source
        else {
            return unsupported("owned rebind destination is not a local binding");
        };
        if !destination.path.is_empty()
            || destination.access != checked_trees::CheckedStructuralAccess::Owned
        {
            return unsupported("owned rebind destination is not its whole local");
        }
        let replacement = self.owned_structural_value(value)?;
        if replacement.structural_type != lookup_type_id(self.type_ids, &destination.type_identity)?
        {
            return unsupported("owned rebind changes the local's declared type");
        }
        let mut locals = self
            .evaluation
            .structural_locals
            .iter_mut()
            .filter(|local| local.root == facts::PlaceRoot::Symbol(symbol));
        let local = locals.next().ok_or(LoweringError::Unsupported(
            "owned rebind lost the destination local",
        ))?;
        if locals.next().is_some()
            || local.structural_type != replacement.structural_type
            || local.argument.access != StructuralAccess::Owned
            || !local.argument.path.is_empty()
        {
            return unsupported("owned rebind has ambiguous or incompatible destination custody");
        }
        local.argument.place = replacement.place;
        local.result_binding_ordinal = value.source_structural_result_binding_ordinal();
        self.evaluation
            .structural_value_owners
            .retain(|owner| owner.symbol != symbol || owner.value.place == replacement.place);
        if let Some(owner) = self
            .evaluation
            .structural_value_owners
            .iter_mut()
            .find(|owner| owner.value.place == replacement.place)
        {
            owner.symbol = symbol;
        }
        Ok(())
    }

    /// Commit ordered cleanup at its exact continuation while retaining scalar values.
    pub(super) fn continuation_cleanup(
        &mut self,
        cleanup_actions: &[checked_trees::CheckedNormalCleanupAction],
    ) -> Result<(), LoweringError> {
        let mut actions = Vec::new();
        for action in cleanup_actions {
            let (place, _) = self.evaluation.cleanup_storage(
                action.source(),
                self.parameters,
                |binding_ordinal| {
                    let place = self
                        .results
                        .continuation_place(binding_ordinal, self.operations)?;
                    let value = self
                        .results
                        .repair_value(binding_ordinal, self.operations)?;
                    Ok((place, value.structural_type))
                },
            )?;
            let path = lower_structural_path(action.path())?;
            let structural_type = lookup_type_id(self.type_ids, action.type_identity())?;
            actions.push(match action {
                checked_trees::CheckedNormalCleanupAction::Discard(_) if path.is_empty() => {
                    terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place)
                }
                checked_trees::CheckedNormalCleanupAction::Discard(_) => {
                    terminal_psi::TerminalAffineCleanupAction::DiscardResidual(
                        terminal_psi::StructuralAffineDiscard {
                            place,
                            path,
                            structural_type,
                        },
                    )
                }
                checked_trees::CheckedNormalCleanupAction::InvokeNominal(cleanup) => {
                    terminal_psi::TerminalAffineCleanupAction::InvokeNominal(
                        crate::unit::unit_cleanup::lower_nominal_cleanup_action(
                            self.checked,
                            cleanup,
                            place,
                            structural_type,
                            path,
                            self.type_ids,
                            self.calls.machine_ids,
                            self.calls.cleanup_receivers,
                            self.calls.requirement_counts,
                            &mut self.calls.next_obligation_identity,
                        )?,
                    )
                }
            });
        }
        self.evaluation
            .continue_values(
                actions,
                self.values,
                self.next_value,
                self.next_block,
                self.next_edge,
                self.operations,
            )
            .map(|_| ())
    }
}
