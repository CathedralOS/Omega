//! Structural operations of the interpreter loop: descriptor stores,
//! scalar cases, byte-sequence literals and reads, trivial affine
//! locals, records, field stores and field reads.

use crate::byte_sequences::ByteSequenceView;
use crate::byte_sequences::binding::ByteSequenceBinding;
use crate::custody::{direct_scalar_field_type, remove_affine_root, resolve_structural_arguments};
use crate::errors::TerminalInterpretError;
use crate::execution::{OperationFlow, TerminalExecution};
use crate::scalar_operations::terminal_scalar_belongs_to_type;
use crate::structural_inputs::case_membership::StructuralCaseContents;
use crate::values::{ScalarFieldNamespace, StructuralScalarRuntimeField};
use crate::values::{StructuralByteSequenceRuntimeField, StructuralRuntimePlace};
use crate::values::{TerminalScalarCaseValue, TerminalScalarValue, TerminalStructuralValue};
use semantic_vocabulary::{IntegerType, IntegerValue, ScalarType, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    OperationKind, StructuralAccess, StructuralAffineDiscard, StructuralArgument,
    StructuralFieldType, StructuralMultiplicity, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};

impl TerminalExecution {
    /// Cases built from scalars and plain records retain their whole payload
    /// under one result identity. Calls may return that storage, and an
    /// unrestricted occurrence needs no affine cleanup at frame exit. Keep
    /// those two decisions on the same supported payload contract.
    pub(crate) fn plain_case_type(&self, structural_type: StructuralTypeId) -> bool {
        let Some(StructuralTypeShape::Sum { cases }) = self
            .program
            .structural_types
            .get(&structural_type)
            .map(|declaration| &declaration.shape)
        else {
            return false;
        };
        cases.iter().all(|case| {
            case.fields.iter().all(|field| {
                !field.relevance.is_erased()
                    && match field.field_type {
                        StructuralFieldType::Structural(child) => self.plain_record_type(child),
                        _ => field.field_type.scalar_type().is_some(),
                    }
            })
        })
    }

