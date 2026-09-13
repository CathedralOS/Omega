//! Versioned canonical wire tables for `TopologyRequest` and `DeploymentPlan`.
//!
//! The package owns these wire numbers — there is no compiler schema
//! registry. Encoding is byte-exact: every multi-item section is emitted in
//! the model's canonical order, integers are little-endian, and all
//! collections are `u32` length-prefixed. The decoder enforces the same
//! canonical order on the wire (one logical value has exactly one byte
//! identity), rejects duplicate or unknown entries, truncation, trailing
//! bytes, reserved completeness tags, and every undeclared tag value.
//!
//! Tag tables (v1):
//!
//! ```text
//! magic       request 0x4C505451 "OTQL"   plan 0x4C505450 "OTPL"
//! version     u32 = 1 (other versions reject)
//! direction   0 = Import, 1 = Export
//! role        0 = Component, 1 = ExternalParticipant
//! completeness 1 = VerifiedComplete { closure: Identity }
//!             0 = ProducerDeclared and 2 = EarlyFrontier are named
//!             rejections: only verifier-established closure facts satisfy
//!             the role; any other tag rejects as unknown.
//! predicate   1 = NoRoute, 2 = OnlyVia
//! outcome     1 = Satisfied { certificate }, 2 = Violated { violation }
//! violation   1 = Bypass { path }, 2 = Disconnected
//! ```
//!
//! Certificate layout per predicate:
//! `NoRoute`   → reachable: count u32 + sorted u32 indices
//! `OnlyVia`   → path: count u32 + ordered u32 indices,
//!               reachable: count u32 + sorted u32 indices
//!
//! Bounded: every count is checked against the limits below before any
//! allocation follows it, and total input size is capped. Exhausted limits
//! are an unsuccessful verification, never a satisfied verdict.

use crate::model::{
    Binding, Certificate, Completeness, ComponentDescription, DeploymentPlan, Endpoint,
    EndpointDirection, EndpointKey, ExecutedPolicy, Identity, InstanceName, InstanceRole,
    PlanInstance, PolicyCall, PolicyOutcome, PolicyPredicate, PolicySelector, RequestedInstance,
    TopologyRequest, Violation,
};
use std::fmt;

pub const REQUEST_MAGIC: u32 = 0x4C50_5451;
pub const PLAN_MAGIC: u32 = 0x4C50_5450;
pub const REQUEST_SCHEMA_VERSION: u32 = 1;
pub const PLAN_SCHEMA_VERSION: u32 = 1;

pub const MAX_PLAN_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_NAME_BYTES: usize = 256;
pub const MAX_INSTANCES: usize = 1024;
pub const MAX_ENDPOINTS_PER_INSTANCE: usize = 4096;
pub const MAX_BINDINGS: usize = 65536;
pub const MAX_POLICIES: usize = 1024;
pub const MAX_SELECTOR_MEMBERS: usize = 1024;
pub const MAX_ASSUMPTIONS: usize = 1024;
pub const MAX_TRANSPORTS: usize = 1024;

const TAG_IMPORT: u8 = 0;
const TAG_EXPORT: u8 = 1;
const TAG_ROLE_COMPONENT: u8 = 0;
const TAG_ROLE_EXTERNAL: u8 = 1;
const TAG_COMPLETENESS_VERIFIED: u8 = 1;
const TAG_COMPLETENESS_PRODUCER_DECLARED: u8 = 0;
const TAG_COMPLETENESS_EARLY_FRONTIER: u8 = 2;
const TAG_NO_ROUTE: u8 = 1;
const TAG_ONLY_VIA: u8 = 2;
const TAG_SATISFIED: u8 = 1;
const TAG_VIOLATED: u8 = 2;
const TAG_BYPASS: u8 = 1;
const TAG_DISCONNECTED: u8 = 2;

