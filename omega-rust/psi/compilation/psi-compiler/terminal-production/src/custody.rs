/*
    Retained Terminal production outputs (`ProducedTerminalProgram` and
    `PortableTerminalProgram`) and independent application judgment custody.

    We pair canonical or invocation-scoped Terminal code with its source-issued
    Build companions, machine provenance, and per-application root and
    `ProgramEntry` receipts, supporting compiler-storage record persistence and
    late application attachment without reopening source trees.
*/

use crate::TerminalBuildFilesystemCall;
use crate::{TerminalBuildBindings, TerminalBuildFilesystem, TerminalBuildRequests};
use compiler_storage::RecordWriter;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    NativeBindingFormation, ProductDeclarationRequest,
};
use std::sync::Arc;
use terminal_codec::entry_receipt::rejoin_program_entry_receipt;
use terminal_codec::{CanonicalTerminalArtifact, CanonicalTerminalArtifactError};
use terminal_psi::CheckedProgramEntryTerminalReceipt;

#[derive(Debug, PartialEq, Eq, compiler_storage::Record)]
pub struct ProducedTerminalApplication {
    pub receipt: Result<Option<CheckedProgramEntryTerminalReceipt>, Vec<Diagnostic>>,
    pub roots: Vec<Result<crate::TerminalAdmissionRoot, Vec<Diagnostic>>>,
}

#[derive(Debug, PartialEq, Eq)]
#[must_use = "Terminal production retains independent application judgments"]
pub struct ProducedTerminalProgram {
    pub(crate) program: terminal_codec::TerminalProgramProduct,
    pub(crate) build_bindings: TerminalBuildBindings,
    pub(crate) build_requests: TerminalBuildRequests,
    pub(crate) build_filesystem: TerminalBuildFilesystem,
    pub(crate) applications: Vec<Result<ProducedTerminalApplication, Vec<Diagnostic>>>,
    pub(crate) provenance: terminal_psi::TerminalMachineProvenance,
    pub(crate) source_records: Vec<checked_trees::typed_trees::SourceRecordCorrespondence>,
}

impl compiler_storage::Record for ProducedTerminalProgram {
    fn write(&self, output: &mut RecordWriter) -> Result<(), compiler_storage::RecordError> {
        output.record(|output| {
            self.artifact()
                .map_err(|error| output.error(error.to_string()))?
                .to_bytes()
                .write(output)?;
            let view = self.view();
            macro_rules! companion {
                ($value:expr) => {
                    $value
                        .ok_or_else(|| output.error("Terminal companion lost its artifact owner"))?
                        .write(output)?
                };
            }
            companion!(self.build_bindings.calls_for(view));
            companion!(self.build_bindings.declarations_for(view));
            companion!(self.build_bindings.queries_for(view));
            companion!(self.build_bindings.native_formations_for(view));
            companion!(self.build_requests.calls_for(view));
            companion!(self.build_filesystem.calls_for(view));
            self.applications.write(output)?;
            self.provenance.write(output)?;
            self.source_records.write(output)
        })
    }

    fn read<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        use compiler_storage::Record;
        input.record(|input| {
            let bytes = Vec::<u8>::read(input)?;
            let artifact = CanonicalTerminalArtifact::from_bytes(&bytes)
                .map_err(|error| input.error(error.to_string()))?;
            let calls: Arc<[crate::TerminalBuildBindingCall]> = Record::read(input)?;
            let declarations: Arc<[Arc<ProductDeclarationRequest>]> = Record::read(input)?;
            let queries: Arc<[crate::TerminalBuildQueryCall]> = Record::read(input)?;
            let native_formations: Arc<[NativeBindingFormation]> = Record::read(input)?;
            let requests: Arc<[crate::TerminalBuildRequestCall]> = Record::read(input)?;
            let filesystem: Arc<[TerminalBuildFilesystemCall]> = Record::read(input)?;
            let applications: Vec<Result<ProducedTerminalApplication, Vec<Diagnostic>>> =
                Record::read(input)?;
            let provenance = Record::read(input)?;
            let source_records = Record::read(input)?;
            for application in applications.iter().flatten() {
                if let Ok(Some(receipt)) = &application.receipt {
                    rejoin_program_entry_receipt(receipt, &artifact)
                        .map_err(|error| input.error(error.to_string()))?;
                }
                for root in application
                    .roots
                    .iter()
                    .filter_map(|root| root.as_ref().ok())
                {
                    if artifact.semantic_module().machine(root.machine).is_none() {
                        return Err(input.error("composition root lost its canonical machine"));
                    }
                }
            }
            let view = artifact.view().into();
            let build_bindings =
                TerminalBuildBindings::new(view, calls, declarations, queries, native_formations);
            let build_requests = TerminalBuildRequests::new(view, requests);
            let build_filesystem = TerminalBuildFilesystem::new(view, filesystem);
            Ok(Self {
                program: terminal_codec::TerminalProgramProduct::Portable(artifact),
                build_bindings,
                build_requests,
                build_filesystem,
                applications,
                provenance,
                source_records,
            })
        })
    }
}

