use psi_core::{
    BlockId, BoundaryMachineId, ClaimId, ContentAlgebra, ContentConservation,
    ContentProjectionExpression, ContentProjectionIdentity, ContentProjectionScalar,
    ContentStructuralPlace, ContentTerm, ContractId, DomainSemanticId, EdgeId, EvidenceTermId,
    IeeeFloatFormat, IeeeFloatValue, IntegerType, IntegerValue, MachineId, ObligationId,
    OperationId, PlaceId, Proposition, PropositionId, ScalarType, ServiceId, StructuralCaseId,
    StructuralDomainId, StructuralFieldId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use psi_language_core::BindingRelevance;
use sha2::{Digest, Sha256};

/// Marker for the single unstable terminal-Psi semantic vocabulary.
///
/// The in-memory representation accepts only the vocabulary it was built with.
/// The terminal codec may migrate an explicitly supported prior wire vocabulary
/// before constructing this marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VocabularyMarker;

impl VocabularyMarker {
    pub const CURRENT: Self = Self;

    pub const fn new(raw: u16) -> Option<Self> {
        if raw == Self::CURRENT.get() {
            Some(Self::CURRENT)
        } else {
            None
        }
    }

    pub const fn get(self) -> u16 {
        66
    }
}

/// Closed lifecycle interpretation for one restored-parent call publication.
///
/// The variants are deliberately not a general restoration algebra. They
/// distinguish the two exact checked tuples accepted by this bounded row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalReborrowRestorationClass {
    ExclusiveReactivation,
    SharedFreezeRestoration,
}

/// One exact member of the closed shared-freeze cohort restored by a bounded
/// restored-parent call publication.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalReborrowSharedCohortMember {
    pub child_owner_identity: String,
    pub child_owner_path: Vec<TerminalBorrowOwnerSegment>,
    pub child_place: TerminalBorrowPlace,
    pub child_access: StructuralAccess,
    pub child_activation: TerminalBorrowBoundarySource,
    pub child_weakening: TerminalBorrowBoundarySource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueDeclaration {
    pub id: ValueId,
    pub scalar_type: ScalarType,
}

/// The normal result shape of one terminal machine.
///
/// Unit is the absence of a runtime value. It therefore has no `ValueId`, no
/// scalar type, and no result pseudo-value that contracts can name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalMachineResult {
    Unit,
    Scalar(ValueDeclaration),
    Structural(StructuralResultDeclaration),
}

/// The closed proof-recursion relation vocabulary retained in Terminal Psi.
/// The verifier, not the producer, reconstructs its relation identity and
/// proof obligations from the complete component row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalProofRankingRelation {
    StructuralSubterm,
}

/// One proof-only callable participating in a recursive component. Its
/// contract ID joins kernel recursion admission; source identities retain the
/// exact declarations without frontend arena handles.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalProofRecursiveMember {
    pub contract: ContractId,
    pub machine_identity: String,
    pub rank_parameter_identity: String,
}

/// One field in the closed finite-inductive proof-type graph used to replay a
/// structural-subterm ranking path. These rows describe proof data only; they
/// do not authorize runtime layout or projection.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalProofRecursiveField {
    pub identity: String,
    pub type_identity: String,
}

/// One nominal node in a recursive proof-data graph. The verifier requires
/// every retained strict path to resolve field-by-field in this graph and end
/// back at the component's rank type.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalProofRecursiveType {
    pub identity: String,
    pub fields: Vec<TerminalProofRecursiveField>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalProofRecursiveTransitionLane {
    Target,
    Continuation,
}

/// Exact source-independent coordinate of one internal recursive call.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalProofRecursiveCallSite {
    Statement {
        state_identity: String,
        statement_index: u64,
    },
    Expression {
        state_identity: String,
        statement_index: u64,
        expression_ordinal: u64,
    },
    Transition {
        state_identity: String,
        statement_index: u64,
        lane: TerminalProofRecursiveTransitionLane,
    },
}

/// One exact internal edge and its strict declaration-identity path. Repeated
/// calls between the same member pair remain separate rows.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalProofRecursiveEdge {
    pub caller: ContractId,
    pub callee: ContractId,
    pub site: TerminalProofRecursiveCallSite,
    pub strict_member_path: Vec<String>,
}

/// One canonical proof-only strongly connected component. Ranking and
/// well-foundedness occur once; per-edge decrease obligations are verifier-
/// reconstructed from these exact semantic rows.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalProofRecursiveComponent {
    pub ranking_relation: TerminalProofRankingRelation,
    pub rank_type_identity: String,
    pub types: Vec<TerminalProofRecursiveType>,
    pub members: Vec<TerminalProofRecursiveMember>,
    pub edges: Vec<TerminalProofRecursiveEdge>,
}

impl TerminalMachineResult {
    pub const fn scalar(&self) -> Option<ValueDeclaration> {
        match self {
            Self::Scalar(result) => Some(*result),
            Self::Unit | Self::Structural(_) => None,
        }
    }

    pub const fn scalar_ref(&self) -> Option<&ValueDeclaration> {
        match self {
            Self::Scalar(result) => Some(result),
            Self::Unit | Self::Structural(_) => None,
        }
    }

    pub fn scalar_mut(&mut self) -> Option<&mut ValueDeclaration> {
        match self {
            Self::Scalar(result) => Some(result),
            Self::Unit | Self::Structural(_) => None,
        }
    }

