//! Executable CFG, value-flow, effect, fuel, and ownership-event carriers.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PsiProvenance {
    Operation(OperationId),
    Edge(EdgeId),
}

/// One source logical-fuel settlement. Native lowering must retain this even
/// when several source nodes become one physical instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FuelSettlement {
    pub site: PsiProvenance,
    pub units: u64,
}

/// A conservative semantic sequencing token. Initially every node is chained;
/// analyses may later prove selected scalar nodes independent without erasing
/// the source order represented here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EffectLink {
    pub input: u64,
    pub output: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ValueDefinitionSite {
    FunctionParameter(u32),
    BlockParameter { block: BlockId, position: u32 },
    Node { block: BlockId, node: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueDefinition {
    pub value: ValueId,
    pub scalar_type: ScalarType,
    pub site: ValueDefinitionSite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueUse {
    pub value: ValueId,
    pub block: BlockId,
    pub node: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizationEdge {
    pub psi_edge: EdgeId,
    pub target: BlockId,
    pub bindings: Vec<ValueBinding>,
    /// Exact ordered affine discard work executed on this edge.
    pub trivial_affine_discards: Vec<PlaceId>,
    /// Ordered source custody charged only when this exact CFG edge is taken.
    /// The edge's own Psi identity is first; independently validated rewrites
    /// may append inherited edge sources that execute on the same path.
    pub provenance: Vec<PsiProvenance>,
    /// One ordered settlement per edge provenance source.
    pub fuel: Vec<FuelSettlement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnershipEvent {
    ClaimTransfer(Vec<ClaimId>),
    ClaimCompletion(Vec<ClaimId>),
    Cleanup(Vec<TerminalAffineCleanupAction>),
    StructuralReturn(Vec<ClaimId>),
    CrashFrontier(Vec<ClaimId>),
}

/// A proof/range fact is always indexed by its exact source support. The first
/// builder only emits facts reconstructed directly from literal operations;
/// proof-derived facts remain absent (and therefore unavailable to rules)
/// until their verified evidence is retained across the lowering boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizationFact {
    /// A proof-bearing operation's obligation lookup key. This reference is
    /// not itself accepted evidence: publication must resolve it against the
    /// verifier-owned context for the immutable Terminal Psi artifact.
    OperationObligationReference {
        obligation: ObligationId,
        support: OperationId,
    },
    BooleanConstant {
        value: ValueId,
        constant: bool,
        support: OperationId,
    },
    IntegerConstant {
        value: ValueId,
        constant: IntegerValue,
        support: OperationId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizationNode {
    pub operation: AbstractOperation,
    /// Ordered logical source custody. Normally this is the operation's exact
    /// source roster. A validator-authorized unconditional Jump fusion keeps
    /// its own edge first and may append only co-executed inherited edges.
    pub provenance: Vec<PsiProvenance>,
    /// One ordered settlement per provenance source, in the same order.
    pub fuel: Vec<FuelSettlement>,
    pub effect: EffectLink,
    pub definitions: Vec<ValueDefinition>,
    pub uses: Vec<ValueUse>,
    pub successors: Vec<OptimizationEdge>,
    pub ownership: Vec<OwnershipEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizationBlock {
    pub id: BlockId,
    pub parameters: Vec<ValueDefinition>,
    pub nodes: Vec<OptimizationNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiOptimizationFunction {
    pub machine: MachineId,
    /// Exact nominal receiver attachment from the verified Terminal-Psi
    /// signature. Optimization may inspect but never rewrite this identity.
    pub attachment: Option<StructuralTypeId>,
    pub entry: BlockId,
    pub parameters: Vec<ValueDefinition>,
    pub structural_parameters: Vec<StructuralParameterDeclaration>,
    /// Complete verifier-owned structural-place roster, including each root's
    /// exact role, producer, and concrete structural type.
    pub structural_places: Vec<StructuralPlaceDeclaration>,
    /// Exact normal result signature retained independently of executable
    /// return nodes, including Unit and structural-result distinctions.
    pub result: AbstractFunctionResult,
    pub declared_places: BTreeSet<PlaceId>,
    /// Full ordered caller/root claim signature. `entry_claims` below is the
    /// independently checked membership index used by ownership validation.
    pub entry_claim_declarations: Vec<EntryClaim>,
    /// Complete verifier-owned content-claim signature. Content projection
    /// authority cannot be reconstructed from ordinary claims alone.
    pub content_entry_claims: Vec<ContentEntryClaim>,
    /// Complete verifier-owned machine contract. Bare reconstruction seeds do
    /// not carry verifier authority and therefore leave this absent.
    pub verified_contract: Option<MachineContract>,
    /// Complete module evidence-contract roster for this machine. Its absence
    /// is semantically meaningful to exact payloadless-call classification.
    pub evidence_contract_lanes: Vec<EvidenceContractLane>,
    pub entry_claims: BTreeSet<ClaimId>,
    /// Exact verifier-normalized service ceiling in canonical Terminal-Psi
    /// order. It is semantic custody, not an optimizer-selected reach set.
    pub published_service_ceiling: Vec<ServiceId>,
    pub facts: Vec<OptimizationFact>,
    pub blocks: Vec<OptimizationBlock>,
}
