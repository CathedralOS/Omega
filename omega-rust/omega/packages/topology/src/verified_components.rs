//! Admitted component descriptions: the only honest source of a plan
//! instance's component record and endpoint inventory.
//!
//! A plan records, per instance, the component facts it consumed — subject,
//! admission profile, completeness closure, assumptions — and the public
//! endpoint inventory it composed. None of that is evidence until bound to
//! a description the component verifier actually admitted. This module owns
//! the one reconstruction both sides share: producers build roster entries
//! with [`verified_instance`], and `verify_plan` re-derives the same facts
//! for every roster entry and rejects a record that diverges.
//!
//! What the binding establishes for a `Component`-role instance:
//!
//! - `component.subject` is the plan-level identity of the admitted
//!   component's exact `TerminalPsiIdentity` — [`component_subject_identity`]
//!   is the convention the owner uses for `RequestedInstance.subject`.
//! - `component.verification_profile` is the `profile_identity()` of the
//!   consumer request the description was admitted under, so a plan cannot
//!   claim admission under a different profile.
//! - `component.completeness` is `VerifiedComplete` carrying the admission's
//!   closure digest, which binds the exact description bytes to that
//!   profile — the description's entry roster, outgoing authority, custody
//!   constraints, retained providers, and installation obligations ride
//!   inside that digest rather than re-appearing as plan fields.
//! - `component.assumptions` is the description's exact assumption roster,
//!   not merely a subset the owner happened to accept.
//! - `endpoints` is the verified inventory itself: each `ImportSlot`
//!   becomes an `Import` endpoint on its own slot with its canonical
//!   requirement-contract identity, and each `ExportSurface` becomes an
//!   `Export` endpoint on its canonical roster position with the surface's
//!   contract digest. Any extra or missing endpoint — an unaccounted
//!   communication path or a hidden demand — diverges here.
//!
//! An `ExternalParticipant` entry asserts the participant has no verified
//! component description. When a supplied admission does verify its
//! subject, that claim is itself a substitution and rejects.

use component_description::{ComponentVerificationRequest, VerifiedComponent};
use sha2::{Digest, Sha256};
use terminal_psi::TerminalPsiIdentity;

use crate::deployment_plan::{
    Completeness, ComponentDescription, Endpoint, EndpointDirection, Identity, InstanceName,
    InstanceRole, PlanInstance,
};

/// Domain for reducing a component's semantic subject to its plan-level
/// identity — distinct from the description, request, and plan digests.
const COMPONENT_SUBJECT_DOMAIN: &[u8] = b"omega-topology-component-subject-v1";

/// Domain for an export endpoint's contract identity — a digest of the
/// export surface's canonical identity string, never parsed apart.
const EXPORT_CONTRACT_DOMAIN: &[u8] = b"omega-topology-export-contract-v1";

/// One component description admitted under a known consumer request.
///
/// `component` is produced only by
/// `component_description::verify_component`, so the pair is evidence: the
/// description verified under exactly `request`, and it is `request` whose
/// `profile_identity()` the plan's `verification_profile` must name.
#[derive(Debug)]
pub struct AdmittedComponent {
    /// The admission request the description verified under.
    pub request: ComponentVerificationRequest,
    /// The admitted component.
    pub component: VerifiedComponent,
}

/// The plan-level identity of a component's exact semantic subject.
///
/// Owners write this into `RequestedInstance.subject`; verification uses it
/// to find the admission a `Component`-role instance must bind. The digest
/// covers the full `TerminalPsiIdentity`, so the same semantic fingerprint
/// under another vocabulary marker stays a distinct subject.
pub fn component_subject_identity(subject: &TerminalPsiIdentity) -> Identity {
    let mut digest = Sha256::new();
    digest.update(COMPONENT_SUBJECT_DOMAIN);
    digest.update(subject.vocabulary_marker.get().to_le_bytes());
    digest.update(subject.program_fingerprint.as_bytes());
    digest.finalize().into()
}

/// The contract identity an export endpoint records: the digest of the
/// export surface's canonical identity string. The string is compared
/// whole, never parsed.
fn export_contract_identity(export_identity: &str) -> Identity {
    let mut digest = Sha256::new();
    digest.update(EXPORT_CONTRACT_DOMAIN);
    digest.update(export_identity.as_bytes());
    digest.finalize().into()
}