impl ProducedTerminalProgram {
    pub(crate) fn from_accepted_source(
        source: &crate::CheckedInvocationSource,
    ) -> Result<Self, Vec<Diagnostic>> {
        let publication = source.publication();
        let module = publication.artifact.semantic_module();
        let companions = crate::invocation_companions::InvocationCompanions::capture(source)
            .map_err(|diagnostic| vec![diagnostic])?;
        let mut provenance = terminal_psi::TerminalMachineProvenance::default();
        for machine in &module.machines {
            let symbol = *publication.machines.get(machine.id).ok_or_else(|| {
                vec![Diagnostic::error(
                    "invocation lost a checked machine's source declaration",
                )]
            })?;
            provenance.name(
                machine.id,
                source.typed().symbols.display_path(symbol, "::"),
                source.typed().symbols.symbol_source_span(symbol),
            );
        }
        if let Some(terminal_psi::TargetMachineCallable::Machine(entry)) = module.entry {
            provenance = provenance.for_root(module, entry, None);
        }
        let code = terminal_codec::InvocationTerminalCode::from_artifact(&publication.artifact);
        let view = code.view();
        let build_bindings = TerminalBuildBindings::new(
            view,
            companions.bindings,
            companions.declarations,
            companions.queries,
            companions.native_formations,
        );
        let build_requests = TerminalBuildRequests::new(view, companions.requests);
        let build_filesystem = TerminalBuildFilesystem::new(view, companions.filesystem);
        Ok(Self {
            program: terminal_codec::TerminalProgramProduct::Invocation(code),
            build_bindings,
            build_requests,
            build_filesystem,
            applications: Vec::new(),
            provenance,
            source_records: publication.source_records.clone(),
        })
    }

    pub fn into_shared_portable(
        self,
    ) -> Result<PortableTerminalProgram, CanonicalTerminalArtifactError> {
        Ok(PortableTerminalProgram {
            artifact: Arc::new(self.program.into_portable()?),
            build_bindings: self.build_bindings,
            build_requests: self.build_requests,
            build_filesystem: self.build_filesystem,
            applications: self.applications,
            provenance: self.provenance,
            source_records: self.source_records.into(),
        })
    }
    pub const fn build_filesystem(&self) -> &TerminalBuildFilesystem {
        &self.build_filesystem
    }
    pub const fn build_requests(&self) -> &TerminalBuildRequests {
        &self.build_requests
    }
    pub const fn build_bindings(&self) -> &TerminalBuildBindings {
        &self.build_bindings
    }
    pub fn view(&self) -> terminal_codec::TerminalExecutionView<'_> {
        self.program.view()
    }

    pub fn retain_code(&self) -> terminal_codec::RetainedTerminalCode {
        self.program.retain_code()
    }

    pub fn artifact(&self) -> Result<&CanonicalTerminalArtifact, CanonicalTerminalArtifactError> {
        self.program.portable()
    }

    pub fn into_artifact(
        self,
    ) -> Result<CanonicalTerminalArtifact, CanonicalTerminalArtifactError> {
        self.program.into_portable()
    }

    pub fn into_parts(
        self,
    ) -> Result<
        (
            CanonicalTerminalArtifact,
            Vec<Result<ProducedTerminalApplication, Vec<Diagnostic>>>,
        ),
        CanonicalTerminalArtifactError,
    > {
        Ok((self.program.into_portable()?, self.applications))
    }
}

#[derive(Debug)]
pub struct PortableTerminalProgram {
    artifact: Arc<CanonicalTerminalArtifact>,
    build_bindings: TerminalBuildBindings,
    build_requests: TerminalBuildRequests,
    build_filesystem: TerminalBuildFilesystem,
    applications: Vec<Result<ProducedTerminalApplication, Vec<Diagnostic>>>,
    provenance: terminal_psi::TerminalMachineProvenance,
    source_records: Arc<[typed_trees::SourceRecordCorrespondence]>,
}

