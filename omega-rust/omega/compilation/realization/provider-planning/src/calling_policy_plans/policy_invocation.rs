//! The source-free request one calling-policy activation runs on.
//!
//! Provider planning forms the request; the evaluator a `CallingProgram`
//! carries executes it. Build evaluation supplies that evaluator, so this crate
//! never enters the Terminal interpreter.

use super::CallingPolicySignature;
use calling_conventions::ValidatedBoundaryEntryPlan;
use evaluation_accounting::EvaluationInvocationAccount;
use psi_work::WorkUsage;
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::TargetMachineCallable;

/// One activation's complete pre-effect admission. Receiving admission
/// (including whole-map synchronous cycles) precedes the compiler-evaluation
/// floor. Neither record can be supplied from Source.
pub type TerminalCallingPolicyAdmission = terminal_installation::AdmittedCompilerEvaluation;

pub fn admit_terminal_calling_policy(
    artifact: &CanonicalTerminalArtifact,
    declaration: terminal_psi::ProofDeclarationId,
    target: target::TargetProfile,
    providers: &[backend_plan::TerminalTargetProviderPlan],
    requester: semantic_vocabulary::compiler_evaluation::DeclarationCustody,
    authority: Option<&dyn semantic_vocabulary::compiler_evaluation::DeclarationSelectionAuthority>,
) -> Result<TerminalCallingPolicyAdmission, String> {
    let providers = backend_plan::TerminalTargetProviderSelection::from_plans(
        providers.to_vec(),
        artifact.view(),
    )?
    .admit(artifact.view(), target.target_name())?;
    terminal_installation::admit_compiler_evaluation(
        artifact.view(),
        target.target_name(),
        declaration,
        providers,
        requester,
        authority,
    )
}

/// Explicit source-free inputs of a policy activation. The single-use canonical
/// invocation admission precedes this request; neither entry selection nor
/// successful interpretation grants provider, callback or native authority.
pub struct TerminalCallingPolicyInvocation<'input> {
    pub artifact: &'input CanonicalTerminalArtifact,
    pub entry: TargetMachineCallable,
    pub target: target::TargetProfile,
    pub signature: &'input CallingPolicySignature,
    pub account: EvaluationInvocationAccount,
    pub admission: TerminalCallingPolicyAdmission,
}

/// Execute the selected callable on the supplied signature, decode its actual
/// returned plan, and validate every placement against that same signature.
pub type CallingPolicyEvaluator = fn(
    TerminalCallingPolicyInvocation<'_>,
) -> Result<(ValidatedBoundaryEntryPlan, WorkUsage), String>;
