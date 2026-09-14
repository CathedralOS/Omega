//! Per-node translation of syntax into the symbol-resolved carrier.
//!
//! `item` dispatches each root item to the module owning its carrier area;
//! `statement` and `expression` translate bodies; `type_reference` and `name`
//! serve every other module. Each translator borrows the `Lowerer`, appends
//! to its trees, and records the selections a later phase settles. Nothing
//! here assigns a symbol.

pub(crate) mod data;
pub(crate) mod domain;
pub(crate) mod expression;
pub(crate) mod item;
pub(crate) mod machine;
pub(crate) mod measure;
pub(crate) mod name;
pub(crate) mod operator;
pub(crate) mod proposition;
pub(crate) mod state;
pub(crate) mod statement;
pub(crate) mod trait_definition;
pub(crate) mod type_reference;
pub(crate) mod wire;
