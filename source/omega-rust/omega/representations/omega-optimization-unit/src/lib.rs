#![forbid(unsafe_code)]

//! Reconstructible, target-neutral optimization input derived from verified
//! Terminal Psi realization requirements.
//!
//! This crate deliberately performs no optimization. It makes the implicit
//! structure in [`TerminalAbstractOperationPlan`] explicit so independent
//! validators and later passes do not have to rediscover CFG, SSA, semantic
//! fuel, effects, or provenance from a mutable instruction stream.

use std::{collections::BTreeSet, sync::Arc};

use omega_optimization_core::{
    AcceptedObligationFactIdentity, OptimizationUnitIdentity, OwnershipFrontierFactIdentity,
};
use omega_terminal_abstract_operations::{
    TerminalAbstractFunction, TerminalAbstractFunctionResult, TerminalAbstractOperation,
    TerminalAbstractOperationPlan, TerminalAbstractSuccessor, TerminalValueBinding,
};
use psi_core::{
    BlockId, ClaimId, EdgeId, FuelScheduleIdentity, IntegerValue, MachineId, ObligationId,
    OperationId, PlaceId, ScalarType, ServiceId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use psi_terminal::{
    BoundaryMachineDeclaration, ContentEntryClaim, EntryClaim, EvidenceContractLane,
    MachineContract, ProviderCandidateConformance, ServiceDeclaration, StructuralDomainDeclaration,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, TerminalAffineCleanupAction,
    TerminalPsiIdentity, TerminalRootServiceReach,
};

mod identity;
mod ledger;
mod observation;
mod rewrite;

pub use identity::{recompute_psi_optimization_unit_identity, structural_domain_catalog_identity};

pub use ledger::{
    InvalidPsiTransformationLedger, PsiTransformationLedger, PsiTransformationLedgerDecodeError,
    PsiTransformationRecord,
};
pub use observation::{
    ObservationEventClass, ObservationKnowledge, PsiClosedRegionBlockObservation,
    PsiClosedRegionObservation, PsiClosedRegionSemantics, PsiNodeObservation, PsiObservableEvent,
    PsiObservationModel, PsiRegionBoundaryEdgeObservation, PsiRegionFrontierObservation,
    reconstruct_psi_closed_region_observation, reconstruct_psi_observation_model,
};

pub use rewrite::{
    AdjacentBlockMergeRewrite, BlockParameterIncomingBinding, BooleanConstantRewrite,
    ConstantConditionalRewrite, DeadScalarNodeRewrite, DominatingScalarCommonSubexpressionRewrite,
    IntegerConstantRewrite, IntegerEvaluationWitness, LinearEmptyBlockRewrite,
    LocalScalarCommonSubexpressionRewrite, NodeLocation, NonAdjacentBlockMergeRewrite,
    PathQualifiedEmptyBlockRewrite, PhiTranslatedScalarGvnRewrite, PhiTranslatedScalarIncoming,
    ProofCertifiedScalarIdentityKind, ProofCertifiedScalarIdentityRewrite, ProvenanceDisposition,
    ProvenanceRewrite, PsiRealizationSite, PsiRewriteCandidate, PsiRewriteCandidateError,
    PsiRewriteDecisionPoint, PsiRewritePatch, RedundantBlockParameterRewrite,
    RedundantBlockParameterWitness, ScalarConstantValue, ScalarEvaluationWitness,
    ScalarSubstitution, SccpBlockRow, SccpEdgeRow, SccpEdgeState, SccpMachineSnapshot,
    SccpValueRow, SccpValueState, SharedTerminalJumpFusionRewrite,
    UnreachablePrivateMachinesRewrite, derived_sccp_scalar_constant_fact_identity,
    literal_scalar_constant_fact_identity,
};

/// The exact immutable Terminal Psi semantic site realized by one unit node.
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
    pub bindings: Vec<TerminalValueBinding>,
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
    pub operation: TerminalAbstractOperation,
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
    pub result: TerminalAbstractFunctionResult,
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

/// An admitted proof fact projected from the immutable verifier context.
///
/// The row binds both semantic artifact identities and the exact operation
/// owner. It remains attached after a rewrite removes that operation so the
/// transformation ledger and manifest can retain proof custody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedObligationFact {
    pub identity: AcceptedObligationFactIdentity,
    pub terminal_psi: TerminalPsiIdentity,
    pub proof_bundle_fingerprint: [u8; 32],
    pub machine: MachineId,
    pub operation: OperationId,
    pub obligation: ObligationId,
    pub proposition: Vec<u8>,
}

