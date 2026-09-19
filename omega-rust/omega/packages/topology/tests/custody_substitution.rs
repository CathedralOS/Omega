//! Authenticated one-field substitution matrices over the canonical topology
//! custody records: the owner `TopologyRequest`, the `DeploymentPlan`
//! answering it, and the installer-side `InstallationRequest`.
//!
//! The custody chain under test: `encode_request` produces the request's only
//! byte identity, committed by `request_commitment`; `encode_plan` produces
//! the plan's only byte identity, committed by `plan_subject` and bound to a
//! request through `plan.request_commitment`. Two independent replays consume
//! those identities — `verify_plan` reconstructs and re-checks the whole
//! composition from bytes, and `prepare_installation` compares the checked
//! plan's recorded commitment against the supervisor's independently supplied
//! `expected_request`. An `InstallationReceipt.plan` seals the checked
//! subject, so a field the semantic joins do not inspect still cannot keep
//! the published identity.
//!
//! Every representable field gets an independent substitution below. Each
//! case proves the mutated record differs, its canonical bytes recompute a
//! divergent containing identity, replay-direction equivalence holds
//! (`decode(encode(x)) == x`, `encode(decode(bytes)) == bytes`), and the
//! substitution rejects at its join — semantically under replay, at the
//! encoder or decoder when the mutation is not canonically representable, or
//! at the published subject/commitment when the field is evidence the
//! semantic joins deliberately do not adjudicate.

mod support;

use std::collections::BTreeSet;

use support::*;
use topology_plan::topology_installation::{
    AdmittedArtifact, InstallationLifecycle, InstallationRejection, InstallationRequest,
    PipeAdapter, PipePair, PrepareError, prepare_installation,
};
use topology_plan::*;

// ---- simulated pipe adapter ---------------------------------------------
//
// The installer join needs one real `PipeAdapter`. Every rejection this file
// exercises precedes or follows allocation honestly; the simulated provider
// mints distinct physical tokens so preparation's token-collision audit sees
// well-formed custody.

#[derive(Debug)]
struct SimEndpoint(u64);

#[derive(Debug)]
struct SimError(&'static str);

impl std::fmt::Display for SimError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for SimError {}

struct SimAdapter {
    next_token: u64,
    /// Mint two ends with one token, to forge a collision.
    collide: bool,
}

impl SimAdapter {
    fn new() -> Self {
        Self {
            next_token: 100,
            collide: false,
        }
    }
}

impl PipeAdapter for SimAdapter {
    type Endpoint = SimEndpoint;
    type Error = SimError;

    fn create_binding_pair(&mut self) -> Result<PipePair<SimEndpoint>, SimError> {
        let mint = |adapter: &mut Self| {
            let token = adapter.next_token;
            adapter.next_token += 1;
            SimEndpoint(token)
        };
        let mut pair = PipePair {
            request_write: mint(self),
            request_read: mint(self),
            response_write: mint(self),
            response_read: mint(self),
        };
        if self.collide {
            // A dishonest adapter hands back the same physical end twice.
            pair.response_read = SimEndpoint(pair.request_write.0);
        }
        Ok(pair)
    }

    fn endpoint_token(&self, endpoint: &SimEndpoint) -> u64 {
        endpoint.0
    }

    fn close_endpoint(&mut self, _endpoint: SimEndpoint) -> Result<(), SimError> {
        Ok(())
    }

    fn provider(&self) -> Identity {
        identity(0x99)
    }

    fn assumptions(&self) -> &'static [&'static str] {
        &["simulated kernel attestation"]
    }
}

// ---- shared fixtures -----------------------------------------------------

/// The golden payment composition: request value and bytes, plan value and
/// bytes, and the admissions the roster entries bind, all consistent.
fn baseline() -> (
    TopologyRequest,
    Vec<u8>,
    DeploymentPlan,
    Vec<u8>,
    Vec<AdmittedComponent>,
) {
    let request = payment_request();
    let (request_bytes, plan_bytes, plan) = payment_pair();
    (
        request,
        request_bytes,
        plan,
        plan_bytes,
        payment_components(),
    )
}

fn checked_baseline(plan_bytes: &[u8], request_bytes: &[u8]) -> CheckedPlan {
    verify_plan(plan_bytes, request_bytes, &payment_components())
        .expect("the baseline plan verifies")
}

/// Owner intent the supervisor could issue: the commitment under test, one
/// fresh occurrence, and the admitted artifact roster in canonical order —
/// each artifact bound to its instance's real component subject.
fn installation_request(
    expected_request: Identity,
    occurrence: u64,
    components: &[AdmittedComponent],
) -> InstallationRequest {
    InstallationRequest {
        expected_request,
        occurrence,
        artifacts: components
            .iter()
            .enumerate()
            .map(|(index, admission)| AdmittedArtifact {
                artifact: identity(0xA1 + index as u8),
                component_subject: subject_of(admission),
            })
            .collect(),
    }
}

/// The installer's independent replay of the request commitment: an
/// authorization carrying `expected` refuses a checked plan answering
/// `found` before any endpoint exists.
fn assert_unauthorized_request(
    name: &str,
    checked: CheckedPlan,
    expected: Identity,
    found: Identity,
    components: &[AdmittedComponent],
) {
    let mut lifecycle = InstallationLifecycle::default();
    let authorization = lifecycle
        .authorize(installation_request(expected, 7, components))
        .expect("fresh issuance");
    let error = prepare_installation(checked, authorization, SimAdapter::new())
        .expect_err("a mismatched commitment pairing must reject");
    match error {
        PrepareError::Rejected {
            rejection:
                InstallationRejection::UnauthorizedRequest {
                    expected: e,
                    found: f,
                },
            leaked,
        } => {
            assert_eq!(e, expected, "{name}");
            assert_eq!(f, found, "{name}");
            assert!(
                leaked.is_empty(),
                "{name}: commitment rejection precedes endpoint custody"
            );
        }
        other => panic!("{name}: expected UnauthorizedRequest, got {other}"),
    }
}

// ---- plan-side drivers ---------------------------------------------------

/// A representable plan substitution the semantic replay rejects: the
/// mutated record encodes canonically, round-trips in both directions,
/// recomputes a divergent subject, and `verify_plan` refuses it for the
/// expected reason.
fn assert_plan_rejected(
    name: &str,
    mutated: &DeploymentPlan,
    baseline_subject: Identity,
    request_bytes: &[u8],
    components: &[AdmittedComponent],
    expected: impl Fn(&PlanRejection) -> bool,
) {
    let bytes = encode_plan(mutated)
        .unwrap_or_else(|error| panic!("{name}: the substitution must stay encodable: {error}"));
    let decoded = decode_plan(&bytes)
        .unwrap_or_else(|error| panic!("{name}: the substitution must stay decodable: {error}"));
    assert_eq!(&decoded, mutated, "{name}: decode(encode) equivalence");
    assert_eq!(
        encode_plan(&decoded).expect("re-encode"),
        bytes,
        "{name}: encode(decode) equivalence"
    );
    assert_ne!(
        plan_subject(&bytes),
        baseline_subject,
        "{name}: the recomputed subject must diverge"
    );
    let error = verify_plan(&bytes, request_bytes, components)
        .expect_err("a one-field substitution must be rejected by replay");
    assert!(expected(&error), "{name}: unexpected rejection: {error}");
}

/// A representable plan substitution the semantic joins do not adjudicate:
/// the record still verifies — it is honest data — but only under a
/// divergent checked subject, so the published plan identity an
/// `InstallationReceipt.plan` seals cannot match the substituted record.
fn assert_plan_identity_bound(
    name: &str,
    mutated: &DeploymentPlan,
    baseline_subject: Identity,
    request_bytes: &[u8],
    components: &[AdmittedComponent],
) {
    let bytes = encode_plan(mutated)
        .unwrap_or_else(|error| panic!("{name}: the substitution must stay encodable: {error}"));
    let decoded =
        decode_plan(&bytes).unwrap_or_else(|error| panic!("{name}: must stay decodable: {error}"));
    assert_eq!(&decoded, mutated, "{name}: decode(encode) equivalence");
    let subject = plan_subject(&bytes);
    assert_ne!(
        subject, baseline_subject,
        "{name}: the recomputed subject must diverge"
    );
    let checked = verify_plan(&bytes, request_bytes, components)
        .expect("an evidence-field substitution stays verifiable");
    assert_ne!(
        checked.subject, baseline_subject,
        "{name}: the checked subject the receipt seals must diverge"
    );
    assert_eq!(
        &checked.plan, mutated,
        "{name}: replay reconstructs the mutated record"
    );
}

/// A substitution the encoder refuses: the mutated record is not a canonical
/// value, so no wire identity for it exists.
fn assert_plan_encode_rejected(
    name: &str,
    mutated: &DeploymentPlan,
    expected: impl Fn(&CodecError) -> bool,
) {
    let error = encode_plan(mutated).expect_err("a non-canonical record must not encode");
    assert!(expected(&error), "{name}: unexpected rejection: {error}");
}

