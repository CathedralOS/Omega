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
//! Applied requirements retain their prepared operand through each step.
//! Package capture keeps its original declaration and complete argument graph;
//! receiving it must reconstruct the same canonical closed header.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    BuildOperation, CapturedNativeIntent, ExecutedNativeBinding, NativeBindingFormation,
    ProductDeclarationRequest, ProductOperandRole,
};
use semantic_vocabulary::PackageKeyIdentity;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::product_operand::{
    ProductDeclarationLookup, ProductRequestAuthors, native_binding_formation,
};

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

impl NativeBindingSelection {
    /// Publication needs the same boundary and closed arguments that Build
    /// selected. The evaluation receipt separately authenticates the locator.
    pub fn require_canonical_requirement(&self, typed: &TypedTrees) -> Result<(), String> {
        let (subject, requirement) = requirement_owner(typed, self.requirement)?;
        if subject != self.subject {
            return Err("native binding changes its declaring boundary trait".into());
        }
        if let Some(application) = self.application {
            let closed = typed.product_requirement_application(application)?;
            if closed.declaring_trait != subject || closed.signature.symbol != requirement {
                return Err("native binding application selects another closed requirement".into());
            }
        } else if typed.closed_requirement_application(requirement).is_some() {
            return Err("native binding lost its complete requirement application".into());
        }
        Ok(())
    }
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
    declarations: &ProductDeclarationLookup<'_>,
    request: &ProductDeclarationRequest,
    author: SymbolHandle,
) -> Result<
    (
        SymbolHandle,
        SymbolHandle,
        Option<arena::Handle<typed_trees::product_operand::ProductOperandApplication>>,
    ),
    Diagnostic,
