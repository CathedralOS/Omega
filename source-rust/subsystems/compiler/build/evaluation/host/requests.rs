/*
Project requests, logs and generated source from an admitted Terminal Build.

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

use build_declarations::{BuildDependencySource, BuildRequest, DependencyPurpose, ProjectName};
use evaluation_accounting::{EvaluationInvocationAccount, EvaluationStorageLease};
mod input_bindings;
mod output;
mod reach;
mod workspace;
pub use input_bindings::FrozenBuildBinding;
use input_bindings::InputBindings;
pub(crate) use output::BuildRequestOutput;
pub(crate) use reach::ReflectedServices;
use source_compiler::publication::{
    TerminalBuildRequestCall, TerminalBuildRequestOperation as Operation, TerminalBuildRequests,
};
use std::sync::{Arc, Mutex};
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult,
    TerminalImportAdapter, TerminalImportSignature, TerminalScalarValue, TerminalStructuralValue,
};

const FIRST_REGISTRATION_IDENTITY: u64 = u64::MAX / 2;
const REGISTRATIONS_POISONED: &str = "Build registration lock was poisoned";

pub struct IssuedBuildRequest {
    request: BuildRequest,
    forwarded: Option<FrozenBuildBinding>,
    _storage: Option<EvaluationStorageLease>,
}

impl IssuedBuildRequest {
    pub fn forwarded(&self) -> Option<&FrozenBuildBinding> {
        self.forwarded.as_ref()
    }

    pub fn request(&self) -> &BuildRequest {
        &self.request
    }
}

pub struct TerminalBuildRequestHost {
    inputs: InputBindings,
    reach: reach::ReachSets,
    workspace: workspace::WorkspaceCatalogs,
    artifact: terminal_codec::TerminalExecutionIdentity,
    calls: Arc<[TerminalBuildRequestCall]>,
    original: TerminalStructuralValue,
    account: Option<EvaluationInvocationAccount>,
    requests: Mutex<Vec<IssuedBuildRequest>>,
    registrations: Mutex<Registrations>,
    output: Mutex<BuildRequestOutput>,
    original_log: Option<TerminalStructuralValue>,
    diagnostic_output_bytes: Option<u64>,
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
}

impl Registrations {
    fn position(&self, marker: &TerminalStructuralValue) -> Option<usize> {
        let ordinal = FIRST_REGISTRATION_IDENTITY.checked_sub(marker.opaque_identity)?;
        let index = usize::try_from(ordinal).ok()?;
        (&self.issued.get(index)?.marker == marker).then_some(index)
    }
}

/// The registered dependency a parked Build waits to have prepared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildDependencyPreparation {
    pub purpose: DependencyPurpose,
    /// Position among the activation's issued requests of `purpose`.
    pub request_ordinal: usize,
}

impl TerminalBuildRequestHost {
    pub fn new(
        artifact: terminal_codec::TerminalExecutionView<'_>,
        requests: &TerminalBuildRequests,
        original: TerminalStructuralValue,
        diagnostic_output_bytes: Option<u64>,
        account: Option<&EvaluationInvocationAccount>,
        libraries: Arc<[package_compilation::source_imports::SuppliedBuildLibrary]>,
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
            inputs: InputBindings::new(libraries),
            workspace: Default::default(),
            reach: reach::ReachSets::from_module(
                artifact.module(),
                calls
                    .iter()
                    .any(|call| call.action == Operation::ExtendReach),
            )?,
            original,
            account: account.cloned(),
            requests: Mutex::new(Vec::new()),
            registrations: Mutex::new(Registrations::default()),
            output: Mutex::default(),
            original_log,
            diagnostic_output_bytes,
        }))
    }

    pub(crate) fn reflected_services(&self) -> Arc<ReflectedServices> {
        Arc::clone(&self.reach.reflections)
    }

    pub(crate) fn workspace_members(
        &self,
        marker: &TerminalStructuralValue,
    ) -> Result<Vec<build_declarations::WorkspaceMemberPath>, String> {
        self.workspace.paths(marker)
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

    fn output(&self) -> Result<std::sync::MutexGuard<'_, BuildRequestOutput>, &'static str> {
        self.output
            .lock()
            .map_err(|_| "Build request output lock was poisoned")
    }

    pub(crate) fn take_output(&self) -> Result<BuildRequestOutput, String> {
        let mut output = self.output()?;
        Ok(std::mem::take(&mut *output))
    }
}

pub(crate) fn build_facet(
    artifact: terminal_codec::TerminalExecutionView<'_>,
    original: &TerminalStructuralValue,
    name: &str,
) -> Result<TerminalStructuralValue, String> {
    let module = artifact.module();
    let declaration = module
        .types
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
            .position(marker)
            .ok_or_else(|| {
                refuse(
                    "Build::prepare_dependency requires a registration this activation issued; an authored `DependencyRegistration {}` names none",
                )
            })?;
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
        if call.action == Operation::SandboxWrite {
            let ([], [_], [Some(bytes)], terminal_psi::BoundaryMachineResult::Unit) = (
                arguments.as_slice(),
                structural_arguments.as_slice(),
                byte_sequence_arguments.as_slice(),
                result,
            ) else {
                return Err(refuse("sandbox output differs from its admitted signature"));
            };
            let limit = self
                .host
                .diagnostic_output_bytes
                .ok_or_else(|| refuse("Build has no admitted diagnostic sink"))?;
            self.host
                .output()
                .map_err(refuse)?
                .write_stream(bytes, limit, self.host.account.as_ref())
                .map_err(TerminalEffectRejection::new)?;
            return Ok(TerminalEffectResult::Unit);
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
            self.host
                .output()
                .map_err(TerminalEffectRejection::new)?
                .log
                .append(bytes, true, self.host.account.as_ref())
                .map_err(TerminalEffectRejection::new)?;
            return Ok(TerminalEffectResult::Unit);
        }
        if call.action == Operation::PrepareDependency {
            let registrations = self
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
            return Ok(TerminalEffectResult::Unit);
        }
        // A registration returns the marker that names it; every other
        // request returns nothing.
        let registers = matches!(
            call.action,
            Operation::PathDependency
                | Operation::GitDependency
                | Operation::InputBinding
                | Operation::AddBinding
                | Operation::ExtendReach
                | Operation::ExtendWorkspace
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
        if call.action == Operation::PublishSource {
            let [None, Some(name), Some(source)] = byte_sequence_arguments.as_slice() else {
                return Err(refuse(
                    "Build source publication requires a filename and source bytes",
                ));
            };
            if !arguments.is_empty() {
                return Err(refuse(
                    "Build source publication does not accept scalar arguments",
                ));
            }
            let account = self.host.account.as_ref().ok_or_else(|| {
                refuse("Build source publication requires the activation's evaluation account")
            })?;
            self.host
                .output()
                .map_err(TerminalEffectRejection::new)?
                .sources
                .publish(account, name, source)
                .map_err(TerminalEffectRejection::new)?;
            return Ok(TerminalEffectResult::Unit);
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
            Operation::InputBinding => (0, 2),
            Operation::AddBinding => (0, 3),
            Operation::ExtendReach | Operation::ExtendWorkspace => (0, 3),
            Operation::PathDependency => (2, 4),
            Operation::GitDependency => (3, 6),
            Operation::ConfigureTestFixture
            | Operation::ConfigureTestEngine
            | Operation::SandboxWrite
            | Operation::LogLine
            | Operation::PrepareDependency
            | Operation::PublishSource => {
                return Err(refuse("Build request did not complete its host request"));
            }
        };
        if arguments.len() != scalar_count || structural_arguments.len() != structural_count {
            return Err(refuse(
                "Build request argument roster differs from its protocol",
            ));
        }
        if call.action == Operation::InputBinding {
            let terminal_psi::BoundaryMachineResult::Structural(binding) = result else {
                return Err(refuse("Build input binding requires its opaque result"));
            };
            return self
                .host
                .inputs
                .select(
                    text(1)?,
                    binding.structural_type,
                    self.host.account.as_ref(),
                )
                .map(TerminalEffectResult::Structural);
        }
        if call.action == Operation::ExtendWorkspace {
            let terminal_psi::BoundaryMachineResult::Structural(result) = result else {
                return Err(refuse(
                    "workspace member extension requires a snapshot result",
                ));
            };
            let [None, None, Some(path)] = byte_sequence_arguments.as_slice() else {
                return Err(refuse(
                    "workspace member extension requires its pathname bytes",
                ));
            };
            return self
                .host
                .workspace
                .extend(
                    &structural_arguments[1],
                    path,
                    result.structural_type,
                    self.host.account.as_ref(),
                )
                .map(TerminalEffectResult::Structural);
        }
        if call.action == Operation::ExtendReach {
            let terminal_psi::BoundaryMachineResult::Structural(result) = result else {
                return Err(refuse("Build service set requires its opaque result"));
            };
            return self
                .host
                .reach
                .extend(
                    &structural_arguments[1],
                    &structural_arguments[2],
                    result.structural_type,
                    self.host.account.as_ref(),
                )
                .map(TerminalEffectResult::Structural);
        }
        let max_reach = if matches!(
            call.action,
            Operation::PathDependency | Operation::GitDependency | Operation::AddBinding
        ) {
            self.host.reach.get(
                structural_arguments
                    .last()
                    .ok_or_else(|| refuse("Build dependency lost its service set"))?,
            )?
        } else {
            Default::default()
        };
        let forwarded = if call.action == Operation::AddBinding {
            if byte_sequence_arguments[1].is_some() {
                return Err(refuse("Build dependency registration requires a PackageBinding"));
            }
            let binding = self.host.inputs.require(&structural_arguments[1])?;
            if !max_reach.is_subset_of(binding.library().max_reach()) {
                return Err(refuse(
                    "forwarded dependency exceeds its incoming max_reach",
                ));
            }
            Some(binding)
        } else {
            None
        };
        let flag = |position: usize| -> Result<bool, TerminalEffectRejection> {
            match arguments.get(position) {
                Some(TerminalScalarValue::Boolean(value)) => Ok(*value),
                _ => Err(refuse("Build request flag is not Boolean")),
            }
        };
        match call.action {
            Operation::PathDependency | Operation::GitDependency => {
                flag(0)?;
                if flag(1)? {
                    let alias = text(1)?;
                    if !ProjectName::is_valid(alias) {
                        return Err(TerminalEffectRejection::new(format!(
                            "Build dependency alias `{}` must start with an ASCII lowercase letter and use only ASCII lowercase letters, digits, and single interior underscores",
                            alias.escape_debug()
                        )));
                    }
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
        let mut retained_text: Vec<&str> = max_reach.services().collect();
        if let Some(BuildRequest::Dependency { alias, source, .. }) = forwarded
            .as_ref()
            .map(|binding| binding.library().request())
        {
            retained_text.extend(alias.as_ref().map(ProjectName::as_str));
            match source {
                BuildDependencySource::Path(path) => retained_text.push(path),
                BuildDependencySource::Git {
                    repository,
                    revision,
                    package,
                } => {
                    retained_text.extend([repository.as_str(), revision.as_str()]);
                    retained_text.extend(package.as_ref().map(ProjectName::as_str));
                }
            }
        }
        let payload = byte_sequence_arguments
            .iter()
            .flatten()
            .map(|bytes| bytes.len())
            .chain(retained_text.into_iter().map(str::len))
            .try_fold(0u64, |total, length| {
                total.checked_add(u64::try_from(length).ok()?)
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
            Operation::ConfigureTestFixture
            | Operation::ConfigureTestEngine
            | Operation::SandboxWrite
            | Operation::LogLine
            | Operation::PrepareDependency
            | Operation::PublishSource => {
                return Err(refuse("Build request did not complete its host request"));
            }
            Operation::ArtifactOnly => BuildRequest::ArtifactOnly,
            Operation::InputBinding | Operation::ExtendReach | Operation::ExtendWorkspace => {
                return Err(refuse(
                    "Build input binding or service set already returned",
                ));
            }
            Operation::AddBinding => {
                let Some(BuildRequest::Dependency {alias, source, ..}) = forwarded.as_ref().map(|binding|binding.library().request()) else {
                    return Err(refuse("Build input binding lost its frozen dependency"));
                };
                BuildRequest::Dependency {
                    purpose: DependencyPurpose::Product,
                    max_reach,
                    alias: alias.clone(),
                    source: source.clone(),
                }
            }
            Operation::PathDependency | Operation::GitDependency => BuildRequest::Dependency {
                max_reach,
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
            forwarded,
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
            .and_then(|issued| FIRST_REGISTRATION_IDENTITY.checked_sub(issued))
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
