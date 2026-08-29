//! Git and transport executable selection, content custody, and budgets.

use super::*;

#[derive(Debug)]
pub(in crate::resolution::source) struct GitExecutor {
    pub(in crate::resolution::source) identity: GitExecutableIdentity,
    pub(in crate::resolution::source) metadata_identity: GitExecutableMetadataIdentity,
    pub(in crate::resolution::source) transport_executable:
        Option<GitTransportExecutableObservation>,
    pub(in crate::resolution::source) execution_helpers: Vec<GitTransportExecutableObservation>,
    pub(in crate::resolution::source) execution_transport: GitExecutionTransport,
    pub(in crate::resolution::source) requested_network_endpoint:
        ResolverExecutionRequestedEndpoint,
    pub(in crate::resolution::source) started: Instant,
    pub(in crate::resolution::source) timeout: Duration,
    pub(in crate::resolution::source) launches: Cell<usize>,
    pub(in crate::resolution::source) execution_policy_observations:
        RefCell<Vec<ResolverExecutionPolicyObservation>>,
    pub(in crate::resolution::source) command_execution_observations:
        RefCell<Vec<GitCommandExecutionObservation>>,
    pub(in crate::resolution::source) captured_output_budget: GitCapturedOutputBudget,
    pub(in crate::resolution::source) network_transfer_budget: ResolverExecutionTransferBudget,
    pub(in crate::resolution::source) maximum_launches: usize,
    pub(in crate::resolution::source) execution_backend: ResolverExecutionBackend,
}

#[derive(Debug, Clone)]
pub(in crate::resolution::source) struct GitCapturedOutputBudget {
    pub(in crate::resolution::source) ceiling: u64,
    pub(in crate::resolution::source) observed: Arc<AtomicU64>,
}

impl GitCapturedOutputBudget {
    pub(in crate::resolution::source) fn new(ceiling: u64) -> Self {
        Self {
            ceiling,
            observed: Arc::new(AtomicU64::new(0)),
        }
    }

    pub(in crate::resolution::source) fn observed(&self) -> u64 {
        self.observed.load(Ordering::Acquire)
    }