/// Decode-side failures. Each names the malformed or dishonest record; the
/// verifier maps them to `Malformed`/`UnverifiedCompleteness` categories.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    WrongMagic {
        expected: u32,
        found: u32,
    },
    UnsupportedVersion {
        version: u32,
    },
    UnexpectedEnd {
        context: &'static str,
    },
    TrailingBytes {
        count: usize,
    },
    /// A section's entries are not in canonical increasing order, or repeat
    /// a key. Wire canonicalization is an identity rule, not a convenience.
    NotCanonical {
        section: &'static str,
    },
    Duplicate {
        section: &'static str,
    },
    UnknownTag {
        field: &'static str,
        tag: u32,
    },
    /// A completeness tag the contract names but never admits: a
    /// producer-declared flag or an early product frontier is not verified
    /// closure evidence.
    UnverifiedCompleteness {
        tag: u8,
    },
    /// An instance-name byte string is not valid UTF-8.
    InvalidName,
    LimitExceeded {
        what: &'static str,
    },
    /// `via` encoded non-empty for `no_route` or empty for `only_via`.
    ViaArity,
}

impl fmt::Display for CodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongMagic { expected, found } => write!(
                formatter,
                "wrong artifact magic: expected {expected:#010x}, found {found:#010x}"
            ),
            Self::UnsupportedVersion { version } => {
                write!(formatter, "unsupported schema version {version}")
            }
            Self::UnexpectedEnd { context } => {
                write!(formatter, "truncated record while reading {context}")
            }
            Self::TrailingBytes { count } => {
                write!(formatter, "{count} trailing bytes after the record")
            }
            Self::NotCanonical { section } => {
                write!(formatter, "{section} entries are not in canonical order")
            }
            Self::Duplicate { section } => {
                write!(formatter, "{section} contains a duplicate entry")
            }
            Self::UnknownTag { field, tag } => {
                write!(formatter, "unknown tag {tag} in {field}")
            }
            Self::UnverifiedCompleteness { tag } => write!(
                formatter,
                "completeness tag {tag} is not verifier-established closure evidence"
            ),
            Self::InvalidName => formatter.write_str("instance name is not valid UTF-8"),
            Self::LimitExceeded { what } => write!(formatter, "bounded limit exceeded: {what}"),
            Self::ViaArity => {
                formatter.write_str("via must be empty for no_route and non-empty for only_via")
            }
        }
    }
}

impl std::error::Error for CodecError {}

#[derive(Default)]
struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn finish(self) -> Vec<u8> {
        self.bytes
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    fn identity(&mut self, identity: &Identity) {
        self.bytes(identity);
    }

    fn count(&mut self, label: &'static str, len: usize) -> Result<(), CodecError> {
        self.u32(u32::try_from(len).map_err(|_| CodecError::LimitExceeded { what: label })?);
        Ok(())
    }

    fn name(&mut self, name: &InstanceName) -> Result<(), CodecError> {
        self.count("instance name bytes", name.as_str().len())?;
        self.bytes(name.as_str().as_bytes());
        Ok(())
    }
}

struct Reader<'bytes> {
    bytes: &'bytes [u8],
    offset: usize,
}