    pub(crate) fn execute_store_dynamic_descriptor(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::StoreDynamicDescriptor { descriptor_ordinal } = operation.kind else {
            unreachable!("dispatched execute_store_dynamic_descriptor")
        };
        if operation.result != terminal_psi::OperationResult::Unit
            || !self
                .program
                .dynamic_descriptor_templates
                .contains_key(&(self.current_machine, descriptor_ordinal))
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        Ok(OperationFlow::Advance)
    }

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
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        if self.structural_values.contains_key(&result.place)
            || self.scalar_case_values.contains_key(&result.place)
            || !matches!(
                result.multiplicity,
                StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
            )
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let Some(StructuralTypeDeclaration {
            shape: StructuralTypeShape::Sum { cases },
            ..
        }) = self.program.structural_types.get(&result.structural_type)
        else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        let Some(selected) = cases.iter().find(|case| case.id == result_case) else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        if selected.fields.len() != fields.len() {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let mut payload = Vec::with_capacity(fields.len());
        for (declaration, binding) in selected.fields.iter().zip(fields) {
            let value = self
                .values
                .get(&binding.value)
                .copied()
                .ok_or(TerminalInterpretError::VerifiedValueMissing(binding.value))?;
            if declaration.id != binding.field
                || declaration.field_type.scalar_type() != Some(value.scalar_type())
            {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
            if !value.satisfies_field(&declaration.field_type) {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
            payload.push((binding.field, value));
        }
        self.scalar_case_values.insert(
            result.place,
            TerminalScalarCaseValue {
                structural_type: result.structural_type,
                result_case,
                fields: payload,
            },
        );
        if result.multiplicity == StructuralMultiplicity::Affine {
            self.live_affine_frontier.insert(StructuralAffineDiscard {
                place: result.place,
                path: Vec::new(),
                structural_type: result.structural_type,
            });
        }
        Ok(OperationFlow::Advance)
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
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
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
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        if self.structural_values.contains_key(&result.place)
            || self.scalar_case_values.contains_key(&result.place)
            || !matches!(
                result.multiplicity,
                StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
            )
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
        {
            return Err(invalid());
        }
        let (common, cases) = match self
            .program
            .structural_types
            .get(&result.structural_type)
            .map(|declaration| &declaration.shape)
        {
            Some(StructuralTypeShape::Sum { cases }) => (&[][..], cases),
            Some(StructuralTypeShape::Mixed { fields, cases }) => (fields.as_slice(), cases),
            _ => return Err(invalid()),
        };
        let Some(selected) = cases.iter().find(|case| case.id == result_case) else {
            return Err(invalid());
        };
        let roster = common
            .iter()
            .filter(|field| !matches!(field.field_type, StructuralFieldType::Erased { .. }))
            .map(|declaration| (declaration, ScalarFieldNamespace::Member))
            .chain(
                selected
                    .fields
                    .iter()
                    .filter(|field| {
                        !(field.relevance.is_erased()
                            && matches!(field.field_type, StructuralFieldType::Erased { .. }))
                    })
                    .map(|declaration| (declaration, ScalarFieldNamespace::CasePayload)),
            )
            .collect::<Vec<_>>();
        if roster.len() != fields.len() {
            return Err(invalid());
        }
        let value = TerminalStructuralValue {
            opaque_identity: self.local_structural_identities.allocate()?,
            structural_type: result.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        };
        let member_root = StructuralRuntimePlace::from(&value);
        let mut staged_scalars: BTreeMap<StructuralScalarRuntimeField, TerminalScalarValue> =
            BTreeMap::new();
        let mut staged_cases: BTreeMap<StructuralRuntimePlace, StructuralCaseContents> =
            BTreeMap::new();
        let mut staged_byte_fields: BTreeMap<StructuralByteSequenceRuntimeField, ByteSequenceView> =
            BTreeMap::new();
        let mut staged_byte_arrays: BTreeMap<StructuralRuntimePlace, ByteSequenceView> =
            BTreeMap::new();
        let mut staged_primitives: BTreeMap<StructuralRuntimePlace, TerminalScalarValue> =
            BTreeMap::new();
        let mut staged_referents: BTreeMap<StructuralRuntimePlace, TerminalStructuralValue> =
            BTreeMap::new();
        let mut staged_element_views = BTreeMap::new();
        let mut consumed = Vec::new();
        let mut frontier = self.live_affine_frontier.clone();
        for ((declaration, namespace), binding) in roster.into_iter().zip(fields) {
            if binding.field != declaration.id || declaration.relevance.is_erased() {
                return Err(invalid());
            }
            match &binding.value {
                terminal_psi::RecordFieldValue::Scalar { value: operand, .. } => {
                    let scalar = self.values.get(operand).copied().ok_or_else(invalid)?;
                    if declaration.field_type.scalar_type() != Some(scalar.scalar_type())
                        || !terminal_scalar_belongs_to_type(scalar)
                    {
                        return Err(invalid());
                    }
                    if !scalar.satisfies_field(&declaration.field_type) {
                        return Err(invalid());
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
                    // Verification refuses a structural common field, so
                    // only a payload member nests a child subtree.
                    if namespace != ScalarFieldNamespace::CasePayload
                        || argument.access != StructuralAccess::Owned
                    {
                        return Err(invalid());
                    }
                    let member = StructuralRuntimePlace {
                        opaque_identity: value.opaque_identity,
                        path: vec![StructuralPathSegment::Field(declaration.identity.clone())],
                    };
                    let affine =
                        self.stage_constructor_operand(argument, &mut frontier, &mut consumed)?;
                    if affine && result.multiplicity == StructuralMultiplicity::Unrestricted {
                        return Err(invalid());
                    }
                    if argument.path.is_empty()
                        && let Some(case) = self.scalar_case_values.get(&argument.place)
                    {
                        if declaration.field_type
                            != StructuralFieldType::Structural(case.structural_type)
                        {
                            return Err(invalid());
                        }
                        staged_cases.insert(
                            member.clone(),
                            StructuralCaseContents {
                                structural_type: case.structural_type,
                                case: case.result_case,
                            },
                        );
                        for (field, scalar) in &case.fields {
                            staged_scalars.insert(
                                StructuralScalarRuntimeField::case_payload(member.clone(), *field),
                                *scalar,
                            );
                        }
                        continue;
                    }
                    let child = resolve_structural_arguments(
                        &self.program.structural_types,
                        &self.structural_values,
                        std::slice::from_ref(argument),
                    )?
                    .pop()
                    .ok_or_else(invalid)?;
                    if !child.qualifications.is_empty()
                        || declaration.field_type
                            != StructuralFieldType::Structural(child.structural_type)
                    {
                        return Err(invalid());
                    }
                    // Retarget every live subtree cell into member coordinates;
                    // affine children additionally vacate their source region
                    // once the whole roster validates.
                    let retarget = |parent: &StructuralRuntimePlace| {
                        (parent.opaque_identity == child.opaque_identity
                            && parent.path.starts_with(&child.path))
                        .then(|| StructuralRuntimePlace {
                            opaque_identity: member.opaque_identity,
                            path: member
                                .path
                                .iter()
                                .chain(&parent.path[child.path.len()..])
                                .cloned()
                                .collect(),
                        })
                    };
                    if self
                        .program
                        .structural_types
                        .get(&child.structural_type)
                        .is_some_and(|row| {
                            matches!(row.shape, StructuralTypeShape::ElementView { .. })
                        })
                    {
                        let descriptor = self.element_view_argument(argument, &child)?;
                        staged_element_views.insert(member.clone(), descriptor.clone());
                    }
                    for (carrier, descriptor) in &self.structural_element_view_fields {
                        if let Some(place) = retarget(carrier) {
                            staged_element_views.insert(place, descriptor.clone());
                        }
                    }
                    for (carrier, referent) in &self.reference_referents {
                        if let Some(place) = retarget(carrier) {
                            if !affine {
                                return Err(invalid());
                            }
                            staged_referents.insert(place, referent.clone());
                        }
                    }
                    for (cell, scalar) in &self.structural_scalar_fields {
                        if let Some(parent) = retarget(&cell.parent) {
                            staged_scalars.insert(cell.relocated(parent), *scalar);
                        }
                    }
                    for (carrier, contents) in &self.structural_cases {
                        if let Some(place) = retarget(carrier) {
                            staged_cases.insert(place, *contents);
                        }
                    }
                    for (cell, view) in &self.structural_byte_sequence_fields {
                        if let Some(parent) = retarget(&cell.parent) {
                            staged_byte_fields.insert(
                                StructuralByteSequenceRuntimeField {
                                    parent,
                                    field: cell.field,
                                },
                                view.clone(),
                            );
                        }
                    }
                    for (carrier, view) in &self.structural_byte_arrays {
                        if let Some(place) = retarget(carrier) {
                            staged_byte_arrays.insert(place, view.clone());
                        }
                    }
                    for (carrier, scalar) in &self.structural_primitive_storage {
                        if let Some(place) = retarget(carrier) {
                            staged_primitives.insert(place, *scalar);
                        }
                    }
                }
            }
        }
        self.commit_constructor_operands(frontier, &consumed)?;
        self.structural_scalar_fields.extend(staged_scalars);
        self.structural_cases.extend(staged_cases);
        self.structural_byte_sequence_fields
            .extend(staged_byte_fields);
        self.structural_byte_arrays.extend(staged_byte_arrays);
        self.structural_primitive_storage.extend(staged_primitives);
        self.reference_referents.extend(staged_referents);
        self.structural_element_view_fields
            .extend(staged_element_views);
        self.structural_cases.insert(
            member_root,
            StructuralCaseContents {
                structural_type: result.structural_type,
                case: result_case,
            },
        );
        self.structural_values.insert(result.place, value);
        if result.multiplicity == StructuralMultiplicity::Affine {
            self.live_affine_frontier.insert(StructuralAffineDiscard {
                place: result.place,
                path: Vec::new(),
                structural_type: result.structural_type,
            });
        }
        Ok(())
    }

    /// Remove every runtime cell under `from`; paired with subtree copies so
    /// consumed children vacate their source region atomically.
    pub(crate) fn prune_subtree(&mut self, from: &StructuralRuntimePlace) {
        self.structural_scalar_fields.retain(|cell, _| {
            cell.parent.opaque_identity != from.opaque_identity
                || !cell.parent.path.starts_with(&from.path)
        });
        self.structural_byte_sequence_fields.retain(|cell, _| {
            cell.parent.opaque_identity != from.opaque_identity
                || !cell.parent.path.starts_with(&from.path)
        });
        self.structural_byte_arrays.retain(|carrier, _| {
            carrier.opaque_identity != from.opaque_identity || !carrier.path.starts_with(&from.path)
        });
        self.structural_cases.retain(|carrier, _| {
            carrier.opaque_identity != from.opaque_identity || !carrier.path.starts_with(&from.path)
        });
        self.reference_referents.retain(|carrier, _| {
            carrier.opaque_identity != from.opaque_identity || !carrier.path.starts_with(&from.path)
        });
        self.structural_element_view_fields.retain(|carrier, _| {
            carrier.opaque_identity != from.opaque_identity || !carrier.path.starts_with(&from.path)
        });
        self.structural_primitive_storage.retain(|carrier, _| {
            carrier.opaque_identity != from.opaque_identity || !carrier.path.starts_with(&from.path)
        });
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
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let machine = self.program.machines.get(&self.current_machine).ok_or(
            TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
        )?;
        let Some(terminal_psi::StructuralPlaceDeclaration {
            kind:
                semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                    structural_type, ..
                },
            ..
        }) = machine
            .structural_places
            .iter()
            .find(|place| place.id == destination)
        else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
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
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
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
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
            self.structural_values.insert(
                destination,
                TerminalStructuralValue {
                    opaque_identity: destination.get(),
                    structural_type: *structural_type,
                    qualifications: Vec::new(),
                    path: Vec::new(),
                },
            );
        }
        Ok(OperationFlow::Advance)
    }

    /// Materialize a zeroed establishment: scalar leaves receive zero and
    /// bounded byte fields receive empty live backing under the fresh identity.
    pub(crate) fn execute_establish_zeroed_structural(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let OperationKind::EstablishZeroedStructural {} = operation.kind else {
            unreachable!("dispatched execute_establish_zeroed_structural")
        };
        let result = operation.result.structural().ok_or_else(invalid)?;
        if !matches!(
            result.multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        ) || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
            || self.structural_values.contains_key(&result.place)
            || self.scalar_case_values.contains_key(&result.place)
            || self.scalar_array_values.contains_key(&result.place)
            || self
                .live_claims
                .values()
                .any(|claim| claim.place == Some(result.place))
        {
            return Err(invalid());
        }
        let leaves = terminal_semantics::zeroed_structural_leaves(
            self.program.structural_types.values(),
            result.structural_type,
        )
        .ok_or(TerminalInterpretError::UnsupportedSemanticVariant(
            "zeroed structural storage exceeds supported leaf materialization",
        ))?;
        let value = TerminalStructuralValue {
            opaque_identity: self.local_structural_identities.allocate()?,
            structural_type: result.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        };
        let zero = |scalar_type: ScalarType| -> Option<TerminalScalarValue> {
            match scalar_type {
                ScalarType::Boolean => Some(TerminalScalarValue::Boolean(false)),
                ScalarType::Integer(integer_type) => Some(TerminalScalarValue::Integer {
                    scalar_type: integer_type,
                    value: match integer_type.sign() {
                        semantic_vocabulary::IntegerSign::Signed => IntegerValue::Signed(0),
                        semantic_vocabulary::IntegerSign::Unsigned => IntegerValue::Unsigned(0),
                    },
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
            match leaf {
                terminal_semantics::ZeroedStructuralLeaf::RecordField {
                    path,
                    field,
                    scalar_type,
                } => {
                    let parent = StructuralRuntimePlace {
                        opaque_identity: value.opaque_identity,
                        path: path.clone(),
                    };
                    let cell = StructuralScalarRuntimeField::member(parent, *field);
                    self.structural_scalar_fields
                        .insert(cell, zero(*scalar_type).ok_or_else(invalid)?);
                }
                terminal_semantics::ZeroedStructuralLeaf::ByteSequenceField {
                    path, field, ..
                } => {
                    self.structural_byte_sequence_fields.insert(
                        StructuralByteSequenceRuntimeField {
                            parent: StructuralRuntimePlace {
                                opaque_identity: value.opaque_identity,
                                path: path.clone(),
                            },
                            field: *field,
                        },
                        ByteSequenceView::new(Vec::new()),
                    );
                }
                terminal_semantics::ZeroedStructuralLeaf::CaseField {
                    path,
                    field,
                    scalar_type,
                    ..
                } => {
                    let parent = StructuralRuntimePlace {
                        opaque_identity: value.opaque_identity,
                        path: path.clone(),
                    };
                    self.structural_scalar_fields.insert(
                        StructuralScalarRuntimeField::case_payload(parent, *field),
                        zero(*scalar_type).ok_or_else(invalid)?,
                    );
                }
                terminal_semantics::ZeroedStructuralLeaf::ArrayScalar { path, scalar_type } => {
                    // Byte views and scalar indexing must share one backing.
                    // Storing these bytes as separate scalar cells would make
                    // writes through a borrowed byte view invisible to the owner.
                    if let Some((StructuralPathSegment::FixedIndex(_), parent)) = path.split_last()
                        && let Some(array_type) =
                            crate::structural_inputs::byte_arrays::array_path_type(
                                &self.program.structural_types,
                                result.structural_type,
                                parent,
                            )
                        && let Some(length) =
                            crate::structural_inputs::byte_arrays::byte_array_length(
                                &self.program.structural_types,
                                array_type,
                            )
                    {
                        let length = usize::try_from(length).map_err(|_| invalid())?;
                        self.structural_byte_arrays
                            .entry(StructuralRuntimePlace {
                                opaque_identity: value.opaque_identity,
                                path: parent.to_vec(),
                            })
                            .or_insert_with(|| ByteSequenceView::new(vec![0; length]));
                        continue;
                    }
                    self.structural_primitive_storage.insert(
                        StructuralRuntimePlace {
                            opaque_identity: value.opaque_identity,
                            path: path.clone(),
                        },
                        zero(*scalar_type).ok_or_else(invalid)?,
                    );
                }
                terminal_semantics::ZeroedStructuralLeaf::SumSelection {
                    path,
                    structural_type,
                    case,
                    ..
                } => {
                    // The tag is not independently addressable storage — the
                    // structural_cases entry IS the runtime discriminator.
                    self.structural_cases.insert(
                        StructuralRuntimePlace {
                            opaque_identity: value.opaque_identity,
                            path: path.clone(),
                        },
                        StructuralCaseContents {
                            structural_type: *structural_type,
                            case: *case,
                        },
                    );
                }
            }
        }
        self.structural_values.insert(result.place, value);
        // Zeroing establishes payload; it does not discharge the nominal
        // owner's affine debt. The verified continuation still disposes it.
        if result.multiplicity == StructuralMultiplicity::Affine {
            self.live_affine_frontier.insert(StructuralAffineDiscard {
                place: result.place,
                path: Vec::new(),
                structural_type: result.structural_type,
            });
        }
        Ok(OperationFlow::Advance)
    }

    /// Establish a declared structural local's home: the place materializes
    /// as the operation's structural result with no leaf storage — member
    /// operations write each field's backing as they execute.
    pub(crate) fn execute_establish_structural_local(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let OperationKind::EstablishStructuralLocal {} = operation.kind else {
            unreachable!("dispatched execute_establish_structural_local")
        };
        let result = operation.result.structural().ok_or_else(invalid)?;
        if !matches!(
            result.multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        ) || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
            || self.structural_values.contains_key(&result.place)
            || self.scalar_case_values.contains_key(&result.place)
            || self.scalar_array_values.contains_key(&result.place)
            || self
                .live_claims
                .values()
                .any(|claim| claim.place == Some(result.place))
        {
            return Err(invalid());
        }
        self.structural_values.insert(
            result.place,
            TerminalStructuralValue {
                opaque_identity: self.local_structural_identities.allocate()?,
                structural_type: result.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            },
        );
        if result.multiplicity == StructuralMultiplicity::Affine {
            self.live_affine_frontier.insert(StructuralAffineDiscard {
                place: result.place,
                path: Vec::new(),
                structural_type: result.structural_type,
            });
        }
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
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let machine = self.program.machines.get(&self.current_machine).ok_or(
            TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
        )?;
        let Some(terminal_psi::StructuralPlaceDeclaration {
            kind:
                semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                    structural_type, ..
                },
            ..
        }) = machine
            .structural_places
            .iter()
            .find(|place| place.id == destination)
        else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        self.structural_values.insert(
            destination,
            TerminalStructuralValue {
                opaque_identity: destination.get(),
                structural_type: *structural_type,
                qualifications: Vec::new(),
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
        self.establish_record(
            operation
                .result
                .structural()
                .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?,
            fields,
        )?;
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
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let machine = self.program.machines.get(&self.current_machine).ok_or(
            TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
        )?;
        let access = machine
            .structural_parameters
            .iter()
            .chain(
                machine
                    .blocks
                    .values()
                    .flat_map(|block| &block.structural_parameters),
            )
            .find(|parameter| parameter.place == destination)
            .filter(|parameter| {
                matches!(
                    parameter.access,
                    StructuralAccess::Owned
                        | StructuralAccess::MutableBorrow
                        | StructuralAccess::WriteOnlyBorrow
                ) && matches!(
                    parameter.multiplicity,
                    StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
                ) && parameter.qualifications.is_empty()
                    && parameter.projected_qualifications.is_empty()
            })
            .map(|parameter| parameter.access)
            .or_else(|| {
                let declaration = machine
                    .structural_places
                    .iter()
                    .find(|declaration| declaration.id == destination)?;
                let semantic_vocabulary::StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                } = declaration.kind
                else {
                    return None;
                };
                let result = machine
                    .blocks
                    .values()
                    .flat_map(|block| &block.operations)
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
                                && value.qualifications.is_empty()
                        })
                    && matches!(
                        result.multiplicity,
                        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
                    )
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.claims.is_empty())
                .then_some(StructuralAccess::Owned)
            })
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        let source = self
            .values
            .get(&value)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(value))?;
        let parent = resolve_structural_arguments(
            &self.program.structural_types,
            &self.structural_values,
            &[StructuralArgument {
                place: destination,
                path: path.clone(),
                access,
            }],
        )?
        .pop()
        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        if direct_scalar_field_type(
            &self.program.structural_types,
            parent.structural_type,
            field,
        ) != Some(source.scalar_type())
            || !terminal_scalar_belongs_to_type(source)
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        // The store's path names no `Case` step: it writes a record field or a
        // mixed shape's common field, never a payload cell.
        self.structural_scalar_fields.insert(
            StructuralScalarRuntimeField::member(StructuralRuntimePlace::from(&parent), field),
            source,
        );
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
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let result = operation.result.structural().ok_or_else(invalid)?;
        if !matches!(
            result.multiplicity,
            StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
        ) || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
            || self.structural_values.contains_key(&result.place)
        {
            return Err(invalid());
        }
        let (hole, field_type) = self.structural_window_hole(source, path, field)?;
        if field_type != result.structural_type {
            return Err(invalid());
        }
        let moved = TerminalStructuralValue {
            opaque_identity: self.local_structural_identities.allocate()?,
            structural_type: result.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        };
        self.relocate_subtree(&hole, &StructuralRuntimePlace::from(&moved));
        self.structural_values.insert(result.place, moved);
        if result.multiplicity == StructuralMultiplicity::Affine {
            self.live_affine_frontier.insert(StructuralAffineDiscard {
                place: result.place,
                path: Vec::new(),
                structural_type: result.structural_type,
            });
        }
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
            destination,
            ref path,
            field,
            ref value,
        } = operation.kind
        else {
            unreachable!("dispatched execute_store_structural_field")
        };
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        if !matches!(operation.result, terminal_psi::OperationResult::Unit)
            || value.access != StructuralAccess::Owned
            || !value.path.is_empty()
        {
            return Err(invalid());
        }
        let (hole, field_type) = self.structural_window_hole(destination, path, field)?;
        let affine = self
            .live_affine_frontier
            .iter()
            .any(|entry| entry.place == value.place && entry.path.is_empty());
        // An established scalar case carries its tag and payload as one value
        // rather than as runtime cells; it lands in the hole exactly as an
        // `EstablishRecord` member of the same case type would.
        if let Some(case) = self.scalar_case_values.get(&value.place).cloned() {
            if case.structural_type != field_type {
                return Err(invalid());
            }
            self.structural_cases.insert(
                hole.clone(),
                StructuralCaseContents {
                    structural_type: case.structural_type,
                    case: case.result_case,
                },
            );
            for (payload_field, scalar) in case.fields {
                self.structural_scalar_fields.insert(
                    StructuralScalarRuntimeField::case_payload(hole.clone(), payload_field),
                    scalar,
                );
            }
            self.scalar_case_values.remove(&value.place);
            if affine && !remove_affine_root(&mut self.live_affine_frontier, value.place) {
                return Err(invalid());
            }
            return Ok(OperationFlow::Advance);
        }
        let child = self.structural_values.get(&value.place).cloned().ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(value.place),
        )?;
        if child.structural_type != field_type || !child.qualifications.is_empty() {
            return Err(invalid());
        }
        self.relocate_subtree(&StructuralRuntimePlace::from(&child), &hole);
        self.structural_values.remove(&value.place);
        if affine && !remove_affine_root(&mut self.live_affine_frontier, value.place) {
            return Err(invalid());
        }
        Ok(OperationFlow::Advance)
    }

    /// The resolved runtime hole path one window operation names and the
    /// hole's declared type. A root window (no field, empty path) is the
    /// loan's whole referent: the resolved root place itself. A field leaf
    /// resolves `path` to the containing record, then the hole sits one
    /// `Field` segment deeper under the field's declared identity. Without
    /// a field, the path already names the hole, including an array element.
    fn structural_window_hole(
        &self,
        root: semantic_vocabulary::PlaceId,
        path: &[StructuralPathSegment],
        field: Option<semantic_vocabulary::StructuralFieldId>,
    ) -> Result<(StructuralRuntimePlace, StructuralTypeId), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let storage = self.structural_values.get(&root).ok_or_else(invalid)?;
        // Resolve a reference crossing against its live runtime referent first;
        // the shared window geometry receives storage, never carrier ownership.
        let prefix_length = usize::from(matches!(
            path.first(),
            Some(StructuralPathSegment::Referent)
        ));
        let base = if prefix_length == 0 {
            storage.clone()
        } else {
            self.resolve_reference_call_arguments(&[StructuralArgument {
                place: root,
                path: path[..prefix_length].to_vec(),
                access: StructuralAccess::MutableBorrow,
            }])?
            .pop()
            .ok_or_else(invalid)?
        };
        let projection = terminal_semantics::structural_window_projection(
            self.program.structural_types.values(),
            base.structural_type,
            &path[prefix_length..],
            field,
        )
        .ok_or_else(invalid)?;
        let hole = crate::custody::project_structural_value(
            &self.program.structural_types,
            &base,
            &projection.path,
        )?;
        Ok((
            StructuralRuntimePlace::from(&hole),
            projection.structural_type,
        ))
    }

    /// Move every runtime cell under `from` to the same subtree suffix under
    /// `to`. Cells are staged before removal so the source region is never
    /// partially vacated.
    fn relocate_subtree(&mut self, from: &StructuralRuntimePlace, to: &StructuralRuntimePlace) {
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
        let staged_scalars: Vec<_> = self
            .structural_scalar_fields
            .iter()
            .filter_map(|(cell, scalar)| {
                retarget(&cell.parent).map(|parent| (cell.relocated(parent), *scalar))
            })
            .collect();
        let staged_byte_fields: Vec<_> = self
            .structural_byte_sequence_fields
            .iter()
            .filter_map(|(cell, view)| {
                retarget(&cell.parent).map(|parent| {
                    (
                        crate::values::StructuralByteSequenceRuntimeField {
                            parent,
                            field: cell.field,
                        },
                        view.clone(),
                    )
                })
            })
            .collect();
        let staged_arrays: Vec<_> = self
            .structural_byte_arrays
            .iter()
            .filter_map(|(carrier, view)| retarget(carrier).map(|place| (place, view.clone())))
            .collect();
        let staged_cases: Vec<_> = self
            .structural_cases
            .iter()
            .filter_map(|(carrier, contents)| retarget(carrier).map(|place| (place, *contents)))
            .collect();
        let staged_element_views: Vec<_> = self
            .structural_element_view_fields
            .iter()
            .filter_map(|(carrier, descriptor)| {
                retarget(carrier).map(|place| (place, descriptor.clone()))
            })
            .collect();
        let staged_referents: Vec<_> = self
            .reference_referents
            .iter()
            .filter_map(|(carrier, referent)| {
                retarget(carrier).map(|place| (place, referent.clone()))
            })
            .collect();
        let staged_primitives: Vec<_> = self
            .structural_primitive_storage
            .iter()
            .filter_map(|(carrier, scalar)| retarget(carrier).map(|place| (place, *scalar)))
            .collect();
        self.structural_scalar_fields.retain(|cell, _| {
            cell.parent.opaque_identity != from.opaque_identity
                || !cell.parent.path.starts_with(&from.path)
        });
        self.structural_byte_sequence_fields.retain(|cell, _| {
            cell.parent.opaque_identity != from.opaque_identity
                || !cell.parent.path.starts_with(&from.path)
        });
        self.structural_byte_arrays.retain(|carrier, _| {
            carrier.opaque_identity != from.opaque_identity || !carrier.path.starts_with(&from.path)
        });
        self.structural_cases.retain(|carrier, _| {
            carrier.opaque_identity != from.opaque_identity || !carrier.path.starts_with(&from.path)
        });
        self.reference_referents.retain(|carrier, _| {
            carrier.opaque_identity != from.opaque_identity || !carrier.path.starts_with(&from.path)
        });
        self.structural_element_view_fields.retain(|carrier, _| {
            carrier.opaque_identity != from.opaque_identity || !carrier.path.starts_with(&from.path)
        });
        self.structural_primitive_storage.retain(|carrier, _| {
            carrier.opaque_identity != from.opaque_identity || !carrier.path.starts_with(&from.path)
        });
        self.structural_scalar_fields.extend(staged_scalars);
        self.structural_byte_sequence_fields
            .extend(staged_byte_fields);
        self.structural_byte_arrays.extend(staged_arrays);
        self.structural_cases.extend(staged_cases);
        self.reference_referents.extend(staged_referents);
        self.structural_element_view_fields
            .extend(staged_element_views);
        self.structural_primitive_storage.extend(staged_primitives);
    }

    /// Copy every runtime cell under `from` to the same subtree suffix under
    /// `to`, leaving `from` intact. The `Unrestricted` leaf's cells duplicate
    /// beneath the result's own opaque identity; nothing vacates.
    fn clone_subtree(&mut self, from: &StructuralRuntimePlace, to: &StructuralRuntimePlace) {
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
        let staged_scalars: Vec<_> = self
            .structural_scalar_fields
            .iter()
            .filter_map(|(cell, scalar)| {
                retarget(&cell.parent).map(|parent| (cell.relocated(parent), *scalar))
            })
            .collect();
        let staged_byte_fields: Vec<_> = self
            .structural_byte_sequence_fields
            .iter()
            .filter_map(|(cell, view)| {
                retarget(&cell.parent).map(|parent| {
                    (
                        crate::values::StructuralByteSequenceRuntimeField {
                            parent,
                            field: cell.field,
                        },
                        view.clone(),
                    )
                })
            })
            .collect();
        let staged_arrays: Vec<_> = self
            .structural_byte_arrays
            .iter()
            .filter_map(|(carrier, view)| retarget(carrier).map(|place| (place, view.clone())))
            .collect();
        let staged_cases: Vec<_> = self
            .structural_cases
            .iter()
            .filter_map(|(carrier, contents)| retarget(carrier).map(|place| (place, *contents)))
            .collect();
        let staged_element_views: Vec<_> = self
            .structural_element_view_fields
            .iter()
            .filter_map(|(carrier, descriptor)| {
                retarget(carrier).map(|place| (place, descriptor.clone()))
            })
            .collect();
        let staged_referents: Vec<_> = self
            .reference_referents
            .iter()
            .filter_map(|(carrier, referent)| {
                retarget(carrier).map(|place| (place, referent.clone()))
            })
            .collect();
        let staged_primitives: Vec<_> = self
            .structural_primitive_storage
            .iter()
            .filter_map(|(carrier, scalar)| retarget(carrier).map(|place| (place, *scalar)))
            .collect();
        self.structural_scalar_fields.extend(staged_scalars);
        self.structural_byte_sequence_fields
            .extend(staged_byte_fields);
        self.structural_byte_arrays.extend(staged_arrays);
        self.structural_cases.extend(staged_cases);
        self.reference_referents.extend(staged_referents);
        self.structural_element_view_fields
            .extend(staged_element_views);
        self.structural_primitive_storage.extend(staged_primitives);
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
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let result = operation.result.structural().ok_or_else(invalid)?;
        if let OperationKind::StructuralLeafCopy { source, path } = &operation.kind
            && path.is_empty()
            && let Some(terminal_psi::StructuralTypeDeclaration {
                shape:
                    StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned {
                        capacity,
                    }),
                ..
            }) = self.program.structural_types.get(&result.structural_type)
        {
            let source_value = self.structural_values.get(source).ok_or_else(invalid)?;
            let bytes = self
                .byte_sequence_values
                .get(source)
                .ok_or_else(invalid)?
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
                return Err(invalid());
            }
            // Snapshot the live byte value before publishing its independent
            // identity. Later indexed writes detach the immutable backing.
            let bytes = bytes.clone();
            let copied = TerminalStructuralValue {
                opaque_identity: self.local_structural_identities.allocate()?,
                structural_type: result.structural_type,
                qualifications: result.qualifications.clone(),
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
        // Affine payload copies and reference relocations need their own
        // frontier/loan publication. Preserve the explicit execution boundary
        // rather than calling an admitted but unsupported operation malformed.
        if matches!(operation.kind, OperationKind::StructuralCaseLeafCopy { .. })
            && result.multiplicity != StructuralMultiplicity::Unrestricted
        {
            return Err(TerminalInterpretError::AffineProjectionNotRepresentable);
        }
        if result.multiplicity != StructuralMultiplicity::Unrestricted
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
            || self.structural_values.contains_key(&result.place)
        {
            return Err(invalid());
        }
        if let OperationKind::StructuralLeafCopy { source, path } = &operation.kind
            && path.is_empty()
            && !self.structural_values.contains_key(source)
            && (self.scalar_array_values.contains_key(source)
                || self.scalar_case_values.contains_key(source))
        {
            return self.copy_whole_scalar_payload(*source, result);
        }
        let (leaf, source, endpoint_field) = match &operation.kind {
            OperationKind::StructuralLeafCopy { source, path } => {
                let root = self.structural_values.get(source).ok_or_else(invalid)?;
                let leaf = crate::custody::project_structural_leaf_copy(
                    &self.program.structural_types,
                    root,
                    path,
                )?;
                let field = crate::custody::inline_structural_field(
                    &self.program.structural_types,
                    root.structural_type,
                    path,
                )
                .map(|field| field.id);
                (leaf, *source, field)
            }
            OperationKind::StructuralCaseLeafCopy { source, path } => {
                // Membership and typed endpoint selection remain owned by the
                // case walker. Its canonical final field identifies the same
                // runtime cell namespace as an ordinary resolved projection.
                let leaf = self.case_payload_leaf(*source, path)?;
                let field = match path.last() {
                    Some(semantic_vocabulary::CanonicalStructuralPathSegment::Field(field)) => {
                        Some(*field)
                    }
                    _ => None,
                };
                (leaf, *source, field)
            }
            _ => return Err(invalid()),
        };
        if leaf.structural_type != result.structural_type {
            // A record leaf copy out of a byte carrier mints fresh storage:
            // the resolved `u8` element is where the region starts and the
            // record's scalar leaves decode little-endian out of the
            // consecutive bytes the source recast judgment proved fit.
            return self.byte_carrier_leaf_copy(&leaf, result);
        }
        let copied = TerminalStructuralValue {
            opaque_identity: self.local_structural_identities.allocate()?,
            structural_type: result.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        };
        if self
            .program
            .structural_types
            .get(&result.structural_type)
            .is_some_and(|declaration| {
                matches!(declaration.shape, StructuralTypeShape::ByteSequence(_))
            })
        {
            let root = self.structural_values.get(&source).ok_or_else(invalid)?;
            let bytes = if StructuralRuntimePlace::from(root) == StructuralRuntimePlace::from(&leaf)
                && root.structural_type == leaf.structural_type
            {
                let bytes = self
                    .byte_sequence_values
                    .get(&source)
                    .ok_or_else(invalid)?
                    .immutable()?;
                if !bytes.matches_descriptor_home(root) {
                    return Err(invalid());
                }
                bytes.clone()
            } else {
                let mut parent = StructuralRuntimePlace::from(&leaf);
                parent.path.pop().ok_or_else(invalid)?;
                let cell = StructuralByteSequenceRuntimeField {
                    parent,
                    field: endpoint_field.ok_or_else(invalid)?,
                };
                self.structural_byte_sequence_fields
                    .get(&cell)
                    .ok_or_else(invalid)?
                    .clone()
            };
            // ByteSequenceView clones its backing handle and exact window.
            // This copies the descriptor, not octets; replacing the source
            // slot cannot retarget this completed value's retained backing.
            if self.byte_sequence_values.contains_key(&result.place) {
                return Err(invalid());
            }
            self.byte_sequence_values.insert(
                result.place,
                ByteSequenceBinding::Immutable(bytes.detached_descriptor()),
            );
        }
        if self
            .program
            .structural_types
            .get(&result.structural_type)
            .is_some_and(|declaration| {
                matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
            })
        {
            let root = self.structural_values.get(&source).ok_or_else(invalid)?;
            let descriptor = if StructuralRuntimePlace::from(root)
                == StructuralRuntimePlace::from(&leaf)
                && root.structural_type == leaf.structural_type
            {
                let descriptor = self.element_view_values.get(&source).ok_or_else(invalid)?;
                if !descriptor.matches_descriptor_home(root) {
                    return Err(invalid());
                }
                descriptor
            } else {
                self.structural_element_view_fields
                    .get(&StructuralRuntimePlace::from(&leaf))
                    .ok_or_else(invalid)?
            };
            let descriptor = descriptor.detached_descriptor();
            if self.element_view_values.contains_key(&result.place) {
                return Err(invalid());
            }
            self.element_view_values.insert(result.place, descriptor);
        }
        self.clone_subtree(
            &StructuralRuntimePlace::from(&leaf),
            &StructuralRuntimePlace::from(&copied),
        );
        self.structural_values.insert(result.place, copied);
        Ok(OperationFlow::Advance)
    }

    /// A whole scalar array or plain scalar case keeps its complete payload
    /// under its own place in `scalar_array_values` or `scalar_case_values`,
    /// never in `structural_values`, so a whole-home copy of one clones that
    /// payload under the fresh result place in the same map. The verifier
    /// admitted the source and the result type; this re-checks only what the
    /// stored payload can contradict: the payload has the result's type and
    /// the result place holds no value yet.
    fn copy_whole_scalar_payload(
        &mut self,
        source: semantic_vocabulary::PlaceId,
        result: &terminal_psi::StructuralOperationResult,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        if self.scalar_array_values.contains_key(&result.place)
            || self.scalar_case_values.contains_key(&result.place)
        {
            return Err(invalid());
        }
        if let Some(array) = self.scalar_array_values.get(&source) {
            if array.structural_type != result.structural_type {
                return Err(invalid());
            }
            let copied = array.clone();
            self.scalar_array_values.insert(result.place, copied);
            return Ok(OperationFlow::Advance);
        }
        let case = self.scalar_case_values.get(&source).ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(source),
        )?;
        if case.structural_type != result.structural_type {
            return Err(invalid());
        }
        let copied = case.clone();
        self.scalar_case_values.insert(result.place, copied);
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
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let Some((StructuralPathSegment::FixedIndex(index), parent_path)) = leaf.path.split_last()
        else {
            return Err(invalid());
        };
        let array = StructuralRuntimePlace {
            opaque_identity: leaf.opaque_identity,
            path: parent_path.to_vec(),
        };
        let bytes = self
            .structural_byte_arrays
            .get(&array)
            .ok_or_else(invalid)?;
        let layout = terminal_semantics::byte_carrier_scalar_layout(
            self.program.structural_types.values(),
            result.structural_type,
        )
        .ok_or_else(invalid)?;
        let index = usize::try_from(*index).map_err(|_| invalid())?;
        let mut leaves = Vec::with_capacity(layout.leaves.len());
        for member in &layout.leaves {
            let offset = index
                .checked_add(usize::try_from(member.offset).map_err(|_| invalid())?)
                .ok_or_else(invalid)?;
            let size = usize::try_from(match member.scalar_type {
                ScalarType::Boolean => 1u16,
                ScalarType::Integer(integer) => integer.bits() / 8,
                _ => return Err(invalid()),
            })
            .map_err(|_| invalid())?;
            let bytes = bytes
                .bytes()
                .get(offset..offset.checked_add(size).ok_or_else(invalid)?)
                .ok_or_else(invalid)?;
            let mut value = 0u128;
            for (position, byte) in bytes.iter().enumerate() {
                value |= u128::from(*byte) << (8 * position);
            }
            let scalar = match member.scalar_type {
                ScalarType::Boolean => match value {
                    0 => TerminalScalarValue::Boolean(false),
                    1 => TerminalScalarValue::Boolean(true),
                    _ => return Err(invalid()),
                },
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
                _ => return Err(invalid()),
            };
            leaves.push((member.field, scalar));
        }
        let copied = TerminalStructuralValue {
            opaque_identity: self.local_structural_identities.allocate()?,
            structural_type: result.structural_type,
            qualifications: Vec::new(),
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
        self.values.insert(
            operation.result.expect_scalar().id,
            TerminalScalarValue::Boolean(active_case == case),
        );
        Ok(OperationFlow::Advance)
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
        let structural_value = self.observation_root(source)?;
        let mut parent = StructuralRuntimePlace::from(&structural_value);
        if let Some(carrier) = terminal_semantics::case_payload_carrier(
            self.program.structural_types.values(),
            structural_value.structural_type,
            path,
        ) {
            parent.path.extend(carrier.path);
            let contents = self
                .structural_cases
                .get(&parent)
                .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
            if contents.structural_type != carrier.sum || contents.case != carrier.case {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
            return Ok(StructuralScalarRuntimeField::case_payload(parent, field));
        }
        let carrier = terminal_semantics::record_field_carrier(
            self.program.structural_types.values(),
            structural_value.structural_type,
            path,
        )
        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
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
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let cell = self.observed_field_cell(source, path, field)?;
        let value = self
            .structural_scalar_fields
            .get(&cell)
            .copied()
            .ok_or(TerminalInterpretError::StructuralBooleanFieldMissing { source, field })?;
        let TerminalScalarValue::Boolean(value) = value else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        self.values.insert(
            operation.result.expect_scalar().id,
            TerminalScalarValue::Boolean(value),
        );
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn execute_integer_structural_field(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::IntegerStructuralField {
            source,
            ref path,
            field,
        } = operation.kind
        else {
            unreachable!("dispatched execute_integer_structural_field")
        };
        let result = operation.result.expect_scalar();
        if !matches!(result.scalar_type, ScalarType::Integer(_)) {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let structural_value = self.observation_root(source)?;
        if terminal_semantics::observed_field_declaration(
            self.program.structural_types.values(),
            structural_value.structural_type,
            path,
            field,
        )
        .and_then(|declaration| declaration.field_type.scalar_type())
            != Some(result.scalar_type)
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let cell = self.observed_field_cell(source, path, field)?;
        let value = self
            .structural_scalar_fields
            .get(&cell)
            .copied()
            .ok_or(TerminalInterpretError::StructuralScalarFieldMissing { source, field })?;
        if value.scalar_type() != result.scalar_type {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        self.values.insert(result.id, value);
        Ok(OperationFlow::Advance)
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
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let count = |operand| -> Result<u64, TerminalInterpretError> {
            let value = self
                .values
                .get(&operand)
                .ok_or(TerminalInterpretError::VerifiedValueMissing(operand))?;
            let TerminalScalarValue::Integer {
                scalar_type,
                value: IntegerValue::Unsigned(value),
            } = value
            else {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            };
            if *scalar_type != count_type {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
            u64::try_from(*value).map_err(|_| TerminalInterpretError::VerifiedOperationMalformed)
        };
        let byte_index = count(index)?;
        let byte_length = count(length)?;
        if self.byte_sequence_length(source)? != byte_length || byte_index >= byte_length {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let byte_index = usize::try_from(byte_index)
            .map_err(|_| TerminalInterpretError::VerifiedOperationMalformed)?;
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
        let OperationKind::ByteSequenceLength { source } = operation.kind else {
            unreachable!("dispatched execute_byte_sequence_length")
        };
        let result = operation.result.expect_scalar();
        let integer_type =
            IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64).expect("u64 is valid");
        if result.scalar_type != ScalarType::Integer(integer_type) {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let length = self.byte_sequence_length(source)?;
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