    pub(in crate::resolution::source) fn charge(
        &self,
        count: usize,
    ) -> Result<(), CapturedOutputLimitExceeded> {
        let count = u64::try_from(count).map_err(|_| CapturedOutputLimitExceeded {
            ceiling: self.ceiling,
            attempted: u64::MAX,
        })?;
        let mut current = self.observed();
        loop {
            let attempted = current
                .checked_add(count)
                .ok_or(CapturedOutputLimitExceeded {
                    ceiling: self.ceiling,
                    attempted: u64::MAX,
                })?;
            if attempted > self.ceiling {
                return Err(CapturedOutputLimitExceeded {
                    ceiling: self.ceiling,
                    attempted,
                });
            }
            match self.observed.compare_exchange_weak(
                current,
                attempted,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(()),
                Err(actual) => current = actual,
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::resolution::source) struct CapturedOutputLimitExceeded {
    pub(in crate::resolution::source) ceiling: u64,
    pub(in crate::resolution::source) attempted: u64,
}

#[derive(Debug)]
pub(in crate::resolution::source) struct GitTransportExecutableObservation {
    pub(in crate::resolution::source) identity: GitTransportExecutableIdentity,
    pub(in crate::resolution::source) metadata_identity: GitExecutableMetadataIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::resolution::source) struct GitExecutableMetadataIdentity {
    length: u64,
    modified: SystemTime,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    mode: u32,
}

impl GitExecutor {
    pub(in crate::resolution::source) fn system(
        execution_transport: GitExecutionTransport,
        requested_network_endpoint: ResolverExecutionRequestedEndpoint,
        limits: LocalSourceLimits,
    ) -> Result<Self, SourceResolveError> {
        for candidate in system_git_candidates() {
            let path = Path::new(candidate);
            if path.is_file() {
                return Self::open_with_budget_for_transport(
                    path,
                    GIT_FIXED_COMMAND_ALLOWANCE,
                    GIT_RESOLUTION_TIMEOUT,
                    git_resolution_captured_output_ceiling(limits),
                    git_resolution_network_transfer_ceiling(limits),
                    execution_transport,
                    requested_network_endpoint,
                );
            }
        }
        Err(SourceResolveError::GitExecutableUnavailable)
    }

    #[cfg(test)]
    pub(in crate::resolution::source) fn open(path: &Path) -> Result<Self, SourceResolveError> {
        Self::open_with_budget_for_transport(
            path,
            GIT_FIXED_COMMAND_ALLOWANCE,
            GIT_RESOLUTION_TIMEOUT,
            git_resolution_captured_output_ceiling(LocalSourceLimits::default()),
            git_resolution_network_transfer_ceiling(LocalSourceLimits::default()),
            GitExecutionTransport::File,
            test_file_network_endpoint(),
        )
    }

    #[cfg(test)]
    pub(in crate::resolution::source) fn open_with_budget(
        path: &Path,
        maximum_launches: usize,
        timeout: Duration,
    ) -> Result<Self, SourceResolveError> {
        Self::open_with_budget_for_transport(
            path,
            maximum_launches,
            timeout,
            git_resolution_captured_output_ceiling(LocalSourceLimits::default()),
            git_resolution_network_transfer_ceiling(LocalSourceLimits::default()),
            GitExecutionTransport::File,
            test_file_network_endpoint(),
        )
    }

    #[cfg(test)]
    pub(in crate::resolution::source) fn open_with_resource_budgets(
        path: &Path,
        maximum_launches: usize,
        timeout: Duration,
        captured_output_ceiling: u64,
    ) -> Result<Self, SourceResolveError> {
        Self::open_with_budget_for_transport(
            path,
            maximum_launches,
            timeout,
            captured_output_ceiling,
            git_resolution_network_transfer_ceiling(LocalSourceLimits::default()),
            GitExecutionTransport::File,
            test_file_network_endpoint(),
        )
    }

    pub(in crate::resolution::source) fn open_with_budget_for_transport(
        path: &Path,
        maximum_launches: usize,
        timeout: Duration,
        captured_output_ceiling: u64,
        network_transfer_ceiling: u64,
        execution_transport: GitExecutionTransport,
        requested_network_endpoint: ResolverExecutionRequestedEndpoint,
    ) -> Result<Self, SourceResolveError> {
        let started = Instant::now();
        if !path.is_absolute() {
            return Err(SourceResolveError::GitExecutableInvalid {
                path: path.to_path_buf(),
                message: "path is not absolute".to_owned(),
            });
        }
        let canonical =
            path.canonicalize()
                .map_err(|error| SourceResolveError::GitExecutableInvalid {
                    path: path.to_path_buf(),
                    message: error.to_string(),
                })?;
        verify_git_executable_custody(&canonical)?;
        let metadata_identity = observe_git_executable_metadata(&canonical)?;
        let content_identity = hash_git_executable(&canonical)?;
        if observe_git_executable_metadata(&canonical)? != metadata_identity {
            return Err(SourceResolveError::GitExecutableChanged { path: canonical });
        }
        let transport_executable = match execution_transport {
            GitExecutionTransport::Ssh => Some(open_ssh_transport_executable(&canonical)?),
            GitExecutionTransport::Https => Some(open_https_transport_executable(&canonical)?),
            #[cfg(test)]
            GitExecutionTransport::File => None,
        };
        let execution_helpers = open_resolver_execution_helpers(execution_transport)?;
        let execution_backend = ResolverExecutionBackend::open().map_err(|error| {
            SourceResolveError::GitExecutionBoundaryInvalid {
                message: error.to_string(),
            }
        })?;
        let network_transfer_budget =
            ResolverExecutionTransferBudget::new(network_transfer_ceiling).map_err(|error| {
                SourceResolveError::GitExecutionBoundaryInvalid {
                    message: format!("cannot establish network-transfer budget: {error}"),
                }
            })?;
        Ok(Self {
            identity: GitExecutableIdentity {
                path: canonical,
                content_identity,
            },
            metadata_identity,
            transport_executable,
            execution_helpers,
            execution_transport,
            requested_network_endpoint,
            started,
            timeout,
            launches: Cell::new(0),
            execution_policy_observations: RefCell::new(Vec::new()),
            command_execution_observations: RefCell::new(Vec::new()),
            captured_output_budget: GitCapturedOutputBudget::new(captured_output_ceiling),
            network_transfer_budget,
            maximum_launches,
            execution_backend,
        })
    }

    pub(in crate::resolution::source) fn verify(&self) -> Result<(), SourceResolveError> {
        if observe_git_executable_metadata(&self.identity.path)? != self.metadata_identity {
            return Err(SourceResolveError::GitExecutableChanged {
                path: self.identity.path.clone(),
            });
        }
        verify_git_executable_custody(&self.identity.path)?;
        if let Some(transport_executable) = &self.transport_executable {
            verify_git_transport_executable(transport_executable)?;
        }
        for helper in &self.execution_helpers {
            verify_git_transport_executable(helper)?;
        }
        self.execution_backend.verify().map_err(|error| {
            SourceResolveError::GitExecutionBoundaryInvalid {
                message: error.to_string(),
            }
        })?;
        Ok(())
    }

    pub(in crate::resolution::source) fn verify_content(&self) -> Result<(), SourceResolveError> {
        self.verify()?;
        if hash_git_executable(&self.identity.path)? != self.identity.content_identity {
            return Err(SourceResolveError::GitExecutableChanged {
                path: self.identity.path.clone(),
            });
        }
        if let Some(transport_executable) = &self.transport_executable
            && hash_git_executable(&transport_executable.identity.path)?
                != transport_executable.identity.content_identity
        {
            return Err(SourceResolveError::GitExecutableChanged {
                path: transport_executable.identity.path.clone(),
            });
        }
        for helper in &self.execution_helpers {
            if hash_git_executable(&helper.identity.path)? != helper.identity.content_identity {
                return Err(SourceResolveError::GitExecutableChanged {
                    path: helper.identity.path.clone(),
                });
            }
        }
        self.verify()?;
        self.verify_budget()
    }

    pub(in crate::resolution::source) fn validate_execution_policy_observations(
        &self,
    ) -> Result<(), SourceResolveError> {
        let observations = self.execution_policy_observations.borrow();
        if observations.len() != self.launches.get() {
            return Err(SourceResolveError::GitExecutionBoundaryInvalid {
                message: "native policy observation count does not match launched command count"
                    .to_owned(),
            });
        }
        let command_observations = self.command_execution_observations.borrow();
        if command_observations.len() != self.launches.get() {
            return Err(SourceResolveError::GitExecutionBoundaryInvalid {
                message: "native command outcome count does not match launched command count"
                    .to_owned(),
            });
        }
        for observation in observations.iter() {
            if observation.executable() != self.identity.path {
                return Err(SourceResolveError::GitExecutionBoundaryInvalid {
                    message: "native policy observation names a different Git executable"
                        .to_owned(),
                });
            }
            let network_phase = matches!(
                observation.phase(),
                ResolverExecutionPhase::TransportDiscovery | ResolverExecutionPhase::Fetch
            );
            let expected_network_transport =
                network_phase.then(|| self.execution_transport.resolver_network_transport());
            if observation.network_transport() != expected_network_transport {
                return Err(SourceResolveError::GitExecutionBoundaryInvalid {
                    message: "native policy observation transport authority does not match the validated source transport"
                        .to_owned(),
                });
            }
            let mut expected = BTreeSet::new();
            if network_phase {
                for executable in self
                    .transport_executable
                    .iter()
                    .chain(self.execution_helpers.iter())
                {
                    expected.insert(executable.identity.invocation_path.clone());
                    expected.insert(executable.identity.path.clone());
                }
            }
            expected.remove(&self.identity.path);
            let observed = observation
                .additional_executables()
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>();
            if observed != expected {
                return Err(SourceResolveError::GitExecutionBoundaryInvalid {
                    message: "native policy observation executable paths do not match verified executable content custody"
                        .to_owned(),
                });
            }
        }
        for (policy, command) in observations.iter().zip(command_observations.iter()) {
            if command.phase != policy.phase()
                || command.policy_identity
                    != format_sha256(&Sha256::digest(policy.canonical_bytes()))
            {
                return Err(SourceResolveError::GitExecutionBoundaryInvalid {
                    message: "native command outcome is not joined to its policy observation"
                        .to_owned(),
                });
            }
            match (policy.endpoint_route(), &command.endpoint_observation) {
                (Some(route), Some(endpoint)) if endpoint.route() == route => {
                    #[cfg(not(test))]
                    let requires_connection = true;
                    #[cfg(test)]
                    let requires_connection =
                        self.execution_transport != GitExecutionTransport::File;
                    if requires_connection
                        && !endpoint.events().iter().any(|event| {
                            event.outcome() == ResolverExecutionEndpointOutcome::Connected
                        })
                    {
                        return Err(SourceResolveError::GitExecutionBoundaryInvalid {
                            message: "successful remote Git command did not traverse its compiler-owned endpoint route"
                                .to_owned(),
                        });
                    }
                }
                (None, None) => {}
                _ => {
                    return Err(SourceResolveError::GitExecutionBoundaryInvalid {
                        message:
                            "native endpoint activity is not joined to its sealed route policy"
                                .to_owned(),
                    });
                }
            }
        }
        Ok(())
    }

    pub(in crate::resolution::source) fn captured_output_observation(
        &self,
    ) -> Result<GitCapturedOutputObservation, SourceResolveError> {
        let expected = git_captured_output_observation(
            &self.command_execution_observations.borrow(),
            self.captured_output_budget.ceiling,
        )?;
        if expected.observed != self.captured_output_budget.observed() {
            return Err(SourceResolveError::GitExecutionBoundaryInvalid {
                message: "Git captured-output counter does not match retained command outcomes"
                    .to_owned(),
            });
        }
        Ok(expected)
    }

    pub(in crate::resolution::source) fn network_transfer_observation(
        &self,
    ) -> Result<GitNetworkTransferObservation, SourceResolveError> {
        let expected = git_network_transfer_observation(
            &self.execution_policy_observations.borrow(),
            &self.command_execution_observations.borrow(),
            self.network_transfer_budget.ceiling(),
        )?;
        if expected.observed() != self.network_transfer_budget.observed() {
            return Err(SourceResolveError::GitExecutionBoundaryInvalid {
                message: "Git network-transfer counter does not match retained endpoint outcomes"
                    .to_owned(),
            });
        }
        Ok(expected)
    }

    pub(in crate::resolution::source) fn record_command_execution(
        &self,
        phase: ResolverExecutionPhase,
        command_identity: String,
        output: &BoundedCommandOutput,
        endpoint_observation: Option<ResolverExecutionEndpointObservation>,
    ) -> Result<(), SourceResolveError> {
        let policy_identity = {
            let policies = self.execution_policy_observations.borrow();
            let index = self.command_execution_observations.borrow().len();
            let policy = policies.get(index).ok_or_else(|| {
                SourceResolveError::GitExecutionBoundaryInvalid {
                    message: "native command completed without a matching policy observation"
                        .to_owned(),
                }
            })?;
            if policy.phase() != phase {
                return Err(SourceResolveError::GitExecutionBoundaryInvalid {
                    message: "native command phase does not match its policy observation"
                        .to_owned(),
                });
            }
            format_sha256(&Sha256::digest(policy.canonical_bytes()))
        };
        #[cfg(unix)]
        let termination_signal = {
            use std::os::unix::process::ExitStatusExt;
            output.status.signal()
        };
        #[cfg(not(unix))]
        let termination_signal = None;
        self.command_execution_observations
            .borrow_mut()
            .push(GitCommandExecutionObservation {
                phase,
                policy_identity,
                command_identity,
                status_code: output.status.code(),
                termination_signal,
                stdout_length: output.stdout.len() as u64,
                stdout_identity: format_sha256(&Sha256::digest(&output.stdout)),
                stderr_length: output.stderr.len() as u64,
                stderr_identity: format_sha256(&Sha256::digest(&output.stderr)),
                endpoint_observation,
            });
        Ok(())
    }

    pub(in crate::resolution::source) fn resolver_connect_helper(
        &self,
    ) -> Option<&GitTransportExecutableObservation> {
        (self.execution_transport == GitExecutionTransport::Ssh)
            .then(|| self.execution_helpers.last())
            .flatten()
    }

    pub(in crate::resolution::source) fn begin_launch(
        &self,
    ) -> Result<Duration, SourceResolveError> {
        self.verify_budget()?;
        let launches = self.launches.get();
        if launches >= self.maximum_launches {
            return Err(SourceResolveError::GitResolutionCommandLimit {
                limit: self.maximum_launches,
            });
        }
        self.launches.set(launches + 1);
        Ok(GIT_COMMAND_TIMEOUT.min(self.remaining_time()?))
    }

    pub(in crate::resolution::source) fn verify_budget(&self) -> Result<(), SourceResolveError> {
        self.remaining_time().map(|_| ())
    }

    pub(in crate::resolution::source) fn remaining_time(
        &self,
    ) -> Result<Duration, SourceResolveError> {
        let elapsed = self.started.elapsed();
        if elapsed >= self.timeout {
            Err(SourceResolveError::GitResolutionTimedOut {
                timeout_millis: duration_millis(self.timeout),
            })
        } else {
            Ok(self.timeout - elapsed)
        }
    }
}

#[cfg(test)]
pub(in crate::resolution::source) fn test_file_network_endpoint()
-> ResolverExecutionRequestedEndpoint {
    ResolverExecutionRequestedEndpoint::new("127.0.0.1", 9)
        .expect("the fixed test endpoint is valid")
}

#[cfg(test)]
pub(in crate::resolution::source) fn test_system_git_executor(
    transport: GitExecutionTransport,
) -> Result<GitExecutor, SourceResolveError> {
    GitExecutor::system(
        transport,
        test_file_network_endpoint(),
        LocalSourceLimits::default(),
    )
}

#[cfg(target_os = "macos")]
fn open_resolver_execution_helpers(
    execution_transport: GitExecutionTransport,
) -> Result<Vec<GitTransportExecutableObservation>, SourceResolveError> {
    let mut paths = match execution_transport {
        GitExecutionTransport::Ssh => vec![PathBuf::from("/bin/sh"), PathBuf::from("/bin/bash")],
        GitExecutionTransport::Https => Vec::new(),
        #[cfg(test)]
        GitExecutionTransport::File => [
            "/bin/sh",
            "/bin/bash",
            "/bin/mv",
            "/bin/sleep",
            "/usr/bin/git-upload-pack",
        ]
        .into_iter()
        .map(PathBuf::from)
        .collect(),
    };
    if execution_transport == GitExecutionTransport::Ssh {
        paths.push(resolver_connect_helper_path()?);
    }
    paths
        .iter()
        .map(|path| open_git_transport_executable(path))
        .collect()
}

#[cfg(not(target_os = "macos"))]
fn open_resolver_execution_helpers(
    execution_transport: GitExecutionTransport,
) -> Result<Vec<GitTransportExecutableObservation>, SourceResolveError> {
    if execution_transport != GitExecutionTransport::Ssh {
        return Ok(Vec::new());
    }
    [resolver_connect_helper_path()?]
        .iter()
        .map(|path| open_git_transport_executable(path))
        .collect()
}

pub(in crate::resolution::source) fn resolver_connect_helper_path()
-> Result<PathBuf, SourceResolveError> {
    let current_executable = std::env::current_exe().map_err(|error| {
        SourceResolveError::GitExecutionBoundaryInvalid {
            message: format!("cannot locate the Omega resolver CONNECT helper: {error}"),
        }
    })?;
    let executable_directory = current_executable.parent().ok_or_else(|| {
        SourceResolveError::GitExecutionBoundaryInvalid {
            message: "the running Omega executable has no installation directory".to_owned(),
        }
    })?;
    let helper_name = if cfg!(windows) {
        format!("{RESOLVER_CONNECT_HELPER_BASENAME}.exe")
    } else {
        RESOLVER_CONNECT_HELPER_BASENAME.to_owned()
    };
    let sibling = executable_directory.join(&helper_name);
    if sibling.is_file() {
        return Ok(sibling);
    }
    #[cfg(test)]
    {
        if executable_directory.file_name() == Some(OsStr::new("deps")) {
            let cargo_sibling = executable_directory
                .parent()
                .expect("Cargo deps directory has a target-profile parent")
                .join(&helper_name);
            if cargo_sibling.is_file() {
                return Ok(cargo_sibling);
            }
        }
        #[cfg(unix)]
        return Ok(PathBuf::from("/usr/bin/true"));
        #[cfg(windows)]
        return Ok(PathBuf::from(r"C:\Windows\System32\where.exe"));
    }
    #[cfg(not(test))]
    Err(SourceResolveError::GitExecutionBoundaryInvalid {
        message: format!(
            "compiler-owned resolver CONNECT helper is missing at {}",
            sibling.display()
        ),
    })
}

fn open_ssh_transport_executable(
    git_executable: &Path,
) -> Result<GitTransportExecutableObservation, SourceResolveError> {
    let requested_path = ssh_transport_executable_path(git_executable);
    let mut observation = open_git_transport_executable(&requested_path)?;
    // SSH is supplied through `GIT_SSH_COMMAND`, so invoke the already
    // authenticated canonical target directly rather than retaining an alias.
    observation.identity.invocation_path = observation.identity.path.clone();
    Ok(observation)
}

pub(in crate::resolution::source) fn open_https_transport_executable(
    git_executable: &Path,
) -> Result<GitTransportExecutableObservation, SourceResolveError> {
    let candidates = https_transport_executable_candidates(git_executable);
    for requested_path in &candidates {
        match std::fs::symlink_metadata(requested_path) {
            Ok(_) => return open_git_transport_executable(requested_path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(SourceResolveError::GitExecutableInvalid {
                    path: requested_path.clone(),
                    message: format!("HTTPS transport executable is unavailable: {error}"),
                });
            }
        }
    }
    Err(SourceResolveError::GitExecutableInvalid {
        path: git_executable.to_path_buf(),
        message: format!(
            "HTTPS transport executable is unavailable at the closed install-relative candidates: {}",
            candidates
                .iter()
                .map(|candidate| candidate.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    })
}

pub(in crate::resolution::source) fn open_git_transport_executable(
    requested_path: &Path,
) -> Result<GitTransportExecutableObservation, SourceResolveError> {
    if !requested_path.is_absolute() {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: requested_path.to_path_buf(),
            message: "transport executable path is not absolute".to_owned(),
        });
    }
    let canonical = requested_path.canonicalize().map_err(|error| {
        SourceResolveError::GitExecutableInvalid {
            path: requested_path.to_path_buf(),
            message: format!("transport executable is unavailable: {error}"),
        }
    })?;
    verify_git_transport_invocation_path(requested_path, &canonical)?;
    verify_git_executable_custody(&canonical)?;
    let metadata_identity = observe_git_executable_metadata(&canonical)?;
    let content_identity = hash_git_executable(&canonical)?;
    if observe_git_executable_metadata(&canonical)? != metadata_identity {
        return Err(SourceResolveError::GitExecutableChanged { path: canonical });
    }
    Ok(GitTransportExecutableObservation {
        identity: GitTransportExecutableIdentity {
            invocation_path: requested_path.to_path_buf(),
            path: canonical,
            content_identity,
        },
        metadata_identity,
    })
}

pub(in crate::resolution::source) fn verify_git_transport_executable(
    executable: &GitTransportExecutableObservation,
) -> Result<(), SourceResolveError> {
    verify_git_transport_invocation_path(
        &executable.identity.invocation_path,
        &executable.identity.path,
    )?;
    if observe_git_executable_metadata(&executable.identity.path)? != executable.metadata_identity {
        return Err(SourceResolveError::GitExecutableChanged {
            path: executable.identity.path.clone(),
        });
    }
    verify_git_executable_custody(&executable.identity.path)
}

fn verify_git_transport_invocation_path(
    invocation_path: &Path,
    expected_canonical: &Path,
) -> Result<(), SourceResolveError> {
    let metadata = std::fs::symlink_metadata(invocation_path).map_err(|error| {
        SourceResolveError::GitExecutableInvalid {
            path: invocation_path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    if !metadata.is_file() && !metadata.file_type().is_symlink() {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: invocation_path.to_path_buf(),
            message: "transport invocation path is not a regular file or symbolic link".to_owned(),
        });
    }
    verify_git_transport_invocation_node_custody(invocation_path, &metadata)?;
    let canonical = invocation_path.canonicalize().map_err(|error| {
        SourceResolveError::GitExecutableInvalid {
            path: invocation_path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    if canonical != expected_canonical {
        return Err(SourceResolveError::GitExecutableChanged {
            path: invocation_path.to_path_buf(),
        });
    }
    Ok(())
}

#[cfg(unix)]
fn verify_git_transport_invocation_node_custody(
    path: &Path,
    metadata: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    use std::os::unix::fs::MetadataExt;

    let effective_user = nix::unistd::Uid::effective().as_raw();
    if metadata.uid() != 0 && metadata.uid() != effective_user {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "transport invocation entry is owned by an unrelated user".to_owned(),
        });
    }
    if metadata.file_type().is_symlink() {
        verify_macos_path_extended_acl_custody(path, false)?;
    } else {
        verify_macos_open_executable_acl_custody(path, metadata)?;
    }
    Ok(())
}

#[cfg(windows)]
fn verify_git_transport_invocation_node_custody(
    path: &Path,
    metadata: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    verify_windows_executable_path_custody(path, metadata)
}

#[cfg(all(not(unix), not(windows)))]
fn verify_git_transport_invocation_node_custody(
    _path: &Path,
    _metadata: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(unix)]
fn verify_git_executable_custody(path: &Path) -> Result<(), SourceResolveError> {
    use std::os::unix::fs::MetadataExt;

    let effective_user = nix::unistd::Uid::effective().as_raw();
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "canonical resolver executable is not a concrete regular file".to_owned(),
        });
    }
    if metadata.uid() != 0 && metadata.uid() != effective_user {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message:
                "resolver executable is owned by neither root nor the resolver's effective user"
                    .to_owned(),
        });
    }
    let mode = metadata.mode();
    if mode & 0o022 != 0 {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "resolver executable is writable by group or other users".to_owned(),
        });
    }
    if mode & 0o6000 != 0 {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "resolver executable must not carry set-user-ID or set-group-ID authority"
                .to_owned(),
        });
    }
    if mode & 0o111 == 0 {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "resolver executable has no executable mode bit".to_owned(),
        });
    }
    verify_macos_open_executable_acl_custody(path, &metadata)?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn verify_macos_path_extended_acl_custody(
    path: &Path,
    follow_symbolic_link: bool,
) -> Result<(), SourceResolveError> {
    let symbolic_link_behavior = if follow_symbolic_link {
        omega_platform_custody::SymbolicLinkBehavior::Follow
    } else {
        omega_platform_custody::SymbolicLinkBehavior::InspectLink
    };
    let has_allow_entry =
        omega_platform_custody::extended_acl_has_allow_entry(path, symbolic_link_behavior)
            .map_err(|error| SourceResolveError::GitExecutableInvalid {
                path: path.to_path_buf(),
                message: format!(
                    "could not inspect resolver executable extended ACL custody: {error}"
                ),
            })?;
    if has_allow_entry {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "resolver executable custody contains an extended ACL allow entry".to_owned(),
        });
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub(in crate::resolution::source) fn verify_macos_open_executable_acl_custody(
    path: &Path,
    classified: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    let parent_path = path
        .parent()
        .ok_or_else(|| SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "resolver executable has no absolute custody parent".to_owned(),
        })?;
    let name = path
        .file_name()
        .ok_or_else(|| SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "resolver executable has no concrete filename".to_owned(),
        })?;
    let parent = open_absolute_directory_nofollow(parent_path).map_err(|error| {
        SourceResolveError::GitExecutableInvalid {
            path: parent_path.to_path_buf(),
            message: format!("could not retain resolver executable parent: {error}"),
        }
    })?;
    let mut options = CapabilityOpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let file = parent.open_with(name, &options).map_err(|error| {
        SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: format!("could not open resolver executable without following links: {error}"),
        }
    })?;
    let opened = file
        .metadata()
        .map_err(|error| SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: format!("could not inspect retained resolver executable: {error}"),
        })?;
    if !opened.is_file() || !same_std_and_capability_file_identity(classified, &opened) {
        return Err(SourceResolveError::GitExecutableChanged {
            path: path.to_path_buf(),
        });
    }
    verify_macos_open_executable_extended_acl_custody(path, &file.into_std())
}

