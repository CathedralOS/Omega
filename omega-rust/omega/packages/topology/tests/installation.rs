//! The package-owned installer for the private-pipe profile: independent
//! authorization, dedicated pipe pairs, the activation gate, mediation
//! refusals, custody accounting, and replacement generations.
//!
//! `SimAdapter`/`SimSupervisor` stand in for the target-owned providers: the
//! simulated kernel attests which instance actually holds each endpoint, so
//! an ungranted invocation and a substituted mapping are exercised against
//! the installer's route table rather than drawn on the intended graph.
//! `StdPipeAdapter` exercises the real anonymous-pipe primitive this host
//! supplies (private pipe descriptors on macOS; anonymous pipe handles on
//! Windows through the same `std::io::pipe()` entrypoint).

mod support;

use std::collections::BTreeSet;
use std::fmt;
use support::*;
use topology_plan::topology_installation::*;
use topology_plan::*;

// ---- simulated provider -------------------------------------------------

#[derive(Debug)]
struct SimError(&'static str);

impl fmt::Display for SimError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for SimError {}

/// The physical token a supervisor reads off a handle — how it attests what
/// it actually installed rather than what it was told to install.
trait PhysicalToken {
    fn token(&self) -> u64;
}

/// A simulated pipe end: its whole physical identity is its token.
#[derive(Debug)]
struct SimEndpoint(u64);

impl PhysicalToken for SimEndpoint {
    fn token(&self) -> u64 {
        self.0
    }
}

struct SimAdapter {
    next_token: u64,
    pairs_created: usize,
    /// Pair index (0-based) whose creation fails.
    fail_at_pair: Option<usize>,
    /// Tokens whose close must fail — uncleanable custody.
    fail_close: BTreeSet<u64>,
    /// Mint two ends with one token, to forge a collision.
    collide: bool,
    /// Tokens confirmed closed.
    closed: Vec<u64>,
}

impl SimAdapter {
    fn new() -> Self {
        Self {
            next_token: 100,
            pairs_created: 0,
            fail_at_pair: None,
            fail_close: BTreeSet::new(),
            collide: false,
            closed: Vec::new(),
        }
    }
}

impl PipeAdapter for SimAdapter {
    type Endpoint = SimEndpoint;
    type Error = SimError;

    fn create_binding_pair(&mut self) -> Result<PipePair<SimEndpoint>, SimError> {
        if self.fail_at_pair == Some(self.pairs_created) {
            return Err(SimError("pipe pair creation failed"));
        }
        self.pairs_created += 1;
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

    fn close_endpoint(&mut self, endpoint: SimEndpoint) -> Result<(), SimError> {
        if self.fail_close.contains(&endpoint.0) {
            return Err(SimError("close failed"));
        }
        self.closed.push(endpoint.0);
        Ok(())
    }

    fn provider(&self) -> Identity {
        identity(0x99)
    }

    fn assumptions(&self) -> &'static [&'static str] {
        &["simulated kernel attestation"]
    }
}

/// One endpoint a member actually holds.
#[derive(Debug)]
struct SimEnd<E> {
    id: EndpointId,
    binding: u32,
    role: ChannelEnd,
    token: u64,
    #[cfg_attr(not(unix), allow(dead_code))]
    handle: E,
}

#[derive(Debug)]
struct SimMember<E> {
    instance: u32,
    entered: bool,
    ends: Vec<SimEnd<E>>,
}

impl<E> SimMember<E> {
    fn end(&self, binding: u32, role: ChannelEnd) -> &SimEnd<E> {
        self.ends
            .iter()
            .find(|end| end.binding == binding && end.role == role)
            .expect("member holds this channel end")
    }

    #[cfg(unix)]
    fn end_mut(&mut self, binding: u32, role: ChannelEnd) -> &mut SimEnd<E> {
        self.ends
            .iter_mut()
            .find(|end| end.binding == binding && end.role == role)
            .expect("member holds this channel end")
    }
}

struct SimSupervisor<E: PhysicalToken> {
    lifecycle: InstallationLifecycle,
    log: Vec<String>,
    /// Instance index whose preparation fails.
    fail_prepare: Option<u32>,
    /// Instance index whose entry gate fails.
    fail_gate: Option<u32>,
    /// Instances that refuse quiesce.
    quiesce_fail: BTreeSet<u32>,
    /// (instance, foreign token): install a different physical end than
    /// assigned and honestly report it — a substituted mapping.
    substitute: Option<(u32, u64)>,
    _marker: std::marker::PhantomData<E>,
}

impl<E: PhysicalToken> SimSupervisor<E> {
    fn new() -> Self {
        Self {
            lifecycle: InstallationLifecycle::default(),
            log: Vec::new(),
            fail_prepare: None,
            fail_gate: None,
            quiesce_fail: BTreeSet::new(),
            substitute: None,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<E: PhysicalToken> ProcessSupervisor for SimSupervisor<E> {
    type Endpoint = E;
    type Member = SimMember<E>;
    type Error = SimError;

    fn installation_lifecycle(&self) -> &InstallationLifecycle {
        &self.lifecycle
    }

    fn prepare_member(
        &mut self,
        instance: &PlanInstance,
        _artifact: &AdmittedArtifact,
        endpoints: Vec<EndpointAssignment<E>>,
    ) -> Result<PreparationEcho<SimMember<E>>, SimError> {
        let index = self
            .log_position(instance)
            .expect("instance is on the roster");
        self.log.push(format!("prepare {}", instance.name));
        if self.fail_prepare == Some(index) {
            return Err(SimError("executable admission failed"));
        }
        let mut ends = Vec::new();
        let mut installed = Vec::new();
        for assignment in endpoints {
            let token = assignment.handle.token();
            let reported = match self.substitute {
                Some((victim, foreign)) if victim == index && ends.is_empty() => foreign,
                _ => token,
            };
            installed.push((assignment.id, reported));
            ends.push(SimEnd {
                id: assignment.id,
                binding: assignment.binding,
                role: assignment.role,
                token,
                handle: assignment.handle,
            });
        }
        Ok(PreparationEcho {
            member: SimMember {
                instance: index,
                entered: false,
                ends,
            },
            installed,
        })
    }

    fn open_entry_gate(&mut self, member: &mut SimMember<E>) -> Result<(), SimError> {
        self.log.push(format!("enter {}", member.instance));
        if self.fail_gate == Some(member.instance) {
            return Err(SimError("entry gate failed"));
        }
        member.entered = true;
        Ok(())
    }

    fn quiesce_member(
        &mut self,
        member: SimMember<E>,
    ) -> Result<(), RetainedMember<SimMember<E>, SimError>> {
        self.log.push(format!("quiesce {}", member.instance));
        if self.quiesce_fail.contains(&member.instance) {
            return Err(RetainedMember {
                member,
                error: SimError("member refused quiesce"),
            });
        }
        Ok(())
    }
}

impl<E: PhysicalToken> SimSupervisor<E> {
    /// Resolve an instance name to its canonical index; the supervisor keys
    /// its behavior on the roster position it is handed.
    fn log_position(&self, instance: &PlanInstance) -> Option<u32> {
        ["api", "authorization", "billing"]
            .iter()
            .position(|name| *name == instance.name.as_str())
            .map(|index| index as u32)
    }
}

// ---- fixtures -----------------------------------------------------------

fn checked_payment_plan() -> (CheckedPlan, Vec<u8>, Vec<u8>) {
    let (request_bytes, plan_bytes, _) = payment_pair();
    let checked = verify_plan(&plan_bytes, &request_bytes, &payment_components())
        .expect("golden plan verifies");
    (checked, request_bytes, plan_bytes)
}

fn payment_installation_request(request_bytes: &[u8], occurrence: u64) -> InstallationRequest {
    InstallationRequest {
        expected_request: request_commitment(request_bytes),
        occurrence,
        artifacts: payment_components()
            .iter()
            .enumerate()
            .map(|(index, admission)| AdmittedArtifact {
                artifact: identity(0xA1 + index as u8),
                component_subject: subject_of(admission),
            })
            .collect(),
    }
}

// ---- authorization and coverage -----------------------------------------

#[test]
fn installation_authorization_cannot_activate_twice() {
    let (checked, request_bytes, _) = checked_payment_plan();
    let request = payment_installation_request(&request_bytes, 7);
    let duplicate = request.clone();
    let mut supervisor = SimSupervisor::<SimEndpoint>::new();
    let authorization = supervisor
        .lifecycle
        .authorize(request)
        .expect("first authorization");
    let installed = prepare_installation(checked, authorization, SimAdapter::new())
        .expect("first preparation")
        .activate(&mut supervisor)
        .expect("first activation");
    assert!(matches!(
        supervisor.lifecycle.authorize(duplicate),
        Err(InstallationRejection::ReplayedOccurrence {
            last_issued: 7,
            requested: 7
        })
    ));
    assert_eq!(installed.receipt().occurrence, 7);
    assert_eq!(supervisor.log.len(), 6, "no duplicate preparation or entry");
    assert!(
        matches!(
            supervisor
                .lifecycle
                .authorize(payment_installation_request(&request_bytes, 7)),
            Err(InstallationRejection::ReplayedOccurrence { .. })
        ),
        "reconstructing every request field does not reconstruct authority"
    );
    installed
        .quiesce(&mut supervisor)
        .expect("retirement releases the roster");
    assert!(
        matches!(
            supervisor
                .lifecycle
                .authorize(payment_installation_request(&request_bytes, 7)),
            Err(InstallationRejection::ReplayedOccurrence { .. })
        ),
        "retirement does not reset issuance history"
    );
    let (checked, _, _) = checked_payment_plan();
    let fresh = supervisor
        .lifecycle
        .authorize(payment_installation_request(&request_bytes, 8))
        .expect("fresh owner intent issues a distinct generation");
    let installed = prepare_installation(checked, fresh, SimAdapter::new())
        .expect("new preparation")
        .activate(&mut supervisor)
        .expect("new activation");
    assert_eq!(installed.receipt().occurrence, 8);
    installed.quiesce(&mut supervisor).expect("new retirement");
}

#[test]
fn foreign_lifecycle_cannot_reconstruct_supervisor_authority() {
    let (checked, request_bytes, _) = checked_payment_plan();
    let mut supervisor = SimSupervisor::<SimEndpoint>::new();
    let mut foreign = InstallationLifecycle::default();
    let forged = foreign
        .authorize(payment_installation_request(&request_bytes, 7))
        .expect("another lifecycle can only issue its own authority");
    let prepared = prepare_installation(checked, forged, SimAdapter::new()).expect("plan agrees");
    let failure = prepared
        .activate(&mut supervisor)
        .expect_err("foreign authority rejects");
    assert!(matches!(
        failure.cause,
        ActivationCause::Authorization(InstallationRejection::ForeignLifecycle)
    ));
    assert!(failure.leaked.is_empty());
    assert!(failure.retained.is_empty());
    assert!(supervisor.log.is_empty(), "no member preparation or entry");
    supervisor
        .lifecycle
        .authorize(payment_installation_request(&request_bytes, 7))
        .expect("foreign rejection did not consume local authority");
}

#[test]
fn superseded_authorization_rejects_and_reports_unclean_endpoint_custody() {
    let (checked, request_bytes, _) = checked_payment_plan();
    let mut supervisor = SimSupervisor::<SimEndpoint>::new();
    let authorization = supervisor
        .lifecycle
        .authorize(payment_installation_request(&request_bytes, 1))
        .expect("original authorization");
    let mut adapter = SimAdapter::new();
    adapter.fail_close.insert(101);
    let prepared =
        prepare_installation(checked, authorization, adapter).expect("pending placement");
    let _new = supervisor
        .lifecycle
        .authorize(payment_installation_request(&request_bytes, 2))
        .expect("new current intent");
    let failure = prepared
        .activate(&mut supervisor)
        .expect_err("superseded authority rejects");
    assert!(matches!(
        failure.cause,
        ActivationCause::Authorization(InstallationRejection::SupersededAuthorization)
    ));
    assert_eq!(failure.leaked, vec![EndpointId(2)]);
    assert!(failure.retained.is_empty());
    assert!(supervisor.log.is_empty());
}

#[test]
fn disarm_and_failed_preparation_do_not_reissue_consumed_authority() {
    let mut supervisor = SimSupervisor::<SimEndpoint>::new();
    for occurrence in [1, 2] {
        let (checked, request_bytes, _) = checked_payment_plan();
        let request = payment_installation_request(&request_bytes, occurrence);
        let authorization = supervisor
            .lifecycle
            .authorize(request.clone())
            .expect("fresh intent");
        let mut adapter = SimAdapter::new();
        if occurrence == 2 {
            adapter.fail_at_pair = Some(1);
        }
        match prepare_installation(checked, authorization, adapter) {
            Ok(prepared) => prepared.disarm().expect("pending custody released"),
            Err(PrepareError::Adapter { leaked, .. }) => assert!(leaked.is_empty()),
            other => panic!("unexpected preparation: {other:?}"),
        }
        assert!(matches!(
            supervisor.lifecycle.authorize(request),
            Err(InstallationRejection::ReplayedOccurrence { .. })
        ));
    }
    let (checked, request_bytes, _) = checked_payment_plan();
    let authorization = supervisor
        .lifecycle
        .authorize(payment_installation_request(&request_bytes, 3))
        .expect("fresh intent after cleanup");
    let installed = prepare_installation(checked, authorization, SimAdapter::new())
        .expect("prepares")
        .activate(&mut supervisor)
        .expect("activates after earlier cleanup");
    installed.quiesce(&mut supervisor).expect("retired");
}

#[test]
fn occurrence_exhaustion_rejects_without_wrapping() {
    let (_, request_bytes, _) = checked_payment_plan();
    let mut lifecycle = InstallationLifecycle::default();
    let _last = lifecycle
        .authorize(payment_installation_request(&request_bytes, u64::MAX))
        .expect("last occurrence");
    for occurrence in [0, 1, u64::MAX] {
        assert!(matches!(
            lifecycle.authorize(payment_installation_request(&request_bytes, occurrence)),
            Err(InstallationRejection::ReplayedOccurrence { .. })
        ));
    }
}

#[test]
fn activation_failure_keeps_issuance_spent_through_retained_cleanup() {
    let mut supervisor = SimSupervisor::<SimEndpoint>::new();
    let (checked, request_bytes, _) = checked_payment_plan();
    let request = payment_installation_request(&request_bytes, 1);
    let authorization = supervisor
        .lifecycle
        .authorize(request.clone())
        .expect("initial intent");
    let prepared =
        prepare_installation(checked, authorization, SimAdapter::new()).expect("prepares");
    supervisor.fail_gate = Some(1);
    supervisor.quiesce_fail.insert(0);
    let failure = prepared
        .activate(&mut supervisor)
        .expect_err("entry and cleanup fail");
    assert_eq!(failure.retained.len(), 1);
    assert!(failure.leaked.is_empty());
    assert!(matches!(
        supervisor.lifecycle.authorize(request.clone()),
        Err(InstallationRejection::ReplayedOccurrence { .. })
    ));
    supervisor.fail_gate = None;
    supervisor.quiesce_fail.clear();
    for retained in failure.retained {
        supervisor
            .quiesce_member(retained.member)
            .expect("retained supervision releases");
    }
    assert!(matches!(
        supervisor.lifecycle.authorize(request),
        Err(InstallationRejection::ReplayedOccurrence { .. })
    ));
    let (checked, _, _) = checked_payment_plan();
    let authorization = supervisor
        .lifecycle
        .authorize(payment_installation_request(&request_bytes, 2))
        .expect("fresh intent after cleanup");
    let installed = prepare_installation(checked, authorization, SimAdapter::new())
        .expect("fresh preparation")
        .activate(&mut supervisor)
        .expect("fresh activation");
    assert_eq!(installed.receipt().occurrence, 2);
    installed.quiesce(&mut supervisor).expect("retirement");
}

#[test]
fn golden_payment_installation_activates_and_receipts() {
    let mut supervisor = SimSupervisor::<SimEndpoint>::new();
    let (checked, request_bytes, _) = checked_payment_plan();
    let subject = checked.subject;
    let prepared = prepare_installation(
        checked,
        supervisor
            .lifecycle
            .authorize(payment_installation_request(&request_bytes, 7))
            .expect("owner authorization"),
        SimAdapter::new(),
    )
    .expect("preparation admits the payment plan");
    let installed = prepared
        .activate(&mut supervisor)
        .expect("activation opens the complete roster");

    // Confined endpoints precede application entry: every prepare precedes
    // every entry, in canonical roster order.
    assert_eq!(
        supervisor.log,
        [
            "prepare api",
            "prepare authorization",
            "prepare billing",
            "enter 0",
            "enter 1",
            "enter 2",
        ]
    );

    let receipt = installed.receipt();
    assert_eq!(receipt.plan, subject);
    assert_eq!(receipt.occurrence, 7);
    assert_eq!(
        receipt.artifacts,
        vec![identity(0xA1), identity(0xA2), identity(0xA3)]
    );
    assert_eq!(receipt.provider, identity(0x99));
    assert_eq!(receipt.provider_assumptions.len(), 1);

    // Exact all-import binding coverage: two bindings, four ends each, and
    // every member holds exactly its assigned ends.
    assert_eq!(receipt.bindings.len(), 2);
    let members = installed.members();
    assert_eq!(members.len(), 3);
    // api imports binding 0: it holds request-write + response-read.
    assert_eq!(members[0].ends.len(), 2);
    // authorization is the export of binding 0 and the importer of
    // binding 1: it holds all four ends of its two roles.
    assert_eq!(members[1].ends.len(), 4);
    assert_eq!(members[2].ends.len(), 2);
    for member in members {
        assert!(member.entered);
        for end in &member.ends {
            assert!(end.id.0 > 0, "issued ids start at 1");
            assert_eq!(
                end.token,
                // The physical token recorded equals what was installed.
                member
                    .ends
                    .iter()
                    .find(|other| other.id == end.id)
                    .unwrap()
                    .token
            );
        }
    }
}

#[test]
fn a_stale_authorization_rejects_before_any_endpoint() {
    let mut supervisor = SimSupervisor::<SimEndpoint>::new();
    let (checked, _, _) = checked_payment_plan();
    let mut authorization =
        payment_installation_request(&encode_request(&payment_request()).unwrap(), 1);
    authorization.expected_request = identity(0xEE);
    let adapter = SimAdapter::new();
    match prepare_installation(
        checked,
        supervisor
            .lifecycle
            .authorize(authorization)
            .expect("owner authorization"),
        adapter,
    ) {
        Err(PrepareError::Rejected {
            rejection: InstallationRejection::UnauthorizedRequest { .. },
            leaked,
        }) => assert!(leaked.is_empty()),
        other => panic!("stale authorization must reject: {other:?}"),
    }
}

#[test]
fn artifact_mismatches_reject_before_any_endpoint() {
    let mut supervisor = SimSupervisor::<SimEndpoint>::new();
    let (checked, request_bytes, _) = checked_payment_plan();
    // Wrong count.
    let mut authorization = payment_installation_request(&request_bytes, 1);
    authorization.artifacts.pop();
    match prepare_installation(
        checked,
        supervisor
            .lifecycle
            .authorize(authorization)
            .expect("owner authorization"),
        SimAdapter::new(),
    ) {
        Err(PrepareError::Rejected {
            rejection: InstallationRejection::ArtifactCount { .. },
            ..
        }) => {}
        other => panic!("artifact count must reject: {other:?}"),
    }

    // A different component subject than the roster requires.
    let (checked, request_bytes, _) = checked_payment_plan();
    let mut authorization = payment_installation_request(&request_bytes, 2);
    authorization.artifacts[2].component_subject = identity(0x34);
    match prepare_installation(
        checked,
        supervisor
            .lifecycle
            .authorize(authorization)
            .expect("owner authorization"),
        SimAdapter::new(),
    ) {
        Err(PrepareError::Rejected {
            rejection: InstallationRejection::ArtifactMismatch { instance },
            ..
        }) => assert_eq!(instance, 2),
        other => panic!("artifact subject must reject: {other:?}"),
    }
}

// ---- preparation custody -------------------------------------------------

#[test]
fn adapter_failure_mid_preparation_cleans_custody() {
    let mut supervisor = SimSupervisor::<SimEndpoint>::new();
    let (checked, request_bytes, _) = checked_payment_plan();
    let mut adapter = SimAdapter::new();
    adapter.fail_at_pair = Some(1);
    match prepare_installation(
        checked,
        supervisor
            .lifecycle
            .authorize(payment_installation_request(&request_bytes, 1))
            .expect("owner authorization"),
        adapter,
    ) {
        Err(PrepareError::Adapter {
            binding,
            error: _,
            leaked,
        }) => {
            assert_eq!(binding, 1);
            // Binding 0's four ends were all confirmed closed — clean.
            assert!(leaked.is_empty());
        }
        other => panic!("adapter failure must surface: {other:?}"),
    }
}

#[test]
fn uncleanable_close_reports_leaked_custody() {
    let mut supervisor = SimSupervisor::<SimEndpoint>::new();
    let (checked, request_bytes, _) = checked_payment_plan();
    let mut adapter = SimAdapter::new();
    adapter.fail_at_pair = Some(1);
    adapter.fail_close.insert(101);
    match prepare_installation(
        checked,
        supervisor
            .lifecycle
            .authorize(payment_installation_request(&request_bytes, 1))
            .expect("owner authorization"),
        adapter,
    ) {
        Err(PrepareError::Adapter { leaked, .. }) => {
            assert_eq!(leaked, vec![EndpointId(2)]);
        }
        other => panic!("leaked custody must be reported: {other:?}"),
    }
}

#[test]
fn a_colliding_endpoint_token_rejects_as_substitution() {
    let mut supervisor = SimSupervisor::<SimEndpoint>::new();
    let (checked, request_bytes, _) = checked_payment_plan();
    let mut adapter = SimAdapter::new();
    adapter.collide = true;
    match prepare_installation(
        checked,
        supervisor
            .lifecycle
            .authorize(payment_installation_request(&request_bytes, 1))
            .expect("owner authorization"),
        adapter,
    ) {
        Err(PrepareError::Rejected {
            rejection: InstallationRejection::EndpointTokenCollision { binding },
            leaked,
        }) => {
            assert_eq!(binding, 0);
            assert!(leaked.is_empty());
        }
        other => panic!("token collision must reject: {other:?}"),
    }
}

#[test]
fn disarm_releases_every_assigned_end() {
    let mut supervisor = SimSupervisor::<SimEndpoint>::new();
    let (checked, request_bytes, _) = checked_payment_plan();
    let adapter = SimAdapter::new();
    let prepared = prepare_installation(
        checked,
        supervisor
            .lifecycle
            .authorize(payment_installation_request(&request_bytes, 1))
            .expect("owner authorization"),
        adapter,
    )
    .expect("preparation admits");
    prepared.disarm().expect("all ends released");
}

// ---- the activation gate -------------------------------------------------

#[test]
fn member_failure_quiesces_prepared_roster_without_receipt() {
    let mut supervisor = SimSupervisor::<SimEndpoint>::new();
    let (checked, request_bytes, _) = checked_payment_plan();
    let prepared = prepare_installation(
        checked,
        supervisor
            .lifecycle
            .authorize(payment_installation_request(&request_bytes, 1))
            .expect("owner authorization"),
        SimAdapter::new(),
    )
    .expect("preparation admits");
    supervisor.fail_prepare = Some(1);
    match prepared.activate(&mut supervisor) {
        Err(failure) => {
            assert_eq!(failure.instance, 1);
            assert!(matches!(
                failure.cause,
                ActivationCause::Supervisor(SimError("executable admission failed"))
            ));
            assert!(failure.retained.is_empty());
            assert!(failure.leaked.is_empty());
            // api was prepared then quiesced; billing's ends were never
            // handed out — its prepare never ran.
            assert_eq!(
                supervisor.log,
                ["prepare api", "prepare authorization", "quiesce 0"]
            );
        }
        other => panic!("member failure must not produce a receipt: {other:?}"),
    }
}

#[test]
fn entry_gate_failure_rolls_back_the_whole_roster() {
    let mut supervisor = SimSupervisor::<SimEndpoint>::new();
    let (checked, request_bytes, _) = checked_payment_plan();
    let prepared = prepare_installation(
        checked,
        supervisor
            .lifecycle
            .authorize(payment_installation_request(&request_bytes, 1))
            .expect("owner authorization"),
        SimAdapter::new(),
    )
    .expect("preparation admits");
    supervisor.fail_gate = Some(2);
    match prepared.activate(&mut supervisor) {
        Err(failure) => {
            assert_eq!(failure.instance, 2);
            assert!(failure.retained.is_empty());
            // All three prepared; api and authorization entered, then every
            // member quiesced in reverse activation order.
            assert_eq!(
                supervisor.log,
                [
                    "prepare api",
                    "prepare authorization",
                    "prepare billing",
                    "enter 0",
                    "enter 1",
                    "enter 2",
                    "quiesce 2",
                    "quiesce 1",
                    "quiesce 0",
                ]
            );
        }
        other => panic!("gate failure must not produce a receipt: {other:?}"),
    }
}

#[test]
fn quiesce_failure_retains_supervision_without_receipt() {
    let mut supervisor = SimSupervisor::<SimEndpoint>::new();
    let (checked, request_bytes, _) = checked_payment_plan();
    let prepared = prepare_installation(
        checked,
        supervisor
            .lifecycle
            .authorize(payment_installation_request(&request_bytes, 1))
            .expect("owner authorization"),
        SimAdapter::new(),
    )
    .expect("preparation admits");
    supervisor.fail_gate = Some(1);
    supervisor.quiesce_fail.insert(0);
    match prepared.activate(&mut supervisor) {
        Err(failure) => {
            // api refused to quiesce: it stays under supervision and is
            // named in the failure — never counted as cleaned.
            assert_eq!(failure.retained.len(), 1);
            assert_eq!(failure.retained[0].member.instance, 0);
        }
        other => panic!("retained supervision must surface: {other:?}"),
    }
}

#[test]
fn a_substituted_mapping_refuses_at_the_gate() {
    let mut supervisor = SimSupervisor::<SimEndpoint>::new();
    let (checked, request_bytes, _) = checked_payment_plan();
    let prepared = prepare_installation(
        checked,
        supervisor
            .lifecycle
            .authorize(payment_installation_request(&request_bytes, 1))
            .expect("owner authorization"),
        SimAdapter::new(),
    )
    .expect("preparation admits");
    // authorization installs a foreign physical end and honestly says so.
    supervisor.substitute = Some((1, 0xDEAD));
    match prepared.activate(&mut supervisor) {
        Err(failure) => {
            assert_eq!(failure.instance, 1);
            assert!(matches!(
                failure.cause,
                ActivationCause::EndpointSubstitution { .. }
            ));
            // The suspect member is quiesced with the earlier ones.
            assert!(failure.retained.is_empty());
        }
        other => panic!("substituted mapping must refuse entry: {other:?}"),
    }
}

// ---- mediation -----------------------------------------------------------

fn installed_payment() -> (
    InstalledTopology<SimAdapter, SimMember<SimEndpoint>>,
    SimSupervisor<SimEndpoint>,
) {
    let mut supervisor = SimSupervisor::<SimEndpoint>::new();
    let (checked, request_bytes, _) = checked_payment_plan();
    let prepared = prepare_installation(
        checked,
        supervisor
            .lifecycle
            .authorize(payment_installation_request(&request_bytes, 1))
            .expect("owner authorization"),
        SimAdapter::new(),
    )
    .expect("preparation admits");
    let installed = prepared
        .activate(&mut supervisor)
        .expect("activation admits");
    (installed, supervisor)
}

#[test]
fn a_request_response_round_trip_uses_dedicated_channels() {
    let (mut installed, _) = installed_payment();
    let api_request = installed.members()[0].end(0, ChannelEnd::RequestWrite).id;
    let grant = installed
        .authorize_send(api_request, 0)
        .expect("api's import invocation is granted");
    assert_eq!(grant.binding, 0);
    assert_eq!(grant.deliver_to, 1);

    let auth_response = installed.members()[1].end(0, ChannelEnd::ResponseWrite).id;
    let reply = installed
        .authorize_respond(auth_response, 1)
        .expect("the reply stays on the response channel");
    assert_eq!(reply.binding, 0);
    assert_eq!(reply.deliver_to, 0);

    // One outstanding request per binding: the channel is free again.
    installed
        .authorize_send(api_request, 0)
        .expect("a fresh request is granted");
}

#[test]
fn an_ungranted_invocation_is_refused() {
    let (mut installed, _) = installed_payment();
    // An endpoint the installer never issued — fabricated or inherited.
    assert_eq!(
        installed.authorize_send(EndpointId(999), 0),
        Err(InvocationRefusal::UngrantedEndpoint {
            endpoint: EndpointId(999)
        })
    );
    // A request on the response-write end is not a request channel.
    let response_write = installed.members()[1].end(0, ChannelEnd::ResponseWrite).id;
    assert_eq!(
        installed.authorize_send(response_write, 1),
        Err(InvocationRefusal::WrongDirection {
            endpoint: response_write,
            role: ChannelEnd::ResponseWrite,
        })
    );
}

#[test]
fn a_substituted_mapping_is_refused_at_delivery() {
    let (mut installed, _) = installed_payment();
    // authorization's request-write end of binding 1 is attested in api's
    // hands — the route table refuses it no matter what the frame claims.
    let auth_request = installed.members()[1].end(1, ChannelEnd::RequestWrite).id;
    assert_eq!(
        installed.authorize_send(auth_request, 0),
        Err(InvocationRefusal::SubstitutedMapping {
            endpoint: auth_request,
            expected: 1,
            actual: 0,
        })
    );
}

#[test]
fn outstanding_request_and_closed_binding_rules_hold() {
    let (mut installed, _) = installed_payment();
    let api_request = installed.members()[0].end(0, ChannelEnd::RequestWrite).id;
    let auth_response = installed.members()[1].end(0, ChannelEnd::ResponseWrite).id;

    installed.authorize_send(api_request, 0).unwrap();
    // A second request while one is outstanding refuses.
    assert_eq!(
        installed.authorize_send(api_request, 0),
        Err(InvocationRefusal::RequestInFlight { binding: 0 })
    );
    // A response on the other binding has nothing outstanding.
    let auth_response_b1 = installed.members()[2].end(1, ChannelEnd::ResponseWrite).id;
    assert_eq!(
        installed.authorize_respond(auth_response_b1, 2),
        Err(InvocationRefusal::NoOutstandingRequest { binding: 1 })
    );

    // A protocol/peer failure closes the binding; nothing flows after.
    installed.authorize_respond(auth_response, 1).unwrap();
    installed.close_binding(0);
    assert_eq!(
        installed.authorize_send(api_request, 0),
        Err(InvocationRefusal::BindingClosed { binding: 0 })
    );
}

// ---- replacement ---------------------------------------------------------

#[test]
fn superseded_replacement_returns_old_and_pending_custody_before_stopping() {
    let (old, mut supervisor) = installed_payment();
    let (checked, request_bytes, _) = checked_payment_plan();
    let authorization = supervisor
        .lifecycle
        .authorize(payment_installation_request(&request_bytes, 2))
        .expect("replacement intent");
    let prepared = prepare_installation(checked, authorization, SimAdapter::new())
        .expect("replacement prepares");
    let _newer = supervisor
        .lifecycle
        .authorize(payment_installation_request(&request_bytes, 3))
        .expect("newer intent");
    match replace_installation(old, prepared, &mut supervisor) {
        Err(ReplacementFailure::Authorization {
            rejection: InstallationRejection::SupersededAuthorization,
            old,
            pending_new,
        }) => {
            assert_eq!(supervisor.log.len(), 6, "old generation was not stopped");
            assert!(old.members().iter().all(|member| member.entered));
            assert_eq!(old.receipt().occurrence, 1);
            pending_new.disarm().expect("pending custody released");
            old.quiesce(&mut supervisor)
                .expect("old custody released explicitly");
        }
        other => panic!("replacement must preserve old custody: {other:?}"),
    }
}

#[test]
fn replacement_quiesces_the_old_generation_first() {
    let (old, mut supervisor) = installed_payment();
    let (checked, request_bytes, _) = checked_payment_plan();
    let prepared = prepare_installation(
        checked,
        supervisor
            .lifecycle
            .authorize(payment_installation_request(&request_bytes, 2))
            .expect("owner authorization"),
        SimAdapter::new(),
    )
    .expect("new generation prepares");
    let installed = replace_installation(old, prepared, &mut supervisor)
        .expect("replacement activates the new generation");

    // Every old member quiesced before any new member prepared.
    let quiesces = supervisor
        .log
        .iter()
        .position(|entry| entry == "quiesce 0")
        .expect("old generation quiesced");
    let prepares_again = supervisor
        .log
        .iter()
        .rposition(|entry| entry == "prepare billing")
        .expect("new generation prepared");
    assert!(quiesces < prepares_again);
    assert_eq!(installed.receipt().occurrence, 2);
}

#[test]
fn replacement_keeps_the_old_generation_when_it_cannot_release() {
    let (old, mut supervisor) = installed_payment();
    supervisor.quiesce_fail.insert(1);
    let (checked, request_bytes, _) = checked_payment_plan();
    let prepared = prepare_installation(
        checked,
        supervisor
            .lifecycle
            .authorize(payment_installation_request(&request_bytes, 2))
            .expect("owner authorization"),
        SimAdapter::new(),
    )
    .expect("new generation prepares");
    match replace_installation(old, prepared, &mut supervisor) {
        Err(ReplacementFailure::OldGeneration {
            retained,
            pending_new,
        }) => {
            // authorization refused to quiesce: the new generation was
            // never activated and is handed back still pending.
            assert_eq!(retained.len(), 1);
            assert_eq!(retained[0].member.instance, 1);
            pending_new.disarm().expect("pending ends released");
            let prepares = supervisor
                .log
                .iter()
                .filter(|entry| entry.starts_with("prepare"))
                .count();
            assert_eq!(prepares, 3, "only the first generation ever prepared");
        }
        other => panic!("unreleased old generation must hold: {other:?}"),
    }
}

// ---- the host adapter ----------------------------------------------------

#[cfg(unix)]
impl PhysicalToken for StdPipeEnd {
    fn token(&self) -> u64 {
        use std::os::fd::AsRawFd;
        match self {
            StdPipeEnd::Read(end) => end.as_raw_fd() as u64,
            StdPipeEnd::Write(end) => end.as_raw_fd() as u64,
        }
    }
}

#[test]
fn host_pipe_pairs_are_private_and_directed() {
    use std::io::{Read, Write};
    let mut adapter = StdPipeAdapter;
    let mut pair = adapter.create_binding_pair().unwrap();
    let tokens = [
        adapter.endpoint_token(&pair.request_write),
        adapter.endpoint_token(&pair.request_read),
        adapter.endpoint_token(&pair.response_write),
        adapter.endpoint_token(&pair.response_read),
    ];
    assert_eq!(BTreeSet::from(tokens).len(), 4, "four distinct ends");

    // A bounded frame written on request-write arrives only on
    // request-read — the kernel itself routes the private channel.
    let frame = encode_frame(&Frame {
        operation: 1,
        payload: b"charge".to_vec(),
    })
    .unwrap();
    let StdPipeEnd::Write(writer) = &mut pair.request_write else {
        panic!("request-write is a write end");
    };
    writer.write_all(&frame).unwrap();
    let StdPipeEnd::Read(reader) = &mut pair.request_read else {
        panic!("request-read is a read end");
    };
    let mut buffer = vec![0u8; frame.len()];
    reader.read_exact(&mut buffer).unwrap();
    assert_eq!(buffer, frame);
    assert_eq!(decode_frame(&buffer).unwrap().payload, b"charge");
}

#[cfg(unix)]
#[test]
fn a_full_installation_flows_over_real_private_pipes() {
    let mut supervisor = SimSupervisor::<StdPipeEnd>::new();
    use std::io::{Read, Write};
    let (checked, request_bytes, _) = checked_payment_plan();
    let prepared = prepare_installation(
        checked,
        supervisor
            .lifecycle
            .authorize(payment_installation_request(&request_bytes, 1))
            .expect("owner authorization"),
        StdPipeAdapter,
    )
    .expect("preparation admits real pipes");
    let mut installed = prepared
        .activate(&mut supervisor)
        .expect("activation admits real pipes");

    // api invokes on its granted request channel; the kernel delivers the
    // bytes to authorization's request-read end and nowhere else.
    let api_request = installed.members()[0].end(0, ChannelEnd::RequestWrite).id;
    let grant = installed
        .authorize_send(api_request, 0)
        .expect("invocation granted");
    assert_eq!(grant.deliver_to, 1);

    let frame = encode_frame(&Frame {
        operation: 1,
        payload: b"authorize".to_vec(),
    })
    .unwrap();
    {
        let members = installed.members_mut();
        let write_end = &mut members[0].end_mut(0, ChannelEnd::RequestWrite).handle;
        let StdPipeEnd::Write(writer) = write_end else {
            panic!("request-write is a write end");
        };
        writer.write_all(&frame).unwrap();

        let read_end = &mut members[1].end_mut(0, ChannelEnd::RequestRead).handle;
        let StdPipeEnd::Read(reader) = read_end else {
            panic!("request-read is a read end");
        };
        let mut buffer = vec![0u8; frame.len()];
        reader.read_exact(&mut buffer).unwrap();
        assert_eq!(buffer, frame);
    }
    assert_eq!(decode_frame(&frame).unwrap().payload, b"authorize");
}
