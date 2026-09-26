//! Substitutable-field inventory for the owner `TopologyRequest` custody
//! matrix driven by `tests/custody_substitution.rs`.
//!
//! The declared lanes are the request's representable custody axes: each
//! instance name and component subject, the roster's membership and
//! canonical order, each policy argument and the predicate/via pair, the
//! selected verifier, the transport profile, and the accepted-assumption
//! set. Keeping the inventory in its own file names the boundary between
//! what is declared substitutable and the shared
//! `run_one_field_substitution_matrix` driver that exercises every leg.
//!
//! Outcomes split three ways: a non-canonical record has no wire identity
//! and rejects at `encode_request`; a record the encoder carries but the
//! wire rules refuse rejects at `decode_request` (and surfaces at replay as
//! a malformed request); a representable substitution's rejection is the
//! honestly re-committed plan's verdict under `verify_plan` — including the
//! admissible widenings, whose only rejection is the divergent commitment
//! both cross-pairings surface.

use topology_plan::{
    CodecError, Identity, PlanRejection, PolicyCall, PolicyPredicate, PolicySelector,
    RequestedInstance, TopologyRequest,
};

use crate::support::{identity, name, payment_request, transport};

mutation_matrix::custody_field_inventory! {
    /// One substitutable lane of the owner `TopologyRequest`; checked by the
    /// codec-plus-replay chain in `custody_substitution.rs`. Predicate lanes
    /// that also move `via` are single axes: `no_route` and `only_via` each
    /// have one canonical arity, so the predicate and its argument cannot be
    /// substituted independently.
    pub enum TopologyRequestCustodyFieldForTest {
        Instance0Name,
        Instance1Name,
        Instance2Name,
        Instance0Subject,
        Instance1Subject,
        Instance2SubjectZeroed,
        InstancesDropped,
        InstancesInserted,
        InstancesReordered,
        InstancesDuplicated,
        Policy0Sources,
        Policy0Targets,
        Policy1Via,
        Policy1PredicateRelaxed,
        Policy0PredicateStrengthened,
        PoliciesDropped,
        PoliciesInserted,
        PoliciesReordered,
        PoliciesDuplicated,
        Policy0ViaArity,
        VerifierSubstituted,
        VerifierZeroed,
        VerifierPlanSubject,
        TransportsSubstituted,
        TransportsEmptied,
        TransportsWidened,
        TransportsReordered,
        TransportsDuplicated,
        AcceptedAssumptionsAdded,
        AcceptedAssumptionsReordered,
        AcceptedAssumptionsDuplicated,
    }
}

/// The `PlanRejection` variant the honestly re-committed plan's semantic
/// replay reports. `PlanRejection` is deliberately not `PartialEq` — its
/// payloads are evidence — so the matrix compares the named kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReboundRejection {
    RosterMismatch,
    UnselectedTransport,
    UnselectedPolicyExecutable,
    MissingRequiredPolicy,
    UnexpectedPolicy,
}

/// Classify a rebound-plan rejection into the family's named vocabulary; any
/// other variant means the substitution produced a verdict shape the family
/// does not declare.
pub fn classify_rebound(rejection: &PlanRejection) -> ReboundRejection {
    match rejection {
        PlanRejection::RosterMismatch { .. } => ReboundRejection::RosterMismatch,
        PlanRejection::UnselectedTransport { .. } => ReboundRejection::UnselectedTransport,
        PlanRejection::UnselectedPolicyExecutable { .. } => {
            ReboundRejection::UnselectedPolicyExecutable
        }
        PlanRejection::MissingRequiredPolicy { .. } => ReboundRejection::MissingRequiredPolicy,
        PlanRejection::UnexpectedPolicy { .. } => ReboundRejection::UnexpectedPolicy,
        other => panic!("rebound rejection outside the declared vocabulary: {other}"),
    }
}

