//! Current structural access demand, separate from descriptor authority and
//! lifetime. Geometry retains a checked field prefix and the first captured
//! runtime selector; unresolved suffixes never establish disjointness.

use semantic_vocabulary::{
    CanonicalStructuralPathSegment, PlaceId, StructuralFieldId, StructuralTypeId,
};
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralAccess, StructuralArgument,
    StructuralPathSegment, StructuralTypeShape, TerminalMachine, TerminalModule,
};
use terminal_semantics::borrowed_storage::{Position, Projection};

pub(super) struct Access {
    pub place: PlaceId,
    pub projection: Projection,
    pub exclusive: bool,
}

pub(super) fn root_type(machine: &TerminalMachine, place: PlaceId) -> Option<StructuralTypeId> {
    crate::validation::structural::operations::caller_structural_root_type(machine, place)
}

fn argument(
    module: &TerminalModule,
    machine: &TerminalMachine,
    argument: &StructuralArgument,
    position: Position,
) -> Access {
    Access {
        place: argument.place,
        projection: projection(
            module,
            machine,
            argument.place,
            &argument.path,
            None,
            position,
        ),
        // An unrestricted by-value operand is copied before the consumer
        // runs. Its owned destination grants no write to the original
        // storage, including the backing of a copied shared descriptor.
        exclusive: match argument.access {
            StructuralAccess::SharedBorrow => false,
            StructuralAccess::Owned => {
                crate::validation::structural::result_contracts::source_signature(
                    machine,
                    argument.place,
                )
                .is_none_or(|source| {
                    source.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                })
            }
            _ => true,
        },
    }
}

fn whole(place: PlaceId, exclusive: bool) -> Access {
    Access {
        place,
        projection: Projection::new(&[]),
        exclusive,
    }
}

/// Follow the declaration owner's dynamic path interpretation. A nested
/// descriptor dereferences external backing, not the containing record slot.
/// Whole view roots already have a backing alias in the caller's adapter; a
/// nested view has no such exact alias and must remain indirect.
fn path_projection(
    module: &TerminalModule,
    root: Option<StructuralTypeId>,
    path: &[StructuralPathSegment],
    position: Position,
) -> (Projection, Option<StructuralTypeId>) {
    let mut projection = Projection::new(path).at(position);
    let Some(mut current) = root else {
        projection.indirect = true;
        return (projection, None);
    };
    for (index, segment) in path.iter().enumerate() {
        let Some(declaration) = module
            .structural_types
            .iter()
            .find(|item| item.id == current)
        else {
            projection.indirect = true;
            return (projection, None);
        };
        if matches!(declaration.shape, StructuralTypeShape::Reference { .. })
            || (index != 0
                && (matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
                    || declaration.shape.is_borrowed_byte_view()))
        {
            projection.indirect = true;
            return (projection, None);
        }
        // Byte operations express their final byte selector as region geometry;
        // it is not a structural child type. Root backing aliases remain valid.
        if matches!(declaration.shape, StructuralTypeShape::ByteSequence(_))
            && index + 1 == path.len()
            && matches!(
                segment,
                StructuralPathSegment::FixedIndex(_) | StructuralPathSegment::RuntimeIndex { .. }
            )
        {
            return (projection, Some(current));
        }
        // A fixed byte window is range geometry over an array, not a
        // structural child or an indirect load. Its ordinary source/type
        // validator checks byte element identity and bounds independently.
        if matches!(declaration.shape, StructuralTypeShape::FixedArray { .. })
            && index + 1 == path.len()
            && matches!(segment, StructuralPathSegment::FixedByteRange { .. })
        {
            return (projection, Some(current));
        }
        let Some(next) = crate::validation::foundation::resolve_runtime_projection(
            module,
            current,
            std::slice::from_ref(segment),
        ) else {
            projection.indirect = true;
            return (projection, None);
        };
        current = next;
    }
    (projection, Some(current))
}