impl<'bytes> Reader<'bytes> {
    const fn new(bytes: &'bytes [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }

    fn take(&mut self, context: &'static str, len: usize) -> Result<&'bytes [u8], CodecError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(CodecError::UnexpectedEnd { context })?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(CodecError::UnexpectedEnd { context })?;
        self.offset = end;
        Ok(bytes)
    }

    fn u8(&mut self, context: &'static str) -> Result<u8, CodecError> {
        Ok(self.take(context, 1)?[0])
    }

    fn u32(&mut self, context: &'static str) -> Result<u32, CodecError> {
        Ok(u32::from_le_bytes(
            self.take(context, 4)?.try_into().unwrap(),
        ))
    }

    fn identity(&mut self, context: &'static str) -> Result<Identity, CodecError> {
        Ok(self.take(context, 32)?.try_into().unwrap())
    }

    fn count(
        &mut self,
        context: &'static str,
        what: &'static str,
        limit: usize,
    ) -> Result<usize, CodecError> {
        let count = self.u32(context)? as usize;
        if count > limit {
            return Err(CodecError::LimitExceeded { what });
        }
        Ok(count)
    }

    fn name(&mut self, context: &'static str) -> Result<InstanceName, CodecError> {
        let len = self.count(context, "instance name bytes", MAX_NAME_BYTES)?;
        let bytes = self.take(context, len)?;
        let text = std::str::from_utf8(bytes).map_err(|_| CodecError::InvalidName)?;
        InstanceName::new(text).map_err(|_| CodecError::InvalidName)
    }

    fn selector(&mut self, context: &'static str) -> Result<PolicySelector, CodecError> {
        let count = self.count(context, "selector members", MAX_SELECTOR_MEMBERS)?;
        let mut members = Vec::with_capacity(count);
        let mut previous: Option<InstanceName> = None;
        for _ in 0..count {
            let member = self.name(context)?;
            if let Some(previous) = &previous {
                match previous.cmp(&member) {
                    std::cmp::Ordering::Less => {}
                    std::cmp::Ordering::Equal => {
                        return Err(CodecError::Duplicate {
                            section: "policy selector",
                        });
                    }
                    std::cmp::Ordering::Greater => {
                        return Err(CodecError::NotCanonical {
                            section: "policy selector",
                        });
                    }
                }
            }
            previous = Some(member.clone());
            members.push(member);
        }
        Ok(PolicySelector { members })
    }
}

fn write_description(
    writer: &mut Writer,
    description: &ComponentDescription,
) -> Result<(), CodecError> {
    writer.identity(&description.subject);
    writer.identity(&description.verification_profile);
    match &description.completeness {
        Completeness::VerifiedComplete { closure } => {
            writer.u8(TAG_COMPLETENESS_VERIFIED);
            writer.identity(closure);
        }
    }
    writer.count("assumptions", description.assumptions.len())?;
    for assumption in &description.assumptions {
        writer.identity(assumption);
    }
    Ok(())
}

fn read_description(reader: &mut Reader<'_>) -> Result<ComponentDescription, CodecError> {
    let subject = reader.identity("component subject")?;
    let verification_profile = reader.identity("verification profile")?;
    let completeness = match reader.u8("completeness")? {
        TAG_COMPLETENESS_VERIFIED => Completeness::VerifiedComplete {
            closure: reader.identity("completeness closure")?,
        },
        tag @ (TAG_COMPLETENESS_PRODUCER_DECLARED | TAG_COMPLETENESS_EARLY_FRONTIER) => {
            return Err(CodecError::UnverifiedCompleteness { tag });
        }
        tag => {
            return Err(CodecError::UnknownTag {
                field: "completeness",
                tag: tag as u32,
            });
        }
    };
    let assumption_count = reader.count("assumptions", "assumptions", MAX_ASSUMPTIONS)?;
    let mut assumptions = Vec::with_capacity(assumption_count);
    for _ in 0..assumption_count {
        assumptions.push(reader.identity("assumption")?);
    }
    if !assumptions.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err(
            if assumptions.len() > 1 && assumptions[0] == assumptions[1] {
                CodecError::Duplicate {
                    section: "assumptions",
                }
            } else {
                CodecError::NotCanonical {
                    section: "assumptions",
                }
            },
        );
    }
    Ok(ComponentDescription {
        subject,
        verification_profile,
        completeness,
        assumptions,
    })
}

fn write_endpoint(writer: &mut Writer, endpoint: &Endpoint) {
    writer.u32(endpoint.slot);
    writer.u8(match endpoint.direction {
        EndpointDirection::Import => TAG_IMPORT,
        EndpointDirection::Export => TAG_EXPORT,
    });
    writer.identity(&endpoint.contract);
}

