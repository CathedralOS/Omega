//! Exact core fixed-representation constructor recognition.
//!
//! Policies remain ordinary implementation operands. Only the toolchain's
//! constructor and its same-source interface receive compiler semantics.
use crate::{
    TypedTrees,
    types::{TypeReferenceHandle, TypeReferenceNode},
};
use symbols::SymbolHandle;

#[derive(Debug, Clone, Copy)]
pub struct LayoutStorageArguments {
    pub application: TypeReferenceHandle,
    pub semantic_type: TypeReferenceHandle,
    pub policy: TypeReferenceHandle,
    pub contract: SymbolHandle,
}

impl TypedTrees {
    pub fn core_layout_declaration(&self, symbol: SymbolHandle, name: &str) -> bool {
        symbol.is_valid()
            && self.symbols.symbol_source_origin(symbol) == Some(source::SourceOrigin::Toolchain)
            && self.symbols.name(symbol) == name
            && self.data_definition_by_symbol(symbol).is_some_and(|data| {
                data.supply_mode == language_semantics::DataSupplyMode::BoundaryOpaque
            })
    }

    pub fn layout_storage_arguments(
        &self,
        reference: TypeReferenceHandle,
    ) -> Option<LayoutStorageArguments> {
        let mut application = self.type_reference_table.peeled_type_reference(reference);
        if let TypeReferenceNode::Named { symbol, .. } =
            self.type_reference_table.type_reference(application)
        {
            application = self.data_definition_by_symbol(*symbol)?.generic_instance?;
        }
        let TypeReferenceNode::Generic {
            base_symbol,
            lifetime_arguments,
            arguments,
            ..
        } = self.type_reference_table.type_reference(application)
        else {
            return None;
        };
        if !self.core_layout_declaration(*base_symbol, "LayoutStorage")
            || !lifetime_arguments.is_empty()
        {
            return None;
        }
        let constructor = self.data_definition_by_symbol(*base_symbol)?;
        let [semantic, policy] = self.data_type_parameters(constructor) else {
            return None;
        };
        if semantic.kind != crate::data::TypeParameterKind::Type
            || policy.kind != crate::data::TypeParameterKind::Implementation
        {
            return None;
        }
        let [bound] = constructor.conformance_bounds.as_slice() else {
            return None;
        };
        if bound.binder != policy.symbol || bound.arguments.len() != 1 {
            return None;
        }
        let contract = self.trait_definition_by_symbol(bound.trait_symbol)?;
        if contract.is_boundary
            || contract.name.as_str() != "FixedLayout"
            || self.symbols.symbol_source_origin(contract.symbol)
                != Some(source::SourceOrigin::Toolchain)
            || self.symbols.symbol_source_span(contract.symbol)?.source_id
                != self
                    .symbols
                    .symbol_source_span(constructor.symbol)?
                    .source_id
            || self.type_reference_table.type_symbol(bound.arguments[0]) != semantic.symbol
        {
            return None;
        }
        let [semantic_type, policy] = self.type_reference_table.type_reference_handles(*arguments)
        else {
            return None;
        };
        Some(LayoutStorageArguments {
            application,
            semantic_type: *semantic_type,
            policy: *policy,
            contract: contract.symbol,
        })
    }

    pub fn plan_laid_layout_for_data(
        &self,
        data: &crate::data::DataDefinition,
    ) -> Option<&crate::PlanLaidLayout> {
        self.plan_laid_layout_for_type(data.generic_instance?)
    }

    /// Geometry joins by the complete constructor application, never T's symbol.
    pub fn plan_laid_layout_for_type(
        &self,
        reference: TypeReferenceHandle,
    ) -> Option<&crate::PlanLaidLayout> {
        let reference = self.type_reference_table.peeled_type_reference(reference);
        let identity = self.normalized_type_identity(reference);
        self.plan_laid_layouts.iter().find(|plan| {
            plan.application.is_valid()
                && self.normalized_type_identity(plan.application) == identity
        })
    }
}
