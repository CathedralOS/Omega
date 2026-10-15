//! Source correspondence and ordered emission for bounded byte-field replacement.
//!
//! Field operands retain either a formal's storage or an established local's
//! current owned home. Both routes observe live length and snapshot the bytes
//! before replacing the destination; neither reuses an initializer's contents
//! after a call has mutated that home. Destination authority remains separate.
use super::{
    CheckedUnitStructuralPathSegment, LoweringError, Operation, OperationKind, OperationResult,
    PrimitiveType, StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralPlaceKind,
    StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape, allocate_dense,
    obligation_id, place_id, structural_type_id, terminal_scalar_type, unsupported, value_id,
};
use crate::emission::operation_emission::buffer::{
    OperationBuffer, StructuralBinding, scalar_result_declaration,
};

use checked_trees::{CheckedByteSequenceValue as ByteValue, CheckedByteSequenceValuePlan};
use terminal_psi::control_flow::ByteSequenceOperand;

const LITERAL_VIEW_IDENTITY: &str = "compiler(byte-sequence-literal-view)";

/// Physical homes for one literal occurrence, reserved independently of when
/// its operand is evaluated. Both call schedules consume this materializer.
/// Image descriptors stay immutable; an owned carrier gets fresh backing from
/// ordinary concatenation, with observed lengths and a capacity obligation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ByteLiteral {
    pub(crate) place: semantic_vocabulary::PlaceId,
    pub(crate) structural_type: StructuralTypeId,
    pub(crate) bytes: Vec<u8>,
    storage: LiteralStorage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LiteralStorage {
    Borrowed,
    Owned {
        source: semantic_vocabulary::PlaceId,
        empty: semantic_vocabulary::PlaceId,
        view_type: StructuralTypeId,
    },
}

impl ByteLiteral {
    /// A checked shared literal uses the expression schedule's reserved home.
    /// Its image backing never acquires owned or mutable storage authority.
    pub(crate) fn borrowed(
        place: semantic_vocabulary::PlaceId,
        structural_type: StructuralTypeId,
        bytes: &[u8],
        types: &[StructuralTypeDeclaration],
    ) -> Result<Self, LoweringError> {
        if !terminal_psi::find_structural_type(types, structural_type)
            .is_some_and(|declaration| declaration.shape.is_borrowed_byte_view())
        {
            return unsupported("shared byte literal has a non-view carrier");
        }
        Ok(Self {
            place,
            structural_type,
            bytes: bytes.to_vec(),
            storage: LiteralStorage::Borrowed,
        })
    }

    pub(crate) fn reserve(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        bytes: &[u8],
        structural_type: StructuralTypeId,
        types: &[StructuralTypeDeclaration],
        next_place: &mut u64,
    ) -> Result<Self, LoweringError> {
        let storage = match terminal_psi::find_structural_type(types, structural_type)
            .map(|declaration| &declaration.shape)
        {
            Some(StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
            )) => LiteralStorage::Borrowed,
            Some(StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
            )) => {
                if u64::try_from(bytes.len()).map_or(true, |length| length > *capacity) {
                    return unsupported("byte literal exceeds its declared capacity");
                }
                LiteralStorage::Owned {
                    source: place_id(allocate_dense(next_place)?),
                    empty: place_id(allocate_dense(next_place)?),
                    view_type: existing_literal_view_type(identities, types)?,
                }
            }
            _ => return unsupported("byte literal has neither an owned nor a borrowed carrier"),
        };
        Ok(Self {
            place: place_id(allocate_dense(next_place)?),
            structural_type,
            bytes: bytes.to_vec(),
            storage,
        })
    }

    fn declare_views(
        &self,
        places: &mut Vec<StructuralPlaceDeclaration>,
    ) -> Result<(), LoweringError> {
        let (views, structural_type): (&[_], _) = match self.storage {
            LiteralStorage::Borrowed => (&[self.place], self.structural_type),
            LiteralStorage::Owned {
                source,
                empty,
                view_type,
            } => (&[source, empty], view_type),
        };
        for &place in views {
            let declaration_ordinal = super::next_byte_sequence_literal_ordinal(places)?;
            places.push(StructuralPlaceDeclaration {
                id: place,
                kind: StructuralPlaceKind::ByteSequenceLiteral {
                    declaration_ordinal,
                    structural_type,
                },
            });
        }
        Ok(())
    }

    pub(crate) fn emit(
        &self,
        qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
        next_value: &mut u64,
        next_obligation: &mut u64,
        operations: &mut OperationBuffer,
    ) -> Result<(semantic_vocabulary::OperationId, OperationResult), LoweringError> {
        let LiteralStorage::Owned {
            source,
            empty,
            view_type,
        } = self.storage
        else {
            let id = emit_literal(
                self.place,
                self.structural_type,
                &self.bytes,
                qualifications,
                operations,
            );
            return Ok((id, OperationResult::Unit));
        };
        let mut operand = |place, bytes: &[u8]| -> Result<ByteSequenceOperand, LoweringError> {
            emit_literal(place, view_type, bytes, Vec::new(), operations);
            let length = value_id(allocate_dense(next_value)?);
            operations.push_leaf(
                scalar_result_declaration(length, terminal_scalar_type(PrimitiveType::U64)?),
                OperationKind::ByteSequenceLength {
                    source: terminal_psi::StructuralArgument::whole(
                        place,
                        terminal_psi::StructuralAccess::SharedBorrow,
                    ),
                },
            );
            Ok(ByteSequenceOperand::Whole {
                source: terminal_psi::StructuralArgument::whole(
                    place,
                    terminal_psi::StructuralAccess::SharedBorrow,
                ),
                length,
            })
        };
        let left = operand(source, &self.bytes)?;
        let right = operand(empty, &[])?;
        let (producer, result) = emit_concatenation(
            self.place,
            left,
            right,
            self.structural_type,
            qualifications,
            next_obligation,
            operations,
        )?;
        Ok((producer, OperationResult::Structural(result)))
    }
}

