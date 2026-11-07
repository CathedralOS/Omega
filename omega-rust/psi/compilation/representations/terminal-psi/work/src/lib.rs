#![forbid(unsafe_code)]

//! Canonical logical-work schedule and meter for terminal Psi.
//!
//! Logical work is deterministic portable work, not native instructions,
//! cycles, energy, or elapsed time. The executing program cannot inspect this
//! meter; an interpreter, evaluator, or bounded compiler service owns it.
//! Native execution has no implicit logical-work meter.

use std::collections::BTreeMap;

use evaluation_accounting::{
    EvaluationAccountingError, EvaluationInvocationAccount, EvaluationMeasurementIdentity,
};
use semantic_vocabulary::{EdgeId, OperationId};
use terminal_psi::{Operation, OperationKind, Terminator};

pub use semantic_vocabulary::WorkScheduleIdentity;

/// The current schedule charges one unit for each executed semantic operation
/// and one unit for each taken terminal edge. Adding a vocabulary variant
/// forces an explicit update to these exhaustive matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorkSchedule {
    identity: WorkScheduleIdentity,
}

impl WorkSchedule {
    pub const CURRENT: Self = Self {
        identity: match WorkScheduleIdentity::new(1) {
            Some(identity) => identity,
            None => unreachable!(),
        },
    };

    pub const fn identity(self) -> WorkScheduleIdentity {
        self.identity
    }