/// How a one-field request substitution disposes of the mutated record.
/// Every leg is an exact verdict: the record is never honest, so the
/// checker's `Ok` arm (the request commitment a clean replay would carry)
/// is never constructed.
#[derive(Debug, PartialEq)]
pub enum RequestCustodyCheck {
    /// `encode_request` refuses the mutated record: a non-canonical value
    /// has no wire identity to commit.
    Encode(CodecError),
    /// The encoder carries the record but `decode_request` refuses it — a
    /// wire-only rule that replay surfaces as a malformed request.
    Decode(CodecError),
    /// The plan honestly re-committed to the mutated request rejects
    /// semantically under `verify_plan`.
    Rebound(ReboundRejection),
    /// The substitution is admissible intent: the re-committed plan
    /// verifies under the divergent commitment, and both cross-pairings —
    /// the baseline plan against the mutated request and the installer's
    /// authorization join in each direction — refuse the baseline. The
    /// divergent published commitment is the rejection.
    ForeignIntentAdmitted,
}

/// An authentic foreign request: the honest payment intent with one extra
/// accepted assumption — genuinely composable, and its retained custody
/// differs from the honest record the matrices build.
pub fn foreign_request_donor() -> TopologyRequest {
    let mut request = payment_request();
    request.accepted_assumptions = vec![identity(0xAA)];
    request
}

/// The family's honest-recomputation hook: mutate exactly the declared field
/// of `request`, keeping every other axis honest. `selected_subject` is the
/// baseline plan's published subject — the foreign identity the
/// `VerifierPlanSubject` leg borrows; no donor field supplies it.
pub fn substitute_request_custody_for_test(
    request: &mut TopologyRequest,
    field: TopologyRequestCustodyFieldForTest,
    selected_subject: Identity,
) {
    use TopologyRequestCustodyFieldForTest as Leg;
    match field {
        Leg::Instance0Name => request.instances[0].name = name("aqi"),
        Leg::Instance1Name => request.instances[1].name = name("authorizatior"),
        Leg::Instance2Name => request.instances[2].name = name("billing2"),
        Leg::Instance0Subject => request.instances[0].subject = identity(0x12),
        Leg::Instance1Subject => request.instances[1].subject = identity(0x23),
        Leg::Instance2SubjectZeroed => request.instances[2].subject = [0u8; 32],
        Leg::InstancesDropped => {
            request.instances.remove(2);
        }
        Leg::InstancesInserted => request.instances.insert(
            1,
            RequestedInstance {
                name: name("auditor"),
                subject: identity(0x44),
            },
        ),
        Leg::InstancesReordered => request.instances.swap(0, 1),
        Leg::InstancesDuplicated => request.instances.insert(1, request.instances[0].clone()),
        Leg::Policy0Sources => request.policies[0].sources = PolicySelector::new([name("api")]),
        Leg::Policy0Targets => {
            request.policies[0].targets = PolicySelector::new([name("authorization")])
        }
        Leg::Policy1Via => request.policies[1].via = PolicySelector::new([name("billing")]),
        // Relaxing `only_via` to `no_route` drops the routing obligation and
        // its argument together: the mutated call re-sorts ahead of the
        // other `no_route` row, so the request stays canonical.
        Leg::Policy1PredicateRelaxed => {
            request.policies[1].predicate = PolicyPredicate::NoRoute;
            request.policies[1].via = PolicySelector {
                members: Vec::new(),
            };
            request.policies.sort_by_key(|call| call.canonical_key());
        }
        // Strengthening `no_route` to `only_via` adds an obligation the plan
        // never recorded; the mutated call's canonical key moves it out of
        // order, so the record never reaches the wire.
        Leg::Policy0PredicateStrengthened => {
            request.policies[0].predicate = PolicyPredicate::OnlyVia;
            request.policies[0].via = PolicySelector::new([name("authorization")]);
        }
        Leg::PoliciesDropped => {
            request.policies.remove(0);
        }
        Leg::PoliciesInserted => request.policies.insert(
            0,
            PolicyCall::no_route(
                PolicySelector::new([name("api")]),
                PolicySelector::new([name("authorization")]),
            ),
        ),
        Leg::PoliciesReordered => request.policies.swap(0, 1),
        Leg::PoliciesDuplicated => request.policies.insert(1, request.policies[0].clone()),
        // `via` arity on the request's own wire: encodable, never decodable.
        Leg::Policy0ViaArity => request.policies[0].via = PolicySelector::new([name("api")]),
        Leg::VerifierSubstituted => request.verifier = identity(0x51),
        Leg::VerifierZeroed => request.verifier = [0u8; 32],
        Leg::VerifierPlanSubject => request.verifier = selected_subject,
        Leg::TransportsSubstituted => request.transports = vec![identity(0x78)],
        Leg::TransportsEmptied => request.transports = Vec::new(),
        Leg::TransportsWidened => request.transports = vec![identity(0x78), transport()],
        Leg::TransportsReordered => request.transports = vec![transport(), identity(0x78)],
        Leg::TransportsDuplicated => request.transports = vec![transport(), transport()],
        Leg::AcceptedAssumptionsAdded => request.accepted_assumptions = vec![identity(0xAA)],
        Leg::AcceptedAssumptionsReordered => {
            request.accepted_assumptions = vec![identity(0xBB), identity(0xAA)]
        }
        Leg::AcceptedAssumptionsDuplicated => {
            request.accepted_assumptions = vec![identity(0xAA), identity(0xAA)]
        }
    }
}