/// A substitution the encoder carries but the decoder refuses: nested
/// canonical order is a wire rule, enforced on replay even though the writer
/// emits the order it is given.
fn assert_plan_decode_rejected(
    name: &str,
    mutated: &DeploymentPlan,
    request_bytes: &[u8],
    components: &[AdmittedComponent],
    expected: impl Fn(&CodecError) -> bool,
) {
    let bytes = encode_plan(mutated).unwrap_or_else(|error| {
        panic!("{name}: the encoder must carry the mutated record: {error}")
    });
    let error = decode_plan(&bytes).expect_err("the wire order rule must reject");
    assert!(expected(&error), "{name}: unexpected rejection: {error}");
    assert!(
        matches!(
            verify_plan(&bytes, request_bytes, components),
            Err(PlanRejection::MalformedPlan(_))
        ),
        "{name}: replay must surface the same rejection as a malformed plan"
    );
}

// ---- request-side drivers ------------------------------------------------

/// How an honestly re-committed plan fares under the mutated request.
enum Rebound {
    /// The re-committed plan still rejects, for this semantic reason.
    Rejected(fn(&PlanRejection) -> bool),
    /// The substitution widens or restates intent without contradicting the
    /// plan's evidence: the re-committed plan verifies under a divergent
    /// subject, and both cross-pairings still reject.
    Verifies,
}

/// One owner-request field substitution through the whole custody chain:
/// divergent recomputed commitment, the baseline plan stale under it, the
/// installer's authorization join refusing the baseline plan in the opposite
/// direction, and the honestly re-committed plan meeting its classified
/// replay outcome.
fn assert_request_substitution(name: &str, mutated: &TopologyRequest, rebound: Rebound) {
    let (request, request_bytes, plan, plan_bytes, components) = baseline();
    let baseline_commitment = request_commitment(&request_bytes);
    let baseline_subject = plan_subject(&plan_bytes);
    assert_ne!(
        mutated, &request,
        "{name}: the substitution must change the request"
    );
    let mutated_bytes = encode_request(mutated)
        .unwrap_or_else(|error| panic!("{name}: the substitution must stay encodable: {error}"));
    let decoded = decode_request(&mutated_bytes)
        .unwrap_or_else(|error| panic!("{name}: the substitution must stay decodable: {error}"));
    assert_eq!(&decoded, mutated, "{name}: decode(encode) equivalence");
    assert_eq!(
        encode_request(&decoded).expect("re-encode"),
        mutated_bytes,
        "{name}: encode(decode) equivalence"
    );
    let mutated_commitment = request_commitment(&mutated_bytes);
    assert_ne!(
        mutated_commitment, baseline_commitment,
        "{name}: the recomputed commitment must diverge"
    );

    // The baseline plan answers the baseline request only: under mutated
    // intent its recorded commitment is stale before any semantic check.
    let error = verify_plan(&plan_bytes, &mutated_bytes, &components)
        .expect_err("the baseline plan must be stale under mutated intent");
    assert!(
        matches!(
            error,
            PlanRejection::StaleRequest { expected, found }
                if expected == mutated_commitment && found == baseline_commitment
        ),
        "{name}: unexpected rejection: {error}"
    );

    // The installer's independent commitment replay refuses the baseline
    // checked plan when the current authorization names the mutated intent.
    assert_unauthorized_request(
        name,
        checked_baseline(&plan_bytes, &request_bytes),
        mutated_commitment,
        baseline_commitment,
        &components,
    );

    // Honestly re-commit the plan's records to the mutated request: the
    // containing identity recomputes, and the same plan bytes are stale
    // against the baseline request too — custody binds in both directions.
    let mut rebound_plan = plan.clone();
    rebound_plan.request_commitment = mutated_commitment;
    let rebound_bytes = encode_plan(&rebound_plan).expect("the rebound plan encodes");
    assert_ne!(
        plan_subject(&rebound_bytes),
        baseline_subject,
        "{name}: the rebound plan's subject must diverge"
    );
    let error = verify_plan(&rebound_bytes, &request_bytes, &components)
        .expect_err("a rebound plan must be stale under the baseline request");
    assert!(
        matches!(
            error,
            PlanRejection::StaleRequest { expected, found }
                if expected == baseline_commitment && found == mutated_commitment
        ),
        "{name}: unexpected rejection: {error}"
    );

    match rebound {
        Rebound::Rejected(expected) => {
            let error = verify_plan(&rebound_bytes, &mutated_bytes, &components).expect_err(
                "the honestly re-committed substitution must still reject semantically",
            );
            assert!(expected(&error), "{name}: unexpected rejection: {error}");
        }
        Rebound::Verifies => {
            let checked = verify_plan(&rebound_bytes, &mutated_bytes, &components)
                .expect("an intent-widening substitution verifies under its own commitment");
            assert_eq!(
                checked.plan.request_commitment, mutated_commitment,
                "{name}: the checked plan binds the mutated request"
            );
            // The verified substitution still cannot pose as the baseline:
            // an authorization for the original intent refuses it.
            assert_unauthorized_request(
                name,
                checked,
                baseline_commitment,
                mutated_commitment,
                &components,
            );
        }
    }
}

/// A request mutation the encoder refuses: non-canonical intent has no wire
/// identity to commit at all.
fn assert_request_encode_rejected(
    name: &str,
    mutated: &TopologyRequest,
    expected: impl Fn(&CodecError) -> bool,
) {
    let error = encode_request(mutated).expect_err("non-canonical intent must not encode");
    assert!(expected(&error), "{name}: unexpected rejection: {error}");
}

// ---- the plan matrix -----------------------------------------------------

