use super::super::declarations::nominal_identity;
use super::super::encoding::{canonical_digest_label, framed_identity};
use crate::record::PackageReviewNominalOwner;
use compiler::CheckedCompilation;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

pub(crate) fn review_domain_lifetime_label(
    compilation: &CheckedCompilation,
    domain: &typed_trees::types::DomainConstraint,
) -> Result<String, Vec<Diagnostic>> {
    use typed_trees::types::{DomainConstraintSubject, OmegaLayoutGrammar};

    match domain.subject {
        DomainConstraintSubject::Declared => {
            let identity = nominal_identity(compilation, domain.symbol)?;
            let owner = match identity.owner {
                PackageReviewNominalOwner::Package(package) => {
                    canonical_digest_label("package", package.digest())
                }
                PackageReviewNominalOwner::ToolchainSource(source) => {
                    canonical_digest_label("toolchain-source", source.digest())
                }
                PackageReviewNominalOwner::Unresolved => {
                    return Err(vec![Diagnostic::error(
                        "package review rejects a declared domain without exact nominal ownership",
                    )]);
                }
            };
            Ok(framed_identity("declared-domain", &[owner, identity.path]))
        }
        DomainConstraintSubject::Carry(permission) => Ok(framed_identity(
            "compiler-domain",
            &[
                "carry".to_owned(),
                match permission {
                    language_semantics::CarryPermission::AcrossSuspend => "across-suspend",
                    language_semantics::CarryPermission::AnyCpu => "any-cpu",
                    language_semantics::CarryPermission::AnyThread => "any-thread",
                    language_semantics::CarryPermission::MovableAddress => "movable-address",
                }
                .to_owned(),
            ],
        )),
        DomainConstraintSubject::Value(value_domain) => Ok(framed_identity(
            "compiler-domain",
            &[
                "value".to_owned(),
                match value_domain {
                    language_semantics::value_domain::ValueDomain::Finite => "finite",
                }
                .to_owned(),
            ],
        )),
        DomainConstraintSubject::OmegaLayout { grammar } => Ok(framed_identity(
            "compiler-domain",
            &[
                "omega-layout".to_owned(),
                match grammar {
                    OmegaLayoutGrammar::Derived => "derived",
                }
                .to_owned(),
            ],
        )),
    }
}