/// Exact verifier-owned source site whose path-sensitive ownership state is
/// retained by the optimization unit. Entry and exit are deliberately distinct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OwnershipFrontierSite {
    BlockEntry(BlockId),
    OperationEntry(OperationId),
    OperationExit(OperationId),
    EdgeEntry(EdgeId),
    /// Present for control-successor edges in the current verifier vocabulary.
    /// Terminal return/crash edges currently retain entry state only.
    EdgeExit(EdgeId),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OwnershipFrontierLiveClaim {
    pub claim: ClaimId,
    pub input: Option<PlaceId>,
    pub path: Vec<StructuralPathSegment>,
    pub multiplicity: Option<StructuralMultiplicity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OwnershipFrontierOwnedPlace {
    pub place: PlaceId,
    pub multiplicity: StructuralMultiplicity,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OwnershipFrontierPartialCustody {
    pub place: PlaceId,
    pub moved_paths: Vec<Vec<StructuralPathSegment>>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OwnershipFrontierSnapshot {
    pub claims: Vec<OwnershipFrontierLiveClaim>,
    pub owned_places: Vec<OwnershipFrontierOwnedPlace>,
    pub partial_custody: Vec<OwnershipFrontierPartialCustody>,
}

/// One immutable source ownership fact projected from the retained verifier
/// context. Rewrites preserve this catalog; analyses bind usable rows to the
/// current unit revision rather than manufacturing new ownership authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnershipFrontierFact {
    pub identity: OwnershipFrontierFactIdentity,
    pub terminal_psi: TerminalPsiIdentity,
    pub machine: MachineId,
    pub site: OwnershipFrontierSite,
    pub snapshot: OwnershipFrontierSnapshot,
}

impl OwnershipFrontierFact {
    pub fn new(
        terminal_psi: TerminalPsiIdentity,
        machine: MachineId,
        site: OwnershipFrontierSite,
        snapshot: OwnershipFrontierSnapshot,
    ) -> Self {
        let identity = ownership_frontier_fact_identity(terminal_psi, machine, site, &snapshot);
        Self {
            identity,
            terminal_psi,
            machine,
            site,
            snapshot,
        }
    }

    pub fn has_canonical_identity(&self) -> bool {
        self.identity
            == ownership_frontier_fact_identity(
                self.terminal_psi,
                self.machine,
                self.site,
                &self.snapshot,
            )
    }
}

pub fn ownership_frontier_fact_identity(
    terminal_psi: TerminalPsiIdentity,
    machine: MachineId,
    site: OwnershipFrontierSite,
    snapshot: &OwnershipFrontierSnapshot,
) -> OwnershipFrontierFactIdentity {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.psi-ownership-frontier-fact.v1\0");
    canonical.extend_from_slice(terminal_psi.program_fingerprint.as_bytes());
    canonical.extend_from_slice(&terminal_psi.vocabulary_marker.get().to_le_bytes());
    canonical.extend_from_slice(&machine.get().to_le_bytes());
    encode_frontier_site_identity(&mut canonical, site);
    encode_frontier_snapshot_identity(&mut canonical, snapshot);
    OwnershipFrontierFactIdentity::from_canonical_bytes(&canonical)
}

fn encode_frontier_site_identity(bytes: &mut Vec<u8>, site: OwnershipFrontierSite) {
    let (tag, identity) = match site {
        OwnershipFrontierSite::BlockEntry(id) => (1, id.get()),
        OwnershipFrontierSite::OperationEntry(id) => (2, id.get()),
        OwnershipFrontierSite::OperationExit(id) => (3, id.get()),
        OwnershipFrontierSite::EdgeEntry(id) => (4, id.get()),
        OwnershipFrontierSite::EdgeExit(id) => (5, id.get()),
    };
    bytes.push(tag);
    bytes.extend_from_slice(&identity.to_le_bytes());
}

fn encode_frontier_snapshot_identity(bytes: &mut Vec<u8>, snapshot: &OwnershipFrontierSnapshot) {
    encode_frontier_len(bytes, snapshot.claims.len());
    for claim in &snapshot.claims {
        bytes.extend_from_slice(&claim.claim.get().to_le_bytes());
        encode_frontier_optional_id(bytes, claim.input.map(PlaceId::get));
        encode_frontier_path(bytes, &claim.path);
        encode_frontier_multiplicity(bytes, claim.multiplicity);
    }
    encode_frontier_len(bytes, snapshot.owned_places.len());
    for place in &snapshot.owned_places {
        bytes.extend_from_slice(&place.place.get().to_le_bytes());
        encode_frontier_multiplicity(bytes, Some(place.multiplicity));
    }
    encode_frontier_len(bytes, snapshot.partial_custody.len());
    for partial in &snapshot.partial_custody {
        bytes.extend_from_slice(&partial.place.get().to_le_bytes());
        encode_frontier_len(bytes, partial.moved_paths.len());
        for path in &partial.moved_paths {
            encode_frontier_path(bytes, path);
        }
    }
}

fn encode_frontier_len(bytes: &mut Vec<u8>, len: usize) {
    bytes.extend_from_slice(
        &u64::try_from(len)
            .expect("canonical ownership-frontier length fits u64")
            .to_le_bytes(),
    );
}

fn encode_frontier_optional_id(bytes: &mut Vec<u8>, id: Option<u64>) {
    bytes.push(u8::from(id.is_some()));
    if let Some(id) = id {
        bytes.extend_from_slice(&id.to_le_bytes());
    }
}

fn encode_frontier_path(bytes: &mut Vec<u8>, path: &[StructuralPathSegment]) {
    encode_frontier_len(bytes, path.len());
    for segment in path {
        match segment {
            StructuralPathSegment::Field(identity) => {
                bytes.push(1);
                encode_frontier_len(bytes, identity.len());
                bytes.extend_from_slice(identity.as_bytes());
            }
            StructuralPathSegment::FixedIndex(index) => {
                bytes.push(2);
                bytes.extend_from_slice(&index.to_le_bytes());
            }
        }
    }
}

fn encode_frontier_multiplicity(bytes: &mut Vec<u8>, multiplicity: Option<StructuralMultiplicity>) {
    bytes.push(match multiplicity {
        None => 0,
        Some(StructuralMultiplicity::Unrestricted) => 1,
        Some(StructuralMultiplicity::Affine) => 2,
        Some(StructuralMultiplicity::Linear) => 3,
    });
}

impl AcceptedObligationFact {
    pub fn new(
        terminal_psi: TerminalPsiIdentity,
        proof_bundle_fingerprint: [u8; 32],
        machine: MachineId,
        operation: OperationId,
        obligation: ObligationId,
        proposition: Vec<u8>,
    ) -> Self {
        let identity = accepted_obligation_fact_identity(
            terminal_psi,
            proof_bundle_fingerprint,
            machine,
            operation,
            obligation,
            &proposition,
        );
        Self {
            identity,
            terminal_psi,
            proof_bundle_fingerprint,
            machine,
            operation,
            obligation,
            proposition,
        }
    }

    pub fn has_canonical_identity(&self) -> bool {
        self.identity
            == accepted_obligation_fact_identity(
                self.terminal_psi,
                self.proof_bundle_fingerprint,
                self.machine,
                self.operation,
                self.obligation,
                &self.proposition,
            )
    }
}

pub fn accepted_obligation_fact_identity(
    terminal_psi: TerminalPsiIdentity,
    proof_bundle_fingerprint: [u8; 32],
    machine: MachineId,
    operation: OperationId,
    obligation: ObligationId,
    proposition: &[u8],
) -> AcceptedObligationFactIdentity {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.psi-accepted-obligation-fact.v1\0");
    canonical.extend_from_slice(terminal_psi.program_fingerprint.as_bytes());
    canonical.extend_from_slice(&terminal_psi.vocabulary_marker.get().to_le_bytes());
    canonical.extend_from_slice(&proof_bundle_fingerprint);
    canonical.extend_from_slice(&machine.get().to_le_bytes());
    canonical.extend_from_slice(&operation.get().to_le_bytes());
    canonical.extend_from_slice(&obligation.get().to_le_bytes());
    canonical.extend_from_slice(
        &u64::try_from(proposition.len())
            .expect("canonical proposition length fits u64")
            .to_le_bytes(),
    );
    canonical.extend_from_slice(proposition);
    AcceptedObligationFactIdentity::from_canonical_bytes(&canonical)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiOptimizationUnit {
    pub identity: OptimizationUnitIdentity,
    pub terminal_psi: TerminalPsiIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub entry: MachineId,
    /// Target-neutral module declarations needed by layout, ABI, and checked
    /// provider installation after the full Terminal module is discarded.
    pub structural_types: Vec<StructuralTypeDeclaration>,
    /// Exact verifier-owned qualification-domain catalog. Bare lowering seeds
    /// leave this empty; optimizer admission attaches it before rewrites run.
    pub structural_domains: Arc<[StructuralDomainDeclaration]>,
    /// Exact verifier-owned boundary-service hierarchy. Bare lowering seeds
    /// leave this empty; optimizer admission attaches the complete catalog so
    /// call reach and concrete service effects remain independently replayable.
    pub services: Arc<[ServiceDeclaration]>,
    /// Exact current-revision closure of services executable from `entry`.
    /// Unlike declaration custody, this derived row may narrow when a checked
    /// rewrite removes an unreachable call or concrete service effect.
    pub root_service_reach: TerminalRootServiceReach,
    pub boundary_machines: Vec<BoundaryMachineDeclaration>,
    pub provider_candidates: Vec<ProviderCandidateConformance>,
    pub accepted_obligation_facts: Vec<AcceptedObligationFact>,
    /// Immutable verifier projection, absent only on low-level bare seeds that
    /// are not authorized optimizer inputs.
    pub ownership_frontier_facts: Vec<OwnershipFrontierFact>,
    /// Canonical custody for source functions removed by independently proven
    /// whole-program reachability rewrites. Source ordinals bind each removed
    /// machine to the immutable verified Terminal-Psi function roster.
    pub pruned_machines: Vec<PrunedMachineCustody>,
    pub functions: Vec<PsiOptimizationFunction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PrunedMachineCustody {
    pub machine: MachineId,
    pub source_ordinal: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnershipFrontierFactIndexError {
    AlreadyAttached,
    TerminalIdentityMismatch,
    InvalidFactIdentity,
    NonCanonicalOrder,
    NonCanonicalSnapshot,
}

impl std::fmt::Display for OwnershipFrontierFactIndexError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid ownership frontier fact index: {self:?}")
    }
}

impl std::error::Error for OwnershipFrontierFactIndexError {}

/// Attach the complete verifier projection exactly once and bind it into unit
/// identity. Construction is intentionally separate from the bare seed API.
pub fn attach_ownership_frontier_facts(
    mut unit: PsiOptimizationUnit,
    facts: Vec<OwnershipFrontierFact>,
) -> Result<PsiOptimizationUnit, OwnershipFrontierFactIndexError> {
    if !unit.ownership_frontier_facts.is_empty() {
        return Err(OwnershipFrontierFactIndexError::AlreadyAttached);
    }
    if facts
        .iter()
        .any(|fact| fact.terminal_psi != unit.terminal_psi)
    {
        return Err(OwnershipFrontierFactIndexError::TerminalIdentityMismatch);
    }
    if facts.iter().any(|fact| !fact.has_canonical_identity()) {
        return Err(OwnershipFrontierFactIndexError::InvalidFactIdentity);
    }
    if facts
        .windows(2)
        .any(|pair| (pair[0].machine, pair[0].site) >= (pair[1].machine, pair[1].site))
    {
        return Err(OwnershipFrontierFactIndexError::NonCanonicalOrder);
    }
    if facts
        .iter()
        .any(|fact| !canonical_ownership_frontier_snapshot(&fact.snapshot))
    {
        return Err(OwnershipFrontierFactIndexError::NonCanonicalSnapshot);
    }
    unit.ownership_frontier_facts = facts;
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    Ok(unit)
}

pub fn canonical_ownership_frontier_snapshot(snapshot: &OwnershipFrontierSnapshot) -> bool {
    strictly_ordered_by(&snapshot.claims, |claim| claim.claim)
        && strictly_ordered_by(&snapshot.owned_places, |place| place.place)
        && strictly_ordered_by(&snapshot.partial_custody, |partial| partial.place)
        && snapshot
            .partial_custody
            .iter()
            .all(|partial| partial.moved_paths.windows(2).all(|pair| pair[0] < pair[1]))
}

fn strictly_ordered_by<T, K: Ord>(values: &[T], key: impl Fn(&T) -> K) -> bool {
    values.windows(2).all(|pair| key(&pair[0]) < key(&pair[1]))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptedObligationFactIndexError {
    AlreadyAttached,
    TerminalIdentityMismatch,
    InvalidFactIdentity,
    DuplicateOwner,
}

impl std::fmt::Display for AcceptedObligationFactIndexError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid accepted obligation fact index: {self:?}"
        )
    }
}

impl std::error::Error for AcceptedObligationFactIndexError {}

/// Attach the canonical verifier projection exactly once and bind it into the
/// optimization-unit identity. Bare units intentionally retain an empty index.
pub fn attach_accepted_obligation_facts(
    mut unit: PsiOptimizationUnit,
    mut facts: Vec<AcceptedObligationFact>,
) -> Result<PsiOptimizationUnit, AcceptedObligationFactIndexError> {
    if !unit.accepted_obligation_facts.is_empty() {
        return Err(AcceptedObligationFactIndexError::AlreadyAttached);
    }
    if facts
        .iter()
        .any(|fact| fact.terminal_psi != unit.terminal_psi)
    {
        return Err(AcceptedObligationFactIndexError::TerminalIdentityMismatch);
    }
    if facts.iter().any(|fact| !fact.has_canonical_identity()) {
        return Err(AcceptedObligationFactIndexError::InvalidFactIdentity);
    }
    facts.sort_by_key(|fact| (fact.machine, fact.operation, fact.obligation));
    if facts.windows(2).any(|pair| {
        (pair[0].machine, pair[0].operation, pair[0].obligation)
            == (pair[1].machine, pair[1].operation, pair[1].obligation)
    }) {
        return Err(AcceptedObligationFactIndexError::DuplicateOwner);
    }
    unit.accepted_obligation_facts = facts;
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    Ok(unit)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizationUnitBuildError {
    MissingBlocks(MachineId),
    FirstBlockDoesNotStartAtZero(MachineId),
    InvalidBlockOffset { machine: MachineId, offset: usize },
    DuplicateBlock(MachineId, BlockId),
    NodeIndexOverflow(MachineId),
    ParameterIndexOverflow(MachineId),
}

impl std::fmt::Display for OptimizationUnitBuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "cannot construct canonical Psi optimization unit: {self:?}"
        )
    }
}

