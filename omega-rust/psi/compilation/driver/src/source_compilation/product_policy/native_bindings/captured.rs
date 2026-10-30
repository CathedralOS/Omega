//! Admit completed dependency intent under its original supplier custody.
//! Erased Build declarations are not receiving Product execution roots.
use super::*;

impl SourceContinuation {
    pub(in crate::source_compilation) fn admit_captured_native_requests(
        &mut self,
        declarations: &[CapturedNativeDeclaration],
        intents: &[CapturedNativeIntent],
    ) -> Result<Vec<NativeBindingSelection>, Vec<Diagnostic>> {
        // A declaration-only request has no supplying telescope to reify.
        // Resolve it under the original package/stratum custody, independently
        // of whether its erased Build author belongs to the receiving program.
        for declaration in declarations {
            self.prepare_captured_native_request(
                declaration,
                None,
                ProductApplicationDemand::FormationOnly,
            )?;
        }
        let mut prepared = Vec::with_capacity(intents.len());
        for (event_ordinal, intent) in intents.iter().enumerate() {
            let declaration = declarations
                .get(intent.declaration as usize)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "captured native effect lost its exact declaration",
                    )]
                })?;
            let (application, table) = self.prepare_captured_native_request(
                declaration,
                intent.static_environment.as_deref(),
                ProductApplicationDemand::Selected,
            )?;
            let source_span = self
                .typed()
                .symbols
                .symbol_provenance_source_span(application.requirement)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "captured native requirement lost its receiving source",
                    )]
                })?;
            prepared.push(PreparedNativeIntent {
                application,
                table,
                declaration: declaration.clone(),
                target: intent.target,
                intent: intent.intent.clone(),
                source_span,
                event_ordinal,
            });
        }
        native_bindings::collect(self.typed(), &prepared)
    }

    fn prepare_captured_native_request(
        &mut self,
        declaration: &CapturedNativeDeclaration,
        environment: Option<&ProductStaticEnvironment>,
        demand: ProductApplicationDemand,
    ) -> Result<(NativeRequirementApplication, Option<NativeTableField>), Vec<Diagnostic>> {
        let author_source = declaration
            .sources
            .get(declaration.author.source_id)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "captured native author lost original source custody",
                )]
            })?;
        if author_source.package_identity != Some(declaration.producer.package)
            || author_source.resolution_stratum != declaration.resolution_stratum
        {
            return Err(vec![Diagnostic::error(
                "captured native author changed its package or source stratum",
            )]);
        }
        if declaration.author.span.start >= declaration.author.span.end
            || declaration.author.span.end > author_source.source.len()
        {
            return Err(vec![Diagnostic::error(
                "captured native author has invalid source bounds",
            )]);
        }
        let request = &declaration.request;
        if !request.operation.is_native_intent() {
            return Err(vec![Diagnostic::error(
                "captured native request has a different operation",
            )]);
        }
        request
            .validate_shape()
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        if request
            .operands
            .iter()
            .any(|operand| operand.application.is_some())
        {
            let (request, author) = rejoin_declaration(self.typed(), declaration)?;
            let environment = environment
                .map(|environment| rejoin_environment(self.typed(), declaration, environment))
                .transpose()?;
            return self.prepare_native_request(&request, author, environment.as_ref(), demand);
        }
        let resolve =
            |operand: &language_semantics::declaration_selection::ProductDeclarationOperand,
             role| {
                typed_trees::product_operand::resolve_captured_product_path(
                self.typed(), &operand.path, declaration.producer.package,
                declaration.resolution_stratum, role,
            ).ok_or_else(|| vec![Diagnostic::error(format!(
                "captured native operand `{}` is not one visible declaration in its original supplier scope",
                operand.path,
            ))])
            };
        let requirement = resolve(
            &request.operands[0],
            language_semantics::declaration_selection::ProductOperandRole::Requirement,
        )?;
        native_bindings::requirement_owner(self.typed(), requirement)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let table = request.operands.get(1).map(|operand| {
            let field = resolve(operand, language_semantics::declaration_selection::ProductOperandRole::TableField)?;
            let typed = self.typed();
            let table = typed.data_definitions().iter().find(|definition| {
                typed.data_members(definition).iter().filter_map(|member| member.as_field())
                    .any(|candidate| candidate.symbol == field)
            }).map(|definition| definition.symbol)
                .ok_or_else(|| vec![Diagnostic::error("captured native field lost its exact table")])?;
            validate_table_field(typed, table, field,
                request.operation == language_semantics::declaration_selection::BuildOperation::NativeTableFunction)
                .map_err(|reason| vec![Diagnostic::error(reason)])
        }).transpose()?;
        Ok((
            NativeRequirementApplication {
                requirement,
                closed_requirement: None,
                trait_scope: None,
            },
            table,
        ))
    }
}

