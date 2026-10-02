//! Declared parameter qualifications become ordinary membership requirements.
//!
//! Initial signature lowering and instantiated call contracts use the same
//! domain roster. Application substitution changes the required type without
//! proving membership or changing a domain's exact identity and arguments.

use language_semantics::SemanticDomainId;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

/// Every normalized declared domain on a parameter, through its reference and
/// qualification shells. Arithmetic policy is not a membership contract.
pub fn parameter_domain_constraints(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Vec<(
    SymbolHandle,
    String,
    Vec<TypeReferenceHandle>,
    SemanticDomainId,
)> {
    parameter_domain_constraints_with_substitutions(program, type_reference, &[])
}

/// Read the required parameter after one simultaneous application. A selected
/// actual belongs to the caller's scope, so the callee's tuple is not applied
/// again inside it. Qualifications outside and inside that selection both stay
/// in the roster; callers own their ordinary proof and provenance obligations.
pub fn parameter_domain_constraints_with_substitutions(
    program: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
    mut substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Vec<(
    SymbolHandle,
    String,
    Vec<TypeReferenceHandle>,
    SemanticDomainId,
)> {
    let mut domains = Vec::new();
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                domains.extend(
                    program
                        .type_reference_table
                        .constraints(*constraints)
                        .iter()
                        .filter_map(|constraint| match constraint {
                            TypeConstraintNode::Domain(domain) if domain.symbol.is_valid() => {
                                program
                                    .domain_definitions()
                                    .iter()
                                    .find(|definition| definition.symbol == domain.symbol)
                                    .map(|definition| {
                                        (
                                            domain.symbol,
                                            definition.name.as_str().to_owned(),
                                            domain.arguments.clone(),
                                            domain.semantic_id,
                                        )
                                    })
                            }
                            TypeConstraintNode::Domain(domain)
                                if language_semantics::CarryPermission::from_name(
                                    domain.name.as_str(),
                                )
                                .is_some() =>
                            {
                                Some((
                                    SymbolHandle::invalid(),
                                    domain.name.as_str().to_owned(),
                                    Vec::new(),
                                    SemanticDomainId::NULL,
                                ))
                            }
                            _ => None,
                        }),
                );
                type_reference = *base_type;
            }
            TypeReferenceNode::Named { symbol, .. } if symbol.is_valid() => {
                let Some((_, selected)) = substitutions
                    .iter()
                    .rev()
                    .find(|(formal, _)| formal == symbol)
                else {
                    break;
                };
                type_reference = *selected;
                substitutions = &[];
            }
            _ => break,
        }
    }
    domains
}
