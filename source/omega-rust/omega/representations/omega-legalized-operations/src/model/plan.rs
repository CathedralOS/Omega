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
    /// The true leaf keeps `b` live across the first resident use:
    /// `r + (b + (r + (a + b)))`; the false leaf returns one immediate.
    ReturnU64ActiveResidentExactAddBridgeChainConditionalV1,
    /// The true leaf retains the fork/join graph required for an eligible
    /// original epoch-two victim:
    /// `r + ((r + (a + b)) + (b + r))`; the false leaf returns one immediate.
    ReturnU64ActiveResidentExactAddOriginalVictimChainConditionalV1,
    /// Equality of two ordered U64 entry parameters controls two immediate
    /// U64 return arms.
    ReturnU64IntegerEqualParametersConditionalV1,
    /// Strict unsigned ordering of two ordered U64 entry parameters controls
    /// two immediate U64 return arms.
    ReturnU64IntegerLessThanParametersConditionalV1,
    /// Inclusive unsigned ordering of two ordered U64 entry parameters
    /// controls two immediate U64 return arms.
    ReturnU64IntegerLessOrEqualParametersConditionalV1,
    /// Inequality of two ordered U64 entry parameters, authored as exact
    /// integer equality followed by Boolean negation, controls two immediate
    /// U64 return arms.
    ReturnU64IntegerNotEqualParametersConditionalV1,
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

/// Closed identity legalization for the first result-bearing structural ABI
/// family. This recipe retains authority; it does not select instructions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProjectedStructuralCallReturnLegalizationRecipe {
    OwnedLinearDirectV1,
}

/// Exact target and optimizer custody for one two-function projected-roster
/// closure. Keeping the pair atomic prevents either local function from
/// acquiring qualification authority without its matching peer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedProjectedStructuralCallReturn {
    pub recipe: ProjectedStructuralCallReturnLegalizationRecipe,
    pub caller: omega_target_operations::TargetFunction,
    pub callee: omega_target_operations::TargetFunction,
    pub caller_entry_block: BlockId,
    pub callee_entry_block: BlockId,
    pub caller_nodes: Vec<LegalizedStructuralNodeCustody>,
    pub callee_nodes: Vec<LegalizedStructuralNodeCustody>,
}

/// Optimizer metadata retained beside an identity-legalized structural node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedStructuralNodeCustody {
    pub fuel: Vec<FuelSettlement>,
    pub effect: EffectLink,
    pub ownership: Vec<OwnershipEvent>,
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
    /// Exact attached-Unit scalar-call bodies. This roster remains separate
    /// from plain Unit so calls, result homes, and ABI placement cannot be
    /// projected away by the value-less baseline.
    pub scalar_call_unit_functions: Vec<super::scalar_call_unit::LegalizedScalarCallUnitFunction>,
    /// Exact structural-call Unit functions. This roster is deliberately
    /// distinct from `unit_functions`: accepting a structural signature in
    /// the value-less baseline would erase its ABI and ownership transfer.
    pub structural_unit_functions: Vec<LegalizedStructuralUnitFunction>,
    /// Atomic result-bearing structural call/return closures. Instruction
    /// selection intentionally has no consumer for this roster yet.
    pub projected_structural_call_returns: Vec<LegalizedProjectedStructuralCallReturn>,
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
