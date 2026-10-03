//! Complete record establishment binds declarations, never layout-compatible names.
use super::{
    BTreeMap, BTreeSet, ModuleError, OperationKind, PlaceId, ScalarType, StructuralAccess,
    StructuralFieldType, StructuralMultiplicity, StructuralPlaceKind, StructuralTypeId,
    StructuralTypeShape, TerminalMachine, TerminalModule, ValueId,
};
use terminal_psi::{RecordFieldValue, StructuralFieldDeclaration};

pub(super) fn plain_type(module: &TerminalModule, root: StructuralTypeId) -> bool {
    record_type(module, root, false, false)
}

/// Local construction can relocate checked reference carriers and
/// shared-view descriptors alike: the binding roster checks each relocated
/// loan's source. Keep this separate from plain payload classification:
/// plain calls and returns do not thereby acquire recursive reference
/// transfer semantics.
pub(super) fn constructible_type(module: &TerminalModule, root: StructuralTypeId) -> bool {
    record_type(module, root, true, true)
}

/// A record a shared loan may observe: every runtime member is plain storage
/// or a shared-view descriptor (`&[T]`/`&[u8]` members), so borrowing the
/// record reads its stored loans without moving them. Descriptor members
/// carry no owned leaf cells — they relocate caller storage the loan source
/// already names.
pub(super) fn shared_view_carrier_type(module: &TerminalModule, root: StructuralTypeId) -> bool {
    record_type(module, root, false, true)
}

/// Whether a member's shape is a shared-view descriptor (`&[T]` element
/// view): the record stores a loan, not owned bytes.
fn shared_view_member(module: &TerminalModule, child: StructuralTypeId) -> bool {
    module.structural_types.iter().any(|item| {
        item.id == child && matches!(item.shape, StructuralTypeShape::ElementView { .. })
    })
}

fn record_type(
    module: &TerminalModule,
    root: StructuralTypeId,
    references: bool,
    shared_view_members: bool,
) -> bool {
    fn field_valid(
        module: &TerminalModule,
        field: &StructuralFieldDeclaration,
        active: &mut Vec<StructuralTypeId>,
        complete: &mut BTreeSet<StructuralTypeId>,
        references: bool,
        shared_view_members: bool,
    ) -> bool {
        // An erased member carries its declaration only: proof material
        // contributes no runtime field and needs no carrier recursion.
        if field.relevance.is_erased()
            && matches!(field.field_type, StructuralFieldType::Erased { .. })
        {
            return true;
        }
        field.relevance == terminal_psi::BindingRelevance::Relevant
            && match field.field_type {
                StructuralFieldType::Structural(child) => {
                    (references && super::references::referent(module, child).is_some())
                        || (shared_view_members && shared_view_member(module, child))
                        || visit(
                            module,
                            child,
                            active,
                            complete,
                            references,
                            shared_view_members,
                        )
                }
                // A borrowed view is a relocated descriptor, not owned leaf
                // storage: records carrying one stay plain but may be locally
                // constructed, where the binding roster checks the loan source.
                StructuralFieldType::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
                )
                | StructuralFieldType::PredicateByteSequence {
                    carrier: terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
                    ..
                } => references || shared_view_members,
                StructuralFieldType::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                )
                | StructuralFieldType::PredicateByteSequence {
                    carrier: terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                    ..
                } => true,
                _ => field.field_type.scalar_type().is_some(),
            }
    }
    fn visit(
        module: &TerminalModule,
        root: StructuralTypeId,
        active: &mut Vec<StructuralTypeId>,
        complete: &mut BTreeSet<StructuralTypeId>,
        references: bool,
        shared_view_members: bool,
    ) -> bool {
        if complete.contains(&root) {
            return true;
        }
        if active.contains(&root) {
            return false;
        }
        let mut candidates = module
            .structural_types
            .iter()
            .filter(|item| item.id == root);
        let Some(declaration) = candidates.next() else {
            return false;
        };
        if candidates.next().is_some() {
            return false;
        }
        active.push(root);
        // A plain or constructible owned type keeps owned storage at every
        // leaf: records and mixed sums carry their fields, sums carry their
        // case payloads, and a fixed array is its element leaf repeated.
        // Borrowed carriers (references, byte sequences, element views) stay
        // rejected — owning or returning them rides the borrowing lattice,
        // not this one.
        let valid = match &declaration.shape {
            StructuralTypeShape::Record { fields } => fields.iter().all(|field| {
                field_valid(
                    module,
                    field,
                    active,
                    complete,
                    references,
                    shared_view_members,
                )
            }),
            StructuralTypeShape::Mixed { fields, cases } => {
                fields.iter().all(|field| {
                    field_valid(
                        module,
                        field,
                        active,
                        complete,
                        references,
                        shared_view_members,
                    )
                }) && cases.iter().all(|case| {
                    case.fields.iter().all(|field| {
                        field_valid(
                            module,
                            field,
                            active,
                            complete,
                            references,
                            shared_view_members,
                        )
                    })
                })
            }
            StructuralTypeShape::Sum { cases } => cases.iter().all(|case| {
                case.fields.iter().all(|field| {
                    field_valid(
                        module,
                        field,
                        active,
                        complete,
                        references,
                        shared_view_members,
                    )
                })
            }),
            StructuralTypeShape::FixedArray { element, .. } => visit(
                module,
                *element,
                active,
                complete,
                references,
                shared_view_members,
            ),
            StructuralTypeShape::PrimitiveScalar(_) => true,
            _ => false,
        };
        active.pop();
        if valid {
            complete.insert(root);
        }
        valid
    }
    visit(
        module,
        root,
        &mut Vec::new(),
        &mut BTreeSet::new(),
        references,
        shared_view_members,
    )
}

