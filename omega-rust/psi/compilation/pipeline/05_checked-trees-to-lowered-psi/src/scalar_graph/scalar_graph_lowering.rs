//! Scalar-graph preparation, validation, partial evaluation, and lowering.
//!
//! This file lowers one scalar-graph machine. `graph_preparation.rs`
//! prepares the graph under its contract mode, `known_evaluation.rs`
//! evaluates compile-known graphs and direct expressions,
//! `call_lowering.rs` lowers scalar calls and successors,
//! `expression_lowering.rs` lowers scalar and boolean expressions,
//! `graph_validation.rs` checks graph cycles and stages short-circuit bindings,
//! `contract_lowering.rs` lowers closed scalar contracts. Shared primitive types
//! and lowered-expression checks belong to emission; the remaining files
//! carry bindings, field stores, guards, cycles and locals.

mod bindings;
pub(crate) mod branch_destinations;
mod call_lowering;
mod contract_lowering;
pub(crate) mod cycles;
mod field_stores;
mod graph_preparation;
mod graph_validation;
pub(crate) mod guards;
mod known_evaluation;
pub(crate) mod prepared_graph;
pub(crate) mod primitive_locals;
#[cfg(test)]
mod primitive_read_tests;
pub(crate) mod structural_values;
pub(crate) mod unit_operations;

use crate::emission::expression_validation::{
    validate_boolean_parameter_types, validate_direct_parameter_types,
};
use crate::emission::scalar_types::terminal_scalar_type;
pub(crate) use call_lowering::lower_scalar_call;

pub(crate) use graph_preparation::{
    prepare_scalar_graph_in_namespace, prepare_scalar_graph_machine, prepare_scalar_graph_root,
};
pub(crate) use graph_validation::staged_short_circuit_bindings_terminator;
pub(crate) use known_evaluation::KnownDirectScalar;

use super::{
    CheckedScalarBranchDestination, CheckedScalarExpressionRole, CheckedScalarMachineGraph,
    CheckedScalarStateTerminator, CheckedScalarSuccessor, CheckedTrees, IntegerValue,
    LoweringError, Multiplicity, PlaceId, QualifiedScalarType, ScalarType, StructuralAccess,
    StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralTypeDeclaration, lower_checked_crash_exit, scalar_carriers,
    unsupported,
};
use crate::expression_preparation::qualifications::PreparedScalarQualifications;

use crate::expression_preparation::bindings as storage;
use crate::expression_preparation::source_custody;
use crate::scalar_graph::scalar_computations as computations;

pub(crate) fn checked_scalar_computation_call_targets(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
    crate::expression_preparation::computation_graph::call_targets(checked, machine)
}
