/*
    Structural operation execution: record and case construction, field projection,
    subtree transplant, and byte/array view operations in the Terminal Psi engine.

    We execute structural operations against live runtime tables while preserving
    paid cell reservations and namespace separation between common and case-payload
    fields.
*/

use crate::boundary::errors::{TerminalInterpretError, malformed};
use crate::boundary::structural_inputs::case_membership::StructuralCaseContents;
use crate::boundary::values::{StructuralByteSequenceRuntimeField, StructuralRuntimePlace};
use crate::boundary::values::{StructuralFieldNamespace, StructuralScalarRuntimeField};
use crate::boundary::values::{
    TerminalScalarAggregateValue, TerminalScalarValue, TerminalStructuralValue,
};
use crate::engine::custody::whole_path_discard;
use crate::engine::execution::{
    OperationFlow, TerminalExecution, dispatched_operation, operation_kind,
};
use crate::engine::scalar_operations::terminal_scalar_belongs_to_type;
use crate::storage::byte_sequences::ByteSequenceView;
use crate::storage::byte_sequences::binding::ByteSequenceBinding;
use crate::storage::byte_sequences::storage::ByteSequenceStorage;
use crate::storage::primitive_storage::PrimitiveStorageEndpoint;
use crate::storage::record::storage::{ConstructorStorage, ConstructorValue};
use crate::storage::reference::{AggregateViewTip, translate_aggregate_view_field};
use crate::storage::runtime_rows::RuntimeRows;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IeeeFloatFormat, IeeeFloatValue, IntegerSign, IntegerType,
    IntegerValue, ScalarType, StructuralDomainId, StructuralTypeId,
};
use terminal_psi::{
    ByteSequenceCarrier, OperationKind, StructuralAccess, StructuralArgument, StructuralFieldType,
    StructuralMultiplicity, StructuralPathQualification, StructuralPathSegment,
    StructuralTypeShape, find_structural_case, nonlinear_multiplicity_pair_pattern,
};
use terminal_semantics::{
    ZeroedStructuralLeaf, byte_carrier_scalar_layout, is_copyable_carrier,
    observed_field_declaration, record_field_carrier, scalar_array_leaf_shape,
};

struct ResolvedStructuralWindow {
    backing: TerminalStructuralValue,
    structural_type: semantic_vocabulary::StructuralTypeId,
    path: Vec<StructuralPathSegment>,
    aggregate_view: bool,
}

