//! The independent source-free plan consumer.
//!
//! `verify_plan` is given the candidate plan bytes, the *current* owner
//! request bytes, and the admitted component descriptions the consumer
//! actually verified — never the plan's own claim about any of them. It
//! reconstructs the graph from the plan's records, compares the request
//! exactly, replays every required predicate itself, and checks the recorded
//! certificates against the real edges. Nothing is trusted: a stored success
//! flag, a correctly signed stale request, a roster that merely names the
//! right subjects, a policy executable the owner did not select, and a
//! component record or endpoint inventory substituted for what the admitted
//! description establishes all fail here by comparison or replay — never by
//! loading code.
//!
//! Success produces `CheckedPlan`: `composition checked` only. It is
//! deliberately not an `installation admitted` value — this crate binds no
//! endpoints, admits no executable, and grants no activation.

use crate::deployment_plan::codec::{CodecError, decode_plan, decode_request};
use crate::deployment_plan::graph::{GraphError, NormalizedGraph};
use crate::deployment_plan::predicate::{
    CertificateRejection, PolicyEvaluation, SelectorError, check_outcome, evaluate_policy,
};
use crate::deployment_plan::{
    Completeness, DeploymentPlan, Identity, InstanceName, PolicyCall, PolicyOutcome,
    TopologyRequest, plan_subject, request_commitment,
};
use crate::verified_components::{
    AdmittedComponent, ComponentBindingFailure, check_instance_binding,
};
use std::collections::BTreeMap;
use std::fmt;

