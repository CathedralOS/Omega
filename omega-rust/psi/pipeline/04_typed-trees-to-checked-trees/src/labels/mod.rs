//! Labels: the text the checking stage uses for machines, call targets,
//! symbols, places and facts, and the conversion of checked-tree fact kinds
//! into the fact plan's kinds.
//!
//! There is no entry function. Each child is a set of queries that the
//! checks, fact construction and flow facts call directly:
//!
//! - `names` names a machine, a call target (a free machine's implicit
//!   `entry` state takes the machine's name) and any symbol;
//! - `places` labels a borrow access, a place with extra segments, and the
//!   requirement or Boolean expression a semantic fact states;
//! - `operators` renders an operator contract expression with each formal
//!   parameter replaced by a given operand label;
//! - `kinds` converts checked-tree contract and proof fact kinds into the
//!   fact plan's kinds.
//!
//! Most uses are diagnostic text, but some labels are compared. Flow facts
//! store the operator rendering as the instantiated expression of an
//! operator `ensures` fact, the operator `requires` check and named operator
//! crash routes compare a rendered requirement with the text of Boolean
//! facts, and `semantic::places` matches parameters by `symbol_name`.

mod kinds;
mod names;
mod operators;
mod places;

pub(crate) use kinds::{semantic_contract_fact_kind, semantic_proof_obligation_kind};
pub(crate) use names::{call_target_label, machine_name, symbol_name};
pub(crate) use operators::instantiate_operator_contract_expression_label_with_labels;
pub(crate) use places::{
    borrow_access_label, joined_place_label, semantic_boolean_fact_label,
    semantic_fact_requirement_label,
};
