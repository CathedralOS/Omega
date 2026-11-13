/*
Structural value construction, record and structural-case emission, element views, reference
formation, and completed aggregate storage in attached Unit bodies.
*/

pub(crate) use crate::unit::*;

pub(crate) mod attached_unit {
    pub(crate) use crate::unit::attached_unit::*;
    pub(crate) mod structural_values {

        use super::CheckedTrees;
        pub(crate) mod completed_aggregate {

            use super::super::super::StructuralFieldType;
            use super::super::{
                CheckedScalarExpressionRole, LoweringError, OperationKind, OperationResult,
                PlaceId, StructuralMultiplicity, StructuralPlaceDeclaration, StructuralTypeId,
                unsupported,
            };
            use super::member_memberships;
            use crate::emission::operation_emission::buffer::{
                OperationBuffer, structural_result_declaration,
            };
            use crate::emission::operation_emission::place_declarations::operation_result_place;
            use crate::unit::attached_unit::structural_values::ValueOutcome;
            use terminal_psi::{
                RecordFieldInitializer, RecordFieldValue, StructuralArgument,
                StructuralFieldDeclaration, StructuralParameterDeclaration,
                StructuralPathQualification, StructuralPathSegment,
            };

            pub(crate) enum CompletedAggregate<'a> {
                Record {
                    fields: Vec<RecordFieldInitializer>,
                    declarations: &'a [StructuralFieldDeclaration],
                },
                Case {
                    result_case: semantic_vocabulary::StructuralCaseId,
                    fields: Vec<RecordFieldInitializer>,
                    declarations: &'a [StructuralFieldDeclaration],
                    common_fields: usize,
                },
                Array {
                    elements: Vec<StructuralArgument>,
                },
            }

            pub(crate) fn emit_completed<'parameter>(
                place: PlaceId,
                structural_type: StructuralTypeId,
                multiplicity: StructuralMultiplicity,
                aggregate: CompletedAggregate<'_>,
                parameters: impl Iterator<Item = &'parameter StructuralParameterDeclaration> + Clone,
                operations: &mut OperationBuffer,
            ) -> Result<StructuralPlaceDeclaration, LoweringError> {
                let mut rows = Vec::new();
                let mut additional = Vec::new();
                let kind = match aggregate {
                    CompletedAggregate::Record {
                        fields,
                        declarations,
                    } => {
                        append_fields(
                            &fields,
                            declarations,
                            None,
                            parameters,
                            operations,
                            &mut rows,
                            &mut additional,
                        )?;
                        OperationKind::EstablishRecord { fields }
                    }
                    CompletedAggregate::Case {
                        result_case,
                        fields,
                        declarations,
                        common_fields,
                    } => {
                        append_fields(
                            &fields,
                            declarations,
                            Some((result_case, common_fields)),
                            parameters,
                            operations,
                            &mut rows,
                            &mut additional,
                        )?;
                        OperationKind::EstablishStructuralCase {
                            result_case,
                            fields,
                        }
                    }
                    CompletedAggregate::Array { elements } => {
                        append_elements(
                            &elements,
                            parameters,
                            operations,
                            &mut rows,
                            &mut additional,
                        )?;
                        OperationKind::EstablishStructuralArray { elements }
                    }
                };
                rows.sort();
                rows.dedup();
                additional.sort();
                additional.dedup();
                declare_establishment(
                    place,
                    structural_type,
                    multiplicity,
                    rows,
                    additional,
                    kind,
                    operations,
                )
            }

            fn declare_establishment(
                place: PlaceId,
                structural_type: StructuralTypeId,
                multiplicity: StructuralMultiplicity,
                projected_qualifications: Vec<StructuralPathQualification>,
                additional_input_qualifications: Vec<StructuralPathQualification>,
                kind: OperationKind,
                operations: &mut OperationBuffer,
            ) -> Result<StructuralPlaceDeclaration, LoweringError> {
                let mut result =
                    structural_result_declaration(place, structural_type, multiplicity);
                if let OperationResult::Structural(result) = &mut result {
                    result.projected_qualifications = projected_qualifications;
                    result.additional_input_qualifications = additional_input_qualifications;
                }
                let operation = operations.push_leaf(result, kind);
                Ok(operation_result_place(place, operation, structural_type))
            }

