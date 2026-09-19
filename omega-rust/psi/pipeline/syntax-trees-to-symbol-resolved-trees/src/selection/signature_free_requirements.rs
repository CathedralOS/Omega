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

/// Pool the machine candidates an occurrence may name under the same
/// module/dependency scope law as signature-free trait candidates: the
/// occurrence's own module (or unmoduled package frontier) wins, authored
/// imports outrank unrelated unmoduled declarations, and a genuinely
/// contested spelling stays `NotUnique` instead of silently selecting.
pub(crate) fn signature_free_machine_candidates<'program>(
    program: &'program SymbolResolvedTrees,
    machine_name: &str,
    use_span: source::SourceSpan,
) -> Vec<&'program symbol_resolved_trees::machine::Machine> {
    let candidates = program
        .machines
        .iter()
        .filter(|machine| {
            machine.spelling.is_none()
                && same_semantic_name(machine.name.as_str(), machine_name)
                && program
                    .symbols
                    .source_reference_can_see_symbol(use_span, machine.symbol)
        })
        .collect::<Vec<_>>();
    if use_span.span.start == use_span.span.end {
        return candidates;
    }
    let symbols = &program.symbols;
    let occurrence_module = symbols.source_module(use_span.source_id);
    let local = candidates
        .iter()
        .copied()
        .filter(|machine| {
            symbols.symbol_module(machine.symbol) == occurrence_module
                && symbols
                    .symbol_provenance_source_span(machine.symbol)
                    .is_some_and(|declaration| symbols.same_source_package(use_span, declaration))
        })
        .collect::<Vec<_>>();
    if !local.is_empty() {
        return local;
    }
    let imported = candidates
        .iter()
        .copied()
        .filter(|machine| {
            symbols
                .source_module_import_paths(use_span.source_id)
                .any(|path| {
                    symbols.source_module_import_target(use_span.source_id, path)
                        == Some(machine.symbol)
                })
        })
        .collect::<Vec<_>>();
    if !imported.is_empty() {
        return imported;
    }
    let unmoduled = candidates
        .iter()
        .copied()
        .filter(|machine| !symbols.symbol_module(machine.symbol).is_valid())
        .collect::<Vec<_>>();
    if !unmoduled.is_empty() {
        return unmoduled;
    }
    candidates
}

pub(crate) fn same_semantic_name(left: &str, right: &str) -> bool {
    left == right
        || (!left.contains("::") && right.rsplit("::").next().is_some_and(|leaf| leaf == left))
        || (!right.contains("::") && left.rsplit("::").next().is_some_and(|leaf| leaf == right))
}

/// Pool the trait candidates an occurrence may name under the
/// module/dependency scope law, retaining ambiguity inside the winning scope
/// instead of silently selecting.
///
/// A bare leaf spelling matches same-named traits program-wide, but only the
/// occurrence's own scope may claim it: declarations in the occurrence's
/// module (or its unmoduled package frontier, the flat-namespace fallback)
/// precede imported and unrelated foreign declarations. When no local
/// candidate exists, declarations selected by the occurrence's authored
/// imports outrank the remaining unmoduled pool. This mirrors the
/// precedence `SymbolTable::select_namespace_candidate` applies to ordinary
/// top-level references — including the package boundary that separates a
/// dependency's same-leaf trait from the importing package's own — while
/// this path keeps `TraitNotUnique` for genuinely contested spellings.
pub(crate) fn signature_free_trait_candidates<'program>(
    program: &'program SymbolResolvedTrees,
    trait_name: &str,
    use_span: source::SourceSpan,
) -> Vec<&'program TraitDefinition> {
    let candidates = program
        .traits
        .iter()
        .filter(|definition| {
            same_semantic_name(definition.name.as_str(), trait_name)
                && program
                    .symbols
                    .source_reference_can_see_symbol(use_span, definition.symbol)
        })
        .collect::<Vec<_>>();
    if use_span.span.start == use_span.span.end {
        return candidates;
    }
    let symbols = &program.symbols;
    let occurrence_module = symbols.source_module(use_span.source_id);
    let local = candidates
        .iter()
        .copied()
        .filter(|definition| {
            symbols.symbol_module(definition.symbol) == occurrence_module
                && symbols
                    .symbol_provenance_source_span(definition.symbol)
                    .is_some_and(|declaration| symbols.same_source_package(use_span, declaration))
        })
        .collect::<Vec<_>>();
    if !local.is_empty() {
        return local;
    }
    let imported = candidates
        .iter()
        .copied()
        .filter(|definition| {
            symbols
                .source_module_import_paths(use_span.source_id)
                .any(|path| {
                    symbols.source_module_import_target(use_span.source_id, path)
                        == Some(definition.symbol)
                })
        })
        .collect::<Vec<_>>();
    if !imported.is_empty() {
        return imported;
    }
    let unmoduled = candidates
        .iter()
        .copied()
        .filter(|definition| !symbols.symbol_module(definition.symbol).is_valid())
        .collect::<Vec<_>>();
    if !unmoduled.is_empty() {
        return unmoduled;
    }
    candidates
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
