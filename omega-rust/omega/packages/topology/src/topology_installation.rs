//! The package-owned installer: authorization, preparation, the activation
//! gate, mediation, and the installation receipt.
//!
//! `verify_plan` decides `composition checked`; this module sequences what
//! the contract calls `installation admitted` for the private-pipe profile:
//!
//! - `prepare_installation` joins the checked plan to an independently
//!   supplied `InstallationAuthorization` — the supervisor's current request
//!   commitment, the installation occurrence, and the per-instance admitted
//!   executables. A plan answering a stale request, or an artifact whose
//!   verified component subject differs from the roster, rejects before any
//!   endpoint exists.
//! - Preparation then creates one dedicated request/response pipe pair per
//!   binding through the selected [`PipeAdapter`] and assigns every end
//!   exactly: the importer receives the request-write and response-read
//!   ends, the exporter the request-read and response-write ends. Nothing
//!   else is minted, so confined endpoints are exact by construction. A
//!   failure closes every end created so far and reports any custody that
//!   could not be confirmed closed — leaked ends are named, never silently
//!   claimed clean.
//! - [`PreparedInstallation::activate`] is the runtime installation gate:
//!   it prepares every member through the [`ProcessSupervisor`] with exactly
//!   its assigned endpoints and checks each installed-token echo against
//!   the recorded assignment before any application entry opens. A
//!   substituted mapping refuses at the gate. A member failure quiesces
//!   every prepared or entered member and reports whichever could not be
//!   released — a partial roster never produces a receipt.
//! - The retained route table mediates every frame:
//!   [`InstalledTopology::authorize_send`] grants only a request-write end
//!   whose physically attested token — the kernel identity of the end
//!   itself, not a caller-supplied instance number — equals the recorded
//!   assignment, on an open binding with no outstanding request; the reply
//!   travels only on the binding's response channel. Unknown endpoints,
//!   wrong-direction sends, a foreign physical end, a second in-flight
//!   request, responses with nothing outstanding, and sends on a closed
//!   binding are each explicit refusals — an actual ungranted invocation
//!   and a substituted mapping are refused, not merely unlisted. Granted
//!   frames then pass the destination contract's exact operation schema
//!   before delivery ([`InstalledTopology::deliver_send`]); an invalid
//!   length, operation, or payload is a protocol/transport failure that
//!   closes the binding.
//!
//! What stays outside: executable admission policy and process custody
//! (the [`ProcessSupervisor`] owns both — `local_supervisor` is the unix
//! leg built on this crate's neutral process boundary), the schema
//! *contents* (each contract owns its own operation table; the mediator
//! enforces it, it does not derive it), and the OS guarantees named in
//! [`PipeAdapter::assumptions`] — those remain disclosed provider
//! assumptions, not properties this crate proves.

mod mediation;
mod operation_schema;
mod pipe_adapter;

#[cfg(unix)]
mod local_supervisor;

#[cfg(unix)]
pub use local_supervisor::{
    ExecutableLaunch, LocalMember, LocalProcessSupervisor, LocalSupervisorError, ReadVerdict,
    WriteVerdict,
};
pub use mediation::{DeliveryRefusal, InvocationRefusal, ResponseGrant, SendGrant};
mod frame;

pub use frame::{Frame, FrameError, MAX_FRAME_BYTES, decode_frame, encode_frame};
pub use operation_schema::{
    OperationSchema, OperationSchemas, PayloadSchema, SchemaRegistrationError, SchemaViolation,
};
pub use pipe_adapter::{PipeAdapter, PipePair, StdPipeAdapter, StdPipeEnd};

use crate::deployment_plan::{
    EndpointDirection, EndpointKey, Identity, InstanceName, PlanInstance,
};
use crate::plan_verification::CheckedPlan;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

