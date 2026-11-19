//! Provenance and semantic obligations carried out of native planning.
use optimization_core::{OptimizationUnitIdentity, OptimizedAbstractPlanProjectionIdentity};
use semantic_vocabulary::{MachineId, OperationId, PlaceId, StructuralTypeId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmissionProgramFacts {
    pub psi: terminal_psi::TerminalPsiIdentity,
    pub entry: MachineId,
    pub functions: Vec<EmissionFunctionFacts>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmissionFunctionFacts {
    pub machine: MachineId,
    pub attachment: Option<StructuralTypeId>,
    pub parameters: Vec<operation_operands::values::AbstractParameter>,
    pub structural_parameters: Vec<terminal_psi::StructuralParameterDeclaration>,
    pub result: operation_operands::values::AbstractFunctionResult,
    pub entry_claims: Vec<terminal_psi::EntryClaim>,
    pub source_occurrences: Vec<EmissionOperationProvenance>,
    pub requires_graph_storage_replay: bool,
    pub descriptors: Vec<DescriptorProducerFact>,
    pub primitive_locals: Vec<PrimitiveLocalFact>,
    pub scalar_call_results: Vec<ScalarCallResultFact>,
    pub ram_obligations: Vec<RamObligation>,
}

/// One original ordinal, including operations without a source operation ID.
/// These coordinates carry attribution; they cannot reconstruct executable code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmissionOperationProvenance {
    pub operation: Option<OperationId>,
    pub boundary: Option<(
        semantic_vocabulary::BoundaryMachineId,
        semantic_vocabulary::InvocationSite,
    )>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptorProducerFact {
    pub operation: OperationId,
    pub descriptor: DescriptorProducer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DescriptorProducer {
    ByteSubslice(terminal_psi::StructuralOperationResult),
    ElementView(terminal_psi::StructuralOperationResult),
    ByteLiteral {
        place: PlaceId,
        structural_type: StructuralTypeId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimitiveLocalFact {
    pub operation: OperationId,
    pub result: terminal_psi::StructuralOperationResult,
    pub initializer: operation_operands::values::AbstractResult,
}

impl EmissionFunctionFacts {
    pub fn primitive_local(&self, place: PlaceId) -> Option<&PrimitiveLocalFact> {
        let mut matching = self
            .primitive_locals
            .iter()
            .filter(|row| row.result.place == place);
        let result = matching.next()?;
        matching.next().is_none().then_some(result)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarCallResultFact {
    pub operation: OperationId,
    pub target: ScalarCallOrigin,
    pub result: operation_operands::values::AbstractResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarCallOrigin {
    Authored(operation_operands::calls::AbstractCallTarget),
    Boundary(semantic_vocabulary::BoundaryMachineId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RamObligation {
    pub operation: Option<OperationId>,
    pub source: RamSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RamSource {
    PlacedBacking,
    Boundary(OperationId),
    ExternalCompletion(semantic_vocabulary::EdgeId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizedProgramProjection {
    pub identity: OptimizedAbstractPlanProjectionIdentity,
    pub terminal: terminal_psi::TerminalPsiIdentity,
    pub final_unit: OptimizationUnitIdentity,
}
