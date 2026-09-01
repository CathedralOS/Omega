#![forbid(unsafe_code)]

//! Canonical logical-fuel schedule and meter for terminal Psi.
//!
//! Logical fuel is deterministic portable work, not native instructions,
//! cycles, energy, or elapsed time. The executing program cannot inspect this
//! meter; an interpreter, evaluator, or trusted native realization owns it on
//! behalf of the execution sponsor.

use std::collections::BTreeMap;

use psi_core::{EdgeId, OperationId};
use psi_terminal::{Operation, OperationKind, Terminator};

pub use psi_core::FuelScheduleIdentity;

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
            OperationKind::EstablishPayloadlessCase { .. }
            | OperationKind::EstablishByteSequenceLiteral { .. }
            | OperationKind::EstablishTrivialAffineLocal { .. }
            | OperationKind::Call { .. }
            | OperationKind::CallUnit { .. }
            | OperationKind::CallStructuralScalar { .. }
            | OperationKind::CallStructural { .. }
            | OperationKind::WriteOnlyPrimitiveStore { .. }
            | OperationKind::BoundaryCall { .. }
            | OperationKind::PortWrite { .. }
            | OperationKind::IntegerConstant { .. }
            | OperationKind::IeeeFloatConstant { .. }
            | OperationKind::NearestIeeeFloatFusedMultiplyAdd { .. }
            | OperationKind::BooleanConstant { .. }
            | OperationKind::BooleanStructuralField { .. }
            | OperationKind::BooleanNot { .. }
            | OperationKind::BooleanEqual { .. }
            | OperationKind::IntegerEqual { .. }
            | OperationKind::IntegerLessThan { .. }
            | OperationKind::IntegerLessOrEqual { .. }
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
            | OperationKind::ExactIntegerAdd { .. }
            | OperationKind::ExactIntegerSubtract { .. }
            | OperationKind::ExactIntegerMultiply { .. }
            | OperationKind::ExactIntegerDivide { .. }
            | OperationKind::ExactIntegerRemainder { .. }
            | OperationKind::WrappingIntegerDivide { .. }
            | OperationKind::WrappingIntegerRemainder { .. }
            | OperationKind::SaturatingIntegerDivide { .. }
            | OperationKind::SaturatingIntegerRemainder { .. }
            | OperationKind::WrappingIntegerAdd { .. }
            | OperationKind::SaturatingIntegerAdd { .. }
            | OperationKind::WrappingIntegerSubtract { .. }
            | OperationKind::SaturatingIntegerSubtract { .. }
            | OperationKind::WrappingIntegerMultiply { .. }
            | OperationKind::SaturatingIntegerMultiply { .. } => 1,
        }
    }

    pub const fn terminator_units(self, terminator: &Terminator) -> u64 {
        match terminator {
            Terminator::Jump { .. }
            | Terminator::Conditional { .. }
            | Terminator::Return { .. }
            | Terminator::ReturnUnit { .. }
            | Terminator::ReturnUnitPartialAffine { .. }
            | Terminator::ReturnUnitNominalAffine { .. }
            | Terminator::ReturnStructural { .. }
            | Terminator::Crash { .. } => 1,
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalFuelMeter {
    schedule: TerminalFuelSchedule,
    remaining_allowance: Option<u64>,
    usage: TerminalFuelUsage,
}

impl TerminalFuelMeter {
    pub fn unbounded() -> Self {
        Self::new(TerminalFuelSchedule::CURRENT, None)
    }

    pub fn with_allowance(units: u64) -> Self {
        Self::new(TerminalFuelSchedule::CURRENT, Some(units))
    }

    fn new(schedule: TerminalFuelSchedule, remaining_allowance: Option<u64>) -> Self {
        Self {
            schedule,
            remaining_allowance,
            usage: TerminalFuelUsage::empty(schedule.identity()),
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
            | Terminator::Return { edge, .. }
            | Terminator::ReturnUnit { edge, .. }
            | Terminator::ReturnUnitPartialAffine { edge, .. }
            | Terminator::ReturnUnitNominalAffine { edge, .. }
            | Terminator::ReturnStructural { edge, .. }
            | Terminator::Crash { edge, .. } => *edge,
            Terminator::Conditional { .. } => {
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
    ConditionalEdgeNotSelected,
    EdgeNotOwnedByTerminator(EdgeId),
    Exhausted(FuelExhaustion),
    AccountingOverflow(FuelChargeSite),
    AllowanceOverflow,
}

impl std::fmt::Display for FuelMeterError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for FuelMeterError {}