/// Validate the authorization against a checked plan, then allocate and
/// assign every binding's private pipe pair.
///
/// `checked` must come from `verify_plan` against the *current* request
/// bytes — this function never re-runs verification, and a plan that was
/// never checked cannot reach here because `CheckedPlan` is only
/// constructible by the verifier. The authorization is compared exactly:
/// its expected request commitment must equal the plan's recorded
/// commitment, and each artifact's verified component subject must equal
/// the roster's.
pub fn prepare_installation<A: PipeAdapter>(
    checked: CheckedPlan,
    authorization: InstallationAuthorization,
    mut adapter: A,
    schemas: OperationSchemas,
) -> Result<PreparedInstallation<A>, PrepareError<A>> {
    let request = &authorization.request;
    if checked.plan.request_commitment != request.expected_request {
        return Err(PrepareError::Rejected {
            rejection: InstallationRejection::UnauthorizedRequest {
                expected: request.expected_request,
                found: checked.plan.request_commitment,
            },
            leaked: Vec::new(),
        });
    }
    let instances = checked.graph.instances();
    if request.artifacts.len() != instances.len() {
        return Err(PrepareError::Rejected {
            rejection: InstallationRejection::ArtifactCount {
                expected: instances.len(),
                found: request.artifacts.len(),
            },
            leaked: Vec::new(),
        });
    }
    for (index, (artifact, instance)) in request.artifacts.iter().zip(instances.iter()).enumerate()
    {
        if artifact.artifact == [0u8; 32] {
            return Err(PrepareError::Rejected {
                rejection: InstallationRejection::NullIdentity {
                    field: "artifact identity",
                },
                leaked: Vec::new(),
            });
        }
        if artifact.component_subject != instance.component.subject {
            return Err(PrepareError::Rejected {
                rejection: InstallationRejection::ArtifactMismatch {
                    instance: index as u32,
                },
                leaked: Vec::new(),
            });
        }
    }

    // Every bound endpoint's contract must have a registered operation
    // schema before any end exists — a channel the selected codec cannot
    // check is refused at admission rather than discovered mid-wire.
    for binding in &checked.plan.bindings {
        for (endpoint_key, direction) in [
            (binding.import, EndpointDirection::Import),
            (binding.export, EndpointDirection::Export),
        ] {
            let contract = endpoint_contract(instances, endpoint_key, direction);
            if schemas.for_contract(&contract).is_none() {
                return Err(PrepareError::Rejected {
                    rejection: InstallationRejection::MissingOperationSchema {
                        endpoint: endpoint_key,
                        contract,
                    },
                    leaked: Vec::new(),
                });
            }
        }
    }

    // Allocate one dedicated pair per binding in canonical order and assign
    // every end exactly once. EndpointId(0) stays reserved.
    let mut next_id = 1u32;
    let mut routes = Vec::with_capacity(checked.plan.bindings.len() * 4);
    let mut assigned: Vec<Vec<EndpointAssignment<A::Endpoint>>> =
        (0..instances.len()).map(|_| Vec::new()).collect();
    let mut seen_tokens = BTreeSet::new();

    for (binding_index, binding) in checked.plan.bindings.iter().enumerate() {
        let pair = match adapter.create_binding_pair() {
            Ok(pair) => pair,
            Err(error) => {
                let leaked = drain_assigned(&mut adapter, &mut assigned);
                return Err(PrepareError::Adapter {
                    binding: binding_index as u32,
                    error,
                    leaked,
                });
            }
        };
        let ends = [
            (
                binding.import,
                ChannelEnd::RequestWrite,
                binding.import.instance,
                pair.request_write,
            ),
            (
                binding.export,
                ChannelEnd::RequestRead,
                binding.export.instance,
                pair.request_read,
            ),
            (
                binding.export,
                ChannelEnd::ResponseWrite,
                binding.export.instance,
                pair.response_write,
            ),
            (
                binding.import,
                ChannelEnd::ResponseRead,
                binding.import.instance,
                pair.response_read,
            ),
        ];
        // Read every end's physical token before assigning it anywhere. An
        // adapter that cannot name an end fails the pair in place — the
        // unassigned handles are closed first so custody accounting never
        // loses them silently.
        let mut tokens = [0u64; 4];
        let mut collision = false;
        let mut token_failure = None;
        for (index, (_, _, _, handle)) in ends.iter().enumerate() {
            match adapter.endpoint_token(handle) {
                Ok(token) => {
                    tokens[index] = token;
                    if !seen_tokens.insert(token) {
                        collision = true;
                    }
                }
                Err(error) => {
                    token_failure = Some(error);
                    break;
                }
            }
        }
        if collision || token_failure.is_some() {
            let mut leaked = Vec::new();
            for (index, (_, _, _, handle)) in ends.into_iter().enumerate() {
                if adapter.close_endpoint(handle).is_err() {
                    leaked.push(EndpointId(next_id + index as u32));
                }
            }
            leaked.extend(drain_assigned(&mut adapter, &mut assigned));
            return Err(match token_failure {
                Some(error) => PrepareError::Adapter {
                    binding: binding_index as u32,
                    error,
                    leaked,
                },
                None => PrepareError::Rejected {
                    rejection: InstallationRejection::EndpointTokenCollision {
                        binding: binding_index as u32,
                    },
                    leaked,
                },
            });
        }
        for ((endpoint, role, holder, handle), token) in ends.into_iter().zip(tokens) {
            let id = EndpointId(next_id);
            next_id += 1;
            routes.push(RouteRecord {
                id,
                binding: binding_index as u32,
                endpoint,
                role,
                holder,
                contract: endpoint_contract(instances, endpoint, role_direction(role)),
                token,
            });
            assigned[holder as usize].push(EndpointAssignment {
                binding: binding_index as u32,
                endpoint,
                role,
                id,
                token,
                handle,
            });
        }
    }

    Ok(PreparedInstallation {
        checked,
        authorization,
        schemas,
        assigned,
        routes,
        adapter,
    })
}

/// The contract one checked-plan endpoint declares: the roster record is
/// the only honest source.
fn endpoint_contract(
    instances: &[PlanInstance],
    key: EndpointKey,
    direction: EndpointDirection,
) -> Identity {
    instances[key.instance as usize]
        .endpoints
        .iter()
        .find(|endpoint| endpoint.slot == key.slot && endpoint.direction == direction)
        .expect("a checked plan binds only endpoints the roster declares")
        .contract
}

