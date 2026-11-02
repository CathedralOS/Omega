//! The source-free request one calling-policy activation runs on.
//!
//! Provider planning forms the request; the evaluator a `CallingProgram`
//! carries executes it. Build evaluation supplies that evaluator, so this crate
//! never enters the Terminal interpreter.

use super::CallingPolicySignature;
use calling_conventions::ValidatedBoundaryEntryPlan;
use evaluation_accounting::EvaluationInvocationAccount;
use terminal_codec::CanonicalTerminalArtifact;
use terminal_fuel::TerminalFuelUsage;
use terminal_psi::TargetMachineCallable;

/// Explicit source-free inputs of a policy activation. The producer's checked
/// invocation admission precedes this request; neither entry selection nor
/// successful interpretation grants provider, callback or native authority.
pub struct TerminalCallingPolicyInvocation<'input> {
    pub artifact: &'input CanonicalTerminalArtifact,
    pub entry: TargetMachineCallable,
    pub target: target::TargetProfile,
    pub signature: &'input CallingPolicySignature,
    pub account: EvaluationInvocationAccount,
    pub providers: &'input [backend_plan::TerminalTargetProviderPlan],
}

/// Execute the selected callable on the supplied signature, decode its actual
/// returned plan, and validate every placement against that same signature.
pub type CallingPolicyEvaluator =
    fn(
        TerminalCallingPolicyInvocation<'_>,
    ) -> Result<(ValidatedBoundaryEntryPlan, TerminalFuelUsage), String>;