#[cfg(target_os = "macos")]
fn verify_macos_open_executable_extended_acl_custody(
    path: &Path,
    file: &File,
) -> Result<(), SourceResolveError> {
    let has_allow_entry = omega_platform_custody::open_file_extended_acl_has_allow_entry(file)
        .map_err(|error| SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: format!(
                "could not inspect retained resolver executable extended ACL custody: {error}"
            ),
        })?;
    if has_allow_entry {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "resolver executable custody contains an extended ACL allow entry".to_owned(),
        });
    }
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn verify_macos_path_extended_acl_custody(
    _path: &Path,
    _follow_symbolic_link: bool,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
pub(in crate::resolution::source) fn verify_macos_open_executable_acl_custody(
    _path: &Path,
    _classified: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(windows)]
fn verify_git_executable_custody(path: &Path) -> Result<(), SourceResolveError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "canonical resolver executable is not a concrete regular file".to_owned(),
        });
    }
    verify_windows_executable_path_custody(path, &metadata)
}

#[cfg(windows)]
fn verify_windows_executable_path_custody(
    path: &Path,
    classified: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    let parent_path = path
        .parent()
        .ok_or_else(|| SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "resolver executable entry has no absolute custody parent".to_owned(),
        })?;
    let name = path
        .file_name()
        .ok_or_else(|| SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "resolver executable entry has no concrete filename".to_owned(),
        })?;
    let parent = open_absolute_directory_nofollow(parent_path).map_err(|error| {
        SourceResolveError::GitExecutableInvalid {
            path: parent_path.to_path_buf(),
            message: format!("could not retain resolver executable parent: {error}"),
        }
    })?;
    let mut options = CapabilityOpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let file = parent.open_with(name, &options).map_err(|error| {
        SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: format!(
                "could not retain resolver executable entry without following reparse points: {error}"
            ),
        }
    })?;
    let opened = file
        .metadata()
        .map_err(|error| SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: format!("could not inspect retained resolver executable entry: {error}"),
        })?;
    if !same_std_and_capability_file_identity(classified, &opened) {
        return Err(SourceResolveError::GitExecutableChanged {
            path: path.to_path_buf(),
        });
    }
    verify_windows_open_executable_custody(path, &file.into_std())
}

