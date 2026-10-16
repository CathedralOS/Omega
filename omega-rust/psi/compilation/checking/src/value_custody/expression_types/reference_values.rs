//! Declared projected reference values share exact root and field selection.
//!
//! Type matching does not manufacture a loan. Forwarding additionally anchors
//! the current-state binding and checks every enclosing reference permission;
//! a mutable leaf behind shared storage cannot authorize an exclusive call.
use super::{
    ExpressionHandle, ExpressionNode, TypeReferenceHandle, TypeReferenceNode,
    named_value_type_reference,
};
use language_semantics::ReferenceAccess;
use numerics::bignum::BigInt;
use typed_trees::data::TypeParameterKind;
use typed_trees::type_identity::TypeIdentityRequest;
use typed_trees::types::{FixedArrayLength, TypeConstraintNode};

/// Reference correspondence does not form a loan. Array-to-slice adaptation
/// retains element identity and readable access; callers still establish the
/// original backing, extent and permission. Write-only attenuation is explicit.
pub(super) fn reference_type_matches(
    program: &dyn crate::CheckingInput,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
    required_substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
) -> bool {
    let (required, required_substitutions) =
        substituted_required_reference(program, required, required_substitutions);
    let TypeReferenceNode::Reference {
        access: actual_access,
        referee: actual_referee,
        ..
    } = program.type_reference_table.type_reference(actual)
    else {
        return false;
    };
    let TypeReferenceNode::Reference {
        access: required_access,
        referee: required_referee,
        ..
    } = program.type_reference_table.type_reference(required)
    else {
        return false;
    };
    let actual_identity = |reference| {
        program.type_identity(TypeIdentityRequest {
            substitutions,
            ..TypeIdentityRequest::ordinary(reference)
        })
    };
    if actual_identity(actual)
        == program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
            substitutions: required_substitutions,
            ..TypeIdentityRequest::ordinary(required)
        })
    {
        return true;
    }
    if *actual_access == ReferenceAccess::WriteOnly
        || *required_access == ReferenceAccess::WriteOnly
        || (*actual_access != *required_access
            && !(*actual_access == ReferenceAccess::Mutable
                && *required_access == ReferenceAccess::Shared))
    {
        return false;
    }
    actual_identity(*actual_referee) == program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
            substitutions: required_substitutions,
            ..TypeIdentityRequest::ordinary(*required_referee)
        })
        // A shared view may forget a carrier predicate without changing its
        // contents. A raw mutable view could violate that predicate; retain
        // rejection until its invariant-window obligations are established.
        || ((*required_access == ReferenceAccess::Shared
            || matches!(
                program.type_reference_table.type_reference(*actual_referee),
                TypeReferenceNode::FixedArray { .. }
            ))
            && owned_array_projects_to_slice(
                program,
                *actual_referee,
                *required_referee,
                substitutions,
                required_substitutions,
            ))
        // The required referee's own constraints remain the callee's claim:
        // an owned constrained array satisfies `&[u8] in D` only when its
        // declared domains imply each constraint the view keeps. The access
        // pair above already admits only same-access or mutable-to-shared
        // loans, so a mutable view obligation lands on a mutable source.
        || owned_array_satisfies_constrained_referee(
            program,
            *actual_referee,
            *required_referee,
            substitutions,
            required_substitutions,
        )
        // The same forget covers a constrained referee that is not an owned
        // array: `&[u8] in D` spells `Reference { Constrained { Slice } }`, so
        // unwrapping the qualification leaves the identical slice a shared
        // parameter asks for. Only the actual side strips — the required
        // referee's own constraints remain the callee's claim.
        || (*required_access == ReferenceAccess::Shared
            && constrained_referee_forgets_qualification(
                program,
                *actual_referee,
                *required_referee,
                substitutions,
                required_substitutions,
            ))
        || fixed_array_binds_open_length(
            program,
            *actual_referee,
            *required_referee,
            substitutions,
            required_substitutions,
        )
}

/// A reference argument relation selected by the evidence available to its
/// consumer. Ordinary calls prove qualifications from flow facts; logical
/// formation and post-check consumers retain declaration correspondence.
pub(super) type ReferenceCorrespondence = fn(
    &dyn crate::CheckingInput,
    TypeReferenceHandle,
    TypeReferenceHandle,
    &[(symbols::SymbolHandle, TypeReferenceHandle)],
    &[(symbols::SymbolHandle, TypeReferenceHandle)],
) -> bool;

