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

use checked_trees::{
    CheckedByteSequenceStoreDestination as Destination, CheckedByteSequenceValue as ByteValue,
    CheckedByteSequenceValuePlan,
};
use terminal_psi::control_flow::ByteSequenceOperand;

const LITERAL_VIEW_IDENTITY: &str = "compiler(byte-sequence-literal-view)";

/// The ordered frame supplies a real formal or a previously produced local
/// reference or owned home. Keeping these distinct prevents manufacturing parameter custody
/// for a call result; source replay and Terminal verify the loan independently.
pub(crate) enum StoreRoot<'a> {
    Parameter(&'a StructuralParameterDeclaration),
    Reference {
        place: semantic_vocabulary::PlaceId,
        structural_type: StructuralTypeId,
    },
    Owned {
        place: semantic_vocabulary::PlaceId,
        structural_type: StructuralTypeId,
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
    types: &[StructuralTypeDeclaration],
) -> Result<StructuralTypeId, LoweringError> {
    let declaration = types
        .iter()
        .find(|declaration| declaration.identity == LITERAL_VIEW_IDENTITY)
        .ok_or(LoweringError::Unsupported(
            "composed callable produced a type absent from the shared catalog",
        ))?;
    if !declaration.shape.is_borrowed_byte_view() {
        return unsupported("generated literal-view identity has a different carrier");
    }
    Ok(declaration.id)
}

/// Rejoin a retained result ordinal to the already emitted authored call.
/// Carrier shape and the permission to consume it remain the caller's checks.
pub(crate) fn completed_call_result(
    operations: &OperationBuffer,
    state: symbols::SymbolHandle,
    statement_index: u32,
    binding_ordinal: u32,
) -> Result<&terminal_psi::StructuralOperationResult, LoweringError> {
    let mut bindings = operations
        .structural_values
        .iter()
        .filter(|(ordinal, _)| *ordinal == binding_ordinal);
    let (_, binding) = bindings.next().ok_or(LoweringError::Unsupported(
        "byte call result has no completed binding",
    ))?;
    let produced = binding.operation_result()?;
    let mut occurrences = operations.source_calls.iter().filter(|occurrence| {
        occurrence.source_state == state
            && occurrence.statement_index == statement_index as usize
            && occurrence.call_ordinal == 0
    });
    let occurrence = occurrences.next().ok_or(LoweringError::Unsupported(
        "byte call result has no completed authored occurrence",
    ))?;
    if bindings.next().is_some()
        || occurrences.next().is_some()
        || !operations.operations.iter().any(|operation| {
            operation.id == occurrence.terminal_operation
                && operation.result.structural() == Some(produced)
        })
    {
        return unsupported("byte call result substituted its completed occurrence");
    }
    Ok(produced)
}

/// Generated owned carriers are shared by capacity, independently of live extent.
pub(crate) fn prepare_types(
    value: &CheckedByteSequenceValuePlan,
    types: &mut Vec<StructuralTypeDeclaration>,
) -> Result<(), LoweringError> {
    literal_view_type(types)?;
    if value
        .nodes
        .iter()
        .any(|(_, node)| matches!(node, ByteValue::Descriptor { .. }))
    {
        descriptor_value_type(types)?;
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
            if let Some(existing) = types
                .iter()
                .find(|candidate| candidate.identity == identity)
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
                    id: structural_type_id(ordinal),
                    identity,
                    shape,
                });
            }
        }
    }
    Ok(())
}

/// A generated immutable carrier, not a nominal authored type or parameter.
pub(crate) fn literal_view_type(
    types: &mut Vec<StructuralTypeDeclaration>,
) -> Result<StructuralTypeId, LoweringError> {
    let shape =
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
            access: Some(terminal_psi::StructuralAccess::SharedBorrow),
        });
    if let Some(declaration) = types
        .iter()
        .find(|declaration| declaration.identity == LITERAL_VIEW_IDENTITY)
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
        id,
        identity: LITERAL_VIEW_IDENTITY.into(),
        shape,
    });
    Ok(id)
}