> {
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
    let declaration = declarations.resolve(&operand.path, operand.source_span, ProductOperandRole::Requirement)
        .ok_or_else(|| Diagnostic::error(format!(
            "native binding operand `{}` does not resolve to one visible Product boundary requirement", operand.path
        )).with_source_span(operand.source_span))?;
    let (selected, application) = if operand.application.is_some() {
        let handle = typed
            .prepared_product_operand(operand, None, author, ProductOperandRole::Requirement)
            .ok_or_else(|| {
                Diagnostic::error("native requirement lost its prepared complete application")
                    .with_source_span(operand.source_span)
            })?;
        let closed = typed
            .product_requirement_application(handle)
            .map_err(|reason| Diagnostic::error(reason).with_source_span(operand.source_span))?;
        if closed.template != declaration {
            return Err(Diagnostic::error(
                "native requirement application belongs to another declaration",
            )
            .with_source_span(operand.source_span));
        }
        (closed.signature.symbol, Some(handle))
    } else {
        (declaration, None)
    };
    let (owner, selected) = requirement_owner(typed, selected).map_err(Diagnostic::error)?;
    Ok((owner, selected, application))
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
    formations: &[(std::sync::Arc<ProductDeclarationRequest>, SymbolHandle)],
) -> Result<Vec<NativeBindingSelection>, Vec<Diagnostic>> {
    if !declared
        .iter()
        .any(|request| request.operation == BuildOperation::NativeBinding)
        && executed.is_empty()
    {
        return Ok(Vec::new());
    }
    let declarations = ProductDeclarationLookup::new(typed, &[ProductOperandRole::Requirement]);
    let authors = ProductRequestAuthors::new(typed);
    let mut authors_by_request = formations
        .iter()
        .filter(|(request, _)| request.operation == BuildOperation::NativeBinding);
    for request in declared
        .iter()
        .filter(|request| request.operation == BuildOperation::NativeBinding)
    {
        let Some((formed, author)) = authors_by_request.next() else {
            return Err(vec![Diagnostic::error(
                "native binding lost its authored formation",
            )]);
        };
        if !typed.same_product_request_source(formed, request) {
            return Err(vec![Diagnostic::error(
                "native binding formation differs from its declared request",
            )]);
        }
        requirement(typed, &declarations, request, *author)
            .map_err(|error| vec![error.with_source_span(request.source_span)])?;
    }
    if authors_by_request.next().is_some() {
        return Err(vec![Diagnostic::error(
            "native binding has an undeclared formation",
        )]);
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
        let author = authors
            .resolve(event.selecting_machine)
            .map_err(|reason| refuse(&reason))?;
        let (subject, selected, application) =
            requirement(typed, &declarations, &event.request, author)
                .map_err(|error| vec![error.with_source_span(span)])?;
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
            application,
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
    if formations.is_empty() && executed.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let program = typed;
    let lookup = ProductDeclarationLookup::new(program, &[ProductOperandRole::Requirement]);
    let authors = ProductRequestAuthors::new(program);
    let declarations = formations
        .iter()
        .map(|formation| {
            let source = formation
                .seal(package)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            let author = authors
                .resolve(formation.author)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            let (_, selected, prepared) = requirement(program, &lookup, &formation.request, author)
                .map_err(|error| vec![error])?;
            let application = prepared
                .map(|handle| {
                    let identity = program
                        .product_operand_application_identity(handle)
                        .map_err(|reason| vec![Diagnostic::error(reason)])?
                        .ok_or_else(|| {
                            vec![Diagnostic::error(
                                "native application lost its canonical identity",
                            )]
                        })?;
                    crate::provider_declarations::RetainedProductApplication::capture(
                        program, handle, identity,
                    )
                    .map_err(|reason| vec![Diagnostic::error(reason)])
                })
                .transpose()?;
            let (owner, signature) =
                program.trait_machine_signature(selected).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "captured native requirement lost its Product signature",
                    )]
                })?;
            // The anchor is kept free of this typing's source numbering.
            let template = program
                .closed_requirement_application(selected)
                .map_or(selected, |closed| closed.template);
            let anchor = symbols::RetainedDeclaration::capture(&program.symbols, template)
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
                application,
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
    let mut resolved = Vec::with_capacity(declarations.len());
    for declaration in declarations {
        let program = &*typed;
        let refuse = |reason: &str| {
            vec![Diagnostic::error(format!(
                "captured native requirement `{}`: {reason}",
                declaration.source.requirement_path,
            ))]
        };
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
        // Rejoin has authenticated the complete source and declaration above.
        // Public toolchain Product declarations have no ordinary package owner;
        // they remain visible without inventing a dependency or package identity.
        let source = declaration.requirement.source();
        let public_toolchain_product = dependency.is_none()
            && owner.is_public
            && source.origin == source::SourceOrigin::Toolchain
            && source.dependency_scope == source::DependencyScope::Product;
        if !(program.symbols.symbol_product_package_identity(selected) == Some(target)
            || public_toolchain_product)
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
        let application = declaration
            .application
            .as_ref()
            .map(|application| {
                application
                    .prepare(
                        typed,
                        ProductOperandRole::Requirement,
                        selected,
                        &declaration.source.requirement_path,
                    )
                    .map_err(|error| vec![error])
            })
            .transpose()?;
        let requirement = application.map_or(selected, |handle| {
            typed.product_operand_applications.get(handle).declaration
        });
        let program = &*typed;
        let (subject, requirement) =
            requirement_owner(program, requirement).map_err(|reason| refuse(&reason))?;
        let (owner, signature) = program
            .trait_machine_signature(requirement)
            .ok_or_else(|| refuse("lost its exact Product signature"))?;
        if program.trait_requirement_callable_identity_text(owner, signature)
            != declaration.requirement_identity
        {
            return Err(refuse("changed its retained complete requirement identity"));
        }
        resolved.push(Some((subject, requirement, application)));
    }
    let program = &*typed;
    let mut selections = Vec::with_capacity(intents.len());
    for intent in intents {
        let declaration = declarations
            .get(intent.declaration as usize)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "captured native effect lost its declared source occurrence",
                )]
            })?;
        let Some((subject, requirement, application)) = resolved[intent.declaration as usize]
        else {
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
            application,
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

#[cfg(test)]
#[path = "native_bindings_tests.rs"]
mod tests;

/// The boundary trait that declares `symbol` as one of its requirements. A
/// boundary with static parameters is bound only as a complete application.
fn requirement_owner(
    typed: &TypedTrees,
    symbol: SymbolHandle,
) -> Result<(SymbolHandle, SymbolHandle), String> {
    let (owner, _) = typed
        .trait_machine_signature(symbol)
        .filter(|(owner, requirement)| owner.is_boundary && requirement.symbol == symbol)
        .ok_or("native binding requirement lost its exact boundary declaration")?;
    let declaration = typed
        .closed_requirement_application(symbol)
        .map_or(symbol, |closed| closed.template);
    if !typed
        .trait_machine_signatures(owner)
        .iter()
        .any(|signature| signature.symbol == declaration)
    {
        return Err("native binding requirement has no exact declaring requirement".into());
    }
    if !typed.trait_type_parameters(owner).is_empty() || !owner.lifetime_parameters.is_empty() {
        return Err("native binding needs its complete declaring trait application".into());
    }
    Ok((owner.symbol, symbol))
}
