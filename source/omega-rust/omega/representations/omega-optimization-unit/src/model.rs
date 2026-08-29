//! Immutable optimization-unit, fact, proof, ownership, and CFG model.

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

/// An admitted proof fact projected from the immutable verifier context.
///
/// The row binds both semantic artifact identities and the exact operation
/// owner. It remains attached after a rewrite removes that operation so the
/// transformation ledger and manifest can retain proof custody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedObligationFact {
    pub identity: AcceptedObligationFactIdentity,
    pub psi: TerminalPsiIdentity,
    pub proof_bundle_fingerprint: [u8; 32],
    pub machine: MachineId,
    pub operation: OperationId,
    pub obligation: ObligationId,
    pub proposition: Vec<u8>,
}

/// Exact verifier owner of one retained proof question. Positional coordinates
/// are semantic: they prevent equal propositions at distinct source sites from
/// becoming interchangeable optimizer authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProofQuestionOwner {
    Operation {
        machine: MachineId,
        operation: OperationId,
    },
    CallRequires {
        machine: MachineId,
        operation: OperationId,
        requirement_position: u32,
    },
    NominalCleanupRequires {
        machine: MachineId,
        edge: EdgeId,
        cleanup_position: u32,
        requirement_position: u32,
    },
    ContractEnsures {
        machine: MachineId,
        contract: ContractId,
        clause_position: u32,
    },
}

impl ProofQuestionOwner {
    pub const fn machine(self) -> MachineId {
        match self {
            Self::Operation { machine, .. }
            | Self::CallRequires { machine, .. }
            | Self::NominalCleanupRequires { machine, .. }
            | Self::ContractEnsures { machine, .. } => machine,
        }
    }
}

/// Source-independent mirror of the proof-admission classification retained
/// at optimizer admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProofQuestionAdmissionKind {
    ForeignBoundaryGuarantee,
    ProviderFact,
    CheckedAssemblyClaim,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProofQuestionClass {
    Derivable,
    AdmissionAuthorized {
        site: AdmissionSiteId,
        kind: ProofQuestionAdmissionKind,
        authority_identity: EvidenceIdentity,
    },
}

/// Immutable, complete proof question projected one-for-one from Terminal
/// verification. Canonical proposition bytes retain exact ordered premises and
/// axioms without coupling this target-neutral representation to a prover.
/// Rewrites preserve the entire catalog, including rows owned by pruned code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofQuestion {
    pub identity: ProofQuestionIdentity,
    pub terminal_psi: TerminalPsiIdentity,
    pub proof_bundle_fingerprint: [u8; 32],
    pub owner: ProofQuestionOwner,
    pub obligation: ObligationId,
    pub class: ProofQuestionClass,
    pub proposition: Vec<u8>,
    pub requirements: Vec<Vec<u8>>,
    pub semantic_axioms: Vec<Vec<u8>>,
    pub canonical_certificate: bool,
}

/// Exact authority consumed by one derived current-revision integer range.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ValueRangeSupport {
    ScalarConstant(ScalarConstantFactIdentity),
    AcceptedOperationProof {
        accepted: AcceptedObligationFactIdentity,
        question: ProofQuestionIdentity,
        operation: OperationId,
    },
}

/// A range is either valid wherever its SSA value is available, or only from
/// one verified operation entry through the operation's dominated region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ValueRangeScope {
    EntireValue,
    DominatedOperationEntry {
        block: BlockId,
        node: u32,
        operation: OperationId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueRangeRegion {
    pub revision: OptimizationUnitIdentity,
    pub machine: MachineId,
    pub value: ValueId,
    pub scope: ValueRangeScope,
    /// Canonical current-CFG blocks dominated by the proof owner. Empty for
    /// an entire-value scalar fact.
    pub dominated_blocks: Vec<BlockId>,
}

/// One identity-bound interval derived from current scalar or proof custody.
/// This carrier is not stored in [`PsiOptimizationUnit`]; analyses recompute it
/// for each revision and independent validators reconstruct it on demand.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueRangeFact {
    pub identity: ValueRangeFactIdentity,
    pub value: ValueId,
    pub scalar_type: IntegerType,
    pub minimum: IntegerValue,
    pub maximum: IntegerValue,
    pub support: ValueRangeSupport,
    pub valid_in: ValueRangeRegion,
}

