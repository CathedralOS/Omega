//! Execution-role and completion evidence, separate from admission authority.

use omega_target_operations::{
    BoundaryExecutionBinding, CompilerBuiltinExecution, CompletionClaimSource,
    ProviderExecutionBinding,
};
use psi_terminal::CompletionReceipt;

/// Non-authoritative artifact custody for one exact completion receipt.
///
/// The row deliberately repeats the exact source and provider-execution
/// records instead of replacing either with a producer-authored aggregate or
/// authorization fingerprint. Object and installation validation rederive the
/// complete ordered catalog from the enclosing settlement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionProviderCustodyBinding {
    pub source: CompletionClaimSource,
    pub receipt: CompletionReceipt,
    pub provider_execution: ProviderExecutionRecord,
}

/// Source-free physical custody for one realized boundary execution role.
/// Compiler builtins remain structural catalog identities and never acquire
/// provider-execution report coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryExecutionRecord {
    AdmittedProvider(ProviderExecutionRecord),
    CompilerBuiltin(CompilerBuiltinExecution),
}

impl From<BoundaryExecutionBinding> for BoundaryExecutionRecord {
    fn from(binding: BoundaryExecutionBinding) -> Self {
        match binding {
            BoundaryExecutionBinding::AdmittedProvider(execution) => {
                Self::AdmittedProvider(execution.into())
            }
            BoundaryExecutionBinding::CompilerBuiltin(execution) => {
                Self::CompilerBuiltin(execution)
            }
        }
    }
}

pub fn derive_completion_provider_custody(
    execution: BoundaryExecutionRecord,
    sources: &[CompletionClaimSource],
    receipts: &[CompletionReceipt],
) -> Option<Vec<CompletionProviderCustodyBinding>> {
    let BoundaryExecutionRecord::AdmittedProvider(provider_execution) = execution else {
        return (sources.is_empty() && receipts.is_empty()).then(Vec::new);
    };
    receipts
        .iter()
        .map(|receipt| {
            let source = sources
                .iter()
                .find(|source| source.claim() == receipt.claim)?;
            Some(CompletionProviderCustodyBinding {
                source: source.clone(),
                receipt: *receipt,
                provider_execution,
            })
        })
        .collect()
}

/// Non-authoritative serialized projection of an admitted provider execution.
/// This can be decoded for validation/reporting but cannot be used to invoke
/// target lowering, which requires the ledger-owned admitted binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderExecutionRecord {
    pub provider_plan_report_identity: u64,
    pub provider_execution_report_identity: u64,
    pub provider_execution_report_fingerprint: u64,
    pub normalized_root_report_identity: u64,
    pub boundary_contract_report_fingerprint: u64,
}

impl ProviderExecutionRecord {
    pub fn new(
        provider_plan_report_identity: u64,
        provider_execution_report_identity: u64,
        provider_execution_report_fingerprint: u64,
        normalized_root_report_identity: u64,
        boundary_contract_report_fingerprint: u64,
    ) -> Option<Self> {
        [
            provider_plan_report_identity,
            provider_execution_report_identity,
            provider_execution_report_fingerprint,
            normalized_root_report_identity,
            boundary_contract_report_fingerprint,
        ]
        .iter()
        .all(|identity| *identity != 0)
        .then_some(Self {
            provider_plan_report_identity,
            provider_execution_report_identity,
            provider_execution_report_fingerprint,
            normalized_root_report_identity,
            boundary_contract_report_fingerprint,
        })
    }
}

impl From<ProviderExecutionBinding> for ProviderExecutionRecord {
    fn from(binding: ProviderExecutionBinding) -> Self {
        Self {
            provider_plan_report_identity: binding.provider_plan_report_identity().get(),
            provider_execution_report_identity: binding.provider_execution_report_identity(),
            provider_execution_report_fingerprint: binding.provider_execution_report_fingerprint(),
            normalized_root_report_identity: binding.normalized_root_report_identity(),
            boundary_contract_report_fingerprint: binding.boundary_contract_report_fingerprint(),
        }
    }
}
