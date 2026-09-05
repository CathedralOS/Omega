//! The current, self-contained Terminal program.
//!
//! Start with `TerminalModule` below. Its declarations live in `types`,
//! `values`, `control_flow`, and `boundary`; `ownership` and `proof` retain
//! the semantic facts those declarations consume. `observation` defines what
//! consumers compare, while `identity` names the exact published vocabulary.
//! No target layout, selected installation, or transformation history lives here.

pub mod boundary;
pub mod control_flow;
pub mod identity;
pub mod observation;
pub mod ownership;
pub mod proof;
pub mod types;
pub mod values;

pub use boundary::*;
pub use control_flow::*;
pub use identity::*;
pub use observation::*;
pub use ownership::*;
pub use proof::*;
pub use types::*;
pub use values::*;

use semantic_vocabulary::MachineId;

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
    /// Exact source-free live frontier at each possibly-suspending ordinary
    /// call. This is a semantic carry demand only; it adds no control edge,
    /// cleanup action, activation choice, or park/resume behavior.
    pub suspension_call_plan_count: u32,
    /// Independent call-side roster. Every row must have exactly one detailed
    /// plan with the same key/target and committed frontier.
    pub suspension_call_sites: Vec<TerminalSuspensionCallSite>,
    pub suspension_call_plans: Vec<TerminalSuspensionCallPlan>,
    /// Canonical proof-only quotient correspondence, strictly ordered by its
    /// independently replayable identity. The public operation's hermetic
    /// identity is the semantic owner; these rows do not join an executable
    /// Terminal machine or authorize a representative call.
    pub quotient_correspondences: Vec<crate::RetainedQuotientCorrespondence>,
    pub machines: Vec<TerminalMachine>,
}