/// Call carrier correspondence is used only before independent flow contract
/// checking. Shared views can use live qualification evidence even when their
/// declaration is bare; mutable invariants keep declaration correspondence.
pub(super) fn value_carrier_matches_reference(
    program: &dyn crate::CheckingInput,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
    required_substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
) -> bool {
    let (required, required_substitutions) =
        substituted_required_reference(program, required, required_substitutions);
    let TypeReferenceNode::Reference {
        access: ReferenceAccess::Shared,
        referee: required_referee,
        ..
    } = program.type_reference_table.type_reference(required)
    else {
        return declared_value_matches_reference(
            program,
            actual,
            required,
            substitutions,
            required_substitutions,
        );
    };
    let actual_referee = match program.type_reference_table.type_reference(actual) {
        TypeReferenceNode::Reference {
            access: ReferenceAccess::WriteOnly,
            ..
        } => return false,
        TypeReferenceNode::Reference { referee, .. } => *referee,
        _ => actual,
    };
    shared_referee_matches(
        program,
        actual_referee,
        *required_referee,
        substitutions,
        required_substitutions,
    )
}

/// Shared access compares the referee's carrier, independently of the facts
/// required at this use. A local may have a bare declared type and still carry
/// a predicate from its producer or a guard. Requiring that predicate in its
/// declaration rejects a valid call before flow contract checking can prove it.
///
/// Only outer declared-domain requirements are deferred: element types, nested reference
/// access, nominal identities and fixed extents still participate. Contract
/// checking proves the required qualification on the actual value; domain
/// weakening, carry and loan checks separately retain semantic and ownership
/// obligations. This relation supplies neither membership nor a loan.
fn shared_referee_matches(
    program: &dyn crate::CheckingInput,
    mut actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
    required_substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
) -> bool {
    let (mut required, required_substitutions) =
        substituted_required_reference(program, required, required_substitutions);
    if program.type_identity(TypeIdentityRequest {
        substitutions,
        ..TypeIdentityRequest::ordinary(actual)
    }) == program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
        substitutions: required_substitutions,
        ..TypeIdentityRequest::ordinary(required)
    }) {
        return true;
    }
    actual = program
        .type_reference_table
        .unconstrained_type_reference(actual);
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(required)
    {
        // Declared domains become call contract facts. Other constraints
        // (including compiler-owned carry/layout domains) do not all have
        // that discharge route, so shape matching cannot introduce them.
        if !program
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .all(|constraint| {
                matches!(constraint, TypeConstraintNode::Domain(domain)
                if domain.symbol.is_valid()
                    && domain.subject == typed_trees::types::DomainConstraintSubject::Declared)
            })
        {
            break;
        }
        required = *base_type;
    }
    program.type_identity(TypeIdentityRequest {
        substitutions,
        ..TypeIdentityRequest::ordinary(actual)
    }) == program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
        substitutions: required_substitutions,
        ..TypeIdentityRequest::ordinary(required)
    }) || owned_array_projects_to_slice(
        program,
        actual,
        required,
        substitutions,
        required_substitutions,
    ) || fixed_array_binds_open_length(
        program,
        actual,
        required,
        substitutions,
        required_substitutions,
    )
}

