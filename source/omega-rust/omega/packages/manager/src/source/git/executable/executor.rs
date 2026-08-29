//! Git executor lifecycle and joined policy/result observations.

use super::budget::GitCapturedOutputBudget;
use super::custody::verify_git_executable_custody;
use super::identity::{
    GitExecutableMetadataIdentity, hash_git_executable, observe_git_executable_metadata,
};
use super::selection::{
    GitTransportExecutableObservation, open_https_transport_executable,
    open_resolver_execution_helpers, open_ssh_transport_executable, system_git_candidates,
    verify_git_transport_executable,
};
use crate::source::SourceResolveError;
use crate::source::git::request::GitExecutionTransport;
use crate::source::git::{BoundedCommandOutput, duration_millis, format_sha256};
use crate::source::limits::{
    GIT_COMMAND_TIMEOUT, GIT_FIXED_COMMAND_ALLOWANCE, GIT_RESOLUTION_TIMEOUT, LocalSourceLimits,
};
use crate::source::observations::{
    GitCapturedOutputObservation, GitCommandExecutionObservation, GitExecutableIdentity,
    GitNetworkTransferObservation, git_captured_output_observation,
    git_network_transfer_observation, git_resolution_captured_output_ceiling,
    git_resolution_network_transfer_ceiling,
};
use omega_resolver_execution::{
    ResolverExecutionBackend, ResolverExecutionEndpointObservation,
    ResolverExecutionEndpointOutcome, ResolverExecutionPhase, ResolverExecutionPolicyObservation,
    ResolverExecutionRequestedEndpoint, ResolverExecutionTransferBudget,
};
use sha2::{Digest, Sha256};
use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub(in crate::source) struct GitExecutor {
    pub(in crate::source) identity: GitExecutableIdentity,
    pub(in crate::source) metadata_identity: GitExecutableMetadataIdentity,
    pub(in crate::source) transport_executable: Option<GitTransportExecutableObservation>,
    pub(in crate::source) execution_helpers: Vec<GitTransportExecutableObservation>,
    pub(in crate::source) execution_transport: GitExecutionTransport,
    pub(in crate::source) requested_network_endpoint: ResolverExecutionRequestedEndpoint,
    pub(in crate::source) started: Instant,
    pub(in crate::source) timeout: Duration,
    pub(in crate::source) launches: Cell<usize>,
    pub(in crate::source) execution_policy_observations:
        RefCell<Vec<ResolverExecutionPolicyObservation>>,
    pub(in crate::source) command_execution_observations:
        RefCell<Vec<GitCommandExecutionObservation>>,
    pub(in crate::source) captured_output_budget: GitCapturedOutputBudget,
    pub(in crate::source) network_transfer_budget: ResolverExecutionTransferBudget,
    pub(in crate::source) maximum_launches: usize,
    pub(in crate::source) execution_backend: ResolverExecutionBackend,
}

impl GitExecutor {
    pub(in crate::source) fn system(
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
    pub(in crate::source) fn open(path: &Path) -> Result<Self, SourceResolveError> {
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
    pub(in crate::source) fn open_with_budget(
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
    pub(in crate::source) fn open_with_resource_budgets(
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

    pub(in crate::source) fn open_with_budget_for_transport(
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
        execution_backend
            .require_package_resolution_floor()
            .map_err(|error| SourceResolveError::GitExecutionBoundaryInvalid {
                message: error.to_string(),
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

    pub(in crate::source) fn verify(&self) -> Result<(), SourceResolveError> {
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

    pub(in crate::source) fn verify_content(&self) -> Result<(), SourceResolveError> {
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

    pub(in crate::source) fn validate_execution_policy_observations(
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
                || command.completion.policy() != policy
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

    pub(in crate::source) fn captured_output_observation(
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

    pub(in crate::source) fn network_transfer_observation(
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

    pub(in crate::source) fn record_command_execution(
        &self,
        phase: ResolverExecutionPhase,
        command_identity: String,
        output: &BoundedCommandOutput,
        endpoint_observation: Option<ResolverExecutionEndpointObservation>,
    ) -> Result<(), SourceResolveError> {
        let completion = &output.completion;
        let policy = completion.policy();
        if policy.phase() != phase {
            return Err(SourceResolveError::GitExecutionBoundaryInvalid {
                message: "native command phase does not match its completed policy observation"
                    .to_owned(),
            });
        }
        #[cfg(unix)]
        let termination_signal = {
            use std::os::unix::process::ExitStatusExt;
            output.status.signal()
        };
        #[cfg(not(unix))]
        let termination_signal = None;
        if completion.status().success() != output.status.success()
            || completion.status().code() != output.status.code()
            || completion.status().unix_signal() != termination_signal
        {
            return Err(SourceResolveError::GitExecutionBoundaryInvalid {
                message: "captured Git status does not match resolver completion".to_owned(),
            });
        }
        let policy_identity = format_sha256(&Sha256::digest(policy.canonical_bytes()));
        let observation = GitCommandExecutionObservation {
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
            completion: completion.clone(),
        };
        self.execution_policy_observations
            .borrow_mut()
            .push(policy.clone());
        self.command_execution_observations
            .borrow_mut()
            .push(observation);
        Ok(())
    }

    pub(in crate::source) fn resolver_connect_helper(
        &self,
    ) -> Option<&GitTransportExecutableObservation> {
        (self.execution_transport == GitExecutionTransport::Ssh)
            .then(|| self.execution_helpers.last())
            .flatten()
    }

    pub(in crate::source) fn begin_launch(&self) -> Result<Duration, SourceResolveError> {
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

    pub(in crate::source) fn verify_budget(&self) -> Result<(), SourceResolveError> {
        self.remaining_time().map(|_| ())
    }

    pub(in crate::source) fn remaining_time(&self) -> Result<Duration, SourceResolveError> {
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
pub(in crate::source) fn test_file_network_endpoint() -> ResolverExecutionRequestedEndpoint {
    ResolverExecutionRequestedEndpoint::new("127.0.0.1", 9)
        .expect("the fixed test endpoint is valid")
}

#[cfg(test)]
pub(in crate::source) fn test_system_git_executor(
    transport: GitExecutionTransport,
) -> Result<GitExecutor, SourceResolveError> {
    GitExecutor::system(
        transport,
        test_file_network_endpoint(),
        LocalSourceLimits::default(),
    )
}
