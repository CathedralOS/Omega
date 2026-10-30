//! Native intents are prepared within the mutable Product source owner.
use super::*;

mod captured;
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
            Some(validate_table_field(
                self.typed(), table, field,
                request.operation == language_semantics::declaration_selection::BuildOperation::NativeTableFunction,
            ).map_err(|reason| refuse(operand.source_span, reason))?)
        } else {
            None
        };
        Ok((application, table))
    }
}

fn validate_table_field(
    typed: &typed_trees::TypedTrees,
    table: SymbolHandle,
    field: SymbolHandle,
    drops_receiver: bool,
) -> Result<NativeTableField, &'static str> {
    let definition = typed
        .data_definition_by_symbol(table)
        .ok_or("native table lost its data declaration")?;
    let field_definition = typed
        .data_members(definition)
        .iter()
        .filter_map(|member| member.as_field())
        .find(|candidate| candidate.symbol == field)
        .ok_or("native table field does not belong to its designated data declaration")?;
    if field_definition.relevance.is_erased()
        || typed.primitive_type_reference(field_definition.type_reference)
            != Some(typed_trees::types::PrimitiveType::Addr)
    {
        return Err("native table dispatch requires a runtime addr field");
    }
    Ok(NativeTableField {
        table,
        field,
        drops_receiver,
    })
}

fn refuse(span: source::SourceSpan, reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(reason).with_source_span(span)]
}