/// The direction of the plan endpoint a channel role serves: request-write
/// and response-read are the importer's ends; request-read and
/// response-write are the exporter's.
fn role_direction(role: ChannelEnd) -> EndpointDirection {
    match role {
        ChannelEnd::RequestWrite | ChannelEnd::ResponseRead => EndpointDirection::Import,
        ChannelEnd::RequestRead | ChannelEnd::ResponseWrite => EndpointDirection::Export,
    }
}

impl<A: PipeAdapter> PreparedInstallation<A> {
    /// The gate: prepare every member with exactly its assigned endpoints,
    /// check each installed-token echo against the route table, then open
    /// application entries in canonical order. Confined endpoints always
    /// precede application entry — no member runs application code while
    /// any binding or roster member is unadmitted.
    pub fn activate<S>(
        mut self,
        supervisor: &mut S,
    ) -> Result<InstalledTopology<A, S::Member>, ActivationFailure<S>>
    where
        S: ProcessSupervisor<Endpoint = A::Endpoint>,
    {
        if let Err(rejection) = supervisor
            .installation_lifecycle()
            .check_authorization(&self.authorization)
        {
            let leaked = drain_assigned(&mut self.adapter, &mut self.assigned);
            return Err(ActivationFailure {
                instance: 0,
                cause: ActivationCause::Authorization(rejection),
                retained: Vec::new(),
                leaked,
            });
        }
        let count = self.checked.graph.instances().len();
        let mut members: Vec<S::Member> = Vec::with_capacity(count);

        // Pass 1: admit every member. No entry opens in this pass.
        for index in 0..count {
            let endpoints = std::mem::take(&mut self.assigned[index]);
            let artifact = &self.authorization.request.artifacts[index];
            let instance = &self.checked.graph.instances()[index];
            let echo = match supervisor.prepare_member(instance, artifact, endpoints) {
                Ok(echo) => echo,
                Err(error) => {
                    let (retained, leaked) = self.rollback(supervisor, members);
                    return Err(ActivationFailure {
                        instance: index as u32,
                        cause: ActivationCause::Supervisor(error),
                        retained,
                        leaked,
                    });
                }
            };
            if let Err(detail) = self.check_echo(index as u32, &echo.installed) {
                members.push(echo.member);
                let (retained, leaked) = self.rollback(supervisor, members);
                return Err(ActivationFailure {
                    instance: index as u32,
                    cause: ActivationCause::EndpointSubstitution { detail },
                    retained,
                    leaked,
                });
            }
            members.push(echo.member);
        }

        // Pass 2: the complete roster holds confined endpoints — open
        // application entries in canonical order.
        for index in 0..count {
            if let Err(error) = supervisor.open_entry_gate(&mut members[index]) {
                let (retained, leaked) = self.rollback(supervisor, members);
                return Err(ActivationFailure {
                    instance: index as u32,
                    cause: ActivationCause::Supervisor(error),
                    retained,
                    leaked,
                });
            }
        }

        let receipt = InstallationReceipt {
            plan: self.checked.subject,
            occurrence: self.authorization.request.occurrence,
            artifacts: self
                .authorization
                .request
                .artifacts
                .iter()
                .map(|artifact| artifact.artifact)
                .collect(),
            bindings: self.binding_mappings(),
            provider: self.adapter.provider(),
            provider_assumptions: self.adapter.assumptions().to_vec(),
        };
        Ok(InstalledTopology {
            receipt,
            routes: self.routes,
            schemas: self.schemas,
            in_flight: BTreeSet::new(),
            closed_bindings: BTreeSet::new(),
            members,
            adapter: self.adapter,
        })
    }

    /// Compare the supervisor's installed-token echo to the recorded
    /// assignment for `instance`: the multiset of `(id, token)` must be
    /// exactly equal. Any extra, missing, or physically different end is a
    /// substituted mapping.
    fn check_echo(&self, instance: u32, installed: &[(EndpointId, u64)]) -> Result<(), String> {
        let expected: BTreeMap<EndpointId, u64> = self
            .routes
            .iter()
            .filter(|record| record.holder == instance)
            .map(|record| (record.id, record.token))
            .collect();
        let found: BTreeMap<EndpointId, u64> = installed.iter().copied().collect();
        if found.len() != installed.len() {
            return Err("the echo reports one endpoint id twice".to_owned());
        }
        if expected == found {
            return Ok(());
        }
        let mut detail = String::new();
        for (id, token) in &expected {
            match found.get(id) {
                None => detail.push_str(&format!("endpoint {} never installed; ", id.0)),
                Some(other) if other != token => detail.push_str(&format!(
                    "endpoint {} installed with foreign token {other}; ",
                    id.0
                )),
                Some(_) => {}
            }
        }
        for id in found.keys() {
            if !expected.contains_key(id) {
                detail.push_str(&format!("unassigned endpoint {} installed; ", id.0));
            }
        }
        Err(detail)
    }