impl PortableTerminalProgram {
    /// Retain an already checked program for inspection without invoking a
    /// source producer again. Build operations need their own source-request
    /// correspondence; absence is checked against every actual boundary call
    /// before creating empty companions for an ordinary program.
    pub fn from_checked_artifact(
        artifact: Arc<CanonicalTerminalArtifact>,
        typed: &typed_trees::FrozenTypedTrees,
        boundaries: &semantic_vocabulary::IdTable<
            semantic_vocabulary::BoundaryMachineId,
            symbols::SymbolHandle,
        >,
        provenance: terminal_psi::TerminalMachineProvenance,
        source_records: Arc<[typed_trees::SourceRecordCorrespondence]>,
    ) -> Result<Self, Vec<Diagnostic>> {
        for machine in &artifact.semantic_module().machines {
            for invocation in machine.boundary_invocations() {
                let source = *boundaries.get(invocation.boundary).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "inspection lost an invoked boundary's exact source declaration",
                    )]
                })?;
                if needs_build_companions(typed, source)
                    .map_err(|reason| vec![Diagnostic::error(reason)])?
                {
                    return Err(vec![Diagnostic::error(
                        "inspection of Build operations requires their retained source-request correspondence",
                    )]);
                }
            }
        }
        let view: terminal_codec::TerminalExecutionView<'_> = artifact.view().into();
        let build_bindings = TerminalBuildBindings::new(
            view,
            Vec::<crate::TerminalBuildBindingCall>::new(),
            Vec::<Arc<ProductDeclarationRequest>>::new(),
            Vec::<crate::TerminalBuildQueryCall>::new(),
            Vec::<NativeBindingFormation>::new(),
        );
        let build_requests =
            TerminalBuildRequests::new(view, Vec::<crate::TerminalBuildRequestCall>::new());
        let build_filesystem =
            TerminalBuildFilesystem::new(view, Vec::<TerminalBuildFilesystemCall>::new());
        Ok(Self {
            artifact,
            build_bindings,
            build_requests,
            build_filesystem,
            applications: Vec::new(),
            provenance,
            source_records,
        })
    }

    pub fn source_records(&self) -> &[checked_trees::typed_trees::SourceRecordCorrespondence] {
        &self.source_records
    }
    pub fn attach_application(
        &self,
        application: &crate::TerminalApplicationCustody<'_>,
        observer: &mut impl crate::PsiCompilationObserver,
    ) -> Result<ProducedTerminalApplication, Vec<Diagnostic>> {
        application.attach(
            self.artifact.view().into(),
            Ok(&self.artifact),
            &self.provenance,
            observer,
        )
    }

    pub fn artifact(&self) -> &Arc<CanonicalTerminalArtifact> {
        &self.artifact
    }
    pub fn build_bindings(&self) -> &TerminalBuildBindings {
        &self.build_bindings
    }
    pub fn build_requests(&self) -> &TerminalBuildRequests {
        &self.build_requests
    }
    pub fn build_filesystem(&self) -> &TerminalBuildFilesystem {
        &self.build_filesystem
    }
    pub fn applications(&self) -> &[Result<ProducedTerminalApplication, Vec<Diagnostic>>] {
        &self.applications
    }
}

fn needs_build_companions(
    typed: &typed_trees::TypedTrees,
    declaration: symbols::SymbolHandle,
) -> Result<bool, &'static str> {
    let mut pending = vec![declaration];
    let mut visited = Vec::new();
    while let Some(declaration) = pending.pop() {
        if !typed.symbols.symbols().nodes().is_valid(declaration) {
            return Err("inspection boundary names a stale source declaration");
        }
        if visited.contains(&declaration) {
            continue;
        }
        visited.push(declaration);
        let source = typed
            .symbols
            .symbol_source_span(declaration)
            .and_then(|span| typed.symbols.source_file(span))
            .ok_or("inspection boundary declaration has no source custody")?;
        // The compiler installs Build protocols in this exact toolchain
        // source. A same-named user declaration grants no Build meaning.
        if source.origin == source::SourceOrigin::Toolchain
            && source.path == std::path::Path::new("<build-prelude>")
        {
            return Ok(true);
        }
        if let Some((owner, _)) = typed.trait_machine_signature(declaration) {
            let source = typed
                .symbols
                .symbol_source_span(owner.symbol)
                .and_then(|span| typed.symbols.source_file(span))
                .ok_or("inspection boundary schema has no source custody")?;
            if source.origin == source::SourceOrigin::Toolchain
                && source.path == std::path::Path::new("<build-prelude>")
            {
                return Ok(true);
            }
            continue;
        }
        let owner = match typed.symbols.get(declaration).kind {
            symbols::SymbolKind::State => typed.symbols.get(declaration).parent,
            _ => declaration,
        };
        let machine = typed
            .machine_by_symbol(owner)
            .ok_or("inspection boundary has no exact source machine or requirement")?;
        pending.extend(
            typed
                .machine_trait_conformances(machine)
                .iter()
                .map(|conformance| conformance.requirement_symbol),
        );
    }
    Ok(false)
}

#[derive(Clone, Copy, Default)]
pub struct TerminalProductionCustody<'a> {
    pub applications: &'a [crate::TerminalApplicationCustody<'a>],
}
