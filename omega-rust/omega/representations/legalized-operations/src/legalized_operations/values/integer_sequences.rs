//! Ordered current U64 definitions, independent of their producer's graph shape.

use crate::LegalizedImmediate;
use optimization_core::AcceptedObligationFactIdentity;
use optimization_unit::{FuelSettlement, ValueDefinitionSite};
use semantic_vocabulary::{ObligationId, OperationId, ValueId};

/// Exact fixed-width unsigned 64-bit arithmetic; no wrapping or widening.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegalizedExactIntegerOperator {
    Add,
    Subtract,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedExactIntegerBinary {
    pub operator: LegalizedExactIntegerOperator,
    pub source_value: ValueId,
    pub obligation: ObligationId,
    pub accepted_fact: AcceptedObligationFactIdentity,
    pub operation: OperationId,
    pub definition_site: ValueDefinitionSite,
    pub fuel: Vec<FuelSettlement>,
    pub left: ValueId,
    pub right: ValueId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegalizedIntegerStep {
    Immediate(LegalizedImmediate),
    ExactBinary(LegalizedExactIntegerBinary),
}

/// A topologically ordered sequence whose inputs refer to existing ABI values.
/// The enclosing leaf identifies its result, which need not be the last step.
/// These public data retain evidence but grant no admission authority.
/// One contiguous row allocation owns this leaf's program. References between
/// rows use semantic ValueId, not offsets or nested owned expressions. An arena
/// conversion needs a program-wide storage owner and an honest absent cell;
/// no dummy arithmetic instruction is introduced merely to satisfy Default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedExactIntegerSequence {
    pub steps: Vec<LegalizedIntegerStep>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegalizedIntegerSequenceError {
    DuplicateValue(ValueId),
    UnavailableValue(ValueId),
    DuplicateOperation(OperationId),
    DuplicateDefinitionSite(ValueDefinitionSite),
    NonNodeDefinition(ValueDefinitionSite),
    NonU64Immediate(ValueId),
}