fn read_endpoint(reader: &mut Reader<'_>) -> Result<Endpoint, CodecError> {
    let slot = reader.u32("endpoint slot")?;
    let direction = match reader.u8("endpoint direction")? {
        TAG_IMPORT => EndpointDirection::Import,
        TAG_EXPORT => EndpointDirection::Export,
        tag => {
            return Err(CodecError::UnknownTag {
                field: "endpoint direction",
                tag: tag as u32,
            });
        }
    };
    let contract = reader.identity("endpoint contract")?;
    Ok(Endpoint {
        slot,
        direction,
        contract,
    })
}

fn write_instance(writer: &mut Writer, instance: &PlanInstance) -> Result<(), CodecError> {
    writer.name(&instance.name)?;
    writer.u8(match instance.role {
        InstanceRole::Component => TAG_ROLE_COMPONENT,
        InstanceRole::ExternalParticipant => TAG_ROLE_EXTERNAL,
    });
    write_description(writer, &instance.component)?;
    writer.count("endpoints", instance.endpoints.len())?;
    for endpoint in &instance.endpoints {
        write_endpoint(writer, endpoint);
    }
    Ok(())
}

fn read_instance(reader: &mut Reader<'_>) -> Result<PlanInstance, CodecError> {
    let name = reader.name("instance name")?;
    let role = match reader.u8("instance role")? {
        TAG_ROLE_COMPONENT => InstanceRole::Component,
        TAG_ROLE_EXTERNAL => InstanceRole::ExternalParticipant,
        tag => {
            return Err(CodecError::UnknownTag {
                field: "instance role",
                tag: tag as u32,
            });
        }
    };
    let component = read_description(reader)?;
    let endpoint_count = reader.count("endpoints", "endpoints", MAX_ENDPOINTS_PER_INSTANCE)?;
    let mut endpoints = Vec::with_capacity(endpoint_count);
    let mut previous: Option<(u32, EndpointDirection)> = None;
    for _ in 0..endpoint_count {
        let endpoint = read_endpoint(reader)?;
        let key = (endpoint.slot, endpoint.direction);
        if let Some(previous) = previous {
            match previous.cmp(&key) {
                std::cmp::Ordering::Less => {}
                std::cmp::Ordering::Equal => {
                    return Err(CodecError::Duplicate {
                        section: "endpoints",
                    });
                }
                std::cmp::Ordering::Greater => {
                    return Err(CodecError::NotCanonical {
                        section: "endpoints",
                    });
                }
            }
        }
        previous = Some(key);
        endpoints.push(endpoint);
    }
    Ok(PlanInstance {
        name,
        component,
        role,
        endpoints,
    })
}

fn write_selector(writer: &mut Writer, selector: &PolicySelector) -> Result<(), CodecError> {
    writer.count("selector members", selector.members.len())?;
    for member in &selector.members {
        writer.name(member)?;
    }
    Ok(())
}

fn write_call(writer: &mut Writer, call: &PolicyCall) -> Result<(), CodecError> {
    writer.u8(match call.predicate {
        PolicyPredicate::NoRoute => TAG_NO_ROUTE,
        PolicyPredicate::OnlyVia => TAG_ONLY_VIA,
    });
    write_selector(writer, &call.sources)?;
    write_selector(writer, &call.targets)?;
    write_selector(writer, &call.via)?;
    Ok(())
}

fn read_call(reader: &mut Reader<'_>) -> Result<PolicyCall, CodecError> {
    let predicate = match reader.u8("policy predicate")? {
        TAG_NO_ROUTE => PolicyPredicate::NoRoute,
        TAG_ONLY_VIA => PolicyPredicate::OnlyVia,
        tag => {
            return Err(CodecError::UnknownTag {
                field: "policy predicate",
                tag: tag as u32,
            });
        }
    };
    let sources = reader.selector("policy sources")?;
    let targets = reader.selector("policy targets")?;
    let via = reader.selector("policy via")?;
    match (predicate, via.members.is_empty()) {
        (PolicyPredicate::NoRoute, true) | (PolicyPredicate::OnlyVia, false) => {}
        _ => return Err(CodecError::ViaArity),
    }
    Ok(PolicyCall {
        predicate,
        sources,
        targets,
        via,
    })
}