/// The ordered frame supplies a real formal or a previously produced local
/// home. Local ownership comes from its declaration, not a record/reference
/// shape classification; projected pointer crossings retain their access.
pub(crate) enum StoreRoot<'a> {
    Parameter(&'a StructuralParameterDeclaration),
    Local {
        place: semantic_vocabulary::PlaceId,
        structural_type: StructuralTypeId,
        multiplicity: terminal_psi::StructuralMultiplicity,
        access: terminal_psi::StructuralAccess,
    },
    /// A declared descriptor local's home, whose own establishment is its
    /// write authority. `qualified` records whether it declares byte
    /// predicates that every store must prove.
    DescriptorHome {
        place: semantic_vocabulary::PlaceId,
        structural_type: StructuralTypeId,
        qualified: bool,
    },
}

/// Shared callees must use the closure's existing carrier, not allocate one.
pub(crate) fn existing_literal_view_type(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    types: &[StructuralTypeDeclaration],
) -> Result<StructuralTypeId, LoweringError> {
    let declaration =
        terminal_psi::structural_type_by_identity(types, identities.intern(LITERAL_VIEW_IDENTITY))
            .ok_or(LoweringError::Unsupported(
                "composed callable produced a type absent from the shared catalog",
            ))?;
    if !declaration.shape.is_borrowed_byte_view() {
        return unsupported("generated literal-view identity has a different carrier");
    }
    Ok(declaration.id)
}

/// The completed result the call emitted for a retained result ordinal.
/// Carrier shape and the permission to consume it remain the caller's checks.
pub(crate) fn completed_call_result(
    operations: &OperationBuffer,
    binding_ordinal: u32,
) -> Result<&terminal_psi::StructuralOperationResult, LoweringError> {
    let (_, binding) = operations
        .structural_values
        .iter()
        .find(|(ordinal, _)| *ordinal == binding_ordinal)
        .ok_or(LoweringError::Unsupported(
            "byte call result has no completed binding",
        ))?;
    binding.operation_result()
}

/// Generated owned carriers are shared by capacity, independently of live extent.
pub(crate) fn prepare_types(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    value: &CheckedByteSequenceValuePlan,
    types: &mut Vec<StructuralTypeDeclaration>,
) -> Result<(), LoweringError> {
    literal_view_type(identities, types)?;
    if value
        .nodes
        .iter()
        .any(|(_, node)| matches!(node, ByteValue::Descriptor { .. }))
    {
        descriptor_value_type(identities, types)?;
    }
    for (identity, node) in value.nodes.iter() {
        let capacity = match node {
            ByteValue::Concatenation { capacity, .. } => Some(capacity),
            ByteValue::Field { capacity, .. } | ByteValue::Whole { capacity, .. }
                if identity == value.root =>
            {
                Some(capacity)
            }
            _ => None,
        };
        if let Some(capacity) = capacity {
            let identity = format!("compiler(byte-sequence-owned:{capacity})");
            let shape = StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedOwned {
                    capacity: *capacity,
                },
            );
            if let Some(existing) =
                terminal_psi::structural_type_by_identity(types, identities.intern(&identity))
            {
                if existing.shape != shape {
                    return unsupported("generated owned byte carrier has a different shape");
                }
            } else {
                let ordinal = types
                    .iter()
                    .map(|candidate| candidate.id.get())
                    .max()
                    .unwrap_or(0)
                    .checked_add(1)
                    .ok_or(LoweringError::Unsupported(
                        "owned byte type identity overflow",
                    ))?;
                types.push(StructuralTypeDeclaration {
                    quotient_carrier: None,
                    no_code_affine_cleanup: false,
                    descriptor_cleanup: false,
                    scalar_restriction: None,
                    id: structural_type_id(ordinal),
                    identity: identities.intern(&identity),
                    shape,
                });
            }
        }
    }
    Ok(())
}

/// A generated immutable carrier, not a nominal authored type or parameter.
pub(crate) fn literal_view_type(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    types: &mut Vec<StructuralTypeDeclaration>,
) -> Result<StructuralTypeId, LoweringError> {
    let shape =
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
            access: Some(terminal_psi::StructuralAccess::SharedBorrow),
        });
    if let Some(declaration) =
        terminal_psi::structural_type_by_identity(types, identities.intern(LITERAL_VIEW_IDENTITY))
    {
        return if declaration.shape == shape {
            Ok(declaration.id)
        } else {
            unsupported("generated literal-view identity has a different carrier")
        };
    }
    let identity = types
        .iter()
        .map(|declaration| declaration.id.get())
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "literal-view type identity overflow",
        ))?;
    let id = structural_type_id(identity);
    types.push(StructuralTypeDeclaration {
        quotient_carrier: None,
        no_code_affine_cleanup: false,
        descriptor_cleanup: false,
        scalar_restriction: None,
        id,
        identity: identities.intern(LITERAL_VIEW_IDENTITY),
        shape,
    });
    Ok(id)
}