    /// Quiesce every member in reverse activation order and close every end
    /// still in installer custody. Returns `(retained members, leaked ends)`.
    fn rollback<S>(
        &mut self,
        supervisor: &mut S,
        members: Vec<S::Member>,
    ) -> (Vec<RetainedMember<S::Member, S::Error>>, Vec<EndpointId>)
    where
        S: ProcessSupervisor<Endpoint = A::Endpoint>,
    {
        let mut retained = Vec::new();
        for member in members.into_iter().rev() {
            if let Err(retained_member) = supervisor.quiesce_member(member) {
                retained.push(retained_member);
            }
        }
        let leaked = drain_assigned(&mut self.adapter, &mut self.assigned);
        (retained, leaked)
    }

    /// Per-binding physical mapping for the receipt, in canonical order.
    fn binding_mappings(&self) -> Vec<BindingMapping> {
        self.checked
            .plan
            .bindings
            .iter()
            .enumerate()
            .map(|(binding_index, binding)| {
                let mut endpoints = [(EndpointId(0), 0u64); 4];
                for record in self
                    .routes
                    .iter()
                    .filter(|record| record.binding == binding_index as u32)
                {
                    // The physical mapping must serve the plan's own
                    // endpoint pair: request channel ends belong to the
                    // import/export endpoints the binding names.
                    let expected = match record.role {
                        ChannelEnd::RequestWrite | ChannelEnd::ResponseRead => binding.import,
                        ChannelEnd::RequestRead | ChannelEnd::ResponseWrite => binding.export,
                    };
                    debug_assert!(
                        record.endpoint == expected,
                        "route record serves a different plan endpoint than its binding"
                    );
                    let slot = match record.role {
                        ChannelEnd::RequestWrite => 0,
                        ChannelEnd::RequestRead => 1,
                        ChannelEnd::ResponseWrite => 2,
                        ChannelEnd::ResponseRead => 3,
                    };
                    endpoints[slot] = (record.id, record.token);
                }
                BindingMapping {
                    import: binding.import,
                    export: binding.export,
                    endpoints,
                }
            })
            .collect()
    }

    /// Release every allocated end without activating. `Ok` confirms all
    /// custody closed; `Err` names the ends whose close failed.
    pub fn disarm(mut self) -> Result<(), Vec<EndpointId>> {
        let leaked = drain_assigned(&mut self.adapter, &mut self.assigned);
        if leaked.is_empty() {
            Ok(())
        } else {
            Err(leaked)
        }
    }
}

impl<A: PipeAdapter, Member> InstalledTopology<A, Member> {
    /// The `installation admitted` evidence for this generation.
    pub fn receipt(&self) -> &InstallationReceipt {
        &self.receipt
    }

    /// Borrow the member roster in canonical instance order.
    pub fn members(&self) -> &[Member] {
        &self.members
    }

    /// Borrow the member roster mutably — the mediator drives frames and
    /// lifecycle through the members' installed endpoints.
    pub fn members_mut(&mut self) -> &mut [Member] {
        &mut self.members
    }

    /// The selected transport provider. It is retained for the whole
    /// installation lifetime — the mediator uses it to read tokens and
    /// deliver frames on the physical channels it created.
    pub fn adapter(&self) -> &A {
        &self.adapter
    }

    /// Stop the generation: quiesce every member in canonical roster order.
    /// Callers drain before their callees stop — an importer never loses
    /// its exporter mid-request while an orderly shutdown remains possible.
    /// `Ok` confirms the whole roster released; `Err` returns exactly the
    /// members still under supervision.
    pub fn quiesce<S>(
        self,
        supervisor: &mut S,
    ) -> Result<(), Vec<RetainedMember<S::Member, S::Error>>>
    where
        S: ProcessSupervisor<Endpoint = A::Endpoint, Member = Member>,
    {
        let mut retained = Vec::new();
        for member in self.members {
            if let Err(retained_member) = supervisor.quiesce_member(member) {
                retained.push(retained_member);
            }
        }
        if retained.is_empty() {
            Ok(())
        } else {
            Err(retained)
        }
    }
}

/// Replace one generation with the next. The new plan must already be a
/// `PreparedInstallation` — its request was independently verified and its
/// authorization checked before this call, so a stale candidate cannot
/// reach here. The old generation quiesces completely before the new one
/// activates; failure before that point leaves the old installation
/// unchanged, and failure after it can cause downtime but never a silent
/// restart under different authority.
pub fn replace_installation<A, S>(
    old: InstalledTopology<A, S::Member>,
    new: PreparedInstallation<A>,
    supervisor: &mut S,
) -> Result<InstalledTopology<A, S::Member>, ReplacementFailure<A, S>>
where
    A: PipeAdapter,
    S: ProcessSupervisor<Endpoint = A::Endpoint>,
{
    if let Err(rejection) = supervisor
        .installation_lifecycle()
        .check_authorization(&new.authorization)
    {
        return Err(ReplacementFailure::Authorization {
            rejection,
            old,
            pending_new: new,
        });
    }
    if let Err(retained) = old.quiesce(supervisor) {
        return Err(ReplacementFailure::OldGeneration {
            retained,
            pending_new: new,
        });
    }
    new.activate(supervisor)
        .map_err(ReplacementFailure::Activation)
}

/// Installer-issued identity of one physical pipe end. `EndpointId(0)` is
/// the reserved absence state; issued ids start at 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct EndpointId(pub u32);