fn write_indices(writer: &mut Writer, indices: &[u32]) -> Result<(), CodecError> {
    writer.count("indices", indices.len())?;
    for &index in indices {
        writer.u32(index);
    }
    Ok(())
}

fn read_indices(
    reader: &mut Reader<'_>,
    context: &'static str,
    limit: usize,
) -> Result<Vec<u32>, CodecError> {
    let count = reader.count(context, "indices", limit)?;
    let mut indices = Vec::with_capacity(count);
    for _ in 0..count {
        indices.push(reader.u32(context)?);
    }
    Ok(indices)
}

fn write_outcome(writer: &mut Writer, outcome: &PolicyOutcome) -> Result<(), CodecError> {
    match outcome {
        PolicyOutcome::Satisfied { certificate } => {
            writer.u8(TAG_SATISFIED);
            match certificate {
                Certificate::NoRoute { reachable } => {
                    write_indices(writer, reachable)?;
                }
                Certificate::OnlyVia { path, reachable } => {
                    write_indices(writer, path)?;
                    write_indices(writer, reachable)?;
                }
            }
        }
        PolicyOutcome::Violated { violation } => {
            writer.u8(TAG_VIOLATED);
            match violation {
                Violation::Bypass { path } => {
                    writer.u8(TAG_BYPASS);
                    write_indices(writer, path)?;
                }
                Violation::Disconnected => {
                    writer.u8(TAG_DISCONNECTED);
                }
            }
        }
    }
    Ok(())
}

fn read_outcome(
    reader: &mut Reader<'_>,
    predicate: PolicyPredicate,
) -> Result<PolicyOutcome, CodecError> {
    match reader.u8("policy outcome")? {
        TAG_SATISFIED => {
            let certificate = match predicate {
                PolicyPredicate::NoRoute => Certificate::NoRoute {
                    reachable: read_indices(reader, "no_route reachable", MAX_INSTANCES)?,
                },
                PolicyPredicate::OnlyVia => Certificate::OnlyVia {
                    path: read_indices(reader, "only_via path", MAX_INSTANCES)?,
                    reachable: read_indices(reader, "only_via reachable", MAX_INSTANCES)?,
                },
            };
            Ok(PolicyOutcome::Satisfied { certificate })
        }
        TAG_VIOLATED => {
            let violation = match reader.u8("violation kind")? {
                TAG_BYPASS => Violation::Bypass {
                    path: read_indices(reader, "bypass path", MAX_INSTANCES)?,
                },
                TAG_DISCONNECTED => Violation::Disconnected,
                tag => {
                    return Err(CodecError::UnknownTag {
                        field: "violation kind",
                        tag: tag as u32,
                    });
                }
            };
            Ok(PolicyOutcome::Violated { violation })
        }
        tag => Err(CodecError::UnknownTag {
            field: "policy outcome",
            tag: tag as u32,
        }),
    }
}

fn check_canonical_order<K: Ord>(
    keys: impl Iterator<Item = K>,
    section: &'static str,
) -> Result<(), CodecError> {
    let mut previous: Option<K> = None;
    for key in keys {
        if let Some(previous) = previous.take() {
            match previous.cmp(&key) {
                std::cmp::Ordering::Less => {}
                std::cmp::Ordering::Equal => {
                    return Err(CodecError::Duplicate { section });
                }
                std::cmp::Ordering::Greater => {
                    return Err(CodecError::NotCanonical { section });
                }
            }
        }
        previous = Some(key);
    }
    Ok(())
}