#[cfg(windows)]
fn verify_windows_open_executable_custody(
    path: &Path,
    file: &File,
) -> Result<(), SourceResolveError> {
    use omega_platform_custody::{
        WindowsFileCustodyViolation, WindowsFileOwnerPolicy, inspect_open_windows_file_custody,
    };

    let violation = inspect_open_windows_file_custody(
        file,
        WindowsFileOwnerPolicy::CurrentUserSystemOrAdministrators,
    )
    .map_err(|error| SourceResolveError::GitExecutableInvalid {
        path: path.to_path_buf(),
        message: format!("could not inspect retained Windows executable custody: {error}"),
    })?;
    if let Some(violation) = violation {
        let message = match violation {
            WindowsFileCustodyViolation::UntrustedOwner => {
                "resolver executable is owned by an untrusted Windows principal"
            }
            WindowsFileCustodyViolation::NullDacl => {
                "resolver executable has a null DACL granting unrestricted access"
            }
            WindowsFileCustodyViolation::UntrustedMutationAuthority => {
                "resolver executable grants mutation authority to an untrusted Windows principal"
            }
            WindowsFileCustodyViolation::UnsupportedAllowAce => {
                "resolver executable contains an unsupported access-allowing Windows ACE"
            }
        };
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: message.to_owned(),
        });
    }
    Ok(())
}

