//! Encoding and decoding the installation record envelope.

use crate::installation_record::codec::boundary_settlement::{
    decode_boundary_settlements, encode_boundary_settlements,
};
use crate::installation_record::codec::dynamic_conformance::{
    decode_dynamic_conformance_custody, encode_dynamic_conformance_custody,
};
use crate::installation_record::codec::function::{decode_functions, encode_functions};
use crate::installation_record::codec::installation_header::{
    DecodedInstallationHeader, decode_installation_header, encode_installation_header,
};
use crate::installation_record::codec::internal_unit_call::{
    decode_internal_unit_calls, encode_internal_unit_calls,
};
use crate::installation_record::codec::internal_unit_scalar_call::{
    decode_internal_unit_scalar_calls, encode_internal_unit_scalar_calls,
};
use crate::installation_record::codec::opaque_application::{
    decode_boundary_opaque_applications, encode_boundary_opaque_applications,
};
use crate::installation_record::codec::port_effect::{decode_port_effects, encode_port_effects};
use crate::installation_record::codec::private_function::{
    decode_private_functions, encode_private_functions,
};
use crate::installation_record::codec::provider_plan::{
    decode_provider_plans, encode_provider_plans,
};
use crate::installation_record::codec::semantic_code_attribution::{
    decode_semantic_code_attributions, encode_semantic_code_attributions,
};
use crate::installation_record::codec::structural_return::{
    decode_structural_returns, encode_structural_returns,
};
use crate::installation_record::codec::wire::Reader;
use crate::installation_record::record_shape::validate_record_shape;
use crate::installation_record::{InstallationError, InstallationRecord};

pub(crate) const MAGIC: &[u8; 8] = b"PSIINST\0";

pub fn encode_installation_record(
    record: &InstallationRecord,
) -> Result<Vec<u8>, InstallationError> {
    validate_record_shape(record)?;
    let provider_count = u32::try_from(record.selected_provider_plans.len())
        .map_err(|_| InstallationError::TooManyProviderPlans)?;
    let settlement_count = u32::try_from(record.boundary_settlements.len())
        .map_err(|_| InstallationError::TooManyBoundarySettlements)?;
    let function_count = u32::try_from(record.functions.len())
        .map_err(|_| InstallationError::TooManyInstalledFunctions)?;
    let private_function_count = u32::try_from(record.private_functions.len())
        .map_err(|_| InstallationError::TooManyCompilerPrivateFunctions)?;
    let structural_return_count = u32::try_from(record.structural_returns.len())
        .map_err(|_| InstallationError::TooManyStructuralReturns)?;
    let internal_unit_call_count = u32::try_from(record.internal_unit_calls.len())
        .map_err(|_| InstallationError::TooManyInternalUnitCalls)?;
    let internal_unit_scalar_call_count = u32::try_from(record.internal_unit_scalar_calls.len())
        .map_err(|_| InstallationError::TooManyInternalUnitScalarCalls)?;
    let semantic_code_attribution_count = u32::try_from(record.semantic_code_attribution.len())
        .map_err(|_| InstallationError::TooManySemanticCodeAttributions)?;
    let port_effect_count = u32::try_from(record.port_effects.len())
        .map_err(|_| InstallationError::TooManyPortEffects)?;
    let text_relocation_count =
        u64::try_from(record.compiler_text_validation.text_relocation_count)
            .map_err(|_| InstallationError::CountNotRepresentable("text relocations"))?;
    let checked_instruction_validation_count = u64::try_from(
        record
            .compiler_text_validation
            .checked_instruction_validation_count,
    )
    .map_err(|_| InstallationError::CountNotRepresentable("checked instructions"))?;

    let mut bytes = Vec::with_capacity(294 + record.selected_provider_plans.len() * 8);
    encode_installation_header(
        &mut bytes,
        record,
        text_relocation_count,
        checked_instruction_validation_count,
    )?;
    encode_provider_plans(&mut bytes, provider_count, &record.selected_provider_plans);
    encode_functions(&mut bytes, function_count, &record.functions)?;
    encode_private_functions(
        &mut bytes,
        private_function_count,
        &record.private_functions,
    )?;
    encode_structural_returns(
        &mut bytes,
        structural_return_count,
        &record.structural_returns,
    )?;
    encode_internal_unit_calls(
        &mut bytes,
        internal_unit_call_count,
        &record.internal_unit_calls,
    )?;
    encode_internal_unit_scalar_calls(
        &mut bytes,
        internal_unit_scalar_call_count,
        &record.internal_unit_scalar_calls,
    )?;
    encode_dynamic_conformance_custody(
        &mut bytes,
        &record.dynamic_conformance_tables,
        &record.dynamic_calls,
        &record.stored_dynamic_calls,
        &record.forwarded_dynamic_descriptor_adapters,
        &record.forwarded_dynamic_descriptor_tables,
        &record.forwarded_dynamic_descriptor_calls,
        &record.dynamic_parameter_calls,
        &record.forwarded_dynamic_parameter_calls,
    )?;
    encode_semantic_code_attributions(
        &mut bytes,
        semantic_code_attribution_count,
        &record.semantic_code_attribution,
    )?;
    encode_port_effects(&mut bytes, port_effect_count, &record.port_effects)?;
    encode_boundary_settlements(&mut bytes, settlement_count, &record.boundary_settlements)?;
    encode_boundary_opaque_applications(&mut bytes, &record.boundary_opaque_applications);
    Ok(bytes)
}

