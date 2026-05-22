use crate::{
    data, domain, expression, invariant, machine, platform, signature, snapshot, trait_definition,
    types,
};
use omega_core::arena::{Arena, HandleSpan};
use omega_core::diagnostics::PhaseSnapshot;
use omega_core::symbols::SymbolTable;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypedTrees {
    pub root_data_definitions: HandleSpan<data::DataDefinition>,
    pub data_definitions: Arena<data::DataDefinition>,
    pub data_type_parameters: Arena<data::TypeParameter>,
    pub data_members: Arena<data::DataMember>,
    pub root_domains: HandleSpan<domain::DomainDefinition>,
    pub domain_definitions: Arena<domain::DomainDefinition>,
    pub root_invariants: HandleSpan<invariant::InvariantDefinition>,
    pub invariant_definitions: Arena<invariant::InvariantDefinition>,
    pub root_machines: HandleSpan<machine::Machine>,
    pub machines: Arena<machine::Machine>,
    pub machine_contained_objects: Arena<machine::ContainedObject>,
    pub machine_owned_data: Arena<machine::OwnedData>,
    pub machine_trait_conformances: Arena<machine::TraitConformance>,
    pub machine_states: Arena<crate::state::State>,
    pub state_parameters: Arena<signature::StateParameter>,
    pub root_platforms: HandleSpan<platform::Platform>,
    pub platforms: Arena<platform::Platform>,
    pub platform_state_signatures: Arena<signature::StateSignature>,
    pub root_traits: HandleSpan<trait_definition::TraitDefinition>,
    pub traits: Arena<trait_definition::TraitDefinition>,
    pub trait_requirements: Arena<trait_definition::TraitRequirement>,
    pub trait_machine_signatures: Arena<signature::StateSignature>,
    pub signature_effects: Arena<crate::name::ProgramName>,
    pub signature_contracts: Arena<signature::SignatureContract>,
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

    pub fn push_domain_definition(&mut self, domain_definition: domain::DomainDefinition) {
        self.domain_definitions
            .append_to_span(&mut self.root_domains, domain_definition);
    }

    pub fn domain_definitions(&self) -> &[domain::DomainDefinition] {
        self.domain_definitions.span_or_empty(self.root_domains)
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

    pub fn push_trait_definition(&mut self, trait_definition: trait_definition::TraitDefinition) {
        self.traits
            .append_to_span(&mut self.root_traits, trait_definition);
    }

    pub fn traits(&self) -> &[trait_definition::TraitDefinition] {
        self.traits.span_or_empty(self.root_traits)
    }

    pub fn push_trait_requirement(
        &mut self,
        trait_definition: &mut trait_definition::TraitDefinition,
        requirement: trait_definition::TraitRequirement,
    ) {
        self.trait_requirements
            .append_to_span(&mut trait_definition.requires, requirement);
    }

    pub fn trait_requirements(
        &self,
        trait_definition: &trait_definition::TraitDefinition,
    ) -> &[trait_definition::TraitRequirement] {
        self.trait_requirements
            .span_or_empty(trait_definition.requires)
    }

    pub fn push_trait_machine_signature(
        &mut self,
        trait_definition: &mut trait_definition::TraitDefinition,
        signature: signature::StateSignature,
    ) {
        self.trait_machine_signatures
            .append_to_span(&mut trait_definition.machines, signature);
    }

    pub fn trait_machine_signatures(
        &self,
        trait_definition: &trait_definition::TraitDefinition,
    ) -> &[signature::StateSignature] {
        self.trait_machine_signatures
            .span_or_empty(trait_definition.machines)
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

    pub fn push_machine_trait_conformance(
        &mut self,
        machine: &mut machine::Machine,
        conformance: machine::TraitConformance,
    ) {
        self.machine_trait_conformances
            .append_to_span(&mut machine.satisfies, conformance);
    }

    pub fn machine_trait_conformances(
        &self,
        machine: &machine::Machine,
    ) -> &[machine::TraitConformance] {
        self.machine_trait_conformances
            .span_or_empty(machine.satisfies)
    }

    pub fn push_machine_effect(
        &mut self,
        machine: &mut machine::Machine,
        effect: crate::name::ProgramName,
    ) {
        self.signature_effects
            .append_to_span(&mut machine.effects, effect);
    }

    pub fn machine_effects(&self, machine: &machine::Machine) -> &[crate::name::ProgramName] {
        self.signature_effects.span_or_empty(machine.effects)
    }

    pub fn push_machine_contract(
        &mut self,
        machine: &mut machine::Machine,
        contract: signature::SignatureContract,
    ) {
        self.signature_contracts
            .append_to_span(&mut machine.contracts, contract);
    }

    pub fn machine_contracts(&self, machine: &machine::Machine) -> &[signature::SignatureContract] {
        self.signature_contracts.span_or_empty(machine.contracts)
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

    pub fn push_state_signature_effect(
        &mut self,
        signature: &mut signature::StateSignature,
        effect: crate::name::ProgramName,
    ) {
        self.signature_effects
            .append_to_span(&mut signature.effects, effect);
    }

    pub fn state_signature_effects(
        &self,
        signature: &signature::StateSignature,
    ) -> &[crate::name::ProgramName] {
        self.signature_effects.span_or_empty(signature.effects)
    }

    pub fn push_state_signature_contract(
        &mut self,
        signature: &mut signature::StateSignature,
        contract: signature::SignatureContract,
    ) {
        self.signature_contracts
            .append_to_span(&mut signature.contracts, contract);
    }

    pub fn state_signature_contracts(
        &self,
        signature: &signature::StateSignature,
    ) -> &[signature::SignatureContract] {
        self.signature_contracts.span_or_empty(signature.contracts)
    }

    pub fn display_type_reference(&self, type_reference: types::TypeReferenceHandle) -> String {
        self.type_reference_table.display_name(type_reference)
    }

    pub fn display_type_reference_with_constraints(
        &self,
        type_reference: types::TypeReferenceHandle,
    ) -> String {
        self.type_reference_table
            .display_name_with_constraints(type_reference, &self.expression_table)
    }

    pub fn primitive_type_reference(
        &self,
        type_reference: types::TypeReferenceHandle,
    ) -> Option<types::PrimitiveType> {
        self.type_reference_table.primitive_type(type_reference)
    }

    pub fn type_reference_symbol(
        &self,
        type_reference: types::TypeReferenceHandle,
    ) -> omega_core::symbols::SymbolHandle {
        self.type_reference_table.type_symbol(type_reference)
    }

    pub fn snapshot(&self) -> snapshot::TypedTreesSnapshot {
        snapshot::TypedTreesSnapshot::from_typed_trees(self)
    }

    pub fn snapshot_json(&self) -> Result<String, serde_json::Error> {
        self.snapshot().to_json()
    }

    pub fn snapshot_json_pretty(&self) -> Result<String, serde_json::Error> {
        self.snapshot().to_json_pretty()
    }
}

impl PhaseSnapshot for TypedTrees {
    type Snapshot = snapshot::TypedTreesSnapshot;

    fn snapshot(&self) -> Self::Snapshot {
        TypedTrees::snapshot(self)
    }
}
