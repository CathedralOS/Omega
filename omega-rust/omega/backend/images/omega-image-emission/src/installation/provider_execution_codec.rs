//! Canonical format-36 codec for admitted provider-execution evidence.
//!
//! Both enclosing settlements and nested completion-custody rows use this
//! exact five-identity grammar. Admission and closure validation remain in the
//! installation parent.

use omega_machine_code::ProviderExecutionRecord;

use super::{InstallationError, Reader, push_u64};

pub(super) fn encode_provider_execution(bytes: &mut Vec<u8>, execution: ProviderExecutionRecord) {
    push_u64(bytes, execution.provider_plan_report_identity);
    push_u64(bytes, execution.provider_execution_report_identity);
    push_u64(bytes, execution.provider_execution_report_fingerprint);
    push_u64(bytes, execution.normalized_root_report_identity);
    push_u64(bytes, execution.boundary_contract_report_fingerprint);
}

pub(super) fn decode_provider_execution(
    reader: &mut Reader<'_>,
) -> Result<ProviderExecutionRecord, InstallationError> {
    ProviderExecutionRecord::new(
        reader.u64()?,
        reader.u64()?,
        reader.u64()?,
        reader.u64()?,
        reader.u64()?,
    )
    .ok_or(InstallationError::ZeroProviderExecutionEvidence)
}
