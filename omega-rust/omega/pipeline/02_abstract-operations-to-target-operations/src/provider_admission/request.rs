//! The provider-settlement vocabulary a realization supplies and the narrow
//! request provider admission reads.

use installation_evidence::ProviderExecutionEvidence;
use target_operations::BoundaryRealization;

use super::{TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy};

#[derive(Debug, Clone, Copy)]
pub enum NativeBoundaryRealization<'execution> {
    Builtin(BoundaryRealization),
    NormalizedForeignCall(&'execution task_plans::AdmittedSameStackContribution),
}

impl<'execution> From<BoundaryRealization> for NativeBoundaryRealization<'execution> {
    fn from(realization: BoundaryRealization) -> Self {
        Self::Builtin(realization)
    }
}

macro_rules! builtin_native_realization_conversion {
    ($realization:ty) => {
        impl<'execution> From<$realization> for NativeBoundaryRealization<'execution> {
            fn from(realization: $realization) -> Self {
                Self::Builtin(realization.into())
            }
        }
    };
}

builtin_native_realization_conversion!(target_operations::MetadataOnlyPortRealization);
builtin_native_realization_conversion!(target_operations::DirectPortReadU8Realization);
builtin_native_realization_conversion!(target_operations::LinuxWriteLineRealization);
builtin_native_realization_conversion!(target_operations::HostedExitProcessI32Realization);
builtin_native_realization_conversion!(target_operations::HostedReadByteRealization);
builtin_native_realization_conversion!(target_operations::ClaimCompletionOnlyRealization);

/// Provider-supplied realization input for one Terminal boundary. The exact
/// requirement comes from admitted execution evidence rather than a caller-
/// authored numeric boundary ID.
#[derive(Debug, Clone, Copy)]
pub struct NativeProviderSettlement<'execution> {
    pub provider_execution: &'execution dyn ProviderExecutionEvidence,
    /// Complete selected-plan evidence. The compact report identity remains a
    /// report coordinate and cannot select or authorize a plan by itself.
    pub provider_plan: &'execution effects::provider_plan::ProviderPlan,
    pub realization: NativeBoundaryRealization<'execution>,
}

/// Target-constrained compiler-builtin proposal consumed by the local native
/// lowerer. This carries no provider execution or installation receipt.
#[derive(Debug, Clone, Copy)]
pub struct NativeCompilerBuiltinSettlement<'execution> {
    pub requirement_identity: &'execution str,
    pub provider_plan: &'execution effects::provider_plan::ProviderPlan,
    pub execution: target_operations::CompilerBuiltinExecution,
}

/// The inputs provider admission reads from one target realization: the
/// target, entry settlement, selected plans, external bindings, supplied
/// settlements and builtins, callback arguments and the receiving
/// terminal-authority policies.
pub struct ProviderAdmissionRequest<'request> {
    pub target: target::NativeTarget,
    pub profile: &'request proof_admission::AdmissionProfile,
    pub program_entry: terminal_psi_to_abstract_operations::NativeProgramEntrySettlement<'request>,
    pub selected_provider_plans: &'request effects::SelectedProviderPlanFacts,
    pub external_binding_rows: &'request [calling_conventions::ExternalBindingRow],
    pub settlements: &'request [NativeProviderSettlement<'request>],
    pub compiler_builtins: &'request [NativeCompilerBuiltinSettlement<'request>],
    pub native_callbacks: &'request [crate::AdmittedNativeCallbackArgument],
    pub terminal_authority_policy: &'request TerminalAuthorityPolicy,
    pub terminal_authority_permission_policy: &'request Option<TerminalAuthorityPermissionPolicy>,
}
