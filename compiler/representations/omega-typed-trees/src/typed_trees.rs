use crate::{
    data, domain, expression, invariant, machine, platform, signature, snapshot, trait_definition,
    types,
};
use omega_core::arena::{Arena, HandleSpan};
use omega_core::diagnostics::PhaseSnapshot;
use omega_core::symbols::SymbolTable;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypedTrees {
    pub roots: TypedTreeRoots,
    pub tables: TypedTreeTables,
    pub symbols: SymbolTable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypedTreeTables {
    pub data_definitions: Arena<data::DataDefinition>,
    pub data_type_parameters: Arena<data::TypeParameter>,
    pub data_members: Arena<data::DataMember>,
    pub domain_definitions: Arena<domain::DomainDefinition>,
    pub proof_facts: Arena<domain::ProofFact>,
    pub domain_path_members: Arena<crate::name::Identifier>,
    pub operator_path_members: Arena<crate::name::Identifier>,
    pub invariant_definitions: Arena<invariant::InvariantDefinition>,
    pub machines: Arena<machine::Machine>,
    pub operators: Arena<crate::operator::OperatorDefinition>,
    pub machine_contained_objects: Arena<machine::ContainedObject>,
    pub machine_owned_data: Arena<machine::OwnedData>,
    pub machine_trait_conformances: Arena<machine::TraitConformance>,
    pub machine_states: Arena<crate::state::State>,
    pub state_parameters: Arena<signature::StateParameter>,
    pub platforms: Arena<platform::Platform>,
    pub platform_state_signatures: Arena<signature::StateSignature>,
    pub traits: Arena<trait_definition::TraitDefinition>,
    pub trait_requirements: Arena<trait_definition::TraitRequirement>,
    pub trait_machine_signatures: Arena<signature::StateSignature>,
    pub signature_effects: Arena<crate::name::Identifier>,
    pub signature_contracts: Arena<signature::SignatureContract>,
    pub expression_table: expression::ExpressionTable,
    pub statement_table: crate::statement::StatementTable,
    pub type_reference_table: types::TypeReferenceTable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypedTreeRoots {
    pub data_definitions: HandleSpan<data::DataDefinition>,
    pub domain_definitions: HandleSpan<domain::DomainDefinition>,
    pub invariant_definitions: HandleSpan<invariant::InvariantDefinition>,
    pub machines: HandleSpan<machine::Machine>,
    pub operators: HandleSpan<crate::operator::OperatorDefinition>,
    pub platforms: HandleSpan<platform::Platform>,
    pub traits: HandleSpan<trait_definition::TraitDefinition>,
}

impl TypedTreeRoots {
    pub fn with_roots(
        data_definitions: HandleSpan<data::DataDefinition>,
        domain_definitions: HandleSpan<domain::DomainDefinition>,
        invariant_definitions: HandleSpan<invariant::InvariantDefinition>,
        machines: HandleSpan<machine::Machine>,
        operators: HandleSpan<crate::operator::OperatorDefinition>,
        platforms: HandleSpan<platform::Platform>,
        traits: HandleSpan<trait_definition::TraitDefinition>,
    ) -> Self {
        Self {
            data_definitions,
            domain_definitions,
            invariant_definitions,
            machines,
            operators,
            platforms,
            traits,
        }
    }
}

impl TypedTrees {
    pub fn with_roots(
        roots: TypedTreeRoots,
        tables: TypedTreeTables,
        symbols: SymbolTable,
    ) -> Self {
        Self {
            roots,
            tables,
            symbols,
        }
    }

    pub fn push_data_definition(&mut self, data_definition: data::DataDefinition) {
        self.tables
            .data_definitions
            .append_to_span(&mut self.roots.data_definitions, data_definition);
    }

    pub fn data_definitions(&self) -> &[data::DataDefinition] {
        self.tables
            .data_definitions
            .span_or_empty(self.roots.data_definitions)
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
        self.tables
            .domain_definitions
            .append_to_span(&mut self.roots.domain_definitions, domain_definition);
    }

    pub fn push_domain_operator(
        &mut self,
        domain: &mut domain::DomainDefinition,
        operator: crate::operator::OperatorDefinition,
    ) {
        self.operators
            .append_to_span(&mut domain.operators, operator);
    }

    pub fn domain_definitions(&self) -> &[domain::DomainDefinition] {
        self.tables
            .domain_definitions
            .span_or_empty(self.roots.domain_definitions)
    }

    pub fn domain_operators(
        &self,
        domain: &domain::DomainDefinition,
    ) -> &[crate::operator::OperatorDefinition] {
        self.operators.span_or_empty(domain.operators)
    }

    pub fn proof_facts(&self, domain: &domain::DomainDefinition) -> &[domain::ProofFact] {
        self.proof_facts.span_or_empty(domain.facts)
    }

    pub fn domain_path_members(
        &self,
        span: HandleSpan<crate::name::Identifier>,
    ) -> &[crate::name::Identifier] {
        self.domain_path_members.span_or_empty(span)
    }

    pub fn push_invariant_definition(
        &mut self,
        invariant_definition: invariant::InvariantDefinition,
    ) {
        self.tables
            .invariant_definitions
            .append_to_span(&mut self.roots.invariant_definitions, invariant_definition);
    }

    pub fn invariant_definitions(&self) -> &[invariant::InvariantDefinition] {
        self.tables
            .invariant_definitions
            .span_or_empty(self.roots.invariant_definitions)
    }

    pub fn push_platform(&mut self, platform: platform::Platform) {
        self.tables
            .platforms
            .append_to_span(&mut self.roots.platforms, platform);
    }

    pub fn push_operator(&mut self, operator: crate::operator::OperatorDefinition) {
        self.tables
            .operators
            .append_to_span(&mut self.roots.operators, operator);
    }

    pub fn operators(&self) -> &[crate::operator::OperatorDefinition] {
        self.tables.operators.span_or_empty(self.roots.operators)
    }

    pub fn push_operator_path_member(
        &mut self,
        operator: &mut crate::operator::OperatorDefinition,
        member: crate::name::Identifier,
    ) {
        self.operator_path_members
            .append_to_span(&mut operator.name, member);
    }

    pub fn push_operator_type_parameter(
        &mut self,
        operator: &mut crate::operator::OperatorDefinition,
        parameter: data::TypeParameter,
    ) {
        self.data_type_parameters
            .append_to_span(&mut operator.type_parameters, parameter);
    }

    pub fn operator_type_parameters(
        &self,
        operator: &crate::operator::OperatorDefinition,
    ) -> &[data::TypeParameter] {
        self.data_type_parameters
            .span_or_empty(operator.type_parameters)
    }

    pub fn push_operator_parameter(
        &mut self,
        operator: &mut crate::operator::OperatorDefinition,
        parameter: signature::StateParameter,
    ) {
        self.state_parameters
            .append_to_span(&mut operator.parameters, parameter);
    }

    pub fn operator_parameters(
        &self,
        operator: &crate::operator::OperatorDefinition,
    ) -> &[signature::StateParameter] {
        self.state_parameters.span_or_empty(operator.parameters)
    }

    pub fn push_operator_contract(
        &mut self,
        operator: &mut crate::operator::OperatorDefinition,
        contract: signature::SignatureContract,
    ) {
        self.signature_contracts
            .append_to_span(&mut operator.contracts, contract);
    }

    pub fn operator_path_members(
        &self,
        span: HandleSpan<crate::name::Identifier>,
    ) -> &[crate::name::Identifier] {
        self.operator_path_members.span_or_empty(span)
    }

    pub fn platforms(&self) -> &[platform::Platform] {
        self.tables.platforms.span_or_empty(self.roots.platforms)
    }

    pub fn push_trait_definition(&mut self, trait_definition: trait_definition::TraitDefinition) {
        self.tables
            .traits
            .append_to_span(&mut self.roots.traits, trait_definition);
    }

    pub fn traits(&self) -> &[trait_definition::TraitDefinition] {
        self.tables.traits.span_or_empty(self.roots.traits)
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
        self.tables
            .machines
            .append_to_span(&mut self.roots.machines, machine);
    }

    pub fn machines(&self) -> &[machine::Machine] {
        self.tables.machines.span_or_empty(self.roots.machines)
    }

    pub fn machines_mut(&mut self) -> &mut [machine::Machine] {
        self.tables.machines.span_mut_or_empty(self.roots.machines)
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
        effect: crate::name::Identifier,
    ) {
        self.signature_effects
            .append_to_span(&mut machine.effects, effect);
    }

    pub fn machine_effects(&self, machine: &machine::Machine) -> &[crate::name::Identifier] {
        self.signature_effects.span_or_empty(machine.effects)
    }

    pub fn machine_decrease_order(
        &self,
        span: HandleSpan<crate::name::Identifier>,
    ) -> &[crate::name::Identifier] {
        self.signature_effects.span_or_empty(span)
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
        effect: crate::name::Identifier,
    ) {
        self.signature_effects
            .append_to_span(&mut signature.effects, effect);
    }

    pub fn state_signature_effects(
        &self,
        signature: &signature::StateSignature,
    ) -> &[crate::name::Identifier] {
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

impl Deref for TypedTrees {
    type Target = TypedTreeTables;

    fn deref(&self) -> &Self::Target {
        &self.tables
    }
}

impl DerefMut for TypedTrees {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.tables
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        TypedTreeRoots, TypedTreeTables, TypedTrees, data, domain, invariant, machine, operator,
        platform, trait_definition,
    };
    use omega_core::arena::HandleSpan;
    use omega_core::symbols::SymbolTable;

    #[test]
    fn typed_tree_roots_constructor_keeps_top_level_roots_explicit() {
        let data_definitions = HandleSpan::<data::DataDefinition>::default();
        let domain_definitions = HandleSpan::<domain::DomainDefinition>::default();
        let invariant_definitions = HandleSpan::<invariant::InvariantDefinition>::default();
        let machines = HandleSpan::<machine::Machine>::default();
        let operators = HandleSpan::<operator::OperatorDefinition>::default();
        let platforms = HandleSpan::<platform::Platform>::default();
        let traits = HandleSpan::<trait_definition::TraitDefinition>::default();

        let roots = TypedTreeRoots::with_roots(
            data_definitions,
            domain_definitions,
            invariant_definitions,
            machines,
            operators,
            platforms,
            traits,
        );

        assert_eq!(roots.data_definitions, data_definitions);
        assert_eq!(roots.domain_definitions, domain_definitions);
        assert_eq!(roots.invariant_definitions, invariant_definitions);
        assert_eq!(roots.machines, machines);
        assert_eq!(roots.operators, operators);
        assert_eq!(roots.platforms, platforms);
        assert_eq!(roots.traits, traits);
    }

    #[test]
    fn typed_trees_constructor_keeps_roots_tables_and_symbols_explicit() {
        let roots = TypedTreeRoots::default();
        let tables = TypedTreeTables::default();
        let symbols = SymbolTable::default();

        let trees = TypedTrees::with_roots(roots.clone(), tables.clone(), symbols.clone());

        assert_eq!(trees.roots, roots);
        assert_eq!(trees.tables, tables);
        assert_eq!(trees.symbols, symbols);
    }
}
