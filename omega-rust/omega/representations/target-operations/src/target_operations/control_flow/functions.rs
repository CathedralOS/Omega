//! Target functions and their declared ABI and semantic provenance.

use crate::{
    FixedIntegerScalarFunctionAbi, MixedStructuralScalarFunctionAbi, TargetOperation,
    TerminalPsiProvenance,
};
use semantic_vocabulary::{MachineId, StructuralTypeId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetFunction {
    pub machine: MachineId,
    /// Exact terminal attachment retained for artifact-side nominal cleanup validation.
    pub attachment: Option<StructuralTypeId>,
    /// Canonical native ABI for the deliberately bounded service-free,
    /// fixed-integer scalar function family. Other function shapes carry no
    /// scalar ABI claim.
    pub fixed_integer_scalar_abi: Option<FixedIntegerScalarFunctionAbi>,
    /// Canonical ABI for the bounded scalar-result family whose ordered inputs
    /// have a fixed-integer prefix and structural suffix. Results may be a
    /// fixed integer or Boolean.
    pub mixed_structural_scalar_abi: Option<MixedStructuralScalarFunctionAbi>,
    pub provenance: TerminalPsiProvenance,
    pub operation: TargetOperation,
}
