//! Canonical installation record wire format: the manifest metadata sealed
//! over one emitted executable image. This root owns the format marker.
//! `record_types.rs` holds the record carriers, `record_construction.rs`
//! builds a record from an image, `record_validation.rs` validates a decoded
//! record against the image, `record_shape` validates a decoded record's
//! canonical shape, and `installation_errors.rs` names the errors. `codec/`
//! owns the bytes: the envelope and one codec per record family.
//! `installed_unit_scalar_transport` plus the structural children replay
//! installed call custody. The record grants no executable authority.

mod borrowed_structural;
mod codec;
mod graph_structural;
mod incoming_structural;
mod installation_errors;
mod installed_unit_scalar_transport;
mod record_construction;
mod record_shape;
mod record_types;
mod record_validation;
#[cfg(test)]
mod resource_tests;
mod semantic_code_attribution;
mod unit_dynamic_descriptor_join;

pub use codec::envelope_codec::{decode_installation_record, encode_installation_record};
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

use codec::fingerprint_codec::fingerprint_initialized_data;

use installed_unit_scalar_transport::{
    installed_forwarded_dynamic_scalar_result_is_canonical,
    installed_function_scalar_transport_is_canonical, validate_installed_unit_scalar_calls,
    validate_installed_unit_structural_scalar_field_stores,
    validate_installed_unit_write_only_primitive_stores,
};

use codec::structural_case_codec::{decode_structural_cases, encode_structural_cases};
use codec::structural_record_codec::{decode_structural_fields, encode_structural_fields};

use codec::structural_scalar_codec::{
    decode_identity, decode_multiplicity, encode_identity, multiplicity_tag,
};
use codec::structural_type_codec::{decode_structural_types, encode_structural_types};
use codec::wire_codec::{Reader, decode_boolean, push_u16, push_u32, push_u64, push_u128};
use unit_dynamic_descriptor_join::validate_installed_unit_dynamic_descriptor_joins;

/// The current vocabulary includes owned incoming stack pointers and AArch64's
/// dedicated indirect-result register. Earlier envelopes cannot carry those roles.
/// Marker 99 retains the by-value opaque boundary application's strong
/// selected-application custody so install replay compares it with the bound
/// artifact's coverage. Marker 98 carries reference-bearing structural
/// metadata: referent path segments, reference structural type shapes, and
/// result source rosters. Marker 97 seals complete placed-region inventory
/// identities for the bound image; marker 96 records carry no complete-custody
/// digests.
pub const INSTALLATION_FORMAT_MARKER: u16 = 99;

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
