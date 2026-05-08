pub mod data;
pub mod expression;
pub mod identity;
pub mod invariant;
pub mod lowering;
pub mod machine;
pub mod platform;
pub mod signature;
pub mod state;
pub mod statement;
pub mod types;

use omega_core::arena::Arena;
use omega_core::symbols::SymbolTable;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Program {
    pub data_definitions: Vec<data::DataDefinition>,
    pub invariant_definitions: Vec<invariant::InvariantDefinition>,
    pub machines: Vec<machine::Machine>,
    pub platforms: Vec<platform::Platform>,
    pub type_constraints: Arena<types::TypeConstraint>,
    pub symbols: SymbolTable,
}