#[test]
fn deployment_plan_rejects_every_one_field_substitution() {
    let (_request, request_bytes, plan, plan_bytes, components) = baseline();
    let baseline_subject = plan_subject(&plan_bytes);
    let baseline_commitment = request_commitment(&request_bytes);
    // Control: the baseline replays to its own published subject and the
    // decoded record is the composed one.
    let checked = checked_baseline(&plan_bytes, &request_bytes);
    assert_eq!(checked.subject, baseline_subject);
    assert_eq!(checked.plan, plan);
    assert_eq!(checked.plan.request_commitment, baseline_commitment);

    // ── `request_commitment`: the request this plan answers. ──
    for (name, commitment) in [
        ("request_commitment::foreign", identity(0xEE)),
        ("request_commitment::zeroed", [0u8; 32]),
        (
            "request_commitment::self-subject",
            plan_subject(&plan_bytes),
        ),
    ] {
        let mut mutated = plan.clone();
        mutated.request_commitment = commitment;
        assert_plan_rejected(
            name,
            &mutated,
            baseline_subject,
            &request_bytes,
            &components,
            |error| matches!(error, PlanRejection::StaleRequest { .. }),
        );
    }

    // ── `instances[i].name`: the roster's `InstanceKey`. Renames that keep
    //    canonical order are representable and reject at the roster join;
    //    renames that break it or collide are not encodable. ──
    for (name, index, renamed) in [
        ("instances[0].name", 0usize, "aqi"),
        ("instances[1].name", 1, "authorizatior"),
        ("instances[2].name", 2, "billing2"),
    ] {
        let mut mutated = plan.clone();
        mutated.instances[index].name = support::name(renamed);
        assert_plan_rejected(
            name,
            &mutated,
            baseline_subject,
            &request_bytes,
            &components,
            |error| matches!(error, PlanRejection::RosterMismatch { .. }),
        );
    }
    let mut mutated = plan.clone();
    mutated.instances[2].name = support::name("aaa"); // sorts before api
    assert_plan_encode_rejected("instances[2].name::unsorted", &mutated, |error| {
        matches!(error, CodecError::NotCanonical { .. })
    });
    let mut mutated = plan.clone();
    mutated.instances[1].name = support::name("api"); // collides with index 0
    assert_plan_encode_rejected("instances[1].name::collision", &mutated, |error| {
        matches!(error, CodecError::Duplicate { .. })
    });

    // ── `instances[i].component`: the description record the plan claims.
    //    `subject` joins the request roster; every other field must equal
    //    what the instance's admitted component establishes — verification
    //    profile, completeness closure, assumptions — so a substituted
    //    record rejects rather than merely verifying under a divergent
    //    published subject. ──
    for (name, index, subject) in [
        ("instances[0].component.subject", 0usize, identity(0x12)),
        ("instances[1].component.subject", 1, identity(0x23)),
        ("instances[2].component.subject::zeroed", 2, [0u8; 32]),
    ] {
        let mut mutated = plan.clone();
        mutated.instances[index].component.subject = subject;
        assert_plan_rejected(
            name,
            &mutated,
            baseline_subject,
            &request_bytes,
            &components,
            |error| matches!(error, PlanRejection::RosterMismatch { .. }),
        );
    }
    for (name, index, profile) in [
        (
            "instances[0].component.verification_profile",
            0usize,
            identity(0x21),
        ),
        (
            "instances[1].component.verification_profile",
            1,
            identity(0x2F),
        ),
        (
            "instances[2].component.verification_profile::zeroed",
            2,
            [0u8; 32],
        ),
    ] {
        let mut mutated = plan.clone();
        mutated.instances[index].component.verification_profile = profile;
        assert_plan_rejected(
            name,
            &mutated,
            baseline_subject,
            &request_bytes,
            &components,
            |error| {
                matches!(
                    error,
                    PlanRejection::ComponentBinding {
                        failure: ComponentBindingFailure::Substituted {
                            field: "verification_profile"
                        },
                        ..
                    }
                )
            },
        );
    }
    for (name, index, closure) in [
        (
            "instances[0].component.completeness.closure",
            0usize,
            identity(0x31),
        ),
        (
            "instances[1].component.completeness.closure",
            1,
            identity(0x3F),
        ),
        (
            "instances[2].component.completeness.closure::zeroed",
            2,
            [0u8; 32],
        ),
    ] {
        let mut mutated = plan.clone();
        mutated.instances[index].component.completeness =
            Completeness::VerifiedComplete { closure };
        assert_plan_rejected(
            name,
            &mutated,
            baseline_subject,
            &request_bytes,
            &components,
            |error| {
                matches!(
                    error,
                    PlanRejection::ComponentBinding {
                        failure: ComponentBindingFailure::Substituted {
                            field: "completeness"
                        },
                        ..
                    }
                )
            },
        );
    }
    // A forged assumption digest is a substituted record — it rejects
    // before owner acceptance is ever consulted; the only representable
    // completeness tag is `VerifiedComplete`.
    let mut mutated = plan.clone();
    mutated.instances[0].component.assumptions = vec![identity(0xAA)];
    assert_plan_rejected(
        "instances[0].component.assumptions::forged",
        &mutated,
        baseline_subject,
        &request_bytes,
        &components,
        |error| {
            matches!(
                error,
                PlanRejection::ComponentBinding {
                    failure: ComponentBindingFailure::Substituted {
                        field: "assumptions"
                    },
                    ..
                }
            )
        },
    );
    // A second fixture whose api really writes the port makes the roster
    // field's remaining axes representable: the description demands the
    // port-mechanism assumption and the owner accepts it.
    let (assumption_module, assumption) = assumption_api_module();
    let rich_components = vec![
        admit_with(&assumption_module, BTreeSet::from([assumption])),
        admit(&authorization_module()),
        admit(&billing_module()),
    ];
    let mut rich_request = payment_request();
    rich_request.instances[0].subject = subject_of(&rich_components[0]);
    rich_request.accepted_assumptions = vec![assumption];
    let rich_request_bytes = encode_request(&rich_request).unwrap();
    let rich_instances = vec![
        verified_instance(support::name("api"), &rich_components[0]),
        verified_instance(support::name("authorization"), &rich_components[1]),
        verified_instance(support::name("billing"), &rich_components[2]),
    ];
    let (rich_plan, _) = compose_plan(
        &rich_request,
        &rich_request_bytes,
        rich_instances,
        payment_bindings(),
        verifier(),
        &rich_components,
    )
    .expect("assumption-bearing composition");
    let rich_plan_bytes = encode_plan(&rich_plan).unwrap();
    let rich_subject = plan_subject(&rich_plan_bytes);
    // Element substitutions to a foreign digest — even one the owner also
    // accepts — and a dropped row are all substituted records now.
    let mut mutated = rich_plan.clone();
    mutated.instances[0].component.assumptions = vec![identity(0xBB)];
    assert_plan_rejected(
        "instances[0].component.assumptions[0]::other",
        &mutated,
        rich_subject,
        &rich_request_bytes,
        &rich_components,
        |error| {
            matches!(
                error,
                PlanRejection::ComponentBinding {
                    failure: ComponentBindingFailure::Substituted {
                        field: "assumptions"
                    },
                    ..
                }
            )
        },
    );
    let mut mutated = rich_plan.clone();
    mutated.instances[0].component.assumptions = vec![identity(0xCC)];
    assert_plan_rejected(
        "instances[0].component.assumptions[0]::foreign",
        &mutated,
        rich_subject,
        &rich_request_bytes,
        &rich_components,
        |error| {
            matches!(
                error,
                PlanRejection::ComponentBinding {
                    failure: ComponentBindingFailure::Substituted {
                        field: "assumptions"
                    },
                    ..
                }
            )
        },
    );
    let mut mutated = rich_plan.clone();
    mutated.instances[0].component.assumptions = Vec::new();
    assert_plan_rejected(
        "instances[0].component.assumptions::dropped",
        &mutated,
        rich_subject,
        &rich_request_bytes,
        &rich_components,
        |error| {
            matches!(
                error,
                PlanRejection::ComponentBinding {
                    failure: ComponentBindingFailure::Substituted {
                        field: "assumptions"
                    },
                    ..
                }
            )
        },
    );
    // And the owner-acceptance check itself still bites on the *honest*
    // record: verified inventory demands the assumption, the owner does
    // not accept it.
    let mut unaccepting_request = rich_request.clone();
    unaccepting_request.accepted_assumptions = Vec::new();
    let unaccepting_bytes = encode_request(&unaccepting_request).unwrap();
    let (unaccepting_plan, _) = compose_plan(
        &unaccepting_request,
        &unaccepting_bytes,
        vec![
            verified_instance(support::name("api"), &rich_components[0]),
            verified_instance(support::name("authorization"), &rich_components[1]),
            verified_instance(support::name("billing"), &rich_components[2]),
        ],
        payment_bindings(),
        verifier(),
        &rich_components,
    )
    .expect("composition does not adjudicate owner acceptance");
    let unaccepting_plan_bytes = encode_plan(&unaccepting_plan).unwrap();
    assert!(
        matches!(
            verify_plan(&unaccepting_plan_bytes, &unaccepting_bytes, &rich_components),
            Err(PlanRejection::UnacceptedAssumption { assumption: found, .. })
                if found == assumption
        ),
        "instances[0].component.assumptions::unaccepted must reject"
    );

    // ── `instances[i].role`: the component/external-participant marker.
    //    Relabeling a verified subject claims unverified inventory — the
    //    binding rejects it. ──
    let mut mutated = plan.clone();
    mutated.instances[1].role = InstanceRole::ExternalParticipant;
    assert_plan_rejected(
        "instances[1].role::external-participant",
        &mutated,
        baseline_subject,
        &request_bytes,
        &components,
        |error| {
            matches!(
                error,
                PlanRejection::ComponentBinding {
                    failure: ComponentBindingFailure::ExternalSubjectVerified,
                    ..
                }
            )
        },
    );

    // ── `instances[i].endpoints[j]`: the public inventory the verified
    //    description establishes. Slot, direction, and contract are bound
    //    fields — a touched row is a substituted record, and roster axes on
    //    the inventory (dropped or invented endpoints, bound or not) reject
    //    the same way. ──
    // Slot mutations must keep `(slot, direction)` canonical order to stay
    // encodable: rename billing's and authorization's requirement exports
    // forward, not api's import backward.
    for (name, index, endpoint_index, slot) in [
        (
            "instances[1].endpoints[2].slot::export",
            1usize,
            2usize,
            9u32,
        ),
        ("instances[2].endpoints[1].slot", 2, 1, 9),
    ] {
        let mut mutated = plan.clone();
        mutated.instances[index].endpoints[endpoint_index].slot = slot;
        assert_plan_rejected(
            name,
            &mutated,
            baseline_subject,
            &request_bytes,
            &components,
            |error| {
                matches!(
                    error,
                    PlanRejection::ComponentBinding {
                        failure: ComponentBindingFailure::Substituted { field: "endpoints" },
                        ..
                    }
                )
            },
        );
    }
    // Renaming api's import slot forward leaves the roster unsorted — an
    // inventory-order violation the decoder itself refuses.
    let mut mutated = plan.clone();
    mutated.instances[0].endpoints[0].slot = 7;
    assert_plan_decode_rejected(
        "instances[0].endpoints[0].slot::unsorted",
        &mutated,
        &request_bytes,
        &components,
        |error| matches!(error, CodecError::NotCanonical { .. }),
    );
    // Relabeling billing's requirement export as an import stays sorted and
    // unique — representable, and bound-side substitution.
    let mut mutated = plan.clone();
    mutated.instances[2].endpoints[1].direction = EndpointDirection::Import;
    assert_plan_rejected(
        "instances[2].endpoints[1].direction",
        &mutated,
        baseline_subject,
        &request_bytes,
        &components,
        |error| {
            matches!(
                error,
                PlanRejection::ComponentBinding {
                    failure: ComponentBindingFailure::Substituted { field: "endpoints" },
                    ..
                }
            )
        },
    );
    for (name, instance_index, endpoint_index, contract) in [
        (
            "instances[0].endpoints[0].contract",
            0usize,
            0usize,
            identity(0xC2),
        ),
        ("instances[1].endpoints[1].contract", 1, 1, identity(0xC3)),
        (
            "instances[1].endpoints[0].contract::zeroed",
            1,
            0,
            [0u8; 32],
        ),
    ] {
        let mut mutated = plan.clone();
        mutated.instances[instance_index].endpoints[endpoint_index].contract = contract;
        assert_plan_rejected(
            name,
            &mutated,
            baseline_subject,
            &request_bytes,
            &components,
            |error| {
                matches!(
                    error,
                    PlanRejection::ComponentBinding {
                        failure: ComponentBindingFailure::Substituted { field: "endpoints" },
                        ..
                    }
                )
            },
        );
    }
    // Roster axes on the endpoint inventory: a dropped demanded import, an
    // invented demanded import, and an invented *export* — unbound exports
    // are legal in the graph but never in the verified roster — are all
    // substituted records.
    let mut mutated = plan.clone();
    mutated.instances[0].endpoints.remove(0);
    assert_plan_rejected(
        "instances[0].endpoints::dropped",
        &mutated,
        baseline_subject,
        &request_bytes,
        &components,
        |error| {
            matches!(
                error,
                PlanRejection::ComponentBinding {
                    failure: ComponentBindingFailure::Substituted { field: "endpoints" },
                    ..
                }
            )
        },
    );
    // An invented import must append past billing's highest slot to stay
    // canonical on the wire.
    let mut mutated = plan.clone();
    mutated.instances[2].endpoints.push(Endpoint {
        slot: 2,
        direction: EndpointDirection::Import,
        contract: identity(0xC0),
    });
    assert_plan_rejected(
        "instances[2].endpoints::added-import",
        &mutated,
        baseline_subject,
        &request_bytes,
        &components,
        |error| {
            matches!(
                error,
                PlanRejection::ComponentBinding {
                    failure: ComponentBindingFailure::Substituted { field: "endpoints" },
                    ..
                }
            )
        },
    );
    let mut mutated = plan.clone();
    mutated.instances[0].endpoints.push(Endpoint {
        slot: 5,
        direction: EndpointDirection::Export,
        contract: identity(0xC1),
    });
    assert_plan_rejected(
        "instances[0].endpoints::added-export",
        &mutated,
        baseline_subject,
        &request_bytes,
        &components,
        |error| {
            matches!(
                error,
                PlanRejection::ComponentBinding {
                    failure: ComponentBindingFailure::Substituted { field: "endpoints" },
                    ..
                }
            )
        },
    );
    // A duplicated `(slot, direction)` pair is representable in the value
    // domain but never canonical on the wire — insert it at its own sorted
    // position so the duplicate check, not the order check, sees it.
    let mut mutated = plan.clone();
    let duplicated = mutated.instances[0].endpoints[0].clone();
    mutated.instances[0].endpoints.insert(0, duplicated);
    assert_plan_decode_rejected(
        "instances[0].endpoints::duplicated",
        &mutated,
        &request_bytes,
        &components,
        |error| matches!(error, CodecError::Duplicate { .. }),
    );
    let mut mutated = plan.clone();
    mutated.instances[1].endpoints.swap(0, 1);
    assert_plan_decode_rejected(
        "instances[1].endpoints::reordered",
        &mutated,
        &request_bytes,
        &components,
        |error| matches!(error, CodecError::NotCanonical { .. }),
    );

    // ── `instances` roster axes. ──
    let mut mutated = plan.clone();
    mutated.instances.remove(2);
    assert_plan_rejected(
        "instances::dropped",
        &mutated,
        baseline_subject,
        &request_bytes,
        &components,
        |e| matches!(e, PlanRejection::RosterMismatch { .. }),
    );
    let mut mutated = plan.clone();
    mutated
        .instances
        .insert(1, instance("auditor", 0x44, &[9], &[]));
    assert_plan_rejected(
        "instances::inserted",
        &mutated,
        baseline_subject,
        &request_bytes,
        &components,
        |e| matches!(e, PlanRejection::RosterMismatch { .. }),
    );
    let mut mutated = plan.clone();
    mutated.instances.swap(0, 1);
    assert_plan_encode_rejected("instances::reordered", &mutated, |error| {
        matches!(error, CodecError::NotCanonical { .. })
    });
    let mut mutated = plan.clone();
    mutated.instances.insert(1, mutated.instances[0].clone());
    assert_plan_encode_rejected("instances::duplicated", &mutated, |error| {
        matches!(error, CodecError::Duplicate { .. })
    });

    // ── `bindings[i]`: import key, export key, transport. The inventory
    //    the keys resolve against is bound — so a mutated *key* still
    //    reaches graph normalization and rejects there. ──
    let mut mutated = plan.clone();
    mutated.bindings[0].import = endpoint(0, 7);
    assert_plan_rejected(
        "bindings[0].import.slot",
        &mutated,
        baseline_subject,
        &request_bytes,
        &components,
        |e| {
            matches!(
                e,
                PlanRejection::InvalidGraph(GraphError::UnknownEndpoint { .. })
            )
        },
    );
    // An out-of-roster import instance must keep binding order canonical to
    // stay encodable — an index on the second row does so.
    let mut mutated = plan.clone();
    mutated.bindings[1].import = endpoint(9, 0);
    assert_plan_rejected(
        "bindings[1].import.instance",
        &mutated,
        baseline_subject,
        &request_bytes,
        &components,
        |e| {
            matches!(
                e,
                PlanRejection::InvalidGraph(GraphError::UnknownInstance { .. })
            )
        },
    );
    // Re-pointing api's import at authorization's own import is
    // representable; the demanded import is then bound twice.
    let mut mutated = plan.clone();
    mutated.bindings[0].import = endpoint(1, 0);
    assert_plan_rejected(
        "bindings[0].import::other-import",
        &mutated,
        baseline_subject,
        &request_bytes,
        &components,
        |e| {
            matches!(
                e,
                PlanRejection::InvalidGraph(GraphError::DuplicateBinding { .. })
            )
        },
    );
    let mut mutated = plan.clone();
    mutated.bindings[0].export = endpoint(1, 9);
    assert_plan_rejected(
        "bindings[0].export.slot",
        &mutated,
        baseline_subject,
        &request_bytes,
        &components,
        |e| {
            matches!(
                e,
                PlanRejection::InvalidGraph(GraphError::UnknownEndpoint { .. })
            )
        },
    );
    let mut mutated = plan.clone();
    mutated.bindings[0].export = endpoint(9, 1);
    assert_plan_rejected(
        "bindings[0].export.instance",
        &mutated,
        baseline_subject,
        &request_bytes,
        &components,
        |e| {
            matches!(
                e,
                PlanRejection::InvalidGraph(GraphError::UnknownInstance { .. })
            )
        },
    );
    // Every import slot also carries an export at slot 0, so an
    // export-at-import mutation must come through the import key: billing's
    // canonical slot is export-only, and binding order still sorts it last.
    let mut mutated = plan.clone();
    mutated.bindings[1].import = endpoint(2, 0);
    assert_plan_rejected(
        "bindings[1].import::export-endpoint",
        &mutated,
        baseline_subject,
        &request_bytes,
        &components,
        |e| {
            matches!(
                e,
                PlanRejection::InvalidGraph(GraphError::WrongDirection { .. })
            )
        },
    );
    // Re-routing api directly to billing keeps every structural join —
    // canonical order, resolution, coverage, transport — and only the
    // independent predicate replay sees the route around authorization.
    let mut mutated = plan.clone();
    mutated.bindings[0].export = endpoint(2, 1);
    assert_plan_rejected(
        "bindings[0].export::rerouted",
        &mutated,
        baseline_subject,
        &request_bytes,
        &components,
        |e| matches!(e, PlanRejection::ReplayMismatch { index: 1 }),
    );
    for (name, index, transport) in [
        ("bindings[0].transport::unselected", 0usize, identity(0x78)),
        ("bindings[1].transport::unselected", 1, identity(0x99)),
        ("bindings[0].transport::zeroed", 0, [0u8; 32]),
    ] {
        let mut mutated = plan.clone();
        mutated.bindings[index].transport = transport;
        assert_plan_rejected(
            name,
            &mutated,
            baseline_subject,
            &request_bytes,
            &components,
            |e| matches!(e, PlanRejection::UnselectedTransport { .. }),
        );
    }
    // ── `bindings` roster axes. ──
    let mut mutated = plan.clone();
    mutated.bindings.remove(0);
    assert_plan_rejected(
        "bindings::dropped",
        &mutated,
        baseline_subject,
        &request_bytes,
        &components,
        |e| {
            matches!(
                e,
                PlanRejection::InvalidGraph(GraphError::UnboundImport { .. })
            )
        },
    );
    let mut mutated = plan.clone();
    mutated.bindings.insert(0, mutated.bindings[0].clone());
    assert_plan_encode_rejected("bindings::duplicated", &mutated, |error| {
        matches!(error, CodecError::Duplicate { .. })
    });
    let mut mutated = plan.clone();
    mutated.bindings.swap(0, 1);
    assert_plan_encode_rejected("bindings::reordered", &mutated, |error| {
        matches!(error, CodecError::NotCanonical { .. })
    });

    // ── `policies[i].verifier`: the recorded policy executable, compared
    //    to the request's selected verifier by identity. ──
    for (name, index, executable) in [
        ("policies[0].verifier", 0usize, identity(0x51)),
        ("policies[1].verifier", 1, identity(0x5F)),
        ("policies[0].verifier::zeroed", 0, [0u8; 32]),
    ] {
        let mut mutated = plan.clone();
        mutated.policies[index].verifier = executable;
        assert_plan_rejected(
            name,
            &mutated,
            baseline_subject,
            &request_bytes,
            &components,
            |e| matches!(e, PlanRejection::UnselectedPolicyExecutable { .. }),
        );
    }

    // ── `policies[i].call`: predicate identity and its exact selector
    //    arguments. Any change moves the canonical key off the required
    //    set — or off canonical order. ──
    let mut mutated = plan.clone();
    mutated.policies[0].call.predicate = PolicyPredicate::OnlyVia;
    assert_plan_encode_rejected("policies[0].call.predicate", &mutated, |error| {
        matches!(error, CodecError::NotCanonical { .. })
    });
    let mut mutated = plan.clone();
    mutated.policies[0].call.sources = PolicySelector::new([support::name("api")]);
    assert_plan_rejected(
        "policies[0].call.sources",
        &mutated,
        baseline_subject,
        &request_bytes,
        &components,
        |e| matches!(e, PlanRejection::UnexpectedPolicy { .. }),
    );
    let mut mutated = plan.clone();
    mutated.policies[0].call.targets = PolicySelector::new([support::name("authorization")]);
    assert_plan_rejected(
        "policies[0].call.targets",
        &mutated,
        baseline_subject,
        &request_bytes,
        &components,
        |e| matches!(e, PlanRejection::UnexpectedPolicy { .. }),
    );
    // Selector membership is part of the key: adding a member changes it.
    let mut mutated = plan.clone();
    mutated.policies[0].call.sources =
        PolicySelector::new([support::name("api"), support::name("billing")]);
    assert_plan_rejected(
        "policies[0].call.sources::extended",
        &mutated,
        baseline_subject,
        &request_bytes,
        &components,
        |e| matches!(e, PlanRejection::UnexpectedPolicy { .. }),
    );
    let mut mutated = plan.clone();
    mutated.policies[1].call.sources = PolicySelector::new([support::name("authorization")]);
    assert_plan_rejected(
        "policies[1].call.sources",
        &mutated,
        baseline_subject,
        &request_bytes,
        &components,
        |e| matches!(e, PlanRejection::UnexpectedPolicy { .. }),
    );
    let mut mutated = plan.clone();
    mutated.policies[1].call.targets = PolicySelector::new([support::name("api")]);
    assert_plan_rejected(
        "policies[1].call.targets",
        &mutated,
        baseline_subject,
        &request_bytes,
        &components,
        |e| matches!(e, PlanRejection::UnexpectedPolicy { .. }),
    );
    let mut mutated = plan.clone();
    mutated.policies[1].call.via = PolicySelector::new([support::name("billing")]);
    assert_plan_rejected(
        "policies[1].call.via",
        &mutated,
        baseline_subject,
        &request_bytes,
        &components,
        |e| matches!(e, PlanRejection::UnexpectedPolicy { .. }),
    );
    // A `via` member the roster does not contain keeps the key off the
    // required set — selector resolution never runs.
    let mut mutated = plan.clone();
    mutated.policies[1].call.via = PolicySelector::new([support::name("phantom")]);
    assert_plan_rejected(
        "policies[1].call.via::unknown-member",
        &mutated,
        baseline_subject,
        &request_bytes,
        &components,
        |e| matches!(e, PlanRejection::UnexpectedPolicy { .. }),
    );
    // `via` arity is a wire rule the writer does not enforce: a non-empty
    // `via` on `no_route` or an empty `via` on `only_via` encodes, then
    // rejects at decode.
    let mut mutated = plan.clone();
    mutated.policies[0].call.via = PolicySelector::new([support::name("api")]);
    assert_plan_decode_rejected(
        "policies[0].call.via::arity",
        &mutated,
        &request_bytes,
        &components,
        |error| matches!(error, CodecError::ViaArity),
    );
    let mut mutated = plan.clone();
    mutated.policies[1].call.via = PolicySelector {
        members: Vec::new(),
    };
    assert_plan_decode_rejected(
        "policies[1].call.via::arity",
        &mutated,
        &request_bytes,
        &components,
        |error| matches!(error, CodecError::ViaArity),
    );
    // Selector member order and uniqueness are wire rules too.
    let mut mutated = plan.clone();
    mutated.policies[0].call.sources = PolicySelector {
        members: vec![support::name("billing"), support::name("api")],
    };
    assert_plan_decode_rejected(
        "policies[0].call.sources::unordered",
        &mutated,
        &request_bytes,
        &components,
        |error| matches!(error, CodecError::NotCanonical { .. }),
    );
    let mut mutated = plan.clone();
    mutated.policies[0].call.sources = PolicySelector {
        members: vec![support::name("billing"), support::name("billing")],
    };
    assert_plan_decode_rejected(
        "policies[0].call.sources::duplicated",
        &mutated,
        &request_bytes,
        &components,
        |error| matches!(error, CodecError::Duplicate { .. }),
    );

    // ── `policies[i].outcome`: the recorded verdict and its checkable
    //    evidence. Baseline rows record `Satisfied` for
    //    `no_route({billing},{api})` at index 0 — certificate
    //    `NoRoute { reachable: [2] }` — and `only_via({api},{billing},
    //    {authorization})` at index 1 — certificate `OnlyVia { path:
    //    [0,1,2], reachable: [0] }`. ──
    for (name, violation) in [
        (
            "policies[0].outcome::bypass-recorded",
            Violation::Bypass {
                path: vec![2, 1, 0],
            },
        ),
        ("policies[0].outcome::disconnected", Violation::Disconnected),
    ] {
        let mut mutated = plan.clone();
        mutated.policies[0].outcome = PolicyOutcome::Violated { violation };
        assert_plan_rejected(
            name,
            &mutated,
            baseline_subject,
            &request_bytes,
            &components,
            |e| matches!(e, PlanRejection::PolicyNotSatisfied { index: 0 }),
        );
    }
    let mut mutated = plan.clone();
    mutated.policies[1].outcome = PolicyOutcome::Violated {
        violation: Violation::Disconnected,
    };
    assert_plan_rejected(
        "policies[1].outcome::disconnected",
        &mutated,
        baseline_subject,
        &request_bytes,
        &components,
        |e| matches!(e, PlanRejection::PolicyNotSatisfied { index: 1 }),
    );
    // Every certificate axis rejects at structural checking.
    for (name, certificate) in [
        // omits the source vertex billing(2)
        (
            "policies[0].certificate::missing-source",
            Certificate::NoRoute { reachable: vec![0] },
        ),
        // contains the forbidden target api(0)
        (
            "policies[0].certificate::contains-target",
            Certificate::NoRoute {
                reachable: vec![0, 2],
            },
        ),
        // not sorted-unique on the wire
        (
            "policies[0].certificate::unordered",
            Certificate::NoRoute {
                reachable: vec![2, 0],
            },
        ),
        (
            "policies[0].certificate::duplicated",
            Certificate::NoRoute {
                reachable: vec![2, 2],
            },
        ),
    ] {
        let mut mutated = plan.clone();
        mutated.policies[0].outcome = PolicyOutcome::Satisfied { certificate };
        assert_plan_rejected(
            name,
            &mutated,
            baseline_subject,
            &request_bytes,
            &components,
            |e| matches!(e, PlanRejection::InvalidCertificate { index: 0, .. }),
        );
    }
    for (name, certificate) in [
        // no edge api(0) -> billing(2) exists in the baseline graph
        (
            "policies[1].certificate::invalid-path",
            Certificate::OnlyVia {
                path: vec![0, 2],
                reachable: vec![0],
            },
        ),
        (
            "policies[1].certificate::empty-path",
            Certificate::OnlyVia {
                path: Vec::new(),
                reachable: vec![0],
            },
        ),
        // omits the source vertex api(0)
        (
            "policies[1].certificate::missing-source",
            Certificate::OnlyVia {
                path: vec![0, 1, 2],
                reachable: vec![1],
            },
        ),
        // contains the via vertex authorization(1)
        (
            "policies[1].certificate::contains-via",
            Certificate::OnlyVia {
                path: vec![0, 1, 2],
                reachable: vec![0, 1],
            },
        ),
        // contains the forbidden target billing(2)
        (
            "policies[1].certificate::contains-target",
            Certificate::OnlyVia {
                path: vec![0, 1, 2],
                reachable: vec![0, 2],
            },
        ),
        // not sorted-unique on the wire
        (
            "policies[1].certificate::unordered",
            Certificate::OnlyVia {
                path: vec![0, 1, 2],
                reachable: vec![0, 0],
            },
        ),
    ] {
        let mut mutated = plan.clone();
        mutated.policies[1].outcome = PolicyOutcome::Satisfied { certificate };
        assert_plan_rejected(
            name,
            &mutated,
            baseline_subject,
            &request_bytes,
            &components,
            |e| matches!(e, PlanRejection::InvalidCertificate { index: 1, .. }),
        );
    }
    // A certificate of the other predicate's shape is not even
    // representable: the outcome's wire layout is keyed by the call's
    // predicate tag, so the writer emits a record the decoder cannot parse
    // back — the byte stream misaligns and rejects before evidence checking
    // could ever see it.
    for (name, index, certificate) in [
        (
            "policies[0].certificate::wrong-shape",
            0usize,
            Certificate::OnlyVia {
                path: vec![2],
                reachable: vec![2],
            },
        ),
        (
            "policies[1].certificate::wrong-shape",
            1,
            Certificate::NoRoute { reachable: vec![0] },
        ),
    ] {
        let mut mutated = plan.clone();
        mutated.policies[index].outcome = PolicyOutcome::Satisfied { certificate };
        let bytes = encode_plan(&mutated).expect("the writer emits the stored shape");
        let error = decode_plan(&bytes)
            .expect_err("a foreign certificate shape cannot decode under this predicate");
        assert!(
            matches!(
                error,
                CodecError::UnknownTag { .. }
                    | CodecError::UnexpectedEnd { .. }
                    | CodecError::TrailingBytes { .. }
                    | CodecError::NotCanonical { .. }
                    | CodecError::Duplicate { .. }
            ),
            "{name}: the misaligned stream rejects: {error}"
        );
        assert!(
            matches!(
                verify_plan(&bytes, &request_bytes, &components),
                Err(PlanRejection::MalformedPlan(_))
            ),
            "{name}: replay surfaces the malformed plan"
        );
    }
    // An honestly enlarged reachable set that stays closed and disjoint is
    // admissible evidence — certificate minimality is not a rule — so only
    // the divergent published identity refuses the substitution.
    let mut mutated = plan.clone();
    mutated.policies[0].outcome = PolicyOutcome::Satisfied {
        certificate: Certificate::NoRoute {
            reachable: vec![1, 2],
        },
    };
    assert_plan_identity_bound(
        "policies[0].certificate::superset",
        &mutated,
        baseline_subject,
        &request_bytes,
        &components,
    );

    // ── `policies` roster axes. ──
    let mut mutated = plan.clone();
    mutated.policies.remove(0);
    assert_plan_rejected(
        "policies::dropped-first",
        &mutated,
        baseline_subject,
        &request_bytes,
        &components,
        |e| matches!(e, PlanRejection::MissingRequiredPolicy { .. }),
    );
    let mut mutated = plan.clone();
    mutated.policies.remove(1);
    assert_plan_rejected(
        "policies::dropped-last",
        &mutated,
        baseline_subject,
        &request_bytes,
        &components,
        |e| matches!(e, PlanRejection::MissingRequiredPolicy { .. }),
    );
    let mut mutated = plan.clone();
    mutated.policies.insert(
        0,
        ExecutedPolicy {
            call: PolicyCall::no_route(
                PolicySelector::new([support::name("api")]),
                PolicySelector::new([support::name("authorization")]),
            ),
            verifier: verifier(),
            outcome: PolicyOutcome::Satisfied {
                certificate: Certificate::NoRoute { reachable: vec![0] },
            },
        },
    );
    assert_plan_rejected(
        "policies::inserted",
        &mutated,
        baseline_subject,
        &request_bytes,
        &components,
        |e| matches!(e, PlanRejection::UnexpectedPolicy { .. }),
    );
    let mut mutated = plan.clone();
    mutated.policies.insert(1, mutated.policies[0].clone());
    assert_plan_encode_rejected("policies::duplicated", &mutated, |error| {
        matches!(error, CodecError::Duplicate { .. })
    });
    let mut mutated = plan.clone();
    mutated.policies.swap(0, 1);
    assert_plan_encode_rejected("policies::reordered", &mutated, |error| {
        matches!(error, CodecError::NotCanonical { .. })
    });
    // Swapping the two rows' recorded outcomes crosses each row's evidence
    // shape — the same unrepresentable wire as a wrong-shape certificate:
    // the no_route row's bytes now carry the two-list only_via layout.
    let mut mutated = plan.clone();
    let outcome = mutated.policies[0].outcome.clone();
    mutated.policies[0].outcome = mutated.policies[1].outcome.clone();
    mutated.policies[1].outcome = outcome;
    let bytes = encode_plan(&mutated).expect("the writer emits the swapped shapes");
    assert!(
        decode_plan(&bytes).is_err(),
        "policies::outcomes-swapped: crossed evidence shapes cannot decode"
    );
    assert!(
        matches!(
            verify_plan(&bytes, &request_bytes, &components),
            Err(PlanRejection::MalformedPlan(_))
        ),
        "policies::outcomes-swapped: replay surfaces the malformed plan"
    );
}