pub fn value_range_fact_identity(
    value: ValueId,
    scalar_type: IntegerType,
    minimum: IntegerValue,
    maximum: IntegerValue,
    support: &ValueRangeSupport,
    valid_in: &ValueRangeRegion,
) -> Option<ValueRangeFactIdentity> {
    if valid_in.value != value
        || scalar_type.carrier() != IntegerCarrier::Fixed
        || !scalar_type.admits(minimum)
        || !scalar_type.admits(maximum)
        || integer_value_cmp(scalar_type, minimum, maximum).is_none_or(|order| order.is_gt())
        || match (support, valid_in.scope) {
            (ValueRangeSupport::ScalarConstant(_), ValueRangeScope::EntireValue) => {
                !valid_in.dominated_blocks.is_empty()
            }
            (
                ValueRangeSupport::AcceptedOperationProof { operation, .. },
                ValueRangeScope::DominatedOperationEntry {
                    block,
                    operation: scope_operation,
                    ..
                },
            ) => {
                *operation != scope_operation
                    || valid_in.dominated_blocks.is_empty()
                    || valid_in
                        .dominated_blocks
                        .windows(2)
                        .any(|pair| pair[0] >= pair[1])
                    || valid_in.dominated_blocks.binary_search(&block).is_err()
            }
            _ => true,
        }
    {
        return None;
    }

    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.psi-value-range-fact.v1\0");
    canonical.extend_from_slice(&valid_in.revision.bytes());
    canonical.extend_from_slice(&valid_in.machine.get().to_le_bytes());
    canonical.extend_from_slice(&valid_in.value.get().to_le_bytes());
    canonical.extend_from_slice(&value.get().to_le_bytes());
    encode_range_integer_type(&mut canonical, scalar_type);
    encode_range_integer_value(&mut canonical, minimum);
    encode_range_integer_value(&mut canonical, maximum);
    match support {
        ValueRangeSupport::ScalarConstant(identity) => {
            canonical.push(1);
            canonical.extend_from_slice(&identity.bytes());
        }
        ValueRangeSupport::AcceptedOperationProof {
            accepted,
            question,
            operation,
        } => {
            canonical.push(2);
            canonical.extend_from_slice(&accepted.bytes());
            canonical.extend_from_slice(&question.bytes());
            canonical.extend_from_slice(&operation.get().to_le_bytes());
        }
    }
    match valid_in.scope {
        ValueRangeScope::EntireValue => canonical.push(1),
        ValueRangeScope::DominatedOperationEntry {
            block,
            node,
            operation,
        } => {
            canonical.push(2);
            canonical.extend_from_slice(&block.get().to_le_bytes());
            canonical.extend_from_slice(&node.to_le_bytes());
            canonical.extend_from_slice(&operation.get().to_le_bytes());
        }
    }
    canonical.extend_from_slice(
        &u64::try_from(valid_in.dominated_blocks.len())
            .expect("canonical dominated-block count fits u64")
            .to_le_bytes(),
    );
    for block in &valid_in.dominated_blocks {
        canonical.extend_from_slice(&block.get().to_le_bytes());
    }
    Some(ValueRangeFactIdentity::from_canonical_bytes(&canonical))
}

fn integer_value_cmp(
    scalar_type: IntegerType,
    left: IntegerValue,
    right: IntegerValue,
) -> Option<std::cmp::Ordering> {
    if !scalar_type.admits(left) || !scalar_type.admits(right) {
        return None;
    }
    match (scalar_type.sign(), left, right) {
        (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
            Some(left.cmp(&right))
        }
        (IntegerSign::Unsigned, IntegerValue::Unsigned(left), IntegerValue::Unsigned(right)) => {
            Some(left.cmp(&right))
        }
        _ => None,
    }
}

fn encode_range_integer_type(canonical: &mut Vec<u8>, scalar_type: IntegerType) {
    canonical.push(match scalar_type.carrier() {
        IntegerCarrier::Fixed => 1,
        IntegerCarrier::Address => 2,
    });
    canonical.push(match scalar_type.sign() {
        IntegerSign::Signed => 1,
        IntegerSign::Unsigned => 2,
    });
    canonical.extend_from_slice(&scalar_type.bits().to_le_bytes());
}

fn encode_range_integer_value(canonical: &mut Vec<u8>, value: IntegerValue) {
    match value {
        IntegerValue::Signed(value) => {
            canonical.push(1);
            canonical.extend_from_slice(&value.to_le_bytes());
        }
        IntegerValue::Unsigned(value) => {
            canonical.push(2);
            canonical.extend_from_slice(&value.to_le_bytes());
        }
    }
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
    pub psi: TerminalPsiIdentity,
    pub machine: MachineId,
    pub site: OwnershipFrontierSite,
    pub snapshot: OwnershipFrontierSnapshot,
}

