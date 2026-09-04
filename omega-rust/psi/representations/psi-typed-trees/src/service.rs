//! Exact recognition of the toolchain-owned routed service carrier.
//!
//! A same-named package declaration is ordinary opaque data. Compiler
//! privilege requires the complete core source, declaration shape, closed
//! boundary requirement, and `Bound` domain identity.

use crate::TypedTrees;
use crate::types::{
    DomainConstraintSubject, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use psi_symbols::SymbolHandle;

pub const SERVICE_CORE_SOURCE: &str = "service.omg";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactBoundServiceCarrier {
    pub service_data: SymbolHandle,
    pub bound_domain: SymbolHandle,
    pub requirement: SymbolHandle,
}

/// Classify one type shell. `Ok(None)` means it is not the exact core
/// `Service` carrier; `Err` means it does name that carrier but violates the
/// deliberately narrow first-rung contract.
pub fn classify_exact_bound_service_carrier(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Result<Option<ExactBoundServiceCarrier>, String> {
    let mut current = type_reference;
    let mut constraints = Vec::new();
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints: span,
    } = program.type_reference_table.type_reference(current)
    {
        constraints.extend_from_slice(program.type_reference_table.constraints(*span));
        current = *base_type;
    }

    let exact_bound_domain = exact_bound_domain_symbol(program);
    let carries_exact_bound = exact_bound_domain.is_some_and(|bound| {
        constraints.iter().any(|constraint| {
            matches!(
                constraint,
                TypeConstraintNode::Domain(domain)
                    if domain.subject == DomainConstraintSubject::Declared
                        && domain.symbol == bound
            )
        })
    });
    let wrong_bound_carrier = || {
        Err(
            "the exact toolchain-owned `Bound` domain routes only a closed `Service<R>` carrier"
                .to_owned(),
        )
    };

    let generic_origin = match program.type_reference_table.type_reference(current) {
        TypeReferenceNode::Generic { .. } => current,
        TypeReferenceNode::Named { symbol, .. } => {
            let Some(origin) = program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == *symbol)
                .and_then(|definition| definition.generic_instance)
            else {
                return if carries_exact_bound {
                    wrong_bound_carrier()
                } else {
                    Ok(None)
                };
            };
            origin
        }
        _ => {
            return if carries_exact_bound {
                wrong_bound_carrier()
            } else {
                Ok(None)
            };
        }
    };
    let TypeReferenceNode::Generic {
        base_symbol,
        lifetime_arguments,
        arguments,
        ..
    } = program.type_reference_table.type_reference(generic_origin)
    else {
        return if carries_exact_bound {
            wrong_bound_carrier()
        } else {
            Ok(None)
        };
    };
    if !is_exact_service_data_symbol(program, *base_symbol) {
        return if carries_exact_bound {
            wrong_bound_carrier()
        } else {
            Ok(None)
        };
    }
    if !lifetime_arguments.is_empty() {
        return Err("the core `Service` carrier takes no lifetime arguments".to_owned());
    }
    let arguments = program
        .type_reference_table
        .type_reference_handles(*arguments);
    let [requirement] = arguments else {
        return Err(format!(
            "the core `Service` carrier requires exactly one closed boundary requirement, but {} arguments were supplied",
            arguments.len()
        ));
    };
    let TypeReferenceNode::Named {
        symbol: requirement,
        name,
    } = program.type_reference_table.type_reference(*requirement)
    else {
        return Err(
            "the first Service rung accepts only one closed, monomorphic boundary-trait requirement"
                .to_owned(),
        );
    };
    let Some(requirement_definition) = program
        .traits()
        .iter()
        .find(|definition| definition.symbol == *requirement)
    else {
        return Err(format!(
            "`Service<{name}>` does not name an exact boundary-trait requirement"
        ));
    };
    if !requirement_definition.is_public {
        return Err(format!(
            "`Service<{name}>` requires a public boundary trait as its stable slot contract"
        ));
    }
    if !requirement_definition.is_boundary {
        return Err(format!(
            "`Service<{name}>` requires a boundary trait; ordinary traits are local interfaces"
        ));
    }
    if !requirement_definition.lifetime_parameters.is_empty()
        || !program
            .trait_type_parameters(requirement_definition)
            .is_empty()
    {
        return Err(format!(
            "the first Service rung accepts only a closed, nongeneric, lifetime-free boundary requirement; `{name}` remains open"
        ));
    }

    let Some(bound_domain) = exact_bound_domain else {
        return Err(
            "the exact core `Service::Bound` domain declaration is missing or malformed".to_owned(),
        );
    };
    let mut bound_count = 0usize;
    for constraint in constraints {
        match constraint {
            TypeConstraintNode::Domain(domain)
                if domain.subject == DomainConstraintSubject::Declared
                    && domain.symbol == bound_domain =>
            {
                bound_count += 1;
            }
            _ => {
                return Err(format!(
                    "`Service<{name}>` may carry only the exact toolchain-owned `Bound` domain"
                ));
            }
        }
    }
    if bound_count != 1 {
        return Err(format!(
            "`Service<{name}>` requires exactly one exact toolchain-owned `in Bound` qualification"
        ));
    }

    Ok(Some(ExactBoundServiceCarrier {
        service_data: *base_symbol,
        bound_domain,
        requirement: *requirement,
    }))
}