/// The exact checker verdict each declared leg must produce.
pub fn request_custody_outcome(
    field: TopologyRequestCustodyFieldForTest,
) -> mutation_matrix::MutationOutcome<RequestCustodyCheck> {
    use TopologyRequestCustodyFieldForTest as Leg;
    let check = match field {
        Leg::Instance0Name
        | Leg::Instance1Name
        | Leg::Instance2Name
        | Leg::Instance0Subject
        | Leg::Instance1Subject
        | Leg::Instance2SubjectZeroed
        | Leg::InstancesDropped
        | Leg::InstancesInserted => RequestCustodyCheck::Rebound(ReboundRejection::RosterMismatch),
        Leg::InstancesReordered => RequestCustodyCheck::Encode(CodecError::NotCanonical {
            section: "request instances",
        }),
        Leg::InstancesDuplicated => RequestCustodyCheck::Encode(CodecError::Duplicate {
            section: "request instances",
        }),
        Leg::Policy0Sources
        | Leg::Policy0Targets
        | Leg::Policy1Via
        | Leg::Policy1PredicateRelaxed
        | Leg::PoliciesDropped => RequestCustodyCheck::Rebound(ReboundRejection::UnexpectedPolicy),
        Leg::PoliciesInserted => {
            RequestCustodyCheck::Rebound(ReboundRejection::MissingRequiredPolicy)
        }
        Leg::Policy0PredicateStrengthened | Leg::PoliciesReordered => {
            RequestCustodyCheck::Encode(CodecError::NotCanonical {
                section: "request policies",
            })
        }
        Leg::PoliciesDuplicated => RequestCustodyCheck::Encode(CodecError::Duplicate {
            section: "request policies",
        }),
        Leg::Policy0ViaArity => RequestCustodyCheck::Decode(CodecError::ViaArity),
        Leg::VerifierSubstituted | Leg::VerifierZeroed | Leg::VerifierPlanSubject => {
            RequestCustodyCheck::Rebound(ReboundRejection::UnselectedPolicyExecutable)
        }
        Leg::TransportsSubstituted | Leg::TransportsEmptied => {
            RequestCustodyCheck::Rebound(ReboundRejection::UnselectedTransport)
        }
        Leg::TransportsWidened | Leg::AcceptedAssumptionsAdded => {
            RequestCustodyCheck::ForeignIntentAdmitted
        }
        Leg::TransportsReordered => RequestCustodyCheck::Encode(CodecError::NotCanonical {
            section: "request transports",
        }),
        Leg::TransportsDuplicated => RequestCustodyCheck::Encode(CodecError::Duplicate {
            section: "request transports",
        }),
        Leg::AcceptedAssumptionsReordered => {
            RequestCustodyCheck::Encode(CodecError::NotCanonical {
                section: "accepted assumptions",
            })
        }
        Leg::AcceptedAssumptionsDuplicated => RequestCustodyCheck::Encode(CodecError::Duplicate {
            section: "accepted assumptions",
        }),
    };
    mutation_matrix::MutationOutcome::ExactError(check)
}
