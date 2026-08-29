use super::scalar::LegalizedFunction;
use super::shared::*;
use super::structural::LegalizedStructuralUnitFunction;

use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LegalizedOperationPlanIdentity([u8; 32]);

impl LegalizedOperationPlanIdentity {
    pub fn from_canonical_bytes(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Dense function-local identity for a value introduced by target
/// legalization rather than Terminal Psi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LegalizedTemporaryId(pub u32);

/// Closed semantic theorem that authorizes a non-identity legalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegalizationTheorem {
    /// For unsigned exact addition with a discharged narrow overflow
    /// obligation, zero-extension commutes with addition.
    UnsignedExactAddCommutesWithWidenV1,
    /// For unsigned exact subtraction with a discharged narrow underflow
    /// obligation, zero-extension commutes with subtraction while preserving
    /// the authored operand order.
    UnsignedExactSubtractCommutesWithWidenV1,
}

/// The closed V4 legality recipe admitted for one function.
///
/// The original recipes are identity legalizations. The widened-u8 recipes
/// are closed non-identity transformations with explicit theorem, temporary,
/// source-operation, proof, and fuel custody.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegalizationRecipe {
    ReturnU64ImmediateConditionalV1,
    ReturnU64EntryParameterConditionalV1,
    ReturnU64ExactAddImmediateConditionalV1,
    ReturnU64ExactSubtractImmediateConditionalV1,
    ReturnU64WidenedU8ExactAddImmediateConditionalV1,
    ReturnU64WidenedU8ExactSubtractImmediateConditionalV1,
    /// The true leaf materializes `r`, `a`, and `b`, then computes the exact
    /// chain `(r + (r + (a + b)))`; the false leaf returns one immediate.
    ReturnU64ActiveResidentExactAddChainConditionalV1,
}

/// Closed identity legalization admitted for a value-less Unit function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UnitLegalizationRecipe {
    ReturnUnitV1,
}

/// Closed structural-Unit legalization forms admitted by the mandatory stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralUnitLegalizationRecipe {
    ReturnUnitV1,
    AuthoredCallThenReturnUnitV1,
    InstalledProviderCallThenReturnUnitV1,
    ClaimCompletionSettlementsThenReturnUnitV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedOperationPlan {
    pub psi: TerminalPsiIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub target: NativeTarget,
    pub entry: MachineId,
    pub functions: Vec<LegalizedFunction>,
    /// Exact straight-line Unit functions admitted independently from the
    /// scalar conditional recipe inventory. Keeping this roster distinct
    /// prevents a value-less return from acquiring a fabricated scalar leaf.
    pub unit_functions: Vec<LegalizedUnitFunction>,
    /// Exact structural-call Unit functions. This roster is deliberately
    /// distinct from `unit_functions`: accepting a structural signature in
    /// the value-less baseline would erase its ABI and ownership transfer.
    pub structural_unit_functions: Vec<LegalizedStructuralUnitFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedUnitFunction {
    pub machine: MachineId,
    pub attachment: Option<psi_core::StructuralTypeId>,
    pub provenance: TerminalPsiProvenance,
    pub recipe: UnitLegalizationRecipe,
    pub entry_block: BlockId,
    pub return_edge: EdgeId,
    pub return_fuel: Vec<FuelSettlement>,
}
