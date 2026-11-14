/*
Project, dependency, and log host requests from an admitted Terminal Build.

Source production authenticates protocol declarations and exact call sites, and
each activation authenticates its original compiler-issued `Build` receiver (or
projected `Build.log` facet) before accepting guest arguments. We validate all
borrowed UTF-8, identifier, and workspace-member-path inputs before reserving
accounting storage or copying request payloads.

The coordinator drains `IssuedBuildRequest` items between execution steps to
start dependency work while owning the final role, alias, and dependency join.
Draining requests does not release payload accounting: each `IssuedBuildRequest`
carries its own storage lease, and `BuildLog` retains its line leases until
normal-return settlement charges result custody and publishes the log bytes.
*/

use build_declarations::{
    BuildDependencySource, BuildRequest, DependencyPurpose, ProjectName, WorkspaceMemberPath,
};
use evaluation_accounting::{EvaluationInvocationAccount, EvaluationStorageLease};
use source_compiler::{
    TerminalBuildRequestCall, TerminalBuildRequestOperation as Operation, TerminalBuildRequests,
};
use std::sync::{Arc, Mutex};
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult,
    TerminalImportAdapter, TerminalImportSignature, TerminalScalarValue, TerminalStructuralValue,
};

const REGISTRATIONS_POISONED: &str = "Build registration lock was poisoned";

pub struct IssuedBuildRequest {
    request: BuildRequest,
    _storage: Option<EvaluationStorageLease>,
}

impl IssuedBuildRequest {
    pub fn request(&self) -> &BuildRequest {
        &self.request
    }
}

pub struct TerminalBuildRequestHost {
    artifact: terminal_codec::TerminalExecutionIdentity,
    calls: Arc<[TerminalBuildRequestCall]>,
    original: TerminalStructuralValue,
    account: Option<EvaluationInvocationAccount>,
    requests: Mutex<Vec<IssuedBuildRequest>>,
    registrations: Mutex<Registrations>,
    log: Mutex<BuildLog>,
    original_log: Option<TerminalStructuralValue>,
}

/// One dependency registration this activation issued a marker for. The
/// marker is the only value that names the registration: the guest holds a
/// copy, and `Build::prepare_dependency` hands one back.
struct Registration {
    marker: TerminalStructuralValue,
    purpose: DependencyPurpose,
    /// Position among the activation's requests of the same purpose, the
    /// coordinate the coordinator acquires an issued request by.
    request_ordinal: usize,
    prepared: bool,
    _storage: Option<EvaluationStorageLease>,
}

#[derive(Default)]
struct Registrations {
    issued: Vec<Registration>,
    /// The registration a parked `Build::prepare_dependency` waits for.
    awaiting: Option<usize>,
    /// Whether the request this host completed last was a dependency
    /// preparation. A stage that is not the last one must end there.
    ended_at_preparation: bool,
}

/// What the request host of a returned stage hands to the host of the next
/// stage of the same activation (`admission/build_stages.rs`): every
/// registration issued so far, so later registrations keep counting from
/// them, and the log written so far. The markers of earlier registrations
/// name types of the earlier stage's module; no later value equals one, and
/// no local carries one across.
pub(crate) struct CarriedRequests {
    registrations: Registrations,
    log: BuildLog,
}

/// The registered dependency a parked Build waits to have prepared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildDependencyPreparation {
    pub purpose: DependencyPurpose,
    /// Position among the activation's issued requests of `purpose`.
    pub request_ordinal: usize,
}

#[derive(Default)]
pub(crate) struct BuildLog {
    bytes: Vec<u8>,
    storage: Vec<EvaluationStorageLease>, // Retained until normal-return settlement publishes `bytes`.
}

impl BuildLog {
    pub(crate) fn publish(self, account: &EvaluationInvocationAccount) -> Result<Vec<u8>, String> {
        let bytes = u64::try_from(self.bytes.len()).map_err(|_| "Build log size overflowed")?;
        account
            .charge_result_custody(u64::from(bytes != 0), bytes)
            .map_err(|error| error.to_string())?;
        Ok(self.bytes)
    }
}

impl TerminalBuildRequestHost {
    pub fn new(
        artifact: terminal_codec::TerminalExecutionView<'_>,
        requests: &TerminalBuildRequests,
        original: TerminalStructuralValue,
        account: Option<&EvaluationInvocationAccount>,
    ) -> Result<Arc<Self>, String> {
        let calls = requests
            .calls_for(artifact)
            .ok_or("Build requests belong to another Terminal artifact")?;
        let original_log = if calls.iter().any(|call| call.action == Operation::LogLine) {
            Some(build_facet(artifact, &original, "log")?)
        } else {
            None
        };
        Ok(Arc::new(Self {
            artifact: artifact.custody_identity(),
            calls: Arc::clone(calls),
            original,
            account: account.cloned(),
            requests: Mutex::new(Vec::new()),
            registrations: Mutex::new(Registrations::default()),
            log: Mutex::new(BuildLog::default()),
            original_log,
        }))
    }

