//! Structural operations of the interpreter loop: descriptor stores,
//! scalar cases, byte-sequence literals and reads, trivial affine
//! locals, records, field stores and field reads.
//!
//! Structural windows use live projection to cross stored reference and view
//! descriptors before applying the final inline field geometry. A binding's
//! view first translates its own index or complete fixed-array companion;
//! interpreting the remaining descriptor path as inline storage loses the
//! original backing. Whole-referent authority and restoration custody remain
//! separate obligations, not consequences of resolving that storage.

use crate::byte_sequences::ByteSequenceView;
use crate::byte_sequences::binding::ByteSequenceBinding;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::{OperationFlow, TerminalExecution};
use crate::record::storage::{ConstructorStorage, ConstructorValue};
use crate::scalar_operations::terminal_scalar_belongs_to_type;
use crate::structural_inputs::case_membership::StructuralCaseContents;
use crate::values::{StructuralByteSequenceRuntimeField, StructuralRuntimePlace};
use crate::values::{StructuralFieldNamespace, StructuralScalarRuntimeField};
use crate::values::{TerminalScalarCaseValue, TerminalScalarValue, TerminalStructuralValue};
use semantic_vocabulary::{IntegerType, IntegerValue, ScalarType};
use std::collections::BTreeMap;
use terminal_psi::{
    OperationKind, StructuralAccess, StructuralAffineDiscard, StructuralArgument,
    StructuralFieldType, StructuralMultiplicity, StructuralPathSegment, StructuralTypeShape,
    find_structural_case,
};