/// A `const` binder in a fixed-array length is an inference slot, not a closed
/// identity: an actual literal length supplies the binding only when the
/// literal satisfies the binder's declared carrier (primitive fit plus every
/// declared closed range, exactly as `validate_const_integer_range` checks an
/// explicit const argument), so `&[u8; 5]` still fails a `u64[0..=3]`
/// parameter rather than weakening the range check. A forwarded caller binder
/// binds only when both declared carriers match exactly -- the same rule
/// `callee<K>()` and specialization's own `call_bindings` inference apply to
/// explicit const arguments. The required side keeps its authored constraint
/// shells: a `([u8; N] in Domain)` parameter cannot be satisfied by shape
/// alone.
pub(super) fn fixed_array_binds_open_length(
    program: &dyn crate::CheckingInput,
    mut actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
    required_substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
) -> bool {
    let (required, required_substitutions) =
        substituted_required_reference(program, required, required_substitutions);
    let TypeReferenceNode::FixedArray {
        element_type: required_element,
        length:
            FixedArrayLength::ConstParameter {
                symbol: required_symbol,
                ..
            },
    } = program.type_reference_table.type_reference(required)
    else {
        return false;
    };
    actual = program
        .type_reference_table
        .unconstrained_type_reference(actual);
    let TypeReferenceNode::FixedArray {
        element_type: actual_element,
        length: actual_length,
    } = program.type_reference_table.type_reference(actual)
    else {
        return false;
    };
    if program.type_identity(TypeIdentityRequest {
        substitutions,
        ..TypeIdentityRequest::ordinary(*actual_element)
    }) != program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
        substitutions: required_substitutions,
        ..TypeIdentityRequest::ordinary(*required_element)
    }) {
        return false;
    }
    let Some(required_declared) = const_length_declared_type(program, *required_symbol) else {
        return false;
    };
    match actual_length {
        FixedArrayLength::Literal(length) => {
            literal_satisfies_declared_const(program, *length, required_declared)
        }
        FixedArrayLength::ConstParameter {
            symbol: actual_symbol,
            ..
        } => const_length_declared_type(program, *actual_symbol).is_some_and(|actual_declared| {
            crate::value_custody::type_references::type_references_match(
                program,
                actual_declared,
                required_declared,
            )
        }),
        FixedArrayLength::Expression(_) | FixedArrayLength::ConstCall { .. } => false,
    }
}

/// The declared integer carrier of a `Const`/`Value` type parameter -- the
/// type an inferred or forwarded length argument must satisfy. Type
/// parameters live in one flat arena across machines, data, domains,
/// operators, conformances and signatures, so a symbol lookup finds the exact
/// declaring telescope without knowing the call's owner.
fn const_length_declared_type(
    program: &dyn crate::CheckingInput,
    symbol: symbols::SymbolHandle,
) -> Option<TypeReferenceHandle> {
    if !symbol.is_valid() {
        return None;
    }
    program
        .data_type_parameters
        .iter()
        .find(|(_, parameter)| parameter.symbol == symbol)
        .and_then(|(_, parameter)| match parameter.kind {
            TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference } => Some(type_reference),
            _ => None,
        })
}

/// Whether a literal array length is admissible as the const binder's value:
/// the declared primitive must accept the integer and every declared closed
/// range must contain it. A range endpoint that is not closed (still open or
/// non-integer) supplies no provable bound, so the match refuses rather than
/// inferring a length the declared carrier might exclude.
fn literal_satisfies_declared_const(
    program: &dyn crate::CheckingInput,
    length: usize,
    declared: TypeReferenceHandle,
) -> bool {
    let Some(primitive) = program.primitive_type_reference(declared) else {
        return false;
    };
    let Ok(value) = i128::try_from(length) else {
        return false;
    };
    if !primitive.accepts_integer_literal()
        || !crate::value_custody::type_references::const_integer_value_fits_primitive(
            primitive, value,
        )
    {
        return false;
    }
    let exact = BigInt::from_i128(value);
    let Some(bounds) = crate::constrained_range_bounds(program, declared, false) else {
        return false;
    };
    for (minimum, maximum) in bounds {
        if exact < minimum || exact > maximum {
            return false;
        }
    }
    true
}

/// A `collection[a..b]` range result is a fresh view over the collection's own
/// backing at a narrower extent, so the produced reference carries the
/// collection's resolved reference type. The collection resolves like any
/// other projected selection — element indexing inside is deliberately not a
/// range — and `reference_type_matches` keeps the access direction honest:
/// a shared source cannot authorize a mutable subslice.
pub(super) fn subslice_view_matches_reference(
    program: &dyn crate::CheckingInput,
    indexed: &typed_trees::expression::TableIndexedExpression,
    required: TypeReferenceHandle,
    correspondence: ReferenceCorrespondence,
    required_substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
) -> bool {
    let mut substitutions = Vec::new();
    let Some(collection) =
        selected_value_type(program, indexed.collection, &mut substitutions, None, None)
    else {
        return false;
    };
    let Some(collection) = substituted_reference(program, collection, &substitutions) else {
        return false;
    };
    matches!(
        program.type_reference_table.type_reference(collection),
        TypeReferenceNode::Reference { .. }
    ) && correspondence(
        program,
        collection,
        required,
        &substitutions,
        required_substitutions,
    )
}

