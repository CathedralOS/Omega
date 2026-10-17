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
use crate::unit::{
    CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan, LoweringError, Operation,
    OperationKind, OperationResult, PlaceId, StructuralAccess, StructuralDomainId,
    StructuralPlaceDeclaration, StructuralPlaceKind, ValueDeclaration, allocate_dense,
    lookup_type_id, place_id, terminal_scalar_type, unsupported,
};
use checked_trees::CheckedUnitStructuralArgumentPlan;

impl OperationFrame<'_, '_> {
    /// The one lowering of a local data binding: the bound value's
    /// representation selects which registration path runs.
    pub(super) fn establish_local(
        mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        operation: &CheckedUnitEffectOperationPlan,
        binding: &checked_trees::CheckedUnitLocalBindingPlan,
    ) -> Result<(), LoweringError> {
        match binding {
            checked_trees::CheckedUnitLocalBindingPlan::Scalar { result, value } => {
                self.establish_scalar_local(identities, result, value)
            }
            checked_trees::CheckedUnitLocalBindingPlan::ScalarArray { .. } => {
                self.establish_scalar_array(identities, operation)
            }
            checked_trees::CheckedUnitLocalBindingPlan::ByteSequence { .. } => {
                self.establish_byte_sequence(identities, operation)
            }
            checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { .. } => {
                self.establish_structural_value(identities, operation)
            }
            checked_trees::CheckedUnitLocalBindingPlan::Structural { .. } => {
                self.establish_structural_local(operation)
            }
            // Primitive and affine storage belong to the ordinary machine's
            // statement sequence, never an operation frame.
            checked_trees::CheckedUnitLocalBindingPlan::Primitive { .. }
            | checked_trees::CheckedUnitLocalBindingPlan::TrivialAffine { .. } => {
                unreachable!("not an operation-frame local binding")
            }
        }
    }

