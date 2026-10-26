//! Evaluated Build filesystem calls over one admitted captured-input scope.
//!
//! Guest shapes carry no authority. Original facets and activation-issued
//! opaque markers are authenticated before the scope performs an operation.

use evaluation_accounting::{EvaluationInvocationAccount, EvaluationStorageLease};
use filesystem_authority::FilesystemGrantRootIdentity;
use package_compilation::build_execution::{
    BUILD_OUTPUT_ROOT_IDENTITY, BUILD_SOURCE_ROOT_IDENTITY, BuildFilesystemEffects,
    BuildFilesystemRuntime, BuildMachineFilesystemScope,
};
use psi_compiler::{TerminalBuildFilesystemCall, TerminalBuildFilesystemOperation as Operation};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use std::sync::Mutex;
use terminal_interpreter::{
    TerminalBoundaryByteBuffer, TerminalEffect, TerminalEffectHandler, TerminalEffectRejection,
    TerminalEffectResult, TerminalImportAdapter, TerminalImportSignature, TerminalScalarValue,
    TerminalStructuralValue,
};

struct Marker {
    value: TerminalStructuralValue,
    kind: MarkerKind,
    _storage: EvaluationStorageLease,
}

enum MarkerKind {
    Source(FilesystemGrantRootIdentity),
    Path(FilesystemGrantRootIdentity, Vec<u8>),
    Obligation(usize),
    Receipt,
}

struct FilesystemActivation<'scope> {
    runtime: BuildFilesystemRuntime<'scope>,
    markers: Vec<Marker>,
}

struct FilesystemFacets {
    source: TerminalStructuralValue,
    inputs: TerminalStructuralValue,
    output: TerminalStructuralValue,
}

pub(crate) struct TerminalBuildFilesystemHost<'scope> {
    calls: &'scope [TerminalBuildFilesystemCall],
    facets: Option<FilesystemFacets>,
    account: &'scope EvaluationInvocationAccount,
    machine_name: &'scope str,
    activation: Mutex<Option<FilesystemActivation<'scope>>>,
}

impl<'scope> TerminalBuildFilesystemHost<'scope> {
    pub(crate) fn new(
        produced: &'scope psi_compiler::ProducedTerminalProgram,
        original: &TerminalStructuralValue,
        account: &'scope EvaluationInvocationAccount,
        scope: &'scope BuildMachineFilesystemScope,
        machine_name: &'scope str,
    ) -> Result<Self, String> {
        let artifact = produced.view();
        let calls = produced
            .build_filesystem()
            .calls_for(artifact)
            .ok_or("Build filesystem calls lost their invocation custody")?;
        let activation = if calls.is_empty() {
            None
        } else {
            Some(FilesystemActivation {
                runtime: scope.begin_filesystem_runtime(account.clone())?,
                markers: Vec::new(),
            })
        };
        Ok(Self {
            calls,
            facets: if calls.is_empty() {
                None
            } else {
                Some(FilesystemFacets {
                    source: crate::terminal_requests::build_facet(artifact, original, "source")?,
                    inputs: crate::terminal_requests::build_facet(artifact, original, "inputs")?,
                    output: crate::terminal_requests::build_facet(artifact, original, "output")?,
                })
            },
            account,
            machine_name,
            activation: Mutex::new(activation),
        })
    }

    pub(crate) fn adapter(
        &self,
        signature: TerminalImportSignature,
    ) -> FilesystemAdapter<'_, 'scope> {
        FilesystemAdapter {
            host: self,
            signature,
        }
    }

    pub(crate) fn take_effects(&self) -> Result<Option<BuildFilesystemEffects>, String> {
        let mut activation = self
            .activation
            .lock()
            .map_err(|_| "Build filesystem lock was poisoned")?;
        activation
            .take()
            .map(|activation| activation.runtime.finish())
            .transpose()
    }

    fn source_root(
        &self,
        markers: &[Marker],
        value: &TerminalStructuralValue,
    ) -> Result<FilesystemGrantRootIdentity, String> {
        if self
            .facets
            .as_ref()
            .is_some_and(|facets| value == &facets.source)
        {
            return Ok(BUILD_SOURCE_ROOT_IDENTITY);
        }
        match &marker(markers, value)?.kind {
            MarkerKind::Source(root) => Ok(*root),
            _ => Err("BuildSource operation requires an issued input facet".into()),
        }
    }
}

pub(crate) struct FilesystemAdapter<'host, 'scope> {
    host: &'host TerminalBuildFilesystemHost<'scope>,
    signature: TerminalImportSignature,
}

impl TerminalImportAdapter for FilesystemAdapter<'_, '_> {
    fn implemented_signature(&self) -> TerminalImportSignature {
        self.signature.clone()
    }
}

