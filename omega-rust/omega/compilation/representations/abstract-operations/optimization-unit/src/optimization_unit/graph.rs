//! Executable CFG, value-flow, effect, fuel, and ownership-event carriers.

use super::super::{
    AbstractFunctionResult, AbstractOperation, BTreeSet, BlockId, ClaimId, ContentEntryClaim,
    EdgeId, EntryClaim, EvidenceContractLane, IntegerValue, MachineContract, ObligationId,
    OperationId, PlaceId, ScalarType, ServiceId, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralTypeId, TerminalAffineCleanupAction, ValueBinding,
    ValueId,
};
use super::MachineId;
use semantic_vocabulary::StructuralPlaceKind;
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PsiProvenance {
    Operation(OperationId),
    Edge(EdgeId),
}

impl From<OperationId> for PsiProvenance {
    fn from(operation: OperationId) -> Self {
        Self::Operation(operation)
    }
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

/// The canonical identity encoding of [`ValueDefinitionSite`].
///
/// The variant tags written here — `0` for `FunctionParameter`, `1` for
/// `BlockParameter`, `2` for `Node` — and the little-endian field order that
/// follows each tag *are* the identity: selected-instruction plan identities,
/// register-home recovery plans, legalized-operation scalar identities and the
/// replay evidence derived from them all hash exactly these bytes. Changing a
/// tag or reordering a field changes every artifact that embeds a definition
/// site.
///
/// Consumers must call this function rather than repeat the table. A second
/// copy is how two crates that are supposed to agree drift apart silently: the
/// encoding stays compilable, each side keeps producing bytes, and only a
/// mismatched artifact identity reveals it.
pub fn encode_value_definition_site_identity(bytes: &mut Vec<u8>, site: ValueDefinitionSite) {
    match site {
        ValueDefinitionSite::FunctionParameter(position) => {
            bytes.push(0);
            crate::codec_bytes::push_encoded(bytes, position.to_le_bytes());
        }
        ValueDefinitionSite::BlockParameter { block, position } => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, block.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, position.to_le_bytes());
        }
        ValueDefinitionSite::Node { block, node } => {
            bytes.push(2);
            crate::codec_bytes::push_encoded(bytes, block.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, node.to_le_bytes());
        }
    }
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
    pub structural_bindings: Vec<abstract_operations::AbstractStructuralBinding>,
    /// Exact ordered cleanup performed after outgoing capture and before
    /// successor entry, including executable hooks and no-code dispositions.
    pub cleanup_actions: Vec<TerminalAffineCleanupAction>,
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
    ClaimCompletion(Vec<terminal_psi::CompletionReceipt>),
    Cleanup(Vec<TerminalAffineCleanupAction>),
    StructuralReturn(Vec<ClaimId>),
    CrashFrontier(Vec<ClaimId>),
    ExternalCompletionFrontier(Vec<ClaimId>),
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

/// Blocks and functions answer the same structural-parameter lookup:
/// `find_by_place` over their declared `structural_parameters`. One row per
/// owner expands that accessor.
macro_rules! structural_parameter_accessor {
    ($name:ty) => {
        impl $name {
            /// The structural parameter arriving at `place`, when declared.
            pub fn structural_parameter(
                &self,
                place: PlaceId,
            ) -> Option<&StructuralParameterDeclaration> {
                StructuralParameterDeclaration::find_by_place(&self.structural_parameters, place)
            }
        }
    };
}

impl OptimizationEdge {
    pub fn binding(&self, parameter: ValueId) -> Option<&ValueBinding> {
        self.bindings
            .iter()
            .find(|binding| binding.parameter == parameter)
    }

    /// The structural binding keyed `parameter`, when the edge declares it.
    pub fn structural_binding(
        &self,
        parameter: PlaceId,
    ) -> Option<&abstract_operations::AbstractStructuralBinding> {
        self.structural_bindings
            .iter()
            .find(|binding| binding.parameter == parameter)
    }
}

impl OptimizationNode {
    pub fn edge(&self, psi_edge: EdgeId) -> Option<&OptimizationEdge> {
        self.successors
            .iter()
            .find(|edge| edge.psi_edge == psi_edge)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizationBlock {
    pub id: BlockId,
    pub structural_parameters: Vec<StructuralParameterDeclaration>,
    pub parameters: Vec<ValueDefinition>,
    pub nodes: Vec<OptimizationNode>,
}

structural_parameter_accessor!(OptimizationBlock);

impl OptimizationBlock {
    /// Every node's value definitions, in node order.
    pub fn definitions(&self) -> impl Iterator<Item = &ValueDefinition> + '_ {
        self.nodes.iter().flat_map(|node| &node.definitions)
    }

    /// Every node's value uses, in node order.
    pub fn uses(&self) -> impl Iterator<Item = &ValueUse> + '_ {
        self.nodes.iter().flat_map(|node| &node.uses)
    }