/// Which end of one binding's pair a physical endpoint serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelEnd {
    /// Writes request frames; held by the importing instance.
    RequestWrite,
    /// Reads request frames; held by the exporting instance.
    RequestRead,
    /// Writes the response frame; held by the exporting instance.
    ResponseWrite,
    /// Reads the response frame; held by the importing instance.
    ResponseRead,
}

impl fmt::Display for ChannelEnd {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::RequestWrite => "request-write",
            Self::RequestRead => "request-read",
            Self::ResponseWrite => "response-write",
            Self::ResponseRead => "response-read",
        };
        formatter.write_str(text)
    }
}

/// One executable admitted through generic executable installation.
#[derive(Debug, Clone)]
pub struct AdmittedArtifact {
    /// Exact artifact identity from executable admission.
    pub artifact: Identity,
    /// The component subject admission verified the artifact implements;
    /// it must equal the roster instance's component subject.
    pub component_subject: Identity,
}

/// What the supervisor hands the installer, independent of any candidate
/// plan: the currently authorized request commitment, the installation
/// occurrence, and the admitted executables in canonical instance order.
/// This is copyable owner intent, not activation authority. Only the
/// supervisor's lifecycle can issue an [`InstallationAuthorization`].
#[derive(Debug, Clone)]
pub struct InstallationRequest {
    /// Commitment of the request the owner currently authorizes. A checked
    /// plan answering any other request — including a correctly formed but
    /// superseded one — rejects.
    pub expected_request: Identity,
    /// Installation occurrence: distinguishes generations of one plan so
    /// two installations never share runtime authority.
    pub occurrence: u64,
    /// Admitted executables in canonical instance order.
    pub artifacts: Vec<AdmittedArtifact>,
}

/// Supervisor-owned issuance state for one installation lifecycle. Keep this
/// same value for the lifetime of the supervised installation, including
/// retirement and failed cleanup. Creating another value creates a different
/// authority domain, whose tokens cannot activate through this lifecycle.
/// This state is local to the provider; it is not a global registry or durable
/// recovery protocol.
#[derive(Debug, Default)]
pub struct InstallationLifecycle {
    identity: Arc<()>,
    last_issued: Option<u64>,
}

impl InstallationLifecycle {
    /// Issue current owner intent independently of a candidate plan. Occurrences
    /// must increase strictly, including after failed preparation or disarm.
    /// Issuing fresh intent supersedes any older token not yet activated.
    pub fn authorize(
        &mut self,
        request: InstallationRequest,
    ) -> Result<InstallationAuthorization, InstallationRejection> {
        if let Some(last_issued) = self.last_issued
            && request.occurrence <= last_issued
        {
            return Err(InstallationRejection::ReplayedOccurrence {
                last_issued,
                requested: request.occurrence,
            });
        }
        self.last_issued = Some(request.occurrence);
        Ok(InstallationAuthorization {
            owner: Arc::clone(&self.identity),
            request,
        })
    }

    fn check_authorization(
        &self,
        authorization: &InstallationAuthorization,
    ) -> Result<(), InstallationRejection> {
        if !Arc::ptr_eq(&self.identity, &authorization.owner) {
            return Err(InstallationRejection::ForeignLifecycle);
        }
        if self.last_issued != Some(authorization.request.occurrence) {
            return Err(InstallationRejection::SupersededAuthorization);
        }
        Ok(())
    }
}

/// Opaque, non-clonable authority for one activation, issued only by the
/// supervisor's [`InstallationLifecycle`]. Copying request fields cannot
/// reconstruct this token. Preparation and activation consume its custody.
///
/// ```compile_fail
/// use topology_plan::topology_installation::InstallationAuthorization;
/// fn duplicate(authorization: InstallationAuthorization) {
///     let replay = authorization.clone();
/// }
/// ```
///
/// ```compile_fail
/// use topology_plan::topology_installation::{InstallationAuthorization, InstallationRequest};
/// use std::sync::Arc;
/// fn forge(request: InstallationRequest) -> InstallationAuthorization {
///     InstallationAuthorization { owner: Arc::new(()), request }
/// }
/// ```
#[derive(Debug)]
pub struct InstallationAuthorization {
    owner: Arc<()>,
    request: InstallationRequest,
}

