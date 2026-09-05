//! Closed argument identities and lifetime-bearing type substitution.

use super::*;

/// A distinguishing slug for each argument -- the Phase-1 gate. `Some` when
/// EVERY argument is either a plain concrete `Named` type, a recursively
/// nonzero literal fixed array of one, or a `Named` carrying only nameable
/// constraints (an arithmetic/carrier domain, `Box<i32 in Wrapping>` /
/// `Store<u8 in Utf8>`); `None` if any argument is a nested generic, zero or
/// nonliteral array, slice, reference, or a range-bounded type whose bound is
/// an expression. The slug is used only to name the
/// synthetic record -- the SUBSTITUTION points the field at the argument's own
/// type reference, so a domain constraint on the argument rides along
/// unchanged. Distinct spellings must slug distinctly (`i32 in Wrapping` vs
/// `i32 in Saturating`); identical spellings share one instance.
pub(in crate::generic_data) fn monomorphizable_argument_slugs(
    syntax: &SyntaxTrees,
    argument_handles: &[TypeReferenceHandle],
) -> Option<Vec<String>> {
    argument_handles
        .iter()
        .map(|&argument| type_reference_slug(syntax, argument))
        .collect()
}

/// Rebind erased lifetimes carried by an already-synthesized local instance
/// from one concrete outer use to the outer template's own binder roster.
///
/// This first exact cohort is deliberately positional: the nested instance
/// must forward the complete outer lifetime application in the same order.
/// That preserves one stable synthesized definition across differently named
/// use-site lifetimes without inventing binders or choosing an alias/routing
/// policy. Broader permutations remain on the unnormalized path.
pub(in crate::generic_data) fn canonicalize_monomorphizable_argument_handles(
    syntax: &mut SyntaxTrees,
    base_info: &GenericData,
    outer_lifetime_arguments: &[Identifier],
    argument_handles: &[TypeReferenceHandle],
) -> Option<Vec<TypeReferenceHandle>> {
    base_info
        .const_parameter_types
        .iter()
        .zip(argument_handles)
        .map(|(const_parameter_type, argument)| {
            if const_parameter_type.is_some() {
                Some(*argument)
            } else {
                canonicalize_lifetime_bearing_type_argument(
                    syntax,
                    *argument,
                    &base_info.lifetime_parameters,
                    outer_lifetime_arguments,
                )
            }
        })
        .collect()
}

pub(in crate::generic_data) fn canonicalize_lifetime_bearing_type_argument(
    syntax: &mut SyntaxTrees,
    type_reference: TypeReferenceHandle,
    outer_lifetime_parameters: &[Identifier],
    outer_lifetime_arguments: &[Identifier],
) -> Option<TypeReferenceHandle> {
    let node = syntax
        .tables
        .type_references
        .type_reference(type_reference)
        .clone();
    match node {
        TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments,
            arguments,
        } if !lifetime_arguments.is_empty()
            && syntax
                .tables
                .type_references
                .type_reference_handles(arguments)
                .is_empty()
            && exact_synthesized_lifetime_instance(
                syntax,
                base_name.as_str(),
                lifetime_arguments.len(),
            ) =>
        {
            if outer_lifetime_parameters.len() != outer_lifetime_arguments.len()
                || lifetime_arguments.len() != outer_lifetime_arguments.len()
                || !lifetime_arguments
                    .iter()
                    .zip(outer_lifetime_arguments)
                    .all(|(nested, outer)| nested.as_str() == outer.as_str())
            {
                return None;
            }
            Some(
                syntax
                    .tables
                    .type_references
                    .insert(TypeReferenceNode::Generic {
                        base_name,
                        lifetime_arguments: outer_lifetime_parameters.to_vec(),
                        arguments: HandleSpan::empty(),
                    }),
            )
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } => {
            let element_type = canonicalize_lifetime_bearing_type_argument(
                syntax,
                element_type,
                outer_lifetime_parameters,
                outer_lifetime_arguments,
            )?;
            Some(
                syntax
                    .tables
                    .type_references
                    .insert(TypeReferenceNode::FixedArray {
                        element_type,
                        length: FixedArrayLength::Literal(length),
                    }),
            )
        }
        _ => Some(type_reference),
    }
}

pub(in crate::generic_data) fn exact_synthesized_lifetime_instance(
    syntax: &SyntaxTrees,
    name: &str,
    lifetime_arity: usize,
) -> bool {
    lifetime_arity > 0
        && syntax.root_items().any(|item| {
            matches!(
                item,
                Item::Data(definition)
                    if definition.name.as_str() == name
                        && definition.generic_instance.is_some()
                        && definition.type_parameters.is_empty()
                        && definition.lifetime_parameters.len() == lifetime_arity
            )
        })
}

/// The naming slug for an argument type, or `None` for a shape Phase 1 leaves
/// to the existing generic path. Plain `Named`, recursively nonzero literal
/// fixed arrays, and `Named in Domain...` only.
pub(in crate::generic_data) fn type_reference_slug(
    syntax: &SyntaxTrees,
    handle: TypeReferenceHandle,
) -> Option<String> {
    match syntax.tables.type_references.type_reference(handle) {
        TypeReferenceNode::Named(name) => Some(name.as_str().to_string()),
        TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments,
            arguments,
        } if syntax
            .tables
            .type_references
            .type_reference_handles(*arguments)
            .is_empty()
            && exact_synthesized_lifetime_instance(
                syntax,
                base_name.as_str(),
                lifetime_arguments.len(),
            ) =>
        {
            Some(base_name.as_str().to_owned())
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } if *length > 0 => Some(format!(
            "[{}; {length}]",
            type_reference_slug(syntax, *element_type)?
        )),
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let base = type_reference_slug(syntax, *base_type)?;
            let mut rendered = Vec::new();
            for constraint in syntax.tables.type_references.constraints(*constraints) {
                rendered.push(constraint_slug(constraint)?);
            }
            if rendered.is_empty() {
                return Some(base);
            }
            Some(format!("{base} in {}", rendered.join(" + ")))
        }
        _ => None,
    }
}

/// The naming slug for a constraint, or `None` for a range bound (an expression
/// -- Phase 3). Only the nameable behaviour/domain tags slug here.
pub(in crate::generic_data) fn constraint_slug(constraint: &TypeConstraintNode) -> Option<String> {
    match constraint {
        TypeConstraintNode::Named(name) => Some(name.as_str().to_string()),
        TypeConstraintNode::Domain(domain) if domain.arguments.is_empty() => {
            Some(domain.name.as_str().to_string())
        }
        TypeConstraintNode::Domain(_) => None,
        TypeConstraintNode::ArithmeticDomain(domain) => Some(domain.name().to_string()),
        TypeConstraintNode::Range { .. } => None,
    }
}