pub(crate) fn fields<'a>(
    module: &'a TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<&'a [StructuralFieldDeclaration], ModuleError> {
    let failure = || ModuleError::RecordResultMismatch(operation.id);
    let OperationKind::EstablishRecord { fields } = &operation.kind else {
        return Err(failure());
    };
    let result = operation.result.structural().ok_or_else(failure)?;
    let exact_place = machine.structural_places.iter().any(|place| {
        place.id == result.place
            && matches!(place.kind,
            StructuralPlaceKind::OperationResult { producer, structural_type }
                if producer == operation.id && structural_type == result.structural_type)
    });
    let owned = matches!(
        result.multiplicity,
        StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
    );
    if !owned
        || !exact_place
        || !constructible_type(module, result.structural_type)
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
    {
        return Err(failure());
    }
    let declaration = module
        .structural_types
        .iter()
        .find(|item| item.id == result.structural_type)
        .ok_or_else(failure)?;
    let StructuralTypeShape::Record {
        fields: declarations,
    } = &declaration.shape
    else {
        return Err(failure());
    };
    if declarations.len() != fields.len() {
        return Err(failure());
    }
    for (declaration, binding) in declarations.iter().zip(fields) {
        if declaration.id != binding.field {
            return Err(failure());
        }
        match (&declaration.field_type, &binding.value) {
            (field_type, RecordFieldValue::Structural(argument))
                if matches!(
                    field_type.byte_sequence_carrier(),
                    Some(terminal_psi::ByteSequenceCarrier::BoundedOwned { .. })
                ) =>
            {
                let source =
                    super::structural::result_contracts::source_signature(machine, argument.place)
                        .ok_or_else(failure)?;
                let carrier = module
                    .structural_types
                    .iter()
                    .find(|candidate| candidate.id == source.structural_type)
                    .ok_or_else(failure)?;
                let predicates = terminal_semantics::byte_qualification_predicates_in(
                    module.structural_domains.iter(),
                    source.structural_type,
                    source.qualifications,
                )
                .ok_or_else(failure)?;
                if argument.access != StructuralAccess::Owned
                    || !argument.path.is_empty()
                    || argument.place == result.place
                    || source.multiplicity != StructuralMultiplicity::Unrestricted
                    || !source.projected_qualifications.is_empty()
                    || carrier.shape
                        != StructuralTypeShape::ByteSequence(
                            field_type.byte_sequence_carrier().ok_or_else(failure)?,
                        )
                    || !predicates.implies(field_type.byte_predicates())
                    || machine
                        .structural_parameters
                        .iter()
                        .chain(
                            machine
                                .blocks
                                .iter()
                                .flat_map(|block| &block.structural_parameters),
                        )
                        .any(|parameter| {
                            parameter.place == argument.place
                                && parameter.access != StructuralAccess::Owned
                        })
                    || machine
                        .blocks
                        .iter()
                        .flat_map(|block| &block.operations)
                        .any(|producer| {
                            producer.result.structural().is_some_and(|source| {
                                source.place == argument.place
                                    && (!source.claims.is_empty()
                                        || !source.qualification_establishments.is_empty())
                            })
                        })
                {
                    return Err(failure());
                }
            }
            (StructuralFieldType::Structural(expected), RecordFieldValue::Structural(argument)) => {
                let source =
                    super::structural::result_contracts::source_signature(machine, argument.place)
                        .ok_or_else(failure)?;
                if argument.access != StructuralAccess::Owned
                    || !argument.path.is_empty()
                    || argument.place == result.place
                    || source.structural_type != *expected
                    || !source.qualifications.is_empty()
                    || !source.projected_qualifications.is_empty()
                    || !matches!(
                        source.multiplicity,
                        StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
                    )
                    || (result.multiplicity == StructuralMultiplicity::Unrestricted
                        && source.multiplicity != StructuralMultiplicity::Unrestricted)
                    || machine
                        .structural_parameters
                        .iter()
                        .chain(
                            machine
                                .blocks
                                .iter()
                                .flat_map(|block| &block.structural_parameters),
                        )
                        .any(|parameter| {
                            parameter.place == argument.place
                                && parameter.access != StructuralAccess::Owned
                        })
                    || machine
                        .blocks
                        .iter()
                        .flat_map(|block| &block.operations)
                        .any(|producer| {
                            producer.result.structural().is_some_and(|source| {
                                source.place == argument.place && !source.claims.is_empty()
                            })
                        })
                {
                    return Err(failure());
                }
            }
            (
                StructuralFieldType::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
                ),
                RecordFieldValue::Structural(argument),
            ) => {
                // The field owns a copy of an existing shared-view descriptor:
                // the source must name a borrowed view, arrive whole, and come
                // from a place the machine may read and share.
                let source =
                    super::structural::result_contracts::source_signature(machine, argument.place)
                        .ok_or_else(failure)?;
                let shared_view = module.structural_types.iter().any(|item| {
                    item.id == source.structural_type
                        && matches!(
                            item.shape,
                            StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                            )
                        )
                });
                if argument.access != StructuralAccess::Owned
                    || !argument.path.is_empty()
                    || argument.place == result.place
                    || !shared_view
                    || !source.qualifications.is_empty()
                    || !source.projected_qualifications.is_empty()
                    || !matches!(
                        source.multiplicity,
                        StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
                    )
                    || (result.multiplicity == StructuralMultiplicity::Unrestricted
                        && source.multiplicity != StructuralMultiplicity::Unrestricted)
                    || machine
                        .structural_parameters
                        .iter()
                        .chain(
                            machine
                                .blocks
                                .iter()
                                .flat_map(|block| &block.structural_parameters),
                        )
                        .any(|parameter| {
                            parameter.place == argument.place
                                && !matches!(
                                    parameter.access,
                                    StructuralAccess::Owned | StructuralAccess::SharedBorrow
                                )
                        })
                    || machine
                        .blocks
                        .iter()
                        .flat_map(|block| &block.operations)
                        .any(|producer| {
                            producer.result.structural().is_some_and(|source| {
                                source.place == argument.place && !source.claims.is_empty()
                            })
                        })
                {
                    return Err(failure());
                }
            }
            (
                field_type,
                RecordFieldValue::Scalar {
                    range_obligation, ..
                },
            ) if field_type.scalar_type().is_some() => {
                if matches!(field_type, StructuralFieldType::BoundedInteger(_))
                    != range_obligation.is_some()
                {
                    return Err(failure());
                }
            }
            _ => return Err(failure()),
        }
    }
    Ok(declarations)
}

