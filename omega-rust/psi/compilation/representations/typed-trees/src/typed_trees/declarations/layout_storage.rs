//! Exact core fixed-representation constructor recognition.
//!
//! Policies remain ordinary implementation operands. Only the toolchain's
//! constructor and its same-source interface receive compiler semantics.
use crate::{
    TypedTrees,
    types::{TypeReferenceHandle, TypeReferenceNode},
};
use symbols::SymbolHandle;

const CORE_LAYOUT_SOURCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../source/library/core/layout.omg"
));

#[derive(Debug, Clone, Copy)]
pub struct LayoutStorageArguments {
    pub application: TypeReferenceHandle,
    pub semantic_type: TypeReferenceHandle,
    pub policy: TypeReferenceHandle,
    pub contract: SymbolHandle,
}

impl TypedTrees {
    /// The snapshot operation belongs to the exact sealed Core layout source.
    /// A same-spelled user operation remains an ordinary call. Its result is
    /// checked Source data; only the opaque argument has compiler provenance.
    pub fn layout_target_facts_result(
        &self,
        selected: SymbolHandle,
    ) -> Result<Option<SymbolHandle>, &'static str> {
        let Some(machine) = self
            .machine_by_symbol(selected)
            .or_else(|| self.machine_holding_state(selected))
        else {
            return Ok(None);
        };
        if machine.name.as_str().rsplit("::").next() != Some("layout_target_facts") {
            return Ok(None);
        }
        let source = self
            .symbols
            .symbol_source_span(machine.symbol)
            .and_then(|span| self.symbols.source_file(span));
        if source.is_none_or(|file| {
            file.origin != source::SourceOrigin::Toolchain
                || file.path.strip_prefix(&file.package_root).ok()
                    != Some(std::path::Path::new("layout.omg"))
        }) {
            return Ok(None);
        }
        if !self.sealed_layout_source(machine.symbol) {
            return Err(
                "layout target snapshot source differs from the immutable compiler catalogue",
            );
        }
        let invalid = "sealed layout target snapshot has an invalid declaration contract";
        let [state] = self.machine_states(machine) else {
            return Err(invalid);
        };
        let [parameter] = self.state_parameters(state) else {
            return Err(invalid);
        };
        let TypeReferenceNode::Reference {
            access, referee, ..
        } = self
            .type_reference_table
            .type_reference(parameter.type_reference)
        else {
            return Err(invalid);
        };
        let target = self.type_reference_table.type_symbol(*referee);
        let result = self.type_reference_table.type_symbol(state.return_type);
        if *access != language_semantics::ReferenceAccess::Shared
            || !self.core_layout_declaration(target, "LayoutTarget")
            || self.symbols.name(result) != "LayoutTargetFacts"
            || self
                .symbols
                .symbol_source_span(target)
                .map(|span| span.source_id)
                != self
                    .symbols
                    .symbol_source_span(machine.symbol)
                    .map(|span| span.source_id)
            || self
                .symbols
                .symbol_source_span(result)
                .map(|span| span.source_id)
                != self
                    .symbols
                    .symbol_source_span(machine.symbol)
                    .map(|span| span.source_id)
            || machine.attached_data_symbol.is_valid()
            || !self.machine_type_parameters(machine).is_empty()
            || machine.lifetime_parameters.len() != 1
            || parameter.is_const
            || parameter.is_mutable
            || machine.supply_mode != language_semantics::MachineSupplyMode::Boundary
            || machine.body_is_present
            || machine.suspends
            || machine.blocks
            || machine.target.is_some()
            || machine.authored_target.is_some()
            || !machine.conformance_bounds.is_empty()
            || !self.machine_invokes(machine).is_empty()
            || !self.machine_owned_data(machine).is_empty()
            || !self.machine_trait_conformances(machine).is_empty()
        {
            return Err(invalid);
        }
        Ok(Some(result))
    }

    fn sealed_layout_source(&self, symbol: SymbolHandle) -> bool {
        self.symbols
            .symbol_source_span(symbol)
            .and_then(|span| self.symbols.source_file(span))
            .is_some_and(|file| {
                file.origin == source::SourceOrigin::Toolchain
                    && file.package_identity.is_none()
                    && file.path.strip_prefix(&file.package_root).ok()
                        == Some(std::path::Path::new("layout.omg"))
                    && file.source.as_bytes() == CORE_LAYOUT_SOURCE
            })
    }

    pub fn core_layout_declaration(&self, symbol: SymbolHandle, name: &str) -> bool {
        symbol.is_valid()
            && self.symbols.name(symbol) == name
            && self.sealed_layout_source(symbol)
            && self.data_definition_by_symbol(symbol).is_some_and(|data| {
                data.supply_mode == language_semantics::DataSupplyMode::BoundaryOpaque
            })
    }

    pub fn layout_storage_arguments(
        &self,
        reference: TypeReferenceHandle,
    ) -> Option<LayoutStorageArguments> {
        // A reference is a stored permission descriptor, not its backing value.
        // Only value qualifications may expose the representation constructor.
        let mut application = self
            .type_reference_table
            .unconstrained_type_reference(reference);
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