    pub const fn operation_units(self, kind: &OperationKind) -> u64 {
        match kind {
            OperationKind::EstablishNativeCodeLayout { .. } | OperationKind::EstablishAccessReference { .. }
            | OperationKind::EstablishReference { .. }
            | OperationKind::ReleaseReference { .. }
            | OperationKind::EstablishScalarCase { .. }
            | OperationKind::EstablishScalarArray { .. }
            | OperationKind::EstablishActivationClaim { .. }
            | OperationKind::ReleaseActivationClaim { .. }
            | OperationKind::SplitActivationClaim { .. }
            | OperationKind::MergeActivationClaim { .. }
            | OperationKind::EstablishZeroedStructural { .. }
            | OperationKind::EstablishStructuralLocal { .. }
            | OperationKind::EstablishByteSequenceConcatenation { .. }
            | OperationKind::EstablishByteSequenceLiteral { .. }
            | OperationKind::ByteSequenceLength { .. }
            | OperationKind::ByteSequenceRead { .. }
            | OperationKind::ByteSequenceWrite { .. }
            | OperationKind::ByteSequenceSubslice { .. }
            | OperationKind::ByteSequencePredicate { .. }
            | OperationKind::ByteSequenceEqual { .. }
            | terminal_psi::element_view_establish_pair_pattern!(..)
            | OperationKind::ElementViewLength { .. }
            | OperationKind::ElementViewRead { .. }
            | OperationKind::ElementViewWrite { .. }
            | OperationKind::ElementViewSubslice { .. }
            | OperationKind::EstablishTrivialAffineLocal { .. }
            | OperationKind::EstablishStructuralArray { .. }
            | OperationKind::EstablishRecord { .. }
            | OperationKind::EstablishStructuralCase { .. }
            | OperationKind::Call { .. }
            | OperationKind::CallUnit { .. }
            | OperationKind::CallStructuralScalar { .. }
            | terminal_psi::call_structural_pair_pattern!(..)
            | OperationKind::SelectImplementation { .. }
            | OperationKind::CallDynamic { .. }
            | OperationKind::WriteOnlyPrimitiveStore { .. }
            | OperationKind::EstablishPrimitiveLocal { .. }
            | OperationKind::PrimitiveScalarRead { .. }
            | OperationKind::AtomicAccess { .. }
            | OperationKind::AtomicFence { .. }
            | OperationKind::StructuralScalarFieldStore { .. }
            | terminal_psi::move_or_store_structural_field_pattern!()
            | OperationKind::MoveStructuralValue { .. }
            | OperationKind::TransformContent { .. }
            | OperationKind::ByteSequenceStore { .. }
            | OperationKind::StructuralByteSequenceFieldStore { .. }
            | OperationKind::StructuralByteSequenceFieldLength { .. }
            | OperationKind::StructuralByteSequenceFieldByteStore { .. }
            | OperationKind::StructuralByteSequenceFieldRead { .. }
            | OperationKind::StructuralByteViewFieldStore { .. }
            | OperationKind::BoundaryCall { .. }
            | OperationKind::PortWrite { .. }
            | OperationKind::RegisterAccess { .. }
            | OperationKind::IntegerConstant { .. }
            | OperationKind::ReadScalarApplication { .. }
            | OperationKind::IeeeFloatConstant { .. }
            | OperationKind::NearestIeeeFloatFusedMultiplyAdd { .. }
            | OperationKind::IeeeFloatCompare { .. }
            | OperationKind::ScalarRepresentationView { .. }
            | OperationKind::BooleanConstant { .. }
            | OperationKind::BooleanStructuralField { .. }
            | OperationKind::StructuralCaseMembership { .. }
            | terminal_psi::structural_leaf_copy_pair_pattern!()
            | OperationKind::ScalarStructuralField { .. }
            | OperationKind::BooleanNot { .. }
            | OperationKind::BooleanEqual { .. }
            | OperationKind::IntegerEqual { .. }
            | terminal_psi::integer_less_or_equal_pair_pattern!(..)
            | OperationKind::IntegerBitwiseNot { .. }
            | OperationKind::IntegerWiden { .. }
            | OperationKind::IntegerExactCast { .. }
            | OperationKind::IntegerBitwiseAnd { .. }
            | OperationKind::IntegerBitwiseOr { .. }
            | OperationKind::IntegerBitwiseXor { .. }
            | OperationKind::WrappingIntegerShiftLeft { .. }
            | OperationKind::WrappingIntegerShiftRight { .. }
            | OperationKind::ExactIntegerShiftLeft { .. }
            | OperationKind::ExactIntegerShiftRight { .. }
            | terminal_psi::integer_arithmetic_operations!()
            // A Trapping primitive charges its one unit before deciding
            // whether it traps; the trap outcome takes no edge.
            | OperationKind::TrappingInteger { .. }
            | OperationKind::FloatResultPolicy { .. }
            | OperationKind::StorageAddress { .. }
            | OperationKind::GlobalDescriptorTable { .. }
            | OperationKind::ActivateTaskRegister { .. }
            | OperationKind::ReadTaskRegister { .. }
            | OperationKind::ReloadSegmentDescriptors { .. }
            | OperationKind::ReadSegmentSelector { .. } => 1,
        }
    }

    pub const fn terminator_units(self, terminator: &Terminator) -> u64 {
        match terminator {
            Terminator::Jump { .. }
            | Terminator::Conditional { .. }
            | Terminator::StructuralCase { .. }
            | terminal_psi::terminator_return_pair_pattern!(..)
            | Terminator::ReturnStructural { .. }
            | Terminator::Crash { .. }
            | Terminator::InvokeNoNormal { .. }
            | Terminator::ExternalCompletion { .. } => 1,
        }
    }
}

/// Stable semantic site to which executed logical work is attributed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum WorkChargeSite {
    Operation(OperationId),
    Edge(EdgeId),
}

/// Aggregate execution count and logical units charged at one semantic site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, compiler_storage::Record)]
pub struct WorkAttribution {
    executions: u64,
    units: u64,
}

impl WorkAttribution {
    pub const fn executions(self) -> u64 {
        self.executions
    }

    pub const fn units(self) -> u64 {
        self.units
    }
}

/// Deterministic usage for one concrete terminal-Psi execution.
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct WorkUsage {
    schedule: WorkScheduleIdentity,
    total_units: u64,
    attribution: BTreeMap<WorkChargeSite, WorkAttribution>,
}

impl WorkUsage {
    fn empty(schedule: WorkScheduleIdentity) -> Self {
        Self {
            schedule,
            total_units: 0,
            attribution: BTreeMap::new(),
        }
    }