impl TerminalExecution<'_> {
    pub(crate) fn execute_establish_scalar_case(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::EstablishScalarCase {
            result_case,
            ref fields,
        } = operation.kind
        else {
            unreachable!("dispatched execute_establish_scalar_case")
        };
        let terminal_psi::OperationResult::Structural(result) = &operation.result else {
            return Err(malformed());
        };
        if self.structural_values.contains_key(&result.place)
            || !matches!(
                result.multiplicity,
                terminal_psi::nonlinear_multiplicity_pair_pattern!()
            )
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
        {
            return Err(malformed());
        }
        let value = TerminalScalarCaseValue {
            structural_type: result.structural_type,
            result_case,
            fields: fields
                .iter()
                .map(|binding| Ok((binding.field, self.scalar_value(binding.value)?)))
                .collect::<Result<Vec<_>, TerminalInterpretError>>()?,
        };
        self.publish_scalar_case(result, value)?;
        Ok(OperationFlow::Advance)
    }

    /// Import semantic scalar-case contents into the same placed storage as
    /// every other constructed sum. The external payload supplies no identity,
    /// so publication allocates one home only after validating all contents.
    pub(crate) fn publish_scalar_case(
        &mut self,
        result: &terminal_psi::StructuralOperationResult,
        case: TerminalScalarCaseValue,
    ) -> Result<(), TerminalInterpretError> {
        if case.structural_type != result.structural_type
            || self.structural_values.contains_key(&result.place)
            || self.scalar_array_values.contains_key(&result.place)
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
            || !matches!(
                result.multiplicity,
                terminal_psi::nonlinear_multiplicity_pair_pattern!()
            )
        {
            return Err(malformed());
        }
        case.validate_against(
            self.program
                .structural_types()
                .get(&result.structural_type)
                .ok_or_else(malformed)?,
        )?;
        let value = TerminalStructuralValue::minted(
            self.local_structural_identities.allocate()?,
            result.structural_type,
            Vec::new(),
        );
        let root = StructuralRuntimePlace::from(&value);
        let mut storage = ConstructorStorage::default();
        storage.stage_case(root.clone(), case.structural_type, case.result_case);
        for (field, scalar) in case.fields {
            storage.stage_scalar_endpoint(
                &crate::primitive_storage::PrimitiveStorageEndpoint::ScalarField(
                    StructuralScalarRuntimeField::case_payload(root.clone(), field),
                ),
                scalar,
            )?;
        }
        storage.publish(self)?;
        self.structural_values.insert(result.place, value);
        self.track_affine_result(result);
        Ok(())
    }

    /// Record the affine debt a fresh affine result owes the live frontier.
    fn track_affine_result(&mut self, result: &terminal_psi::StructuralOperationResult) {
        if result.multiplicity == StructuralMultiplicity::Affine {
            self.live_affine_frontier.insert(StructuralAffineDiscard {
                place: result.place,
                path: Vec::new(),
                structural_type: result.structural_type,
            });
        }
    }

    pub(crate) fn execute_establish_structural_case(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::EstablishStructuralCase {
            result_case,
            ref fields,
        } = operation.kind
        else {
            unreachable!("dispatched execute_establish_structural_case")
        };
        let terminal_psi::OperationResult::Structural(result) = &operation.result else {
            return Err(malformed());
        };
        self.establish_structural_case(result, result_case, fields)?;
        Ok(OperationFlow::Advance)
    }

    /// Stage a case's non-scalar payload the way `establish_record` stages
    /// record members: every child subtree copies (or, for affine children,
    /// moves) into the fresh identity under `Field(member)` prefixes, and the
    /// selected case's discriminator lands on the new root.
    ///
    /// The roster is the one Terminal verification checks
    /// (`validation/structural/case.rs`): a mixed shape's runtime common
    /// fields in declaration order, then the selected case's runtime payload.
    /// Erased members take no position. Common and payload identities are
    /// separate declaration namespaces, so a roster entry's position, not its
    /// field identity, names its declaration, and each scalar lands in its
    /// own cell namespace on the new root.
    fn establish_structural_case(
        &mut self,
        result: &terminal_psi::StructuralOperationResult,
        result_case: semantic_vocabulary::StructuralCaseId,
        fields: &[terminal_psi::RecordFieldInitializer],
    ) -> Result<(), TerminalInterpretError> {
        if self.structural_values.contains_key(&result.place) || !result.qualifications.is_empty() {
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
        let mut value = TerminalStructuralValue {
            opaque_identity: self.local_structural_identities.allocate()?,
            structural_type: result.structural_type,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            path: Vec::new(),
        };
        let member_root = StructuralRuntimePlace::from(&value);
        let mut staged_scalars: BTreeMap<StructuralScalarRuntimeField, TerminalScalarValue> =
            BTreeMap::new();
        let mut storage = ConstructorStorage::default();
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
                    staged_scalars.insert(
                        StructuralScalarRuntimeField {
                            parent: member_root.clone(),
                            namespace,
                            field: binding.field,
                        },
                        scalar,
                    );
                }
                terminal_psi::RecordFieldValue::Structural(argument) => {
                    if argument.access != StructuralAccess::Owned {
                        return Err(malformed());
                    }
                    let member = StructuralRuntimePlace {
                        opaque_identity: value.opaque_identity,
                        path: vec![StructuralPathSegment::Field(declaration.identity.clone())],
                    };
                    // Custody retains the namespace; physical payload storage
                    // overlays the sum and therefore omits its Case segment.
                    let mut claim_path = Vec::new();
                    if namespace == StructuralFieldNamespace::CasePayload {
                        claim_path.push(StructuralPathSegment::Case(result_case));
                    }
                    claim_path.extend_from_slice(&member.path);
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
                        );
                        continue;
                    }
                    let transfers_custody =
                        self.stage_constructor_operand(argument, &mut frontier, &mut consumed)?;
                    if declaration.field_type
                        != StructuralFieldType::Structural(child.structural_type())
                    {
                        return Err(malformed());
                    }
                    // Only payload memberships select Case; common children
                    // remain reachable independently of the active tag.
                    child.append_memberships(&claim_path, &mut value.projected_qualifications);
                    storage.stage(self, argument, &child, &member, transfers_custody)?;
                }
            }
        }
        storage.stage_case(member_root, result.structural_type, result_case);
        value.projected_qualifications.sort();
        value.projected_qualifications.dedup();
        storage.validate_projected_memberships(self, &value, &result.projected_qualifications)?;
        let claims =
            self.stage_constructor_claims(result, &claim_operands, &mut frontier, &|current| {
                storage.case_at_projection(self, &value, &current.path)
            })?;
        self.commit_constructor_operands(frontier, &consumed, claims)?;
        self.structural_scalar_fields.extend(staged_scalars);
        storage.publish(self)?;
        self.structural_values.insert(result.place, value);
        Ok(())
    }

    /// Retire precisely the consumed subtree after its cells have been staged.
    pub(crate) fn prune_subtree(&mut self, from: &StructuralRuntimePlace) {
        crate::structural_storage::prune(&mut self.structural_scalar_fields, from);
        crate::structural_storage::prune(&mut self.structural_byte_sequence_fields, from);
        crate::structural_storage::prune(&mut self.structural_byte_storage, from);
        crate::structural_storage::prune(&mut self.structural_cases, from);
        crate::structural_storage::prune(&mut self.reference_referents, from);
        crate::structural_storage::prune(&mut self.structural_element_view_fields, from);
        crate::structural_storage::prune(&mut self.structural_primitive_storage, from);
    }

    pub(crate) fn execute_establish_byte_sequence_literal(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::EstablishByteSequenceLiteral {
            destination,
            ref bytes,
            ..
        } = operation.kind
        else {
            unreachable!("dispatched execute_establish_byte_sequence_literal")
        };
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
            // Reentering the unique literal producer retains the
            // same immutable value. Fuel was charged above;
            // existing aliases keep their exact original bytes.
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
                    ByteSequenceBinding::Immutable(ByteSequenceView::new(bytes.clone())),
                )
                .is_some()
            {
                return Err(malformed());
            }
            self.structural_values.insert(
                destination,
                TerminalStructuralValue {
                    opaque_identity: destination.get(),
                    structural_type: *structural_type,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    path: Vec::new(),
                },
            );
        }
        Ok(OperationFlow::Advance)
    }

    /// Materialize a zeroed establishment: scalar leaves receive zero and
    /// bounded byte fields receive empty live backing under the fresh identity.
    /// Whether the result `place` already carries a materialized value or a
    /// live claim (the shared tail of every fresh-place admissibility guard).
    fn result_place_in_use(&self, place: semantic_vocabulary::PlaceId) -> bool {
        self.structural_values.contains_key(&place)
            || self.scalar_array_values.contains_key(&place)
            || self
                .live_claims
                .values()
                .any(|claim| claim.place == Some(place))
    }

    pub(crate) fn execute_establish_zeroed_structural(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::EstablishZeroedStructural {} = operation.kind else {
            unreachable!("dispatched execute_establish_zeroed_structural")
        };
        let result = operation.result.structural().ok_or_else(malformed)?;
        if !matches!(
            result.multiplicity,
            terminal_psi::nonlinear_multiplicity_pair_pattern!()
        ) || !terminal_semantics::zeroed_projected_byte_qualifications_hold(
            self.program.structural_types().values(),
            self.program.catalogs.structural_domains.iter(),
            result.structural_type,
            &result.projected_qualifications,
        ) || !result.claims.is_empty()
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
        self.materialize_zeroed_structural_storage(&value)?;
        self.structural_values.insert(result.place, value);
        // Zeroing establishes payload; it does not discharge the nominal
        // owner's affine debt. The verified continuation still disposes it.
        self.track_affine_result(result);
        Ok(OperationFlow::Advance)
    }

    /// One materializer for both local zero establishments and explicit host
    /// zero-backed entry inputs. It creates storage, never qualifications,
    /// authority, claims, or release receipts.
    pub(crate) fn materialize_zeroed_structural_storage(
        &mut self,
        value: &TerminalStructuralValue,
    ) -> Result<(), TerminalInterpretError> {
        self.materialize_zeroed_structural_storage_with_established_fields(value, &[])
    }

    pub(crate) fn materialize_zeroed_structural_storage_with_established_fields(
        &mut self,
        value: &TerminalStructuralValue,
        established_fields: &[terminal_semantics::EstablishedErasedStructuralField],
    ) -> Result<(), TerminalInterpretError> {
        let leaves = terminal_semantics::zeroed_structural_leaves_with_established_erased_fields(
            self.program.structural_types().values(),
            value.structural_type,
            established_fields,
        )
        .ok_or(TerminalInterpretError::UnsupportedSemanticVariant(
            "zeroed structural backing or erased establishment cannot be materialized",
        ))?;
        let zero = |scalar_type: ScalarType| -> Option<TerminalScalarValue> {
            match scalar_type {
                ScalarType::Boolean => Some(TerminalScalarValue::Boolean(false)),
                ScalarType::Integer(integer_type) => Some(TerminalScalarValue::Integer {
                    scalar_type: integer_type,
                    value: integer_type.zero(),
                }),
                ScalarType::IeeeFloat(format) => {
                    Some(TerminalScalarValue::IeeeFloat(match format {
                        semantic_vocabulary::IeeeFloatFormat::Binary32 => {
                            semantic_vocabulary::IeeeFloatValue::Binary32(0)
                        }
                        semantic_vocabulary::IeeeFloatFormat::Binary64 => {
                            semantic_vocabulary::IeeeFloatValue::Binary64(0)
                        }
                    }))
                }
            }
        };
        for leaf in &leaves {
            // Runtime places name a case payload member by its field alone;
            // the leaf's `Case` segment only selects that payload's fields.
            let storage_path = || {
                let mut path = value.path.clone();
                path.extend(
                    leaf.path()
                        .iter()
                        .filter(|segment| !matches!(segment, StructuralPathSegment::Case(_)))
                        .cloned(),
                );
                path
            };
            match leaf {
                terminal_semantics::ZeroedStructuralLeaf::RecordField {
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
                        .insert(cell, zero(*scalar_type).ok_or_else(malformed)?);
                }
                terminal_semantics::ZeroedStructuralLeaf::ByteSequenceField { field, .. } => {
                    self.structural_byte_sequence_fields.insert(
                        StructuralByteSequenceRuntimeField {
                            parent: StructuralRuntimePlace {
                                opaque_identity: value.opaque_identity,
                                path: storage_path(),
                            },
                            namespace: StructuralFieldNamespace::for_carrier(leaf.path()),
                            field: *field,
                        },
                        ByteSequenceBinding::Immutable(ByteSequenceView::new(Vec::new())),
                    );
                }
                terminal_semantics::ZeroedStructuralLeaf::CaseField {
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
                    );
                }
                terminal_semantics::ZeroedStructuralLeaf::ByteArray { length, .. } => {
                    // Byte views and scalar indexing must share one backing.
                    // Storing these bytes as separate scalar cells would make
                    // writes through a borrowed byte view invisible to the owner.
                    let length = usize::try_from(*length).map_err(|_| malformed())?;
                    self.structural_byte_storage
                        .entry(StructuralRuntimePlace {
                            opaque_identity: value.opaque_identity,
                            path: storage_path(),
                        })
                        .or_insert_with(|| ByteSequenceView::new(vec![0; length]));
                }
                terminal_semantics::ZeroedStructuralLeaf::ArrayScalar { scalar_type, .. } => {
                    self.structural_primitive_storage.insert(
                        StructuralRuntimePlace {
                            opaque_identity: value.opaque_identity,
                            path: storage_path(),
                        },
                        zero(*scalar_type).ok_or_else(malformed)?,
                    );
                }
                terminal_semantics::ZeroedStructuralLeaf::SumSelection {
                    path: _,
                    structural_type,
                    case,
                    ..
                } => {
                    // The tag is not independently addressable storage — the
                    // structural_cases entry IS the runtime discriminator.
                    self.structural_cases.insert(
                        StructuralRuntimePlace {
                            opaque_identity: value.opaque_identity,
                            path: storage_path(),
                        },
                        StructuralCaseContents {
                            structural_type: *structural_type,
                            case: *case,
                        },
                    );
                }
            }
        }
        Ok(())
    }

    /// Establish a declared structural local's home: the place materializes
    /// as the operation's structural result with no leaf storage — member
    /// operations write each field's backing as they execute.
    pub(crate) fn execute_establish_structural_local(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::EstablishStructuralLocal {} = operation.kind else {
            unreachable!("dispatched execute_establish_structural_local")
        };
        let result = operation.result.structural().ok_or_else(malformed)?;
        // A descriptor home may declare byte qualifications its stores prove.
        let descriptor_home = matches!(
            self.program.structural_types().get(&result.structural_type),
            Some(declaration) if declaration.shape.is_borrowed_byte_view()
        );
        if !matches!(
            result.multiplicity,
            terminal_psi::nonlinear_multiplicity_pair_pattern!()
        ) || (!result.qualifications.is_empty() && !descriptor_home)
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
            || self.result_place_in_use(result.place)
        {
            return Err(malformed());
        }
        self.structural_values.insert(
            result.place,
            TerminalStructuralValue {
                opaque_identity: self.local_structural_identities.allocate()?,
                structural_type: result.structural_type,
                qualifications: result.qualifications.clone(),
                projected_qualifications: Vec::new(),
                path: Vec::new(),
            },
        );
        self.track_affine_result(result);
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn execute_establish_trivial_affine_local(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::EstablishTrivialAffineLocal { destination } = operation.kind else {
            unreachable!("dispatched execute_establish_trivial_affine_local")
        };
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
            TerminalStructuralValue {
                opaque_identity: destination.get(),
                structural_type: *structural_type,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                path: Vec::new(),
            },
        );
        self.live_affine_frontier.insert(StructuralAffineDiscard {
            place: destination,
            path: Vec::new(),
            structural_type: *structural_type,
        });
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn execute_establish_record(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::EstablishRecord { ref fields } = operation.kind else {
            unreachable!("dispatched execute_establish_record")
        };
        self.establish_record(operation.result.structural().ok_or(malformed())?, fields)?;
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn execute_structural_scalar_field_store(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::StructuralScalarFieldStore {
            destination,
            ref path,
            field,
            value,
            ..
        } = operation.kind
        else {
            unreachable!("dispatched execute_structural_scalar_field_store")
        };
        if !matches!(operation.result, terminal_psi::OperationResult::Unit) {
            return Err(malformed());
        }
        let machine = self.program.verified_machine(self.current_machine)?;
        let module = &self.program.catalogs;
        let preserves = |root, whole: &[_], projected: &[_]| {
            terminal_semantics::scalar_field_store_preserves_qualifications(
                self.program.structural_types().values(),
                module.structural_domains.iter(),
                &module.identities,
                root,
                whole,
                projected,
                path,
                field,
            ) && terminal_semantics::scalar_field_store_preserves_content(
                self.program.structural_types().values(),
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
                // The declaration and live value identify the owned home.
                // Initialization syntax does not confer storage authority.
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
        // Owning a reference carrier permits a write through its exclusive
        // loan, not ownership of the referent. Cross retained descriptors at
        // the requested write authority before selecting the scalar endpoint.
        let projection_access = match access {
            StructuralAccess::Owned => StructuralAccess::WriteOnlyBorrow,
            access => access,
        };
        // A final case step selects a payload namespace, not another value.
        let (carrier_path, namespace) = match path.split_last() {
            Some((StructuralPathSegment::Case(case), prefix)) => (
                prefix,
                vec![semantic_vocabulary::CanonicalStructuralPathSegment::Case(
                    *case,
                )],
            ),
            _ => (path.as_slice(), Vec::new()),
        };
        let (parent, view) =
            self.resolve_live_view_projection(destination, carrier_path, projection_access)?;
        let declared_parent = match view {
            None => parent.structural_type,
            Some(crate::reference::AggregateViewTip::Aggregate { view, backing })
                if backing == parent.structural_type =>
            {
                view.structural_type
            }
            Some(_) => return Err(malformed()),
        };
        let types = self.program.structural_types();
        let member = terminal_semantics::observed_field_declaration(
            types.values(),
            declared_parent,
            &namespace,
            field,
        )
        .ok_or_else(malformed)?;
        if member.relevance.is_erased()
            || member.field_type.scalar_type() != Some(source.scalar_type())
            || !terminal_scalar_belongs_to_type(source)
        {
            return Err(malformed());
        }
        if let Some(view) = view {
            let carrier = terminal_semantics::record_field_carrier(
                types.values(),
                declared_parent,
                &namespace,
            )
            .ok_or_else(malformed)?;
            let mut logical = carrier.path;
            logical.push(StructuralPathSegment::Field(member.identity));
            let (storage, scalar_type, _) =
                self.aggregate_view_primitive_access(&parent, view, &logical)?;
            self.store_primitive_storage(destination, storage, scalar_type, source)?;
            return Ok(OperationFlow::Advance);
        }
        let cell = self.observed_field_cell_in(&parent, None, &namespace, field)?;
        let mut leaf = parent.clone();
        leaf.path = cell.parent.path.clone();
        leaf.path
            .push(StructuralPathSegment::Field(member.identity.clone()));
        if let Some((storage, scalar_type)) = self.reference_byte_region_access(&parent, &leaf)? {
            self.store_primitive_storage(destination, storage, scalar_type, source)?;
            return Ok(OperationFlow::Advance);
        }
        self.structural_scalar_fields.insert(cell, source);
        Ok(OperationFlow::Advance)
    }

    /// Move a declared structural subtree out of a borrowed root into a fresh
    /// place. Every runtime cell beneath the hole — scalar leaves, byte
    /// contents, cases, primitive cells, reference referents — travels to the
    /// moved value's own opaque identity, so the borrowed root keeps the
    /// caller's identity while its absent subtree is unreachable: reads under
    /// it observe missing storage until a repair store reseats the exact hole.
    pub(crate) fn execute_move_structural_field(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::MoveStructuralField {
            source,
            ref path,
            field,
        } = operation.kind
        else {
            unreachable!("dispatched execute_move_structural_field")
        };
        let result = operation.result.structural().ok_or_else(malformed)?;
        if self.structural_values.contains_key(&result.place) {
            return Err(malformed());
        }
        let storage = self.structural_values.get(&source).ok_or_else(malformed)?;
        let selected = self.structural_window_hole(
            source,
            storage,
            path,
            field,
            StructuralAccess::MutableBorrow,
        )?;
        if selected.structural_type != result.structural_type
            || result
                .qualifications
                .iter()
                .any(|domain| !selected.qualifications.contains(domain))
            || result
                .projected_qualifications
                .iter()
                .any(|row| !selected.projected_qualifications.contains(row))
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
        self.relocate_subtree(&hole, &StructuralRuntimePlace::from(&moved));
        self.structural_values.insert(result.place, moved);
        self.live_affine_frontier = frontier;
        self.live_claims = claims;
        Ok(OperationFlow::Advance)
    }

    /// Reseat an open borrowed hole with a consumed whole-place value. The
    /// value's cells relocate under the destination's opaque identity at the
    /// exact hole path, so a caller observing the borrowed root afterward sees
    /// the updated contents. The consumed place leaves both the value map and
    /// the affine frontier — the store is the move's exact-once counterpart.
    pub(crate) fn execute_store_structural_field(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::StoreStructuralField {
            disposition,
            destination,
            ref path,
            field,
            ref value,
        } = operation.kind
        else {
            unreachable!("dispatched execute_store_structural_field")
        };
        if !matches!(operation.result, terminal_psi::OperationResult::Unit)
            || value.access != StructuralAccess::Owned
            || !value.path.is_empty()
        {
            return Err(malformed());
        }
        let machine = self
            .program
            .machines()
            .get(&self.current_machine)
            .ok_or_else(malformed)?;
        if !terminal_semantics::structural_store_value_admitted(
            machine,
            self.program.structural_types().values(),
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
        // Packed values have an activation home rather than opaque payload
        // cells. Reuse ordinary loan preparation's staged storage identity for
        // geometry and custody; actual writes still resolve the pinned frame.
        let mut identity_cursor = self.local_structural_identities.cursor();
        let (storage, _) = self
            .array_storage_root_staged(destination, &mut identity_cursor, &mut BTreeMap::new())
            .map_err(|error| failure("destination home", error))?;
        let selected = self
            .structural_window_hole(
                destination,
                &storage,
                path,
                field,
                disposition.required_access(),
            )
            .map_err(|error| failure("destination storage", error))?;
        let field_type = selected.structural_type;
        let hole = StructuralRuntimePlace::from(&selected);
        let (required, projected_required) = self
            .structural_window_requirements(operation, destination, path, field)
            .map_err(|error| failure("receiving memberships", error))?;
        let mut frontier = self.live_affine_frontier.clone();
        frontier.retain(|entry| entry.place != value.place);
        self.restore_store_frontier(&mut frontier, destination, path, &hole, field_type)
            .map_err(|error| failure("restoration frontier", error))?;
        let claims = self
            .stage_window_store_claims(
                destination,
                &storage,
                path,
                &hole,
                value.place,
                &mut frontier,
            )
            .map_err(|error| failure("restored claims", error))?;
        // Completed primitive arrays keep their payload outside the opaque
        // structural-place map. Resolve both representations, and stage their
        // exact typed leaves before publishing the same window transaction.
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
        // Stage memberships with the custody transaction. Only the repaired
        // region receives the replacement's actual roster; retained siblings
        // and containing opaque memberships are never re-established here.
        let mut memberships = Vec::new();
        for (place, owner) in &self.structural_values {
            if owner.opaque_identity != hole.opaque_identity {
                continue;
            }
            let Some(relative) = hole.path.strip_prefix(owner.path.as_slice()) else {
                continue;
            };
            if terminal_psi::StructuralPathQualification::select_restorable(
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
                terminal_psi::StructuralPathQualification::append_under(
                    qualifications,
                    projected_qualifications,
                    relative,
                    &mut repaired.projected_qualifications,
                );
                repaired.projected_qualifications.sort();
                repaired.projected_qualifications.dedup();
            }
            memberships.push((*place, repaired));
        }
        // Resolve scalar writes relative to the authored loan before retiring
        // anything. Its payload may be pinned in a suspended caller, not in the
        // opaque maps. Descriptor-bearing values keep the construction storage
        // transaction, including standalone view descriptors and their custody.
        let mut destination_path = path.clone();
        if field.is_some() {
            let Some(segment @ StructuralPathSegment::Field(_)) = hole.path.last() else {
                return Err(malformed());
            };
            destination_path.push(segment.clone());
        }
        if let Some(writes) = child
            .replacement_writes(self, value, destination, &destination_path)
            .map_err(|error| failure("scalar storage preparation", error))?
        {
            writes.commit(self)?;
        } else {
            let mut storage = ConstructorStorage::default();
            storage.stage(self, value, &child, &hole, !copies_storage)?;
            let source = match child {
                ConstructorValue::Placed(child) => Some(StructuralRuntimePlace::from(&child)),
                ConstructorValue::PackedScalars { .. } => None,
            };
            self.prune_subtree(&hole);
            if !copies_storage && let Some(source) = source {
                self.prune_subtree(&source);
            }
            storage.publish(self)?;
        }
        self.structural_values.extend(memberships);
        if !copies_storage {
            self.structural_values.remove(&value.place);
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

    /// Repair owes the destination's declared memberships, not any additional
    /// facts held by the detached old value. Rejoin the exact producer/parameter
    /// contract and declaration path independently of the live payload below.
    fn structural_window_requirements(
        &self,
        operation: &terminal_psi::Operation,
        root: semantic_vocabulary::PlaceId,
        path: &[StructuralPathSegment],
        field: Option<semantic_vocabulary::StructuralFieldId>,
    ) -> Result<
        (
            Vec<semantic_vocabulary::StructuralDomainId>,
            Vec<terminal_psi::StructuralPathQualification>,
        ),
        TerminalInterpretError,
    > {
        let machine = self.program.verified_machine(self.current_machine)?;
        // Runtime dispatch has already bounded and concretized every selector.
        // Type geometry still uses the authored operation's captured selector
        // coordinates: a concrete index alone cannot establish a view's extent.
        let declared = machine.operation(operation.id).ok_or_else(malformed)?;
        let OperationKind::StoreStructuralField {
            destination,
            path: declared_path,
            field: declared_field,
            ..
        } = &declared.kind
        else {
            return Err(malformed());
        };
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
            self.program.structural_types().values(),
            root_type,
            declared_path,
            field,
        )
        .ok_or_else(malformed)?;
        // Memberships belong to the actual selected cells. Keep the resolved
        // declaration-local field suffix while substituting concrete indexes.
        projection
            .path
            .get_mut(..path.len())
            .ok_or_else(malformed)?
            .clone_from_slice(path);
        terminal_psi::StructuralPathQualification::select_restorable(
            whole,
            projected,
            &projection.path,
        )
        .ok_or_else(malformed)
    }

    /// The resolved runtime hole path one window operation names and the
    /// hole's declared type. A root window (no field, empty path) is the
    /// loan's whole referent: the resolved root place itself. A field leaf
    /// resolves `path` to the containing record, then the hole sits one
    /// `Field` segment deeper under the field's declared identity. Without
    /// a field, the path already names the hole, including an array element
    /// or the element a mutable view selects in its collection.
    fn structural_window_hole(
        &self,
        root: semantic_vocabulary::PlaceId,
        storage: &TerminalStructuralValue,
        path: &[StructuralPathSegment],
        field: Option<semantic_vocabulary::StructuralFieldId>,
        required_access: StructuralAccess,
    ) -> Result<TerminalStructuralValue, TerminalInterpretError> {
        // Resolve a reference crossing against its live runtime referent first;
        // the shared window geometry receives storage, never carrier ownership.
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
            let machine = self
                .program
                .machines()
                .get(&self.current_machine)
                .ok_or_else(malformed)?;
            // `base` comes from the live, access-checked captured reference,
            // not raw pointer equality. Its semantic storage identity must
            // belong to this frame's exclusive ingress or live owned storage.
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
            // Storage authority comes from the live owned home. The store's
            // receiving-membership transaction independently proves every
            // restored value qualification before committing its bytes.
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
            // Normalize only this binding's view or fixed-array companion.
            // Its suffix can cross further descriptors; the shared live
            // projector follows those to their own backing and checks access.
            // Static window geometry cannot interpret that suffix as inline
            // collection storage, even after the outer index is concrete.
            let (collection, selected) =
                view.structural_projection(&self.program.structural_types(), path, true)?;
            self.resolve_reference_storage_projection(&collection, &selected, required_access)?
                .ok_or_else(malformed)?
                .value
        } else {
            // The ordinary live projection owner crosses every reference and
            // element-view descriptor, including fields and nested views. Window
            // extraction must reach that same original backing, not interpret a
            // descriptor's carrier bytes as inline elements. Whole-referent origin
            // authentication above and restoration custody below remain separate.
            self.resolve_storage_projection_with_leaf_type(
                root,
                storage,
                path,
                required_access,
                terminal_psi::StructuralFieldType::canonical_leaf_type,
            )?
            .value
        };
        let projection = terminal_semantics::structural_window_projection(
            self.program.structural_types().values(),
            selected.structural_type,
            &[],
            field,
        )
        .ok_or_else(malformed)?;
        let hole = crate::custody::project_structural_value(
            &self.program.structural_types(),
            &selected,
            &projection.path,
        )?;
        Ok(hole)
    }

    /// Stage every runtime cell under `from` at the same subtree suffix under
    /// `to`, then insert the staged cells. `vacate` retires the source
    /// subtree's cells for a move; a copy leaves `from` fully intact.
    fn transplant_subtree(
        &mut self,
        from: &StructuralRuntimePlace,
        to: &StructuralRuntimePlace,
        vacate: bool,
    ) {
        let retarget = |parent: &StructuralRuntimePlace| {
            (parent.opaque_identity == from.opaque_identity && parent.path.starts_with(&from.path))
                .then(|| StructuralRuntimePlace {
                    opaque_identity: to.opaque_identity,
                    path: to
                        .path
                        .iter()
                        .chain(&parent.path[from.path.len()..])
                        .cloned()
                        .collect(),
                })
        };
        let staged_scalars: Vec<_> =
            crate::structural_storage::cells(&self.structural_scalar_fields, from)
                .filter_map(|(cell, scalar)| {
                    retarget(&cell.parent).map(|parent| (cell.relocated(parent), *scalar))
                })
                .collect();
        let staged_byte_fields: Vec<_> =
            crate::structural_storage::cells(&self.structural_byte_sequence_fields, from)
                .filter_map(|(cell, view)| {
                    retarget(&cell.parent).map(|parent| (cell.relocated(parent), view.clone()))
                })
                .collect();
        let staged_arrays: Vec<_> =
            crate::structural_storage::cells(&self.structural_byte_storage, from)
                .filter_map(|(carrier, view)| retarget(carrier).map(|place| (place, view.clone())))
                .collect();
        let staged_cases: Vec<_> = crate::structural_storage::cells(&self.structural_cases, from)
            .filter_map(|(carrier, contents)| retarget(carrier).map(|place| (place, *contents)))
            .collect();
        let staged_element_views: Vec<_> =
            crate::structural_storage::cells(&self.structural_element_view_fields, from)
                .filter_map(|(carrier, descriptor)| {
                    retarget(carrier).map(|place| (place, descriptor.clone()))
                })
                .collect();
        let staged_referents: Vec<_> =
            crate::structural_storage::cells(&self.reference_referents, from)
                .filter_map(|(carrier, referent)| {
                    retarget(carrier).map(|place| (place, referent.clone()))
                })
                .collect();
        let staged_primitives: Vec<_> =
            crate::structural_storage::cells(&self.structural_primitive_storage, from)
                .filter_map(|(carrier, scalar)| retarget(carrier).map(|place| (place, *scalar)))
                .collect();
        self.prune_subtree(to);
        if vacate {
            self.prune_subtree(from);
        }
        self.structural_scalar_fields.extend(staged_scalars);
        self.structural_byte_sequence_fields
            .extend(staged_byte_fields);
        self.structural_byte_storage.extend(staged_arrays);
        self.structural_cases.extend(staged_cases);
        self.reference_referents.extend(staged_referents);
        self.structural_element_view_fields
            .extend(staged_element_views);
        self.structural_primitive_storage.extend(staged_primitives);
    }

    /// Move every runtime cell under `from` to the same subtree suffix under
    /// `to`. Cells are staged before removal so the source region is never
    /// partially vacated.
    fn relocate_subtree(&mut self, from: &StructuralRuntimePlace, to: &StructuralRuntimePlace) {
        self.transplant_subtree(from, to, true)
    }

    /// Copy every runtime cell under `from` to the same subtree suffix under
    /// `to`, leaving `from` intact. The `Unrestricted` leaf's cells duplicate
    /// beneath the result's own opaque identity; nothing vacates.
    fn clone_subtree(&mut self, from: &StructuralRuntimePlace, to: &StructuralRuntimePlace) {
        self.transplant_subtree(from, to, false)
    }

    /// Project an `Unrestricted` leaf out of a live root into a fresh owned
    /// place by copying its cells: the source subtree stays fully intact, so
    /// shared loans admit the observation where a move would vacate borrowed
    /// storage. The result's opaque identity is new; later writes to either
    /// carrier cannot alias. Copied shared descriptors still retain the same
    /// backing window; only their descriptor storage is independent.
    pub(crate) fn execute_structural_leaf_copy(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let result = operation.result.structural().ok_or_else(malformed)?;
        if let OperationKind::StructuralLeafCopy { source, path } = &operation.kind
            && path.is_empty()
            && let Some(terminal_psi::StructuralTypeDeclaration {
                shape:
                    StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned {
                        capacity,
                    }),
                ..
            }) = self.program.structural_types().get(&result.structural_type)
        {
            let source_value = self.structural_values.get(source).ok_or_else(malformed)?;
            let bytes = self
                .byte_sequence_values
                .get(source)
                .ok_or_else(malformed)?
                .immutable()?;
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
            // Snapshot the live byte value before publishing its independent
            // identity. Later indexed writes detach the immutable backing.
            let bytes = bytes.clone();
            let copied = TerminalStructuralValue {
                opaque_identity: self.local_structural_identities.allocate()?,
                structural_type: result.structural_type,
                qualifications: result.qualifications.clone(),
                projected_qualifications: Vec::new(),
                path: Vec::new(),
            };
            self.byte_sequence_values.insert(
                result.place,
                crate::byte_sequences::binding::ByteSequenceBinding::Immutable(
                    bytes.detached_descriptor(),
                ),
            );
            self.structural_values.insert(result.place, copied);
            return Ok(OperationFlow::Advance);
        }
        // A shared descriptor may carry affine disposal metadata even though
        // copying its permission is admitted. Its cells retain the captured
        // backing below; no referent snapshot or parent reborrow is created.
        let shared_descriptor = terminal_semantics::is_shared_reference_carrier(
            self.program.structural_types().values(),
            result.structural_type,
        );
        // Other affine payloads still need their owned frontier publication.
        if matches!(operation.kind, OperationKind::StructuralCaseLeafCopy { .. })
            && result.multiplicity != StructuralMultiplicity::Unrestricted
            && !shared_descriptor
        {
            return Err(TerminalInterpretError::AffineProjectionNotRepresentable);
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
        if let OperationKind::StructuralLeafCopy { source, path } = &operation.kind
            && terminal_semantics::scalar_array_leaf_shape(
                self.program.structural_types().values(),
                result.structural_type,
            )
            .is_some()
        {
            // Copy through the original source, not its opaque metadata alone:
            // a borrowed array's payload can live in a suspended caller frame.
            // Packed, placed and borrowed arrays establish independent completed
            // payloads; none transfers or fabricates source ownership.
            let argument = StructuralArgument {
                place: *source,
                path: path.clone(),
                access: StructuralAccess::SharedBorrow,
            };
            let value = if self.scalar_array_values.contains_key(source) {
                ConstructorValue::resolve(self, &argument)?
            } else {
                ConstructorValue::Placed(
                    self.resolve_live_leaf_copy(*source, path, result.structural_type)?
                        .0,
                )
            };
            if value.structural_type() == result.structural_type {
                if self.scalar_array_values.contains_key(&result.place) {
                    return Err(malformed());
                }
                let mut elements = Vec::new();
                value
                    .visit_scalar_leaves(self, &argument, |_, _, scalar| {
                        elements.push(scalar);
                        Ok(())
                    })?
                    .ok_or_else(malformed)?;
                // Plain results retain the completed row-major batch. Besides
                // avoiding a cell map per scalar, this makes a huge outer
                // dimension with zero inner extent constant-size storage.
                // Evidence-bearing results retain their placed metadata below.
                if result.is_unrestricted_plain() {
                    self.scalar_array_values.insert(
                        result.place,
                        crate::scalar_array::TerminalScalarArrayValue {
                            qualifications: Vec::new(),
                            structural_type: result.structural_type,
                            elements,
                        },
                    );
                    return Ok(OperationFlow::Advance);
                }
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
                let mut storage = ConstructorStorage::default();
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
                self.structural_values.insert(result.place, copied);
                self.local_structural_identities
                    .commit_cursor(identity_cursor);
                return Ok(OperationFlow::Advance);
            }
        }
        let (leaf, source, endpoint_field, copy_path) = match &operation.kind {
            OperationKind::StructuralLeafCopy { source, path } => {
                let (leaf, field) =
                    self.resolve_live_leaf_copy(*source, path, result.structural_type)?;
                (leaf, *source, field, path.clone())
            }
            OperationKind::StructuralCaseLeafCopy { source, path } => {
                // Membership and typed endpoint selection remain owned by the
                // case walker. Its canonical final field identifies the same
                // runtime cell namespace as an ordinary resolved projection.
                let (leaf, projection) = self.case_payload_projection(*source, path)?;
                let field = match path.last() {
                    Some(semantic_vocabulary::CanonicalStructuralPathSegment::Field(field)) => {
                        Some(*field)
                    }
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
                self.program.structural_types().values(),
                leaf.structural_type,
                occurrence_access,
                result.structural_type,
                result.multiplicity,
            )
        {
            // A record leaf copy out of a byte carrier mints fresh storage:
            // the resolved `u8` element is where the region starts and the
            // record's scalar leaves decode little-endian out of the
            // consecutive bytes the source recast judgment proved fit.
            return self.byte_carrier_leaf_copy(&leaf, result);
        }
        // Primitive array copies have one owned representation regardless of
        // their source home. A selected row/field snapshots the same checked
        // primitive storage as a call argument; publishing it as opaque placed
        // storage would strand its payload when a scalar-array return consumes it.
        // Resolve the live projection above even for empty arrays: their absence
        // of element reads cannot waive source access or active-case custody.
        // Qualified arrays retain their membership metadata in placed storage;
        // the detached primitive payload representation carries no domains.
        if result.is_unrestricted_plain()
            && terminal_semantics::scalar_array_leaf_shape(
                self.program.structural_types().values(),
                result.structural_type,
            )
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
            .program
            .structural_types()
            .get(&result.structural_type)
            .is_some_and(|declaration| {
                matches!(declaration.shape, StructuralTypeShape::Reference { .. })
            })
            && !self.reference_referents.contains_key(&source_place)
        {
            return Err(malformed());
        }
        // Aggregate copies retain their independently stored descriptor leaves.
        // Every copied runtime association must rejoin that exact declaration;
        // cloning storage cannot amplify an exclusive permission. Runtime paths
        // omit Case, so the stored discriminator selects the declaration rather
        // than treating a possible payload catalogue as resident storage.
        for carrier in self.reference_referents.keys().filter(|carrier| {
            carrier.opaque_identity == source_place.opaque_identity
                && carrier.path.starts_with(&source_place.path)
        }) {
            let reference_type = self.actual_storage_type(&leaf, carrier)?;
            if !self
                .program
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
            .program
            .structural_types()
            .get(&result.structural_type)
            .is_some_and(|declaration| {
                matches!(declaration.shape, StructuralTypeShape::ByteSequence(_))
            })
        {
            let root = self.structural_values.get(&source).ok_or_else(malformed)?;
            let bytes = if StructuralRuntimePlace::from(root) == StructuralRuntimePlace::from(&leaf)
                && root.structural_type == leaf.structural_type
            {
                let bytes = self
                    .byte_sequence_values
                    .get(&source)
                    .ok_or_else(malformed)?
                    .immutable()?;
                if !bytes.matches_descriptor_home(root) {
                    return Err(malformed());
                }
                bytes.clone()
            } else if let Some(bytes) = self
                .structural_byte_storage
                .get(&StructuralRuntimePlace::from(&leaf))
            {
                bytes.clone()
            } else {
                let mut parent = StructuralRuntimePlace::from(&leaf);
                parent.path.pop().ok_or_else(malformed)?;
                let namespace = match &operation.kind {
                    OperationKind::StructuralLeafCopy { path, .. } => {
                        StructuralFieldNamespace::for_carrier(&path[..path.len().saturating_sub(1)])
                    }
                    OperationKind::StructuralCaseLeafCopy { path, .. } => {
                        if matches!(
                            path.iter().rev().nth(1),
                            Some(semantic_vocabulary::CanonicalStructuralPathSegment::Case(_))
                        ) {
                            StructuralFieldNamespace::CasePayload
                        } else {
                            StructuralFieldNamespace::Member
                        }
                    }
                    _ => return Err(malformed()),
                };
                let cell = StructuralByteSequenceRuntimeField {
                    parent,
                    namespace,
                    field: endpoint_field.ok_or_else(malformed)?,
                };
                self.structural_byte_sequence_fields
                    .get(&cell)
                    .ok_or_else(malformed)?
                    .immutable()?
                    .clone()
            };
            // ByteSequenceView clones its backing handle and exact window.
            // This copies the descriptor, not octets; replacing the source
            // slot cannot retarget this completed value's retained backing.
            if self.byte_sequence_values.contains_key(&result.place) {
                return Err(malformed());
            }
            self.byte_sequence_values.insert(
                result.place,
                ByteSequenceBinding::Immutable(bytes.detached_descriptor()),
            );
        }
        if self
            .program
            .structural_types()
            .get(&result.structural_type)
            .is_some_and(|declaration| {
                matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
            })
        {
            let root = self.structural_values.get(&source).ok_or_else(malformed)?;
            let descriptor = if StructuralRuntimePlace::from(root)
                == StructuralRuntimePlace::from(&leaf)
                && root.structural_type == leaf.structural_type
            {
                let descriptor = self
                    .element_view_values
                    .get(&source)
                    .ok_or_else(malformed)?;
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
            self.element_view_values.insert(result.place, descriptor);
        }
        self.clone_subtree(
            &StructuralRuntimePlace::from(&leaf),
            &StructuralRuntimePlace::from(&copied),
        );
        self.structural_values.insert(result.place, copied);
        self.track_affine_result(result);
        Ok(OperationFlow::Advance)
    }

    /// Publish a detached primitive-array snapshot only after all source reads
    /// and exact type checks complete. This consumes no source storage or loans.
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
        self.scalar_array_values.insert(result.place, copied);
        Ok(OperationFlow::Advance)
    }
    /// Mint the copied record a byte-carrier leaf copy names: the resolved
    /// leaf is the `u8` element where the overlaid region starts, and each
    /// scalar leaf of the record's fixed layout decodes little-endian out of
    /// the consecutive bytes the recast judgment proved it occupies. The
    /// verifier admits this shape only for a literal index inside a `u8`
    /// fixed array with the layout's extent proven against the declared
    /// length, so a miss on any byte is a malformed module, never a partial
    /// record.
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
        let bytes = self
            .structural_byte_storage
            .get(&array)
            .ok_or_else(malformed)?;
        let layout = terminal_semantics::byte_carrier_scalar_layout(
            self.program.structural_types().values(),
            result.structural_type,
        )
        .ok_or_else(malformed)?;
        let index = usize::try_from(*index).map_err(|_| malformed())?;
        let mut leaves = Vec::with_capacity(layout.leaves.len());
        for member in &layout.leaves {
            let offset = index
                .checked_add(usize::try_from(member.offset).map_err(|_| malformed())?)
                .ok_or_else(malformed)?;
            let size = usize::try_from(match member.scalar_type {
                ScalarType::Boolean => 1u16,
                ScalarType::Integer(integer) => integer.bits() / 8,
                _ => return Err(malformed()),
            })
            .map_err(|_| malformed())?;
            let bytes = bytes
                .bytes()
                .get(offset..offset.checked_add(size).ok_or_else(malformed)?)
                .ok_or_else(malformed)?;
            let mut value = 0u128;
            for (position, byte) in bytes.iter().enumerate() {
                value |= u128::from(*byte) << (8 * position);
            }
            let scalar = match member.scalar_type {
                ScalarType::Boolean => crate::primitive_storage::encoding::boolean_bits(value)?,
                ScalarType::Integer(scalar_type) => {
                    let value = match scalar_type.sign() {
                        semantic_vocabulary::IntegerSign::Signed => {
                            let bits = u32::from(scalar_type.bits());
                            let signed = if bits < 128 && (value >> (bits - 1)) & 1 == 1 {
                                (value | (!0u128 << bits)) as i128
                            } else {
                                value as i128
                            };
                            IntegerValue::Signed(signed)
                        }
                        semantic_vocabulary::IntegerSign::Unsigned => IntegerValue::Unsigned(value),
                    };
                    TerminalScalarValue::Integer { scalar_type, value }
                }
                _ => return Err(malformed()),
            };
            leaves.push((member.field, scalar));
        }
        let copied = TerminalStructuralValue {
            opaque_identity: self.local_structural_identities.allocate()?,
            structural_type: result.structural_type,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            path: Vec::new(),
        };
        let parent = StructuralRuntimePlace::from(&copied);
        for (field, scalar) in leaves {
            self.structural_scalar_fields.insert(
                StructuralScalarRuntimeField::member(parent.clone(), field),
                scalar,
            );
        }
        self.structural_values.insert(result.place, copied);
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn execute_structural_case_membership(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::StructuralCaseMembership {
            source,
            ref path,
            case,
        } = operation.kind
        else {
            unreachable!("dispatched execute_structural_case_membership")
        };
        let active_case = self.observe_structural_case(source, path)?;
        self.store_scalar(operation, TerminalScalarValue::Boolean(active_case == case))
    }

    /// The runtime cell one observed scalar field names: a field of the
    /// record (or a common field of the mixed shape) a record-only path
    /// reaches, or a payload field of the case a final `Case` step qualifies.
    /// A payload lives at its sum's own place, overlaid by case, so the
    /// stored discriminator must name the observed case. Verification proves
    /// that case active before the read; a mismatch here is a malformed
    /// module, never a read of another case's overlay.
    pub(crate) fn observed_field_cell(
        &self,
        source: semantic_vocabulary::PlaceId,
        path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
        field: semantic_vocabulary::StructuralFieldId,
    ) -> Result<StructuralScalarRuntimeField, TerminalInterpretError> {
        let (structural_value, view) = self.observation_view_root(source)?;
        self.observed_field_cell_in(&structural_value, view, path, field)
    }

    /// Select a field in already resolved backing. Reference traversal owns
    /// the loan and pointer crossings; this join retains the live case and
    /// declaration-local field identity used by ordinary scalar observations.
    pub(crate) fn observed_field_cell_in(
        &self,
        structural_value: &crate::values::TerminalStructuralValue,
        view: Option<crate::reference::AggregateViewTip>,
        path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
        field: semantic_vocabulary::StructuralFieldId,
    ) -> Result<StructuralScalarRuntimeField, TerminalInterpretError> {
        // An observation through an aggregate re-view names the view's
        // fields; the complete captured region selects the original cells.
        let translated;
        let (path, field) = match view {
            None => (path, field),
            Some(crate::reference::AggregateViewTip::Aggregate { view, backing })
                if backing == structural_value.structural_type =>
            {
                let (backing_path, backing_field) =
                    crate::reference::translate_aggregate_view_field(
                        &self.program.structural_types(),
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
            self.program.structural_types().values(),
            structural_value.structural_type,
            path,
        ) {
            parent.path.extend(carrier.path);
            let contents = self.structural_cases.get(&parent).ok_or(malformed())?;
            if contents.structural_type != carrier.sum || contents.case != carrier.case {
                return Err(malformed());
            }
            return Ok(StructuralScalarRuntimeField::case_payload(parent, field));
        }
        let carrier = terminal_semantics::record_field_carrier(
            self.program.structural_types().values(),
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
        let OperationKind::BooleanStructuralField {
            source,
            ref path,
            field,
        } = operation.kind
        else {
            unreachable!("dispatched execute_boolean_structural_field")
        };
        if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
            return Err(malformed());
        }
        let value = self.read_observed_field(source, path, field, ScalarType::Boolean)?;
        self.store_scalar(operation, value)
    }

    pub(crate) fn execute_scalar_structural_field(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::ScalarStructuralField {
            source,
            ref path,
            field,
        } = operation.kind
        else {
            unreachable!("dispatched execute_scalar_structural_field")
        };
        let scalar_type = operation.result.expect_scalar().scalar_type;
        let value = self.read_observed_field(source, path, field, scalar_type)?;
        self.store_scalar(operation, value)
    }

    /// Field operations observe the same retained storage as primitive paths.
    /// A borrowed byte region has an exact codec instead of a typed field cell;
    /// forwarding its opaque identity must preserve that distinction.
    fn read_observed_field(
        &self,
        source: semantic_vocabulary::PlaceId,
        path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
        field: semantic_vocabulary::StructuralFieldId,
        scalar_type: ScalarType,
    ) -> Result<TerminalScalarValue, TerminalInterpretError> {
        let (structural_value, view) = self.observation_view_root(source)?;
        let declared_root = match view {
            Some(crate::reference::AggregateViewTip::Aggregate { view, .. }) => {
                view.structural_type
            }
            Some(crate::reference::AggregateViewTip::Scalar { .. }) => return Err(malformed()),
            None => structural_value.structural_type,
        };
        let declaration = terminal_semantics::observed_field_declaration(
            self.program.structural_types().values(),
            declared_root,
            path,
            field,
        )
        .ok_or_else(malformed)?;
        if declaration.field_type.scalar_type() != Some(scalar_type) {
            return Err(malformed());
        }
        if let Some(view) = view {
            let carrier = terminal_semantics::record_field_carrier(
                self.program.structural_types().values(),
                declared_root,
                path,
            )
            .ok_or_else(malformed)?;
            let mut logical = carrier.path;
            logical.push(StructuralPathSegment::Field(declaration.identity));
            let (storage, logical_scalar, shared_view) =
                self.aggregate_view_primitive_access(&structural_value, view, &logical)?;
            return self.read_primitive_storage(
                source,
                storage,
                logical_scalar,
                scalar_type,
                shared_view,
            );
        }
        let cell = self.observed_field_cell(source, path, field)?;
        // A typed aggregate re-view translates to its backing's cells below.
        // Byte-backed views already carry their observed declaration, so the
        // full field path selects the retained region's exact physical codec.
        if view.is_none() {
            let mut leaf = structural_value.clone();
            leaf.path = cell.parent.path.clone();
            leaf.path
                .push(StructuralPathSegment::Field(declaration.identity.clone()));
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
        let value = self.structural_scalar_fields.get(&cell).copied().ok_or(
            if scalar_type == ScalarType::Boolean {
                TerminalInterpretError::StructuralBooleanFieldMissing { source, field }
            } else {
                TerminalInterpretError::StructuralScalarFieldMissing { source, field }
            },
        )?;
        if value.scalar_type() != scalar_type {
            return Err(malformed());
        }
        Ok(value)
    }

    pub(crate) fn execute_byte_sequence_read(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::ByteSequenceRead {
            source,
            index,
            length,
            ..
        } = operation.kind
        else {
            unreachable!("dispatched execute_byte_sequence_read")
        };
        let result = operation.result.expect_scalar();
        let byte_type =
            IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8).expect("u8 is valid");
        let count_type =
            IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64).expect("u64 is valid");
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
        );
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
        let integer_type =
            IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64).expect("u64 is valid");
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
            );
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
        );
        Ok(OperationFlow::Advance)
    }
}
