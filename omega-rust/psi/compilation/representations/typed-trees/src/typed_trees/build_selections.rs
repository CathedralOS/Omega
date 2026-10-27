//! Exact known requirement applications retained by Build readiness.

use crate::TypedTrees;
use crate::trait_definition::{AppliedTraitScope, TraitApplicationArguments};
use crate::types::{TypeReferenceHandle, TypeReferenceNode};
use symbols::SymbolHandle;

impl TypedTrees {
    /// Read the authored application using the ordinary trait telescope. This
    /// establishes closure, not visibility, operand formation or satisfaction.
    /// Those remain immutable checking and provider-admission obligations.
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
