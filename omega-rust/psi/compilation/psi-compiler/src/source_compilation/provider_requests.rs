//! Complete source admission of Product provider requests after the source join.
//! Formation checks every authored recipe, including unused helpers; only reached
//! requests receive their captured static substitution and activate a selection.

use super::SourceContinuation;
use build_time_evaluation::provider_declarations::{
    ProviderPlanningContext, ProviderSelection, collect_product_provider_selections,
};
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::product_operands::ProductStaticEnvironment;
use language_semantics::declaration_selection::{
    BuildOperation, ExecutedProductSelection, ProductDeclarationRequest,
};
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::product_operand::ProductApplicationDemand;

impl SourceContinuation {
    /// Form authored provider recipes and admit the exact executed selections.
    /// Call after Product source admission; this does not execute Product code,
    /// choose target providers, or grant authority beyond the Build activation.
    pub(super) fn admit_product_provider_requests(
        &mut self,
        activation: Option<SymbolHandle>,
        declared: &[Arc<ProductDeclarationRequest>],
        executed: &[ExecutedProductSelection],
    ) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
        let formations = self.provider_formation_authors(declared)?;
        for (request, author) in formations {
            self.prepare_provider_request(
                &request,
                author,
                None,
                ProductApplicationDemand::FormationOnly,
            )?;
        }
        for event in executed
            .iter()
            .filter(|event| has_provider_application(&event.request))
        {
            let author = typed_trees::product_operand::resolve_build_request_author(
                self.typed(),
                event.selecting_machine.ok_or_else(|| {
                    vec![Diagnostic::error(
                        "executed provider recipe lost its original author",
                    )]
                })?,
            )
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
            self.prepare_provider_request(
                &event.request,
                author,
                event.static_environment.as_deref(),
                ProductApplicationDemand::Selected,
            )?;
        }
        collect_product_provider_selections(
            ProviderPlanningContext::new(self.typed(), source::DependencyScope::Product),
            activation,
            declared,
            executed,
        )
    }

    /// Capture original lexical owners before application preparation mutates
    /// the typing frontier and may add specialized body occurrences.
    fn provider_formation_authors(
        &self,
        declared: &[Arc<ProductDeclarationRequest>],
    ) -> Result<Vec<(Arc<ProductDeclarationRequest>, SymbolHandle)>, Vec<Diagnostic>> {
        let typed = self.typed();
        let occurrences =
            typed_trees::product_operand::collect_product_selection_occurrences(typed);
        let mut formations = Vec::new();
        for request in declared
            .iter()
            .filter(|request| has_provider_application(request))
        {
            let mut owners = occurrences
                .iter()
                .filter(|occurrence| {
                    typed.same_product_request_source(&occurrence.request, request)
                })
                .map(|occurrence| typed.symbols.authored_origin(occurrence.machine));
            let owner = owners.next().ok_or_else(|| {
                vec![
                    Diagnostic::error(
                        "Product provider recipe lost its original authored body occurrence",
                    )
                    .with_source_span(request.source_span),
                ]
            })?;
            if owners.any(|other| other != owner) {
                return Err(vec![
                    Diagnostic::error(
                        "Product provider recipe has multiple original lexical owners",
                    )
                    .with_source_span(request.source_span),
                ]);
            }
            formations.push((Arc::clone(request), owner));
        }
        Ok(formations)
    }

    fn prepare_provider_request(
        &mut self,
        request: &ProductDeclarationRequest,
        author: SymbolHandle,
        static_environment: Option<&ProductStaticEnvironment>,
        demand: ProductApplicationDemand,
    ) -> Result<(), Vec<Diagnostic>> {
        for (operand_index, operand) in request.operands.iter().enumerate() {
            let role = request
                .operation
                .product_operand_role(operand_index)
                .ok_or_else(|| vec![Diagnostic::error("provider recipe lost its operand role")])?;
            typed_trees_to_typed_trees::prepare_product_operand_application(
                Arc::make_mut(&mut self.input).typing.typed_mut(),
                operand,
                role,
                author,
                static_environment,
                demand,
            )?;
        }
        Ok(())
    }
}

fn has_provider_application(request: &ProductDeclarationRequest) -> bool {
    request.operation == BuildOperation::ProviderSelection
        && request
            .operands
            .iter()
            .any(|operand| operand.application.is_some())
}
