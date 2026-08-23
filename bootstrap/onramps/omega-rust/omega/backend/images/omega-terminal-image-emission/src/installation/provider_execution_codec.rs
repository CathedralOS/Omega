//! Canonical format-34 codec for admitted provider-execution evidence.
//!
//! Both enclosing settlements and nested completion-custody rows use this
//! exact five-identity grammar. Admission and closure validation remain in the
//! installation parent.

use omega_terminal_machine_code::TerminalProviderExecutionRecord;

use super::{Reader, TerminalInstallationError, push_u64};

pub(super) fn encode_provider_execution(
    bytes: &mut Vec<u8>,
    execution: TerminalProviderExecutionRecord,
) {
    push_u64(bytes, execution.provider_plan);
    push_u64(bytes, execution.provider_execution_identity);
    push_u64(bytes, execution.provider_execution_fingerprint);
    push_u64(bytes, execution.normalized_root_identity);
    push_u64(bytes, execution.boundary_contract_fingerprint);
}

pub(super) fn decode_provider_execution(
    reader: &mut Reader<'_>,
) -> Result<TerminalProviderExecutionRecord, TerminalInstallationError> {
    TerminalProviderExecutionRecord::new(
        reader.u64()?,
        reader.u64()?,
        reader.u64()?,
        reader.u64()?,
        reader.u64()?,
    )
    .ok_or(TerminalInstallationError::ZeroProviderExecutionEvidence)
}
