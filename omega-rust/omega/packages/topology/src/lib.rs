#![forbid(unsafe_code)]

//! Checked boundary topology: deployment-plan data, canonical wire codec, and
//! the independent source-free verifier.
//!
//! This crate implements the data plane of the
//! [topology contract](../../../../../wiki/spec/packages/topology.md): the
//! versioned `TopologyRequest`/plan schema, bounded deterministic graph
//! normalization, the fixed `no_route`/`only_via` reference predicates, and
//! `verify_plan`, the source-free consumer that reconstructs a plan's graph,
//! compares it against independently supplied owner intent, and replays the
//! selected predicates.
//!
//! Trust boundary (what this code may and may not decide):
//!
//! - The plan is package data, not a compiler verdict. `plan_composition` is the
//!   producer-side reference for the eventual build-only Omega package; it
//!   emits bytes only after every required policy evaluates satisfied, so a
//!   partial or failed composition cannot fabricate a checked result.
//! - `verify_plan` trusts nothing the plan asserts. A stored `Satisfied` flag
//!   is not evidence: the verifier replays each predicate on the graph it
//!   reconstructed itself and independently checks each recorded certificate
//!   (a reachable set must be closed under the actual edges and disjoint from
//!   the target set; a bypass witness must be a real path). Replaying the
//!   reference predicates decides the verdict; the structural certificate
//!   check is the correctness evidence that does not trust the producer's
//!   traversal.
//! - Owner intent arrives out of band. The verifier is handed the current
//!   request bytes and compares the plan's recorded commitment exactly; a
//!   correctly formed plan for a stale or weaker request still rejects.
//!   Policies whose recorded verifier differs from the request's selected
//!   verifier reject by identity comparison — policy executables are never
//!   loaded or run by this crate.
//! - Component facts arrive as evidence, not assertions. The verifier is
//!   handed the admitted component descriptions and reconstructs every
//!   instance's component record and endpoint inventory from them; a plan
//!   that merely decodes a `VerifiedComplete` tag or closure digest no
//!   admitted description establishes rejects. See [`verified_components`].
//! - `composition checked` and `installation admitted` stay distinct.
//!   Publication of exact plan bytes is not an installation claim; only the
//!   `topology_installation` module's gated activation produces the second, and its
//!   receipt binds the plan, artifacts, physical endpoint mapping, provider
//!   assumptions, and installation occurrence — evidence for one
//!   generation, never a reusable authorization.
//!
//! What this slice deliberately does not do: run inside a build evaluation
//! (the scoped build output route owns that orchestration) or prove OS-level
//! confinement — the pipe adapter's named assumptions disclose what the
//! hosts must establish. Hand-authored inventory is never verified closure:
//! every `Component`-role roster entry must bind an [`AdmittedComponent`].

pub mod deployment_plan;
pub mod plan_composition;
pub mod plan_verification;
pub mod topology_installation;
pub mod verified_components;

pub use deployment_plan::codec::{
    CodecError, MAX_ASSUMPTIONS, MAX_BINDINGS, MAX_ENDPOINTS_PER_INSTANCE, MAX_INSTANCES,
    MAX_NAME_BYTES, MAX_PLAN_BYTES, MAX_POLICIES, MAX_SELECTOR_MEMBERS, MAX_TRANSPORTS,
    PLAN_SCHEMA_VERSION, REQUEST_SCHEMA_VERSION, decode_plan, decode_request, encode_plan,
    encode_request,
};
pub use deployment_plan::graph::{GraphError, NormalizedGraph};
pub use deployment_plan::predicate::{
    CertificateRejection, PolicyEvaluation, SelectorError, SelectorRole, evaluate_policy,
    validate_selector,
};
pub use deployment_plan::{
    Binding, Certificate, Completeness, ComponentDescription, DeploymentPlan, Endpoint,
    EndpointDirection, EndpointKey, ExecutedPolicy, Identity, InstanceName, InstanceRole,
    PlanInstance, PolicyCall, PolicyOutcome, PolicyPredicate, PolicySelector, RequestedInstance,
    TopologyRequest, Violation, plan_subject, request_commitment,
};
pub use plan_composition::{CompositionError, CompositionFailure, compose_plan};
pub use plan_verification::{CheckedPlan, PlanRejection, verify_plan};
pub use verified_components::{
    AdmittedComponent, ComponentBindingFailure, component_subject_identity, verified_instance,
    verified_instance_facts,
};
