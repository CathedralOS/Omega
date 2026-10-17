//! Type identities, structural access, and the type-shape predicates the Unit
//! plans key on.

use super::*;

pub(in crate::execution::terminal_unit) fn terminal_field_identity(
    program: &dyn ::validation::CheckingInput,
    symbol: SymbolHandle,
) -> Option<checked_trees::CheckedMemberIdentity> {
    terminal_field_declaration(program, symbol).map(|field| {
        checked_trees::CheckedMemberIdentity::of_member(field.identity, field.name.clone())
    })
}

/// Common and payload fields use the same declaration identity. The path's
/// Case segment, not a spelling search, supplies the payload namespace.
pub(crate) fn terminal_field_declaration(
    program: &dyn ::validation::CheckingInput,
    symbol: SymbolHandle,
) -> Option<&typed_trees::data::DataField> {
    if !symbol.is_valid() {
        return None;
    }
    let mut fields = program
        .data_definitions()
        .iter()
        .flat_map(|definition| program.data_member_fields(program.data_members(definition)))
        .filter(|field| field.symbol == symbol);
    let field = fields.next()?;
    fields.next().is_none().then_some(field)
}

pub(crate) fn is_reference(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> bool {
    validation::reference_shell(program, type_reference).is_some()
}

pub(in crate::execution) fn structural_access_for_type_reference(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<CheckedStructuralAccess> {
    let Some(shell) = validation::reference_shell(program, type_reference) else {
        return Some(CheckedStructuralAccess::Owned);
    };
    // A structural carrier represents one referent occurrence. Nested
    // reference values need a separate pointer-as-data shape and therefore
    // remain outside this rung.
    (!is_reference(program, shell.referee)).then(|| shell.access.into())
}

/// The identity a byte carrier's structural plan registers under, or `None`
/// when `type_reference` is not a byte carrier. The identity rule itself —
/// which shells a view, a bounded view and a bounded owner keep — belongs to
/// `validation::structural_type_identity`.
pub(crate) fn byte_sequence_type_identity(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Option<typed_trees::type_identity::TypeIdentityId> {
    byte_sequence_carrier(program, type_reference, substitutions)?;
    Some(validation::structural_type_identity(
        program,
        type_reference,
        binders,
        substitutions,
    ))
}

pub(in crate::execution) use validation::type_graph_requires_nominal_drop;

/// Join stored reference identity to its borrowed presentation without
/// erasing the source declaration. Access and custody remain the edge's checks.
pub(in crate::execution::terminal_unit) fn stored_type_presents_borrowed(
    program: &dyn ::validation::CheckingInput,
    source: TypeReferenceHandle,
    stored: typed_trees::type_identity::TypeIdentityId,
    borrowed: typed_trees::type_identity::TypeIdentityId,
) -> bool {
    validation::stored_structural_type_identity(program, source, &[], &[]) == stored
        && base_type_identity(program, source, &[], &[]) == Some(borrowed)
}

pub(in crate::execution::terminal_unit) fn is_unit(
    program: &dyn ::validation::CheckingInput,
    mut type_reference: TypeReferenceHandle,
) -> bool {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Unit => return true,
            _ => return false,
        }
    }
}

/// The structural plan identity a call operand, formal or result of
/// `type_reference` joins, offered only when the storage beneath its loan and
/// qualification shells is a nominal, a slice or a literal-extent array: the
/// carriers a call presents. Other storage declines. The identity itself is
/// the rule `validation::structural_type_identity` owns, so a bounded byte
/// owner keeps its qualification here exactly as its registered plan does.

/// The same identity under a call edge's checked generic substitutions: the
/// callee's `Type` parameters resolve to the actuals its admitted
/// specialization derived, so a requirement formal's identity compares
/// against the caller's concrete presentation.
pub(in crate::execution::terminal_unit) fn base_type_identity(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Option<typed_trees::type_identity::TypeIdentityId> {
    let storage = program
        .type_reference_table
        .peeled_type_reference(type_reference);
    matches!(
        program.type_reference_table.type_reference(storage),
        TypeReferenceNode::Named { .. }
            | TypeReferenceNode::Generic { .. }
            | TypeReferenceNode::Slice { .. }
            | TypeReferenceNode::FixedArray {
                length: typed_trees::types::FixedArrayLength::Literal(_),
                ..
            }
    )
    .then(|| validation::structural_type_identity(program, type_reference, binders, substitutions))
}

/// The structural identity of the storage a type lends: a loan's referent, or
/// the type itself when it is no loan. Two types lend the same storage when
/// these agree, whichever shell each spells — a `&mut [u8; N]` alias and the
/// `[u8; N]` it borrows, or a plain array and the `&[u8; N]` bounded view a
/// formal declares over it — while a qualified byte owner still lends
/// different storage from the plain array.
pub(in crate::execution::terminal_unit) fn lent_storage_identity(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Option<typed_trees::type_identity::TypeIdentityId> {
    let storage = validation::reference_shell(program, type_reference)
        .map_or(type_reference, |shell| shell.referee);
    base_type_identity(program, storage, &[], substitutions)
}

/// A call edge's checked specialization maps a bare `Type` formal to the
/// actual its admission derived before custody or shape evidence is
/// replayed against the caller's concrete values. A compound formal keeps
/// its own reference: the substitution only names a top-level `Type`
/// parameter, and identity-level substitution covers the generic arguments
/// inside it.
pub(in crate::execution::terminal_unit) fn substituted_formal_type(
    program: &dyn ::validation::CheckingInput,
    mut type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> TypeReferenceHandle {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Named { symbol, .. } => {
                let Some((_, actual)) = substitutions
                    .iter()
                    .rev()
                    .find(|(parameter, _)| parameter == symbol)
                else {
                    return type_reference;
                };
                if *actual == type_reference {
                    return type_reference;
                }
                type_reference = *actual;
            }
            _ => return type_reference,
        }
    }
}

pub(in crate::execution::terminal_unit) fn closed_data_identity(
    program: &dyn ::validation::CheckingInput,
    data: &typed_trees::data::DataDefinition,
    binders: &[(SymbolHandle, String)],
) -> typed_trees::type_identity::TypeIdentityId {
    // A synthesized owner's spelling is diagnostic metadata. Its attachment
    // and every ordinary field/reference use must retain the same original
    // generic application identity, including the exact argument tuple.
    if let Some(application) = data.generic_instance {
        return program.type_identity(TypeIdentityRequest {
            binders,
            ..TypeIdentityRequest::ordinary(application)
        });
    }
    program
        .normalized_nominal_type_identity(data.symbol)
        .unwrap_or_default()
}

pub(in crate::execution::terminal_unit) fn scalar_type(
    program: &dyn ::validation::CheckingInput,
    mut type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Option<PrimitiveType> {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Named { symbol, name } => {
                if let Some((_, replacement)) = substitutions
                    .iter()
                    .rev()
                    .find(|(parameter, _)| parameter == symbol)
                {
                    // A substitution can bind a parameter to its own
                    // reference; rewriting it again only loops.
                    if *replacement == type_reference {
                        return PrimitiveType::from_name(name.as_str());
                    }
                    type_reference = *replacement;
                    continue;
                }
                return PrimitiveType::from_name(name.as_str());
            }
            TypeReferenceNode::DynamicTrait { .. } => return Some(PrimitiveType::Selector),
            _ => return None,
        }
    }
}