impl TerminalEffectHandler for FilesystemAdapter<'_, '_> {
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new(
            "Build filesystem requires result-aware dispatch",
        ))
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        self.dispatch(effect, &mut [])
            .map_err(TerminalEffectRejection::new)
    }

    fn handle_effect_with_byte_buffers(
        &mut self,
        effect: &TerminalEffect,
        buffers: &mut [TerminalBoundaryByteBuffer],
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        self.dispatch(effect, buffers)
            .map_err(TerminalEffectRejection::new)
    }
}

impl FilesystemAdapter<'_, '_> {
    fn dispatch(
        &self,
        effect: &TerminalEffect,
        buffers: &mut [TerminalBoundaryByteBuffer],
    ) -> Result<TerminalEffectResult, String> {
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
            return Err("not a Build filesystem boundary call".into());
        };
        let action = self
            .host
            .calls
            .iter()
            .find(|call| call.operation == *operation && call.boundary == *boundary)
            .map(|call| call.action)
            .ok_or("filesystem call has no source-issued protocol custody")?;
        if !buffers.is_empty() && action != Operation::Read {
            return Err("unexpected mutable filesystem operand".into());
        }
        let structural = |position: usize| {
            structural_arguments
                .get(position)
                .ok_or_else(|| "missing structural filesystem operand".to_string())
        };
        let bytes = |position: usize| {
            byte_sequence_arguments
                .get(position)
                .and_then(Option::as_deref)
                .ok_or_else(|| "missing filesystem byte operand".to_string())
        };
        let integer = |position: usize| match arguments.get(position) {
            Some(TerminalScalarValue::Integer { value, .. }) => Ok(*value),
            _ => Err("missing filesystem integer operand".to_string()),
        };
        let descriptor = || -> Result<i32, String> {
            signed(integer(0)?).and_then(|value| {
                i32::try_from(value).map_err(|_| "filesystem descriptor exceeds i32".into())
            })
        };
        let mut guard = self
            .host
            .activation
            .lock()
            .map_err(|_| "Build filesystem lock was poisoned")?;
        let activation = guard
            .as_mut()
            .ok_or("Build filesystem activation is absent or already settled")?;
        let FilesystemActivation { runtime, markers } = activation;
        let facets = self
            .host
            .facets
            .as_ref()
            .ok_or("Build filesystem facets were not admitted")?;
        let output = || -> Result<(), String> {
            if structural(0)? == &facets.output {
                Ok(())
            } else {
                Err(
                    "BuildOutput operation requires the original compiler-issued output facet"
                        .into(),
                )
            }
        };
        let path = |position| -> Result<(FilesystemGrantRootIdentity, &[u8]), String> {
            match &marker(markers, structural(position)?)?.kind {
                MarkerKind::Path(root, path) => Ok((*root, path)),
                _ => Err("filesystem operation requires an issued BuildPath".into()),
            }
        };
        let obligation = |position| -> Result<usize, String> {
            match marker(markers, structural(position)?)?.kind {
                MarkerKind::Obligation(obligation) => Ok(obligation),
                _ => Err("filesystem operation requires an issued RequiredOutput".into()),
            }
        };
        match action {
            Operation::ResolveSource | Operation::ResolveOutput => {
                let root = if action == Operation::ResolveSource {
                    self.host.source_root(markers, structural(0)?)?
                } else {
                    output()?;
                    BUILD_OUTPUT_ROOT_IDENTITY
                };
                let path = bytes(1)?;
                runtime.resolve(root, path)?;
                mint(markers, self.host.account, result, path.len(), || {
                    let mut retained = Vec::new();
                    retained
                        .try_reserve_exact(path.len())
                        .map_err(|_| "filesystem marker allocation was refused")?;
                    retained.extend_from_slice(path);
                    Ok(MarkerKind::Path(root, retained))
                })
            }
            Operation::NamedInput => {
                if structural(0)? != &facets.inputs {
                    return Err(
                        "named input lookup requires the original compiler-issued inputs facet"
                            .into(),
                    );
                }
                let root = runtime.named_input(bytes(1)?)?;
                mint(markers, self.host.account, result, 0, || {
                    Ok(MarkerKind::Source(root))
                })
            }
            Operation::Open => {
                let root = self.host.source_root(markers, structural(0)?)?;
                let (path_root, relative) = path(1)?;
                if root != path_root {
                    return Err("BuildPath belongs to another input root".into());
                }
                integer_result(i64::from(runtime.open(root, relative, descriptor()?)?), 32)
            }
            Operation::Read => {
                let root = self.host.source_root(markers, structural(0)?)?;
                let count = unsigned(integer(1)?)?;
                let [buffer] = buffers else {
                    return Err("Build read requires one bounded mutable byte buffer".into());
                };
                if buffer.argument_index() != 1 {
                    return Err("Build read buffer is not its declared argument".into());
                }
                integer_result(
                    runtime.read(root, descriptor()?, buffer.bytes_mut(), count)?,
                    64,
                )
            }
            Operation::CloseSource => {
                let root = self.host.source_root(markers, structural(0)?)?;
                integer_result(i64::from(runtime.close(root, descriptor()?)?), 32)
            }
            Operation::Create => {
                output()?;
                let (root, relative) = path(1)?;
                if root != BUILD_OUTPUT_ROOT_IDENTITY {
                    return Err("BuildOutput cannot create an input path".into());
                }
                integer_result(i64::from(runtime.create(relative, descriptor()?)?), 32)
            }
            Operation::Write => {
                output()?;
                integer_result(runtime.write(descriptor()?, bytes(1)?)?, 64)
            }
            Operation::CloseOutput => {
                output()?;
                integer_result(
                    i64::from(runtime.close(BUILD_OUTPUT_ROOT_IDENTITY, descriptor()?)?),
                    32,
                )
            }
            Operation::IncludeSource => {
                output()?;
                let (root, relative) = path(1)?;
                if root != BUILD_OUTPUT_ROOT_IDENTITY {
                    return Err("generated source is not an Output path".into());
                }
                runtime.include_source(relative)?;
                Ok(TerminalEffectResult::Unit)
            }
            Operation::Require => {
                output()?;
                let obligation = runtime.require(bytes(1)?)?;
                mint(markers, self.host.account, result, 0, || {
                    Ok(MarkerKind::Obligation(obligation))
                })
            }
            Operation::CanComplete | Operation::Complete => {
                output()?;
                let obligation = obligation(1)?;
                let (root, relative) = path(2)?;
                if root != BUILD_OUTPUT_ROOT_IDENTITY {
                    return Err("required output settlement needs an Output path".into());
                }
                if action == Operation::CanComplete {
                    Ok(TerminalEffectResult::Scalar(TerminalScalarValue::Boolean(
                        runtime.can_complete(obligation, relative)?,
                    )))
                } else {
                    runtime.complete_output(obligation, relative)?;
                    mint(markers, self.host.account, result, 0, || {
                        Ok(MarkerKind::Receipt)
                    })
                }
            }
            Operation::Fail => {
                output()?;
                runtime.fail_output(obligation(1)?, bytes(2)?, self.host.machine_name)?;
                Ok(TerminalEffectResult::Unit)
            }
            Operation::RequiredPath => {
                Err("Terminal Build required-output path byte results are not installed".into())
            }
        }
    }
}