pub(crate) fn review_lifetime_topology_with_substitutions(
    compilation: &CheckedCompilation,
    type_reference: typed_trees::types::TypeReferenceHandle,
    lifetime_binders: &[typed_trees::name::Identifier],
    substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
    lifetime_substitutions: &[(typed_trees::name::Identifier, typed_trees::name::Identifier)],
    active_substitutions: &mut Vec<SymbolHandle>,
) -> Result<String, Vec<Diagnostic>> {
    use typed_trees::types::{TypeConstraintNode, TypeReferenceNode};

    let topology = match compilation
        .type_reference_table
        .type_reference(type_reference)
    {
        TypeReferenceNode::Reference {
            referee, lifetime, ..
        } => {
            let lifetime = match lifetime {
                Some(lifetime) => format!(
                    "binder:{}",
                    substituted_lifetime_binder_ordinal(
                        lifetime,
                        lifetime_binders,
                        lifetime_substitutions,
                        "public type",
                    )?
                ),
                None => "elided".to_owned(),
            };
            framed_identity(
                "reference",
                &[
                    lifetime,
                    review_lifetime_topology_with_substitutions(
                        compilation,
                        *referee,
                        lifetime_binders,
                        substitutions,
                        lifetime_substitutions,
                        active_substitutions,
                    )?,
                ],
            )
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let mut constraint_topologies = compilation
                .type_reference_table
                .constraints(*constraints)
                .iter()
                .filter_map(|constraint| match constraint {
                    TypeConstraintNode::Domain(domain) if !domain.arguments.is_empty() => {
                        Some((|| {
                            let label = review_domain_lifetime_label(compilation, domain)?;
                            let arguments = domain
                                .arguments
                                .iter()
                                .map(|argument| {
                                    review_lifetime_topology_with_substitutions(
                                        compilation,
                                        *argument,
                                        lifetime_binders,
                                        substitutions,
                                        lifetime_substitutions,
                                        active_substitutions,
                                    )
                                })
                                .collect::<Result<Vec<_>, _>>()?;
                            Ok::<String, Vec<Diagnostic>>(framed_identity(&label, &arguments))
                        })())
                    }
                    _ => None,
                })
                .collect::<Result<Vec<_>, _>>()?;
            constraint_topologies.sort();
            constraint_topologies.dedup();
            let mut children = vec![review_lifetime_topology_with_substitutions(
                compilation,
                *base_type,
                lifetime_binders,
                substitutions,
                lifetime_substitutions,
                active_substitutions,
            )?];
            children.extend(constraint_topologies);
            framed_identity("constrained", &children)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => framed_identity(
            "array",
            &[review_lifetime_topology_with_substitutions(
                compilation,
                *element_type,
                lifetime_binders,
                substitutions,
                lifetime_substitutions,
                active_substitutions,
            )?],
        ),
        TypeReferenceNode::Slice { element_type } => framed_identity(
            "slice",
            &[review_lifetime_topology_with_substitutions(
                compilation,
                *element_type,
                lifetime_binders,
                substitutions,
                lifetime_substitutions,
                active_substitutions,
            )?],
        ),
        TypeReferenceNode::Generic {
            lifetime_arguments,
            arguments,
            ..
        } => {
            let mut children = lifetime_arguments
                .iter()
                .map(|lifetime| {
                    substituted_lifetime_binder_ordinal(
                        lifetime,
                        lifetime_binders,
                        lifetime_substitutions,
                        "public type",
                    )
                    .map(|ordinal| format!("binder:{ordinal}"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            children.extend(
                compilation
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .iter()
                    .map(|argument| {
                        review_lifetime_topology_with_substitutions(
                            compilation,
                            *argument,
                            lifetime_binders,
                            substitutions,
                            lifetime_substitutions,
                            active_substitutions,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            );
            framed_identity("generic", &children)
        }
        TypeReferenceNode::Named { symbol, .. } => {
            let Some((_, replacement)) = substitutions
                .iter()
                .rev()
                .find(|(parameter, _)| parameter == symbol)
            else {
                return Ok("named".to_owned());
            };
            if active_substitutions.contains(symbol) {
                return Err(vec![Diagnostic::error(
                    "package review rejects a cyclic inherited type substitution",
                )]);
            }
            active_substitutions.push(*symbol);
            let topology = review_lifetime_topology_with_substitutions(
                compilation,
                *replacement,
                lifetime_binders,
                substitutions,
                lifetime_substitutions,
                active_substitutions,
            );
            active_substitutions.pop();
            topology?
        }
        TypeReferenceNode::DynamicTrait { .. } => "dynamic-trait".to_owned(),
        TypeReferenceNode::ConstExpression(_) => "const-expression".to_owned(),
        TypeReferenceNode::Unit => "unit".to_owned(),
    };
    Ok(topology)
}

pub(crate) fn substituted_lifetime_binder_ordinal(
    lifetime: &typed_trees::name::Identifier,
    lifetime_binders: &[typed_trees::name::Identifier],
    substitutions: &[(typed_trees::name::Identifier, typed_trees::name::Identifier)],
    context: &str,
) -> Result<u32, Vec<Diagnostic>> {
    let lifetime = substitutions
        .iter()
        .rev()
        .find_map(|(parameter, argument)| (parameter == lifetime).then_some(argument))
        .unwrap_or(lifetime);
    lifetime_binder_ordinal(lifetime, lifetime_binders, context)
}

pub(crate) fn lifetime_binder_ordinal(
    lifetime: &typed_trees::name::Identifier,
    lifetime_binders: &[typed_trees::name::Identifier],
    context: &str,
) -> Result<u32, Vec<Diagnostic>> {
    let Some(ordinal) = lifetime_binders
        .iter()
        .position(|candidate| candidate == lifetime)
    else {
        return Err(vec![Diagnostic::error(format!(
            "{context} refers to unresolved lifetime `'{}'",
            lifetime.as_str()
        ))]);
    };
    u32::try_from(ordinal).map_err(|_| {
        vec![Diagnostic::error(format!(
            "{context} lifetime binder ordinal exceeds the portable package-review limit"
        ))]
    })
}
