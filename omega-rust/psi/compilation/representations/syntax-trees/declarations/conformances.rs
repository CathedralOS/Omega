use crate::item::*;

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct ConformanceItem {
    pub is_public: bool,
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub trait_name: Identifier,
    pub trait_lifetime_arguments: Vec<Identifier>,
    pub trait_arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    pub alias: Option<Identifier>,
    pub members: HandleSpan<ConformanceMember>,
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub enum ConformanceMember {
    Machine(Machine),
    TraitDefault {
        declaring_trait: Identifier,
        requirement_ordinal: usize,
        machine: Machine,
    },
    Synthesized(Machine),
    /*
        A selection row, `Trait::member = Existing::machine;`. The
        implementation is a record of selected operations
        (wiki/spec/language/conformances.md, "Implementations are
        operation records"), so this slot holds the named machine
        itself: no member machine is declared, and the selected
        declaration keeps its own identity, contract and laws.
    */
    Selection {
        declaring_trait: Identifier,
        requirement: Identifier,
        target: HandleSpan<Identifier>,
    },
}

impl Default for ConformanceMember {
    fn default() -> Self {
        Self::Machine(Machine::default())
    }
}
