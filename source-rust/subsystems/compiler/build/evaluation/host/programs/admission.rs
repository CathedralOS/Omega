//! Resolve nominal program selectors and seal the executable bindings for one Build.
use super::{ProgramBinding, ProgramInstallation, ResultShape};
use build_program_launch::ReviewedProgram;
use diagnostics::Diagnostic;
use package_compilation::build_execution::{
    BuildProgramBinding, BuildProgramInvocation, ProgramDeclarationOwner, RestrictedProgramLaunch,
};
use source_compiler::publication::{ProducedTerminalProgram, TerminalBuildRequestOperation};
use source_compiler::representations::source_state::TypedTrees;
use source_text::SourceSpan;
use std::{sync::Arc, time::Duration};

impl ProgramInstallation {
    pub(crate) fn admit(
        source: &TypedTrees,
        artifact: &ProducedTerminalProgram,
        packages: Option<&package_compilation::PackageCompilationInputs>,
        invocation: &BuildProgramInvocation,
        (entry, entry_span): (&str, SourceSpan),
        filesystem: &package_compilation::BuildMachineFilesystemScope,
        imports: &[semantic_vocabulary::BoundaryMachineId],
    ) -> Result<Arc<Self>, Vec<Diagnostic>> {
        let refused = |why: String| {
            vec![Diagnostic::error(format!(
                "Build `{entry}` ProgramLaunch: {why}"
            ))]
        };
        let entry_file = source
            .symbols
            .source_file(entry_span)
            .ok_or_else(|| refused("missing requesting source occurrence".into()))?;
        let view = artifact.view();
        let calls = artifact
            .build_requests()
            .calls_for(view)
            .ok_or_else(|| refused("lost the program demand source".into()))?;
        let mut demands = Vec::new();
        let mut result = None;
        for call in calls
            .iter()
            .filter(|call| imports.contains(&call.site.boundary))
        {
            let TerminalBuildRequestOperation::LaunchProgram { program } = call.action else {
                continue;
            };
            if !invocation.allow {
                return Err(vec![Diagnostic::error(format!("Build `{entry}` requests ProgramLaunch without the root --allow ProgramLaunch ceiling")).with_source_span(program)]);
            }
            if !demands.contains(&program) {
                demands.push(program);
            }
            let boundary = view
                .module()
                .boundary_machine(call.site.boundary)
                .ok_or_else(|| refused("missing launcher signature".into()))?;
            let terminal_psi::BoundaryMachineResult::Structural(result_type) = &boundary.result
            else {
                return Err(refused("invalid launcher result".into()));
            };
            let candidate =
                ResultShape::new(view.module(), result_type.structural_type).map_err(refused)?;
            if let Some(previous) = result.as_ref()
                && previous != &candidate
            {
                return Err(refused("inconsistent launcher result declarations".into()));
            }
            result = Some(candidate);
        }

        let mut selected = Vec::new();
        for binding in &invocation.bindings {
            if !invocation.allow {
                return Err(refused(
                    "an executable binding needs the ProgramLaunch ceiling".into(),
                ));
            }
            let mut declarations = source.data_definitions().iter().filter_map(|data| {
                if data.generic_instance.is_some() {
                    return None;
                }
                let span = source.symbols.symbol_source_span(data.symbol)?;
                let file = source.symbols.source_file(span)?;
                let declaration = source.symbols.display_path(data.symbol, "::");
                let at_root = !file.origin.is_toolchain()
                    && file.package_root == entry_file.package_root
                    && file.package_identity == entry_file.package_identity;
                let package = file
                    .package_identity
                    .and_then(|owner| packages?.package_name(owner))
                    .or_else(|| {
                        at_root
                            .then_some(invocation.reviewed_root_name.as_deref())
                            .flatten()
                    });
                let qualified = package.map(|name| format!("{name}::{declaration}"));
                (binding.selector == declaration || qualified.as_ref() == Some(&binding.selector))
                    .then_some((span, file, declaration, at_root))
            });
            let (span, file, declaration, at_root) = declarations.next().ok_or_else(|| {
                refused(format!(
                    "unknown program declaration `{}`",
                    binding.selector
                ))
            })?;
            if declarations.next().is_some() {
                return Err(refused(format!(
                    "ambiguous program declaration `{}`",
                    binding.selector
                )));
            }
            if selected.iter().any(|b: &ProgramBinding| b.source == span) {
                return Err(refused(format!("duplicate binding for `{declaration}`")));
            }
            let grant = ProgramBinding::capture(binding, span, file, declaration, at_root)
                .map_err(refused)?;
            selected.push(grant);
        }
        for demand in &demands {
            if !selected.iter().any(|binding| binding.source == *demand) {
                let name = source
                    .data_definitions()
                    .iter()
                    .find(|data| source.symbols.symbol_source_span(data.symbol) == Some(*demand))
                    .map(|data| source.symbols.display_path(data.symbol, "::"))
                    .ok_or_else(|| refused("requested program lost its declaration".into()))?;
                return Err(vec![Diagnostic::error(format!(
                    "Build `{entry}` lacks an executable binding for ProgramLaunch declaration `{name}`"
                )).with_source_span(*demand)]);
            }
        }
        selected.retain(|binding| demands.contains(&binding.source));
        for binding in &selected {
            filesystem
                .admit_program_files(&binding.meaning.readable, &binding.meaning.writable)
                .map_err(|why| {
                    refused(format!("program `{}`: {why}", binding.meaning.declaration))
                })?;
        }
        if !selected.is_empty() {
            let worker = invocation
                .worker
                .as_deref()
                .ok_or_else(|| refused("no program supervisor was installed".into()))?;
            build_program_launch::require_host(worker).map_err(|e| refused(e.to_string()))?;
        }
        Ok(Arc::new(Self {
            worker: invocation.worker.clone(),
            bindings: selected,
            runtime_cases: result,
        }))
    }
}

