//! Target functions and their declared ABI and semantic provenance.

use crate::{
    MixedStructuralScalarFunctionAbi, ScalarFunctionAbi, TargetControlGraph, TerminalPsiProvenance,
};
use semantic_vocabulary::{MachineId, StructuralTypeId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetFunction {
    pub machine: MachineId,
    /// Exact terminal attachment retained for artifact-side nominal cleanup validation.
    pub attachment: Option<StructuralTypeId>,
    /// Exact owner-designated cleanup subject, independent of attachment or call sites.
    pub nominal_cleanup_subject: Option<StructuralTypeId>,
    /// Canonical native ABI for supported fixed-width scalar inputs and result.
    /// Service reach is verified separately and does not change this layout.
    /// Other function shapes carry no scalar ABI claim.
    pub scalar_abi: Option<ScalarFunctionAbi>,
    /// Canonical ABI for the bounded scalar-result family whose ordered inputs
    /// have a fixed-integer prefix and structural suffix. Results may be a
    /// fixed integer or Boolean.
    pub mixed_structural_scalar_abi: Option<MixedStructuralScalarFunctionAbi>,
    pub provenance: TerminalPsiProvenance,
    pub graph: TargetControlGraph,
}