/// Projected byte leaves copy to the canonical standalone descriptor shape.
/// An inline shared field has no nominal standalone type identity of its own.
pub(crate) fn descriptor_value_type(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    types: &mut Vec<StructuralTypeDeclaration>,
) -> Result<StructuralTypeId, LoweringError> {
    const IDENTITY: &str = "compiler(shared-byte-descriptor)";
    // Inline leaves have no nominal standalone identity. Match the first
    // exact canonical shape, as Terminal's leaf-copy endpoint resolver does;
    // a second same-shaped compiler declaration would name a different type.
    if let Some(existing) = descriptor_value_declaration(types) {
        return Ok(existing.id);
    }
    let shape =
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
            access: None,
        });
    if types
        .iter()
        .any(|declaration| declaration.identity == identities.intern(&IDENTITY))
    {
        return unsupported("descriptor carrier identity changed shape");
    }
    let id = structural_type_id(
        types
            .iter()
            .map(|declaration| declaration.id.get())
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(LoweringError::Unsupported(
                "descriptor carrier identity overflow",
            ))?,
    );
    types.push(StructuralTypeDeclaration {
        quotient_carrier: None,
        no_code_affine_cleanup: false,
        descriptor_cleanup: false,
        scalar_restriction: None,
        id,
        identity: identities.intern(IDENTITY),
        shape,
    });
    Ok(id)
}

pub(crate) fn existing_descriptor_value_type(
    types: &[StructuralTypeDeclaration],
) -> Result<StructuralTypeId, LoweringError> {
    descriptor_value_declaration(types)
        .map(|declaration| declaration.id)
        .ok_or(LoweringError::Unsupported(
            "descriptor value type was not prepared",
        ))
}

fn descriptor_value_declaration(
    types: &[StructuralTypeDeclaration],
) -> Option<&StructuralTypeDeclaration> {
    types.iter().find(|declaration| {
        declaration.shape
            == StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
                access: None,
            })
    })
}

/// Replace a bounded-owned carrier's live contents: the whole root (an empty
/// `path`) or the byte field `path` ends at.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_content_store(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    path: &[CheckedUnitStructuralPathSegment],
    value: &CheckedByteSequenceValuePlan,
    root: StoreRoot<'_>,
    parameters: &[(u32, StructuralParameterDeclaration)],
    locals: &[crate::expression_preparation::bindings::StructuralLocalBinding],
    structural_types: &[StructuralTypeDeclaration],
    literal_places: &mut Vec<StructuralPlaceDeclaration>,
    next_place: &mut u64,
    next_value: &mut u64,
    next_obligation: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<OperationKind, LoweringError> {
    let destination = match &root {
        StoreRoot::Parameter(parameter) => parameter.place,
        StoreRoot::Local { place, .. } => *place,
        StoreRoot::DescriptorHome { .. } => {
            return unsupported("a descriptor home holds no owned byte contents");
        }
    };
    let field_destination = match path.split_last() {
        None => {
            let StoreRoot::Parameter(parameter) = &root else {
                return unsupported("whole byte store requires a borrowed bounded formal");
            };
            if !matches!(
                parameter.access,
                terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
            ) || !terminal_psi::find_structural_type(structural_types, parameter.structural_type)
                .is_some_and(|declaration| declaration.shape.is_bounded_owned_byte_sequence())
            {
                return unsupported("whole byte store requires a borrowed bounded owner");
            }
            None
        }
        Some((CheckedUnitStructuralPathSegment::Field(field_identity), carrier_path)) => {
            let (path, field) = match &root {
                StoreRoot::Parameter(parameter) => {
                    crate::emission::structural_scalar_store::lower_structural_field_place(
                        identities,
                        carrier_path,
                        field_identity,
                        parameter,
                        structural_types,
                    )?
                }
                StoreRoot::DescriptorHome { .. } => {
                    return unsupported("a descriptor home has no byte field");
                }
                StoreRoot::Local {
                    structural_type,
                    multiplicity,
                    access,
                    ..
                } => {
                    let (path, field) =
                        crate::emission::structural_scalar_store::lower_structural_field_path(
                            identities,
                            *structural_type,
                            carrier_path,
                            field_identity,
                            structural_types,
                        )?;
                    // The local resolver authenticated this occurrence's home.
                    // Owning a descriptor still cannot grant mutation through
                    // a shared pointer anywhere in the selected carrier path.
                    if !terminal_semantics::reference_source_projection_accessible(
                        structural_types.iter(),
                        *structural_type,
                        *multiplicity,
                        *access,
                        &path,
                        terminal_psi::StructuralAccess::WriteOnlyBorrow,
                    ) {
                        return unsupported("byte-field local projection has no write authority");
                    }
                    (path, field)
                }
            };
            if !matches!(
                field.field_type.byte_sequence_carrier(),
                Some(terminal_psi::ByteSequenceCarrier::BoundedOwned { .. })
            ) {
                return unsupported("byte-field store destination is not bounded owned bytes");
            }
            Some((path, field.id))
        }
        Some(_) => {
            return unsupported("byte contents store does not end at a byte field");
        }
    };
    let mut emitter = ValueEmitter {
        plan: value,
        parameters,
        locals,
        emission: ByteEmission {
            structural_types,
            places: literal_places,
            next_place,
            next_value,
            next_obligation,
            operations,
        },
    };
    let (operand, capacity) = emitter.emit(identities, value.root, 0)?;
    let (source, length) = match operand {
        ByteSequenceOperand::Whole { source, length }
            if source.path.is_empty()
                && !matches!(
                    value.nodes.get(value.root),
                    ByteValue::Whole {
                        root: checked_trees::CheckedStoreRoot::Local { .. },
                        ..
                    }
                ) =>
        {
            (source.place, length)
        }
        source => {
            // Local owners and fields must be snapshotted before the
            // destination write, including writes through an alias.
            // Parameter sources retain the store's exact formal-access
            // contract; generated literals and concatenations are already
            // independent sources.
            let (empty, _) = emitter.emission.literal(identities, &[])?;
            emitter.owned(identities, source, empty, capacity)?
        }
    };
    let obligation = obligation_id(allocate_dense(next_obligation)?);
    match field_destination {
        Some((path, field)) => {
            operations.invalidate_field_lengths_under(destination, &path);
            Ok(OperationKind::StructuralByteSequenceFieldStore {
                destination,
                path,
                field,
                source,
                length,
                obligation,
            })
        }
        None => {
            operations.invalidate_field_lengths_under(destination, &[]);
            Ok(OperationKind::ByteSequenceStore {
                destination,
                source,
                length,
                obligation,
            })
        }
    }
}