/// Reconstruct and independently check a candidate plan against the current
/// owner request and the admitted component descriptions. Every
/// `Component`-role instance must bind to a supplied admission — subject,
/// admission profile, completeness closure, assumptions, and endpoint
/// inventory are reconstructed from it, so a forged or substituted record
/// rejects. All policies are checked in canonical order; the first rejection
/// is returned deterministically.
pub fn verify_plan(
    plan_bytes: &[u8],
    request_bytes: &[u8],
    components: &[AdmittedComponent],
) -> Result<CheckedPlan, PlanRejection> {
    let request = decode_request(request_bytes).map_err(PlanRejection::MalformedRequest)?;
    let plan = decode_plan(plan_bytes).map_err(|error| match error {
        CodecError::UnverifiedCompleteness { tag } => PlanRejection::UnverifiedCompleteness { tag },
        other => PlanRejection::MalformedPlan(other),
    })?;

    // Owner intent, exactly: the plan must name this request's commitment.
    let commitment = request_commitment(request_bytes);
    if plan.request_commitment != commitment {
        return Err(PlanRejection::StaleRequest {
            expected: commitment,
            found: plan.request_commitment,
        });
    }

    // Exact roster: same names in canonical order, each bound to the request's
    // component subject. An added, renamed, removed, or code-swapped instance
    // mismatches here.
    if plan.instances.len() != request.instances.len() {
        return Err(PlanRejection::RosterMismatch {
            detail: format!(
                "plan rosters {} instances, request requires {}",
                plan.instances.len(),
                request.instances.len()
            ),
        });
    }
    for (planned, required) in plan.instances.iter().zip(request.instances.iter()) {
        if planned.name != required.name {
            return Err(PlanRejection::RosterMismatch {
                detail: format!(
                    "instance `{}` does not match required `{}`",
                    planned.name, required.name
                ),
            });
        }
        if planned.component.subject != required.subject {
            return Err(PlanRejection::RosterMismatch {
                detail: format!(
                    "instance `{}` binds a different component subject",
                    planned.name
                ),
            });
        }
        // The record must be the admission's, not merely carry the right
        // subject: profile, completeness closure, assumptions, and endpoint
        // inventory are reconstructed from the verified description.
        check_instance_binding(planned, components).map_err(|failure| {
            PlanRejection::ComponentBinding {
                instance: planned.name.clone(),
                failure,
            }
        })?;
        // Completeness is enforced structurally at decode (only
        // VerifiedComplete is representable); the assumptions the description
        // consumed must each be owner-accepted.
        debug_assert!(matches!(
            planned.component.completeness,
            Completeness::VerifiedComplete { .. }
        ));
        for assumption in &planned.component.assumptions {
            if !request.accepted_assumptions.contains(assumption) {
                return Err(PlanRejection::UnacceptedAssumption {
                    instance: planned.name.clone(),
                    assumption: *assumption,
                });
            }
        }
    }

    // Transport profile: every binding's selection must be owner-allowed.
    for (index, binding) in plan.bindings.iter().enumerate() {
        if !request.transports.contains(&binding.transport) {
            return Err(PlanRejection::UnselectedTransport { binding: index });
        }
    }

    // Required policy coverage, exact in both directions: the plan may not
    // omit a required policy nor add one the owner did not select, and every
    // row must name the selected verifier.
    let required: BTreeMap<_, ()> = request
        .policies
        .iter()
        .map(|call| (call.canonical_key(), ()))
        .collect();
    let mut planned_keys = BTreeMap::new();
    for (index, row) in plan.policies.iter().enumerate() {
        if row.verifier != request.verifier {
            return Err(PlanRejection::UnselectedPolicyExecutable { policy: index });
        }
        let key = row.call.canonical_key();
        if !required.contains_key(&key) {
            return Err(PlanRejection::UnexpectedPolicy {
                call: Box::new(row.call.clone()),
            });
        }
        planned_keys.insert(key, index);
    }
    for call in &request.policies {
        if !planned_keys.contains_key(&call.canonical_key()) {
            return Err(PlanRejection::MissingRequiredPolicy {
                call: Box::new(call.clone()),
            });
        }
    }

    // Reconstruct the graph from the plan's own records — duplicate
    // instances, undeclared endpoints, wrong directions, duplicate or missing
    // bindings all reject here.
    let graph = NormalizedGraph::new(plan.instances.clone(), plan.bindings.clone())
        .map_err(PlanRejection::InvalidGraph)?;

    // Replay every recorded policy on the reconstructed graph in canonical
    // order, then check its recorded evidence against the real edges.
    for (index, row) in plan.policies.iter().enumerate() {
        match evaluate_policy(&graph, &row.call) {
            PolicyEvaluation::InvalidSelectors(errors) => {
                return Err(PlanRejection::InvalidPolicy { index, errors });
            }
            PolicyEvaluation::Decided(replayed) => {
                let satisfied = matches!(replayed, PolicyOutcome::Satisfied { .. });
                match &row.outcome {
                    PolicyOutcome::Violated { .. } => {
                        return Err(PlanRejection::PolicyNotSatisfied { index });
                    }
                    PolicyOutcome::Satisfied { .. } if !satisfied => {
                        return Err(PlanRejection::ReplayMismatch { index });
                    }
                    PolicyOutcome::Satisfied { .. } => {}
                }
            }
        }
        if let Err(reason) = check_outcome(&graph, &row.call, &row.outcome) {
            return Err(PlanRejection::InvalidCertificate { index, reason });
        }
    }

    Ok(CheckedPlan {
        subject: plan_subject(plan_bytes),
        graph,
        plan,
        request,
    })
}

