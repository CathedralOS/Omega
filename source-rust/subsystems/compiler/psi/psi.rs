//! The portable Psi program shared by source publication, interpretation and native lowering.
//! The wire format, artifact envelope and receiving authorization have separate owners.

#![forbid(unsafe_code)]

/// A concrete program, its declared boundaries, and the evidence carried in its canonical encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Psi {
    pub scalar_applications: ScalarApplications,
    pub folded_constants: FoldedConstantClosure,
    pub indexed_theorems: Vec<TerminalIndexedTheorem>,
    pub mathematical_carriers: Vec<TerminalMathematicalCarrier>,
    pub identity_formations: TerminalIdentityFormations,
    pub quotients: quotients::TerminalQuotientCatalog,
    pub vocabulary_marker: VocabularyMarker,
    pub identities: TerminalIdentityTable,
    pub entry: Option<TargetMachineCallable>,
    pub scalar_qualifications: ScalarQualificationCatalog,
    pub structural_types: Vec<StructuralTypeDeclaration>,
    pub structural_referents: Vec<types::StructuralReferentIdentity>,
    pub structural_domains: Vec<StructuralDomainDeclaration>,
    pub structural_qualification_changes: Vec<types::StructuralQualificationChange>,
    pub services: Vec<ServiceDeclaration>,
    pub root_service_reach: TerminalRootServiceReach,
    pub access_view_inputs: Vec<TerminalAccessViewInput>,
    pub access_view_plans: Vec<TerminalAccessViewPlan>,
    pub access_lifecycle_plans: Vec<TerminalAccessLifecyclePlan>,
    pub reborrow_restored_call_uses: Vec<TerminalReborrowRestoredCallUse>,
    pub boundary_machines: Vec<BoundaryMachineDeclaration>,
    pub calling_signatures: Vec<crate::CallingSignatureDeclaration>,
    pub calling_abi_signatures: Vec<crate::CallingAbiDeclaration>,
    pub calling_policy_requests: Vec<crate::CallingPolicyRequest>,
    pub service_schemas: Vec<crate::TerminalServiceSchema>,
    pub program_entry_contracts: Vec<crate::TerminalProgramEntryContract>,
    pub provider_selections: Vec<crate::TerminalProviderSelection>,
    pub opaque_representations: Vec<crate::TerminalOpaqueRepresentation>,
    pub unoptimized_behavior: Vec<crate::UnoptimizedMachineBehavior>,
    pub external_providers: Vec<ExternalProviderConformance>,
    pub provider_candidates: Vec<ProviderCandidateConformance>,
    pub provider_instance_requirements: Vec<ProviderInstanceRequirements>,
    pub boundary_application_candidates: Vec<crate::BoundaryApplicationCandidate>,
    pub callback_applications: Vec<crate::TerminalCallbackApplication>,
    pub installed_interrupt_registrations: Vec<crate::TerminalInstalledInterruptRegistration>,
    pub dynamic_dispatch: TerminalDynamicDispatchCatalog,
    pub callables: crate::TerminalCallableCatalog,
    pub float_meaning_projections: Vec<crate::FloatMeaningProjection>,
    pub float_meaning_equalities: Vec<crate::FloatMeaningEqualityProposition>,
    pub proposition_declarations: Vec<PropositionDeclaration>,
    pub scalar_predicate_definitions: Vec<ScalarPredicateDefinition>,
    pub proposition_applications: Vec<PropositionApplicationIdentity>,
    pub evidence_terms: Vec<EvidenceTermDeclaration>,
    pub evidence_contract_lanes: Vec<EvidenceContractLane>,
    pub proof_output_calls: Vec<ProofOutputCall>,
    pub proof_recursive_components: Vec<TerminalProofRecursiveComponent>,
    pub scalar_block_invariants: Vec<ScalarBlockInvariant>,
    pub operation_crash_contracts: Vec<TerminalOperationCrashContract>,
    pub closed_conformance_applications: Vec<ClosedConformanceApplication>,
    pub suspension_call_plan_count: u32,
    pub suspension_call_sites: Vec<TerminalSuspensionCallSite>,
    pub suspension_call_plans: Vec<TerminalSuspensionCallPlan>,
    /// Real declaration contracts with no executable body, ordered by ContractId.
    pub declaration_contracts: Vec<crate::MachineContract>,
    pub machines: Vec<TerminalMachine>,
    pub target_layout_applications: Vec<crate::TargetLayoutApplication>,
    pub native_layouts: crate::NativeLayoutCatalog,
    pub boundary_application_demands: Vec<TerminalBoundaryApplicationDemand>,
    pub task_activation_demands: TerminalTaskActivationDemands,
}

#[path = "machines/published_entry.rs"]
mod published_entry;
#[path = "evidence/artifacts/mod.rs"]
pub mod artifacts;
#[path = "boundary/mod.rs"]
pub mod boundary;
#[path = "machines/call_contexts.rs"]
mod call_contexts;
#[path = "machines/callable_values.rs"]
pub mod callable_values;
#[path = "machines/generic_calls.rs"]
pub mod generic_calls;
#[macro_use]
#[path = "machines/control_flow.rs"]
pub mod control_flow;
#[path = "identity/mod.rs"]
pub mod identity;
#[macro_use]
#[path = "ownership/mod.rs"]
pub mod ownership;
#[path = "evidence/proof/mod.rs"]
pub mod proof;
#[path = "boundary/service_schemas.rs"]
pub mod service_schemas;
#[path = "machines/static_reach.rs"]
pub mod static_reach;
#[path = "types/structural_path_encoding.rs"]
pub mod structural_path_encoding;
#[path = "vocabulary.rs"]
pub mod terminal_module;
#[path = "types/mod.rs"]
pub mod types_and_values;
pub(crate) use ::wire::digest_encoder;
#[path = "machines/roots.rs"]
pub mod prepared_roots;
#[path = "boundary/callback_application.rs"]
pub mod callback_application;
#[path = "boundary/installed_interrupt.rs"]
pub mod installed_interrupt;
#[path = "boundary/calling_policies.rs"]
pub mod calling_policies;
#[path = "types/layouts/native_layouts.rs"]
pub mod native_layouts;
#[path = "machines/unoptimized_behavior.rs"]
pub mod unoptimized_behavior;
#[path = "boundary/callback_declarations.rs"]
pub mod callback_declarations;
#[path = "types/layouts/target_layouts.rs"]
pub mod target_layouts;
#[path = "machines/direct_calls.rs"]
pub mod direct_calls;

mod exports;
pub use exports::*;
