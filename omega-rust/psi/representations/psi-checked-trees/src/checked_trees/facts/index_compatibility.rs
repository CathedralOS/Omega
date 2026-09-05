use psi_facts::{FactHandle, ProgramPoint};
use psi_language_semantics::SemanticDomainId;
use psi_symbols::SymbolHandle;
use psi_typed_trees::expression::ExpressionHandle;
use psi_typed_trees::types::TypeReferenceHandle;

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
    EstablishedLocalFacts {
        /// Exact facts in the semantic flow context that discharged each
        /// unequal member of the index pack. Evidence never rewrites either
        /// indexed-domain instance.
        facts: Vec<FactHandle>,
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