pub(super) fn projected_matches_reference(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    required: TypeReferenceHandle,
    correspondence: ReferenceCorrespondence,
    required_substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
) -> bool {
    let mut substitutions = Vec::new();
    let Some(actual) = declared_value_type(program, expression, &mut substitutions) else {
        return false;
    };
    let Some(actual) = substituted_reference(program, actual, &substitutions) else {
        return false;
    };
    correspondence(
        program,
        actual,
        required,
        &substitutions,
        required_substitutions,
    )
}

pub(super) fn declared_value_matches_reference(
    program: &dyn crate::CheckingInput,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
    required_substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
) -> bool {
    let (required, required_substitutions) =
        substituted_required_reference(program, required, required_substitutions);
    let actual_identity = |reference| {
        program.type_identity(TypeIdentityRequest {
            substitutions,
            ..TypeIdentityRequest::ordinary(reference)
        })
    };
    let TypeReferenceNode::Reference {
        access: required_access,
        referee: required_referee,
        ..
    } = program.type_reference_table.type_reference(required)
    else {
        return false;
    };
    if matches!(
        program.type_reference_table.type_reference(actual),
        TypeReferenceNode::Reference { .. }
    ) {
        return reference_type_matches(
            program,
            actual,
            required,
            substitutions,
            required_substitutions,
        );
    }
    // The existing implicit shared borrow of an owned field still needs that
    // field's actual type. Member syntax alone cannot match an arbitrary referee.
    *required_access == ReferenceAccess::Shared
        && (actual_identity(actual)
            == program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
                substitutions: required_substitutions,
                ..TypeIdentityRequest::ordinary(*required_referee)
            })
            || owned_array_projects_to_slice(
                program,
                actual,
                *required_referee,
                substitutions,
                required_substitutions,
            )
            || owned_array_satisfies_constrained_referee(
                program,
                actual,
                *required_referee,
                substitutions,
                required_substitutions,
            )
            || fixed_array_binds_open_length(
                program,
                actual,
                *required_referee,
                substitutions,
                required_substitutions,
            ))
}

/// Shape correspondence for a shared view, not a loan or a domain proof.
/// Carrier qualifications may be forgotten by a plain view; element identities
/// and qualifications are unchanged, and no required view constraint is inferred.
fn owned_array_projects_to_slice(
    program: &dyn crate::CheckingInput,
    mut actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
    required_substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
) -> bool {
    let (required, required_substitutions) =
        substituted_required_reference(program, required, required_substitutions);
    let TypeReferenceNode::Slice {
        element_type: required_element,
    } = program.type_reference_table.type_reference(required)
    else {
        return false;
    };
    actual = program
        .type_reference_table
        .unconstrained_type_reference(actual);
    let TypeReferenceNode::FixedArray {
        element_type: actual_element,
        ..
    } = program.type_reference_table.type_reference(actual)
    else {
        return false;
    };
    program
        .type_reference_table
        .contains_type_reference(*actual_element)
        && program
            .type_reference_table
            .contains_type_reference(*required_element)
        && program.type_identity(TypeIdentityRequest {
            substitutions,
            ..TypeIdentityRequest::ordinary(*actual_element)
        }) == program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
            substitutions: required_substitutions,
            ..TypeIdentityRequest::ordinary(*required_element)
        })
}

