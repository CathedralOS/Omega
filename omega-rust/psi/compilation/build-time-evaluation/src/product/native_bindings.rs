//! Native bindings a Build made: `Build::bind<Requirement>(builder, target,
//! locator)` says where one boundary requirement is found on one target
//! (`wiki/spec/build/foreign_bindings.md`). The Build names the requirement
//! without running anything of the Product, so it is held to the Product
//! here, when the Product is typed.
//!
//! Three steps share one reading of the request (`requirement`):
//!
//! - `collect` admits the bindings of the package being compiled.
//! - `capture_package_intents` turns a package's bindings into rows it hands
//!   to the packages that depend on it: the requirement as the source that
//!   declares it, and each executed binding as a target and a locator.
//! - `collect_captured` finds those requirements again in a consumer's own
//!   typing of that source and states the bindings there.
//!
//! A requirement applied to static arguments (a binding that closes the
//! arguments of a generic boundary) needs its prepared operand carried
//! beside it and is refused by name in each step.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    BuildOperation, CapturedNativeIntent, ExecutedNativeBinding, NativeBindingFormation,
    ProductDeclarationRequest, ProductOperandRole,
};
use semantic_vocabulary::PackageKeyIdentity;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::product_operand::{
    native_binding_formation, resolve_build_request_author, resolve_product_path,
};

const APPLIED: &str =
    "no product route yet: native binding admission: a requirement applied to static arguments";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeBindingSelection {
    pub application: Option<arena::Handle<typed_trees::product_operand::ProductOperandApplication>>,
    pub subject: SymbolHandle,
    pub requirement: SymbolHandle,
    pub target: language_semantics::declaration_selection::ProductTarget,
    pub locator: semantic_vocabulary::NativeBindingLocator,
    pub producer: language_semantics::declaration_selection::NativeBindingProducer,
    pub source_occurrence_digest: [u8; 32],
    pub supplier_package: PackageKeyIdentity,
    pub source_span: source::SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedNativeDeclaration {
    pub application: Option<crate::provider_declarations::RetainedProductApplication>,
    pub source: language_semantics::declaration_selection::NativeBindingSource,
    pub requirement: symbols::RetainedDeclaration,
    pub requirement_identity: String,
    pub dependency_alias: Option<String>,
}

/// The boundary trait and the requirement a binding request names.
fn requirement(
    typed: &TypedTrees,
    request: &ProductDeclarationRequest,
) -> Result<(SymbolHandle, SymbolHandle), Diagnostic> {
    let typed = typed.product_declarations();
    request.validate_shape().map_err(Diagnostic::error)?;
    if request.operation != BuildOperation::NativeBinding {
        return Err(Diagnostic::error(
            "native binding has the wrong designated operation",
        ));
    }
    let [operand] = request.operands.as_ref() else {
        return Err(Diagnostic::error(
            "native binding lost its exact requirement operand",
        ));
    };
    if operand.application.is_some() {
        return Err(Diagnostic::error(APPLIED).with_source_span(operand.source_span));
    }
    let symbol = resolve_product_path(
        typed,
        &operand.path,
        operand.source_span,
        ProductOperandRole::Requirement,
    )
    .ok_or_else(|| {
        Diagnostic::error(format!(
            "native binding operand `{}` does not resolve to one visible Product boundary requirement",
            operand.path
        ))
        .with_source_span(operand.source_span)
    })?;
    requirement_owner(typed, symbol).map_err(Diagnostic::error)
}

/// The bindings the Build of the package being compiled made, in the order
/// it made them. Every request it authored must name a requirement,
/// executed or not. Two bindings of one requirement for one target from one
/// supplier must name the same locator.
pub fn collect(
    typed: &TypedTrees,
    activation: Option<SymbolHandle>,
    declared: &[std::sync::Arc<ProductDeclarationRequest>],
    executed: &[ExecutedNativeBinding],
) -> Result<Vec<NativeBindingSelection>, Vec<Diagnostic>> {
    for request in declared
        .iter()
        .filter(|request| request.operation == BuildOperation::NativeBinding)
    {
        requirement(typed, request)
            .map_err(|error| vec![error.with_source_span(request.source_span)])?;
    }
    if executed.is_empty() {
        return Ok(Vec::new());
    }
    let root = activation
        .filter(|symbol| typed.machine(*symbol).is_some())
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "native binding has no authoritative completed Build activation",
            )]
        })?;
    let mut selections: Vec<NativeBindingSelection> = Vec::new();
    for event in executed {
        let span = event.request.source_span;
        let refuse = |reason: &str| vec![Diagnostic::error(reason).with_source_span(span)];
        if !declared.contains(&event.request) {
            return Err(refuse(
                "executed native binding has no retained source formation request",
            ));
        }
        let (subject, selected) = requirement(typed, &event.request)
            .map_err(|error| vec![error.with_source_span(span)])?;
        let author = resolve_build_request_author(typed, event.selecting_machine)
            .map_err(|reason| refuse(&reason))?;
        let supplier_package = typed
            .symbols
            .symbol_package_identity(author)
            .ok_or_else(|| refuse("native binding supplier has no immutable package identity"))?;
        if selections.iter().any(|prior| {
            prior.requirement == selected
                && prior.target == event.target
                && prior.supplier_package == supplier_package
                && prior.locator != event.locator
        }) {
            return Err(refuse(
                "conflicting native bindings for the same requirement, target and supplier precedence",
            ));
        }
        let scope = native_binding_formation(typed, author, Some(root), event.request.clone())
            .and_then(|formation| formation.seal(supplier_package))
            .map_err(|reason| refuse(&reason))?;
        selections.push(NativeBindingSelection {
            application: None,
            subject,
            requirement: selected,
            target: event.target,
            locator: event.locator.clone(),
            producer: scope.producer,
            source_occurrence_digest: scope.source_occurrence_digest,
            supplier_package,
            source_span: typed.symbols.rejoin_product_occurrence(span),
        });
    }
    Ok(selections)
}