#[cfg(all(not(unix), not(windows)))]
fn verify_git_executable_custody(_path: &Path) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(target_os = "macos")]
pub(in crate::resolution::source) fn system_git_candidates() -> &'static [&'static str] {
    &[
        "/Library/Developer/CommandLineTools/usr/bin/git",
        "/Applications/Xcode.app/Contents/Developer/usr/bin/git",
        "/usr/local/bin/git",
        "/opt/homebrew/bin/git",
    ]
}

#[cfg(all(unix, not(target_os = "macos")))]
pub(in crate::resolution::source) fn system_git_candidates() -> &'static [&'static str] {
    &["/usr/bin/git", "/usr/local/bin/git"]
}

#[cfg(windows)]
pub(in crate::resolution::source) fn system_git_candidates() -> &'static [&'static str] {
    &[
        r"C:\Program Files\Git\cmd\git.exe",
        r"C:\Program Files\Git\bin\git.exe",
        r"C:\Program Files (x86)\Git\cmd\git.exe",
    ]
}

#[cfg(unix)]
fn https_transport_executable_candidates(git_executable: &Path) -> Vec<PathBuf> {
    let Some(installation_root) = git_executable.parent().and_then(Path::parent) else {
        return Vec::new();
    };
    vec![
        installation_root.join("libexec/git-core/git-remote-https"),
        installation_root.join("lib/git-core/git-remote-https"),
    ]
}