// ---- the request matrix --------------------------------------------------

#[test]
fn topology_request_rejects_every_one_field_substitution() {
    // Control: the baseline request commits and its plan verifies.
    let (request, request_bytes, plan, plan_bytes, components) = baseline();
    let baseline_commitment = request_commitment(&request_bytes);
    assert_eq!(plan.request_commitment, baseline_commitment);
    checked_baseline(&plan_bytes, &request_bytes);

    // ── `instances[i]`: the required roster of names bound to component
    //    subjects. ──
    for (name, index, renamed) in [
        ("instances[0].name", 0usize, "aqi"),
        ("instances[1].name", 1, "authorizatior"),
        ("instances[2].name", 2, "billing2"),
    ] {
        let mut mutated = request.clone();
        mutated.instances[index].name = support::name(renamed);
        assert_request_substitution(
            name,
            &mutated,
            Rebound::Rejected(|e| matches!(e, PlanRejection::RosterMismatch { .. })),
        );
    }
    for (name, index, subject) in [
        ("instances[0].subject", 0usize, identity(0x12)),
        ("instances[1].subject", 1, identity(0x23)),
        ("instances[2].subject::zeroed", 2, [0u8; 32]),
    ] {
        let mut mutated = request.clone();
        mutated.instances[index].subject = subject;
        assert_request_substitution(
            name,
            &mutated,
            Rebound::Rejected(|e| matches!(e, PlanRejection::RosterMismatch { .. })),
        );
    }
    let mut mutated = request.clone();
    mutated.instances.remove(2);
    assert_request_substitution(
        "instances::dropped",
        &mutated,
        Rebound::Rejected(|e| matches!(e, PlanRejection::RosterMismatch { .. })),
    );
    let mut mutated = request.clone();
    mutated.instances.insert(
        1,
        RequestedInstance {
            name: support::name("auditor"),
            subject: identity(0x44),
        },
    );
    assert_request_substitution(
        "instances::inserted",
        &mutated,
        Rebound::Rejected(|e| matches!(e, PlanRejection::RosterMismatch { .. })),
    );
    let mut mutated = request.clone();
    mutated.instances.swap(0, 1);
    assert_request_encode_rejected("instances::reordered", &mutated, |error| {
        matches!(error, CodecError::NotCanonical { .. })
    });
    let mut mutated = request.clone();
    mutated.instances.insert(1, mutated.instances[0].clone());
    assert_request_encode_rejected("instances::duplicated", &mutated, |error| {
        matches!(error, CodecError::Duplicate { .. })
    });

    // ── `policies[i]`: the required policy set. Mutating any argument moves
    //    the canonical key; the rebound plan's rows become unexpected or
    //    the new requirement unmet. ──
    let mut mutated = request.clone();
    mutated.policies[0].sources = PolicySelector::new([support::name("api")]);
    assert_request_substitution(
        "policies[0].sources",
        &mutated,
        Rebound::Rejected(|e| matches!(e, PlanRejection::UnexpectedPolicy { .. })),
    );
    let mut mutated = request.clone();
    mutated.policies[0].targets = PolicySelector::new([support::name("authorization")]);
    assert_request_substitution(
        "policies[0].targets",
        &mutated,
        Rebound::Rejected(|e| matches!(e, PlanRejection::UnexpectedPolicy { .. })),
    );
    let mut mutated = request.clone();
    mutated.policies[1].via = PolicySelector::new([support::name("billing")]);
    assert_request_substitution(
        "policies[1].via",
        &mutated,
        Rebound::Rejected(|e| matches!(e, PlanRejection::UnexpectedPolicy { .. })),
    );
    // Relaxing `only_via` to `no_route` keeps the sources/targets but drops
    // the routing obligation: the plan's recorded row is now unrequired. The
    // mutated call re-sorts ahead of the other `no_route` row, so the
    // request stays canonical.
    let mut mutated = request.clone();
    mutated.policies[1].predicate = PolicyPredicate::NoRoute;
    mutated.policies[1].via = PolicySelector {
        members: Vec::new(),
    };
    mutated.policies.sort_by_key(|call| call.canonical_key());
    assert_request_substitution(
        "policies[1].predicate::relaxed",
        &mutated,
        Rebound::Rejected(|e| matches!(e, PlanRejection::UnexpectedPolicy { .. })),
    );
    // Strengthening `no_route` to `only_via` adds an obligation the plan
    // never recorded.
    let mut mutated = request.clone();
    mutated.policies[0].predicate = PolicyPredicate::OnlyVia;
    mutated.policies[0].via = PolicySelector::new([support::name("authorization")]);
    assert_request_encode_rejected("policies[0].predicate::strengthened", &mutated, |error| {
        matches!(error, CodecError::NotCanonical { .. })
    });
    let mut mutated = request.clone();
    mutated.policies.remove(0);
    assert_request_substitution(
        "policies::dropped",
        &mutated,
        Rebound::Rejected(|e| matches!(e, PlanRejection::UnexpectedPolicy { .. })),
    );
    let mut mutated = request.clone();
    mutated.policies.insert(
        0,
        PolicyCall::no_route(
            PolicySelector::new([support::name("api")]),
            PolicySelector::new([support::name("authorization")]),
        ),
    );
    assert_request_substitution(
        "policies::inserted",
        &mutated,
        Rebound::Rejected(|e| matches!(e, PlanRejection::MissingRequiredPolicy { .. })),
    );
    let mut mutated = request.clone();
    mutated.policies.swap(0, 1);
    assert_request_encode_rejected("policies::reordered", &mutated, |error| {
        matches!(error, CodecError::NotCanonical { .. })
    });
    let mut mutated = request.clone();
    mutated.policies.insert(1, mutated.policies[0].clone());
    assert_request_encode_rejected("policies::duplicated", &mutated, |error| {
        matches!(error, CodecError::Duplicate { .. })
    });
    // `via` arity on the request's own wire: encodable, never decodable.
    let mut mutated = request.clone();
    mutated.policies[0].via = PolicySelector::new([support::name("api")]);
    let bytes = encode_request(&mutated).expect("the writer carries the arity violation");
    assert!(
        matches!(decode_request(&bytes), Err(CodecError::ViaArity)),
        "policies[0].via::arity must reject at decode"
    );
    assert!(
        matches!(
            verify_plan(&plan_bytes, &bytes, &components),
            Err(PlanRejection::MalformedRequest(CodecError::ViaArity))
        ),
        "policies[0].via::arity must surface as a malformed request"
    );

    // ── `verifier`: the one selected policy executable. ──
    for (name, selected) in [
        ("verifier::other", identity(0x51)),
        ("verifier::zeroed", [0u8; 32]),
        ("verifier::plan-subject", plan_subject(&plan_bytes)),
    ] {
        let mut mutated = request.clone();
        mutated.verifier = selected;
        assert_request_substitution(
            name,
            &mutated,
            Rebound::Rejected(|e| matches!(e, PlanRejection::UnselectedPolicyExecutable { .. })),
        );
    }

    // ── `transports`: the allowed binding realizations. Narrowing the
    //    profile strands the plan's bindings; widening it is admissible
    //    intent the plan already satisfies — under a divergent commitment. ──
    let mut mutated = request.clone();
    mutated.transports = vec![identity(0x78)];
    assert_request_substitution(
        "transports::substituted",
        &mutated,
        Rebound::Rejected(|e| matches!(e, PlanRejection::UnselectedTransport { .. })),
    );
    let mut mutated = request.clone();
    mutated.transports = Vec::new();
    assert_request_substitution(
        "transports::emptied",
        &mutated,
        Rebound::Rejected(|e| matches!(e, PlanRejection::UnselectedTransport { .. })),
    );
    let mut mutated = request.clone();
    mutated.transports = vec![identity(0x78), transport()];
    assert_request_substitution("transports::widened", &mutated, Rebound::Verifies);
    let mut mutated = request.clone();
    mutated.transports = vec![transport(), identity(0x78)];
    assert_request_encode_rejected("transports::reordered", &mutated, |error| {
        matches!(error, CodecError::NotCanonical { .. })
    });
    let mut mutated = request.clone();
    mutated.transports = vec![transport(), transport()];
    assert_request_encode_rejected("transports::duplicated", &mutated, |error| {
        matches!(error, CodecError::Duplicate { .. })
    });

    // ── `accepted_assumptions`: widening the owner's acceptance set cannot
    //    contradict a plan that demands none of it — admissible under a
    //    divergent commitment; unordered or repeated sets are unencodable. ──
    let mut mutated = request.clone();
    mutated.accepted_assumptions = vec![identity(0xAA)];
    assert_request_substitution("accepted_assumptions::added", &mutated, Rebound::Verifies);
    let mut mutated = request.clone();
    mutated.accepted_assumptions = vec![identity(0xBB), identity(0xAA)];
    assert_request_encode_rejected("accepted_assumptions::reordered", &mutated, |error| {
        matches!(error, CodecError::NotCanonical { .. })
    });
    let mut mutated = request.clone();
    mutated.accepted_assumptions = vec![identity(0xAA), identity(0xAA)];
    assert_request_encode_rejected("accepted_assumptions::duplicated", &mutated, |error| {
        matches!(error, CodecError::Duplicate { .. })
    });
}

