//! Storage-carrier comparison with explicit substitution ownership.
//! Declaration equations refine both sides transitively. An application
//! substitutes its formal side once; supplied operands keep caller scope.

use super::peel_declared_constraints;
use symbols::SymbolHandle;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[derive(Clone, Copy)]
enum Substitutions<'bindings> {
    Exact,
    DeclarationEquations(&'bindings [(SymbolHandle, TypeReferenceHandle)]),
    Application(&'bindings [(SymbolHandle, TypeReferenceHandle)]),
}

pub(super) fn under_equations(
    program: &dyn crate::CheckingInput,
    left: TypeReferenceHandle,
    right: TypeReferenceHandle,
    equations: &[(SymbolHandle, TypeReferenceHandle)],
) -> bool {
    let substitutions = if equations.is_empty() {
        Substitutions::Exact
    } else {
        Substitutions::DeclarationEquations(equations)
    };
    compare(program, left, substitutions, right, substitutions)
}

pub(super) fn under_application(
    program: &dyn crate::CheckingInput,
    left: TypeReferenceHandle,
    right: TypeReferenceHandle,
    bindings: &[(SymbolHandle, TypeReferenceHandle)],
) -> bool {
    compare(
        program,
        left,
        Substitutions::Exact,
        right,
        Substitutions::Application(bindings),
    )
}

fn normalize_root<'bindings>(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    substitutions: Substitutions<'bindings>,
) -> Option<(TypeReferenceHandle, Substitutions<'bindings>)> {
    if !reference.is_valid() {
        return None;
    }
    match substitutions {
        Substitutions::DeclarationEquations(equations) => {
            substituted_declared_carrier(program, reference, equations)
                .map(|reference| (reference, substitutions))
        }
        Substitutions::Exact => {
            Some((peel_declared_constraints(program, reference), substitutions))
        }
        Substitutions::Application(bindings) => {
            if bindings.is_empty() {
                return normalize_root(program, reference, Substitutions::Exact);
            }
            let reference = peel_declared_constraints(program, reference);
            if let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(reference)
                && let Some((_, actual)) = bindings.iter().find(|(formal, _)| formal == symbol)
            {
                // The complete actual belongs to the caller, including nested
                // leaves. A sibling binding must never substitute it again.
                normalize_root(program, *actual, Substitutions::Exact)
            } else {
                Some((reference, substitutions))
            }
        }
    }
}

fn compare(
    program: &dyn crate::CheckingInput,
    left: TypeReferenceHandle,
    left_substitutions: Substitutions<'_>,
    right: TypeReferenceHandle,
    right_substitutions: Substitutions<'_>,
) -> bool {
    let (Some((left, left_substitutions)), Some((right, right_substitutions))) = (
        normalize_root(program, left, left_substitutions),
        normalize_root(program, right, right_substitutions),
    ) else {
        return false;
    };
    let compare_children = |left, right| {
        compare(
            program,
            left,
            left_substitutions,
            right,
            right_substitutions,
        )
    };
    match (
        program.type_reference_table.type_reference(left),
        program.type_reference_table.type_reference(right),
    ) {
        (
            TypeReferenceNode::Reference {
                referee: left,
                access: left_access,
                ..
            },
            TypeReferenceNode::Reference {
                referee: right,
                access: right_access,
                ..
            },
        ) => left_access == right_access && compare_children(*left, *right),
        (
            TypeReferenceNode::Slice { element_type: left },
            TypeReferenceNode::Slice {
                element_type: right,
            },
        ) => compare_children(*left, *right),
        (
            TypeReferenceNode::FixedArray {
                element_type: left,
                length: left_length,
            },
            TypeReferenceNode::FixedArray {
                element_type: right,
                length: right_length,
            },
        ) => {
            let extent = |length, substitutions| match substitutions {
                Substitutions::Exact => program.fixed_array_length_identity(length, &[], &[]),
                Substitutions::DeclarationEquations(bindings) => {
                    program.fixed_array_length_identity(length, &[], bindings)
                }
                Substitutions::Application(bindings) => program
                    .fixed_array_length_identity_with_simultaneous_substitutions(length, bindings),
            };
            extent(left_length, left_substitutions)
                .zip(extent(right_length, right_substitutions))
                .is_some_and(|(left, right)| left == right)
                && compare_children(*left, *right)
        }
        (
            TypeReferenceNode::Generic {
                base_symbol: left_symbol,
                arguments: left_arguments,
                ..
            },
            TypeReferenceNode::Generic {
                base_symbol: right_symbol,
                arguments: right_arguments,
                ..
            },
        ) => {
            let left = program
                .type_reference_table
                .type_reference_handles(*left_arguments);
            let right = program
                .type_reference_table
                .type_reference_handles(*right_arguments);
            left_symbol == right_symbol
                && left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| compare_children(*left, *right))
        }
        _ => {
            let identity = |reference, substitutions| match substitutions {
                Substitutions::Exact => program.normalized_type_identity(reference),
                Substitutions::DeclarationEquations(bindings) => {
                    program.type_identity(typed_trees::type_identity::TypeIdentityRequest {
                        substitutions: bindings,
                        ..typed_trees::type_identity::TypeIdentityRequest::ordinary(reference)
                    })
                }
                Substitutions::Application(bindings) => program
                    .type_identity_with_simultaneous_substitutions(
                        typed_trees::type_identity::TypeIdentityRequest {
                            substitutions: bindings,
                            ..typed_trees::type_identity::TypeIdentityRequest::ordinary(reference)
                        },
                    ),
            };
            let left = identity(left, left_substitutions);
            let right = identity(right, right_substitutions);
            left.is_valid() && left == right
        }
    }
}

pub(crate) fn substituted_declared_carrier(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Option<TypeReferenceHandle> {
    let mut visited = Vec::new();
    loop {
        if !reference.is_valid() {
            return None;
        }
        reference = peel_declared_constraints(program, reference);
        let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(reference)
        else {
            return Some(reference);
        };
        let Some((_, replacement)) = substitutions.iter().find(|(binder, _)| binder == symbol)
        else {
            return Some(reference);
        };
        if visited.contains(symbol) {
            return None;
        }
        visited.push(*symbol);
        reference = *replacement;
    }
}

#[cfg(test)]
mod tests;