/// The destination of a descriptor replacement, before the ordered frame
/// evaluates its selectors. Runtime elements retain the same bounds as any
/// other structural field store. A record field slot names its field; the
/// whole slot of a declared descriptor local names none.
pub(crate) struct DescriptorStore {
    pub(crate) destination: semantic_vocabulary::PlaceId,
    pub(crate) structural_type: StructuralTypeId,
    pub(crate) steps: Vec<crate::emission::runtime_elements::ProjectionStep>,
    field: Option<semantic_vocabulary::StructuralFieldId>,
    requires_predicate_obligation: bool,
}

pub(crate) fn prepare_descriptor_store(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    root: &StoreRoot<'_>,
    path: &[CheckedUnitStructuralPathSegment],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<DescriptorStore, LoweringError> {
    let shared_view = Some(terminal_psi::ByteSequenceCarrier::BorrowedView {
        access: Some(terminal_psi::StructuralAccess::SharedBorrow),
    });
    let Some((field_identity, carrier_path)) = path.split_last() else {
        // The whole slot is the root itself: a declared descriptor home. Its
        // write authority is the home's own; the shared access of the view
        // belongs to the referenced bytes. A standalone view type binds no
        // access: the home holds a shared descriptor because every store
        // installs a readable shared source.
        let StoreRoot::DescriptorHome {
            place,
            structural_type,
            qualified,
        } = root
        else {
            return unsupported("whole descriptor store requires a declared descriptor home");
        };
        let declaration = terminal_psi::find_structural_type(structural_types, *structural_type)
            .ok_or(LoweringError::Unsupported(
                "whole descriptor store home type was not declared",
            ))?;
        if !matches!(
            declaration.shape,
            StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
                access: None | Some(terminal_psi::StructuralAccess::SharedBorrow),
            })
        ) {
            return unsupported("whole descriptor store destination is not a shared view home");
        }
        return Ok(DescriptorStore {
            destination: *place,
            structural_type: *structural_type,
            steps: Vec::new(),
            field: None,
            requires_predicate_obligation: *qualified,
        });
    };
    let CheckedUnitStructuralPathSegment::Field(field_identity) = field_identity else {
        return unsupported("view-field store destination does not end at a field");
    };
    let (destination, structural_type, (steps, field)) = match root {
        StoreRoot::Parameter(parameter) => (
            parameter.place,
            parameter.structural_type,
            crate::emission::structural_scalar_store::lower_structural_field_place_steps(
                identities,
                carrier_path,
                field_identity,
                parameter,
                structural_types,
            )?,
        ),
        StoreRoot::Local {
            place,
            structural_type,
            ..
        } => (
            *place,
            *structural_type,
            crate::emission::structural_scalar_store::lower_structural_field_steps(
                identities,
                *structural_type,
                carrier_path,
                field_identity,
                structural_types,
            )?,
        ),
        StoreRoot::DescriptorHome { .. } => {
            return unsupported("a descriptor home has no record field slots");
        }
    };
    if field.field_type.byte_sequence_carrier() != shared_view {
        return unsupported("view-field store destination is not a shared view field");
    }
    Ok(DescriptorStore {
        destination,
        structural_type,
        steps,
        field: Some(field.id),
        requires_predicate_obligation: !field.field_type.byte_predicates().is_empty(),
    })
}