fn field_declaration(
    module: &TerminalModule,
    carrier: StructuralTypeId,
    field: StructuralFieldId,
) -> Option<&terminal_psi::StructuralFieldDeclaration> {
    let declaration = module
        .structural_types
        .iter()
        .find(|item| item.id == carrier)?;
    let (StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. }) =
        &declaration.shape
    else {
        return None;
    };
    fields
        .iter()
        .find(|candidate| candidate.id == field && !candidate.relevance.is_erased())
}

fn byte_payload_projection(
    module: &TerminalModule,
    root: Option<StructuralTypeId>,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    position: Position,
) -> Projection {
    let (mut projection, carrier) = path_projection(module, root, path, position);
    let field = carrier.and_then(|carrier| field_declaration(module, carrier, field));
    if !field
        .and_then(|field| field.field_type.byte_sequence_carrier())
        .is_some_and(|carrier| {
            matches!(
                carrier,
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
            )
        })
    {
        projection.indirect = true;
    }
    if projection.exact
        && let Some(field) = field
    {
        projection
            .path
            .push(StructuralPathSegment::Field(field.identity.clone()));
    }
    projection
}

/// Resolve the separate field ID only after the carrier projection is exact.
/// A runtime-selected enclosing array already conservatively bounds the field.
pub(super) fn projection(
    module: &TerminalModule,
    machine: &TerminalMachine,
    place: PlaceId,
    path: &[StructuralPathSegment],
    field: Option<StructuralFieldId>,
    position: Position,
) -> Projection {
    let (mut projection, selected_type) =
        path_projection(module, root_type(machine, place), path, position);
    let Some(field) = field else {
        return projection;
    };
    if !projection.exact {
        return projection;
    }
    let selected = selected_type.and_then(|carrier| field_declaration(module, carrier, field));
    if let Some(field) = selected {
        projection
            .path
            .push(StructuralPathSegment::Field(field.identity.clone()));
    } else {
        projection.exact = false;
    }
    projection
}

fn canonical_projection(
    module: &TerminalModule,
    machine: &TerminalMachine,
    place: PlaceId,
    path: &[CanonicalStructuralPathSegment],
    field: Option<StructuralFieldId>,
    position: Position,
) -> Projection {
    let selected = root_type(machine, place).and_then(|root| {
        terminal_semantics::record_field_carrier(module.structural_types.iter(), root, path)
    });
    match selected {
        Some(carrier) => projection(module, machine, place, &carrier.path, field, position),
        None => {
            let mut unknown = Projection::new(&[]);
            unknown.exact = false;
            // An unresolved canonical path may cross a nested descriptor or
            // reference. Its containing home cannot establish alias freedom.
            unknown.indirect = true;
            unknown
        }
    }
}