impl TerminalExecution<'_> {
    pub(crate) fn execute_establish_scalar_case(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        dispatched_operation!(operation => EstablishScalarCase { result_case, ref fields });
        let terminal_psi::OperationResult::Structural(result) = &operation.result else {
            return Err(malformed());
        };
        if self.structural_values.contains_key(&result.place)
            || !matches!(result.multiplicity, nonlinear_multiplicity_pair_pattern!())
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
        {
            return Err(malformed());
        }
        let value = TerminalScalarAggregateValue {
            structural_type: result.structural_type,
            result_case: Some(result_case),
            fields: fields
                .iter()
                .map(|binding| {
                    Ok((
                        vec![CanonicalStructuralPathSegment::Field(binding.field)],
                        self.scalar_value(binding.value)?,
                    ))
                })
                .collect::<Result<Vec<_>, TerminalInterpretError>>()?,
        };
        self.publish_scalar_aggregate(result, value)?;
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn publish_scalar_aggregate(
        &mut self,
        result: &terminal_psi::StructuralOperationResult,
        aggregate: TerminalScalarAggregateValue,
    ) -> Result<(), TerminalInterpretError> {
        if aggregate.structural_type != result.structural_type
            || self.structural_values.contains_key(&result.place)
            || self.scalar_array_values.contains_key(&result.place)
            || !result.projected_qualifications.is_empty()
            || (!result.claims.is_empty() && result.multiplicity != StructuralMultiplicity::Linear)
        {
            return Err(malformed());
        }
        aggregate.validate_against(
            self.structural_declaration(result.structural_type)?,
            |identity| self.structural_types().get(&identity),
        )?;
        let value = TerminalStructuralValue::minted(
            self.local_structural_identities.allocate()?,
            result.structural_type,
            result.qualifications.clone(),
        );
        let root = StructuralRuntimePlace::from(&value);
        let mut storage = ConstructorStorage::new(self.account.as_ref());
        storage.stage_scalar_aggregate(self, &aggregate, &root)?;
        storage.publish(self)?;
        self.structural_values.insert(result.place, value)?;
        self.track_affine_result(result);
        Ok(())
    }

    pub(crate) fn publish_host_reference_result(
        &mut self,
        result: &terminal_psi::StructuralOperationResult,
        supply: crate::boundary::effects::results::TerminalReferenceResult,
    ) -> Result<(), TerminalInterpretError> {
        if !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
            || self.structural_values.contains_key(&result.place)
        {
            return Err(malformed());
        }
        let (referent, _access) = self.reference_contract(result.structural_type)?;
        let value = TerminalStructuralValue::minted(
            self.local_structural_identities.allocate()?,
            result.structural_type,
            result.qualifications.clone(),
        );
        let view = self.stage_host_referent(result.place, referent, &supply)?;
        self.reference_referents.insert(
            StructuralRuntimePlace::from(&value),
            crate::storage::reference::ReferenceReferent {
                view,
                placed: None,
                primitive_loan: None,
                collection_loan: None,
                byte_owner: None,
                byte_region: None,
                aggregate_view: None,
            },
        )?;
        self.structural_host_atoms.insert(
            StructuralRuntimePlace::from(&value),
            crate::storage::structural_storage::HostStructuralPayload::Materialized(
                value.opaque_identity,
            ),
        )?;
        self.structural_values.insert(result.place, value)?;
        self.track_affine_result(result);
        Ok(())
    }

    fn stage_host_referent(
        &mut self,
        source: semantic_vocabulary::PlaceId,
        referent: StructuralTypeId,
        supply: &crate::boundary::effects::results::TerminalReferenceResult,
    ) -> Result<TerminalStructuralValue, TerminalInterpretError> {
        if let crate::boundary::effects::results::TerminalReferencePayload::ScalarAggregate(
            aggregate,
        ) = &supply.referent
        {
            if aggregate.structural_type != referent {
                return Err(malformed());
            }
            aggregate.validate_against(self.structural_declaration(referent)?, |identity| {
                self.structural_types().get(&identity)
            })?;
        }
        let mut declared = Vec::new();
        self.referent_reference_paths(referent, &mut Vec::new(), &mut declared)?;
        if declared.len() != supply.references.len()
            || !declared
                .iter()
                .all(|path| supply.references.iter().any(|(offered, _)| offered == path))
        {
            return Err(malformed());
        }
        let view = TerminalStructuralValue {
            opaque_identity: self.local_structural_identities.allocate()?,
            structural_type: referent,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            path: Vec::new(),
        };
        match &supply.referent {
            crate::boundary::effects::results::TerminalReferencePayload::ScalarAggregate(
                aggregate,
            ) => {
                let root = StructuralRuntimePlace::from(&view);
                let mut storage = ConstructorStorage::new(self.account.as_ref());
                storage.stage_scalar_aggregate(self, aggregate, &root)?;
                storage.publish(self)?;
            }
            crate::boundary::effects::results::TerminalReferencePayload::Zeroed => {
                if supply.references.is_empty() {
                    self.materialize_zeroed_structural_storage(source, &view)?;
                } else {
                    let leaves = terminal_semantics::zeroed_structural_leaves_modulo_references(
                        self.structural_types().values(),
                        referent,
                    )
                    .ok_or(
                        TerminalInterpretError::UnsupportedSemanticVariant(
                            "host referent's zeroed leaves cannot be materialized",
                        ),
                    )?;
                    self.materialize_zeroed_leaves(source, &view, &leaves)?;
                }
            }
        }
        for (path, nested) in &supply.references {
            let (nested_referent, _access) = self.host_referent_field_reference(referent, path)?;
            let nested_view = self.stage_host_referent(source, nested_referent, nested)?;
            self.reference_referents.insert(
                StructuralRuntimePlace {
                    opaque_identity: view.opaque_identity,
                    path: terminal_semantics::canonical_structural_storage_path(
                        self.structural_types().values(),
                        referent,
                        path,
                    )
                    .ok_or_else(malformed)?,
                },
                crate::storage::reference::ReferenceReferent {
                    view: nested_view,
                    placed: None,
                    primitive_loan: None,
                    collection_loan: None,
                    byte_owner: None,
                    byte_region: None,
                    aggregate_view: None,
                },
            )?;
        }
        Ok(view)
    }

    fn host_referent_field_reference(
        &self,
        referent: StructuralTypeId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Result<(StructuralTypeId, StructuralAccess), TerminalInterpretError> {
        let mut current = referent;
        for segment in path {
            let CanonicalStructuralPathSegment::Field(identity) = segment else {
                return Err(malformed());
            };
            let StructuralTypeShape::Record { fields, .. } =
                &self.structural_declaration(current)?.shape
            else {
                return Err(malformed());
            };
            let field = fields
                .iter()
                .find(|field| &field.id == identity && !field.relevance.is_erased())
                .ok_or_else(malformed)?;
            current = field.field_type.as_structural().ok_or_else(malformed)?;
        }
        self.reference_contract(current)
    }

    fn referent_reference_paths(
        &self,
        referent: StructuralTypeId,
        prefix: &mut Vec<CanonicalStructuralPathSegment>,
        found: &mut Vec<Vec<CanonicalStructuralPathSegment>>,
    ) -> Result<(), TerminalInterpretError> {
        let StructuralTypeShape::Record { fields, .. } =
            &self.structural_declaration(referent)?.shape
        else {
            return Ok(());
        };
        for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
            let Some(member) = field.field_type.as_structural() else {
                continue;
            };
            prefix.push(CanonicalStructuralPathSegment::Field(field.id));
            if matches!(
                self.structural_declaration(member)?.shape,
                StructuralTypeShape::Reference { .. }
            ) {
                found.push(prefix.clone());
            } else {
                self.referent_reference_paths(member, prefix, found)?;
            }
            prefix.pop();
        }
        Ok(())
    }

    pub(crate) fn track_affine_result(&mut self, result: &terminal_psi::StructuralOperationResult) {
        if result.multiplicity == StructuralMultiplicity::Affine {
            self.live_affine_frontier
                .insert(whole_path_discard(result.place, result.structural_type));
        }
    }

    pub(crate) fn execute_establish_structural_case(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        dispatched_operation!(operation => EstablishStructuralCase { result_case, ref fields });
        let terminal_psi::OperationResult::Structural(result) = &operation.result else {
            return Err(malformed());
        };
        self.establish_structural_case(result, result_case, fields)?;
        Ok(OperationFlow::Advance)
    }

    fn establish_structural_case(
        &mut self,
        result: &terminal_psi::StructuralOperationResult,
        result_case: semantic_vocabulary::StructuralCaseId,
        fields: &[terminal_psi::RecordFieldInitializer],
    ) -> Result<(), TerminalInterpretError> {
        if self.structural_values.contains_key(&result.place) {
            return Err(malformed());
        }
        let (common, cases) = match self
            .program
            .structural_types()
            .get(&result.structural_type)
            .map(|declaration| &declaration.shape)
        {
            Some(StructuralTypeShape::Sum { cases }) => (&[][..], cases),
            Some(StructuralTypeShape::Mixed { fields, cases }) => (fields.as_slice(), cases),
            _ => return Err(malformed()),
        };
        let Some(selected) = find_structural_case(cases, result_case) else {
            return Err(malformed());
        };
        let roster = common
            .iter()
            .filter(|field| !matches!(field.field_type, StructuralFieldType::Erased { .. }))
            .map(|declaration| (declaration, StructuralFieldNamespace::Member))
            .chain(
                selected
                    .fields
                    .iter()
                    .filter(|field| {
                        !(field.relevance.is_erased()
                            && matches!(field.field_type, StructuralFieldType::Erased { .. }))
                    })
                    .map(|declaration| (declaration, StructuralFieldNamespace::CasePayload)),
            )
            .collect::<Vec<_>>();
        if roster.len() != fields.len() {
            return Err(malformed());
        }
        let mut value = TerminalStructuralValue::minted(
            self.local_structural_identities.allocate()?,
            result.structural_type,
            result.qualifications.clone(),
        );
        let member_root = StructuralRuntimePlace::from(&value);
        let mut storage = ConstructorStorage::new(self.account.as_ref());
        let mut consumed = Vec::new();
        let mut frontier = self.live_affine_frontier.clone();
        let mut claim_operands = Vec::new();
        for ((declaration, namespace), binding) in roster.into_iter().zip(fields) {
            if binding.field != declaration.id || declaration.relevance.is_erased() {
                return Err(malformed());
            }
            match &binding.value {
                terminal_psi::RecordFieldValue::Scalar { value: operand, .. } => {
                    let scalar = self.values.get(operand).copied().ok_or_else(malformed)?;
                    if declaration.field_type.scalar_type() != Some(scalar.scalar_type())
                        || !terminal_scalar_belongs_to_type(scalar)
                    {
                        return Err(malformed());
                    }
                    if !scalar.satisfies_field(&declaration.field_type) {
                        return Err(malformed());
                    }
                    storage.stage_scalar_endpoint(
                        &PrimitiveStorageEndpoint::ScalarField(StructuralScalarRuntimeField {
                            parent: member_root.clone(),
                            namespace,
                            field: binding.field,
                        }),
                        scalar,
                    )?;
                }
                terminal_psi::RecordFieldValue::Structural(argument) => {
                    if argument.access != StructuralAccess::Owned {
                        return Err(malformed());
                    }
                    let mut claim_path = Vec::new();
                    if namespace == StructuralFieldNamespace::CasePayload {
                        claim_path.push(StructuralPathSegment::Case(result_case));
                    }
                    claim_path.push(StructuralPathSegment::Field(declaration.identity));
                    let member = StructuralRuntimePlace {
                        opaque_identity: value.opaque_identity,
                        path: claim_path.clone(),
                    };
                    claim_operands.push((argument, claim_path.clone()));
                    let child = ConstructorValue::resolve(self, argument)?;
                    if let Some(bytes) = self.prepare_constructor_byte_field(
                        declaration,
                        argument,
                        &child,
                        &mut frontier,
                        &mut consumed,
                    )? {
                        storage.stage_byte_field(
                            StructuralByteSequenceRuntimeField {
                                parent: member_root.clone(),
                                namespace,
                                field: binding.field,
                            },
                            bytes,
                        )?;
                        continue;
                    }
                    let transfers_custody =
                        self.stage_constructor_operand(argument, &mut frontier, &mut consumed)?;
                    if declaration.field_type
                        != StructuralFieldType::Structural(child.structural_type())
                    {
                        return Err(malformed());
                    }
                    child.append_memberships(&claim_path, &mut value.projected_qualifications);
                    storage.stage(self, argument, &child, &member, transfers_custody)?;
                }
            }
        }
        storage.stage_case(member_root, result.structural_type, result_case)?;
        value.projected_qualifications.sort();
        value.projected_qualifications.dedup();
        storage.validate_projected_memberships(self, &value, &result.projected_qualifications)?;
        let claims = self.stage_constructor_claims(
            crate::storage::record::ResultClaimOrigin::Construction,
            result,
            &claim_operands,
            &mut frontier,
            &|current| storage.case_at_projection(self, &value, &current.path),
        )?;
        let retirement = self.prepare_constructor_operands(frontier, &consumed, claims)?;
        storage.publish(self)?;
        self.structural_values.insert(result.place, value)?;
        retirement.commit(self)?;
        Ok(())
    }

    pub(crate) fn prepare_subtree_retirement(
        &self,
        from: &StructuralRuntimePlace,
    ) -> Result<
        crate::storage::scalar_array::empty_storage::EmptyArrayRetirement,
        TerminalInterpretError,
    > {
        self.empty_array_storage.prepare_retirement(
            &self.structural_types(),
            self.account.as_ref(),
            from,
        )
    }

    pub(crate) fn prune_subtree(
        &mut self,
        retirement: crate::storage::scalar_array::empty_storage::EmptyArrayRetirement,
    ) -> Result<(), TerminalInterpretError> {
        let from = retirement.root();
        self.require_guest_storage_mutation(from)?;
        crate::storage::structural_storage::prune_rows(&mut self.original_storage_roots, from);
        crate::storage::structural_storage::prune_rows(
            &mut self.provider_storage.descriptors,
            from,
        );
        crate::storage::structural_storage::prune_rows(
            &mut self.activation_storage.claim_descriptors,
            from,
        );
        self.invalidate_stored_provenance(from);
        crate::storage::structural_storage::prune_rows(&mut self.structural_host_atoms, from);
        self.structural_scalar_fields.prune(from);
        crate::storage::structural_storage::prune_rows(
            &mut self.structural_byte_sequence_fields,
            from,
        );
        crate::storage::structural_storage::prune_rows(&mut self.structural_byte_descriptors, from);
        crate::storage::structural_storage::prune_rows(&mut self.structural_byte_storage, from);
        crate::storage::structural_storage::prune_rows(&mut self.structural_cases, from);
        crate::storage::structural_storage::prune_rows(&mut self.reference_referents, from);
        crate::storage::structural_storage::prune_rows(
            &mut self.structural_element_view_fields,
            from,
        );
        self.structural_primitive_storage.prune(from);
        self.empty_array_storage.retire(retirement)
    }

    pub(crate) fn execute_establish_byte_sequence_literal(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        dispatched_operation!(operation => EstablishByteSequenceLiteral {
            destination, ref bytes, ..
        });
        if !matches!(operation.result, terminal_psi::OperationResult::Unit) {
            return Err(malformed());
        }
        let machine = self.program.verified_machine(self.current_machine)?;
        let Some(terminal_psi::StructuralPlaceDeclaration {
            kind:
                semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                    structural_type, ..
                },
            ..
        }) = machine.structural_place(destination)
        else {
            return Err(malformed());
        };
        if let Some(previous) = self.structural_values.get(&destination) {
            if previous.opaque_identity != destination.get()
                || previous.structural_type != *structural_type
                || !previous.path.is_empty()
                || !previous.qualifications.is_empty()
                || self.byte_sequence_values.get(&destination).is_none_or(
                    |binding| !matches!(binding.immutable(), Ok(view) if view.bytes() == bytes),
                )
            {
                return Err(malformed());
            }
        } else {
            if self
                .byte_sequence_values
                .insert(
                    destination,
                    ByteSequenceBinding::Immutable(ByteSequenceView::new(
                        ByteSequenceStorage::copy(bytes, self.account.as_ref())?,
                    )),
                )?
                .is_some()
            {
                return Err(malformed());
            }
            self.structural_values.insert(
                destination,
                TerminalStructuralValue::minted(destination.get(), *structural_type, Vec::new()),
            )?;
        }
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn result_place_in_use(&self, place: semantic_vocabulary::PlaceId) -> bool {
        self.structural_values.contains_key(&place)
            || self.linear_homes.contains_key(&place)
            || self.scalar_array_values.contains_key(&place)
            || crate::engine::custody::place_is_claimed(&self.live_claims, place)
    }

    pub(crate) fn execute_establish_zeroed_structural(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        dispatched_operation!(operation => EstablishZeroedStructural {});
        let result = operation.result.structural().ok_or_else(malformed)?;
        if !matches!(result.multiplicity, nonlinear_multiplicity_pair_pattern!())
            || !terminal_semantics::zeroed_projected_byte_qualifications_hold(
                self.structural_types().values(),
                self.program.catalogs.structural_domains.iter(),
                result.structural_type,
                &result.projected_qualifications,
            )
            || !result.claims.is_empty()
            || self.result_place_in_use(result.place)
        {
            return Err(malformed());
        }
        let value = TerminalStructuralValue {
            opaque_identity: self.local_structural_identities.allocate()?,
            structural_type: result.structural_type,
            qualifications: result.qualifications.clone(),
            projected_qualifications: result.projected_qualifications.clone(),
            path: Vec::new(),
        };
        self.materialize_zeroed_structural_storage(result.place, &value)?;
        self.structural_values.insert(result.place, value)?;
        self.track_affine_result(result);
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn materialize_zeroed_structural_storage(
        &mut self,
        source: semantic_vocabulary::PlaceId,
        value: &TerminalStructuralValue,
    ) -> Result<(), TerminalInterpretError> {
        self.materialize_zeroed_structural_storage_with_established_fields(source, value, &[])
    }

    pub(crate) fn materialize_zeroed_structural_storage_with_established_fields(
        &mut self,
        source: semantic_vocabulary::PlaceId,
        value: &TerminalStructuralValue,
        established_fields: &[terminal_semantics::EstablishedErasedStructuralField],
    ) -> Result<(), TerminalInterpretError> {
        let leaves = terminal_semantics::zeroed_structural_leaves_with_established_erased_fields(
            self.structural_types().values(),
            value.structural_type,
            established_fields,
        )
        .ok_or(TerminalInterpretError::UnsupportedSemanticVariant(
            "zeroed structural backing or erased establishment cannot be materialized",
        ))?;
        self.materialize_zeroed_leaves(source, value, &leaves)
    }

    fn materialize_zeroed_leaves(
        &mut self,
        source: semantic_vocabulary::PlaceId,
        value: &TerminalStructuralValue,
        leaves: &[ZeroedStructuralLeaf],
    ) -> Result<(), TerminalInterpretError> {
        let zero = |scalar_type: ScalarType| -> Option<TerminalScalarValue> {
            match scalar_type {
                ScalarType::Callable(_) => None,
                ScalarType::Boolean => Some(TerminalScalarValue::Boolean(false)),
                ScalarType::Integer(integer_type) => Some(TerminalScalarValue::Integer {
                    scalar_type: integer_type,
                    value: integer_type.zero(),
                }),
                ScalarType::IeeeFloat(format) => {
                    Some(TerminalScalarValue::IeeeFloat(match format {
                        IeeeFloatFormat::Binary32 => IeeeFloatValue::Binary32(0),
                        IeeeFloatFormat::Binary64 => IeeeFloatValue::Binary64(0),
                    }))
                }
            }
        };
        for leaf in leaves {
            let storage_path = || {
                let mut path = value.path.clone();
                path.extend_from_slice(crate::boundary::values::structural_storage_path(
                    leaf.path(),
                ));
                path
            };
            match leaf {
                ZeroedStructuralLeaf::RecordField {
                    path: _,
                    field,
                    scalar_type,
                } => {
                    let parent = StructuralRuntimePlace {
                        opaque_identity: value.opaque_identity,
                        path: storage_path(),
                    };
                    let cell = StructuralScalarRuntimeField::member(parent, *field);
                    self.structural_scalar_fields
                        .insert(cell, zero(*scalar_type).ok_or_else(malformed)?)?;
                }
                ZeroedStructuralLeaf::EmptyView {
                    field: Some(field),
                    carrier,
                    ..
                } if carrier.byte_sequence_carrier().is_some() => {
                    self.structural_byte_sequence_fields.insert(
                        StructuralByteSequenceRuntimeField {
                            parent: StructuralRuntimePlace {
                                opaque_identity: value.opaque_identity,
                                path: storage_path(),
                            },
                            namespace: StructuralFieldNamespace::for_carrier(leaf.path()),
                            field: *field,
                        },
                        ByteSequenceBinding::Immutable(ByteSequenceView::new(
                            ByteSequenceStorage::zeroed(0, self.account.as_ref())?,
                        )),
                    )?;
                }
                ZeroedStructuralLeaf::EmptyView {
                    field: None,
                    carrier: StructuralFieldType::Structural(ty),
                    ..
                } => {
                    use crate::storage::element_views::{
                        ElementView, immutable_sequence::ImmutableStructuralSequence,
                    };
                    if !matches!(
                        self.structural_declaration(*ty)?.shape,
                        StructuralTypeShape::ElementView {
                            access: None | Some(StructuralAccess::SharedBorrow),
                            ..
                        }
                    ) {
                        return Err(malformed());
                    }
                    let descriptor = TerminalStructuralValue {
                        opaque_identity: value.opaque_identity,
                        structural_type: *ty,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        path: storage_path(),
                    };
                    let empty = ImmutableStructuralSequence::from_values(
                        0,
                        std::iter::empty(),
                        self.account.as_ref(),
                    )?;
                    self.structural_element_view_fields.insert(
                        StructuralRuntimePlace::from(&descriptor),
                        ElementView::immutable_sequence(empty, descriptor, source),
                    )?;
                }
                ZeroedStructuralLeaf::EmptyView { .. } => return Err(malformed()),
                ZeroedStructuralLeaf::CaseField {
                    path: _,
                    field,
                    scalar_type,
                    ..
                } => {
                    let parent = StructuralRuntimePlace {
                        opaque_identity: value.opaque_identity,
                        path: storage_path(),
                    };
                    self.structural_scalar_fields.insert(
                        StructuralScalarRuntimeField::case_payload(parent, *field),
                        zero(*scalar_type).ok_or_else(malformed)?,
                    )?;
                }
                ZeroedStructuralLeaf::EmptyScalarArray {
                    structural_type, ..
                } => {
                    self.empty_array_storage.establish(
                        &self.program.structural_types(),
                        StructuralRuntimePlace {
                            opaque_identity: value.opaque_identity,
                            path: storage_path(),
                        },
                        *structural_type,
                    )?;
                }
                ZeroedStructuralLeaf::ByteArray { length, .. } => {
                    let length = usize::try_from(*length).map_err(|_| malformed())?;
                    let home = StructuralRuntimePlace {
                        opaque_identity: value.opaque_identity,
                        path: storage_path(),
                    };
                    if !self.structural_byte_storage.contains_key(&home) {
                        self.structural_byte_storage.insert(
                            home,
                            crate::boundary::structural_inputs::byte_arrays::storage::StructuralByteArrayStorage::new(
                                crate::boundary::structural_inputs::byte_arrays::array_path_type(
                                    &self.structural_types(), value.structural_type, leaf.path(),
                                ).ok_or_else(malformed)?,
                                ByteSequenceView::new(ByteSequenceStorage::zeroed(length, self.account.as_ref())?),
                                self.account.as_ref(),
                            ),
                        )?;
                    }
                }
                ZeroedStructuralLeaf::ArrayScalar { scalar_type, .. } => {
                    self.structural_primitive_storage.insert(
                        StructuralRuntimePlace {
                            opaque_identity: value.opaque_identity,
                            path: storage_path(),
                        },
                        zero(*scalar_type).ok_or_else(malformed)?,
                    )?;
                }
                ZeroedStructuralLeaf::SumSelection {
                    path: _,
                    structural_type,
                    case,
                    ..
                } => {
                    self.structural_cases.insert(
                        StructuralRuntimePlace {
                            opaque_identity: value.opaque_identity,
                            path: storage_path(),
                        },
                        StructuralCaseContents {
                            structural_type: *structural_type,
                            case: *case,
                        },
                    )?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn execute_establish_structural_local(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        dispatched_operation!(operation => EstablishStructuralLocal {});
        let result = operation.result.structural().ok_or_else(malformed)?;
        terminal_semantics::validate_vacant_structural_local(
            self.structural_types().values(),
            result,
        )
        .map_err(|reason| TerminalInterpretError::VerifiedOperationContractInvalid { reason })?;
        if self.result_place_in_use(result.place) {
            return Err(malformed());
        }
        let home = TerminalStructuralValue::minted(
            self.local_structural_identities.allocate()?,
            result.structural_type,
            result.qualifications.clone(),
        );
        if result.multiplicity == StructuralMultiplicity::Linear {
            self.linear_homes.insert(result.place, home)?;
        } else {
            self.structural_values.insert(result.place, home)?;
        }
        self.track_affine_result(result);
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn execute_establish_trivial_affine_local(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        dispatched_operation!(operation => EstablishTrivialAffineLocal { destination });
        if !matches!(operation.result, terminal_psi::OperationResult::Unit)
            || self.structural_values.contains_key(&destination)
        {
            return Err(malformed());
        }
        let machine = self.program.verified_machine(self.current_machine)?;
        let Some(terminal_psi::StructuralPlaceDeclaration {
            kind:
                semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                    structural_type, ..
                },
            ..
        }) = machine.structural_place(destination)
        else {
            return Err(malformed());
        };
        self.structural_values.insert(
            destination,
            TerminalStructuralValue::minted(destination.get(), *structural_type, Vec::new()),
        )?;
        self.live_affine_frontier
            .insert(whole_path_discard(destination, *structural_type));
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn execute_establish_record(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        dispatched_operation!(operation => EstablishRecord { ref fields });
        self.establish_record(operation.result.structural().ok_or(malformed())?, fields)?;
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn execute_structural_scalar_field_store(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        dispatched_operation!(operation => StructuralScalarFieldStore {
            destination, ref path, field, value, ..
        });
        if !matches!(operation.result, terminal_psi::OperationResult::Unit) {
            return Err(malformed());
        }
        let machine = self.program.verified_machine(self.current_machine)?;
        let module = &self.program.catalogs;
        let preserves = |root, whole: &[_], projected: &[_]| {
            terminal_semantics::scalar_field_store_preserves_domain_content(
                self.structural_types().values(),
                module.structural_domains.iter(),
                &module.identities,
                root,
                whole,
                projected,
                path,
                field,
            ) && terminal_semantics::scalar_field_store_preserves_content(
                self.structural_types().values(),
                module.structural_domains.iter(),
                &module.identities,
                root,
                destination,
                machine.content_entry_claims.iter(),
                path,
                field,
            )
        };
        let access = machine
            .structural_parameter(destination)
            .filter(|parameter| {
                matches!(
                    parameter.access,
                    StructuralAccess::Owned
                        | StructuralAccess::MutableBorrow
                        | StructuralAccess::WriteOnlyBorrow
                ) && preserves(
                    parameter.structural_type,
                    &parameter.qualifications,
                    &parameter.projected_qualifications,
                )
            })
            .map(|parameter| parameter.access)
            .or_else(|| {
                let declaration = machine.structural_place(destination)?;
                let semantic_vocabulary::StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                } = declaration.kind
                else {
                    return None;
                };
                let result = machine
                    .operations()
                    .find(|operation| operation.id == producer)?
                    .result
                    .structural()?;
                (result.place == destination
                    && result.structural_type == structural_type
                    && self
                        .structural_values
                        .get(&destination)
                        .is_some_and(|value| {
                            value.structural_type == structural_type
                                && preserves(
                                    value.structural_type,
                                    &value.qualifications,
                                    &value.projected_qualifications,
                                )
                        })
                    && preserves(
                        result.structural_type,
                        &result.qualifications,
                        &result.projected_qualifications,
                    ))
                .then_some(StructuralAccess::Owned)
            })
            .ok_or(malformed())?;
        let source = self.scalar_value(value)?;
        let projection_access = match access {
            StructuralAccess::Owned => StructuralAccess::WriteOnlyBorrow,
            access => access,
        };
        let (carrier_path, namespace) = match path.split_last() {
            Some((StructuralPathSegment::Case(case), prefix)) => {
                (prefix, vec![CanonicalStructuralPathSegment::Case(*case)])
            }
            _ => (path.as_slice(), Vec::new()),
        };
        let projection =
            self.resolve_live_storage_projection(destination, carrier_path, projection_access)?;
        let (parent, view) = (projection.value, projection.view_tip);
        let declared_parent = match view {
            None => parent.structural_type,
            Some(AggregateViewTip::Aggregate { view, backing })
                if backing == parent.structural_type =>
            {
                view.structural_type
            }
            Some(_) => return Err(malformed()),
        };
        let types = self.structural_types();
        let member = observed_field_declaration(types.values(), declared_parent, &namespace, field)
            .ok_or_else(malformed)?;
        if member.relevance.is_erased()
            || member.field_type.scalar_type() != Some(source.scalar_type())
            || !terminal_scalar_belongs_to_type(source)
        {
            return Err(malformed());
        }
        if let Some(view) = view {
            let carrier = record_field_carrier(types.values(), declared_parent, &namespace)
                .ok_or_else(malformed)?;
            let mut logical = carrier.path;
            logical.push(StructuralPathSegment::Field(member.identity));
            let (storage, scalar_type, _) = self.aggregate_view_primitive_access(
                &parent,
                view,
                &logical,
                projection.collection_home.as_ref().or_else(|| {
                    self.aggregate_argument_backings
                        .get(&destination)
                        .and_then(|backing| backing.collection_home.as_ref())
                }),
                true,
            )?;
            self.store_primitive_storage(destination, storage, scalar_type, source)?;
            return Ok(OperationFlow::Advance);
        }
        let cell = self.observed_field_cell_in(&parent, None, &namespace, field)?;
        let mut leaf = parent.clone();
        leaf.path = cell.parent.path.clone();
        leaf.path
            .push(StructuralPathSegment::Field(member.identity));
        if let Some((storage, scalar_type)) = self.reference_byte_region_access(&parent, &leaf)? {
            self.store_primitive_storage(destination, storage, scalar_type, source)?;
            return Ok(OperationFlow::Advance);
        }
        self.require_guest_storage_mutation(&cell.parent)?;
        self.invalidate_stored_provenance(&cell.parent);
        self.structural_scalar_fields.insert(cell, source)?;
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn execute_move_structural_value(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let (source, parents, derivations) = match &operation.kind {
            OperationKind::MoveStructuralValue { source } => (source, &[][..], &[][..]),
            OperationKind::TransformContent {
                source,
                parents,
                derivations,
            } => (source, parents.as_slice(), derivations.as_slice()),
            _ => return Err(malformed()),
        };
        let retired = self.content_parent_claims(parents, derivations)?;
        let result = operation.result.structural().ok_or_else(malformed)?;
        let machine = self.program.verified_machine(self.current_machine)?;
        let whole_multiplicity = machine
            .structural_parameter(source.place)
            .map(|parameter| parameter.multiplicity)
            .or_else(|| {
                machine
                    .producing_operation(source.place)
                    .and_then(|(producer, _)| producer.result.structural())
                    .map(|result| result.multiplicity)
            });
        if source.access != StructuralAccess::Owned
            || self.result_place_in_use(result.place)
            || result.multiplicity == StructuralMultiplicity::Unrestricted
            || whole_multiplicity == Some(StructuralMultiplicity::Unrestricted)
            || (!source.path.is_empty()
                && result.multiplicity != StructuralMultiplicity::Linear
                && result.claims.iter().any(|claim| claim.path.is_empty()))
            || !result.qualification_establishments.is_empty()
            || machine
                .structural_parameter(source.place)
                .is_some_and(|parameter| parameter.access != StructuralAccess::Owned)
            || (source.path.is_empty() && whole_multiplicity != Some(result.multiplicity))
        {
            return Err(malformed());
        }
        let mut identity_cursor = self.local_structural_identities.cursor();
        let crate::storage::record::owned_value::StagedOwnedValue {
            mut value, storage, ..
        } = self.stage_owned_value(
            source,
            result.structural_type,
            result.multiplicity,
            &mut identity_cursor,
        )?;
        let whole = &value.qualifications;
        let projected = &value.projected_qualifications;
        if matches!(operation.kind, OperationKind::TransformContent { .. }) {
            terminal_semantics::content_transformations::validate_memberships(
                &self.program.catalogs.structural_types,
                &self.program.catalogs.structural_domains,
                &self.program.catalogs.identities,
                result,
                derivations,
                whole,
                projected,
            )
            .map_err(TerminalInterpretError::ContentTransformationMismatch)?;
        }
        if whole
            .iter()
            .any(|domain| !result.qualifications.contains(domain))
            || projected
                .iter()
                .any(|row| !result.projected_qualifications.contains(row))
        {
            return Err(malformed());
        }
        value.qualifications = result.qualifications.clone();
        value.projected_qualifications = result.projected_qualifications.clone();
        let mut frontier = self.live_affine_frontier.clone();
        let mut consumed = Vec::new();
        let transfers_custody =
            self.stage_constructor_operand(source, &mut frontier, &mut consumed)?;
        if !transfers_custody {
            return Err(TerminalInterpretError::AffineFrontierMismatch);
        }
        let mut claims = self.stage_constructor_claims(
            crate::storage::record::ResultClaimOrigin::Transfer,
            result,
            &[(source, Vec::new())],
            &mut frontier,
            &|current| storage.case_at_projection(self, &value, &current.path),
        )?;
        for parent in parents {
            if !self.stage_constructor_operand(parent, &mut frontier, &mut consumed)? {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
        }
        for identity in retired {
            if claims.remove(&identity).is_none() {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
        }
        self.commit_constructor_operands(frontier, &consumed, claims)?;
        storage.publish(self)?;
        self.local_structural_identities
            .commit_cursor(identity_cursor);
        self.structural_values.insert(result.place, value)?;
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn execute_move_structural_field(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        dispatched_operation!(operation => MoveStructuralField { source, ref path, field });
        let result = operation.result.structural().ok_or_else(malformed)?;
        if self.structural_values.contains_key(&result.place) {
            return Err(malformed());
        }
        let storage = self.structural_value(source)?;
        let window = self.structural_window_hole(
            operation,
            source,
            storage,
            path,
            field,
            StructuralAccess::MutableBorrow,
        )?;
        if window.aggregate_view {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "moving an aggregate storage interpretation",
            ));
        }
        let selected = window.backing;
        if selected.structural_type != result.structural_type {
            return Err(malformed());
        }
        let required_projected =
            crate::boundary::structural_inputs::case_membership::active_projected_qualifications(
                &self.structural_types(),
                &selected,
                self.structural_cases.as_map(),
                &result.projected_qualifications,
            )?;
        let available_projected =
            crate::boundary::structural_inputs::case_membership::active_projected_qualifications(
                &self.structural_types(),
                &selected,
                self.structural_cases.as_map(),
                &selected.projected_qualifications,
            )?;
        if result
            .qualifications
            .iter()
            .any(|domain| !selected.qualifications.contains(domain))
            || required_projected
                .iter()
                .any(|row| !available_projected.contains(row))
        {
            return Err(malformed());
        }
        let hole = StructuralRuntimePlace::from(&selected);
        let mut frontier = self.live_affine_frontier.clone();
        let claims = self.stage_window_move_custody(source, path, &hole, result, &mut frontier)?;
        let moved = TerminalStructuralValue {
            opaque_identity: self.local_structural_identities.allocate()?,
            structural_type: result.structural_type,
            qualifications: selected.qualifications,
            projected_qualifications: selected.projected_qualifications,
            path: Vec::new(),
        };
        self.transplant_subtree(&hole, &StructuralRuntimePlace::from(&moved), true)?;
        self.structural_values.insert(result.place, moved)?;
        self.live_affine_frontier = frontier;
        self.live_claims = claims;
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn execute_store_structural_field(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        dispatched_operation!(operation => StoreStructuralField {
            disposition, destination, ref path, field, ref value
        });
        if !matches!(operation.result, terminal_psi::OperationResult::Unit)
            || value.access != StructuralAccess::Owned
            || !value.path.is_empty()
        {
            return Err(malformed());
        }
        let machine = self.program.machine(self.current_machine)?;
        if !terminal_semantics::structural_store_value_admitted(
            machine,
            self.structural_types().values(),
            disposition,
            value.place,
        ) {
            return Err(malformed());
        }
        let copies_storage = machine
            .structural_parameter(value.place)
            .map(|parameter| parameter.multiplicity)
            .or_else(|| {
                machine
                    .producing_operation(value.place)
                    .and_then(|(operation, _)| {
                        operation
                            .result
                            .structural()
                            .map(|result| result.multiplicity)
                    })
            })
            .ok_or_else(malformed)?
            == StructuralMultiplicity::Unrestricted;
        let failure = |phase, cause| TerminalInterpretError::StructuralReplacement {
            operation: operation.id,
            phase,
            cause: Box::new(cause),
        };
        let mut identity_cursor = self.local_structural_identities.cursor();
        let reserved = self.linear_homes.get(&destination).cloned();
        let (storage, window) = if let Some(home) = &reserved {
            let (producer, declared_type) = machine
                .producing_operation(destination)
                .ok_or_else(malformed)?;
            let result = producer.result.structural().ok_or_else(malformed)?;
            if !matches!(producer.kind, OperationKind::EstablishStructuralLocal {})
                || result.place != destination
                || result.structural_type != declared_type
                || home.structural_type != declared_type
                || result.multiplicity != StructuralMultiplicity::Linear
                || !result.claims.is_empty()
            {
                return Err(malformed());
            }
            let projection = terminal_semantics::structural_window_projection(
                self.structural_types().values(),
                declared_type,
                path,
                field,
            )
            .ok_or_else(malformed)?;
            if !terminal_semantics::projection_stays_in_inline_storage(
                self.structural_types().values(),
                declared_type,
                &projection.path,
            ) {
                return Err(malformed());
            }
            let selected = crate::engine::custody::project_structural_value(
                &self.structural_types(),
                home,
                &projection.path,
            )?;
            (
                home.clone(),
                ResolvedStructuralWindow {
                    backing: selected,
                    structural_type: projection.structural_type,
                    path: projection.path,
                    aggregate_view: false,
                },
            )
        } else {
            let (storage, _) = self
                .array_storage_root_staged(
                    destination,
                    &mut identity_cursor,
                    &mut RuntimeRows::new(self.account.as_ref()),
                )
                .map_err(|error| failure("destination home", error))?;
            let window = self
                .structural_window_hole(
                    operation,
                    destination,
                    &storage,
                    path,
                    field,
                    disposition.required_access(),
                )
                .map_err(|error| failure("destination storage", error))?;
            (storage, window)
        };
        let field_type = window.structural_type;
        let hole = StructuralRuntimePlace::from(&window.backing);
        if window.aggregate_view
            && (disposition != terminal_psi::StructuralStoreDisposition::ReplaceDiscardable
                || !copies_storage
                || scalar_array_leaf_shape(self.structural_types().values(), field_type).is_none())
        {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "non-copy replacement of an aggregate storage interpretation",
            ));
        }
        let (required, projected_required) = if let Some(home) = &reserved {
            StructuralPathQualification::select_restorable(
                &home.qualifications,
                &home.projected_qualifications,
                &window.path,
            )
            .ok_or_else(malformed)?
        } else {
            self.structural_window_requirements(operation, destination, path, field)
                .map_err(|error| failure("receiving memberships", error))?
        };
        let mut frontier = self.live_affine_frontier.clone();
        frontier.retain(|entry| entry.place != value.place);
        self.restore_store_frontier(&mut frontier, destination, path, &hole, field_type)
            .map_err(|error| failure("restoration frontier", error))?;
        let claims = self
            .stage_window_store_claims(destination, &storage, path, &hole, value, &mut frontier)
            .map_err(|error| failure("restored claims", error))?;
        let initializes_home = disposition == terminal_psi::StructuralStoreDisposition::Vacant
            && path.is_empty()
            && field.is_none()
            && machine
                .producing_operation(destination)
                .is_some_and(|(producer, ty)| {
                    matches!(producer.kind, OperationKind::EstablishStructuralLocal {})
                        && producer.result.structural().is_some_and(|result| {
                            result.place == destination && result.structural_type == ty
                        })
                });
        let publishes_value = reserved.is_none() || initializes_home;
        let child = ConstructorValue::resolve(self, value)
            .map_err(|error| failure("completed source", error))?;
        let (qualifications, projected_qualifications) = child.memberships();
        if child.structural_type() != field_type
            || required
                .iter()
                .any(|domain| !qualifications.contains(domain))
            || projected_required
                .iter()
                .any(|row| !projected_qualifications.contains(row))
        {
            return Err(TerminalInterpretError::StructuralStorePreparation {
                stage: "replacement memberships",
                detail: format!(
                    "expected_type={field_type:?}, actual_type={:?}, required={required:?}, projected_required={projected_required:?}, actual={qualifications:?}, projected_actual={projected_qualifications:?}",
                    child.structural_type()
                ),
            });
        }
        let mut memberships = Vec::new();
        for (frame_index, place, owner) in self
            .call_stack
            .iter()
            .enumerate()
            .flat_map(|(frame_index, frame)| {
                frame
                    .structural_values
                    .iter()
                    .map(move |(place, owner)| (frame_index, place, owner))
            })
            .chain(
                self.structural_values
                    .iter()
                    .map(|(place, owner)| (self.call_stack.len(), place, owner)),
            )
            .chain(
                self.linear_homes
                    .iter()
                    .map(|(place, owner)| (self.call_stack.len(), place, owner)),
            )
        {
            if window.aggregate_view {
                continue;
            }
            if owner.opaque_identity != hole.opaque_identity {
                continue;
            }
            let Some(relative) = hole.path.strip_prefix(owner.path.as_slice()) else {
                continue;
            };
            if StructuralPathQualification::select_restorable(
                &owner.qualifications,
                &owner.projected_qualifications,
                relative,
            )
            .is_none()
            {
                return Err(TerminalInterpretError::StructuralStorePreparation {
                    stage: "preserve enclosing memberships",
                    detail: format!("owner={owner:?}, hole={hole:?}, relative={relative:?}"),
                });
            }
            let mut repaired = owner.clone();
            if relative.is_empty() {
                repaired.qualifications = qualifications.to_vec();
                repaired.projected_qualifications = projected_qualifications.to_vec();
            } else {
                repaired
                    .projected_qualifications
                    .retain(|row| !row.path.starts_with(relative));
                StructuralPathQualification::append_under(
                    qualifications,
                    projected_qualifications,
                    relative,
                    &mut repaired.projected_qualifications,
                );
                repaired.projected_qualifications.sort();
                repaired.projected_qualifications.dedup();
            }
            memberships.push((frame_index, *place, repaired));
        }
        let destination_path = window.path;
        let writes = if initializes_home || reserved.is_some() {
            None
        } else {
            child
                .replacement_writes(self, value, destination, &destination_path)
                .map_err(|error| failure("scalar storage preparation", error))?
        };
        if let Some(writes) = writes {
            writes.commit(self)?;
        } else if window.aggregate_view {
            if !scalar_array_leaf_shape(self.structural_types().values(), field_type)
                .is_some_and(|(_, count)| count == 0)
            {
                return Err(malformed());
            }
        } else {
            let mut storage = ConstructorStorage::new(self.account.as_ref());
            storage.stage(self, value, &child, &hole, !copies_storage)?;
            let source = match child {
                ConstructorValue::Access(child) => Some(StructuralRuntimePlace::from(&child)),
                ConstructorValue::PackedScalars { .. } => None,
            };
            storage.preserve_replaced_residents(self, &hole)?;
            let hole_retirement = self.prepare_subtree_retirement(&hole)?;
            let source_retirement = source
                .as_ref()
                .filter(|_| !copies_storage)
                .map(|source| self.prepare_subtree_retirement(source))
                .transpose()?;
            self.prune_subtree(hole_retirement)?;
            if let Some(retirement) = source_retirement {
                self.prune_subtree(retirement)?;
            }
            storage.publish(self)?;
        }
        for (frame_index, place, repaired) in memberships {
            if frame_index == self.call_stack.len() {
                if place == destination && reserved.is_some() && !publishes_value {
                    self.linear_homes.insert(place, repaired)?;
                } else {
                    self.structural_values.insert(place, repaired)?;
                }
            } else {
                self.call_stack
                    .get_mut(frame_index)
                    .ok_or_else(malformed)?
                    .structural_values
                    .insert(place, repaired)?;
            }
        }
        if reserved.is_some() && publishes_value {
            self.linear_homes
                .remove(&destination)
                .ok_or_else(malformed)?;
        }
        if !copies_storage {
            if let Some(retired) = self.structural_values.remove(&value.place) {
                let origin = self
                    .reference_referents
                    .get(&StructuralRuntimePlace::from(&retired))
                    .cloned();
                self.retired_structural_homes
                    .push((value.place, retired, origin));
            }
            self.scalar_array_values.remove(&value.place);
            self.element_view_values.remove(&value.place);
            self.byte_sequence_values.remove(&value.place);
        }
        self.live_affine_frontier = frontier;
        self.live_claims = claims;
        self.local_structural_identities
            .commit_cursor(identity_cursor);
        Ok(OperationFlow::Advance)
    }

    fn structural_window_requirements(
        &self,
        operation: &terminal_psi::Operation,
        root: semantic_vocabulary::PlaceId,
        path: &[StructuralPathSegment],
        field: Option<semantic_vocabulary::StructuralFieldId>,
    ) -> Result<(Vec<StructuralDomainId>, Vec<StructuralPathQualification>), TerminalInterpretError>
    {
        let machine = self.program.verified_machine(self.current_machine)?;
        let declared = machine.operation(operation.id).ok_or_else(malformed)?;
        operation_kind!(*declared => StoreStructuralField {
            destination, path: declared_path, field: declared_field, ..
        });
        if *destination != root || *declared_field != field || declared_path.len() != path.len() {
            return Err(malformed());
        }
        let (root_type, whole, projected) =
            if let Some(parameter) = machine.structural_parameter(root) {
                (
                    parameter.structural_type,
                    parameter.qualifications.as_slice(),
                    parameter.projected_qualifications.as_slice(),
                )
            } else {
                let result = machine
                    .operations()
                    .filter_map(|operation| operation.result.structural())
                    .find(|result| result.place == root)
                    .ok_or_else(malformed)?;
                (
                    result.structural_type,
                    result.qualifications.as_slice(),
                    result.projected_qualifications.as_slice(),
                )
            };
        let mut projection = terminal_semantics::structural_window_projection(
            self.structural_types().values(),
            root_type,
            declared_path,
            field,
        )
        .ok_or_else(malformed)?;
        projection
            .path
            .get_mut(..path.len())
            .ok_or_else(malformed)?
            .clone_from_slice(path);
        StructuralPathQualification::select_restorable(whole, projected, &projection.path)
            .ok_or_else(malformed)
    }

    fn structural_window_hole(
        &self,
        operation: &terminal_psi::Operation,
        root: semantic_vocabulary::PlaceId,
        storage: &TerminalStructuralValue,
        path: &[StructuralPathSegment],
        field: Option<semantic_vocabulary::StructuralFieldId>,
        required_access: StructuralAccess,
    ) -> Result<ResolvedStructuralWindow, TerminalInterpretError> {
        let machine = self.program.verified_machine(self.current_machine)?;
        let declared = machine.operation(operation.id).ok_or_else(malformed)?;
        let (declared_root, declared_path, declared_field) = match &declared.kind {
            OperationKind::MoveStructuralField {
                source,
                path,
                field,
            } => (*source, path, *field),
            OperationKind::StoreStructuralField {
                destination,
                path,
                field,
                ..
            } => (*destination, path, *field),
            _ => return Err(malformed()),
        };
        if declared_root != root || declared_field != field || declared_path.len() != path.len() {
            return Err(malformed());
        }
        let projection = terminal_semantics::structural_window_projection(
            self.structural_types().values(),
            self.live_structural_type(root)?,
            declared_path,
            field,
        )
        .ok_or_else(malformed)?;
        let mut complete_path = projection.path;
        complete_path
            .get_mut(..path.len())
            .ok_or_else(malformed)?
            .clone_from_slice(path);
        let path = complete_path.as_slice();
        let prefix_length = path
            .iter()
            .rposition(|segment| matches!(segment, StructuralPathSegment::Referent))
            .map_or(0, |position| position + 1);
        if prefix_length != 0 && !terminal_semantics::structural_window_selects_child(path, field) {
            let base = self
                .resolve_reference_call_arguments(&[StructuralArgument {
                    place: root,
                    path: path[..prefix_length].to_vec(),
                    access: required_access,
                }])?
                .pop()
                .ok_or_else(malformed)?;
            let machine = self.program.machine(self.current_machine)?;
            let borrowed_origin = machine
                .structural_parameters
                .iter()
                .chain(machine.block_structural_parameters())
                .any(|parameter| {
                    required_access.admits_referent(parameter.access)
                        && parameter.multiplicity != StructuralMultiplicity::Linear
                        && self
                            .structural_values
                            .get(&parameter.place)
                            .is_some_and(|storage| {
                                storage.opaque_identity == base.opaque_identity
                                    && base.path.starts_with(&storage.path)
                            })
                });
            let owned_origin = self.structural_values.iter().any(|(place, storage)| {
                if storage.opaque_identity != base.opaque_identity
                    || !base.path.starts_with(&storage.path)
                {
                    return false;
                }
                if let Some(parameter) = machine.structural_parameter(*place) {
                    return parameter.access == StructuralAccess::Owned
                        && parameter.multiplicity != StructuralMultiplicity::Linear;
                }
                machine.producing_operation(*place).is_some_and(|(operation, _)| {
                    matches!(&operation.result, terminal_psi::OperationResult::Structural(result)
                        if result.place == *place
                            && result.multiplicity != StructuralMultiplicity::Linear
                            && result.claims.is_empty())
                })
            });
            let view_origin = self.element_view_values.iter().any(|(place, view)| {
                self.structural_values
                    .get(place)
                    .is_some_and(|storage| view.matches_descriptor_home(storage))
                    && view.lends_structural_storage(&base)
            });
            if !borrowed_origin && !owned_origin && !view_origin {
                return Err(malformed());
            }
        }
        let selected = if let Some(view) = self.element_view_values.get(&root) {
            if !view.matches_descriptor_home(storage)
                || !required_access.admits_referent(view.access())
            {
                return Err(malformed());
            }
            let (collection, selected) =
                view.structural_projection(&self.structural_types(), path, true)?;
            self.resolve_reference_storage_projection(&collection, &selected, required_access)?
                .ok_or_else(malformed)?
        } else {
            self.resolve_storage_projection_with_leaf_type(
                root,
                storage,
                path,
                required_access,
                None,
                terminal_psi::StructuralFieldType::canonical_leaf_type,
            )?
        };
        let selected_type = match selected.view_tip {
            Some(AggregateViewTip::Aggregate { view, .. }) => view.structural_type,
            _ => selected.value.structural_type,
        };
        if selected_type != projection.structural_type {
            return Err(malformed());
        }
        let aggregate_view = selected_type != selected.value.structural_type
            && matches!(selected.view_tip, Some(AggregateViewTip::Aggregate { .. }));
        Ok(ResolvedStructuralWindow {
            backing: selected.value,
            structural_type: selected_type,
            path: complete_path,
            aggregate_view,
        })
    }

    fn transplant_subtree(
        &mut self,
        from: &StructuralRuntimePlace,
        to: &StructuralRuntimePlace,
        vacate: bool,
    ) -> Result<(), TerminalInterpretError> {
        let mut storage =
            crate::storage::record::storage::ConstructorStorage::new(self.account.as_ref());
        storage.stage_subtree(self, from, to, vacate)?;
        storage.preserve_replaced_residents(self, to)?;
        let destination_retirement = self.prepare_subtree_retirement(to)?;
        let source_retirement = vacate
            .then(|| self.prepare_subtree_retirement(from))
            .transpose()?;
        self.prune_subtree(destination_retirement)?;
        if let Some(retirement) = source_retirement {
            self.prune_subtree(retirement)?;
        }
        storage.publish(self)
    }

    pub(crate) fn execute_structural_leaf_copy(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let result = operation.result.structural().ok_or_else(malformed)?;
        if let OperationKind::StructuralLeafCopy { source, path } = &operation.kind
            && path.is_empty()
            && let Some(terminal_psi::StructuralTypeDeclaration {
                shape:
                    StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity }),
                ..
            }) = self.structural_types().get(&result.structural_type)
        {
            let source_value = self.structural_value(*source)?;
            let bytes = self.byte_sequence_values.require(source)?.immutable()?;
            if result.multiplicity != StructuralMultiplicity::Unrestricted
                || result.structural_type != source_value.structural_type
                || result.qualifications != source_value.qualifications
                || !source_value.path.is_empty()
                || !bytes.matches_descriptor_home(source_value)
                || !result.projected_qualifications.is_empty()
                || !result.qualification_establishments.is_empty()
                || !result.claims.is_empty()
                || self.structural_values.contains_key(&result.place)
                || self.byte_sequence_values.contains_key(&result.place)
                || u64::try_from(bytes.len()).map_or(true, |length| length > *capacity)
                || !terminal_semantics::byte_literal_qualifications_hold_in(
                    self.program.catalogs.structural_domains.iter(),
                    result.structural_type,
                    &result.qualifications,
                    bytes.bytes(),
                )
            {
                return Err(malformed());
            }
            let bytes = bytes.clone();
            let copied = TerminalStructuralValue::minted(
                self.local_structural_identities.allocate()?,
                result.structural_type,
                result.qualifications.clone(),
            );
            self.byte_sequence_values.insert(
                result.place,
                ByteSequenceBinding::Immutable(bytes.owned_snapshot()),
            )?;
            self.structural_values.insert(result.place, copied)?;
            return Ok(OperationFlow::Advance);
        }
        let shared_descriptor =
            is_copyable_carrier(self.structural_types().values(), result.structural_type);
        if matches!(operation.kind, OperationKind::StructuralCaseLeafCopy { .. })
            && result.multiplicity != StructuralMultiplicity::Unrestricted
            && !shared_descriptor
        {
            return Err(TerminalInterpretError::AffineProjectionNotRepresentable {
                context: "an affine case payload requires its owned frontier".into(),
            });
        }
        if (result.multiplicity != StructuralMultiplicity::Unrestricted
            && !(result.multiplicity == StructuralMultiplicity::Affine && shared_descriptor))
            || !result.qualification_establishments.is_empty()
            || !result.claims.is_empty()
            || self.structural_values.contains_key(&result.place)
            || self.live_affine_discard_at(result.place).is_some()
        {
            return Err(malformed());
        }
        let scalar_array_result =
            scalar_array_leaf_shape(self.structural_types().values(), result.structural_type)
                .is_some();
        let primitive_result = self
            .structural_types()
            .get(&result.structural_type)
            .is_some_and(|declaration| {
                matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(_))
            });
        if let OperationKind::StructuralLeafCopy { source, path } = &operation.kind
            && (scalar_array_result || primitive_result)
        {
            let argument = StructuralArgument {
                place: *source,
                path: path.clone(),
                access: StructuralAccess::SharedBorrow,
            };
            let value = if self.scalar_array_values.contains_key(source) {
                ConstructorValue::resolve(self, &argument)?
            } else {
                let declaration = self.structural_declaration(result.structural_type)?;
                let selected = self.resolve_live_projection_with_leaf_type(
                    *source,
                    path,
                    StructuralAccess::SharedBorrow,
                    Some(result.structural_type),
                    |field| field.leaf_copy_type(Some(declaration)),
                )?;
                if scalar_array_result
                    && result.is_unrestricted_plain()
                    && matches!(selected.view_tip,
                        Some(AggregateViewTip::Aggregate { view, .. })
                            if view.structural_type == result.structural_type)
                {
                    let copied = self.scalar_array_snapshot(&argument, result.structural_type)?;
                    self.scalar_array_values.publish(result.place, copied)?;
                    return Ok(OperationFlow::Advance);
                }
                ConstructorValue::Access(selected.value)
            };
            if value.structural_type() == result.structural_type {
                if self.scalar_array_values.contains_key(&result.place) {
                    return Err(malformed());
                }
                if scalar_array_result && result.is_unrestricted_plain() {
                    let mut copied =
                        self.scalar_array_snapshot(&argument, result.structural_type)?;
                    copied.qualifications.clone_from(&result.qualifications);
                    self.scalar_array_values.publish(result.place, copied)?;
                    return Ok(OperationFlow::Advance);
                }
                let mut elements = Vec::new();
                value
                    .visit_scalar_leaves(self, &argument, |_, _, scalar| {
                        elements.push(crate::storage::scalar_storage::ScalarCell::Local(scalar));
                        Ok(())
                    })?
                    .ok_or_else(malformed)?;
                let mut identity_cursor = self.local_structural_identities.cursor();
                let copied = TerminalStructuralValue {
                    opaque_identity: self
                        .local_structural_identities
                        .allocate_staged(&mut identity_cursor)?,
                    structural_type: result.structural_type,
                    qualifications: result.qualifications.clone(),
                    projected_qualifications: result.projected_qualifications.clone(),
                    path: Vec::new(),
                };
                let mut storage = ConstructorStorage::new(self.account.as_ref());
                storage.stage(
                    self,
                    &argument,
                    &ConstructorValue::PackedScalars {
                        structural_type: value.structural_type(),
                        elements: &elements,
                        qualifications: value.memberships().0.to_vec(),
                        projected_qualifications: value.memberships().1.to_vec(),
                    },
                    &StructuralRuntimePlace::from(&copied),
                    false,
                )?;
                storage.publish(self)?;
                self.structural_values.insert(result.place, copied)?;
                self.local_structural_identities
                    .commit_cursor(identity_cursor);
                return Ok(OperationFlow::Advance);
            }
        }
        let (leaf, source, _, copy_path) = match &operation.kind {
            OperationKind::StructuralLeafCopy { source, path } => {
                let (leaf, field) =
                    self.resolve_live_leaf_copy(*source, path, result.structural_type)?;
                (leaf, *source, field, path.clone())
            }
            OperationKind::StructuralCaseLeafCopy { source, path } => {
                let (leaf, projection) = self.case_payload_projection(*source, path)?;
                let field = match path.last() {
                    Some(CanonicalStructuralPathSegment::Field(field)) => Some(*field),
                    _ => None,
                };
                (leaf, *source, field, projection)
            }
            _ => return Err(malformed()),
        };
        let quotient_adaptation = terminal_semantics::quotients::quotient_copy_adaptation(
            &self.program.catalogs,
            self.program.verified_machine(self.current_machine)?,
            operation,
        )
        .is_some();
        let occurrence_access = copy_path
            .is_empty()
            .then(|| {
                self.element_view_values
                    .get(&source)
                    .map(|descriptor| descriptor.access())
                    .or_else(|| {
                        self.byte_sequence_values
                            .get(&source)
                            .map(|descriptor| descriptor.access())
                    })
            })
            .flatten();
        if leaf.structural_type != result.structural_type
            && !quotient_adaptation
            && !terminal_semantics::shared_descriptor_copy_matches(
                self.structural_types().values(),
                leaf.structural_type,
                occurrence_access,
                result.structural_type,
                result.multiplicity,
            )
        {
            return self.byte_carrier_leaf_copy(&leaf, result);
        }
        if result.is_unrestricted_plain()
            && scalar_array_leaf_shape(self.structural_types().values(), result.structural_type)
                .is_some()
        {
            return self.copy_scalar_array_payload(
                &StructuralArgument {
                    place: source,
                    path: copy_path,
                    access: StructuralAccess::Owned,
                },
                result,
            );
        }
        let source_place = StructuralRuntimePlace::from(&leaf);
        if self
            .structural_types()
            .get(&result.structural_type)
            .is_some_and(|declaration| {
                matches!(declaration.shape, StructuralTypeShape::Reference { .. })
            })
            && !self.reference_referents.contains_key(&source_place)
        {
            return Err(malformed());
        }
        for carrier in self.reference_referents.keys().filter(|carrier| {
            carrier.opaque_identity == source_place.opaque_identity
                && carrier.path.starts_with(&source_place.path)
        }) {
            let reference_type = self.actual_storage_type(&leaf, carrier)?;
            if !self
                .structural_types()
                .get(&reference_type)
                .is_some_and(|declaration| {
                    matches!(
                        declaration.shape,
                        StructuralTypeShape::Reference {
                            access: StructuralAccess::SharedBorrow,
                            ..
                        }
                    )
                })
            {
                return Err(malformed());
            }
        }
        let copied = TerminalStructuralValue {
            opaque_identity: self.local_structural_identities.allocate()?,
            structural_type: result.structural_type,
            qualifications: result.qualifications.clone(),
            projected_qualifications: result.projected_qualifications.clone(),
            path: Vec::new(),
        };
        if self
            .structural_types()
            .get(&result.structural_type)
            .is_some_and(|declaration| {
                matches!(declaration.shape, StructuralTypeShape::ByteSequence(_))
            })
        {
            let argument = StructuralArgument {
                place: source,
                path: copy_path.clone(),
                access: StructuralAccess::SharedBorrow,
            };
            self.validate_descriptor_borrow(&argument, StructuralAccess::SharedBorrow)?;
            let binding = self.byte_descriptor_binding(&argument, &leaf)?;
            self.byte_descriptor_length(&binding, &leaf)?;
            let bytes = self.reborrow_byte_sequence_binding(
                &binding,
                &leaf,
                StructuralAccess::SharedBorrow,
            )?;
            if self.byte_sequence_values.contains_key(&result.place) {
                return Err(malformed());
            }
            self.byte_sequence_values
                .insert(result.place, bytes.with_descriptor_home(copied.clone()))?;
        }
        if self
            .structural_types()
            .get(&result.structural_type)
            .is_some_and(|declaration| {
                matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
            })
        {
            let root = self.structural_value(source)?;
            let descriptor = if StructuralRuntimePlace::from(root)
                == StructuralRuntimePlace::from(&leaf)
                && root.structural_type == leaf.structural_type
            {
                let descriptor = self.element_view_values.require(&source)?;
                if !descriptor.matches_descriptor_home(root) {
                    return Err(malformed());
                }
                descriptor
            } else {
                self.structural_element_view_fields
                    .get(&StructuralRuntimePlace::from(&leaf))
                    .ok_or_else(malformed)?
            };
            if descriptor.access() != StructuralAccess::SharedBorrow {
                return Err(malformed());
            }
            let descriptor = descriptor.detached_descriptor();
            if self.element_view_values.contains_key(&result.place) {
                return Err(malformed());
            }
            self.element_view_values.insert(result.place, descriptor)?;
        }
        self.transplant_subtree(
            &StructuralRuntimePlace::from(&leaf),
            &StructuralRuntimePlace::from(&copied),
            false,
        )?;
        self.structural_values.insert(result.place, copied)?;
        self.track_affine_result(result);
        Ok(OperationFlow::Advance)
    }

    fn copy_scalar_array_payload(
        &mut self,
        source: &StructuralArgument,
        result: &terminal_psi::StructuralOperationResult,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        if result.multiplicity != StructuralMultiplicity::Unrestricted
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
            || !result.qualification_establishments.is_empty()
            || self.scalar_array_values.contains_key(&result.place)
            || self.structural_values.contains_key(&result.place)
        {
            return Err(malformed());
        }
        let mut copied = self.scalar_array_snapshot(source, result.structural_type)?;
        self.validate_scalar_array_qualifications(&copied, &result.qualifications)?;
        copied.qualifications = result.qualifications.clone();
        self.scalar_array_values.publish(result.place, copied)?;
        Ok(OperationFlow::Advance)
    }
    fn byte_carrier_leaf_copy(
        &mut self,
        leaf: &TerminalStructuralValue,
        result: &terminal_psi::StructuralOperationResult,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let Some((StructuralPathSegment::FixedIndex(index), parent_path)) = leaf.path.split_last()
        else {
            return Err(malformed());
        };
        let array = StructuralRuntimePlace {
            opaque_identity: leaf.opaque_identity,
            path: parent_path.to_vec(),
        };
        let bytes = self.structural_byte_storage.require(&array)?;
        let layout =
            byte_carrier_scalar_layout(self.structural_types().values(), result.structural_type)
                .ok_or_else(malformed)?;
        let index = usize::try_from(*index).map_err(|_| malformed())?;
        let mut leaves = Vec::with_capacity(layout.leaves.len());
        for member in &layout.leaves {
            let offset = index
                .checked_add(usize::try_from(member.offset).map_err(|_| malformed())?)
                .ok_or_else(malformed)?;
            let size = usize::from(match member.scalar_type {
                ScalarType::Boolean => 1u16,
                ScalarType::Integer(integer) => integer.bits() / 8,
                _ => return Err(malformed()),
            });
            let end = offset.checked_add(size).ok_or_else(malformed)?;
            let window = bytes.read_window(offset, end)?;
            let mut value = 0u128;
            for position in 0..size {
                value |= u128::from(window.read(position)?) << (8 * position);
            }
            let scalar = match member.scalar_type {
                ScalarType::Boolean => {
                    crate::storage::primitive_storage::encoding::boolean_bits(value)?
                }
                ScalarType::Integer(scalar_type) => {
                    let value = match scalar_type.sign() {
                        IntegerSign::Signed => {
                            let bits = u32::from(scalar_type.bits());
                            let signed = if bits < 128 && (value >> (bits - 1)) & 1 == 1 {
                                (value | (!0u128 << bits)) as i128
                            } else {
                                value as i128
                            };
                            IntegerValue::Signed(signed)
                        }
                        IntegerSign::Unsigned => IntegerValue::Unsigned(value),
                    };
                    TerminalScalarValue::Integer { scalar_type, value }
                }
                _ => return Err(malformed()),
            };
            leaves.push((member.field, scalar));
        }
        let copied = TerminalStructuralValue::minted(
            self.local_structural_identities.allocate()?,
            result.structural_type,
            Vec::new(),
        );
        let parent = StructuralRuntimePlace::from(&copied);
        for (field, scalar) in leaves {
            self.structural_scalar_fields.insert(
                StructuralScalarRuntimeField::member(parent.clone(), field),
                scalar,
            )?;
        }
        self.structural_values.insert(result.place, copied)?;
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn execute_structural_case_membership(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        dispatched_operation!(operation => StructuralCaseMembership { source, ref path, case });
        let active_case = self.observe_structural_case(source, path)?;
        self.store_scalar(operation, TerminalScalarValue::Boolean(active_case == case))
    }

    pub(crate) fn observed_field_cell(
        &self,
        source: semantic_vocabulary::PlaceId,
        path: &[CanonicalStructuralPathSegment],
        field: semantic_vocabulary::StructuralFieldId,
    ) -> Result<StructuralScalarRuntimeField, TerminalInterpretError> {
        let (structural_value, view) = self.observation_view_root(source)?;
        self.observed_field_cell_in(&structural_value, view, path, field)
    }

    pub(crate) fn observed_field_cell_in(
        &self,
        structural_value: &crate::boundary::values::TerminalStructuralValue,
        view: Option<AggregateViewTip>,
        path: &[CanonicalStructuralPathSegment],
        field: semantic_vocabulary::StructuralFieldId,
    ) -> Result<StructuralScalarRuntimeField, TerminalInterpretError> {
        let translated;
        let (path, field) = match view {
            None => (path, field),
            Some(AggregateViewTip::Aggregate { view, backing })
                if backing == structural_value.structural_type =>
            {
                let (backing_path, backing_field) = translate_aggregate_view_field(
                    &self.structural_types(),
                    view,
                    backing,
                    path,
                    field,
                )
                .ok_or_else(malformed)?;
                translated = backing_path;
                (translated.as_slice(), backing_field)
            }
            Some(_) => return Err(malformed()),
        };
        let mut parent = StructuralRuntimePlace::from(structural_value);
        if let Some(carrier) = terminal_semantics::case_payload_carrier(
            self.structural_types().values(),
            structural_value.structural_type,
            path,
        ) {
            parent.path.extend(carrier.path);
            let contents = self.structural_cases.require(&parent)?;
            if contents.structural_type != carrier.sum || contents.case != carrier.case {
                return Err(malformed());
            }
            return Ok(StructuralScalarRuntimeField::case_payload(parent, field));
        }
        let carrier = record_field_carrier(
            self.structural_types().values(),
            structural_value.structural_type,
            path,
        )
        .ok_or(malformed())?;
        parent.path.extend(carrier.path);
        Ok(StructuralScalarRuntimeField::member(parent, field))
    }

    pub(crate) fn execute_boolean_structural_field(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        dispatched_operation!(operation => BooleanStructuralField { source, ref path, field });
        self.require_boolean_result(operation)?;
        let value = self.read_observed_field(source, path, field, ScalarType::Boolean)?;
        self.store_scalar(operation, value)
    }

    pub(crate) fn execute_scalar_structural_field(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        dispatched_operation!(operation => ScalarStructuralField { source, ref path, field });
        let scalar_type = operation.result.expect_scalar().scalar_type;
        let value = self.read_observed_field(source, path, field, scalar_type)?;
        self.store_scalar(operation, value)
    }

    fn read_observed_field(
        &self,
        source: semantic_vocabulary::PlaceId,
        path: &[CanonicalStructuralPathSegment],
        field: semantic_vocabulary::StructuralFieldId,
        scalar_type: ScalarType,
    ) -> Result<TerminalScalarValue, TerminalInterpretError> {
        let (structural_value, view) = self.observation_view_root(source)?;
        let declared_root = match view {
            Some(AggregateViewTip::Aggregate { view, .. }) => view.structural_type,
            Some(AggregateViewTip::Scalar { .. } | AggregateViewTip::ScalarRegion { .. }) => {
                return Err(malformed());
            }
            None => structural_value.structural_type,
        };
        let declaration = observed_field_declaration(
            self.structural_types().values(),
            declared_root,
            path,
            field,
        )
        .ok_or_else(malformed)?;
        if declaration.field_type.scalar_type() != Some(scalar_type) {
            return Err(malformed());
        }
        if path.contains(&CanonicalStructuralPathSegment::Referent) {
            let mut selected = path.to_vec();
            selected.push(CanonicalStructuralPathSegment::Field(field));
            let physical = terminal_semantics::canonical_structural_storage_path(
                self.structural_types().values(),
                declared_root,
                &selected,
            )
            .ok_or_else(malformed)?;
            let (storage, actual, shared) = self.primitive_access(source, false, &physical)?;
            return self.read_primitive_storage(source, storage, actual, scalar_type, shared);
        }
        if let Some(view) = view {
            let carrier =
                record_field_carrier(self.structural_types().values(), declared_root, path)
                    .ok_or_else(malformed)?;
            let mut logical = carrier.path;
            logical.push(StructuralPathSegment::Field(declaration.identity));
            let (storage, logical_scalar, shared_view) = self.aggregate_view_primitive_access(
                &structural_value,
                view,
                &logical,
                self.aggregate_argument_backings
                    .get(&source)
                    .and_then(|backing| backing.collection_home.as_ref()),
                false,
            )?;
            return self.read_primitive_storage(
                source,
                storage,
                logical_scalar,
                scalar_type,
                shared_view,
            );
        }
        let cell = self.observed_field_cell(source, path, field)?;
        if view.is_none() {
            let mut leaf = structural_value.clone();
            leaf.path = cell.parent.path.clone();
            leaf.path
                .push(StructuralPathSegment::Field(declaration.identity));
            if let Some((storage, logical_scalar)) =
                self.reference_byte_region_access(&structural_value, &leaf)?
            {
                return self.read_primitive_storage(
                    source,
                    storage,
                    logical_scalar,
                    scalar_type,
                    false,
                );
            }
        }
        let value = self
            .structural_scalar_fields
            .get(&cell)
            .map(crate::storage::scalar_storage::ScalarCell::read)
            .transpose()?
            .ok_or(if scalar_type == ScalarType::Boolean {
                TerminalInterpretError::StructuralBooleanFieldMissing { source, field }
            } else {
                TerminalInterpretError::StructuralScalarFieldMissing { source, field }
            })?;
        if value.scalar_type() != scalar_type {
            return Err(malformed());
        }
        Ok(value)
    }

    pub(crate) fn execute_byte_sequence_read(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        dispatched_operation!(operation => ByteSequenceRead { source, index, length, .. });
        let result = operation.result.expect_scalar();
        let byte_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 is valid");
        let count_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid");
        if result.scalar_type != ScalarType::Integer(byte_type) {
            return Err(malformed());
        }
        let count = |operand| -> Result<u64, TerminalInterpretError> {
            let value = self.scalar_value(operand)?;
            let TerminalScalarValue::Integer {
                scalar_type,
                value: IntegerValue::Unsigned(value),
            } = value
            else {
                return Err(malformed());
            };
            if scalar_type != count_type {
                return Err(malformed());
            }
            u64::try_from(value).map_err(|_| malformed())
        };
        let byte_index = count(index)?;
        let byte_length = count(length)?;
        if self.byte_sequence_length(source)? != byte_length || byte_index >= byte_length {
            return Err(malformed());
        }
        let byte_index = usize::try_from(byte_index).map_err(|_| malformed())?;
        let byte = self.byte_sequence_read(source, byte_index)?;
        self.values.insert(
            result.id,
            TerminalScalarValue::Integer {
                scalar_type: byte_type,
                value: IntegerValue::Unsigned(u128::from(byte)),
            },
        )?;
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn execute_byte_sequence_length(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::ByteSequenceLength { source } = &operation.kind else {
            unreachable!("dispatched execute_byte_sequence_length")
        };
        let result = operation.result.expect_scalar();
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid");
        if result.scalar_type != ScalarType::Integer(integer_type) {
            return Err(malformed());
        }
        if let Some(bytes) = self.fixed_byte_array_observation(source)? {
            self.values.insert(
                result.id,
                TerminalScalarValue::Integer {
                    scalar_type: integer_type,
                    value: IntegerValue::Unsigned(bytes.len() as u128),
                },
            )?;
            return Ok(OperationFlow::Advance);
        }
        self.validate_descriptor_borrow(source, StructuralAccess::SharedBorrow)?;
        let selected = self.resolve_live_projection(
            source.place,
            &source.path,
            StructuralAccess::SharedBorrow,
        )?;
        let binding = self.byte_descriptor_binding(source, &selected)?;
        let length = self.byte_descriptor_length(&binding, &selected)?;
        self.values.insert(
            result.id,
            TerminalScalarValue::Integer {
                scalar_type: integer_type,
                value: IntegerValue::Unsigned(u128::from(length)),
            },
        )?;
        Ok(OperationFlow::Advance)
    }
}