impl std::error::Error for OptimizationUnitBuildError {}

/// Low-level deterministic projection from the clean lowering seed.
///
/// This is not an optimizer admission boundary: consumers that may transform
/// the unit must use the verified constructor owned by the Terminal-Psi
/// artifact boundary so the plan cannot detach from its verifier context.
pub fn reconstruct_psi_optimization_unit_seed(
    plan: &TerminalAbstractOperationPlan,
    fuel_schedule: FuelScheduleIdentity,
) -> Result<PsiOptimizationUnit, OptimizationUnitBuildError> {
    let functions = plan
        .functions
        .iter()
        .map(build_function)
        .collect::<Result<Vec<_>, _>>()?;
    let mut unit = PsiOptimizationUnit {
        identity: OptimizationUnitIdentity::from_canonical_bytes(b"pending canonical content"),
        terminal_psi: plan.terminal_psi,
        fuel_schedule,
        entry: plan.entry,
        structural_types: plan.structural_types.clone(),
        structural_domains: Arc::new([]),
        services: Arc::new([]),
        root_service_reach: TerminalRootServiceReach::default(),
        boundary_machines: plan.boundary_machines.clone(),
        provider_candidates: plan.provider_candidates.clone(),
        accepted_obligation_facts: Vec::new(),
        ownership_frontier_facts: Vec::new(),
        pruned_machines: Vec::new(),
        functions,
    };
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    Ok(unit)
}

