use crate::source_state::TypedTrees;
use crate::source_state::trait_definition::{AppliedTraitScope, TraitApplicationArguments};
use crate::source_state::types::{TypeReferenceHandle, TypeReferenceNode};
use symbols::SymbolHandle;

impl TypedTrees {
    pub fn build_provider_backing(
        &self,
        operand: &crate::declaration_selection::ProductDeclarationOperand,
    ) -> Result<&crate::source_state::trait_definition::Conformance, &'static str> {
        self.symbols
            .find_top_level_by_name_and_kinds_from_source(
                &operand.path,
                &[symbols::SymbolKind::Conformance],
                operand.source_span,
            )
            .and_then(|symbol| self.conformance_by_symbol(symbol))
            .filter(|row| row.is_public)
            .ok_or("Build backing names no visible public implementation in its supplied library")
    }

    pub fn check_build_provider_declarations(&self) -> Result<(), diagnostics::Diagnostic> {
        for (request, owner) in self.expression_table.build_provider_readiness_calls() {
            let refuse = |why: &str| {
                diagnostics::Diagnostic::error(why).with_source_span(request.backing.source_span)
            };
            let backing = self
                .build_provider_backing(&request.backing)
                .map_err(refuse)?;
            let supplied = AppliedTraitScope::from_conformance(self, backing)
                .ok_or_else(|| refuse("Build backing has no checked requirement application"))?;
            let expected = self
                .build_provider_requirement_scope(request.requirement, owner)
                .map_err(refuse)?;
            if !supplied.same_application(self, &expected) {
                return Err(refuse(
                    "supplied Build provider satisfies a different exact requirement application",
                ));
            }
        }
        Ok(())
    }

    pub fn build_provider_requirement_scope(
        &self,
        reference: TypeReferenceHandle,
        lexical_owner: SymbolHandle,
    ) -> Result<AppliedTraitScope<'_>, &'static str> {
        if !reference.is_valid() {
            return Err("provider readiness lost its requirement application");
        }
        let (symbol, arguments, lifetimes) =
            match self.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Named { symbol, .. } => (*symbol, &[][..], &[][..]),
                TypeReferenceNode::Generic {
                    base_symbol,
                    arguments,
                    lifetime_arguments,
                    ..
                } => (
                    *base_symbol,
                    self.type_reference_table.type_reference_handles(*arguments),
                    lifetime_arguments.as_slice(),
                ),
                _ => return Err("provider readiness requires a boundary trait application"),
            };
        let declaration = self
            .trait_definition_by_symbol(symbol)
            .filter(|declaration| declaration.is_boundary)
            .ok_or("provider readiness requires a known boundary trait")?;
        let scope = AppliedTraitScope::new(
            self,
            declaration,
            TraitApplicationArguments {
                lexical_owner,
                enclosing_data_applications: &[],
                arguments,
                lifetimes,
            },
        )
        .ok_or("provider readiness requires every trait argument and lifetime")?;
        self.require_receiving_trait_operands(&scope)?;
        Ok(scope)
    }
}
