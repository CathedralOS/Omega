//! Refusals reported to the host when binding, invoking or closing a Psi instance.

use semantic_vocabulary::BoundaryMachineId;

use crate::boundary::errors::TerminalInterpretError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalBindingRejection {
    ExternalCompletionRequiresDomainAdmission {
        boundary: BoundaryMachineId,
    },
    ExternalCompletionDomainMismatch {
        boundary: BoundaryMachineId,
    },
    DomainDeathCrossingAdmissionMissing {
        boundary: BoundaryMachineId,
    },
    UnknownImport {
        boundary: BoundaryMachineId,
    },
    NotRequiredImport {
        boundary: BoundaryMachineId,
    },
    ProviderRealized {
        boundary: BoundaryMachineId,
    },
    DuplicateBinding {
        boundary: BoundaryMachineId,
    },
    SignatureMismatch {
        boundary: BoundaryMachineId,
    },
    /// The adapter does not serve a guest admitted without producer trust.
    UncontainedAdapter {
        boundary: BoundaryMachineId,
    },
    InstanceClosed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalInstanceError {
    Closed,
    /// The guest was discarded and can never resume.
    Discarded,
    UnboundImports {
        missing: Vec<BoundaryMachineId>,
    },
    UnboundExternalCompletions {
        missing: Vec<crate::engine::capabilities::TerminalExternalCompletionDemand>,
    },
    Synchronization(psi_provider_selections::PrivateWaitFailure),
    Execution(TerminalInterpretError),
}

impl TerminalInstanceError {
    /// Follow the interpreter's operation context to the resource the host refused.
    pub fn sponsor_exhaustion(&self) -> Option<&str> {
        let Self::Execution(error) = self else {
            return None;
        };
        let mut current = error;
        loop {
            match current {
                TerminalInterpretError::ExecutionSite { cause, .. }
                | TerminalInterpretError::StructuralReplacement { cause, .. } => {
                    current = cause;
                }
                TerminalInterpretError::EffectRejected { rejection, .. } => {
                    return rejection.exhaustion_reason();
                }
                _ => return None,
            }
        }
    }
}

impl std::fmt::Display for TerminalInstanceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Execution(reason) => reason.fmt(formatter),
            Self::Synchronization(reason) => reason.fmt(formatter),
            other => write!(formatter, "{other:?}"),
        }
    }
}

impl std::error::Error for TerminalInstanceError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalInstanceCloseError {
    Busy,
}
