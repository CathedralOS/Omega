//! Execute one admitted call without acquiring or compiling source.
//!
//! The caller supplies accepted immutable code, its sealed selection admission,
//! arguments and already admitted target supplies. Preparation and execution have
//! separate ownership: this package cannot depend on a source representation or
//! a compiler producer. Sharing code never shares an execution or its work account.
#![forbid(unsafe_code)]

pub mod values;
pub use build_time_values::BuildTimeValue;

use evaluation_accounting::EvaluationInvocationAccount;
use std::sync::Arc;
use terminal_codec::TerminalExecutionView;
use terminal_installation::{AdmittedCompilerEvaluation, AdmittedScalarApplications};
use terminal_interpreter::{
    TerminalExecutionResult, TerminalExecutionStatus, TerminalInstantiation, TerminalProgram,
    TerminalRuntimeSupplies,
};
use terminal_work::{WorkMeter, WorkUsage};

/// Values and observations offered for one call. `bind` checks these against
/// the independently admitted code and supplies before sealing the invocation.
pub struct InvocationInput<'input> {
    pub code: TerminalExecutionView<'input>,
    pub admission: &'input AdmittedCompilerEvaluation,
    pub target: Option<&'input str>,
    pub layout_target: Option<&'input terminal_installation::LayoutTargetFacts>,
    pub arguments: &'input Arc<[BuildTimeValue]>,
    pub inputs: &'input Arc<[BuildTimeValue]>,
    pub supplies: &'input TerminalRuntimeSupplies,
    pub scalars: Option<&'input AdmittedScalarApplications>,
}

/// One invocation, including its actual account. Execution accepts no replacement
/// inputs or meter and consumes the binding, so it cannot quietly replay a call.
pub struct ArtifactInvocation<'input> {
    input: InvocationInput<'input>,
    meter: &'input mut WorkMeter,
}

#[derive(Debug, Clone)]
pub struct InvocationReceipt {
    code: terminal_codec::TerminalExecutionIdentity,
    requester: semantic_vocabulary::compiler_evaluation::DeclarationCustody,
    entry: terminal_psi::TargetMachineCallable,
    target: Option<String>,
    arguments: Arc<[BuildTimeValue]>,
    inputs: Arc<[BuildTimeValue]>,
    account: Option<EvaluationInvocationAccount>,
    usage: WorkUsage,
}

impl InvocationReceipt {
    pub fn code(&self) -> &terminal_codec::TerminalExecutionIdentity {
        &self.code
    }
    pub fn require_inputs(
        &self,
        arguments: &Arc<[BuildTimeValue]>,
        inputs: &Arc<[BuildTimeValue]>,
        account: Option<&EvaluationInvocationAccount>,
    ) -> Result<(), String> {
        if !Arc::ptr_eq(&self.arguments, arguments)
            || !Arc::ptr_eq(&self.inputs, inputs)
            || !self.same_account(account)
        {
            return Err("invocation receipt changed its bound arguments, inputs or account".into());
        }
        Ok(())
    }
    pub fn arguments(&self) -> &[BuildTimeValue] {
        &self.arguments
    }
    pub fn usage(&self) -> &WorkUsage {
        &self.usage
    }
    pub fn same_account(&self, account: Option<&EvaluationInvocationAccount>) -> bool {
        match (self.account.as_ref(), account) {
            (Some(expected), Some(actual)) => expected.same_invocation(actual),
            (None, None) => true,
            _ => false,
        }
    }
    pub fn require_admission(
        &self,
        code: TerminalExecutionView<'_>,
        admission: &AdmittedCompilerEvaluation,
    ) -> Result<(), String> {
        admission.require_invocation(code, self.target.as_deref(), self.entry)?;
        if self.code != code.custody_identity() || self.requester != admission.requester() {
            return Err("invocation receipt changed its admitted caller or code".into());
        }
        Ok(())
    }
}

/// A failed execution returns its actual charges and the same bound invocation.
pub struct InvocationOutcome {
    pub value: Result<BuildTimeValue, String>,
    pub receipt: InvocationReceipt,
}

impl<'input> ArtifactInvocation<'input> {
    pub fn bind(
        input: InvocationInput<'input>,
        meter: &'input mut WorkMeter,
    ) -> Result<Self, String> {
        let entry = input
            .code
            .module()
            .entry
            .ok_or("source invocation lost its emitted root")?;
        input
            .admission
            .require_invocation(input.code, input.target, entry)?;
        input
            .supplies
            .require_compiler_evaluation(input.code, input.admission, input.scalars)?;
        if input.target.is_none() && input.layout_target.is_some() {
            return Err("target-independent evaluation cannot carry target observations".into());
        }
        for argument in input.inputs.iter() {
            if let BuildTimeValue::LayoutTarget(facts) = argument
                && (input.target != Some(facts.profile.as_str())
                    || input.layout_target != Some(facts))
            {
                return Err(
                    "issued layout context differs from the actual invocation target".into(),
                );
            }
        }
        Ok(Self { input, meter })
    }

