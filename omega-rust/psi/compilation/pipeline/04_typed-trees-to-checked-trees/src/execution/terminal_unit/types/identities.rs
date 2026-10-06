//! Type identities, structural access, and the type-shape predicates the Unit
//! plans key on.

use super::*;

pub(in crate::execution::terminal_unit) fn terminal_field_identity(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<String> {
    program.data_definitions().iter().find_map(|definition| {
        program.data_members(definition).iter().find_map(|member| {
            let typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            (field.symbol == symbol).then(|| field.path_identity())
        })
    })
}

pub(crate) fn is_reference(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    validation::reference_shell(program, type_reference).is_some()
}

pub(in crate::execution::terminal_unit) fn structural_access_for_type_reference(
    program: &TypedTrees,
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

/// Shared references to this existing whole affine carrier do not own or
/// qualify the referent. Nested references and constrained carriers stay out.
pub(in crate::execution::terminal_unit) fn shared_plain_affine_referent(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    let TypeReferenceNode::Reference {
        access: language_semantics::ReferenceAccess::Shared,
        referee,
        ..
    } = program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    (program.type_multiplicity(*referee) == Multiplicity::Affine
        && has_contents(
            program,
            *referee,
            validation::ContentsRequirement::PlainOwned,
        ))
    .then_some(*referee)
}

/// Shared references to an existing whole unrestricted carrier, such as the
/// `&u8` a foreign `const char*` parameter lowers to, lend the caller's
/// referent for the call's duration exactly as the affine form above does:
/// the callee reads through the loan, and the caller keeps custody of the
/// referent. An unrestricted referent adds no ownership question the affine
/// one lacks; it is kept separate only because ordinary call admission
/// consumes the affine form with its own closed-referent companion.
/// Nested references and constrained carriers stay out.
pub(in crate::execution::terminal_unit) fn shared_plain_unrestricted_referent(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    let TypeReferenceNode::Reference {
        access: language_semantics::ReferenceAccess::Shared,
        referee,
        ..
    } = program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    (program.type_multiplicity(*referee) == Multiplicity::Unrestricted
        && has_contents(
            program,
            *referee,
            validation::ContentsRequirement::PlainOwned,
        ))
    .then_some(*referee)
}

/// Exclusive references to this existing whole non-linear carrier do not own
/// or qualify the referent either: the boundary writes back through them as an
/// out-parameter, and custody of the referent stays with the caller. Linear
/// referents, nested references, and constrained carriers stay out.
pub(in crate::execution::terminal_unit) fn mutable_plain_nonlinear_referent(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    let TypeReferenceNode::Reference {
        access: language_semantics::ReferenceAccess::Mutable,
        referee,
        ..
    } = program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    (program.type_multiplicity(*referee) != Multiplicity::Linear
        && has_contents(
            program,
            *referee,
            validation::ContentsRequirement::PlainOwned,
        ))
    .then_some(*referee)
}

pub(crate) fn byte_sequence_type_identity(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Option<String> {
    let carrier = byte_sequence_carrier(program, type_reference, substitutions)?;
    let mut identity_type = type_reference;
    if carrier.is_borrowed_view()
        || matches!(
            carrier,
            checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. }
        )
    {
        while let TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } = program.type_reference_table.type_reference(identity_type)
        {
            identity_type = *referee;
        }
    }
    Some(
        program
            .type_identity(TypeIdentityRequest {
                binders,
                substitutions,
                ..TypeIdentityRequest::ordinary(identity_type)
            })
            .into_string(),
    )
}

pub(in crate::execution) use validation::type_graph_requires_nominal_drop;

pub(in crate::execution::terminal_unit) fn is_unit(
    program: &TypedTrees,
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

pub(in crate::execution::terminal_unit) fn base_type_identity(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
) -> Option<String> {
    base_type_identity_with_substitutions(program, type_reference, binders, &[])
}

/// The same base identity under a call edge's checked generic
/// substitutions: the callee's `Type` parameters resolve to the actuals its
/// admitted specialization derived, so a requirement formal's identity
/// compares against the caller's concrete presentation.
pub(in crate::execution::terminal_unit) fn base_type_identity_with_substitutions(
    program: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Option<String> {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. }
            | TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => type_reference = *referee,
            TypeReferenceNode::Named { .. }
            | TypeReferenceNode::Generic { .. }
            | TypeReferenceNode::Slice { .. }
            | TypeReferenceNode::FixedArray {
                length: typed_trees::types::FixedArrayLength::Literal(_),
                ..
            } => {
                return Some(
                    program
                        .type_identity(TypeIdentityRequest {
                            binders,
                            substitutions,
                            ..TypeIdentityRequest::ordinary(type_reference)
                        })
                        .into_string(),
                );
            }
            _ => return None,
        }
    }
}

/// A call edge's checked specialization maps a bare `Type` formal to the
/// actual its admission derived before custody or shape evidence is
/// replayed against the caller's concrete values. A compound formal keeps
/// its own reference: the substitution only names a top-level `Type`
/// parameter, and identity-level substitution covers the generic arguments
/// inside it.
pub(in crate::execution::terminal_unit) fn substituted_formal_type(
    program: &TypedTrees,
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
    program: &TypedTrees,
    data: &typed_trees::data::DataDefinition,
    binders: &[(SymbolHandle, String)],
) -> String {
    // A synthesized owner's spelling is diagnostic metadata. Its attachment
    // and every ordinary field/reference use must retain the same original
    // generic application identity, including the exact argument tuple.
    if let Some(application) = data.generic_instance {
        return program
            .type_identity(TypeIdentityRequest {
                binders,
                ..TypeIdentityRequest::ordinary(application)
            })
            .into_string();
    }
    let path = program.symbols.display_path(data.symbol, "::");
    format!("named({})", normalized_atom("name", &path))
}

pub(in crate::execution::terminal_unit) fn scalar_type(
    program: &TypedTrees,
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
            _ => return None,
        }
    }
}

pub(in crate::execution::terminal_unit) fn normalized_atom(tag: &str, value: &str) -> String {
    let mut output = String::with_capacity(tag.len() + value.len() + 2);
    output.push_str(tag);
    output.push('(');
    for character in value.chars() {
        if matches!(character, '\\' | '(' | ')' | ',') {
            output.push('\\');
        }
        output.push(character);
    }
    output.push(')');
    output
}
