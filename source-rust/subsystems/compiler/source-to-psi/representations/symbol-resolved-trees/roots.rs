//! Ordered top-level declarations and machine roots.
use crate::constant::ConstDeclaration;
use crate::trait_definition::TraitDefinition;
use crate::{
    OrderedRootArena, machine_declarations, mathematical, measure, operator, proposition, wire,
};
#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
pub struct SymbolResolvedRoots {
    pub const_declarations: OrderedRootArena<crate::source_resolved::constant::ConstDeclaration>,
    pub data_definitions: OrderedRootArena<crate::source_resolved::data::DataDefinition>,
    pub domain_definitions: OrderedRootArena<crate::source_resolved::domain::DomainDefinition>,
    pub machines: machine_declarations::MachineDeclarations,
    pub measures: OrderedRootArena<measure::MeasureDefinition>,
    pub operators: OrderedRootArena<operator::OperatorDefinition>,
    pub propositions: OrderedRootArena<proposition::PropositionDefinition>,
    pub mathematical_definitions: OrderedRootArena<mathematical::MathematicalDefinition>,
    pub traits: OrderedRootArena<crate::source_resolved::trait_definition::TraitDefinition>,
    pub conformances: OrderedRootArena<crate::source_resolved::trait_definition::Conformance>,
    pub wire_schemas: OrderedRootArena<wire::WireSchema>,
    pub type_aliases: OrderedRootArena<crate::source_resolved::type_aliases::TypeAliasDefinition>,
}

impl SymbolResolvedRoots {
    pub fn trait_definition(&self, symbol: symbols::SymbolHandle) -> Option<&TraitDefinition> {
        self.traits
            .iter()
            .find(|definition| definition.symbol == symbol)
    }

    pub fn const_declaration(&self, symbol: symbols::SymbolHandle) -> Option<&ConstDeclaration> {
        self.const_declarations
            .iter()
            .find(|declaration| declaration.symbol == symbol)
    }

    pub fn with_roots(
        data_definitions: OrderedRootArena<crate::source_resolved::data::DataDefinition>,
        domain_definitions: OrderedRootArena<crate::source_resolved::domain::DomainDefinition>,
        machines: OrderedRootArena<crate::source_resolved::machine::Machine>,
        operators: OrderedRootArena<operator::OperatorDefinition>,
        traits: OrderedRootArena<crate::source_resolved::trait_definition::TraitDefinition>,
    ) -> Self {
        Self {
            const_declarations: OrderedRootArena::default(),
            data_definitions,
            domain_definitions,
            machines: machines.into(),
            measures: OrderedRootArena::default(),
            operators,
            propositions: OrderedRootArena::default(),
            mathematical_definitions: OrderedRootArena::default(),
            traits,
            conformances: OrderedRootArena::default(),
            wire_schemas: OrderedRootArena::default(),
            type_aliases: OrderedRootArena::default(),
        }
    }
}