    pub const fn structural(&self) -> Option<&StructuralResultDeclaration> {
        match self {
            Self::Structural(result) => Some(result),
            Self::Unit | Self::Scalar(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalModule {
    pub vocabulary_marker: VocabularyMarker,
    pub entry: MachineId,
    /// Concrete target-neutral instantiated type shapes, ordered by `id`.
    /// Native layout is deliberately absent and is selected by Omega.
    pub structural_types: Vec<StructuralTypeDeclaration>,
    /// Structural qualification domains, ordered by `id`.
    pub structural_domains: Vec<StructuralDomainDeclaration>,
    /// Boundary-service declarations and their normalized parent closure.
    pub services: Vec<ServiceDeclaration>,
    /// Source-handle-free service-reach closure of the selected entry.
    /// Concrete reach remains distinct from bounded installation dependencies;
    /// final installation substitutes one selected provider row per dependency.
    pub root_service_reach: TerminalRootServiceReach,
    /// Closure-wide direct entry inputs whose opaque placed-view meaning is
    /// bound to one exact source-derived placement interpretation. This is
    /// semantic custody only and grants no runtime storage or access.
    pub placed_view_inputs: Vec<TerminalPlacedViewInput>,
    /// Exact direct-root custody restored by independently replayed, finite
    /// linear exclusive-reborrow lineages. These rows grant no cleanup,
    /// transfer, or linear-discharge authority.
    pub reborrow_root_handoffs: Vec<TerminalReborrowRootHandoff>,
    /// One exact whole-parent mutating call after a one-hop exclusive child
    /// reactivates its direct mutable root. These rows grant use only at the
    /// named call and cannot express cleanup, transfer, or discharge.
    pub reborrow_restored_call_uses: Vec<TerminalReborrowRestoredCallUse>,
    /// Bodyless target-neutral Unit machines callable from terminal Psi.
    pub boundary_machines: Vec<BoundaryMachineDeclaration>,
    /// Every checked, target-neutral provider candidate eligible to realize a
    /// retained Unit boundary requirement. This is a semantic catalog, not a
    /// selection: installation policy remains outside terminal-Psi identity.
    pub provider_candidates: Vec<ProviderCandidateConformance>,
    /// Source-handle-free proof-only float projections. These rows are
    /// semantic-module evidence, never executable operations or runtime values.
    pub float_meaning_projections: Vec<crate::FloatMeaningProjection>,
    /// Proof-only propositions consuming exact float projection results.
    pub float_meaning_equalities: Vec<crate::FloatMeaningEqualityProposition>,
    /// Nominal proof-formula vocabulary, strictly ordered by `id`.
    /// Transparent aliases never receive a declaration row.
    pub proposition_declarations: Vec<PropositionDeclaration>,
    /// Normalized applications retained without frontend arena handles.
    pub proposition_applications: Vec<PropositionApplicationIdentity>,
    /// Canonical erased witness identities. Multiple terms may inhabit the
    /// same proposition application; a forwarding assignment preserves its
    /// source identity and therefore does not add a declaration here.
    pub evidence_terms: Vec<EvidenceTermDeclaration>,
    /// Strictly ordered erased machine-contract lane rows. These reference
    /// term vocabulary identities and have no runtime representation.
    pub evidence_contract_lanes: Vec<EvidenceContractLane>,
    /// Canonical immediate invocations that introduce fresh caller-local
    /// evidence from a proof-output lane. Runtime-value bindings retain
    /// their exact ordinary scalar call operation.
    pub proof_output_calls: Vec<ProofOutputCall>,
    /// Source-free proof-only SCCs reachable from the retained root proof
    /// closure. These are semantic obligation inputs, not producer evidence.
    pub proof_recursive_components: Vec<TerminalProofRecursiveComponent>,
    /// Exact source-handle-free generic conformance applications used by the
    /// retained machine closure. Rows are owned by the concrete terminal
    /// machine whose specialization selected the application.
    pub closed_conformance_applications: Vec<ClosedConformanceApplication>,
    /// Source-free local dynamic selection and dispatch custody.
    pub dynamic_dispatch: crate::TerminalDynamicDispatchCatalog,
    /// Canonical proof-only quotient correspondence, strictly ordered by its
    /// independently replayable identity. The public operation's hermetic
    /// identity is the semantic owner; these rows do not join an executable
    /// Terminal machine or authorize a representative call.
    pub quotient_correspondences: Vec<crate::RetainedQuotientCorrespondence>,
    pub machines: Vec<TerminalMachine>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalRootServiceReach {
    pub concrete: Vec<ServiceId>,
    pub installation_dependencies: Vec<InstallationReachDependency>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalBorrowBoundarySource {
    Statement {
        statement_index: u64,
    },
    Call {
        statement_index: u64,
        call_ordinal: u64,
        target_identity: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalBorrowOwnerSegment {
    Field(String),
    Case(String),
    FixedIndex(u64),
    DynamicIndex,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalBorrowPlaceSegment {
    Field(String),
    Case(String),
    FixedIndex(u64),
    FixedRange { start: u64, end: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalBorrowPlace {
    pub root_identity: String,
    pub segments: Vec<TerminalBorrowPlaceSegment>,
}

/// One exact child edge in a finite exclusive-reborrow root-handoff lineage.
///
/// Rows are ordered from the direct-root child toward the leaf whose closure
/// reaches state exit. The immediate parent's place and access are therefore
/// the handoff root for the first row and the preceding child's for every later
/// row. This representation has no shared-cohort or branching vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalReborrowRootHandoffStep {
    pub child_owner_identity: String,
    pub child_owner_path: Vec<TerminalBorrowOwnerSegment>,
    pub child_place: TerminalBorrowPlace,
    pub projection_remainder: Vec<TerminalBorrowPlaceSegment>,
    pub child_access: StructuralAccess,
    pub child_activation: TerminalBorrowBoundarySource,
    pub formation_boundary: TerminalBorrowBoundarySource,
    pub child_weakening: TerminalBorrowBoundarySource,
}

/// Closed publication of direct-root custody after one exact finite linear
/// exclusive-reborrow lineage has reached a checked state-exit handoff. The
/// row's vocabulary is intentionally incapable of expressing cleanup,
/// transfer, discharge, shared cohorts, or branching.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalReborrowRootHandoff {
    pub machine: MachineId,
    pub source_machine_identity: String,
    pub source_state_identity: String,
    pub direct_root_owner_identity: String,
    pub direct_root_owner_path: Vec<TerminalBorrowOwnerSegment>,
    pub direct_root_place: TerminalBorrowPlace,
    pub direct_root_access: StructuralAccess,
    pub direct_root_activation: TerminalBorrowBoundarySource,
    pub direct_root_weakening: TerminalBorrowBoundarySource,
    pub direct_root_lifetime_identity: String,
    pub lineage: Vec<TerminalReborrowRootHandoffStep>,
}

/// Closed publication of one exact use after one direct exclusive child, or
/// an exact one- or two-member shared-freeze cohort, has restored its mutable
/// parent. The canonical operation identifies the sole authorized use. Access,
/// restoration class, source call, and the exact shared roster are explicit;
/// carrier-read and restored-place facts fixed by these bounded forms remain
/// verifier rules. This vocabulary cannot express cleanup, transfer, or
/// discharge.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalReborrowRestoredCallUse {
    pub machine: MachineId,
    pub operation: OperationId,
    pub restoration_class: TerminalReborrowRestorationClass,
    pub call_boundary: TerminalBorrowBoundarySource,
    pub call_target_machine: MachineId,
    pub source_machine_identity: String,
    pub source_state_identity: String,
    pub direct_root_owner_identity: String,
    pub direct_root_owner_path: Vec<TerminalBorrowOwnerSegment>,
    pub direct_root_place: TerminalBorrowPlace,
    pub direct_root_activation: TerminalBorrowBoundarySource,
    pub direct_root_weakening: TerminalBorrowBoundarySource,
    pub direct_root_lifetime_identity: String,
    pub child_owner_identity: String,
    pub child_owner_path: Vec<TerminalBorrowOwnerSegment>,
    pub child_place: TerminalBorrowPlace,
    pub projection_remainder: Vec<TerminalBorrowPlaceSegment>,
    pub child_access: StructuralAccess,
    pub child_activation: TerminalBorrowBoundarySource,
    pub formation_boundary: TerminalBorrowBoundarySource,
    pub child_weakening: TerminalBorrowBoundarySource,
    pub shared_cohort: Vec<TerminalReborrowSharedCohortMember>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalPlacedViewInput {
    pub machine: MachineId,
    pub position: u32,
    pub source_machine_identity: String,
    pub source_state_identity: String,
    pub source_parameter_identity: String,
    pub access: StructuralAccess,
    pub binding_is_const: bool,
    pub binding_is_mutable: bool,
    pub view_identity: String,
    pub policy_identity: String,
    pub policy_plan_machine_identity: String,
    pub schema_identity: String,
    /// Compatibility/report coordinate only. `placement_commitment` is the
    /// collision-resistant canonical layout/access/reach identity.
    pub placement_report_fingerprint: u64,
    pub placement_commitment: [u8; 32],
}

/// Canonical source-free identity of the synthesized `Placed<P, T>` view.
/// Length framing keeps the policy/schema pair injective even when declaration
/// paths contain punctuation used by the presentation grammar.
pub fn canonical_placed_view_identity(policy: &str, schema: &str) -> String {
    format!(
        "placed-view:{}:{policy}:{}:{schema}",
        policy.len(),
        schema.len()
    )
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstallationReachDependency {
    pub requirement_identity: String,
    pub upper_bound: Vec<ServiceId>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClosedConformanceApplication {
    pub owner: MachineId,
    pub declaration_identity: String,
    pub telescope: Vec<ClosedConformanceParameterBinding>,
    pub subject_identity: Option<String>,
    pub trait_identity: String,
    pub trait_lifetime_arguments: Vec<String>,
    pub trait_arguments: Vec<String>,
    /// Ordered standalone replay registry derived from the exact checked
    /// source-machine closure. Rows name entries by canonical callable
    /// identity without duplicating the artifact-local machine coordinate.
    pub realization_callables: Vec<ClosedConformanceRealizationCallable>,
    pub rows: Vec<ClosedConformanceRow>,
    /// Historical compact report/index coordinate. It cannot authorize a
    /// dispatch or replay without the adjacent strong commitment.
    pub report_fingerprint: u64,
    /// Domain-separated SHA-256 commitment to the exact source-free
    /// application structure.
    pub commitment: ClosedConformanceApplicationCommitment,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClosedConformanceApplicationCommitment([u8; 32]);

impl ClosedConformanceApplicationCommitment {
    pub const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn is_zero(self) -> bool {
        self.0 == [0; 32]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClosedConformanceParameterKind {
    Lifetime,
    Type,
    Const,
    Machine,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClosedConformanceParameterBinding {
    pub parameter: String,
    pub kind: ClosedConformanceParameterKind,
    pub argument: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClosedConformanceRow {
    pub declaring_trait_identity: String,
    /// Canonical normalized overload identity of the public requirement.
    pub public_requirement_identity: String,
    /// Declaration path retained separately for exact row-map replay.
    pub requirement_identity: String,
    pub realization_identity: String,
    /// Reference into the owning application's standalone callable registry.
    /// Rows outside the bounded static named-witness lane remain map-free.
    pub realization_callable_identity: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClosedConformanceRealizationCallable {
    /// Canonical checked callable identity, not a declaration display path.
    pub source_callable_identity: String,
    /// Artifact-local Terminal machine emitted for that exact callable.
    pub machine: MachineId,
    /// Source-derived matched requirement/realization result class for the
    /// bounded static requirement cohort.
    /// Callable identity intentionally excludes return type, so this separately
    /// committed value prevents coordinated scalar-result retargeting.
    pub result: ClosedConformanceCallableResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClosedConformanceCallableResult {
    Unit,
    I32,
    Bool,
}

pub fn closed_conformance_application_report_fingerprint(
    application: &ClosedConformanceApplication,
) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    fn push(bytes: &mut Vec<u8>, value: &str) {
        bytes.extend((value.len() as u64).to_le_bytes());
        bytes.extend(value.as_bytes());
    }

    let mut bytes = Vec::new();
    push(&mut bytes, &application.declaration_identity);
    push(
        &mut bytes,
        application
            .subject_identity
            .as_deref()
            .unwrap_or("<subjectless>"),
    );
    push(&mut bytes, &application.trait_identity);
    bytes.extend((application.trait_lifetime_arguments.len() as u64).to_le_bytes());
    for argument in &application.trait_lifetime_arguments {
        push(&mut bytes, argument);
    }
    bytes.extend((application.telescope.len() as u64).to_le_bytes());
    for binding in &application.telescope {
        push(&mut bytes, &binding.parameter);
        bytes.push(match binding.kind {
            ClosedConformanceParameterKind::Lifetime => 1,
            ClosedConformanceParameterKind::Type => 2,
            ClosedConformanceParameterKind::Const => 3,
            ClosedConformanceParameterKind::Machine => 4,
        });
        push(&mut bytes, &binding.argument);
    }
    bytes.extend((application.trait_arguments.len() as u64).to_le_bytes());
    for argument in &application.trait_arguments {
        push(&mut bytes, argument);
    }
    bytes.extend((application.realization_callables.len() as u64).to_le_bytes());
    for callable in &application.realization_callables {
        push(&mut bytes, &callable.source_callable_identity);
        bytes.extend(callable.machine.get().to_le_bytes());
        bytes.push(match callable.result {
            ClosedConformanceCallableResult::Unit => 1,
            ClosedConformanceCallableResult::I32 => 2,
            ClosedConformanceCallableResult::Bool => 3,
        });
    }
    bytes.extend((application.rows.len() as u64).to_le_bytes());
    for row in &application.rows {
        push(&mut bytes, &row.declaring_trait_identity);
        push(&mut bytes, &row.public_requirement_identity);
        push(&mut bytes, &row.requirement_identity);
        push(&mut bytes, &row.realization_identity);
        bytes.push(u8::from(row.realization_callable_identity.is_some()));
        if let Some(identity) = &row.realization_callable_identity {
            push(&mut bytes, identity);
        }
    }
    bytes.into_iter().fold(OFFSET, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(PRIME)
    })
}

/// Authority-bearing identity for a closed conformance application.
///
/// The owner is deliberately outside this commitment: ownership is an exact
/// independent join, while this value commits to the reusable semantic
/// application structure itself.
pub fn closed_conformance_application_commitment(
    application: &ClosedConformanceApplication,
) -> ClosedConformanceApplicationCommitment {
    fn push(digest: &mut Sha256, value: &str) {
        digest.update((value.len() as u64).to_le_bytes());
        digest.update(value.as_bytes());
    }

    let mut digest = Sha256::new();
    digest.update(b"omega.psi.terminal.closed-conformance-application.v3\0");
    push(&mut digest, &application.declaration_identity);
    push(
        &mut digest,
        application
            .subject_identity
            .as_deref()
            .unwrap_or("<subjectless>"),
    );
    push(&mut digest, &application.trait_identity);
    digest.update((application.trait_lifetime_arguments.len() as u64).to_le_bytes());
    for argument in &application.trait_lifetime_arguments {
        push(&mut digest, argument);
    }
    digest.update((application.telescope.len() as u64).to_le_bytes());
    for binding in &application.telescope {
        push(&mut digest, &binding.parameter);
        digest.update([match binding.kind {
            ClosedConformanceParameterKind::Lifetime => 1,
            ClosedConformanceParameterKind::Type => 2,
            ClosedConformanceParameterKind::Const => 3,
            ClosedConformanceParameterKind::Machine => 4,
        }]);
        push(&mut digest, &binding.argument);
    }
    digest.update((application.trait_arguments.len() as u64).to_le_bytes());
    for argument in &application.trait_arguments {
        push(&mut digest, argument);
    }
    digest.update((application.realization_callables.len() as u64).to_le_bytes());
    for callable in &application.realization_callables {
        push(&mut digest, &callable.source_callable_identity);
        digest.update(callable.machine.get().to_le_bytes());
        digest.update([match callable.result {
            ClosedConformanceCallableResult::Unit => 1,
            ClosedConformanceCallableResult::I32 => 2,
            ClosedConformanceCallableResult::Bool => 3,
        }]);
    }
    digest.update((application.rows.len() as u64).to_le_bytes());
    for row in &application.rows {
        push(&mut digest, &row.declaring_trait_identity);
        push(&mut digest, &row.public_requirement_identity);
        push(&mut digest, &row.requirement_identity);
        push(&mut digest, &row.realization_identity);
        digest.update([u8::from(row.realization_callable_identity.is_some())]);
        if let Some(identity) = &row.realization_callable_identity {
            push(&mut digest, identity);
        }
    }
    ClosedConformanceApplicationCommitment::from_digest(digest.finalize().into())
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralTypeDeclaration {
    pub id: StructuralTypeId,
    pub identity: String,
    pub shape: StructuralTypeShape,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralTypeShape {
    /// One whole primitive scalar held behind structural ownership/borrowing
    /// custody. This is a semantic referent shape, not a native layout claim.
    PrimitiveScalar(ScalarType),
    /// One immutable borrowed view over an exact sequence of bytes. The bytes
    /// are semantic payload, not UTF-8 text and not a native pointer/layout.
    ByteSequence(ByteSequenceCarrier),
    Record {
        /// Declaration order is semantic. Field IDs must nevertheless be
        /// strictly increasing so the same record has one canonical spelling.
        fields: Vec<StructuralFieldDeclaration>,
    },
    FixedArray {
        element: StructuralTypeId,
        length: u64,
    },
    /// A closed pure sum. Case and payload-field declaration order is semantic;
    /// their IDs are strictly increasing in the canonical encoding.
    Sum {
        cases: Vec<StructuralCaseDeclaration>,
    },
    /// A closed sum with fields available independently of the selected case.
    /// Common-field and case declaration order is semantic and all IDs are
    /// canonical within their respective namespaces.
    Mixed {
        fields: Vec<StructuralFieldDeclaration>,
        cases: Vec<StructuralCaseDeclaration>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralCaseDeclaration {
    pub id: StructuralCaseId,
    pub identity: String,
    pub fields: Vec<StructuralFieldDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralFieldDeclaration {
    pub id: StructuralFieldId,
    pub identity: String,
    /// Authored semantic relevance. Erased rows remain in terminal identity and
    /// proof structure even though Omega omits them from native layout.
    pub relevance: BindingRelevance,
    pub field_type: StructuralFieldType,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralFieldType {
    Scalar(ScalarType),
    /// Relevant IEEE leaf retained for structural identity and predicates.
    IeeeFloat(IeeeFloatFormat),
    ByteSequence(ByteSequenceCarrier),
    Structural(StructuralTypeId),
    /// Exact semantic type identity for an erased field whose carrier need not
    /// belong to the executable structural/layout vocabulary.
    Erased {
        type_identity: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ByteSequenceCarrier {
    BorrowedView,
    BoundedOwned { capacity: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralDomainDeclaration {
    pub id: StructuralDomainId,
    /// Stable source-free semantic-domain identity, distinct from this
    /// module-local dense declaration ID.
    pub semantic_domain: DomainSemanticId,
    pub identity: String,
    /// Exact carrier accepted by this domain. Qualification never changes the
    /// runtime carrier and never authorizes its own establishment.
    pub carrier: StructuralTypeId,
    /// Owner-unique normalized `Content<A>` definition, when this
    /// qualification is content-bearing. This row is independent of any
    /// boundary route that may introduce a program-local occurrence; those
    /// routes must replay this exact definition rather than restating one.
    pub content_projection: Option<StructuralContentProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralContentProjection {
    pub identity: ContentProjectionIdentity,
    pub algebra: ContentAlgebra,
    pub expression: ContentProjectionExpression,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServiceDeclaration {
    pub id: ServiceId,
    pub identity: String,
    /// Strictly ordered canonical parent closure.
    pub parents: Vec<ServiceId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralMultiplicity {
    Unrestricted,
    Affine,
    Linear,
}

/// Semantic access carried by a structural parameter or call argument.
/// Borrowed variants intentionally share a physical pointer representation;
/// this closed axis prevents semantic authority from being erased by ABI
/// equivalence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralAccess {
    Owned,
    SharedBorrow,
    MutableBorrow,
    WriteOnlyBorrow,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralParameterDeclaration {
    pub place: PlaceId,
    pub position: u32,
    pub is_self: bool,
    pub structural_type: StructuralTypeId,
    pub multiplicity: StructuralMultiplicity,
    pub access: StructuralAccess,
    /// Strictly ordered exact signature preconditions. A parameter does not
    /// establish these facts by declaration: its caller or root installation
    /// must discharge them at invocation.
    pub qualifications: Vec<StructuralDomainId>,
    /// Strictly ordered exact qualification preconditions rooted beneath this
    /// parameter. Whole-root qualifications remain in `qualifications`; every
    /// row here must carry a nonempty path whose resolved structural type is
    /// the declared domain carrier.
    pub projected_qualifications: Vec<StructuralPathQualification>,
}

/// One exact qualification carried by a nonempty structural path beneath a
/// parameter root. The path is occurrence-relative, not a type-wide rule: a
/// qualification on one field never qualifies a sibling, prefix, or root.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralPathQualification {
    pub path: Vec<StructuralPathSegment>,
    pub domain: StructuralDomainId,
}

/// Exact normal structural result signature. The result place is proof-visible
/// and receives ownership only through a `ReturnStructural` edge.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralResultDeclaration {
    pub place: PlaceId,
    pub structural_type: StructuralTypeId,
    pub multiplicity: StructuralMultiplicity,
    /// Strictly ordered qualifications transferred with the value.
    pub qualifications: Vec<StructuralDomainId>,
    /// Strictly ordered exact qualifications transferred with nonempty paths
    /// beneath the result root.
    pub projected_qualifications: Vec<StructuralPathQualification>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// One qualification-only boundary call-admission check. The corresponding
/// structural argument must already carry `domain`; this row does not create a
/// proposition or mint an obligation identity.
pub struct StructuralDomainRequirement {
    pub argument_index: u32,
    pub domain: StructuralDomainId,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoundaryMachineDeclaration {
    pub id: BoundaryMachineId,
    pub identity: String,
    pub attachment: Option<StructuralTypeId>,
    /// Ordered primitive scalar parameters. Boundary calls bind their scalar
    /// arguments positionally and preserve this authored order exactly.
    pub scalar_parameters: Vec<ScalarType>,
    /// Ordered runtime structural parameters, independently positional from
    /// the scalar lane. A primitive scalar result is retained when the
    /// successful invocation returns a runtime status/value.
    pub structural_parameters: Vec<StructuralParameterDeclaration>,
    pub result: Option<ScalarType>,
    /// Strictly ordered qualification checks by `(argument_index, domain)`.
    /// Admission consumes qualifications already carried by the arguments;
    /// these rows are not proof propositions.
    pub requires: Vec<StructuralDomainRequirement>,
    /// Exact portable schemas authorized by this requirement's domain routes.
    /// These rows describe per-occurrence capacity but introduce no authority;
    /// installation must still bind a concrete occurrence and cardinality.
    pub program_local_root_introductions: Vec<ProgramLocalRootIntroductionSchema>,
    /// Authored content guarantees of this exact boundary requirement. These
    /// are provider assumptions, not executable proof terms; a caller may use
    /// one only through the successful `BoundaryCall` operation that selected
    /// this declaration.
    pub content_guarantees: Vec<ContentConservationGuarantee>,
    /// Strictly ordered normalized published ceiling.
    pub published_service_ceiling: Vec<ServiceId>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentConservationGuarantee {
    /// Non-authoritative compact coordinate for reports and cache joins. The
    /// exact retained conservation equation and structural-place replay carry
    /// theorem authority.
    pub report_fingerprint: u64,
    /// Guarantee-local structural roots, alpha-matched to the boundary
    /// signature by parameter position.
    pub structural_places: Vec<StructuralPlaceDeclaration>,
    pub conservation: ContentConservation,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProgramLocalRootIntroductionSchema {
    /// Dense index in this boundary declaration's structural argument lane.
    pub argument_index: u32,
    /// Authored semantic parameter position before scalar/structural lanes split.
    pub source_parameter_position: u32,
    pub qualification: StructuralDomainId,
    pub carrier: StructuralTypeId,
    pub projection: ContentProjectionIdentity,
    pub algebra: ContentAlgebra,
    pub capacity: ContentProjectionExpression,
    /// Non-authoritative compatibility report identity of all fields above
    /// plus the enclosing requirement. Exact schema fields and owner
    /// projection replay carry semantic authority.
    pub compatibility_report_identity: u64,
}

pub fn program_local_root_introduction_compatibility_report_identity(
    requirement_identity: &str,
    qualification_identity: &str,
    carrier_identity: &str,
    schema: &ProgramLocalRootIntroductionSchema,
) -> u64 {
    fn bytes(hash: &mut u64, value: &[u8]) {
        for byte in value {
            *hash ^= u64::from(*byte);
            *hash = hash.wrapping_mul(1_099_511_628_211);
        }
    }
    fn string(hash: &mut u64, value: &str) {
        bytes(hash, &(value.len() as u64).to_le_bytes());
        bytes(hash, value.as_bytes());
    }
    fn scalar(hash: &mut u64, value: &ContentProjectionScalar) {
        match value {
            ContentProjectionScalar::SubjectField(path)
            | ContentProjectionScalar::RuntimeScalarEmbedding(path) => {
                bytes(
                    hash,
                    &[
                        if matches!(value, ContentProjectionScalar::SubjectField(_)) {
                            1
                        } else {
                            2
                        },
                    ],
                );
                bytes(hash, &(path.len() as u64).to_le_bytes());
                for segment in path {
                    string(hash, segment);
                }
            }
            ContentProjectionScalar::Natural(value) => {
                bytes(hash, &[3]);
                string(hash, value);
            }
            ContentProjectionScalar::Successor(inner) => {
                bytes(hash, &[4]);
                scalar(hash, inner);
            }
            ContentProjectionScalar::Add(left, right)
            | ContentProjectionScalar::Subtract(left, right)
            | ContentProjectionScalar::Multiply(left, right) => {
                bytes(
                    hash,
                    &[match value {
                        ContentProjectionScalar::Add(_, _) => 5,
                        ContentProjectionScalar::Subtract(_, _) => 6,
                        ContentProjectionScalar::Multiply(_, _) => 7,
                        _ => unreachable!(),
                    }],
                );
                scalar(hash, left);
                scalar(hash, right);
            }
        }
    }
    let mut hash = 14_695_981_039_346_656_037_u64;
    bytes(&mut hash, b"psi.program-local-root-introduction.v1");
    string(&mut hash, requirement_identity);
    string(&mut hash, qualification_identity);
    string(&mut hash, carrier_identity);
    bytes(&mut hash, &schema.argument_index.to_le_bytes());
    bytes(&mut hash, &schema.source_parameter_position.to_le_bytes());
    bytes(
        &mut hash,
        &schema
            .projection
            .projection_report_fingerprint
            .to_le_bytes(),
    );
    bytes(
        &mut hash,
        &[match schema.algebra.kind {
            psi_core::ContentAlgebraKind::IntervalSet => 1,
            psi_core::ContentAlgebraKind::CountedQuantity => 2,
        }],
    );
    string(&mut hash, &schema.algebra.parameter);
    match &schema.capacity {
        ContentProjectionExpression::IntervalSet(members) => {
            bytes(&mut hash, &[1]);
            bytes(&mut hash, &(members.len() as u64).to_le_bytes());
            for (start, end) in members {
                scalar(&mut hash, start);
                scalar(&mut hash, end);
            }
        }
        ContentProjectionExpression::CountedQuantity(magnitude) => {
            bytes(&mut hash, &[2]);
            scalar(&mut hash, magnitude);
        }
    }
    if hash == 0 { 1 } else { hash }
}

/// One exact checked provider candidate for a Unit boundary requirement.
///
/// The candidate body is an ordinary terminal machine. The extra row binds it
/// to the requirement and records the structured signature/refinement witness
/// independently checked by the terminal verifier. A readable method spelling
/// is deliberately absent; `requirement_identity` is the canonical overload
/// identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderCandidateConformance {
    pub boundary: BoundaryMachineId,
    pub requirement_identity: String,
    pub provider_identity: String,
    /// Canonical checked-machine identity named by the selected
    /// `CheckedAdapter` row. The dense `candidate` ID is artifact-local.
    pub candidate_identity: String,
    pub candidate: MachineId,
    pub signature: ProviderUnitSignature,
    pub refinement: ProviderUnitRefinement,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderUnitSignature {
    pub parameters: Vec<ProviderSignatureParameter>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderSignatureParameter {
    pub position: u32,
    pub is_self: bool,
    pub structural_type: StructuralTypeId,
    pub multiplicity: StructuralMultiplicity,
    pub access: StructuralAccess,
    pub qualifications: Vec<StructuralDomainId>,
    pub projected_qualifications: Vec<StructuralPathQualification>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderParameterRefinement {
    pub boundary_index: u32,
    pub candidate_index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderUnitRefinement {
    /// Complete dense positional correspondence between requirement and
    /// candidate parameters. Reordering cannot hide behind equal types.
    pub positional_parameters: Vec<ProviderParameterRefinement>,
    /// Exact boundary-domain premises inherited by the candidate.
    pub required_domains: Vec<StructuralDomainRequirement>,
    /// Exact checked candidate reach, proved to refine the boundary ceiling.
    pub realized_service_ceiling: Vec<ServiceId>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PropositionDeclaration {
    pub id: psi_core::PropositionId,
    pub name: String,
    pub binders: Vec<PropositionBinderDeclaration>,
    pub parameter_types: Vec<String>,
    pub evidence: PropositionEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PropositionBinderDeclaration {
    pub name: String,
    pub kind: PropositionBinderKind,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PropositionBinderKind {
    Type,
    Const { type_identity: String },
    Machine,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PropositionEvidence {
    FactOnly,
    Witness { evidence_type: String },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PropositionApplicationIdentity {
    pub id: psi_core::PropositionId,
    pub declaration: psi_core::PropositionId,
    pub binder_arguments: Vec<PropositionBinderArgumentIdentity>,
    pub arguments: Vec<String>,
    /// Exact instantiated carrierless interface. This is present exactly for
    /// witness-bearing applications and is their terminal identity authority.
    pub evidence_interface: Option<EvidenceInterfaceIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PropositionBinderArgumentIdentity {
    pub kind: PropositionBinderArgumentKind,
    /// Canonical identity of an ordinary static argument. Evidence
    /// projections leave this empty and use the structured carrier below.
    pub identity: String,
    pub evidence_projection: Option<EvidenceProjectionIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvidenceProjectionIdentity {
    pub term: EvidenceTermId,
    pub declaring_trait_identity: String,
    pub declaring_trait_arguments: Vec<String>,
    pub requirement_identity: String,
}

/// One exact carrierless witness identity retained independently of both its
/// nominal proposition and the proof provenance that established it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvidenceTermDeclaration {
    pub id: EvidenceTermId,
    /// Exact normalized proposition application inhabited by this term.
    pub proposition: psi_core::PropositionId,
    /// Source-handle-free exact carrierless interface. This structured row,
    /// not `PropositionEvidence::Witness::evidence_type`, is the terminal
    /// identity authority for projection.
    pub interface: EvidenceInterfaceIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvidenceInterfaceIdentity {
    pub trait_identity: String,
    pub arguments: Vec<String>,
    /// Complete canonical direct and inherited proof-static surface.
    pub requirements: Vec<EvidenceRequirementIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvidenceRequirementIdentity {
    pub declaring_trait_identity: String,
    pub declaring_trait_arguments: Vec<String>,
    pub requirement_identity: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EvidenceContractLaneKind {
    Requires,
    Ensures,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvidenceContractLane {
    pub machine: MachineId,
    pub kind: EvidenceContractLaneKind,
    pub position: u32,
    pub term: EvidenceTermId,
    /// Public named proof output. Present exactly on an
    /// `ensures` lane; `requires` names remain local input aliases.
    pub output_field: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProofOutputCall {
    pub caller: MachineId,
    /// Dense canonical order within the caller; source coordinates erase.
    pub ordinal: u32,
    /// Canonical checked callable identity, never a diagnostic display path.
    pub target_machine_identity: String,
    /// Exact private realization selected for a static trait-requirement call.
    /// The public target above remains the requirement callable identity; this
    /// row binds it to one closed conformance application and its emitted
    /// runtime realization without exposing the satisfier's evidence term.
    pub static_requirement_dispatch: Option<StaticRequirementDispatch>,
    /// Declared execution shape, independent of the operation link so a
    /// missing or spurious link is verifier-visible. `None` is erased proof
    /// construction; `Unit` and `Scalar` each require one ordinary call.
    pub runtime_result: Option<ProofOutputRuntimeResult>,
    /// Exact canonical ordinary call which produced `runtime_result`.
    pub runtime_call: Option<ProofOutputRuntimeCall>,
    /// Explicit erased inputs supplied to the callee's named `requires` lane.
    pub evidence_arguments: Vec<ProofOutputEvidenceArgument>,
    /// Complete canonical proof-output set, ordered by callee lane.
    pub outputs: Vec<ProofOutput>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StaticRequirementDispatch {
    /// Non-authoritative compatibility coordinate for the exact application
    /// owned by `ProofOutputCall::caller`.
    pub conformance_application_report_fingerprint: u64,
    /// Authority-bearing join to the complete closed application.
    pub conformance_application_commitment: ClosedConformanceApplicationCommitment,
    /// Canonical public requirement overload exposed to the caller. This is
    /// deliberately distinct from the selected row's declaration path.
    pub public_requirement_identity: String,
    /// Exact selected row within that closed application.
    pub declaring_trait_identity: String,
    pub requirement_identity: String,
    pub realization_identity: String,
    /// Canonical source callable independently joined through the selected
    /// closed-conformance row to `realization`.
    pub realization_callable_identity: String,
    /// Artifact-local machine emitted for the selected realization.
    pub realization: MachineId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProofOutputEvidenceArgument {
    pub input_position: u32,
    /// Formal proposition declared at this target lane. The lane itself is
    /// identified by target-machine identity plus `input_position`; it is not
    /// a produced evidence term.
    pub callee_proposition: PropositionId,
    pub source: EvidenceTermId,
    pub instantiated_proposition: PropositionId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProofOutputRuntimeResult {
    Unit,
    Scalar(ScalarType),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProofOutputRuntimeCall {
    pub operation: OperationId,
    pub callee: MachineId,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProofOutput {
    pub output_position: u32,
    /// Exact public proof selector from the callee lane.
    pub output_field: String,
    /// Formal proposition declared by the target lane.
    pub callee_proposition: PropositionId,
    /// Distinct producer-backed witness declaration. A directly forwarded
    /// input has no new callee term and records its input position below.
    pub callee_output: Option<EvidenceTermId>,
    /// Exact proposition after substituting this invocation's ordinary Type
    /// arguments, including when the caller omits this witness.
    pub instantiated_proposition: PropositionId,
    /// Input lane whose exact witness this output forwards. `None` means the
    /// callee produced a distinct witness with retained producer provenance.
    pub forwarded_input_position: Option<u32>,
    /// Distinct caller-local copy, or `None` when omitted or discarded.
    pub output: Option<EvidenceTermId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PropositionBinderArgumentKind {
    Type,
    Const,
    Machine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalMachine {
    pub id: MachineId,
    /// Nominal type to which this machine is attached. An attached static
    /// machine need not have a runtime `self` parameter.
    pub attachment: Option<StructuralTypeId>,
    pub parameters: Vec<ValueDeclaration>,
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
    /// Strictly ordered normalized executable boundary-service ceiling. Public
    /// machines retain their authored ceiling; private machines and executable
    /// entries retain their exact checked inferred reach.
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

/// One exact ranked strongly connected component in Terminal-Psi identity.
///
/// The current representation admits only the deliberately narrow unsigned
/// countdown shape. The row names Terminal identities exclusively; frontend
/// arena handles and source coordinates cannot survive this boundary.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalRankedScc {
    pub header: BlockId,
    pub rank_parameter: ValueId,
    pub rank_type: IntegerType,
    pub lower_bound: IntegerValue,
    pub upper_bound: IntegerValue,
    /// Strictly ordered by `edge`; every cyclic edge must appear exactly once.
    pub covered_cyclic_edges: Vec<TerminalRankedSccEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalRankedSccEdge {
    pub edge: EdgeId,
    pub source: BlockId,
    pub target: BlockId,
    pub guard: TerminalRankedGuard,
    pub successor_argument: TerminalRankedSuccessorArgument,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalRankedGuard {
    UnsignedParameterPositive {
        block: BlockId,
        edge: EdgeId,
        condition: ValueId,
        parameter: ValueId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalRankedSuccessorArgument {
    UnsignedParameterMinusOne {
        argument_index: u32,
        argument: ValueId,
        source_parameter: ValueId,
        target_parameter: ValueId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntryClaim {
    pub claim: ClaimId,
    /// Structural parameter root that owns this claim.
    pub input: PlaceId,
    /// Statically typed structural projection below `input`. Empty names the
    /// complete parameter.
    pub path: Vec<StructuralPathSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralPathSegment {
    Field(String),
    FixedIndex(u64),
}

impl From<String> for StructuralPathSegment {
    fn from(identity: String) -> Self {
        Self::Field(identity)
    }
}

impl From<&str> for StructuralPathSegment {
    fn from(identity: &str) -> Self {
        Self::Field(identity.to_owned())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralPlaceDeclaration {
    pub id: PlaceId,
    pub kind: StructuralPlaceKind,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClaimContentProjection {
    pub projection: ContentProjectionIdentity,
    pub algebra: ContentAlgebra,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentEntryClaim {
    pub claim: ClaimId,
    pub input: ContentStructuralPlace,
    /// Strictly ordered by `(projection, algebra)` in canonical modules.
    pub projections: Vec<ClaimContentProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentIdentityReshuffle {
    pub claim: ClaimId,
    pub input: ContentStructuralPlace,
    pub output: ContentStructuralPlace,
    /// Strictly ordered by `(projection, algebra)` in canonical modules.
    pub projections: Vec<ClaimContentProjection>,
}

impl ContentIdentityReshuffle {
    pub fn inferred_propositions(&self) -> impl Iterator<Item = Proposition> + '_ {
        self.projections.iter().map(|content| {
            Proposition::ContentConservation(ContentConservation::new(
                content.algebra.clone(),
                ContentTerm::Projection {
                    projection: content.projection,
                    subject: self.input.clone(),
                },
                ContentTerm::Projection {
                    projection: content.projection,
                    subject: self.output.clone(),
                },
            ))
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentPlaceSubstitution {
    pub source: ContentStructuralPlace,
    pub target: ContentStructuralPlace,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentPartitionComposition {
    /// Exact call operation whose successful normal completion establishes the
    /// source theorem used by this composition. Merely carrying this row is
    /// never semantic authority.
    pub producer_operation: OperationId,
    /// Non-authoritative compact coordinate for reporting and caches. The
    /// exact source equation below is independently reconstructed and replayed.
    pub source_report_fingerprint: u64,
    /// Structural-place declarations for the source callable's theorem. They
    /// live in a namespace local to this witness rather than the wrapper.
    pub source_structural_places: Vec<StructuralPlaceDeclaration>,
    pub source: ContentConservation,
    /// Dense machine-local claims whose exact entry projections participate.
    pub input_claims: Vec<ClaimId>,
    /// Strictly ordered by `source`; every source projection has exactly one
    /// substitution and all rows are used by replay.
    pub substitutions: Vec<ContentPlaceSubstitution>,
    pub derived: ContentConservation,
}

impl ContentPartitionComposition {
    pub fn inferred_proposition(&self) -> Proposition {
        Proposition::ContentConservation(self.derived.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineContract {
    pub id: ContractId,
    /// Strictly ordered canonical may-routes. Omitting a cause forbids it.
    pub crash_routes: Vec<CrashRouteBucket>,
    pub requires: Vec<Proposition>,
    pub ensures: Vec<ContractClause>,
    /// Outcome-specific guarantees remain disjoint from unconditional lanes.
    /// Canonical order is `(result_type, result_case, position)`.
    pub outcome_specific_ensures: Vec<OutcomeSpecificEnsure>,
}

/// Exact nominal result-case guard for one semantic guarantee row.
///
/// This is independent from the proposition and evidence-term identities. It
/// authorizes executable matching-exit replay only when Terminal verification
/// independently recognizes an exact case-producing return carrier; wider
/// structural control remains fail closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OutcomeSpecificGuard {
    pub result_type: StructuralTypeId,
    pub result_case: StructuralCaseId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeSpecificEnsure {
    pub guard: OutcomeSpecificGuard,
    /// Dense zero-based order within one exact result-case group.
    pub position: u32,
    pub obligation: ObligationId,
    pub proposition: Proposition,
    /// Present exactly for a named witness-bearing guarantee.
    pub evidence: Option<OutcomeSpecificEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeSpecificEvidence {
    pub term: EvidenceTermId,
    pub output_field: String,
}

/// One caller-local evidence term selected from an exact guarded callee row.
///
/// This carrier is proof-only. The guard remains conditional on the runtime
/// structural result; the binding neither asserts case membership nor adds an
/// operation. A bounded payloadless structural call may retain any selected
/// subset, canonically ordered by guarded row coordinate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeSpecificCallEvidence {
    pub guard: OutcomeSpecificGuard,
    pub position: u32,
    pub callee_obligation: ObligationId,
    pub callee_term: EvidenceTermId,
    pub output_field: String,
    /// Exact proposition application declared by the guarded callee row.
    pub callee_proposition: PropositionId,
    /// Exact caller-side application after substituting the call result.
    pub instantiated_proposition: PropositionId,
    pub output: EvidenceTermId,
    /// Present exactly when the proposition application mentions the complete
    /// structural result. This source-handle-free row, rather than application
    /// display strings, authorizes the one bounded substitution.
    pub result_substitution: Option<OutcomeSpecificCallResultSubstitution>,
    pub validity: OutcomeSpecificCallEvidenceValidity,
    /// Independent cardinality commitment for the bounded selected-term use
    /// lane. This keeps omission distinct from an intentionally unused row.
    pub expected_use_count: u32,
    pub uses: Vec<OutcomeSpecificEvidenceUse>,
}

/// One proof-only consumption of a selected guarded term by an exact direct
/// tail-state `requires` position. It adds no runtime edge or fuel unit.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OutcomeSpecificEvidenceUse {
    pub target: MachineId,
    pub input_position: u32,
    pub target_requirement: PropositionId,
    pub target_term: EvidenceTermId,
    pub source: EvidenceTermId,
    pub instantiated_proposition: PropositionId,
    pub target_parameter: PlaceId,
    pub caller_result: PlaceId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OutcomeSpecificCallResultSubstitution {
    pub argument_position: u32,
    pub callee_result: PlaceId,
    pub caller_result: PlaceId,
}

/// Source-handle-free roots of the checked guarded-term validity intersection.
///
/// The bounded payloadless call has no arguments or payload projections, so
/// every retained occurrence can name only its exact structural result root.
/// Interface identity is repeated deliberately so codec and verifier replay
/// cannot silently detach validity from the selected witness carrier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeSpecificCallEvidenceValidity {
    pub result: PlaceId,
    pub proposition_dependencies: Vec<PlaceId>,
    pub evidence_interface: EvidenceInterfaceIdentity,
    pub interface_dependencies: Vec<PlaceId>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CrashRouteBucket {
    pub cause: CrashCause,
    /// Canonical nonempty disjunction. `Truth`, when present, is the sole row.
    pub alternatives: Vec<CrashRouteGuard>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CrashRouteGuard {
    Truth,
    Predicate(CrashPredicateTerm),
}

/// Canonical source-independent term for one normalized crash predicate.
///
/// Terminal Psi retains the proposition itself. The verifier can therefore
/// type-check it, substitute callee values at a call, and reconstruct the exact
/// surviving continuation without trusting producer-authored identity bytes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CrashPredicateTerm(Proposition);

impl CrashPredicateTerm {
    pub const fn new(proposition: Proposition) -> Self {
        Self(proposition)
    }

    pub const fn proposition(&self) -> &Proposition {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractClause {
    pub obligation: ObligationId,
    pub proposition: Proposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub id: BlockId,
    pub parameters: Vec<ValueDeclaration>,
    pub operations: Vec<Operation>,
    pub terminator: Terminator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    pub id: OperationId,
    pub result: OperationResult,
    pub kind: OperationKind,
}

/// Runtime result of one operation. Unit creates no `ValueId` or structural
/// place. A structural result establishes its declared place only after the
/// operation succeeds.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OperationResult {
    Unit,
    Scalar(ValueDeclaration),
    Structural(StructuralOperationResult),
}

/// Exact structural value and caller-local claim frontier established only by
/// successful completion of one operation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralOperationResult {
    pub place: PlaceId,
    pub structural_type: StructuralTypeId,
    pub multiplicity: StructuralMultiplicity,
    pub qualifications: Vec<StructuralDomainId>,
    /// Strictly ordered exact qualifications rooted beneath `place`. Calls
    /// copy this roster exactly from the callee result declaration.
    pub projected_qualifications: Vec<StructuralPathQualification>,
    /// Strictly ordered caller-local claim occurrences rooted beneath `place`.
    pub claims: Vec<StructuralResultClaimBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralResultClaimBinding {
    pub claim: ClaimId,
    pub path: Vec<StructuralPathSegment>,
}

impl OperationResult {
    pub const fn scalar(&self) -> Option<ValueDeclaration> {
        match self {
            Self::Unit | Self::Structural(_) => None,
            Self::Scalar(value) => Some(*value),
        }
    }

    pub const fn scalar_ref(&self) -> Option<&ValueDeclaration> {
        match self {
            Self::Unit | Self::Structural(_) => None,
            Self::Scalar(value) => Some(value),
        }
    }

    pub fn scalar_mut(&mut self) -> Option<&mut ValueDeclaration> {
        match self {
            Self::Unit | Self::Structural(_) => None,
            Self::Scalar(value) => Some(value),
        }
    }

    pub const fn structural(&self) -> Option<&StructuralOperationResult> {
        match self {
            Self::Structural(result) => Some(result),
            Self::Unit | Self::Scalar(_) => None,
        }
    }

    /// Scalar-only consumer helper. Callers must reject Unit-capable operations
    /// before using this accessor.
    pub const fn expect_scalar(&self) -> ValueDeclaration {
        match self {
            Self::Scalar(value) => *value,
            Self::Unit | Self::Structural(_) => panic!("operation has no scalar result"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralArgument {
    pub place: PlaceId,
    pub path: Vec<StructuralPathSegment>,
    pub access: StructuralAccess,
}

/// One exact claim-free affine structural place disposed on an ordinary edge.
/// Unlike the root-only trivial discard vocabulary, this action retains the
/// canonical path and independently checkable leaf type reached by that path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralAffineDiscard {
    pub place: PlaceId,
    pub path: Vec<StructuralPathSegment>,
    pub structural_type: StructuralTypeId,
}

/// One whole claim-free affine structural parameter disposed by its exact
/// nominal cleanup machine. Unlike a trivial affine discard, this action is
/// executable edge work and therefore retains the selected machine identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NominalAffineCleanup {
    pub place: PlaceId,
    pub structural_type: StructuralTypeId,
    pub cleanup_machine: MachineId,
    /// Target-contract-local proof root for the borrowed cleanup receiver.
    /// This is not an executable structural parameter or ABI argument.
    pub cleanup_receiver: Option<PlaceId>,
    /// Obligation identities aligned positionally with the selected cleanup
    /// machine's contextual `requires` clauses.
    pub requirement_obligations: Vec<ObligationId>,
}

/// One exact affine cleanup action committed by a terminal ownership edge.
/// The surrounding vector is the semantic execution order; consumers must not
/// regroup actions by kind or reconstruct their order from declarations.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalAffineCleanupAction {
    DiscardRoot(PlaceId),
    DiscardResidual(StructuralAffineDiscard),
    InvokeNominal(NominalAffineCleanup),
}

/// Transfer one caller-local live claim through the structural argument at
/// `argument_index`. The callee reconstructs its own entry claim from that
/// parameter; callers cannot author callee-local claim identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClaimTransfer {
    pub claim: ClaimId,
    pub argument_index: u32,
}

/// Transfer one claim returned by an in-module structural callee back into the
/// caller's claim namespace. Claim identities are machine-local, so neither
/// side may infer that equal numeric ids denote the same occurrence. The
/// returned claim's structural path is reconstructed from the callee's
/// verified result frontier and preserved beneath the operation result place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralResultClaimTransfer {
    pub callee_claim: ClaimId,
    pub caller_claim: ClaimId,
}

/// Correlate successful completion of one exact bodyless boundary invocation
/// with one caller-local live claim and structural argument position.
///
/// The receipt becomes effective only after the boundary effect succeeds. A
/// rejected effect consumes no claim, so it cannot acknowledge completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CompletionReceipt {
    pub claim: ClaimId,
    pub argument_index: u32,
}

/// Closed operation vocabulary for the current pre-release compiler.
///
/// `IntegerConstant` writes the declared integer value to its result and
/// establishes the semantic axiom `result == literal`. It cannot trap and
/// generates no additional obligation because construction verifies that the
/// literal belongs to the declared terminal integer type.
///
/// `BooleanConstant` writes the declared Boolean value to its result and
/// establishes `result == literal`.
///
/// `WrappingIntegerAdd` reads two values of
/// the result's exact integer type and reduces their sum modulo the declared
/// width. Signed values interpret the reduced bits as two's complement. It is
/// total and therefore generates no overflow obligation; the verifier
/// reconstructs its exact result-term axiom.
///
/// `SaturatingIntegerMultiply` reads two
/// values of the result's exact integer type and clamps their product at that
/// type's representable bounds. It is total and generates no overflow
/// obligation; the verifier reconstructs its exact result-term axiom.
///
/// `WrappingIntegerMultiply` reads two
/// values of the result's exact integer type and reduces their product modulo
/// the declared width. Signed values interpret the reduced bits as two's
/// complement. It is total and generates no overflow obligation; the verifier
/// reconstructs its exact result-term axiom.
///
/// `SaturatingIntegerSubtract` reads two
/// values of the result's exact integer type and clamps `left - right` at that
/// type's representable bounds. It is total and generates no overflow
/// obligation; the verifier reconstructs its exact result-term axiom.
///
/// `SaturatingIntegerAdd` reads two values
/// of the result's exact integer type and clamps their sum at that type's
/// representable bounds. It is total and therefore generates no overflow
/// obligation; the verifier reconstructs its exact result-term axiom.
///
/// `WrappingIntegerSubtract` reads two
/// values of the result's exact integer type and reduces `left - right` modulo
/// the declared width. Signed values interpret the reduced bits as two's
/// complement. It is total and generates no overflow obligation; the verifier
/// reconstructs its exact result-term axiom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationKind {
    /// Store one already-defined scalar value through one exact whole-root
    /// mutable or write-only structural parameter. The operation does not
    /// observe the previous referent value, and structural custody is
    /// preserved.
    WriteOnlyPrimitiveStore {
        destination: PlaceId,
        value: ValueId,
    },
    /// Store one already-defined scalar into one exact relevant field beneath
    /// a structural parameter. `path` resolves from the parameter root to the
    /// record containing `field`; authority remains on the parameter
    /// declaration rather than being repeated by the operation.
    StructuralScalarFieldStore {
        destination: PlaceId,
        path: Vec<StructuralPathSegment>,
        field: StructuralFieldId,
        value: ValueId,
    },
    /// Establish one exact payloadless case of a declared structural sum. The
    /// destination and structural type are carried by the structural operation
    /// result; this row contributes the exact case-membership fact without
    /// inventing payload fields or runtime scalar work.
    EstablishPayloadlessCase {
        result_case: StructuralCaseId,
    },
    /// Establish one immutable borrowed byte-sequence literal in a declared
    /// structural place. `bytes` are exact octets; no text transcoding occurs.
    EstablishByteSequenceLiteral {
        destination: PlaceId,
        bytes: Vec<u8>,
    },
    /// Establish one whole, claim-free affine empty-record local. This is a
    /// semantic ownership event, not an ABI input or a target storage choice.
    EstablishTrivialAffineLocal {
        destination: PlaceId,
    },
    /// Invoke one in-module machine with positional scalar arguments. Each
    /// callee `requires` clause has the obligation identity at the same index;
    /// successful return binds the operation result. `crash_continuations`
    /// records the invocation-specific no-successor routes that survive call
    /// composition. The verifier reconstructs guarded in-module routes by
    /// substituting callee parameter values with these exact argument values.
    Call {
        callee: MachineId,
        arguments: Vec<ValueId>,
        requirement_obligations: Vec<ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Invoke one in-module Unit machine with positional structural arguments.
    CallUnit {
        callee: MachineId,
        structural_arguments: Vec<StructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Invoke one in-module scalar-result machine with positional structural
    /// arguments. This is the scalar-result counterpart of `CallUnit`: exact
    /// structural custody crosses the call while successful return binds the
    /// operation result.
    CallStructuralScalar {
        callee: MachineId,
        structural_arguments: Vec<StructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Invoke one scalar-result requirement through an owner-local dynamic
    /// descriptor. Exact descriptor versions, conformance application, table
    /// row, and realization callable remain in the module dynamic-dispatch
    /// catalog. This operation intentionally carries no static callee or raw
    /// source argument.
    CallDynamicScalar {
        descriptor_ordinal: u32,
        requirement_obligations: Vec<ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Invoke one scalar-result requirement through an existential descriptor
    /// received as a machine parameter. The dynamic catalog owns the exact
    /// parameter interface and operation-to-slot join; this operation retains
    /// only the executable coordinates.
    CallDynamicParameterScalar {
        parameter_ordinal: u32,
        requirement_slot: u32,
        requirement_obligations: Vec<ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Invoke one in-module structural-result machine. The general form
    /// transfers input claims and applies the exact returned-claim namespace
    /// mapping on normal return. The bounded payloadless form instead has no
    /// arguments, claims, or ordinary contract lanes and returns one
    /// unrestricted exact structural case. Crash and suspension paths
    /// establish neither result twice.
    CallStructural {
        callee: MachineId,
        structural_arguments: Vec<StructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        returned_claim_transfers: Vec<StructuralResultClaimTransfer>,
        requirement_obligations: Vec<ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
        /// Canonical proof-only selections from exact outcome-specific callee
        /// rows. Each is valid only beneath its matching result-case refinement
        /// and has no runtime representation or fuel cost.
        selected_evidence: Vec<OutcomeSpecificCallEvidence>,
    },
    /// Invoke one exact bodyless boundary machine. Completion receipts
    /// name every live caller claim consumed by the successful invocation at
    /// its exact structural argument position. The operation result must agree
    /// with the boundary declaration's optional scalar result.
    BoundaryCall {
        boundary: BoundaryMachineId,
        /// Positional scalar arguments in the boundary declaration's exact
        /// authored parameter order.
        arguments: Vec<ValueId>,
        structural_arguments: Vec<StructuralArgument>,
        completion_receipts: Vec<CompletionReceipt>,
    },
    /// Immediate x86 port-space byte output. This first closed variant retains
    /// exactly a `u16` port and `u8` value; runtime operands are a later slice.
    /// The exact service identity is carried by the operation rather than
    /// rediscovered from a declaration name by downstream consumers.
    PortWrite {
        service: ServiceId,
        port: u16,
        value: u8,
    },
    IntegerConstant {
        value: IntegerValue,
    },
    BooleanConstant {
        value: bool,
    },
    /// Establish one exact runtime IEEE scalar from its interchange bits.
    IeeeFloatConstant {
        value: IeeeFloatValue,
    },
    /// Compute `round_nearest_even(left * right + addend)` in the result's
    /// exact IEEE format. This remains distinct from multiply-then-add.
    NearestIeeeFloatFusedMultiplyAdd {
        left: ValueId,
        right: ValueId,
        addend: ValueId,
    },
    /// Read one direct relevant Boolean field from an entry structural
    /// parameter. The canonical field identity, rather than an authored name
    /// or native byte offset, is part of terminal-Psi semantics; Omega selects
    /// and validates the target ABI load.
    BooleanStructuralField {
        source: PlaceId,
        field: StructuralFieldId,
    },
    /// Read one direct relevant integer field from a structural parameter.
    /// The exact integer type is carried by the scalar result declaration;
    /// the field identity remains type-local Terminal custody.
    IntegerStructuralField {
        source: PlaceId,
        field: StructuralFieldId,
    },
    BooleanNot {
        operand: ValueId,
    },
    BooleanEqual {
        left: ValueId,
        right: ValueId,
    },
    IntegerEqual {
        left: ValueId,
        right: ValueId,
    },
    IntegerLessThan {
        left: ValueId,
        right: ValueId,
    },
    IntegerLessOrEqual {
        left: ValueId,
        right: ValueId,
    },
    IntegerBitwiseNot {
        operand: ValueId,
    },
    IntegerWiden {
        operand: ValueId,
    },
    IntegerExactCast {
        operand: ValueId,
        obligation: ObligationId,
    },
    IntegerBitwiseAnd {
        left: ValueId,
        right: ValueId,
    },
    IntegerBitwiseOr {
        left: ValueId,
        right: ValueId,
    },
    IntegerBitwiseXor {
        left: ValueId,
        right: ValueId,
    },
    WrappingIntegerShiftLeft {
        value: ValueId,
        count: ValueId,
    },
    WrappingIntegerShiftRight {
        value: ValueId,
        count: ValueId,
    },
    ExactIntegerShiftLeft {
        value: ValueId,
        count: ValueId,
        obligation: ObligationId,
    },
    ExactIntegerShiftRight {
        value: ValueId,
        count: ValueId,
        obligation: ObligationId,
    },
    ExactIntegerAdd {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    ExactIntegerSubtract {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    ExactIntegerMultiply {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    ExactIntegerDivide {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    ExactIntegerRemainder {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    WrappingIntegerDivide {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    WrappingIntegerRemainder {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    SaturatingIntegerDivide {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    SaturatingIntegerRemainder {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    WrappingIntegerAdd {
        left: ValueId,
        right: ValueId,
    },
    SaturatingIntegerAdd {
        left: ValueId,
        right: ValueId,
    },
    WrappingIntegerSubtract {
        left: ValueId,
        right: ValueId,
    },
    SaturatingIntegerSubtract {
        left: ValueId,
        right: ValueId,
    },
    WrappingIntegerMultiply {
        left: ValueId,
        right: ValueId,
    },
    SaturatingIntegerMultiply {
        left: ValueId,
        right: ValueId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Terminator {
    /// Simultaneously bind target block parameters from the listed values.
    Jump {
        edge: EdgeId,
        target: BlockId,
        arguments: Vec<ValueId>,
        /// Exact no-code affine discards performed after edge fuel and outgoing
        /// scalar materialization, in reverse parameter declaration order.
        trivial_affine_discards: Vec<PlaceId>,
    },
    /// Select exactly one ordered successor from an already-defined Boolean
    /// value. Exhaustiveness and mutual exclusion are structural.
    Conditional {
        condition: ValueId,
        when_true: SuccessorEdge,
        when_false: SuccessorEdge,
    },
    /// Bind a scalar result, then perform the exact ordered affine cleanup
    /// actions before returning to the caller.
    Return {
        edge: EdgeId,
        value: ValueId,
        /// Semantic execution order. Consumers must preserve this list rather
        /// than regrouping actions by cleanup kind.
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
    },
    /// Finish normally without producing or binding a runtime value.
    ReturnUnit {
        edge: EdgeId,
        /// Exact no-code affine discards performed after outgoing-value
        /// materialization and before control returns to the caller.
        /// Entries are structural places in reverse declaration order.
        trivial_affine_discards: Vec<PlaceId>,
    },
    /// Finish normally after committing an exact projected transfer and then
    /// disposing only the remaining live affine structural places. This is a
    /// distinct pre-release variant so root-only consumers cannot silently
    /// erase path-sensitive cleanup.
    ReturnUnitPartialAffine {
        edge: EdgeId,
        trivial_affine_discards: Vec<PlaceId>,
        residual_affine_discards: Vec<StructuralAffineDiscard>,
    },
    /// Finish normally after executing the exact ordered nominal cleanups for
    /// whole affine structural parameters. Entries are in reverse parameter
    /// declaration order.
    ReturnUnitNominalAffine {
        edge: EdgeId,
        cleanups: Vec<NominalAffineCleanup>,
    },
    /// Transfer one structural value and its complete live claim set to the
    /// machine result. Fuel is charged before any transfer or cleanup commits.
    ReturnStructural {
        edge: EdgeId,
        source: PlaceId,
        /// Strictly ordered exact live claims transferred with `source`.
        returned_claims: Vec<ClaimId>,
        /// Exact no-code affine discards committed after result materialization.
        trivial_affine_discards: Vec<PlaceId>,
    },
    /// Leave checked execution without cleanup or a successor.
    ///
    /// `site_guard` is the canonical conjunction known on every path into this
    /// site. `frontier_lower_bound` is deliberately not described as the
    /// complete process-wide abandonment set: it is the machine-local claim
    /// frontier the verifier can reconstruct at this site.
    Crash {
        edge: EdgeId,
        cause: CrashCause,
        site_guard: Vec<CrashPredicateTerm>,
        frontier_lower_bound: Vec<ClaimId>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CrashCause {
    Trap,
    Abort,
}

impl Terminator {
    /// The sole edge of an unconditional terminator.
    ///
    /// Conditional consumers must use [`Self::edges`] or inspect the selected
    /// successor instead of silently treating one arm as the terminator edge.
    pub const fn edge(&self) -> EdgeId {
        match self {
            Self::Jump { edge, .. }
            | Self::Return { edge, .. }
            | Self::ReturnUnit { edge, .. }
            | Self::ReturnUnitPartialAffine { edge, .. }
            | Self::ReturnUnitNominalAffine { edge, .. }
            | Self::ReturnStructural { edge, .. }
            | Self::Crash { edge, .. } => *edge,
            Self::Conditional { .. } => {
                panic!("a conditional terminator has two successor edges")
            }
        }
    }

    pub fn edges(&self) -> impl Iterator<Item = EdgeId> + '_ {
        let (first, second) = match self {
            Self::Jump { edge, .. }
            | Self::Return { edge, .. }
            | Self::ReturnUnit { edge, .. }
            | Self::ReturnUnitPartialAffine { edge, .. }
            | Self::ReturnUnitNominalAffine { edge, .. }
            | Self::ReturnStructural { edge, .. }
            | Self::Crash { edge, .. } => (*edge, None),
            Self::Conditional {
                when_true,
                when_false,
                ..
            } => (when_true.edge, Some(when_false.edge)),
        };
        std::iter::once(first).chain(second)
    }
}

/// One ordered conditional successor and its simultaneous block-parameter
/// bindings. The bindings are the current scalar edge-action vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuccessorEdge {
    pub edge: EdgeId,
    pub target: BlockId,
    pub arguments: Vec<ValueId>,
    /// Exact no-code affine discards committed only when this successor is
    /// selected, in reverse parameter declaration order.
    pub trivial_affine_discards: Vec<PlaceId>,
}
