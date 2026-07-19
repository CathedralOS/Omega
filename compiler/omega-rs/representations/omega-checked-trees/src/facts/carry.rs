//! Checker-owned carry facts. The declaration keeps the authored minimum;
//! this plan records the effective policy derived from the complete stored
//! shape so later liveness, runtime-admission, artifact, and model-export
//! passes never need to reinterpret source syntax.

use omega_core::semantics::CarryPolicy;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CarryFacts {
    /// One entry per data declaration, in declaration order.
    pub data: Vec<DataCarryFact>,
}

impl CarryFacts {
    pub fn for_data(&self, data: SymbolHandle) -> Option<&DataCarryFact> {
        self.data.iter().find(|fact| fact.data == data)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataCarryFact {
    pub data: SymbolHandle,
    /// The optional authored minimum promise retained for diagnostics and
    /// published-contract work. It is not the effective policy.
    pub declared: Option<CarryPolicy>,
    /// The checker-derived policy for this transparent stored shape.
    pub effective: CarryPolicy,
}
