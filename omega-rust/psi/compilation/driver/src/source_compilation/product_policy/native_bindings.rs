//! Native intents are prepared within the mutable Product source owner.
use super::*;
use build_time_evaluation::native_bindings::{
    self, NativeRequirementApplication, NativeTableField, PreparedNativeIntent,
};
use language_semantics::declaration_selection::product_operands::ProductStaticEnvironment;
use typed_trees::product_operand::ProductApplicationDemand;

impl SourceContinuation {
    pub(super) fn admit_native_requests(
        &mut self,
        activation: Option<SymbolHandle>,
        declared: &[Arc<ProductDeclarationRequest>],
        executed: &[ExecutedNativeBinding],
    ) -> Result<Vec<NativeBindingSelection>, Vec<Diagnostic>> {
        let occurrences =
            typed_trees::product_operand::collect_product_selection_occurrences(self.typed());
        let mut formations = Vec::new();
        for request in declared
            .iter()
            .filter(|request| request.operation.is_native_intent())
        {
            let mut owners = occurrences
                .iter()
                .filter(|occurrence| {
                    self.typed()
                        .same_product_request_source(&occurrence.request, request)
                })
                .map(|occurrence| self.typed().symbols.authored_origin(occurrence.machine));
            let owner = owners.next().ok_or_else(|| {
                refuse(
                    request.source_span,
                    "native recipe lost its original authored occurrence",
                )
            })?;
            if owners.any(|other| other != owner) {
                return Err(refuse(
                    request.source_span,
                    "native recipe has ambiguous lexical authors",
                ));
            }
            formations.push((request.clone(), owner));
        }
        for (request, author) in &formations {
            self.prepare_native_request(
                request,
                *author,
                None,
                ProductApplicationDemand::FormationOnly,
            )?;
        }
        let mut prepared = Vec::with_capacity(executed.len());
        for (event_ordinal, event) in executed.iter().enumerate() {
            if !declared.iter().any(|request| {
                self.typed()
                    .same_product_request_source(request, &event.request)
            }) {
                return Err(refuse(
                    event.request.source_span,
                    "executed native effect has no retained authored formation",
                ));
            }
            let author = typed_trees::product_operand::resolve_build_request_author(
                self.typed(),
                event.selecting_machine,
            )
            .map_err(|reason| refuse(event.request.source_span, &reason))?;
            let (application, table) = self.prepare_native_request(
                &event.request,
                author,
                event.static_environment.as_deref(),
                ProductApplicationDemand::Selected,
            )?;
            let supplier = self
                .typed()
                .symbols
                .symbol_package_identity(author)
                .ok_or_else(|| {
                    refuse(
                        event.request.source_span,
                        "native supplier lost its immutable package identity",
                    )
                })?;
            let declaration = typed_trees::product_operand::native_binding_formation(
                self.typed(),
                author,
                activation,
                event.request.clone(),
            )
            .and_then(|formation| formation.seal(supplier))
            .map_err(|reason| refuse(event.request.source_span, &reason))?;
            prepared.push(PreparedNativeIntent {
                application,
                table,
                declaration,
                target: event.target,
                intent: event.intent.clone(),
                source_span: self
                    .typed()
                    .symbols
                    .rejoin_product_occurrence(event.request.source_span),
                event_ordinal,
            });
        }
        native_bindings::collect(self.typed(), &prepared)
    }

    fn prepare_native_request(
        &mut self,
        request: &ProductDeclarationRequest,
        author: SymbolHandle,
        environment: Option<&ProductStaticEnvironment>,
        demand: ProductApplicationDemand,
    ) -> Result<(NativeRequirementApplication, Option<NativeTableField>), Vec<Diagnostic>> {
        request
            .validate_shape()
            .map_err(|reason| refuse(request.source_span, &reason))?;
        if !request.operation.is_native_intent() {
            return Err(refuse(
                request.source_span,
                "native recipe has a different operation",
            ));
        }
        let operand = &request.operands[0];
        let prepared =
            self.prepare_product_requirement_operand(operand, author, environment, demand)?;
        native_bindings::requirement_owner(self.typed(), prepared.requirement)
            .map_err(|reason| refuse(operand.source_span, &reason))?;
        let application = NativeRequirementApplication {
            requirement: prepared.requirement,
            closed_requirement: prepared.closed_requirement,
            trait_scope: prepared.trait_scope,
        };
        let table = if let Some(operand) = request.operands.get(1) {
            if operand.application.is_some() {
                return Err(refuse(
                    operand.source_span,
                    "table field designation must name an exact declaration",
                ));
            }
            let (table, field) = typed_trees::product_operand::resolve_native_table_field(
                self.typed(),
                &operand.path,
                operand.source_span,
            )
            .ok_or_else(|| {
                refuse(
                    operand.source_span,
                    "native table designation has no exact visible data field",
                )
            })?;
            let definition = self
                .typed()
                .data_definition_by_symbol(table)
                .ok_or_else(|| {
                    refuse(
                        operand.source_span,
                        "native table lost its data declaration",
                    )
                })?;
            let field_definition = self
                .typed()
                .data_members(definition)
                .iter()
                .filter_map(|member| member.as_field())
                .find(|candidate| candidate.symbol == field)
                .ok_or_else(|| {
                    refuse(
                        operand.source_span,
                        "native table field does not belong to its designated data declaration",
                    )
                })?;
            if field_definition.relevance.is_erased()
                || self
                    .typed()
                    .primitive_type_reference(field_definition.type_reference)
                    != Some(typed_trees::types::PrimitiveType::Addr)
            {
                return Err(refuse(
                    operand.source_span,
                    "native table dispatch requires a runtime addr field",
                ));
            }
            Some(NativeTableField { table, field, drops_receiver: request.operation == language_semantics::declaration_selection::BuildOperation::NativeTableFunction })
        } else {
            None
        };
        Ok((application, table))
    }

    pub(super) fn admit_captured_native_requests(
        &mut self,
        declarations: &[CapturedNativeDeclaration],
        intents: &[CapturedNativeIntent],
    ) -> Result<Vec<NativeBindingSelection>, Vec<Diagnostic>> {
        let mut joined = Vec::with_capacity(declarations.len());
        for declaration in declarations {
            let (request, author) = rejoin_declaration(self.typed(), declaration)?;
            self.prepare_native_request(
                &request,
                author,
                None,
                ProductApplicationDemand::FormationOnly,
            )?;
            joined.push((request, author));
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
            let (request, author) = &joined[intent.declaration as usize];
            let environment = intent
                .static_environment
                .as_deref()
                .map(|environment| rejoin_environment(self.typed(), declaration, environment))
                .transpose()?;
            let (application, table) = self.prepare_native_request(
                request,
                *author,
                environment.as_ref(),
                ProductApplicationDemand::Selected,
            )?;
            prepared.push(PreparedNativeIntent {
                application,
                table,
                declaration: declaration.clone(),
                target: intent.target,
                intent: intent.intent.clone(),
                source_span: request.source_span,
                event_ordinal,
            });
        }
        native_bindings::collect(self.typed(), &prepared)
    }
}

fn refuse(span: source::SourceSpan, reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(reason).with_source_span(span)]
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
