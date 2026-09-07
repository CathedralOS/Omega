//! legality in the legalized operations program.

/// Checked exact arithmetic. The instruction retains its semantic integer type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegalizedExactIntegerOperator {
    Add,
    Subtract,
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
