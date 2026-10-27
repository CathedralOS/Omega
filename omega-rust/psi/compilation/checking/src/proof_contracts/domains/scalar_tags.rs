//! Declaration-backed scalar tags whose membership has no predicate or route.
//!
//! Source flow and scalar graph custody share this boundary. Retain both the
//! declaration and its normalized application; a family symbol alone cannot
//! equate different indices. The full qualified Terminal
//! signature carries these tags, so their exact parameter requirements need no
//! duplicate executable Boolean predicate. Other state contracts still need
//! their own graph evidence and must not disappear during this classification.

use symbols::SymbolHandle;
use typed_trees::domain::DomainDefinition;
use typed_trees::types::{
    DomainConstraintSubject, PrimitiveType, TypeConstraintNode, TypeReferenceHandle,
    TypeReferenceNode,
};

/// `reference`'s constrained-type domain constraints satisfying `admits`,
/// peeling `Constrained` bases until the reference stops rewrapping (with a
/// `seen` guard against a cyclic spelling).
fn constrained_scalar_domains(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
    admits: impl Fn(
        &dyn crate::CheckingInput,
        &typed_trees::types::DomainConstraint,
        PrimitiveType,
    ) -> bool,
) -> Vec<(SymbolHandle, language_semantics::SemanticDomainId)> {
    let Some(primitive) = program.primitive_type_reference(reference) else {
        return Vec::new();
    };
    let mut domains = Vec::new();
    let mut seen = Vec::new();
    while !seen.contains(&reference) {
        seen.push(reference);
        let TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = program.type_reference_table.type_reference(reference)
        else {
            break;
        };
        for constraint in program.type_reference_table.constraints(*constraints) {
            if let TypeConstraintNode::Domain(domain) = constraint
                && admits(program, domain, primitive)
                && !domains.contains(&(domain.symbol, domain.semantic_id))
            {
                domains.push((domain.symbol, domain.semantic_id));
            }
        }
        reference = *base_type;
    }
    domains
}

pub fn scalar_type_tags(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Vec<(SymbolHandle, language_semantics::SemanticDomainId)> {
    constrained_scalar_domains(program, reference, scalar_tag_constraint)
}

/// Routed atoms whose complete predicate component can be retained separately.
/// Unlike tags these always need provenance at introduction; classification
/// says only that the Terminal signature has a lossless representation.
pub fn scalar_type_routed_domains(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Vec<(SymbolHandle, language_semantics::SemanticDomainId)> {
    constrained_scalar_domains(program, reference, |program, domain, primitive| {
        domain.subject == DomainConstraintSubject::Declared
            && !domain.establishment_routes.is_empty()
            && super::scalar_domain_value_predicate(
                program,
                domain,
                checked_trees::CheckedScalarExpression::Parameter {
                    position: 0,
                    primitive_type: primitive,
                },
            )
            .is_some()
    })
}

/// Classify the exact application once for signature and stored-field readers.
/// This is a vacuous storage predicate, not evidence that two indexed tags are
/// compatible or that an establishment route has been executed.
pub(crate) fn scalar_tag_constraint(
    program: &dyn crate::CheckingInput,
    constraint: &typed_trees::types::DomainConstraint,
    primitive: PrimitiveType,
) -> bool {
    constraint.subject == DomainConstraintSubject::Declared
        && constraint.establishment_routes.is_empty()
        && program.domain_definitions().iter().any(|declaration| {
            declaration.symbol == constraint.symbol
                && exact_instance(
                    program,
                    declaration,
                    &constraint.arguments,
                    constraint.semantic_id,
                )
                && vacuous_scalar_tag(program, declaration, primitive, &mut Vec::new())
        })
}

/// An entry membership on an explicit scalar subject uses the same exact tag
/// classification as a qualified type. Generic tag carriers take the subject's
/// primitive type; their indices need not be integer predicate substitutions.
pub fn scalar_membership_is_tag(
    program: &dyn crate::CheckingInput,
    membership: &typed_trees::domain::ProofMembershipFact,
    primitive: PrimitiveType,
) -> bool {
    let arguments = program
        .type_reference_table
        .type_reference_handles(membership.domain_arguments);
    arguments.len() == membership.domain_arguments.len()
        && program
            .domain_definition(membership.domain_symbol)
            .is_some_and(|declaration| {
                exact_instance(program, declaration, arguments, membership.semantic_domain)
                    && vacuous_scalar_tag(program, declaration, primitive, &mut Vec::new())
            })
}

/// Declared scalar domains whose complete meaning has a subject-bound
/// predicate. Membership still needs proof at each arrival; this classification
/// neither identifies equal theories nor grants a routed qualification.
pub fn scalar_predicate_domains(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Vec<(SymbolHandle, language_semantics::SemanticDomainId)> {
    constrained_scalar_domains(program, reference, |program, domain, primitive| {
        domain.subject == DomainConstraintSubject::Declared
            && !scalar_tag_constraint(program, domain, primitive)
            && super::scalar_field_domain_predicate(program, domain, primitive).is_some()
    })
}

/// A tag has no predicate, fact or establishment route. A denotation role
/// (`Quantity<Units::METER>` with its own operators) is still a tag: it
/// selects operator meaning, not membership, and `as` may introduce it
/// (domains.md, "Exact coercion and erasure"). Arithmetic policies are
/// `ArithmeticDomain` constraints with their own owners; a domain carrying an
/// arithmetic policy role would change arithmetic, so it is not a tag.
fn vacuous_scalar_tag(
    program: &dyn crate::CheckingInput,
    declaration: &DomainDefinition,
    primitive: PrimitiveType,
    active: &mut Vec<SymbolHandle>,
) -> bool {
    if !declaration.symbol.is_valid()
        || !declaration.semantic_id.is_valid()
        || active.contains(&declaration.symbol)
        || declaration.predicate_body.is_present()
        || !declaration.facts.is_empty()
        || !declaration.establishment_routes.is_empty()
        || declaration.semantic_roles.arithmetic_policy.is_some()
        || (!typed_trees::domain::has_generic_carrier(program, declaration)
            && program.primitive_type_reference(declaration.target_type) != Some(primitive))
    {
        return false;
    }
    let Some(alias) = &declaration.alias else {
        return true;
    };
    active.push(declaration.symbol);
    let valid = !alias.constituents.is_empty()
        && alias.constituents.iter().all(|constituent| {
            program
                .domain_definition(constituent.domain_symbol)
                .is_some_and(|candidate| {
                    typed_trees::domain::index_parameters(program, candidate).is_empty()
                        && vacuous_scalar_tag(program, candidate, primitive, active)
                })
        });
    active.pop();
    valid
}

pub(super) fn exact_instance(
    program: &dyn crate::CheckingInput,
    domain: &DomainDefinition,
    arguments: &[TypeReferenceHandle],
    semantic_domain: language_semantics::SemanticDomainId,
) -> bool {
    let parameters = typed_trees::domain::index_parameters(program, domain);
    semantic_domain.is_valid()
        && parameters.len() == arguments.len()
        && arguments.iter().all(|argument| {
            program
                .type_reference_table
                .contains_type_reference(*argument)
        })
        && typed_trees::domain::indexed_domain_instance_name(program, domain, parameters, arguments)
            .is_ok_and(|identity| {
                program.semantic_domains.name(semantic_domain) == Some(identity.as_str())
            })
}