// ---- nested wire canonicalization ----------------------------------------

/// The nested-order, arity, name, and ceiling rules the decoder enforces on
/// records the encoder will carry: representable in the value domain, never
/// on the wire. Raw-byte substitutions cover what no `InstanceName` value
/// can express.
#[test]
fn canonical_wire_rejects_unrepresentable_substitutions() {
    let (_request, request_bytes, plan, plan_bytes, components) = baseline();
    let baseline_subject = plan_subject(&plan_bytes);

    // Raw name bytes inside the first plan instance record: the value domain
    // cannot hold an empty, overlong, or non-UTF-8 name — the wire can be
    // made to say one anyway. Plan layout: magic(4) + version(4) +
    // request_commitment(32) + instance_count(4) → api's name at 44.
    let raw_name_cases: Vec<(
        &'static str,
        Box<dyn Fn(&mut Vec<u8>)>,
        fn(&CodecError) -> bool,
    )> = vec![
        (
            "instances[0].name::empty-wire",
            Box::new(|bytes: &mut Vec<u8>| bytes[44..48].copy_from_slice(&0u32.to_le_bytes())),
            |error| matches!(error, CodecError::InvalidName),
        ),
        (
            "instances[0].name::overlong-wire",
            Box::new(|bytes: &mut Vec<u8>| {
                bytes[44..48].copy_from_slice(&(MAX_NAME_BYTES as u32 + 1).to_le_bytes())
            }),
            |error| matches!(error, CodecError::LimitExceeded { .. }),
        ),
        (
            "instances[0].name::non-utf8-wire",
            Box::new(|bytes: &mut Vec<u8>| bytes[48..51].copy_from_slice(&[0xFF, 0xFE, 0xFD])),
            |error| matches!(error, CodecError::InvalidName),
        ),
    ];
    for (name, edit, expected) in raw_name_cases {
        let mut corrupted = plan_bytes.clone();
        edit(&mut corrupted);
        let error = decode_plan(&corrupted).expect_err("the wire substitution must reject");
        assert!(expected(&error), "{name}: unexpected rejection: {error}");
        assert!(
            matches!(
                verify_plan(&corrupted, &request_bytes, &components),
                Err(PlanRejection::MalformedPlan(_))
            ),
            "{name}: replay must surface the malformed plan"
        );
    }

    // The request record's own wire: the same name rules apply to the
    // request's first roster name, at offset 12 after magic+version+count.
    let mut corrupted = request_bytes.clone();
    corrupted[12..16].copy_from_slice(&0u32.to_le_bytes());
    assert!(
        matches!(decode_request(&corrupted), Err(CodecError::InvalidName)),
        "request instance name must reject empty on the wire"
    );

    // Description assumptions are emitted in stored order and required
    // sorted-unique on the wire.
    let mut mutated = plan.clone();
    mutated.instances[0].component.assumptions = vec![identity(0xBB), identity(0xAA)];
    assert_plan_decode_rejected(
        "component.assumptions::unordered",
        &mutated,
        &request_bytes,
        &components,
        |error| matches!(error, CodecError::NotCanonical { .. }),
    );
    let mut mutated = plan.clone();
    mutated.instances[0].component.assumptions = vec![identity(0xAA), identity(0xAA)];
    assert_plan_decode_rejected(
        "component.assumptions::duplicated",
        &mutated,
        &request_bytes,
        &components,
        |error| matches!(error, CodecError::Duplicate { .. }),
    );

    // Bounded ceilings bite before allocation on replay even though the
    // writer will emit an oversized section.
    let mut mutated = plan.clone();
    mutated.instances[0].endpoints = (0..=MAX_ENDPOINTS_PER_INSTANCE as u32)
        .map(|slot| Endpoint {
            slot,
            direction: EndpointDirection::Import,
            contract: identity(0xC0),
        })
        .collect();
    assert_plan_decode_rejected(
        "instances[0].endpoints::over-ceiling",
        &mutated,
        &request_bytes,
        &components,
        |error| matches!(error, CodecError::LimitExceeded { .. }),
    );
    let mut mutated = plan.clone();
    mutated.policies[0].call.sources = PolicySelector::new(
        (0..=MAX_SELECTOR_MEMBERS).map(|index| support::name(&format!("m{index:04}"))),
    );
    assert_plan_decode_rejected(
        "selector.members::over-ceiling",
        &mutated,
        &request_bytes,
        &components,
        |error| matches!(error, CodecError::LimitExceeded { .. }),
    );

    // A whole-section count lie is a bounded rejection, not an allocation:
    // the existing suite covers the instance count; here the binding count
    // is inflated past the section's real bytes. Its offset follows the
    // instance section — recompute it by decoding the prefix shape: magic
    // + version + commitment + count + three instance records.
    let binding_count_offset = {
        let mut offset = 44;
        for instance in &plan.instances {
            // Per instance on the wire: u32 name length + name bytes, u8
            // role, description (32+32+1+32 fixed + u32 assumption count +
            // 32 per assumption), u32 endpoint count + 37 bytes per endpoint.
            offset += 4
                + instance.name.as_str().len()
                + 1
                + 97
                + 4
                + 32 * instance.component.assumptions.len()
                + 4
                + 37 * instance.endpoints.len();
        }
        offset
    };
    let mut corrupted = plan_bytes.clone();
    assert_eq!(
        u32::from_le_bytes(
            corrupted[binding_count_offset..binding_count_offset + 4]
                .try_into()
                .unwrap()
        ),
        2,
        "fixture layout sanity: the baseline plan carries two bindings"
    );
    corrupted[binding_count_offset..binding_count_offset + 4]
        .copy_from_slice(&u32::MAX.to_le_bytes());
    let error = decode_plan(&corrupted).expect_err("an over-ceiling count must reject");
    assert!(
        matches!(error, CodecError::LimitExceeded { .. }),
        "binding-count ceiling: {error}"
    );

    // A foreign well-formed plan — honestly composed for a different
    // request — can never collide with the baseline's published subject, and
    // replays as stale under the baseline request.
    let mut foreign_request = payment_request();
    foreign_request.accepted_assumptions = vec![identity(0xAA)];
    let foreign_request_bytes = encode_request(&foreign_request).unwrap();
    let (foreign_plan, _) = compose_plan(
        &foreign_request,
        &foreign_request_bytes,
        payment_instances(),
        payment_bindings(),
        verifier(),
        &components,
    )
    .expect("foreign composition");
    let foreign_bytes = encode_plan(&foreign_plan).unwrap();
    assert_ne!(plan_subject(&foreign_bytes), baseline_subject);
    assert!(
        matches!(
            verify_plan(&foreign_bytes, &request_bytes, &components),
            Err(PlanRejection::StaleRequest { .. })
        ),
        "a foreign plan is stale under the baseline request"
    );
}