    pub const fn schedule(&self) -> WorkScheduleIdentity {
        self.schedule
    }

    pub const fn total_units(&self) -> u64 {
        self.total_units
    }

    pub fn attribution(&self) -> &BTreeMap<WorkChargeSite, WorkAttribution> {
        &self.attribution
    }

    pub fn at(&self, site: WorkChargeSite) -> Option<WorkAttribution> {
        self.attribution.get(&site).copied()
    }
}

/// Sponsor-owned meter for one terminal execution.
///
/// An absent budget meters without imposing a ceiling. A finite budget
/// fails before executing the first site it cannot pay for. Exhaustion leaves
/// both usage and remaining work unchanged, so it is a sponsor event and
/// never partially charges one semantic instruction.
#[derive(Debug, Clone)]
pub struct WorkMeter {
    schedule: WorkSchedule,
    work_remaining: Option<u64>,
    usage: WorkUsage,
    invocation: Option<EvaluationInvocationAccount>,
}

impl WorkMeter {
    pub fn unbounded() -> Self {
        Self::new(WorkSchedule::CURRENT, None)
    }

    pub fn with_budget(units: u64) -> Self {
        Self::new(WorkSchedule::CURRENT, Some(units))
    }

    /// Pay each real Terminal charge from this exact invocation account as well
    /// as the optional resumable local budget. Cloning a meter shares the
    /// account; replenishing local budget cannot enlarge the sponsor's budget.
    /// Account refusal is a host accounting error, never a guest crash/result.
    /// This connects work only: it does not claim that the interpreter's storage
    /// or returned values have been reserved against the same account.
    pub fn with_invocation(
        invocation: EvaluationInvocationAccount,
        budget: Option<u64>,
    ) -> Result<Self, WorkMeterError> {
        let schedule = WorkSchedule::CURRENT;
        let EvaluationMeasurementIdentity::TerminalPsi { work_schedule, .. } =
            invocation.measurement()
        else {
            return Err(WorkMeterError::InvocationMeasurementMismatch);
        };
        if work_schedule != schedule.identity() {
            return Err(WorkMeterError::InvocationMeasurementMismatch);
        }
        let mut meter = Self::new(schedule, budget);
        meter.invocation = Some(invocation);
        Ok(meter)
    }

    fn new(schedule: WorkSchedule, work_remaining: Option<u64>) -> Self {
        Self {
            schedule,
            work_remaining,
            usage: WorkUsage::empty(schedule.identity()),
            invocation: None,
        }
    }

    pub const fn schedule(&self) -> WorkSchedule {
        self.schedule
    }

    /// Execution must use this same account for storage as well as work.
    /// Borrowing it does not increase the sponsor's budget.
    pub fn invocation_account(&self) -> Option<&EvaluationInvocationAccount> {
        self.invocation.as_ref()
    }

    pub const fn work_remaining(&self) -> Option<u64> {
        self.work_remaining
    }

    pub const fn usage(&self) -> &WorkUsage {
        &self.usage
    }

    pub fn into_usage(self) -> WorkUsage {
        self.usage
    }

    /// Add sponsor budget without changing usage. An unbounded meter stays
    /// unbounded; a finite budget fails rather than wrapping.
    pub fn replenish(&mut self, additional_units: u64) -> Result<(), WorkMeterError> {
        let Some(remaining) = self.work_remaining else {
            return Ok(());
        };
        self.work_remaining = Some(
            remaining
                .checked_add(additional_units)
                .ok_or(WorkMeterError::BudgetOverflow)?,
        );
        Ok(())
    }

    pub fn charge_operation(&mut self, operation: &Operation) -> Result<(), WorkMeterError> {
        self.charge(
            WorkChargeSite::Operation(operation.id),
            self.schedule.operation_units(&operation.kind),
        )
    }

