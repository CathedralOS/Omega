//! The installation record wire format.
//!
//! `envelope` frames the whole record and delegates each family to its codec;
//! `wire` owns the primitive reader and writers. The remaining names divide by
//! what the rows describe: `structural` and `scalar` for the two value worlds,
//! `unit` and `internal_unit` for a unit body's own rows and the calls it
//! makes, and `function` for what one installed function owns. A leaf module
//! encodes and decodes exactly one family of rows.

pub(super) mod boundary_result_scalar;
pub(super) mod boundary_settlement;
pub(super) mod call_site_owner;
pub(super) mod completion_custody;
pub(super) mod dynamic_conformance;
pub(super) mod envelope;
pub(super) mod fingerprint;
pub(super) mod function;
pub(super) mod installation_header;
pub(super) mod internal_unit;
pub(super) mod mixed_structural_scalar_abi;
pub(super) mod opaque_application;
pub(super) mod parameter_abi;
pub(super) mod port_effect;
pub(super) mod private_function;
pub(super) mod provider_execution;
pub(super) mod provider_plan;
pub(super) mod scalar;
pub(super) mod semantic_code_attribution;
pub(super) mod structural;
pub(super) mod trivial_affine_local;
pub(super) mod unit;
pub(super) mod value_placement;
pub(super) mod wire;
