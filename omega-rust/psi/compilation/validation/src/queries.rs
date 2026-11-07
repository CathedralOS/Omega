//! Read-only semantic queries that checking, lowering and compile-time
//! evaluation all read: `execution_projection` selects executable declaration
//! contexts without granting a checked judgment; `expression_traversal` walks
//! structural expression edges; `type_applications` and `type_regions` read
//! the frozen type table; `layout_checking` borrows one exact layout
//! alternative at a time; `product_queries` answers demand-time Product
//! declaration questions; `substitutions` looks up parameter bindings.

pub mod execution_projection;
pub mod expression_traversal;
pub mod statement_sites;
pub(crate) mod layout_checking;
pub mod product_queries;
pub mod substitutions;
pub(crate) mod type_applications;
pub(crate) mod type_regions;
