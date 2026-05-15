use crate::{data, expression, invariant, machine, platform, signature, tables, types};
use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolTable;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypedTrees {
    pub root_data_definitions: HandleSpan<data::DataDefinition>,
    pub data_definitions: Arena<data::DataDefinition>,
    pub root_invariants: HandleSpan<invariant::InvariantDefinition>,
    pub invariant_definitions: Arena<invariant::InvariantDefinition>,
    pub root_machines: HandleSpan<machine::Machine>,
    pub machines: Arena<machine::Machine>,
    pub machine_contained_objects: Arena<machine::ContainedObject>,
    pub machine_owned_data: Arena<machine::OwnedData>,
    pub machine_states: Arena<crate::state::State>,
    pub root_platforms: HandleSpan<platform::Platform>,
    pub platforms: Arena<platform::Platform>,
    pub platform_state_signatures: Arena<signature::StateSignature>,
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

    pub fn push_platform_state_signature(
        &mut self,
        platform: &mut platform::Platform,
        signature: signature::StateSignature,
    ) {
        self.platform_state_signatures
            .append_to_span(&mut platform.states, signature);
    }

    pub fn platform_state_signatures(
        &self,
        platform: &platform::Platform,
    ) -> &[signature::StateSignature] {
        self.platform_state_signatures
            .span_or_empty(platform.states)
    }

    pub fn push_machine(&mut self, machine: machine::Machine) {
        self.machines
            .append_to_span(&mut self.root_machines, machine);
    }

    pub fn machines(&self) -> &[machine::Machine] {
        self.machines.span_or_empty(self.root_machines)
    }

    pub fn machines_mut(&mut self) -> &mut [machine::Machine] {
        self.machines.span_mut_or_empty(self.root_machines)
    }

    pub fn push_machine_contained_object(
        &mut self,
        machine: &mut machine::Machine,
        contained_object: machine::ContainedObject,
    ) {
        self.machine_contained_objects
            .append_to_span(&mut machine.contains, contained_object);
    }

    pub fn machine_contained_objects(
        &self,
        machine: &machine::Machine,
    ) -> &[machine::ContainedObject] {
        self.machine_contained_objects
            .span_or_empty(machine.contains)
    }

    pub fn push_machine_owned_data(
        &mut self,
        machine: &mut machine::Machine,
        owned_data: machine::OwnedData,
    ) {
        self.machine_owned_data
            .append_to_span(&mut machine.owned_data, owned_data);
    }

    pub fn machine_owned_data(&self, machine: &machine::Machine) -> &[machine::OwnedData] {
        self.machine_owned_data.span_or_empty(machine.owned_data)
    }

    pub fn push_machine_state(
        &mut self,
        machine: &mut machine::Machine,
        state: crate::state::State,
    ) {
        self.machine_states
            .append_to_span(&mut machine.states, state);
    }

    pub fn machine_states(&self, machine: &machine::Machine) -> &[crate::state::State] {
        self.machine_states.span_or_empty(machine.states)
    }

    pub fn machine_states_mut(
        &mut self,
        machine: &machine::Machine,
    ) -> &mut [crate::state::State] {
        self.machine_states.span_mut_or_empty(machine.states)
    }

    pub fn rebuild_tables(&mut self) {
        let tables = tables::TypedProgramTables::from_typed_trees_with_state_spans(self);
        self.expression_table = tables.expressions;
        self.statement_table = tables.statements;
        self.type_reference_table = tables.type_references;
    }
}