impl OwnershipFrontierFact {
    pub fn new(
        psi: TerminalPsiIdentity,
        machine: MachineId,
        site: OwnershipFrontierSite,
        snapshot: OwnershipFrontierSnapshot,
    ) -> Self {
        let identity = ownership_frontier_fact_identity(psi, machine, site, &snapshot);
        Self {
            identity,
            psi,
            machine,
            site,
            snapshot,
        }
    }

    pub fn has_canonical_identity(&self) -> bool {
        self.identity
            == ownership_frontier_fact_identity(self.psi, self.machine, self.site, &self.snapshot)
    }
}

pub fn ownership_frontier_fact_identity(
    psi: TerminalPsiIdentity,
    machine: MachineId,
    site: OwnershipFrontierSite,
    snapshot: &OwnershipFrontierSnapshot,
) -> OwnershipFrontierFactIdentity {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.psi-ownership-frontier-fact.v1\0");
    canonical.extend_from_slice(psi.program_fingerprint.as_bytes());
    canonical.extend_from_slice(&psi.vocabulary_marker.get().to_le_bytes());
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
        psi: TerminalPsiIdentity,
        proof_bundle_fingerprint: [u8; 32],
        machine: MachineId,
        operation: OperationId,
        obligation: ObligationId,
        proposition: Vec<u8>,
    ) -> Self {
        let identity = accepted_obligation_fact_identity(
            psi,
            proof_bundle_fingerprint,
            machine,
            operation,
            obligation,
            &proposition,
        );
        Self {
            identity,
            psi,
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
                self.psi,
                self.proof_bundle_fingerprint,
                self.machine,
                self.operation,
                self.obligation,
                &self.proposition,
            )
    }
}

pub fn accepted_obligation_fact_identity(
    psi: TerminalPsiIdentity,
    proof_bundle_fingerprint: [u8; 32],
    machine: MachineId,
    operation: OperationId,
    obligation: ObligationId,
    proposition: &[u8],
) -> AcceptedObligationFactIdentity {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.psi-accepted-obligation-fact.v1\0");
    canonical.extend_from_slice(psi.program_fingerprint.as_bytes());
    canonical.extend_from_slice(&psi.vocabulary_marker.get().to_le_bytes());
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

impl ProofQuestion {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        terminal_psi: TerminalPsiIdentity,
        proof_bundle_fingerprint: [u8; 32],
        owner: ProofQuestionOwner,
        obligation: ObligationId,
        class: ProofQuestionClass,
        proposition: Vec<u8>,
        requirements: Vec<Vec<u8>>,
        semantic_axioms: Vec<Vec<u8>>,
        canonical_certificate: bool,
    ) -> Self {
        let identity = proof_question_identity(
            terminal_psi,
            proof_bundle_fingerprint,
            owner,
            obligation,
            class,
            &proposition,
            &requirements,
            &semantic_axioms,
            canonical_certificate,
        );
        Self {
            identity,
            terminal_psi,
            proof_bundle_fingerprint,
            owner,
            obligation,
            class,
            proposition,
            requirements,
            semantic_axioms,
            canonical_certificate,
        }
    }

    pub fn has_canonical_identity(&self) -> bool {
        self.identity
            == proof_question_identity(
                self.terminal_psi,
                self.proof_bundle_fingerprint,
                self.owner,
                self.obligation,
                self.class,
                &self.proposition,
                &self.requirements,
                &self.semantic_axioms,
                self.canonical_certificate,
            )
    }
}

#[allow(clippy::too_many_arguments)]
pub fn proof_question_identity(
    terminal_psi: TerminalPsiIdentity,
    proof_bundle_fingerprint: [u8; 32],
    owner: ProofQuestionOwner,
    obligation: ObligationId,
    class: ProofQuestionClass,
    proposition: &[u8],
    requirements: &[Vec<u8>],
    semantic_axioms: &[Vec<u8>],
    canonical_certificate: bool,
) -> ProofQuestionIdentity {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.psi-proof-question.v1\0");
    canonical.extend_from_slice(terminal_psi.program_fingerprint.as_bytes());
    canonical.extend_from_slice(&terminal_psi.vocabulary_marker.get().to_le_bytes());
    canonical.extend_from_slice(&proof_bundle_fingerprint);
    encode_proof_question_owner(&mut canonical, owner);
    canonical.extend_from_slice(&obligation.get().to_le_bytes());
    encode_proof_question_class(&mut canonical, class);
    encode_proof_question_bytes(&mut canonical, proposition);
    encode_proof_question_byte_rows(&mut canonical, requirements);
    encode_proof_question_byte_rows(&mut canonical, semantic_axioms);
    canonical.push(u8::from(canonical_certificate));
    ProofQuestionIdentity::from_canonical_bytes(&canonical)
}

