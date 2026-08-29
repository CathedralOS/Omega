use omega_target::NativeTarget;
use omega_target_operations::ScalarParameterLocation;
use psi_core::{
    ClaimId, EdgeId, IntegerType, IntegerValue, MachineId, OperationId, ScalarType,
    StructuralTypeId, ValueId,
};
use psi_terminal::{CrashCause, CrashPredicateTerm, TerminalPsiIdentity};

use super::AbstractToTargetTranslationFamily;

/// Independent source-to-target validation retained at the lowering boundary.
///
/// Root and function-roster custody cover the complete plan. Every function
/// row carries exactly one validated family receipt or an explicit uncovered
/// disposition, so parallel family rosters cannot drift apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractToTargetTranslationValidationReceipt {
    psi: TerminalPsiIdentity,
    target: NativeTarget,
    entry: MachineId,
    function_roster: Vec<AbstractToTargetFunctionRosterReceipt>,
}

impl AbstractToTargetTranslationValidationReceipt {
    pub(in crate::validation) fn new(
        psi: TerminalPsiIdentity,
        target: NativeTarget,
        entry: MachineId,
        function_roster: Vec<AbstractToTargetFunctionRosterReceipt>,
    ) -> Self {
        Self {
            psi,
            target,
            entry,
            function_roster,
        }
    }

    pub const fn psi(&self) -> TerminalPsiIdentity {
        self.psi
    }

    pub const fn target(&self) -> NativeTarget {
        self.target
    }

    pub const fn entry(&self) -> MachineId {
        self.entry
    }

    pub const fn function_count(&self) -> usize {
        self.function_roster.len()
    }

