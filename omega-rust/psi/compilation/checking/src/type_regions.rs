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

    /// Rewrite this scope when the walk reaches an operand owned elsewhere.
    fn enter_operand(
        self,
        _program: &dyn crate::CheckingInput,
        _owner: symbols::SymbolHandle,
    ) -> Self {
        self
    }

    /// A nominal type can carry regions from several supplying telescopes.
    /// Authenticate each retained declaration/ordinal before selecting its
    /// scope; a whole-node owner cannot stand for every slot of that type.
    fn at_lifetime(
        self,
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
        slot: usize,
        spelling: Option<&Identifier>,
    ) -> Option<Self> {
        let regions = program.type_reference_table.operand_lifetimes(reference);
        if regions.is_empty() {
            return Some(self);
        }
        let expected = match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference {
                lifetime: Some(_), ..
            } => 1,
            TypeReferenceNode::Generic {
                lifetime_arguments, ..
            } => lifetime_arguments.len(),
            _ => 0,
        };
        if regions.len() != expected {
            return None;
        }
        let region = *regions.get(slot)?;
        if !region.declaration.is_valid() {
            return Some(self);
        }
        if typed_trees::type_identity::lifetime_parameter_name(program.typed_program(), region)
            != spelling
        {
            return None;
        }
        Some(self.enter_operand(program, region.declaration))
    }

    /// One named alias of this scope's rosters: `None` when the name is not an
    /// alias here, so the walk falls through to the declaration instance.
    fn named_alias(
        self,
        _program: &dyn crate::CheckingInput,
        _symbol: symbols::SymbolHandle,
        _reference: TypeReferenceHandle,
    ) -> Option<ReferenceStep<Self>> {
        None
    }

    /// Walk one reference to its alias-free form under this scope. Validity,
    /// the visited set, the operand-lifetime owner step and the generic-instance
    /// alias are the same operation for every scope, so the walk lives here
    /// once: every scope restated it before.
    fn resolve(
        mut self,
        program: &dyn crate::CheckingInput,
        mut reference: TypeReferenceHandle,
    ) -> Option<(TypeReferenceHandle, Self)> {
        let mut visited = Vec::new();
        loop {
            if !reference.is_valid() {
                return Some((reference, self));
            }
            if visited.contains(&reference)
                || !program
                    .type_reference_table
                    .contains_type_reference(reference)
            {
                return None;
            }
            visited.push(reference);
            let owner = program
                .type_reference_table
                .operand_lifetime_owner(reference);
            if owner.is_valid() {
                self = self.enter_operand(program, owner);
            }
            let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(reference)
            else {
                return Some((reference, self));
            };
            match self.named_alias(program, *symbol, reference) {
                Some(ReferenceStep::Refuse) => return None,
                Some(ReferenceStep::Stop) => return Some((reference, self)),
                Some(ReferenceStep::Chase(scope, next)) => {
                    self = scope;
                    reference = next;
                    continue;
                }
                None => {}
            }
            if let Some(application) = program
                .data_definition_by_symbol(*symbol)
                .and_then(|definition| definition.generic_instance)
            {
                reference = application;
                continue;
            }
            return Some((reference, self));
        }
    }
}

/// One named-alias step of the reference walk: chase the substituted operand
/// with the scope that supplied it, stop on the reference itself, or refuse an
/// alias this scope cannot apply.
pub(crate) enum ReferenceStep<Scope> {
    Chase(Scope, TypeReferenceHandle),
    Stop,
    Refuse,
}

/// Callable inputs may accept weaker qualifications; a provider application
/// must retain the complete qualification set in both directions. Erasing
/// regions can collapse distinct domain terms, so erased equality alone does
/// not establish that second judgment.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypeRegionRelationship {
    CallableRefinement,
    ExactApplication,
}

pub(crate) fn type_reference_regions_match<Scope: TypeRegionScope, State: Clone + PartialEq>(
    program: &dyn crate::CheckingInput,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    actual_scope: Scope,
    required_scope: Scope,
    state: &mut State,
    lifetime_matches: &mut impl FnMut(
        &mut State,
        Scope,
        Scope,
        TypeReferenceHandle,
        Option<&Identifier>,
        Option<&Identifier>,
    ) -> bool,
) -> bool {
    let mut applications = vec![state.clone()];
    if !type_reference_region_applications(
        program,
        actual,
        required,
        actual_scope,
        required_scope,
        TypeRegionRelationship::CallableRefinement,
        &mut applications,
        lifetime_matches,
    ) {
        return false;
    }
    let [application] = applications.as_slice() else {
        return false;
    };
    *state = application.clone();
    true
}

/// Retain distinct viable applications across the complete structural query.
/// A later operand can disambiguate equal-erased-shape domain alternatives.
/// Each branch owns its constraints; a failed branch contributes nothing.
/// Consumers settle uniqueness only after all their input operands are joined.
pub(crate) fn type_reference_region_applications<
    Scope: TypeRegionScope,
    State: Clone + PartialEq,
