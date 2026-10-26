#![forbid(unsafe_code)]

//! Canonical logical-fuel schedule and meter for terminal Psi.
//!
//! Logical fuel is deterministic portable work, not native instructions,
//! cycles, energy, or elapsed time. The executing program cannot inspect this
//! meter; an interpreter, evaluator, or bounded compiler service owns it.
//! Native execution has no implicit logical-fuel meter.

use std::collections::BTreeMap;

use evaluation_accounting::{
    EvaluationAccountingError, EvaluationInvocationAccount, EvaluationMeasurementIdentity,
};
use semantic_vocabulary::{EdgeId, OperationId};
use terminal_psi::{Operation, OperationKind, Terminator};

pub use semantic_vocabulary::FuelScheduleIdentity;

/// The current schedule charges one unit for each executed semantic operation
/// and one unit for each taken terminal edge. Adding a vocabulary variant
/// forces an explicit update to these exhaustive matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalFuelSchedule {
    identity: FuelScheduleIdentity,
}

impl TerminalFuelSchedule {
    pub const CURRENT: Self = Self {
        identity: match FuelScheduleIdentity::new(1) {
            Some(identity) => identity,
            None => unreachable!(),
        },
    };

    pub const fn identity(self) -> FuelScheduleIdentity {
        self.identity
    }

    pub const fn operation_units(self, kind: &OperationKind) -> u64 {
        match kind {
            OperationKind::EstablishNativeCodeLayout { .. } | OperationKind::EstablishPlacedReference { .. }
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
            | OperationKind::StorageAddress { .. } => 1,
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
            | Terminator::ExternalCompletion { .. } => 1,
        }
    }
}

/// Stable semantic site to which executed logical work is attributed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FuelChargeSite {
    Operation(OperationId),
    Edge(EdgeId),
}

/// Aggregate execution count and logical units charged at one semantic site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FuelAttribution {
    executions: u64,
    units: u64,
}

impl FuelAttribution {
    pub const fn executions(self) -> u64 {
        self.executions
    }

    pub const fn units(self) -> u64 {
        self.units
    }
}

/// Deterministic usage for one concrete terminal-Psi execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalFuelUsage {
    schedule: FuelScheduleIdentity,
    total_units: u64,
    attribution: BTreeMap<FuelChargeSite, FuelAttribution>,
}

impl TerminalFuelUsage {
    fn empty(schedule: FuelScheduleIdentity) -> Self {
        Self {
            schedule,
            total_units: 0,
            attribution: BTreeMap::new(),
        }
    }

    pub const fn schedule(&self) -> FuelScheduleIdentity {
        self.schedule
    }

    pub const fn total_units(&self) -> u64 {
        self.total_units
    }

    pub fn attribution(&self) -> &BTreeMap<FuelChargeSite, FuelAttribution> {
        &self.attribution
    }

    pub fn at(&self, site: FuelChargeSite) -> Option<FuelAttribution> {
        self.attribution.get(&site).copied()
    }
}

/// Sponsor-owned meter for one terminal execution.
///
/// An absent allowance meters without imposing a ceiling. A finite allowance
/// fails before executing the first site it cannot pay for. Exhaustion leaves
/// both usage and remaining allowance unchanged, so it is a sponsor event and
/// never partially charges one semantic instruction.
#[derive(Debug, Clone)]
pub struct TerminalFuelMeter {
    schedule: TerminalFuelSchedule,
    remaining_allowance: Option<u64>,
    usage: TerminalFuelUsage,
    invocation: Option<EvaluationInvocationAccount>,
}

impl TerminalFuelMeter {
    pub fn unbounded() -> Self {
        Self::new(TerminalFuelSchedule::CURRENT, None)
    }

    pub fn with_allowance(units: u64) -> Self {
        Self::new(TerminalFuelSchedule::CURRENT, Some(units))
    }

    /// Pay each real Terminal charge from this exact invocation account as well
    /// as the optional resumable local allowance. Cloning a meter shares the
    /// account; replenishing local allowance cannot enlarge the sponsor's budget.
    /// Account refusal is a host accounting error, never a guest crash/result.
    /// This connects fuel only: it does not claim that the interpreter's storage
    /// or returned values have been reserved against the same account.
    pub fn with_invocation(
        invocation: EvaluationInvocationAccount,
        allowance: Option<u64>,
    ) -> Result<Self, FuelMeterError> {
        let schedule = TerminalFuelSchedule::CURRENT;
        let EvaluationMeasurementIdentity::TerminalPsi { fuel_schedule, .. } =
            invocation.measurement()
        else {
            return Err(FuelMeterError::InvocationMeasurementMismatch);
        };
        if fuel_schedule != schedule.identity() {
            return Err(FuelMeterError::InvocationMeasurementMismatch);
        }
        let mut meter = Self::new(schedule, allowance);
        meter.invocation = Some(invocation);
        Ok(meter)
    }