pub(super) fn operands(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let declarations = fields(module, machine, operation)?;
    let OperationKind::EstablishRecord { fields } = &operation.kind else {
        unreachable!()
    };
    for (declaration, binding) in declarations.iter().zip(fields) {
        if let RecordFieldValue::Scalar { value, .. } = binding.value {
            super::operations::require_defined(value, value_types, defined)?;
            if declaration.field_type.scalar_type() != value_types.get(&value).copied() {
                return Err(ModuleError::RecordResultMismatch(operation.id));
            }
        }
    }
    Ok(())
}

/// A completed plain value is available at its producing operation, not a
/// disposal obligation merely because it came from a storage window. Extracting
/// a field still opens independently tracked restoration debt on the borrowed
/// owner. Once repaired, an unused unrestricted displaced value cannot prevent
/// a join; affine results still remain in the ownership frontier. Reads of
/// either result must retain this producer's dominance and operation order.
pub(super) fn completed_source<'a>(
    module: &TerminalModule,
    machine: &'a TerminalMachine,
    place: PlaceId,
) -> Option<&'a terminal_psi::StructuralOperationResult> {
    completed_source_where(module, machine, place, plain_type, false)
}

/// A call-scoped loan retains its owned carrier, including any shared
/// descriptors stored within it. Lending that carrier mutably permits replacing
/// a descriptor slot; it does not grant writes to the descriptor's backing.
/// The producer and live-loan replays separately check establishment and every
/// retained backing region. Owned transfers keep `completed_source`'s plain
/// payload contract.
pub(super) fn loan_source<'a>(
    module: &TerminalModule,
    machine: &'a TerminalMachine,
    place: PlaceId,
) -> Option<&'a terminal_psi::StructuralOperationResult> {
    completed_source_where(module, machine, place, shared_view_carrier_type, false)
}