/// Constrained-referee correspondence: an owned `[u8; N] in D` carrier
/// satisfies `&[u8] in D` only when the array's declared domains imply every
/// constraint the required view keeps — the fixed-to-view implication the
/// flow facts use for the same carriers. A required referee without
/// constraints takes the plain shape correspondence; a required constraint
/// that is not a domain, or one no actual domain implies, still rejects.
fn owned_array_satisfies_constrained_referee(
    program: &dyn crate::CheckingInput,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
    required_substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
) -> bool {
    let (required, required_substitutions) =
        substituted_required_reference(program, required, required_substitutions);
    let mut required_carrier = required;
    let mut required_constraints = Vec::new();
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program
        .type_reference_table
        .type_reference(required_carrier)
    {
        required_constraints.extend(
            program
                .type_reference_table
                .constraints(*constraints)
                .iter(),
        );
        required_carrier = *base_type;
    }
    if required_constraints.is_empty() {
        return false;
    }
    let TypeReferenceNode::Slice {
        element_type: required_element,
    } = program
        .type_reference_table
        .type_reference(required_carrier)
    else {
        return false;
    };
    let mut actual_carrier = actual;
    let mut actual_domains = Vec::new();
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(actual_carrier)
    {
        actual_domains.extend(
            program
                .type_reference_table
                .constraints(*constraints)
                .iter()
                .filter_map(|constraint| match constraint {
                    TypeConstraintNode::Domain(domain) => Some(domain.symbol),
                    _ => None,
                }),
        );
        actual_carrier = *base_type;
    }
    let TypeReferenceNode::FixedArray {
        element_type: actual_element,
        ..
    } = program.type_reference_table.type_reference(actual_carrier)
    else {
        return false;
    };
    if !program
        .type_reference_table
        .contains_type_reference(*actual_element)
        || !program
            .type_reference_table
            .contains_type_reference(*required_element)
        || program.type_identity(TypeIdentityRequest {
            substitutions,
            ..TypeIdentityRequest::ordinary(*actual_element)
        }) != program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
            substitutions: required_substitutions,
            ..TypeIdentityRequest::ordinary(*required_element)
        })
    {
        return false;
    }
    required_constraints
        .iter()
        .all(|constraint| match constraint {
            TypeConstraintNode::Domain(target) => actual_domains.iter().any(|source| {
                crate::field_domains::domain_membership_implies(program, *source, target.symbol)
            }),
            _ => false,
        })
}

/// The shared-view forget for a constrained referee that is not an owned
/// array. A declared `&[u8] in D` spells `Reference { Constrained { Slice } }`:
/// unwrapping the qualification leaves the identical slice a shared parameter
/// asks for, and the callee simply cannot rely on the dropped predicate. Only
/// the actual side strips — the required referee's own constraints remain the
/// callee's claim, so an unqualified view still cannot satisfy `&[u8] in D`.
fn constrained_referee_forgets_qualification(
    program: &dyn crate::CheckingInput,
    mut actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
    required_substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
) -> bool {
    let (required, required_substitutions) =
        substituted_required_reference(program, required, required_substitutions);
    let mut constrained = false;
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(actual)
    {
        constrained = true;
        actual = *base_type;
    }
    constrained
        && program.type_identity(TypeIdentityRequest {
            substitutions,
            ..TypeIdentityRequest::ordinary(actual)
        }) == program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
            substitutions: required_substitutions,
            ..TypeIdentityRequest::ordinary(required)
        })
}

pub(super) fn substituted_reference(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
    substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
) -> Option<TypeReferenceHandle> {
    for _ in 0..=substitutions.len() {
        let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(reference)
        else {
            return Some(reference);
        };
        let Some((_, replacement)) = substitutions
            .iter()
            .find(|(parameter, _)| parameter == symbol)
        else {
            return Some(reference);
        };
        reference = *replacement;
    }
    None
}

pub(super) fn declared_value_type(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    substitutions: &mut Vec<(symbols::SymbolHandle, TypeReferenceHandle)>,
) -> Option<TypeReferenceHandle> {
    selected_value_type(program, expression, substitutions, None, None)
}

/// A known current-state root supplies identity, not a new loan. Intermediate
/// reference permissions constrain forwarding independently of the leaf type.
pub(crate) fn place_forwarded_exclusive_reference_access(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    root: symbols::SymbolHandle,
    root_type: TypeReferenceHandle,
) -> Option<ReferenceAccess> {
    let mut substitutions = Vec::new();
    let Some(selected) = selected_value_type(
        program,
        expression,
        &mut substitutions,
        Some((root, root_type)),
        Some(ReferenceAccess::Mutable),
    ) else {
        return None;
    };
    let Some(mut selected) = substituted_reference(program, selected, &substitutions) else {
        return None;
    };
    selected = program
        .type_reference_table
        .unconstrained_type_reference(selected);
    declared_reference_access(program, selected).filter(|access| access.is_exclusive())
}

pub(crate) fn declared_reference_access(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
) -> Option<language_semantics::ReferenceAccess> {
    reference = program
        .type_reference_table
        .unconstrained_type_reference(reference);
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Reference { access, .. } => Some(*access),
        _ => None,
    }
}