// ---- the installer-side request record ------------------------------------

/// `InstallationRequest` is the second owner-intent record: the commitment
/// the plan must answer, the issuance occurrence, and the admitted artifact
/// roster. Each field substitutes independently against the same checked
/// plan.
#[test]
fn installation_request_rejects_every_one_field_substitution() {
    let (_request, request_bytes, _plan, plan_bytes, components) = baseline();
    let commitment = request_commitment(&request_bytes);
    let checked = || checked_baseline(&plan_bytes, &request_bytes);

    // Control: the baseline authorization prepares a pending installation
    // with clean custody, then disarms it.
    let mut lifecycle = InstallationLifecycle::default();
    let authorization = lifecycle
        .authorize(installation_request(commitment, 7, &components))
        .expect("fresh issuance");
    let prepared = prepare_installation(checked(), authorization, SimAdapter::new())
        .expect("the baseline request prepares");
    prepared.disarm().expect("prepared custody disarms cleanly");

    // `expected_request`: any other commitment — foreign, zeroed, or the
    // plan's own subject — is a different authorization.
    for (name, expected) in [
        ("expected_request::foreign", identity(0xEE)),
        ("expected_request::zeroed", [0u8; 32]),
        ("expected_request::plan-subject", plan_subject(&plan_bytes)),
    ] {
        let mut lifecycle = InstallationLifecycle::default();
        let authorization = lifecycle
            .authorize(installation_request(expected, 7, &components))
            .expect("fresh issuance");
        let error = prepare_installation(checked(), authorization, SimAdapter::new())
            .expect_err("a substituted expected request must reject");
        match error {
            PrepareError::Rejected {
                rejection: InstallationRejection::UnauthorizedRequest { expected: e, found },
                leaked,
            } => {
                assert_eq!(e, expected, "{name}");
                assert_eq!(found, commitment, "{name}");
                assert!(leaked.is_empty(), "{name}: no endpoint custody issued");
            }
            other => panic!("{name}: expected UnauthorizedRequest, got {other}"),
        }
    }

    // `occurrence`: issuance requires strictly increasing generations —
    // replayed or moved-back occurrences reject at authorize, before a
    // checked plan is even consulted.
    let mut lifecycle = InstallationLifecycle::default();
    lifecycle
        .authorize(installation_request(commitment, 7, &components))
        .expect("first issuance");
    for (name, occurrence) in [
        ("occurrence::replayed", 7u64),
        ("occurrence::moved-back", 6),
    ] {
        let error = lifecycle
            .authorize(installation_request(commitment, occurrence, &components))
            .expect_err("a non-increasing occurrence must reject");
        assert!(
            matches!(error, InstallationRejection::ReplayedOccurrence { .. }),
            "{name}: {error}"
        );
    }

    // `artifacts[i]`: the roster must equal the instance roster in canonical
    // order, each artifact nonzero and bound to the instance's component
    // subject.
    let artifact_cases: Vec<(
        &'static str,
        Box<dyn Fn(&mut InstallationRequest)>,
        Box<dyn Fn(&InstallationRejection) -> bool>,
    )> = vec![
        (
            "artifacts::dropped",
            Box::new(|request: &mut InstallationRequest| {
                request.artifacts.remove(0);
            }),
            Box::new(|rejection: &InstallationRejection| {
                matches!(
                    rejection,
                    InstallationRejection::ArtifactCount {
                        expected: 3,
                        found: 2
                    }
                )
            }),
        ),
        (
            "artifacts::extended",
            Box::new(|request: &mut InstallationRequest| {
                request.artifacts.push(AdmittedArtifact {
                    artifact: identity(0xA4),
                    component_subject: identity(0x44),
                });
            }),
            Box::new(|rejection: &InstallationRejection| {
                matches!(
                    rejection,
                    InstallationRejection::ArtifactCount {
                        expected: 3,
                        found: 4
                    }
                )
            }),
        ),
        (
            "artifacts[0].artifact::zeroed",
            Box::new(|request: &mut InstallationRequest| {
                request.artifacts[0].artifact = [0u8; 32];
            }),
            Box::new(|rejection: &InstallationRejection| {
                matches!(rejection, InstallationRejection::NullIdentity { .. })
            }),
        ),
        (
            "artifacts[1].component_subject",
            Box::new(|request: &mut InstallationRequest| {
                request.artifacts[1].component_subject = identity(0x23);
            }),
            Box::new(|rejection: &InstallationRejection| {
                matches!(
                    rejection,
                    InstallationRejection::ArtifactMismatch { instance: 1 }
                )
            }),
        ),
        (
            "artifacts::reordered",
            Box::new(|request: &mut InstallationRequest| {
                request.artifacts.swap(0, 2);
            }),
            Box::new(|rejection: &InstallationRejection| {
                matches!(
                    rejection,
                    InstallationRejection::ArtifactMismatch { instance: 0 }
                )
            }),
        ),
    ];
    for (name, mutate, expected) in artifact_cases {
        let mut request = installation_request(commitment, 7, &components);
        mutate(&mut request);
        let mut lifecycle = InstallationLifecycle::default();
        let authorization = lifecycle.authorize(request).expect("fresh issuance");
        let error = prepare_installation(checked(), authorization, SimAdapter::new())
            .expect_err("a substituted artifact roster must reject");
        match error {
            PrepareError::Rejected { rejection, leaked } => {
                assert!(expected(&rejection), "{name}: {rejection}");
                assert!(leaked.is_empty(), "{name}: no endpoint custody issued");
            }
            other => panic!("{name}: expected a rejection, got {other}"),
        }
    }

    // `artifacts[i].artifact`: the provider-issued executable identity is
    // evidence adopted verbatim — a different nonzero identity prepares
    // because the installer's join binds the component subject, not the
    // provider's issuance record. The receipt carries the substitution
    // forward as divergent evidence.
    let mut request = installation_request(commitment, 7, &components);
    request.artifacts[0].artifact = identity(0xA9);
    let mut lifecycle = InstallationLifecycle::default();
    let authorization = lifecycle.authorize(request).expect("fresh issuance");
    let prepared = prepare_installation(checked(), authorization, SimAdapter::new())
        .expect("a substituted artifact identity is adopted evidence");
    prepared.disarm().expect("prepared custody disarms cleanly");

    // The physical-mapping axis: an adapter that mints two ends with one
    // token is a substituted mapping at creation — preparation refuses and
    // drains the ends it already holds.
    let mut adapter = SimAdapter::new();
    adapter.collide = true;
    let mut lifecycle = InstallationLifecycle::default();
    let authorization = lifecycle
        .authorize(installation_request(commitment, 7, &components))
        .expect("fresh issuance");
    let error = prepare_installation(checked(), authorization, adapter)
        .expect_err("an endpoint token collision must reject");
    match error {
        PrepareError::Rejected {
            rejection: InstallationRejection::EndpointTokenCollision { binding: 0 },
            leaked,
        } => {
            assert!(leaked.is_empty(), "collided custody drains cleanly");
        }
        other => panic!("expected EndpointTokenCollision, got {other}"),
    }
}
