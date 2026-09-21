//! What an execution reports: its result, status, crash site and the fuel
//! measure around it.

use crate::effects::TerminalEffect;
use crate::errors::TerminalInterpretError;
use crate::scalar_array::TerminalScalarArrayResult;
use crate::values::{
    TerminalScalarCaseValue, TerminalScalarValue, TerminalStructuralPrimitiveValue,
    TerminalStructuralValue,
};
use semantic_vocabulary::{BlockId, BoundaryMachineId, ClaimId, MachineId, OperationId};
use terminal_fuel::{FuelExhaustion, FuelMeterError, TerminalFuelUsage};
use terminal_psi::CrashCause;

/// The normal result of terminal-Psi execution.
///
/// Unit is a successful absence of a value, not a distinguished scalar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalExecutionResult {
    Unit,
    Scalar(TerminalScalarValue),
    Structural(TerminalStructuralResult),
    ScalarCase(TerminalScalarCaseResult),
    ScalarArray(TerminalScalarArrayResult),
}

/// A structural value returned with the exact live claims transferred into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalStructuralResult {
    pub value: TerminalStructuralValue,
    pub claims: Vec<ClaimId>,
}

/// A returned selected case whose primitive payload carries no structural claims.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalScalarCaseResult {
    pub value: TerminalScalarCaseValue,
}

pub(crate) fn meter_status(
    error: FuelMeterError,
) -> Result<TerminalExecutionStatus, TerminalInterpretError> {
    match error {
        FuelMeterError::Exhausted(exhaustion) => {
            Ok(TerminalExecutionStatus::SponsorExhausted(exhaustion))
        }
        other => Err(TerminalInterpretError::Fuel(other)),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalExecutionStatus {
    Complete(TerminalExecutionResult),
    SponsorExhausted(FuelExhaustion),
    Crashed(TerminalCrash),
}

/// The explicit terminal-Psi crash outcome reached by an execution.
///
/// `frontier_lower_bound` is the artifact-retained frontier for an authored edge
/// or the interpreter's current machine-local live claims for a boundary crash.
/// Neither asserts that no suspended caller or wider runtime state was abandoned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCrash {
    pub site: TerminalCrashSite,
    pub cause: CrashCause,
    /// Static guard on an authored crash edge. Boundary invocations instead
    /// validate declaration-local routes against their observed effect inputs;
    /// they do not invent a local edge guard.
    pub site_guard: Vec<terminal_psi::CrashPredicateTerm>,
    pub frontier_lower_bound: Vec<ClaimId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalCrashSite {
    Edge(semantic_vocabulary::EdgeId),
    BoundaryCall {
        machine: MachineId,
        block: BlockId,
        operation: OperationId,
        boundary: BoundaryMachineId,
    },
}

/// A successful semantic result paired with deterministic terminal-Psi fuel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasuredTerminalExecution {
    pub(crate) value: TerminalExecutionResult,
    pub(crate) usage: TerminalFuelUsage,
    pub(crate) effects: Vec<TerminalEffect>,
    pub(crate) structural_primitive_values: Vec<TerminalStructuralPrimitiveValue>,
}

impl MeasuredTerminalExecution {
    pub fn value(&self) -> TerminalExecutionResult {
        self.value.clone()
    }

    pub const fn usage(&self) -> &TerminalFuelUsage {
        &self.usage
    }

    pub fn effects(&self) -> &[TerminalEffect] {
        &self.effects
    }

    pub fn structural_primitive_values(&self) -> &[TerminalStructuralPrimitiveValue] {
        &self.structural_primitive_values
    }

    pub fn into_value(self) -> TerminalExecutionResult {
        self.value
    }

    pub fn into_parts(self) -> (TerminalExecutionResult, TerminalFuelUsage) {
        (self.value, self.usage)
    }
}
