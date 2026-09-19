//! Producer-side composition: the reference builder the eventual build-only
//! Omega package mirrors.
//!
//! `compose_plan` consumes the owner request, the instance records built
//! from verified component descriptions (see [`crate::verified_components`]),
//! the admitted components they came from, and the chosen bindings. It checks
//! the roster against the request exactly, re-binds every instance's record
//! and endpoint inventory to its admission, normalizes the graph
//! (establishing complete binding coverage), and evaluates every required
//! policy in canonical order. Only when all of them are satisfied does it
//! assemble a plan — a failed composition returns the evaluations and emits
//! no artifact, so an unsuccessful result cannot be serialized into a
//! checked plan.
//!
//! The builder may choose bindings that satisfy the request; it may not add
//! an undeclared instance, swap component code, omit a required policy, or
//! add one the owner did not select.

use crate::deployment_plan::graph::{GraphError, NormalizedGraph};
use crate::deployment_plan::predicate::{PolicyEvaluation, SelectorError, evaluate_policy};
use crate::deployment_plan::{
    Binding, DeploymentPlan, ExecutedPolicy, Identity, InstanceName, PlanInstance, PolicyOutcome,
    TopologyRequest, Violation, request_commitment,
};
use crate::verified_components::{
    AdmittedComponent, ComponentBindingFailure, check_instance_binding,
};
use std::fmt;

/// Compose a deployment plan. On success returns the canonical plan plus the
/// per-policy evaluations (each satisfied, with its certificate) for the
/// caller's diagnostics. The returned plan's `request_commitment` binds the
/// supplied request; encode with `crate::deployment_plan::codec::encode_plan`.
pub fn compose_plan(
    request: &TopologyRequest,
    request_bytes: &[u8],
    instances: Vec<PlanInstance>,
    bindings: Vec<Binding>,
    verifier: Identity,
    components: &[AdmittedComponent],
) -> Result<(DeploymentPlan, Vec<PolicyOutcome>), CompositionError> {
    // The producer's selected verifier must be the owner's selected one —
    // otherwise the plan would name an unselected executable by construction.
    if verifier != request.verifier {
        return Err(CompositionError::Rejected(
            CompositionFailure::UnselectedVerifier,
        ));
    }

    // Exact roster: the builder cannot add, rename, remove, or re-code an
    // instance.
    if instances.len() != request.instances.len() {
        return Err(CompositionError::Rejected(
            CompositionFailure::RosterMismatch {
                detail: format!(
                    "supplied {} instances, request requires {}",
                    instances.len(),
                    request.instances.len()
                ),
            },
        ));
    }
    let mut sorted = instances.clone();
    sorted.sort_by(|left, right| left.name.cmp(&right.name));
    for (supplied, required) in sorted.iter().zip(request.instances.iter()) {
        if supplied.name != required.name || supplied.component.subject != required.subject {
            return Err(CompositionError::Rejected(
                CompositionFailure::RosterMismatch {
                    detail: format!("instance `{}` differs from the request", required.name),
                },
            ));
        }
        // The producer's records must be the admission's: a hand-authored
        // inventory can never compose into a plan an independent verifier
        // would accept.
        if let Err(failure) = check_instance_binding(supplied, components) {
            return Err(CompositionError::Rejected(
                CompositionFailure::ComponentBinding {
                    instance: supplied.name.clone(),
                    failure,
                },
            ));
        }
    }

    let graph = NormalizedGraph::new(instances, bindings)
        .map_err(|error| CompositionError::Rejected(CompositionFailure::InvalidGraph(error)))?;

    // Evaluate every required policy in canonical order. A violated or
    // invalid policy produces witnesses, not a plan.
    let mut calls = request.policies.clone();
    calls.sort_by_key(|call| call.canonical_key());
    let mut outcomes = Vec::with_capacity(calls.len());
    let mut violations = Vec::new();
    let mut invalid = Vec::new();
    for call in &calls {
        match evaluate_policy(&graph, call) {
            PolicyEvaluation::InvalidSelectors(errors) => {
                invalid.push((describe_call(call), errors));
            }
            PolicyEvaluation::Decided(outcome) => {
                if let PolicyOutcome::Violated { violation } = &outcome {
                    violations.push((describe_call(call), violation.clone()));
                }
                outcomes.push(outcome);
            }
        }
    }
    if !violations.is_empty() || !invalid.is_empty() {
        return Err(CompositionError::PoliciesViolated {
            evaluations: violations,
            invalid,
        });
    }

    let policies = calls
        .into_iter()
        .zip(outcomes.iter().cloned())
        .map(|(call, outcome)| ExecutedPolicy {
            call,
            verifier,
            outcome,
        })
        .collect();

    Ok((
        DeploymentPlan {
            request_commitment: request_commitment(request_bytes),
            instances: graph.instances().to_vec(),
            bindings: graph.bindings().to_vec(),
            policies,
        },
        outcomes,
    ))
}