impl ProgramBinding {
    fn capture(
        binding: &BuildProgramBinding,
        span: SourceSpan,
        file: &source_admission::SourceFile,
        declaration: String,
        at_root: bool,
    ) -> Result<Self, String> {
        let relative = file.logical_filename()?;
        for path in binding.read.iter().chain(&binding.write) {
            if !filesystem_authority::canonical_filesystem_metadata_path_is_canonical(path, false) {
                return Err(format!(
                    "program `{declaration}` has an invalid logical resource path"
                ));
            }
        }
        let timeout = binding.timeout.unwrap_or(Duration::from_secs(60));
        let millis = u64::try_from(timeout.as_millis()).ok().filter(|n| (1..=300_000).contains(n))
                .ok_or_else(|| format!("program `{declaration}` timeout is outside the host's 1..300000 millisecond budget"))?;
        let image = ReviewedProgram::capture(&binding.executable)
            .map_err(|e| format!("cannot bind `{declaration}`: {e}"))?;
        let mut readable = binding.read.clone();
        let mut writable = binding.write.clone();
        readable.sort();
        writable.sort();
        if readable.windows(2).any(|p| p[0] == p[1]) || writable.windows(2).any(|p| p[0] == p[1]) {
            return Err(format!("duplicate resource for `{declaration}`"));
        }
        let owner = if at_root {
            ProgramDeclarationOwner::RequestingSource
        } else if let Some(owner) = file.package_identity {
            ProgramDeclarationOwner::Package(owner.digest())
        } else if file.origin.is_toolchain() {
            ProgramDeclarationOwner::Toolchain(
                source_admission::source_identity::toolchain_source_identity_digest(file)?,
            )
        } else {
            return Err(format!("program `{declaration}` has no reviewed owner"));
        };
        let meaning = RestrictedProgramLaunch {
            owner,
            declaration,
            file: relative,
            image_sha256: image.image_sha256(),
            timeout_ms: millis,
            readable,
            writable,
        };
        Ok(Self {
            source: span,
            meaning,
            image,
        })
    }
}