fn build_function(
    function: &TerminalAbstractFunction,
) -> Result<PsiOptimizationFunction, OptimizationUnitBuildError> {
    if function.block_entries.is_empty() {
        return Err(OptimizationUnitBuildError::MissingBlocks(function.machine));
    }
    if function.block_entries[0].operation_offset != 0 {
        return Err(OptimizationUnitBuildError::FirstBlockDoesNotStartAtZero(
            function.machine,
        ));
    }
    let mut block_ids = BTreeSet::new();
    for entry in &function.block_entries {
        if entry.operation_offset > function.operations.len() {
            return Err(OptimizationUnitBuildError::InvalidBlockOffset {
                machine: function.machine,
                offset: entry.operation_offset,
            });
        }
        if !block_ids.insert(entry.block) {
            return Err(OptimizationUnitBuildError::DuplicateBlock(
                function.machine,
                entry.block,
            ));
        }
    }

    let parameters = function
        .parameters
        .iter()
        .enumerate()
        .map(|(position, parameter)| {
            Ok(ValueDefinition {
                value: parameter.value,
                scalar_type: parameter.scalar_type,
                site: ValueDefinitionSite::FunctionParameter(u32::try_from(position).map_err(
                    |_| OptimizationUnitBuildError::ParameterIndexOverflow(function.machine),
                )?),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut facts = Vec::new();
    let mut structural_places = function
        .structural_parameters
        .iter()
        .map(|parameter| StructuralPlaceDeclaration {
            id: parameter.place,
            kind: StructuralPlaceKind::Parameter {
                position: parameter.position,
                is_self: parameter.is_self,
            },
        })
        .chain(
            function
                .result
                .structural()
                .map(|result| StructuralPlaceDeclaration {
                    id: result.place,
                    kind: StructuralPlaceKind::Result,
                }),
        )
        .collect::<Vec<_>>();
    let mut declared_places = function
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .chain(function.entry_claims.iter().map(|claim| claim.input))
        .chain(function.result.structural().map(|result| result.place))
        .collect::<BTreeSet<_>>();
    let mut effect_token = 0u64;
    let mut blocks = Vec::with_capacity(function.block_entries.len());
    for (block_index, entry) in function.block_entries.iter().enumerate() {
        let end = function
            .block_entries
            .get(block_index + 1)
            .map_or(function.operations.len(), |next| next.operation_offset);
        if end < entry.operation_offset {
            return Err(OptimizationUnitBuildError::InvalidBlockOffset {
                machine: function.machine,
                offset: end,
            });
        }
        let block_parameter_rows = entry
            .parameters
            .iter()
            .enumerate()
            .map(|(position, parameter)| {
                Ok(ValueDefinition {
                    value: parameter.value,
                    scalar_type: parameter.scalar_type,
                    site: ValueDefinitionSite::BlockParameter {
                        block: entry.block,
                        position: u32::try_from(position).map_err(|_| {
                            OptimizationUnitBuildError::ParameterIndexOverflow(function.machine)
                        })?,
                    },
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut nodes = Vec::with_capacity(end - entry.operation_offset);
        for (local_index, operation) in function.operations[entry.operation_offset..end]
            .iter()
            .enumerate()
        {
            let node = u32::try_from(local_index)
                .map_err(|_| OptimizationUnitBuildError::NodeIndexOverflow(function.machine))?;
            let provenance = operation_node_provenance(operation);
            let fuel = provenance
                .iter()
                .copied()
                .map(|site| FuelSettlement { site, units: 1 })
                .collect();
            let definitions = operation_definition(operation)
                .into_iter()
                .map(|(value, scalar_type)| ValueDefinition {
                    value,
                    scalar_type,
                    site: ValueDefinitionSite::Node {
                        block: entry.block,
                        node,
                    },
                })
                .collect();
            let uses = operation_uses(operation)
                .into_iter()
                .map(|value| ValueUse {
                    value,
                    block: entry.block,
                    node,
                })
                .collect();
            collect_places(operation, &mut declared_places);
            match operation {
                TerminalAbstractOperation::EstablishPayloadlessCase {
                    psi_operation,
                    result,
                    ..
                }
                | TerminalAbstractOperation::CallStructural {
                    psi_operation,
                    result,
                    ..
                } => structural_places.push(StructuralPlaceDeclaration {
                    id: result.place,
                    kind: StructuralPlaceKind::OperationResult {
                        producer: *psi_operation,
                        structural_type: result.structural_type,
                    },
                }),
                TerminalAbstractOperation::EstablishByteSequenceLiteral { place, .. }
                | TerminalAbstractOperation::EstablishTrivialAffineLocal { place, .. } => {
                    structural_places.push(*place);
                }
                _ => {}
            }
            collect_fact(operation, &mut facts);
            let ownership = operation_ownership(operation);
            let successors = operation_edges(operation);
            nodes.push(OptimizationNode {
                operation: operation.clone(),
                provenance,
                fuel,
                effect: EffectLink {
                    input: effect_token,
                    output: effect_token + 1,
                },
                definitions,
                uses,
                successors,
                ownership,
            });
            effect_token += 1;
        }
        blocks.push(OptimizationBlock {
            id: entry.block,
            parameters: block_parameter_rows,
            nodes,
        });
    }

    Ok(PsiOptimizationFunction {
        machine: function.machine,
        attachment: function.attachment,
        entry: function.entry,
        parameters,
        structural_parameters: function.structural_parameters.clone(),
        structural_places,
        result: function.result.clone(),
        declared_places,
        entry_claim_declarations: function.entry_claims.clone(),
        content_entry_claims: Vec::new(),
        verified_contract: None,
        evidence_contract_lanes: Vec::new(),
        entry_claims: function
            .entry_claims
            .iter()
            .map(|claim| claim.claim)
            .collect(),
        published_service_ceiling: function.published_service_ceiling.clone(),
        facts,
        blocks,
    })
}

fn operation_node_provenance(operation: &TerminalAbstractOperation) -> Vec<PsiProvenance> {
    use TerminalAbstractOperation as O;
    let site = match operation {
        O::Jump { .. } | O::Conditional { .. } => return Vec::new(),
        O::Return { psi_edge, .. } | O::ReturnUnit { psi_edge, .. } | O::Crash { psi_edge, .. } => {
            PsiProvenance::Edge(*psi_edge)
        }
        O::ReturnStructural {
            psi_edge,
            trivial_affine_locals,
            ..
        } => {
            // Provenance is custody order, not execution order: the terminal
            // edge remains the primary realization site, followed by the
            // compressed establishment operations in tuple order. Rewrites
            // may append inherited custody only after this exact prefix.
            return std::iter::once(PsiProvenance::Edge(*psi_edge))
                .chain(
                    trivial_affine_locals
                        .iter()
                        .map(|(operation, _, _)| PsiProvenance::Operation(*operation)),
                )
                .collect();
        }
        O::EstablishPayloadlessCase { psi_operation, .. }
        | O::EstablishByteSequenceLiteral { psi_operation, .. }
        | O::EstablishTrivialAffineLocal { psi_operation, .. }
        | O::CallUnit { psi_operation, .. }
        | O::CallStructuralScalar { psi_operation, .. }
        | O::CallStructural { psi_operation, .. }
        | O::BoundaryCall { psi_operation, .. }
        | O::PortWrite { psi_operation, .. }
        | O::Call { psi_operation, .. }
        | O::IntegerConstant { psi_operation, .. }
        | O::BooleanConstant { psi_operation, .. }
        | O::BooleanStructuralField { psi_operation, .. }
        | O::BooleanNot { psi_operation, .. }
        | O::BooleanEqual { psi_operation, .. }
        | O::IntegerEqual { psi_operation, .. }
        | O::IntegerLessThan { psi_operation, .. }
        | O::IntegerLessOrEqual { psi_operation, .. }
        | O::IntegerBitwiseNot { psi_operation, .. }
        | O::IntegerWiden { psi_operation, .. }
        | O::IntegerExactCast { psi_operation, .. }
        | O::IntegerBitwiseAnd { psi_operation, .. }
        | O::IntegerBitwiseOr { psi_operation, .. }
        | O::IntegerBitwiseXor { psi_operation, .. }
        | O::WrappingIntegerShiftLeft { psi_operation, .. }
        | O::WrappingIntegerShiftRight { psi_operation, .. }
        | O::ExactIntegerShiftLeft { psi_operation, .. }
        | O::ExactIntegerShiftRight { psi_operation, .. }
        | O::WrappingIntegerAdd { psi_operation, .. }
        | O::ExactIntegerAdd { psi_operation, .. }
        | O::SaturatingIntegerAdd { psi_operation, .. }
        | O::WrappingIntegerSubtract { psi_operation, .. }
        | O::ExactIntegerSubtract { psi_operation, .. }
        | O::SaturatingIntegerSubtract { psi_operation, .. }
        | O::WrappingIntegerMultiply { psi_operation, .. }
        | O::ExactIntegerMultiply { psi_operation, .. }
        | O::ExactIntegerDivide { psi_operation, .. }
        | O::ExactIntegerRemainder { psi_operation, .. }
        | O::WrappingIntegerDivide { psi_operation, .. }
        | O::WrappingIntegerRemainder { psi_operation, .. }
        | O::SaturatingIntegerDivide { psi_operation, .. }
        | O::SaturatingIntegerRemainder { psi_operation, .. }
        | O::SaturatingIntegerMultiply { psi_operation, .. } => {
            PsiProvenance::Operation(*psi_operation)
        }
    };
    vec![site]
}

fn operation_definition(operation: &TerminalAbstractOperation) -> Option<(ValueId, ScalarType)> {
    use TerminalAbstractOperation as O;
    match operation {
        O::Call {
            result,
            scalar_type,
            ..
        }
        | O::IntegerConstant {
            result,
            scalar_type,
            ..
        } => Some((*result, *scalar_type)),
        O::CallStructuralScalar { result, .. } => Some((result.value, result.scalar_type)),
        O::BoundaryCall {
            result: Some(result),
            ..
        } => Some((result.value, result.scalar_type)),
        O::BooleanConstant { result, .. }
        | O::BooleanStructuralField { result, .. }
        | O::BooleanNot { result, .. }
        | O::BooleanEqual { result, .. }
        | O::IntegerEqual { result, .. }
        | O::IntegerLessThan { result, .. }
        | O::IntegerLessOrEqual { result, .. } => Some((*result, ScalarType::Boolean)),
        O::IntegerBitwiseNot {
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseAnd {
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseOr {
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseXor {
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerAdd {
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerAdd {
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerAdd {
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerSubtract {
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerSubtract {
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerSubtract {
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerMultiply {
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerMultiply {
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerDivide {
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerRemainder {
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerDivide {
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerRemainder {
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerDivide {
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerRemainder {
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerMultiply {
            result,
            scalar_type,
            ..
        } => Some((*result, ScalarType::Integer(*scalar_type))),
        O::IntegerWiden {
            result,
            target_type,
            ..
        }
        | O::IntegerExactCast {
            result,
            target_type,
            ..
        } => Some((*result, ScalarType::Integer(*target_type))),
        O::WrappingIntegerShiftLeft {
            result, value_type, ..
        }
        | O::WrappingIntegerShiftRight {
            result, value_type, ..
        }
        | O::ExactIntegerShiftLeft {
            result, value_type, ..
        }
        | O::ExactIntegerShiftRight {
            result, value_type, ..
        } => Some((*result, ScalarType::Integer(*value_type))),
        _ => None,
    }
}

fn operation_uses(operation: &TerminalAbstractOperation) -> Vec<ValueId> {
    use TerminalAbstractOperation as O;
    match operation {
        O::Call { arguments, .. } | O::BoundaryCall { arguments, .. } => arguments.clone(),
        O::BooleanNot { operand, .. }
        | O::IntegerBitwiseNot { operand, .. }
        | O::IntegerWiden { operand, .. }
        | O::IntegerExactCast { operand, .. } => vec![*operand],
        O::BooleanEqual { left, right, .. }
        | O::IntegerEqual { left, right, .. }
        | O::IntegerLessThan { left, right, .. }
        | O::IntegerLessOrEqual { left, right, .. }
        | O::IntegerBitwiseAnd { left, right, .. }
        | O::IntegerBitwiseOr { left, right, .. }
        | O::IntegerBitwiseXor { left, right, .. }
        | O::WrappingIntegerAdd { left, right, .. }
        | O::ExactIntegerAdd { left, right, .. }
        | O::SaturatingIntegerAdd { left, right, .. }
        | O::WrappingIntegerSubtract { left, right, .. }
        | O::ExactIntegerSubtract { left, right, .. }
        | O::SaturatingIntegerSubtract { left, right, .. }
        | O::WrappingIntegerMultiply { left, right, .. }
        | O::ExactIntegerMultiply { left, right, .. }
        | O::ExactIntegerDivide { left, right, .. }
        | O::ExactIntegerRemainder { left, right, .. }
        | O::WrappingIntegerDivide { left, right, .. }
        | O::WrappingIntegerRemainder { left, right, .. }
        | O::SaturatingIntegerDivide { left, right, .. }
        | O::SaturatingIntegerRemainder { left, right, .. }
        | O::SaturatingIntegerMultiply { left, right, .. } => vec![*left, *right],
        O::WrappingIntegerShiftLeft { value, count, .. }
        | O::WrappingIntegerShiftRight { value, count, .. }
        | O::ExactIntegerShiftLeft { value, count, .. }
        | O::ExactIntegerShiftRight { value, count, .. } => vec![*value, *count],
        O::Jump { bindings, .. } => bindings.iter().map(|binding| binding.argument).collect(),
        O::Conditional {
            condition,
            when_true,
            when_false,
        } => std::iter::once(*condition)
            .chain(when_true.bindings.iter().map(|binding| binding.argument))
            .chain(when_false.bindings.iter().map(|binding| binding.argument))
            .collect(),
        O::Return { value, .. } => vec![*value],
        _ => Vec::new(),
    }
}

fn operation_edges(operation: &TerminalAbstractOperation) -> Vec<OptimizationEdge> {
    use TerminalAbstractOperation as O;
    match operation {
        O::Jump {
            psi_edge,
            target,
            bindings,
            trivial_affine_discards,
        } => vec![OptimizationEdge {
            psi_edge: *psi_edge,
            target: *target,
            bindings: bindings.clone(),
            trivial_affine_discards: trivial_affine_discards.clone(),
            provenance: vec![PsiProvenance::Edge(*psi_edge)],
            fuel: vec![FuelSettlement {
                site: PsiProvenance::Edge(*psi_edge),
                units: 1,
            }],
        }],
        O::Conditional {
            when_true,
            when_false,
            ..
        } => vec![successor_edge(when_true), successor_edge(when_false)],
        _ => Vec::new(),
    }
}

fn successor_edge(successor: &TerminalAbstractSuccessor) -> OptimizationEdge {
    OptimizationEdge {
        psi_edge: successor.psi_edge,
        target: successor.target,
        bindings: successor.bindings.clone(),
        trivial_affine_discards: successor.trivial_affine_discards.clone(),
        provenance: vec![PsiProvenance::Edge(successor.psi_edge)],
        fuel: vec![FuelSettlement {
            site: PsiProvenance::Edge(successor.psi_edge),
            units: 1,
        }],
    }
}

fn collect_places(operation: &TerminalAbstractOperation, places: &mut BTreeSet<PlaceId>) {
    use TerminalAbstractOperation as O;
    match operation {
        O::EstablishByteSequenceLiteral { place, .. }
        | O::EstablishTrivialAffineLocal { place, .. } => {
            places.insert(place.id);
        }
        O::EstablishPayloadlessCase { result, .. } | O::CallStructural { result, .. } => {
            places.insert(result.place);
        }
        O::BooleanStructuralField { source, .. } | O::ReturnStructural { source, .. } => {
            places.insert(*source);
        }
        _ => {}
    }
}

fn collect_fact(operation: &TerminalAbstractOperation, facts: &mut Vec<OptimizationFact>) {
    if let Some((obligation, support)) = operation_obligation(operation) {
        facts.push(OptimizationFact::OperationObligationReference {
            obligation,
            support,
        });
    }
    match operation {
        TerminalAbstractOperation::BooleanConstant {
            psi_operation,
            result,
            value,
        } => facts.push(OptimizationFact::BooleanConstant {
            value: *result,
            constant: *value,
            support: *psi_operation,
        }),
        TerminalAbstractOperation::IntegerConstant {
            psi_operation,
            result,
            value,
            ..
        } => facts.push(OptimizationFact::IntegerConstant {
            value: *result,
            constant: *value,
            support: *psi_operation,
        }),
        _ => {}
    }
}

fn operation_obligation(
    operation: &TerminalAbstractOperation,
) -> Option<(ObligationId, OperationId)> {
    use TerminalAbstractOperation as O;
    match operation {
        O::IntegerExactCast {
            psi_operation,
            obligation,
            ..
        }
        | O::ExactIntegerShiftLeft {
            psi_operation,
            obligation,
            ..
        }
        | O::ExactIntegerShiftRight {
            psi_operation,
            obligation,
            ..
        }
        | O::ExactIntegerAdd {
            psi_operation,
            obligation,
            ..
        }
        | O::ExactIntegerSubtract {
            psi_operation,
            obligation,
            ..
        }
        | O::ExactIntegerMultiply {
            psi_operation,
            obligation,
            ..
        }
        | O::ExactIntegerDivide {
            psi_operation,
            obligation,
            ..
        }
        | O::ExactIntegerRemainder {
            psi_operation,
            obligation,
            ..
        }
        | O::WrappingIntegerDivide {
            psi_operation,
            obligation,
            ..
        }
        | O::WrappingIntegerRemainder {
            psi_operation,
            obligation,
            ..
        }
        | O::SaturatingIntegerDivide {
            psi_operation,
            obligation,
            ..
        }
        | O::SaturatingIntegerRemainder {
            psi_operation,
            obligation,
            ..
        } => Some((*obligation, *psi_operation)),
        _ => None,
    }
}

fn operation_ownership(operation: &TerminalAbstractOperation) -> Vec<OwnershipEvent> {
    use TerminalAbstractOperation as O;
    match operation {
        O::CallUnit {
            claim_transfers, ..
        }
        | O::CallStructuralScalar {
            claim_transfers, ..
        } => {
            vec![OwnershipEvent::ClaimTransfer(
                claim_transfers
                    .iter()
                    .map(|transfer| transfer.claim)
                    .collect(),
            )]
        }
        O::CallStructural {
            claim_transfers, ..
        } => vec![OwnershipEvent::ClaimTransfer(
            claim_transfers
                .iter()
                .map(|transfer| transfer.claim)
                .collect(),
        )],
        O::BoundaryCall {
            completion_receipts,
            ..
        } => vec![OwnershipEvent::ClaimCompletion(
            completion_receipts
                .iter()
                .map(|receipt| receipt.claim)
                .collect(),
        )],
        O::Return {
            cleanup_actions, ..
        }
        | O::ReturnUnit {
            cleanup_actions, ..
        } => {
            vec![OwnershipEvent::Cleanup(cleanup_actions.clone())]
        }
        O::ReturnStructural {
            returned_claims, ..
        } => {
            vec![OwnershipEvent::StructuralReturn(returned_claims.clone())]
        }
        O::Crash {
            frontier_lower_bound,
            ..
        } => {
            vec![OwnershipEvent::CrashFrontier(frontier_lower_bound.clone())]
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_terminal_abstract_operations::{
        TerminalAbstractBlockEntry, TerminalAbstractFunctionResult, TerminalAbstractParameter,
        TerminalAbstractResult, TerminalValueBinding,
    };
    use psi_core::{
        BoundaryMachineId, ContentPlaceVersion, DomainSemanticId, IntegerSign, IntegerType,
        IntegerValue, ServiceId, StructuralDomainId, StructuralTypeId,
    };
    use psi_terminal::{
        BoundaryMachineDeclaration, ByteSequenceCarrier, ProviderCandidateConformance,
        ProviderUnitRefinement, ProviderUnitSignature, SemanticFingerprint,
        StructuralTypeDeclaration, StructuralTypeShape, VocabularyMarker,
    };

    fn id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
        constructor(raw).expect("nonzero test identity")
    }

    fn plan() -> TerminalAbstractOperationPlan {
        let machine = id(1, MachineId::new);
        let block = id(2, BlockId::new);
        let value = id(3, ValueId::new);
        let result = id(4, ValueId::new);
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("valid width");
        TerminalAbstractOperationPlan {
            terminal_psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![TerminalAbstractFunction {
                machine,
                attachment: None,
                entry: block,
                parameters: vec![TerminalAbstractParameter {
                    value,
                    scalar_type: ScalarType::Integer(integer),
                }],
                structural_parameters: Vec::new(),
                result: TerminalAbstractFunctionResult::Scalar(
                    omega_terminal_abstract_operations::TerminalAbstractResult {
                        value: result,
                        scalar_type: ScalarType::Integer(integer),
                    },
                ),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![TerminalAbstractBlockEntry {
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    TerminalAbstractOperation::IntegerConstant {
                        psi_operation: id(5, OperationId::new),
                        result,
                        scalar_type: ScalarType::Integer(integer),
                        value: IntegerValue::Unsigned(9),
                    },
                    TerminalAbstractOperation::Return {
                        psi_edge: id(6, EdgeId::new),
                        result,
                        value: result,
                        scalar_type: ScalarType::Integer(integer),
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        }
    }

    #[test]
    fn rebuild_is_deterministic_and_keeps_distinct_fuel_sites() {
        let schedule = FuelScheduleIdentity::new(1).expect("nonzero schedule");
        let first = reconstruct_psi_optimization_unit_seed(&plan(), schedule).unwrap();
        let second = reconstruct_psi_optimization_unit_seed(&plan(), schedule).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.functions[0].blocks[0].nodes.len(), 2);
        assert_ne!(
            first.functions[0].blocks[0].nodes[0].fuel[0].site,
            first.functions[0].blocks[0].nodes[1].fuel[0].site
        );
        let source = plan();
        assert_eq!(first.structural_types, source.structural_types);
        assert_eq!(first.boundary_machines, source.boundary_machines);
        assert_eq!(first.provider_candidates, source.provider_candidates);
        assert!(first.accepted_obligation_facts.is_empty());
        assert!(first.ownership_frontier_facts.is_empty());
        assert_eq!(
            first.functions[0].attachment,
            source.functions[0].attachment
        );
        assert_eq!(first.functions[0].result, source.functions[0].result);
        assert_eq!(
            first.functions[0].entry_claim_declarations,
            source.functions[0].entry_claims
        );
        assert_eq!(
            first.functions[0].published_service_ceiling,
            source.functions[0].published_service_ceiling
        );
    }

    #[test]
    fn canonical_identity_is_content_recomputable_and_history_independent() {
        let schedule = FuelScheduleIdentity::new(1).expect("nonzero schedule");
        let first = reconstruct_psi_optimization_unit_seed(&plan(), schedule).unwrap();
        let second = reconstruct_psi_optimization_unit_seed(&plan(), schedule).unwrap();
        assert_eq!(
            recompute_psi_optimization_unit_identity(&first),
            recompute_psi_optimization_unit_identity(&second)
        );

        let mut different_stored_history = first.clone();
        different_stored_history.identity =
            OptimizationUnitIdentity::from_canonical_bytes(b"unrelated stored history");
        assert_eq!(
            recompute_psi_optimization_unit_identity(&first),
            recompute_psi_optimization_unit_identity(&different_stored_history)
        );
    }

    #[test]
    fn canonical_identity_binds_every_retained_field_class() {
        let baseline = reconstruct_psi_optimization_unit_seed(
            &plan(),
            FuelScheduleIdentity::new(1).expect("nonzero schedule"),
        )
        .unwrap();
        let baseline_identity = recompute_psi_optimization_unit_identity(&baseline);
        let machine = baseline.functions[0].machine;
        let block = baseline.functions[0].blocks[0].id;
        let scalar_type = baseline.functions[0].parameters[0].scalar_type;
        let mut mutations = Vec::new();

        let mut unit = baseline.clone();
        unit.terminal_psi.program_fingerprint = SemanticFingerprint::from_bytes([8; 32]);
        mutations.push(("terminal identity", unit));
        let mut unit = baseline.clone();
        unit.fuel_schedule = FuelScheduleIdentity::new(2).unwrap();
        mutations.push(("fuel schedule", unit));
        let mut unit = baseline.clone();
        unit.entry = id(90, MachineId::new);
        mutations.push(("entry machine", unit));
        let structural_type = id(105, StructuralTypeId::new);
        let boundary = id(106, BoundaryMachineId::new);
        let mut unit = baseline.clone();
        unit.structural_types.push(StructuralTypeDeclaration {
            id: structural_type,
            identity: "identity-test-structural-type".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
        });
        mutations.push(("module structural type", unit));
        let mut unit = baseline.clone();
        unit.structural_domains = Arc::from(vec![psi_terminal::StructuralDomainDeclaration {
            id: id(112, StructuralDomainId::new),
            semantic_domain: id(113, DomainSemanticId::new),
            identity: "identity-test-structural-domain".into(),
            carrier: structural_type,
            content_projection: None,
        }]);
        mutations.push(("module structural domain", unit));
        let mut unit = baseline.clone();
        unit.root_service_reach.concrete = vec![id(116, ServiceId::new)];
        mutations.push(("root concrete service reach", unit));
        let mut unit = baseline.clone();
        unit.root_service_reach.installation_dependencies =
            vec![psi_terminal::InstallationReachDependency {
                requirement_identity: "identity-test-installation-requirement".into(),
                upper_bound: vec![id(117, ServiceId::new)],
            }];
        mutations.push(("root installation service reach", unit));
        let mut unit = baseline.clone();
        unit.boundary_machines.push(BoundaryMachineDeclaration {
            id: boundary,
            identity: "identity-test-boundary".into(),
            attachment: Some(structural_type),
            scalar_parameters: vec![ScalarType::Boolean],
            structural_parameters: Vec::new(),
            result: Some(ScalarType::Boolean),
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: vec![id(107, ServiceId::new)],
        });
        mutations.push(("module boundary declaration", unit));
        let mut unit = baseline.clone();
        unit.provider_candidates.push(ProviderCandidateConformance {
            boundary,
            requirement_identity: "identity-test-requirement".into(),
            provider_identity: "identity-test-provider".into(),
            candidate_identity: "identity-test-candidate".into(),
            candidate: machine,
            signature: ProviderUnitSignature {
                parameters: Vec::new(),
            },
            refinement: ProviderUnitRefinement {
                positional_parameters: Vec::new(),
                required_domains: Vec::new(),
                realized_service_ceiling: vec![id(108, ServiceId::new)],
            },
        });
        mutations.push(("module provider candidate", unit));
        let mut unit = baseline.clone();
        unit.accepted_obligation_facts
            .push(AcceptedObligationFact::new(
                unit.terminal_psi,
                [4; 32],
                machine,
                id(5, OperationId::new),
                id(91, ObligationId::new),
                vec![1, 2, 3],
            ));
        mutations.push(("accepted fact", unit));
        let mut unit = baseline.clone();
        unit.ownership_frontier_facts
            .push(OwnershipFrontierFact::new(
                unit.terminal_psi,
                machine,
                OwnershipFrontierSite::BlockEntry(block),
                OwnershipFrontierSnapshot {
                    claims: Vec::new(),
                    owned_places: Vec::new(),
                    partial_custody: Vec::new(),
                },
            ));
        mutations.push(("ownership frontier fact", unit));
        let mut unit = baseline.clone();
        unit.pruned_machines.push(PrunedMachineCustody {
            machine: id(109, MachineId::new),
            source_ordinal: 1,
        });
        mutations.push(("pruned machine custody", unit));
        let mut unit = baseline.clone();
        unit.functions[0].machine = id(92, MachineId::new);
        mutations.push(("function identity", unit));
        let mut unit = baseline.clone();
        unit.functions[0].attachment = Some(structural_type);
        mutations.push(("function attachment", unit));
        let mut unit = baseline.clone();
        unit.functions[0].parameters[0].value = id(93, ValueId::new);
        mutations.push(("scalar parameter", unit));
        let mut unit = baseline.clone();
        unit.functions[0].structural_parameters.push(
            psi_terminal::StructuralParameterDeclaration {
                place: id(94, PlaceId::new),
                position: 0,
                is_self: false,
                structural_type: id(95, psi_core::StructuralTypeId::new),
                multiplicity: psi_terminal::StructuralMultiplicity::Affine,
                access: psi_terminal::StructuralAccess::Owned,
                qualifications: Vec::new(),
            },
        );
        mutations.push(("structural parameter", unit));
        let mut unit = baseline.clone();
        let structural_place = id(114, PlaceId::new);
        unit.functions[0]
            .structural_places
            .push(psi_terminal::StructuralPlaceDeclaration {
                id: structural_place,
                kind: StructuralPlaceKind::Result,
            });
        mutations.push(("structural place declaration", unit));
        let mut unit = baseline.clone();
        unit.functions[0]
            .content_entry_claims
            .push(psi_terminal::ContentEntryClaim {
                claim: id(115, ClaimId::new),
                input: psi_core::ContentStructuralPlace {
                    version: ContentPlaceVersion::Entry,
                    root: structural_place,
                    segments: Vec::new(),
                },
                projections: Vec::new(),
            });
        mutations.push(("content entry claim", unit));
        let mut unit = baseline.clone();
        unit.functions[0].result = TerminalAbstractFunctionResult::Unit;
        mutations.push(("function result signature", unit));
        let mut unit = baseline.clone();
        unit.functions[0]
            .declared_places
            .insert(id(96, PlaceId::new));
        mutations.push(("declared place", unit));
        let mut unit = baseline.clone();
        unit.functions[0].entry_claim_declarations.push(EntryClaim {
            claim: id(109, ClaimId::new),
            input: id(110, PlaceId::new),
            path: Vec::new(),
        });
        mutations.push(("entry claim declaration", unit));
        let mut unit = baseline.clone();
        unit.functions[0].entry_claims.insert(id(97, ClaimId::new));
        mutations.push(("entry claim", unit));
        let mut unit = baseline.clone();
        unit.functions[0]
            .published_service_ceiling
            .push(id(111, ServiceId::new));
        mutations.push(("function service ceiling", unit));
        let mut unit = baseline.clone();
        unit.functions[0].facts.clear();
        mutations.push(("optimization fact", unit));
        let mut unit = baseline.clone();
        unit.functions[0].blocks[0].id = id(98, BlockId::new);
        mutations.push(("block", unit));
        let mut unit = baseline.clone();
        let TerminalAbstractOperation::IntegerConstant { value, .. } =
            &mut unit.functions[0].blocks[0].nodes[0].operation
        else {
            unreachable!()
        };
        *value = IntegerValue::Unsigned(10);
        mutations.push(("operation payload", unit));
        let mut unit = baseline.clone();
        unit.functions[0].blocks[0].nodes[0].provenance[0] =
            PsiProvenance::Operation(id(99, OperationId::new));
        mutations.push(("provenance", unit));
        let mut unit = baseline.clone();
        unit.functions[0].blocks[0].nodes[0].fuel[0].units = 2;
        mutations.push(("fuel settlement", unit));
        let mut unit = baseline.clone();
        unit.functions[0].blocks[0].nodes[0].effect.output = 77;
        mutations.push(("effect", unit));
        let mut unit = baseline.clone();
        unit.functions[0].blocks[0].nodes[0].definitions[0].scalar_type = ScalarType::Boolean;
        mutations.push(("definition", unit));
        let mut unit = baseline.clone();
        unit.functions[0].blocks[0].nodes[1].uses[0].value = id(100, ValueId::new);
        mutations.push(("use", unit));
        let mut unit = baseline.clone();
        unit.functions[0].blocks[0].nodes[0]
            .successors
            .push(OptimizationEdge {
                psi_edge: id(101, EdgeId::new),
                target: block,
                bindings: vec![TerminalValueBinding {
                    parameter: id(102, ValueId::new),
                    argument: id(103, ValueId::new),
                    scalar_type,
                }],
                trivial_affine_discards: Vec::new(),
                provenance: vec![PsiProvenance::Edge(id(101, EdgeId::new))],
                fuel: vec![FuelSettlement {
                    site: PsiProvenance::Edge(id(101, EdgeId::new)),
                    units: 1,
                }],
            });
        mutations.push(("successor", unit));
        let mut unit = baseline.clone();
        unit.functions[0].blocks[0].nodes[0]
            .ownership
            .push(OwnershipEvent::ClaimTransfer(vec![id(104, ClaimId::new)]));
        mutations.push(("ownership", unit));

        for (field_class, unit) in mutations {
            assert_ne!(
                recompute_psi_optimization_unit_identity(&unit),
                baseline_identity,
                "{field_class} must contribute to canonical content identity"
            );
        }
    }

    #[test]
    fn ownership_frontier_attachment_is_canonical_and_single_use() {
        let seed = reconstruct_psi_optimization_unit_seed(
            &plan(),
            FuelScheduleIdentity::new(1).expect("nonzero schedule"),
        )
        .unwrap();
        let machine = seed.functions[0].machine;
        let block = seed.functions[0].entry;
        let empty = OwnershipFrontierSnapshot {
            claims: Vec::new(),
            owned_places: Vec::new(),
            partial_custody: Vec::new(),
        };
        let block_fact = OwnershipFrontierFact::new(
            seed.terminal_psi,
            machine,
            OwnershipFrontierSite::BlockEntry(block),
            empty.clone(),
        );
        let edge_fact = OwnershipFrontierFact::new(
            seed.terminal_psi,
            machine,
            OwnershipFrontierSite::EdgeEntry(id(6, EdgeId::new)),
            empty,
        );
        assert_eq!(
            attach_ownership_frontier_facts(
                seed.clone(),
                vec![edge_fact.clone(), block_fact.clone()]
            ),
            Err(OwnershipFrontierFactIndexError::NonCanonicalOrder)
        );
        let place = id(20, PlaceId::new);
        let duplicate_place_snapshot = OwnershipFrontierSnapshot {
            claims: Vec::new(),
            owned_places: vec![
                OwnershipFrontierOwnedPlace {
                    place,
                    multiplicity: StructuralMultiplicity::Affine,
                },
                OwnershipFrontierOwnedPlace {
                    place,
                    multiplicity: StructuralMultiplicity::Affine,
                },
            ],
            partial_custody: Vec::new(),
        };
        assert_eq!(
            attach_ownership_frontier_facts(
                seed.clone(),
                vec![OwnershipFrontierFact::new(
                    seed.terminal_psi,
                    machine,
                    OwnershipFrontierSite::BlockEntry(block),
                    duplicate_place_snapshot,
                )],
            ),
            Err(OwnershipFrontierFactIndexError::NonCanonicalSnapshot)
        );

        let attached = attach_ownership_frontier_facts(
            seed.clone(),
            vec![block_fact.clone(), edge_fact.clone()],
        )
        .unwrap();
        let replay = attach_ownership_frontier_facts(seed, vec![block_fact, edge_fact]).unwrap();
        assert_eq!(attached, replay);
        assert_eq!(
            attach_ownership_frontier_facts(attached, Vec::new()),
            Err(OwnershipFrontierFactIndexError::AlreadyAttached)
        );
    }

    #[test]
    fn observation_projection_keeps_external_events_and_semantic_accounting() {
        let unit = reconstruct_psi_optimization_unit_seed(
            &plan(),
            FuelScheduleIdentity::new(1).expect("nonzero schedule"),
        )
        .unwrap();
        let observations = reconstruct_psi_observation_model(&unit);

        assert_eq!(observations.revision, unit.identity);
        assert_eq!(observations.nodes.len(), 2);
        assert!(observations.nodes[0].events.is_empty());
        assert_eq!(observations.nodes[0].crash, ObservationKnowledge::No);
        assert_eq!(observations.nodes[0].provenance.len(), 1);
        assert_eq!(observations.nodes[0].fuel.len(), 1);
        assert_eq!(observations.nodes[1].events.len(), 1);
        assert_eq!(
            observations.nodes[1].events[0].class,
            ObservationEventClass::NormalExit
        );
        assert!(matches!(
            observations.nodes[1].events[0].operation,
            TerminalAbstractOperation::Return { .. }
        ));
    }

    #[test]
    fn block_parameters_keep_terminal_declaration_order() {
        let mut plan = plan();
        let function = &mut plan.functions[0];
        let entry = function.entry;
        let target = id(20, BlockId::new);
        // Deliberately descending identities prove this is declaration order,
        // not the previous BTreeMap order.
        let first_parameter = id(90, ValueId::new);
        let second_parameter = id(80, ValueId::new);
        let first_argument = function.parameters[0].value;
        let second_argument = id(70, ValueId::new);
        let scalar_type = function.parameters[0].scalar_type;
        function.parameters.push(TerminalAbstractParameter {
            value: second_argument,
            scalar_type,
        });
        function.result = TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
            value: first_parameter,
            scalar_type,
        });
        function.block_entries = vec![
            TerminalAbstractBlockEntry {
                block: entry,
                parameters: Vec::new(),
                operation_offset: 0,
            },
            TerminalAbstractBlockEntry {
                block: target,
                parameters: vec![
                    TerminalAbstractParameter {
                        value: first_parameter,
                        scalar_type,
                    },
                    TerminalAbstractParameter {
                        value: second_parameter,
                        scalar_type,
                    },
                ],
                operation_offset: 1,
            },
        ];
        function.operations = vec![
            TerminalAbstractOperation::Jump {
                psi_edge: id(60, EdgeId::new),
                target,
                bindings: vec![
                    TerminalValueBinding {
                        parameter: first_parameter,
                        argument: first_argument,
                        scalar_type,
                    },
                    TerminalValueBinding {
                        parameter: second_parameter,
                        argument: second_argument,
                        scalar_type,
                    },
                ],
                trivial_affine_discards: Vec::new(),
            },
            TerminalAbstractOperation::Return {
                psi_edge: id(61, EdgeId::new),
                result: first_parameter,
                value: first_parameter,
                scalar_type,
                cleanup_actions: Vec::new(),
            },
        ];

        let unit = reconstruct_psi_optimization_unit_seed(
            &plan,
            FuelScheduleIdentity::new(1).expect("nonzero schedule"),
        )
        .expect("ordered block parameters");
        assert_eq!(
            unit.functions[0].blocks[1]
                .parameters
                .iter()
                .map(|parameter| parameter.value)
                .collect::<Vec<_>>(),
            vec![first_parameter, second_parameter]
        );
    }
}