/// Why an installation is rejected before activation. These are admission
/// failures, not policy verdicts.
#[derive(Debug)]
pub enum InstallationRejection {
    /// This lifecycle already issued the occurrence or a newer one.
    ReplayedOccurrence { last_issued: u64, requested: u64 },
    /// The token belongs to a different supervisor lifecycle.
    ForeignLifecycle,
    /// The owner supplied newer intent after this token was issued.
    SupersededAuthorization,
    /// The checked plan answers a request other than the currently
    /// authorized one — a stale or foreign authorization.
    UnauthorizedRequest { expected: Identity, found: Identity },
    /// The artifact roster does not match the instance roster.
    ArtifactCount { expected: usize, found: usize },
    /// An artifact's verified component subject differs from the instance's.
    ArtifactMismatch { instance: u32 },
    /// The reserved zero identity appeared where an exact one is required.
    NullIdentity { field: &'static str },
    /// The adapter reported one physical token for two distinct ends — a
    /// substituted mapping at creation is still a substitution.
    EndpointTokenCollision { binding: u32 },
    /// A bound endpoint's contract has no registered operation schema —
    /// the selected codec cannot check its frames, so the binding never
    /// becomes a live channel.
    MissingOperationSchema {
        endpoint: EndpointKey,
        contract: Identity,
    },
}

impl fmt::Display for InstallationRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReplayedOccurrence {
                last_issued,
                requested,
            } => write!(
                formatter,
                "installation occurrence {requested} does not follow issued occurrence {last_issued}"
            ),
            Self::ForeignLifecycle => {
                formatter.write_str("authorization belongs to another installation lifecycle")
            }
            Self::SupersededAuthorization => {
                formatter.write_str("authorization was superseded by newer owner intent")
            }
            Self::UnauthorizedRequest { .. } => formatter.write_str(
                "the plan answers a request that is not the current installation authorization",
            ),
            Self::ArtifactCount { expected, found } => write!(
                formatter,
                "authorization admits {found} artifacts, the roster requires {expected}"
            ),
            Self::ArtifactMismatch { instance } => write!(
                formatter,
                "artifact for instance {instance} implements a different component subject"
            ),
            Self::NullIdentity { field } => {
                write!(formatter, "{field} uses the reserved zero identity")
            }
            Self::EndpointTokenCollision { binding } => write!(
                formatter,
                "binding {binding} produced two ends with one physical token"
            ),
            Self::MissingOperationSchema { endpoint, .. } => write!(
                formatter,
                "endpoint ({},{})'s contract has no registered operation schema",
                endpoint.instance, endpoint.slot
            ),
        }
    }
}

impl std::error::Error for InstallationRejection {}

/// Why preparation did not produce a pending installation. Every outcome
/// carries its custody accounting: `leaked` names the ends whose close
/// could not be confirmed, and is empty only when teardown was clean.
pub enum PrepareError<A: PipeAdapter> {
    /// Rejected by the installer's own checks.
    Rejected {
        rejection: InstallationRejection,
        leaked: Vec<EndpointId>,
    },
    /// The adapter failed while creating the pair for `binding`.
    Adapter {
        binding: u32,
        error: A::Error,
        leaked: Vec<EndpointId>,
    },
}

impl<A: PipeAdapter> fmt::Debug for PrepareError<A> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl<A: PipeAdapter> fmt::Display for PrepareError<A> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected { rejection, leaked } => {
                write!(formatter, "{rejection}")?;
                if !leaked.is_empty() {
                    write!(formatter, " ({} endpoints leaked)", leaked.len())?;
                }
                Ok(())
            }
            Self::Adapter {
                binding,
                error,
                leaked,
            } => {
                write!(formatter, "adapter failed at binding {binding}: {error}")?;
                if !leaked.is_empty() {
                    write!(formatter, " ({} endpoints leaked)", leaked.len())?;
                }
                Ok(())
            }
        }
    }
}

impl<A: PipeAdapter> std::error::Error for PrepareError<A> {}

/// One physical end and its recorded placement.
pub struct EndpointAssignment<Endpoint> {
    /// Binding index in the checked plan.
    pub binding: u32,
    /// The plan endpoint this physical end serves.
    pub endpoint: EndpointKey,
    pub role: ChannelEnd,
    /// Installer-issued identity retained in the route table.
    pub id: EndpointId,
    /// The physical token the adapter reported for `handle`.
    pub token: u64,
    /// The physical handle itself.
    pub handle: Endpoint,
}

/// The route record the mediator consults: which binding, plan endpoint,
/// channel role, and instance each issued end belongs to, the contract
/// whose operation schema checks its inbound frames, and the physical
/// token a send must attest.
struct RouteRecord {
    id: EndpointId,
    binding: u32,
    endpoint: EndpointKey,
    role: ChannelEnd,
    holder: u32,
    /// The contract identity of the plan endpoint this end serves — the
    /// destination-side schema key for delivered frames.
    contract: Identity,
    token: u64,
}

/// A checked plan whose endpoints are allocated and assigned but not yet
/// activated. It borrows no Build authority and grants nothing: custody of
/// every end remains here until [`activate`](Self::activate) hands each to
/// its member. [`disarm`](Self::disarm) returns custody explicitly with
/// per-end accounting; dropping without activating releases the handles
/// themselves (a pipe end closes on drop) but reports nothing, so callers
/// needing custody evidence use `disarm`.
pub struct PreparedInstallation<A: PipeAdapter> {
    checked: CheckedPlan,
    authorization: InstallationAuthorization,
    /// The per-contract operation schemas this installation enforces.
    schemas: OperationSchemas,
    /// Per-instance assigned ends, canonical instance order.
    assigned: Vec<Vec<EndpointAssignment<A::Endpoint>>>,
    /// Every issued end, in `EndpointId` order.
    routes: Vec<RouteRecord>,
    adapter: A,
}

