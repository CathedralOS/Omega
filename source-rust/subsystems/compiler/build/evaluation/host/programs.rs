//! Resolve invoker selectors against reviewed data declarations before Build
//! entry. An effect presents its checked call site and that declaration's
//! coordinate; guest strings can never select an image.
use build_program_launch::{ReviewedProgram, RunStatus};
use package_compilation::build_execution::{
    RestrictedBuildBounds, RestrictedBuildRequest, RestrictedProgramLaunch,
};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Path, IntegerSign, IntegerType, IntegerValue,
    StructuralCaseId, StructuralFieldId, StructuralTypeId,
};
use source_text::SourceSpan;
use std::{path::PathBuf, time::Duration};
use terminal_interpreter::{
    TerminalAggregateValue, TerminalBoundaryStorageBuffer, TerminalEffectResult,
    TerminalScalarValue, TerminalStorageContents,
};

mod admission;

struct ProgramBinding {
    source: SourceSpan,
    image: ReviewedProgram,
    meaning: RestrictedProgramLaunch,
}

#[derive(Default)]
pub(crate) struct ProgramInstallation {
    worker: Option<PathBuf>,
    bindings: Vec<ProgramBinding>,
    runtime_cases: Option<ResultShape>,
}

impl ProgramInstallation {
    pub(crate) fn requests(
        &self,
        bounds: &RestrictedBuildBounds,
        profile: Option<target::TargetProfile>,
    ) -> impl Iterator<Item = RestrictedBuildRequest> {
        self.bindings.iter().map(move |binding| {
            RestrictedBuildRequest::for_program(binding.meaning.clone(), bounds.clone(), profile)
        })
    }

    pub(crate) fn uses_files(&self) -> bool {
        self.bindings.iter().any(|binding| {
            !binding.meaning.readable.is_empty() || !binding.meaning.writable.is_empty()
        })
    }
}

#[derive(PartialEq, Eq)]
struct ResultShape {
    id: StructuralTypeId,
    exited: StructuralCaseId,
    status: StructuralFieldId,
    failed: StructuralCaseId,
    abnormal: StructuralCaseId,
    timeout: StructuralCaseId,
}

impl ResultShape {
    fn new(module: &terminal_psi::TerminalModule, id: StructuralTypeId) -> Result<Self, String> {
        let decl = module
            .types
            .structural_types
            .iter()
            .find(|r| r.id == id)
            .ok_or("lost program result type")?;
        let terminal_psi::StructuralTypeShape::Sum { cases } = &decl.shape else {
            return Err("launcher result must be a sum".into());
        };
        let case = |name: &str| {
            cases
                .iter()
                .find(|c| module.identities.text(c.identity) == Some(name))
                .ok_or_else(|| format!("missing launcher result case `{name}`"))
        };
        let exited = case("Exited")?;
        let [status] = exited.fields.as_slice() else {
            return Err("missing launcher status field".into());
        };
        Ok(Self {
            id,
            exited: exited.id,
            status: status.id,
            failed: case("StartFailed")?.id,
            abnormal: case("AbnormalExit")?.id,
            timeout: case("TimedOut")?.id,
        })
    }

    fn finish(&self, outcome: RunStatus) -> Result<TerminalEffectResult, String> {
        let (case, fields) = match outcome {
            RunStatus::StartFailed => (self.failed, vec![]),
            RunStatus::AbnormalExit => (self.abnormal, vec![]),
            RunStatus::TimedOut => (self.timeout, vec![]),
            RunStatus::Exited(code) => (
                self.exited,
                vec![(
                    vec![Path::Case(self.exited), Path::Field(self.status)],
                    TerminalScalarValue::Integer {
                        scalar_type: IntegerType::new(IntegerSign::Unsigned, 32)
                            .map_err(|e| format!("{e:?}"))?,
                        value: IntegerValue::Unsigned(u128::from(code)),
                    },
                )],
            ),
        };
        Ok(TerminalEffectResult::Aggregate(
            TerminalAggregateValue::from_scalars(self.id, Some(case), fields),
        ))
    }
}

