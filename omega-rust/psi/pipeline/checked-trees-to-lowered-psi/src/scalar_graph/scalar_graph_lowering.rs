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
    prepare_scalar_graph_in_namespace, prepare_scalar_graph_machine,
};
pub(crate) use graph_validation::staged_short_circuit_bindings_terminator;
pub(crate) use known_evaluation::KnownDirectScalar;

use super::{
    CheckedScalarBranchDestination, CheckedScalarExpressionRole, CheckedScalarMachineGraph,
    CheckedScalarStateTerminator, CheckedScalarSuccessor, CheckedTrees, IntegerValue, LoweredPsi,
    LoweringError, Multiplicity, PlaceId, QualifiedScalarType, ScalarType, StructuralAccess,
    StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralTypeDeclaration, build_scalar_graph_module,
    lower_checked_crash_exit, machine_id, scalar_carriers, unsupported,
};
use crate::expression_preparation::qualifications::PreparedScalarQualifications;

use crate::expression_preparation::bindings as storage;
use crate::expression_preparation::source_custody;
use crate::scalar_graph::scalar_computations as computations;
use crate::scalar_graph::scalar_graph_lowering::graph_preparation::prepare_standalone_scalar_graph_machine;

pub(crate) fn checked_scalar_computation_call_targets(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
    crate::expression_preparation::computation_graph::call_targets(checked, machine)
}

pub(crate) fn lower_scalar_graph_machine(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    graph: &CheckedScalarMachineGraph,
) -> Result<LoweredPsi, LoweringError> {
    let prepared = prepare_standalone_scalar_graph_machine(checked, machine, graph)?;
    let machine_ids = [(machine, machine_id(1))];
    let requirement_counts = [(machine, prepared.contract.requirement_count())];
    let lowered = build_scalar_graph_module(
        &prepared.states,
        &prepared.state_symbols,
        prepared.result_type,
        &prepared.scalar_qualifications,
        prepared.contract,
        prepared.crash_routes,
        prepared.identity_reshuffles,
        prepared.partition_compositions,
        machine_id(1),
        0,
        &machine_ids,
        &requirement_counts,
        prepared.loop_plan.as_ref(),
    )?;
    // The root assembler installs the complete proof vocabulary before
    // allocating invariant obligations and finalizing executable certificates.
    Ok(lowered)
}
