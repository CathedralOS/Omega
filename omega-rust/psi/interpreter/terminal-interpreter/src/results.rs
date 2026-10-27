//! What an execution reports: its result, status, crash site and the fuel
//! measure around it.

use crate::errors::TerminalInterpretError;
use crate::scalar_array::TerminalScalarArrayResult;
use crate::values::{TerminalScalarAggregateValue, TerminalScalarValue, TerminalStructuralValue};
use semantic_vocabulary::{BlockId, BoundaryMachineId, ClaimId, MachineId, OperationId};
use terminal_fuel::{FuelExhaustion, FuelMeterError};
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
    storage: std::sync::Arc<ScalarCaseStorage>,
}

struct ScalarCaseStorage {
    value: TerminalScalarAggregateValue,
    _lease: Option<evaluation_accounting::EvaluationStorageLease>,
}

impl TerminalScalarCaseResult {
    /// Result inspection shares one immutable payload and its live reservation.
    pub fn value(&self) -> &TerminalScalarAggregateValue {
        &self.storage.value
    }

    pub(crate) fn collect(
        declaration: &terminal_psi::StructuralTypeDeclaration,
        case: semantic_vocabulary::StructuralCaseId,
        count: usize,
        fields: impl IntoIterator<
            Item = Result<
                (semantic_vocabulary::StructuralFieldId, TerminalScalarValue),
                TerminalInterpretError,
            >,
        >,
        account: Option<&evaluation_accounting::EvaluationInvocationAccount>,
    ) -> Result<Self, TerminalInterpretError> {
        let cells = u64::try_from(count).map_err(|_| crate::errors::malformed())?;
        let lease = account
            .map(|account| account.reserve_storage(cells, 0))
            .transpose()
            .map_err(TerminalInterpretError::Storage)?;
        let mut payload = Vec::new();
        let requested_bytes = count
            .checked_mul(std::mem::size_of::<(
                Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
                TerminalScalarValue,
            )>())
            .ok_or_else(crate::errors::malformed)?;
        payload.try_reserve_exact(count).map_err(|reason| {
            TerminalInterpretError::StorageAllocationFailed {
                requested_bytes,
                reason: reason.to_string(),
            }
        })?;
        let mut fields = fields.into_iter();
        for _ in 0..count {
            let (field, scalar) = fields.next().ok_or_else(crate::errors::malformed)??;
            payload.push((
                vec![semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                    field,
                )],
                scalar,
            ));
        }
        if fields.next().is_some() {
            return Err(crate::errors::malformed());
        }
        let value = TerminalScalarAggregateValue {
            structural_type: declaration.id,
            result_case: Some(case),
            fields: payload,
        };
        value.validate_against(declaration, |_| None)?;
        Ok(Self {
            storage: std::sync::Arc::new(ScalarCaseStorage {
                value,
                _lease: lease,
            }),
        })
    }
}

impl std::fmt::Debug for ScalarCaseStorage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value.fmt(formatter)
    }
}
impl PartialEq for ScalarCaseStorage {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}
impl Eq for ScalarCaseStorage {}

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
    /// The checked call retains its operands and loans; no completion or
    /// writeback has occurred. Readiness is independent of scheduling fuel.
    AwaitingEffect(TerminalEffectWait),
    /// Executor-local pause for a synchronous host result, not a guest task
    /// suspension or permission to release the invocation's live storage.
    HostComputationPending(TerminalEffectWait),
    Crashed(TerminalCrash),
    ExternalCompletion(TerminalExternalCompletion),
}

/// Exact code-local wait site. The caller retains the owning instance; a site
/// is an observation, not authority to install a provider or complete a call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalEffectWait {
    pub program: terminal_psi::TerminalPsiIdentity,
    pub machine: MachineId,
    pub block: BlockId,
    pub operation: OperationId,
    pub boundary: BoundaryMachineId,
}

/// Domain-ending abandonment, not a normal value or discharge receipt.
/// Preceding effects remain on the execution; the frontier is machine-local,
/// not a census of suspended frames or the containing domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalExternalCompletion {
    pub machine: MachineId,
    pub block: BlockId,
    pub edge: semantic_vocabulary::EdgeId,
    pub boundary: BoundaryMachineId,
    pub kind: semantic_vocabulary::ExternalCompletionKind,
    pub return_code: i32,
    pub frontier_lower_bound: Vec<ClaimId>,
}

/// The explicit terminal-Psi crash outcome reached by an execution.
///
/// `frontier_lower_bound` is the artifact-retained frontier for an authored edge
/// or the interpreter's current machine-local live claims for a boundary or
/// Trapping-operation crash.
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
    /// A Trapping primitive's own operation-level site: no edge or boundary
    /// identity is fabricated for it.
    Operation {
        machine: MachineId,
        block: BlockId,
        operation: OperationId,
    },
    BoundaryCall {
        machine: MachineId,
        block: BlockId,
        operation: OperationId,
        boundary: BoundaryMachineId,
    },
}