    fn new(schedule: TerminalFuelSchedule, remaining_allowance: Option<u64>) -> Self {
        Self {
            schedule,
            remaining_allowance,
            usage: TerminalFuelUsage::empty(schedule.identity()),
            invocation: None,
        }
    }

    pub const fn schedule(&self) -> TerminalFuelSchedule {
        self.schedule
    }

    pub const fn remaining_allowance(&self) -> Option<u64> {
        self.remaining_allowance
    }

    pub const fn usage(&self) -> &TerminalFuelUsage {
        &self.usage
    }

    pub fn into_usage(self) -> TerminalFuelUsage {
        self.usage
    }

    /// Add sponsor allowance without changing usage. An unbounded meter stays
    /// unbounded; a finite allowance fails rather than wrapping.
    pub fn replenish(&mut self, additional_units: u64) -> Result<(), FuelMeterError> {
        let Some(remaining) = self.remaining_allowance else {
            return Ok(());
        };
        self.remaining_allowance = Some(
            remaining
                .checked_add(additional_units)
                .ok_or(FuelMeterError::AllowanceOverflow)?,
        );
        Ok(())
    }

    pub fn charge_operation(&mut self, operation: &Operation) -> Result<(), FuelMeterError> {
        self.charge(
            FuelChargeSite::Operation(operation.id),
            self.schedule.operation_units(&operation.kind),
        )
    }

    pub fn charge_terminator(&mut self, terminator: &Terminator) -> Result<(), FuelMeterError> {
        let edge = match terminator {
            Terminator::Jump { edge, .. }
            | terminal_psi::terminator_return_pair_pattern!(edge, ..)
            | Terminator::ReturnStructural { edge, .. }
            | Terminator::Crash { edge, .. }
            | Terminator::ExternalCompletion { edge, .. } => *edge,
            Terminator::Conditional { .. } | Terminator::StructuralCase { .. } => {
                return Err(FuelMeterError::ConditionalEdgeNotSelected);
            }
        };
        self.charge_edge(edge, terminator)
    }

    /// Charge exactly the selected successor of a conditional terminator.
    pub fn charge_edge(
        &mut self,
        edge: EdgeId,
        terminator: &Terminator,
    ) -> Result<(), FuelMeterError> {
        if !terminator.edges().any(|candidate| candidate == edge) {
            return Err(FuelMeterError::EdgeNotOwnedByTerminator(edge));
        }
        self.charge(
            FuelChargeSite::Edge(edge),
            self.schedule.terminator_units(terminator),
        )
    }

    fn charge(&mut self, site: FuelChargeSite, units: u64) -> Result<(), FuelMeterError> {
        if let Some(remaining) = self.remaining_allowance
            && remaining < units
        {
            return Err(FuelMeterError::Exhausted(FuelExhaustion {
                schedule: self.schedule.identity(),
                site,
                required_units: units,
                remaining_units: remaining,
            }));
        }

        let previous = self.usage.at(site).unwrap_or_default();
        let total_units = self
            .usage
            .total_units
            .checked_add(units)
            .ok_or(FuelMeterError::AccountingOverflow(site))?;
        let executions = previous
            .executions
            .checked_add(1)
            .ok_or(FuelMeterError::AccountingOverflow(site))?;
        let attributed_units = previous
            .units
            .checked_add(units)
            .ok_or(FuelMeterError::AccountingOverflow(site))?;

        // All local refusals precede shared payment. The account atomically
        // checks invocation and aggregate ceilings; only successful payment
        // commits local attribution and permits the semantic site to execute.
        if let Some(invocation) = &self.invocation {
            invocation
                .charge_fuel(units)
                .map_err(|reason| FuelMeterError::InvocationCharge { site, reason })?;
        }
        self.usage.total_units = total_units;
        self.usage.attribution.insert(
            site,
            FuelAttribution {
                executions,
                units: attributed_units,
            },
        );
        if let Some(remaining) = &mut self.remaining_allowance {
            *remaining -= units;
        }
        Ok(())
    }
}

impl Default for TerminalFuelMeter {
    fn default() -> Self {
        Self::unbounded()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FuelExhaustion {
    pub schedule: FuelScheduleIdentity,
    pub site: FuelChargeSite,
    pub required_units: u64,
    pub remaining_units: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuelMeterError {
    InvocationMeasurementMismatch,
    InvocationCharge {
        site: FuelChargeSite,
        reason: EvaluationAccountingError,
    },
    ConditionalEdgeNotSelected,
    EdgeNotOwnedByTerminator(EdgeId),
    Exhausted(FuelExhaustion),
    AccountingOverflow(FuelChargeSite),
    AllowanceOverflow,
}

impl std::fmt::Display for FuelMeterError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvocationCharge { site, reason } => write!(formatter, "{reason} at {site:?}"),
            other => write!(formatter, "{other:?}"),
        }
    }
}

impl std::error::Error for FuelMeterError {}
