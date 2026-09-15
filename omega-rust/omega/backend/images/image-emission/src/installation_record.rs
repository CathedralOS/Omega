//! Canonical installation record wire format: the manifest metadata sealed
//! over one emitted executable image. This root owns the format marker;
//! `record_types.rs` holds the record carriers, `record_construction.rs`
//! builds a record from an image, `envelope_codec.rs` encodes and decodes the
//! envelope, `record_validation.rs` validates a decoded record against the
//! image and `installation_errors.rs` names the errors. Each `*_codec` child
//! owns one record family's bytes, `record_shape` owns canonical-shape
//! validation of a decoded record, and `installed_unit_scalar_transport` plus
//! the structural children replay installed call custody. The record grants
//! no executable authority.

mod borrowed_structural;
mod boundary_result_scalar_codec;
mod boundary_settlement_codec;
mod call_site_owner_codec;
mod completion_custody_codec;
mod dynamic_conformance_codec;
mod envelope_codec;
mod fingerprint_codec;
mod function_affine_cleanup_codec;
mod function_codec;
mod function_parameter_codec;
mod function_stack_codec;
mod graph_structural;
mod incoming_structural;
mod installation_errors;
mod installation_header_codec;
mod installed_unit_scalar_transport;
mod internal_unit_call_codec;
mod internal_unit_call_source_codec;
mod internal_unit_scalar_call_codec;
mod mixed_structural_scalar_abi_codec;
mod parameter_abi_codec;
mod port_effect_codec;
mod private_function_codec;
mod provider_execution_codec;
mod provider_plan_codec;
mod record_construction;
mod record_shape;
mod record_types;
mod record_validation;
#[cfg(test)]
mod resource_tests;
mod scalar_abi_codec;
mod scalar_call_plan_codec;
mod scalar_structural_scalar_field_store_codec;
mod semantic_code_attribution;
mod semantic_code_attribution_codec;
mod structural_argument_codec;
mod structural_case_codec;
mod structural_field_codec;
mod structural_record_codec;
mod structural_return_codec;
mod structural_scalar_codec;
mod structural_signature_codec;
mod structural_source_codec;
mod structural_type_codec;
mod trivial_affine_local_codec;
mod unit_continuation_codec;
mod unit_dynamic_descriptor_join;
mod unit_scalar_codec;
mod unit_structural_scalar_field_store_codec;
mod unit_write_only_primitive_store_codec;
mod value_placement_codec;
mod wire_codec;

pub use envelope_codec::{decode_installation_record, encode_installation_record};
pub use installation_errors::InstallationError;
pub use record_construction::{
    InstallationStackError, build_installation_record, build_installation_record_with_evidence,
    build_installation_record_with_provider_executions,
    build_installation_record_with_selected_provider_plans_and_evidence,
    derive_installation_stack_demand,
};
pub use record_types::{
    ImageFingerprint, InitializedDataFingerprint, InstallationFingerprint, InstallationRecord,
    InstalledCompilerPrivateFunction, InstalledComponentProgress, InstalledDynamicCall,
    InstalledDynamicConformanceSlot, InstalledDynamicConformanceTable,
    InstalledDynamicParameterCall, InstalledForeignCallStack,
    InstalledForwardedDynamicDescriptorAdapter, InstalledForwardedDynamicDescriptorCall,
    InstalledForwardedDynamicDescriptorSlot, InstalledForwardedDynamicDescriptorTable,
    InstalledForwardedDynamicParameterCall, InstalledFunction, InstalledImageSections,
    InstalledInternalUnitCall, InstalledInternalUnitScalarCall, InstalledStoredDynamicCall,
    InstalledStructuralReturn, SelectedProviderPlanReportIdentity,
};
pub use record_validation::{installation_fingerprint, validate_installation_record};

use crate::object_artifact::replay::boundary::byte_sequence_custody::linux_write_line_custody_is_exact;
use crate::object_artifact::replay::boundary::completion_receipts::{
    CompletionCustodyError, validate_completion_custody,
};
use crate::object_artifact::replay::boundary::result_placement::boundary_result_is_exact;
use crate::object_artifact::replay::boundary::runtime_scalar_custody::hosted_write_byte_custody_is_exact;
use crate::{
    ObjectBoundarySettlement, ObjectCodeAttribution, ObjectPortEffect, can_emit_executable_image,
};
use calling_conventions::{
    CallSignature, CallingPolicy, ValueClass, ValueLocation, ValuePlacement, ValueShape,
    evaluate_call_plan,
};
use machine_code::SemanticCodeSite;
use semantic_vocabulary::{MachineId, StructuralTypeId};
use target::{Architecture, ObjectFormat};
use target_operations::{BoundaryRealization, CallSiteOwner};
use terminal_psi::{StructuralMultiplicity, StructuralPathSegment, StructuralTypeShape};

use fingerprint_codec::fingerprint_initialized_data;

use installed_unit_scalar_transport::{
    installed_forwarded_dynamic_scalar_result_is_canonical,
    installed_function_scalar_transport_is_canonical, validate_installed_unit_scalar_calls,
    validate_installed_unit_structural_scalar_field_stores,
    validate_installed_unit_write_only_primitive_stores,
};

use structural_case_codec::{decode_structural_cases, encode_structural_cases};
use structural_record_codec::{decode_structural_fields, encode_structural_fields};

use structural_scalar_codec::{
    decode_identity, decode_multiplicity, encode_identity, multiplicity_tag,
};
use structural_type_codec::{decode_structural_types, encode_structural_types};
use unit_dynamic_descriptor_join::validate_installed_unit_dynamic_descriptor_joins;
use wire_codec::{Reader, decode_boolean, push_u16, push_u32, push_u64, push_u128};

/// The current vocabulary includes owned incoming stack pointers and AArch64's
/// dedicated indirect-result register. Earlier envelopes cannot carry those roles.
/// Marker 97 seals complete placed-region inventory identities for the bound
/// image; marker 96 records carry no complete-custody digests.
pub const INSTALLATION_FORMAT_MARKER: u16 = 97;

fn direct_structural_return_placement(placement: &ValuePlacement) -> bool {
    if placement.shape.class != ValueClass::Integer
        || !((placement.shape.byte_size == 8 && placement.shape.alignment == 8)
            || (9..=16).contains(&placement.shape.byte_size))
        || !(1..=2).contains(&placement.locations.len())
    {
        return false;
    }
    let mut expected_offset = 0_u16;
    for location in &placement.locations {
        let ValueLocation::Register {
            value_byte_offset,
            byte_size,
            ..
        } = *location
        else {
            return false;
        };
        let expected_size = (placement.shape.byte_size - expected_offset).min(8);
        if value_byte_offset != expected_offset || byte_size != expected_size {
            return false;
        }
        let Some(next) = expected_offset.checked_add(byte_size) else {
            return false;
        };
        expected_offset = next;
    }
    expected_offset == placement.shape.byte_size
}