    pub fn adapter(
        self: &Arc<Self>,
        signature: TerminalImportSignature,
    ) -> TerminalBuildRequestAdapter {
        TerminalBuildRequestAdapter {
            host: Arc::clone(self),
            signature,
        }
    }

    pub fn drain_requests(&self) -> Result<Vec<IssuedBuildRequest>, String> {
        let mut requests = self
            .requests
            .lock()
            .map_err(|_| "Build request activation lock was poisoned")?;
        Ok(std::mem::take(&mut *requests))
    }

    /// The registration a parked `Build::prepare_dependency` call waits
    /// for, until `dependency_prepared` answers it.
    pub(crate) fn awaiting_preparation(
        &self,
    ) -> Result<Option<BuildDependencyPreparation>, String> {
        let registrations = self
            .registrations
            .lock()
            .map_err(|_| REGISTRATIONS_POISONED)?;
        Ok(registrations.awaiting.map(|position| {
            let registration = &registrations.issued[position];
            BuildDependencyPreparation {
                purpose: registration.purpose,
                request_ordinal: registration.request_ordinal,
            }
        }))
    }

    /// The coordinator prepared the awaited registration; the parked call
    /// completes when the activation is next advanced.
    pub(crate) fn dependency_prepared(&self) -> Result<(), String> {
        let mut registrations = self
            .registrations
            .lock()
            .map_err(|_| REGISTRATIONS_POISONED)?;
        let position = registrations
            .awaiting
            .take()
            .ok_or("Build has no dependency preparation pause")?;
        registrations.issued[position].prepared = true;
        Ok(())
    }

    /// Whether the stage this host served returned right after a dependency
    /// preparation completed, with no other request after it.
    pub(crate) fn ended_at_preparation(&self) -> Result<bool, String> {
        Ok(self
            .registrations
            .lock()
            .map_err(|_| REGISTRATIONS_POISONED)?
            .ended_at_preparation)
    }

    pub(crate) fn carry(&self) -> Result<CarriedRequests, String> {
        let mut registrations = self
            .registrations
            .lock()
            .map_err(|_| REGISTRATIONS_POISONED)?;
        if registrations.awaiting.is_some() {
            return Err("a Build stage ended while it awaited a dependency preparation".into());
        }
        Ok(CarriedRequests {
            registrations: std::mem::take(&mut *registrations),
            log: self.take_log()?,
        })
    }

    pub(crate) fn adopt(&self, carried: CarriedRequests) -> Result<(), String> {
        let mut registrations = self
            .registrations
            .lock()
            .map_err(|_| REGISTRATIONS_POISONED)?;
        let mut log = self.log.lock().map_err(|_| "Build log lock was poisoned")?;
        if !registrations.issued.is_empty() || !log.bytes.is_empty() {
            return Err("a Build stage adopts earlier requests before it runs".into());
        }
        *registrations = carried.registrations;
        registrations.ended_at_preparation = false;
        *log = carried.log;
        Ok(())
    }

    pub(crate) fn take_log(&self) -> Result<BuildLog, String> {
        let mut log = self.log.lock().map_err(|_| "Build log lock was poisoned")?;
        Ok(std::mem::take(&mut *log))
    }
}

pub(crate) fn build_facet(
    artifact: terminal_codec::TerminalExecutionView<'_>,
    original: &TerminalStructuralValue,
    name: &str,
) -> Result<TerminalStructuralValue, String> {
    let module = artifact.module();
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == original.structural_type)
        .ok_or("Build receiver lost its declaration")?;
    let terminal_psi::StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
        return Err("Build receiver is not a record".into());
    };
    let mut selected = fields
        .iter()
        .filter(|field| module.identities.text(field.identity) == Some(name));
    let field = selected
        .next()
        .ok_or_else(|| format!("Build receiver has no {name} facet"))?;
    if selected.next().is_some() {
        return Err(format!("Build receiver has multiple {name} facets"));
    }
    let terminal_psi::StructuralFieldType::Structural(structural_type) = field.field_type else {
        return Err(format!("Build {name} facet is not structural"));
    };
    let mut log =
        TerminalStructuralValue::minted(original.opaque_identity, structural_type, Vec::new());
    log.path = original.path.clone();
    log.path
        .push(terminal_psi::StructuralPathSegment::Field(field.identity));
    Ok(log)
}