impl<A: PipeAdapter> fmt::Debug for PreparedInstallation<A> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedInstallation")
            .field("occurrence", &self.authorization.request.occurrence)
            .field("routes", &self.routes.len())
            .finish()
    }
}

/// Close every end still in installer custody; returns the ids whose close
/// could not be confirmed — the leaked custody a failure must report.
fn drain_assigned<A, E>(
    adapter: &mut A,
    assigned: &mut [Vec<EndpointAssignment<E>>],
) -> Vec<EndpointId>
where
    A: PipeAdapter<Endpoint = E>,
{
    let mut leaked = Vec::new();
    for endpoints in assigned.iter_mut() {
        for assignment in endpoints.drain(..) {
            if adapter.close_endpoint(assignment.handle).is_err() {
                leaked.push(assignment.id);
            }
        }
    }
    leaked
}

/// What preparation reports back: the prepared member plus the physical
/// token of every endpoint actually installed, keyed by installer id. The
/// activation gate compares this echo to the recorded assignment — a
/// substituted pipe cannot satisfy it by naming the right `EndpointId`.
pub struct PreparationEcho<Member> {
    pub member: Member,
    /// `(endpoint id, physical token)` per installed end, any order.
    pub installed: Vec<(EndpointId, u64)>,
}

/// A member that refused release: custody stays with the caller, and the
/// failure reports exactly which members could not be quiesced. A retained
/// member is never counted as cleaned.
#[derive(Debug)]
pub struct RetainedMember<Member, Error> {
    pub member: Member,
    pub error: Error,
}

/// Generic executable installation and process custody for the roster.
/// The supervisor admits executables, installs endpoints, holds application
/// entry behind the installation gate, and quiesces members on demand. Its
/// `Endpoint` type must equal the selected adapter's — the installer moves
/// the adapter's ends directly into member custody.
pub trait ProcessSupervisor {
    /// The physical endpoint type this supervisor installs.
    type Endpoint;
    /// One prepared or entered member under supervision.
    type Member;
    type Error: fmt::Debug + fmt::Display;

    /// The stable lifecycle that issues this supervisor's owner-authorized
    /// installation tokens. Expose only a shared view at the installation
    /// boundary: callers cannot replace/reset the lifecycle through this API.
    /// The provider's owner-authorized path alone obtains mutable issuance
    /// access. Failed attempts require fresh intent and a new occurrence.
    ///
    /// ```compile_fail
    /// use topology_plan::topology_installation::{InstallationLifecycle, ProcessSupervisor};
    /// fn reset<S: ProcessSupervisor>(supervisor: &mut S) {
    ///     *supervisor.installation_lifecycle() = InstallationLifecycle::default();
    /// }
    /// ```
    fn installation_lifecycle(&self) -> &InstallationLifecycle;

    /// Admit `artifact` for `instance` and install exactly `endpoints` — no
    /// inherited handles, no substitutes. The member is prepared but must
    /// not run application code until the gate opens. The echo reports the
    /// physical token of every endpoint actually installed; on failure the
    /// supervisor must have released every end it received.
    fn prepare_member(
        &mut self,
        instance: &PlanInstance,
        artifact: &AdmittedArtifact,
        endpoints: Vec<EndpointAssignment<Self::Endpoint>>,
    ) -> Result<PreparationEcho<Self::Member>, Self::Error>;

    /// Permit application entry for one prepared member. Reachable only
    /// through the installation gate after the complete roster is admitted.
    fn open_entry_gate(&mut self, member: &mut Self::Member) -> Result<(), Self::Error>;

    /// Quiesce one member — stop its entries and release its endpoint
    /// custody. `Ok` confirms full release; on failure the member is
    /// returned inside [`RetainedMember`] so supervision is never dropped
    /// silently.
    fn quiesce_member(
        &mut self,
        member: Self::Member,
    ) -> Result<(), RetainedMember<Self::Member, Self::Error>>;
}

/// What failed at the gate.
#[derive(Debug)]
pub enum ActivationCause<Error> {
    /// The gate refused foreign or superseded lifecycle authority before
    /// preparing any member.
    Authorization(InstallationRejection),
    /// The supervisor reported this failure from preparation or an entry
    /// gate.
    Supervisor(Error),
    /// The installed-token echo did not equal the recorded assignment: a
    /// substituted mapping refuses before any entry opens.
    EndpointSubstitution { detail: String },
}

/// A member failure during activation. Every prepared or entered member is
/// quiesced in reverse activation order; `retained` names those whose
/// quiesce failed (supervision is retained, not lost) and `leaked` names
/// installer-custody ends whose close failed. No receipt is ever produced
/// for a partial roster.
pub struct ActivationFailure<S: ProcessSupervisor> {
    /// Instance whose preparation, echo check, or entry gate failed; zero
    /// when `cause` is a roster-wide authorization refusal.
    pub instance: u32,
    pub cause: ActivationCause<S::Error>,
    /// Members that could not be released — still under supervision.
    pub retained: Vec<RetainedMember<S::Member, S::Error>>,
    /// Installer-custody ends whose close failed.
    pub leaked: Vec<EndpointId>,
}

