//! Resolve invoker selectors against reviewed data declarations before Build
//! entry. An effect presents its checked call site and that declaration's
//! coordinate; guest strings can never select an image.
use build_program_launch::RunStatus;
use package_compilation::build_execution::DelegatedProgram;
use package_compilation::build_execution::{RestrictedBuildBounds, RestrictedBuildRequest};
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
mod files;
use package_compilation::ProgramOutputSink;

struct ProgramBinding {
    source: SourceSpan,
    grant: DelegatedProgram,
    review_owner: package_compilation::ProgramDeclarationOwner,
}

#[derive(Default)]
pub(crate) struct ProgramInstallation {
    worker: Option<PathBuf>,
    bindings: Vec<ProgramBinding>,
    runtime_cases: Option<ResultShape>,
}

impl ProgramInstallation {
    pub(crate) fn demanded(&self) -> impl Iterator<Item = &DelegatedProgram> {
        self.bindings.iter().map(|binding| &binding.grant)
    }

    pub(crate) fn requests(
        &self,
        bounds: &RestrictedBuildBounds,
        profile: Option<target::TargetProfile>,
    ) -> impl Iterator<Item = RestrictedBuildRequest> {
        self.bindings.iter().map(move |binding| {
            let mut meaning = binding.grant.meaning.clone();
            meaning.owner = binding.review_owner;
            RestrictedBuildRequest::for_program(meaning, bounds.clone(), profile)
        })
    }

    pub(crate) fn delegated(
        &self,
        program: SourceSpan,
        filesystem: &std::sync::Arc<super::filesystem::TerminalBuildFilesystemHost>,
    ) -> Result<DelegatedProgram, String> {
        let mut grant = self
            .bindings
            .iter()
            .find(|binding| binding.source == program)
            .ok_or("the forwarded program has no installed authority")?
            .grant
            .clone();
        if grant.files.is_none()
            && (!grant.meaning.readable.is_empty() || !grant.meaning.writable.is_empty())
        {
            grant.files = Some(filesystem.forward_program_files(&grant.meaning)?);
        }
        Ok(grant)
    }

    pub(crate) fn uses_files(&self) -> bool {
        self.bindings.iter().any(|binding| {
            binding.grant.files.is_none()
                && (!binding.grant.meaning.readable.is_empty()
                    || !binding.grant.meaning.writable.is_empty())
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
                    vec![Path::Field(self.status)],
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
        let grant = &binding.grant;
        let initial = match &grant.files {
            Some(files) => files.initial.clone(),
            None => filesystem.initial_program_files(&grant.meaning.readable)?,
        };
        let files = bounded_process::PrivateFilesystemFiles {
            initial,
            outputs: grant.meaning.writable.clone(),
        };
        let timeout = Duration::from_millis(binding.grant.meaning.timeout_ms);
        let output = binding
            .grant
            .image
            .run(worker, &arguments, files, timeout)
            .map_err(|e| e.to_string())?;
        if !output.outputs.is_empty() {
            match &grant.files {
                Some(files) => files
                    .output
                    .as_ref()
                    .ok_or("program returned outputs without a staging grant")?
                    .write(output.outputs)?,
                None => filesystem.write(&grant.meaning.writable, output.outputs)?,
            }
        }
        shape.finish(output.status)
    }
}
