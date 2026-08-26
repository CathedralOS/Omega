use omega_register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use omega_terminal_selected_instructions::{TerminalSelectedBlockId, TerminalVirtualRegisterId};
use psi_core::MachineId;

use crate::{
    TerminalLiveRangeIdentity, TerminalLiveRangePoint, TerminalVirtualFixedConstraintSite,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalAllocationLegalityIdentity(pub(crate) [u8; 32]);

impl TerminalAllocationLegalityIdentity {
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Exact physical-view legality before allocation. This is analysis output,
/// not a home assignment: incompatible fixed views remain explicit transition
/// requirements rather than being silently reconciled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAllocationLegalityPlan {
    pub ranges: TerminalLiveRangeIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub functions: Vec<TerminalFunctionAllocationLegality>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalFunctionAllocationLegality {
    pub machine: MachineId,
    pub virtual_registers: Vec<TerminalVirtualRegisterAllocationLegality>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalVirtualRegisterAllocationLegality {
    pub virtual_register: TerminalVirtualRegisterId,
    pub class: RegisterClassId,
    pub points: Vec<TerminalVirtualPointLegality>,
    pub entry_transitions: Vec<TerminalEntryFixedViewTransition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalVirtualPointLegality {
    pub block: TerminalSelectedBlockId,
    pub point: TerminalLiveRangePoint,
    /// Canonical view-ID-sorted candidates legal at this exact phase.
    pub candidates: Vec<RegisterViewId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalEntryFixedViewTransition {
    pub from_view: RegisterViewId,
    pub to_site: TerminalVirtualFixedConstraintSite,
    pub to_view: RegisterViewId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalAllocationLegalityValidationReceipt {
    pub(crate) identity: TerminalAllocationLegalityIdentity,
    pub(crate) ranges: TerminalLiveRangeIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) function_count: usize,
    pub(crate) virtual_register_count: usize,
    pub(crate) point_count: usize,
    pub(crate) candidate_count: usize,
    pub(crate) entry_transition_count: usize,
}

impl TerminalAllocationLegalityValidationReceipt {
    pub const fn identity(self) -> TerminalAllocationLegalityIdentity {
        self.identity
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
    pub const fn virtual_register_count(self) -> usize {
        self.virtual_register_count
    }
    pub const fn point_count(self) -> usize {
        self.point_count
    }
    pub const fn candidate_count(self) -> usize {
        self.candidate_count
    }
    pub const fn entry_transition_count(self) -> usize {
        self.entry_transition_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTerminalAllocationLegality {
    pub(crate) plan: TerminalAllocationLegalityPlan,
    pub(crate) receipt: TerminalAllocationLegalityValidationReceipt,
}

impl ValidatedTerminalAllocationLegality {
    pub const fn plan(&self) -> &TerminalAllocationLegalityPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> TerminalAllocationLegalityValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalAllocationLegalityError {
    RootMismatch,
    UnknownClass {
        function: usize,
        register: u32,
        class: u16,
    },
    UnknownFixedView {
        function: usize,
        register: u32,
        view: u16,
    },
    IllegalFixedView {
        function: usize,
        register: u32,
        view: u16,
    },
    NoCandidateViews {
        function: usize,
        register: u32,
        block: u32,
        point: u32,
    },
    PointOverflow {
        function: usize,
    },
    FunctionMismatch {
        function: usize,
    },
    VirtualRegisterMismatch {
        function: usize,
        register: u32,
    },
    NonCanonicalRows {
        function: usize,
        register: u32,
    },
}

impl std::fmt::Display for TerminalAllocationLegalityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Terminal allocation-legality derivation failed: {self:?}"
        )
    }
}

impl std::error::Error for TerminalAllocationLegalityError {}