/// An owned local under memberwise establishment is a writable destination
/// root before its members complete: the establishment declares the root's
/// type and custody, and the member operations finish it. It is not a
/// completed source — nothing may read or move a member the stores have not
/// established — but every member store resolves its root through here.
pub(super) fn established_local_source<'a>(
    module: &TerminalModule,
    machine: &'a TerminalMachine,
    place: PlaceId,
) -> Option<&'a terminal_psi::StructuralOperationResult> {
    let declaration = machine
        .structural_places
        .iter()
        .find(|declaration| declaration.id == place)?;
    let StructuralPlaceKind::OperationResult { producer, .. } = declaration.kind else {
        return None;
    };
    machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| {
            if operation.id != producer
                || !matches!(
                    operation.kind,
                    OperationKind::EstablishStructuralLocal { .. }
                )
            {
                return None;
            }
            operation.result.structural().filter(|result| {
                result.place == place
                    && matches!(
                        result.multiplicity,
                        StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
                    )
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.claims.is_empty()
                    && module
                        .structural_types
                        .iter()
                        .any(|row| row.id == result.structural_type)
            })
        })
}

/// The `completed_source` producer set under the constructible-type gate: a
/// record carrying borrowed-view members is no plain payload but a locally
/// constructed record still answers member reads — reading a scalar member
/// moves no view custody. Only scalar-field observation calls this; transfer
/// and claim paths keep the plain gate.
pub(super) fn constructed_source<'a>(
    module: &TerminalModule,
    machine: &'a TerminalMachine,
    place: PlaceId,
) -> Option<&'a terminal_psi::StructuralOperationResult> {
    completed_source_where(module, machine, place, constructible_type, false)
}