pub struct TerminalBuildRequestAdapter {
    host: Arc<TerminalBuildRequestHost>,
    signature: TerminalImportSignature,
}

impl TerminalImportAdapter for TerminalBuildRequestAdapter {
    fn implemented_signature(&self) -> TerminalImportSignature {
        self.signature.clone()
    }
}

impl TerminalBuildRequestAdapter {
    /// Authenticates a `Build::prepare_dependency` call and names the issued
    /// registration its marker stands for.
    fn preparation(
        &self,
        effect: &TerminalEffect,
        registrations: &Registrations,
    ) -> Result<usize, TerminalEffectRejection> {
        let refuse = TerminalEffectRejection::new;
        let TerminalEffect::BoundaryCall {
            arguments,
            structural_arguments,
            byte_sequence_arguments,
            result,
            ..
        } = effect
        else {
            return Err(refuse("not a Build request effect"));
        };
        let [receiver, marker] = structural_arguments.as_slice() else {
            return Err(refuse(
                "Build dependency preparation has an invalid argument roster",
            ));
        };
        if !arguments.is_empty()
            || byte_sequence_arguments.as_slice() != [None, None]
            || *result != terminal_psi::BoundaryMachineResult::Unit
        {
            return Err(refuse(
                "Build dependency preparation differs from its protocol",
            ));
        }
        if receiver != &self.host.original {
            return Err(refuse(
                "project requests require the original compiler-issued Build receiver",
            ));
        }
        let position = registrations
            .issued
            .iter()
            .position(|registration| &registration.marker == marker)
            .ok_or_else(|| {
                refuse(
                    "Build::prepare_dependency requires a registration this activation issued; an authored `DependencyRegistration {}` names none",
                )
            })?;
        // scoped_execution.md, "Dependency declarations and binding":
        // "`Build::build_dependency` registers a dependency for interpreted
        // Build code", and preparation "admits and prepares that dependency
        // for the requested execution profile". What preparing a Product
        // registration would make ready for a Build is not built.
        if registrations.issued[position].purpose != DependencyPurpose::Build {
            return Err(refuse(
                "not supported yet: Build::prepare_dependency on a registration made with Build::dependency; only a Build::build_dependency registration is prepared for Build code",
            ));
        }
        Ok(position)
    }
}