/// Commit the already completed source descriptor after destination selectors
/// and RHS evaluation. Predicate and lifetime evidence remain independent.
pub(crate) fn emit_descriptor_store(
    destination: DescriptorStore,
    path: Vec<terminal_psi::StructuralPathSegment>,
    source: semantic_vocabulary::PlaceId,
    next_obligation: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<OperationKind, LoweringError> {
    let predicate_obligation = if destination.requires_predicate_obligation {
        Some(obligation_id(allocate_dense(next_obligation)?))
    } else {
        None
    };
    operations.invalidate_field_lengths_under(destination.destination, &path);
    if destination.field.is_none() {
        // A whole replacement changes the home's own extent as well.
        operations
            .length_observations
            .bytes
            .retain(|(place, _)| *place != destination.destination);
    }
    Ok(OperationKind::StructuralByteViewFieldStore {
        destination: destination.destination,
        path,
        field: destination.field,
        source,
        predicate_obligation,
    })
}

/// One image-backed descriptor producer shared by content snapshots and
/// descriptor operands. Literal syntax changes backing residence, not the
/// consuming store's lifetime or predicate rules.
pub(crate) fn emit_literal_view(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    bytes: &[u8],
    qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
    structural_types: &[StructuralTypeDeclaration],
    places: &mut Vec<StructuralPlaceDeclaration>,
    next_place: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<semantic_vocabulary::PlaceId, LoweringError> {
    let structural_type = existing_literal_view_type(identities, structural_types)?;
    let source = place_id(allocate_dense(next_place)?);
    let declaration_ordinal = super::next_byte_sequence_literal_ordinal(places)?;
    places.push(StructuralPlaceDeclaration {
        id: source,
        kind: StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal,
            structural_type,
        },
    });
    emit_literal(source, structural_type, bytes, qualifications, operations);
    Ok(source)
}

fn emit_literal(
    destination: semantic_vocabulary::PlaceId,
    structural_type: StructuralTypeId,
    bytes: &[u8],
    mut qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
    operations: &mut OperationBuffer,
) -> semantic_vocabulary::OperationId {
    operations
        .literal_types
        .push((destination, structural_type));
    qualifications.sort();
    qualifications.dedup();
    let id = operations.allocate();
    operations.push(Operation {
        id,
        static_reach_binding: None,
        suspension_crossing: None,
        result: OperationResult::Unit,
        kind: OperationKind::EstablishByteSequenceLiteral {
            destination,
            bytes: bytes.to_vec(),
            qualifications,
        },
    });
    id
}

/// Publish emitted literal homes regardless of which expression consumed them.
/// Reservations alone are not declarations: only actual producers enter this
/// roster. Existing Unit/local emitters may already have published a home.
/// Owned copies have ordinary typed operation results and join the common
/// result-declaration collector instead of this image-storage namespace.
pub(crate) fn retain_literal_declarations(
    operations: &OperationBuffer,
    declarations: &mut Vec<StructuralPlaceDeclaration>,
) -> Result<(), LoweringError> {
    for &(place, structural_type) in &operations.literal_types {
        if declarations
            .iter()
            .any(|declaration| declaration.id == place)
        {
            continue;
        }
        let kind = StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal: super::next_byte_sequence_literal_ordinal(declarations)?,
            structural_type,
        };
        declarations.push(StructuralPlaceDeclaration { id: place, kind });
    }
    Ok(())
}

/// Establish an owned bounded byte local from its checked initializer.
///
/// `let text: [u8; 8] in Utf8 = "150";` owns capacity 8 and live length 3
/// (wiki/spec/language/domains.md, "Byte containers and encoding domains").
/// The initializer's live bytes are copied into fresh backing of the local's
/// declared carrier. Later mutations of an operand cannot change the local.
/// Concatenation proves capacity and Terminal replays the byte predicates.
///
/// Returns the result place's declaration and the established result. The
/// caller publishes the declaration in its own result roster.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_owned_value(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    value: &CheckedByteSequenceValuePlan,
    structural_type: StructuralTypeId,
    qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
    parameters: &[(u32, StructuralParameterDeclaration)],
    locals: &[crate::expression_preparation::bindings::StructuralLocalBinding],
    structural_types: &[StructuralTypeDeclaration],
    places: &mut Vec<StructuralPlaceDeclaration>,
    next_place: &mut u64,
    next_value: &mut u64,
    next_obligation: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<
    (
        StructuralPlaceDeclaration,
        terminal_psi::StructuralOperationResult,
    ),
    LoweringError,
> {
    let Some(StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned {
        capacity,
    })) =
        terminal_psi::find_structural_type(structural_types, structural_type).map(|row| &row.shape)
    else {
        return unsupported("owned byte local has no bounded carrier");
    };
    let mut emitter = ValueEmitter {
        plan: value,
        parameters,
        locals,
        emission: ByteEmission {
            structural_types,
            places,
            next_place,
            next_value,
            next_obligation,
            operations,
        },
    };
    let (operand, maximum_length) = emitter.emit(identities, value.root, 0)?;
    if maximum_length > *capacity {
        return unsupported("owned byte initializer exceeds its declared capacity");
    }
    // Snapshot into the declaration's carrier even when the operand is an
    // existing owner. The local is an independent value, never an alias.
    let (empty, _) = emitter.emission.literal(identities, &[])?;
    emitter
        .emission
        .concatenate(operand, empty, structural_type, qualifications)
}

pub(crate) fn emit_owned_literal(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    bytes: &[u8],
    structural_type: StructuralTypeId,
    qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
    structural_types: &[StructuralTypeDeclaration],
    literal_places: &mut Vec<StructuralPlaceDeclaration>,
    next_place: &mut u64,
    next_value: &mut u64,
    next_obligation: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<
    (
        StructuralPlaceDeclaration,
        terminal_psi::StructuralOperationResult,
    ),
    LoweringError,
> {
    let literal = ByteLiteral::reserve(
        identities,
        bytes,
        structural_type,
        structural_types,
        next_place,
    )?;
    if !matches!(literal.storage, LiteralStorage::Owned { .. }) {
        return unsupported("owned literal local carrier is not a bounded byte owner");
    }
    literal.declare_views(literal_places)?;
    let (producer, result) =
        literal.emit(qualifications, next_value, next_obligation, operations)?;
    let OperationResult::Structural(result) = result else {
        return unsupported("owned literal materialization has no owned result");
    };
    Ok((
        StructuralPlaceDeclaration {
            id: literal.place,
            kind: StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            },
        },
        result,
    ))
}