/// Why a candidate plan is rejected. Categories stay distinct so a caller can
/// report the actual failure instead of a generic "invalid".
#[derive(Debug)]
pub enum PlanRejection {
    /// The request bytes themselves are malformed.
    MalformedRequest(CodecError),
    /// The plan bytes are malformed or corrupt.
    MalformedPlan(CodecError),
    /// A completeness tag the codec names but never admits was supplied:
    /// producer-declared or early-frontier records are not verified closure.
    UnverifiedCompleteness { tag: u8 },
    /// The plan's recorded request commitment differs from the current
    /// authorized request — including a correctly formed but superseded one.
    StaleRequest { expected: Identity, found: Identity },
    /// The plan roster differs from the request roster: an instance was
    /// added, renamed, removed, or bound to a different component subject.
    RosterMismatch { detail: String },
    /// A component description relies on an assumption the owner did not
    /// accept.
    UnacceptedAssumption {
        instance: InstanceName,
        assumption: Identity,
    },
    /// The instance's component record or endpoint inventory does not equal
    /// what a supplied admission verifies: a missing admission, an external
    /// claim over a verified subject, or a forged or substituted record.
    ComponentBinding {
        instance: InstanceName,
        failure: ComponentBindingFailure,
    },
    /// A binding selects a transport outside the request's allowed profile.
    UnselectedTransport { binding: usize },
    /// A policy row names an executable other than the selected verifier.
    /// Rejected by identity comparison; the named code is never loaded.
    UnselectedPolicyExecutable { policy: usize },
    /// The plan omits a policy the owner required.
    MissingRequiredPolicy { call: Box<PolicyCall> },
    /// The plan records a policy the owner did not require.
    UnexpectedPolicy { call: Box<PolicyCall> },
    /// A recorded policy row is malformed for this graph (bad selectors).
    InvalidPolicy {
        index: usize,
        errors: Vec<SelectorError>,
    },
    /// A policy row records a violated outcome — no successful composition
    /// exists to publish.
    PolicyNotSatisfied { index: usize },
    /// The recorded verdict does not survive independent replay on the
    /// reconstructed graph.
    ReplayMismatch { index: usize },
    /// The recorded certificate fails structural checking against the real
    /// edges.
    InvalidCertificate {
        index: usize,
        reason: CertificateRejection,
    },
    /// The plan's records do not normalize into a complete bounded graph.
    InvalidGraph(GraphError),
}

impl fmt::Display for PlanRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedRequest(error) => write!(formatter, "malformed request: {error}"),
            Self::MalformedPlan(error) => write!(formatter, "malformed plan: {error}"),
            Self::UnverifiedCompleteness { tag } => write!(
                formatter,
                "a component description supplies completeness tag {tag}, not verified closure"
            ),
            Self::StaleRequest { .. } => {
                formatter.write_str("plan answers a request that is not the current authorization")
            }
            Self::RosterMismatch { detail } => {
                write!(formatter, "plan roster differs from the request: {detail}")
            }
            Self::UnacceptedAssumption { instance, .. } => {
                write!(
                    formatter,
                    "instance `{instance}` needs an assumption the owner did not accept"
                )
            }
            Self::ComponentBinding { instance, failure } => {
                write!(
                    formatter,
                    "instance `{instance}` does not bind an admitted component: {failure}"
                )
            }
            Self::UnselectedTransport { binding } => {
                write!(
                    formatter,
                    "binding {binding} selects a transport outside the allowed profile"
                )
            }
            Self::UnselectedPolicyExecutable { policy } => write!(
                formatter,
                "policy row {policy} names an executable the owner did not select"
            ),
            Self::MissingRequiredPolicy { .. } => {
                formatter.write_str("plan omits a required policy")
            }
            Self::UnexpectedPolicy { .. } => {
                formatter.write_str("plan records a policy the owner did not require")
            }
            Self::InvalidPolicy { index, .. } => {
                write!(formatter, "policy row {index} has invalid selectors")
            }
            Self::PolicyNotSatisfied { index } => {
                write!(
                    formatter,
                    "policy row {index} records an unsatisfied outcome"
                )
            }
            Self::ReplayMismatch { index } => {
                write!(
                    formatter,
                    "policy row {index} does not survive independent replay"
                )
            }
            Self::InvalidCertificate { index, reason } => {
                write!(
                    formatter,
                    "policy row {index} carries invalid evidence: {reason}"
                )
            }
            Self::InvalidGraph(error) => write!(formatter, "plan graph is invalid: {error}"),
        }
    }
}

impl std::error::Error for PlanRejection {}

/// `composition checked`: the plan reconstructs to a complete graph that
/// satisfies the current owner request under the selected predicates. This
/// is not an installation verdict — no endpoint was bound and no authority
/// was granted.
#[derive(Debug)]
pub struct CheckedPlan {
    /// The plan's external identity: the digest of its canonical bytes.
    pub subject: Identity,
    /// The independently reconstructed, normalized graph.
    pub graph: NormalizedGraph,
    /// The decoded plan records.
    pub plan: DeploymentPlan,
    /// The decoded current request the plan was checked against.
    pub request: TopologyRequest,
}
