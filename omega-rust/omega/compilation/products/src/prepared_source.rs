//! Retained source and frontend products, without source loading or stage execution.
//!
//! A child either reuses an exact typed frontier after live custody validation,
//! or receives the parsed sources and an optional retention slot. The coordinator
//! owns package assembly while Psi owns resolution and typing; publication into
//! the slot occurs only after preparation succeeds. Neither product grants authority.

use artifacts::compile_timings::CompileTimings;
use diagnostics::Diagnostic;
use package_compilation::source_assembly::{
    DeferredProductSources, ImmutableSourceParseCheckpoint,
};
use package_compilation::{PackageCompilationInputs, ProductSourceInputs, SourceAssemblyInputs};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// One invocation owns source coordinates and canonical Psi frontend inputs.
/// The coordinator borrows coordinates only during admission; no execution or
/// semantic stage runs while the catalogue is locked.
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

    /// A private copy for preparation that may be thrown away. Its admissions
    /// stay invisible here unless [`Self::adopt`] accepts them.
    pub fn fork(&self) -> Result<PreparationFork, Vec<Diagnostic>> {
        let catalogue = self.catalogue()?;
        let frontends = self.frontends.retained_count()?;
        Ok(PreparationFork {
            from: (catalogue.len(), frontends),
            preparation: SourcePreparation {
                catalogue: Arc::new(Mutex::new(catalogue.fork())),
                frontends: self.frontends.fork()?,
            },
        })
    }

    /// Take `fork`'s admissions as this preparation's own when nothing was
    /// admitted here since the fork, so the result is the state sequential
    /// preparation would have reached. Otherwise leave this one untouched and
    /// return false. Admission only appends, so equal counts mean no admission.
    pub fn adopt(&self, fork: &PreparationFork) -> Result<bool, Vec<Diagnostic>> {
        let mut catalogue = self.catalogue()?;
        if (catalogue.len(), self.frontends.retained_count()?) != fork.from {
            return Ok(false);
        }
        *catalogue = fork.preparation.catalogue()?.fork();
        self.frontends.replace_with(&fork.preparation.frontends)?;
        Ok(true)
    }
}

/// A [`SourcePreparation`] copy and the admission counts it was taken at.
pub struct PreparationFork {
    from: (usize, usize),
    pub preparation: SourcePreparation,
}

/// Indexed declarations and completed Build-context bodies before admission.
/// Product imports remain unopened under captured custody while the selected
/// authored or compiler-default Build executes.
/// This is a reusable compilation checkpoint, not a portable program form.
#[derive(Clone)]
pub struct TypedCompilationFrontier {
    pub preparation: SourcePreparation,
    pub source: psi_compiler::SourceContinuation,
    /// Acquired source custody survives frontend preparation. Build uses the
    /// same inventory before its executed declaration has a package owner.
    pub anonymous_source: Option<package_compilation::AnonymousSourceCapture>,
    /// Product imports and declarations are prepared only after Build's join.
    /// No checked identity or initial name-resolution context is rewritten.
    pub product_sources: Option<DeferredProductCompilation>,
    pub build_source: Option<package_compilation::BuildSourceSelection>,
    pub source_file_count: usize,
    pub generated_source_custody: Vec<(source::SourceId, build_output::PackageGeneratedSource)>,
    pub base_sources: Arc<source::SourceMap>,
}

/// Siblings of one captured Build prefix share its deferred Product preparation.
/// Only the frontend is retained: Build returns, charges and selections are not.
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

    /// Publish only successful preparation, never a partially typed judgment.
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

/// Immutable parsing shared by exact checked children. Forks share storage;
/// consuming the last checkpoint allows assembly to move the original arenas.
/// Every child supplies its own policy and independently admits its custody.
pub struct PreparedCheckedSource {
    preparation: SourcePreparation,
    source_checkpoint: ImmutableSourceParseCheckpoint,
    shared_timings: CompileTimings,
    toolchain_defaults: Option<package_compilation::CompletedProviderDefaults>,
    frontiers: Arc<Mutex<Vec<(SourceFrontierInputs, TypedCompilationFrontier)>>>,
}

/// A changed completed supplier must not reuse an earlier declaration view.
/// Equal suppliers retain the original frontend coordinates instead of
/// rebuilding them after Product preparation has extended the catalogue.
#[derive(Clone, PartialEq, Eq)]
struct SourceFrontierInputs {
    assembly: SourceAssemblyInputs,
    toolchain_defaults: Option<package_compilation::CompletedProviderDefaults>,
}

/// Reuse is a product-selection decision, not a hidden frontend execution path.
pub enum PreparedSourceFrontier {
    Retained(TypedCompilationFrontier, CompileTimings),
    Pending {
        sources: ImmutableSourceParseCheckpoint,
        timings: CompileTimings,
        retention: Option<TypedFrontierRetention>,
    },
}

/// One pending child's exact retention slot. No callback or compiler entrypoint
/// is stored here; the coordinator supplies the completed frontend product.
pub struct TypedFrontierRetention {
    inputs: SourceFrontierInputs,
    frontiers: Arc<Mutex<Vec<(SourceFrontierInputs, TypedCompilationFrontier)>>>,
}

impl PreparedCheckedSource {
    pub fn preparation(&self) -> SourcePreparation {
        self.preparation.clone()
    }
    /// Fork only at a real sibling boundary, freezing shared source pages once.
    pub fn fork(&mut self) -> Self {
        Self {
            preparation: self.preparation.clone(),
            source_checkpoint: self.source_checkpoint.fork(),
            shared_timings: self.shared_timings.clone(),
            toolchain_defaults: self.toolchain_defaults.clone(),
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
            toolchain_defaults: None,
            frontiers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// A new default supplier changes the source inputs. Keep parsed pages,
    /// but do not reuse a typed frontier prepared before these roots existed.
    pub fn with_toolchain_defaults(
        mut self,
        defaults: &package_compilation::CompletedProviderDefaults,
    ) -> Result<Self, Vec<Diagnostic>> {
        self.source_checkpoint = self.source_checkpoint.with_toolchain_defaults(defaults)?;
        self.toolchain_defaults = Some(defaults.clone());
        Ok(self)
    }

    pub fn admit_root(&self, root_path: &Path) -> Result<(), Vec<Diagnostic>> {
        self.source_checkpoint.admit_root(root_path)
    }

    /// Select by the package owner's complete assembly inputs. Even an exact
    /// hit revalidates live package custody before any Build effects can run.
    /// The final consumer retains no new frontend copy for nonexistent siblings.
    pub fn into_frontier(
        self,
        package_inputs: Option<&PackageCompilationInputs>,
        build_execution_profile: Option<target::TargetProfile>,
    ) -> Result<PreparedSourceFrontier, Vec<Diagnostic>> {
        let inputs = SourceFrontierInputs {
            assembly: SourceAssemblyInputs::new(package_inputs, build_execution_profile),
            toolchain_defaults: self.toolchain_defaults,
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
    /// Retain only a successfully produced frontend under its exact input key.
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
            Err(reason) => {
                phase_profile::count("source.frontier.authority_misses", 1);
                if phase_profile::enabled() {
                    eprintln!("retained source frontier ignored: {reason}");
                }
                None
            }
        }
    }
}