/// A package's bindings as it hands them to its consumers: one declaration
/// row per authored binding, and one intent per executed binding naming its
/// declaration row, target and locator.
pub fn capture_package_intents(
    typed: &TypedTrees,
    package: PackageKeyIdentity,
    formations: &[NativeBindingFormation],
    executed: &[ExecutedNativeBinding],
) -> Result<(Vec<CapturedNativeDeclaration>, Vec<CapturedNativeIntent>), Vec<Diagnostic>> {
    let program = typed.product_declarations();
    let declarations = formations
        .iter()
        .map(|formation| {
            let source = formation
                .seal(package)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            let (_, selected) =
                requirement(program, &formation.request).map_err(|error| vec![error])?;
            let (owner, signature) =
                program.trait_machine_signature(selected).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "captured native requirement lost its Product signature",
                    )]
                })?;
            // The anchor is kept free of this typing's source numbering.
            let anchor = symbols::RetainedDeclaration::capture(&program.symbols, selected)
                .and_then(|anchor| {
                    symbols::RetainedDeclaration::from_retained_parts(
                        anchor.source().clone(),
                        anchor.span(),
                        anchor.kind(),
                    )
                })
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            // A requirement named through a direct dependency keeps the
            // alias it was named by, so a consumer can hold it to the same
            // dependency edge.
            let dependency_alias =
                source
                    .requirement_path
                    .split_once("::")
                    .and_then(|(alias, _)| {
                        program
                            .symbols
                            .source_map()
                            .and_then(|sources| sources.product_dependency_target(package, alias))
                            .map(|_| alias.to_owned())
                    });
            Ok(CapturedNativeDeclaration {
                application: None,
                source,
                requirement: anchor,
                requirement_identity: program
                    .trait_requirement_callable_identity_text(owner, signature),
                dependency_alias,
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    let intents = executed
        .iter()
        .map(|event| {
            let index = formations
                .iter()
                .position(|formation| {
                    formation.request == event.request
                        && formation.author == event.selecting_machine
                })
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "native effect lost its exact captured source formation and author",
                    )]
                })?;
            Ok(CapturedNativeIntent {
                declaration: u32::try_from(index).map_err(|_| {
                    vec![Diagnostic::error(
                        "native formation roster exceeds its coordinate",
                    )]
                })?,
                target: event.target,
                locator: event.locator.clone(),
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    Ok((declarations, intents))
}

/// The bindings a dependency handed over, among the declarations of this
/// typing. A requirement whose source this compilation did not load binds
/// nothing here. One it did load must be the same declaration, visible
/// under the supplier's own package and dependency edge, with the identity
/// it was captured with.
pub fn collect_captured(
    typed: &mut TypedTrees,
    declarations: &[CapturedNativeDeclaration],
    intents: &[CapturedNativeIntent],
) -> Result<Vec<NativeBindingSelection>, Vec<Diagnostic>> {
    let program = typed.product_declarations();
    let mut resolved = Vec::with_capacity(declarations.len());
    for declaration in declarations {
        let refuse = |reason: &str| {
            vec![Diagnostic::error(format!(
                "captured native requirement `{}`: {reason}",
                declaration.source.requirement_path,
            ))]
        };
        if declaration.application.is_some() {
            return Err(refuse(APPLIED));
        }
        let requirement_loaded = program.symbols.source_files().any(|file| {
            file.dependency_scope == source::DependencyScope::Product
                && program
                    .symbols
                    .source_map()
                    .is_none_or(|sources| sources.source_is_selected(file.source_id))
                && file.path == declaration.requirement.source().path
        });
        if !requirement_loaded {
            resolved.push(None);
            continue;
        }
        let selected = declaration
            .requirement
            .rejoin(
                &program.symbols,
                program
                    .traits()
                    .iter()
                    .filter(|owner| owner.is_boundary)
                    .flat_map(|owner| {
                        program
                            .trait_machine_signatures(owner)
                            .iter()
                            .filter(|signature| {
                                program.symbols.symbol_source_is_selected(signature.symbol)
                            })
                            .map(|signature| signature.symbol)
                    }),
            )
            .map_err(refuse)?;
        let (owner, _) = program
            .trait_machine_signature(selected)
            .ok_or_else(|| refuse("lost its exact Product signature"))?;
        let sources = program
            .symbols
            .source_map()
            .ok_or_else(|| refuse("has no receiving Product source custody"))?;
        let supplier = declaration.source.producer.package;
        let dependency = declaration
            .source
            .requirement_path
            .split_once("::")
            .and_then(|(alias, _)| {
                sources
                    .product_dependency_target(supplier, alias)
                    .map(|package| (alias, package))
            });
        if dependency.map(|(alias, _)| alias) != declaration.dependency_alias.as_deref() {
            return Err(refuse(
                "changed its original direct Product dependency scope",
            ));
        }
        let target = dependency.map_or(supplier, |(_, package)| package);
        if program.symbols.symbol_product_package_identity(selected) != Some(target)
            || dependency.is_some() && !owner.is_public
            || !source::SourceMap::captured_reference_can_see_stratum(
                supplier,
                declaration.source.resolution_stratum,
                declaration.requirement.source(),
            )
        {
            return Err(refuse(
                "is not visible under its original Product package and temporal scope",
            ));
        }
        let (subject, requirement) =
            requirement_owner(program, selected).map_err(|reason| refuse(&reason))?;
        let (owner, signature) = program
            .trait_machine_signature(requirement)
            .ok_or_else(|| refuse("lost its exact Product signature"))?;
        if program.trait_requirement_callable_identity_text(owner, signature)
            != declaration.requirement_identity
        {
            return Err(refuse("changed its retained complete requirement identity"));
        }
        resolved.push(Some((subject, requirement)));
    }
    let mut selections = Vec::with_capacity(intents.len());
    for intent in intents {
        let declaration = declarations
            .get(intent.declaration as usize)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "captured native effect lost its declared source occurrence",
                )]
            })?;
        let Some((subject, requirement)) = resolved[intent.declaration as usize] else {
            continue;
        };
        let span = program
            .symbols
            .symbol_provenance_source_span(requirement)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "captured native requirement lost its checked Product source",
                )]
            })?;
        selections.push(NativeBindingSelection {
            application: None,
            subject,
            requirement,
            target: intent.target,
            locator: intent.locator.clone(),
            producer: declaration.source.producer.clone(),
            supplier_package: declaration.source.producer.package,
            source_occurrence_digest: declaration.source.source_occurrence_digest,
            source_span: span,
        });
    }
    Ok(selections)
}

/// The boundary trait that declares `symbol` as one of its requirements. A
/// boundary with static parameters is bound only as a complete application.
fn requirement_owner(
    typed: &TypedTrees,
    symbol: SymbolHandle,
) -> Result<(SymbolHandle, SymbolHandle), String> {
    let owner = typed
        .traits()
        .iter()
        .find(|owner| {
            owner.is_boundary
                && typed
                    .trait_machine_signatures(owner)
                    .iter()
                    .any(|signature| signature.symbol == symbol)
        })
        .ok_or("native binding requirement lost its exact boundary declaration")?;
    if !typed.trait_type_parameters(owner).is_empty() || !owner.lifetime_parameters.is_empty() {
        return Err(APPLIED.into());
    }
    Ok((owner.symbol, symbol))
}