/// Materialize one literal at its exact call carrier. Borrowed views lend
/// immutable bytes; owned formals get fresh concatenation backing. Both Unit
/// control forms use this owner so neither can reinterpret image storage as
/// mutable owned storage. The caller retains the returned argument place;
/// `places` retains every supporting declaration, including that place.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_call_literal(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    bytes: &[u8],
    structural_type: StructuralTypeId,
    qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
    structural_types: &[StructuralTypeDeclaration],
    places: &mut Vec<StructuralPlaceDeclaration>,
    next_place: &mut u64,
    next_value: &mut u64,
    next_obligation: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let literal = ByteLiteral::reserve(
        identities,
        bytes,
        structural_type,
        structural_types,
        next_place,
    )?;
    let declaration_ordinal = super::next_byte_sequence_literal_ordinal(places)?;
    literal.declare_views(places)?;
    let (producer, result) =
        literal.emit(qualifications, next_value, next_obligation, operations)?;
    let kind = if matches!(result, OperationResult::Structural(_)) {
        StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        }
    } else {
        StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal,
            structural_type,
        }
    };
    let declaration = StructuralPlaceDeclaration {
        id: literal.place,
        kind,
    };
    if matches!(result, OperationResult::Structural(_)) {
        places.push(declaration);
    }
    Ok(declaration)
}

/// The operation, place and value counters one byte materialization emits
/// into. Literal views and owned concatenations are its only producers.
struct ByteEmission<'input> {
    structural_types: &'input [StructuralTypeDeclaration],
    places: &'input mut Vec<StructuralPlaceDeclaration>,
    next_place: &'input mut u64,
    next_value: &'input mut u64,
    next_obligation: &'input mut u64,
    operations: &'input mut OperationBuffer,
}

impl ByteEmission<'_> {
    fn length(
        &mut self,
        kind: OperationKind,
    ) -> Result<semantic_vocabulary::ValueId, LoweringError> {
        let length = value_id(allocate_dense(self.next_value)?);
        let id = self.operations.allocate();
        self.operations.push(Operation {
            id,
            kind,
            static_reach_binding: None,
            suspension_crossing: None,
            result: scalar_result_declaration(length, terminal_scalar_type(PrimitiveType::U64)?),
        });
        Ok(length)
    }

    fn literal(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        bytes: &[u8],
    ) -> Result<(ByteSequenceOperand, u64), LoweringError> {
        let source = emit_literal_view(
            identities,
            bytes,
            Vec::new(),
            self.structural_types,
            self.places,
            self.next_place,
            self.operations,
        )?;
        let capacity = u64::try_from(bytes.len())
            .map_err(|_| LoweringError::Unsupported("byte literal length overflow"))?;
        let length = self.length(OperationKind::ByteSequenceLength {
            source: terminal_psi::StructuralArgument::whole(
                source,
                terminal_psi::StructuralAccess::SharedBorrow,
            ),
        })?;
        Ok((
            ByteSequenceOperand::Whole {
                source: terminal_psi::StructuralArgument::whole(
                    source,
                    terminal_psi::StructuralAccess::SharedBorrow,
                ),
                length,
            },
            capacity,
        ))
    }

    /// Materialize `left` then `right` into fresh owned backing of
    /// `structural_type`. The result place's declaration is returned rather
    /// than published: a store's temporary and a local binding publish it in
    /// different rosters.
    fn concatenate(
        &mut self,
        left: ByteSequenceOperand,
        right: ByteSequenceOperand,
        structural_type: StructuralTypeId,
        qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
    ) -> Result<
        (
            StructuralPlaceDeclaration,
            terminal_psi::StructuralOperationResult,
        ),
        LoweringError,
    > {
        let source = place_id(allocate_dense(self.next_place)?);
        let (id, result) = emit_concatenation(
            source,
            left,
            right,
            structural_type,
            qualifications,
            self.next_obligation,
            self.operations,
        )?;
        let declaration = StructuralPlaceDeclaration {
            id: source,
            kind: StructuralPlaceKind::OperationResult {
                producer: id,
                structural_type,
            },
        };
        Ok((declaration, result))
    }
}

/// Establish concatenation in a reserved home. Namespace allocation belongs
/// to the enclosing schedule, while this producer owns its exact result and
/// capacity obligation for both immediate and deferred materialization.
fn emit_concatenation(
    place: semantic_vocabulary::PlaceId,
    left: ByteSequenceOperand,
    right: ByteSequenceOperand,
    structural_type: StructuralTypeId,
    mut qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
    next_obligation: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<
    (
        semantic_vocabulary::OperationId,
        terminal_psi::StructuralOperationResult,
    ),
    LoweringError,
> {
    // Source-domain order does not imply allocated Terminal-domain order.
    // Publish the membership set in the identity namespace it now belongs to.
    qualifications.sort();
    qualifications.dedup();
    let id = operations.allocate();
    let result = terminal_psi::StructuralOperationResult {
        place,
        structural_type,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        qualifications,
        projected_qualifications: Vec::new(),
        qualification_establishments: Vec::new(),
        claims: Vec::new(),
    };
    operations.push(Operation {
        id,
        static_reach_binding: None,
        suspension_crossing: None,
        result: OperationResult::Structural(result.clone()),
        kind: OperationKind::EstablishByteSequenceConcatenation {
            left,
            right,
            capacity_obligation: obligation_id(allocate_dense(next_obligation)?),
        },
    });
    Ok((id, result))
}

struct ValueEmitter<'input> {
    plan: &'input CheckedByteSequenceValuePlan,
    parameters: &'input [(u32, StructuralParameterDeclaration)],
    locals: &'input [crate::expression_preparation::bindings::StructuralLocalBinding],
    emission: ByteEmission<'input>,
}