fn encode_proof_question_owner(bytes: &mut Vec<u8>, owner: ProofQuestionOwner) {
    match owner {
        ProofQuestionOwner::Operation { machine, operation } => {
            bytes.push(1);
            bytes.extend_from_slice(&machine.get().to_le_bytes());
            bytes.extend_from_slice(&operation.get().to_le_bytes());
        }
        ProofQuestionOwner::CallRequires {
            machine,
            operation,
            requirement_position,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&machine.get().to_le_bytes());
            bytes.extend_from_slice(&operation.get().to_le_bytes());
            bytes.extend_from_slice(&requirement_position.to_le_bytes());
        }
        ProofQuestionOwner::NominalCleanupRequires {
            machine,
            edge,
            cleanup_position,
            requirement_position,
        } => {
            bytes.push(3);
            bytes.extend_from_slice(&machine.get().to_le_bytes());
            bytes.extend_from_slice(&edge.get().to_le_bytes());
            bytes.extend_from_slice(&cleanup_position.to_le_bytes());
            bytes.extend_from_slice(&requirement_position.to_le_bytes());
        }
        ProofQuestionOwner::ContractEnsures {
            machine,
            contract,
            clause_position,
        } => {
            bytes.push(4);
            bytes.extend_from_slice(&machine.get().to_le_bytes());
            bytes.extend_from_slice(&contract.get().to_le_bytes());
            bytes.extend_from_slice(&clause_position.to_le_bytes());
        }
    }
}

fn encode_proof_question_class(bytes: &mut Vec<u8>, class: ProofQuestionClass) {
    match class {
        ProofQuestionClass::Derivable => bytes.push(1),
        ProofQuestionClass::AdmissionAuthorized {
            site,
            kind,
            authority_identity,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&site.get().to_le_bytes());
            bytes.push(match kind {
                ProofQuestionAdmissionKind::ForeignBoundaryGuarantee => 1,
                ProofQuestionAdmissionKind::ProviderFact => 2,
                ProofQuestionAdmissionKind::CheckedAssemblyClaim => 3,
            });
            bytes.extend_from_slice(&authority_identity.get().to_le_bytes());
        }
    }
}

fn encode_proof_question_byte_rows(bytes: &mut Vec<u8>, rows: &[Vec<u8>]) {
    bytes.extend_from_slice(
        &u64::try_from(rows.len())
            .expect("canonical proof-question row count fits u64")
            .to_le_bytes(),
    );
    for row in rows {
        encode_proof_question_bytes(bytes, row);
    }
}

fn encode_proof_question_bytes(bytes: &mut Vec<u8>, value: &[u8]) {
    bytes.extend_from_slice(
        &u64::try_from(value.len())
            .expect("canonical proof-question byte length fits u64")
            .to_le_bytes(),
    );
    bytes.extend_from_slice(value);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiOptimizationUnit {
    pub identity: OptimizationUnitIdentity,
    pub psi: TerminalPsiIdentity,
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
    /// Complete immutable verifier proof-question roster in reconstruction
    /// order. This is source-site authority, not a function-wide range index.
    pub proof_questions: Vec<ProofQuestion>,
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
    if facts.iter().any(|fact| fact.psi != unit.psi) {
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
    if facts.iter().any(|fact| fact.psi != unit.psi) {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofQuestionIndexError {
    AlreadyAttached,
    TerminalIdentityMismatch,
    InvalidQuestionIdentity,
    DuplicateQuestion,
}

impl std::fmt::Display for ProofQuestionIndexError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid proof-question index: {self:?}")
    }
}

impl std::error::Error for ProofQuestionIndexError {}

/// Attach the verifier's complete ordered proof-question projection exactly
/// once. The input order is retained rather than reconstructed or sorted.
pub fn attach_proof_questions(
    mut unit: PsiOptimizationUnit,
    questions: Vec<ProofQuestion>,
) -> Result<PsiOptimizationUnit, ProofQuestionIndexError> {
    if !unit.proof_questions.is_empty() {
        return Err(ProofQuestionIndexError::AlreadyAttached);
    }
    if questions
        .iter()
        .any(|question| question.terminal_psi != unit.psi)
    {
        return Err(ProofQuestionIndexError::TerminalIdentityMismatch);
    }
    if questions
        .iter()
        .any(|question| !question.has_canonical_identity())
    {
        return Err(ProofQuestionIndexError::InvalidQuestionIdentity);
    }
    let mut identities = BTreeSet::new();
    let mut owners = BTreeSet::new();
    if questions.iter().any(|question| {
        !identities.insert(question.identity)
            || !owners.insert((question.owner, question.obligation))
    }) {
        return Err(ProofQuestionIndexError::DuplicateQuestion);
    }
    unit.proof_questions = questions;
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    Ok(unit)
}