/// Canonically encode an owner request. The commitment consumers compare is
/// `request_commitment` over these bytes.
pub fn encode_request(request: &TopologyRequest) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    writer.u32(REQUEST_MAGIC);
    writer.u32(REQUEST_SCHEMA_VERSION);
    writer.count("request instances", request.instances.len())?;
    check_canonical_order(
        request.instances.iter().map(|instance| &instance.name),
        "request instances",
    )?;
    for instance in &request.instances {
        writer.name(&instance.name)?;
        writer.identity(&instance.subject);
    }
    writer.count("request policies", request.policies.len())?;
    check_canonical_order(
        request.policies.iter().map(|call| call.canonical_key()),
        "request policies",
    )?;
    for call in &request.policies {
        write_call(&mut writer, call)?;
    }
    writer.identity(&request.verifier);
    writer.count("transports", request.transports.len())?;
    check_canonical_order(request.transports.iter(), "request transports")?;
    for transport in &request.transports {
        writer.identity(transport);
    }
    writer.count("accepted assumptions", request.accepted_assumptions.len())?;
    check_canonical_order(request.accepted_assumptions.iter(), "accepted assumptions")?;
    for assumption in &request.accepted_assumptions {
        writer.identity(assumption);
    }
    Ok(writer.finish())
}

/// Decode a canonical request, enforcing wire order, tags, limits, and exact
/// length.
pub fn decode_request(bytes: &[u8]) -> Result<TopologyRequest, CodecError> {
    if bytes.len() > MAX_PLAN_BYTES {
        return Err(CodecError::LimitExceeded {
            what: "request bytes",
        });
    }
    let mut reader = Reader::new(bytes);
    let magic = reader.u32("request magic")?;
    if magic != REQUEST_MAGIC {
        return Err(CodecError::WrongMagic {
            expected: REQUEST_MAGIC,
            found: magic,
        });
    }
    let version = reader.u32("request version")?;
    if version != REQUEST_SCHEMA_VERSION {
        return Err(CodecError::UnsupportedVersion { version });
    }
    let instance_count = reader.count("request instances", "instances", MAX_INSTANCES)?;
    let mut instances = Vec::with_capacity(instance_count);
    for _ in 0..instance_count {
        let name = reader.name("request instance name")?;
        let subject = reader.identity("request component subject")?;
        instances.push(RequestedInstance { name, subject });
    }
    check_canonical_order(
        instances.iter().map(|instance| &instance.name),
        "request instances",
    )?;
    let policy_count = reader.count("request policies", "policies", MAX_POLICIES)?;
    let mut policies = Vec::with_capacity(policy_count);
    for _ in 0..policy_count {
        policies.push(read_call(&mut reader)?);
    }
    check_canonical_order(
        policies.iter().map(|call| call.canonical_key()),
        "request policies",
    )?;
    let verifier = reader.identity("selected verifier")?;
    let transport_count = reader.count("transports", "transports", MAX_TRANSPORTS)?;
    let mut transports = Vec::with_capacity(transport_count);
    for _ in 0..transport_count {
        transports.push(reader.identity("transport")?);
    }
    check_canonical_order(transports.iter(), "request transports")?;
    let assumption_count = reader.count(
        "accepted assumptions",
        "accepted assumptions",
        MAX_ASSUMPTIONS,
    )?;
    let mut accepted_assumptions = Vec::with_capacity(assumption_count);
    for _ in 0..assumption_count {
        accepted_assumptions.push(reader.identity("accepted assumption")?);
    }
    check_canonical_order(accepted_assumptions.iter(), "accepted assumptions")?;
    if reader.remaining() != 0 {
        return Err(CodecError::TrailingBytes {
            count: reader.remaining(),
        });
    }
    Ok(TopologyRequest {
        instances,
        policies,
        verifier,
        transports,
        accepted_assumptions,
    })
}