/// Rejoin through exact supplier coordinates; receiving root imports confer no
/// access to private names. Product source admission must precede this query.
fn rejoin_span(
    typed: &typed_trees::TypedTrees,
    declaration: &CapturedNativeDeclaration,
    span: source::SourceSpan,
) -> Result<source::SourceSpan, Vec<Diagnostic>> {
    let current = typed
        .symbols
        .source_map()
        .ok_or_else(|| refuse(span, "receiving Product lost source custody"))?;
    let original = declaration.sources.get(span.source_id).ok_or_else(|| {
        refuse(
            span,
            "captured native operand has a foreign source coordinate",
        )
    })?;
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
    let file = matching.next().ok_or_else(|| {
        refuse(
            span,
            "captured native source is outside the admitted Product closure",
        )
    })?;
    if matching.next().is_some() {
        return Err(refuse(
            span,
            "captured native source has ambiguous receiving custody",
        ));
    }
    Ok(file.source_span(span.span))
}

fn rejoin_declaration(
    typed: &typed_trees::TypedTrees,
    declaration: &CapturedNativeDeclaration,
) -> Result<(ProductDeclarationRequest, SymbolHandle), Vec<Diagnostic>> {
    let author = rejoin_span(typed, declaration, declaration.author)?;
    let author = typed_trees::product_operand::resolve_build_request_author(typed, author)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    if typed.symbols.symbol_package_identity(author) != Some(declaration.producer.package) {
        return Err(vec![Diagnostic::error(
            "captured native author changed its supplying package",
        )]);
    }
    let mut request = declaration.request.as_ref().clone();
    request.source_span = rejoin_span(typed, declaration, request.source_span)?;
    for operand in &mut request.operands {
        operand.source_span = rejoin_span(typed, declaration, operand.source_span)?;
        if let Some(graph) = &mut operand.application {
            let mut failure = None;
            Arc::make_mut(graph).remap_source_spans(|span| {
                match rejoin_span(typed, declaration, span) {
                    Ok(joined) => joined,
                    Err(errors) => {
                        failure = Some(errors);
                        span
                    }
                }
            });
            if let Some(errors) = failure {
                return Err(errors);
            }
        }
    }
    Ok((request, author))
}

fn rejoin_environment(
    typed: &typed_trees::TypedTrees,
    declaration: &CapturedNativeDeclaration,
    environment: &ProductStaticEnvironment,
) -> Result<ProductStaticEnvironment, Vec<Diagnostic>> {
    let mut environment = environment.clone();
    environment.template = environment
        .template
        .map(|span| rejoin_span(typed, declaration, span))
        .transpose()?;
    environment.lexical_owner = environment
        .lexical_owner
        .map(|span| rejoin_span(typed, declaration, span))
        .transpose()?;
    let mut failure = None;
    let mut remap = |span| match rejoin_span(typed, declaration, span) {
        Ok(joined) => joined,
        Err(errors) => {
            failure = Some(errors);
            span
        }
    };
    environment.graph.remap_source_spans(&mut remap);
    environment.bindings.for_each_mut(|_, binding| {
        binding.parameter = remap(binding.parameter);
        binding.lexical_owner = remap(binding.lexical_owner);
    });
    environment.lifetimes.for_each_mut(|_, lifetime| {
        lifetime.parameter_owner = remap(lifetime.parameter_owner);
        lifetime.lexical_owner = remap(lifetime.lexical_owner);
    });
    if let Some(errors) = failure {
        return Err(errors);
    }
    Ok(environment)
}
