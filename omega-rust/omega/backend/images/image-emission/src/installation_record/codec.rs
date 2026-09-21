//! The installation record wire format, one codec per record family.
//! `envelope_codec` frames the whole record and delegates each family to
//! its codec; `wire_codec` owns the primitive reader and writers; every
//! other codec encodes and decodes exactly one family of rows.

pub(super) mod boundary_result_scalar_codec;
pub(super) mod boundary_settlement_codec;
pub(super) mod call_site_owner_codec;
pub(super) mod completion_custody_codec;
pub(super) mod dynamic_conformance_codec;
pub(super) mod envelope_codec;
pub(super) mod fingerprint_codec;
pub(super) mod function_affine_cleanup_codec;
pub(super) mod function_codec;
pub(super) mod function_parameter_codec;
pub(super) mod function_stack_codec;
pub(super) mod installation_header_codec;
pub(super) mod internal_unit_call_codec;
pub(super) mod internal_unit_call_source_codec;
pub(super) mod internal_unit_scalar_call_codec;
pub(super) mod mixed_structural_scalar_abi_codec;
pub(super) mod opaque_application_codec;
pub(super) mod parameter_abi_codec;
pub(super) mod port_effect_codec;
pub(super) mod private_function_codec;
pub(super) mod provider_execution_codec;
pub(super) mod provider_plan_codec;
pub(super) mod scalar_abi_codec;
pub(super) mod scalar_call_plan_codec;
pub(super) mod scalar_structural_scalar_field_store_codec;
pub(super) mod semantic_code_attribution_codec;
pub(super) mod structural_argument_codec;
pub(super) mod structural_case_codec;
pub(super) mod structural_field_codec;
pub(super) mod structural_record_codec;
pub(super) mod structural_return_codec;
pub(super) mod structural_scalar_codec;
pub(super) mod structural_signature_codec;
pub(super) mod structural_source_codec;
pub(super) mod structural_type_codec;
pub(super) mod trivial_affine_local_codec;
pub(super) mod unit_continuation_codec;
pub(super) mod unit_scalar_codec;
pub(super) mod unit_structural_scalar_field_store_codec;
pub(super) mod unit_write_only_primitive_store_codec;
pub(super) mod value_placement_codec;
pub(super) mod wire_codec;