pub fn exact_bound_service_requirement(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    classify_exact_bound_service_carrier(program, type_reference)
        .ok()
        .flatten()
        .map(|carrier| carrier.requirement)
}

pub fn is_exact_service_data_symbol(program: &TypedTrees, symbol: SymbolHandle) -> bool {
    if !exact_toolchain_source(program, symbol, SERVICE_CORE_SOURCE) {
        return false;
    }
    let Some(definition) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == symbol)
    else {
        return false;
    };
    let parameters = program.data_type_parameters(definition);
    definition.name.as_str() == "Service"
        && definition.is_public
        && definition.supply_mode == psi_language_semantics::DataSupplyMode::BoundaryOpaque
        && definition.properties.multiplicity == psi_language_semantics::Multiplicity::Affine
        && definition.properties.carry.is_none()
        && definition.lifetime_parameters.is_empty()
        && matches!(parameters, [parameter] if matches!(parameter.kind, crate::data::TypeParameterKind::Type))
        && program.data_members(definition).is_empty()
}

fn exact_bound_domain_symbol(program: &TypedTrees) -> Option<SymbolHandle> {
    let mut matches = program.domain_definitions().iter().filter_map(|domain| {
        let parameters = program.domain_type_parameters(domain);
        let exact_generic_carrier = matches!(
            parameters,
            [parameter]
                if matches!(parameter.kind, crate::data::TypeParameterKind::Type)
                    && matches!(
                        program.type_reference_table.type_reference(domain.target_type),
                        TypeReferenceNode::Named { symbol, .. }
                            if *symbol == parameter.symbol
                    )
        );
        (domain.name.as_str() == "Bound"
            && domain.is_public
            && exact_generic_carrier
            && domain.index_arguments.is_empty()
            && domain.alias.is_none()
            && domain.classification.is_none()
            && domain.predicate_body == psi_language_semantics::DomainPredicateBody::Bodyless
            && domain.establishment_routes.is_empty()
            && exact_toolchain_source(program, domain.symbol, SERVICE_CORE_SOURCE))
        .then_some(domain.symbol)
    });
    let exact = matches.next()?;
    matches.next().is_none().then_some(exact)
}

fn exact_toolchain_source(program: &TypedTrees, symbol: SymbolHandle, relative: &str) -> bool {
    let Some(span) = program.symbols.symbol_source_span(symbol) else {
        return false;
    };
    let Some(source) = program.symbols.source_file(span) else {
        return false;
    };
    source.origin == psi_source::SourceOrigin::Toolchain
        && source
            .path
            .strip_prefix(&source.package_root)
            .ok()
            .is_some_and(|path| path == std::path::Path::new(relative))
}