    pub fn function_roster(&self) -> &[AbstractToTargetFunctionRosterReceipt] {
        &self.function_roster
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractToTargetFunctionRosterReceipt {
    machine: MachineId,
    attachment: Option<StructuralTypeId>,
    translation: AbstractToTargetFunctionTranslationDisposition,
}

impl AbstractToTargetFunctionRosterReceipt {
    pub(in crate::validation) const fn new(
        machine: MachineId,
        attachment: Option<StructuralTypeId>,
        translation: AbstractToTargetFunctionTranslationDisposition,
    ) -> Self {
        Self {
            machine,
            attachment,
            translation,
        }
    }

    pub const fn machine(&self) -> MachineId {
        self.machine
    }

    pub const fn attachment(&self) -> Option<StructuralTypeId> {
        self.attachment
    }

    pub const fn translation(&self) -> &AbstractToTargetFunctionTranslationDisposition {
        &self.translation
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractToTargetFunctionTranslationDisposition {
    Uncovered,
    Validated(AbstractToTargetFunctionTranslationReceipt),
}

impl AbstractToTargetFunctionTranslationDisposition {
    pub const fn validated(&self) -> Option<&AbstractToTargetFunctionTranslationReceipt> {
        match self {
            Self::Uncovered => None,
            Self::Validated(receipt) => Some(receipt),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractToTargetFunctionTranslationReceipt {
    StraightLineIntegerImmediate(StraightLineIntegerImmediateTranslationReceipt),
    StraightLineBooleanImmediate(StraightLineBooleanImmediateTranslationReceipt),
    StraightLineScalarCrash(StraightLineScalarCrashTranslationReceipt),
    StraightLineIntegerParameter(StraightLineIntegerParameterTranslationReceipt),
    StraightLineBooleanParameter(StraightLineBooleanParameterTranslationReceipt),
    StraightLineBooleanNotParameter(StraightLineBooleanNotParameterTranslationReceipt),
}

impl AbstractToTargetFunctionTranslationReceipt {
    pub const fn family(&self) -> AbstractToTargetTranslationFamily {
        match self {
            Self::StraightLineIntegerImmediate(_) => {
                AbstractToTargetTranslationFamily::StraightLineIntegerImmediate
            }
            Self::StraightLineBooleanImmediate(_) => {
                AbstractToTargetTranslationFamily::StraightLineBooleanImmediate
            }
            Self::StraightLineScalarCrash(_) => {
                AbstractToTargetTranslationFamily::StraightLineScalarCrash
            }
            Self::StraightLineIntegerParameter(_) => {
                AbstractToTargetTranslationFamily::StraightLineIntegerParameter
            }
            Self::StraightLineBooleanParameter(_) => {
                AbstractToTargetTranslationFamily::StraightLineBooleanParameter
            }
            Self::StraightLineBooleanNotParameter(_) => {
                AbstractToTargetTranslationFamily::StraightLineBooleanNotParameter
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightLineBooleanImmediateTranslationReceipt {
    machine: MachineId,
    constant_operation: OperationId,
    return_edge: EdgeId,
    source_value: ValueId,
    value: bool,
}

impl StraightLineBooleanImmediateTranslationReceipt {
    pub(in crate::validation) const fn new(
        machine: MachineId,
        constant_operation: OperationId,
        return_edge: EdgeId,
        source_value: ValueId,
        value: bool,
    ) -> Self {
        Self {
            machine,
            constant_operation,
            return_edge,
            source_value,
            value,
        }
    }

    pub const fn machine(self) -> MachineId {
        self.machine
    }

    pub const fn constant_operation(self) -> OperationId {
        self.constant_operation
    }

    pub const fn return_edge(self) -> EdgeId {
        self.return_edge
    }

    pub const fn source_value(self) -> ValueId {
        self.source_value
    }

    pub const fn value(self) -> bool {
        self.value
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightLineIntegerImmediateTranslationReceipt {
    machine: MachineId,
    constant_operation: OperationId,
    return_edge: EdgeId,
    source_value: ValueId,
    scalar_type: IntegerType,
    value: IntegerValue,
}

impl StraightLineIntegerImmediateTranslationReceipt {
    pub(in crate::validation) const fn new(
        machine: MachineId,
        constant_operation: OperationId,
        return_edge: EdgeId,
        source_value: ValueId,
        scalar_type: IntegerType,
        value: IntegerValue,
    ) -> Self {
        Self {
            machine,
            constant_operation,
            return_edge,
            source_value,
            scalar_type,
            value,
        }
    }

    pub const fn machine(self) -> MachineId {
        self.machine
    }

    pub const fn constant_operation(self) -> OperationId {
        self.constant_operation
    }

    pub const fn return_edge(self) -> EdgeId {
        self.return_edge
    }

    pub const fn source_value(self) -> ValueId {
        self.source_value
    }

    pub const fn scalar_type(self) -> IntegerType {
        self.scalar_type
    }

    pub const fn value(self) -> IntegerValue {
        self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StraightLineScalarCrashTranslationReceipt {
    machine: MachineId,
    result_type: ScalarType,
    crash_edge: EdgeId,
    cause: CrashCause,
    site_guard: Vec<CrashPredicateTerm>,
    frontier_lower_bound: Vec<ClaimId>,
}

impl StraightLineScalarCrashTranslationReceipt {
    pub(in crate::validation) fn new(
        machine: MachineId,
        result_type: ScalarType,
        crash_edge: EdgeId,
        cause: CrashCause,
        site_guard: Vec<CrashPredicateTerm>,
        frontier_lower_bound: Vec<ClaimId>,
    ) -> Self {
        Self {
            machine,
            result_type,
            crash_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        }
    }

    pub const fn machine(&self) -> MachineId {
        self.machine
    }

    pub const fn result_type(&self) -> ScalarType {
        self.result_type
    }

    pub const fn crash_edge(&self) -> EdgeId {
        self.crash_edge
    }

    pub const fn cause(&self) -> CrashCause {
        self.cause
    }

    pub fn site_guard(&self) -> &[CrashPredicateTerm] {
        &self.site_guard
    }

    pub fn frontier_lower_bound(&self) -> &[ClaimId] {
        &self.frontier_lower_bound
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightLineIntegerParameterTranslationReceipt {
    machine: MachineId,
    return_edge: EdgeId,
    source_value: ValueId,
    scalar_type: IntegerType,
    parameter_index: usize,
    location: ScalarParameterLocation,
}

impl StraightLineIntegerParameterTranslationReceipt {
    pub(in crate::validation) const fn new(
        machine: MachineId,
        return_edge: EdgeId,
        source_value: ValueId,
        scalar_type: IntegerType,
        parameter_index: usize,
        location: ScalarParameterLocation,
    ) -> Self {
        Self {
            machine,
            return_edge,
            source_value,
            scalar_type,
            parameter_index,
            location,
        }
    }

    pub const fn machine(self) -> MachineId {
        self.machine
    }

    pub const fn return_edge(self) -> EdgeId {
        self.return_edge
    }

    pub const fn source_value(self) -> ValueId {
        self.source_value
    }

    pub const fn scalar_type(self) -> IntegerType {
        self.scalar_type
    }

    pub const fn parameter_index(self) -> usize {
        self.parameter_index
    }

    pub const fn location(self) -> ScalarParameterLocation {
        self.location
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightLineBooleanParameterTranslationReceipt {
    machine: MachineId,
    return_edge: EdgeId,
    source_value: ValueId,
    parameter_index: usize,
    location: ScalarParameterLocation,
}

impl StraightLineBooleanParameterTranslationReceipt {
    pub(in crate::validation) const fn new(
        machine: MachineId,
        return_edge: EdgeId,
        source_value: ValueId,
        parameter_index: usize,
        location: ScalarParameterLocation,
    ) -> Self {
        Self {
            machine,
            return_edge,
            source_value,
            parameter_index,
            location,
        }
    }

    pub const fn machine(self) -> MachineId {
        self.machine
    }

    pub const fn return_edge(self) -> EdgeId {
        self.return_edge
    }

    pub const fn source_value(self) -> ValueId {
        self.source_value
    }

    pub const fn parameter_index(self) -> usize {
        self.parameter_index
    }

    pub const fn location(self) -> ScalarParameterLocation {
        self.location
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightLineBooleanNotParameterTranslationReceipt {
    machine: MachineId,
    not_operation: OperationId,
    return_edge: EdgeId,
    source_value: ValueId,
    operand_value: ValueId,
    parameter_index: usize,
    location: ScalarParameterLocation,
}

impl StraightLineBooleanNotParameterTranslationReceipt {
    pub(in crate::validation) const fn new(
        machine: MachineId,
        not_operation: OperationId,
        return_edge: EdgeId,
        source_value: ValueId,
        operand_value: ValueId,
        parameter_index: usize,
        location: ScalarParameterLocation,
    ) -> Self {
        Self {
            machine,
            not_operation,
            return_edge,
            source_value,
            operand_value,
            parameter_index,
            location,
        }
    }

    pub const fn machine(self) -> MachineId {
        self.machine
    }

    pub const fn not_operation(self) -> OperationId {
        self.not_operation
    }

    pub const fn return_edge(self) -> EdgeId {
        self.return_edge
    }

    pub const fn source_value(self) -> ValueId {
        self.source_value
    }

    pub const fn operand_value(self) -> ValueId {
        self.operand_value
    }

    pub const fn parameter_index(self) -> usize {
        self.parameter_index
    }

    pub const fn location(self) -> ScalarParameterLocation {
        self.location
    }
}