    /// Bind one scalar local in the same source namespace ordinary bodies and
    /// composed states share. Anonymous operation results occupy dense value
    /// slots but never shift this local's authored ordinal.
    pub(super) fn establish_scalar_local(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        result: &checked_trees::CheckedUnitScalarResultBindingPlan,
        value: &checked_trees::CheckedCallScalarArgument,
    ) -> Result<(), LoweringError> {
        // The source value is keyed by the local's authored ordinal, not by
        // the dense binding ordinal the plan assigns it.
        let role = if self.scalar_result == Some(result) {
            CheckedScalarExpressionRole::Return
        } else {
            CheckedScalarExpressionRole::LocalInitializer {
                binding_ordinal: self
                    .checked
                    .facts
                    .state_locals
                    .scalar_locals_before(self.state, result.statement_index),
            }
        };
        let lowered = self.evaluation.source_value(
            identities,
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
        let lowered =
            self.refine_scalar_local_value(identities, result.statement_index, lowered)?;
        self.evaluation
            .scalar_bindings
            .append(lowered.scalar_type, self.values.len());
        self.values.push(lowered);
        Ok(())
    }

    /// Establish the declared predicate on a completed local value before
    /// publishing its source binding. The continuation forwards the existing
    /// namespace and this value together, regardless of its producing operation.
    pub(super) fn refine_scalar_local_value(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        statement_index: u32,
        value: ValueDeclaration,
    ) -> Result<ValueDeclaration, LoweringError> {
        let Some(local) = self
            .checked
            .facts
            .state_locals
            .at_statement(self.state, statement_index)
            .copied()
        else {
            return Ok(value);
        };
        if local.type_is_inferred {
            return Ok(value);
        }
        let primitive_type = self
            .checked
            .primitive_type_reference(local.declared_type)
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
            local.declared_type,
        )?;
        let predicates = validation::scalar_type_refinement_predicates(
            &self.checked.typed,
            local.declared_type,
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
        let machine = crate::terminal_identities::lookup_machine_id_with(
            self.calls.machine_ids,
            self.machine,
            "local refinement lost its machine identity",
        )?;
        self.values.push(value);
        self.evaluation.require_value_refinements(
            identities,
            self.checked,
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
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<(), LoweringError> {
        let CheckedUnitEffectOperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::ScalarArray {
                qualifications,
                source,
                result,
                elements,
            },
        ) = operation
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
                identities,
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
        let structural_type = lookup_type_id(self.type_ids, &result.type_identity)?;
        let qualifications = qualifications
            .iter()
            .map(|domain| {
                crate::terminal_identities::lookup_domain_id_for_carrier(
                    self.callees.domain_ids,
                    *domain,
                    structural_type,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let declaration = super::super::scalar_arrays::emit(
            result,
            qualifications,
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
            let Some(produced) = producer.result.as_structural() else {
                return unsupported("array establishment produced no structural value");
            };
            let produced = produced.clone();
            self.evaluation.establish_structural_result(
                state.state,
                result,
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
                    operation,
                    declaration,
                    crate::terminal_identities::lookup_type_id(
                        self.type_ids,
                        &result.type_identity,
                    )?,
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
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        statement_index: u32,
        moved: &CheckedUnitStructuralArgumentPlan,
        result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Result<PlaceId, LoweringError> {
        // Extraction completes an owned value just like any other structural
        // producer. Reuse its checked establishment identities rather than
        // dropping contained claims because the source home was borrowed.
        // A displacement is not a new lexical establishment. Its old value
        // is governed by the selected replacement death instead.
        let owner = if result.local.is_named() {
            Some((
                facts::PlaceRoot::Symbol(result.local.symbol),
                result.local.declared_type,
            ))
        } else if expression.is_valid() {
            let reference = self.checked.facts.resolved_types.result(expression);
            if !reference.is_valid() {
                return unsupported("extracted value occurrence has no checked type");
            }
            Some((
                facts::PlaceRoot::Expression(expression),
                validation::assignment_value_type(&self.checked.typed, reference),
            ))
        } else {
            None
        };
        let prepared_claims = if let Some((owner, reference)) = owner {
            crate::emission::claim_bindings::prepare_establishment_claims(
                identities,
                self.checked,
                self.machine,
                self.state,
                statement_index as usize,
                owner,
                reference,
                lookup_type_id(self.type_ids, &result.type_identity)?,
                self.structural_types.declarations(),
            )?
        } else {
            Vec::new()
        };
        let result_claims = crate::emission::claim_bindings::emit_establishment_claims(
            &prepared_claims,
            &mut self.calls.claims,
        )?;
        let mut evaluation = WindowEvaluation {
            checked: self.checked,
            machine: self.machine,
            state: self.state,
            statement_index,
            evaluation: &mut *self.evaluation,
            source_value_count: self.source_value_count,
            calls: &mut *self.calls,
        };
        let place = self.windows.emit_move(
            identities,
            moved,
            expression,
            result,
            result_claims,
            &self.parameters,
            self.structural_types.declarations(),
            self.type_ids,
            &mut evaluation,
            self.values,
            self.next_place,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
        )?;
        crate::emission::claim_bindings::record_prepared_introductions(
            self.operations,
            place,
            &prepared_claims,
            &self.calls.claims,
            crate::emission::claim_bindings::ClaimEstablishmentKind::DetachedValue,
        )?;
        Ok(place)
    }

    /// A window store evaluated through this frame, pairing the move.
    pub(super) fn emit_window_store(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        statement_index: u32,
        destination: &CheckedUnitStructuralArgumentPlan,
        opening: Option<PlaceId>,
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
            identities,
            destination,
            opening,
            value,
            &self.parameters,
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
    /// the matching window repair store; each route requires the ledger
    /// closed before its sequence's exits.
    pub(super) fn move_structural_field(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
        source: &CheckedUnitStructuralArgumentPlan,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Result<(), LoweringError> {
        self.results
            .require_next(result, "borrowed-window move result binding is not dense")?;
        require_parameter_window_root(&self.parameters, source, self.evaluation)?;
        let moved = self.emit_window_move(
            identities,
            result.statement_index,
            source,
            result,
            expression,
        )?;
        let producer = self
            .operations
            .operations
            .last()
            .ok_or(LoweringError::Unsupported(
                "borrowed-window move emitted no operation",
            ))?;
        let Some(produced) = producer.result.as_structural() else {
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
            result,
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
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<(), LoweringError> {
        let CheckedUnitEffectOperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::ByteSequence {
                result,
                value,
                qualifications,
            },
        ) = operation
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
                    && declaration.shape.is_bounded_owned_byte_sequence()
            })
        {
            return self.establish_owned_byte_sequence(
                identities,
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
                    && declaration.shape.is_borrowed_view_byte_sequence()
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
            result,
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
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,

        result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
        value: &checked_trees::CheckedByteSequenceValuePlan,
        structural_type: semantic_vocabulary::StructuralTypeId,
        qualifications: Vec<StructuralDomainId>,
    ) -> Result<(), LoweringError> {
        // Only a roster this body owns may gain the generated literal-view
        // carrier; a published roster must already hold it.
        if let StructuralTypeRoster::Owned(types) = &mut self.structural_types {
            crate::emission::structural_byte_sequence_store::prepare_types(
                identities, value, types,
            )?;
        }
        let (declaration, produced) =
            crate::emission::structural_byte_sequence_store::emit_owned_value(
                identities,
                value,
                structural_type,
                qualifications,
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
        self.register_structural_result(declaration, result, produced)
    }

    /// Bind one immutable view local to an exclusive range. The range
    /// replays at its `LocalBinding` site and emits through the same replay
    /// and Terminal operation a call argument or an edge transfer uses. The
    /// published place joins the local namespace exactly as a whole-collection
    /// view local's does, so later edges, calls, lengths and element reads
    /// resolve either kind of view local the same way.
    ///
    /// A range over array backing has no view to narrow yet. It first
    /// establishes a whole element view of that collection
    /// under a shared loan -- the same `EstablishElementView` a
    /// whole-collection view local emits -- into a private temporary, and narrows that. The
    /// verifier then relates the whole view's length to the array's declared
    /// extent and checks the range against it like any other subslice.
    pub(super) fn establish_view_subslice(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<(), LoweringError> {
        let CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, source } = operation
        else {
            return unsupported("view subslice binding has no producer");
        };
        self.results
            .require_next(result, "view subslice result binding is not dense")?;
        let range = super::super::view_ranges::range(source).ok_or(LoweringError::Unsupported(
            "view subslice binding lost its range source",
        ))?;

        // Endpoints read the body's scalar namespace, including the view
        // locals and parameters a `len` endpoint may observe.
        let bindings = self
            .evaluation
            .scalar_bindings
            .clone()
            .with_structural_locals(&self.evaluation.structural_locals)
            .with_view_locals(&self.evaluation.view_locals)
            .with_place_observations(&self.evaluation.place_observations);
        let destination = place_id(allocate_dense(self.next_place)?);
        let site = ViewRangeSite {
            state: self.state,
            statement: result.statement_index,
            range,
        };
        let result_type = lookup_type_id(self.type_ids, &result.type_identity)?;
        let storage = super::super::view_ranges::storage(
            identities,
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
            identities,
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
            )?;
            self.private_places.push(declaration);
        }
        let declaration = super::super::view_ranges::emit(
            identities,
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
            self.state,
            result,
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
        let CheckedUnitEffectOperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::Structural {
                result,
                qualifications,
                discard_result_on_return,
                ..
            },
        ) = operation
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
            self.state,
            result,
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
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        statement_index: u32,
        destination: &CheckedUnitStructuralArgumentPlan,
        opening_binding: Option<u32>,
        value: &CheckedUnitStructuralArgumentPlan,
        multiplicity: language_semantics::Multiplicity,
    ) -> Result<(), LoweringError> {
        let mut root = value.clone();
        root.path.clear();
        let mut repair = self.owned_structural_value(&root)?;
        if !value.path.is_empty() {
            let path = crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
                identities, &value.path, repair.structural_type, self.structural_types.declarations(),
            )?;
            // Owning a descriptor never owns its referent. Owned operand
            // completion selects storage children, not borrowed byte windows.
            if path.iter().any(|segment| {
                matches!(
                    segment,
                    terminal_psi::StructuralPathSegment::Referent
                        | terminal_psi::StructuralPathSegment::FixedByteRange { .. }
                )
            }) {
                return unsupported("owned store operand crosses borrowed storage");
            }
            let selected = terminal_semantics::runtime_structural_path_tip(
                self.structural_types.declarations().iter(),
                repair.structural_type,
                &path,
            )
            .ok_or(LoweringError::Unsupported(
                "owned store operand has no declared subtree",
            ))?
            .id;
            if selected != lookup_type_id(self.type_ids, &value.type_identity)? {
                return unsupported("owned store operand changes its selected source type");
            }
            let source_contract = self
                .evaluation
                .structural_root_parameters()
                .chain(self.parameters.iter())
                .find(|parameter| parameter.place == repair.place)
                .map(|parameter| {
                    (
                        parameter.qualifications.clone(),
                        parameter.projected_qualifications.clone(),
                    )
                })
                .or_else(|| {
                    self.operations.iter().find_map(|operation| {
                        operation
                            .result
                            .structural()
                            .filter(|result| result.place == repair.place)
                            .map(|result| {
                                (
                                    result.qualifications.clone(),
                                    result.projected_qualifications.clone(),
                                )
                            })
                    })
                })
                .ok_or(LoweringError::Unsupported(
                    "owned store operand lost its producer contract",
                ))?;
            let (qualifications, projected_qualifications) =
                terminal_psi::StructuralPathQualification::select(
                    &source_contract.0,
                    &source_contract.1,
                    &path,
                );
            let declaration = terminal_psi::StructuralParameterDeclaration {
                place: repair.place,
                position: 0,
                structural_type: selected,
                multiplicity: crate::terminal_identities::structural_multiplicity(multiplicity),
                access: StructuralAccess::Owned,
                qualifications,
                projected_qualifications,
            };
            let mut arguments = vec![terminal_psi::StructuralArgument {
                place: repair.place,
                path,
                access: StructuralAccess::Owned,
            }];
            // A store consumes a completed value. Use the same argument edge
            // as a continuation: it transfers the selected custody and keeps
            // the parent's remaining children at their original home. No
            // borrowed extraction debt or replacement-specific copy exists.
            self.evaluation.capture_structural_arguments(
                identities,
                self.checked,
                &mut arguments,
                &[declaration],
                self.values,
                self.next_place,
                self.private_places,
                self.next_value,
                self.next_block,
                self.next_edge,
                self.operations,
            )?;
            let captured = self
                .evaluation
                .block_structural_parameters
                .last()
                .ok_or(LoweringError::Unsupported(
                    "owned store operand has no completed arrival",
                ))?
                .clone();
            repair.place = captured.place;
            repair.structural_type = captured.structural_type;
            self.parameters.to_mut().push(captured);
        }
        // The result roster keeps the extraction's identity even if its
        // detached value has already transferred or been disposed. This is
        // a debt reference, not a second ownership use of that value.
        let opening = opening_binding
            .map(|binding| {
                self.results
                    .repair_value(binding, self.operations)
                    .map(|value| value.place)
            })
            .transpose()?;
        require_parameter_window_root(&self.parameters, destination, self.evaluation)?;
        self.emit_window_store(identities, statement_index, destination, opening, repair)
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
                let local = crate::expression_preparation::bindings::structural_local_by_root(
                    &self.evaluation.structural_locals,
                    facts::PlaceRoot::Symbol(symbol),
                );
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
        Ok(repair)
    }

    /// Owned bindings can change value identity after the prior occurrence's
    /// ordered disposition. Reuse the completed producer's home; manufacturing
    /// a borrowed-storage window here would give an owned binding a second
    /// owner's restoration debt and require a spurious extraction.
    pub(super) fn rebind_structural_binding(
        &mut self,
        statement_index: u32,
        destination: checked_trees::CheckedStoreRoot,
        type_identity: typed_trees::type_identity::TypeIdentityId,
        value: &CheckedUnitStructuralArgumentPlan,
    ) -> Result<(), LoweringError> {
        let replacement = self.owned_structural_value(value)?;
        if replacement.structural_type != lookup_type_id(self.type_ids, &type_identity)? {
            return unsupported("owned rebind changes the binding's declared type");
        }
        let symbol = match destination {
            checked_trees::CheckedStoreRoot::Local { symbol } => {
                if !self
                    .checked
                    .facts
                    .state_locals
                    .named(self.state, symbol)
                    .is_some_and(|local| {
                        local.statement_index < statement_index
                            // Rebinding replaces the stored carrier, not the
                            // borrowed referent exposed by argument presentation.
                            && validation::stored_structural_type_identity(
                                &self.checked.typed,
                                local.declared_type,
                                &[],
                                &[],
                            ) == type_identity
                    })
                {
                    return unsupported(
                        "owned rebind names no matching dominating local declaration",
                    );
                }
                symbol
            }
            checked_trees::CheckedStoreRoot::Parameter { position } => {
                let state = self
                    .checked
                    .state_by_symbol(self.state)
                    .ok_or(LoweringError::Unsupported("owned rebind state is absent"))?;
                let formal = self
                    .checked
                    .state_parameters(state)
                    .get(position as usize)
                    .ok_or(LoweringError::Unsupported("owned rebind formal is absent"))?;
                if !formal.is_mutable {
                    return unsupported("owned rebind names no mutable formal");
                }
                let mut parameters = self.evaluation.structural_parameters.clone();
                let (_, parameter) = parameters
                    .iter_mut()
                    .find(|(authored, _)| *authored == position)
                    .ok_or(LoweringError::Unsupported(
                        "owned rebind formal home is absent",
                    ))?;
                if parameter.access != StructuralAccess::Owned
                    || parameter.structural_type != replacement.structural_type
                {
                    return unsupported("owned rebind changes formal custody or type");
                }
                self.evaluation
                    .previous_parameter_homes
                    .push(parameter.clone());
                parameter.place = replacement.place;
                self.evaluation
                    .bind_structural_parameters(parameters, self.structural_types.declarations());
                formal.symbol
            }
        };
        let types = self.structural_types.declarations();
        let same_presentation = |source, destination| {
            terminal_semantics::borrowed_referent_presentation(types.iter(), source, destination)
                == Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
        };
        let mut locals = self
            .evaluation
            .structural_locals
            .iter()
            .filter(|local| local.root == facts::PlaceRoot::Symbol(symbol));
        let old = locals.next();
        // A view's local argument exposes its borrowed contents, but the
        // binding still owns the descriptor being replaced. Its observation
        // roster must identify the same home before either namespace changes.
        let mut views = self
            .evaluation
            .view_locals
            .iter()
            .filter(|view| view.symbol == symbol);
        let previous_view = views.next().copied();
        if views.next().is_some() {
            return unsupported("owned rebind has more than one current view observation");
        }
        if previous_view.is_some_and(|view| {
            !same_presentation(view.structural_type, replacement.structural_type)
        }) {
            return unsupported("owned rebind changes its observed carrier representation");
        }
        let replacement_view_carrier = previous_view
            .map(|_| {
                crate::expression_preparation::bindings::view_locals::ViewCarrier::observation_of(
                    replacement.structural_type,
                    types,
                )
                .ok_or(LoweringError::Unsupported(
                    "owned rebind replacement has no view observation family",
                ))
            })
            .transpose()?;
        let binding_access = old
            .map(|local| local.argument.access)
            .or_else(|| previous_view.map(|view| view.access))
            .unwrap_or(StructuralAccess::Owned);
        if locals.next().is_some()
            || old.is_some_and(|local| {
                !same_presentation(local.structural_type, replacement.structural_type)
                    || (local.argument.access != StructuralAccess::Owned
                        && !previous_view.is_some_and(|view| {
                            view.place == local.argument.place
                                && same_presentation(view.structural_type, local.structural_type)
                                && view.access == local.argument.access
                        }))
                    || !local.argument.path.is_empty()
            })
        {
            return unsupported("owned rebind has ambiguous or incompatible destination custody");
        }
        // The declaration survives a consuming move; its old value binding
        // does not. Assignment publishes one new current occurrence whether
        // the preceding slot was occupied or vacated.
        self.evaluation
            .structural_locals
            .retain(|local| local.root != facts::PlaceRoot::Symbol(symbol));
        self.evaluation.structural_locals.push(
            crate::expression_preparation::bindings::StructuralLocalBinding {
                result_binding_ordinal: value.source_structural_result_binding_ordinal(),
                root: facts::PlaceRoot::Symbol(symbol),
                structural_type: replacement.structural_type,
                argument: terminal_psi::StructuralArgument {
                    place: replacement.place,
                    path: Vec::new(),
                    access: binding_access,
                },
            },
        );
        // Lengths and indexed reads use this same current descriptor. Earlier
        // captured operands retain their own places and are not retargeted.
        if let (Some(view), Some(carrier)) = (
            self.evaluation
                .view_locals
                .iter_mut()
                .find(|view| view.symbol == symbol),
            replacement_view_carrier,
        ) {
            view.place = replacement.place;
            view.structural_type = replacement.structural_type;
            view.carrier = carrier;
        }
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
            // Availability and cleanup order belong to this receiving event,
            // not the supplying producer's older declaration statement.
            owner.statement = statement_index;
        }
        Ok(())
    }

    /// Commit ordered cleanup at its exact continuation while retaining scalar values.
    pub(super) fn continuation_cleanup(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,

        boundary: &checked_trees::CheckedUnitCleanupBoundary,
        cleanup_actions: &[checked_trees::CheckedNormalCleanupAction],
        selected_regions: &[checked_trees::CheckedUnitStructuralArgumentPlan],
    ) -> Result<(), LoweringError> {
        let actions = crate::unit::unit_cleanup::lower_checked_cleanup_regions(
            identities,
            self.checked,
            cleanup_actions,
            selected_regions,
            &self
                .checked
                .facts
                .flow
                .terminal_unit_effects
                .cleanup_control,
            self.structural_types.declarations(),
            self.type_ids,
            |identities, source, selected_path| {
                let captured = match boundary {
                    checked_trees::CheckedUnitCleanupBoundary::Replacement {
                        statement_index,
                        destination,
                        old_value,
                        ..
                    } if destination.source == old_value.source
                        && destination.path == old_value.path =>
                    {
                        self.windows.captured_assignment_place(
                            identities,
                            *statement_index,
                            source,
                            selected_path,
                        )?
                    }
                    _ => None,
                };
                if let Some(captured) = captured {
                    return Ok((captured.source, captured.path));
                }
                let mut storage = self.evaluation.cleanup_storage(
                    source,
                    &self.parameters,
                    |binding_ordinal| {
                        self.results
                            .continuation_storage(binding_ordinal, self.operations)
                    },
                )?;
                storage.path.extend(crate::expression_preparation::bindings::structural_paths::lower_typed_argument_path(identities, selected_path, storage.selected_type, self.structural_types.declarations(), None, &mut std::iter::empty(), None)?);
                Ok((storage.place, storage.path))
            },
            |place| self.evaluation.carrier_multiplicity(place, self.operations),
            |cleanup, place, structural_type, path| {
                let mut action = crate::unit::unit_cleanup::lower_nominal_cleanup_action(
                    cleanup,
                    place,
                    structural_type,
                    path,
                    self.calls.machine_ids,
                    self.calls.cleanup_receivers,
                    self.calls.requirement_counts,
                    &mut self.calls.next_obligation_identity,
                )?;
                crate::unit::unit_cleanup::bind_consumed_claims(
                    &mut action,
                    &cleanup.consumed_claims,
                    self.calls.claims.bindings(),
                )?;
                Ok(action)
            },
        )?;
        self.evaluation.continue_cleanup_regions(
            identities,
            self.checked,
            actions,
            self.values,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
        )
    }
}
