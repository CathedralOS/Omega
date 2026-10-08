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

mod call_lowering;
pub(crate) mod guards;
pub(crate) mod prepared_graph;
pub(crate) mod structural_values;
pub(crate) mod unit_operations;

use crate::emission::expression_validation::validate_boolean_parameter_types;
use crate::emission::scalar_types::terminal_scalar_type;
pub(crate) use call_lowering::lower_scalar_call;

use super::{
    CheckedScalarBranchDestination, CheckedScalarExpressionRole, CheckedTrees, LoweringError,
    Multiplicity, PlaceId, QualifiedScalarType, ScalarType, StructuralAccess, StructuralArgument,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralTypeDeclaration,
    scalar_carriers, unsupported,
};
use crate::expression_preparation::qualifications::PreparedScalarQualifications;

use crate::expression_preparation::bindings as storage;
use crate::scalar_graph::scalar_computations as computations;