impl ProgramInstallation {
    pub(super) fn run(
        &self,
        source: SourceSpan,
        storage: &[TerminalBoundaryStorageBuffer],
        filesystem: &super::filesystem::TerminalBuildFilesystemHost,
    ) -> Result<TerminalEffectResult, String> {
        let binding = self
            .bindings
            .iter()
            .find(|b| b.source == source)
            .ok_or("the requested program has no installed authority")?;
        let worker = self
            .worker
            .as_deref()
            .ok_or("the program supervisor is unavailable")?;
        let shape = self
            .runtime_cases
            .as_ref()
            .ok_or("lost the admitted launcher result type")?;
        let [TerminalBoundaryStorageBuffer::Structural(argument)] = storage else {
            return Err("launcher requires one shared argument array".into());
        };
        if argument.argument_index() != 0
            || argument.access() != terminal_psi::StructuralAccess::SharedBorrow
        {
            return Err("launcher argument array lost its borrowed access".into());
        }
        let [outer] = argument.values() else {
            return Err("launcher argument array has no retained backing".into());
        };
        let TerminalStorageContents::Elements { length, values } = &outer.contents else {
            return Err("launcher argument array has no element view".into());
        };
        if !outer.path.is_empty() || *length != values.len() {
            return Err("launcher argument array lost its checked extent".into());
        }
        if *length > 4096 {
            return shape.finish(RunStatus::StartFailed);
        }
        let mut arguments = Vec::with_capacity(*length);
        let mut bytes_left = 4 << 20;
        for (index, value) in values.iter().enumerate() {
            let TerminalStorageContents::Bytes(bytes) = &value.contents else {
                return Err("launcher argument requires a byte view".into());
            };
            if value.path.as_slice() != [Path::FixedIndex(index as u64)] {
                return Err("launcher argument lost its array position".into());
            }
            let Some(remaining) = usize::checked_sub(bytes_left, bytes.len()) else {
                return shape.finish(RunStatus::StartFailed);
            };
            bytes_left = remaining;
            arguments.push(bytes.to_vec());
        }
        let mut files = bounded_process::PrivateFilesystemFiles {
            initial: Default::default(),
            outputs: binding.meaning.writable.clone(),
        };
        if !binding.meaning.readable.is_empty() {
            filesystem.with_program_files(|runtime| {
                let mut remaining = 64 << 20;
                for path in &binding.meaning.readable {
                    let bytes = read_input(runtime, path, remaining)?;
                    remaining -= bytes.len();
                    files.initial.insert(path.clone(), bytes.into());
                }
                Ok(())
            })?;
        }
        let timeout = Duration::from_millis(binding.meaning.timeout_ms);
        let output = binding
            .image
            .run(worker, &arguments, files, timeout)
            .map_err(|e| e.to_string())?;
        if !output.outputs.is_empty() {
            if output.outputs.len() != binding.meaning.writable.len() {
                return Err("lost program output grant coordinates".into());
            }
            filesystem.with_program_files(|runtime| {
                for (path, bytes) in binding.meaning.writable.iter().zip(output.outputs) {
                    if let Some(bytes) = bytes {
                        write_output(runtime, path, &bytes)?;
                    }
                }
                Ok(())
            })?;
        }
        shape.finish(output.status)
    }
}

fn read_input(
    runtime: &mut package_compilation::build_execution::BuildFilesystemRuntime,
    path: &[u8],
    limit: usize,
) -> Result<Vec<u8>, String> {
    use package_compilation::build_execution::BUILD_SOURCE_ROOT_IDENTITY as ROOT;
    let fd = runtime.open(ROOT, path, 0)?;
    if fd < 0 {
        return Err(format!(
            "program cannot read logical input `{}`",
            String::from_utf8_lossy(path)
        ));
    }
    let captured = (|| {
        let mut bytes = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            let count = runtime.read(ROOT, fd, &mut chunk, 8192)?;
            let count = usize::try_from(count).map_err(|_| "program input read failed")?;
            if count == 0 {
                return Ok(bytes);
            }
            if count > limit - bytes.len() {
                return Err("program inputs exceed their grant budget".into());
            }
            bytes.extend_from_slice(chunk.get(..count).ok_or("invalid program input count")?);
        }
    })();
    if runtime.close(ROOT, fd)? < 0 {
        return Err("program input close failed".into());
    }
    captured
}

fn write_output(
    runtime: &mut package_compilation::build_execution::BuildFilesystemRuntime,
    path: &[u8],
    bytes: &[u8],
) -> Result<(), String> {
    use package_compilation::build_execution::BUILD_OUTPUT_ROOT_IDENTITY as ROOT;
    let fd = runtime.create(path, 0o600)?;
    if fd < 0 {
        return Err(format!(
            "program cannot create logical output `{}`",
            String::from_utf8_lossy(path)
        ));
    }
    let written = runtime.write(fd, bytes);
    let closed = runtime.close(ROOT, fd)?;
    if usize::try_from(written?).ok() != Some(bytes.len()) || closed < 0 {
        return Err("program output could not be staged in full".into());
    }
    Ok(())
}
