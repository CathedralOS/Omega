use crate::{data, expression, invariant, machine, platform, tables, types};
use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolTable;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypedTrees {
    pub root_data_definitions: HandleSpan<data::DataDefinition>,
    pub data_definitions: Arena<data::DataDefinition>,
    pub root_invariants: HandleSpan<invariant::InvariantDefinition>,
    pub invariant_definitions: Arena<invariant::InvariantDefinition>,
    pub machines: Vec<machine::Machine>,
    pub root_platforms: HandleSpan<platform::Platform>,
    pub platforms: Arena<platform::Platform>,
    pub type_constraints: Arena<types::TypeConstraint>,
    pub expression_table: expression::ExpressionTable,
    pub statement_table: crate::statement::StatementTable,
    pub type_reference_table: types::TypeReferenceTable,
    pub symbols: SymbolTable,
}

impl TypedTrees {
    pub fn push_data_definition(&mut self, data_definition: data::DataDefinition) {
        self.data_definitions
            .append_to_span(&mut self.root_data_definitions, data_definition);
    }

    pub fn data_definitions(&self) -> &[data::DataDefinition] {
        self.data_definitions
            .span_or_empty(self.root_data_definitions)
    }

    pub fn push_invariant_definition(
        &mut self,
        invariant_definition: invariant::InvariantDefinition,
    ) {
        self.invariant_definitions
            .append_to_span(&mut self.root_invariants, invariant_definition);
    }

    pub fn invariant_definitions(&self) -> &[invariant::InvariantDefinition] {
        self.invariant_definitions
            .span_or_empty(self.root_invariants)
    }

    pub fn push_platform(&mut self, platform: platform::Platform) {
        self.platforms
            .append_to_span(&mut self.root_platforms, platform);
    }

    pub fn platforms(&self) -> &[platform::Platform] {
        self.platforms.span_or_empty(self.root_platforms)
    }

    pub fn rebuild_tables(&mut self) {
        let tables = tables::TypedProgramTables::from_typed_trees_with_state_spans(self);
        self.expression_table = tables.expressions;
        self.statement_table = tables.statements;
        self.type_reference_table = tables.type_references;
    }
}
