//! The installation record wire format, one codec per record family.
//! `envelope` frames the whole record and delegates each family to
//! its codec; `wire` owns the primitive reader and writers; every
//! other codec encodes and decodes exactly one family of rows.

pub(super) mod boundary_result_scalar;
pub(super) mod boundary_settlement;
pub(super) mod call_site_owner;
pub(super) mod completion_custody;
pub(super) mod dynamic_conformance;
pub(super) mod envelope;
pub(super) mod fingerprint;
pub(super) mod function;
pub(super) mod function_affine_cleanup;
pub(super) mod function_parameter;
pub(super) mod function_stack;
pub(super) mod installation_header;
pub(super) mod internal_unit_call;
pub(super) mod internal_unit_call_source;
pub(super) mod internal_unit_scalar_call;
pub(super) mod mixed_structural_scalar_abi;
pub(super) mod opaque_application;
pub(super) mod parameter_abi;
pub(super) mod port_effect;
pub(super) mod private_function;
pub(super) mod provider_execution;
pub(super) mod provider_plan;
pub(super) mod scalar_abi;
pub(super) mod scalar_call_plan;
pub(super) mod scalar_structural_scalar_field_store;
pub(super) mod semantic_code_attribution;
pub(super) mod structural_argument;
pub(super) mod structural_case;
pub(super) mod structural_field;
pub(super) mod structural_record;
pub(super) mod structural_return;
pub(super) mod structural_scalar;
pub(super) mod structural_signature;
pub(super) mod structural_source;
pub(super) mod structural_type;
pub(super) mod trivial_affine_local;
pub(super) mod unit_continuation;
pub(super) mod unit_scalar;
pub(super) mod unit_structural_scalar_field_store;
pub(super) mod unit_write_only_primitive_store;
pub(super) mod value_placement;
pub(super) mod wire;