/// Borrowing a selected referent preserves the access of every enclosing
/// reference. The owning context supplies the exact visible root and whether
/// its owned storage is writable; a mutable reference leaf can lend its
/// referent without making the containing owned binding mutable.
pub(crate) fn place_permits_reference_borrow(
    program: &dyn crate::CheckingInput,
    mut expression: ExpressionHandle,
    root: symbols::SymbolHandle,
    root_type: TypeReferenceHandle,
    owned_root_is_writable: bool,
    access: ReferenceAccess,
) -> bool {
    // A range lends its collection's backing. Its bounds remain independently
    // checked; selecting its extent cannot manufacture source permission.
    if let Some(indexed) = program.expression_table.indexed_node(expression)
        && matches!(
            program.expression_table.expression(indexed.index),
            ExpressionNode::Range(_)
        )
    {
        expression = indexed.collection;
    }
    let mut substitutions = Vec::new();
    let Some(selected) = selected_value_type(
        program,
        expression,
        &mut substitutions,
        Some((root, root_type)),
        Some(access),
    ) else {
        return false;
    };
    let Some(mut selected) = substituted_reference(program, selected, &substitutions) else {
        return false;
    };
    selected = program
        .type_reference_table
        .unconstrained_type_reference(selected);
    match program.type_reference_table.type_reference(selected) {
        TypeReferenceNode::Reference {
            access: source_access,
            ..
        } => reference_access_permits(*source_access, access),
        _ => {
            owned_root_is_writable
                || declared_reference_access(program, root_type)
                    .is_some_and(|source_access| reference_access_permits(source_access, access))
        }
    }
}

fn reference_access_permits(source: ReferenceAccess, required: ReferenceAccess) -> bool {
    matches!(
        (source, required),
        (ReferenceAccess::Shared, ReferenceAccess::Shared)
            | (
                ReferenceAccess::Mutable,
                ReferenceAccess::Shared | ReferenceAccess::Mutable | ReferenceAccess::WriteOnly
            )
            | (ReferenceAccess::WriteOnly, ReferenceAccess::WriteOnly)
    )
}

/// A store replaces the selected slot, not a reference held in that slot's
/// referent. Check every crossed reference but leave the endpoint unpeeled:
/// replacing a shared descriptor is legal through mutable containing storage,
/// while indexing through that descriptor cannot authorize a write.
pub(crate) fn place_permits_storage_write(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    root: symbols::SymbolHandle,
    root_type: TypeReferenceHandle,
) -> bool {
    selected_value_type(
        program,
        expression,
        &mut Vec::new(),
        Some((root, root_type)),
        Some(ReferenceAccess::WriteOnly),
    )
    .is_some()
}

fn projection_receiver_type(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
    substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
    required_access: Option<ReferenceAccess>,
) -> Option<TypeReferenceHandle> {
    for _ in 0..128 {
        reference = substituted_reference(program, reference, substitutions)?;
        reference = match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => *base_type,
            TypeReferenceNode::Reference {
                referee, access, ..
            } => {
                if required_access
                    .is_some_and(|required| !reference_access_permits(*access, required))
                {
                    return None;
                }
                *referee
            }
            _ => return Some(reference),
        };
    }
    None
}

fn is_builtin_indexed_shape(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
) -> bool {
    loop {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::FixedArray { .. } | TypeReferenceNode::Slice { .. } => return true,
            _ => return false,
        }
    }
}