impl<S: ProcessSupervisor> fmt::Debug for ActivationFailure<S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActivationFailure")
            .field("instance", &self.instance)
            .field("cause", &self.cause)
            .field("retained", &self.retained.len())
            .field("leaked", &self.leaked)
            .finish()
    }
}

impl<S: ProcessSupervisor> fmt::Display for ActivationFailure<S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.cause {
            ActivationCause::Authorization(rejection) => write!(formatter, "{rejection}"),
            ActivationCause::Supervisor(error) => write!(
                formatter,
                "member {} failed under the supervisor: {error}",
                self.instance
            ),
            ActivationCause::EndpointSubstitution { detail } => write!(
                formatter,
                "member {} installed a substituted endpoint mapping: {detail}",
                self.instance
            ),
        }?;
        if !self.retained.is_empty() {
            write!(
                formatter,
                " ({} members retained under supervision)",
                self.retained.len()
            )?;
        }
        if !self.leaked.is_empty() {
            write!(formatter, " ({} endpoints leaked)", self.leaked.len())?;
        }
        Ok(())
    }
}

impl<S: ProcessSupervisor> std::error::Error for ActivationFailure<S> {}

/// The physical endpoint mapping of one established binding, recorded in
/// the receipt.
#[derive(Debug, Clone)]
pub struct BindingMapping {
    pub import: EndpointKey,
    pub export: EndpointKey,
    /// `(endpoint id, physical token)` in fixed channel order:
    /// request-write, request-read, response-write, response-read.
    pub endpoints: [(EndpointId, u64); 4],
}

/// `installation admitted`: the receipt binds the plan, the admitted
/// executable identities, the physical endpoint mapping, the provider and
/// its disclosed assumptions, and the installation occurrence. It is
/// evidence for this generation only — never a reusable activation token.
#[derive(Debug, Clone)]
pub struct InstallationReceipt {
    /// Exact plan subject this installation serves.
    pub plan: Identity,
    /// Installation occurrence; two generations of one plan never share
    /// runtime authority.
    pub occurrence: u64,
    /// Admitted executable per instance, canonical order.
    pub artifacts: Vec<Identity>,
    /// Physical mapping per binding, canonical binding order.
    pub bindings: Vec<BindingMapping>,
    /// Selected provider identity.
    pub provider: Identity,
    /// The provider's disclosed OS/loader assumptions.
    pub provider_assumptions: Vec<&'static str>,
}

/// A live installation generation: the receipt, the retained route table,
/// and the member roster under supervision. Mediation checks run against
/// the route table; quiescing consumes the installation.
pub struct InstalledTopology<A: PipeAdapter, Member> {
    receipt: InstallationReceipt,
    routes: Vec<RouteRecord>,
    /// Per-contract operation schemas frames are checked against before
    /// delivery.
    schemas: OperationSchemas,
    /// Bindings with one outstanding request.
    in_flight: BTreeSet<u32>,
    /// Bindings closed by protocol or peer failure.
    closed_bindings: BTreeSet<u32>,
    /// Members in canonical instance order.
    members: Vec<Member>,
    adapter: A,
}

impl<A: PipeAdapter, Member> fmt::Debug for InstalledTopology<A, Member> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InstalledTopology")
            .field("occurrence", &self.receipt.occurrence)
            .field("members", &self.members.len())
            .field("in_flight", &self.in_flight.len())
            .finish()
    }
}

/// Why a replacement did not produce a new generation.
pub enum ReplacementFailure<A: PipeAdapter, S: ProcessSupervisor<Endpoint = A::Endpoint>> {
    /// The replacement no longer has current owner authority. No member of
    /// the old generation was stopped; both old and pending custody return.
    Authorization {
        rejection: InstallationRejection,
        old: InstalledTopology<A, S::Member>,
        pending_new: PreparedInstallation<A>,
    },
    /// The old generation could not fully release: the new generation was
    /// never activated and is returned still pending so the caller can
    /// `disarm` it. The old installation remains the authority.
    OldGeneration {
        retained: Vec<RetainedMember<S::Member, S::Error>>,
        pending_new: PreparedInstallation<A>,
    },
    /// The old generation fully stopped; the new activation failed.
    /// Downtime is possible, but the old generation cannot silently restart
    /// under different authority — it is gone.
    Activation(ActivationFailure<S>),
}

impl<A, S> fmt::Debug for ReplacementFailure<A, S>
where
    A: PipeAdapter,
    S: ProcessSupervisor<Endpoint = A::Endpoint>,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Authorization { rejection, .. } => formatter
                .debug_tuple("Authorization")
                .field(rejection)
                .finish(),
            Self::OldGeneration { retained, .. } => formatter
                .debug_struct("OldGeneration")
                .field("retained", &retained.len())
                .finish(),
            Self::Activation(failure) => {
                formatter.debug_tuple("Activation").field(failure).finish()
            }
        }
    }
}

/// Convenience: instance names for diagnostics in canonical order.
pub fn roster_names<A: PipeAdapter>(prepared: &PreparedInstallation<A>) -> Vec<&InstanceName> {
    prepared
        .checked
        .graph
        .instances()
        .iter()
        .map(|instance| &instance.name)
        .collect()
}