>(
    program: &dyn crate::CheckingInput,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    actual_scope: Scope,
    required_scope: Scope,
    relationship: TypeRegionRelationship,
    applications: &mut Vec<State>,
    lifetime_matches: &mut impl FnMut(
        &mut State,
        Scope,
        Scope,
        TypeReferenceHandle,
        Option<&Identifier>,
        Option<&Identifier>,
    ) -> bool,
) -> bool {
    if applications.is_empty() {
        return false;
    }
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
    if !domain_region_applications(
        program,
        &actual_domains,
        &required_domains,
        DomainCoverage::Required,
        relationship,
        applications,
        lifetime_matches,
    ) || (relationship == TypeRegionRelationship::ExactApplication
        && !domain_region_applications(
            program,
            &actual_domains,
            &required_domains,
            DomainCoverage::Actual,
            relationship,
            applications,
            lifetime_matches,
        ))
    {
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
            let Some(actual_region_scope) =
                actual_scope.at_lifetime(program, actual, 0, actual_lifetime.as_ref())
            else {
                return false;
            };
            let Some(required_region_scope) =
                required_scope.at_lifetime(program, required, 0, required_lifetime.as_ref())
            else {
                return false;
            };
            applications.retain_mut(|state| {
                lifetime_matches(
                    state,
                    actual_region_scope,
                    required_region_scope,
                    actual,
                    actual_lifetime.as_ref(),
                    required_lifetime.as_ref(),
                )
            });
            type_reference_region_applications(
                program,
                *actual_referee,
                *required_referee,
                actual_scope,
                required_scope,
                relationship,
                applications,
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
        ) => type_reference_region_applications(
            program,
            *actual_element,
            *required_element,
            actual_scope,
            required_scope,
            relationship,
            applications,
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
                && actual_lifetimes
                    .iter()
                    .zip(required_lifetimes)
                    .enumerate()
                    .all(|(slot, (actual_lifetime, required_lifetime))| {
                        let Some(actual_region_scope) =
                            actual_scope.at_lifetime(program, actual, slot, Some(actual_lifetime))
                        else {
                            return false;
                        };
                        let Some(required_region_scope) = required_scope.at_lifetime(
                            program,
                            required,
                            slot,
                            Some(required_lifetime),
                        ) else {
                            return false;
                        };
                        applications.retain_mut(|state| {
                            lifetime_matches(
                                state,
                                actual_region_scope,
                                required_region_scope,
                                actual,
                                Some(actual_lifetime),
                                Some(required_lifetime),
                            )
                        });
                        !applications.is_empty()
                    })
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
                        type_reference_region_applications(
                            program,
                            *actual_argument,
                            *required_argument,
                            actual_scope,
                            required_scope,
                            relationship,
                            applications,
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
                        type_reference_region_applications(
                            program,
                            *actual,
                            *required,
                            actual_scope,
                            required_scope,
                            relationship,
                            applications,
                            lifetime_matches,
                        )
                    })
        }
        _ => true,
    }
}

#[derive(Clone, Copy)]
enum DomainCoverage {
    Required,
    Actual,
}

fn domain_region_applications<Scope: TypeRegionScope, State: Clone + PartialEq>(
    program: &dyn crate::CheckingInput,
    actual_domains: &[(&DomainConstraint, Scope)],
    required_domains: &[(&DomainConstraint, Scope)],
    coverage: DomainCoverage,
    relationship: TypeRegionRelationship,
    applications: &mut Vec<State>,
    lifetime_matches: &mut impl FnMut(
        &mut State,
        Scope,
        Scope,
        TypeReferenceHandle,
        Option<&Identifier>,
        Option<&Identifier>,
    ) -> bool,
) -> bool {
    let (obligations, candidates) = match coverage {
        DomainCoverage::Required => (required_domains, actual_domains),
        DomainCoverage::Actual => (actual_domains, required_domains),
    };
    for (obligation, obligation_scope) in obligations {
        let corresponding = candidates.iter().filter(|(candidate, candidate_scope)| {
            candidate.subject == obligation.subject
                && candidate.symbol == obligation.symbol
                && candidate.arguments.len() == obligation.arguments.len()
                && candidate.arguments.iter().zip(&obligation.arguments).all(
                    |(candidate, obligation)| {
                        candidate_scope.erased_identity(program, *candidate)
                            == obligation_scope.erased_identity(program, *obligation)
                    },
                )
        });
        // Keep branch constraints private and delay uniqueness until all
        // operands have contributed. Reverse coverage still constrains the
        // original supplied-to-receiving relation; it never inverts binders.
        let mut found = false;
        let mut alternatives = Vec::new();
        for (candidate, candidate_scope) in corresponding {
            found = true;
            let (actual, actual_scope, required, required_scope) = match coverage {
                DomainCoverage::Required => {
                    (*candidate, *candidate_scope, *obligation, *obligation_scope)
                }
                DomainCoverage::Actual => {
                    (*obligation, *obligation_scope, *candidate, *candidate_scope)
                }
            };
            let mut candidate_applications = applications.clone();
            if actual
                .arguments
                .iter()
                .zip(&required.arguments)
                .all(|(actual, required)| {
                    type_reference_region_applications(
                        program,
                        *actual,
                        *required,
                        actual_scope,
                        required_scope,
                        relationship,
                        &mut candidate_applications,
                        lifetime_matches,
                    )
                })
            {
                for application in candidate_applications {
                    if !alternatives.contains(&application) {
                        alternatives.push(application);
                    }
                }
            }
        }
        if !found && relationship == TypeRegionRelationship::CallableRefinement {
            // Formation already checked property implication. No shared term
            // adds a region constraint to that independent refinement judgment.
            continue;
        }
        *applications = alternatives;
        if applications.is_empty() {
            return false;
        }
    }
    true
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