/// Projected byte leaves copy to the canonical standalone descriptor shape.
/// An inline shared field has no nominal standalone type identity of its own.
pub(crate) fn descriptor_value_type(
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
        .any(|declaration| declaration.identity == IDENTITY)
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
        id,
        identity: IDENTITY.into(),
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

pub(crate) fn emit_content_store(
    store: &checked_trees::CheckedByteSequenceStorePlan,
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
        StoreRoot::Reference { place, .. } | StoreRoot::Owned { place, .. } => *place,
        StoreRoot::DescriptorHome { .. } => {
            return unsupported("a descriptor home holds no owned byte contents");
        }
    };
    let field_destination = match &store.destination {
        Destination::Whole => {
            let StoreRoot::Parameter(parameter) = &root else {
                return unsupported("whole byte store requires a borrowed bounded formal");
            };
            if !matches!(
                parameter.access,
                terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
            ) || !structural_types
                .iter()
                .find(|declaration| declaration.id == parameter.structural_type)
                .is_some_and(|declaration| {
                    matches!(
                        declaration.shape,
                        StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                        )
                    )
                })
            {
                return unsupported("whole byte store requires a borrowed bounded owner");
            }
            None
        }
        Destination::Field {
            carrier_path,
            field_identity,
        } => {
            let (path, field) = match &root {
                StoreRoot::Parameter(parameter) => {
                    crate::emission::structural_scalar_store::lower_structural_field_place(
                        store.statement_index,
                        store.statement_index,
                        parameter.position,
                        carrier_path,
                        field_identity,
                        parameter,
                        structural_types,
                    )?
                }
                StoreRoot::Reference {
                    structural_type, ..
                } => {
                    if !structural_types.iter().any(|declaration| {
                        declaration.id == *structural_type
                            && matches!(
                                declaration.shape,
                                StructuralTypeShape::Reference {
                                    access: terminal_psi::StructuralAccess::MutableBorrow,
                                    ..
                                }
                            )
                    }) {
                        return unsupported(
                            "byte-field local destination lost mutable reference custody",
                        );
                    }
                    crate::emission::structural_scalar_store::lower_structural_field_path(
                        *structural_type,
                        carrier_path,
                        field_identity,
                        structural_types,
                    )?
                }
                StoreRoot::DescriptorHome { .. } => {
                    return unsupported("a descriptor home has no byte field");
                }
                StoreRoot::Owned {
                    place,
                    structural_type,
                } => {
                    // Resolve an existing whole local, never a fabricated formal
                    // or a type-only promise that its fields are initialized.
                    let produced = operations
                        .operations
                        .iter()
                        .filter_map(|operation| operation.result.structural())
                        .find(|result| result.place == *place)
                        .ok_or(LoweringError::Unsupported(
                            "byte-field owned destination has no completed producer",
                        ))?;
                    if produced.structural_type != *structural_type
                        || produced.multiplicity
                            != terminal_psi::StructuralMultiplicity::Unrestricted
                        || !produced.qualifications.is_empty()
                        || !produced.projected_qualifications.is_empty()
                        || !produced.qualification_establishments.is_empty()
                        || !produced.claims.is_empty()
                        || !locals.iter().any(
                            |crate::expression_preparation::bindings::StructuralLocalBinding {
                                 argument: local,
                                 ..
                             }| {
                                local.place == *place
                                    && local.access == terminal_psi::StructuralAccess::Owned
                                    && local.path.is_empty()
                            },
                        )
                        || !structural_types.iter().any(|declaration| {
                            declaration.id == *structural_type
                                && matches!(declaration.shape, StructuralTypeShape::Record { .. })
                        })
                    {
                        return unsupported(
                            "byte-field local destination lost owned record custody",
                        );
                    }
                    crate::emission::structural_scalar_store::lower_structural_field_path(
                        *structural_type,
                        carrier_path,
                        field_identity,
                        structural_types,
                    )?
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
        Destination::Descriptor { .. } => {
            return unsupported("descriptor store reached the owned byte emitter");
        }
    };
    let mut emitter = ValueEmitter {
        plan: &store.value,
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
    let (operand, capacity) = emitter.emit(store.value.root, 0)?;
    let (source, length) = match operand {
        ByteSequenceOperand::Whole { source, length }
            if !matches!(
                store.value.nodes.get(store.value.root),
                ByteValue::Whole {
                    root: checked_trees::CheckedStoreRoot::Local { .. },
                    ..
                }
            ) =>
        {
            (source, length)
        }
        source => {
            // Local owners and fields must be snapshotted before the
            // destination write, including writes through an alias.
            // Parameter sources retain the store's exact formal-access
            // contract; generated literals and concatenations are already
            // independent sources.
            let (empty, _) = emitter.emission.literal(&[])?;
            emitter.owned(source, empty, capacity)?
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
    store: &checked_trees::CheckedByteSequenceStorePlan,
    root: &StoreRoot<'_>,
    path: &[CheckedUnitStructuralPathSegment],
    field_identity: Option<&str>,
    structural_types: &[StructuralTypeDeclaration],
) -> Result<DescriptorStore, LoweringError> {
    let shared_view = Some(terminal_psi::ByteSequenceCarrier::BorrowedView {
        access: Some(terminal_psi::StructuralAccess::SharedBorrow),
    });
    let Some(field_identity) = field_identity else {
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
        let declaration = structural_types
            .iter()
            .find(|declaration| declaration.id == *structural_type)
            .ok_or(LoweringError::Unsupported(
                "whole descriptor store home type was not declared",
            ))?;
        if !path.is_empty()
            || !matches!(
                declaration.shape,
                StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView {
                        access: None | Some(terminal_psi::StructuralAccess::SharedBorrow),
                    }
                )
            )
        {
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
    let (destination, structural_type, (steps, field)) = match root {
        StoreRoot::Parameter(parameter) => (
            parameter.place,
            parameter.structural_type,
            crate::emission::structural_scalar_store::lower_structural_field_place_steps(
                store.statement_index,
                store.statement_index,
                parameter.position,
                path,
                field_identity,
                parameter,
                structural_types,
            )?,
        ),
        StoreRoot::Reference {
            place,
            structural_type,
        }
        | StoreRoot::Owned {
            place,
            structural_type,
        } => (
            *place,
            *structural_type,
            crate::emission::structural_scalar_store::lower_structural_field_steps(
                *structural_type,
                path,
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
            .byte_lengths
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
    bytes: &[u8],
    structural_types: &[StructuralTypeDeclaration],
    places: &mut Vec<StructuralPlaceDeclaration>,
    next_place: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<semantic_vocabulary::PlaceId, LoweringError> {
    let structural_type = existing_literal_view_type(structural_types)?;
    let source = place_id(allocate_dense(next_place)?);
    let declaration_ordinal = super::next_byte_sequence_literal_ordinal(places)?;
    places.push(StructuralPlaceDeclaration {
        id: source,
        kind: StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal,
            structural_type,
        },
    });
    let id = operations.allocate();
    operations.push(Operation {
        id,
        static_reach_binding: None,
        suspension_crossing: None,
        result: OperationResult::Unit,
        kind: OperationKind::EstablishByteSequenceLiteral {
            destination: source,
            bytes: bytes.to_vec(),
            qualifications: Vec::new(),
        },
    });
    Ok(source)
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
    })) = structural_types
        .iter()
        .find(|row| row.id == structural_type)
        .map(|row| &row.shape)
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
    let (operand, maximum_length) = emitter.emit(value.root, 0)?;
    if maximum_length > *capacity {
        return unsupported("owned byte initializer exceeds its declared capacity");
    }
    // Snapshot into the declaration's carrier even when the operand is an
    // existing owner. The local is an independent value, never an alias.
    let (empty, _) = emitter.emission.literal(&[])?;
    emitter
        .emission
        .concatenate(operand, empty, structural_type, qualifications)
}

pub(crate) fn emit_owned_literal(
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
    let Some(StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned {
        capacity,
    })) = structural_types
        .iter()
        .find(|declaration| declaration.id == structural_type)
        .map(|declaration| declaration.shape.clone())
    else {
        return unsupported("owned literal local carrier is not a bounded byte owner");
    };
    if u64::try_from(bytes.len()).map_or(true, |length| length > capacity) {
        return unsupported("owned literal local exceeds its declared capacity");
    }
    let mut emission = ByteEmission {
        structural_types,
        places: literal_places,
        next_place,
        next_value,
        next_obligation,
        operations,
    };
    let (literal, _) = emission.literal(bytes)?;
    let (empty, _) = emission.literal(&[])?;
    emission.concatenate(literal, empty, structural_type, qualifications)
}

/// Materialize one literal at its exact call carrier. Borrowed views lend
/// immutable bytes; owned formals get fresh concatenation backing. Both Unit
/// control forms use this owner so neither can reinterpret image storage as
/// mutable owned storage. The caller retains the returned argument place;
/// `places` retains every supporting declaration, including that place.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_call_literal(
    bytes: &[u8],
    structural_type: StructuralTypeId,
    access: checked_trees::CheckedStructuralAccess,
    qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
    structural_types: &[StructuralTypeDeclaration],
    places: &mut Vec<StructuralPlaceDeclaration>,
    next_place: &mut u64,
    next_value: &mut u64,
    next_obligation: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let shape = structural_types
        .iter()
        .find(|row| row.id == structural_type)
        .map(|row| &row.shape);
    let declaration = match (access, shape) {
        (
            checked_trees::CheckedStructuralAccess::Owned,
            Some(StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
            )),
        ) => {
            emit_owned_literal(
                bytes,
                structural_type,
                qualifications,
                structural_types,
                places,
                next_place,
                next_value,
                next_obligation,
                operations,
            )?
            .0
        }
        (
            checked_trees::CheckedStructuralAccess::SharedBorrow,
            Some(StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
            )),
        ) => {
            let declaration = StructuralPlaceDeclaration {
                id: place_id(allocate_dense(next_place)?),
                kind: StructuralPlaceKind::ByteSequenceLiteral {
                    declaration_ordinal: super::next_byte_sequence_literal_ordinal(places)?,
                    structural_type,
                },
            };
            let id = operations.allocate();
            operations.push(Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id,
                result: OperationResult::Unit,
                kind: OperationKind::EstablishByteSequenceLiteral {
                    destination: declaration.id,
                    bytes: bytes.to_vec(),
                    qualifications,
                },
            });
            declaration
        }
        _ => return unsupported("literal call access differs from its storage carrier"),
    };
    places.push(declaration);
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

    fn literal(&mut self, bytes: &[u8]) -> Result<(ByteSequenceOperand, u64), LoweringError> {
        let source = emit_literal_view(
            bytes,
            self.structural_types,
            self.places,
            self.next_place,
            self.operations,
        )?;
        let capacity = u64::try_from(bytes.len())
            .map_err(|_| LoweringError::Unsupported("byte literal length overflow"))?;
        let length = self.length(OperationKind::ByteSequenceLength { source })?;
        Ok((ByteSequenceOperand::Whole { source, length }, capacity))
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
        let id = self.operations.allocate();
        let declaration = StructuralPlaceDeclaration {
            id: source,
            kind: StructuralPlaceKind::OperationResult {
                producer: id,
                structural_type,
            },
        };
        let result = terminal_psi::StructuralOperationResult {
            place: source,
            structural_type,
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            qualifications,
            projected_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        };
        self.operations.push(Operation {
            id,
            static_reach_binding: None,
            suspension_crossing: None,
            result: OperationResult::Structural(result.clone()),
            kind: OperationKind::EstablishByteSequenceConcatenation {
                left,
                right,
                capacity_obligation: obligation_id(allocate_dense(self.next_obligation)?),
            },
        });
        Ok((declaration, result))
    }
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
                candidate.identity == identity
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
            source: result.place,
        })?;
        Ok((result.place, length))
    }

    fn emit(
        &mut self,
        node: arena::Handle<ByteValue>,
        depth: usize,
    ) -> Result<(ByteSequenceOperand, u64), LoweringError> {
        if depth > 128 || !self.plan.nodes.iter().any(|(identity, _)| identity == node) {
            return unsupported("byte value plan has an invalid child");
        }
        match self.plan.nodes.get(node) {
            ByteValue::Literal(bytes) => self.emission.literal(bytes),
            ByteValue::Descriptor { .. } => {
                unsupported("descriptor value is not an owned byte-content snapshot")
            }
            ByteValue::CallResult {
                state,
                statement_index,
                binding_ordinal,
                capacity,
            } => {
                let produced = completed_call_result(
                    self.emission.operations,
                    *state,
                    *statement_index,
                    *binding_ordinal,
                )?;
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
                let length = self
                    .emission
                    .length(OperationKind::ByteSequenceLength { source })?;
                Ok((ByteSequenceOperand::Whole { source, length }, *capacity))
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
                let length = self
                    .emission
                    .length(OperationKind::ByteSequenceLength { source })?;
                Ok((ByteSequenceOperand::Whole { source, length }, *capacity))
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
                let (left, left_capacity) = self.emit(*left, depth + 1)?;
                let (right, right_capacity) = self.emit(*right, depth + 1)?;
                if left_capacity.checked_add(right_capacity) != Some(*capacity) {
                    return unsupported(
                        "byte concatenation capacity differs from operand carriers",
                    );
                }
                let (source, length) = self.owned(left, right, *capacity)?;
                Ok((ByteSequenceOperand::Whole { source, length }, *capacity))
            }
        }
    }
}
