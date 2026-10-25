//! Receive evaluated project requests from one admitted Terminal Build.
//!
//! The producer authenticates protocol declarations and exact call sites; the
//! activation authenticates its original receiver. Arguments are ordinary guest
//! values. The coordinator drains requests between execution steps, starts
//! dependency work, and owns the final role/alias/dependency join. Draining does
//! not release payload accounting: each request carries its reservation.

use build_declarations::{
    BuildDependencySource, BuildRequest, DependencyPurpose, ProjectName, WorkspaceMemberPath,
};
use evaluation_accounting::{EvaluationInvocationAccount, EvaluationStorageLease};
use psi_compiler::{
    TerminalBuildRequestCall, TerminalBuildRequestOperation as Operation, TerminalBuildRequests,
};
use std::sync::Mutex;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult,
    TerminalImportAdapter, TerminalImportSignature, TerminalScalarValue, TerminalStructuralValue,
};

pub struct IssuedBuildRequest {
    request: BuildRequest,
    _storage: Option<EvaluationStorageLease>,
}

impl IssuedBuildRequest {
    pub fn request(&self) -> &BuildRequest {
        &self.request
    }
}

pub struct TerminalBuildRequestHost<'context> {
    calls: &'context [TerminalBuildRequestCall],
    original: TerminalStructuralValue,
    account: Option<&'context EvaluationInvocationAccount>,
    requests: Mutex<Vec<IssuedBuildRequest>>,
}

impl<'context> TerminalBuildRequestHost<'context> {
    pub fn new(
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        requests: &'context TerminalBuildRequests,
        original: TerminalStructuralValue,
        account: Option<&'context EvaluationInvocationAccount>,
    ) -> Result<Self, String> {
        let calls = requests
            .calls_for(artifact)
            .ok_or("Build requests belong to another Terminal artifact")?;
        Ok(Self {
            calls,
            original,
            account,
            requests: Mutex::new(Vec::new()),
        })
    }

    pub fn adapter(
        &self,
        signature: TerminalImportSignature,
    ) -> TerminalBuildRequestAdapter<'_, 'context> {
        TerminalBuildRequestAdapter {
            host: self,
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
}

pub struct TerminalBuildRequestAdapter<'host, 'context> {
    host: &'host TerminalBuildRequestHost<'context>,
    signature: TerminalImportSignature,
}

impl TerminalImportAdapter for TerminalBuildRequestAdapter<'_, '_> {
    fn implemented_signature(&self) -> TerminalImportSignature {
        self.signature.clone()
    }
}

impl TerminalEffectHandler for TerminalBuildRequestAdapter<'_, '_> {
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new(
            "Build requests require result-aware dispatch",
        ))
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let refuse = TerminalEffectRejection::new;
        let TerminalEffect::BoundaryCall {
            operation,
            boundary,
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
            .find(|call| call.operation == *operation && call.boundary == *boundary)
            .ok_or_else(|| refuse("Build request has no source-issued protocol call"))?;
        if structural_arguments.first() != Some(&self.host.original)
            || byte_sequence_arguments.first() != Some(&None)
            || structural_arguments.len() != byte_sequence_arguments.len()
            || *result != terminal_psi::BoundaryMachineResult::Unit
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
        // Validate borrowed inputs before reserving or copying. The source's
        // ordinary helper already decomposed Path/Git and package selection.
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
        requests.push(IssuedBuildRequest {
            request,
            _storage: storage,
        });
        Ok(TerminalEffectResult::Unit)
    }
}