            fn append_fields<'parameter>(
                fields: &[RecordFieldInitializer],
                declarations: &[StructuralFieldDeclaration],
                payload: Option<(semantic_vocabulary::StructuralCaseId, usize)>,
                parameters: impl Iterator<Item = &'parameter StructuralParameterDeclaration> + Clone,
                operations: &OperationBuffer,
                destination: &mut Vec<StructuralPathQualification>,
                additional_destination: &mut Vec<StructuralPathQualification>,
            ) -> Result<(), LoweringError> {
                let declarations = declarations
                    .iter()
                    .filter(|declaration| !declaration.relevance.is_erased())
                    .collect::<Vec<_>>();
                if declarations.len() != fields.len() {
                    return unsupported("completed aggregate member has no declaration");
                }
                for (position, (field, declaration)) in fields.iter().zip(&declarations).enumerate()
                {
                    if field.field != declaration.id {
                        return unsupported("completed aggregate member has no declaration");
                    }
                    let RecordFieldValue::Structural(argument) = &field.value else {
                        continue;
                    };
                    if !matches!(declaration.field_type, StructuralFieldType::Structural(_)) {
                        continue;
                    }
                    let mut prefix = Vec::new();
                    if let Some((selected_case, common_fields)) = payload
                        && position >= common_fields
                    {
                        prefix.push(StructuralPathSegment::Case(selected_case));
                    }
                    prefix.push(StructuralPathSegment::Field(declaration.identity));
                    member_memberships::append(
                        argument,
                        &prefix,
                        parameters.clone(),
                        operations,
                        destination,
                        additional_destination,
                    );
                }
                Ok(())
            }

            fn append_elements<'parameter>(
                elements: &[StructuralArgument],
                parameters: impl Iterator<Item = &'parameter StructuralParameterDeclaration> + Clone,
                operations: &OperationBuffer,
                destination: &mut Vec<StructuralPathQualification>,
                additional_destination: &mut Vec<StructuralPathQualification>,
            ) -> Result<(), LoweringError> {
                for (ordinal, argument) in elements.iter().enumerate() {
                    let index = u64::try_from(ordinal).map_err(|_| {
                        LoweringError::Unsupported("aggregate element ordinal exceeds its carrier")
                    })?;
                    let prefix = [StructuralPathSegment::FixedIndex(index)];
                    member_memberships::append(
                        argument,
                        &prefix,
                        parameters.clone(),
                        operations,
                        destination,
                        additional_destination,
                    );
                }
                Ok(())
            }

            impl super::emission::Emission<'_, '_, '_> {
                pub(crate) fn completed_member_value(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    field: &checked_trees::CheckedStructuralRecordField,
                    target: &StructuralFieldDeclaration,
                    role: CheckedScalarExpressionRole,
                    selected_case: Option<semantic_vocabulary::StructuralCaseId>,
                    scalar_positions: &mut Vec<(usize, usize)>,
                    field_index: usize,
                    source_count: usize,
                    claims: &mut Vec<terminal_psi::StructuralResultClaimBinding>,
                ) -> Result<ValueOutcome<RecordFieldValue>, LoweringError> {
                    use checked_trees::CheckedStructuralRecordFieldValue;
                    Ok(ValueOutcome::Normal(match field.value {
                        CheckedStructuralRecordFieldValue::Scalar(value) => {
                            let evaluated = self.scalar(identities, role, value, source_count)?;
                            self.scalar_field_value(
                                identities,
                                target,
                                evaluated,
                                scalar_positions,
                                field_index,
                            )?
                        }
                        CheckedStructuralRecordFieldValue::Structural(value) => {
                            if let Some(scalar_type) = target.field_type.scalar_type() {
                                self.zeroed_scalar_field(
                                    identities,
                                    value,
                                    scalar_type,
                                    target,
                                    scalar_positions,
                                    field_index,
                                )?
                            } else {
                                let mut prefix = Vec::new();
                                if let Some(selected_case) = selected_case {
                                    prefix.push(StructuralPathSegment::Case(selected_case));
                                }
                                prefix.push(StructuralPathSegment::Field(target.identity));
                                let child_type = super::field_carrier::resolve(
                                    identities,
                                    self.checked,
                                    self.structural_types,
                                    &target.field_type,
                                    field.type_reference,
                                )?;
                                let ValueOutcome::Normal(completed) = self
                                    .child_constructor_operand(
                                        identities,
                                        child_type,
                                        value,
                                        field.type_reference,
                                    )?
                                else {
                                    return Ok(ValueOutcome::NoNormal);
                                };
                                if self.multiplicity == StructuralMultiplicity::Affine {
                                    for claim in completed.claims {
                                        let mut path = prefix.clone();
                                        path.extend(claim.path);
                                        claims.push(terminal_psi::StructuralResultClaimBinding {
                                            claim: claim.claim,
                                            path,
                                        });
                                    }
                                }
                                RecordFieldValue::Structural(completed.argument)
                            }
                        }
                    }))
                }
            }
        }
        pub(crate) mod emission {

            use super::super::super::{
                BlockId, ScalarType, StructuralAccess, StructuralArgument,
                StructuralParameterDeclaration, StructuralTypeDeclaration, SuccessorEdge, ValueId,
                block_id,
            };
            use super::super::{
                Block, CheckedScalarExpressionRole, LoweringError, Multiplicity, OperationKind,
                OperationResult, PlaceId, StructuralMultiplicity, StructuralPlaceDeclaration,
                StructuralPlaceKind, StructuralTypeId, StructuralTypeShape, Terminator,
                ValueDeclaration, allocate_dense, argument_evaluation, edge_id, lookup_type_id,
                obligation_id,
                operation_frame::{
                    allocate_structural_result, block_parameter_declaration,
                    structural_parameter_declaration,
                },
                place_id, unsupported, value_id,
            };
            use super::{CheckedTrees, ValueOutcome};
            use crate::emission::claim_bindings::emit_establishment_claims;
            use crate::emission::operation_emission::buffer::{
                OperationBuffer, scalar_result_declaration,
            };
            use crate::emission::operation_emission::calls::CallEmissionContext;
            use crate::emission::operation_emission::expressions::emit_scalar_leaf;
            use crate::emission::operation_emission::place_declarations::operation_result_place;
            use crate::emission::scalar_types::terminal_scalar_type;
            use crate::expression_preparation::bindings::StructuralLocalBinding;
            use crate::expression_preparation::bindings::structural_local_by_root;
            use crate::expression_preparation::bindings::structural_paths::lower_static_argument_path;
            use crate::expression_preparation::bindings::structural_paths::terminal_access;
            use crate::expression_preparation::bindings::{LoanAccess, ScalarBindings};
            use crate::expression_preparation::byte_literals;
            use crate::expression_preparation::computation_graph::fields;
            use crate::scalar_lowering::computations::cases::declarations;
            use crate::shared::checked_facts::{flow_facts, value_facts};
            use crate::shared::reference_custody::binding_multiplicity;
            use crate::shared::terminal_identities::lookup_machine_id;
            use crate::shared::terminal_identities::stored_structural_identity;
            use arena::HandleSpan;
            use checked_trees::{
                CheckedStructuralValue, CheckedStructuralValueHandle, CheckedStructuralValueKind,
            };

            pub(crate) mod completed_projection {
                //! Complete an authenticated place in its original descriptor/selector order.
                //! Value reads and dispatch captures share this evaluator; the caller decides
                //! whether to consume the endpoint immediately or retain its address.

                use crate::emission::completed_place::Frontier;
                use crate::emission::operation_emission::buffer::OperationBuffer;
                use crate::emission::operation_emission::calls::CallEmissionContext;
                use crate::expression_preparation::bindings::structural_paths::{
                    PreparedStructuralPathSegment, emit_captured_argument_path,
                };
                use crate::shared::checked_facts::value_facts;
                use crate::shared::lowering_error::LoweringError;
                use crate::unit::attached_unit::argument_evaluation::Evaluation;
                use crate::unit::attached_unit::operation_frame::OperationEnvironment;
                use crate::unit::attached_unit::operation_frame::completed_arguments::selected_memberships;
                use semantic_vocabulary::PlaceId;
                use terminal_psi::{StructuralAccess, StructuralArgument, ValueDeclaration};

                pub(crate) struct CompletedProjection {
                    pub(crate) expression: checked_trees::expression::ExpressionHandle,
                    pub(crate) argument: StructuralArgument,
                    pub(crate) source_root: PlaceId,
                }

                pub(crate) struct ProjectionEmission<'a, 'b, 'calls> {
                    pub(crate) statement: u32,
                    pub(crate) environment: &'b mut OperationEnvironment<'a>,
                    pub(crate) calls: &'b mut CallEmissionContext<'calls>,
                    pub(crate) evaluation: &'b mut Evaluation,
                    pub(crate) values: &'b mut Vec<ValueDeclaration>,
                    pub(crate) next_value: &'b mut u64,
                    pub(crate) next_block: &'b mut u64,
                    pub(crate) next_edge: &'b mut u64,
                    pub(crate) operations: &'b mut OperationBuffer,
                }

                impl ProjectionEmission<'_, '_, '_> {
                    /// Retain the completed subject address before any pattern operand runs.
                    /// The alias names this actual borrowed continuation, never a fresh owner.
                    pub(crate) fn capture_subject(
                        &mut self,
                        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                        expression: checked_trees::expression::ExpressionHandle,
                        source: &checked_trees::CheckedUnitStructuralArgumentPlan,
                        alias: checked_trees::expression::ExpressionHandle,
                        place: PlaceId,
                    ) -> Result<(), LoweringError> {
                        let completed =
                            self.readable_place_argument(identities, expression, source)?;
                        let argument = completed.argument;
                        let expected = crate::shared::terminal_identities::lookup_type_id(
                            self.environment.type_ids,
                            &source.type_identity,
                        )?;
                        let (qualifications, projected, additional) = selected_memberships(
                            &argument,
                            &self.environment.parameters,
                            self.evaluation,
                            self.operations,
                        )?;
                        let (structural_type, qualifications, projected, additional) =
                            crate::emission::completed_place::present_capture(
                                &crate::emission::completed_place::Capture::Address(expected),
                                argument.access,
                                self.environment.structural_types,
                                self.calls.domain_declarations,
                                qualifications,
                                projected,
                                additional,
                            )?;
                        crate::emission::completed_address::capture_into(
                            argument.place,
                            argument.path,
                            structural_type,
                            argument.access,
                            qualifications,
                            projected,
                            additional,
                            place,
                            self.evaluation,
                            self.values,
                            self.next_value,
                            self.next_block,
                            self.next_edge,
                            self.environment.private_places,
                            self.operations,
                        )?;
                        crate::expression_preparation::bindings::retain_completed_structural_homes(
                            &mut self.evaluation.structural_locals,
                            &[
                                crate::expression_preparation::bindings::StructuralLocalBinding {
                                    result_binding_ordinal: None,
                                    root: facts::PlaceRoot::Expression(alias),
                                    argument: StructuralArgument::whole(place, argument.access),
                                    structural_type,
                                },
                            ],
                        )
                    }

                    pub(crate) fn complete_projection(
                        &mut self,
                        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                        expression: checked_trees::expression::ExpressionHandle,
                        mut place: PlaceId,
                        mut prepared: Vec<PreparedStructuralPathSegment>,
                        access: StructuralAccess,
                    ) -> Result<CompletedProjection, LoweringError> {
                        let source_root = place;
                        let root = self.projection_root_type(place)?;
                        let frontiers = crate::emission::completed_place::plan(
                            self.environment.structural_types,
                            root,
                            &prepared,
                            false,
                            access,
                            crate::emission::completed_place::EndpointUse::Storage,
                        )?;
                        let last_selector = frontiers
                            .iter()
                            .rposition(|frontier| matches!(frontier, Frontier::Selector { .. }));
                        let original = prepared.clone();
                        let source_count = self.values.len();
                        let mut removed = 0;
                        let mut consumed = 0;
                        for (frontier_position, frontier) in frontiers.into_iter().enumerate() {
                            match frontier {
                                Frontier::Selector { position } => {
                                    self.projection_selector(
                                        identities,
                                        expression,
                                        &original[position],
                                        source_count,
                                    )?;
                                }
                                Frontier::Capture { prefix, capture } => {
                                    if last_selector.is_none_or(|last| frontier_position > last) {
                                        continue;
                                    }
                                    let length = prefix.checked_sub(removed).ok_or(
                                        LoweringError::Unsupported(
                                            "projection frontier precedes its completed address",
                                        ),
                                    )?;
                                    let selected = prepared.get(..length).ok_or(
                                        LoweringError::Unsupported(
                                            "projection frontier exceeds its retained path",
                                        ),
                                    )?;
                                    let selector_count = selected
                                        .iter()
                                        .filter(|segment| evaluated_selector(segment))
                                        .count();
                                    let mut selectors = self.values[source_count + consumed..]
                                        .iter()
                                        .map(|value| value.id);
                                    let path = emit_captured_argument_path(
                                        selected,
                                        place,
                                        Some(self.values.as_slice()),
                                        &mut selectors,
                                        Some(&mut *self.calls),
                                        Some((&mut *self.next_value, &mut *self.operations)),
                                    )?;
                                    let source = StructuralArgument {
                                        place,
                                        path,
                                        access,
                                    };
                                    let (qualifications, projected, additional) =
                                        selected_memberships(
                                            &source,
                                            &[],
                                            self.evaluation,
                                            self.operations,
                                        )?;
                                    let (structural_type, qualifications, projected, additional) =
                                        crate::emission::completed_place::present_capture(
                                            &capture,
                                            access,
                                            self.environment.structural_types,
                                            self.calls.domain_declarations,
                                            qualifications,
                                            projected,
                                            additional,
                                        )?;
                                    place = crate::emission::completed_address::capture(
                                        source.place,
                                        source.path,
                                        structural_type,
                                        access,
                                        qualifications,
                                        projected,
                                        additional,
                                        self.evaluation,
                                        self.values,
                                        self.next_value,
                                        self.next_block,
                                        self.next_edge,
                                        self.environment.next_place,
                                        self.environment.private_places,
                                        self.operations,
                                    )?;
                                    prepared.drain(..length);
                                    removed = prefix;
                                    consumed += selector_count;
                                }
                                Frontier::Value { .. } => {
                                    return crate::shared::lowering_error::unsupported(
                                        "projection address evaluation cannot manufacture a value copy",
                                    );
                                }
                            }
                        }
                        let mut selectors = self.values[source_count + consumed..]
                            .iter()
                            .map(|value| value.id);
                        let path = emit_captured_argument_path(
                            &prepared,
                            place,
                            Some(self.values.as_slice()),
                            &mut selectors,
                            Some(&mut *self.calls),
                            Some((&mut *self.next_value, &mut *self.operations)),
                        )?;
                        self.values.truncate(source_count);
                        let argument = StructuralArgument {
                            place,
                            path,
                            access,
                        };
                        Ok(CompletedProjection {
                            expression,
                            argument,
                            source_root,
                        })
                    }

                    /// Geometry comes from established source homes and producer declarations,
                    /// including private places that have no operation-result declaration.
                    /// This lookup grants neither storage liveness nor projection permission.
                    pub(crate) fn projection_root_type(
                        &self,
                        place: PlaceId,
                    ) -> Result<semantic_vocabulary::StructuralTypeId, LoweringError>
                    {
                        self.evaluation
                            .structural_root_parameters()
                            .find(|row| row.place == place)
                            .map(|row| row.structural_type)
                            .or_else(|| {
                                self.evaluation
                                    .completed_bindings(
                                        self.operations,
                                        self.environment.private_places.iter().cloned(),
                                    )
                                    .structural_place_type(place)
                            })
                            .ok_or(LoweringError::Unsupported(
                                "projection lost its physical root declaration",
                            ))
                    }

                    fn projection_selector(
                        &mut self,
                        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                        expression: checked_trees::expression::ExpressionHandle,
                        segment: &PreparedStructuralPathSegment,
                        source_count: usize,
                    ) -> Result<(), LoweringError> {
                        match segment {
                            PreparedStructuralPathSegment::ComputedIndex(computation)
                            | PreparedStructuralPathSegment::EvaluatedFixedIndex {
                                computation,
                                ..
                            } => {
                                let mut roles = value_facts(self.environment.checked)
                                    .scalar_computations
                                    .roots_for_statement(self.environment.state, self.statement)
                                    .into_iter()
                                    .filter_map(|root| {
                                        matches!(root.role,
                            checked_trees::CheckedScalarExpressionRole::StructuralValueSelector {
                                expression: owner, ..
                            } if owner == expression && root.root == *computation
                        )
                        .then_some(root.role)
                                    });
                                let role = roles.next().ok_or(LoweringError::Unsupported(
                                    "projection selector lost its exact authored computation root",
                                ))?;
                                if roles.next().is_some() {
                                    return crate::shared::lowering_error::unsupported(
                                        "projection selector has multiple authored roots",
                                    );
                                }
                                let value = self.evaluation.source_value(
                                    identities,
                                    self.environment.checked,
                                    self.environment.machine,
                                    self.environment.state,
                                    self.statement,
                                    role,
                                    &checked_trees::CheckedCallScalarArgument::Computation(
                                        *computation,
                                    ),
                                    source_count,
                                    self.values,
                                    self.next_value,
                                    self.next_block,
                                    self.next_edge,
                                    self.operations,
                                    self.calls,
                                    self.environment,
                                )?;
                                self.values.push(value);
                            }
                            PreparedStructuralPathSegment::RuntimeIndex(
                                checked_trees::CheckedRuntimeIndex::StructuralField(_)
                                | checked_trees::CheckedRuntimeIndex::StorageRead { .. },
                            ) => {
                                let bindings = self.evaluation.contextual_bindings();
                                let observation = match segment {
                                    PreparedStructuralPathSegment::RuntimeIndex(
                                        checked_trees::CheckedRuntimeIndex::StorageRead { symbol },
                                    ) => bindings.runtime_index_storage(*symbol)?,
                                    PreparedStructuralPathSegment::RuntimeIndex(
                                        checked_trees::CheckedRuntimeIndex::StructuralField(field),
                                    ) => bindings.runtime_index_field(identities, field)?,
                                    _ => unreachable!("matched runtime selector"),
                                };
                                if !matches!(observation.scalar_type(), semantic_vocabulary::ScalarType::Integer(integer)
                                    if !integer.is_address())
                                {
                                    return crate::shared::lowering_error::unsupported(
                                        "projection selector is not an integer read",
                                    );
                                }
                                let id =
                                    crate::emission::operation_emission::emit_direct_expression(
                                        &observation,
                                        self.values.as_slice(),
                                        self.next_value,
                                        self.operations,
                                    );
                                let value = self
                                    .operations
                                    .iter()
                                    .rev()
                                    .filter_map(|operation| operation.result.scalar())
                                    .find(|value| value.id == id)
                                    .or_else(|| {
                                        self.values.iter().find(|value| value.id == id).copied()
                                    })
                                    .ok_or(LoweringError::Unsupported(
                                        "projection selector lost its evaluated value",
                                    ))?;
                                self.values.push(value);
                            }
                            _ => {
                                return crate::shared::lowering_error::unsupported(
                                    "projection frontier has no executable selector",
                                );
                            }
                        }
                        Ok(())
                    }
                }

                fn evaluated_selector(segment: &PreparedStructuralPathSegment) -> bool {
                    matches!(
                        segment,
                        PreparedStructuralPathSegment::ComputedIndex(_)
                            | PreparedStructuralPathSegment::EvaluatedFixedIndex { .. }
                            | PreparedStructuralPathSegment::RuntimeIndex(
                                checked_trees::CheckedRuntimeIndex::StructuralField(_)
                                    | checked_trees::CheckedRuntimeIndex::StorageRead { .. }
                            )
                    )
                }

                impl<'source, 'frame, 'calls> super::Emission<'source, 'frame, 'calls> {
                    pub(super) fn complete_projection_source(
                        &mut self,
                        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                        expression: checked_trees::expression::ExpressionHandle,
                        source: &super::ProjectionSource,
                        evaluations: arena::HandleSpan<
                            checked_trees::CheckedScalarComputationHandle,
                        >,
                        endpoint: typed_trees::type_identity::TypeIdentityId,
                        access: StructuralAccess,
                    ) -> Result<StructuralArgument, LoweringError> {
                        let root = crate::shared::terminal_identities::lookup_type_id(
                            self.type_ids,
                            &source.root_type_identity,
                        )?;
                        // The checked producer owns the root coordinate. A stored reference
                        // can present logical backing without granting ownership of it; a
                        // whole descriptor or saved physical projection stays Published.
                        // Leaf copy/move and endpoint completion remain separate judgments.
                        let mut suffix =
                crate::expression_preparation::bindings::structural_paths::prepare_projection_path(
                    identities,
                    self.checked,
                    &source.path,
                    if source.logical_root_type_identity.is_valid() {
                        crate::expression_preparation::bindings::structural_paths::ProjectionRoot::Borrowed {
                        published: root,
                        logical: source.logical_root_type_identity,
                        // projected_argument_path deliberately omits root-shell
                        // steps for symbol-rooted logical storage projections.
                        spelled_reference_hops: 0,
                    }
                    } else {
                        crate::expression_preparation::bindings::structural_paths::ProjectionRoot::Published(root)
                    },
                    endpoint,
                    access,
                    self.structural_types,
                )?;
                        crate::expression_preparation::bindings::structural_paths::retain_selector_evaluations(
                &source.path,
                evaluations,
                &self.checked.facts.values.scalar_computations.operands,
                &mut suffix,
            )?;
                        let mut prepared = source
                            .root_path
                            .iter()
                            .cloned()
                            .map(PreparedStructuralPathSegment::Static)
                            .collect::<Vec<_>>();
                        prepared.extend(suffix);
                        self.complete_projection(
                            identities,
                            expression,
                            source.root,
                            prepared,
                            access,
                        )
                    }

    pub(super) fn projection_emission(
        &mut self,
    ) -> crate::unit::attached_unit::structural_values::emission::completed_projection::ProjectionEmission<
        'source,
        '_,
        'calls,
                    >{
                        crate::unit::attached_unit::structural_values::emission::completed_projection::ProjectionEmission {
            statement: self.statement,
            environment: self.environment,
            calls: self.calls,
            evaluation: self.evaluation,
            values: self.values,
            next_value: self.next_value,
            next_block: self.next_block,
            next_edge: self.next_edge,
            operations: self.operations,
        }
                    }

                    pub(super) fn complete_projection(
                        &mut self,
                        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                        expression: checked_trees::expression::ExpressionHandle,
                        place: PlaceId,
                        prepared: Vec<PreparedStructuralPathSegment>,
                        access: StructuralAccess,
                    ) -> Result<StructuralArgument, LoweringError> {
                        let completed = self
                            .projection_emission()
                            .complete_projection(identities, expression, place, prepared, access)?;
                        let argument = completed.argument.clone();
                        self.completed_projections.push(completed);
                        Ok(argument)
                    }

                    pub(super) fn projection_root_type(
                        &mut self,
                        place: PlaceId,
                    ) -> Result<semantic_vocabulary::StructuralTypeId, LoweringError>
                    {
                        self.projection_emission().projection_root_type(place)
                    }
                }
            }
            pub(crate) mod completed_storage {

                use super::{
                    Emission, LoweringError, OperationBuffer, PlaceId, StructuralAccess,
                    StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
                    StructuralPlaceDeclaration, StructuralPlaceKind, unsupported,
                };
                use crate::emission::claim_bindings::direct_establishment_claims;
                use crate::emission::claim_bindings::emit_establishment_claims;
                use crate::emission::formation_memberships::publish;
                use crate::emission::operation_emission::place_declarations::operation_result_place;
                use crate::shared::terminal_identities::lookup_domain_id_for_carrier;

                impl Emission<'_, '_, '_> {
                    pub(crate) fn transported_parameter(
                        &self,
                        source: &StructuralArgument,
                        place: PlaceId,
                        position: u32,
                        structural_type: semantic_vocabulary::StructuralTypeId,
                        multiplicity: StructuralMultiplicity,
                    ) -> Result<StructuralParameterDeclaration, LoweringError> {
                        let (qualifications, projected_qualifications, additional) = self
                            .completed_memberships(source.place)
                            .ok_or(LoweringError::Unsupported(
                                "owned transport has no source membership declaration",
                            ))?;
                        let (mut qualifications, mut projected_qualifications) =
                            terminal_psi::StructuralPathQualification::select(
                                qualifications,
                                projected_qualifications,
                                &source.path,
                            );
                        let mut additional_input_qualifications =
                            terminal_psi::StructuralPathQualification::select_rows(
                                additional,
                                &source.path,
                            );
                        terminal_semantics::present_structural_membership_contract(
                            self.structural_types.iter(),
                            self.domain_declarations.iter(),
                            structural_type,
                            &mut qualifications,
                            &mut projected_qualifications,
                            &mut additional_input_qualifications,
                        )
                        .map_err(LoweringError::Unsupported)?;
                        Ok(StructuralParameterDeclaration {
                            place,
                            position,
                            structural_type,
                            multiplicity,
                            access: StructuralAccess::Owned,
                            qualifications,
                            projected_qualifications,
                            additional_input_qualifications,
                        })
                    }

                    pub(crate) fn value_binding_parameter(
                        &self,
                        _identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                        source: &StructuralArgument,
                        place: PlaceId,
                        position: u32,
                    ) -> Result<StructuralParameterDeclaration, LoweringError> {
                        let parameter = self.transported_parameter(
                            source,
                            place,
                            position,
                            self.structural_type,
                            self.multiplicity,
                        )?;
                        if !self.binding_type_reference.is_valid() {
                            return Ok(parameter);
                        }
                        let mut whole = validation::view_result_carrier_qualifications(
                            &self.checked.typed,
                            self.binding_type_reference,
                        )
                        .map_err(LoweringError::Unsupported)?
                        .into_iter()
                        .map(|domain| {
                            lookup_domain_id_for_carrier(
                                self.domain_ids,
                                domain,
                                self.structural_type,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                        whole.sort();
                        whole.dedup();
                        if whole
                            .iter()
                            .any(|domain| !parameter.qualifications.contains(domain))
                        {
                            return Err(LoweringError::at_statement(
                                self.checked,
                                self.machine,
                                self.state,
                                self.statement,
                                "owned binding cannot establish a receiving membership",
                            ));
                        }
                        Ok(parameter)
                    }

                    pub(crate) fn completed_memberships(
                        &self,
                        place: PlaceId,
                    ) -> Option<(
                        &[semantic_vocabulary::StructuralDomainId],
                        &[terminal_psi::StructuralPathQualification],
                        &[terminal_psi::StructuralPathQualification],
                    )> {
                        self.operations
                            .structural_result_contract(place)
                            .map(|result| {
                                (
                                    result.qualifications.as_slice(),
                                    result.projected_qualifications.as_slice(),
                                    result.additional_input_qualifications.as_slice(),
                                )
                            })
                            .or_else(|| {
                                self.operations
                                    .iter()
                                    .find_map(|operation| {
                                        match &operation.kind {
                                        terminal_psi::OperationKind::EstablishByteSequenceLiteral {
                                            destination,
                                            qualifications,
                                            ..
                                        } if *destination == place => {
                                            Some((qualifications.as_slice(), &[][..], &[][..]))
                                        }
                                        _ => None,
                                    }
                                    })
                            })
                            .or_else(|| {
                                self.evaluation
                                    .structural_root_parameters()
                                    .find(|parameter| parameter.place == place)
                                    .map(|parameter| {
                                        (
                                            parameter.qualifications.as_slice(),
                                            parameter.projected_qualifications.as_slice(),
                                            parameter.additional_input_qualifications.as_slice(),
                                        )
                                    })
                            })
                    }

                    pub(crate) fn selected_input_qualifications(
                        &self,
                        source: &StructuralArgument,
                        whole: &[semantic_vocabulary::StructuralDomainId],
                        projected: &[terminal_psi::StructuralPathQualification],
                    ) -> Result<Vec<terminal_psi::StructuralPathQualification>, LoweringError>
                    {
                        let (_, _, additional) =
                            self.completed_memberships(source.place).ok_or_else(|| {
                                LoweringError::at_statement(
                                    self.checked,
                                    self.machine,
                                    self.state,
                                    self.statement,
                                    "input fact source contract is missing",
                                )
                            })?;
                        let mut selected = terminal_psi::StructuralPathQualification::select_rows(
                            additional,
                            &source.path,
                        );
                        terminal_semantics::present_structural_memberships(
                            self.structural_types.iter(),
                            self.domain_declarations.iter(),
                            self.structural_type,
                            &mut Vec::new(),
                            &mut selected,
                        )
                        .map_err(|reason| {
                            LoweringError::at_statement(
                                self.checked,
                                self.machine,
                                self.state,
                                self.statement,
                                reason,
                            )
                        })?;
                        selected.retain(|row| {
                            if row.path.is_empty() {
                                whole.contains(&row.domain)
                            } else {
                                projected.contains(row)
                            }
                        });
                        Ok(selected)
                    }

                    pub(crate) fn checked_membership_subject(
                        &self,
                    ) -> Result<
                        (
                            semantic_vocabulary::StructuralTypeId,
                            &'static [terminal_psi::StructuralPathSegment],
                        ),
                        LoweringError,
                    > {
                        let declaration = terminal_psi::find_structural_type(
                            self.structural_types,
                            self.structural_type,
                        )
                        .ok_or(LoweringError::Unsupported(
                            "checked membership carrier is absent",
                        ))?;
                        Ok(match declaration.shape {
                            terminal_psi::StructuralTypeShape::Reference { referent, .. } => {
                                (referent, &[terminal_psi::StructuralPathSegment::Referent])
                            }
                            _ => (self.structural_type, &[]),
                        })
                    }

                    pub(crate) fn checked_membership_domain(
                        &self,
                        decision: &checked_trees::CheckedValueQualification,
                        carrier: semantic_vocabulary::StructuralTypeId,
                    ) -> Result<semantic_vocabulary::StructuralDomainId, LoweringError>
                    {
                        lookup_domain_id_for_carrier(
                            self.domain_ids,
                            decision.semantic_domain,
                            carrier,
                        )
                        .map_err(|cause| {
                            LoweringError::InvalidOccurrenceMembership {
                                machine: self.checked.symbols.display_path(self.machine, "::"),
                                state: self.checked.symbols.display_path(self.state, "::"),
                                statement_index: self.statement,
                                domain: self
                                    .checked
                                    .typed
                                    .semantic_domains
                                    .name(decision.semantic_domain)
                                    .unwrap_or("<unregistered domain>")
                                    .to_owned(),
                                carrier,
                                cause: Box::new(cause),
                            }
                        })
                    }

                    pub(crate) fn checked_membership_domains(
                        &self,
                        decisions: arena::HandleSpan<checked_trees::CheckedValueQualification>,
                        carrier: semantic_vocabulary::StructuralTypeId,
                    ) -> Result<Vec<semantic_vocabulary::StructuralDomainId>, LoweringError>
                    {
                        let mut domains = self
                            .checked
                            .facts
                            .qualifications
                            .value_formations
                            .span(decisions)
                            .ok_or(LoweringError::Unsupported(
                                "operand membership decisions are stale",
                            ))?
                            .iter()
                            .map(|decision| self.checked_membership_domain(decision, carrier))
                            .collect::<Result<Vec<_>, _>>()?;
                        domains.sort();
                        domains.dedup();
                        Ok(domains)
                    }

                    pub(crate) fn complete_construction(
                        &mut self,
                        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                        place: PlaceId,
                        decisions: arena::HandleSpan<checked_trees::CheckedValueQualification>,
                        completion: super::CompletionOccurrence,
                        continuation: Option<&mut super::ValueContinuation>,
                    ) -> Result<PlaceId, LoweringError> {
                        let node = self.structural_value_node(completion.value);
                        if !matches!(
                            &node.kind,
                            checked_trees::CheckedStructuralValueKind::ZeroedStructural { .. }
                        ) && validation::linear_claim_frontier(
                            &self.checked.typed,
                            node.type_reference,
                        )
                        .iter()
                        .any(|claim| claim.path.is_empty())
                        {
                            let mut prepared =
                                crate::emission::claim_bindings::prepare_establishment_claims(
                                    identities,
                                    self.checked,
                                    self.machine,
                                    self.state,
                                    self.statement as usize,
                                    facts::PlaceRoot::Expression(node.expression),
                                    node.type_reference,
                                    self.structural_type,
                                    self.structural_types,
                                )?;
                            prepared.retain(|claim| claim.path.is_empty());
                            if prepared.len() != 1 {
                                return Err(LoweringError::MissingConstructionClaim {
                                    machine: self.machine,
                                    state: self.state,
                                    statement_index: self.statement,
                                    expression: node.expression,
                                    structural_type: self.structural_type,
                                    root_candidates: prepared.len(),
                                });
                            }
                            let claims =
                                emit_establishment_claims(&prepared, &mut self.calls.claims)?;
                            let introductions =
                                direct_establishment_claims(&prepared, &self.calls.claims)?;
                            Self::bind_completion_claims(
                                self.operations,
                                place,
                                &claims,
                                &introductions,
                            )?;
                        }
                        let domains =
                            self.checked_membership_domains(decisions, self.structural_type)?;
                        publish(self.operations, place, domains)?;
                        self.completed(identities, place, completion, continuation)
                    }

                    pub(crate) fn retain_checked_memberships(
                        &mut self,
                        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                        source: StructuralArgument,
                        decisions: arena::HandleSpan<checked_trees::CheckedValueQualification>,
                        expression: typed_trees::expression::ExpressionHandle,
                        first_new_place: u64,
                    ) -> Result<StructuralArgument, LoweringError> {
                        let source = self.refresh_memberships(
                            identities,
                            source,
                            expression,
                            first_new_place,
                        )?;
                        let memberships = self
                            .checked
                            .facts
                            .qualifications
                            .value_formations
                            .span(decisions)
                            .ok_or(LoweringError::Unsupported(
                                "operand membership decisions are stale",
                            ))?;
                        if memberships.is_empty() {
                            return Ok(source);
                        }
                        let (membership_carrier, relative) = self.checked_membership_subject()?;
                        let mut membership_path = source.path.clone();
                        membership_path.extend_from_slice(relative);
                        let (qualifications, projected_qualifications, _) = self
                            .completed_memberships(source.place)
                            .ok_or(LoweringError::Unsupported(
                                "completed operand membership source is missing",
                            ))?;
                        let (mut qualifications, _) =
                            terminal_psi::StructuralPathQualification::select(
                                qualifications,
                                projected_qualifications,
                                &membership_path,
                            );
                        terminal_semantics::present_structural_memberships(
                            self.structural_types.iter(),
                            self.domain_declarations.iter(),
                            membership_carrier,
                            &mut qualifications,
                            &mut Vec::new(),
                        )
                        .map_err(LoweringError::Unsupported)?;
                        let mut established = Vec::new();
                        for decision in memberships {
                            let domain =
                                self.checked_membership_domain(decision, membership_carrier)?;
                            if !qualifications.contains(&domain) {
                                let declaration = self
                                    .domain_declarations
                                    .iter()
                                    .find(|row| row.id == domain)
                                    .ok_or(LoweringError::Unsupported(
                                        "owned membership declaration is absent",
                                    ))?;
                                let missing = declaration
                                    .missing_membership_requirements(
                                        self.domain_declarations,
                                        &qualifications,
                                    )
                                    .ok_or(LoweringError::Unsupported(
                                        "owned membership transport has unknown prerequisites",
                                    ))?;
                                if missing
                                    .iter()
                                    .any(|row| !row.establishment_routes.is_empty())
                                {
                                    return unsupported(
                                        "owned membership transport cannot establish routed authority",
                                    );
                                }
                                established.push(domain);
                            }
                        }
                        if established.is_empty() {
                            return Ok(source);
                        }
                        if self.multiplicity != StructuralMultiplicity::Unrestricted {
                            if source.access != StructuralAccess::Owned {
                                return unsupported(
                                    "membership transfer requires owned value custody",
                                );
                            }
                            let completed = self.capture_operand(
                                identities,
                                source,
                                expression,
                                first_new_place,
                            )?;
                            let source = &completed.argument;
                            let (whole, projected, _) = self
                                .completed_memberships(source.place)
                                .ok_or(LoweringError::Unsupported(
                                    "owned membership source disappeared",
                                ))?;
                            let (mut qualifications, mut projected_qualifications) =
                                terminal_psi::StructuralPathQualification::select(
                                    whole,
                                    projected,
                                    &source.path,
                                );
                            if relative.is_empty() {
                                qualifications.extend(established);
                                qualifications.sort();
                                qualifications.dedup();
                            } else {
                                terminal_psi::StructuralPathQualification::append_under(
                                    &established,
                                    &[],
                                    relative,
                                    &mut projected_qualifications,
                                );
                                projected_qualifications.sort();
                                projected_qualifications.dedup();
                            }
                            return self.move_completed_operand(
                                completed,
                                qualifications,
                                projected_qualifications,
                            );
                        }
                        self.copied_leaf_place(identities, &source, decisions, expression)
                            .map(|place| StructuralArgument {
                                place,
                                path: Vec::new(),
                                access: StructuralAccess::Owned,
                            })
                    }
                    pub(crate) fn move_completed_operand(
                        &mut self,
                        completed: super::super::operand_claims::CompletedOperand,
                        qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
                        projected_qualifications: Vec<terminal_psi::StructuralPathQualification>,
                    ) -> Result<StructuralArgument, LoweringError> {
                        let source = completed.argument;
                        if source.access != StructuralAccess::Owned {
                            return unsupported("owned subtree move requires exact source custody");
                        }
                        let additional_input_qualifications = self.selected_input_qualifications(
                            &source,
                            &qualifications,
                            &projected_qualifications,
                        )?;
                        let source_place = source.place;
                        let source_path = source.path.clone();
                        let transported_claims = completed.claims;
                        let place =
                            super::place_id(super::allocate_dense(self.environment.next_place)?);
                        let operation = self.operations.allocate();
                        self.operations.push(terminal_psi::Operation::unsuspended(
                            operation,
                            terminal_psi::OperationResult::Structural(
                                terminal_psi::StructuralOperationResult {
                                    place,
                                    structural_type: self.structural_type,
                                    multiplicity: self.multiplicity,
                                    qualifications,
                                    projected_qualifications,
                                    additional_input_qualifications,
                                    qualification_establishments: Vec::new(),
                                    claims: transported_claims.clone(),
                                },
                            ),
                            terminal_psi::OperationKind::MoveStructuralValue { source },
                        ));
                        self.evaluation
                            .retire_structural_operation_operands(operation, self.operations)?;
                        self.environment.private_places.push(operation_result_place(
                            place,
                            operation,
                            self.structural_type,
                        ));
                        self.operations.register_structural_materialization(
                            place,
                            self.state,
                            self.statement,
                        )?;
                        if self.evaluation.current_claims.contains(source_place) {
                            self.evaluation.current_claims.replace(
                                source_place,
                                &source_path,
                                &[],
                            )?;
                        }
                        self.evaluation
                            .current_claims
                            .publish(place, transported_claims)?;
                        Ok(StructuralArgument {
                            place,
                            path: Vec::new(),
                            access: StructuralAccess::Owned,
                        })
                    }
                }

                pub(crate) fn completed_result<'a>(
                    place: PlaceId,
                    declaration: &StructuralPlaceDeclaration,
                    operations: &OperationBuffer,
                    mut parameters: impl Iterator<Item = &'a StructuralParameterDeclaration> + Clone,
                    arrival_claims: &[terminal_psi::StructuralResultClaimBinding],
                ) -> Result<terminal_psi::StructuralOperationResult, LoweringError>
                {
                    let mut producers = operations.iter().filter_map(|operation| {
                        operation
                            .result
                            .structural()
                            .filter(|result| result.place == place)
                            .map(|result| (operation.id, result))
                    });
                    if let Some((producer, result)) = producers.next() {
                        if producers.next().is_some()
                            || parameters.clone().any(|parameter| parameter.place == place)
                            || declaration.kind
                                != (StructuralPlaceKind::OperationResult {
                                    producer,
                                    structural_type: result.structural_type,
                                })
                        {
                            return unsupported("completed value does not have one exact producer");
                        }
                        return Ok(result.clone());
                    }
                    let StructuralPlaceDeclaration {
                        kind: StructuralPlaceKind::BlockParameter { .. },
                        ..
                    } = declaration
                    else {
                        return unsupported(
                            "completed value has no producer or arrival declaration",
                        );
                    };
                    let parameter = parameters
                        .find(|parameter| parameter.place == place)
                        .ok_or(LoweringError::Unsupported(
                            "completed value has no current arrival",
                        ))?;
                    if arrival_claims.windows(2).any(|pair| pair[0] >= pair[1])
                        || (parameter.multiplicity == terminal_psi::StructuralMultiplicity::Linear
                            && arrival_claims.is_empty())
                    {
                        return unsupported(
                            "completed linear arrival lost its exact claim frontier",
                        );
                    }
                    Ok(terminal_psi::StructuralOperationResult {
                        place,
                        structural_type: parameter.structural_type,
                        multiplicity: parameter.multiplicity,
                        qualifications: parameter.qualifications.clone(),
                        projected_qualifications: parameter.projected_qualifications.clone(),
                        additional_input_qualifications: parameter
                            .additional_input_qualifications
                            .to_vec(),
                        qualification_establishments: Vec::new(),
                        claims: arrival_claims.to_vec(),
                    })
                }
            }
            pub(crate) mod completions {

                use super::super::super::super::{StructuralAccess, StructuralArgument};
                use super::super::super::{LoweringError, PlaceId, unsupported};
                use super::{
                    CompletionOccurrence, Emission, NormalDeathReceipt, ValueCompletion,
                    ValueContinuation,
                };
                use crate::emission::claim_bindings::completed_value_paths;
                use crate::expression_preparation::bindings::LoanAccess;
                use crate::shared::checked_facts::flow_facts;

                impl Emission<'_, '_, '_> {
                    pub(crate) fn finish_value_completion(
                        &mut self,
                        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                        value: checked_trees::CheckedStructuralValueHandle,
                        produced: ValueCompletion,
                    ) -> Result<
                        (
                            super::super::operand_claims::CompletedOperand,
                            NormalDeathReceipt,
                        ),
                        LoweringError,
                    > {
                        let (mut argument, occurrence, previous) = match produced {
                            ValueCompletion::NoNormal => {
                                return super::unsupported(
                                    "normal value completion has no arriving operand",
                                );
                            }
                            ValueCompletion::Awaiting {
                                argument,
                                occurrence,
                            } => (argument, occurrence, None),
                            ValueCompletion::Complete { argument, receipt } => {
                                let occurrence = receipt.occurrence;
                                (argument, occurrence, Some(receipt))
                            }
                        };
                        let node = self.structural_value_node(value);
                        if occurrence.value != value
                            || occurrence.first_new_place > *self.environment.next_place
                            || node.machine != self.machine
                            || node.state != self.state
                            || node.statement_index != self.statement
                        {
                            return unsupported(
                                "normal death belongs to another completed value occurrence",
                            );
                        }
                        if self
                            .evaluation
                            .retired_structural_homes
                            .contains(&argument.place)
                        {
                            return unsupported(
                                "normal death has a retired completed value source",
                            );
                        }
                        if let Some(receipt) = previous {
                            if receipt.source != argument
                                || receipt.block != self.evaluation.current
                            {
                                return unsupported(
                                    "normal-death receipt names another source or execution frame",
                                );
                            }
                            let completed = self.capture_operand(
                                identities,
                                argument,
                                node.expression,
                                occurrence.first_new_place,
                            )?;
                            return Ok((completed, receipt));
                        }
                        if argument.access == StructuralAccess::Owned && argument.path.is_empty() {
                            self.completed(identities, argument.place, occurrence, None)?;
                        }
                        if let Some(transaction) = self.content_transformation(&node)? {
                            argument = self.transform_content(identities, argument, transaction)?;
                            self.completed(identities, argument.place, occurrence, None)?;
                        }
                        let completed = self.capture_operand(
                            identities,
                            argument.clone(),
                            node.expression,
                            occurrence.first_new_place,
                        )?;
                        self.complete_value_cleanup(identities, node.expression)?;
                        if self
                            .evaluation
                            .retired_structural_homes
                            .contains(&argument.place)
                        {
                            return unsupported("normal death retired its completed result source");
                        }
                        let receipt = NormalDeathReceipt {
                            occurrence,
                            source: argument,
                            block: self.evaluation.current,
                        };
                        Ok((completed, receipt))
                    }

                    pub(crate) fn finish_value_arrival(
                        &mut self,
                        completed: super::super::operand_claims::CompletedOperand,
                        receipt: NormalDeathReceipt,
                        continuation: Option<&mut ValueContinuation>,
                    ) -> Result<ValueCompletion, LoweringError> {
                        if receipt.source != completed.argument
                            || receipt.block != self.evaluation.current
                            || self
                                .evaluation
                                .retired_structural_homes
                                .contains(&completed.argument.place)
                        {
                            return unsupported(
                                "completed value arrival lost its normal-death source",
                            );
                        }
                        let (argument, block) = if let Some(continuation) = continuation {
                            let receiving_access = continuation
                                .structural_parameters
                                .first()
                                .ok_or(LoweringError::Unsupported(
                                    "completed value arrival has no receiving source slot",
                                ))?
                                .access;
                            self.complete_value(completed, continuation)?;
                            (
                                StructuralArgument::whole(continuation.place, receiving_access),
                                continuation.block,
                            )
                        } else {
                            (completed.argument, self.evaluation.current)
                        };
                        Ok(ValueCompletion::Complete {
                            receipt: NormalDeathReceipt {
                                occurrence: receipt.occurrence,
                                source: argument.clone(),
                                block,
                            },
                            argument,
                        })
                    }

                    pub(crate) fn complete_value_cleanup(
                        &mut self,
                        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                        expression: typed_trees::expression::ExpressionHandle,
                    ) -> Result<(), LoweringError> {
                        let control = &flow_facts(self.checked)
                            .terminal_unit_effects
                            .cleanup_control;
                        let Some(completion) = control.value_completion(
                            self.machine,
                            self.state,
                            self.statement,
                            expression,
                        ) else {
                            return Ok(());
                        };
                        let actions = crate::unit::unit_cleanup::expression_cleanup_regions(
                            identities,
                            self.checked,
                            control.actions.span_or_empty(completion.actions),
                            control.partition_refs.span_or_empty(completion.partitions),
                            &self.evaluation.structural_locals,
                            |place| self.evaluation.carrier_multiplicity(place, self.operations),
                            self.calls,
                        )?;
                        let mut retired = Vec::new();
                        for action in actions.iter().filter_map(|region| region.action.physical()) {
                            let place = action.place();
                            if action.path().is_empty() && !retired.contains(&place) {
                                retired.push(place);
                            }
                        }
                        self.evaluation.continue_cleanup_regions(
                            identities,
                            self.checked,
                            actions,
                            self.values,
                            self.next_value,
                            self.next_block,
                            self.next_edge,
                            self.operations,
                        )?;
                        for place in retired {
                            self.evaluation.retire_structural_home(place);
                        }
                        Ok(())
                    }

                    pub(crate) fn admit_fresh_operand(
                        &mut self,
                        place: PlaceId,
                        completion: CompletionOccurrence,
                    ) -> Result<(), LoweringError> {
                        if self.evaluation.current_claims.contains(place) {
                            return Ok(());
                        }
                        if place.get() < completion.first_new_place {
                            return super::super::super::unsupported(
                                "completed operand is absent from its execution frame",
                            );
                        }
                        let argument = StructuralArgument {
                            place,
                            path: Vec::new(),
                            access: StructuralAccess::Owned,
                        };
                        let claims = match self.operations.declared_source_claims(&argument)? {
                            Some(claims) => claims,
                            None if completed_value_paths(self.checked, completion.value)?
                                .is_empty() =>
                            {
                                Vec::new()
                            }
                            None => {
                                return super::super::super::unsupported(
                                    "fresh completed operand has no producer custody namespace",
                                );
                            }
                        };
                        self.evaluation.current_claims.publish(place, claims)
                    }

                    pub(crate) fn completed(
                        &mut self,
                        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                        place: PlaceId,
                        completion: CompletionOccurrence,
                        continuation: Option<&mut ValueContinuation>,
                    ) -> Result<PlaceId, LoweringError> {
                        self.admit_fresh_operand(place, completion)?;
                        if place.get() >= completion.first_new_place
                            && self.operations.structural_establishments.contains(&place)
                        {
                            self.operations.register_structural_materialization(
                                place,
                                self.state,
                                self.statement,
                            )?;
                        }
                        if let Some(continuation) = continuation {
                            self.complete_operand(
                                identities,
                                StructuralArgument {
                                    place,
                                    path: Vec::new(),
                                    access: self.result_access,
                                },
                                completion,
                                continuation,
                            )?;
                        }
                        Ok(place)
                    }

                    pub(crate) fn complete_operand(
                        &mut self,
                        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                        selected: StructuralArgument,
                        completion: CompletionOccurrence,
                        continuation: &mut ValueContinuation,
                    ) -> Result<(), LoweringError> {
                        let expression = self.structural_value_node(completion.value).expression;
                        let completed = self.capture_operand(
                            identities,
                            selected,
                            expression,
                            completion.first_new_place,
                        )?;
                        self.complete_value(completed, continuation)
                    }

                    pub(crate) fn complete_value(
                        &mut self,
                        completed: super::super::operand_claims::CompletedOperand,
                        continuation: &mut ValueContinuation,
                    ) -> Result<(), LoweringError> {
                        let super::super::operand_claims::CompletedOperand {
                            argument: selected,
                            claims,
                        } = completed;
                        let structural_arguments = vec![selected];
                        if structural_arguments.len() != continuation.structural_parameters.len() {
                            return unsupported(
                                "structural value has unequal source and receiving slots at its join",
                            );
                        }
                        if let Some(common) = &mut continuation.common_claims {
                            common.retain(|claim| claims.contains(claim));
                        } else {
                            continuation.common_claims = Some(claims);
                        }
                        self.join_memberships(&structural_arguments, continuation)?;
                        self.operations
                            .reference_backings
                            .forward(&structural_arguments[0], continuation.place);
                        let edge = self.edge(
                            continuation.block,
                            self.values.iter().map(|value| value.id).collect(),
                            structural_arguments,
                        )?;
                        self.bind_continuation_names(continuation, edge.edge)?;
                        self.evaluation.retire_consumed_structural_arguments(
                            &edge.structural_arguments,
                            self.operations,
                        )?;
                        self.finish_jump(edge, Vec::new());
                        Ok(())
                    }

                    pub(crate) fn complete_loan(
                        &mut self,
                        argument: StructuralArgument,
                        continuation: &mut ValueContinuation,
                        loan: LoanAccess,
                    ) -> Result<(), LoweringError> {
                        if continuation.structural_parameters.len() != 1 {
                            return unsupported(match loan {
                                LoanAccess::Shared => {
                                    "borrowed selection joined residual ownership"
                                }
                                LoanAccess::Mutable => {
                                    "mutable borrowed selection joined residual ownership"
                                }
                            });
                        }
                        self.join_memberships(std::slice::from_ref(&argument), continuation)?;
                        self.operations
                            .reference_backings
                            .record(continuation.place, argument.clone());
                        let edge = self.edge(
                            continuation.block,
                            self.values.iter().map(|value| value.id).collect(),
                            vec![argument],
                        )?;
                        self.bind_continuation_names(continuation, edge.edge)?;
                        self.finish_jump(edge, Vec::new());
                        Ok(())
                    }

                    fn bind_continuation_names(
                        &mut self,
                        continuation: &mut ValueContinuation,
                        edge: semantic_vocabulary::EdgeId,
                    ) -> Result<(), LoweringError> {
                        let aliases = crate::emission::claim_bindings::selections::edge_bindings(
                            self.checked,
                            &continuation.claim_destinations,
                            &self.selection_route,
                            edge,
                            &self.calls.claims,
                        )?;
                        self.evaluation.current_claims =
                            self.evaluation.current_claims.with_edge_aliases(&aliases)?;
                        if let Some(current) = &mut continuation.current_claims {
                            current.meet(&self.evaluation.current_claims)?;
                        } else {
                            continuation.current_claims =
                                Some(self.evaluation.current_claims.clone());
                        }
                        self.operations.claim_edge_bindings.extend(aliases);
                        Ok(())
                    }

                    fn join_memberships(
                        &self,
                        arguments: &[StructuralArgument],
                        continuation: &mut ValueContinuation,
                    ) -> Result<(), LoweringError> {
                        if arguments.len() != continuation.structural_parameters.len() {
                            return unsupported(
                                "membership join has unequal source and receiving slots",
                            );
                        }
                        for (argument, parameter) in arguments
                            .iter()
                            .zip(&mut continuation.structural_parameters)
                        {
                            let incoming = self.transported_parameter(
                                argument,
                                parameter.place,
                                parameter.position,
                                parameter.structural_type,
                                parameter.multiplicity,
                            )?;
                            if continuation.has_incoming_memberships {
                                parameter
                                    .qualifications
                                    .retain(|domain| incoming.qualifications.contains(domain));
                                parameter.projected_qualifications.retain(|domain| {
                                    incoming.projected_qualifications.contains(domain)
                                });
                                parameter.additional_input_qualifications.retain(|row| {
                                    incoming.additional_input_qualifications.contains(row)
                                        && if row.path.is_empty() {
                                            parameter.qualifications.contains(&row.domain)
                                        } else {
                                            parameter.projected_qualifications.contains(row)
                                        }
                                });
                            } else {
                                parameter.qualifications = incoming.qualifications;
                                parameter.projected_qualifications =
                                    incoming.projected_qualifications;
                                parameter.additional_input_qualifications =
                                    incoming.additional_input_qualifications;
                            }
                            if let Some(owner) = continuation
                                .remaining_owners
                                .iter_mut()
                                .find(|owner| owner.value.place == parameter.place)
                            {
                                owner.value.qualifications = parameter.qualifications.clone();
                                owner.value.projected_qualifications =
                                    parameter.projected_qualifications.clone();
                                owner.value.additional_input_qualifications =
                                    parameter.additional_input_qualifications.clone();
                            }
                        }
                        continuation.has_incoming_memberships = true;
                        Ok(())
                    }
                }
            }
            pub(crate) mod content_transformations {

                use crate::shared::terminal_identities::lookup_claim_id;
                pub(crate) mod boundary_correspondence {

                    use super::{Emission, LoweringError, source_path, unsupported};
                    use crate::shared::checked_facts::flow_facts;
                    use arena::Handle;
                    use checked_trees::facts::{
                        ContentBodyAvailableInput, ContentBodyClaimBinding, ContentBodyCustody,
                        ContentBodyDerivation,
                    };
                    use language_semantics::content::{
                        ContentConservationTerm as SourceTerm, ContentPlaceRoot as SourceRoot,
                    };
                    use semantic_vocabulary::{ContentTerm, StructuralPlaceKind};
                    use terminal_psi::{ContentClaimProjection, ContentTransformationCustody};

                    impl Emission<'_, '_, '_> {
                        pub(crate) fn boundary_body_custody(
                            &self,
                            derived: &ContentBodyDerivation,
                            inputs: &[(
                                Handle<ContentBodyAvailableInput>,
                                ContentClaimProjection,
                            )],
                            outputs: &[(Handle<ContentBodyClaimBinding>, ContentClaimProjection)],
                        ) -> Result<ContentTransformationCustody, LoweringError>
                        {
                            let ContentBodyCustody::Boundary {
                                requirements,
                                addresses,
                            } = &derived.custody
                            else {
                                return Ok(ContentTransformationCustody::Derived);
                            };
                            let content = &self.checked.facts.qualifications.content;
                            let equation = content.body_conservations.get(derived.conservation);
                            // Psi05 authenticates the source law and completed frame before
                            // publishing Boundary custody. Lower its original formal positions.
                            let body_law = &content.conservation_plans[equation.plan_index];
                            let mut retained_requirements = Vec::new();
                            for requirement in content
                                .boundary_body_requirements
                                .span_or_empty(*requirements)
                            {
                                let boundary = self
                                    .calls
                                    .boundaries
                                    .iter()
                                    .find(|boundary| boundary.source == requirement.requirement)
                                    .ok_or(LoweringError::Unsupported(
                                        "boundary body custody lost its exact public declaration",
                                    ))?;
                                let source_law = content
                                    .boundary_requirement_plans
                                    .get(requirement.conservation_plan_index)
                                    .ok_or(LoweringError::Unsupported(
                                        "boundary body custody lost its retained public law",
                                    ))?;
                                let lowered = crate::proofs::content_conservation::lower_content_conservation_plan(
                                    source_law,
                                )?;
                                let semantic_vocabulary::Proposition::ContentConservation(
                                    conservation,
                                ) = lowered.proposition
                                else {
                                    return unsupported(
                                        "boundary body law is not a content equation",
                                    );
                                };
                                let guarantee = terminal_psi::ContentConservationGuarantee {
                                    report_fingerprint: lowered.source_report_fingerprint,
                                    structural_places: lowered.structural_places,
                                    conservation,
                                };
                                let mut substitutions = Vec::new();
                                let mut source_terms = Vec::new();
                                projections(body_law.equation.left(), &mut source_terms);
                                projections(body_law.equation.right(), &mut source_terms);
                                let mut public_terms = Vec::new();
                                portable_projections(
                                    guarantee.conservation.left(),
                                    &mut public_terms,
                                );
                                portable_projections(
                                    guarantee.conservation.right(),
                                    &mut public_terms,
                                );
                                for (projection, subject) in public_terms {
                                    let projection = *projection;
                                    let kind = guarantee
                                        .structural_places
                                        .iter()
                                        .find(|place| place.id == subject.root)
                                        .ok_or(LoweringError::Unsupported(
                                            "boundary body law has no declared subject root",
                                        ))?
                                        .kind;
                                    let mut targets = Vec::new();
                                    for source in &source_terms {
                                        let SourceTerm::Projection {
                                            semantic_domain,
                                            subject: source_subject,
                                            ..
                                        } = source
                                        else {
                                            continue;
                                        };
                                        let source_segments = source_subject
                                            .segments
                                            .iter()
                                            .map(|segment| match segment {
                                                language_semantics::content::ContentPlaceSegment::Field(field) => {
                                                    semantic_vocabulary::ContentPlaceSegment::Field(
                                                        field.name.clone(),
                                                    )
                                                }
                                                language_semantics::content::ContentPlaceSegment::Case(case) => {
                                                    semantic_vocabulary::ContentPlaceSegment::Case(
                                                        case.name.clone(),
                                                    )
                                                }
                                                language_semantics::content::ContentPlaceSegment::FixedIndex(
                                                    index,
                                                ) => semantic_vocabulary::ContentPlaceSegment::FixedIndex(*index),
                                            })
                                            .collect::<Vec<_>>();
                                        if source_segments != subject.segments
                                            || u64::from(semantic_domain.0)
                                                != projection.domain.get()
                                        {
                                            continue;
                                        }
                                        match (&kind, &source_subject.root) {
                                            (
                                                StructuralPlaceKind::Parameter { position },
                                                SourceRoot::Parameter {
                                                    position: source_position,
                                                    symbol,
                                                    ..
                                                },
                                            ) if position == source_position => {
                                                let parameter = self.body_parameter_at_completion(
                                                    equation, *symbol,
                                                )?;
                                                for (available, target) in inputs {
                                                    let available = content
                                                        .body_available_inputs
                                                        .get(*available);
                                                    let [entry] = content
                                                        .body_claim_bindings
                                                        .span_or_empty(available.entry_bindings)
                                                    else {
                                                        return unsupported(
                                                            "boundary body input needs an exact original projection correspondence, not only joint ancestry",
                                                        );
                                                    };
                                                    if entry.parameter_symbol == parameter
                                                        && target.projection == projection
                                                        && source_path(
                                                            self.checked,
                                                            flow_facts(self.checked)
                                                                .ownership
                                                                .segments
                                                                .span_or_empty(entry.segments),
                                                        )? == subject.segments
                                                    {
                                                        targets.push(target.clone());
                                                    }
                                                }
                                            }
                                            (StructuralPlaceKind::Result, SourceRoot::Result) => {
                                                targets.extend(
                                                    outputs
                                                        .iter()
                                                        .filter(|(_, target)| {
                                                            target.projection == projection
                                                                && target.subject.segments
                                                                    == subject.segments
                                                        })
                                                        .map(|(_, target)| target.clone()),
                                                );
                                            }
                                            _ => {}
                                        }
                                    }
                                    targets.sort();
                                    targets.dedup();
                                    let target =
                                        terminal_semantics::content_transformations::claim_term(
                                            &targets,
                                        )
                                        .map_err(LoweringError::Unsupported)?;
                                    substitutions.push(
                                        terminal_psi::ContentBodyProjectionSubstitution {
                                            source: ContentTerm::Projection {
                                                projection,
                                                subject: subject.clone(),
                                            },
                                            target,
                                        },
                                    );
                                }
                                substitutions.sort();
                                substitutions.dedup();
                                retained_requirements.push(
                                    terminal_psi::ContentBoundaryBodyRequirement {
                                        boundary: boundary.id,
                                        guarantee,
                                        substitutions,
                                    },
                                );
                            }
                            let mut retained_addresses = Vec::new();
                            for address in content.backing_addresses.span_or_empty(*addresses) {
                                let input = inputs
                                    .iter()
                                    .find(|(handle, _)| *handle == address.input)
                                    .map(|(_, binding)| binding)
                                    .ok_or(LoweringError::Unsupported(
                                        "backing input is absent from the exact custody transaction",
                                    ))?;
                                let output = outputs
                                    .iter()
                                    .find(|(handle, _)| *handle == address.output)
                                    .map(|(_, binding)| binding)
                                    .ok_or(LoweringError::Unsupported(
                                        "backing output is absent from the exact custody transaction",
                                    ))?;
                                let mut input_subject = input.subject.clone();
                                input_subject.segments.extend(source_path(
                                    self.checked,
                                    flow_facts(self.checked)
                                        .ownership
                                        .segments
                                        .span_or_empty(address.input_segments),
                                )?);
                                let mut output_subject = output.subject.clone();
                                output_subject.segments.extend(source_path(
                                    self.checked,
                                    flow_facts(self.checked)
                                        .ownership
                                        .segments
                                        .span_or_empty(address.output_segments),
                                )?);
                                retained_addresses.push(
                                    terminal_psi::ContentBackingAddressTransport {
                                        input: terminal_psi::ContentClaimAddress {
                                            claim: input.claim,
                                            subject: input_subject,
                                        },
                                        output: terminal_psi::ContentClaimAddress {
                                            claim: output.claim,
                                            subject: output_subject,
                                        },
                                    },
                                );
                            }
                            retained_requirements.sort();
                            retained_requirements.dedup();
                            retained_addresses.sort();
                            retained_addresses.dedup();
                            Ok(ContentTransformationCustody::Boundary {
                                requirements: retained_requirements,
                                addresses: retained_addresses,
                            })
                        }

                        fn body_parameter_at_completion(
                            &self,
                            equation: &checked_trees::facts::ContentBodyConservationFact,
                            parameter: symbols::SymbolHandle,
                        ) -> Result<symbols::SymbolHandle, LoweringError> {
                            let law = &self.checked.facts.qualifications.content.conservation_plans
                                [equation.plan_index];
                            if law.callable == equation.state_symbol {
                                return Ok(parameter);
                            }
                            let flow = flow_facts(self.checked)
                                .control
                                .state(equation.machine_symbol, equation.state_symbol)
                                .ok_or(LoweringError::Unsupported(
                                    "boundary body has no completion state flow",
                                ))?;
                            let mut origins = flow_facts(self.checked)
                                .control
                                .exits
                                .span_or_empty(flow.exits)
                                .iter()
                                .flat_map(|exit| {
                                    flow_facts(self.checked)
                                        .control
                                        .exit_parameter_origins
                                        .span_or_empty(exit.parameter_origins)
                                })
                                .filter(|origin| origin.entry_parameter == parameter);
                            let selected = origins
                                .next()
                                .ok_or(LoweringError::Unsupported(
                                    "boundary body formal has no proved completion origin",
                                ))?
                                .state_parameter;
                            if !selected.is_valid()
                                || origins.any(|origin| origin.state_parameter != selected)
                            {
                                return unsupported(
                                    "boundary body formal has ambiguous completion origins",
                                );
                            }
                            Ok(selected)
                        }
                    }

                    fn projections<'a>(term: &'a SourceTerm, output: &mut Vec<&'a SourceTerm>) {
                        match term {
                            SourceTerm::Projection { .. } => output.push(term),
                            SourceTerm::Separate(children) => {
                                for child in children {
                                    projections(child, output);
                                }
                            }
                        }
                    }

                    fn portable_projections<'a>(
                        term: &'a ContentTerm,
                        output: &mut Vec<(
                            &'a semantic_vocabulary::ContentProjectionIdentity,
                            &'a semantic_vocabulary::ContentStructuralPlace,
                        )>,
                    ) {
                        match term {
                            ContentTerm::Projection {
                                projection,
                                subject,
                            } => output.push((projection, subject)),
                            ContentTerm::Separate(children) => {
                                for child in children {
                                    portable_projections(child, output);
                                }
                            }
                        }
                    }
                }

                use super::{Emission, LoweringError, StructuralArgument, unsupported};
                use crate::emission::operation_emission::place_declarations::operation_result_place;
                use crate::shared::checked_facts::flow_facts;
                use semantic_vocabulary::{
                    ContentPlaceSegment, ContentPlaceVersion, ContentStructuralPlace,
                };
                use terminal_psi::{
                    ContentClaimProjection, ContentTransformationDerivation,
                    StructuralPathQualification,
                };

                fn source_path(
                    checked: &checked_trees::CheckedTrees,
                    segments: &[facts::PlaceSegment],
                ) -> Result<Vec<ContentPlaceSegment>, LoweringError> {
                    use checked_trees::data::DataMember;
                    segments
                        .iter()
                        .map(|segment| match segment {
                            facts::PlaceSegment::FixedIndex { index } => Ok(
                                ContentPlaceSegment::FixedIndex(u64::try_from(*index).map_err(
                                    |_| LoweringError::Unsupported("content path index overflows"),
                                )?),
                            ),
                            facts::PlaceSegment::Field { symbol } => {
                                for definition in checked.data_definitions() {
                                    for member in checked.data_members(definition) {
                                        match member {
                                            DataMember::Field(field) if field.symbol == *symbol => {
                                                return Ok(ContentPlaceSegment::Field(
                                                    field.path_identity(),
                                                ));
                                            }
                                            DataMember::Variant(variant) => {
                                                if let Some(field) = checked
                                                    .data_payload_fields(variant)
                                                    .iter()
                                                    .find(|field| field.symbol == *symbol)
                                                {
                                                    return Ok(ContentPlaceSegment::Field(
                                                        field.path_identity(),
                                                    ));
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                unsupported("content field has no declared member identity")
                            }
                            facts::PlaceSegment::Case { variant: symbol } => {
                                for definition in checked.data_definitions() {
                                    for member in checked.data_members(definition) {
                                        if let DataMember::Variant(variant) = member
                                            && variant.symbol == *symbol
                                        {
                                            return Ok(ContentPlaceSegment::Case(
                                                variant.path_identity(),
                                            ));
                                        }
                                    }
                                }
                                unsupported("content case has no declared member identity")
                            }
                            _ => unsupported(
                                "content transformation requires an exact finite owned path",
                            ),
                        })
                        .collect()
                }

                impl Emission<'_, '_, '_> {
                    pub(crate) fn content_transformation(
                        &self,
                        node: &checked_trees::CheckedStructuralValue,
                    ) -> Result<
                        Option<checked_trees::facts::ContentBodyTransformation>,
                        LoweringError,
                    > {
                        let content = &self.checked.facts.qualifications.content;
                        if !node.content_transformation.is_valid() {
                            return Ok(None);
                        }
                        if !content
                            .body_transformations
                            .is_valid(node.content_transformation)
                        {
                            return unsupported(
                                "content value lost its exact completion transaction",
                            );
                        }
                        Ok(Some(
                            content
                                .body_transformations
                                .get(node.content_transformation)
                                .clone(),
                        ))
                    }

                    pub(crate) fn transform_content(
                        &mut self,
                        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                        source: StructuralArgument,
                        transaction: checked_trees::facts::ContentBodyTransformation,
                    ) -> Result<StructuralArgument, LoweringError> {
                        use terminal_psi::{
                            Operation, OperationKind, OperationResult, StructuralAccess,
                            StructuralOperationResult,
                        };
                        let content = &self.checked.facts.qualifications.content;
                        let ownership = &self.checked.facts.flow.ownership;
                        let place =
                            super::place_id(super::allocate_dense(self.environment.next_place)?);
                        let operation = self.operations.allocate();
                        if source.access != StructuralAccess::Owned || !source.path.is_empty() {
                            return unsupported(
                                "content formation requires its complete owned payload",
                            );
                        }
                        let claims = self.evaluation.current_claims.source(&source)?.ok_or(
                            LoweringError::Unsupported("content payload has no completed custody"),
                        )?;
                        let (whole, projected, _) = self
                            .completed_memberships(source.place)
                            .ok_or(LoweringError::Unsupported(
                                "content payload lacks its completed membership roster",
                            ))?;
                        let mut qualifications = whole.to_vec();
                        let mut projected_qualifications = projected.to_vec();
                        let mut parents = Vec::new();
                        let mut derivations = Vec::new();
                        let (_, authored) = crate::shared::declarations::state_declaration(
                            self.checked,
                            self.state,
                        )?;
                        let source_parameters = self.checked.state_parameters(authored);
                        for derived in content
                            .body_derivations
                            .span_or_empty(transaction.derivations)
                        {
                            let equation = content.body_conservations.get(derived.conservation);
                            let mut inputs = Vec::new();
                            let mut outputs = Vec::new();
                            let mut algebra = None;
                            let mut input_bindings = Vec::new();
                            let mut output_bindings = Vec::new();
                            for available_handle in equation.available_inputs.handles() {
                                let available = content.body_available_inputs.get(available_handle);
                                let binding = content.body_claim_bindings.get(available.binding);
                                let (mut argument, carrier) =
                                    crate::expression_preparation::bindings::owned_structural_home(
                                        available.root,
                                        source_parameters,
                                        &self.evaluation.structural_parameters,
                                        &self.evaluation.structural_locals,
                                    )?;
                                let root_carrier =
                                    crate::expression_preparation::bindings::published_structural_type(
                                        &self.evaluation.structural_parameters,
                                        &self.evaluation.structural_locals,
                                        self.environment.private_places,
                                        argument.place,
                                    )
                                    .ok_or(LoweringError::Unsupported(
                                        "content alias root carrier is absent",
                                    ))?;
                                let mut segments =
                                    terminal_semantics::content_transformations::content_path(
                                        self.structural_types.iter(),
                                        &identities.snapshot(),
                                        root_carrier,
                                        &argument.path,
                                    )
                                    .ok_or(
                                        LoweringError::Unsupported(
                                            "content alias requires an exact typed owned path",
                                        ),
                                    )?;
                                let relative = source_path(
                                    self.checked,
                                    ownership.segments.span_or_empty(available.segments),
                                )?;
                                let relative_path =
                                    terminal_semantics::content_transformations::structural_path(
                                        self.structural_types.iter(),
                                        &identities.snapshot(),
                                        carrier,
                                        &relative,
                                    )
                                    .ok_or(
                                        LoweringError::Unsupported(
                                            "content parent path has no typed carrier",
                                        ),
                                    )?;
                                argument.path.extend(relative_path);
                                segments.extend(relative);
                                if !parents.contains(&argument) {
                                    parents.push(argument.clone());
                                }
                                let projection =
                                    self.content_projection(binding.semantic_domain)?;
                                algebra = Some(projection.algebra.clone());
                                let input = ContentClaimProjection {
                                    claim: lookup_claim_id(
                                        self.calls.claims.bindings(),
                                        binding.claim_identity,
                                    )?,
                                    projection: projection.identity,
                                    subject: ContentStructuralPlace {
                                        version: ContentPlaceVersion::Current,
                                        root: argument.place,
                                        segments,
                                    },
                                };
                                input_bindings.push((available_handle, input.clone()));
                                inputs.push(input);
                            }
                            for binding_handle in equation.outputs.handles() {
                                let binding = content.body_claim_bindings.get(binding_handle);
                                let segments = source_path(
                                    self.checked,
                                    ownership.segments.span_or_empty(binding.segments),
                                )?;
                                let path =
                                    terminal_semantics::content_transformations::structural_path(
                                        self.structural_types.iter(),
                                        &identities.snapshot(),
                                        self.structural_type,
                                        &segments,
                                    )
                                    .ok_or(
                                        LoweringError::Unsupported(
                                            "content result path has no typed carrier",
                                        ),
                                    )?;
                                let projection =
                                    self.content_projection(binding.semantic_domain)?;
                                let domain = self
                                    .domain_declarations
                                    .iter()
                                    .find(|domain| {
                                        domain.content_projection.as_ref() == Some(projection)
                                    })
                                    .ok_or(LoweringError::Unsupported(
                                        "derived membership has no exact domain",
                                    ))?;
                                if matches!(
                                    derived.custody,
                                    checked_trees::facts::ContentBodyCustody::Derived
                                ) {
                                    if path.is_empty() {
                                        qualifications.push(domain.id);
                                    } else {
                                        projected_qualifications.push(
                                            StructuralPathQualification {
                                                path,
                                                domain: domain.id,
                                            },
                                        );
                                    }
                                }
                                let output = ContentClaimProjection {
                                    claim: lookup_claim_id(
                                        self.calls.claims.bindings(),
                                        binding.claim_identity,
                                    )?,
                                    projection: projection.identity,
                                    subject: ContentStructuralPlace {
                                        version: ContentPlaceVersion::Current,
                                        root: place,
                                        segments,
                                    },
                                };
                                output_bindings.push((binding_handle, output.clone()));
                                outputs.push(output);
                            }
                            let custody = self.boundary_body_custody(
                                derived,
                                &input_bindings,
                                &output_bindings,
                            )?;
                            inputs.sort();
                            outputs.sort();
                            let conservation = semantic_vocabulary::ContentConservation::new(
                                algebra.ok_or(LoweringError::Unsupported(
                                    "content transformation has no input algebra",
                                ))?,
                                terminal_semantics::content_transformations::claim_term(&inputs)
                                    .map_err(LoweringError::Unsupported)?,
                                terminal_semantics::content_transformations::claim_term(&outputs)
                                    .map_err(LoweringError::Unsupported)?,
                            );
                            derivations.push(ContentTransformationDerivation {
                                conservation,
                                lineage_roots: {
                                    let mut roots = content
                                        .body_claim_bindings
                                        .span_or_empty(derived.lineage_roots)
                                        .iter()
                                        .map(|binding| {
                                            lookup_claim_id(
                                                self.calls.claims.bindings(),
                                                binding.claim_identity,
                                            )
                                        })
                                        .collect::<Result<Vec<_>, LoweringError>>()?;
                                    roots.sort();
                                    roots.dedup();
                                    roots
                                },
                                ancestry: self.content_ancestry(derived.conservation)?,
                                custody,
                                inputs,
                                outputs,
                            });
                        }
                        qualifications.sort();
                        qualifications.dedup();
                        projected_qualifications.sort();
                        projected_qualifications.dedup();
                        derivations.sort();
                        for consumed in parents.iter().chain(std::iter::once(&source)) {
                            if self.evaluation.current_claims.contains(consumed.place) {
                                self.evaluation.current_claims.replace(
                                    consumed.place,
                                    &consumed.path,
                                    &[],
                                )?;
                            }
                        }
                        self.evaluation.retire_consumed_structural_arguments(
                            parents.iter().chain(std::iter::once(&source)),
                            self.operations,
                        )?;
                        self.operations.push(Operation::unsuspended(
                            operation,
                            OperationResult::Structural(StructuralOperationResult {
                                place,
                                structural_type: self.structural_type,
                                multiplicity: self.multiplicity,
                                qualifications,
                                projected_qualifications,
                                additional_input_qualifications: Vec::new(),
                                qualification_establishments: Vec::new(),
                                claims,
                            }),
                            OperationKind::TransformContent {
                                source,
                                parents,
                                derivations,
                            },
                        ));
                        self.environment.private_places.push(operation_result_place(
                            place,
                            operation,
                            self.structural_type,
                        ));
                        self.operations.register_structural_materialization(
                            place,
                            self.state,
                            self.statement,
                        )?;
                        Ok(StructuralArgument::whole(place, StructuralAccess::Owned))
                    }

                    fn content_ancestry(
                        &self,
                        conservation: arena::Handle<
                            checked_trees::facts::ContentBodyConservationFact,
                        >,
                    ) -> Result<Vec<terminal_psi::ContentLineageStep>, LoweringError>
                    {
                        use checked_trees::facts::ContentClaimTransformationSource;
                        let content = &self.checked.facts.qualifications.content;
                        let equation = content.body_conservations.get(conservation);
                        let completed = flow_facts(self.checked)
                            .control
                            .operands
                            .get(equation.completed_value);
                        let mut retained = Vec::new();
                        let frontier = content
                            .descendant_frontier(
                                equation.inputs,
                                equation.machine_symbol,
                                equation.state_symbol,
                                completed.statement_index,
                                equation.evaluation,
                                conservation,
                            )
                            .map_err(LoweringError::Unsupported)?;
                        for (handle, _, _) in frontier.replacements {
                            if !retained.contains(&handle) {
                                retained.push(handle);
                            }
                        }
                        let mut ancestry = Vec::new();
                        for handle in retained {
                            let transformation = content.claim_transformations.get(handle);
                            let claims = |bindings: arena::HandleSpan<
                                checked_trees::facts::ContentBodyClaimBinding,
                            >| {
                                let mut claims = content
                                    .body_claim_bindings
                                    .span_or_empty(bindings)
                                    .iter()
                                    .map(|binding| {
                                        lookup_claim_id(
                                            self.calls.claims.bindings(),
                                            binding.claim_identity,
                                        )
                                    })
                                    .collect::<Result<Vec<_>, LoweringError>>()?;
                                claims.sort();
                                Ok::<_, LoweringError>(claims)
                            };
                            let inputs = claims(transformation.inputs)?;
                            let outputs = claims(transformation.outputs)?;
                            let producer_operation = match transformation.source {
                                ContentClaimTransformationSource::Call { call_ordinal, .. } => {
                                    let mut calls =
                                        self.operations.source_calls.iter().filter(|call| {
                                            call.matches_statement(
                                                transformation.state_symbol,
                                                transformation.statement_index,
                                                call_ordinal,
                                            )
                                        });
                                    let call = calls.next().ok_or(LoweringError::Unsupported(
                                        "content ancestry call was not emitted",
                                    ))?;
                                    if calls.next().is_some() {
                                        return unsupported(
                                            "content ancestry call has ambiguous emission",
                                        );
                                    }
                                    let lowered_psi::LoweredSourceCallSite::Operation(operation) =
                                        call.terminal_site
                                    else {
                                        return unsupported(
                                            "content ancestry cannot come from a nonreturning exit",
                                        );
                                    };
                                    operation
                                }
                                ContentClaimTransformationSource::Body { .. } => {
                                    let mut producers = self.operations.iter().filter(|operation| {
                                        matches!(&operation.kind, terminal_psi::OperationKind::TransformContent { derivations, .. }
                                            if derivations.iter().any(|derived| derived.outputs.iter().map(|output| output.claim).collect::<Vec<_>>() == outputs))
                                    });
                                    let producer =
                                        producers.next().ok_or(LoweringError::Unsupported(
                                            "content ancestry transformation was not emitted",
                                        ))?;
                                    if producers.next().is_some() {
                                        return unsupported(
                                            "content ancestry transformation has ambiguous emission",
                                        );
                                    }
                                    producer.id
                                }
                                ContentClaimTransformationSource::Unknown => {
                                    return unsupported("content ancestry has no producer");
                                }
                            };
                            ancestry.push(terminal_psi::ContentLineageStep {
                                producer_operation,
                                inputs,
                                outputs,
                            });
                        }
                        Ok(ancestry)
                    }

                    fn content_projection(
                        &self,
                        domain: language_semantics::SemanticDomainId,
                    ) -> Result<&terminal_psi::StructuralContentProjection, LoweringError>
                    {
                        self.domain_declarations
                            .iter()
                            .filter_map(|domain| domain.content_projection.as_ref())
                            .find(|projection| {
                                projection.identity.domain.get() == u64::from(domain.0)
                            })
                            .ok_or(LoweringError::Unsupported(
                                "content transformation has no retained owner projection",
                            ))
                    }
                }
            }
            pub(crate) mod copied_place {

                use super::{Emission, LoweringError, StructuralArgument};
                use crate::emission::operation_emission::expressions::{
                    emit_element_length, emit_scalar_leaf,
                };
                use checked_trees::CheckedUnitStructuralArgumentPlan;
                use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};
                use terminal_psi::{OperationKind, StructuralPathSegment, StructuralTypeShape};

                impl Emission<'_, '_, '_> {
                    pub(crate) fn readable_place_argument(
                        &mut self,
                        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                        expression: checked_trees::expression::ExpressionHandle,
                        source: &CheckedUnitStructuralArgumentPlan,
                    ) -> Result<StructuralArgument, LoweringError> {
                        let completed = self
                            .projection_emission()
                            .readable_place_argument(identities, expression, source)?;
                        let argument = completed.argument.clone();
                        self.completed_projections.push(completed);
                        Ok(argument)
                    }

                    pub(crate) fn copied_place_path(
                        &mut self,
                        argument: &StructuralArgument,
                    ) -> Result<Vec<StructuralPathSegment>, LoweringError> {
                        let mut path = argument.path.clone();
                        if !matches!(
                            path.first(),
                            Some(
                                StructuralPathSegment::FixedIndex(_)
                                    | StructuralPathSegment::RuntimeIndex { .. }
                            )
                        ) {
                            return Ok(path);
                        }
                        let root_type = self.projection_root_type(argument.place)?;
                        let declaration =
                            terminal_psi::find_structural_type(self.structural_types, root_type)
                                .ok_or(LoweringError::Unsupported(
                                    "indexed copy root type is absent from the catalog",
                                ))?;
                        if !matches!(declaration.shape, StructuralTypeShape::ElementView { .. }) {
                            return Ok(path);
                        }
                        if let Some(StructuralPathSegment::FixedIndex(index)) = path.first() {
                            let integer_type = IntegerType::new(IntegerSign::Unsigned, 64)
                                .map_err(|_| {
                                    LoweringError::Unsupported(
                                        "view index has no unsigned count carrier",
                                    )
                                })?;
                            let index = emit_scalar_leaf(
                                OperationKind::IntegerConstant {
                                    value: IntegerValue::Unsigned(u128::from(*index)),
                                },
                                ScalarType::Integer(integer_type),
                                self.next_value,
                                self.operations,
                            );
                            path[0] = StructuralPathSegment::RuntimeIndex {
                                index,
                                obligation: self.calls.allocate_requirement()?,
                            };
                        }
                        emit_element_length(
                            terminal_psi::StructuralArgument::whole(
                                argument.place,
                                terminal_psi::StructuralAccess::SharedBorrow,
                            ),
                            self.next_value,
                            self.operations,
                        );
                        Ok(path)
                    }
                }

                impl super::completed_projection::ProjectionEmission<'_, '_, '_> {
                    pub(crate) fn readable_place_argument(
                        &mut self,
                        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                        expression: checked_trees::expression::ExpressionHandle,
                        source: &CheckedUnitStructuralArgumentPlan,
                    ) -> Result<super::completed_projection::CompletedProjection, LoweringError>
                    {
                        if !matches!(
                            source.access,
                            checked_trees::CheckedStructuralAccess::SharedBorrow
                                | checked_trees::CheckedStructuralAccess::MutableBorrow
                        ) {
                            return crate::shared::lowering_error::unsupported(
                                "storage projection requires readable source custody",
                            );
                        }
                        let authored = crate::shared::declarations::state_declaration(
                            self.environment.checked,
                            self.environment.state,
                        )?;
                        let bindings =
                            crate::expression_preparation::bindings::ScalarBindings::new(
                                self.values.iter().map(|value| value.scalar_type),
                            )
                            .with_callables_from(&self.evaluation.scalar_bindings)
                            .with_structural_parameters(&self.evaluation.structural_parameters)
                            .with_structural_locals(&self.evaluation.structural_locals)
                            .with_structural_observations(self.environment.structural_types)
                            .with_place_observations(&self.evaluation.place_observations);
                        let parameters = self.environment.checked.state_parameters(authored.1);
                        let resolved = bindings.resolve_borrowed_argument(
                            self.environment.checked,
                            source,
                            parameters,
                        )?;
                        let home = &resolved.source;
                        let root = bindings.structural_argument_type(home).ok_or(
                            LoweringError::Unsupported(
                                "readable projection lost its published root type",
                            ),
                        )?;
                        let prepared = resolved.prepare_path(
                            identities,
                            self.environment.checked,
                            root,
                            bindings.structural_declarations(),
                        )?;
                        let prepared = home.path.iter().cloned().map(
                            crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment::Static,
                        ).chain(prepared).collect::<Vec<_>>();
                        self.complete_projection(
                identities,
                expression,
                home.place,
                prepared,
                crate::expression_preparation::bindings::structural_paths::terminal_access(
                    source.access,
                ),
            )
                    }
                }
            }
            pub(crate) mod current_memberships {
                use super::{Emission, LoweringError, StructuralArgument};
                use crate::shared::checked_facts::flow_facts;
                use terminal_psi::{StructuralPathQualification, StructuralPathSegment};

                pub(crate) mod descriptor_bindings {
                    use arena::Handle;
                    use checked_trees::{CheckedTrees, FlowOperandFact};

                    pub(crate) fn unchanged(
                        checked: &CheckedTrees,
                        operand: Handle<FlowOperandFact>,
                    ) -> bool {
                        let control = &checked.facts.flow.control;
                        if !control.operands.is_valid(operand) {
                            return false;
                        }
                        let completed = control.operands.get(operand);
                        if completed.entry_binding_unknown_write {
                            return false;
                        }
                        let Some(writes) = control
                            .operand_origin_writes
                            .span(completed.entry_binding_writes)
                        else {
                            return false;
                        };
                        let mut receiver = completed.backing;
                        let mut visited = vec![operand];
                        while receiver.is_valid() {
                            if !control.operands.is_valid(receiver) || visited.contains(&receiver) {
                                return false;
                            }
                            visited.push(receiver);
                            let captured = control.operands.get(receiver);
                            if captured.state_symbol != completed.state_symbol
                                || captured.statement_index != completed.statement_index
                            {
                                return false;
                            }
                            if validation::reference_shell(&checked.typed, captured.type_reference)
                                .is_some()
                            {
                                if captured.entry_binding_unknown_write {
                                    return false;
                                }
                                let Some(before) = control
                                    .operand_origin_writes
                                    .span(captured.entry_binding_writes)
                                else {
                                    return false;
                                };
                                if !writes.starts_with(before) {
                                    return false;
                                }
                                let Some(subject) =
                                    validation::storage_place_from_expression_in_state(
                                        &checked.typed,
                                        captured.state_symbol,
                                        captured.statement_index,
                                        captured.expression,
                                    )
                                else {
                                    return false;
                                };
                                for write in &writes[before.len()..] {
                                    if write.root != subject.root {
                                        continue;
                                    }
                                    let Some(segments) =
                                        control.operand_referent_segments.span(write.segments)
                                    else {
                                        return false;
                                    };
                                    if validation::storage_path_relations::binding_overwrite(
                                        &checked.typed,
                                        &subject,
                                        segments,
                                    ) {
                                        return false;
                                    }
                                }
                            }
                            receiver = captured.backing;
                        }
                        true
                    }
                }

                type Memberships = (
                    Vec<semantic_vocabulary::StructuralDomainId>,
                    Vec<StructuralPathQualification>,
                );

                impl Emission<'_, '_, '_> {
                    pub(crate) fn refresh_memberships(
                        &mut self,
                        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                        source: StructuralArgument,
                        expression: typed_trees::expression::ExpressionHandle,
                        first_new_place: u64,
                    ) -> Result<StructuralArgument, LoweringError> {
                        if source.place.get() >= first_new_place {
                            return Ok(source);
                        }
                        let current =
                            self.occurrence_memberships(identities, &source, expression)?;
                        let (whole, projected, _) = self
                            .completed_memberships(source.place)
                            .ok_or(LoweringError::Unsupported(
                                "completed source membership contract disappeared",
                            ))?;
                        let born =
                            StructuralPathQualification::select(whole, projected, &source.path);
                        if current == born {
                            return Ok(source);
                        }
                        if self.multiplicity == super::StructuralMultiplicity::Unrestricted {
                            let place = self.copied_leaf_place(
                                identities,
                                &source,
                                arena::HandleSpan::empty(),
                                expression,
                            )?;
                            Ok(StructuralArgument::whole(
                                place,
                                super::StructuralAccess::Owned,
                            ))
                        } else {
                            let completed = self.capture_operand_claims(
                                identities,
                                source,
                                expression,
                                first_new_place,
                            )?;
                            self.move_completed_operand(completed, current.0, current.1)
                        }
                    }

                    pub(crate) fn occurrence_memberships(
                        &self,
                        identities: &terminal_psi::TerminalIdentityTableBuilder,
                        source: &StructuralArgument,
                        expression: typed_trees::expression::ExpressionHandle,
                    ) -> Result<Memberships, LoweringError> {
                        let (whole, projected, _) = self
                            .completed_memberships(source.place)
                            .ok_or(LoweringError::Unsupported(
                                "completed source has no published memberships",
                            ))?;
                        let (born_whole, born_projected) =
                            StructuralPathQualification::select(whole, projected, &source.path);
                        let mut whole = born_whole.clone();
                        let mut projected = born_projected.clone();
                        for candidate in self.operations.call_membership_candidates(
                            source.place,
                            self.state,
                            self.statement,
                        ) {
                            let (added_whole, added_projected) =
                                StructuralPathQualification::select(
                                    &candidate.qualifications,
                                    &candidate.projected,
                                    &source.path,
                                );
                            whole.extend(added_whole);
                            projected.extend(added_projected);
                        }
                        whole.sort();
                        whole.dedup();
                        projected.sort();
                        projected.dedup();
                        if whole.is_empty() && projected.is_empty() {
                            return Ok((whole, projected));
                        }
                        let projection =
                            self.completed_projections.iter().rev().find(|projection| {
                                projection.expression == expression
                                    && projection.argument == *source
                            });
                        let source_root =
                            projection.map_or(source.place, |projection| projection.source_root);
                        let projection_captures = projection
                            .map(|_| {
                                validation::completed_storage::consumer_captures(
                                    &self.checked.facts.flow.control,
                                    self.state,
                                    self.statement as usize,
                                    expression,
                                )
                                .ok_or(LoweringError::Unsupported(
                                    "completed projection has no exact consumer capture",
                                ))
                            })
                            .transpose()?;
                        if projection_captures.as_ref().is_some_and(|captures| {
                            !captures.iter().all(|capture| {
                                descriptor_bindings::unchanged(self.checked, *capture)
                            })
                        }) {
                            return Ok((Vec::new(), Vec::new()));
                        }
                        let is_local = self
                            .evaluation
                            .structural_locals
                            .iter()
                            .any(|local| local.argument.place == source_root);
                        let is_ingress = self
                            .evaluation
                            .structural_parameters
                            .iter()
                            .any(|(_, parameter)| parameter.place == source_root);
                        if !is_local && !is_ingress {
                            if self
                                .operations
                                .structural_materialization_statement(source_root, self.state)?
                                == Some(self.statement)
                            {
                                return Ok((born_whole, born_projected));
                            }
                            return Err(LoweringError::at_statement(
                                self.checked,
                                self.machine,
                                self.state,
                                self.statement,
                                "membership source lacks its completed physical/source correspondence",
                            ));
                        }
                        let subject = validation::storage_place_from_expression_in_state(
                            &self.checked.typed,
                            self.state,
                            self.statement as usize,
                            expression,
                        )
                        .ok_or(LoweringError::Unsupported(
                            "resident membership source has no canonical observation",
                        ))?;
                        let (prefix, covered) = if let Some(local) =
                            self.evaluation.structural_locals.iter().find(|local| {
                                local.argument.place == source_root && local.root == subject.root
                            }) {
                            (local.argument.path.as_slice(), 0)
                        } else if let Some(pair) = self
                            .evaluation
                            .structural_locals
                            .iter()
                            .filter(|local| local.argument.place == source_root)
                            .find_map(|local| {
                                self.saved_subject_prefix(local.root, &subject)
                                    .map(|covered| (local.argument.path.as_slice(), covered))
                            })
                        {
                            pair
                        } else {
                            let (_, state) = crate::shared::declarations::state_declaration(
                                self.checked,
                                self.state,
                            )?;
                            self.evaluation
                                .structural_parameters
                                .iter()
                                .any(|(index, parameter)| {
                                    parameter.place == source_root
                                        && self
                                            .checked
                                            .state_parameters(state)
                                            .get(*index as usize)
                                            .is_some_and(|parameter| {
                                                subject.root
                                                    == facts::PlaceRoot::Symbol(parameter.symbol)
                                            })
                                })
                                .then_some((&[][..], 0))
                                .ok_or(LoweringError::Unsupported(
                                    "resident membership home has no exact source projection",
                                ))?
                        };
                        if projection.is_none()
                            && !source.path.strip_prefix(prefix).is_some_and(|suffix| {
                                self.membership_path_matches(
                                    identities,
                                    suffix,
                                    &subject.segments[covered..],
                                )
                            })
                        {
                            return Err(LoweringError::Unsupported(
                                "resident membership physical and source paths differ",
                            ));
                        }
                        let captures = match projection_captures {
                            Some(captures) => captures,
                            None => validation::completed_storage::consumer_captures(
                                &self.checked.facts.flow.control,
                                self.state,
                                self.statement as usize,
                                expression,
                            )
                            .ok_or(LoweringError::Unsupported(
                                "resident membership read has no exact completed consumer",
                            ))?,
                        };
                        let birth = self
                            .operations
                            .structural_materialization_statement(source.place, self.state)?
                            .and_then(|birth| {
                                flow_facts(self.checked).control.states.iter().find_map(
                                    |(_, state)| {
                                        (state.state_symbol == self.state)
                                            .then(|| {
                                                flow_facts(self.checked)
                                                    .control
                                                    .statements
                                                    .span_or_empty(state.statements)
                                                    .iter()
                                                    .find(|statement| {
                                                        statement.statement_index == birth as usize
                                                    })
                                                    .map(|statement| statement.storage_snapshot)
                                            })
                                            .flatten()
                                    },
                                )
                            });
                        let crosses_reference = source
                            .path
                            .iter()
                            .any(|segment| matches!(segment, StructuralPathSegment::Referent))
                            || self
                                .evaluation
                                .structural_locals
                                .iter()
                                .filter(|local| {
                                    local.argument.place == source.place
                                        && local.root == subject.root
                                })
                                .any(|local| {
                                    local.argument.path.iter().any(|segment| {
                                        matches!(segment, StructuralPathSegment::Referent)
                                    })
                                });
                        let source_type = validation::canonical_place_type_reference(
                            &self.checked.typed,
                            self.state,
                            self.statement as usize,
                            &subject,
                        )
                        .ok_or(LoweringError::Unsupported(
                            "membership source lost its exact declared carrier",
                        ))?;
                        let current =
                            |domain,
                             path: &[StructuralPathSegment],
                             relative: &[facts::PlaceSegment]| {
                                let born = if path.is_empty() {
                                    born_whole.contains(&domain)
                                } else {
                                    born_projected
                                        .iter()
                                        .any(|row| row.domain == domain && row.path == path)
                                };
                                let Some(binding) = self
                                    .domain_ids
                                    .iter()
                                    .find(|binding| binding.domain == domain)
                                else {
                                    return false;
                                };
                                let domain_symbols = self
                                    .checked
                                    .domain_definitions()
                                    .iter()
                                    .filter(|definition| {
                                        self.checked
                                            .domain_definition_for_instance(
                                                definition.symbol,
                                                binding.semantic_domain,
                                            )
                                            .is_some()
                                    })
                                    .map(|definition| definition.symbol)
                                    .collect::<Vec<_>>();
                                let mut target = subject.clone();
                                target.segments.extend_from_slice(relative);
                                captures.iter().all(|capture| {
                                let operand = self.checked.facts.flow.control.operands.get(*capture);
                                if born
                                    && !crosses_reference
                                    && !path
                                        .iter()
                                        .any(|segment| matches!(segment, StructuralPathSegment::Referent))
                                    && birth.is_some_and(|birth| {
                                        same_contents(
                                            &self.checked.typed,
                                            &self.checked.facts.flow.control,
                                            birth,
                                            operand.storage_snapshot,
                                            &target,
                                        )
                                    })
                                {
                                    return true;
                                }
                                let contexts = flow_facts(self.checked)
                                    .contexts
                                    .semantic_constraint_contexts(operand.constraints)
                                    .collect::<Vec<_>>();
                                let evidence = validation::membership_evidence::MembershipEvidence::prepare(
                                    &self.checked.typed,
                                    &self.checked.facts.semantic,
                                    &contexts,
                                );
                                domain_symbols.iter().any(|domain_symbol| {
                                    evidence.contains(&target, *domain_symbol, binding.semantic_domain)
                                        || validation::completed_storage::completed_membership(
                                            &self.checked.typed,
                                            &self.checked.facts,
                                            *capture,
                                            relative,
                                            *domain_symbol,
                                            binding.semantic_domain,
                                        )
                                })
                            })
                            };
                        whole.retain(|domain| current(*domain, &[], &[]));
                        let mut retained = Vec::with_capacity(projected.len());
                        for row in projected {
                            let relative =
                                self.membership_source_path(identities, source_type, &row.path)?;
                            if current(row.domain, &row.path, &relative) {
                                retained.push(row);
                            }
                        }
                        Ok((whole, retained))
                    }

                    fn saved_subject_prefix(
                        &self,
                        root: facts::PlaceRoot,
                        subject: &validation::CanonicalPlace,
                    ) -> Option<usize> {
                        let symbol = root.as_symbol()?;
                        let (_, state) = crate::shared::declarations::state_declaration(
                            self.checked,
                            self.state,
                        )
                        .ok()?;
                        let (_, local) = self.checked.state_local_declaration(state, symbol)?;
                        if local.is_mutable || !local.initial_value.is_valid() {
                            return None;
                        }
                        let bound = validation::storage_place_from_expression_in_state(
                            &self.checked.typed,
                            self.state,
                            self.statement as usize,
                            local.initial_value,
                        )?;
                        (bound.root == subject.root
                            && subject.segments.starts_with(&bound.segments))
                        .then_some(bound.segments.len())
                    }

                    fn membership_source_path(
                        &self,
                        identities: &terminal_psi::TerminalIdentityTableBuilder,
                        source_type: typed_trees::types::TypeReferenceHandle,
                        path: &[StructuralPathSegment],
                    ) -> Result<Vec<facts::PlaceSegment>, LoweringError> {
                        use checked_trees::CheckedUnitStructuralPathSegment as Segment;
                        let mut checked_path = Vec::with_capacity(path.len());
                        for segment in path {
                            checked_path.push(match segment {
                                StructuralPathSegment::Field(identity) => Segment::Field(
                                    checked_trees::CheckedMemberIdentity::of_spelling(
                                        identities.text(*identity).ok_or(
                                            LoweringError::Unsupported(
                                                "membership field identity is absent",
                                            ),
                                        )?,
                                    ),
                                ),
                                StructuralPathSegment::Case(selected) => {
                                    let case = self
                                        .structural_types
                                        .iter()
                                        .flat_map(|declaration| {
                                            declaration.shape.sum_cases().into_iter().flatten()
                                        })
                                        .find(|case| case.id == *selected)
                                        .ok_or(LoweringError::Unsupported(
                                            "membership case identity is absent",
                                        ))?;
                                    Segment::Case(
                                        checked_trees::CheckedMemberIdentity::of_spelling(
                                            identities.text(case.identity).ok_or(
                                                LoweringError::Unsupported(
                                                    "membership case spelling is absent",
                                                ),
                                            )?,
                                        ),
                                    )
                                }
                                StructuralPathSegment::FixedIndex(index) => {
                                    Segment::FixedIndex(*index)
                                }
                                StructuralPathSegment::AllElements => Segment::AllElements,
                                StructuralPathSegment::Referent => Segment::Referent,
                                _ => {
                                    return Err(LoweringError::Unsupported(
                                        "membership has a nonstatic path coordinate",
                                    ));
                                }
                            });
                        }
                        validation::canonical_qualification_path(
                            &self.checked.typed,
                            source_type,
                            &checked_path,
                        )
                        .map_err(LoweringError::Unsupported)
                    }

                    fn membership_path_matches(
                        &self,
                        identities: &terminal_psi::TerminalIdentityTableBuilder,
                        physical: &[StructuralPathSegment],
                        source: &[facts::PlaceSegment],
                    ) -> bool {
                        let physical = physical
                            .iter()
                            .filter(|segment| !matches!(segment, StructuralPathSegment::Referent))
                            .collect::<Vec<_>>();
                        physical.len() == source.len()
                            && physical.into_iter().zip(source).all(|(physical, source)| {
                                match (physical, source) {
                                    (
                                        StructuralPathSegment::Field(identity),
                                        facts::PlaceSegment::Field { symbol },
                                    ) => validation::structural_member_identity(
                                        &self.checked.typed,
                                        *symbol,
                                    )
                                    .is_some_and(|member| {
                                        identities.text(*identity)
                                            == Some(member.spelling().as_str())
                                    }),
                                    (
                                        StructuralPathSegment::Case(selected),
                                        facts::PlaceSegment::Case { variant },
                                    ) => {
                                        let identity = self
                                            .structural_types
                                            .iter()
                                            .flat_map(|declaration| {
                                                declaration.shape.sum_cases().into_iter().flatten()
                                            })
                                            .find(|case| case.id == *selected)
                                            .and_then(|case| identities.text(case.identity));
                                        validation::structural_member_identity(
                                            &self.checked.typed,
                                            *variant,
                                        )
                                        .is_some_and(
                                            |member| identity == Some(member.spelling().as_str()),
                                        )
                                    }
                                    (
                                        StructuralPathSegment::FixedIndex(index),
                                        facts::PlaceSegment::FixedIndex { index: source },
                                    ) => usize::try_from(*index).ok() == Some(*source),
                                    (
                                        StructuralPathSegment::AllElements,
                                        facts::PlaceSegment::FixedRange {
                                            start: 0,
                                            end: usize::MAX,
                                        },
                                    ) => true,
                                    _ => false,
                                }
                            })
                    }
                }

                fn same_contents(
                    program: &dyn validation::CheckingInput,
                    control: &checked_trees::FlowControlFacts,
                    birth: arena::Handle<checked_trees::FlowStorageSnapshot>,
                    current: arena::Handle<checked_trees::FlowStorageSnapshot>,
                    subject: &validation::CanonicalPlace,
                ) -> bool {
                    use validation::completed_storage::{StoredObservation, at_snapshot};
                    let read = |cut| {
                        at_snapshot(program, control, cut, subject, StoredObservation::Contents)?
                            .captures()
                    };
                    let (Some(before), Some(after)) = (read(birth), read(current)) else {
                        return false;
                    };
                    !before.is_empty()
                        && before.len() == after.len()
                        && before.iter().all(|left| {
                            after.iter().any(|right| {
                                left.capture == right.capture
                                    && left.reference == right.reference
                                    && left.projection == right.projection
                            })
                        })
                }
            }
            pub(crate) mod dispatch {

                use super::super::super::{
                    CheckedScalarExpressionRole, LoweringError, OperationKind, PlaceId, ScalarType,
                    unsupported,
                };
                use super::{Emission, ValueContinuation, ValueOutcome};
                use crate::emission::claim_bindings::selections::SelectedDecision;
                use crate::emission::operation_emission::integer::LoweredIntegerComparisonKind;
                use crate::shared::checked_facts::value_facts;

                impl Emission<'_, '_, '_> {
                    pub(crate) fn dispatch(
                        &mut self,
                        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                        value: checked_trees::CheckedStructuralValueHandle,
                        expression: checked_trees::expression::ExpressionHandle,
                        subject: checked_trees::CheckedDispatchSubject,
                        arms: arena::HandleSpan<checked_trees::CheckedStructuralDispatchArm>,
                        continuation: Option<&mut ValueContinuation>,
                    ) -> Result<ValueOutcome<PlaceId>, LoweringError> {
                        let completed_prefix = self.evaluation.completed_operand_scope.len();
                        let result = self.dispatch_value(
                            identities,
                            value,
                            expression,
                            subject,
                            arms,
                            continuation,
                        );
                        self.evaluation
                            .completed_operand_scope
                            .truncate(completed_prefix);
                        result
                    }

                    fn dispatch_value(
                        &mut self,
                        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                        value: checked_trees::CheckedStructuralValueHandle,
                        expression: checked_trees::expression::ExpressionHandle,
                        subject: checked_trees::CheckedDispatchSubject,
                        arms: arena::HandleSpan<checked_trees::CheckedStructuralDispatchArm>,
                        continuation: Option<&mut ValueContinuation>,
                    ) -> Result<ValueOutcome<PlaceId>, LoweringError> {
                        let arms = value_facts(self.checked)
                            .structural_values
                            .dispatch_arms
                            .span(arms)
                            .ok_or(LoweringError::Unsupported(
                                "structural dispatch arm span is stale",
                            ))?
                            .to_vec();
                        let source_count = self.values.len();
                        match subject {
                            checked_trees::CheckedDispatchSubject::Scalar(subject) => {
                                let value = self.scalar(
                                    identities,
                                    CheckedScalarExpressionRole::StructuralValueSubject {
                                        expression,
                                    },
                                    subject,
                                    source_count,
                                )?;
                                self.values.push(value);
                            }
                            checked_trees::CheckedDispatchSubject::Structural {
                                expression: subject_expression,
                                value,
                                alias,
                                place,
                            } => {
                                if !value.is_valid() {
                                    let captured = super::place_id(super::allocate_dense(
                                        self.environment.next_place,
                                    )?);
                                    self.projection_emission().capture_subject(
                                        identities,
                                        subject_expression,
                                        &place,
                                        alias,
                                        captured,
                                    )?;
                                } else {
                                    let request = crate::scalar_lowering::branch_states::structural_values::ValueRequest::prepare(identities, self.checked, value, self.structural_types)?;
                                    self.environment.produce_value(
                                        identities,
                                        value,
                                        request.structural_type,
                                        self.evaluation,
                                        self.values,
                                        self.next_value,
                                        self.next_block,
                                        self.next_edge,
                                        self.calls,
                                        self.operations,
                                    )?;
                                    if self.operations.invocation_completion.is_some() {
                                        return Ok(ValueOutcome::NoNormal);
                                    }
                                    self.evaluation.bind_completed_value_alias(
                                        value,
                                        alias,
                                        self.operations,
                                    )?;
                                    // The actual producer above dominates every predicate and
                                    // selected arm. Carry that dependency into their fresh scalar
                                    // expansions, not every historical entry in computed_values.
                                    self.evaluation.completed_operand_scope.push(value);
                                }
                            }
                        }
                        let mut owned_continuation;
                        let is_root = continuation.is_none();
                        let continuation = if let Some(continuation) = continuation {
                            continuation
                        } else {
                            owned_continuation = self.root_value_continuation(
                                identities,
                                value,
                                self.values[..source_count].to_vec(),
                            )?;
                            owned_continuation.claim_destinations =
                                crate::emission::claim_bindings::selections::reserve(
                                    self.checked,
                                    self.machine,
                                    self.state,
                                    self.statement,
                                    expression,
                                    0,
                                    &mut self.calls.claims,
                                )?;
                            &mut owned_continuation
                        };
                        let route_prefix = self.selection_route.len();
                        let mut normal_arrivals = 0usize;
                        for (position, arm) in arms.iter().enumerate() {
                            let mut fallback = None;
                            if let checked_trees::CheckedScalarDispatchPattern::Predicate(
                                predicate,
                            ) = arm.pattern
                            {
                                let predicate = self.scalar(
                                    identities,
                                    CheckedScalarExpressionRole::StructuralValuePattern {
                                        source_arm: arm.source_arm,
                                    },
                                    predicate,
                                    source_count,
                                )?;
                                if predicate.scalar_type != ScalarType::Boolean {
                                    return unsupported(
                                        "structural dispatch predicate is not Boolean",
                                    );
                                }
                                if position + 1 < arms.len() {
                                    self.boolean_value_split(predicate.id, &mut fallback)?;
                                }
                            }
                            if let checked_trees::CheckedScalarDispatchPattern::Value(pattern) =
                                arm.pattern
                            {
                                let pattern = self.scalar(
                                    identities,
                                    CheckedScalarExpressionRole::StructuralValuePattern {
                                        source_arm: arm.source_arm,
                                    },
                                    pattern,
                                    source_count,
                                )?;
                                let subject = self.values[source_count];
                                if pattern.value_type() != subject.value_type()
                                    || arm.equality_use.is_valid()
                                {
                                    return unsupported(
                                        "structural dispatch comparison requires matching builtin scalar operands",
                                    );
                                }
                                if position + 1 < arms.len() {
                                    let kind = if subject.scalar_type == ScalarType::Boolean {
                                        OperationKind::BooleanEqual {
                                            left: subject.id,
                                            right: pattern.id,
                                        }
                                    } else if matches!(
                                        subject.scalar_type,
                                        ScalarType::Integer { .. }
                                    ) {
                                        LoweredIntegerComparisonKind::Equal
                                            .operation(subject.id, pattern.id)
                                    } else {
                                        return unsupported(
                                            "structural dispatch needs selected floating comparison lowering",
                                        );
                                    };
                                    self.boolean_case_split(kind, &mut fallback)?;
                                }
                            }
                            self.values.truncate(source_count);
                            if position + 1 < arms.len() {
                                self.selection_route.push(SelectedDecision {
                                    expression,
                                    decision: position,
                                    selected: true,
                                });
                            }
                            let emitted =
                                self.value(identities, arm.value, Some(&mut *continuation));
                            self.selection_route.truncate(route_prefix + position);
                            if position + 1 < arms.len() {
                                self.selection_route.push(SelectedDecision {
                                    expression,
                                    decision: position,
                                    selected: false,
                                });
                            }
                            match emitted? {
                                ValueOutcome::Normal(_) => normal_arrivals += 1,
                                ValueOutcome::NoNormal => {
                                    if fallback.is_some() || normal_arrivals != 0 {
                                        let terminator = self
                                            .operations
                                            .take_invocation_completion(self.next_edge)?
                                            .ok_or(LoweringError::Unsupported(
                                                "nonreturning value arm lost its invocation",
                                            ))?;
                                        self.finish(terminator);
                                    }
                                }
                            }
                            self.restore_fallback(fallback)?;
                        }
                        self.selection_route.truncate(route_prefix);
                        if normal_arrivals == 0 {
                            if is_root {
                                self.environment
                                    .private_places
                                    .retain(|declaration| declaration.id != continuation.place);
                                self.operations
                                    .block_entry_claims
                                    .retain(|claim| claim.input != continuation.place);
                            }
                            return Ok(ValueOutcome::NoNormal);
                        }
                        if is_root {
                            self.record_root_continuation(continuation)?;
                        }
                        Ok(ValueOutcome::Normal(continuation.place))
                    }
                }
            }
            pub(crate) mod element_views {

                use super::super::super::super::StructuralArgument;
                use super::super::super::{
                    CheckedScalarExpressionRole, LoweringError, PlaceId, StructuralMultiplicity,
                    allocate_dense, place_id, unsupported,
                };
                use super::{Emission, ValueContinuation};
                use crate::emission::operation_emission::expressions::{
                    LoweredDirectExpression, emit_direct_expression,
                };
                use crate::expression_preparation::bindings::LoanAccess;
                use crate::expression_preparation::prepare_expression::exact_u64_index;
                use crate::shared::terminal_identities::obligation_id;

                impl Emission<'_, '_, '_> {
                    pub(crate) fn direct_element_view(
                        &mut self,
                        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                        source: &checked_trees::CheckedUnitStructuralArgumentPlan,
                        region: &checked_trees::CheckedElementViewRegion,
                        expression: checked_trees::expression::ExpressionHandle,
                        loan: LoanAccess,
                    ) -> Result<PlaceId, LoweringError> {
                        let multiplicity = match loan {
                            LoanAccess::Shared => StructuralMultiplicity::Unrestricted,
                            LoanAccess::Mutable => StructuralMultiplicity::Affine,
                        };
                        self.establish_element_view(
                            identities,
                            source,
                            region,
                            expression,
                            loan,
                            multiplicity,
                        )
                    }

                    pub(crate) fn joined_element_view(
                        &mut self,
                        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                        source: &checked_trees::CheckedUnitStructuralArgumentPlan,
                        region: &checked_trees::CheckedElementViewRegion,
                        expression: checked_trees::expression::ExpressionHandle,
                        loan: LoanAccess,
                        continuation: Option<&mut ValueContinuation>,
                    ) -> Result<PlaceId, LoweringError> {
                        let Some(continuation) = continuation else {
                            return self
                                .direct_element_view(identities, source, region, expression, loan);
                        };
                        let transfers =
                            continuation
                                .structural_parameters
                                .first()
                                .is_some_and(|parameter| {
                                    parameter.access == terminal_psi::StructuralAccess::Owned
                                });
                        let multiplicity = match loan {
                            LoanAccess::Mutable if transfers => StructuralMultiplicity::Affine,
                            _ => StructuralMultiplicity::Unrestricted,
                        };
                        let place = self.establish_element_view(
                            identities,
                            source,
                            region,
                            expression,
                            loan,
                            multiplicity,
                        )?;
                        let argument = StructuralArgument {
                            place,
                            path: Vec::new(),
                            access: if transfers {
                                terminal_psi::StructuralAccess::Owned
                            } else {
                                loan.structural_access()
                            },
                        };
                        self.complete_loan(argument, continuation, loan)?;
                        Ok(continuation.place)
                    }

                    pub(crate) fn establish_element_view(
                        &mut self,
                        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                        source: &checked_trees::CheckedUnitStructuralArgumentPlan,
                        region: &checked_trees::CheckedElementViewRegion,
                        expression: checked_trees::expression::ExpressionHandle,
                        loan: LoanAccess,
                        multiplicity: StructuralMultiplicity,
                    ) -> Result<PlaceId, LoweringError> {
                        if let Some(range) = super::super::super::view_ranges::range(source) {
                            use super::super::super::view_ranges;
                            if !matches!(
                                region,
                                checked_trees::CheckedElementViewRegion::WholeCollection
                            ) {
                                return unsupported(
                                    "element range cannot replace a representation-byte region",
                                );
                            }
                            let bindings = self.structural_bindings();
                            let (bound, establishment) = view_ranges::stage_source(
                                identities,
                                self.checked,
                                self.state,
                                self.statement,
                                source,
                                self.structural_types,
                                &bindings,
                                self.environment.next_place,
                                self.values,
                                self.next_value,
                                self.operations,
                            )?;
                            if let Some(declaration) = establishment {
                                self.operations.register_structural_materialization(
                                    declaration.id,
                                    self.state,
                                    self.statement,
                                )?;
                                self.environment.private_places.push(declaration);
                            }
                            let place = place_id(allocate_dense(self.environment.next_place)?);
                            let declaration = view_ranges::emit(
                                identities,
                                self.checked,
                                view_ranges::ViewRangeSite {
                                    state: self.state,
                                    statement: self.statement,
                                    range: range.endpoints(),
                                },
                                bound,
                                self.structural_type,
                                place,
                                self.evaluation,
                                self.values.len(),
                                self.values,
                                self.next_value,
                                self.next_block,
                                self.next_edge,
                                self.operations,
                                self.calls,
                                self.environment,
                            )?;
                            self.operations.register_structural_materialization(
                                place,
                                self.state,
                                self.statement,
                            )?;
                            self.environment.private_places.push(declaration);
                            return Ok(place);
                        }
                        let element = super::super::super::view_ranges::element_type(
                            identities,
                            self.checked,
                            self.structural_type,
                            self.structural_types,
                        )?;
                        if source.access
                            != match loan {
                                LoanAccess::Shared => {
                                    checked_trees::CheckedStructuralAccess::SharedBorrow
                                }
                                LoanAccess::Mutable => {
                                    checked_trees::CheckedStructuralAccess::MutableBorrow
                                }
                            }
                        {
                            return unsupported("element view changes its supplying loan access");
                        }
                        let argument =
                            self.readable_place_argument(identities, expression, source)?;
                        let endpoints = match region {
                            checked_trees::CheckedElementViewRegion::WholeCollection
                            | checked_trees::CheckedElementViewRegion::WholeRepresentation => None,
                            checked_trees::CheckedElementViewRegion::RepresentationBytes {
                                start,
                                end,
                            } => {
                                let start = self.scalar(
                                    identities,
                                    CheckedScalarExpressionRole::RepresentationViewOffset {
                                        expression,
                                    },
                                    *start,
                                    self.sources.len(),
                                )?;
                                let index = exact_u64_index(LoweredDirectExpression::Parameter {
                                    position: 0,
                                    scalar_type: start.scalar_type,
                                })?;
                                let start = emit_direct_expression(
                                    &index,
                                    std::slice::from_ref(&start),
                                    self.next_value,
                                    self.operations,
                                );
                                let end = emit_direct_expression(
                                    &LoweredDirectExpression::IntegerLiteral {
                                        value: semantic_vocabulary::IntegerValue::Unsigned(
                                            (*end).into(),
                                        ),
                                        scalar_type:
                                            crate::emission::scalar_types::terminal_scalar_type(
                                                checked_trees::types::PrimitiveType::U64,
                                            )?,
                                    },
                                    &[],
                                    self.next_value,
                                    self.operations,
                                );
                                Some((start, end))
                            }
                        };
                        let source_length = if matches!(
                            region,
                            checked_trees::CheckedElementViewRegion::WholeRepresentation
                        ) {
                            Some(
                                crate::emission::operation_emission::expressions::emit_scalar_leaf(
                                    terminal_psi::OperationKind::ElementViewLength {
                                        source: argument.clone(),
                                    },
                                    crate::emission::scalar_types::terminal_scalar_type(
                                        checked_trees::types::PrimitiveType::U64,
                                    )?,
                                    self.next_value,
                                    self.operations,
                                ),
                            )
                        } else {
                            None
                        };
                        let id = self.operations.allocate();
                        let region = match endpoints {
                            None if matches!(
                                region,
                                checked_trees::CheckedElementViewRegion::WholeRepresentation
                            ) =>
                            {
                                terminal_psi::ElementViewRegion::WholeRepresentation {
                                    source_length: source_length.ok_or(LoweringError::Unsupported(
                                        "representation view lost its captured count",
                                    ))?,
                                    obligation: crate::shared::terminal_identities::obligation_id(
                                        id.get().checked_add(1).ok_or(LoweringError::Unsupported(
                                            "representation count obligation identity overflows",
                                        ))?,
                                    ),
                                }
                            }
                            None => terminal_psi::ElementViewRegion::WholeCollection,
                            Some((start, end)) => terminal_psi::ElementViewRegion::RepresentationBytes {
                                start,
                                end,
                                obligation: obligation_id(id.get().checked_add(1).ok_or(
                                    LoweringError::Unsupported("representation view obligation identity overflows"),
                                )?),
                            },
                        };
                        let place = place_id(allocate_dense(self.environment.next_place)?);
                        self.environment.private_places.push(
                            super::super::super::view_ranges::emit_view(
                                place,
                                self.structural_type,
                                argument,
                                element,
                                region,
                                multiplicity,
                                id,
                                self.operations,
                            ),
                        );
                        self.operations.register_structural_materialization(
                            place,
                            self.state,
                            self.statement,
                        )?;
                        self.evaluation.current_claims.publish(place, Vec::new())?;
                        Ok(place)
                    }
                }
            }
            pub(crate) mod parameters {

                use super::super::super::super::{
                    StructuralAccess, StructuralArgument, StructuralParameterDeclaration,
                };
                use super::super::super::{
                    LoweringError, PlaceId, StructuralMultiplicity, lookup_type_id, unsupported,
                };
                use super::{Emission, ProjectionSource};
                use crate::expression_preparation::bindings::StructuralLocalBinding;
                use crate::shared::terminal_identities::stored_structural_identity;
                use crate::unit::attached_unit::structural_values::ValueOutcome;
                use checked_trees::{CheckedStructuralValueHandle, CheckedStructuralValueKind};

                impl Emission<'_, '_, '_> {
                    pub(crate) fn parameter_source(
                        &self,
                        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
                    ) -> Result<Option<PlaceId>, LoweringError> {
                        let Some(declaration) = self.parameter_declaration(argument)? else {
                            return Ok(None);
                        };
                        if declaration.multiplicity == StructuralMultiplicity::Unrestricted {
                            return unsupported(
                                "owned selection parameter lacks whole owned custody",
                            );
                        }
                        Ok(Some(declaration.place))
                    }

                    pub(crate) fn copied_parameter(
                        &self,
                        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
                    ) -> Result<Option<StructuralArgument>, LoweringError> {
                        Ok(self
                            .parameter_declaration(argument)?
                            .filter(|declaration| {
                                declaration.multiplicity == StructuralMultiplicity::Unrestricted
                            })
                            .map(|declaration| StructuralArgument {
                                place: declaration.place,
                                path: Vec::new(),
                                access: StructuralAccess::Owned,
                            }))
                    }

                    pub(crate) fn parameter_declaration(
                        &self,
                        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
                    ) -> Result<Option<&StructuralParameterDeclaration>, LoweringError>
                    {
                        let (_, authored) = crate::shared::declarations::state_declaration(
                            self.checked,
                            self.state,
                        )?;
                        let parameters = self.checked.state_parameters(authored);
                        let (position, parameter) = match &argument.source {
                            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
                                let Some(entry) = parameters
                                    .iter()
                                    .enumerate()
                                    .find(|(_, parameter)| parameter.symbol == *symbol)
                                else {
                                    return Ok(None);
                                };
                                entry
                            }
                            checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                                parameter_index,
                            } => {
                                let Some(entry) = parameters
                                    .iter()
                                    .enumerate()
                                    .filter(|(_, parameter)| {
                                        !parameter.is_const
                                            && checked_trees::CheckedScalarType::from_type_reference(
                                                self.checked, parameter.type_reference,
                                            )
                                                .is_none()
                                    })
                                    .nth(*parameter_index as usize)
                                else {
                                    return unsupported("owned selection parameter index is out of range");
                                };
                                entry
                            }
                            _ => return Ok(None),
                        };
                        if parameter.is_const {
                            return unsupported("owned selection parameter is not runtime ingress");
                        }
                        let declaration = self
                            .evaluation
                            .structural_parameters
                            .iter()
                            .find(|(source_position, _)| *source_position as usize == position)
                            .map(|(_, declaration)| declaration)
                            .ok_or(LoweringError::Unsupported(
                                "owned selection parameter has no signature place",
                            ))?;
                        if declaration.access != StructuralAccess::Owned {
                            return unsupported("owned selection parameter lacks owned ingress");
                        }
                        Ok(Some(declaration))
                    }

                    pub(crate) fn projection_source(
                        &mut self,
                        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                        source: CheckedStructuralValueHandle,
                        path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
                        logical_root_type_identity: typed_trees::type_identity::TypeIdentityId,
                    ) -> Result<ValueOutcome<ProjectionSource>, LoweringError> {
                        let node = self.structural_value_node(source);
                        if super::structural_values(self.checked)
                            .root_for_value(source)
                            .is_some_and(|root| root.captured_copy.is_valid())
                        {
                            if node.machine != self.machine
                                || node.state != self.state
                                || node.statement_index != self.statement
                            {
                                return unsupported(
                                    "saved copied projection has a different checked source coordinate",
                                );
                            }
                            let saved = self
                                .evaluation
                                .saved_value_binding(source, self.operations)?;
                            return Ok(ValueOutcome::Normal(ProjectionSource {
                                root: saved.argument.place,
                                root_path: Vec::new(),
                                root_type_identity: stored_structural_identity(
                                    self.checked,
                                    node.type_reference,
                                ),
                                path,
                                logical_root_type_identity,
                            }));
                        }
                        let mut root_alias_path = Vec::new();
                        let (root, root_type_identity) = match node.kind {
                            CheckedStructuralValueKind::Place(argument) => {
                                if !argument.path.is_empty()
                                    || argument.access
                                        != checked_trees::CheckedStructuralAccess::Owned
                                {
                                    return unsupported(
                                        "projected selection root requires whole owned custody",
                                    );
                                }
                                let place = if let Some(place) = self.parameter_source(&argument)? {
                                    place
                                } else {
                                    let source = self
                                        .structural_bindings()
                                        .owned_argument_home(&argument)?;
                                    root_alias_path = source.path;
                                    source.place
                                };
                                (place, argument.type_identity)
                            }
                            _ => {
                                let root_type_identity =
                                    stored_structural_identity(self.checked, node.type_reference);
                                let root_type = lookup_type_id(self.type_ids, &root_type_identity)
                                    .map_err(|_| {
                                        LoweringError::MissingUnitStructuralType(format!(
                                            "projection source: {}",
                                            self.checked
                                                .type_identity_text(root_type_identity)
                                                .unwrap_or_else(|| format!(
                                                    "{root_type_identity:?}"
                                                )),
                                        ))
                                    })?;
                                let ValueOutcome::Normal(completed) = self
                                    .child_constructor_operand(
                                        identities,
                                        root_type,
                                        source,
                                        node.type_reference,
                                    )?
                                else {
                                    return Ok(ValueOutcome::NoNormal);
                                };
                                if !completed.argument.path.is_empty()
                                    || completed.argument.access != StructuralAccess::Owned
                                {
                                    return unsupported(
                                        "projected selection requires a completed whole owner",
                                    );
                                }
                                crate::expression_preparation::bindings::retain_completed_structural_homes(
                                    &mut self.evaluation.structural_locals,
                                    &[StructuralLocalBinding {
                                        result_binding_ordinal: None,
                                        root: facts::PlaceRoot::Expression(node.expression),
                                        argument: completed.argument.clone(),
                                        structural_type: root_type,
                                    }],
                                )?;
                                (completed.argument.place, root_type_identity)
                            }
                        };
                        Ok(ValueOutcome::Normal(ProjectionSource {
                            root,
                            root_path: root_alias_path,
                            path,
                            root_type_identity,
                            logical_root_type_identity,
                        }))
                    }
                }
            }
            pub(crate) mod quotient_calls {
                use super::*;
                use crate::shared::terminal_identities::stored_structural_identity;
                use crate::unit::attached_unit::structural_values::ValueOutcome;
                impl Emission<'_, '_, '_> {
                    pub(crate) fn quotient_call(
                        &mut self,
                        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                        value: CheckedStructuralValueHandle,
                        arguments: &[CheckedStructuralValueHandle],
                    ) -> Result<ValueOutcome<PlaceId>, LoweringError> {
                        let mut actuals = Vec::new();
                        for argument in arguments {
                            let child = self.structural_value_node(*argument);
                            let ty = lookup_type_id(
                                self.type_ids,
                                &stored_structural_identity(self.checked, child.type_reference),
                            )?;
                            let ValueOutcome::Normal(child) = self.child_constructor_operand(
                                identities,
                                ty,
                                *argument,
                                child.type_reference,
                            )?
                            else {
                                return Ok(ValueOutcome::NoNormal);
                            };
                            if !child.claims.is_empty() || !child.argument.path.is_empty() {
                                return unsupported("quotient invocation input carries custody");
                            }
                            actuals.push((child.argument, ty));
                        }
                        let place = place_id(allocate_dense(self.environment.next_place)?);
                        let declaration = crate::emission::quotient_values::emit_call(
                            self.checked,
                            value,
                            &actuals,
                            place,
                            self.structural_type,
                            self.next_value,
                            self.operations,
                            self.calls,
                        )?;
                        self.environment.private_places.push(declaration);
                        Ok(ValueOutcome::Normal(place))
                    }
                }
            }
            pub(crate) mod reference {

                use super::{
                    Emission, LoweringError, PlaceId, StructuralTypeShape, ValueContinuation,
                };
                use super::{lookup_type_id, unsupported};
                use crate::expression_preparation::bindings::LoanAccess;
                use crate::expression_preparation::bindings::StructuralLocalBinding;
                use crate::expression_preparation::bindings::structural_paths::structural_access_can_supply;
                use crate::shared::checked_facts::value_facts;
                use crate::shared::terminal_identities::stored_structural_identity;
                use crate::unit::attached_unit::structural_values::ValueOutcome;
                use checked_trees::CheckedUnitStructuralArgumentPlan;

                impl Emission<'_, '_, '_> {
                    pub(crate) fn reference_value(
                        &mut self,
                        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                        formation: super::super::super::reference_results::Formation<'_>,
                        source: &CheckedUnitStructuralArgumentPlan,
                        selectors: arena::HandleSpan<checked_trees::CheckedScalarComputationHandle>,
                        source_value: checked_trees::CheckedStructuralValueHandle,
                        storage: checked_trees::CheckedReferenceStorage,
                        completion: super::CompletionOccurrence,
                        continuation: Option<&mut ValueContinuation>,
                    ) -> Result<ValueOutcome<PlaceId>, LoweringError> {
                        self.evaluation.prepare_borrowed_argument(
                            identities,
                            self.checked,
                            self.state,
                            source,
                            self.values,
                            self.structural_types,
                            self.domain_declarations,
                            self.environment.next_place,
                            self.operations,
                            self.environment.private_places,
                        )?;
                        let (_, state) = crate::shared::declarations::state_declaration(
                            self.checked,
                            self.state,
                        )?;
                        let referent_type = lookup_type_id(self.type_ids, &source.type_identity)?;
                        let referent = terminal_psi::find_structural_type(
                            self.structural_types,
                            referent_type,
                        )
                        .ok_or(LoweringError::Unsupported(
                            "reference referent type is missing",
                        ))?;
                        if source_value.is_valid() {
                            let node = self.structural_value_node(source_value);
                            let root_type = lookup_type_id(
                                self.type_ids,
                                &stored_structural_identity(self.checked, node.type_reference),
                            )?;
                            let root = source.structural_storage_root().ok_or(
                                LoweringError::Unsupported(
                                    "computed reference backing has no storage identity",
                                ),
                            )?;
                            if self
                                .evaluation
                                .completed_source_binding(
                                    source_value,
                                    root,
                                    root_type,
                                    self.operations,
                                )?
                                .is_none()
                            {
                                // A genuinely unproduced backing completes through its own
                                // occurrence and publishes its claims before the loan forms.
                                let ValueOutcome::Normal(completed) = self
                                    .child_constructor_operand(
                                        identities,
                                        root_type,
                                        source_value,
                                        node.type_reference,
                                    )?
                                else {
                                    return Ok(ValueOutcome::NoNormal);
                                };
                                crate::expression_preparation::bindings::retain_completed_structural_homes(
                        &mut self.evaluation.structural_locals,
                        &[StructuralLocalBinding {
                            result_binding_ordinal: None,
                            root,
                            argument: completed.argument,
                            structural_type: root_type,
                        }],
                    )?;
                            }
                            // Cached reuse already proved the exact current root, argument
                            // and type. Publishing that same binding with no result alias
                            // would be an exact no-op; only a fresh producer publishes here.
                        }
                        let evaluations = value_facts(self.checked)
                            .scalar_computations
                            .operands
                            .span(selectors)
                            .ok_or(LoweringError::Unsupported(
                                "reference has stale selector evaluations",
                            ))?;
                        if source
                            .computation_operands(
                                &self.checked.facts.values.scalar_computations.operands,
                            )
                            .any(|root| !evaluations.contains(&root))
                        {
                            return unsupported(
                                "reference path has no retained selector evaluation",
                            );
                        }
                        let bindings = self
                            .evaluation
                            .contextual_bindings()
                            .with_computation_operands(
                                0,
                                self.values.iter().map(|value| value.scalar_type),
                            )
                            .with_structural_observations(self.structural_types);
                        let mut argument = match referent.shape {
                StructuralTypeShape::PrimitiveScalar(_)
                    if matches!(
                        source.source,
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::ScalarParameter { .. }
                    ) =>
                {
                    bindings.scalar_parameter_borrow(
                        identities,
                        self.checked,
                        source,
                        self.structural_types,
                    )?
                }
                StructuralTypeShape::PrimitiveScalar(scalar_type)
                    if matches!(
                        source.source,
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { .. }
                    ) =>
                {
                    bindings.primitive_borrow(
                        identities,
                        self.checked,
                        source,
                        scalar_type,
                        self.structural_types,
                        self.checked.state_parameters(state),
                    )?
                }
                _ => {
                    let argument = bindings.prepared_borrowed_argument(
                        identities,
                        self.checked,
                        source,
                        self.checked.state_parameters(state),
                        self.structural_types,
                    )?;
                    self.complete_projection(
                        identities,
                        formation.expression,
                        argument.source.known_place()?,
                        argument.path,
                        argument.access,
                    )?
                }
            };
                        let same_representation = self.structural_type == referent_type
                            || terminal_semantics::borrowed_referent_presentation(
                                self.structural_types.iter(),
                                referent_type,
                                self.structural_type,
                            ) == Some(
                                terminal_semantics::BorrowedReferentPresentation::Unchanged,
                            );
                        if self.result_access != terminal_psi::StructuralAccess::Owned
                            && same_representation
                        {
                            if self.structural_type != referent_type {
                                if !structural_access_can_supply(
                                    argument.access,
                                    self.result_access,
                                ) {
                                    return unsupported(
                                        "reference alias changes its referent carrier",
                                    );
                                }
                                argument.access = self.result_access;
                            }
                            let loan = match argument.access {
                                terminal_psi::StructuralAccess::SharedBorrow => LoanAccess::Shared,
                                terminal_psi::StructuralAccess::MutableBorrow => {
                                    LoanAccess::Mutable
                                }
                                _ => {
                                    return unsupported(
                                        "reference alias requires readable loan access",
                                    );
                                }
                            };
                            return if let Some(continuation) = continuation {
                                self.complete_loan(argument, continuation, loan)?;
                                Ok(ValueOutcome::Normal(continuation.place))
                            } else {
                                self.direct_borrow(argument).map(ValueOutcome::Normal)
                            };
                        }
                        let (storage, _) = super::super::super::reference_results::publish_storage(
                            storage,
                            self.next_value,
                            self.operations,
                        )?;
                        let carrier = super::super::super::reference_results::emit_carrier(
                            identities,
                            formation,
                            self.structural_type,
                            argument,
                            storage,
                            self.structural_types,
                            self.evaluation,
                            super::super::super::place_id(super::super::super::allocate_dense(
                                self.environment.next_place,
                            )?),
                            self.operations,
                        )?;
                        let place = carrier.id;
                        self.environment.private_places.push(carrier);
                        self.completed(identities, place, completion, continuation)
                            .map(ValueOutcome::Normal)
                    }
                }
            }
            pub(crate) mod residual_complement {

                use crate::LoweringError;
                use checked_trees::{
                    CheckedUnitPartialAffineDiscardPlan, CheckedUnitStructuralArgumentSourcePlan,
                    CheckedUnitStructuralPathSegment, CheckedUnitStructuralTypePlan,
                };

                pub(crate) fn is_empty(
                    checked: &checked_trees::CheckedTrees,
                    state: symbols::SymbolHandle,
                    statement: usize,
                    symbol: symbols::SymbolHandle,
                    vacancies: &[Vec<facts::PlaceSegment>],
                ) -> Result<bool, LoweringError> {
                    let root_type = validation::canonical_place_type_reference(
                        &checked.typed,
                        state,
                        statement,
                        &validation::CanonicalPlace {
                            root: facts::PlaceRoot::Symbol(symbol),
                            segments: Vec::new(),
                        },
                    )
                    .ok_or(LoweringError::Unsupported(
                        "residual owner has no exact type",
                    ))?;
                    let has_nominal_hook = |reference| {
                        let reference = checked
                            .type_reference_table
                            .unconstrained_type_reference(reference);
                        let nominal = match checked.type_reference_table.type_reference(reference) {
                            typed_trees::types::TypeReferenceNode::Named { symbol, .. }
                            | typed_trees::types::TypeReferenceNode::Generic {
                                base_symbol: symbol,
                                ..
                            } => *symbol,
                            _ => return false,
                        };
                        checked.machines().iter().any(|machine| {
                            validation::nominal_cleanup_owner(&checked.typed, machine.symbol)
                                == Some(nominal)
                        })
                    };
                    if has_nominal_hook(root_type) {
                        return Ok(false);
                    }
                    for vacancy in vacancies {
                        for ancestor_length in 1..vacancy.len() {
                            let ancestor_path = &vacancy[..ancestor_length];
                            if matches!(
                                ancestor_path.last(),
                                Some(facts::PlaceSegment::Case { .. })
                            ) {
                                continue;
                            }
                            let (_, ancestor) = validation::structural_owned_path(
                                &checked.typed,
                                root_type,
                                ancestor_path,
                            )
                            .map_err(LoweringError::Unsupported)?;
                            if has_nominal_hook(ancestor) {
                                return Ok(false);
                            }
                        }
                    }
                    let root_identity = validation::stored_structural_type_identity(
                        &checked.typed,
                        root_type,
                        &[],
                        &[],
                    );
                    let moved = vacancies
                        .iter()
                        .map(|path| {
                            let (path, leaf) =
                                validation::structural_owned_path(&checked.typed, root_type, path)
                                    .map_err(LoweringError::Unsupported)?;
                            Ok((
                                path,
                                validation::stored_structural_type_identity(
                                    &checked.typed,
                                    leaf,
                                    &[],
                                    &[],
                                ),
                            ))
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                    let moved = moved
                        .iter()
                        .map(|(path, identity)| (path.as_slice(), *identity))
                        .collect::<Vec<_>>();
                    Ok(complement(
                        &checked.facts.flow.terminal_unit_effects.structural_types,
                        &CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
                        root_identity,
                        &moved,
                    )?
                    .is_empty())
                }

                pub(crate) fn complement(
                    types: &[CheckedUnitStructuralTypePlan],
                    source: &CheckedUnitStructuralArgumentSourcePlan,
                    root_type: typed_trees::type_identity::TypeIdentityId,
                    moved_paths: &[(
                        &[CheckedUnitStructuralPathSegment],
                        typed_trees::type_identity::TypeIdentityId,
                    )],
                ) -> Result<Vec<CheckedUnitPartialAffineDiscardPlan>, LoweringError>
                {
                    let borrowed = types
                        .iter()
                        .map(|declaration| (declaration.identity, declaration))
                        .collect();
                    validation::reconstruct(
                        &borrowed,
                        source,
                        root_type,
                        moved_paths,
                        &[],
                        usize::MAX,
                    )
                    .ok_or(LoweringError::Unsupported(
                        "projected move's residual complement has no partial-affine discard route",
                    ))
                }
            }
            pub(crate) mod subslices {
                use super::{
                    CompletionOccurrence, Emission, LoweringError, PlaceId, StructuralTypeShape,
                    ValueContinuation, allocate_dense, lookup_type_id, place_id, unsupported,
                };
                use crate::emission::operation_emission::view_subslice::ViewFamily;
                use crate::expression_preparation::bindings::StructuralLocalBinding;
                use crate::shared::terminal_identities::stored_structural_identity;
                use crate::unit::attached_unit::structural_values::ValueOutcome;
                use crate::unit::attached_unit::view_ranges;
                use checked_trees::{
                    CheckedCallScalarArgument, CheckedStructuralValueHandle, CheckedSubsliceSite,
                };

                impl Emission<'_, '_, '_> {
                    #[allow(clippy::too_many_arguments)]
                    pub(crate) fn subslice_value(
                        &mut self,
                        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                        source: CheckedStructuralValueHandle,
                        start: &Option<CheckedCallScalarArgument>,
                        end: &Option<CheckedCallScalarArgument>,
                        end_inclusive: bool,
                        site: CheckedSubsliceSite,
                        completion: CompletionOccurrence,
                        continuation: Option<&mut ValueContinuation>,
                    ) -> Result<ValueOutcome<PlaceId>, LoweringError> {
                        let node = self.structural_value_node(source);
                        let source_type = lookup_type_id(
                            self.type_ids,
                            &stored_structural_identity(self.checked, node.type_reference),
                        )?;
                        let family = match terminal_psi::find_structural_type(
                            self.structural_types,
                            self.structural_type,
                        )
                        .map(|declaration| &declaration.shape)
                        {
                            Some(StructuralTypeShape::ByteSequence(_)) => ViewFamily::Bytes,
                            Some(StructuralTypeShape::ElementView { .. }) => ViewFamily::Elements,
                            _ => {
                                return unsupported(
                                    "subslice value has no view descriptor carrier",
                                );
                            }
                        };
                        let ValueOutcome::Normal(completed) = self.child_constructor_operand(
                            identities,
                            source_type,
                            source,
                            node.type_reference,
                        )?
                        else {
                            return Ok(ValueOutcome::NoNormal);
                        };
                        crate::expression_preparation::bindings::retain_completed_structural_homes(
                            &mut self.evaluation.structural_locals,
                            &[StructuralLocalBinding {
                                result_binding_ordinal: None,
                                root: facts::PlaceRoot::Expression(node.expression),
                                argument: completed.argument.clone(),
                                structural_type: source_type,
                            }],
                        )?;
                        if !completed.claims.is_empty() {
                            return unsupported(
                                "view descriptor unexpectedly carries an owned content claim",
                            );
                        }
                        let access = match validation::slice_view_shell(
                            &self.checked.typed,
                            self.type_reference,
                            &[],
                        )
                        .map(|view| view.access)
                        {
                            Some(language_core::ReferenceAccess::Shared) => {
                                terminal_psi::StructuralAccess::SharedBorrow
                            }
                            Some(language_core::ReferenceAccess::Mutable) => {
                                terminal_psi::StructuralAccess::MutableBorrow
                            }
                            _ => {
                                return unsupported("subslice value requires readable view access");
                            }
                        };
                        if completed.argument.access == terminal_psi::StructuralAccess::SharedBorrow
                            && access == terminal_psi::StructuralAccess::MutableBorrow
                        {
                            return unsupported(
                                "subslice cannot amplify its completed source access",
                            );
                        }
                        let place = place_id(allocate_dense(self.environment.next_place)?);
                        let declaration = view_ranges::emit(
                            identities,
                            self.checked,
                            view_ranges::ViewRangeSite {
                                state: self.state,
                                statement: self.statement,
                                range: view_ranges::RangeEndpoints {
                                    start,
                                    end,
                                    end_inclusive,
                                    site,
                                },
                            },
                            view_ranges::ViewRangeSource {
                                place: completed.argument.place,
                                path: completed.argument.path.into_iter().map(crate::emission::operation_emission::expressions::LoweredElementPathSegment::Static).collect(),
                                structural_type: source_type,
                                family,
                                access,
                            },
                            self.structural_type,
                            place,
                            self.evaluation,
                            self.values.len(),
                            self.values,
                            self.next_value,
                            self.next_block,
                            self.next_edge,
                            self.operations,
                            self.calls,
                            self.environment,
                        )?;
                        self.environment.private_places.push(declaration);
                        self.completed(identities, place, completion, continuation)
                            .map(ValueOutcome::Normal)
                    }
                }
            }

            #[derive(Clone, Copy, PartialEq, Eq)]
            pub(crate) enum ValueDestination {
                Binding,
                Operand,
                Evaluation,
            }

            pub(crate) struct ValueProduction<'source> {
                pub(crate) value: CheckedStructuralValueHandle,
                pub(crate) owner: facts::PlaceRoot,
                pub(crate) destination: ValueDestination,
                pub(crate) statement: u32,
                pub(crate) structural_type: StructuralTypeId,
                pub(crate) multiplicity: Multiplicity,
                pub(crate) binding_type_reference: checked_trees::types::TypeReferenceHandle,
                pub(crate) access: StructuralAccess,
                pub(crate) selection: arena::Handle<checked_trees::FlowOwnedSelectionReceipt>,
                pub(crate) calls: &'source [checked_trees::CheckedStructuralValueCall],
            }

            pub(crate) struct CompletedValue {
                pub(crate) argument: StructuralArgument,
                pub(crate) binding: crate::emission::operation_emission::buffer::StructuralBinding,
                pub(crate) declaration: Option<StructuralPlaceDeclaration>,
            }

            #[allow(clippy::too_many_arguments)]
            pub(crate) fn emit<'source>(
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                production: ValueProduction<'source>,
                environment: &mut super::super::operation_frame::OperationEnvironment<'source>,
                calls: &mut CallEmissionContext<'_>,
                evaluation: &mut argument_evaluation::Evaluation,
                values: &mut Vec<ValueDeclaration>,
                next_value: &mut u64,
                next_block: &mut u64,
                next_edge: &mut u64,
                operations: &mut OperationBuffer,
            ) -> Result<ValueOutcome<CompletedValue>, LoweringError> {
                emit_source(
                    identities,
                    production,
                    None,
                    environment,
                    calls,
                    evaluation,
                    values,
                    next_value,
                    next_block,
                    next_edge,
                    operations,
                )
            }

            pub(crate) fn receive_completed<'source>(
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                initialization: &checked_trees::CheckedStructuralLocalInitialization,
                result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
                source: argument_evaluation::StructuralValueStorage,
                environment: &mut super::super::operation_frame::OperationEnvironment<'source>,
                calls: &mut CallEmissionContext<'_>,
                evaluation: &mut argument_evaluation::Evaluation,
                values: &mut Vec<ValueDeclaration>,
                next_value: &mut u64,
                next_block: &mut u64,
                next_edge: &mut u64,
                operations: &mut OperationBuffer,
            ) -> Result<ValueOutcome<CompletedValue>, LoweringError> {
                let production = ValueProduction {
                    value: initialization.value,
                    owner: facts::PlaceRoot::Symbol(result.local.symbol),
                    destination: ValueDestination::Binding,
                    statement: result.statement_index,
                    structural_type: source.selected_type,
                    multiplicity: result.multiplicity,
                    binding_type_reference: Default::default(),
                    access: StructuralAccess::Owned,
                    selection: Default::default(),
                    calls: &[],
                };
                emit_source(
                    identities,
                    production,
                    Some(StructuralArgument {
                        place: source.place,
                        path: source.path,
                        access: terminal_access(initialization.source().access),
                    }),
                    environment,
                    calls,
                    evaluation,
                    values,
                    next_value,
                    next_block,
                    next_edge,
                    operations,
                )
            }

            fn emit_source<'source>(
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                production: ValueProduction<'source>,
                completed_source: Option<StructuralArgument>,
                environment: &mut super::super::operation_frame::OperationEnvironment<'source>,
                calls: &mut CallEmissionContext<'_>,
                evaluation: &mut argument_evaluation::Evaluation,
                values: &mut Vec<ValueDeclaration>,
                next_value: &mut u64,
                next_block: &mut u64,
                next_edge: &mut u64,
                operations: &mut OperationBuffer,
            ) -> Result<ValueOutcome<CompletedValue>, LoweringError> {
                let checked = environment.checked;
                let machine = environment.machine;
                let state = environment.state;
                let structural_types = environment.structural_types;
                let type_ids = environment.type_ids;
                let domain_ids = environment.callees.domain_ids;
                let domain_declarations = environment.callees.domain_declarations;
                let value = &production.value;
                let operand_calls = production.calls;
                let structural_type = production.structural_type;
                let multiplicity = crate::shared::terminal_identities::structural_multiplicity(
                    production.multiplicity,
                );
                let occurrence = checked.facts.values.structural_values.nodes.get(*value);
                if occurrence.machine != machine
                    || occurrence.state != state
                    || occurrence.statement_index != production.statement
                {
                    return unsupported(
                        "structural producer has a different checked source coordinate",
                    );
                }
                let prepared_claims = if completed_source.is_some()
                    || production.destination == ValueDestination::Evaluation
                {
                    Vec::new()
                } else {
                    crate::emission::claim_bindings::prepare_establishment_claims(
                        identities,
                        checked,
                        machine,
                        state,
                        production.statement as usize,
                        production.owner,
                        occurrence.type_reference,
                        structural_type,
                        structural_types,
                    )?
                };
                let result_claims = emit_establishment_claims(&prepared_claims, &mut calls.claims)?;
                let authored_state =
                    crate::shared::declarations::state_declaration(checked, state)?.1;
                let captured_sources = production
                    .selection
                    .is_valid()
                    .then(|| {
                        flow_facts(checked)
                            .ownership
                            .owned_selections
                            .get(production.selection)
                    })
                    .map(|receipt| {
                        flow_facts(checked)
                            .ownership
                            .selection_sources
                            .span_or_empty(receipt.sources)
                            .iter()
                            .enumerate()
                            .filter(|(_, source)| source.retains_storage)
                            .map(|(ordinal, source)| {
                                let source_ordinal = u32::try_from(ordinal).map_err(|_| {
                            LoweringError::Unsupported(
                                "selected source ordinal exceeds its checked representation",
                            )
                        })?;
                                if let Some(local) = structural_local_by_root(
                                    &evaluation.structural_locals,
                                    facts::PlaceRoot::Symbol(source.symbol),
                                ) {
                                    return Ok(argument_evaluation::CapturedSelectionSource {
                                        source_ordinal,
                                        storage: argument_evaluation::StructuralValueStorage {
                                            place: local.argument.place,
                                            path: local.argument.path.clone(),
                                            selected_type: local.structural_type,
                                        },
                                    });
                                }
                                let parameter = checked
                                    .state_parameters(authored_state)
                                    .iter()
                                    .enumerate()
                                    .find(|(_, parameter)| parameter.symbol == source.symbol)
                                    .ok_or(LoweringError::Unsupported(
                                        "owned selection source has no established physical local",
                                    ))?;
                                let declaration = evaluation
                                    .structural_parameters
                                    .iter()
                                    .find(|(position, _)| *position == parameter.0 as u32)
                                    .map(|(_, declaration)| declaration)
                                    .ok_or(LoweringError::Unsupported(
                                        "owned selection parameter source has no signature place",
                                    ))?;
                                if declaration.access != StructuralAccess::Owned {
                                    return Err(LoweringError::Unsupported(
                                        "owned selection parameter source lacks owned custody",
                                    ));
                                }
                                Ok(argument_evaluation::CapturedSelectionSource {
                                    source_ordinal,
                                    storage: argument_evaluation::StructuralValueStorage {
                                        place: declaration.place,
                                        path: Vec::new(),
                                        selected_type: declaration.structural_type,
                                    },
                                })
                            })
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .transpose()?
                    .unwrap_or_default();
                let sources = captured_sources
                    .iter()
                    .map(|source| StructuralArgument {
                        place: source.storage.place,
                        path: source.storage.path.clone(),
                        access: StructuralAccess::Owned,
                    })
                    .collect::<Vec<_>>();
                let declaration =
                    terminal_psi::find_structural_type(structural_types, structural_type);
                let stored_descriptor_value = declaration.is_some_and(|declaration| {
                    terminal_semantics::stored_reference_access(&declaration.shape).is_some()
                });
                let stored_view_value = stored_descriptor_value
                    && declaration.is_some_and(|declaration| {
                        declaration.shape.is_borrowed_byte_carrier()
                            || matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
                    });
                let borrowed_alias = match &structural_values(checked).nodes.get(*value).kind {
                    CheckedStructuralValueKind::Reference {
                        source,
                        source_value,
                        ..
                    } if !stored_view_value
                        && !source_value.is_valid()
                        && lookup_type_id(type_ids, &source.type_identity)? == structural_type =>
                    {
                        Some(terminal_access(source.access))
                    }
                    _ => None,
                };
                let view_loan = declaration.and_then(|declaration| {
                    borrowed_view_join_access(&declaration.shape, production.multiplicity)
                });
                let result_access = if completed_source.is_some() {
                    StructuralAccess::Owned
                } else if let Some(access) = borrowed_alias {
                    access
                } else if stored_descriptor_value {
                    StructuralAccess::Owned
                } else if let Some(access) = view_loan {
                    access
                } else if production.access == StructuralAccess::SharedBorrow
                    && production.multiplicity == Multiplicity::Unrestricted
                {
                    StructuralAccess::SharedBorrow
                } else {
                    StructuralAccess::Owned
                };
                if result_access == StructuralAccess::SharedBorrow
                    && !sources.is_empty()
                    && view_loan != Some(StructuralAccess::SharedBorrow)
                {
                    return unsupported("borrowed selection cannot carry owned residual custody");
                }
                let owners = if sources.is_empty() {
                    Vec::new()
                } else {
                    let owners = prepare_owners(
                        checked,
                        machine,
                        state,
                        production.statement,
                        production.selection,
                        structural_types,
                        evaluation,
                        operations,
                    )?;
                    if owners
                        .iter()
                        .filter(|owner| {
                            sources
                                .iter()
                                .any(|source| source.place == owner.value.place)
                        })
                        .count()
                        != sources.len()
                    {
                        return Err(LoweringError::MissingSelectionSources {
                            machine: checked.symbols.display_path(machine, "::"),
                            state: checked.symbols.display_path(state, "::"),
                            statement_index: production.statement,
                            source_places: sources.iter().map(|source| source.place).collect(),
                            live_places: owners.iter().map(|owner| owner.value.place).collect(),
                        });
                    }
                    owners
                };
                evaluation.capture_selection_sources(production.selection, captured_sources)?;
                let mut emission = Emission {
                    produced_root: *value,
                    checked,
                    machine,
                    state,
                    statement: production.statement,
                    structural_type,
                    multiplicity,
                    structural_types,
                    type_ids,
                    domain_ids,
                    domain_declarations,
                    binding_type_reference: production.binding_type_reference,
                    type_reference: occurrence.type_reference,
                    environment,
                    calls,
                    operand_calls,
                    evaluation,
                    values,
                    next_value,
                    next_block,
                    next_edge,
                    operations,
                    sources,
                    owners,
                    result_access,
                    published_places: Vec::new(),
                    completed_projections: Vec::new(),
                    selection_route: Vec::new(),
                };
                if production.destination == ValueDestination::Evaluation {
                    return match emission.value(identities, *value, None)? {
                        ValueOutcome::NoNormal => Ok(ValueOutcome::NoNormal),
                        ValueOutcome::Normal(_) => unsupported(
                            "evaluation-only checked value unexpectedly completes normally",
                        ),
                    };
                }
                let mut storage_value = *value;
                let mut storage_memberships = Vec::new();
                loop {
                    let node = structural_values(checked).nodes.get(storage_value);
                    let CheckedStructuralValueKind::Projection { source, path, .. } = &node.kind
                    else {
                        break;
                    };
                    if !path.is_empty() {
                        break;
                    }
                    storage_memberships.push((node.qualification_decisions, node.expression));
                    storage_value = *source;
                }
                let storage_projection = match structural_values(checked)
                    .nodes
                    .get(storage_value)
                    .kind
                    .clone()
                {
                    CheckedStructuralValueKind::Projection {
                        source,
                        logical_root_type_identity,
                        path,
                        type_identity,
                        projection_evaluations,
                        copies_selected_value: false,
                        ..
                    } if !path.is_empty()
                        && (production.destination == ValueDestination::Operand
                            || matches!(
                                structural_values(checked).nodes.get(source).kind,
                                CheckedStructuralValueKind::Place(_)
                            )) =>
                    {
                        Some((
                            source,
                            path,
                            projection_evaluations,
                            type_identity,
                            logical_root_type_identity,
                        ))
                    }
                    CheckedStructuralValueKind::Place(argument)
                        if production.destination == ValueDestination::Operand
                            && checked.type_multiplicity(
                                structural_values(checked)
                                    .nodes
                                    .get(storage_value)
                                    .type_reference,
                            ) != Multiplicity::Unrestricted
                            && multiplicity != StructuralMultiplicity::Unrestricted =>
                    {
                        Some((
                            storage_value,
                            Vec::new(),
                            HandleSpan::empty(),
                            argument.type_identity,
                            Default::default(),
                        ))
                    }
                    _ => None,
                };
                let projected_storage = if completed_source.is_none()
                    && emission.sources.is_empty()
                    && let Some((
                        source,
                        path,
                        projection_evaluations,
                        type_identity,
                        logical_root_type_identity,
                    )) = storage_projection
                {
                    let ValueOutcome::Normal(projected) = emission.projection_source(
                        identities,
                        source,
                        path,
                        logical_root_type_identity,
                    )?
                    else {
                        return Ok(ValueOutcome::NoNormal);
                    };
                    let alias = emission.complete_projection_source(
                        identities,
                        structural_values(checked)
                            .nodes
                            .get(storage_value)
                            .expression,
                        &projected,
                        projection_evaluations,
                        type_identity,
                        StructuralAccess::Owned,
                    )?;
                    let alias_path = alias.path;
                    let published_claims = emission
                        .evaluation
                        .current_claims
                        .source(&StructuralArgument {
                            place: projected.root,
                            path: Vec::new(),
                            access: StructuralAccess::Owned,
                        })?
                        .ok_or(LoweringError::Unsupported(
                            "projected alias has no current root custody namespace",
                        ))?;
                    for binding in &result_claims {
                        let mut published_path = alias_path.clone();
                        published_path.extend(binding.path.iter().cloned());
                        if !published_claims.iter().any(|published| {
                            published.claim == binding.claim && published.path == published_path
                        }) {
                            return unsupported(
                                "projected alias claim is not published by its root",
                            );
                        }
                    }
                    let selected_claims = published_claims
                        .into_iter()
                        .filter_map(|claim| {
                            claim
                                .path
                                .strip_prefix(alias_path.as_slice())
                                .map(|suffix| terminal_psi::StructuralResultClaimBinding {
                                    claim: claim.claim,
                                    path: suffix.to_vec(),
                                })
                        })
                        .collect();
                    let alias =
                crate::emission::operation_emission::buffer::StructuralBinding::ProjectedAlias {
                    place: projected.root,
                    path: alias_path,
                    structural_type,
                    multiplicity,
                    claims: selected_claims,
                };
                    Some(alias)
                } else {
                    None
                };
                let place = if let Some(source) = completed_source {
                    let completed = emission.completed_operand(source)?;
                    emission.materialize_completed_operand(identities, completed)?
                } else if let Some(alias) = projected_storage {
                    let first_new_place = *emission.environment.next_place;
                    let mut argument = StructuralArgument {
                        place: alias.place(),
                        path: alias.path().to_vec(),
                        access: StructuralAccess::Owned,
                    };
                    let projected = structural_values(checked).nodes.get(storage_value);
                    if matches!(
                        projected.kind,
                        CheckedStructuralValueKind::Projection { .. }
                    ) {
                        storage_memberships
                            .push((projected.qualification_decisions, projected.expression));
                    }
                    for (decisions, expression) in storage_memberships.into_iter().rev() {
                        argument = emission.retain_checked_memberships(
                            identities,
                            argument,
                            decisions,
                            expression,
                            first_new_place,
                        )?;
                    }
                    if argument.place == alias.place() {
                        return Ok(ValueOutcome::Normal(CompletedValue {
                            argument,
                            binding: alias,
                            declaration: None,
                        }));
                    }
                    argument.place
                } else if emission.sources.is_empty()
                    && let Some((source, region, loan)) =
                        match &structural_values(checked).nodes.get(*value).kind {
                            CheckedStructuralValueKind::BorrowedSliceView { source, region }
                                if source.access
                                    == checked_trees::CheckedStructuralAccess::SharedBorrow =>
                            {
                                Some((source, region, LoanAccess::Shared))
                            }
                            CheckedStructuralValueKind::MutableSliceView { source, region }
                                if source.access
                                    == checked_trees::CheckedStructuralAccess::MutableBorrow =>
                            {
                                Some((source, region, LoanAccess::Mutable))
                            }
                            _ => None,
                        }
                {
                    emission.direct_element_view(
                        identities,
                        source,
                        region,
                        structural_values(checked).nodes.get(*value).expression,
                        loan,
                    )?
                } else {
                    let first_new_place = *emission.environment.next_place;
                    let ValueOutcome::Normal(source) = emission.value(identities, *value, None)?
                    else {
                        return Ok(ValueOutcome::NoNormal);
                    };
                    let source = if emission
                        .environment
                        .private_places
                        .iter()
                        .any(|declaration| {
                            declaration.id == source.place
                                && matches!(
                                    declaration.kind,
                                    StructuralPlaceKind::ByteSequenceLiteral { .. }
                                )
                        }) {
                        let place = emission.copied_leaf_place(
                            identities,
                            &StructuralArgument {
                                access: StructuralAccess::SharedBorrow,
                                ..source
                            },
                            structural_values(checked)
                                .nodes
                                .get(*value)
                                .qualification_decisions,
                            structural_values(checked).nodes.get(*value).expression,
                        )?;
                        StructuralArgument::whole(place, StructuralAccess::Owned)
                    } else {
                        source
                    };
                    let node = structural_values(checked).nodes.get(*value);
                    let source = emission.retain_checked_memberships(
                        identities,
                        source,
                        node.qualification_decisions,
                        node.expression,
                        first_new_place,
                    )?;
                    let binding =
                        emission.value_binding_parameter(identities, &source, source.place, 0)?;
                    let (source_whole, source_projected, _) = emission
                        .completed_memberships(source.place)
                        .ok_or(LoweringError::Unsupported(
                            "binding source has no exact membership declaration",
                        ))?;
                    let (source_whole, source_projected) =
                        terminal_psi::StructuralPathQualification::select(
                            source_whole,
                            source_projected,
                            &source.path,
                        );
                    let distinct_contract = binding.qualifications != source_whole
                        || binding.projected_qualifications != source_projected;
                    if source.place.get() < first_new_place || distinct_contract {
                        emission.materialize_place(identities, source, *value, first_new_place)?
                    } else {
                        source.place
                    }
                };
                let declaration = emission
                    .environment
                    .private_places
                    .iter()
                    .position(|item| item.id == place)
                    .map(|position| emission.environment.private_places.remove(position));
                let published_declaration =
                    emission.published_places.iter().find(|row| row.id == place);
                let completed_declaration = declaration.as_ref().or(published_declaration).ok_or(
                    LoweringError::Unsupported("structural value place was not declared"),
                )?;
                let arrival_claims = emission
                    .operations
                    .arrival_claims
                    .iter()
                    .find(|(arrival, _)| *arrival == place)
                    .map(|(_, claims)| claims.clone())
                    .unwrap_or_default();
                let completed = completed_storage::completed_result(
                    place,
                    completed_declaration,
                    emission.operations,
                    emission.evaluation.structural_root_parameters(),
                    &arrival_claims,
                )?;
                if emission.result_access == StructuralAccess::Owned && declaration.is_some() {
                    emission.operations.register_structural_materialization(
                        place,
                        state,
                        production.statement,
                    )?;
                }
                Ok(ValueOutcome::Normal(CompletedValue {
                    argument: StructuralArgument::whole(place, emission.result_access),
                    binding: completed.into(),
                    declaration,
                }))
            }

            fn borrowed_view_join_access(
                shape: &StructuralTypeShape,
                multiplicity: Multiplicity,
            ) -> Option<StructuralAccess> {
                match shape {
                    StructuralTypeShape::ElementView { access: None, .. } => (multiplicity
                        == Multiplicity::Unrestricted)
                        .then_some(StructuralAccess::SharedBorrow),
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView { access: None }
                        | terminal_psi::ByteSequenceCarrier::BoundedView { access: None, .. },
                    ) => match multiplicity {
                        Multiplicity::Unrestricted => Some(StructuralAccess::SharedBorrow),
                        Multiplicity::Affine => Some(StructuralAccess::MutableBorrow),
                        Multiplicity::Linear => None,
                    },
                    _ => None,
                }
            }

            fn fresh_value_declarations(
                next_value: &mut u64,
                values: &[ValueDeclaration],
            ) -> Result<Vec<ValueDeclaration>, LoweringError> {
                values
                    .iter()
                    .map(|value| {
                        Ok(ValueDeclaration {
                            id: value_id(allocate_dense(next_value)?),
                            ..*value
                        })
                    })
                    .collect()
            }

            fn structural_values(
                checked: &CheckedTrees,
            ) -> &checked_trees::CheckedStructuralValuePlans {
                &checked.facts.values.structural_values
            }

            fn prepare_owners(
                checked: &CheckedTrees,
                machine: symbols::SymbolHandle,
                state: symbols::SymbolHandle,
                statement: u32,
                selection: arena::Handle<checked_trees::FlowOwnedSelectionReceipt>,
                structural_types: &[StructuralTypeDeclaration],
                evaluation: &argument_evaluation::Evaluation,
                operations: &OperationBuffer,
            ) -> Result<Vec<argument_evaluation::StructuralValueOwner>, LoweringError> {
                let source_parameters = checked.state_parameters(
                    crate::shared::declarations::state_declaration(checked, state)?.1,
                );
                let available = |symbol: symbols::SymbolHandle,
                                 formation: u32|
                 -> Result<bool, LoweringError> {
                    if !symbol.is_valid() {
                        return Ok(true);
                    }
                    if evaluation
                        .retired_structural_bindings
                        .iter()
                        .any(|binding| binding.root == facts::PlaceRoot::Symbol(symbol))
                    {
                        return Ok(false);
                    }
                    let entry = source_parameters
                        .iter()
                        .any(|parameter| parameter.symbol == symbol);
                    let has_conflicting_selection = flow_facts(checked)
                        .ownership
                        .with_state_owned_selections(state, |handles| {
                            handles.iter().copied().any(|handle| {
                                let receipt =
                                    checked.facts.flow.ownership.owned_selections.get(handle);
                                receipt.machine == machine
                                    && receipt.state == state
                                    && matches!(
                                receipt.evaluation,
                                checked_trees::FlowPermissionEvaluation::Statement
                                    | checked_trees::FlowPermissionEvaluation::TransitionGuard
                            ) && (entry || receipt.statement_ordinal > formation)
                                    && receipt.statement_ordinal < statement
                                    && flow_facts(checked)
                                        .ownership
                                        .selection_sources
                                        .span_or_empty(receipt.sources)
                                        .iter()
                                        .any(|source| source.symbol == symbol)
                            })
                        });
                    if has_conflicting_selection {
                        return Ok(false);
                    }
                    let mut vacancies = Vec::new();
                    let state_permissions = flow_facts(checked)
                        .ownership
                        .with_state_permissions(state, |handles| handles.to_vec());
                    for handle in state_permissions {
                        let event = checked.facts.flow.ownership.permissions.get(handle);
                        if event.machine_symbol != machine
                            || event.state_symbol != state
                            || event.root != facts::PlaceRoot::Symbol(symbol)
                            || event.access != language_semantics::PermissionAccess::Owned
                        {
                            continue;
                        }
                        if !matches!(
                            event.evaluation,
                            checked_trees::FlowPermissionEvaluation::Statement
                                | checked_trees::FlowPermissionEvaluation::TransitionGuard
                        ) {
                            continue;
                        }
                        let ordinal = match event.source {
                            language_semantics::PermissionEventSource::Statement {
                                statement_index,
                            }
                            | language_semantics::PermissionEventSource::Call {
                                statement_index,
                                ..
                            } => statement_index,
                            _ => continue,
                        };
                        if (!entry && ordinal <= formation as usize)
                            || ordinal >= statement as usize
                        {
                            continue;
                        }
                        let path = flow_facts(checked)
                            .ownership
                            .segments
                            .span_or_empty(event.segments);
                        match event.kind {
                            language_semantics::PermissionEventKind::Establish
                            | language_semantics::PermissionEventKind::Restore => {
                                vacancies.retain(|vacancy: &Vec<_>| !vacancy.starts_with(path));
                            }
                            language_semantics::PermissionEventKind::Transfer
                            | language_semantics::PermissionEventKind::Consume
                            | language_semantics::PermissionEventKind::AffineDrop => {
                                if !vacancies.iter().any(|vacancy| path.starts_with(vacancy)) {
                                    vacancies.retain(|vacancy| !vacancy.starts_with(path));
                                    vacancies.push(path.to_vec());
                                }
                            }
                        }
                    }
                    if vacancies.iter().any(Vec::is_empty) {
                        return Ok(false);
                    }
                    if !vacancies.is_empty() {
                        if residual_complement::is_empty(
                            checked,
                            state,
                            statement as usize,
                            symbol,
                            &vacancies,
                        )? {
                            return Ok(false);
                        }
                        return unsupported(
                            "selection pass-through of partially moved roots requires retained residual frontier correspondence",
                        );
                    }
                    Ok(true)
                };
                let mut owners: Vec<argument_evaluation::StructuralValueOwner> = Vec::new();
                for owner in &evaluation.structural_value_owners {
                    if available(owner.symbol, owner.statement)? {
                        if let Some(existing) = owners
                            .iter_mut()
                            .find(|existing| existing.value.place == owner.value.place)
                        {
                            if !existing.symbol.is_valid() && owner.symbol.is_valid() {
                                existing.symbol = owner.symbol;
                                existing.statement = owner.statement;
                            }
                            continue;
                        }
                        owners.push(owner.clone());
                    }
                }
                let locals = checked.facts.state_locals.locals(state);
                for StructuralLocalBinding { root, argument, .. } in &evaluation.structural_locals {
                    let Some(symbol) = root.as_symbol() else {
                        continue;
                    };
                    let local = match checked.facts.state_locals.named(state, symbol) {
                        Some(local) => local,
                        None => {
                            let parameter = source_parameters
                                .iter()
                                .find(|parameter| parameter.symbol == symbol)
                                .ok_or(LoweringError::Unsupported(
                                    "selection home has no checked source declaration",
                                ))?;
                            if binding_multiplicity(checked, parameter.type_reference)
                                == Multiplicity::Unrestricted
                            {
                                continue;
                            }
                            return unsupported(
                                "selection home requires an owned ingress frontier",
                            );
                        }
                    };
                    if !available(symbol, local.statement_index)? {
                        continue;
                    }
                    if let Some(position) = owners.iter().position(|owner| {
                        owner.symbol == symbol || owner.value.place == argument.place
                    }) {
                        if owners[position].value.place == argument.place
                            && owners[position].symbol.is_valid()
                            && owners[position].symbol != symbol
                        {
                            return unsupported("affine home is owned by two locals");
                        }
                        if !owners[position].symbol.is_valid() {
                            owners[position].symbol = symbol;
                            owners[position].statement = local.statement_index;
                        }
                        continue;
                    }
                    if binding_multiplicity(checked, local.declared_type) != Multiplicity::Affine {
                        continue;
                    }
                    if local.statement_index >= statement
                        || !argument.path.is_empty()
                        || argument.access != StructuralAccess::Owned
                        || !crate::shared::content_classes::stored_value_contents(
                            checked,
                            local.declared_type,
                        )
                    {
                        return unsupported("selection survivor requires whole owned custody");
                    }
                    let value = operations
                .iter()
                .filter_map(|operation| operation.result.structural())
                .find(|result| result.place == argument.place)
                .ok_or(LoweringError::Unsupported(
                    "selection survivor requires an exact emitted operation-result frontier",
                ))?;
                    if value.multiplicity
                        != crate::shared::terminal_identities::structural_multiplicity(
                            checked.type_multiplicity(local.declared_type),
                        )
                        || !structural_types
                            .iter()
                            .any(|declaration| declaration.id == value.structural_type)
                    {
                        return unsupported(
                            "selection survivor disagrees with its declared owned carrier",
                        );
                    }
                    owners.push(argument_evaluation::StructuralValueOwner {
                        symbol,
                        statement: local.statement_index,
                        value: value.clone(),
                    });
                }
                if selection.is_valid() {
                    let receipt = checked.facts.flow.ownership.owned_selections.get(selection);
                    for (position, parameter) in source_parameters.iter().enumerate() {
                        if !flow_facts(checked)
                            .ownership
                            .selection_sources
                            .span_or_empty(receipt.sources)
                            .iter()
                            .any(|source| {
                                source.symbol == parameter.symbol
                                    && source.statement_ordinal == position as u32
                                    && source.retains_storage
                            })
                        {
                            continue;
                        }
                        if parameter.is_const {
                            return unsupported(
                                "selection parameter source is not owned runtime ingress",
                            );
                        }
                        let declaration = evaluation
                            .structural_parameters
                            .iter()
                            .find(|(source_position, _)| *source_position as usize == position)
                            .map(|(_, declaration)| declaration)
                            .ok_or(LoweringError::Unsupported(
                                "selection parameter source has no signature place",
                            ))?;
                        if evaluation
                            .retired_structural_homes
                            .contains(&declaration.place)
                        {
                            continue;
                        }
                        if declaration.access != StructuralAccess::Owned
                            || declaration.multiplicity != StructuralMultiplicity::Affine
                            || checked.type_multiplicity(parameter.type_reference)
                                != Multiplicity::Affine
                            || !crate::shared::content_classes::stored_value_contents(
                                checked,
                                parameter.type_reference,
                            )
                        {
                            return unsupported(
                                "selection parameter source lacks exact owned custody",
                            );
                        }
                        if owners
                            .iter()
                            .any(|owner| owner.value.place == declaration.place)
                        {
                            return unsupported(
                                "selection parameter source duplicated its frontier entry",
                            );
                        }
                        owners.push(argument_evaluation::StructuralValueOwner {
                            symbol: parameter.symbol,
                            statement: u32::try_from(position).map_err(|_| {
                                LoweringError::Unsupported(
                                    "selection parameter position exceeds u32",
                                )
                            })?,
                            value: terminal_psi::StructuralOperationResult {
                                qualification_establishments: Vec::new(),
                                place: declaration.place,
                                structural_type: declaration.structural_type,
                                multiplicity: declaration.multiplicity,
                                qualifications: declaration.qualifications.clone(),
                                projected_qualifications: declaration
                                    .projected_qualifications
                                    .clone(),
                                additional_input_qualifications: declaration
                                    .additional_input_qualifications
                                    .clone(),
                                claims: Vec::new(),
                            },
                        });
                    }
                }
                owners.sort_by_key(|owner| {
                    let is_parameter = source_parameters
                        .iter()
                        .any(|parameter| parameter.symbol == owner.symbol);
                    (!is_parameter, owner.statement)
                });
                for local in locals
                    .iter()
                    .take_while(|local| local.statement_index < statement)
                {
                    if binding_multiplicity(checked, local.declared_type) == Multiplicity::Affine
                        && available(local.symbol, local.statement_index)?
                        && !owners.iter().any(|owner| owner.symbol == local.symbol)
                        && !evaluation
                            .retired_structural_bindings
                            .iter()
                            .any(|retired| {
                                retired.root == facts::PlaceRoot::Symbol(local.symbol)
                                    && retired.statement == Some(local.statement_index)
                                    && retired.end
                                        == argument_evaluation::StructuralBindingEnd::RetiredHome
                            })
                    {
                        return Err(LoweringError::MissingSelectionFrontier {
                            machine: checked.symbols.display_path(machine, "::"),
                            state: checked.symbols.display_path(state, "::"),
                            statement_index: statement,
                            local: checked.symbols.display_path(local.symbol, "::"),
                            formation_statement: local.statement_index,
                        });
                    }
                }
                Ok(owners)
            }

            pub(crate) struct Emission<'a, 'b, 'calls> {
                produced_root: CheckedStructuralValueHandle,
                pub(crate) checked: &'a CheckedTrees,
                pub(crate) machine: symbols::SymbolHandle,
                pub(crate) state: symbols::SymbolHandle,
                pub(crate) statement: u32,
                pub(crate) structural_type: StructuralTypeId,
                pub(crate) multiplicity: StructuralMultiplicity,
                pub(crate) structural_types: &'a [StructuralTypeDeclaration],
                pub(crate) type_ids:
                    &'a [(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
                pub(crate) domain_ids:
                    &'a [crate::shared::terminal_identities::StructuralDomainBinding],
                pub(crate) domain_declarations: &'a [terminal_psi::StructuralDomainDeclaration],
                type_reference: checked_trees::types::TypeReferenceHandle,
                binding_type_reference: checked_trees::types::TypeReferenceHandle,
                pub(crate) environment:
                    &'b mut super::super::operation_frame::OperationEnvironment<'a>,
                pub(crate) calls: &'b mut CallEmissionContext<'calls>,
                pub(crate) operand_calls: &'a [checked_trees::CheckedStructuralValueCall],
                pub(crate) evaluation: &'b mut argument_evaluation::Evaluation,
                pub(crate) values: &'b mut Vec<ValueDeclaration>,
                pub(crate) next_value: &'b mut u64,
                pub(crate) next_block: &'b mut u64,
                pub(crate) next_edge: &'b mut u64,
                pub(crate) operations: &'b mut OperationBuffer,
                sources: Vec<StructuralArgument>,
                owners: Vec<argument_evaluation::StructuralValueOwner>,
                result_access: StructuralAccess,
                published_places: Vec<StructuralPlaceDeclaration>,
                completed_projections: Vec<completed_projection::CompletedProjection>,
                selection_route: Vec<crate::emission::claim_bindings::selections::SelectedDecision>,
            }

            #[derive(Clone, Copy)]
            pub(crate) struct CompletionOccurrence {
                value: CheckedStructuralValueHandle,
                first_new_place: u64,
            }

            pub(crate) enum ValueCompletion {
                NoNormal,
                Awaiting {
                    argument: StructuralArgument,
                    occurrence: CompletionOccurrence,
                },
                Complete {
                    argument: StructuralArgument,
                    receipt: NormalDeathReceipt,
                },
            }

            pub(crate) struct NormalDeathReceipt {
                occurrence: CompletionOccurrence,
                source: StructuralArgument,
                block: BlockId,
            }

            impl ValueCompletion {
                fn into_argument(self) -> ValueOutcome<StructuralArgument> {
                    match self {
                        Self::Awaiting { argument, .. } | Self::Complete { argument, .. } => {
                            ValueOutcome::Normal(argument)
                        }
                        Self::NoNormal => ValueOutcome::NoNormal,
                    }
                }
            }

            pub(crate) struct ValueContinuation {
                block: BlockId,
                parameters: Vec<ValueDeclaration>,
                structural_parameters: Vec<StructuralParameterDeclaration>,
                place: PlaceId,
                remaining_owners: Vec<argument_evaluation::StructuralValueOwner>,
                claim_destinations: Vec<language_semantics::PermissionClaimIdentity>,
                claims: Vec<terminal_psi::StructuralResultClaimBinding>,
                common_claims: Option<Vec<terminal_psi::StructuralResultClaimBinding>>,
                current_claims: Option<crate::emission::current_claims::CurrentClaimNames>,
                has_incoming_memberships: bool,
            }

            pub(crate) struct FallbackContinuation {
                block: BlockId,
                values: Vec<ValueDeclaration>,
                current_claims: crate::emission::current_claims::CurrentClaimNames,
                sources: argument_evaluation::StructuralSourceFrontier,
            }

            pub(crate) struct ProjectionSource {
                pub(crate) root: PlaceId,
                pub(crate) root_path: Vec<terminal_psi::StructuralPathSegment>,
                pub(crate) path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
                pub(crate) root_type_identity: typed_trees::type_identity::TypeIdentityId,
                pub(crate) logical_root_type_identity: typed_trees::type_identity::TypeIdentityId,
            }

            impl Emission<'_, '_, '_> {
                pub(crate) fn retire_completed_operands(
                    &mut self,
                    declaration: &StructuralPlaceDeclaration,
                ) -> Result<(), LoweringError> {
                    let StructuralPlaceKind::OperationResult { producer, .. } = declaration.kind
                    else {
                        return Ok(());
                    };
                    self.evaluation
                        .retire_structural_operation_operands(producer, self.operations)
                }

                fn bind_completion_claims(
                    operations: &mut OperationBuffer,
                    place: PlaceId,
                    claims: &[terminal_psi::StructuralResultClaimBinding],
                    direct_establishments: &[semantic_vocabulary::ClaimId],
                ) -> Result<(), LoweringError> {
                    let (operation, kind, produced) = operations
                        .operations
                        .iter_mut()
                        .find_map(|operation| match &mut operation.result {
                            OperationResult::Structural(result) if result.place == place => {
                                Some((operation.id, &operation.kind, result))
                            }
                            _ => None,
                        })
                        .ok_or(LoweringError::Unsupported(
                            "owned completion has no operation result",
                        ))?;
                    if !produced.claims.is_empty() && produced.claims != claims {
                        return unsupported(
                            "owned completion changes its established claim identities",
                        );
                    }
                    produced.claims = claims.to_vec();
                    if !matches!(
                        kind,
                        OperationKind::EstablishRecord { .. }
                            | OperationKind::EstablishStructuralCase { .. }
                            | OperationKind::EstablishStructuralArray { .. }
                            | OperationKind::EstablishZeroedStructural { .. }
                    ) {
                        return Ok(());
                    }
                    for binding in claims.iter().filter(|binding| {
                        binding.path.is_empty() && direct_establishments.contains(&binding.claim)
                    }) {
                        let introduction = crate::emission::claim_bindings::ClaimIntroduction {
                            operation,
                            claim: binding.claim,
                        };
                        if !operations.claim_introductions.contains(&introduction) {
                            operations.claim_introductions.push(introduction);
                        }
                    }
                    Ok(())
                }

                fn emit_operand_call(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    call: &checked_trees::CheckedStructuralValueCall,
                ) -> Result<ValueOutcome<StructuralPlaceDeclaration>, LoweringError>
                {
                    use super::super::argument_schedule::{self, Step};
                    let planned = call
                        .operands()
                        .iter()
                        .chain(call.operation())
                        .cloned()
                        .collect::<Vec<_>>();
                    let schedule = argument_schedule::build_operations(
                        self.checked,
                        self.machine,
                        self.state,
                        &planned,
                    )?;
                    let mut staged_values = vec![
                            super::super::argument_evaluation::ScalarArgumentCaptures::default();
                            planned.len()
                        ];
                    let mut staged_selectors = vec![Vec::new(); planned.len()];
                    let mut staged_structural_arguments = vec![Vec::new(); planned.len()];
                    let mut staged_ranges = vec![Vec::new(); planned.len()];
                    let mut source_count = None;
                    let mut completed = None;
                    for step in schedule {
                        let (index, staged) = match step {
                            Step::CaptureBefore(statement) => {
                                self.evaluation.prepare_storage_before(
                                    identities,
                                    self.checked,
                                    self.state,
                                    statement,
                                    self.structural_types,
                                    self.domain_declarations,
                                    self.values,
                                    self.environment.next_place,
                                    self.next_value,
                                    self.operations,
                                    self.environment.private_places,
                                )?;
                                continue;
                            }
                            Step::Begin => {
                                if source_count.replace(self.values.len()).is_some() {
                                    return unsupported(
                                        "structural operand argument group is already active",
                                    );
                                }
                                continue;
                            }
                            Step::End => {
                                let count =
                                    source_count.take().ok_or(LoweringError::Unsupported(
                                        "structural operand argument group is absent",
                                    ))?;
                                self.values.truncate(count);
                                continue;
                            }
                            Step::Argument { operation, ordinal } => {
                                let count = source_count.ok_or(LoweringError::Unsupported(
                                    "structural operand scalar has no source prefix",
                                ))?;
                                let value = self.evaluation.argument_at(
                                    identities,
                                    self.checked,
                                    self.machine,
                                    self.state,
                                    &planned[operation],
                                    ordinal,
                                    count,
                                    self.values,
                                    self.next_value,
                                    self.next_block,
                                    self.next_edge,
                                    self.operations,
                                    self.calls,
                                    self.environment,
                                )?;
                                staged_values[operation].record(ordinal, self.values.len());
                                self.values.push(value);
                                continue;
                            }
                            Step::Selector {
                                operation,
                                argument,
                                segment,
                            } => {
                                let count = source_count.ok_or(LoweringError::Unsupported(
                                    "structural selector has no source prefix",
                                ))?;
                                let value = self.evaluation.call_selector(
                                    identities,
                                    self.checked,
                                    self.machine,
                                    self.state,
                                    &planned[operation],
                                    argument,
                                    segment,
                                    count,
                                    self.values,
                                    self.next_value,
                                    self.next_block,
                                    self.next_edge,
                                    self.operations,
                                    self.calls,
                                    self.environment,
                                )?;
                                staged_selectors[operation].push(self.values.len());
                                self.values.push(value);
                                continue;
                            }
                            Step::Structural {
                                operation,
                                argument,
                                complete,
                            } => {
                                let count = source_count.ok_or(LoweringError::Unsupported(
                                    "structural actual has no source prefix",
                                ))?;
                                let arguments = planned[operation].structural_operands();
                                let offset = arguments[..argument]
                                    .iter()
                                    .map(|source| source.call_selector_positions().count())
                                    .sum::<usize>();
                                let selectors = staged_selectors[operation][offset..].to_vec();
                                if staged_structural_arguments[operation].is_empty() {
                                    staged_structural_arguments[operation] =
                                        vec![None; arguments.len()];
                                }
                                let previous =
                                    staged_structural_arguments[operation][argument].take();
                                let captured = self.environment.capture_argument(
                                    identities,
                                    &planned[operation],
                                    argument,
                                    &selectors,
                                    previous,
                                    complete,
                                    count,
                                    self.evaluation,
                                    self.values,
                                    self.next_value,
                                    self.next_block,
                                    self.next_edge,
                                    self.calls,
                                    self.operations,
                                )?;
                                staged_structural_arguments[operation][argument] = Some(captured);
                                continue;
                            }
                            Step::Subslice { operation, ordinal } => {
                                let count = source_count.ok_or(LoweringError::Unsupported(
                                    "structural range operand has no source prefix",
                                ))?;
                                let bindings = self.evaluation.contextual_bindings();
                                let context = super::super::byte_subslices::StagingContext {
                                    state: self.state,
                                    structural_types: self.structural_types,
                                    type_ids: self.type_ids,
                                    bindings: &bindings,
                                };
                                let place = super::super::byte_subslices::emit(
                                    identities,
                                    self.checked,
                                    &context,
                                    &planned[operation],
                                    ordinal,
                                    self.evaluation,
                                    count,
                                    self.values,
                                    self.next_value,
                                    self.next_block,
                                    self.next_edge,
                                    self.operations,
                                    self.calls,
                                    self.environment,
                                )?;
                                staged_ranges[operation].push((ordinal, place));
                                continue;
                            }
                            Step::Operation(index) | Step::Call(index)
                                if source_count.is_some() =>
                            {
                                (index, true)
                            }
                            Step::Ordinary(index) if source_count.is_none() => (index, false),
                            _ => {
                                return unsupported(
                                    "structural operand argument schedule is unbalanced",
                                );
                            }
                        };
                        let operation = &planned[index];
                        let count = source_count.unwrap_or(self.values.len());
                        let evaluated = if staged && operation.call().is_some() {
                            Some(staged_values[index].values(self.values))
                        } else if operation.call().is_some() {
                            self.evaluation.arguments(
                                identities,
                                self.checked,
                                self.machine,
                                self.state,
                                operation,
                                self.values,
                                self.next_value,
                                self.next_block,
                                self.next_edge,
                                self.operations,
                                self.calls,
                                self.environment,
                            )?
                        } else {
                            None
                        };
                        let selectors = staged_selectors[index]
                            .iter()
                            .map(|slot| {
                                self.values
                                    .get(*slot)
                                    .copied()
                                    .ok_or(LoweringError::Unsupported(
                                        "structural selector lost its retained value",
                                    ))
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        let range_places = super::super::byte_subslices::argument_places(
                            operation.structural_operands(),
                            &[],
                            &mut 0,
                            &staged_ranges[index],
                        )?;
                        let declaration = self.environment.emit_operand(
                            identities,
                            operation,
                            evaluated.as_deref(),
                            staged.then_some(selectors.as_slice()),
                            (!staged_structural_arguments[index].is_empty())
                                .then_some(staged_structural_arguments[index].as_slice()),
                            &range_places,
                            count,
                            self.evaluation,
                            self.values,
                            self.next_value,
                            self.next_block,
                            self.next_edge,
                            self.calls,
                            self.operations,
                        )?;
                        if self.operations.invocation_completion.is_some() {
                            return Ok(ValueOutcome::NoNormal);
                        }
                        if index + 1 == planned.len() {
                            completed = declaration;
                        }
                    }
                    if self.operations.invocation_completion.is_some() {
                        return Ok(ValueOutcome::NoNormal);
                    }
                    for completion in call
                        .completions()
                        .into_iter()
                        .flat_map(|sequence| sequence.iter())
                    {
                        self.environment.emit_operand(
                            identities,
                            completion,
                            None,
                            None,
                            None,
                            &[],
                            source_count.unwrap_or(self.values.len()),
                            self.evaluation,
                            self.values,
                            self.next_value,
                            self.next_block,
                            self.next_edge,
                            self.calls,
                            self.operations,
                        )?;
                    }
                    let completed = completed.ok_or(LoweringError::Unsupported(
                        "structural operand call did not publish its result",
                    ))?;
                    let result = call
                        .operation()
                        .and_then(|operation| operation.structural_result_binding())
                        .ok_or(LoweringError::Unsupported(
                            "structural operand call lost its checked result",
                        ))?;
                    let binding = self
                        .operations
                        .structural_values
                        .iter()
                        .find(|(ordinal, _)| *ordinal == result.binding_ordinal)
                        .map(|(_, binding)| binding)
                        .ok_or(LoweringError::Unsupported(
                            "structural operand call has no completed binding",
                        ))?;
                    if binding.place() != completed.id || !binding.path().is_empty() {
                        return unsupported(
                            "structural operand call completion names different storage",
                        );
                    }
                    let result = completed_storage::completed_result(
                        completed.id,
                        &completed,
                        self.operations,
                        self.evaluation.structural_root_parameters(),
                        binding.claims(),
                    )?;
                    let source_expression = self.structural_value_node(call.value).expression;
                    let structural_type = result.structural_type;
                    self.operations
                        .arrival_claims
                        .push((completed.id, result.claims));
                    crate::expression_preparation::bindings::retain_completed_structural_homes(
                        &mut self.evaluation.structural_locals,
                        &[StructuralLocalBinding {
                            result_binding_ordinal: None,
                            root: facts::PlaceRoot::Expression(source_expression),
                            argument: StructuralArgument::whole(
                                completed.id,
                                StructuralAccess::Owned,
                            ),
                            structural_type,
                        }],
                    )?;
                    self.published_places.push(completed);
                    Ok(ValueOutcome::Normal(completed))
                }

                pub(crate) fn child_constructor_operand(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    child_type: StructuralTypeId,
                    value: CheckedStructuralValueHandle,
                    reference: checked_trees::types::TypeReferenceHandle,
                ) -> Result<ValueOutcome<super::operand_claims::CompletedOperand>, LoweringError>
                {
                    let parent_type = self.structural_type;
                    let parent_multiplicity = self.multiplicity;
                    let parent_binding = std::mem::take(&mut self.binding_type_reference);
                    let parent_access =
                        std::mem::replace(&mut self.result_access, StructuralAccess::Owned);
                    self.structural_type = child_type;
                    self.multiplicity = crate::shared::terminal_identities::structural_multiplicity(
                        crate::shared::reference_custody::result_multiplicity(
                            self.checked,
                            reference,
                        ),
                    );
                    let argument = self.constructor_operand(identities, value, reference);
                    self.structural_type = parent_type;
                    self.multiplicity = parent_multiplicity;
                    self.result_access = parent_access;
                    self.binding_type_reference = parent_binding;
                    argument
                }

                pub(crate) fn constructor_operand(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    value: CheckedStructuralValueHandle,
                    reference: checked_trees::types::TypeReferenceHandle,
                ) -> Result<ValueOutcome<super::operand_claims::CompletedOperand>, LoweringError>
                {
                    let first_new_place = *self.environment.next_place;
                    let node = self.structural_value_node(value);
                    if let CheckedStructuralValueKind::Projection {
                        source,
                        logical_root_type_identity,
                        path,
                        type_identity,
                        projection_evaluations,
                        copies_selected_value: false,
                    } = node.kind
                        && !path.is_empty()
                        && matches!(
                            structural_values(self.checked).nodes.get(source).kind,
                            CheckedStructuralValueKind::Place(_)
                        )
                    {
                        let ValueOutcome::Normal(projected) = self.projection_source(
                            identities,
                            source,
                            path,
                            logical_root_type_identity,
                        )?
                        else {
                            return Ok(ValueOutcome::NoNormal);
                        };
                        let argument = self.complete_projection_source(
                            identities,
                            node.expression,
                            &projected,
                            projection_evaluations,
                            type_identity,
                            StructuralAccess::Owned,
                        )?;
                        let argument = self.retain_checked_memberships(
                            identities,
                            argument,
                            node.qualification_decisions,
                            node.expression,
                            first_new_place,
                        )?;
                        return self
                            .capture_operand(identities, argument, node.expression, first_new_place)
                            .map(ValueOutcome::Normal);
                    }
                    let sources = std::mem::take(&mut self.sources);
                    let owners = std::mem::take(&mut self.owners);
                    let parent_reference = std::mem::replace(&mut self.type_reference, reference);
                    let produced = self.value(identities, value, None);
                    self.type_reference = parent_reference;
                    self.sources = sources;
                    self.owners = owners;
                    let ValueOutcome::Normal(argument) = produced? else {
                        return Ok(ValueOutcome::NoNormal);
                    };
                    let argument = self.retain_checked_memberships(
                        identities,
                        argument,
                        node.qualification_decisions,
                        node.expression,
                        first_new_place,
                    )?;
                    self.capture_operand(identities, argument, node.expression, first_new_place)
                        .map(ValueOutcome::Normal)
                }

                pub(crate) fn structural_value_node(
                    &self,
                    value: CheckedStructuralValueHandle,
                ) -> CheckedStructuralValue {
                    structural_values(self.checked).nodes.get(value).clone()
                }

                pub(crate) fn structural_value_fields(
                    &self,
                    fields: HandleSpan<checked_trees::CheckedStructuralRecordField>,
                    stale: &'static str,
                ) -> Result<Vec<checked_trees::CheckedStructuralRecordField>, LoweringError>
                {
                    Ok(structural_values(self.checked)
                        .record_fields
                        .span(fields)
                        .ok_or(LoweringError::Unsupported(stale))?
                        .to_vec())
                }

                pub(crate) fn remap_saved_scalar_fields(
                    &mut self,
                    scalar_positions: Vec<(usize, usize)>,
                    initialized: &mut [(usize, terminal_psi::RecordFieldInitializer)],
                    source_count: usize,
                ) {
                    for (field, position) in scalar_positions {
                        let terminal_psi::RecordFieldValue::Scalar { value, .. } =
                            &mut initialized[field].1.value
                        else {
                            unreachable!()
                        };
                        *value = self.values[position].id;
                    }
                    self.values.truncate(source_count);
                }

                pub(crate) fn value(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,

                    value: CheckedStructuralValueHandle,
                    continuation: Option<&mut ValueContinuation>,
                ) -> Result<ValueOutcome<StructuralArgument>, LoweringError> {
                    let node = self.structural_value_node(value);
                    if node.machine != self.machine
                        || node.state != self.state
                        || node.statement_index != self.statement
                    {
                        return unsupported(
                            "structural operand has a different checked source coordinate",
                        );
                    }
                    if value != self.produced_root
                        && structural_values(self.checked)
                            .root_for_value(value)
                            .is_some_and(|root| root.captured_copy.is_valid())
                    {
                        let captured = self
                            .evaluation
                            .saved_value_binding(value, self.operations)?;
                        return Ok(ValueOutcome::Normal(StructuralArgument {
                            access: StructuralAccess::SharedBorrow,
                            ..captured.argument.clone()
                        }));
                    }
                    let requires_completion = self.content_transformation(&node)?.is_some()
                        || flow_facts(self.checked)
                            .terminal_unit_effects
                            .cleanup_control
                            .value_completion(
                                self.machine,
                                self.state,
                                self.statement,
                                node.expression,
                            )
                            .is_some();
                    if requires_completion {
                        let produced = self.raw_value(identities, value, None)?;
                        if matches!(produced, ValueCompletion::NoNormal) {
                            return Ok(ValueOutcome::NoNormal);
                        }
                        let (completed, receipt) =
                            self.finish_value_completion(identities, value, produced)?;
                        return self
                            .finish_value_arrival(completed, receipt, continuation)
                            .map(ValueCompletion::into_argument);
                    }
                    self.raw_value(identities, value, continuation)
                        .map(ValueCompletion::into_argument)
                }

                fn raw_value(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    value: CheckedStructuralValueHandle,
                    continuation: Option<&mut ValueContinuation>,
                ) -> Result<ValueCompletion, LoweringError> {
                    let node = self.structural_value_node(value);
                    let completion = CompletionOccurrence {
                        value,
                        first_new_place: *self.environment.next_place,
                    };
                    macro_rules! finish {
                        ($place:expr) => {
                            self.complete_construction(
                                identities,
                                $place,
                                node.qualification_decisions,
                                completion,
                                continuation,
                            )
                        };
                    }
                    let place = match node.kind {
                        CheckedStructuralValueKind::NativeCodeLayout { selected_state } => {
                            let (selected, _) = validation::find_machine_by_entry_state(
                                &self.checked.typed,
                                selected_state,
                            )
                            .ok_or(LoweringError::Unsupported(
                                "native layout query lost its exact selected executable entry",
                            ))?;
                            let machine =
                                lookup_machine_id(self.calls.machine_ids, selected.symbol)?;
                            let place = place_id(allocate_dense(self.environment.next_place)?);
                            let operation = self.operations.allocate();
                            self.operations.push(terminal_psi::Operation::unsuspended(
                        operation,
                        crate::emission::operation_emission::buffer::structural_result_declaration(
                            place,
                            self.structural_type,
                            self.multiplicity,
                        ),
                        OperationKind::EstablishNativeCodeLayout { machine },
                    ));
                            self.environment.private_places.push(operation_result_place(
                                place,
                                operation,
                                self.structural_type,
                            ));
                            self.completed(identities, place, completion, continuation)
                        }

                        CheckedStructuralValueKind::QuotientCall { arguments, .. } => {
                            let ValueOutcome::Normal(place) =
                                self.quotient_call(identities, value, &arguments)?
                            else {
                                return Ok(ValueCompletion::NoNormal);
                            };
                            self.completed(identities, place, completion, continuation)
                        }
                        CheckedStructuralValueKind::QuotientAdaptation { source, .. } => {
                            let child = self.structural_value_node(source);
                            let source_type = lookup_type_id(
                                self.type_ids,
                                &stored_structural_identity(self.checked, child.type_reference),
                            )?;
                            let ValueOutcome::Normal(completed) = self.child_constructor_operand(
                                identities,
                                source_type,
                                source,
                                child.type_reference,
                            )?
                            else {
                                return Ok(ValueCompletion::NoNormal);
                            };
                            if !completed.claims.is_empty()
                                || !completed.argument.path.is_empty()
                                || self.multiplicity != StructuralMultiplicity::Unrestricted
                            {
                                return unsupported(
                                    "quotient adaptation requires a whole custody-free copy value",
                                );
                            }
                            let place = place_id(allocate_dense(self.environment.next_place)?);
                            let declaration = crate::emission::quotient_values::emit_adaptation(
                                value,
                                completed.argument.place,
                                source_type,
                                place,
                                self.structural_type,
                                self.operations,
                            );
                            self.environment.private_places.push(declaration);
                            self.completed(identities, place, completion, continuation)
                        }
                        CheckedStructuralValueKind::ViewSubslice {
                            source,
                            start,
                            end,
                            end_inclusive,
                            site,
                        } => match self.subslice_value(
                            identities,
                            source,
                            &start,
                            &end,
                            end_inclusive,
                            site,
                            completion,
                            continuation,
                        )? {
                            ValueOutcome::Normal(place) => Ok(place),
                            ValueOutcome::NoNormal => return Ok(ValueCompletion::NoNormal),
                        },
                        CheckedStructuralValueKind::BorrowedSliceView { source, region } => self
                            .joined_element_view(
                                identities,
                                &source,
                                &region,
                                node.expression,
                                LoanAccess::Shared,
                                continuation,
                            ),
                        CheckedStructuralValueKind::MutableSliceView { source, region } => self
                            .joined_element_view(
                                identities,
                                &source,
                                &region,
                                node.expression,
                                LoanAccess::Mutable,
                                continuation,
                            ),
                        CheckedStructuralValueKind::Reference {
                            source,
                            selectors,
                            source_value,
                            storage,
                            backing_type,
                            source_contract,
                        } => match self.reference_value(
                            identities,
                            super::super::reference_results::Formation {
                                checked: self.checked,
                                statement_index: node.statement_index as usize,
                                expression: node.expression,
                                backing_type,
                                source_contract: &source_contract,
                                qualification_decisions: node.qualification_decisions,
                                domain_ids: self.domain_ids,
                            },
                            &source,
                            selectors,
                            source_value,
                            storage,
                            completion,
                            continuation,
                        )? {
                            ValueOutcome::Normal(place) => Ok(place),
                            ValueOutcome::NoNormal => return Ok(ValueCompletion::NoNormal),
                        },
                        CheckedStructuralValueKind::Record { .. } => {
                            let ValueOutcome::Normal(place) = self.record(identities, value)?
                            else {
                                return Ok(ValueCompletion::NoNormal);
                            };

                            finish!(place)
                        }
                        CheckedStructuralValueKind::FixedArray { elements } => {
                            let ValueOutcome::Normal(place) =
                                self.fixed_array(identities, &elements, self.type_reference)?
                            else {
                                return Ok(ValueCompletion::NoNormal);
                            };

                            finish!(place)
                        }
                        CheckedStructuralValueKind::Call { .. } => {
                            let mut matching =
                                self.operand_calls.iter().filter(|call| call.value == value);
                            let call = matching.next().ok_or(LoweringError::Unsupported(
                                "structural operand call missing",
                            ))?;
                            if matching.next().is_some() {
                                return unsupported("structural operand call is duplicated");
                            }
                            let ValueOutcome::Normal(declaration) =
                                self.emit_operand_call(identities, call)?
                            else {
                                return Ok(ValueCompletion::NoNormal);
                            };
                            self.completed(identities, declaration.id, completion, continuation)
                        }
                        CheckedStructuralValueKind::Projection {
                            source,
                            logical_root_type_identity,
                            path,
                            projection_evaluations,
                            type_identity,
                            copies_selected_value,
                        } => {
                            if path.is_empty() && !copies_selected_value {
                                let first_new_place = *self.environment.next_place;
                                let ValueOutcome::Normal(argument) =
                                    self.value(identities, source, None)?
                                else {
                                    return Ok(ValueCompletion::NoNormal);
                                };
                                let mut argument = self.retain_checked_memberships(
                                    identities,
                                    argument,
                                    node.qualification_decisions,
                                    node.expression,
                                    first_new_place,
                                )?;
                                argument.access = self.result_access;
                                if let Some(continuation) = continuation {
                                    self.complete_operand(
                                        identities,
                                        argument.clone(),
                                        completion,
                                        continuation,
                                    )?;
                                }
                                return Ok(ValueCompletion::Awaiting {
                                    argument,
                                    occurrence: completion,
                                });
                            }
                            if copies_selected_value {
                                let ValueOutcome::Normal(projected) = self.projection_source(
                                    identities,
                                    source,
                                    path,
                                    logical_root_type_identity,
                                )?
                                else {
                                    return Ok(ValueCompletion::NoNormal);
                                };
                                let argument = self.complete_projection_source(
                                    identities,
                                    node.expression,
                                    &projected,
                                    projection_evaluations,
                                    type_identity,
                                    StructuralAccess::SharedBorrow,
                                )?;
                                let place = self.copied_leaf_place(
                                    identities,
                                    &argument,
                                    node.qualification_decisions,
                                    node.expression,
                                )?;
                                let produced = ValueCompletion::Awaiting {
                                    argument: StructuralArgument::whole(
                                        place,
                                        StructuralAccess::Owned,
                                    ),
                                    occurrence: completion,
                                };
                                let (completed, receipt) =
                                    self.finish_value_completion(identities, value, produced)?;
                                return self.finish_value_arrival(completed, receipt, continuation);
                            }
                            let mut owned_continuation;
                            let is_root = continuation.is_none();
                            let continuation = if let Some(continuation) = continuation {
                                continuation
                            } else {
                                owned_continuation = self.root_value_continuation(
                                    identities,
                                    value,
                                    self.values.clone(),
                                )?;
                                &mut owned_continuation
                            };
                            let ValueOutcome::Normal(projected) = self.projection_source(
                                identities,
                                source,
                                path,
                                logical_root_type_identity,
                            )?
                            else {
                                return Ok(ValueCompletion::NoNormal);
                            };
                            if matches!(
                                structural_values(self.checked).nodes.get(source).kind,
                                CheckedStructuralValueKind::Place(_)
                            ) {
                                let existing = StructuralArgument {
                                    place: projected.root,
                                    path: projected.root_path.clone(),
                                    access: StructuralAccess::Owned,
                                };
                                if self
                                    .evaluation
                                    .retired_structural_homes
                                    .contains(&existing.place)
                                    || self.evaluation.current_claims.source(&existing)?.is_none()
                                {
                                    return unsupported(
                                        "projected selection root has no current owned source association",
                                    );
                                }
                            }
                            let selected = self.complete_projection_source(
                                identities,
                                node.expression,
                                &projected,
                                projection_evaluations,
                                type_identity,
                                StructuralAccess::Owned,
                            )?;
                            let captured = self.capture_operand(
                                identities,
                                selected,
                                node.expression,
                                completion.first_new_place,
                            )?;
                            let (whole, projected_memberships, _) = self
                                .completed_memberships(captured.argument.place)
                                .ok_or(LoweringError::Unsupported(
                                    "projected move lost its source memberships",
                                ))?;
                            let (qualifications, projected_qualifications) =
                                terminal_psi::StructuralPathQualification::select(
                                    whole,
                                    projected_memberships,
                                    &captured.argument.path,
                                );
                            let selected = self.move_completed_operand(
                                captured,
                                qualifications,
                                projected_qualifications,
                            )?;
                            let produced = ValueCompletion::Awaiting {
                                argument: selected,
                                occurrence: completion,
                            };
                            let (completed, receipt) =
                                self.finish_value_completion(identities, value, produced)?;
                            let arrived = self.finish_value_arrival(
                                completed,
                                receipt,
                                Some(&mut *continuation),
                            )?;
                            if is_root {
                                self.record_root_continuation(continuation)?;
                            }
                            return Ok(arrived);
                        }
                        CheckedStructuralValueKind::CopiedStructuralPlace { source, .. } => {
                            if source.access != checked_trees::CheckedStructuralAccess::SharedBorrow
                            {
                                return unsupported("copied value requires shared source custody");
                            }
                            let argument =
                                self.readable_place_argument(identities, node.expression, &source)?;
                            let place = self.copied_leaf_place(
                                identities,
                                &argument,
                                node.qualification_decisions,
                                node.expression,
                            )?;
                            self.completed(identities, place, completion, continuation)
                        }
                        CheckedStructuralValueKind::ZeroedScalarArray {
                            element,
                            element_count,
                        } => {
                            let computation = value_facts(self.checked)
                                .scalar_computations
                                .nodes
                                .get(element);
                            let checked_trees::CheckedScalarComputationKind::Value(
                                checked_trees::CheckedScalarExpression::IntegerLiteral { literal },
                            ) = &computation.kind
                            else {
                                return unsupported(
                                    "zeroed scalar array element is not a literal zero",
                                );
                            };
                            let scalar_type =
                                terminal_scalar_type(computation.scalar_type.primitive().ok_or(
                                    LoweringError::Unsupported(
                                        "zeroed scalar array has an opaque element carrier",
                                    ),
                                )?)?;
                            let value =
                                crate::emission::scalar_types::integer_value(literal, scalar_type)?;
                            let zero = emit_scalar_leaf(
                                OperationKind::IntegerConstant { value },
                                scalar_type,
                                self.next_value,
                                self.operations,
                            );
                            self.values.push(ValueDeclaration {
                                qualifications: Default::default(),
                                id: zero,
                                scalar_type,
                            });
                            let (place, operation) = allocate_structural_result(
                                self.operations,
                                self.environment.next_place,
                                self.structural_type,
                                self.multiplicity,
                                Vec::new(),
                                OperationKind::EstablishScalarArray {
                                    elements: vec![
                                        zero;
                                        usize::try_from(element_count).map_err(|_| {
                                            LoweringError::Unsupported(
                                                "zeroed scalar array leaf count exceeds usize",
                                            )
                                        })?
                                    ],
                                },
                            )?;
                            self.environment.private_places.push(operation_result_place(
                                place,
                                operation,
                                self.structural_type,
                            ));
                            finish!(place)
                        }
                        CheckedStructuralValueKind::Scalar { value } => {
                            let initialized = self.scalar(
                                identities,
                                CheckedScalarExpressionRole::StructuralScalarValue {
                                    expression: node.expression,
                                },
                                value,
                                self.values.len(),
                            )?;
                            let obligation =
                                super::super::primitive_locals::initialization_obligation(
                                    self.structural_types,
                                    self.structural_type,
                                    &mut self.calls.next_obligation_identity,
                                )?;
                            let (place, declaration) =
                                super::super::primitive_locals::push_established_local(
                                    self.environment.next_place,
                                    self.operations,
                                    initialized,
                                    self.structural_type,
                                    super::super::primitive_locals::PrimitiveStorageCatalog {
                                        types: self.structural_types,
                                        domains: self.domain_declarations,
                                        scalars: self.evaluation.shared_qualifications.catalog(),
                                    },
                                    obligation,
                                )?;
                            self.environment.private_places.push(declaration);
                            self.completed(identities, place, completion, continuation)
                        }
                        CheckedStructuralValueKind::ScalarArray { elements } => {
                            let source_count = self.values.len();
                            for (ordinal, element) in elements.iter().enumerate() {
                                let evaluated = self.scalar(
                                    identities,
                                    CheckedScalarExpressionRole::StructuralArrayElement {
                                        expression: node.expression,
                                        element_ordinal: u32::try_from(ordinal).map_err(|_| {
                                            LoweringError::Unsupported(
                                                "scalar array leaf ordinal overflow",
                                            )
                                        })?,
                                    },
                                    *element,
                                    source_count,
                                )?;
                                self.values.push(evaluated);
                            }
                            let leaves = self.values[source_count..]
                                .iter()
                                .map(|value| value.id)
                                .collect::<Vec<_>>();
                            self.values.truncate(source_count);
                            let (place, operation) = allocate_structural_result(
                                self.operations,
                                self.environment.next_place,
                                self.structural_type,
                                self.multiplicity,
                                Vec::new(),
                                OperationKind::EstablishScalarArray { elements: leaves },
                            )?;
                            self.environment.private_places.push(operation_result_place(
                                place,
                                operation,
                                self.structural_type,
                            ));
                            let domains = self.checked_membership_domains(
                                node.qualification_decisions,
                                self.structural_type,
                            )?;
                            if crate::unit::attached_unit::scalar_arrays::replayed_at_construction(
                                self.domain_declarations,
                                &domains,
                            ) {
                                finish!(place)
                            } else {
                                // The construction states no membership a
                                // receiver cannot replay from its operands;
                                // the checked ones arrive by a qualifying copy.
                                let first_operation = self.operations.operations.len();
                                let qualified = crate::unit::attached_unit::scalar_arrays::qualify(
                                    place,
                                    self.structural_type,
                                    self.multiplicity,
                                    domains,
                                    self.environment.next_place,
                                    self.operations,
                                )?;
                                let place = qualified.id;
                                self.environment.private_places.push(qualified);
                                self.operations.register_structural_materialization(
                                    place,
                                    self.state,
                                    self.statement,
                                )?;
                                self.evaluation.current_claims.publish_operations(
                                    &self.operations.operations[first_operation..],
                                )?;
                                self.completed(identities, place, completion, continuation)
                            }
                        }
                        CheckedStructuralValueKind::ByteLiteralView { bytes } => {
                            let qualifications = self.checked_membership_domains(
                                node.qualification_decisions,
                                self.structural_type,
                            )?;
                            let declaration =
                                crate::emission::structural_byte_sequence_store::emit_call_literal(
                                    identities,
                                    &bytes,
                                    self.structural_type,
                                    qualifications,
                                    self.structural_types,
                                    self.environment.private_places,
                                    self.environment.next_place,
                                    self.next_value,
                                    &mut self.calls.next_obligation_identity,
                                    self.operations,
                                )?;
                            self.completed(identities, declaration.id, completion, continuation)
                        }
                        CheckedStructuralValueKind::ZeroedStructural {
                            projected_qualifications,
                        } => {
                            let projected =
                                super::super::parameters::lower_projected_qualifications(
                                    identities,
                                    &projected_qualifications,
                                    self.domain_ids,
                                    self.structural_type,
                                    self.structural_types,
                                )?;
                            let place = self.establish_zeroed()?;
                            let result = self
                                .operations
                                .operations
                                .last_mut()
                                .and_then(|operation| match &mut operation.result {
                                    OperationResult::Structural(result) => Some(result),
                                    _ => None,
                                })
                                .ok_or(LoweringError::Unsupported(
                                    "zeroed establishment has no result",
                                ))?;
                            result.projected_qualifications = projected;

                            finish!(place)
                        }
                        CheckedStructuralValueKind::Place(argument) => {
                            if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { bytes } = &argument.source {
                                let domains = byte_literals::validate(
                                    self.checked, self.type_reference, &argument,
                                )?;
                                let qualifications = domains.into_iter().map(|domain|
                                    crate::shared::terminal_identities::lookup_domain_id_for_carrier(
                                        self.domain_ids, domain, self.structural_type,
                                    )
                                ).collect::<Result<Vec<_>, _>>()?;
                                let (declaration, _) = crate::emission::structural_byte_sequence_store::emit_owned_literal(identities, bytes, self.structural_type, qualifications, self.structural_types, self.environment.private_places, self.environment.next_place, self.next_value, &mut self.calls.next_obligation_identity, self.operations)?;
                                let place = declaration.id;
                                self.environment.private_places.push(declaration);
                                self.completed(identities, place, completion, continuation)?;
                                return Ok(ValueCompletion::Awaiting {
                                    argument: StructuralArgument { place, path: Vec::new(), access: StructuralAccess::Owned },
                                    occurrence: completion,
                                });
                            }
                            if let Some(source) = self.copied_parameter(&argument)? {
                                let place = self.copied_leaf_place(
                                    identities,
                                    &source,
                                    node.qualification_decisions,
                                    node.expression,
                                )?;
                                self.completed(identities, place, completion, continuation)?;
                                return Ok(ValueCompletion::Awaiting {
                                    argument: StructuralArgument {
                                        place,
                                        path: Vec::new(),
                                        access: StructuralAccess::Owned,
                                    },
                                    occurrence: completion,
                                });
                            }
                            let selected = if let Some(place) = self.parameter_source(&argument)? {
                                StructuralArgument {
                                    place,
                                    path: Vec::new(),
                                    access: StructuralAccess::Owned,
                                }
                            } else {
                                self.structural_bindings()
                                    .owned_argument(identities, &argument)?
                            };
                            if let Some(continuation) = continuation {
                                self.complete_operand(
                                    identities,
                                    selected,
                                    completion,
                                    continuation,
                                )?;
                                Ok(continuation.place)
                            } else {
                                return Ok(ValueCompletion::Awaiting {
                                    argument: selected,
                                    occurrence: completion,
                                });
                            }
                        }
                        CheckedStructuralValueKind::Case(construction) => {
                            let place = place_id(allocate_dense(self.environment.next_place)?);
                            let slot = crate::scalar_lowering::computations::cases::reserve(
                                identities,
                                self.checked,
                                &construction,
                                self.structural_types,
                                place,
                            )?;
                            let fields = fields(self.checked, &construction)?.to_vec();
                            let field_start = self.values.len();
                            for (ordinal, field) in fields.iter().enumerate() {
                                let field_ordinal = u32::try_from(ordinal).map_err(|_| {
                                    LoweringError::Unsupported("structural field ordinal overflow")
                                })?;
                                let value = self.scalar(
                                    identities,
                                    CheckedScalarExpressionRole::StructuralValueField {
                                        expression: construction.expression,
                                        field_ordinal,
                                    },
                                    field.value,
                                    field_start,
                                )?;
                                self.values.push(value);
                            }
                            let completed = slot.construction(&self.values[field_start..])?;
                            crate::scalar_lowering::computations::cases::emit(
                                &completed,
                                &self.values[field_start..],
                                self.next_value,
                                self.operations,
                                self.calls,
                            )?;
                            let declaration = declarations(&self.operations.operations)
                                .find(|declaration| declaration.id == place)
                                .ok_or(LoweringError::Unsupported(
                                    "structural case result declaration missing",
                                ))?;
                            self.environment.private_places.push(declaration);
                            self.values.truncate(field_start);
                            finish!(place)
                        }
                        CheckedStructuralValueKind::StructuralCase { .. } => {
                            let ValueOutcome::Normal(place) =
                                self.structural_case(identities, value)?
                            else {
                                return Ok(ValueCompletion::NoNormal);
                            };

                            finish!(place)
                        }
                        CheckedStructuralValueKind::Dispatch { subject, arms } => match self
                            .dispatch(
                                identities,
                                value,
                                node.expression,
                                subject,
                                arms,
                                continuation,
                            )? {
                            ValueOutcome::Normal(place) => Ok(place),
                            ValueOutcome::NoNormal => return Ok(ValueCompletion::NoNormal),
                        },
                    }?;
                    Ok(ValueCompletion::Awaiting {
                        argument: StructuralArgument {
                            place,
                            path: Vec::new(),
                            access: StructuralAccess::Owned,
                        },
                        occurrence: completion,
                    })
                }

                fn materialize_place(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    source: StructuralArgument,
                    value: CheckedStructuralValueHandle,
                    first_new_place: u64,
                ) -> Result<PlaceId, LoweringError> {
                    let mut consumed_value = value;
                    while let CheckedStructuralValueKind::Projection { source, path, .. } =
                        &structural_values(self.checked)
                            .nodes
                            .get(consumed_value)
                            .kind
                        && path.is_empty()
                    {
                        consumed_value = *source;
                    }
                    let node = structural_values(self.checked).nodes.get(consumed_value);
                    let expression = if validation::completed_storage::consumer_captures(
                        &self.checked.facts.flow.control,
                        self.state,
                        self.statement as usize,
                        node.expression,
                    )
                    .is_some()
                    {
                        node.expression
                    } else {
                        structural_values(self.checked).nodes.get(value).expression
                    };
                    let completed =
                        self.capture_operand(identities, source, expression, first_new_place)?;
                    self.materialize_completed_operand(identities, completed)
                }

                fn materialize_completed_operand(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    completed: super::operand_claims::CompletedOperand,
                ) -> Result<PlaceId, LoweringError> {
                    let source = &completed.argument;
                    let source_claims = &completed.claims;
                    let block = block_id(allocate_dense(self.next_block)?);
                    let place = place_id(allocate_dense(self.environment.next_place)?);
                    let mut structural_parameters = Vec::new();
                    let position = u32::try_from(structural_parameters.len()).map_err(|_| {
                        LoweringError::Unsupported("owned move parameter count exceeds u32")
                    })?;
                    structural_parameters
                        .push(self.value_binding_parameter(identities, source, place, position)?);
                    let parameters = fresh_value_declarations(self.next_value, self.values)?;
                    self.environment
                        .private_places
                        .push(StructuralPlaceDeclaration {
                            id: place,
                            kind: StructuralPlaceKind::BlockParameter { block, position },
                        });
                    let mut continuation = ValueContinuation {
                        block,
                        parameters,
                        structural_parameters,
                        place,
                        remaining_owners: Vec::new(),
                        claim_destinations: Vec::new(),
                        claims: source_claims.clone(),
                        common_claims: None,
                        current_claims: None,
                        has_incoming_memberships: false,
                    };
                    let source = source.clone();
                    let source_claims = source_claims.clone();
                    self.complete_value(completed, &mut continuation)?;
                    self.seal_continuation_names(&mut continuation)?;
                    self.operations.arrival_claims.push((place, source_claims));
                    self.start(block);
                    self.operations.register_structural_materialization(
                        place,
                        self.state,
                        self.statement,
                    )?;
                    if self.multiplicity != StructuralMultiplicity::Unrestricted {
                        self.evaluation.structural_locals.retain(
                            |StructuralLocalBinding { argument, .. }| {
                                argument.place != source.place
                                    || !argument.path.starts_with(&source.path)
                            },
                        );
                    }
                    *self.values = continuation.parameters.clone();
                    self.evaluation.parameters = continuation.parameters;
                    self.evaluation.block_structural_parameters =
                        continuation.structural_parameters;
                    Ok(place)
                }

                fn direct_borrow(
                    &mut self,
                    argument: StructuralArgument,
                ) -> Result<PlaceId, LoweringError> {
                    let join = block_id(allocate_dense(self.next_block)?);
                    let place = place_id(allocate_dense(self.environment.next_place)?);
                    let position = 0u32;
                    self.environment
                        .private_places
                        .push(block_parameter_declaration(place, join, position));
                    let structural_parameters = vec![structural_parameter_declaration(
                        place,
                        position,
                        self.structural_type,
                        self.multiplicity,
                        self.result_access,
                    )];
                    let parameters = fresh_value_declarations(self.next_value, self.values)?;
                    let mut continuation = ValueContinuation {
                        block: join,
                        parameters,
                        structural_parameters,
                        place,
                        remaining_owners: Vec::new(),
                        claim_destinations: Vec::new(),
                        claims: Vec::new(),
                        common_claims: None,
                        current_claims: None,
                        has_incoming_memberships: false,
                    };
                    let loan = match argument.access {
                        StructuralAccess::SharedBorrow => LoanAccess::Shared,
                        StructuralAccess::MutableBorrow => LoanAccess::Mutable,
                        _ => return unsupported("direct borrow requires readable loan access"),
                    };
                    self.complete_loan(argument, &mut continuation, loan)?;
                    self.seal_continuation_names(&mut continuation)?;
                    self.start(join);
                    *self.values = continuation.parameters.clone();
                    self.evaluation.parameters = continuation.parameters;
                    self.evaluation.block_structural_parameters =
                        continuation.structural_parameters;
                    Ok(place)
                }

                fn copied_leaf_place(
                    &mut self,
                    identities: &terminal_psi::TerminalIdentityTableBuilder,
                    argument: &StructuralArgument,
                    qualification_decisions: arena::HandleSpan<
                        checked_trees::CheckedValueQualification,
                    >,
                    expression: typed_trees::expression::ExpressionHandle,
                ) -> Result<PlaceId, LoweringError> {
                    let path = self.copied_place_path(argument)?;
                    let (mut qualifications, mut projected_qualifications) =
                        self.occurrence_memberships(identities, argument, expression)?;
                    terminal_semantics::present_structural_memberships(
                        self.structural_types.iter(),
                        self.domain_declarations.iter(),
                        self.structural_type,
                        &mut qualifications,
                        &mut projected_qualifications,
                    )
                    .map_err(LoweringError::Unsupported)?;
                    let (membership_carrier, membership_path) =
                        self.checked_membership_subject()?;
                    let domains = self
                        .checked_membership_domains(qualification_decisions, membership_carrier)?;
                    if membership_path.is_empty() {
                        qualifications.extend(domains);
                    } else {
                        terminal_psi::StructuralPathQualification::append_under(
                            &domains,
                            &[],
                            membership_path,
                            &mut projected_qualifications,
                        );
                    }
                    qualifications.sort();
                    qualifications.dedup();
                    projected_qualifications.sort();
                    projected_qualifications.dedup();
                    let place = place_id(allocate_dense(self.environment.next_place)?);
                    let additional_input_qualifications = self.selected_input_qualifications(
                        argument,
                        &qualifications,
                        &projected_qualifications,
                    )?;
                    let first_operation = self.operations.operations.len();
                    let operation = self.operations.allocate();
                    self.operations.push(terminal_psi::Operation::unsuspended(
                        operation,
                        OperationResult::Structural(terminal_psi::StructuralOperationResult {
                            qualification_establishments: Vec::new(),
                            place,
                            structural_type: self.structural_type,
                            multiplicity: self.multiplicity,
                            qualifications,
                            projected_qualifications,
                            additional_input_qualifications,
                            claims: Vec::new(),
                        }),
                        OperationKind::StructuralLeafCopy {
                            source: argument.place,
                            path,
                        },
                    ));
                    self.environment.private_places.push(operation_result_place(
                        place,
                        operation,
                        self.structural_type,
                    ));
                    self.operations.register_structural_materialization(
                        place,
                        self.state,
                        self.statement,
                    )?;
                    self.evaluation
                        .current_claims
                        .publish_operations(&self.operations.operations[first_operation..])?;
                    Ok(place)
                }

                fn structural_bindings(&self) -> ScalarBindings {
                    ScalarBindings::new(self.values.iter().map(|value| value.scalar_type))
                        .with_callables_from(&self.evaluation.scalar_bindings)
                        .with_structural_parameters(&self.evaluation.structural_parameters)
                        .with_structural_locals(&self.evaluation.structural_locals)
                        .with_structural_observations(self.structural_types)
                }

                pub(crate) fn scalar(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    role: CheckedScalarExpressionRole,
                    value: checked_trees::CheckedScalarComputationHandle,
                    source_count: usize,
                ) -> Result<ValueDeclaration, LoweringError> {
                    let value = self.evaluation.source_value(
                        identities,
                        self.checked,
                        self.machine,
                        self.state,
                        self.statement,
                        role,
                        &checked_trees::CheckedCallScalarArgument::Computation(value),
                        source_count,
                        self.values,
                        self.next_value,
                        self.next_block,
                        self.next_edge,
                        self.operations,
                        self.calls,
                        self.environment,
                    )?;
                    Ok(value)
                }

                pub(crate) fn boolean_case_split(
                    &mut self,
                    kind: OperationKind,
                    fallback: &mut Option<FallbackContinuation>,
                ) -> Result<(), LoweringError> {
                    let condition = value_id(allocate_dense(self.next_value)?);
                    let id = self.operations.allocate();
                    self.operations.push(terminal_psi::Operation::unsuspended(
                        id,
                        scalar_result_declaration(condition, ScalarType::Boolean),
                        kind,
                    ));
                    self.boolean_value_split(condition, fallback)
                }

                pub(crate) fn boolean_value_split(
                    &mut self,
                    condition: semantic_vocabulary::ValueId,
                    fallback: &mut Option<FallbackContinuation>,
                ) -> Result<(), LoweringError> {
                    let selected = block_id(allocate_dense(self.next_block)?);
                    let next = block_id(allocate_dense(self.next_block)?);
                    let when_true = self.edge(selected, Vec::new(), Vec::new())?;
                    let when_false = self.edge(next, Vec::new(), Vec::new())?;
                    self.finish(Terminator::Conditional {
                        condition,
                        when_true,
                        when_false,
                    });
                    *fallback = Some(FallbackContinuation {
                        block: next,
                        values: self.values.clone(),
                        current_claims: self.evaluation.current_claims.clone(),
                        sources: self.evaluation.structural_source_frontier(),
                    });
                    self.start(selected);
                    Ok(())
                }

                fn finish_jump(
                    &mut self,
                    edge: SuccessorEdge,
                    cleanup_actions: Vec<terminal_psi::TerminalAffineCleanupAction>,
                ) {
                    self.finish(Terminator::Jump {
                        edge: edge.edge,
                        target: edge.target,
                        arguments: edge.arguments,
                        erased_arguments: Vec::new(),
                        erased_proof_arguments: Vec::new(),
                        structural_arguments: edge.structural_arguments,
                        cleanup_actions,
                    });
                }

                fn edge(
                    &mut self,
                    target: BlockId,
                    arguments: Vec<ValueId>,
                    structural_arguments: Vec<StructuralArgument>,
                ) -> Result<SuccessorEdge, LoweringError> {
                    Ok(SuccessorEdge {
                        edge: edge_id(allocate_dense(self.next_edge)?),
                        target,
                        arguments,
                        structural_arguments,
                        erased_arguments: Vec::new(),
                        erased_proof_arguments: Vec::new(),
                        cleanup_actions: Vec::new(),
                    })
                }

                fn finish(&mut self, terminator: Terminator) {
                    self.evaluation.blocks.push(Block {
                        id: self.evaluation.current,
                        parameters: std::mem::take(&mut self.evaluation.parameters),
                        erased_scalar_formals: Vec::new(),
                        erased_proof_formals: Vec::new(),
                        structural_parameters: std::mem::take(
                            &mut self.evaluation.block_structural_parameters,
                        ),
                        operations: self.operations[self.evaluation.operation_start..].to_vec(),
                        terminator,
                    });
                }

                fn start(&mut self, block: BlockId) {
                    self.evaluation.current = block;
                    self.evaluation.operation_start = self.operations.len();
                    self.operations.storage_observations.clear();
                }

                fn restore_fallback(
                    &mut self,
                    fallback: Option<FallbackContinuation>,
                ) -> Result<(), LoweringError> {
                    if let Some(fallback) = fallback {
                        self.evaluation.current_claims = fallback.current_claims;
                        self.evaluation
                            .restore_structural_sources(fallback.sources)?;
                        self.start(fallback.block);
                        *self.values = fallback.values;
                    }
                    Ok(())
                }

                pub(crate) fn scalar_field_value(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    field: &terminal_psi::StructuralFieldDeclaration,
                    evaluated: ValueDeclaration,
                    positions: &mut Vec<(usize, usize)>,
                    field_index: usize,
                ) -> Result<terminal_psi::RecordFieldValue, LoweringError> {
                    if field.field_type.scalar_type() != Some(evaluated.scalar_type) {
                        return unsupported(
                            "field initializer requires its exact scalar carrier and range evidence",
                        );
                    }
                    let receiving_type = self
                        .evaluation
                        .shared_qualifications
                        .catalog()
                        .fields
                        .iter()
                        .find(|row| row.field == field.id)
                        .map(|row| row.value_type)
                        .unwrap_or(evaluated.value_type());
                    let evaluated = self.evaluation.receiving_value(
                        identities,
                        evaluated,
                        receiving_type,
                        self.values,
                        self.next_value,
                        self.next_block,
                        self.next_edge,
                        self.operations,
                        self.calls,
                        self.environment,
                    )?;
                    positions.push((field_index, self.values.len()));
                    self.values.push(evaluated);
                    let range_obligation =
                        if terminal_semantics::scalar_field_requires_initialization_evidence(
                            &field.field_type,
                        ) {
                            Some(obligation_id(allocate_dense(
                                &mut self.calls.next_obligation_identity,
                            )?))
                        } else {
                            None
                        };
                    Ok(terminal_psi::RecordFieldValue::Scalar {
                        value: evaluated.id,
                        range_obligation,
                    })
                }

                fn root_value_continuation(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    value: CheckedStructuralValueHandle,
                    values: Vec<ValueDeclaration>,
                ) -> Result<ValueContinuation, LoweringError> {
                    let join = block_id(allocate_dense(self.next_block)?);
                    let parameters = fresh_value_declarations(self.next_value, &values)?;
                    let place = place_id(allocate_dense(self.environment.next_place)?);
                    self.environment
                        .private_places
                        .push(block_parameter_declaration(place, join, 0));
                    let mut claims = Vec::new();
                    for path in
                        crate::emission::claim_bindings::completed_value_paths(self.checked, value)?
                    {
                        let claim = self
                            .calls
                            .claims
                            .receive_completed_value_claim(value, &path)?;
                        let path = lower_static_argument_path(
                            identities,
                            &path,
                            self.structural_type,
                            self.structural_types,
                        )?;
                        self.operations
                            .block_entry_claims
                            .push(terminal_psi::EntryClaim {
                                claim,
                                input: place,
                                path: path.clone(),
                            });
                        claims.push(terminal_psi::StructuralResultClaimBinding { claim, path });
                    }
                    claims.sort();
                    Ok(ValueContinuation {
                        block: join,
                        parameters,
                        structural_parameters: vec![structural_parameter_declaration(
                            place,
                            0,
                            self.structural_type,
                            self.multiplicity,
                            self.result_access,
                        )],
                        place,
                        remaining_owners: self.owners.clone(),
                        claim_destinations: Vec::new(),
                        claims,
                        common_claims: None,
                        current_claims: None,
                        has_incoming_memberships: false,
                    })
                }

                fn seal_continuation_names(
                    &mut self,
                    continuation: &mut ValueContinuation,
                ) -> Result<(), LoweringError> {
                    crate::emission::claim_bindings::seal_receiving_claims(
                        self.operations,
                        continuation.place,
                        &mut continuation.claims,
                        continuation.common_claims.as_deref(),
                    )?;
                    if let Some(current) = continuation.current_claims.take() {
                        self.evaluation.current_claims = current;
                    }
                    self.evaluation
                        .current_claims
                        .publish(continuation.place, continuation.claims.clone())?;
                    Ok(())
                }

                fn record_root_continuation(
                    &mut self,
                    continuation: &mut ValueContinuation,
                ) -> Result<(), LoweringError> {
                    self.seal_continuation_names(continuation)?;
                    self.start(continuation.block);
                    if self.result_access == StructuralAccess::Owned {
                        self.operations.register_structural_materialization(
                            continuation.place,
                            self.state,
                            self.statement,
                        )?;
                    }
                    *self.values = continuation.parameters.clone();
                    self.evaluation.parameters = continuation.parameters.clone();
                    self.evaluation.block_structural_parameters =
                        continuation.structural_parameters.clone();
                    self.operations
                        .arrival_claims
                        .push((continuation.place, continuation.claims.clone()));
                    self.evaluation.structural_value_owners = continuation.remaining_owners.clone();
                    self.evaluation.structural_locals.retain(
                        |StructuralLocalBinding { argument, .. }| {
                            !self
                                .sources
                                .iter()
                                .any(|source| source.place == argument.place)
                        },
                    );
                    Ok(())
                }
            }
        }
        pub(crate) mod field_carrier {

            use crate::LoweringError;
            use checked_trees::{CheckedTrees, types::TypeReferenceHandle};
            use semantic_vocabulary::StructuralTypeId;
            use terminal_psi::{StructuralFieldType, StructuralTypeDeclaration};

            pub(crate) fn resolve(
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                checked: &CheckedTrees,
                declarations: &[StructuralTypeDeclaration],
                field: &StructuralFieldType,
                reference: TypeReferenceHandle,
            ) -> Result<StructuralTypeId, LoweringError> {
                if let Some(identity) = field.as_structural() {
                    return Ok(identity);
                }
                if field.byte_sequence_carrier().is_none() {
                    return Err(LoweringError::Unsupported(
                        "structural operand has a nonstructural field destination",
                    ));
                }
                let identity =
                    validation::structural_type_identity(&checked.typed, reference, &[], &[]);
                let declaration =
                    crate::shared::terminal_identities::structural_declaration_by_identity(
                        declarations,
                        identities,
                        checked,
                        identity,
                        "byte member has no retained carrier",
                    )?;
                Ok(declaration.id)
            }
        }
        pub(crate) mod fixed_array {
            use super::super::{
                LoweringError, Operation, OperationKind, PlaceId, StructuralTypeShape,
                allocate_dense, place_id, unsupported,
            };
            use super::completed_aggregate::{self, CompletedAggregate};
            use super::emission;
            use crate::emission::operation_emission::buffer::structural_result_declaration;
            use crate::emission::operation_emission::place_declarations::operation_result_place;
            use crate::unit::attached_unit::structural_values::ValueOutcome;
            use checked_trees::CheckedStructuralValueHandle;

            impl emission::Emission<'_, '_, '_> {
                pub(crate) fn fixed_array(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    elements: &[CheckedStructuralValueHandle],
                    type_reference: checked_trees::types::TypeReferenceHandle,
                ) -> Result<ValueOutcome<PlaceId>, LoweringError> {
                    let declaration = terminal_psi::find_structural_type(
                        self.structural_types,
                        self.structural_type,
                    )
                    .ok_or(LoweringError::Unsupported("fixed array carrier is missing"))?;
                    let StructuralTypeShape::FixedArray { element, .. } = declaration.shape else {
                        return unsupported("fixed array literal has a nonarray carrier");
                    };
                    let reference = self.checked.facts.resolved_types.carrier(type_reference);
                    if !reference.is_valid() {
                        return unsupported("fixed array source carrier is missing");
                    }
                    let checked_trees::types::TypeReferenceNode::FixedArray {
                        element_type, ..
                    } = self.checked.type_reference_table.type_reference(reference)
                    else {
                        return unsupported("fixed array source has a nonarray carrier");
                    };
                    let element_reference = *element_type;
                    let parent_type = self.structural_type;
                    let parent_multiplicity = self.multiplicity;
                    let source_count = self.values.len();
                    let mut operands = Vec::with_capacity(elements.len());
                    let mut claims = Vec::new();
                    for (element_position, value) in elements.iter().enumerate() {
                        let element_index = u64::try_from(element_position).map_err(|_| {
                            LoweringError::Unsupported("array element identity overflow")
                        })?;
                        let ValueOutcome::Normal(operand) = self.child_constructor_operand(
                            identities,
                            element,
                            *value,
                            element_reference,
                        )?
                        else {
                            return Ok(ValueOutcome::NoNormal);
                        };
                        for claim in operand.claims {
                            let mut path = vec![terminal_psi::StructuralPathSegment::FixedIndex(
                                element_index,
                            )];
                            path.extend(claim.path);
                            claims.push(terminal_psi::StructuralResultClaimBinding {
                                claim: claim.claim,
                                path,
                            });
                        }
                        operands.push(operand.argument);
                    }
                    claims.sort();
                    let place = place_id(allocate_dense(self.environment.next_place)?);
                    let declaration = completed_aggregate::emit_completed(
                        place,
                        parent_type,
                        parent_multiplicity,
                        CompletedAggregate::Array { elements: operands },
                        self.evaluation.structural_root_parameters(),
                        self.operations,
                    )?;
                    self.retire_completed_operands(&declaration)?;
                    self.environment.private_places.push(declaration);
                    self.operations
                        .set_structural_result_claims(place, claims)?;
                    self.values.truncate(source_count);
                    Ok(ValueOutcome::Normal(place))
                }

                pub(crate) fn establish_zeroed(&mut self) -> Result<PlaceId, LoweringError> {
                    let place = place_id(allocate_dense(self.environment.next_place)?);
                    let operation = self.operations.allocate();
                    self.operations.push(Operation::unsuspended(
                        operation,
                        structural_result_declaration(
                            place,
                            self.structural_type,
                            crate::emission::borrowed_window::zeroed_image(self.multiplicity),
                        ),
                        OperationKind::EstablishZeroedStructural {},
                    ));
                    self.environment.private_places.push(operation_result_place(
                        place,
                        operation,
                        self.structural_type,
                    ));
                    Ok(place)
                }
            }
        }
        pub(crate) mod locals {
            use super::super::CheckedUnitEffectOperationPlan;
            use crate::expression_preparation::bindings::StructuralLocalBinding;
            use crate::expression_preparation::bindings::structural_paths::terminal_access;
            use checked_trees::CheckedArrayConstructionSource;

            pub(crate) fn local_binding(
                operation: &CheckedUnitEffectOperationPlan,
                declaration: terminal_psi::StructuralPlaceDeclaration,
                structural_type: semantic_vocabulary::StructuralTypeId,
                structural_types: &[terminal_psi::StructuralTypeDeclaration],
            ) -> Option<StructuralLocalBinding> {
                let result = match operation {
                    CheckedUnitEffectOperationPlan::EstablishLocal(
                        checked_trees::CheckedUnitLocalBindingPlan::ScalarArray {
                            source: CheckedArrayConstructionSource::Statement,
                            result,
                            ..
                        },
                    ) => result,
                    CheckedUnitEffectOperationPlan::Call(call)
                        if let Some(result) = call.structural_result() =>
                    {
                        result
                    }
                    _ => return None,
                };
                let local = result.local;
                if !local.is_named() {
                    return None;
                }
                let borrowed_access = (local.access
                    != checked_trees::CheckedStructuralAccess::Owned)
                    .then(|| terminal_access(local.access));
                Some(StructuralLocalBinding {
                    result_binding_ordinal: Some(result.binding_ordinal),
                    root: facts::PlaceRoot::Symbol(local.symbol),
                    structural_type,
                    argument: terminal_psi::StructuralArgument {
                        place: declaration.id,
                        access: if structural_types.iter().any(|declaration| {
                            declaration.id == structural_type
                                && matches!(
                                    declaration.shape,
                                    terminal_psi::StructuralTypeShape::Reference { .. }
                                )
                        }) {
                            terminal_psi::StructuralAccess::Owned
                        } else {
                            borrowed_access.unwrap_or(terminal_psi::StructuralAccess::Owned)
                        },
                        path: Vec::new(),
                    },
                })
            }
        }
        pub(crate) mod member_memberships {

            use crate::emission::operation_emission::buffer::OperationBuffer;
            use terminal_psi::{
                StructuralArgument, StructuralParameterDeclaration, StructuralPathQualification,
                StructuralPathSegment,
            };

            pub(crate) fn append<'parameter>(
                argument: &StructuralArgument,
                prefix: &[StructuralPathSegment],
                parameters: impl Iterator<Item = &'parameter StructuralParameterDeclaration>,
                operations: &OperationBuffer,
                destination: &mut Vec<StructuralPathQualification>,
                additional_destination: &mut Vec<StructuralPathQualification>,
            ) {
                if let Some((_, whole, projected, additional)) =
                    source_memberships(argument.place, parameters, operations)
                {
                    let (whole, projected) =
                        StructuralPathQualification::select(whole, projected, &argument.path);
                    StructuralPathQualification::append_under(
                        &whole,
                        &projected,
                        prefix,
                        destination,
                    );
                    let (whole, projected) =
                        StructuralPathQualification::select(&[], additional, &argument.path);
                    StructuralPathQualification::append_under(
                        &whole,
                        &projected,
                        prefix,
                        additional_destination,
                    );
                }
            }

            pub(crate) fn source_memberships<'parameter: 'source, 'source>(
                place: semantic_vocabulary::PlaceId,
                parameters: impl Iterator<Item = &'parameter StructuralParameterDeclaration>,
                operations: &'source OperationBuffer,
            ) -> Option<(
                semantic_vocabulary::StructuralTypeId,
                &'source [semantic_vocabulary::StructuralDomainId],
                &'source [StructuralPathQualification],
                &'source [StructuralPathQualification],
            )> {
                source_memberships_in(place, parameters, operations.iter())
            }

            pub(crate) fn source_memberships_in<'parameter: 'source, 'source>(
                place: semantic_vocabulary::PlaceId,
                parameters: impl Iterator<Item = &'parameter StructuralParameterDeclaration>,
                operations: impl Iterator<Item = &'source terminal_psi::Operation>,
            ) -> Option<(
                semantic_vocabulary::StructuralTypeId,
                &'source [semantic_vocabulary::StructuralDomainId],
                &'source [StructuralPathQualification],
                &'source [StructuralPathQualification],
            )> {
                operations
                    .filter_map(|operation| operation.result.structural())
                    .find(|result| result.place == place)
                    .map(|result| {
                        (
                            result.structural_type,
                            result.qualifications.as_slice(),
                            result.projected_qualifications.as_slice(),
                            result.additional_input_qualifications.as_slice(),
                        )
                    })
                    .or_else(|| {
                        parameters
                            .into_iter()
                            .find(|parameter| parameter.place == place)
                            .map(|parameter| {
                                (
                                    parameter.structural_type,
                                    parameter.qualifications.as_slice(),
                                    parameter.projected_qualifications.as_slice(),
                                    parameter.additional_input_qualifications.as_slice(),
                                )
                            })
                    })
            }
        }
        pub(crate) mod operand_claims {

            use super::super::{LoweringError, StructuralMultiplicity, unsupported};
            use super::emission;

            pub(crate) struct CompletedOperand {
                pub(crate) argument: terminal_psi::StructuralArgument,
                pub(crate) claims: Vec<terminal_psi::StructuralResultClaimBinding>,
            }

            impl CompletedOperand {
                fn from_completed(
                    argument: terminal_psi::StructuralArgument,
                    claims: Option<Vec<terminal_psi::StructuralResultClaimBinding>>,
                ) -> Result<Self, LoweringError> {
                    Self::checked(
                        argument,
                        claims.ok_or(LoweringError::Unsupported(
                            "completed operand has no published custody",
                        ))?,
                    )
                }

                fn checked(
                    argument: terminal_psi::StructuralArgument,
                    claims: Vec<terminal_psi::StructuralResultClaimBinding>,
                ) -> Result<Self, LoweringError> {
                    if claims.iter().enumerate().any(|(index, claim)| {
                        claims[..index]
                            .iter()
                            .any(|prior| prior.claim == claim.claim || prior.path == claim.path)
                    }) {
                        return unsupported("completed operand repeats a claim or path");
                    }
                    Ok(Self { argument, claims })
                }
            }

            impl emission::Emission<'_, '_, '_> {
                pub(crate) fn completed_operand(
                    &self,
                    argument: terminal_psi::StructuralArgument,
                ) -> Result<CompletedOperand, LoweringError> {
                    let claims = self.evaluation.current_claims.source(&argument)?;
                    CompletedOperand::from_completed(argument, claims)
                }

                pub(crate) fn capture_operand(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    argument: terminal_psi::StructuralArgument,
                    expression: typed_trees::expression::ExpressionHandle,
                    first_new_place: u64,
                ) -> Result<CompletedOperand, LoweringError> {
                    let argument = self.refresh_memberships(
                        identities,
                        argument,
                        expression,
                        first_new_place,
                    )?;
                    self.capture_operand_claims(identities, argument, expression, first_new_place)
                }

                pub(crate) fn capture_operand_claims(
                    &self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    argument: terminal_psi::StructuralArgument,
                    expression: typed_trees::expression::ExpressionHandle,
                    first_new_place: u64,
                ) -> Result<CompletedOperand, LoweringError> {
                    let completed_claims =
                        if self.multiplicity == StructuralMultiplicity::Unrestricted {
                            None
                        } else {
                            self.evaluation.current_claims.source(&argument)?
                        };
                    if self.multiplicity != StructuralMultiplicity::Unrestricted
                        && argument.place.get() >= first_new_place
                    {
                        return CompletedOperand::from_completed(argument, completed_claims);
                    }
                    let claims = if self.multiplicity == StructuralMultiplicity::Unrestricted {
                        Vec::new()
                    } else {
                        crate::emission::claim_bindings::lower_owned_operand_claims(
                            identities,
                            self.checked,
                            self.machine,
                            self.state,
                            self.statement,
                            expression,
                            None,
                            None,
                            self.calls.claims.bindings(),
                            completed_claims.as_deref(),
                            self.structural_type,
                            self.structural_types,
                        )?
                    };
                    CompletedOperand::checked(argument, claims)
                }
            }
        }
        pub(crate) mod record {
            use super::super::super::StructuralFieldType;
            use super::super::{
                CheckedScalarExpressionRole, LoweringError, OperationKind, PlaceId, ScalarType,
                StructuralTypeShape, ValueDeclaration, allocate_dense, place_id, unsupported,
            };
            use super::completed_aggregate::{self, CompletedAggregate};
            use super::emission;
            use crate::emission::operation_emission::expressions::emit_scalar_leaf;
            use crate::shared::checked_facts::value_facts;
            use crate::unit::attached_unit::structural_values::ValueOutcome;
            use semantic_vocabulary::{IeeeFloatFormat, IeeeFloatValue};

            impl emission::Emission<'_, '_, '_> {
                pub(crate) fn record(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    value: checked_trees::CheckedStructuralValueHandle,
                ) -> Result<ValueOutcome<PlaceId>, LoweringError> {
                    let node = value_facts(self.checked)
                        .structural_values
                        .nodes
                        .get(value)
                        .clone();
                    let checked_trees::CheckedStructuralValueKind::Record {
                        data_symbol,
                        fields,
                    } = node.kind
                    else {
                        return unsupported("record producer missing");
                    };
                    let fields = self.structural_value_fields(fields, "record fields are stale")?;
                    let record = self
                        .checked
                        .data_definition_by_symbol(data_symbol)
                        .ok_or(LoweringError::Unsupported("record declaration missing"))?;
                    let declarations = self.checked.data_members(record);
                    let shape = terminal_psi::find_structural_type(
                        self.structural_types,
                        self.structural_type,
                    )
                    .ok_or(LoweringError::Unsupported("record structural type missing"))?;
                    let StructuralTypeShape::Record {
                        fields: shape_fields,
                        ..
                    } = &shape.shape
                    else {
                        return unsupported("record construction has a nonrecord shape");
                    };
                    let shape_fields = shape_fields.clone();
                    let source_count = self.values.len();
                    let mut initialized = Vec::new();
                    let mut scalar_positions = Vec::new();
                    let mut claims = Vec::new();
                    for (ordinal, field) in fields.iter().enumerate() {
                        let declaration = declarations
                            .iter()
                            .find_map(|member| match member {
                                checked_trees::data::DataMember::Field(declaration)
                                    if declaration.symbol == field.field =>
                                {
                                    Some(declaration)
                                }
                                _ => None,
                            })
                            .ok_or(LoweringError::Unsupported(
                                "record field declaration missing",
                            ))?;
                        let identity = checked_trees::CheckedMemberIdentity::of_member(
                            declaration.identity,
                            declaration.name.clone(),
                        );
                        let position = shape_fields
                            .iter()
                            .position(|candidate| {
                                candidate.identity == identities.intern(&identity.spelling())
                            })
                            .ok_or(LoweringError::Unsupported("record field identity missing"))?;
                        let target = &shape_fields[position];
                        if target.relevance.is_erased() {
                            if !matches!(target.field_type, StructuralFieldType::Erased { .. }) {
                                return unsupported("erased record field has a runtime carrier");
                            }
                            continue;
                        }
                        let ValueOutcome::Normal(value) = self.completed_member_value(
                            identities,
                            field,
                            target,
                            CheckedScalarExpressionRole::RecordField {
                                expression: node.expression,
                                field_ordinal: u32::try_from(ordinal).map_err(|_| {
                                    LoweringError::Unsupported("record field ordinal overflow")
                                })?,
                            },
                            None,
                            &mut scalar_positions,
                            initialized.len(),
                            source_count,
                            &mut claims,
                        )?
                        else {
                            return Ok(ValueOutcome::NoNormal);
                        };
                        initialized.push((
                            position,
                            terminal_psi::RecordFieldInitializer {
                                field: target.id,
                                value,
                            },
                        ));
                    }
                    self.remap_saved_scalar_fields(
                        scalar_positions,
                        &mut initialized,
                        source_count,
                    );
                    initialized.sort_by_key(|(position, _)| *position);
                    claims.sort();
                    let place = place_id(allocate_dense(self.environment.next_place)?);
                    let declaration = completed_aggregate::emit_completed(
                        place,
                        self.structural_type,
                        self.multiplicity,
                        CompletedAggregate::Record {
                            fields: initialized.into_iter().map(|(_, field)| field).collect(),
                            declarations: &shape_fields,
                        },
                        self.evaluation.structural_root_parameters(),
                        self.operations,
                    )?;
                    self.retire_completed_operands(&declaration)?;
                    self.operations
                        .set_structural_result_claims(place, claims)?;
                    self.environment.private_places.push(declaration);
                    Ok(ValueOutcome::Normal(place))
                }

                pub(crate) fn zeroed_scalar_field(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    value: checked_trees::CheckedStructuralValueHandle,
                    scalar_type: ScalarType,
                    field: &terminal_psi::StructuralFieldDeclaration,
                    positions: &mut Vec<(usize, usize)>,
                    field_index: usize,
                ) -> Result<terminal_psi::RecordFieldValue, LoweringError> {
                    if !matches!(
                        value_facts(self.checked)
                            .structural_values
                            .nodes
                            .get(value)
                            .kind,
                        checked_trees::CheckedStructuralValueKind::ZeroedStructural { .. }
                    ) {
                        return unsupported("nested constructor operand has a scalar destination");
                    }
                    let zero = emit_scalar_leaf(
                        zero_constant(scalar_type)?,
                        scalar_type,
                        self.next_value,
                        self.operations,
                    );
                    self.scalar_field_value(
                        identities,
                        field,
                        ValueDeclaration {
                            qualifications: Default::default(),
                            id: zero,
                            scalar_type,
                        },
                        positions,
                        field_index,
                    )
                }
            }

            pub(crate) fn zero_constant(
                scalar_type: ScalarType,
            ) -> Result<OperationKind, LoweringError> {
                Ok(match scalar_type {
                    ScalarType::Integer(integer) => OperationKind::IntegerConstant {
                        value: integer.zero(),
                    },
                    ScalarType::Boolean => OperationKind::BooleanConstant { value: false },
                    ScalarType::Callable(_) => {
                        return unsupported(
                            "machine values require a selected code descriptor and cannot be zero initialized",
                        );
                    }
                    ScalarType::IeeeFloat(format) => OperationKind::IeeeFloatConstant {
                        value: match format {
                            IeeeFloatFormat::Binary32 => IeeeFloatValue::Binary32(0),
                            IeeeFloatFormat::Binary64 => IeeeFloatValue::Binary64(0),
                        },
                    },
                })
            }
        }
        pub(crate) mod structural_case {
            use super::super::super::StructuralFieldType;
            use super::super::{
                CheckedScalarExpressionRole, LoweringError, PlaceId, StructuralTypeShape,
                allocate_dense, place_id, unsupported,
            };
            use super::completed_aggregate::{self, CompletedAggregate};
            use super::emission;
            use crate::shared::checked_facts::value_facts;
            use crate::unit::attached_unit::structural_values::ValueOutcome;

            impl emission::Emission<'_, '_, '_> {
                pub(crate) fn structural_case(
                    &mut self,
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    value: checked_trees::CheckedStructuralValueHandle,
                ) -> Result<ValueOutcome<PlaceId>, LoweringError> {
                    let node = value_facts(self.checked)
                        .structural_values
                        .nodes
                        .get(value)
                        .clone();
                    let checked_trees::CheckedStructuralValueKind::StructuralCase {
                        data_symbol,
                        case: case_symbol,
                        fields,
                    } = node.kind
                    else {
                        return unsupported("structural case producer missing");
                    };
                    let fields = self.structural_value_fields(fields, "case fields are stale")?;
                    let owner = self
                        .checked
                        .data_definition_by_symbol(data_symbol)
                        .ok_or(LoweringError::Unsupported("case owner declaration missing"))?;
                    let shape = terminal_psi::find_structural_type(
                        self.structural_types,
                        self.structural_type,
                    )
                    .ok_or(LoweringError::Unsupported("case structural type missing"))?;
                    let (common, cases) = match &shape.shape {
                        StructuralTypeShape::Sum { cases } => (Vec::new(), cases.clone()),
                        StructuralTypeShape::Mixed { fields, cases } => (
                            fields
                                .iter()
                                .filter(|field| {
                                    !matches!(field.field_type, StructuralFieldType::Erased { .. })
                                })
                                .cloned()
                                .collect::<Vec<_>>(),
                            cases.clone(),
                        ),
                        _ => return unsupported("case construction has a non-sum shape"),
                    };
                    let common_declarations = self
                        .checked
                        .data_members(owner)
                        .iter()
                        .filter_map(|member| match member {
                            checked_trees::data::DataMember::Field(field) => Some(field),
                            checked_trees::data::DataMember::Variant(_) => None,
                        })
                        .collect::<Vec<_>>();
                    let mut selected = None;
                    for member in self.checked.data_members(owner) {
                        let checked_trees::data::DataMember::Variant(variant) = member else {
                            continue;
                        };
                        let identity = checked_trees::CheckedMemberIdentity::of_member(
                            variant.identity,
                            variant.name.clone(),
                        );
                        if variant.symbol == case_symbol {
                            let case = terminal_psi::structural_case_by_identity(
                                &cases,
                                identities.intern(&identity.spelling()),
                            )
                            .ok_or(LoweringError::Unsupported("case declaration is absent"))?;
                            selected = Some((variant, case.clone()));
                        }
                    }
                    let (variant, shape_case) = selected.ok_or(LoweringError::Unsupported(
                        "case construction selected a foreign case",
                    ))?;
                    let declarations = self.checked.data_payload_fields(variant);
                    let shape_fields = common
                        .iter()
                        .chain(shape_case.fields.iter().filter(|field| {
                            !matches!(field.field_type, StructuralFieldType::Erased { .. })
                        }))
                        .cloned()
                        .collect::<Vec<_>>();
                    let source_count = self.values.len();
                    let mut initialized = Vec::new();
                    let mut scalar_positions = Vec::new();
                    let mut claims = Vec::new();
                    for (ordinal, field) in fields.iter().enumerate() {
                        let (declaration, roster) = match declarations
                            .iter()
                            .find(|declaration| declaration.symbol == field.field)
                        {
                            Some(declaration) => (declaration, common.len()..shape_fields.len()),
                            None => (
                                *common_declarations
                                    .iter()
                                    .find(|declaration| declaration.symbol == field.field)
                                    .ok_or(LoweringError::Unsupported(
                                        "case field declaration missing",
                                    ))?,
                                0..common.len(),
                            ),
                        };
                        let identity = checked_trees::CheckedMemberIdentity::of_member(
                            declaration.identity,
                            declaration.name.clone(),
                        );
                        let position = shape_fields[roster.clone()]
                            .iter()
                            .position(|candidate| {
                                candidate.identity == identities.intern(&identity.spelling())
                            })
                            .map(|offset| roster.start + offset)
                            .ok_or(LoweringError::Unsupported("case field identity missing"))?;
                        let target = &shape_fields[position];
                        if matches!(target.field_type, StructuralFieldType::Erased { .. }) {
                            return unsupported("erased case member has no runtime initializer");
                        }
                        let ValueOutcome::Normal(value) = self.completed_member_value(
                            identities,
                            field,
                            target,
                            CheckedScalarExpressionRole::StructuralValueField {
                                expression: node.expression,
                                field_ordinal: u32::try_from(ordinal).map_err(|_| {
                                    LoweringError::Unsupported("case field ordinal overflow")
                                })?,
                            },
                            (roster.start == common.len()).then_some(shape_case.id),
                            &mut scalar_positions,
                            initialized.len(),
                            source_count,
                            &mut claims,
                        )?
                        else {
                            return Ok(ValueOutcome::NoNormal);
                        };
                        initialized.push((
                            position,
                            terminal_psi::RecordFieldInitializer {
                                field: target.id,
                                value,
                            },
                        ));
                    }
                    self.remap_saved_scalar_fields(
                        scalar_positions,
                        &mut initialized,
                        source_count,
                    );
                    initialized.sort_by_key(|(position, _)| *position);
                    let place = place_id(allocate_dense(self.environment.next_place)?);
                    let declaration = completed_aggregate::emit_completed(
                        place,
                        self.structural_type,
                        self.multiplicity,
                        CompletedAggregate::Case {
                            result_case: shape_case.id,
                            fields: initialized.into_iter().map(|(_, field)| field).collect(),
                            declarations: &shape_fields,
                            common_fields: common.len(),
                        },
                        self.evaluation.structural_root_parameters(),
                        self.operations,
                    )?;
                    self.retire_completed_operands(&declaration)?;
                    self.environment.private_places.push(declaration);
                    claims.sort();
                    self.operations
                        .set_structural_result_claims(place, claims)?;
                    Ok(ValueOutcome::Normal(place))
                }
            }
        }

        pub(crate) use emission::{ValueDestination, ValueProduction, emit, receive_completed};
        pub(crate) use locals::local_binding;

        pub(crate) enum ValueOutcome<T> {
            Normal(T),
            NoNormal,
        }
    }
}