impl TerminalEffectHandler for TerminalBuildRequestAdapter {
    fn poll_effect_readiness(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<bool, TerminalEffectRejection> {
        crate::host::require_effect_owner(effect, &self.host.artifact)?;
        let refuse = TerminalEffectRejection::new;
        let call = self
            .host
            .calls
            .iter()
            .find(|call| crate::host::is_call_at(effect, call.site))
            .ok_or_else(|| refuse("Build request has no source-issued protocol call"))?;
        if call.action != Operation::PrepareDependency {
            return Ok(true);
        }
        let mut registrations = self
            .host
            .registrations
            .lock()
            .map_err(|_| refuse(REGISTRATIONS_POISONED))?;
        let position = self.preparation(effect, &registrations)?;
        if registrations.issued[position].prepared {
            return Ok(true);
        }
        if registrations
            .awaiting
            .is_some_and(|awaiting| awaiting != position)
        {
            return Err(refuse(
                "Build activation already awaits another dependency preparation",
            ));
        }
        registrations.awaiting = Some(position);
        Ok(false)
    }

    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new(
            "Build requests require result-aware dispatch",
        ))
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        crate::host::require_effect_owner(effect, &self.host.artifact)?;
        let refuse = TerminalEffectRejection::new;
        let TerminalEffect::BoundaryCall {
            arguments,
            structural_arguments,
            byte_sequence_arguments,
            result,
            ..
        } = effect
        else {
            return Err(refuse("not a Build request effect"));
        };
        let call = self
            .host
            .calls
            .iter()
            .find(|call| crate::host::is_call_at(effect, call.site))
            .ok_or_else(|| refuse("Build request has no source-issued protocol call"))?;
        if call.action != Operation::PrepareDependency {
            self.host
                .registrations
                .lock()
                .map_err(|_| refuse(REGISTRATIONS_POISONED))?
                .ended_at_preparation = false;
        }
        if call.action == Operation::LogLine {
            let [facet, _] = structural_arguments.as_slice() else {
                return Err(refuse("Build log request has an invalid argument roster"));
            };
            let [None, Some(bytes)] = byte_sequence_arguments.as_slice() else {
                return Err(refuse("Build log request has no byte payload"));
            };
            if !arguments.is_empty() || *result != terminal_psi::BoundaryMachineResult::Unit {
                return Err(refuse("Build log request differs from its protocol"));
            }
            if self.host.original_log.as_ref() != Some(facet) {
                return Err(refuse(
                    "build logging requires the activation's compiler-issued BuildLog facet",
                ));
            }
            let appended = bytes
                .len()
                .checked_add(1)
                .ok_or_else(|| refuse("Build log size overflowed"))?;
            let count = u64::try_from(appended).map_err(|_| refuse("Build log size overflowed"))?;
            let storage = self
                .host
                .account
                .as_ref()
                .map(|account| {
                    let storage = account.reserve_site(0, 1, count)?;
                    account.charge_build_log_bytes(count)?;
                    Ok::<_, evaluation_accounting::EvaluationAccountingError>(storage)
                })
                .transpose()
                .map_err(|error| {
                    TerminalEffectRejection::new(format!("Build log accounting: {error}"))
                })?;
            let mut log = self
                .host
                .log
                .lock()
                .map_err(|_| refuse("Build log lock was poisoned"))?;
            log.bytes
                .try_reserve(appended)
                .map_err(|_| refuse("Build log allocation was refused"))?;
            if let Some(storage) = storage {
                log.storage
                    .try_reserve(1)
                    .map_err(|_| refuse("Build log reservation allocation was refused"))?;
                log.storage.push(storage);
            }
            log.bytes.extend_from_slice(bytes);
            log.bytes.push(b'\n');
            return Ok(TerminalEffectResult::Unit);
        }
        if call.action == Operation::PrepareDependency {
            let mut registrations = self
                .host
                .registrations
                .lock()
                .map_err(|_| refuse(REGISTRATIONS_POISONED))?;
            let position = self.preparation(effect, &registrations)?;
            if !registrations.issued[position].prepared {
                return Err(refuse(
                    "Build dependency preparation requires its completed preparation",
                ));
            }
            registrations.ended_at_preparation = true;
            return Ok(TerminalEffectResult::Unit);
        }
        // A registration returns the marker that names it; every other
        // request returns nothing.
        let registers = matches!(
            call.action,
            Operation::PathDependency | Operation::GitDependency
        );
        if structural_arguments.first() != Some(&self.host.original)
            || byte_sequence_arguments.first() != Some(&None)
            || structural_arguments.len() != byte_sequence_arguments.len()
            || registers != matches!(result, terminal_psi::BoundaryMachineResult::Structural(_))
            || (!registers && *result != terminal_psi::BoundaryMachineResult::Unit)
        {
            return Err(refuse(
                "project requests require the original compiler-issued Build receiver",
            ));
        }
        let text = |position: usize| -> Result<&str, TerminalEffectRejection> {
            let bytes = byte_sequence_arguments
                .get(position)
                .and_then(Option::as_deref)
                .ok_or_else(|| refuse("Build request lost an evaluated byte argument"))?;
            std::str::from_utf8(bytes).map_err(|_| refuse("Build request text is not UTF-8"))
        };
        let name = |value: &str| -> Result<(), TerminalEffectRejection> {
            if ProjectName::is_valid(value) {
                Ok(())
            } else {
                Err(refuse(
                    "Build project name or alias is not a canonical identifier",
                ))
            }
        };
        let (scalar_count, structural_count) = match call.action {
            Operation::ArtifactOnly => (0, 1),
            Operation::Package | Operation::Application | Operation::Member => (0, 2),
            Operation::PathDependency => (2, 3),
            Operation::GitDependency => (3, 5),
            Operation::LogLine | Operation::PrepareDependency => {
                return Err(refuse("Build request did not complete its host request"));
            }
        };
        if arguments.len() != scalar_count || structural_arguments.len() != structural_count {
            return Err(refuse(
                "Build request argument roster differs from its protocol",
            ));
        }
        let flag = |position: usize| -> Result<bool, TerminalEffectRejection> {
            match arguments.get(position) {
                Some(TerminalScalarValue::Boolean(value)) => Ok(*value),
                _ => Err(refuse("Build request flag is not Boolean")),
            }
        };
        match call.action {
            Operation::Package | Operation::Application => name(text(1)?)?,
            Operation::Member if !WorkspaceMemberPath::is_valid(text(1)?) => {
                return Err(refuse("Build workspace member path is not canonical"));
            }
            Operation::PathDependency | Operation::GitDependency => {
                flag(0)?;
                if flag(1)? {
                    name(text(1)?)?;
                } else if !text(1)?.is_empty() {
                    return Err(refuse("implicit dependency alias carries unexpected text"));
                }
                text(2)?;
                if call.action == Operation::GitDependency {
                    text(3)?;
                    if flag(2)? {
                        name(text(4)?)?;
                    } else if !text(4)?.is_empty() {
                        return Err(refuse("root package selection carries unexpected text"));
                    }
                }
            }
            _ => {}
        }
        let payload = byte_sequence_arguments
            .iter()
            .flatten()
            .try_fold(0u64, |total, bytes| {
                u64::try_from(bytes.len())
                    .ok()
                    .and_then(|length| total.checked_add(length))
            })
            .ok_or_else(|| refuse("Build request payload count overflowed"))?;
        let storage = self
            .host
            .account
            .as_ref()
            .map(|account| account.reserve_site(0, 1, payload))
            .transpose()
            .map_err(|error| {
                TerminalEffectRejection::new(format!("Build request storage: {error:?}"))
            })?;
        let copy = |value: &str| -> Result<String, TerminalEffectRejection> {
            let mut copied = String::new();
            copied
                .try_reserve_exact(value.len())
                .map_err(|_| refuse("cannot retain Build request text"))?;
            copied.push_str(value);
            Ok(copied)
        };
        let project_name =
            |value: &str| ProjectName::parse(copy(value)?).map_err(TerminalEffectRejection::new);
        let request = match call.action {
            Operation::LogLine | Operation::PrepareDependency => {
                return Err(refuse("Build request did not complete its host request"));
            }
            Operation::Package => BuildRequest::Package(project_name(text(1)?)?),
            Operation::Application => BuildRequest::Application(project_name(text(1)?)?),
            Operation::Member => BuildRequest::Member(
                WorkspaceMemberPath::parse(copy(text(1)?)?)
                    .map_err(|error| TerminalEffectRejection::new(error.to_string()))?,
            ),
            Operation::ArtifactOnly => BuildRequest::ArtifactOnly,
            Operation::PathDependency | Operation::GitDependency => BuildRequest::Dependency {
                purpose: if flag(0)? {
                    DependencyPurpose::Build
                } else {
                    DependencyPurpose::Product
                },
                alias: if flag(1)? {
                    Some(project_name(text(1)?)?)
                } else {
                    None
                },
                source: if call.action == Operation::PathDependency {
                    BuildDependencySource::Path(copy(text(2)?)?)
                } else {
                    BuildDependencySource::Git {
                        repository: copy(text(2)?)?,
                        revision: copy(text(3)?)?,
                        package: if flag(2)? {
                            Some(project_name(text(4)?)?)
                        } else {
                            None
                        },
                    }
                },
            },
        };
        let mut requests = self
            .host
            .requests
            .lock()
            .map_err(|_| refuse("Build request activation lock was poisoned"))?;
        requests
            .try_reserve(1)
            .map_err(|_| refuse("cannot retain Build request row"))?;
        let registered = match (&request, result) {
            (
                BuildRequest::Dependency { purpose, .. },
                terminal_psi::BoundaryMachineResult::Structural(result),
            ) => Some((*purpose, result.structural_type)),
            _ => None,
        };
        requests.push(IssuedBuildRequest {
            request,
            _storage: storage,
        });
        let Some((purpose, structural_type)) = registered else {
            return Ok(TerminalEffectResult::Unit);
        };
        let mut registrations = self
            .host
            .registrations
            .lock()
            .map_err(|_| refuse(REGISTRATIONS_POISONED))?;
        registrations
            .issued
            .try_reserve(1)
            .map_err(|_| refuse("cannot retain Build dependency registration"))?;
        let storage = self
            .host
            .account
            .as_ref()
            .map(|account| account.reserve_site(0, 1, 0))
            .transpose()
            .map_err(|error| {
                TerminalEffectRejection::new(format!("Build registration storage: {error:?}"))
            })?;
        // Marker identities count down from the middle of the space: the
        // receiver is 1 and filesystem markers count down from the top.
        let identity = u64::try_from(registrations.issued.len())
            .ok()
            .and_then(|issued| (u64::MAX / 2).checked_sub(issued))
            .filter(|identity| *identity > 1)
            .ok_or_else(|| refuse("Build registration marker space is exhausted"))?;
        let marker = TerminalStructuralValue::minted(identity, structural_type, Vec::new());
        let request_ordinal = registrations
            .issued
            .iter()
            .filter(|registration| registration.purpose == purpose)
            .count();
        registrations.issued.push(Registration {
            marker: marker.clone(),
            purpose,
            request_ordinal,
            prepared: false,
            _storage: storage,
        });
        Ok(TerminalEffectResult::Structural(marker))
    }
}