pub fn decode_installation_record(bytes: &[u8]) -> Result<InstallationRecord, InstallationError> {
    let mut reader = Reader::new(bytes);
    let DecodedInstallationHeader {
        psi,
        target,
        subsystem,
        profile_decision,
        component_progress,
        image,
        image_sections,
        compiler_text_validation,
    } = decode_installation_header(&mut reader)?;
    let selected_provider_plans = decode_provider_plans(&mut reader)?;
    let functions = decode_functions(&mut reader)?;
    let private_functions = decode_private_functions(&mut reader)?;
    let structural_returns = decode_structural_returns(&mut reader)?;
    let internal_unit_calls = decode_internal_unit_calls(&mut reader)?;
    let internal_unit_scalar_calls = decode_internal_unit_scalar_calls(&mut reader)?;
    let (
        dynamic_conformance_tables,
        dynamic_calls,
        stored_dynamic_calls,
        forwarded_dynamic_descriptor_adapters,
        forwarded_dynamic_descriptor_tables,
        forwarded_dynamic_descriptor_calls,
        dynamic_parameter_calls,
        forwarded_dynamic_parameter_calls,
    ) = decode_dynamic_conformance_custody(&mut reader)?;
    let semantic_code_attribution = decode_semantic_code_attributions(&mut reader)?;
    let port_effects = decode_port_effects(&mut reader)?;
    let boundary_settlements = decode_boundary_settlements(&mut reader)?;
    let boundary_opaque_applications = decode_boundary_opaque_applications(&mut reader)?;
    if reader.remaining() != 0 {
        return Err(InstallationError::TrailingBytes(reader.remaining()));
    }

    let record = InstallationRecord {
        psi,
        target,
        subsystem,
        profile_decision,
        selected_provider_plans,
        component_progress,
        functions,
        private_functions,
        structural_returns,
        internal_unit_calls,
        internal_unit_scalar_calls,
        dynamic_conformance_tables,
        dynamic_calls,
        stored_dynamic_calls,
        forwarded_dynamic_descriptor_adapters,
        forwarded_dynamic_descriptor_tables,
        forwarded_dynamic_descriptor_calls,
        dynamic_parameter_calls,
        forwarded_dynamic_parameter_calls,
        semantic_code_attribution,
        port_effects,
        boundary_settlements,
        boundary_opaque_applications,
        image,
        image_sections,
        compiler_text_validation,
    };
    validate_record_shape(&record)?;
    if encode_installation_record(&record)? != bytes {
        return Err(InstallationError::NonCanonicalEncoding);
    }
    Ok(record)
}