fn selected_value_type(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    substitutions: &mut Vec<(symbols::SymbolHandle, TypeReferenceHandle)>,
    forwarding_root: Option<(symbols::SymbolHandle, TypeReferenceHandle)>,
    required_projection_access: Option<ReferenceAccess>,
) -> Option<TypeReferenceHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(name) => {
            let [spelling] = program.expression_table.name_path_members(name.members) else {
                return None;
            };
            if !(name.symbol.is_valid() && name.symbol == name.head_symbol) {
                return None;
            }
            if let Some((root, reference)) = forwarding_root {
                // The contextual owner already checked this root, including
                // `self`, whose machine symbol need not be spelled `self`.
                return (root == name.symbol).then_some(reference);
            }
            (program.symbols.name(name.symbol) == spelling.as_str())
                .then(|| named_value_type_reference(program, name))?
        }
        ExpressionNode::Call(_) if forwarding_root.is_none() => {
            crate::machine_calls::calls::call_result_type(program, expression)
        }
        ExpressionNode::Member(member) => {
            if forwarding_root.is_none()
                && let ExpressionNode::Name(root) =
                    program.expression_table.expression(member.receiver)
                && let Some(machine) = program.machines().iter().find(|machine| {
                    root.head_symbol.is_valid() && machine.symbol == root.head_symbol
                })
            {
                return crate::value_custody::places::exact_self_field(
                    program, machine, expression,
                )
                .map(|field| field.type_reference);
            }
            let receiver = selected_value_type(
                program,
                member.receiver,
                substitutions,
                forwarding_root,
                required_projection_access,
            )?;
            let receiver = projection_receiver_type(
                program,
                receiver,
                substitutions,
                required_projection_access,
            )?;
            let (symbol, arguments) = match program.type_reference_table.type_reference(receiver) {
                TypeReferenceNode::Named { symbol, .. } => (*symbol, &[][..]),
                TypeReferenceNode::Generic {
                    base_symbol,
                    arguments,
                    ..
                } => (
                    *base_symbol,
                    program
                        .type_reference_table
                        .type_reference_handles(*arguments),
                ),
                _ => return None,
            };
            // A `&mut self` / `&self` receiver's referee names the machine
            // symbol; its attached data declaration carries the fields. Other
            // receivers already name the data declaration directly.
            let symbol = if program.data_definition_by_symbol(symbol).is_some() {
                symbol
            } else {
                program
                    .machine(symbol)
                    .map(|machine| machine.attached_data_symbol)
                    .unwrap_or(symbol)
            };
            let mut definitions = program
                .data_definitions()
                .iter()
                .filter(|definition| symbol.is_valid() && definition.symbol == symbol);
            let definition = definitions.next()?;
            if definitions.next().is_some()
                || program.data_type_parameters(definition).len() != arguments.len()
            {
                return None;
            }
            // A payload's authored field type belongs to the data telescope,
            // not its caller's same-spelled binder. Compare it under the exact
            // receiver application without inventing a reference or a loan.
            for (parameter, argument) in program
                .data_type_parameters(definition)
                .iter()
                .zip(arguments)
            {
                if let Some((_, existing)) = substitutions
                    .iter()
                    .find(|(symbol, _)| *symbol == parameter.symbol)
                {
                    // Re-entering this telescope with different arguments needs
                    // scoped type views. This flat comparison cannot safely
                    // rebind earlier argument payloads, so retain rejection.
                    if program.type_identity(TypeIdentityRequest {
                        substitutions,
                        ..TypeIdentityRequest::ordinary(*existing)
                    }) != program.type_identity(TypeIdentityRequest {
                        substitutions,
                        ..TypeIdentityRequest::ordinary(*argument)
                    }) {
                        return None;
                    }
                } else {
                    substitutions.push((parameter.symbol, *argument));
                }
            }
            let field = crate::value_custody::places::exact_data_member_field(
                program,
                definition,
                member.member_symbol,
                member.member.as_str(),
                member.case_variant.as_ref().map(|variant| variant.as_str()),
            );
            // Through a forwarding root the declared field symbol can differ
            // from the member's resolved symbol (a generic telescope re-keys
            // it); the name is already unique inside a data declaration, so
            // a name-only lookup stays exact.
            let field = match (field, forwarding_root) {
                (Some(field), _) => Some(field),
                (None, None) => None,
                (None, Some(_)) => crate::value_custody::places::exact_data_member_field(
                    program,
                    definition,
                    symbols::SymbolHandle::invalid(),
                    member.member.as_str(),
                    member.case_variant.as_ref().map(|variant| variant.as_str()),
                ),
            };
            field.map(|field| field.type_reference)
        }
        // Preparation retains the complete constructed value before checking
        // freezes it. Projection needs that producer's applied type, not the
        // receiving reference or a new inference from one selected child.
        // Anonymous construction cannot forward a caller-owned place root.
        ExpressionNode::StructLiteral(_) | ExpressionNode::ArrayLiteral(_)
            if forwarding_root.is_none() =>
        {
            let reference = program
                .expression_table
                .constructed_value_type_reference(expression);
            reference.is_valid().then_some(reference).or_else(|| {
                // Substituted constant arrays retain their declaration's exact
                // carrier when they have no separately prepared construction.
                crate::declared_constant_array_type(program, expression)
            })
        }
        ExpressionNode::Indexed(indexed) => {
            if program.expression_table.indexed_on_range(indexed) {
                return None;
            }
            let collection = selected_value_type(
                program,
                indexed.collection,
                substitutions,
                forwarding_root,
                required_projection_access,
            )?;
            // Selection needs the applied collection, not a raw field
            // telescope whose substituted elements could match another
            // authored operator. This query does not materialize types. A
            // fixed array or slice already selects builtin indexing by its
            // shape: substituting its element or extent cannot introduce an
            // authored operator, so only other carriers must be applied.
            if !substitutions.is_empty()
                && !is_builtin_indexed_shape(program, collection)
                && program.normalized_type_identity(collection)
                    != program.type_identity(TypeIdentityRequest {
                        substitutions,
                        ..TypeIdentityRequest::ordinary(collection)
                    })
            {
                return None;
            }
            let index_type = declared_value_type(program, indexed.index, &mut Vec::new());
            let meaning_is_builtin =
                if let Some(machine) = place_machine_symbol(program, indexed.collection) {
                    typed_trees::operator::has_builtin_spelled_expression_meaning(
                        program,
                        machine,
                        expression,
                        language_core::OperatorSpelling::Index,
                        &[Some(collection), index_type],
                    )
                } else {
                    crate::value_custody::places::has_retained_builtin_index_meaning(
                        program, expression,
                    ) || {
                        // A substituted `const` collection has no place root,
                        // so no selecting machine is recoverable; its `[]`
                        // occurrence is still in checked custody during
                        // validation. With no specialization in the program no
                        // machine can supply a selected trait meaning, making
                        // the machine argument vacuous and the builtin check
                        // exact rather than an approximation.
                        program.machine_specializations.is_empty()
                            && typed_trees::operator::has_builtin_spelled_expression_meaning(
                                program,
                                symbols::SymbolHandle::invalid(),
                                expression,
                                language_core::OperatorSpelling::Index,
                                &[Some(collection), index_type],
                            )
                    }
                };
            if !meaning_is_builtin
                || !typed_trees::operator::resolve_indexed_spelling_for_operands(
                    program,
                    language_core::OperatorSpelling::Index,
                    &[Some(collection), index_type],
                    program.expression_table.source_span(expression),
                )
                .is_empty()
            {
                return None;
            }
            let collection = projection_receiver_type(
                program,
                collection,
                substitutions,
                required_projection_access,
            )?;
            crate::value_custody::places::collection_element_type(program, collection)
        }
        _ => None,
    }
}

