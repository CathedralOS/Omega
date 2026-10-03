//! Scalar callee preparation and catalogs beneath externally assembled control.
//!
//! Selected scalar roots use the same closure assembler as operation bodies in
//! `unit::attached_unit`. A second graph-only assembler made call composition
//! depend on whether a closure happened to contain places or services. Scalar
//! graphs still own their signatures, contracts, effects, and source replay;
//! the common assembler owns their identities and qualification namespace.

use super::{
    CheckedScalarBindingValue, CheckedTerminalSignatureEligibility, CheckedTrees, LoweredPsi,
    LoweringError, PrimitiveType, TERMINAL_MACHINE_IDENTITY_STRIDE, build_scalar_graph_module,
    machine_id, prepare_scalar_graph_machine, unsupported,
};

pub(crate) mod callee;
pub(crate) mod embedded;
