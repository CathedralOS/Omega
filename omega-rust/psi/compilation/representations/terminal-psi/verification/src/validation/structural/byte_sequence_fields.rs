//! Exact bounded field metadata and indexed mutation custody.

use crate::validation::{
    IntegerSign, IntegerType, ModuleError, OperationKind, OperationResult, PlaceId, ScalarType,
    StructuralAccess, StructuralFieldId, StructuralFieldType, StructuralPathSegment,
    StructuralPlaceKind, StructuralTypeShape, TerminalMachine, TerminalModule,
    resolve_structural_path,
};
mod freshness;
pub(crate) use freshness::field_content_length_is_current;
pub(crate) use freshness::literal_field_read_bounds;
pub(crate) use freshness::replacement_length_equation;
pub(crate) use freshness::view_length_is_current;
pub(crate) use freshness::{owned_content_is_ascii, owned_content_is_current};

pub(in crate::validation) fn validate(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<(), ModuleError> {
    let invalid = || ModuleError::InvalidStructuralByteSequenceFieldAccess(operation.id);
    let (root, path, field, writing, element_result) = match &operation.kind {
        OperationKind::StructuralByteSequenceFieldLength {
            source,
            path,
            field,
        } => (*source, path, *field, false, false),
        OperationKind::StructuralByteSequenceFieldByteStore {
            destination,
            path,
            field,
            ..
        } => (*destination, path, *field, true, false),
        OperationKind::StructuralByteSequenceFieldRead {
            source,
            path,
            field,
            ..
        } => (*source, path, *field, false, true),
        _ => return Err(invalid()),
    };
    let (_, access) = terminal_semantics::byte_field_root(machine, root).ok_or_else(invalid)?;
    let access_admitted = if writing {
        matches!(
            access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        ) && machine.structural_places.iter().any(|place| {
            place.id == root && matches!(place.kind, StructuralPlaceKind::Parameter { .. })
        })
    } else {
        matches!(
            access,
            StructuralAccess::Owned
                | StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
        ) || (!element_result && access == StructuralAccess::WriteOnlyBorrow)
    };
    if !access_admitted || !terminal_psi::is_bounded_structural_scalar_store_path(path) {
        return Err(invalid());
    }
    field_path(
        module,
        machine,
        root,
        path,
        field,
        writing,
        !writing && !element_result,
    )
    .ok_or_else(invalid)?;
    if writing {
        if operation.result != OperationResult::Unit
            || !freshness::exact_length_is_current(module, machine, operation)
            || !store_preserves_field_predicates(module, machine, operation)
        {
            return Err(invalid());
        }
    } else if element_result {
        // The element bound must discharge against the field's own current
        // length observation, not a stale snapshot.
        if operation
            .result
            .scalar()
            .is_none_or(|result| result.scalar_type != element_type())
            || !freshness::exact_length_is_current(module, machine, operation)
        {
            return Err(invalid());
        }
    } else if operation
        .result
        .scalar()
        .is_none_or(|result| result.scalar_type != byte_count_type())
    {
        return Err(invalid());
    }
    Ok(())
}

/// A length observation snapshots a bounded owner's live extent. Its declared
/// capacity is an upper bound, never an equation for the current length. The
/// bound belongs to the scalar snapshot and remains true after later writes.
pub(crate) fn length_capacity_bound(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<Option<semantic_vocabulary::Proposition>, ModuleError> {
    let OperationKind::StructuralByteSequenceFieldLength {
        source,
        path,
        field,
    } = &operation.kind
    else {
        return Ok(None);
    };
    validate(module, machine, operation)?;
    let invalid = || ModuleError::InvalidStructuralByteSequenceFieldAccess(operation.id);
    let (structural_type, _) =
        terminal_semantics::byte_field_root(machine, *source).ok_or_else(invalid)?;
    let carrier = resolve_structural_path(module, structural_type, path).ok_or_else(invalid)?;
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == carrier)
        .ok_or_else(invalid)?;
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return Err(invalid());
    };
    let declaration = fields
        .iter()
        .find(|candidate| candidate.id == *field)
        .ok_or_else(invalid)?;
    let Some(terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity }) =
        declaration.field_type.byte_sequence_carrier()
    else {
        return Ok(None);
    };
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid");
    let bound = semantic_vocabulary::ScalarTerm::integer(
        integer_type,
        semantic_vocabulary::IntegerValue::Unsigned(u128::from(capacity)),
    )
    .map_err(|error| {
        ModuleError::OperationSemanticSchema(
            terminal_semantics::OperationSemanticError::InvalidProposition(error),
        )
    })?;
    Ok(Some(semantic_vocabulary::Proposition::LessOrEqual(
        semantic_vocabulary::ScalarTerm::value(
            operation.result.expect_scalar().id,
            byte_count_type(),
        ),
        bound,
    )))
}

/// The declared byte predicates of an indexed store's exact destination field.
/// A plain carrier declares none. `None` means the declaration does not resolve.
fn stored_field_predicates(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Option<terminal_psi::ByteSequencePredicateSet> {
    let OperationKind::StructuralByteSequenceFieldByteStore {
        destination,
        path,
        field,
        ..
    } = &operation.kind
    else {
        return None;
    };
    let parameter = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == *destination)?;
    let carrier = resolve_structural_path(module, parameter.structural_type, path)?;
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == carrier)?;
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return None;
    };
    let field = fields
        .iter()
        .find(|candidate| candidate.id == *field && !candidate.relevance.is_erased())?;
    field.field_type.byte_sequence_carrier()?;
    Some(field.field_type.byte_predicates())
}