impl ValueEmitter<'_> {
    /// Locate the completed owner, without reconstructing its initial contents.
    /// The caller checks the read shape; Terminal checks liveness at the use.
    fn local_source(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Result<&terminal_psi::StructuralOperationResult, LoweringError> {
        let mut locals = self
            .locals
            .iter()
            .filter(|local| local.root == facts::PlaceRoot::Symbol(symbol));
        let crate::expression_preparation::bindings::StructuralLocalBinding {
            argument: local, ..
        } = locals.next().ok_or(LoweringError::Unsupported(
            "byte value local has no established home",
        ))?;
        if locals.next().is_some()
            || local.access != terminal_psi::StructuralAccess::Owned
            || !local.path.is_empty()
        {
            return unsupported("byte value local lost its exact whole owned home");
        }
        self.emission
            .operations
            .operations
            .iter()
            .filter_map(|operation| operation.result.structural())
            .find(|result| result.place == local.place)
            .or_else(|| {
                // Control joins register the same completed owner's custody.
                self.emission
                    .operations
                    .structural_values
                    .iter()
                    .filter_map(|(_, binding)| match binding {
                        StructuralBinding::OperationResult(result) => Some(result),
                        _ => None,
                    })
                    .find(|result| result.place == local.place)
            })
            .ok_or(LoweringError::Unsupported(
                "byte value local has no completed producer",
            ))
    }

    /// Reading a field does not consume its root. Owned formals, borrowed
    /// formals and completed local homes retain the same field-read operation;
    /// the receiving validator separately checks live custody and loan conflicts.
    fn field_source(
        &self,
        root: checked_trees::CheckedStoreRoot,
    ) -> Result<(semantic_vocabulary::PlaceId, StructuralTypeId), LoweringError> {
        match root {
            checked_trees::CheckedStoreRoot::Parameter { position } => {
                let (_, parameter) = self
                    .parameters
                    .iter()
                    .find(|(authored, _)| *authored == position)
                    .ok_or(LoweringError::Unsupported("byte value parameter is absent"))?;
                if !matches!(
                    parameter.access,
                    terminal_psi::StructuralAccess::Owned
                        | terminal_psi::StructuralAccess::SharedBorrow
                        | terminal_psi::StructuralAccess::MutableBorrow
                ) || !matches!(
                    parameter.multiplicity,
                    terminal_psi::StructuralMultiplicity::Unrestricted
                        | terminal_psi::StructuralMultiplicity::Affine
                ) || !parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty()
                {
                    return unsupported("byte value parameter has no ordinary read authority");
                }
                Ok((parameter.place, parameter.structural_type))
            }
            checked_trees::CheckedStoreRoot::Local { symbol } => {
                let produced = self.local_source(symbol)?;
                if !matches!(
                    produced.multiplicity,
                    terminal_psi::StructuralMultiplicity::Unrestricted
                        | terminal_psi::StructuralMultiplicity::Affine
                ) || !produced.qualifications.is_empty()
                    || !produced.projected_qualifications.is_empty()
                    || !produced.claims.is_empty()
                {
                    return unsupported("byte value local lost its ordinary read custody");
                }
                Ok((produced.place, produced.structural_type))
            }
        }
    }

    fn owned(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        left: ByteSequenceOperand,
        right: ByteSequenceOperand,
        capacity: u64,
    ) -> Result<(semantic_vocabulary::PlaceId, semantic_vocabulary::ValueId), LoweringError> {
        let identity = format!("compiler(byte-sequence-owned:{capacity})");
        let structural_type = self
            .emission
            .structural_types
            .iter()
            .find(|candidate| {
                candidate.identity == identities.intern(&identity)
                    && candidate.shape
                        == StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
                        )
            })
            .ok_or(LoweringError::Unsupported(
                "owned byte carrier absent from shared catalog",
            ))?
            .id;
        let (declaration, result) =
            self.emission
                .concatenate(left, right, structural_type, Vec::new())?;
        self.emission.places.push(declaration);
        let length = self.emission.length(OperationKind::ByteSequenceLength {
            source: terminal_psi::StructuralArgument::whole(
                result.place,
                terminal_psi::StructuralAccess::SharedBorrow,
            ),
        })?;
        Ok((result.place, length))
    }

    fn emit(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,

        node: arena::Handle<ByteValue>,
        depth: usize,
    ) -> Result<(ByteSequenceOperand, u64), LoweringError> {
        if depth > 128 || !self.plan.nodes.iter().any(|(identity, _)| identity == node) {
            return unsupported("byte value plan has an invalid child");
        }
        match self.plan.nodes.get(node) {
            ByteValue::Literal(bytes) => self.emission.literal(identities, bytes),
            ByteValue::Descriptor { .. } => {
                unsupported("descriptor value is not an owned byte-content snapshot")
            }
            ByteValue::CallResult {
                binding_ordinal,
                capacity,
                ..
            } => {
                let produced = completed_call_result(self.emission.operations, *binding_ordinal)?;
                if produced.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                    || !produced.projected_qualifications.is_empty()
                    || !produced.claims.is_empty()
                    || !self.emission.structural_types.iter().any(|declaration| {
                        declaration.id == produced.structural_type
                            && declaration.shape
                                == StructuralTypeShape::ByteSequence(
                                    terminal_psi::ByteSequenceCarrier::BoundedOwned {
                                        capacity: *capacity,
                                    },
                                )
                    })
                {
                    return unsupported("byte call result lost exact completed owned custody");
                }
                let source = produced.place;
                let length = self.emission.length(OperationKind::ByteSequenceLength {
                    source: terminal_psi::StructuralArgument::whole(
                        source,
                        terminal_psi::StructuralAccess::SharedBorrow,
                    ),
                })?;
                Ok((
                    ByteSequenceOperand::Whole {
                        source: terminal_psi::StructuralArgument::whole(
                            source,
                            terminal_psi::StructuralAccess::SharedBorrow,
                        ),
                        length,
                    },
                    *capacity,
                ))
            }
            ByteValue::Whole { root, capacity } => {
                let (source, structural_type) = match *root {
                    checked_trees::CheckedStoreRoot::Parameter { position } => {
                        let (_, parameter) = self
                            .parameters
                            .iter()
                            .find(|(authored, _)| *authored == position)
                            .ok_or(LoweringError::Unsupported(
                                "whole byte value parameter is absent",
                            ))?;
                        if !matches!(
                            parameter.access,
                            terminal_psi::StructuralAccess::Owned
                                | terminal_psi::StructuralAccess::SharedBorrow
                                | terminal_psi::StructuralAccess::MutableBorrow
                        ) || parameter.multiplicity
                            != terminal_psi::StructuralMultiplicity::Unrestricted
                            || !parameter.projected_qualifications.is_empty()
                        {
                            return unsupported(
                                "whole byte parameter has no ordinary read authority",
                            );
                        }
                        (parameter.place, parameter.structural_type)
                    }
                    checked_trees::CheckedStoreRoot::Local { symbol } => {
                        let produced = self.local_source(symbol)?;
                        if produced.multiplicity
                            != terminal_psi::StructuralMultiplicity::Unrestricted
                            || !produced.projected_qualifications.is_empty()
                            || !produced.claims.is_empty()
                        {
                            return unsupported("whole byte local has no ordinary read custody");
                        }
                        (produced.place, produced.structural_type)
                    }
                };
                if !self.emission.structural_types.iter().any(|row| {
                    row.id == structural_type
                        && row.shape
                            == StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BoundedOwned {
                                    capacity: *capacity,
                                },
                            )
                }) {
                    return unsupported("whole byte source has a different capacity");
                }
                let length = self.emission.length(OperationKind::ByteSequenceLength {
                    source: terminal_psi::StructuralArgument::whole(
                        source,
                        terminal_psi::StructuralAccess::SharedBorrow,
                    ),
                })?;
                Ok((
                    ByteSequenceOperand::Whole {
                        source: terminal_psi::StructuralArgument::whole(
                            source,
                            terminal_psi::StructuralAccess::SharedBorrow,
                        ),
                        length,
                    },
                    *capacity,
                ))
            }
            ByteValue::Field {
                root,
                carrier_path,
                field_identity,
                capacity,
            } => {
                let (source_place, source_type) = self.field_source(*root)?;
                let (path, field) =
                    crate::emission::structural_scalar_store::lower_structural_field_path(
                        identities,
                        source_type,
                        carrier_path,
                        field_identity,
                        self.emission.structural_types,
                    )?;
                if field.field_type.byte_sequence_carrier()
                    != Some(terminal_psi::ByteSequenceCarrier::BoundedOwned {
                        capacity: *capacity,
                    })
                {
                    return unsupported("byte value field has a different carrier");
                }
                let source = terminal_psi::StructuralArgument {
                    place: source_place,
                    path: path.clone(),
                    access: terminal_psi::StructuralAccess::SharedBorrow,
                };
                let field = field.id;
                // Observe at this operand's evaluation point. Later effects cannot
                // retroactively supply this source's extent or content custody.
                let length =
                    self.emission
                        .length(OperationKind::StructuralByteSequenceFieldLength {
                            source: source.place,
                            path,
                            field,
                        })?;
                Ok((
                    ByteSequenceOperand::Field {
                        source,
                        field,
                        length,
                    },
                    *capacity,
                ))
            }
            ByteValue::Concatenation {
                left,
                right,
                capacity,
            } => {
                let (left, left_capacity) = self.emit(identities, *left, depth + 1)?;
                let (right, right_capacity) = self.emit(identities, *right, depth + 1)?;
                if left_capacity.checked_add(right_capacity) != Some(*capacity) {
                    return unsupported(
                        "byte concatenation capacity differs from operand carriers",
                    );
                }
                let (source, length) = self.owned(identities, left, right, *capacity)?;
                Ok((
                    ByteSequenceOperand::Whole {
                        source: terminal_psi::StructuralArgument::whole(
                            source,
                            terminal_psi::StructuralAccess::SharedBorrow,
                        ),
                        length,
                    },
                    *capacity,
                ))
            }
        }
    }
}