/// Why a composition cannot produce a plan.
#[derive(Debug)]
pub enum CompositionFailure {
    /// The supplied roster differs from the request's exact instance set or
    /// component subjects.
    RosterMismatch { detail: String },
    /// The builder supplied a verifier other than the request's selected one.
    UnselectedVerifier,
    /// An instance's component record or endpoint inventory does not equal
    /// what a supplied admission verifies.
    ComponentBinding {
        instance: InstanceName,
        failure: ComponentBindingFailure,
    },
    /// The records do not normalize: duplicates, undeclared endpoints, wrong
    /// directions, or incomplete binding coverage.
    InvalidGraph(GraphError),
    /// A required policy's selectors are invalid.
    InvalidPolicy {
        call: String,
        errors: Vec<SelectorError>,
    },
}

/// The outcome of a composition attempt.
#[derive(Debug)]
pub enum CompositionError {
    /// The request could not be encoded for its commitment.
    RequestEncoding(String),
    /// The inputs failed before policy evaluation.
    Rejected(CompositionFailure),
    /// One or more required policies evaluated `Violated`; the witnesses are
    /// returned for diagnostics and no plan is emitted.
    PoliciesViolated {
        evaluations: Vec<(String, Violation)>,
        /// Selector-level failures collected alongside.
        invalid: Vec<(String, Vec<SelectorError>)>,
    },
}

impl fmt::Display for CompositionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequestEncoding(detail) => {
                write!(formatter, "request cannot be committed: {detail}")
            }
            Self::Rejected(failure) => write!(formatter, "composition rejected: {failure}"),
            Self::PoliciesViolated { evaluations, .. } => write!(
                formatter,
                "{} required policies evaluated violated",
                evaluations.len()
            ),
        }
    }
}

impl std::error::Error for CompositionError {}

impl fmt::Display for CompositionFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RosterMismatch { detail } => {
                write!(formatter, "roster differs from the request: {detail}")
            }
            Self::UnselectedVerifier => {
                formatter.write_str("supplied verifier is not the request's selected verifier")
            }
            Self::ComponentBinding { instance, failure } => {
                write!(
                    formatter,
                    "instance `{instance}` does not bind an admitted component: {failure}"
                )
            }
            Self::InvalidGraph(error) => write!(formatter, "graph is invalid: {error}"),
            Self::InvalidPolicy { call, .. } => {
                write!(formatter, "policy `{call}` has invalid selectors")
            }
        }
    }
}

impl std::error::Error for CompositionFailure {}

fn describe_call(call: &crate::deployment_plan::PolicyCall) -> String {
    let names = |selector: &crate::deployment_plan::PolicySelector| {
        selector
            .members
            .iter()
            .map(|name| name.as_str().to_owned())
            .collect::<Vec<_>>()
            .join(",")
    };
    match call.predicate {
        crate::deployment_plan::PolicyPredicate::NoRoute => format!(
            "no_route({{{}}}, {{{}}})",
            names(&call.sources),
            names(&call.targets)
        ),
        crate::deployment_plan::PolicyPredicate::OnlyVia => format!(
            "only_via({{{}}}, {{{}}}, {{{}}})",
            names(&call.sources),
            names(&call.targets),
            names(&call.via)
        ),
    }
}