/// Replacing one byte of a predicate-bearing field must leave every declared
/// predicate true, because Terminal Psi keeps a field's declared predicates
/// true after every write: calls and reads trust the declaration, and there
/// is no invariant window to reopen here.
///
/// The store keeps the extent, so `non_empty` survives it. The per-byte
/// classes `ascii_only` and `no_nul` survive when the carrier already held
/// them -- the declaration guarantees that -- and the stored byte is in the
/// class, which the store's goal proves ([`byte_store_goal`]). `valid_utf8`
/// is not per-byte: an ASCII byte written over part of a multi-byte scalar
/// breaks it. It survives only when the carrier is all-ASCII before the
/// store, so it holds no multi-byte scalar to split. A field that also
/// declares `ascii_only` guarantees that; otherwise every path into the store
/// must reestablish it ([`freshness::field_content_is_ascii`]).
fn store_preserves_field_predicates(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> bool {
    use terminal_psi::ByteSequencePredicate;
    let Some(predicates) = stored_field_predicates(module, machine, operation) else {
        return false;
    };
    let mut ascii = terminal_psi::ByteSequencePredicateSet::EMPTY;
    ascii.insert(ByteSequencePredicate::AsciiOnly);
    predicates.predicates().all(|predicate| match predicate {
        ByteSequencePredicate::NonEmpty
        | ByteSequencePredicate::NoNul
        | ByteSequencePredicate::AsciiOnly => true,
        ByteSequencePredicate::ValidUtf8 => {
            predicates.implies(ascii)
                || freshness::field_content_is_ascii(module, machine, operation)
        }
    })
}

/// The indexed store's complete goal: the operand-only `index < length`
/// bound plus the stored byte's class legs, which the exact destination
/// declaration requires ([`terminal_semantics::indexed_byte_store_goal`]).
/// Reconstructing them here means a producer cannot weaken them.
pub(crate) fn byte_store_goal(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    bound: semantic_vocabulary::Proposition,
) -> Result<semantic_vocabulary::Proposition, ModuleError> {
    let OperationKind::StructuralByteSequenceFieldByteStore { value, .. } = &operation.kind else {
        return crate::validation::byte_sequence::write::byte_store_goal(
            module, machine, operation, bound,
        );
    };
    let predicates = stored_field_predicates(module, machine, operation).ok_or(
        ModuleError::InvalidStructuralByteSequenceFieldAccess(operation.id),
    )?;
    terminal_semantics::indexed_byte_store_goal(bound, *value, predicates).map_err(|error| {
        ModuleError::OperationSemanticSchema(
            terminal_semantics::OperationSemanticError::InvalidProposition(error),
        )
    })
}

fn byte_count_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"))
}

fn element_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 is valid"))
}

/// Resolve the field identity before comparing a call's projected write path.
/// `extent_observation` names the length operation's read-only extent view: a
/// shared element-view field admits an extent observation (its descriptor's
/// stored element count) but never a byte-content access through this family.
fn field_path(
    module: &TerminalModule,
    machine: &TerminalMachine,
    root: PlaceId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    writing: bool,
    extent_observation: bool,
) -> Option<Vec<StructuralPathSegment>> {
    let structural_type = if writing {
        machine
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == root)?
            .structural_type
    } else {
        terminal_semantics::byte_field_root(machine, root)?.0
    };
    let carrier = resolve_structural_path(module, structural_type, path)?;
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == carrier)?;
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return None;
    };
    let field = fields
        .iter()
        .find(|candidate| candidate.id == field && !candidate.relevance.is_erased())?;
    // A borrowed-view field's length rides the view descriptor, so a read is
    // admissible whatever the view's access. A store needs write authority,
    // and the carrier now states it: the exclusive accesses admit one, a
    // shared view does not. This is read from the field declaration rather
    // than taken from a producer's mint gate, which is not verifier authority.
    // A predicate-bearing field resolves like its carrier: the path is the
    // same storage. Whether a write into it preserves the declared
    // predicates is the writing operation's own rule, never this lookup's.
    let carrier_admitted = match field.field_type {
        StructuralFieldType::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned {
            ..
        })
        | StructuralFieldType::PredicateByteSequence {
            carrier: terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
            ..
        } => true,
        StructuralFieldType::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
            access,
        }) => {
            !writing
                || matches!(
                    access,
                    Some(
                        terminal_psi::StructuralAccess::MutableBorrow
                            | terminal_psi::StructuralAccess::WriteOnlyBorrow
                    )
                )
        }
        // A qualified shared view field is read like its plain carrier; its
        // descriptor changes only through `StructuralByteViewFieldStore`,
        // whose source predicates imply the declaration.
        StructuralFieldType::PredicateByteSequence {
            carrier:
                terminal_psi::ByteSequenceCarrier::BorrowedView {
                    access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                },
            ..
        } => !writing,
        StructuralFieldType::Structural(view) if extent_observation => {
            module.structural_types.iter().any(|declaration| {
                declaration.id == view
                    && matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
            })
        }
        _ => false,
    };
    if !carrier_admitted {
        return None;
    }
    let mut exact = path.to_vec();
    exact.push(StructuralPathSegment::Field(field.identity.clone()));
    Some(exact)
}
