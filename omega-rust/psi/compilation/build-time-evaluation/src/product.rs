/*
    Admission of Build-issued declarations about the Product: provider
    selections, native bindings, assumed bindings, wire demands, root grants,
    and behavior exclusions.

    We rejoin executed Build choices and target defaults against the exact
    declaration requests and lexical authority recorded in the Product source.
*/

pub mod assumed_bindings {
    use symbols::SymbolHandle;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct AssumedBindingContract {
        pub required: SymbolHandle,
        pub supplied: SymbolHandle,
        pub selecting_machine: SymbolHandle,
        pub source_span: source::SourceSpan,
    }
}
pub mod native_bindings {
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::{
        BuildOperation, ExecutedNativeBinding, ProductDeclarationRequest, ProductOperandRole,
        ProductTarget,
    };
    use semantic_vocabulary::{NativeBindingLocator, PackageKeyIdentity};
    use symbols::SymbolHandle;
    use typed_trees::product_operand::native_binding_formation;
    use typed_trees::{
        TypedTrees,
        product_operand::{resolve_build_request_author, resolve_product_path},
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct NativeBindingSelection {
        pub subject: SymbolHandle,
        pub requirement: SymbolHandle,
        /// Portable identity of the owning boundary's closed application
        /// (`Owner<Arguments>`); `None` for a boundary without parameters.
        /// Provider planning keys the supplied slot on it, so a use at
        /// another application finds no supply from this row.
        pub application_identity: Option<String>,
        pub target: ProductTarget,
        pub locator: NativeBindingLocator,
        pub producer: language_semantics::declaration_selection::NativeBindingProducer,
        pub source_occurrence_digest: [u8; 32],
        pub supplier_package: PackageKeyIdentity,
        pub source_span: source::SourceSpan,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct ResolvedRequirement {
        subject: SymbolHandle,
        requirement: SymbolHandle,
        application_identity: Option<String>,
    }

    /*
        Resolve the one requirement operand of a native binding.

        A bare `Owner::member` names a requirement of a boundary without
        parameters. `Owner<Arguments>::member` names the same member under one
        closed application of a generic boundary; preparing it needs the
        authoring Build machine, because the arguments are ordinary names in
        that machine's scope. The application is reduced to its portable
        identity text, which is what a use site in any package compares.
    */
    fn requirement(
        typed: &mut TypedTrees,
        request: &ProductDeclarationRequest,
        author: Option<SymbolHandle>,
    ) -> Result<ResolvedRequirement, Diagnostic> {
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
        if operand.application.is_none() {
            return requirement_bare(typed, request);
        }
        let refuse = |reason: &str| Diagnostic::error(reason).with_source_span(operand.source_span);
        let author = author.ok_or_else(|| {
            refuse("applied native binding requirement has no authoring Build machine")
        })?;
        let prepared = typed_trees_to_typed_trees::prepare_product_requirement_operand(
            typed, operand, author, None,
        )
        .map_err(|mut errors| {
            if errors.is_empty() {
                refuse("applied native binding requirement was refused")
            } else {
                errors.swap_remove(0)
            }
        })?;
        let (_, signature) = typed
            .trait_machine_signature(prepared.requirement)
            .ok_or_else(|| refuse("applied native binding requirement lost its signature"))?;
        if !typed.state_signature_type_parameters(signature).is_empty()
            || !signature.lifetime_parameters.is_empty()
        {
            return Err(refuse(
                "Build::bind cannot yet bind a requirement member that has static parameters of its own",
            ));
        }
        let scope = prepared.trait_scope.ok_or_else(|| {
            refuse(
                "an applied native binding requirement must apply its owning boundary, as in `Owner<Arguments>::member`",
            )
        })?;
        let identity = scope
            .scope(typed)
            .and_then(|scope| scope.portable_application_identity(typed))
            .and_then(|identity| typed.type_identity_text(identity))
            .ok_or_else(|| {
                refuse(
                    "native binding requires a closed static boundary application: every argument of the owning boundary must be a closed type",
                )
            })?;
        requirement_owner(typed, prepared.requirement, Some(identity))
            .map_err(|reason| refuse(&reason))
    }

    /// The bare `Owner::member` form, which needs no preparation.
    fn requirement_bare(
        typed: &TypedTrees,
        request: &ProductDeclarationRequest,
    ) -> Result<ResolvedRequirement, Diagnostic> {
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
            let symbol = resolve_product_path(typed, &operand.path, operand.source_span, ProductOperandRole::Requirement)
            .ok_or_else(|| Diagnostic::error(format!("native binding operand `{}` does not resolve to one visible Product boundary requirement", operand.path)).with_source_span(operand.source_span))?;
            requirement_owner(typed, symbol, None)
                .map_err(|reason| Diagnostic::error(reason).with_source_span(operand.source_span))
    }

    /*
        Find the Build machine whose body wrote an applied native request, so
        a request that was declared but never executed is still checked.
    */
    fn formation_author(
        typed: &TypedTrees,
        request: &ProductDeclarationRequest,
    ) -> Option<SymbolHandle> {
        if request
            .operands
            .iter()
            .all(|operand| operand.application.is_none())
        {
            return None;
        }
        let occurrences = typed_trees::product_operand::collect_product_selection_occurrences(typed);
        let mut owners = occurrences
            .iter()
            .filter(|occurrence| typed.same_product_request_source(&occurrence.request, request))
            .map(|occurrence| typed.symbols.authored_origin(occurrence.machine));
        let owner = owners.next()?;
        owners.all(|other| other == owner).then_some(owner)
    }

    pub fn collect(
        typed: &mut TypedTrees,
        activation: Option<SymbolHandle>,
        declared: &[std::sync::Arc<ProductDeclarationRequest>],
        executed: &[ExecutedNativeBinding],
    ) -> Result<Vec<NativeBindingSelection>, Vec<Diagnostic>> {
        for request in declared
            .iter()
            .filter(|request| request.operation == BuildOperation::NativeBinding)
        {
            let author = formation_author(typed, request);
            requirement(typed, request, author)
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
        let _ = root;
        let mut selections: Vec<NativeBindingSelection> = Vec::new();
        for event in executed {
            let span = event.request.source_span;
            let refuse = |reason| vec![Diagnostic::error(reason).with_source_span(span)];
            if !declared.contains(&event.request) {
                return Err(refuse(
                    "executed native binding has no retained source formation request",
                ));
            }
            let author = resolve_build_request_author(typed, event.selecting_machine)
                .map_err(|reason| vec![Diagnostic::error(reason).with_source_span(span)])?;
            let ResolvedRequirement {
                subject,
                requirement: selected,
                application_identity,
            } = requirement(typed, &event.request, Some(author))
                .map_err(|error| vec![error.with_source_span(span)])?;
            let supplier_package =
                typed
                    .symbols
                    .symbol_package_identity(author)
                    .ok_or_else(|| {
                        refuse("native binding supplier has no immutable package identity")
                    })?;
            if selections.iter().any(|prior| {
                prior.requirement == selected
                    && prior.application_identity == application_identity
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
                .map_err(|reason| vec![Diagnostic::error(reason).with_source_span(span)])?;
            selections.push(NativeBindingSelection {
                subject,
                requirement: selected,
                application_identity,
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

    mod capture {
        use diagnostics::Diagnostic;
        use language_semantics::declaration_selection::{
            CapturedNativeDeclaration, CapturedNativeIntent, ExecutedNativeBinding,
            NativeBindingFormation,
        };
        use semantic_vocabulary::PackageKeyIdentity;
        pub fn capture_package_intents(
            typed: &typed_trees::TypedTrees,
            package: PackageKeyIdentity,
            formations: &[NativeBindingFormation],
            executed: &[ExecutedNativeBinding],
        ) -> Result<(Vec<CapturedNativeDeclaration>, Vec<CapturedNativeIntent>), Vec<Diagnostic>>
        {
            let mut applied: Option<typed_trees::TypedTrees> = None;
            let declarations = formations
                .iter()
                .map(|formation| {
                    let source = formation
                        .seal(package)
                        .map_err(|reason| vec![Diagnostic::error(reason)])?;
                    let author = formation
                        .request
                        .operands
                        .iter()
                        .any(|operand| operand.application.is_some())
                        .then(|| {
                            typed_trees::product_operand::resolve_build_request_author(
                                typed,
                                formation.author,
                            )
                        })
                        .transpose()
                        .map_err(|reason| vec![Diagnostic::error(reason)])?;
                    let super::ResolvedRequirement {
                        requirement: selected,
                        application_identity,
                        ..
                    } = match author {
                        // Preparing an applied operand interns its argument
                        // types, and capture reads trees that are already
                        // sealed. Only the identity text and the declared
                        // requirement symbol leave this block, both valid in
                        // the sealed trees, so prepare on a private copy.
                        Some(_) => super::requirement(
                            applied.get_or_insert_with(|| typed.clone()),
                            &formation.request,
                            author,
                        ),
                        None => super::requirement_bare(typed, &formation.request),
                    }
                    .map_err(|error| vec![error])?;
                    let program = typed.product_declarations();
                    let (owner, signature) =
                        program.trait_machine_signature(selected).ok_or_else(|| {
                            vec![Diagnostic::error(
                                "captured native requirement lost its Product signature",
                            )]
                        })?;
                    let anchor = symbols::RetainedDeclaration::capture(&program.symbols, selected)
                        .and_then(|anchor| {
                            symbols::RetainedDeclaration::from_retained_parts(
                                anchor.source().clone(),
                                anchor.span(),
                                anchor.kind(),
                            )
                        })
                        .map_err(|reason| vec![Diagnostic::error(reason)])?;
                    let dependency_alias =
                        source
                            .requirement_path
                            .split_once("::")
                            .and_then(|(alias, _)| {
                                program
                                    .symbols
                                    .source_map()
                                    .and_then(|sources| {
                                        sources.product_dependency_target(package, alias)
                                    })
                                    .map(|_| alias.to_owned())
                            });
                    Ok(CapturedNativeDeclaration {
                        source,
                        requirement: anchor,
                        requirement_identity: program
                            .trait_requirement_callable_identity_text(owner, signature),
                        requirement_application: application_identity,
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
    }
    pub use capture::capture_package_intents;

    pub fn collect_captured(
        typed: &TypedTrees,
        declarations: &[language_semantics::declaration_selection::CapturedNativeDeclaration],
        intents: &[language_semantics::declaration_selection::CapturedNativeIntent],
    ) -> Result<Vec<NativeBindingSelection>, Vec<Diagnostic>> {
        let mut resolved = Vec::with_capacity(declarations.len());
        let program = typed.product_declarations();
        for declaration in declarations {
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
            let ResolvedRequirement {
                subject,
                requirement,
                application_identity,
            } = requirement_owner(
                program,
                selected,
                declaration.requirement_application.clone(),
            )
            .map_err(|reason| refuse(&reason))?;
            let (owner, signature) = program
                .trait_machine_signature(requirement)
                .ok_or_else(|| refuse("lost its exact Product signature"))?;
            if program.trait_requirement_callable_identity_text(owner, signature)
                != declaration.requirement_identity
            {
                return Err(refuse("changed its retained complete requirement identity"));
            }
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
            if program.symbols.symbol_product_package_identity(requirement) != Some(target)
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
            resolved.push(Some((subject, requirement, application_identity)));
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
            let Some((subject, requirement, application_identity)) =
                resolved[intent.declaration as usize].clone()
            else {
                continue;
            };
            let span = typed
                .symbols
                .symbol_provenance_source_span(requirement)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "captured native requirement lost its checked Product source",
                    )]
                })?;
            selections.push(NativeBindingSelection {
                subject,
                requirement,
                application_identity,
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

    /*
        Join a requirement to its boundary and to the application it was bound
        at. A generic boundary has one slot per closed application, so a member
        of it without an application names no slot and is refused; a boundary
        without parameters has exactly one slot and carries no application.
    */
    fn requirement_owner(
        typed: &TypedTrees,
        symbol: SymbolHandle,
        application_identity: Option<String>,
    ) -> Result<ResolvedRequirement, String> {
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
        let generic =
            !typed.trait_type_parameters(owner).is_empty() || !owner.lifetime_parameters.is_empty();
        if generic && application_identity.is_none() {
            return Err(format!(
                "native binding requires a closed static boundary application: `{}` has static parameters, so name the application it is bound at, as in `{}<Arguments>::member`",
                owner.name, owner.name,
            ));
        }
        Ok(ResolvedRequirement {
            subject: owner.symbol,
            requirement: symbol,
            application_identity: application_identity.filter(|_| generic),
        })
    }
}
pub mod product_declarations {
    use crate::product::assumed_bindings::AssumedBindingContract;
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::{
        BuildOperation, ExecutedProductSelection, ProductDeclarationRequest, ProductOperandRole,
    };
    use language_semantics::wire::WireCompatibilityDemand;
    use std::sync::Arc;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ServiceAuditRegistration {
        pub identity: String,
        pub name: String,
    }

    pub struct AdmittedProductDeclarations {
        pub service_audits: Vec<ServiceAuditRegistration>,
        pub assumed_bindings: Vec<AssumedBindingContract>,
        pub wire_demands: Vec<WireCompatibilityDemand>,
    }

    fn supported(operation: BuildOperation) -> bool {
        matches!(
            operation,
            BuildOperation::ServiceAudit
                | BuildOperation::AssumedBindingContract
                | BuildOperation::WireCompatibilityRequest
        )
    }

    fn operand(
        typed: &TypedTrees,
        request: &ProductDeclarationRequest,
        position: usize,
    ) -> Result<SymbolHandle, Diagnostic> {
        let selected = request
            .operands
            .get(position)
            .ok_or_else(|| Diagnostic::error("Build request lost its declaration operand"))?;
        let role = request
            .operation
            .product_operand_role(position)
            .filter(|role| *role != ProductOperandRole::WireCompatibilityFact)
            .ok_or_else(|| {
                Diagnostic::error("Build request has no declaration role at this position")
            })?;
        typed_trees::product_operand::resolve_product_path(typed, &selected.path, selected.source_span, role)
        .ok_or_else(|| Diagnostic::error(format!("Build declaration operand does not resolve to one visible product declaration of the required kind: `{}` ({role:?})", selected.path))
            .with_source_span(selected.source_span))
    }

    pub fn admit(
        typed: &TypedTrees,
        activation: Option<SymbolHandle>,
        declared: &[Arc<ProductDeclarationRequest>],
        executed: &[ExecutedProductSelection],
    ) -> Result<AdmittedProductDeclarations, Vec<Diagnostic>> {
        for request in declared
            .iter()
            .filter(|request| supported(request.operation))
        {
            request.validate_shape().map_err(|reason| {
                vec![Diagnostic::error(reason).with_source_span(request.source_span)]
            })?;
            for position in 0..request.operands.len() {
                if request.operation.product_operand_role(position)
                    != Some(ProductOperandRole::WireCompatibilityFact)
                {
                    operand(typed, request, position)
                        .map_err(|error| vec![error.with_source_span(request.source_span)])?;
                }
            }
        }
        let mut audits = Vec::<ServiceAuditRegistration>::new();
        let mut assumed = Vec::new();
        let mut wire = Vec::<WireCompatibilityDemand>::new();
        for event in executed
            .iter()
            .filter(|event| supported(event.request.operation))
        {
            let request = &event.request;
            let refuse =
                |reason| vec![Diagnostic::error(reason).with_source_span(request.source_span)];
            let root = activation
                .filter(|symbol| typed.machine(*symbol).is_some())
                .ok_or_else(|| refuse("Product selection has no authoritative Build activation"))?;
            if !declared.contains(request) || event.composition.is_some() {
                return Err(refuse(
                    "executed Product selection lost its source-issued formation request",
                ));
            }
            let author = typed_trees::product_operand::resolve_build_request_author(
                typed,
                event.selecting_machine.ok_or_else(|| {
                    refuse("executed Product selection lost its authored machine coordinate")
                })?,
            )
            .map_err(|reason| {
                vec![Diagnostic::error(reason).with_source_span(request.source_span)]
            })?;
            let subject = operand(typed, request, 0).map_err(|error| vec![error])?;
            match request.operation {
                BuildOperation::ServiceAudit => {
                    let declaration = typed
                        .trait_definition_by_symbol(subject)
                        .filter(|declaration| declaration.is_boundary)
                        .ok_or_else(|| {
                            refuse(
                                "Build service operand must name an exact visible boundary trait",
                            )
                        })?;
                    let identity = typed
                        .normalized_hermetic_symbol_identity(subject)
                        .map_err(|reason| vec![Diagnostic::error(reason)])?;
                    if !audits.iter().any(|audit| audit.identity == identity) {
                        audits.push(ServiceAuditRegistration {
                            identity,
                            name: declaration.name.as_str().to_owned(),
                        });
                    }
                }
                BuildOperation::AssumedBindingContract => {
                    if typed.symbols.symbol_package_identity(root)
                        != typed.symbols.symbol_package_identity(author)
                    {
                        return Err(refuse(
                            "assumed binding contract may be authorized only by the consuming root, not a dependency helper",
                        ));
                    }
                    assumed.push(AssumedBindingContract {
                        required: subject,
                        supplied: operand(typed, request, 1).map_err(|error| vec![error])?,
                        selecting_machine: author,
                        source_span: typed.symbols.rejoin_product_occurrence(request.source_span),
                    });
                }
                BuildOperation::WireCompatibilityRequest => {
                    let path = |position| {
                        operand(typed, request, position)
                            .map(|symbol| typed.symbols.display_path(symbol, "::"))
                    };
                    let mut demand = language_semantics::wire::WireCompatibilityDemand {
                        edge: path(0).map_err(|error| vec![error])?,
                        lineage: path(1).map_err(|error| vec![error])?,
                        local_schema: path(2).map_err(|error| vec![error])?,
                        peer_schema: path(3).map_err(|error| vec![error])?,
                        require_readable: false,
                        require_writable: false,
                        require_unknown_preservation: false,
                        require_canonical: false,
                        require_complete_migration: false,
                    };
                    for fact in &request.operands[4..] {
                        use language_semantics::wire::WireCompatibilityFact as Fact;
                        *match Fact::from_name(&fact.path).ok_or_else(|| {
                            refuse("Build wire demand names an unknown compatibility fact")
                        })? {
                            Fact::Readable => &mut demand.require_readable,
                            Fact::Writable => &mut demand.require_writable,
                            Fact::PreserveUnknown => &mut demand.require_unknown_preservation,
                            Fact::Canonical => &mut demand.require_canonical,
                            Fact::CompleteMigration => &mut demand.require_complete_migration,
                        } = true;
                    }
                    if wire.iter().any(|existing| {
                        existing.edge == demand.edge
                            && existing.lineage == demand.lineage
                            && existing.local_schema == demand.local_schema
                            && existing.peer_schema == demand.peer_schema
                    }) {
                        return Err(refuse(
                            "wire compatibility demand repeats the same edge, lineage and schema coordinates",
                        ));
                    }
                    wire.push(demand);
                }
                _ => {
                    return Err(refuse(
                        "Product request is not an admitted declaration operation",
                    ));
                }
            }
        }
        Ok(AdmittedProductDeclarations {
            service_audits: audits,
            assumed_bindings: assumed,
            wire_demands: wire,
        })
    }
}
pub mod product_policy {
    mod exclusions {
        use crate::is_exact_toolchain_build_prelude_data;
        use crate::unique::push_unique;
        use diagnostics::Diagnostic;
        use language_semantics::declaration_selection::{
            AuthoredBehaviorExclusion, AuthoredBehaviorExclusionKind, BuildOperation,
            ExclusionVocabulary, ExecutedEnumExclusion, ExecutedProductSelection,
            ProductDeclarationRequest,
        };
        use symbols::SymbolHandle;
        use typed_trees::TypedTrees;
        use typed_trees::data::DataMember;

        pub fn resolve(
            typed: &TypedTrees,
            activation: Option<SymbolHandle>,
            declared: &[std::sync::Arc<ProductDeclarationRequest>],
            executed: &[ExecutedProductSelection],
            enums: &[ExecutedEnumExclusion],
        ) -> Result<Vec<AuthoredBehaviorExclusion>, Vec<Diagnostic>> {
            for request in declared
                .iter()
                .filter(|request| request.operation == BuildOperation::ServiceExclusion)
            {
                service(typed, request)
                    .map_err(|error| vec![error.with_source_span(request.source_span)])?;
            }
            let mut exclusions = Vec::new();
            let mut retain = |kind, source_span, machine_span| -> Result<(), Diagnostic> {
                activation
                    .filter(|symbol| typed.machine(*symbol).is_some())
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "behavior exclusion has no authoritative Build activation",
                        )
                    })?;
                let selecting_machine =
                    typed_trees::product_operand::resolve_build_request_author(typed, machine_span)
                        .map_err(Diagnostic::error)?;
                let row = AuthoredBehaviorExclusion {
                    kind,
                    source_span: typed.symbols.rejoin_product_occurrence(source_span),
                    selecting_machine,
                };
                push_unique(&mut exclusions, row);
                Ok(())
            };
            for event in executed
                .iter()
                .filter(|event| event.request.operation == BuildOperation::ServiceExclusion)
            {
                let result = (|| {
                    if !declared.contains(&event.request) || event.composition.is_some() {
                        return Err(Diagnostic::error(
                            "executed service exclusion lost its source-issued declaration request",
                        ));
                    }
                    retain(
                        service(typed, &event.request)?,
                        event.request.source_span,
                        event.selecting_machine.ok_or_else(|| {
                            Diagnostic::error(
                                "executed service exclusion lost its authored machine coordinate",
                            )
                        })?,
                    )
                })();
                result.map_err(|error| vec![error.with_source_span(event.request.source_span)])?;
            }
            for event in enums {
                let result = (|| {
                    let vocabulary = event.request.vocabulary;
                    let case = usize::try_from(event.code)
                        .ok()
                        .and_then(|code| vocabulary.cases().get(code))
                        .ok_or_else(|| {
                            Diagnostic::error(
                                "executed behavior exclusion has an invalid vocabulary code",
                            )
                        })?;
                    let declaration = typed
                        .data_definitions()
                        .iter()
                        .find(|definition| {
                            is_exact_toolchain_build_prelude_data(
                                typed,
                                definition.symbol,
                                vocabulary.type_name(),
                            )
                        })
                        .ok_or_else(|| {
                            Diagnostic::error("behavior exclusion lost its exact toolchain enum")
                        })?;
                    let case_symbol = typed
                        .data_members(declaration)
                        .iter()
                        .find_map(|member| match member {
                            DataMember::Variant(variant) if variant.name.as_str() == *case => {
                                Some(variant.symbol)
                            }
                            _ => None,
                        })
                        .ok_or_else(|| {
                            Diagnostic::error("behavior exclusion lost its exact toolchain case")
                        })?;
                    let kind = match vocabulary {
                        ExclusionVocabulary::CrashCause => {
                            AuthoredBehaviorExclusionKind::CrashCause {
                                cause: evaluated_crash_cause(typed, case_symbol)?,
                                case_symbol,
                            }
                        }
                        ExclusionVocabulary::PhysicalAuthorityClass => {
                            AuthoredBehaviorExclusionKind::PhysicalAuthorityClass {
                                class: evaluated_physical_authority_class(typed, case_symbol)?,
                                case_symbol,
                            }
                        }
                    };
                    retain(kind, event.request.source_span, event.selecting_machine)
                })();
                result.map_err(|error| vec![error.with_source_span(event.request.source_span)])?;
            }
            Ok(exclusions)
        }

        fn service(
            typed: &TypedTrees,
            request: &ProductDeclarationRequest,
        ) -> Result<AuthoredBehaviorExclusionKind, Diagnostic> {
            let [operand] = request.operands.as_ref() else {
                return Err(Diagnostic::error(
                    "service exclusion requires one exact Product boundary trait",
                ));
            };
            let symbol = typed_trees::product_operand::resolve_product_path(
        typed, &operand.path, operand.source_span,
        typed_trees::product_operand::ProductOperandKind::BoundarySubject,
    ).ok_or_else(|| Diagnostic::error("service exclusion does not resolve to an exact visible product boundary trait declaration"))?;
            let definition = typed
                .product_declarations()
                .trait_definition_by_symbol(symbol)
                .filter(|definition| definition.is_boundary)
                .ok_or_else(|| {
                    Diagnostic::error("service exclusion requires an exact boundary trait")
                })?;
            let identity = typed
                .normalized_hermetic_symbol_identity(definition.symbol)
                .map_err(|error| {
                    Diagnostic::error(format!("service exclusion declaration identity: {error}"))
                })?;
            Ok(AuthoredBehaviorExclusionKind::Service {
                identity,
                name: definition.name.as_str().to_owned(),
            })
        }

        fn evaluated_crash_cause(
            typed: &TypedTrees,
            case_symbol: SymbolHandle,
        ) -> Result<semantic_vocabulary::CrashCause, Diagnostic> {
            match evaluated_exclusion_case(typed, case_symbol, "CrashCause")? {
                "Trap" => Ok(semantic_vocabulary::CrashCause::Trap),
                "Abort" => Ok(semantic_vocabulary::CrashCause::Abort),
                other => Err(Diagnostic::error(format!(
                    "compiler-owned CrashCause contains unsupported case `{other}`"
                ))),
            }
        }

        fn evaluated_physical_authority_class(
            typed: &TypedTrees,
            case_symbol: SymbolHandle,
        ) -> Result<semantic_vocabulary::TerminalAuthorityClass, Diagnostic> {
            use semantic_vocabulary::TerminalAuthorityClass;
            match evaluated_exclusion_case(typed, case_symbol, "PhysicalAuthorityClass")? {
                "FilesystemContentRead" => Ok(TerminalAuthorityClass::FilesystemContentRead),
                "FilesystemContentWrite" => Ok(TerminalAuthorityClass::FilesystemContentWrite),
                "FilesystemMetadataQuery" => Ok(TerminalAuthorityClass::FilesystemMetadataQuery),
                "DirectoryEnumeration" => Ok(TerminalAuthorityClass::DirectoryEnumeration),
                "FilesystemNamespaceMutation" => {
                    Ok(TerminalAuthorityClass::FilesystemNamespaceMutation)
                }
                "FilesystemMetadataMutation" => {
                    Ok(TerminalAuthorityClass::FilesystemMetadataMutation)
                }
                "ProcessOutput" => Ok(TerminalAuthorityClass::ProcessOutput),
                "ProcessTermination" => Ok(TerminalAuthorityClass::ProcessTermination),
                "MachineControl" => Ok(TerminalAuthorityClass::MachineControl),
                "PortIo" => Ok(TerminalAuthorityClass::PortIo),
                "InterruptControl" => Ok(TerminalAuthorityClass::InterruptControl),
                "InterruptEntry" => Ok(TerminalAuthorityClass::InterruptEntry),
                "RootMemoryAccess" => Ok(TerminalAuthorityClass::RootMemoryAccess),
                "ProcessInput" => Ok(TerminalAuthorityClass::ProcessInput),
                other => Err(Diagnostic::error(format!(
                    "compiler-owned PhysicalAuthorityClass contains unsupported case `{other}`"
                ))),
            }
        }

        fn evaluated_exclusion_case<'a>(
            typed: &'a TypedTrees,
            case_symbol: SymbolHandle,
            type_name: &str,
        ) -> Result<&'a str, Diagnostic> {
            let exact_types = typed
                .data_definitions()
                .iter()
                .filter(|definition| {
                    is_exact_toolchain_build_prelude_data(typed, definition.symbol, type_name)
                })
                .collect::<Vec<_>>();
            let [declaration] = exact_types.as_slice() else {
                return Err(Diagnostic::error(format!(
                    "behavior exclusion requires exactly one compiler-owned {type_name} declaration"
                )));
            };
            let selected = typed
                .data_members(declaration)
                .iter()
                .filter_map(|member| match member {
                    DataMember::Variant(variant)
                        if variant.symbol == case_symbol
                            && typed.symbols.get(variant.symbol).parent == declaration.symbol =>
                    {
                        Some(variant)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let [selected] = selected.as_slice() else {
                return Err(Diagnostic::error(format!(
                    "executed behavior exclusion does not name an exact compiler-owned {type_name} case"
                )));
            };
            if !typed.data_payload_fields(selected).is_empty() {
                return Err(Diagnostic::error(format!(
                    "compiler-owned {type_name} case unexpectedly carries a payload"
                )));
            }
            Ok(selected.name.as_str())
        }
    }
    mod root_grants {
        use crate::unique::push_unique;
        use diagnostics::Diagnostic;
        use language_semantics::declaration_selection::{
            AuthoredRootGrant, BuildOperation, ExecutedProductSelection, ProductDeclarationRequest,
        };
        use symbols::SymbolHandle;
        use typed_trees::TypedTrees;
        use typed_trees::product_operand::{ProductOperandKind, resolve_product_path};

        pub fn collect_product_root_grants(
            typed: &TypedTrees,
            activation: Option<SymbolHandle>,
            declared: &[std::sync::Arc<ProductDeclarationRequest>],
            executed: &[ExecutedProductSelection],
        ) -> Result<Vec<AuthoredRootGrant>, Vec<Diagnostic>> {
            for request in declared
                .iter()
                .filter(|request| request.operation == BuildOperation::BoundaryAcceptance)
            {
                subject(typed, request).map_err(|error| vec![error])?;
            }
            let mut grants = Vec::new();
            for event in executed
                .iter()
                .filter(|event| event.request.operation == BuildOperation::BoundaryAcceptance)
            {
                let result = (|| {
                    if !declared.contains(&event.request) || event.composition.is_some() {
                        return Err(Diagnostic::error(
                            "boundary acceptance lost its source-issued declaration request",
                        ));
                    }
                    activation
                        .filter(|symbol| typed.machine(*symbol).is_some())
                        .ok_or_else(|| {
                            Diagnostic::error(
                                "boundary acceptance has no authoritative Build activation",
                            )
                        })?;
                    let machine_span = event.selecting_machine.ok_or_else(|| {
                        Diagnostic::error(
                            "boundary acceptance lost its authored machine coordinate",
                        )
                    })?;
                    let selecting_machine =
                        typed_trees::product_operand::resolve_build_request_author(
                            typed,
                            machine_span,
                        )
                        .map_err(Diagnostic::error)?;
                    let selected = subject(typed, &event.request)?;
                    Ok(AuthoredRootGrant {
                        selector: typed
                            .product_declarations()
                            .symbols
                            .display_path(selected, "::"),
                        selecting_machine,
                        source_span: typed
                            .symbols
                            .rejoin_product_occurrence(event.request.source_span),
                    })
                })();
                let grant = result.map_err(|error: Diagnostic| {
                    vec![error.with_source_span(event.request.source_span)]
                })?;
                push_unique(&mut grants, grant);
            }
            Ok(grants)
        }

        fn subject(
            typed: &TypedTrees,
            request: &ProductDeclarationRequest,
        ) -> Result<SymbolHandle, Diagnostic> {
            let [operand] = request.operands.as_ref() else {
                return Err(
                    Diagnostic::error("boundary acceptance requires one Product subject")
                        .with_source_span(request.source_span),
                );
            };
            resolve_product_path(
                typed,
                &operand.path,
                operand.source_span,
                ProductOperandKind::BoundarySubject,
            )
            .ok_or_else(|| {
                Diagnostic::error(format!(
            "boundary acceptance `{}` does not resolve to one visible Product boundary subject",
            operand.path,
        ))
        .with_source_span(operand.source_span)
            })
        }
    }

    pub use exclusions::resolve as collect_behavior_exclusions;
    pub use root_grants::collect_product_root_grants;
}
pub mod provider_declarations {
    use checked_trees::typed_trees::expression::{ExpressionNode, StaticMachineArgument};
    use checked_trees::typed_trees::statement::StatementNode;
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionKind;
    use source::DependencyScope;

    mod admission {
        use super::{
            CompositionMode, ProviderOperatorFamilySelection, ProviderPlanningContext,
            ProviderSelection, ProviderSelectionIdentity, ProviderSelectionSubject, identities,
        };
        use diagnostics::Diagnostic;
        use language_semantics::declaration_selection::ProductOperandRole;
        use symbols::SymbolKind;

        pub(super) struct ProviderDeclarations {
            subject: ProviderSelectionSubject,
            provider_type: ProviderSelectionIdentity,
        }

        pub(super) fn form(
            context: ProviderPlanningContext<'_>,
            boundary_identity: ProviderSelectionIdentity,
            provider_type: ProviderSelectionIdentity,
        ) -> Result<ProviderDeclarations, Diagnostic> {
            let typed = context.typed;
            if !context.contains(boundary_identity.symbol)
                || !context.contains(provider_type.symbol)
            {
                return Err(Diagnostic::error(format!(
                    "provider selection subject or provider is outside the {:?} planning context",
                    context.scope
                )));
            }
            let subject = if boundary_identity.symbol.is_valid()
                && typed.symbols.get(boundary_identity.symbol).kind == SymbolKind::Trait
                && typed.traits().iter().any(|definition| {
                    definition.symbol == boundary_identity.symbol && definition.is_boundary
                }) {
                ProviderSelectionSubject::BoundaryTrait(boundary_identity)
            } else if boundary_identity.symbol.is_valid()
                && typed.symbols.get(boundary_identity.symbol).kind == SymbolKind::Machine
                && typed.machines().iter().any(|requirement| {
                    requirement.symbol == boundary_identity.symbol
                        && requirement.spelling.is_none()
                        && requirement.supply_mode
                            == language_semantics::MachineSupplyMode::TopLevelRequirement
                })
            {
                ProviderSelectionSubject::BoundaryRequirement(boundary_identity)
            } else if boundary_identity.symbol.is_valid()
                && (typed.symbols.get(boundary_identity.symbol).kind == SymbolKind::Operator
                    || typed.machine_token_bindings().iter().any(|binding| {
                        binding.is_boundary && binding.symbol == boundary_identity.symbol
                    }))
            {
                match ProviderOperatorFamilySelection::derive(
                    context,
                    boundary_identity.symbol,
                    boundary_identity.authored_path,
                ) {
                    Ok(family) => ProviderSelectionSubject::BoundaryOperatorFamily(family),
                    Err(reason) => {
                        return Err(Diagnostic::error(reason));
                    }
                }
            } else {
                return Err(Diagnostic::error(format!(
                    "provider selection subject `{}` does not resolve to an exact boundary trait, top-level boundary requirement, or boundary-operator family",
                    boundary_identity.authored_path
                )));
            };
            if !provider_type.symbol.is_valid()
                || (typed.symbols.get(provider_type.symbol).kind != SymbolKind::Data
                    && !identities::is_named_provider(typed, provider_type.symbol))
            {
                return Err(Diagnostic::error(format!(
                    "provider selection type `{}` does not resolve to an exact provider data declaration or named implementation",
                    provider_type.authored_path
                )));
            }
            if let Some(implementation) = typed.conformances().iter().find(|implementation| {
                implementation.symbol == provider_type.symbol
                    && identities::eligible(implementation)
            }) {
                for row in &implementation.rows {
                    if identities::named_requirement_lifetime_arguments(
                        typed,
                        implementation,
                        row.declaring_trait,
                    )
                    .is_none()
                    {
                        return Err(Diagnostic::error(format!(
                            "named provider `{}` requires an unsupported inherited declaring-trait lifetime application for `{}`; the exact parent substitution is not retained",
                            provider_type.authored_path, row.requirement_name,
                        )));
                    }
                }
            }
            Ok(ProviderDeclarations {
                subject,
                provider_type,
            })
        }

        pub(super) fn admit(
            context: ProviderPlanningContext<'_>,
            boundary_identity: ProviderSelectionIdentity,
            provider_type: ProviderSelectionIdentity,
            composition_mode: CompositionMode,
            target: Option<language_semantics::declaration_selection::ProductTarget>,
            origin: language_semantics::declaration_selection::BuildSelectionOrigin,
            previous: &[ProviderSelection],
        ) -> Result<ProviderSelection, Diagnostic> {
            for (identity, role) in [
                (&boundary_identity, ProductOperandRole::BoundarySubject),
                (&provider_type, ProductOperandRole::Provider),
            ] {
                let has_static_telescope = context
                    .typed
                    .trait_definition_by_symbol(identity.symbol)
                    .is_some_and(|definition| {
                        !context.typed.trait_type_parameters(definition).is_empty()
                    })
                    || context
                        .typed
                        .conformance_by_symbol(identity.symbol)
                        .is_some_and(|definition| {
                            !context
                                .typed
                                .conformance_type_parameters(definition)
                                .is_empty()
                        });
                if identity.applied
                    && has_static_telescope
                    && (!identity.application.is_valid() || identity.application_identity.is_none())
                {
                    return Err(Diagnostic::error(
                        "executed provider choice lost its complete static application",
                    ));
                }
                if identity.application_identity.is_some() || identity.application.is_valid() {
                    if !context
                        .typed
                        .product_operand_applications
                        .is_valid(identity.application)
                    {
                        return Err(Diagnostic::error(
                            "executed provider application has no current prepared source input",
                        ));
                    }
                    let application = context
                        .typed
                        .product_operand_applications
                        .get(identity.application);
                    let actual = context
                        .typed
                        .product_operand_application_identity(identity.application)
                        .map_err(Diagnostic::error)?
                        .and_then(|identity| context.typed.type_identity_text(identity));
                    if application.declaration != identity.symbol
                        || application.role != Some(role)
                        || actual != identity.application_identity
                    {
                        return Err(Diagnostic::error(
                            "executed provider choice differs from its exact prepared application",
                        ));
                    }
                }
            }
            let ProviderDeclarations {
                subject,
                provider_type,
            } = form(context, boundary_identity, provider_type)?;
            if let Some(existing) = previous.iter().find(|selection| {
                selection.target == target && selection.subject.same_declaration_as(&subject)
            }) {
                if existing.provider_type.symbol != provider_type.symbol
                    || existing.provider_type.application_identity
                        != provider_type.application_identity
                {
                    return Err(Diagnostic::error(format!(
                        "build selects two provider types for slot `{}`: `{}` and `{}`",
                        subject.canonical_path(),
                        existing.provider_type.canonical_path,
                        provider_type.canonical_path,
                    )));
                }
                if existing.composition_mode != composition_mode {
                    return Err(Diagnostic::error(format!(
                        "build selects provider `{}` for slot `{}` with conflicting composition modes {:?} and {:?}",
                        provider_type.canonical_path,
                        subject.canonical_path(),
                        existing.composition_mode,
                        composition_mode,
                    )));
                }
            }
            Ok(ProviderSelection {
                target,
                subject,
                provider_type,
                composition_mode,
                origin,
            })
        }
    }
    mod checking_applications {
        use semantic_vocabulary::proof_applications::ProofApplications;

        mod calls {
            use super::schemas::Schema;
            use symbols::SymbolHandle;
            use typed_trees::{
                TypedTrees, expression::ExpressionNode, references::MachineSidecarIndex,
                trait_definition::AppliedTraitScope,
            };

            pub(super) struct Demand {
                pub called: SymbolHandle,
                pub requirement: SymbolHandle,
                pub schema: Schema,
            }

            pub(super) fn collect(
                program: &TypedTrees,
                machines: &[SymbolHandle],
                calls: &[SymbolHandle],
            ) -> Vec<Demand> {
                let sidecars = MachineSidecarIndex::build(program);
                let mut demands = Vec::new();
                let mut add = |owner, called, qualifier, application| {
                    if !calls.contains(&called) {
                        return;
                    }
                    if let Some(demand) = describe(program, owner, called, qualifier, application)
                        && !demands.iter().any(|known: &Demand| {
                            known.called == demand.called && known.schema.key == demand.schema.key
                        })
                    {
                        demands.push(demand);
                    }
                };
                for &symbol in machines {
                    let Some(machine) = program.machine_by_symbol(symbol) else {
                        continue;
                    };
                    let references = typed_trees::references::collect_machine_references(
                        program, machine, &sidecars,
                    );
                    let mut expressions = references.expressions.into_iter().collect::<Vec<_>>();
                    expressions
                        .sort_unstable_by_key(|handle| (handle.arena_index(), handle.generation()));
                    for expression in expressions {
                        if let ExpressionNode::Call(call) =
                            program.expression_table.expression(expression)
                        {
                            add(
                                machine.symbol,
                                call.target_symbol,
                                validation::call_receiver_parts(program, call.receiver).0,
                                call.requirement_application,
                            );
                        }
                    }
                    let mut statements = references.statements.into_iter().collect::<Vec<_>>();
                    statements
                        .sort_unstable_by_key(|handle| (handle.arena_index(), handle.generation()));
                    for statement in statements {
                        if let Some(call) = program.statement_table.statement(statement).as_call() {
                            add(
                                machine.symbol,
                                call.target_symbol,
                                call.receiver_symbol,
                                Default::default(),
                            );
                        }
                    }
                }
                for &called in calls {
                    if demands.iter().any(|demand| demand.called == called) {
                        continue;
                    }
                    if let Some(demand) = describe(
                        program,
                        SymbolHandle::invalid(),
                        called,
                        SymbolHandle::invalid(),
                        Default::default(),
                    ) {
                        demands.push(demand);
                    }
                }
                demands
            }

            fn describe(
                program: &TypedTrees,
                owner: SymbolHandle,
                called: SymbolHandle,
                qualifier: SymbolHandle,
                application: typed_trees::types::TypeReferenceHandle,
            ) -> Option<Demand> {
                let closed = program.closed_requirement_application(called);
                let requirement = closed
                    .as_ref()
                    .map_or(called, |application| application.template);
                let scope = application
                    .is_valid()
                    .then(|| AppliedTraitScope::from_type_reference(program, application, owner))
                    .flatten()
                    .or_else(|| {
                        closed
                            .as_ref()
                            .and_then(|application| application.trait_scope.scope(program))
                    })
                    .or_else(|| validation::qualified_trait_scope(program, owner, qualifier, None))
                    .or_else(|| {
                        program
                            .trait_machine_signature(requirement)
                            .map(|(declaration, _)| AppliedTraitScope::abstract_scope(declaration))
                    });
                let schema = match scope {
                    Some(scope) if scope.declaration.is_boundary => {
                        Schema::trait_scope(program, scope)?
                    }
                    Some(_) => return None,
                    None => Schema::exact(program, requirement)?,
                };
                Some(Demand {
                    called,
                    requirement,
                    schema,
                })
            }
        }
        mod declarations {
            use super::super::{
                ProviderSelection, ProviderSelectionIdentity, ProviderSelectionSubject,
            };
            use super::schemas::Key;
            use language_semantics::declaration_selection::ProductOperandRole;
            use source::DependencyScope;
            use typed_trees::TypedTrees;

            pub(super) struct Declaration<'source> {
                pub source: &'source ProviderSelection,
                provider: Key,
            }
            impl<'source> Declaration<'source> {
                pub fn new(
                    program: &TypedTrees,
                    source: &'source ProviderSelection,
                ) -> Result<Self, String> {
                    validate_identity(
                        program,
                        &source.provider_type,
                        ProductOperandRole::Provider,
                    )?;
                    match &source.subject {
                        ProviderSelectionSubject::BoundaryTrait(identity)
                        | ProviderSelectionSubject::BoundaryRequirement(identity) => {
                            validate_identity(
                                program,
                                identity,
                                ProductOperandRole::BoundarySubject,
                            )?
                        }
                        ProviderSelectionSubject::BoundaryOperatorFamily(family) => {
                            let errors = family.replay_against_typed(
                                super::super::ProviderPlanningContext::new(
                                    program,
                                    DependencyScope::Product,
                                ),
                            );
                            if !errors.is_empty() {
                                return Err(errors.join("\n"));
                            }
                        }
                    }
                    Ok(Self {
                        source,
                        provider: (
                            source.provider_type.package,
                            source.provider_type.canonical_path.clone(),
                            source.provider_type.application_identity.clone(),
                        ),
                    })
                }
            }
            impl Declaration<'_> {
                pub fn slots(&self, available: &[Key]) -> Vec<Key> {
                    match &self.source.subject {
            ProviderSelectionSubject::BoundaryTrait(identity)
            | ProviderSelectionSubject::BoundaryRequirement(identity) => {
                language_semantics::declaration_selection::provider_choices::selected_application_slots(
                    &(identity.package, identity.canonical_path.clone(), identity.application_identity.clone()),
                    available,
                )
            }
            ProviderSelectionSubject::BoundaryOperatorFamily(family) => family
                .coordinates()
                .iter()
                .map(|coordinate| {
                    (
                        family.package,
                        coordinate.requirement_identity.clone(),
                        None,
                    )
                })
                .collect(),
        }
                }
                pub fn provider(&self) -> &Key {
                    &self.provider
                }
            }

            fn validate_identity(
                program: &TypedTrees,
                identity: &ProviderSelectionIdentity,
                role: ProductOperandRole,
            ) -> Result<(), String> {
                if !identity.symbol.is_valid()
                    || program.symbols.symbol_package_identity(identity.symbol) != identity.package
                    || program.symbols.display_path(identity.symbol, "::")
                        != identity.canonical_path
                {
                    return Err(
                        "source provider description changed its exact nominal owner".into(),
                    );
                }
                if identity.application.is_valid() || identity.application_identity.is_some() {
                    if !program
                        .product_operand_applications
                        .is_valid(identity.application)
                    {
                        return Err(
                            "source provider description lost its prepared application".into()
                        );
                    }
                    let prepared = program
                        .product_operand_applications
                        .get(identity.application);
                    let actual = program
                        .product_operand_application_identity(identity.application)?
                        .and_then(|identity| program.type_identity_text(identity));
                    if prepared.declaration != identity.symbol
                        || prepared.role != Some(role)
                        || actual != identity.application_identity
                    {
                        return Err(
                            "source provider description changed its complete prepared application"
                                .into(),
                        );
                    }
                }
                Ok(())
            }
        }
        mod inventory {
            use super::super::{
                ProviderPlanningContext, prepared_named_provider_applications,
                prepared_named_provider_member,
            };
            use super::schemas::{Key, Schema, provider_key};
            use symbols::SymbolHandle;
            use typed_trees::{TypedTrees, trait_definition::AppliedTraitScope};

            pub(super) struct Candidate {
                pub schema: Schema,
                pub provider: Key,
                pub rows: Vec<(SymbolHandle, SymbolHandle)>,
            }
            impl Candidate {
                pub fn extend_checking_roots(
                    &self,
                    program: &TypedTrees,
                    roots: &mut Vec<SymbolHandle>,
                ) {
                    for &(_, member) in &self.rows {
                        if !roots.contains(&member) {
                            roots.push(member);
                        }
                        for machine in program.machines() {
                            if program
                                .machine_instantiation_path(machine.symbol, member)
                                .is_some()
                                && !roots.contains(&machine.symbol)
                            {
                                roots.push(machine.symbol);
                            }
                        }
                    }
                }

                pub fn member_for(
                    &self,
                    program: &TypedTrees,
                    called: SymbolHandle,
                    requirement: SymbolHandle,
                ) -> Option<SymbolHandle> {
                    let mut rows = self.rows.iter().filter(|(row, _)| *row == requirement);
                    let &(_, member) = rows.next()?;
                    if rows.next().is_some() {
                        return None;
                    }
                    if program.closed_requirement_application(called).is_none() {
                        return program
                            .machine_by_symbol(member)
                            .filter(|machine| program.machine_type_parameters(machine).is_empty())
                            .map(|machine| machine.symbol);
                    }
                    let mut instances = program.machines().iter().filter(|machine| {
                        program.machine_type_parameters(machine).is_empty()
                            && program.requirement_realization_matches(
                                called,
                                member,
                                machine.symbol,
                            )
                    });
                    let instance = instances.next()?;
                    instances.next().is_none().then_some(instance.symbol)
                }
            }
            fn selected_member(
                typed: &TypedTrees,
                topology: &semantic_vocabulary::proof_applications::ProofApplicationPlan<
                    SymbolHandle,
                >,
                target: &str,
                member: SymbolHandle,
            ) -> Option<SymbolHandle> {
                let selected = topology.callable_for_target(member, target)?;
                typed
                    .machine_by_symbol(selected)
                    .map(|machine| machine.symbol)
            }

            pub(super) fn prepare(
                context: ProviderPlanningContext<'_>,
                topology: &semantic_vocabulary::proof_applications::ProofApplicationPlan<
                    SymbolHandle,
                >,
                target: &str,
            ) -> Vec<Candidate> {
                let typed = context.typed;
                let mut candidates = Vec::new();
                for application in prepared_named_provider_applications(context) {
                    if !application.scope.declaration.is_boundary {
                        continue;
                    }
                    let Some(schema) = Schema::trait_scope(typed, application.scope.clone()) else {
                        continue;
                    };
                    let mut rows = Vec::new();
                    for row in &application.declaration.rows {
                        let member = if application.capture.is_valid() {
                            prepared_named_provider_member(
                                typed,
                                &application,
                                row.realization_machine,
                            )
                        } else {
                            Some(row.realization_machine)
                        };
                        if let Some(member) = member
                            .and_then(|member| selected_member(typed, topology, target, member))
                        {
                            rows.push((row.requirement, member));
                        }
                    }
                    candidates.push(Candidate {
                        schema,
                        provider: provider_key(
                            context,
                            application.declaration.symbol,
                            application.identity.clone(),
                        ),
                        rows,
                    });
                }
                for machine in typed.machines().iter().filter(|machine| {
                    typed_trees::provider_declarations::satisfaction_provider_declaration(
                        typed, machine,
                    )
                    .is_some()
                        && !machine.target_family_interface
                        && context.contains(machine.symbol)
                        && typed
                            .source_target_context(machine)
                            .is_none_or(|scope| scope == target)
                        && !typed
                            .machine_specializations
                            .iter()
                            .any(|receipt| receipt.instance == machine.symbol)
                }) {
                    for edge in typed_trees::provider_declarations::independent_satisfactions(
                        typed, machine,
                    ) {
                        let schema = if let Some(scope) =
                            AppliedTraitScope::from_machine_conformance(typed, machine, edge)
                        {
                            if !scope.declaration.is_boundary {
                                continue;
                            }
                            Schema::trait_scope(typed, scope)
                        } else {
                            Schema::exact(typed, edge.requirement_symbol)
                        };
                        let Some(schema) = schema else {
                            continue;
                        };
                        let Some(owner) =
                            typed_trees::provider_declarations::satisfaction_provider_declaration(
                                typed, machine,
                            )
                        else {
                            continue;
                        };
                        let provider = provider_key(context, owner, None);
                        let position = candidates.iter().position(|candidate| {
                            candidate.provider == provider && candidate.schema.key == schema.key
                        });
                        let position = position.unwrap_or_else(|| {
                            candidates.push(Candidate {
                                schema,
                                provider,
                                rows: Vec::new(),
                            });
                            candidates.len() - 1
                        });
                        candidates[position]
                            .rows
                            .push((edge.requirement_symbol, machine.symbol));
                    }
                }
                candidates
            }
        }
        mod schemas {
            use super::super::ProviderPlanningContext;
            use language_semantics::MachineSupplyMode;
            use symbols::SymbolHandle;
            use typed_trees::{TypedTrees, trait_definition::AppliedTraitScope};

            pub(super) type Key =
                language_semantics::declaration_selection::provider_choices::ApplicationKey;

            #[derive(Clone)]
            pub(super) struct Schema {
                pub key: Key,
                pub receivers: Vec<Key>,
            }
            impl Schema {
                pub fn trait_scope(
                    program: &TypedTrees,
                    root: AppliedTraitScope<'_>,
                ) -> Option<Self> {
                    let mut scopes: Vec<AppliedTraitScope<'_>> = Vec::new();
                    let mut ambiguous = false;
                    root.visit(program, &mut |scope| {
                        if let Some(known) = scopes
                            .iter()
                            .find(|known| known.declaration.symbol == scope.declaration.symbol)
                        {
                            ambiguous |= !known.same_application(program, scope);
                        } else {
                            scopes.push(scope.clone());
                        }
                    });
                    if ambiguous {
                        return None;
                    }
                    let mut receivers = Vec::new();
                    for scope in scopes.iter().filter(|scope| scope.declaration.is_boundary) {
                        receivers.push(trait_key(program, scope)?);
                    }
                    Some(Self {
                        key: trait_key(program, &root)?,
                        receivers,
                    })
                }
                pub fn exact(program: &TypedTrees, symbol: SymbolHandle) -> Option<Self> {
                    let identity = match program.machine_by_symbol(symbol) {
                        Some(machine)
                            if machine.supply_mode == MachineSupplyMode::TopLevelRequirement =>
                        {
                            if machine.spelling.is_some() {
                                program.machine_callable_identity_text(machine)?
                            } else {
                                program.symbols.display_path(machine.symbol, "::")
                            }
                        }
                        _ => {
                            let operator =
                                typed_trees::operator::declaration_by_symbol(program, symbol)
                                    .or_else(|| {
                                        program
                                            .machine_token_bindings()
                                            .iter()
                                            .find(|row| row.symbol == symbol)
                                    })?;
                            if !operator.is_boundary {
                                return None;
                            }
                            typed_trees::operator::boundary_operator_requirement_identity(
                                program, operator,
                            )
                        }
                    };
                    Some(Self {
                        key: (
                            program.symbols.symbol_package_identity(symbol),
                            identity,
                            None,
                        ),
                        receivers: Vec::new(),
                    })
                }
                pub fn includes(&self, other: &Self) -> bool {
                    self.key == other.key || self.receivers.contains(&other.key)
                }
            }

            pub(super) fn trait_key(
                program: &TypedTrees,
                scope: &AppliedTraitScope<'_>,
            ) -> Option<Key> {
                Some((
                    program
                        .symbols
                        .symbol_package_identity(scope.declaration.symbol),
                    program.trait_declaration_path(scope.declaration),
                    Some(
                        program
                            .type_identity_text(scope.portable_application_identity(program)?)?,
                    ),
                ))
            }

            pub(super) fn provider_key(
                context: ProviderPlanningContext<'_>,
                symbol: SymbolHandle,
                application: Option<String>,
            ) -> Key {
                (
                    context.typed.symbols.symbol_package_identity(symbol),
                    context.typed.symbols.display_path(symbol, "::"),
                    application,
                )
            }
        }

        use super::{ProviderPlanningContext, TargetProviderDefaults};
        use symbols::SymbolHandle;
        use typed_trees::{TypedTrees, references::DeclarationSelection};

        struct CheckingScope<'request> {
            target: &'request str,
            roots: Option<Vec<SymbolHandle>>,
            candidates: Vec<inventory::Candidate>,
            supplied: Vec<bool>,
        }

        pub(crate) fn source_checking_applications(
            typed: &TypedTrees,
            scopes: &[crate::FixedLayoutTargetScope],
            defaults: &[TargetProviderDefaults],
            topology: &semantic_vocabulary::proof_applications::ProofApplicationPlan<SymbolHandle>,
            operational: &flow_effects::OperationalPlan,
        ) -> Result<Vec<DeclarationSelection>, String> {
            let context = ProviderPlanningContext::new(typed, source::DependencyScope::Product);
            let prepared = scopes
                .iter()
                .map(|scope| {
                    CheckingScope::prepare(
                        typed,
                        scope,
                        defaults,
                        inventory::prepare(context, topology, &scope.target),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let applications = ProofApplications::new(topology, operational);
            let mut work = prepared
                .iter()
                .map(|scope| scope.work(typed, &applications, &[]))
                .collect::<Vec<_>>();
            let unscoped = typed
                .machines()
                .iter()
                .filter(|machine| {
                    typed.source_target_context(machine).is_none()
                        && !work
                            .iter()
                            .any(|selection| selection.contains(machine.symbol))
                })
                .map(|machine| machine.symbol)
                .collect::<Vec<_>>();
            let facts_available = scopes.iter().any(|scope| scope.facts.is_some());
            let receives_unscoped = |request: &crate::FixedLayoutTargetScope| {
                request.facts.is_some() || !facts_available
            };
            if !unscoped.is_empty() {
                for ((scope, request), work) in prepared.iter().zip(scopes).zip(&mut work) {
                    if receives_unscoped(request) {
                        *work = scope.work(typed, &applications, &unscoped);
                    }
                }
            }
            let mut declaration_roots = Vec::new();
            for occurrence in typed_trees::references::collect_program_type_occurrences(typed) {
                if typed
                    .layout_storage_arguments(occurrence.reference)
                    .is_none()
                    || typed
                        .machine_by_own_or_state_symbol(occurrence.lexical_owner)
                        .is_some()
                    || !applications
                        .declarations()
                        .contains(&occurrence.lexical_owner)
                    || work
                        .iter()
                        .any(|selection| selection.contains(occurrence.lexical_owner))
                    || declaration_roots.contains(&occurrence.lexical_owner)
                {
                    continue;
                }
                declaration_roots.push(occurrence.lexical_owner);
            }
            for ((scope, request), work) in prepared.iter().zip(scopes).zip(&mut work) {
                let roots = declaration_roots
                    .iter()
                    .copied()
                    .filter(|owner| match declaration_target(typed, *owner) {
                        Some(target) => target == scope.target,
                        None => receives_unscoped(request),
                    })
                    .collect::<Vec<_>>();
                for owner in topology.declaration_closure(&roots) {
                    work.include(owner);
                }
            }
            Ok(work)
        }

        fn declaration_target(typed: &TypedTrees, mut owner: SymbolHandle) -> Option<&str> {
            while owner.is_valid() {
                if let Some(machine) = typed.machine_by_own_or_state_symbol(owner) {
                    return typed
                        .source_target_context(machine)
                        .or_else(|| typed.machine_authored_target(machine));
                }
                owner = typed.declaration_scope_parent(owner);
            }
            None
        }

        impl<'request> CheckingScope<'request> {
            fn prepare(
                typed: &TypedTrees,
                scope: &'request crate::FixedLayoutTargetScope,
                defaults: &[TargetProviderDefaults],
                candidates: Vec<inventory::Candidate>,
            ) -> Result<Self, String> {
                let declarations = scope
                    .provider_declarations
                    .iter()
                    .chain(
                        defaults
                            .iter()
                            .filter(|row| {
                                row.target
                                    .as_deref()
                                    .is_none_or(|target| target == scope.target)
                            })
                            .flat_map(|row| &row.selections),
                    )
                    .filter(|row| {
                        row.target
                            .is_none_or(|target| target.canonical_name() == scope.target)
                    })
                    .map(|row| declarations::Declaration::new(typed, row))
                    .collect::<Result<Vec<_>, _>>()?;
                let mut slots = Vec::new();
                for candidate in &candidates {
                    for key in
                        std::iter::once(&candidate.schema.key).chain(&candidate.schema.receivers)
                    {
                        if !slots.contains(key) {
                            slots.push(key.clone());
                        }
                    }
                }
                let declarations = declarations
                    .iter()
                    .map(|declaration| (declaration, declaration.slots(&slots)))
                    .collect::<Vec<_>>();
                let supplied = candidates
                    .iter()
                    .map(|candidate| {
                        declarations.iter().any(|(declaration, slots)| {
                            declaration.provider() == &candidate.provider
                                && slots.iter().any(|slot| {
                                    slot == &candidate.schema.key
                                        || candidate.schema.receivers.contains(slot)
                                })
                        })
                    })
                    .collect::<Vec<_>>();
                let mut roots = scope.entry_declarations.clone();
                if let Some(roots) = &mut roots {
                    for (candidate, supplied) in candidates.iter().zip(&supplied) {
                        if *supplied {
                            candidate.extend_checking_roots(typed, roots);
                        }
                    }
                }
                Ok(Self {
                    target: &scope.target,
                    roots,
                    candidates,
                    supplied,
                })
            }

            fn work(
                &self,
                typed: &TypedTrees,
                applications: &ProofApplications<'_, SymbolHandle, flow_effects::OperationalPlan>,
                unscoped: &[SymbolHandle],
            ) -> DeclarationSelection {
                let mut roots = self.roots.clone();
                if let Some(roots) = &mut roots {
                    roots.extend(unscoped.iter().copied());
                }
                let mut dependencies = Vec::new();
                loop {
                    let reached = applications.reachability(
                        self.target,
                        self.roots.is_none(),
                        &dependencies,
                        roots.as_deref(),
                    );
                    let demands = calls::collect(typed, &reached.machines, &reached.calls);
                    let mut changed = false;
                    if let Some(roots) = &mut roots {
                        for machine in typed.machines() {
                            let mut owner = typed.declaration_scope_parent(machine.symbol);
                            while owner.is_valid() {
                                if reached.machines.contains(&owner) {
                                    if !roots.contains(&machine.symbol) {
                                        roots.push(machine.symbol);
                                        changed = true;
                                    }
                                    break;
                                }
                                owner = typed.declaration_scope_parent(owner);
                            }
                        }
                        for receipt in &typed.machine_specializations {
                            if reached.machines.contains(&receipt.template)
                                || reached.machines.contains(&receipt.instance)
                            {
                                for symbol in [receipt.template, receipt.instance] {
                                    if !roots.contains(&symbol) {
                                        roots.push(symbol);
                                        changed = true;
                                    }
                                }
                            }
                        }
                    }
                    for demand in &demands {
                        let supplied_for_subject = self.candidates.iter().zip(&self.supplied).any(
                            |(candidate, supplied)| {
                                *supplied && candidate.schema.includes(&demand.schema)
                            },
                        );
                        for (candidate, supplied) in self.candidates.iter().zip(&self.supplied) {
                            if candidate.schema.includes(&demand.schema)
                                && (!supplied_for_subject || *supplied)
                                && let Some(member) =
                                    candidate.member_for(typed, demand.called, demand.requirement)
                                && !dependencies.contains(&(demand.called, member))
                            {
                                dependencies.push((demand.called, member));
                                changed = true;
                            }
                        }
                    }
                    if !changed {
                        let (machines, declarations) = applications.membership(
                            self.target,
                            self.roots.is_none(),
                            &dependencies,
                            roots.as_deref(),
                        );
                        let excluded = DeclarationSelection::from_symbols(
                            typed
                                .machines()
                                .iter()
                                .filter(|machine| !machines.contains(&machine.symbol))
                                .map(|machine| machine.symbol),
                        );
                        return DeclarationSelection::from_symbols(
                            machines.into_iter().chain(declarations),
                        )
                        .excluding(&excluded);
                    }
                }
            }
        }
    }
    mod context {
        pub use checked_trees::typed_trees::provider_declarations::ProviderPlanningContext;
    }
    mod identities {
        use checked_trees::typed_trees::{TypedTrees, trait_definition::Conformance};
        use symbols::SymbolHandle;

        pub fn is_named_provider(typed: &TypedTrees, symbol: SymbolHandle) -> bool {
            typed
                .conformances()
                .iter()
                .any(|definition| definition.symbol == symbol)
        }

        pub fn eligible(definition: &Conformance) -> bool {
            definition.type_parameters.is_empty()
        }

        pub fn named_requirement_lifetime_arguments<'a>(
            typed: &'a TypedTrees,
            implementation: &'a Conformance,
            declaring_trait: SymbolHandle,
        ) -> Option<&'a [u32]> {
            let root = typed.trait_definition_by_symbol(implementation.trait_symbol)?;
            let arguments = implementation.trait_lifetime_arguments.as_slice();
            if arguments.len() != root.lifetime_parameters.len()
                || arguments.iter().any(|ordinal| {
                    usize::try_from(*ordinal).map_or(true, |ordinal| {
                        ordinal >= implementation.lifetime_parameters.len()
                    })
                })
            {
                return None;
            }
            if root.symbol == declaring_trait {
                return Some(arguments);
            }
            let declaring = typed.trait_definition_by_symbol(declaring_trait)?;
            declaring.lifetime_parameters.is_empty().then_some(&[])
        }
    }
    mod named_applications {
        use super::ProviderPlanningContext;
        use typed_trees::typed_trees::CapturedMachineApplication;
        use typed_trees::{
            TypedTrees,
            trait_definition::{AppliedTraitScope, Conformance},
        };

        pub struct NamedProviderApplication<'program> {
            pub declaration: &'program Conformance,
            pub scope: AppliedTraitScope<'program>,
            pub capture: arena::Handle<CapturedMachineApplication>,
            pub identity: Option<String>,
        }

        pub fn prepared_named_provider_applications(
            context: ProviderPlanningContext<'_>,
        ) -> Vec<NamedProviderApplication<'_>> {
            let program = context.typed;
            let mut applications = Vec::new();
            for declaration in program.conformances().iter().filter(|declaration| {
                context.contains(declaration.symbol) && super::eligible(declaration)
            }) {
                if let Some(scope) = AppliedTraitScope::from_conformance(program, declaration) {
                    applications.push(NamedProviderApplication {
                        declaration,
                        scope,
                        capture: arena::Handle::default(),
                        identity: None,
                    });
                }
            }
            for (capture_handle, capture) in program.captured_machine_applications.iter() {
                let Some(declaration) = program
                    .conformance_by_symbol(capture.conformance_application.declaration)
                    .filter(|declaration| context.contains(declaration.symbol))
                else {
                    continue;
                };
                let Some(identity) = program
                    .portable_conformance_application_identity(
                        &capture.conformance_application,
                        capture.supplying_application,
                    )
                    .and_then(|identity| program.type_identity_text(identity))
                else {
                    continue;
                };
                if applications.iter().any(|existing| {
                    existing.declaration.symbol == declaration.symbol
                        && existing.identity.as_ref() == Some(&identity)
                }) {
                    continue;
                }
                let Some(scope) = capture.conformance_application.trait_scope.scope(program) else {
                    continue;
                };
                applications.push(NamedProviderApplication {
                    declaration,
                    scope,
                    capture: capture_handle,
                    identity: Some(identity),
                });
            }
            applications
        }

        pub fn prepared_named_provider_member(
            program: &TypedTrees,
            application: &NamedProviderApplication<'_>,
            template: symbols::SymbolHandle,
        ) -> Option<symbols::SymbolHandle> {
            let identity = application.identity.as_deref()?;
            let mut matching = program
                .captured_machine_applications
                .iter()
                .filter(|(_, capture)| {
                    capture.template == template
                        && capture.conformance_application.declaration
                            == application.declaration.symbol
                        && program
                            .portable_conformance_application_identity(
                                &capture.conformance_application,
                                capture.supplying_application,
                            )
                            .and_then(|identity| program.type_identity_text(identity))
                            .as_deref()
                            == Some(identity)
                })
                .map(|(_, capture)| capture.instance);
            let selected = matching.next()?;
            matching.next().is_none().then_some(selected)
        }
    }
    mod product_requests {
        use super::{
            ProviderPlanningContext, ProviderSelection, ProviderSelectionIdentity, admission,
        };
        use diagnostics::Diagnostic;
        use language_semantics::declaration_selection::product_operands::ProductStaticEnvironment;
        use language_semantics::declaration_selection::{
            BuildOperation, ExecutedProductSelection, ProductDeclarationOperand,
            ProductDeclarationRequest,
        };
        use symbols::SymbolHandle;
        use typed_trees::product_operand::{ProductOperandKind, resolve_product_path};

        fn identities(
            context: ProviderPlanningContext<'_>,
            request: &ProductDeclarationRequest,
            static_environment: Option<&ProductStaticEnvironment>,
        ) -> Result<(ProviderSelectionIdentity, ProviderSelectionIdentity), Diagnostic> {
            if context.scope != source::DependencyScope::Product
                || request.operation != BuildOperation::ProviderSelection
            {
                return Err(Diagnostic::error(
                    "source-issued provider choices require their exact Product planning context",
                ));
            }
            let [subject, provider] = request.operands.as_ref() else {
                return Err(Diagnostic::error(
                    "provider choice lost its two authored declaration operands",
                ));
            };
            let project = |operand: &ProductDeclarationOperand,
                           role|
             -> Result<ProviderSelectionIdentity, Diagnostic> {
                let symbol = resolve_product_path(context.typed, &operand.path, operand.source_span, role)
            .ok_or_else(|| Diagnostic::error(format!("Product provider operand `{}` does not resolve to one visible declaration of role {role:?}", operand.path))
                .with_source_span(operand.source_span))?;
                let application = if operand.application.is_some() {
                    let mut matching = context
                        .typed
                        .live_product_operand_applications()
                        .into_iter()
                        .filter(|handle| {
                            let prepared = context.typed.product_operand_applications.get(*handle);
                            prepared.role == Some(role)
                                && prepared.environment.as_deref() == static_environment
                                && context
                                    .typed
                                    .same_product_operand_source(&prepared.operand, operand)
                        });
                    let selected = matching.next().ok_or_else(|| {
                        Diagnostic::error(
                            "Product provider operand lost its original prepared application",
                        )
                        .with_source_span(operand.source_span)
                    })?;
                    if matching.next().is_some() {
                        return Err(Diagnostic::error(
                            "Product provider operand has multiple prepared lexical applications",
                        )
                        .with_source_span(operand.source_span));
                    }
                    selected
                } else {
                    let incomplete_provider = role == ProductOperandKind::Provider
                        && context
                            .typed
                            .conformance_by_symbol(symbol)
                            .is_some_and(|definition| {
                                !context
                                    .typed
                                    .conformance_type_parameters(definition)
                                    .is_empty()
                            });
                    if incomplete_provider {
                        return Err(Diagnostic::error(
                    "provider choice must supply the declaration's complete static application",
                )
                .with_source_span(operand.source_span));
                    }
                    Default::default()
                };
                let application_identity = if application.is_valid() {
                    context
                        .typed
                        .product_operand_application_identity(application)
                        .map_err(|reason| {
                            Diagnostic::error(reason).with_source_span(operand.source_span)
                        })?
                        .map(|identity| {
                            context.typed.type_identity_text(identity).ok_or_else(|| {
                                Diagnostic::error("Product application lost its canonical identity")
                            })
                        })
                        .transpose()?
                } else {
                    None
                };
                Ok(ProviderSelectionIdentity {
                    application,
                    applied: operand.application.is_some(),
                    application_identity,
                    symbol,
                    package: context.typed.symbols.symbol_package_identity(symbol),
                    canonical_path: context.typed.symbols.display_path(symbol, "::"),
                    authored_path: operand.path.clone(),
                })
            };
            Ok((
                project(subject, ProductOperandKind::BoundarySubject)?,
                project(provider, ProductOperandKind::Provider)?,
            ))
        }

        pub fn validate_product_provider_description(
            context: ProviderPlanningContext<'_>,
            request: &ProductDeclarationRequest,
        ) -> Result<(), Diagnostic> {
            let context =
                ProviderPlanningContext::new(context.typed.product_declarations(), context.scope);
            if context.scope != source::DependencyScope::Product
                || request.operation != BuildOperation::ProviderSelection
            {
                return Err(Diagnostic::error(
                    "provider descriptions require the exact Product declaration frontier",
                ));
            }
            let [subject, provider] = request.operands.as_ref() else {
                return Err(Diagnostic::error(
                    "provider description lost its exact two operands",
                ));
            };
            let describe = |operand: &ProductDeclarationOperand,
                            role|
             -> Result<ProviderSelectionIdentity, Diagnostic> {
                let symbol =
                    resolve_product_path(context.typed, &operand.path, operand.source_span, role)
                        .ok_or_else(|| {
                        Diagnostic::error(
                            "provider description does not name one exact visible declaration",
                        )
                        .with_source_span(operand.source_span)
                    })?;
                Ok(ProviderSelectionIdentity {
                    symbol,
                    application: Default::default(),
                    applied: operand.application.is_some(),
                    application_identity: None,
                    package: context.typed.symbols.symbol_package_identity(symbol),
                    canonical_path: context.typed.symbols.display_path(symbol, "::"),
                    authored_path: operand.path.clone(),
                })
            };
            admission::form(
                context,
                describe(subject, ProductOperandKind::BoundarySubject)?,
                describe(provider, ProductOperandKind::Provider)?,
            )
            .map(|_| ())
        }

        pub fn collect_product_provider_selections(
            context: ProviderPlanningContext<'_>,
            activation: Option<SymbolHandle>,
            declared: &[std::sync::Arc<ProductDeclarationRequest>],
            executed: &[ExecutedProductSelection],
        ) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
            for request in declared
                .iter()
                .filter(|request| request.operation == BuildOperation::ProviderSelection)
            {
                let (subject, provider) = identities(context, request, None)
                    .map_err(|error| vec![error.with_source_span(request.source_span)])?;
                admission::form(context, subject, provider)
                    .map_err(|error| vec![error.with_source_span(request.source_span)])?;
            }
            let has_requests = declared
                .iter()
                .any(|request| request.operation == BuildOperation::ProviderSelection)
                || executed
                    .iter()
                    .any(|event| event.request.operation == BuildOperation::ProviderSelection);
            let Some(activation) = activation else {
                return if has_requests {
                    Err(vec![Diagnostic::error(
                        "provider requests have no authoritative Build activation",
                    )])
                } else {
                    Ok(Vec::new())
                };
            };
            if context.typed.machine(activation).is_none() {
                return Err(vec![Diagnostic::error(
                    "provider requests lost their exact authoritative Build machine",
                )]);
            }
            let mut selections = Vec::new();
            for (ordinal, event) in executed
                .iter()
                .enumerate()
                .filter(|(_, event)| event.request.operation == BuildOperation::ProviderSelection)
            {
                let result = (|| {
                    if !declared.contains(&event.request) {
                        return Err(Diagnostic::error(
                            "executed provider choice has no retained authored formation request",
                        ));
                    }
                    let mode = event.composition.ok_or_else(|| {
                        Diagnostic::error(
                            "executed provider choice lost its evaluated composition mode",
                        )
                    })?;
                    let target = event.target.ok_or_else(|| {
                        Diagnostic::error(
                            "executed provider choice lost its evaluated catalog target",
                        )
                    })?;
                    let (subject, provider) =
                        identities(context, &event.request, event.static_environment.as_deref())?;
                    let author = typed_trees::product_operand::resolve_build_request_author(
                        context.typed,
                        event.selecting_machine.ok_or_else(|| {
                            Diagnostic::error("executed provider choice lost its lexical author")
                        })?,
                    )
                    .map_err(Diagnostic::error)?;
                    let origin =
                        language_semantics::declaration_selection::BuildSelectionOrigin::capture(
                            &context.typed.symbols,
                            activation,
                            author,
                            context
                                .typed
                                .symbols
                                .rejoin_product_occurrence(event.request.source_span),
                            Some(ordinal as u64),
                        )
                        .map_err(Diagnostic::error)?;
                    admission::admit(
                        context,
                        subject,
                        provider,
                        mode,
                        Some(target),
                        origin,
                        &selections,
                    )
                })();
                match result {
                    Ok(selection) => selections.push(selection),
                    Err(error) => {
                        return Err(vec![error.with_source_span(event.request.source_span)]);
                    }
                }
            }
            Ok(selections)
        }
    }
    mod retained_selections {
        use super::{
            ProviderPlanningContext, ProviderSelection, ProviderSelectionIdentity,
            ProviderSelectionSubject, admission,
        };
        use language_semantics::declaration_selection::{
            BuildSelectionOrigin, CompositionMode, ProductTarget,
        };
        use symbols::RetainedDeclaration;
        use typed_trees::TypedTrees;
        mod applications {
            use super::*;
            use crate::build_time_evaluation::expression_table;
            use diagnostics::Diagnostic;
            use language_semantics::declaration_selection::{
                ProductDeclarationOperand, ProductOperandRole,
                product_operands::ProductStaticOperandGraph,
            };
            use std::sync::Arc;
            use typed_trees::product_operand::{ProductOperandApplication, capture_closed_operand};
            use typed_trees_to_typed_trees::prepare_retained_product_application;

            #[derive(Debug, Clone, PartialEq, Eq)]
            pub struct RetainedProviderApplication {
                pub graph: ProductStaticOperandGraph,
                pub sources: Arc<source::SourceMap>,
                pub identity: String,
            }
            impl RetainedProviderApplication {
                pub(super) fn capture(
                    program: &typed_trees::TypedTrees,
                    identity: &ProviderSelectionIdentity,
                ) -> Result<Option<Self>, String> {
                    let Some(expected) = &identity.application_identity else {
                        return Ok(None);
                    };
                    if !program
                        .product_operand_applications
                        .is_valid(identity.application)
                    {
                        return Err(
                            "selected provider application has no prepared source input".into()
                        );
                    }
                    let prepared = program
                        .product_operand_applications
                        .get(identity.application);
                    if prepared.declaration != identity.symbol {
                        return Err(
                            "selected provider application changed its declaration owner".into(),
                        );
                    }
                    let actual = program
                        .product_operand_application_identity(identity.application)
                        .map_err(str::to_owned)?
                        .and_then(|identity| program.type_identity_text(identity));
                    if actual.as_ref() != Some(expected) {
                        return Err(
                            "selected provider application changed its complete static identity"
                                .into(),
                        );
                    }
                    let application = expression_table(program)
                        .static_machine_application(prepared.supplying_application);
                    let [argument] = expression_table(program)
                        .static_machine_application_arguments(prepared.supplying_application)
                    else {
                        return Err(
                            "selected nominal application lost its one whole operand".into()
                        );
                    };
                    let graph =
                        capture_closed_operand(program, argument, application.lexical_owner)?;
                    Ok(Some(Self {
                        graph,
                        sources: program
                            .symbols
                            .source_map()
                            .ok_or("selected application lost immutable source custody")?,
                        identity: expected.clone(),
                    }))
                }

                pub fn declaration_spans(&self) -> impl Iterator<Item = source::SourceSpan> + '_ {
                    self.graph
                        .names
                        .iter()
                        .filter(|(_, name)| {
                            name.builtin_type.is_none() && name.builtin_function.is_none()
                        })
                        .filter_map(|(_, name)| name.declaration)
                }

                pub(super) fn prepare(
                    &self,
                    program: &mut typed_trees::TypedTrees,
                    declaration: symbols::SymbolHandle,
                    role: ProductOperandRole,
                ) -> Result<arena::Handle<ProductOperandApplication>, Diagnostic> {
                    let current = program.symbols.source_map().ok_or_else(|| {
                        Diagnostic::error("receiving Product has no source custody")
                    })?;
                    let rejoin =
                        |span: source::SourceSpan| -> Result<Option<source::SourceSpan>, String> {
                            let original = self
                                .sources
                                .get(span.source_id)
                                .ok_or("retained application has a foreign source coordinate")?;
                            let mut matching = current.selected_files().filter(|candidate| {
                                candidate.package_identity == original.package_identity
                    && candidate.origin == original.origin
                    && candidate.dependency_scope == original.dependency_scope
                    && candidate.path.strip_prefix(&candidate.package_root).ok()
                        == original.path.strip_prefix(&original.package_root).ok()
                    && candidate.source == original.source
                    && (candidate.resolution_stratum == original.resolution_stratum
                        || (original.resolution_stratum
                            == source::SourceResolutionStratum::CurrentActivationExtension
                            && candidate.resolution_stratum
                                == source::SourceResolutionStratum::DependencyActivationExtension))
                            });
                            let selected = matching.next();
                            if matching.next().is_some() {
                                return Err(
                                    "retained application source has ambiguous receiving custody"
                                        .into(),
                                );
                            }
                            Ok(selected.map(|file| file.source_span(span.span)))
                        };
                    for span in self.declaration_spans() {
                        if rejoin(span).map_err(Diagnostic::error)?.is_none() {
                            return Err(Diagnostic::error(
                                "retained application declaration is outside the admitted Product source closure",
                            ));
                        }
                    }
                    let mut graph = self.graph.clone();
                    let mut failure = None;
                    graph.remap_source_spans(|span| match rejoin(span) {
                        Ok(Some(span)) => span,
                        Ok(None) => source::SourceSpan::default(),
                        Err(reason) => {
                            failure = Some(reason);
                            source::SourceSpan::default()
                        }
                    });
                    if let Some(reason) = failure {
                        return Err(Diagnostic::error(reason));
                    }
                    let root = graph.operands.get(graph.root);
                    let path = graph
                        .names
                        .span_or_empty(root.path)
                        .iter()
                        .map(|name| name.text.as_str())
                        .collect::<Vec<_>>()
                        .join("::");
                    let operand = ProductDeclarationOperand {
                        path,
                        source_span: program
                            .symbols
                            .symbol_provenance_source_span(declaration)
                            .ok_or_else(|| {
                                Diagnostic::error(
                                    "retained Product declaration lost its source anchor",
                                )
                            })?,
                        application: Some(Arc::new(graph)),
                    };
                    let handle =
                        prepare_retained_product_application(program, &operand, role, declaration)
                            .map_err(|errors| {
                                Diagnostic::error(
                                    errors
                                        .iter()
                                        .map(|error| error.message.as_str())
                                        .collect::<Vec<_>>()
                                        .join("; "),
                                )
                            })?;
                    let actual = program
                        .product_operand_application_identity(handle)
                        .map_err(Diagnostic::error)?
                        .and_then(|identity| program.type_identity_text(identity));
                    if actual.as_ref() != Some(&self.identity) {
                        return Err(Diagnostic::error(
                            "retained Product application changed its complete nominal or region identity during receiving formation",
                        ));
                    }
                    Ok(handle)
                }
            }
        }
        pub use applications::RetainedProviderApplication;

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct RetainedProviderSelection {
            pub subject: RetainedDeclaration,
            pub subject_identity: String,
            pub subject_application: Option<RetainedProviderApplication>,
            pub provider: RetainedDeclaration,
            pub provider_identity: String,
            pub provider_application: Option<RetainedProviderApplication>,
            pub target: ProductTarget,
            pub composition_mode: CompositionMode,
            pub origin: BuildSelectionOrigin,
        }

        impl RetainedProviderSelection {
            pub fn capture(
                typed: &TypedTrees,
                selection: &ProviderSelection,
            ) -> Result<Self, String> {
                let subject = match &selection.subject {
                    ProviderSelectionSubject::BoundaryTrait(identity)
                    | ProviderSelectionSubject::BoundaryRequirement(identity) => identity.symbol,
                    ProviderSelectionSubject::BoundaryOperatorFamily(family) => {
                        family
                            .coordinates()
                            .first()
                            .ok_or("retained provider family has no declaration")?
                            .symbol
                    }
                };
                let capture = |symbol| {
                    let selected = RetainedDeclaration::capture(&typed.symbols, symbol)?;
                    RetainedDeclaration::from_retained_parts(
                        selected.source().clone(),
                        selected.span(),
                        selected.kind(),
                    )
                };
                Ok(Self {
                    subject_application: match &selection.subject {
                        ProviderSelectionSubject::BoundaryTrait(identity)
                        | ProviderSelectionSubject::BoundaryRequirement(identity) => {
                            RetainedProviderApplication::capture(typed, identity)?
                        }
                        ProviderSelectionSubject::BoundaryOperatorFamily(_) => None,
                    },
                    provider_application: RetainedProviderApplication::capture(
                        typed,
                        &selection.provider_type,
                    )?,
                    subject: capture(subject).map_err(str::to_owned)?,
                    subject_identity: typed.normalized_hermetic_symbol_identity(subject)?,
                    provider: capture(selection.provider_type.symbol).map_err(str::to_owned)?,
                    provider_identity: typed
                        .normalized_hermetic_symbol_identity(selection.provider_type.symbol)?,
                    target: selection
                        .target
                        .ok_or("an executed package provider choice has no target key")?,
                    composition_mode: selection.composition_mode,
                    origin: selection.origin.clone(),
                })
            }

            pub fn rejoin(
                &self,
                program: &mut TypedTrees,
                scope: source::DependencyScope,
                previous: &[ProviderSelection],
                candidates: &mut symbols::SourceSpanCandidates,
            ) -> Result<ProviderSelection, diagnostics::Diagnostic> {
                let identity = |program: &mut TypedTrees,
                                candidates: &mut symbols::SourceSpanCandidates,
                                anchor: &RetainedDeclaration,
                                expected: &str,
                                retained: Option<&RetainedProviderApplication>,
                                role| {
                    let symbol = anchor
                        .rejoin(
                            &program.symbols,
                            candidates
                                .at_span(&program.symbols, anchor.span())
                                .into_iter()
                                .filter(|symbol| {
                                    if program.symbols.get(*symbol).kind != anchor.kind() {
                                        return false;
                                    }
                                    program
                                        .normalized_hermetic_symbol_identity(*symbol)
                                        .is_ok_and(|actual| actual == expected)
                                }),
                        )
                        .map_err(diagnostics::Diagnostic::error)?;
                    let application = retained
                        .map(|retained| retained.prepare(program, symbol, role))
                        .transpose()?
                        .unwrap_or_default();
                    Ok::<_, diagnostics::Diagnostic>(ProviderSelectionIdentity {
                        application,
                        applied: retained.is_some(),
                        application_identity: retained.map(|retained| retained.identity.clone()),
                        symbol,
                        package: program.symbols.symbol_package_identity(symbol),
                        canonical_path: program.symbols.display_path(symbol, "::"),
                        authored_path: program.symbols.display_path(symbol, "::"),
                    })
                };
                let subject = identity(
                    program,
                    candidates,
                    &self.subject,
                    &self.subject_identity,
                    self.subject_application.as_ref(),
                    language_semantics::declaration_selection::ProductOperandRole::BoundarySubject,
                )?;
                let provider = identity(
                    program,
                    candidates,
                    &self.provider,
                    &self.provider_identity,
                    self.provider_application.as_ref(),
                    language_semantics::declaration_selection::ProductOperandRole::Provider,
                )?;
                admission::admit(
                    ProviderPlanningContext::new(program, scope),
                    subject,
                    provider,
                    self.composition_mode,
                    Some(self.target),
                    self.origin.clone(),
                    previous,
                )
            }
        }
    }
    mod selection {
        use super::ProviderPlanningContext;
        use checked_trees::typed_trees::TypedTrees;
        use symbols::SymbolHandle;

        use checked_trees::typed_trees::operator::{OperatorDefinition, declaration_by_symbol};
        pub use language_semantics::declaration_selection::CompositionMode;

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct ProviderSelectionIdentity {
            pub symbol: SymbolHandle,
            pub application: arena::Handle<typed_trees::product_operand::ProductOperandApplication>,
            pub applied: bool,
            pub application_identity: Option<String>,
            pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
            pub canonical_path: String,
            pub authored_path: String,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct ProviderOperatorFamilyCoordinate {
            pub symbol: SymbolHandle,
            pub requirement_identity: String,
            pub static_parameter_count: usize,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct ProviderOperatorFamilySelection {
            pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
            pub canonical_path: String,
            pub authored_path: String,
            coordinates: Vec<ProviderOperatorFamilyCoordinate>,
        }

        impl ProviderOperatorFamilySelection {
            pub fn new(
                package: Option<semantic_vocabulary::PackageKeyIdentity>,
                canonical_path: String,
                authored_path: String,
                mut coordinates: Vec<ProviderOperatorFamilyCoordinate>,
            ) -> Result<Self, String> {
                coordinates.sort_by(|left, right| {
                    left.requirement_identity.cmp(&right.requirement_identity)
                });
                if coordinates.is_empty() {
                    return Err(format!(
                        "boundary-operator family `{authored_path}` contains no applicable coordinates"
                    ));
                }
                for pair in coordinates.windows(2) {
                    if pair[0].requirement_identity == pair[1].requirement_identity {
                        return Err(format!(
                            "boundary-operator family `{authored_path}` contains ambiguous coordinate `{}`",
                            pair[0].requirement_identity
                        ));
                    }
                }
                Ok(Self {
                    package,
                    canonical_path,
                    authored_path,
                    coordinates,
                })
            }

            pub fn derive(
                context: ProviderPlanningContext<'_>,
                operator_symbol: SymbolHandle,
                authored_path: String,
            ) -> Result<Self, String> {
                let typed = context.typed;
                let Some(representative) = declaration_by_symbol(typed, operator_symbol) else {
                    return Err(format!(
                        "provider selection subject `{authored_path}` has no exact retained operator declaration"
                    ));
                };
                if !context.contains(representative.symbol) {
                    return Err(format!(
                        "provider selection subject `{authored_path}` is outside the {:?} planning context",
                        context.scope,
                    ));
                }
                let canonical_path = operator_canonical_path(typed, representative);
                let package = typed.symbols.symbol_package_identity(representative.symbol);
                let coordinates = family_coordinates(context, package, &canonical_path)?;
                Self::new(package, canonical_path, authored_path, coordinates)
            }

            pub fn coordinates(&self) -> &[ProviderOperatorFamilyCoordinate] {
                &self.coordinates
            }

            pub fn replay_against_typed(
                &self,
                context: ProviderPlanningContext<'_>,
            ) -> Vec<String> {
                let typed = context.typed;
                let expected = match family_coordinates(context, self.package, &self.canonical_path)
                {
                    Ok(expected) => expected,
                    Err(reason) => return vec![reason],
                };
                let mut reasons = Vec::new();
                for coordinate in &self.coordinates {
                    let Some(current) = expected.iter().find(|current| {
                        current.requirement_identity == coordinate.requirement_identity
                    }) else {
                        let reason = match declaration_by_symbol(typed, coordinate.symbol) {
                            None => "no longer resolves to a boundary operator declaration",
                            Some(_) => "resolves to a declaration outside the family",
                        };
                        reasons.push(format!(
                    "boundary-operator family `{}` selection retains coordinate `{}`, which {reason}",
                    self.authored_path, coordinate.requirement_identity,
                ));
                        continue;
                    };
                    if current.symbol != coordinate.symbol {
                        reasons.push(format!(
                    "boundary-operator family `{}` selection retains coordinate `{}` under a substituted declaration symbol",
                    self.authored_path, coordinate.requirement_identity,
                ));
                    }
                    if current.static_parameter_count != coordinate.static_parameter_count {
                        reasons.push(format!(
                    "boundary-operator family `{}` selection retains coordinate `{}` with static-telescope arity {}, but its exact declaration has arity {}",
                    self.authored_path,
                    coordinate.requirement_identity,
                    coordinate.static_parameter_count,
                    current.static_parameter_count,
                ));
                    }
                }
                for current in &expected {
                    if !self.coordinates.iter().any(|coordinate| {
                        coordinate.requirement_identity == current.requirement_identity
                    }) {
                        reasons.push(format!(
                    "boundary-operator family `{}` selection omits coordinate `{}`; a family override covers every canonical overload or none",
                    self.authored_path, current.requirement_identity,
                ));
                    }
                }
                reasons
            }
        }

        fn operator_canonical_path(typed: &TypedTrees, operator: &OperatorDefinition) -> String {
            typed.operator_path_string(operator.name)
        }

        fn family_coordinates(
            context: ProviderPlanningContext<'_>,
            package: Option<semantic_vocabulary::PackageKeyIdentity>,
            canonical_path: &str,
        ) -> Result<Vec<ProviderOperatorFamilyCoordinate>, String> {
            let typed = context.typed;
            typed
        .operators()
        .iter()
        .chain(typed.machine_token_bindings())
        .chain(
            typed
                .domain_definitions()
                .iter()
                .flat_map(|domain| typed.domain_operators(domain)),
        )
        .filter(|operator| {
            operator.is_boundary
                && context.contains(operator.symbol)
                && typed.symbols.symbol_package_identity(operator.symbol) == package
                && operator_canonical_path(typed, operator) == canonical_path
        })
        .map(|operator| {
            let requirement_identity = match typed.machine(operator.symbol)
            {
                Some(requirement)
                    if requirement.supply_mode
                        == language_semantics::MachineSupplyMode::TopLevelRequirement =>
                {
                    typed
                        .machine_callable_identity_text(requirement)
                        .ok_or_else(|| {
                            format!(
                                "boundary-operator family `{canonical_path}` has a token machine without an exact overload identity"
                            )
                        })?
                }
                Some(_) => {
                    return Err(format!(
                        "boundary-operator family `{canonical_path}` has a token machine without requirement supply"
                    ));
                }
                None => checked_trees::typed_trees::operator::boundary_operator_requirement_identity(
                    typed, operator,
                ),
            };
            Ok(ProviderOperatorFamilyCoordinate {
                symbol: operator.symbol,
                requirement_identity,
                static_parameter_count: operator.lifetime_parameters.len()
                    + typed.operator_type_parameters(operator).len(),
            })
        })
        .collect()
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum ProviderSelectionSubject {
            BoundaryTrait(ProviderSelectionIdentity),
            BoundaryRequirement(ProviderSelectionIdentity),
            BoundaryOperatorFamily(ProviderOperatorFamilySelection),
        }

        impl ProviderSelectionSubject {
            pub fn package(&self) -> Option<semantic_vocabulary::PackageKeyIdentity> {
                match self {
                    Self::BoundaryTrait(identity) | Self::BoundaryRequirement(identity) => {
                        identity.package
                    }
                    Self::BoundaryOperatorFamily(family) => family.package,
                }
            }

            pub fn canonical_path(&self) -> &str {
                match self {
                    Self::BoundaryTrait(identity) | Self::BoundaryRequirement(identity) => {
                        &identity.canonical_path
                    }
                    Self::BoundaryOperatorFamily(family) => &family.canonical_path,
                }
            }

            pub fn authored_path(&self) -> &str {
                match self {
                    Self::BoundaryTrait(identity) | Self::BoundaryRequirement(identity) => {
                        &identity.authored_path
                    }
                    Self::BoundaryOperatorFamily(family) => &family.authored_path,
                }
            }

            pub fn selects_schema(
                &self,
                schema_symbol: SymbolHandle,
                requirement_identity: &str,
                application: Option<&str>,
            ) -> bool {
                match self {
                    Self::BoundaryTrait(identity) | Self::BoundaryRequirement(identity) => {
                        identity.symbol == schema_symbol
                            && identity
                                .application_identity
                                .as_deref()
                                .is_none_or(|selected| Some(selected) == application)
                    }
                    Self::BoundaryOperatorFamily(family) => {
                        family.coordinates.iter().any(|coordinate| {
                            coordinate.symbol == schema_symbol
                                && coordinate.requirement_identity == requirement_identity
                        })
                    }
                }
            }

            pub fn same_declaration_as(&self, other: &Self) -> bool {
                match (self, other) {
                    (Self::BoundaryTrait(left), Self::BoundaryTrait(right)) => {
                        left.symbol == right.symbol
                            && left.application_identity == right.application_identity
                    }
                    (Self::BoundaryRequirement(left), Self::BoundaryRequirement(right)) => {
                        left.symbol == right.symbol
                            && left.application_identity == right.application_identity
                    }
                    (Self::BoundaryOperatorFamily(left), Self::BoundaryOperatorFamily(right)) => {
                        left.package == right.package && left.canonical_path == right.canonical_path
                    }
                    _ => false,
                }
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct ProviderSelection {
            pub target: Option<language_semantics::declaration_selection::ProductTarget>,
            pub subject: ProviderSelectionSubject,
            pub provider_type: ProviderSelectionIdentity,
            pub composition_mode: CompositionMode,
            pub origin: language_semantics::declaration_selection::BuildSelectionOrigin,
        }
    }
    pub use retained_selections::{RetainedProviderApplication, RetainedProviderSelection};

    use crate::build_time_evaluation::expression_table;
    use crate::is_exact_toolchain_build_prelude_data;
    use checked_trees::typed_trees::TypedTrees;
    pub(crate) use checking_applications::source_checking_applications;
    pub use context::ProviderPlanningContext;
    use diagnostics::Diagnostic;
    pub use identities::{eligible, is_named_provider, named_requirement_lifetime_arguments};
    pub use named_applications::{
        NamedProviderApplication, prepared_named_provider_applications,
        prepared_named_provider_member,
    };
    pub use product_requests::{
        collect_product_provider_selections, validate_product_provider_description,
    };
    pub use selection::{
        CompositionMode, ProviderOperatorFamilySelection, ProviderSelection,
        ProviderSelectionIdentity, ProviderSelectionSubject,
    };
    use symbols::{JoinedPath, SymbolHandle, SymbolKind};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TargetProviderDefaults {
        pub target: Option<String>,
        pub machines: Vec<symbols::SymbolHandle>,
        pub selections: Vec<ProviderSelection>,
    }

    pub fn retain_provider_defaults(
        context: ProviderPlanningContext<'_>,
    ) -> Result<Vec<TargetProviderDefaults>, Vec<Diagnostic>> {
        let mut rows: Vec<TargetProviderDefaults> = Vec::new();
        for machine in context.typed.machines().iter().filter(|machine| {
            machine.purpose == checked_trees::typed_trees::machine::MachinePurpose::ProviderDefaults
                && !machine.target_family_interface
                && context.contains(machine.symbol)
                && (context.scope == DependencyScope::Product || machine.target.is_none())
        }) {
            let target = machine
                .authored_target
                .as_ref()
                .map(|name| name.as_str().to_owned());
            let selections = harvest_provider_selections(context, machine)?;
            if let Some(row) = rows.iter_mut().find(|row| row.target == target) {
                row.machines.push(machine.symbol);
                row.selections.extend(selections);
            } else {
                rows.push(TargetProviderDefaults {
                    target,
                    machines: vec![machine.symbol],
                    selections,
                });
            }
        }
        rows.sort_by(|left, right| left.target.cmp(&right.target));
        Ok(rows)
    }

    fn harvest_provider_selections(
        context: ProviderPlanningContext<'_>,
        machine: &checked_trees::typed_trees::machine::Machine,
    ) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
        let typed = context.typed;
        if !context.contains(machine.symbol) {
            return Err(vec![Diagnostic::error(format!(
                "provider-default declaration `{}` is outside the {:?} planning context",
                machine.name, context.scope,
            ))]);
        }
        let mut requests = Vec::new();
        for state in typed.machine_states(machine) {
            for statement in typed.statement_table.statements(state.statement_nodes) {
                match statement {
                    StatementNode::Expression(expression) => {
                        if let ExpressionNode::Call(call) =
                            typed.expression_table.expression(*expression)
                            && !call.target_symbol.is_valid()
                        {
                            requests.push(ProviderSelectionCall {
                                target: call.target.as_str(),
                                arguments: &call.machine_arguments,
                                value_arguments: expression_table(typed)
                                    .expression_handles(call.arguments),
                                source_span: provider_selection_expression_source_span(
                                    typed,
                                    *expression,
                                ),
                                machine: machine.symbol,
                                composition_case: None,
                                product_operands: false,
                            });
                        }
                    }
                    StatementNode::Call(call) if !call.target_symbol.is_valid() => {
                        requests.push(ProviderSelectionCall {
                            target: call.target.as_str(),
                            arguments: &call.machine_arguments,
                            value_arguments: typed
                                .statement_table
                                .expression_handles(call.arguments),
                            source_span: call.source_span,
                            machine: machine.symbol,
                            composition_case: None,
                            product_operands: false,
                        });
                    }
                    _ => {}
                }
            }
        }
        collect_provider_selections(context, requests)
    }

    struct ProviderSelectionCall<'a> {
        target: &'a str,
        arguments: &'a [StaticMachineArgument],
        value_arguments: &'a [checked_trees::typed_trees::expression::ExpressionHandle],
        source_span: source::SourceSpan,
        machine: SymbolHandle,
        composition_case: Option<SymbolHandle>,
        product_operands: bool,
    }

    pub fn collect_executed_provider_selections(
        context: ProviderPlanningContext<'_>,
        executed: &[checked_interpreter::ExecutedBuildSelection],
    ) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
        let typed = context.typed;
        if context.scope != DependencyScope::Product {
            return Err(vec![Diagnostic::error(
                "executed provider selections require the Product planning context",
            )]);
        }
        let requests = executed
        .iter()
        .filter(|row| row.operation == language_semantics::declaration_selection::BuildOperation::ProviderSelection)
        .map(|row| {
            let handle = row.expression;
            let request = match typed.expression_table.expression(handle) {
                ExpressionNode::Call(call)
                    if language_semantics::declaration_selection::is_unresolved_provider_selection_call(
                        call.target.as_str(),
                        call.target_symbol,
                    ) =>
                {
                    Some(ProviderSelectionCall {
                        target: call.target.as_str(),
                        arguments: &call.machine_arguments,
                        value_arguments: typed.expression_table.expression_handles(call.arguments),
                        source_span: provider_selection_expression_source_span(typed, handle),
                        machine: row.machine,
                        composition_case: Some(row.composition_case),
                        product_operands: true,
                    })
                }
                _ => None,
            };
            request.ok_or_else(|| {
                vec![Diagnostic::error(
                    "executed provider selection did not rejoin its admitted call",
                )]
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
        collect_provider_selections(
            ProviderPlanningContext::new(typed.product_declarations(), DependencyScope::Product),
            requests,
        )
    }

    fn provider_selection_expression_source_span(
        typed: &TypedTrees,
        expression: checked_trees::typed_trees::expression::ExpressionHandle,
    ) -> source::SourceSpan {
        expression_table(typed)
            .authored_selection_occurrences(expression)
            .filter_map(|occurrence| typed.authored_declaration_selections().get(occurrence))
            .find(|selection| selection.kind() == AuthoredDeclarationSelectionKind::Call)
            .map_or_else(
                || typed.expression_table.source_span(expression),
                |selection| selection.source_span(),
            )
    }

    fn collect_provider_selections<'a>(
        context: ProviderPlanningContext<'_>,
        requests: impl IntoIterator<Item = ProviderSelectionCall<'a>>,
    ) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
        let typed = context.typed;
        let mut selections: Vec<ProviderSelection> = Vec::new();
        let mut diagnostics = Vec::new();
        let mut record = |request: ProviderSelectionCall<'_>| {
            let ProviderSelectionCall {
                target,
                arguments,
                value_arguments,
                source_span,
                machine,
                composition_case,
                product_operands,
            } = request;
            if target != "select_provider" {
                return;
            }
            let [boundary_argument, provider_argument] = arguments else {
                diagnostics.push(Diagnostic::error(
                    "provider selection must retain exactly two resolved type paths",
                ));
                return;
            };
            let project_identity = |argument: &StaticMachineArgument, is_provider: bool| {
                let authored_path = argument.path.joined_path();
                let symbol = if product_operands {
                    checked_trees::typed_trees::product_operand::resolve_product_operand(
                        typed,
                        argument,
                        source_span,
                        if is_provider {
                            typed_trees::product_operand::ProductOperandKind::Provider
                        } else {
                            typed_trees::product_operand::ProductOperandKind::BoundarySubject
                        },
                    )
                    .unwrap_or_else(SymbolHandle::invalid)
                } else {
                    argument.symbol
                };
                ProviderSelectionIdentity {
                    application: Default::default(),
                    applied: argument.application.is_some(),
                    application_identity: None,
                    symbol,
                    package: typed.symbols.symbol_package_identity(symbol),
                    canonical_path: typed.symbols.display_path(symbol, "::"),
                    authored_path,
                }
            };
            let boundary_identity = project_identity(boundary_argument, false);
            let provider_type = project_identity(provider_argument, true);
            if !context.contains(boundary_identity.symbol)
                || !context.contains(provider_type.symbol)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "provider selection subject or provider is outside the {:?} planning context",
                    context.scope,
                )));
                return;
            }
            let composition_mode = match composition_case.map_or_else(
                || provider_selection_composition_mode(typed, value_arguments),
                |case| provider_composition_case(typed, case),
            ) {
                Ok(mode) => mode,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    return;
                }
            };
            let origin =
                match language_semantics::declaration_selection::BuildSelectionOrigin::capture(
                    &typed.symbols,
                    machine,
                    machine,
                    source_span,
                    None,
                ) {
                    Ok(origin) => origin,
                    Err(reason) => {
                        diagnostics.push(Diagnostic::error(reason).with_source_span(source_span));
                        return;
                    }
                };
            match admission::admit(
                context,
                boundary_identity,
                provider_type,
                composition_mode,
                None,
                origin,
                &selections,
            ) {
                Ok(selection) => selections.push(selection),
                Err(diagnostic) => diagnostics.push(diagnostic.with_source_span(source_span)),
            }
        };
        for request in requests {
            record(request);
        }
        if diagnostics.is_empty() {
            Ok(selections)
        } else {
            Err(diagnostics)
        }
    }

    fn provider_selection_composition_mode(
        typed: &TypedTrees,
        arguments: &[checked_trees::typed_trees::expression::ExpressionHandle],
    ) -> Result<CompositionMode, Diagnostic> {
        let Some((_, arguments)) = arguments.split_first() else {
            return Err(Diagnostic::error(
                "provider selection requires an explicit activation argument",
            ));
        };
        let [] = arguments else {
            let [argument] = arguments else {
                return Err(Diagnostic::error(
                    "provider selection must retain zero arguments for fused composition or one exact compiler-owned CompositionMode value",
                ));
            };
            let Some(case_symbol) = exact_case_argument_symbol(typed, *argument) else {
                return Err(Diagnostic::error(
                    "provider selection composition mode must be the exact compiler-owned CompositionMode::Fused or CompositionMode::Independent case",
                ));
            };
            return provider_composition_case(typed, case_symbol);
        };
        Ok(CompositionMode::Fused)
    }

    fn provider_composition_case(
        typed: &TypedTrees,
        case_symbol: SymbolHandle,
    ) -> Result<CompositionMode, Diagnostic> {
        if !case_symbol.is_valid() {
            return Ok(CompositionMode::Fused);
        }
        let exact_modes = typed
            .data_definitions()
            .iter()
            .filter(|definition| {
                is_exact_toolchain_build_prelude_data(typed, definition.symbol, "CompositionMode")
            })
            .collect::<Vec<_>>();
        let [modes] = exact_modes.as_slice() else {
            return Err(Diagnostic::error(
                "explicit provider composition mode requires exactly one compiler-owned CompositionMode declaration",
            ));
        };
        let selected = typed
            .data_members(modes)
            .iter()
            .filter_map(|member| match member {
                checked_trees::typed_trees::data::DataMember::Variant(variant)
                    if variant.symbol == case_symbol
                        && typed.symbols.get(variant.symbol).parent == modes.symbol =>
                {
                    Some(variant)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let [selected] = selected.as_slice() else {
            return Err(Diagnostic::error(
                "provider selection composition mode does not name an exact compiler-owned CompositionMode case",
            ));
        };
        if !typed.data_payload_fields(selected).is_empty() {
            return Err(Diagnostic::error(
                "provider selection composition mode case unexpectedly carries a payload",
            ));
        }
        match selected.name.as_str() {
            "Fused" => Ok(CompositionMode::Fused),
            "Independent" => Ok(CompositionMode::Independent),
            other => Err(Diagnostic::error(format!(
                "compiler-owned CompositionMode contains unsupported case `{other}`"
            ))),
        }
    }

    fn exact_case_argument_symbol(
        typed: &TypedTrees,
        argument: checked_trees::typed_trees::expression::ExpressionHandle,
    ) -> Option<SymbolHandle> {
        let symbol = match typed.expression_table.expression(argument) {
            ExpressionNode::Name(path) => Some(path.symbol),
            ExpressionNode::Member(member) => Some(member.member_symbol),
            ExpressionNode::StructLiteral(literal) if literal.fields.is_empty() => {
                Some(literal.case_symbol.unwrap_or_else(SymbolHandle::invalid))
            }
            _ => None,
        }?;
        (symbol.is_valid() && typed.symbols.get(symbol).kind == SymbolKind::Variant)
            .then_some(symbol)
    }
}
