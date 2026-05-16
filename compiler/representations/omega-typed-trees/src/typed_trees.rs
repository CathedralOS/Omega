use crate::{data, expression, invariant, machine, platform, signature, tables, types};
use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolTable;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypedTrees {
    pub root_data_definitions: HandleSpan<data::DataDefinition>,
    pub data_definitions: Arena<data::DataDefinition>,
    pub data_type_parameters: Arena<data::TypeParameter>,
    pub data_members: Arena<data::DataMember>,
    pub root_invariants: HandleSpan<invariant::InvariantDefinition>,
    pub invariant_definitions: Arena<invariant::InvariantDefinition>,
    pub root_machines: HandleSpan<machine::Machine>,
    pub machines: Arena<machine::Machine>,
    pub machine_contained_objects: Arena<machine::ContainedObject>,
    pub machine_owned_data: Arena<machine::OwnedData>,
    pub machine_states: Arena<crate::state::State>,
    pub state_parameters: Arena<signature::StateParameter>,
    pub state_statements: Arena<crate::statement::Statement>,
    pub statement_expressions: Arena<expression::Expression>,
    pub statement_path_members: Arena<crate::name::ProgramName>,
    pub type_reference_arguments: Arena<types::TypeReference>,
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

    pub fn push_data_type_parameter(
        &mut self,
        data_definition: &mut data::DataDefinition,
        type_parameter: data::TypeParameter,
    ) {
        self.data_type_parameters
            .append_to_span(&mut data_definition.type_parameters, type_parameter);
    }

    pub fn data_type_parameters(
        &self,
        data_definition: &data::DataDefinition,
    ) -> &[data::TypeParameter] {
        self.data_type_parameters
            .span_or_empty(data_definition.type_parameters)
    }

    pub fn push_data_member(
        &mut self,
        data_definition: &mut data::DataDefinition,
        member: data::DataMember,
    ) {
        self.data_members
            .append_to_span(&mut data_definition.members, member);
    }

    pub fn data_members(&self, data_definition: &data::DataDefinition) -> &[data::DataMember] {
        self.data_members.span_or_empty(data_definition.members)
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

    pub fn machine_states_mut(&mut self, machine: &machine::Machine) -> &mut [crate::state::State] {
        self.machine_states.span_mut_or_empty(machine.states)
    }

    pub fn push_state_parameter(
        &mut self,
        state: &mut crate::state::State,
        parameter: signature::StateParameter,
    ) {
        self.state_parameters
            .append_to_span(&mut state.parameters, parameter);
    }

    pub fn state_parameters(&self, state: &crate::state::State) -> &[signature::StateParameter] {
        self.state_parameters.span_or_empty(state.parameters)
    }

    pub fn push_state_signature_parameter(
        &mut self,
        signature: &mut signature::StateSignature,
        parameter: signature::StateParameter,
    ) {
        self.state_parameters
            .append_to_span(&mut signature.parameters, parameter);
    }

    pub fn state_signature_parameters(
        &self,
        signature: &signature::StateSignature,
    ) -> &[signature::StateParameter] {
        self.state_parameters.span_or_empty(signature.parameters)
    }

    pub fn push_state_statement(
        &mut self,
        state: &mut crate::state::State,
        statement: crate::statement::Statement,
    ) {
        self.state_statements
            .append_to_span(&mut state.statements, statement);
    }

    pub fn state_statements(&self, state: &crate::state::State) -> &[crate::statement::Statement] {
        self.state_statements.span_or_empty(state.statements)
    }

    pub fn push_statement_expression(
        &mut self,
        expressions: &mut HandleSpan<expression::Expression>,
        expression: expression::Expression,
    ) {
        self.statement_expressions
            .append_to_span(expressions, expression);
    }

    pub fn statement_expressions(
        &self,
        expressions: HandleSpan<expression::Expression>,
    ) -> &[expression::Expression] {
        self.statement_expressions.span_or_empty(expressions)
    }

    pub fn push_statement_path_member(
        &mut self,
        path: &mut HandleSpan<crate::name::ProgramName>,
        member: crate::name::ProgramName,
    ) {
        self.statement_path_members.append_to_span(path, member);
    }

    pub fn statement_path_members(
        &self,
        path: HandleSpan<crate::name::ProgramName>,
    ) -> &[crate::name::ProgramName] {
        self.statement_path_members.span_or_empty(path)
    }

    pub fn call_arguments(&self, call: &crate::statement::Call) -> &[expression::Expression] {
        self.statement_expressions(call.arguments)
    }

    pub fn transition_target_arguments(
        &self,
        target: &crate::statement::TransitionTarget,
    ) -> &[expression::Expression] {
        match target {
            crate::statement::TransitionTarget::Named { arguments, .. } => {
                self.statement_expressions(*arguments)
            }
            _ => &[],
        }
    }

    pub fn push_type_reference_argument(
        &mut self,
        arguments: &mut HandleSpan<types::TypeReference>,
        argument: types::TypeReference,
    ) {
        self.type_reference_arguments
            .append_to_span(arguments, argument);
    }

    pub fn type_reference_arguments(
        &self,
        type_reference: &types::TypeReference,
    ) -> &[types::TypeReference] {
        match type_reference {
            types::TypeReference::Generic { arguments, .. } => {
                self.type_reference_arguments.span_or_empty(*arguments)
            }
            _ => &[],
        }
    }

    pub fn rebuild_tables(&mut self) {
        let tables = tables::TypedProgramTables::from_typed_trees_with_state_spans(self);
        self.expression_table = tables.expressions;
        self.statement_table = tables.statements;
        self.type_reference_table = tables.type_references;
    }
}