    pub fn execute(mut self) -> InvocationOutcome {
        let value = self.run();
        let input = &self.input;
        // bind established the entry; execution cannot change this immutable code.
        let entry = input.code.module().entry.expect("bound invocation entry");
        let receipt = InvocationReceipt {
            code: input.code.custody_identity(),
            requester: input.admission.requester(),
            entry,
            target: input.target.map(str::to_owned),
            arguments: Arc::clone(input.arguments),
            inputs: Arc::clone(input.inputs),
            account: self.meter.invocation_account().cloned(),
            usage: self.meter.usage().clone(),
        };
        InvocationOutcome { value, receipt }
    }

    fn run(&mut self) -> Result<BuildTimeValue, String> {
        let view = self.input.code;
        let meter = &mut *self.meter;
        let program = TerminalProgram::borrow(view);
        let entry = program
            .entry()
            .map_err(|error| format!("source invocation entry: {error:?}"))?;
        if !entry.scalar_parameters.is_empty()
            || entry.structural_parameters.len() != self.input.inputs.len()
        {
            return Err("source invocation retained unmatched host input parameters".into());
        }
        let inputs = entry
            .structural_parameters
            .iter()
            .enumerate()
            .map(|(position, parameter)| {
                let declaration = program
                    .module()
                    .structural_type(parameter.structural_type)
                    .ok_or("source input lost its canonical declaration")?;
                let issued = matches!(
                    self.input.inputs[position],
                    crate::BuildTimeValue::LayoutTarget(_)
                );
                if (issued
                    && (declaration.shape != terminal_psi::StructuralTypeShape::Opaque
                        || parameter.access != terminal_psi::StructuralAccess::Owned))
                    || !matches!(
                        parameter.access,
                        terminal_psi::StructuralAccess::Owned
                            | terminal_psi::StructuralAccess::SharedBorrow
                    )
                    || !parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty()
                    || !parameter.additional_input_qualifications.is_empty()
                {
                    return Err("source input changed its checked input contract");
                }
                let identity = u64::try_from(position)
                    .ok()
                    .and_then(|position| position.checked_add(1))
                    .ok_or("source input identity exceeds its host namespace")?;
                Ok(terminal_interpreter::TerminalStructuralValue::minted(
                    identity,
                    parameter.structural_type,
                    Vec::new(),
                ))
            })
            .collect::<Result<Vec<_>, &str>>()?;
        let contents = entry
            .structural_parameters
            .iter()
            .zip(self.input.inputs.iter())
            .enumerate()
            .filter(|(_, (_, value))| !matches!(value, crate::BuildTimeValue::LayoutTarget(_)))
            .map(|(position, (parameter, value))| {
                crate::values::input_contents(
                    &program,
                    u32::try_from(position)
                        .map_err(|_| "source input exceeds its argument namespace")?,
                    parameter.structural_type,
                    value,
                )
            })
            .collect::<Result<Vec<_>, String>>()?;
        let opaque_inputs = inputs
            .iter()
            .zip(self.input.inputs.iter())
            .collect::<Vec<_>>();
        let result_type = entry
            .result
            .structural()
            .map(|result| result.structural_type);
        let account = meter.invocation_account().cloned();
        let mut instance = program
            .instantiate(TerminalInstantiation {
                account: account.as_ref(),
                installation: Some(self.input.supplies),
                scalar_admission: self.input.scalars,
                structural_inputs: terminal_interpreter::TerminalStructuralInputs {
                    arguments: &inputs,
                    contents: &contents,
                    ..Default::default()
                },
                ..Default::default()
            })
            .map_err(|error| format!("source invocation preparation: {error:?}"))?;
        let status = instance
            .invoke(meter)
            .map_err(|error| format!("source invocation execution: {error:?}"))?;
        match status {
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => {
                Ok(crate::BuildTimeValue::Unit)
            }
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(value)) => {
                crate::values::scalar_result(instance.program(), value)
            }
            TerminalExecutionStatus::Complete(
                TerminalExecutionResult::Structural(_)
                | TerminalExecutionResult::ScalarCase(_)
                | TerminalExecutionResult::ScalarArray(_),
            ) => crate::values::result_value(
                &instance,
                result_type.ok_or("source invocation lost its structured result type")?,
                &opaque_inputs,
            ),
            other => Err(format!(
                "source invocation did not complete normally: {other:?}"
            )),
        }
    }
}

pub mod scalar_applications;
