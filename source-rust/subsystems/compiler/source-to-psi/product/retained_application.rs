//! Carry a closed Product operand across package captures. Reopening checks the
//! declaration spans against the receiving sources and requires the same identity.

use crate::declaration_selection::{
    ProductDeclarationOperand, ProductOperandRole, product_operands::ProductStaticOperandGraph,
};
use crate::source_state::TypedTrees;
use diagnostics::Diagnostic;
use std::sync::Arc;
use symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedProductApplication {
    pub graph: ProductStaticOperandGraph,
    pub sources: Arc<crate::source_inputs::SourceMap>,
    pub identity: String,
}
impl RetainedProductApplication {
    pub fn declaration_spans(&self) -> impl Iterator<Item = source_text::SourceSpan> + '_ {
        self.graph
            .names
            .iter()
            .filter(|(_, name)| name.builtin_type.is_none() && name.builtin_function.is_none())
            .filter_map(|(_, name)| name.declaration)
    }
}

impl RetainedProductApplication {
    /// Capture the prepared argument graph with the canonical identity already
    /// established by its declaration owner; package handoff adds no new meaning.
    pub fn capture(
        typed: &TypedTrees,
        application_handle: arena::Handle<
            crate::source_state::product_operand::ProductOperandApplication,
        >,
        identity: String,
    ) -> Result<Self, String> {
        if !typed
            .product_operand_applications
            .is_valid(application_handle)
        {
            return Err("an applied provider choice lost its prepared application".to_owned());
        }
        let application = typed.product_operand_applications.get(application_handle);
        let owner = typed
            .expression_table
            .static_machine_application(application.supplying_application)
            .lexical_owner;
        Ok(RetainedProductApplication {
            graph: crate::source_state::product_operand::capture_closed_operand(
                typed,
                &application.argument,
                owner,
            )?,
            sources: typed
                .symbols
                .source_map()
                .ok_or("an applied provider choice has no source map for its anchors")?,
            identity,
        })
    }
}

impl RetainedProductApplication {
    /// Rejoin declaration anchors before forming the same complete application;
    /// generated symbols and allocation order cannot identify a dependency row.
    pub fn prepare(
        &self,
        program: &mut TypedTrees,
        role: ProductOperandRole,
        declaration: SymbolHandle,
        authored_path: &str,
    ) -> Result<
        arena::Handle<crate::source_state::product_operand::ProductOperandApplication>,
        Diagnostic,
    > {
        let own = |span: source_text::SourceSpan| -> Option<source_text::SourceSpan> {
            let file = self.sources.get(span.source_id)?;
            let here = program
                .symbols
                .source_files()
                .find(|own| own.path == file.path)?;
            Some(source_text::SourceSpan::new(here.source_id, span.span))
        };
        let mut graph = self.graph.clone();
        let mut foreign = false;
        let mut anchor = |declaration: &mut Option<source_text::SourceSpan>| {
            if let Some(span) = *declaration {
                match own(span) {
                    Some(here) => *declaration = Some(here),
                    None => foreign = true,
                }
            }
        };
        graph
            .names
            .for_each_mut(|_, name| anchor(&mut name.declaration));
        graph
            .operands
            .for_each_mut(|_, operand| anchor(&mut operand.declaration));
        if foreign {
            return Err(Diagnostic::error(format!(
                "retained provider choice `{}` is applied to a declaration in a source this program did not load",
                authored_path
            )));
        }
        let operand = ProductDeclarationOperand {
            path: authored_path.to_owned(),
            source_span: program
                .symbols
                .symbol_provenance_source_span(declaration)
                .unwrap_or_default(),
            application: Some(Arc::new(graph)),
        };
        let application = crate::preparation::prepare_retained_product_application(
            program,
            &operand,
            role,
            declaration,
        )
        .map_err(|mut diagnostics| diagnostics.remove(0))?;
        let here = program
            .product_operand_application_identity(application)
            .map_err(Diagnostic::error)?;
        if here.as_deref() != Some(self.identity.as_str()) {
            return Err(Diagnostic::error(format!(
                "retained provider choice `{}` forms `{}` here and was chosen as `{}`",
                authored_path,
                here.as_deref().unwrap_or("no application"),
                self.identity,
            )));
        }
        program.retain_dependency_product_application(application);
        Ok(application)
    }
}