    pub fn charge_terminator(&mut self, terminator: &Terminator) -> Result<(), WorkMeterError> {
        let edge = match terminator {
            Terminator::Jump { edge, .. }
            | terminal_psi::terminator_return_pair_pattern!(edge, ..)
            | Terminator::ReturnStructural { edge, .. }
            | Terminator::Crash { edge, .. }
            | Terminator::InvokeNoNormal { edge, .. }
            | Terminator::ExternalCompletion { edge, .. } => *edge,
            Terminator::Conditional { .. } | Terminator::StructuralCase { .. } => {
                return Err(WorkMeterError::ConditionalEdgeNotSelected);
            }
        };
        self.charge_edge(edge, terminator)
    }

    /// Charge exactly the selected successor of a conditional terminator.
    pub fn charge_edge(
        &mut self,
        edge: EdgeId,
        terminator: &Terminator,
    ) -> Result<(), WorkMeterError> {
        if !terminator.edges().any(|candidate| candidate == edge) {
            return Err(WorkMeterError::EdgeNotOwnedByTerminator(edge));
        }
        self.charge(
            WorkChargeSite::Edge(edge),
            self.schedule.terminator_units(terminator),
        )
    }

    fn charge(&mut self, site: WorkChargeSite, units: u64) -> Result<(), WorkMeterError> {
        if let Some(remaining) = self.work_remaining
            && remaining < units
        {
            return Err(WorkMeterError::Exhausted(WorkBudgetExhausted {
                schedule: self.schedule.identity(),
                site,
                required_units: units,
                remaining_units: remaining,
                work_used: self.usage.total_units,
            }));
        }

        let previous = self.usage.at(site).unwrap_or_default();
        let total_units = self
            .usage
            .total_units
            .checked_add(units)
            .ok_or(WorkMeterError::AccountingOverflow(site))?;
        let executions = previous
            .executions
            .checked_add(1)
            .ok_or(WorkMeterError::AccountingOverflow(site))?;
        let attributed_units = previous
            .units
            .checked_add(units)
            .ok_or(WorkMeterError::AccountingOverflow(site))?;

        // All local refusals precede shared payment. The account atomically
        // checks invocation and aggregate ceilings; only successful payment
        // commits local attribution and permits the semantic site to execute.
        if let Some(invocation) = &self.invocation {
            invocation
                .charge_work(units)
                .map_err(|reason| WorkMeterError::InvocationCharge { site, reason })?;
        }
        self.usage.total_units = total_units;
        self.usage.attribution.insert(
            site,
            WorkAttribution {
                executions,
                units: attributed_units,
            },
        );
        if let Some(remaining) = &mut self.work_remaining {
            *remaining -= units;
        }
        Ok(())
    }
}

impl Default for WorkMeter {
    fn default() -> Self {
        Self::unbounded()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkBudgetExhausted {
    pub schedule: WorkScheduleIdentity,
    pub site: WorkChargeSite,
    pub required_units: u64,
    pub remaining_units: u64,
    pub work_used: u64,
}

// Exhaustion reports logical charges already paid and the next unpaid charge.
// Replenishment changes the local budget without resetting accumulated usage.
impl std::fmt::Display for WorkBudgetExhausted {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Work budget exhausted: {} units used, {} remaining; next charge requires {} (schedule {})",
            self.work_used,
            self.remaining_units,
            self.required_units,
            self.schedule.marker()
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkMeterError {
    InvocationMeasurementMismatch,
    InvocationCharge {
        site: WorkChargeSite,
        reason: EvaluationAccountingError,
    },
    ConditionalEdgeNotSelected,
    EdgeNotOwnedByTerminator(EdgeId),
    Exhausted(WorkBudgetExhausted),
    AccountingOverflow(WorkChargeSite),
    BudgetOverflow,
}

impl std::fmt::Display for WorkMeterError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvocationCharge { site, reason } => write!(formatter, "{reason} at {site:?}"),
            Self::Exhausted(exhaustion) => exhaustion.fmt(formatter),
            other => write!(formatter, "{other:?}"),
        }
    }
}

impl std::error::Error for WorkMeterError {}