/// Canonically encode a deployment plan. `plan_subject` over these bytes is
/// the plan's external identity.
pub fn encode_plan(plan: &DeploymentPlan) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    writer.u32(PLAN_MAGIC);
    writer.u32(PLAN_SCHEMA_VERSION);
    writer.identity(&plan.request_commitment);
    writer.count("plan instances", plan.instances.len())?;
    check_canonical_order(
        plan.instances.iter().map(|instance| &instance.name),
        "plan instances",
    )?;
    for instance in &plan.instances {
        write_instance(&mut writer, instance)?;
    }
    writer.count("plan bindings", plan.bindings.len())?;
    check_canonical_order(
        plan.bindings
            .iter()
            .map(|binding| (binding.import, binding.export)),
        "plan bindings",
    )?;
    for binding in &plan.bindings {
        writer.u32(binding.import.instance);
        writer.u32(binding.import.slot);
        writer.u32(binding.export.instance);
        writer.u32(binding.export.slot);
        writer.identity(&binding.transport);
    }
    writer.count("plan policies", plan.policies.len())?;
    check_canonical_order(
        plan.policies.iter().map(|row| row.call.canonical_key()),
        "plan policies",
    )?;
    for row in &plan.policies {
        writer.identity(&row.verifier);
        write_call(&mut writer, &row.call)?;
        write_outcome(&mut writer, &row.outcome)?;
    }
    Ok(writer.finish())
}

/// Decode a canonical plan, enforcing wire order, tags, limits, and exact
/// length. Decoding produces data only — `verify_plan` decides whether the
/// data is admissible.
pub fn decode_plan(bytes: &[u8]) -> Result<DeploymentPlan, CodecError> {
    if bytes.len() > MAX_PLAN_BYTES {
        return Err(CodecError::LimitExceeded { what: "plan bytes" });
    }
    let mut reader = Reader::new(bytes);
    let magic = reader.u32("plan magic")?;
    if magic != PLAN_MAGIC {
        return Err(CodecError::WrongMagic {
            expected: PLAN_MAGIC,
            found: magic,
        });
    }
    let version = reader.u32("plan version")?;
    if version != PLAN_SCHEMA_VERSION {
        return Err(CodecError::UnsupportedVersion { version });
    }
    let request_commitment = reader.identity("request commitment")?;
    let instance_count = reader.count("plan instances", "instances", MAX_INSTANCES)?;
    let mut instances = Vec::with_capacity(instance_count);
    for _ in 0..instance_count {
        instances.push(read_instance(&mut reader)?);
    }
    check_canonical_order(
        instances.iter().map(|instance| &instance.name),
        "plan instances",
    )?;
    let binding_count = reader.count("plan bindings", "bindings", MAX_BINDINGS)?;
    let mut bindings = Vec::with_capacity(binding_count);
    for _ in 0..binding_count {
        let import = EndpointKey {
            instance: reader.u32("binding import instance")?,
            slot: reader.u32("binding import slot")?,
        };
        let export = EndpointKey {
            instance: reader.u32("binding export instance")?,
            slot: reader.u32("binding export slot")?,
        };
        let transport = reader.identity("binding transport")?;
        bindings.push(Binding {
            import,
            export,
            transport,
        });
    }
    check_canonical_order(
        bindings
            .iter()
            .map(|binding| (binding.import, binding.export)),
        "plan bindings",
    )?;
    let policy_count = reader.count("plan policies", "policies", MAX_POLICIES)?;
    let mut policies = Vec::with_capacity(policy_count);
    for _ in 0..policy_count {
        let verifier = reader.identity("policy verifier")?;
        let call = read_call(&mut reader)?;
        let outcome = read_outcome(&mut reader, call.predicate)?;
        policies.push(ExecutedPolicy {
            call,
            verifier,
            outcome,
        });
    }
    check_canonical_order(
        policies.iter().map(|row| row.call.canonical_key()),
        "plan policies",
    )?;
    if reader.remaining() != 0 {
        return Err(CodecError::TrailingBytes {
            count: reader.remaining(),
        });
    }
    Ok(DeploymentPlan {
        request_commitment,
        instances,
        bindings,
        policies,
    })
}
