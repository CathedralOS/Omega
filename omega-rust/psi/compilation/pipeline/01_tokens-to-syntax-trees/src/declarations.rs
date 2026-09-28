//! Top-level declarations: `parse_declaration` dispatches one item to the
//! declaration family that owns it -- capabilities, conformances, `const`
//! items, data, domains, mathematical `let` definitions, machines, measures,
//! namespaces, operators, propositions, traits and `use` items.

mod capability;
mod conformance;
mod const_item;
mod data;
mod domain;
mod let_definition;
mod machines;
mod measure;
mod namespace;
mod operator;
pub(crate) mod parse_declaration;
mod proposition;
mod trait_definition;
mod use_item;
