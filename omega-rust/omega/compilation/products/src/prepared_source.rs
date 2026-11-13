// Retained source and frontend products (`SourcePreparation`,
// `TypedCompilationFrontier`, `DeferredProductCompilation`, and
// `PreparedCheckedSource`). A child either reuses an exact typed frontier after
// live package custody validation or receives the parsed sources and an
// optional retention slot; publication into the slot occurs only after
// preparation succeeds.

use artifacts::compile_timings::CompileTimings;
use diagnostics::Diagnostic;
use package_compilation::source_assembly::{
    DeferredProductSources, ImmutableSourceParseCheckpoint,
};
use package_compilation::{PackageCompilationInputs, ProductSourceInputs, SourceAssemblyInputs};
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct SourcePreparation {
    catalogue: Arc<Mutex<source::SourceCatalogue>>,
    pub frontends: psi_compiler::SourceFrontends,
}

impl SourcePreparation {
    pub fn catalogue(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, source::SourceCatalogue>, Vec<Diagnostic>> {
        self.catalogue
            .lock()
            .map_err(|_| vec![Diagnostic::error("source catalogue owner is poisoned")])
    }
}

#[derive(Clone)]
pub struct TypedCompilationFrontier {
    pub preparation: SourcePreparation,
    pub source: psi_compiler::SourceContinuation,
    pub anonymous_source: Option<package_compilation::AnonymousSourceCapture>,
    pub product_sources: Option<DeferredProductCompilation>,
    pub build_source: Option<package_compilation::BuildSourceSelection>,
    pub source_file_count: usize,
    pub generated_source_custody: Vec<(source::SourceId, build_output::PackageGeneratedSource)>,
    pub base_sources: Arc<source::SourceMap>,
}

#[derive(Clone)]
pub struct DeferredProductCompilation {
    sources: DeferredProductSources,
    frontiers: Arc<Mutex<Vec<(ProductSourceInputs, TypedCompilationFrontier)>>>,
}

impl DeferredProductCompilation {
    pub fn new(sources: DeferredProductSources) -> Self {
        Self {
            sources,
            frontiers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn retained(
        &self,
        inputs: &ProductSourceInputs,
        packages: Option<&PackageCompilationInputs>,
    ) -> Result<Option<TypedCompilationFrontier>, Vec<Diagnostic>> {
        let frontier = self
            .frontiers
            .lock()
            .map_err(|_| {
                vec![Diagnostic::error(
                    "deferred Product frontend retention is poisoned",
                )]
            })?
            .iter()
            .find(|(stored, _)| stored == inputs)
            .map(|(_, frontier)| frontier.clone());
        let frontier = frontier.and_then(|frontier| frontier.rejoin_current_authority(packages));
        if frontier.is_some() {
            self.sources.admit_inputs(packages)?;
        }
        Ok(frontier)
    }

    pub fn sources(&self) -> DeferredProductSources {
        self.sources.clone()
    }

    pub fn retain(
        &self,
        inputs: ProductSourceInputs,
        frontier: &TypedCompilationFrontier,
    ) -> Result<(), Vec<Diagnostic>> {
        if Arc::strong_count(&self.frontiers) == 1 {
            return Ok(());
        }
        let mut frontiers = self.frontiers.lock().map_err(|_| {
            vec![Diagnostic::error(
                "deferred Product frontend retention is poisoned",
            )]
        })?;
        frontiers.retain(|(stored, _)| *stored != inputs);
        frontiers.push((inputs, frontier.clone()));
        Ok(())
    }
}

pub struct PreparedCheckedSource {
    preparation: SourcePreparation,
    source_checkpoint: ImmutableSourceParseCheckpoint,
    shared_timings: CompileTimings,
    toolchain_capture: Option<package_compilation::CapturedLocalSource>,
    frontiers: Arc<Mutex<Vec<(SourceFrontierInputs, TypedCompilationFrontier)>>>,
}

#[derive(Clone, PartialEq, Eq)]
struct SourceFrontierInputs {
    assembly: SourceAssemblyInputs,
    // A retained frontier is valid only for the exact core bytes it typed.
    toolchain_capture: Option<package_compilation::CapturedLocalSource>,
}

pub enum PreparedSourceFrontier {
    Retained(TypedCompilationFrontier, CompileTimings),
    Pending {
        sources: ImmutableSourceParseCheckpoint,
        timings: CompileTimings,
        retention: Option<TypedFrontierRetention>,
    },
}

pub struct TypedFrontierRetention {
    inputs: SourceFrontierInputs,
    frontiers: Arc<Mutex<Vec<(SourceFrontierInputs, TypedCompilationFrontier)>>>,
}

impl PreparedCheckedSource {
    pub fn preparation(&self) -> SourcePreparation {
        self.preparation.clone()
    }

    pub fn fork(&mut self) -> Self {
        Self {
            preparation: self.preparation.clone(),
            source_checkpoint: self.source_checkpoint.fork(),
            shared_timings: self.shared_timings.clone(),
            toolchain_capture: self.toolchain_capture.clone(),
            frontiers: self.frontiers.clone(),
        }
    }

    pub fn from_parsed(
        source_checkpoint: ImmutableSourceParseCheckpoint,
        shared_timings: CompileTimings,
        preparation: SourcePreparation,
    ) -> Self {
        Self {
            preparation,
            source_checkpoint,
            shared_timings,
            toolchain_capture: None,
            frontiers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn with_toolchain_capture(
        mut self,
        capture: package_compilation::CapturedLocalSource,
    ) -> Result<Self, Vec<Diagnostic>> {
        self.source_checkpoint = self.source_checkpoint.with_toolchain_capture(&capture)?;
        self.toolchain_capture = Some(capture);
        Ok(self)
    }

    pub fn admit_root(&self, root_path: &Path) -> Result<(), Vec<Diagnostic>> {
        self.source_checkpoint.admit_root(root_path)
    }

    pub fn into_frontier(
        self,
        package_inputs: Option<&PackageCompilationInputs>,
        build_execution_profile: Option<target::TargetProfile>,
    ) -> Result<PreparedSourceFrontier, Vec<Diagnostic>> {
        let inputs = SourceFrontierInputs {
            assembly: SourceAssemblyInputs::new(package_inputs, build_execution_profile),
            toolchain_capture: self.toolchain_capture,
        };
        let retain = Arc::strong_count(&self.frontiers) > 1;
        let retained = self
            .frontiers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .iter()
            .find(|(stored, _)| *stored == inputs)
            .map(|(_, frontier)| frontier.clone());
        let retained =
            retained.and_then(|frontier| frontier.rejoin_current_authority(package_inputs));
        if let Some(frontier) = retained {
            if let Some(packages) = package_inputs {
                packages.validate_for_compilation(
                    self.source_checkpoint.root_path(),
                    &package_compilation::source_imports::bundled_core_root()?,
                )?;
            }
            return Ok(PreparedSourceFrontier::Retained(
                frontier,
                self.shared_timings,
            ));
        }
        Ok(PreparedSourceFrontier::Pending {
            sources: self.source_checkpoint,
            timings: self.shared_timings,
            retention: retain.then_some(TypedFrontierRetention {
                inputs,
                frontiers: self.frontiers,
            }),
        })
    }
}

impl TypedFrontierRetention {
    pub fn publish(self, frontier: &TypedCompilationFrontier) {
        let mut entries = self
            .frontiers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        entries.retain(|(stored, _)| *stored != self.inputs);
        entries.push((self.inputs, frontier.clone()));
    }
}

impl TypedCompilationFrontier {
    fn rejoin_current_authority(self, packages: Option<&PackageCompilationInputs>) -> Option<Self> {
        let authority = packages.map(|packages| {
            Arc::new(packages.clone())
                as Arc<dyn build_time_evaluation::BuildTimeSelectionAuthority>
        });
        match self.source.rejoin_current_authority(authority) {
            Ok(source) => Some(Self { source, ..self }),
            Err(_) => None,
        }
    }
}