fn marker<'marker>(
    markers: &'marker [Marker],
    value: &TerminalStructuralValue,
) -> Result<&'marker Marker, String> {
    markers
        .iter()
        .find(|marker| &marker.value == value)
        .ok_or_else(|| "Build filesystem value was not issued by this activation".into())
}

fn mint(
    markers: &mut Vec<Marker>,
    account: &EvaluationInvocationAccount,
    result: &terminal_psi::BoundaryMachineResult,
    bytes: usize,
    kind: impl FnOnce() -> Result<MarkerKind, String>,
) -> Result<TerminalEffectResult, String> {
    let terminal_psi::BoundaryMachineResult::Structural(result) = result else {
        return Err("filesystem marker result is not structural".into());
    };
    let storage = account
        .reserve_site(
            0,
            1,
            u64::try_from(bytes).map_err(|_| "filesystem marker size overflowed")?,
        )
        .map_err(|error| error.to_string())?;
    markers
        .try_reserve(1)
        .map_err(|_| "cannot retain issued filesystem marker")?;
    let identity = u64::MAX
        .checked_sub(u64::try_from(markers.len()).map_err(|_| "filesystem marker space exhausted")?)
        .filter(|identity| *identity > 1)
        .ok_or("filesystem marker space exhausted")?;
    let value = TerminalStructuralValue::minted(identity, result.structural_type, Vec::new());
    markers.push(Marker {
        value: value.clone(),
        kind: kind()?,
        _storage: storage,
    });
    Ok(TerminalEffectResult::Structural(value))
}

fn signed(value: IntegerValue) -> Result<i64, String> {
    match value {
        IntegerValue::Signed(value) => {
            i64::try_from(value).map_err(|_| "filesystem integer exceeds i64".into())
        }
        _ => Err("filesystem operand is not signed".into()),
    }
}

fn unsigned(value: IntegerValue) -> Result<u64, String> {
    match value {
        IntegerValue::Unsigned(value) => {
            u64::try_from(value).map_err(|_| "filesystem count exceeds u64".into())
        }
        _ => Err("filesystem count is not unsigned".into()),
    }
}

fn integer_result(value: i64, width: u16) -> Result<TerminalEffectResult, String> {
    Ok(TerminalEffectResult::Scalar(TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Signed, width)
            .map_err(|error| format!("filesystem result type: {error:?}"))?,
        value: IntegerValue::Signed(i128::from(value)),
    }))
}
