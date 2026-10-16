//! The provider-settlement vocabulary a realization supplies and the narrow
//! request provider admission reads.

use installation_evidence::ProviderExecutionEvidence;
use target_operations::BoundaryRealization;

use super::{TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy};

#[derive(Debug, Clone, Copy)]
pub enum NativeBoundaryRealization<'execution> {
    Builtin(BoundaryRealization),
    NormalizedForeignCall(&'execution task_plans::AdmittedSameStackContribution),
    /// A direct kernel entry has no foreign callee on the receiver stack.
    /// Admission must still rejoin its number, target and evaluated call plan.
    SystemCall,
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
/// `boundary` is the Terminal boundary derivation bound the row to;
/// `settlement` is the execution derivation closed for it, or the reason it
/// could not, which admission refuses only when this entry's scope executes
/// the boundary.
#[derive(Debug, Clone, Copy)]
pub struct NativeCompilerBuiltinSettlement<'execution> {
    pub requirement_identity: &'execution str,
    pub provider_plan: &'execution effects::provider_plan::ProviderPlan,
    pub boundary: semantic_vocabulary::BoundaryMachineId,
    pub settlement: crate::provider_admission::TerminalCompilerBuiltinSettlement,
}

/// The inputs provider admission reads from one target realization: the
/// target profile, selected plans, external bindings, supplied
/// settlements and builtins, callback arguments and the receiving
/// terminal-authority policies.
// The request borrows invocation-local selections; executions keep their
// provider-owned lifetime when those selections are projected into a roster.
#[derive(Clone, Copy)]
pub struct ProviderAdmissionRequest<'request, 'execution> {
    /// Execution scope and physical geometry come from one profile. Provider
    /// admission needs no application-entry authority: private callback bodies
    /// consume the same operation without impersonating a ProgramEntry.
    pub profile: target::TargetProfile,
    pub selected_provider_plans: &'request effects::SelectedProviderPlanFacts,
    pub checked_adapters: &'request [crate::SelectedProviderAdapter],
    pub external_binding_rows: &'request [calling_conventions::ExternalBindingRow],
    pub settlements: &'request [NativeProviderSettlement<'execution>],
    pub compiler_builtins: &'request [NativeCompilerBuiltinSettlement<'request>],
    pub native_callbacks: &'request [crate::NativeCallbackArgumentProposal],
    pub terminal_authority_policy: &'request TerminalAuthorityPolicy,
    pub terminal_authority_permission_policy: &'request Option<TerminalAuthorityPermissionPolicy>,
    /// Build-admitted AVX+FMA3 provider for this exact profile, if any.
    pub x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
}

impl ProviderAdmissionRequest<'_, '_> {
    pub fn target(&self) -> target::NativeTarget {
        self.profile.native_target()
    }
}
