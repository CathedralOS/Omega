use omega_core::semantics::SemanticDomainId;
use omega_core::symbols::SymbolHandle;
use omega_facts::{FactHandle, ProgramPoint};
use omega_typed_trees::expression::ExpressionHandle;
use omega_typed_trees::types::TypeReferenceHandle;

/// The only three PDI3 routes that may discharge equality between an actual
/// proof-static index and the index required at a value-flow boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexCompatibilityDischarge {
    ClosedEvaluation,
    LicensedNormalization {
        /// Number of exact selected operation nodes that participated. Zero
        /// denotes direct canonical identity of the same open binder/value.
        operation_count: usize,
    },
    EstablishedLocalFact {
        /// Exact fact in the semantic flow context that discharged the named
        /// condition. It is evidence only and never rewrites either instance.
        fact: FactHandle,
    },
}

/// One named verification condition produced by using a proof-static indexed
/// value where another indexed instance is expected. These rows are checked
/// evidence, deliberately separate from semantic domain/type identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexCompatibilityFact {
    pub name: String,
    pub point: ProgramPoint,
    pub value: ExpressionHandle,
    pub target_type: TypeReferenceHandle,
    pub family: SymbolHandle,
    pub actual_instance: SemanticDomainId,
    pub expected_instance: SemanticDomainId,
    pub actual_label: String,
    pub expected_label: String,
    pub discharge: IndexCompatibilityDischarge,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndexCompatibilityFacts {
    pub conditions: Vec<IndexCompatibilityFact>,
}
