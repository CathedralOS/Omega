//! Compare the erased region axis after complete type formation.
//! Conformance and static callable refinement share this structural walk.
//! Reference descendants and unused nominal arguments are type relationships,
//! not additional physical loans. Scope resolution belongs to each consumer.

use typed_trees::name::Identifier;
use typed_trees::types::{
    DomainConstraint, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

pub(crate) trait TypeRegionScope: Copy {
    fn erased_identity(
        self,
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> typed_trees::type_identity::NormalizedTypeIdentity {
        program.normalized_type_identity(reference)
    }

    fn resolve(
        self,
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Option<(TypeReferenceHandle, Self)>;
}

impl TypeRegionScope for () {
    fn resolve(
        self,
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Option<(TypeReferenceHandle, Self)> {
        (!reference.is_valid()
            || program
                .type_reference_table
                .contains_type_reference(reference))
        .then_some((reference, self))
    }
}

pub(crate) fn type_reference_regions_match<Scope: TypeRegionScope>(
    program: &dyn crate::CheckingInput,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    actual_scope: Scope,
    required_scope: Scope,
    lifetime_matches: &mut impl FnMut(
        Scope,
        Scope,
        TypeReferenceHandle,
        Option<&Identifier>,
        Option<&Identifier>,
    ) -> bool,
) -> bool {
    if !actual.is_valid() || !required.is_valid() {
        return actual.is_valid() == required.is_valid();
    }
    let Some((actual, actual_scope, actual_domains)) =
        type_structure(program, actual, actual_scope)
    else {
        return false;
    };
    let Some((required, required_scope, required_domains)) =
        type_structure(program, required, required_scope)
    else {
        return false;
    };
    // Formation owns property implication: a provider may accept a weaker
    // qualification. This query compares only corresponding domain terms,
    // joined by their already-formed erased operands, not domain membership.
    if !required_domains.iter().all(|(required, required_scope)| {
        let corresponding =
            actual_domains
                .iter()
                .filter(|(actual, actual_scope)| {
                    actual.subject == required.subject
                        && actual.symbol == required.symbol
                        && actual.arguments.len() == required.arguments.len()
                        && actual.arguments.iter().zip(&required.arguments).all(
                            |(actual, required)| {
                                actual_scope.erased_identity(program, *actual)
                                    == required_scope.erased_identity(program, *required)
                            },
                        )
                })
                .collect::<Vec<_>>();
        corresponding.is_empty()
            || corresponding.iter().any(|(actual, actual_scope)| {
                actual
                    .arguments
                    .iter()
                    .zip(&required.arguments)
                    .all(|(actual, required)| {
                        type_reference_regions_match(
                            program,
                            *actual,
                            *required,
                            *actual_scope,
                            *required_scope,
                            lifetime_matches,
                        )
                    })
            })
    }) {
        return false;
    }
    match (
        program.type_reference_table.type_reference(actual),
        program.type_reference_table.type_reference(required),
    ) {
        (
            TypeReferenceNode::Reference {
                referee: actual_referee,
                lifetime: actual_lifetime,
                ..
            },
            TypeReferenceNode::Reference {
                referee: required_referee,
                lifetime: required_lifetime,
                ..
            },
        ) => {
            lifetime_matches(
                actual_scope,
                required_scope,
                actual,
                actual_lifetime.as_ref(),
                required_lifetime.as_ref(),
            ) && type_reference_regions_match(
                program,
                *actual_referee,
                *required_referee,
                actual_scope,
                required_scope,
                lifetime_matches,
            )
        }
        (
            TypeReferenceNode::FixedArray {
                element_type: actual_element,
                ..
            },
            TypeReferenceNode::FixedArray {
                element_type: required_element,
                ..
            },
        )
        | (
            TypeReferenceNode::Slice {
                element_type: actual_element,
            },
            TypeReferenceNode::Slice {
                element_type: required_element,
            },
        ) => type_reference_regions_match(
            program,
            *actual_element,
            *required_element,
            actual_scope,
            required_scope,
            lifetime_matches,
        ),
        (
            TypeReferenceNode::Generic {
                lifetime_arguments: actual_lifetimes,
                arguments: actual_arguments,
                ..
            },
            TypeReferenceNode::Generic {
                lifetime_arguments: required_lifetimes,
                arguments: required_arguments,
                ..
            },
        ) => {
            actual_lifetimes.len() == required_lifetimes.len()
                && actual_lifetimes.iter().zip(required_lifetimes).all(
                    |(actual_lifetime, required_lifetime)| {
                        lifetime_matches(
                            actual_scope,
                            required_scope,
                            actual,
                            Some(actual_lifetime),
                            Some(required_lifetime),
                        )
                    },
                )
                && actual_arguments.count() == required_arguments.count()
                && program
                    .type_reference_table
                    .type_reference_handles(*actual_arguments)
                    .len()
                    == actual_arguments.len()
                && program
                    .type_reference_table
                    .type_reference_handles(*required_arguments)
                    .len()
                    == required_arguments.len()
                && program
                    .type_reference_table
                    .type_reference_handles(*actual_arguments)
                    .iter()
                    .zip(
                        program
                            .type_reference_table
                            .type_reference_handles(*required_arguments),
                    )
                    .all(|(actual_argument, required_argument)| {
                        type_reference_regions_match(
                            program,
                            *actual_argument,
                            *required_argument,
                            actual_scope,
                            required_scope,
                            lifetime_matches,
                        )
                    })
        }
        (
            TypeReferenceNode::DynamicTrait {
                arguments: actual, ..
            },
            TypeReferenceNode::DynamicTrait {
                arguments: required,
                ..
            },
        ) => {
            actual.len() == required.len()
                && program
                    .type_reference_table
                    .type_reference_handles(*actual)
                    .len()
                    == actual.len()
                && program
                    .type_reference_table
                    .type_reference_handles(*required)
                    .len()
                    == required.len()
                && program
                    .type_reference_table
                    .type_reference_handles(*actual)
                    .iter()
                    .zip(
                        program
                            .type_reference_table
                            .type_reference_handles(*required),
                    )
                    .all(|(actual, required)| {
                        type_reference_regions_match(
                            program,
                            *actual,
                            *required,
                            actual_scope,
                            required_scope,
                            lifetime_matches,
                        )
                    })
        }
        _ => true,
    }
}

fn type_structure<Scope: TypeRegionScope>(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
    mut scope: Scope,
) -> Option<(TypeReferenceHandle, Scope, Vec<(&DomainConstraint, Scope)>)> {
    let mut domains = Vec::new();
    let mut visited = Vec::new();
    loop {
        let (resolved, resolved_scope) = scope.resolve(program, reference)?;
        reference = resolved;
        scope = resolved_scope;
        if visited.contains(&reference) {
            return None;
        }
        visited.push(reference);
        let TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = program.type_reference_table.type_reference(reference)
        else {
            return Some((reference, scope, domains));
        };
        for constraint in program.type_reference_table.constraints(*constraints) {
            if let TypeConstraintNode::Domain(domain) = constraint {
                domains.push((domain, scope));
            }
        }
        reference = *base_type;
    }
}
