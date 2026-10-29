//! Closed Product inputs transferred with their original immutable source map.
//! Only owned graph handles cross worlds. Rejoining uses declaration custody;
//! neither private-name lookup nor a parser reconstructs the selected tuple.
use super::*;
use language_semantics::declaration_selection::{
    ProductDeclarationOperand, ProductOperandRole, product_operands::ProductStaticOperandGraph,
};
use std::sync::Arc;

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
            return Err("selected provider application has no prepared source input".into());
        }
        let prepared = program
            .product_operand_applications
            .get(identity.application);
        if prepared.declaration != identity.symbol {
            return Err("selected provider application changed its declaration owner".into());
        }
        let actual = program
            .product_operand_application_identity(identity.application)
            .map_err(str::to_owned)?
            .and_then(|identity| program.type_identity_text(identity));
        if actual.as_ref() != Some(expected) {
            return Err(
                "selected provider application changed its complete static identity".into(),
            );
        }
        let application = program
            .expression_table
            .static_machine_application(prepared.supplying_application);
        let [argument] = program
            .expression_table
            .static_machine_application_arguments(prepared.supplying_application)
        else {
            return Err("selected nominal application lost its one whole operand".into());
        };
        let graph = typed_trees::product_operand::capture_closed_operand(
            program,
            argument,
            application.lexical_owner,
        )?;
        Ok(Some(Self {
            graph,
            sources: program
                .symbols
                .source_map()
                .ok_or("selected application lost immutable source custody")?,
            identity: expected.clone(),
        }))
    }

    /// All non-builtin declaration coordinates that the receiving Product
    /// must admit. Diagnostic-only supplier coordinates are not imports.
    pub fn declaration_spans(&self) -> impl Iterator<Item = source::SourceSpan> + '_ {
        self.graph
            .names
            .iter()
            .filter(|(_, name)| name.builtin_type.is_none() && name.builtin_function.is_none())
            .filter_map(|(_, name)| name.declaration)
    }

    pub(super) fn prepare(
        &self,
        program: &mut typed_trees::TypedTrees,
        declaration: symbols::SymbolHandle,
        role: ProductOperandRole,
    ) -> Result<
        arena::Handle<typed_trees::product_operand::ProductOperandApplication>,
        diagnostics::Diagnostic,
    > {
        let current = program.symbols.source_map().ok_or_else(|| {
            diagnostics::Diagnostic::error("receiving Product has no source custody")
        })?;
        let rejoin = |span: source::SourceSpan| -> Result<Option<source::SourceSpan>, String> {
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
                return Err("retained application source has ambiguous receiving custody".into());
            }
            Ok(selected.map(|file| file.source_span(span.span)))
        };
        for span in self.declaration_spans() {
            if rejoin(span)
                .map_err(diagnostics::Diagnostic::error)?
                .is_none()
            {
                return Err(diagnostics::Diagnostic::error(
                    "retained application declaration is outside the admitted Product source closure",
                ));
            }
        }
        let mut graph = self.graph.clone();
        let mut failure = None;
        graph.remap_source_spans(|span| match rejoin(span) {
            Ok(Some(span)) => span,
            // Closed values can retain diagnostic coordinates in an erased
            // Build helper. No receiving source occurrence is invented there.
            Ok(None) => source::SourceSpan::default(),
            Err(reason) => {
                failure = Some(reason);
                source::SourceSpan::default()
            }
        });
        if let Some(reason) = failure {
            return Err(diagnostics::Diagnostic::error(reason));
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
                    diagnostics::Diagnostic::error(
                        "retained Product declaration lost its source anchor",
                    )
                })?,
            application: Some(Arc::new(graph)),
        };
        let handle = typed_trees_to_typed_trees::prepare_retained_product_application(
            program,
            &operand,
            role,
            declaration,
        )
        .map_err(|errors| {
            diagnostics::Diagnostic::error(
                errors
                    .iter()
                    .map(|error| error.message.as_str())
                    .collect::<Vec<_>>()
                    .join("; "),
            )
        })?;
        let actual = program
            .product_operand_application_identity(handle)
            .map_err(diagnostics::Diagnostic::error)?
            .and_then(|identity| program.type_identity_text(identity));
        if actual.as_ref() != Some(&self.identity) {
            return Err(diagnostics::Diagnostic::error(
                "retained Product application changed its complete nominal or region identity during receiving formation",
            ));
        }
        Ok(handle)
    }
}
