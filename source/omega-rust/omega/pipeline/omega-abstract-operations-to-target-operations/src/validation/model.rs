use omega_target::NativeTarget;
use psi_core::{
    EdgeId, IntegerType, IntegerValue, MachineId, OperationId, StructuralTypeId, ValueId,
};
use psi_terminal::TerminalPsiIdentity;

/// Independent source-to-target validation retained at the lowering boundary.
///
/// Root and function-roster custody cover the complete plan. The function
/// rows name the exact lowering families whose semantic translation has also
/// been reconstructed; absence from this roster is not a validation claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractToTargetTranslationValidationReceipt {
    psi: TerminalPsiIdentity,
    target: NativeTarget,
    entry: MachineId,
    function_roster: Vec<AbstractToTargetFunctionRosterReceipt>,
    straight_line_integer_immediates: Vec<StraightLineIntegerImmediateTranslationReceipt>,
}

impl AbstractToTargetTranslationValidationReceipt {
    pub(super) fn new(
        psi: TerminalPsiIdentity,
        target: NativeTarget,
        entry: MachineId,
        function_roster: Vec<AbstractToTargetFunctionRosterReceipt>,
        straight_line_integer_immediates: Vec<StraightLineIntegerImmediateTranslationReceipt>,
    ) -> Self {
        Self {
            psi,
            target,
            entry,
            function_roster,
            straight_line_integer_immediates,
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

    pub fn straight_line_integer_immediates(
        &self,
    ) -> &[StraightLineIntegerImmediateTranslationReceipt] {
        &self.straight_line_integer_immediates
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbstractToTargetFunctionRosterReceipt {
    machine: MachineId,
    attachment: Option<StructuralTypeId>,
}

impl AbstractToTargetFunctionRosterReceipt {
    pub(super) const fn new(machine: MachineId, attachment: Option<StructuralTypeId>) -> Self {
        Self {
            machine,
            attachment,
        }
    }

    pub const fn machine(self) -> MachineId {
        self.machine
    }

    pub const fn attachment(self) -> Option<StructuralTypeId> {
        self.attachment
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
    pub(super) const fn new(
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
pub enum AbstractToTargetTranslationValidationError {
    PsiMismatch,
    TargetMismatch,
    EntryMismatch,
    FunctionCountMismatch,
    FunctionMachineMismatch {
        position: usize,
    },
    FunctionAttachmentMismatch {
        machine: MachineId,
    },
    StraightLineIntegerImmediate {
        machine: MachineId,
        error: StraightLineIntegerImmediateTranslationError,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineIntegerImmediateTranslationError {
    SourceParameters,
    SourceStructuralParameters,
    SourceResult,
    SourceEntryClaims,
    SourcePublishedServices,
    SourceBlockRoster,
    SourceOperationRoster,
    SourceConstantType,
    SourceConstantOutsideType,
    SourceResultLink,
    SourceCleanup,
    TargetProvenance,
    TargetOperation,
}

impl std::fmt::Display for AbstractToTargetTranslationValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "abstract-to-target translation validation failed: {self:?}"
        )
    }
}

impl std::error::Error for AbstractToTargetTranslationValidationError {}