/// Statement projections can precede final operator receipts. Their exact
/// named root still identifies the selection telescope for ordinary builtin
/// meaning reconstruction; no spelling lookup or default machine is allowed.
fn place_machine_symbol(
    program: &dyn crate::CheckingInput,
    mut expression: ExpressionHandle,
) -> Option<symbols::SymbolHandle> {
    for _ in 0..128 {
        match program.expression_table.expression(expression) {
            ExpressionNode::Member(member) => expression = member.receiver,
            ExpressionNode::Indexed(indexed) => expression = indexed.collection,
            ExpressionNode::Name(path)
                if path.symbol.is_valid() && path.symbol == path.head_symbol =>
            {
                let mut symbol = path.symbol;
                for _ in 0..128 {
                    if program.machine(symbol).is_some() {
                        return Some(symbol);
                    }
                    let parent = program.symbols.get(symbol).parent;
                    if !parent.is_valid() || parent == symbol {
                        return None;
                    }
                    symbol = parent;
                }
                return None;
            }
            _ => return None,
        }
    }
    None
}

/// A call's formal-to-actual environment is simultaneous: the replacement
/// already belongs to the caller, including when caller and callee coincide.
pub(super) fn substituted_required_reference<'bindings>(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    substitutions: &'bindings [(symbols::SymbolHandle, TypeReferenceHandle)],
) -> (
    TypeReferenceHandle,
    &'bindings [(symbols::SymbolHandle, TypeReferenceHandle)],
) {
    if let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(reference)
        && let Some((_, actual)) = substitutions.iter().find(|(formal, _)| formal == symbol)
    {
        (*actual, &[])
    } else {
        (reference, substitutions)
    }
}