    /// Every node's outgoing edges, in node order.
    pub fn successors(&self) -> impl Iterator<Item = &OptimizationEdge> + '_ {
        self.nodes.iter().flat_map(|node| &node.successors)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiOptimizationFunction {
    pub machine: MachineId,
    /// Exact nominal receiver attachment from the verified Terminal-Psi
    /// signature. Optimization may inspect but never rewrite this identity.
    pub attachment: Option<StructuralTypeId>,
    /// Exact owner-designated cleanup subject, independent of attachment or call sites.
    pub nominal_cleanup_subject: Option<StructuralTypeId>,
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
    /// Destination-local custody names, not invocation establishments.
    pub block_entry_claims: Vec<EntryClaim>,
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
    /// The Terminal machine's authored fixed service contribution. Root reach
    /// replay counts it as concrete for every reachable machine, inert bodies
    /// included; bare reconstruction seeds leave it empty.
    pub declared_service_reach: Vec<ServiceId>,
    pub facts: Vec<OptimizationFact>,
    pub blocks: Vec<OptimizationBlock>,
}

structural_parameter_accessor!(PsiOptimizationFunction);

impl PsiOptimizationFunction {
    /// The block keyed `block`, when the function declares it.
    pub fn block(&self, block: BlockId) -> Option<&OptimizationBlock> {
        self.blocks.iter().find(|row| row.id == block)
    }

    /// The block keyed `block`, mutably, when the function declares it.
    pub fn block_mut(&mut self, block: BlockId) -> Option<&mut OptimizationBlock> {
        self.blocks.iter_mut().find(|row| row.id == block)
    }

    /// The structural place keyed `place`, when the function declares it.
    pub fn structural_place(&self, place: PlaceId) -> Option<&StructuralPlaceDeclaration> {
        self.structural_places.iter().find(|row| row.id == place)
    }

    /// The position of the block keyed `id`, when the function declares it.
    pub fn block_index(&self, id: BlockId) -> Option<usize> {
        self.blocks.iter().position(|row| row.id == id)
    }

    /// Every block's nodes, in block order.
    pub fn block_nodes(&self) -> impl Iterator<Item = &OptimizationNode> + '_ {
        self.blocks.iter().flat_map(|block| &block.nodes)
    }

    /// Every node's value definitions, in block then node order.
    pub fn node_definitions(&self) -> impl Iterator<Item = &ValueDefinition> + '_ {
        self.block_nodes().flat_map(|node| &node.definitions)
    }

    /// Every node's value uses, in block then node order.
    pub fn node_uses(&self) -> impl Iterator<Item = &ValueUse> + '_ {
        self.block_nodes().flat_map(|node| &node.uses)
    }

    /// Every node's outgoing edges, in block then node order.
    pub fn block_edges(&self) -> impl Iterator<Item = &OptimizationEdge> + '_ {
        self.block_nodes().flat_map(|node| &node.successors)
    }

    /// Every block's value parameters, in block order.
    pub fn block_parameters(&self) -> impl Iterator<Item = &ValueDefinition> + '_ {
        self.blocks.iter().flat_map(|block| &block.parameters)
    }

    /// Every block's structural parameters, in block order.
    pub fn block_structural_parameters(
        &self,
    ) -> impl Iterator<Item = &StructuralParameterDeclaration> + '_ {
        self.blocks
            .iter()
            .flat_map(|block| &block.structural_parameters)
    }

    /// The parameter declaration a structural place arrives as: this unit's
    /// own parameter at its declared position, or a block parameter in the
    /// block that declares it. Both are storage an operation addresses in
    /// place, unlike an operation result or a literal, neither of which has a
    /// caller-owned home.
    ///
    /// A receiver collapses every state onto the one unit parameter, while a
    /// named data parameter gives each state its own block parameter, so
    /// consulting only `structural_parameters` admits the receiver spelling of
    /// a program and refuses the identical program spelled statically.
    /// Callers still read the returned access, multiplicity and
    /// qualifications themselves; this answers only which storage it is.
    /// Duplicate identities or inconsistent kind/position metadata fail closed.
    pub fn arrival_parameter(&self, place: PlaceId) -> Option<&StructuralParameterDeclaration> {
        let mut declarations = self.structural_places.iter().filter(|row| row.id == place);
        let declaration = declarations.next()?;
        if declarations.next().is_some() {
            return None;
        }
        let mut parameters = self
            .structural_parameters
            .iter()
            .chain(self.block_structural_parameters())
            .filter(|parameter| parameter.place == place);
        let parameter = parameters.next()?;
        if parameters.next().is_some() {
            return None;
        }
        match declaration.kind {
            StructuralPlaceKind::Parameter { position }
                if position == parameter.position
                    && self
                        .structural_parameters
                        .iter()
                        .any(|row| row.place == place) => {}
            StructuralPlaceKind::BlockParameter { block, position }
                if block != self.entry
                    && position == parameter.position
                    && self
                        .block(block)
                        .and_then(|block| block.structural_parameters.get(position as usize))
                        .is_some_and(|row| row.place == place) => {}
            _ => return None,
        }
        Some(parameter)
    }
}
