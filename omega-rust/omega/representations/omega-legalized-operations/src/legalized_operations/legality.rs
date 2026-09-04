//! legality in the legalized operations program.

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
    /// Strict signed ordering of two ordered I64 entry parameters controls
    /// two immediate U64 return arms.
    ReturnU64I64LessThanParametersConditionalV1,
    /// Inclusive signed ordering of two ordered I64 entry parameters controls
    /// two immediate U64 return arms.
    ReturnU64I64LessOrEqualParametersConditionalV1,
    /// Equality of one U64 entry parameter with an exact authored U64 zero
    /// controls two immediate U64 return arms.
    ReturnU64EqualZeroParameterConditionalV1,
    /// Inequality of one U64 entry parameter with an exact authored U64 zero,
    /// expressed as equality followed by Boolean negation, controls two
    /// immediate U64 return arms.
    ReturnU64NotEqualZeroParameterConditionalV1,
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