pub(super) fn operation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    position: Position,
) -> Vec<Access> {
    let projected = |place, path: &[StructuralPathSegment], field, exclusive| Access {
        place,
        projection: projection(module, machine, place, path, field, position),
        exclusive,
    };
    match &operation.kind {
        // A length reads descriptor metadata; it does not access the backing.
        OperationKind::ElementViewLength { .. } | OperationKind::ByteSequenceLength { .. } => {
            Vec::new()
        }
        OperationKind::ElementViewSubslice {
            source, start, end, ..
        }
        | OperationKind::ByteSequenceSubslice {
            source, start, end, ..
        } => vec![Access {
            place: *source,
            projection: Projection::window(*start, *end, position),
            exclusive: operation
                .result
                .structural()
                .and_then(|result| {
                    crate::validation::element_view::view_loan_result_in_module(
                        module,
                        machine,
                        result.place,
                    )
                })
                .is_none_or(|(_, access)| access != StructuralAccess::SharedBorrow),
        }],
        OperationKind::ElementViewRead {
            source,
            index,
            obligation,
            path,
            ..
        } => {
            let mut selected = vec![StructuralPathSegment::RuntimeIndex {
                index: *index,
                obligation: *obligation,
            }];
            selected.extend_from_slice(path);
            vec![projected(*source, &selected, None, false)]
        }
        OperationKind::ByteSequenceRead {
            source,
            index,
            obligation,
            ..
        } => vec![projected(
            *source,
            &[StructuralPathSegment::RuntimeIndex {
                index: *index,
                obligation: *obligation,
            }],
            None,
            false,
        )],
        OperationKind::ElementViewWrite {
            destination,
            index,
            obligation,
            ..
        }
        | OperationKind::ByteSequenceWrite {
            destination,
            index,
            obligation,
            ..
        } => vec![projected(
            *destination,
            &[StructuralPathSegment::RuntimeIndex {
                index: *index,
                obligation: *obligation,
            }],
            None,
            true,
        )],
        OperationKind::PrimitiveScalarRead { source, path }
        | OperationKind::StructuralCaseMembership { source, path, .. }
        | OperationKind::StructuralLeafCopy { source, path } => {
            vec![projected(*source, path, None, false)]
        }
        OperationKind::WriteOnlyPrimitiveStore {
            destination, path, ..
        } => vec![projected(*destination, path, None, true)],
        OperationKind::StructuralScalarFieldStore {
            destination,
            path,
            field,
            ..
        } => vec![projected(*destination, path, Some(*field), true)],
        OperationKind::StructuralByteSequenceFieldByteStore {
            destination,
            path,
            field,
            ..
        } => vec![Access {
            place: *destination,
            projection: byte_payload_projection(
                module,
                root_type(machine, *destination),
                path,
                *field,
                position,
            ),
            exclusive: true,
        }],
        OperationKind::StructuralByteSequenceFieldLength {
            source,
            path,
            field,
        } => vec![projected(*source, path, Some(*field), false)],
        OperationKind::StructuralByteSequenceFieldRead {
            source,
            path,
            field,
            ..
        } => vec![Access {
            place: *source,
            projection: byte_payload_projection(
                module,
                root_type(machine, *source),
                path,
                *field,
                position,
            ),
            exclusive: false,
        }],
        OperationKind::StructuralByteSequenceFieldStore {
            destination,
            path,
            field,
            source,
            ..
        }
        | OperationKind::StructuralByteViewFieldStore {
            destination,
            path,
            field,
            source,
        } => vec![
            projected(*destination, path, Some(*field), true),
            whole(*source, false),
        ],
        OperationKind::MoveStructuralField {
            source,
            path,
            field,
        } => vec![projected(*source, path, *field, true)],
        OperationKind::StoreStructuralField {
            destination,
            path,
            field,
            value,
        } => vec![
            projected(*destination, path, *field, true),
            argument(module, machine, value, position),
        ],
        OperationKind::AtomicAccess {
            place,
            path,
            field,
            event,
            ..
        } => vec![projected(
            *place,
            path,
            Some(*field),
            event.modifies_resident(),
        )],
        OperationKind::BooleanStructuralField {
            source,
            path,
            field,
        }
        | OperationKind::IntegerStructuralField {
            source,
            path,
            field,
        } => vec![Access {
            place: *source,
            projection: canonical_projection(
                module,
                machine,
                *source,
                path,
                Some(*field),
                position,
            ),
            exclusive: false,
        }],
        OperationKind::StructuralCaseLeafCopy { source, path } => vec![Access {
            place: *source,
            projection: canonical_projection(module, machine, *source, path, None, position),
            exclusive: false,
        }],
        OperationKind::EstablishReference { source }
        | OperationKind::EstablishElementView { source, .. }
        | OperationKind::EstablishMutableElementView { source, .. } => {
            vec![argument(module, machine, source, position)]
        }
        OperationKind::CallUnit {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructural {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralWithScalarArguments {
            structural_arguments,
            ..
        }
        | OperationKind::BoundaryCall {
            structural_arguments,
            ..
        } => structural_arguments
            .iter()
            .map(|source| argument(module, machine, source, position))
            .collect(),
        OperationKind::EstablishRecord { fields }
        | OperationKind::EstablishStructuralCase { fields, .. } => fields
            .iter()
            .filter_map(|field| match &field.value {
                terminal_psi::RecordFieldValue::Structural(source) => {
                    Some(argument(module, machine, source, position))
                }
                terminal_psi::RecordFieldValue::Scalar { .. } => None,
            })
            .collect(),
        OperationKind::EstablishByteSequenceConcatenation { left, right, .. } => [left, right]
            .into_iter()
            .map(|operand| match operand {
                terminal_psi::control_flow::ByteSequenceOperand::Whole { source, .. } => {
                    whole(*source, false)
                }
                terminal_psi::control_flow::ByteSequenceOperand::Field {
                    source, field, ..
                } => Access {
                    place: source.place,
                    projection: byte_payload_projection(
                        module,
                        root_type(machine, source.place),
                        &source.path,
                        *field,
                        position,
                    ),
                    exclusive: false,
                },
            })
            .collect(),
        // Catalog-only operations do not expose direct root operands. Until
        // their catalog adapter supplies exact effects, retain indirect access.
        OperationKind::StoreDynamicDescriptor { .. }
        | OperationKind::CallDynamicScalar { .. }
        | OperationKind::CallDynamicParameterScalar { .. }
        | OperationKind::CallDynamicUnit { .. }
        | OperationKind::CallDynamicParameterUnit { .. } => machine
            .structural_places
            .first()
            .map(|place| {
                let mut access = whole(place.id, true);
                access.projection.indirect = true;
                access
            })
            .into_iter()
            .collect(),
        _ => {
            // The canonical exhaustive operand inventory owns every remaining
            // intrinsic demand. Unknown geometry keeps its complete root.
            let mut accesses = Vec::new();
            operation
                .kind
                .clone()
                .map_structural_uses_with_access(&mut |place, access| {
                    accesses.push(whole(place, access != StructuralAccess::SharedBorrow));
                    place
                });
            accesses
        }
    }
}

pub(super) fn may_escape(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> bool {
    if let OperationKind::CallStructural {
        structural_arguments,
        claim_transfers,
        returned_claim_transfers,
        ..
    }
    | OperationKind::CallStructuralWithScalarArguments {
        structural_arguments,
        claim_transfers,
        returned_claim_transfers,
        ..
    } = &operation.kind
    {
        return !claim_transfers.is_empty()
            || !returned_claim_transfers.is_empty()
            || terminal_semantics::descriptor_results::call_origins(module, machine, operation)
                .is_none()
            || structural_arguments.iter().any(|argument| {
                argument.access != StructuralAccess::SharedBorrow
                    && !super::call_accesses::argument_storage_closed(module, machine, argument)
            });
    }
    if let OperationKind::CallUnit {
        structural_arguments,
        claim_transfers,
        ..
    }
    | OperationKind::CallStructuralScalar {
        structural_arguments,
        claim_transfers,
        ..
    } = &operation.kind
    {
        return !matches!(
            operation.result,
            OperationResult::Unit | OperationResult::Scalar(_)
        ) || !claim_transfers.is_empty()
            || structural_arguments.iter().any(|argument| {
                argument.access != StructuralAccess::SharedBorrow
                    && !super::call_accesses::argument_storage_closed(module, machine, argument)
            });
    }
    !matches!(
        operation.kind,
        OperationKind::ElementViewLength { .. }
            | OperationKind::ElementViewRead { .. }
            | OperationKind::ByteSequenceLength { .. }
            | OperationKind::ByteSequenceRead { .. }
            | OperationKind::ElementViewWrite { .. }
            | OperationKind::ByteSequenceWrite { .. }
            | OperationKind::EstablishElementView { .. }
            | OperationKind::EstablishMutableElementView { .. }
            | OperationKind::ElementViewSubslice { .. }
            | OperationKind::ByteSequenceSubslice { .. }
            | OperationKind::EstablishRecord { .. }
            | OperationKind::EstablishStructuralCase { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{BlockId, MachineId, ObligationId, ValueId};
    use terminal_psi::{
        BindingRelevance, ByteSequenceCarrier, StructuralFieldDeclaration, StructuralFieldType,
        StructuralTypeDeclaration,
    };

    fn identity(value: u64) -> StructuralTypeId {
        StructuralTypeId::new(value).unwrap()
    }
    fn position() -> Position {
        (BlockId::new(1).unwrap(), 0)
    }
    fn selector() -> StructuralPathSegment {
        StructuralPathSegment::RuntimeIndex {
            index: ValueId::new(1).unwrap(),
            obligation: ObligationId::new(1).unwrap(),
        }
    }
    fn module() -> TerminalModule {
        let mut module = TerminalModule::for_entry(MachineId::new(1).unwrap());
        let fields = [
            ("owned", StructuralFieldType::Structural(identity(2))),
            ("view", StructuralFieldType::Structural(identity(3))),
            (
                "bytes",
                StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                    capacity: 4,
                }),
            ),
            (
                "borrowed",
                StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: Some(StructuralAccess::MutableBorrow),
                }),
            ),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (name, field_type))| StructuralFieldDeclaration {
            id: StructuralFieldId::new(index as u64 + 1).unwrap(),
            identity: name.into(),
            relevance: BindingRelevance::Relevant,
            field_type,
        })
        .collect();
        module.structural_types = vec![
            StructuralTypeDeclaration {
                id: identity(1),
                identity: "Item".into(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
            StructuralTypeDeclaration {
                id: identity(2),
                identity: "Items".into(),
                shape: StructuralTypeShape::FixedArray {
                    element: identity(1),
                    length: 4,
                },
            },
            StructuralTypeDeclaration {
                id: identity(3),
                identity: "ItemView".into(),
                shape: StructuralTypeShape::ElementView {
                    element: identity(1),
                },
            },
            StructuralTypeDeclaration {
                id: identity(4),
                identity: "Container".into(),
                shape: StructuralTypeShape::Record { fields },
            },
            StructuralTypeDeclaration {
                id: identity(5),
                identity: "Containers".into(),
                shape: StructuralTypeShape::FixedArray {
                    element: identity(4),
                    length: 4,
                },
            },
            StructuralTypeDeclaration {
                id: identity(6),
                identity: "Reference".into(),
                shape: StructuralTypeShape::Reference {
                    referent: identity(4),
                    access: StructuralAccess::MutableBorrow,
                },
            },
        ];
        module
    }

    #[test]
    fn descriptor_backing_projection_distinguishes_nested_views_from_owned_elements() {
        let module = module();
        let owned = [StructuralPathSegment::Field("owned".into()), selector()];
        let view = [StructuralPathSegment::Field("view".into()), selector()];
        assert!(
            !path_projection(&module, Some(identity(4)), &owned, position())
                .0
                .indirect
        );
        assert!(
            path_projection(&module, Some(identity(4)), &view, position())
                .0
                .indirect
        );
        let nested = [
            selector(),
            StructuralPathSegment::Field("view".into()),
            selector(),
        ];
        assert!(
            path_projection(&module, Some(identity(5)), &nested, position())
                .0
                .indirect
        );
        assert!(
            !path_projection(&module, Some(identity(3)), &[selector()], position())
                .0
                .indirect
        );
        assert!(
            path_projection(
                &module,
                Some(identity(6)),
                &[StructuralPathSegment::Referent],
                position()
            )
            .0
            .indirect
        );
    }

    #[test]
    fn descriptor_backing_projection_keeps_metadata_and_slot_replacement_direct() {
        let module = module();
        let slot = [StructuralPathSegment::Field("view".into())];
        let (projection, _) = path_projection(&module, Some(identity(4)), &slot, position());
        assert!(projection.exact);
        assert!(!projection.indirect);
        assert_eq!(projection.path, slot);
        let (metadata, _) = path_projection(&module, Some(identity(4)), &[], position());
        assert!(!metadata.indirect);
        assert!(
            byte_payload_projection(
                &module,
                Some(identity(4)),
                &[],
                StructuralFieldId::new(4).unwrap(),
                position()
            )
            .indirect
        );
        assert!(
            !byte_payload_projection(
                &module,
                Some(identity(4)),
                &[],
                StructuralFieldId::new(3).unwrap(),
                position()
            )
            .indirect
        );
    }
}