/// The `completed_source` producer set under the view-descriptor gate: a copy
/// of a stored `&[T]`/`&[u8]` member relocates the loan's descriptor only —
/// the referent stays with the loan source, so the payload is custody-free
/// the way a shared view itself is.
pub(super) fn shared_view_member_source<'a>(
    module: &TerminalModule,
    machine: &'a TerminalMachine,
    place: PlaceId,
) -> Option<&'a terminal_psi::StructuralOperationResult> {
    completed_source_where(
        module,
        machine,
        place,
        |module, root| {
            module.structural_types.iter().any(|row| {
                row.id == root
                    && match &row.shape {
                        StructuralTypeShape::ElementView { .. } => true,
                        StructuralTypeShape::ByteSequence(carrier) => carrier.is_borrowed(),
                        _ => false,
                    }
            })
        },
        true,
    )
}

fn completed_source_where<'a>(
    module: &TerminalModule,
    machine: &'a TerminalMachine,
    place: PlaceId,
    payload: impl Fn(&TerminalModule, StructuralTypeId) -> bool,
    strict_payload: bool,
) -> Option<&'a terminal_psi::StructuralOperationResult> {
    machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| {
            if !matches!(
                operation.kind,
                OperationKind::EstablishRecord { .. }
                    | OperationKind::EstablishZeroedStructural { .. }
                    | OperationKind::EstablishStructuralLocal { .. }
                    | OperationKind::MoveStructuralField { .. }
                    | OperationKind::EstablishStructuralCase { .. }
                    | OperationKind::CallStructural { .. }
                    | OperationKind::CallStructuralWithScalarArguments { .. }
                    | OperationKind::StructuralLeafCopy { .. }
                    | OperationKind::StructuralCaseLeafCopy { .. }
            ) {
                return None;
            }
            operation.result.structural().filter(|result| {
                result.place == place
                    && matches!(
                        result.multiplicity,
                        StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
                    )
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.claims.is_empty()
                    && (payload(module, result.structural_type)
                        || (!strict_payload
                            // Zero establishment also owns bounded byte fields
                            // whose declared predicates admit an empty live range.
                            // Rejoin that constructor's checked contract instead
                            // of treating all byte-bearing records as constructible.
                            && (matches!(operation.kind, OperationKind::EstablishZeroedStructural { .. })
                                && super::structural::operations::validate_establish_zeroed_structural(
                                    module, machine, operation,
                                ).is_ok())
                            // A verified copy or ordinary call preserves the exact owned subtree,
                            // including bounded byte fields' live extent and
                            // predicates. This admits completed snapshots, not
                            // new byte-field constructors or borrowed carriers.
                            || (result.multiplicity == StructuralMultiplicity::Unrestricted
                                && matches!(
                                    operation.kind,
                                    OperationKind::StructuralLeafCopy { .. }
                                        | OperationKind::StructuralCaseLeafCopy { .. }
                                        | OperationKind::CallStructural { .. }
                                        | OperationKind::CallStructuralWithScalarArguments { .. }
                                )
                                && super::structural::result_contracts::has_plain_owned_shape(
                                    module,
                                    result.structural_type,
                                ))))
            })
        })
}

