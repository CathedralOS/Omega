/*
We define the control-flow, SSA value, effect-sequencing, logical-work, and
cyclic-component carriers that make up a `PsiOptimizationFunction`. Every block
and node retains its exact source provenance (`PsiProvenance`), logical work
settlements (`WorkSettlement`), and ownership events (`OwnershipEvent`) alongside
the canonical identity encoding for `ValueDefinitionSite` and the unsigned
countdown ranking certificates used by loop analyses.
*/

use super::MachineId;
use super::{
    AbstractFunctionResult, AbstractOperation, BTreeSet, BlockId, ClaimId, ContentEntryClaim,
    EdgeId, EntryClaim, EvidenceContractLane, IntegerType, IntegerValue, MachineContract,
    ObligationId, OperationId, PlaceId, ScalarType, ServiceId, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralTypeId, TerminalAffineCleanupAction, ValueBinding,
    ValueId,
};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkSettlement {
    pub site: PsiProvenance,
    pub units: u64,
}

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
    pub cleanup_actions: Vec<TerminalAffineCleanupAction>,
    pub provenance: Vec<PsiProvenance>,
    pub work: Vec<WorkSettlement>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizationFact {
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
    pub provenance: Vec<PsiProvenance>,
    pub work: Vec<WorkSettlement>,
    pub effect: EffectLink,
    pub definitions: Vec<ValueDefinition>,
    pub uses: Vec<ValueUse>,
    pub successors: Vec<OptimizationEdge>,
    pub ownership: Vec<OwnershipEvent>,
}

macro_rules! structural_parameter_accessor {
    ($name:ty) => {
        impl $name {
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
    pub fn definitions(&self) -> impl Iterator<Item = &ValueDefinition> + '_ {
        self.nodes.iter().flat_map(|node| &node.definitions)
    }

    pub fn uses(&self) -> impl Iterator<Item = &ValueUse> + '_ {
        self.nodes.iter().flat_map(|node| &node.uses)
    }

    pub fn successors(&self) -> impl Iterator<Item = &OptimizationEdge> + '_ {
        self.nodes.iter().flat_map(|node| &node.successors)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiOptimizationFunction {
    pub machine: MachineId,
    pub attachment: Option<StructuralTypeId>,
    pub nominal_cleanup_subject: Option<StructuralTypeId>,
    pub entry: BlockId,
    pub parameters: Vec<ValueDefinition>,
    pub structural_parameters: Vec<StructuralParameterDeclaration>,
    pub structural_places: Vec<StructuralPlaceDeclaration>,
    pub result: AbstractFunctionResult,
    pub declared_places: BTreeSet<PlaceId>,
    pub entry_claim_declarations: Vec<EntryClaim>,
    pub block_entry_claims: Vec<EntryClaim>,
    pub content_entry_claims: Vec<ContentEntryClaim>,
    pub verified_contract: Option<MachineContract>,
    pub evidence_contract_lanes: Vec<EvidenceContractLane>,
    pub entry_claims: BTreeSet<ClaimId>,
    pub published_service_ceiling: Vec<ServiceId>,
    pub declared_service_reach: Vec<ServiceId>,
    pub facts: Vec<OptimizationFact>,
    pub blocks: Vec<OptimizationBlock>,
}

structural_parameter_accessor!(PsiOptimizationFunction);

impl PsiOptimizationFunction {
    pub fn block(&self, block: BlockId) -> Option<&OptimizationBlock> {
        self.block_index(block).map(|index| &self.blocks[index])
    }

    pub fn block_mut(&mut self, block: BlockId) -> Option<&mut OptimizationBlock> {
        self.block_index(block).map(|index| &mut self.blocks[index])
    }

    pub fn structural_place(&self, place: PlaceId) -> Option<&StructuralPlaceDeclaration> {
        self.structural_places.iter().find(|row| row.id == place)
    }

    pub fn entry_claim_declared(&self, input: PlaceId) -> bool {
        self.entry_claim_declarations
            .iter()
            .any(|claim| claim.input == input)
    }

    pub fn content_entry_claim_declared(&self, root: PlaceId) -> bool {
        self.content_entry_claims
            .iter()
            .any(|claim| claim.input.root == root)
    }

    pub fn block_index(&self, id: BlockId) -> Option<usize> {
        self.blocks.binary_search_by_key(&id, |row| row.id).ok()
    }

    pub fn block_nodes(&self) -> impl Iterator<Item = &OptimizationNode> + '_ {
        self.blocks.iter().flat_map(|block| &block.nodes)
    }

    pub fn node_definitions(&self) -> impl Iterator<Item = &ValueDefinition> + '_ {
        self.block_nodes().flat_map(|node| &node.definitions)
    }

    pub fn node_uses(&self) -> impl Iterator<Item = &ValueUse> + '_ {
        self.block_nodes().flat_map(|node| &node.uses)
    }

    pub fn block_edges(&self) -> impl Iterator<Item = &OptimizationEdge> + '_ {
        self.block_nodes().flat_map(|node| &node.successors)
    }

    pub fn block_parameters(&self) -> impl Iterator<Item = &ValueDefinition> + '_ {
        self.blocks.iter().flat_map(|block| &block.parameters)
    }

    pub fn block_structural_parameters(
        &self,
    ) -> impl Iterator<Item = &StructuralParameterDeclaration> + '_ {
        self.blocks
            .iter()
            .flat_map(|block| &block.structural_parameters)
    }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CycleComponentEdge {
    pub edge: EdgeId,
    pub source: BlockId,
    pub target: BlockId,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CycleComponentId {
    pub machine: MachineId,
    pub internal_edges: Vec<CycleComponentEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizerCycleComponent {
    pub id: CycleComponentId,
    pub members: Vec<BlockId>,
    pub entries: Vec<CycleComponentEdge>,
    pub exits: Vec<CycleComponentEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizerCycleComponentSnapshot {
    pub terminal_psi: terminal_psi::TerminalPsiIdentity,
    pub components: Vec<OptimizerCycleComponent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizerUnsignedCountdownRankingCertificate {
    pub component: CycleComponentId,
    pub header: BlockId,
    pub rank_parameter: ValueId,
    pub rank_type: IntegerType,
    pub lower_bound: IntegerValue,
    pub upper_bound: IntegerValue,
    pub guard: OptimizerUnsignedPositiveGuard,
    pub descent: OptimizerUnsignedMinusOneDescent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizerUnsignedPositiveGuard {
    pub block: BlockId,
    pub edge: EdgeId,
    pub condition: ValueId,
    pub parameter: ValueId,
    pub zero: ValueId,
    pub zero_operation: OperationId,
    pub comparison_operation: OperationId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizerUnsignedMinusOneDescent {
    pub backedge: CycleComponentEdge,
    pub argument_index: u32,
    pub argument: ValueId,
    pub source_parameter: ValueId,
    pub target_parameter: ValueId,
    pub one: ValueId,
    pub one_operation: OperationId,
    pub subtract_operation: OperationId,
    pub subtract_obligation: semantic_vocabulary::ObligationId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizerRankingCertificateSnapshot {
    pub terminal_psi: terminal_psi::TerminalPsiIdentity,
    pub certificates: Vec<OptimizerUnsignedCountdownRankingCertificate>,
}
