//! The shared law for paths that name one trait requirement without a call
//! signature.
//!
//! Neither visible satisfiers nor an expected call shape may select among
//! overloads. Establishment routes, nominal machine binders, and lookup all
//! resolve through this module.

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
    let matching_traits = signature_free_trait_candidates(program, &trait_name, use_span);
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

/// One exact machine declaration selected by a path that carries no call
/// signature. Domain establishment routes may name a free or attached machine
/// (`a::b::make` or `Data::method`); neither visible conformers nor an
/// expected call shape may select among same-named machines.
pub(crate) struct ExactSignatureFreeMachine<'program> {
    pub(crate) machine: &'program symbol_resolved_trees::machine::Machine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SignatureFreeMachineResolutionError {
    NotUnique,
}

pub(crate) fn resolve_signature_free_machine<'program>(
    program: &'program SymbolResolvedTrees,
    path: &[DiagnosticName],
) -> Result<ExactSignatureFreeMachine<'program>, SignatureFreeMachineResolutionError> {
    let machine_name = path
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    let use_span = path
        .last()
        .expect("establishment route paths are nonempty")
        .source_span();
    let matching = signature_free_machine_candidates(program, &machine_name, use_span);
    let [machine] = matching.as_slice() else {
        return Err(SignatureFreeMachineResolutionError::NotUnique);
    };
    Ok(ExactSignatureFreeMachine { machine })
}

/// Resolve machine paths through ordinary namespace selection while retaining
/// signature-free ambiguity. Operator declarations are not exact-machine routes.
pub(crate) fn signature_free_machine_candidates<'program>(
    program: &'program SymbolResolvedTrees,
    machine_name: &str,
    use_span: source::SourceSpan,
) -> Vec<&'program symbol_resolved_trees::machine::Machine> {
    let selected = program
        .symbols
        .lookup_signature_free_top_level_from_source_matching(
            machine_name,
            &[symbols::SymbolKind::Machine],
            use_span,
            |symbol| {
                program
                    .machines
                    .iter()
                    .any(|machine| machine.symbol == symbol && machine.spelling.is_none())
            },
        );
    program
        .machines
        .iter()
        .filter(|machine| lookup_contains(selected, machine.symbol))
        .collect()
}

pub(crate) fn same_semantic_name(left: &str, right: &str) -> bool {
    left == right
        || (!left.contains("::") && right.rsplit("::").next().is_some_and(|leaf| leaf == left))
        || (!right.contains("::") && left.rsplit("::").next().is_some_and(|leaf| leaf == right))
}

/// Share ordinary namespace selection with type and call references. Scanning
/// same-leaf traits is insufficient: a loaded module grants no import exposure,
/// and a package-qualified path need not equal a declaration's display spelling.
/// The lookup retains two witnesses for ambiguity; callers need exactly one
/// owner before checking its requirement signatures.
pub(crate) fn signature_free_trait_candidates<'program>(
    program: &'program SymbolResolvedTrees,
    trait_name: &str,
    use_span: source::SourceSpan,
) -> Vec<&'program TraitDefinition> {
    let selected = program
        .symbols
        .lookup_signature_free_top_level_from_source_matching(
            trait_name,
            &[symbols::SymbolKind::Trait],
            use_span,
            |_| true,
        );
    program
        .traits
        .iter()
        .filter(|definition| lookup_contains(selected, definition.symbol))
        .collect()
}

fn lookup_contains(lookup: symbols::SymbolLookup, symbol: symbols::SymbolHandle) -> bool {
    match lookup {
        symbols::SymbolLookup::NotFound => false,
        symbols::SymbolLookup::Unique(selected) => selected == symbol,
        symbols::SymbolLookup::Ambiguous { first, second } => first == symbol || second == symbol,
    }
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
            // A path that uniquely names a machine declaration is an
            // exact-machine route; trait-requirement overload ambiguity does
            // not apply to it (cross-kind ambiguity rejects at normalization).
            if resolve_signature_free_machine(program, route).is_ok()
                && resolve_signature_free_requirement(program, route).is_err()
            {
                continue;
            }
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
    let matching_traits =
        signature_free_trait_candidates(program, &trait_name, requirement_name.source_span());
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