#[cfg(windows)]
fn https_transport_executable_candidates(git_executable: &Path) -> Vec<PathBuf> {
    let Some(installation_root) = git_executable.parent().and_then(Path::parent) else {
        return Vec::new();
    };
    vec![installation_root.join("mingw64/libexec/git-core/git-remote-https.exe")]
}

#[cfg(unix)]
fn ssh_transport_executable_path(_git_executable: &Path) -> PathBuf {
    PathBuf::from("/usr/bin/ssh")
}

#[cfg(windows)]
fn ssh_transport_executable_path(git_executable: &Path) -> PathBuf {
    git_executable
        .parent()
        .and_then(Path::parent)
        .map(|root| root.join("usr/bin/ssh.exe"))
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files\Git\usr\bin\ssh.exe"))
}

fn hash_git_executable(path: &Path) -> Result<String, SourceResolveError> {
    let metadata =
        std::fs::metadata(path).map_err(|error| SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    if !metadata.is_file() {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "path is not a regular file".to_owned(),
        });
    }
    if metadata.len() > GIT_EXECUTABLE_BYTE_LIMIT {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: format!(
                "file exceeds the {GIT_EXECUTABLE_BYTE_LIMIT}-byte executable ceiling"
            ),
        });
    }
    let mut file = File::open(path).map_err(|error| SourceResolveError::GitExecutableInvalid {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let mut hasher = Sha256::new();
    let mut observed = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count =
            file.read(&mut buffer)
                .map_err(|error| SourceResolveError::GitExecutableInvalid {
                    path: path.to_path_buf(),
                    message: error.to_string(),
                })?;
        if count == 0 {
            break;
        }
        observed = observed.saturating_add(count as u64);
        if observed > GIT_EXECUTABLE_BYTE_LIMIT {
            return Err(SourceResolveError::GitExecutableInvalid {
                path: path.to_path_buf(),
                message: format!(
                    "file exceeds the {GIT_EXECUTABLE_BYTE_LIMIT}-byte executable ceiling"
                ),
            });
        }
        hasher.update(&buffer[..count]);
    }
    if observed != metadata.len() {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "file length changed while it was hashed".to_owned(),
        });
    }
    Ok(format_sha256(&hasher.finalize()))
}

fn observe_git_executable_metadata(
    path: &Path,
) -> Result<GitExecutableMetadataIdentity, SourceResolveError> {
    let metadata =
        std::fs::metadata(path).map_err(|error| SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    if !metadata.is_file() {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "path is not a regular file".to_owned(),
        });
    }
    let modified =
        metadata
            .modified()
            .map_err(|error| SourceResolveError::GitExecutableInvalid {
                path: path.to_path_buf(),
                message: error.to_string(),
            })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        Ok(GitExecutableMetadataIdentity {
            length: metadata.len(),
            modified,
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
        })
    }
    #[cfg(windows)]
    {
        Ok(GitExecutableMetadataIdentity {
            length: metadata.len(),
            modified,
        })
    }
}
