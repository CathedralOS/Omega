#![forbid(unsafe_code)]

//! Target-neutral resolved-language semantic identities, tables, and plans.
//!
//! This foundation vocabulary is shared by symbol-resolved and later Psi
//! representations. It contains no checking pass, target realization, or
//! backend policy.
//!
//! Start at `semantic_identities.rs`, the interned identities the tables key
//! on; every other folder holds one vocabulary. `permissions` names
//! ownership events, `external_bindings` interns external bindings,
//! `machine_termination` carries supply modes and termination plans,
//! `service_reach` the operational interfaces and reach tables,
//! `semantic_domains` the domain table and roles, and
//! `semantic_identities.rs` the interned identities. The public modules hold
//! the larger vocabularies: byte predicates, const values, content,
//! declaration selection, quotient correspondence, type identity, value
//! domains and wire.

pub mod byte_predicates;
pub mod const_value;
pub mod content;
pub mod declaration_selection;
mod external_bindings;
mod machine_termination;
mod permissions;
pub mod quotient_correspondence;
mod semantic_domains;
mod semantic_identities;
mod service_reach;
#[cfg(test)]
mod tests;
pub mod type_identity;
pub mod value_domain;
pub mod wire;

pub use external_bindings::{
    ExternalBindingIdentity, ExternalBindingMechanism, ExternalBindingTable,
};
pub use language_core::{
    CallOperationalAcknowledgement, CallOperationalAcknowledgementOrigin, CarryAddress, CarryCpu,
    CarryHostThread, CarryPermission, CarryPolicy, CarrySuspension, DataSupplyMode,
    DomainClassification, DomainPredicateBody, Multiplicity, ReferenceAccess,
};
pub use machine_termination::{
    MachineSupplyMode, MachineTerminationPlan, ProgressPremise, ProgressSubject,
    RANKING_VIEW_CORE_SOURCE, RankRange, RankingViewDeclaration, RankingWitness,
    TerminationGuarantee, TerminationInterface,
};
pub use permissions::{
    PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
    PermissionProvenance,
};
pub use semantic_domains::{
    DomainEstablishmentRoute, DomainSemanticRole, DomainSemanticRoles, QualificationEvidenceOrigin,
    SemanticDomainTable,
};
pub use semantic_identities::{
    ExternalBindingId, RankingViewId, SemanticDomainId, ServiceReachId, ServiceReachRowId,
};
pub use service_reach::{
    BlockingInterface, BlockingPlan, BlockingSummary, ServiceReachDefinition,
    ServiceReachInterface, ServiceReachPlan, ServiceReachRowTable, ServiceReachSummary,
    ServiceReachTable, SuspensionInterface, SuspensionPlan, SuspensionSummary,
    SynchronousInvocationInterface, SynchronousInvocationPlan,
};
