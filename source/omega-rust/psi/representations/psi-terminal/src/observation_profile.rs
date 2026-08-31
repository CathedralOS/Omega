//! Typed static observation profiles reconstructed from Terminal semantics.

use psi_core::{BlockId, EdgeId, MachineId, ScalarType, StructuralDomainId, StructuralTypeId};

use crate::{CrashCause, StructuralAccess, StructuralMultiplicity, TerminalPsiIdentity};

/// The closed consumer-selected observation schema understood by this build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalObservationSchema {
    TerminalTraceV1,
}

impl TerminalObservationSchema {
    pub const fn version(self) -> u16 {
        match self {
            Self::TerminalTraceV1 => 1,
        }
    }

    pub const fn from_version(version: u16) -> Option<Self> {
        match version {
            1 => Some(Self::TerminalTraceV1),
            _ => None,
        }
    }
}

/// Version-1 compares semantic values exactly; fingerprints and native bytes
/// are never substitute comparison relations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalTraceValueComparison {
    ExactSemanticValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalTraceScalarSchema {
    pub scalar_type: ScalarType,
    pub comparison: TerminalTraceValueComparison,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalTraceStructuralSchema {
    pub structural_type: StructuralTypeId,
    pub multiplicity: StructuralMultiplicity,
    pub access: StructuralAccess,
    pub qualifications: Vec<StructuralDomainId>,
    pub comparison: TerminalTraceValueComparison,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalTraceResultSchema {
    Unit,
    Scalar(TerminalTraceScalarSchema),
    Structural(TerminalTraceStructuralSchema),
}

/// The mandatory nonempty root row of every version-1 profile instance.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalTraceRootRow {
    pub entry: MachineId,
    pub scalar_inputs: Vec<TerminalTraceScalarSchema>,
    pub structural_inputs: Vec<TerminalTraceStructuralSchema>,
    pub result: TerminalTraceResultSchema,
}

/// One exact semantic crash site. Site coordinates are correspondence
/// coordinates and are not themselves user-visible runtime trace values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalTraceCrashSiteRow {
    pub machine: MachineId,
    pub block: BlockId,
    pub edge: EdgeId,
    pub cause: CrashCause,
}

/// Verifier-derived rows before the canonical codec binds the module identity.
/// This is not an independently reusable observation-profile instance.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalTraceV1Rows {
    pub root: TerminalTraceRootRow,
    pub crash_sites: Vec<TerminalTraceCrashSiteRow>,
}

/// First bounded `TerminalTraceV1` instance.
///
/// This rung is deliberately internal-only: its canonical codec includes zero
/// ordinary-event and terminal-external row counts, and reconstruction rejects
/// every operation that version 1 classifies into either later group. The root
/// and complete crash-site roster are already the normative D39 rows.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TerminalTraceV1Profile {
    pub schema: TerminalObservationSchema,
    pub module_identity: TerminalPsiIdentity,
    pub root: TerminalTraceRootRow,
    pub crash_sites: Vec<TerminalTraceCrashSiteRow>,
}
