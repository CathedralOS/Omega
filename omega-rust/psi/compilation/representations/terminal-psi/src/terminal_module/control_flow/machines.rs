use crate::{
    ContentEntryClaim, ContentIdentityReshuffle, ContentPartitionComposition, EntryClaim,
    ErasedProofFormal, MachineContract, Operation, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, TerminalMachineResult, TerminalRankedScc, Terminator,
    ValueDeclaration,
};
use semantic_vocabulary::{BlockId, MachineId, OperationId, PlaceId, ServiceId, StructuralTypeId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalMachine {
    /// Conjunctive catalog architecture demand retained after instruction normalization.
    pub assembly_target_requirement: crate::RegisterTargetApplicability,
    pub id: MachineId,
    /// Exact checked declaration when this is a source-produced body. Compiler
    /// synthetic machines have no authored declaration.
    pub declaration: Option<crate::CheckedMachineDeclaration>,
    /// Nominal type to which this machine is attached. An attached static
    /// machine need not have a runtime `self` parameter.
    pub attachment: Option<StructuralTypeId>,
    /// Exact owner-designated cleanup subject, independent of attachment or call sites.
    pub nominal_cleanup_subject: Option<StructuralTypeId>,
    pub parameters: Vec<ValueDeclaration>,
    /// Authored runtime ordering across scalar and structural lanes. Each tag
    /// consumes the next parameter in that lane; erased formals occupy no tag.
    /// Native bridges read this ordering without reopening checked source.
    pub parameter_order: Vec<crate::BoundaryParameterKind>,
    /// Ordered runtime structural parameters, separate from scalar values.
    pub structural_parameters: Vec<StructuralParameterDeclaration>,
    /// Canonical source-handle-free ranking evidence for the first admitted
    /// cyclic control component. Representation validation reconstructs the
    /// closed identity, guard, successor arithmetic, and exact structural-
    /// frontier preservation fixed point. Execution remains independently
    /// unavailable until interpreter, fuel, and native support land.
    pub ranked_scc: Option<TerminalRankedScc>,
    /// Unit carries no value; scalar results have a stable pseudo-value bound
    /// by every scalar return edge and available to `ensures`.
    pub result: TerminalMachineResult,
    /// Proof-visible roots for structural-place propositions. Runtime scalar
    /// parameters remain independently declared above.
    pub structural_places: Vec<StructuralPlaceDeclaration>,
    /// Dense one-based machine-local claims present at entry, independent of
    /// content projections. Content claims below refine these identities when
    /// present.
    pub entry_claims: Vec<EntryClaim>,
    /// Destination-local claim names for owned block parameters. An arrival
    /// binds its complete active path roster to these names, continuing the
    /// incoming obligations rather than establishing new ones. Parameters
    /// without rows relocate their incoming identities unchanged.
    pub block_entry_claims: Vec<EntryClaim>,
    /// Exact ownership joins of normal exclusive-call completions, independent
    /// of whether the call returns Unit, a scalar or a structural value.
    pub loan_completions: Vec<crate::LoanCompletion>,
    /// Fixed authored service contribution, strictly ordered and parent-closed.
    /// This is a semantic contract input, not an inferred summary: an inert
    /// body retains its declaration, even when the same service also belongs
    /// to an installation-selected requirement's nonconcrete upper bound.
    pub declared_service_reach: Vec<ServiceId>,
    /// Closed source-projected reach contract, independently checked on reload.
    pub closed_reach_application: Option<crate::ClosedReachApplication>,
    /// Strictly ordered normalized executable boundary-service ceiling. Public
    /// checked machines retain their derived published row; private machines
    /// and executable entries retain their exact checked effective reach.
    pub published_service_ceiling: Vec<ServiceId>,
    /// Canonical machine-local identities for claims present at entry. These
    /// rows name content independently of any later output equality.
    pub content_entry_claims: Vec<ContentEntryClaim>,
    /// Canonical one-to-one claim mappings. These are semantic ownership facts,
    /// not authored algebra theorems: each exact projection below yields one
    /// verifier-reconstructed equality between `input` and `output`.
    pub content_identity_reshuffles: Vec<ContentIdentityReshuffle>,
    /// Exact substitutions of already-authored partition theorems. These rows
    /// retain the source theorem and do not permit a producer to introduce a
    /// new `Separate` node in the derived equation.
    pub content_partition_compositions: Vec<ContentPartitionComposition>,
    pub entry: BlockId,
    pub blocks: Vec<Block>,
    pub contract: MachineContract,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub id: BlockId,
    pub parameters: Vec<ValueDeclaration>,
    /// Proof-only erased formals in dense authored order. Each entry's `id`
    /// names one erased scalar parameter of the state inside block-invariant
    /// propositions; it carries no runtime position, storage, or argument
    /// lane. Every incoming edge supplies one erased argument per roster row.
    pub erased_scalar_formals: Vec<ValueDeclaration>,
    /// Erased proof-only formals in dense authored order after the scalar
    /// erased roster. Every incoming edge supplies one `ProofTerm` per row.
    pub erased_proof_formals: Vec<ErasedProofFormal>,
    /// Ordered structural bindings supplied simultaneously by the selected edge.
    /// Positions are dense and zero-based; `is_self` is false. Entry blocks
    /// declare no structural parameters.
    pub structural_parameters: Vec<StructuralParameterDeclaration>,
    pub operations: Vec<Operation>,
    pub terminator: Terminator,
}

impl Block {
    /// A block carrying only its parameters and terminator — the dispatch or
    /// case-block shape with no erased formals, structural parameters, or
    /// operations.
    pub fn plain(id: BlockId, parameters: Vec<ValueDeclaration>, terminator: Terminator) -> Self {
        Self {
            id,
            parameters,
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            operations: Vec::new(),
            terminator,
        }
    }
}

impl TerminalMachine {
    /// Check that the authored order consumes both runtime lanes exactly once.
    pub fn validate_parameter_order(&self) -> Result<(), &'static str> {
        let scalar_count = self
            .parameter_order
            .iter()
            .filter(|kind| **kind == crate::BoundaryParameterKind::Scalar)
            .count();
        let structural_count = self.parameter_order.len() - scalar_count;
        if scalar_count != self.parameters.len()
            || structural_count != self.structural_parameters.len()
        {
            return Err("machine runtime parameter order does not cover its signature");
        }
        Ok(())
    }

    /// Dense runtime formal ordinals in scalar-then-structural storage order.
    /// Erased formals occupy no native lane and therefore no ordinal here.
    pub fn runtime_parameter_formals(&self) -> Result<Vec<u32>, &'static str> {
        self.validate_parameter_order()?;
        let positions = |kind| {
            self.parameter_order
                .iter()
                .enumerate()
                .filter_map(move |(position, candidate)| (*candidate == kind).then_some(position))
        };
        positions(crate::BoundaryParameterKind::Scalar)
            .chain(positions(crate::BoundaryParameterKind::Structural))
            .map(|position| {
                u32::try_from(position)
                    .map_err(|_| "machine runtime formal ordinal cannot be represented")
            })
            .collect()
    }

    /// The block keyed `id`, when the machine declares it.
    pub fn block(&self, id: &BlockId) -> Option<&Block> {
        self.blocks.iter().find(|block| block.id == *id)
    }

    /// Every block's operations, in block order.
    pub fn operations(&self) -> impl Iterator<Item = &Operation> + '_ {
        self.blocks.iter().flat_map(|block| &block.operations)
    }

    /// Every block's value parameters, in block order.
    pub fn block_parameters(&self) -> impl Iterator<Item = &ValueDeclaration> + '_ {
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

    /// The operation with `id`, searched across every block's roster.
    pub fn operation(&self, id: OperationId) -> Option<&Operation> {
        self.operations().find(|operation| operation.id == id)
    }

    /// The structural-place declaration keyed `place`, when the machine
    /// declares it.
    pub fn structural_place(&self, place: PlaceId) -> Option<&StructuralPlaceDeclaration> {
        self.structural_places
            .iter()
            .find(|entry| entry.id == place)
    }

    /// The operation producing `place`, resolved through the structural-place
    /// declaration's producer binding. `None` when the place is not declared
    /// as an operation result.
    pub fn producing_operation(&self, place: PlaceId) -> Option<(&Operation, StructuralTypeId)> {
        let declaration = self.structural_place(place)?;
        let (producer, structural_type) = declaration.kind.as_operation_result()?;
        let operation = self
            .operations()
            .find(|operation| operation.id == producer)?;
        Some((operation, structural_type))
    }

    /// The structural parameter bound to `place`, searched across the machine
    /// roster and every block's bindings.
    pub fn structural_parameter(&self, place: PlaceId) -> Option<&StructuralParameterDeclaration> {
        self.structural_parameters
            .iter()
            .chain(self.block_structural_parameters())
            .find(|parameter| parameter.place == place)
    }
    /// The signature or block structural parameter keyed `place`.
    pub fn parameter(&self, place: PlaceId) -> Option<&StructuralParameterDeclaration> {
        self.structural_parameter(place)
    }
    /// The signature-level structural parameter keyed `place`, excluding block parameters.
    pub fn signature_structural_parameter(
        &self,
        place: PlaceId,
    ) -> Option<&StructuralParameterDeclaration> {
        self.structural_parameters
            .iter()
            .find(|parameter| parameter.place == place)
    }
}

/// The roster position of the signature-level structural parameter keyed `place`.
pub fn structural_parameter_position(
    parameters: &[StructuralParameterDeclaration],
    place: PlaceId,
) -> Option<usize> {
    parameters
        .iter()
        .position(|parameter| parameter.place == place)
}