pub(super) fn validate_uses(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    available: &BTreeSet<PlaceId>,
) -> Result<(), ModuleError> {
    let validate = |place| {
        if loan_source(module, machine, place).is_some() && !available.contains(&place) {
            Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                operation: operation.id,
                place,
            })
        } else {
            Ok(())
        }
    };
    // `frontier::validate_owned_reads` exempts copy payloads from the owned
    // frontier because consuming one leaves it intact. Their producer's
    // dominance is checked here instead: a record or case never consumes a
    // plain scalar or nested case or array whose establishment has not run on every
    // path to it. Machine parameters are live at entry and are not
    // operation results.
    let copy_payload = |place| {
        let produced = !machine
            .structural_parameters
            .iter()
            .any(|parameter| parameter.place == place);
        if produced
            && ((super::scalar::case::plain_return_source(module, machine, place)
                || super::structural::case::plain_return_source(module, machine, place))
                || super::scalar::array::plain_return_source(module, machine, place))
            && !available.contains(&place)
        {
            Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                operation: operation.id,
                place,
            })
        } else {
            Ok(())
        }
    };
    match &operation.kind {
        OperationKind::EstablishRecord { fields }
        | OperationKind::EstablishStructuralCase { fields, .. } => {
            for field in fields {
                if let RecordFieldValue::Structural(argument) = &field.value {
                    validate(argument.place)?;
                    copy_payload(argument.place)?;
                    if owned_byte_source(module, machine, argument.place)
                        && machine.structural_places.iter().any(|place| {
                            place.id == argument.place
                                && matches!(place.kind, StructuralPlaceKind::OperationResult { .. })
                        })
                        && !available.contains(&argument.place)
                    {
                        return Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                            operation: operation.id,
                            place: argument.place,
                        });
                    }
                }
            }
        }

        OperationKind::PrimitiveScalarRead { source, .. }
        | OperationKind::IntegerStructuralField { source, .. }
        | OperationKind::BooleanStructuralField { source, .. } => validate(*source)?,
        OperationKind::WriteOnlyPrimitiveStore { destination, .. }
        | OperationKind::StructuralScalarFieldStore { destination, .. }
        | OperationKind::StructuralByteSequenceFieldStore { destination, .. }
        | OperationKind::StructuralByteViewFieldStore { destination, .. }
        | OperationKind::AtomicAccess {
            place: destination, ..
        } => validate(*destination)?,
        OperationKind::StoreStructuralField {
            destination, value, ..
        } => {
            // Copy results carry no disposal debt, but restoring a field still
            // requires their producing operation to dominate this store.
            validate(*destination)?;
            validate(value.place)?;
            copy_payload(value.place)?;
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
        } => {
            for argument in structural_arguments {
                validate(argument.place)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// A byte child remains an ordinary unrestricted value. Its exact producer,
/// predicates and current availability are checked separately before binding.
pub(super) fn owned_byte_source(
    module: &TerminalModule,
    machine: &TerminalMachine,
    place: PlaceId,
) -> bool {
    super::structural::result_contracts::source_signature(machine, place).is_some_and(|source| {
        source.multiplicity == StructuralMultiplicity::Unrestricted
            && source.projected_qualifications.is_empty()
            && module.structural_types.iter().any(|candidate| {
                candidate.id == source.structural_type
                    && matches!(
                        candidate.shape,
                        StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                        )
                    )
            })
    })
}

/// A validated affine record establishment returns whole: `fields` has already
/// checked every binding, so reference-free records — plain payloads and
/// borrowed-view descriptor carriers alike — relocate into the caller.
/// Reference-containing results still ride the reference return lane.
pub(super) fn established_record_return_source(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: PlaceId,
) -> bool {
    machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation.kind, OperationKind::EstablishRecord { .. }))
        .filter_map(|operation| operation.result.structural())
        .any(|result| {
            result.place == source
                && result.multiplicity == StructuralMultiplicity::Affine
                && result.qualifications.is_empty()
                && result.projected_qualifications.is_empty()
                && result.claims.is_empty()
                && !super::references::contains_reference(module, result.structural_type)
        })
}

pub(super) fn plain_return_source(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: PlaceId,
) -> bool {
    completed_source(module, machine, source).is_some()
        || machine.structural_parameters.iter().any(|parameter| {
            parameter.place == source
                && parameter.access == StructuralAccess::Owned
                && matches!(
                    parameter.multiplicity,
                    StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
                )
                && parameter.qualifications.is_empty()
                && parameter.projected_qualifications.is_empty()
                && plain_type(module, parameter.structural_type)
                && !machine
                    .entry_claims
                    .iter()
                    .any(|claim| claim.input == source)
                && !machine
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == source)
        })
}