/// Reconstruct the plan-side facts one admitted component establishes: the
/// exact `ComponentDescription` record and the public endpoint inventory in
/// canonical `(slot, direction)` order. Export slots are the export
/// roster's canonical positions — the description codec already orders
/// export surfaces by identity.
pub fn verified_instance_facts(
    admission: &AdmittedComponent,
) -> (ComponentDescription, Vec<Endpoint>) {
    let component = ComponentDescription {
        subject: component_subject_identity(&admission.component.subject()),
        verification_profile: admission.request.profile_identity(),
        completeness: Completeness::VerifiedComplete {
            closure: *admission.component.closure(),
        },
        assumptions: admission.component.assumptions().to_vec(),
    };
    let mut endpoints = Vec::new();
    for import in admission.component.imports() {
        endpoints.push(Endpoint {
            slot: import.slot,
            direction: EndpointDirection::Import,
            contract: import.contract_identity,
        });
    }
    for (index, export) in admission.component.exports().iter().enumerate() {
        endpoints.push(Endpoint {
            slot: index as u32,
            direction: EndpointDirection::Export,
            contract: export_contract_identity(&export.identity),
        });
    }
    endpoints.sort_by_key(|endpoint| (endpoint.slot, endpoint.direction));
    (component, endpoints)
}

/// A roster instance whose component record and endpoint inventory are
/// reconstructed entirely from an admitted description. The producer
/// chooses only the instance name; the role is `Component`.
pub fn verified_instance(name: InstanceName, admission: &AdmittedComponent) -> PlanInstance {
    let (component, endpoints) = verified_instance_facts(admission);
    PlanInstance {
        name,
        component,
        role: InstanceRole::Component,
        endpoints,
    }
}

/// How an instance's component record fails to bind the supplied
/// admissions. Producer composition and independent verification share
/// this judgment so both reject the same record identically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentBindingFailure {
    /// A `Component`-role instance names a subject no supplied admission
    /// verifies — its record has no establishing artifact at all.
    MissingVerifiedComponent,
    /// An `ExternalParticipant` names a subject a supplied admission
    /// verifies — a verified component cannot pose as unverified external
    /// inventory.
    ExternalSubjectVerified,
    /// The record diverges from the admission's reconstructed facts;
    /// `field` names the first divergent record.
    Substituted { field: &'static str },
}

impl std::fmt::Display for ComponentBindingFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingVerifiedComponent => {
                formatter.write_str("no admitted component verifies the instance's subject")
            }
            Self::ExternalSubjectVerified => formatter
                .write_str("an external participant claims a subject the admissions verify"),
            Self::Substituted { field } => {
                write!(
                    formatter,
                    "the record's {field} diverges from the verified facts"
                )
            }
        }
    }
}

impl std::error::Error for ComponentBindingFailure {}

/// Bind one instance's component record and endpoint inventory to the
/// supplied admissions. When several admissions verify one subject (the
/// same component admitted under different requests), any exact match
/// satisfies the binding; the first admission's divergence names the
/// field, keeping the rejection deterministic in slice order.
pub(crate) fn check_instance_binding(
    instance: &PlanInstance,
    components: &[AdmittedComponent],
) -> Result<(), ComponentBindingFailure> {
    let matching: Vec<&AdmittedComponent> = components
        .iter()
        .filter(|admission| {
            component_subject_identity(&admission.component.subject()) == instance.component.subject
        })
        .collect();
    match instance.role {
        InstanceRole::Component => {
            if matching.is_empty() {
                return Err(ComponentBindingFailure::MissingVerifiedComponent);
            }
            let mut divergence = None;
            for admission in &matching {
                match divergent_field(instance, admission) {
                    None => return Ok(()),
                    Some(field) => divergence = divergence.or(Some(field)),
                }
            }
            Err(ComponentBindingFailure::Substituted {
                field: divergence.expect("every candidate diverged"),
            })
        }
        InstanceRole::ExternalParticipant => {
            if matching.is_empty() {
                Ok(())
            } else {
                Err(ComponentBindingFailure::ExternalSubjectVerified)
            }
        }
    }
}

/// The first field on which `instance`'s record diverges from what
/// `admission` establishes. The subject itself never diverges: callers only
/// reach here on a subject match.
fn divergent_field(instance: &PlanInstance, admission: &AdmittedComponent) -> Option<&'static str> {
    let (component, endpoints) = verified_instance_facts(admission);
    if component.verification_profile != instance.component.verification_profile {
        return Some("verification_profile");
    }
    if component.completeness != instance.component.completeness {
        return Some("completeness");
    }
    if component.assumptions != instance.component.assumptions {
        return Some("assumptions");
    }
    if endpoints != instance.endpoints {
        return Some("endpoints");
    }
    None
}
