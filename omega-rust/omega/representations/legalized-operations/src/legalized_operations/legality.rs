//! legality in the legalized operations program.

/// Checked exact arithmetic. The instruction retains its semantic integer type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegalizedExactIntegerOperator {
    Add,
    Subtract,
}

/// Closed identity legalization for the first result-bearing structural ABI
/// family. This recipe retains authority; it does not select instructions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProjectedStructuralCallReturnLegalizationRecipe {
    OwnedLinearDirectV1,
}
