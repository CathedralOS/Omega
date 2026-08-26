use omega_register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use omega_terminal_selected_instructions::TerminalVirtualRegisterId;
use psi_core::MachineId;

use crate::{TerminalAllocationLegalityIdentity, TerminalLiveRangeIdentity};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalRegisterHomeIdentity(pub(crate) [u8; 32]);

impl TerminalRegisterHomeIdentity {
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Bounded, deterministic physical homes for one transition-free legality
/// plan. The artifact grants no spill, frame, instruction-emission, or
/// publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalRegisterHomePlan {
    pub legality: TerminalAllocationLegalityIdentity,
    pub ranges: TerminalLiveRangeIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub functions: Vec<TerminalFunctionRegisterHomes>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalFunctionRegisterHomes {
    pub machine: MachineId,
    pub assignments: Vec<TerminalVirtualRegisterHome>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalVirtualRegisterHome {
    pub virtual_register: TerminalVirtualRegisterId,
    pub class: RegisterClassId,
    pub view: RegisterViewId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalRegisterHomeValidationReceipt {
    pub(crate) identity: TerminalRegisterHomeIdentity,
    pub(crate) legality: TerminalAllocationLegalityIdentity,
    pub(crate) ranges: TerminalLiveRangeIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) function_count: usize,
    pub(crate) assignment_count: usize,
}

impl TerminalRegisterHomeValidationReceipt {
    pub const fn identity(self) -> TerminalRegisterHomeIdentity {
        self.identity
    }
    pub const fn legality(self) -> TerminalAllocationLegalityIdentity {
        self.legality
    }
    pub const fn ranges(self) -> TerminalLiveRangeIdentity {
        self.ranges
    }
    pub const fn register_environment(self) -> TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn assignment_count(self) -> usize {
        self.assignment_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTerminalRegisterHomes {
    pub(crate) plan: TerminalRegisterHomePlan,
    pub(crate) receipt: TerminalRegisterHomeValidationReceipt,
}

impl ValidatedTerminalRegisterHomes {
    pub const fn plan(&self) -> &TerminalRegisterHomePlan {
        &self.plan
    }
    pub const fn receipt(&self) -> TerminalRegisterHomeValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalRegisterHomeError {
    RootMismatch,
    FunctionMismatch {
        function: usize,
    },
    VirtualRegisterMismatch {
        function: usize,
        register: u32,
    },
    UnresolvedEntryTransitions {
        function: usize,
        register: u32,
        count: usize,
    },
    NoLivePoints {
        function: usize,
        register: u32,
    },
    NoCommonCandidate {
        function: usize,
        register: u32,
    },
    UnknownOrIncompatibleView {
        function: usize,
        register: u32,
        view: u16,
    },
    NoCompatibleHome {
        function: usize,
        register: u32,
    },
    NonCanonicalAssignments {
        function: usize,
    },
}

impl std::fmt::Display for TerminalRegisterHomeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Terminal register-home assignment failed: {self:?}"
        )
    }
}

impl std::error::Error for TerminalRegisterHomeError {}
