use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::name::DiagnosticName;
use symbol_resolved_trees::signature::StateSignature;
use symbol_resolved_trees::trait_definition::TraitDefinition;

/// One exact trait-requirement row selected by a path that carries no call
/// signature. Domain establishment routes and nominal static-machine binders
/// share this resolution law: neither visible satisfiers nor an expected call
/// shape may select among overloads.
pub(crate) struct ExactSignatureFreeRequirement<'program> {
    pub(crate) trait_definition: &'program TraitDefinition,
    pub(crate) requirement: &'program StateSignature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SignatureFreeRequirementResolutionError {
    InvalidPath,
    TraitNotUnique,
    RequirementNotUnique,
}

pub(crate) fn resolve_signature_free_requirement<'program>(
    program: &'program SymbolResolvedTrees,
    path: &[DiagnosticName],
) -> Result<ExactSignatureFreeRequirement<'program>, SignatureFreeRequirementResolutionError> {
    let [trait_path @ .., requirement_name] = path else {
        return Err(SignatureFreeRequirementResolutionError::InvalidPath);
    };
    if trait_path.is_empty() {
        return Err(SignatureFreeRequirementResolutionError::InvalidPath);
    }

    let trait_name = trait_path
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    let use_span = requirement_name.source_span();
    let matching_traits = program
        .traits
        .iter()
        .filter(|definition| {
            same_semantic_name(definition.name.as_str(), &trait_name)
                && program
                    .symbols
                    .source_reference_can_see_symbol(use_span, definition.symbol)
        })
        .collect::<Vec<_>>();
    let [trait_definition] = matching_traits.as_slice() else {
        return Err(SignatureFreeRequirementResolutionError::TraitNotUnique);
    };

    let matching_requirements = program
        .trait_machine_signatures(trait_definition.machines)
        .iter()
        .filter(|signature| {
            signature.name.as_str() == requirement_name.as_str()
                && program
                    .symbols
                    .source_reference_can_see_symbol(use_span, signature.symbol)
        })
        .collect::<Vec<_>>();
    let [requirement] = matching_requirements.as_slice() else {
        return Err(SignatureFreeRequirementResolutionError::RequirementNotUnique);
    };

    Ok(ExactSignatureFreeRequirement {
        trait_definition,
        requirement,
    })
}

pub(crate) fn same_semantic_name(left: &str, right: &str) -> bool {
    left == right
        || (!left.contains("::") && right.rsplit("::").next().is_some_and(|leaf| leaf == left))
        || (!right.contains("::") && left.rsplit("::").next().is_some_and(|leaf| leaf == right))
}

struct AmbiguousUse {
    trait_symbol: symbols::SymbolHandle,
    trait_name: String,
    requirement_name: String,
    use_span: source::SourceSpan,
    message: String,
}

/// Report overload additions at both sides of the compatibility break before
/// either normalizer consumes authored signature-free paths.
pub(crate) fn validate_signature_free_requirement_compatibility(
    program: &SymbolResolvedTrees,
) -> Vec<diagnostics::Diagnostic> {
    let mut uses = Vec::new();
    for (_, parameter) in program.tables.declarations.data_type_parameters.iter() {
        let symbol_resolved_trees::data::TypeParameterKind::Machine {
            contract:
                symbol_resolved_trees::data::MachineParameterContract::AuthoredNominal { requirement },
        } = &parameter.kind
        else {
            continue;
        };
        let rendered = requirement
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::");
        collect_ambiguous_use(
            program,
            requirement,
            format!(
                "nominal machine parameter `{}` requirement `{rendered}` does not resolve to one exact trait requirement; signature-free references reject overloads",
                parameter.name
            ),
            &mut uses,
        );
    }
    for domain in &program.domain_definitions {
        for route in &domain.authored_routes {
            let rendered = route
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            collect_ambiguous_use(
                program,
                route,
                format!(
                    "domain `{}` establishment route `{rendered}` does not resolve to one exact trait requirement",
                    domain.name
                ),
                &mut uses,
            );
        }
    }
    uses.sort_by_key(|use_site| {
        (
            use_site.use_span.source_id.0,
            use_site.use_span.span.start,
            use_site.use_span.span.end,
        )
    });

    let mut families: Vec<(symbols::SymbolHandle, String, String, source::SourceSpan)> = Vec::new();
    for use_site in &uses {
        if families.iter().any(|(symbol, requirement, _, _)| {
            *symbol == use_site.trait_symbol && requirement == &use_site.requirement_name
        }) {
            continue;
        }
        let trait_definition = program
            .traits
            .iter()
            .find(|definition| definition.symbol == use_site.trait_symbol)
            .expect("ambiguous signature-free family retains its declaring trait");
        families.push((
            use_site.trait_symbol,
            use_site.requirement_name.clone(),
            use_site.trait_name.clone(),
            trait_definition.name.source_span(),
        ));
    }
    families.sort_by_key(|(_, requirement, _, span)| {
        (span.source_id.0, span.span.start, requirement.clone())
    });

    let mut diagnostics = families
        .into_iter()
        .map(|(_, requirement, trait_name, span)| {
            diagnostics::Diagnostic::error(format!(
                "declaring trait `{trait_name}` overloads requirement `{requirement}`; this is a source-compatibility break for signature-free requirement references"
            ))
            .with_source_span(span)
        })
        .collect::<Vec<_>>();
    diagnostics.extend(uses.into_iter().map(|use_site| {
        diagnostics::Diagnostic::error(use_site.message).with_source_span(use_site.use_span)
    }));
    diagnostics
}

fn collect_ambiguous_use(
    program: &SymbolResolvedTrees,
    path: &[DiagnosticName],
    message: String,
    uses: &mut Vec<AmbiguousUse>,
) {
    let [trait_path @ .., requirement_name] = path else {
        return;
    };
    let trait_name = trait_path
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    let matching_traits = program
        .traits
        .iter()
        .filter(|definition| {
            same_semantic_name(definition.name.as_str(), &trait_name)
                && program.symbols.source_reference_can_see_symbol(
                    requirement_name.source_span(),
                    definition.symbol,
                )
        })
        .collect::<Vec<_>>();
    let [trait_definition] = matching_traits.as_slice() else {
        return;
    };
    let count = program
        .trait_machine_signatures(trait_definition.machines)
        .iter()
        .filter(|signature| {
            signature.name.as_str() == requirement_name.as_str()
                && program.symbols.source_reference_can_see_symbol(
                    requirement_name.source_span(),
                    signature.symbol,
                )
        })
        .count();
    if count <= 1 {
        return;
    }
    uses.push(AmbiguousUse {
        trait_symbol: trait_definition.symbol,
        trait_name: trait_definition.name.as_str().to_owned(),
        requirement_name: requirement_name.as_str().to_owned(),
        use_span: requirement_name.source_span(),
        message,
    });
}
