//! Forward only a program's original input bytes and its live output staging area.
use super::super::filesystem::TerminalBuildFilesystemHost;
use package_compilation::{
    ForwardedProgramFiles, ProgramOutputBacking, ProgramOutputSink, RestrictedProgramLaunch,
};
use std::{collections::BTreeMap, sync::Arc};

impl TerminalBuildFilesystemHost {
    pub(super) fn initial_program_files(
        &self,
        paths: &[Vec<u8>],
    ) -> Result<BTreeMap<Vec<u8>, Arc<[u8]>>, String> {
        if paths.is_empty() {
            return Ok(BTreeMap::new());
        }
        self.with_program_files(|runtime| {
            let mut remaining = 64 << 20;
            let mut files = BTreeMap::new();
            for path in paths {
                let bytes = read_input(runtime, path, remaining)?;
                remaining -= bytes.len();
                files.insert(path.clone(), bytes.into());
            }
            Ok(files)
        })
    }

    pub(super) fn forward_program_files(
        self: &Arc<Self>,
        grant: &RestrictedProgramLaunch,
    ) -> Result<Arc<ForwardedProgramFiles>, String> {
        let initial = self.initial_program_files(&grant.readable)?;
        let bytes = initial.values().map(|file| file.len() as u64).sum();
        let storage = self
            .program_file_account()
            .reserve_storage(initial.len() as u64, bytes)
            .map_err(|why| format!("cannot retain delegated program inputs: {why:?}"))?;
        let output = if grant.writable.is_empty() {
            None
        } else {
            self.require_live()?;
            Some(ProgramOutputBacking::new(
                Arc::clone(self) as Arc<dyn ProgramOutputSink>,
                grant.writable.clone(),
            ))
        };
        Ok(Arc::new(ForwardedProgramFiles {
            initial,
            output,
            storage,
        }))
    }
}

impl ProgramOutputSink for TerminalBuildFilesystemHost {
    fn require_live(&self) -> Result<(), String> {
        self.with_program_files(|_| Ok(())).map_err(|why| {
            format!("program output's original staging activation is unavailable: {why}")
        })
    }

    fn write(&self, paths: &[Vec<u8>], files: Vec<Option<Vec<u8>>>) -> Result<(), String> {
        if files.len() != paths.len() {
            return Err("lost program output grant coordinates".into());
        }
        self.with_program_files(|runtime| {
            for (path, file) in paths.iter().zip(files) {
                if let Some(bytes) = file {
                    runtime.write_staged_file(path, &bytes)?.map_err(|error| format!(
                        "program cannot create logical output `{}`: {error:?}", String::from_utf8_lossy(path)
                    ))?;
                }
            }
            Ok(())
        })
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
