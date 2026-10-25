//! Retained source and frontend products, without source loading or stage execution.
//!
//! A child either reuses an exact typed frontier after live custody validation,
//! or receives the parsed sources and an optional retention slot. The coordinator
//! owns package assembly while Psi owns resolution and typing; publication into
//! the slot occurs only after preparation succeeds. Neither product grants authority.

use artifacts::compile_timings::CompileTimings;
use diagnostics::Diagnostic;
use package_compilation::source_assembly::ImmutableSourceParseCheckpoint;
use package_compilation::{PackageCompilationInputs, SourceAssemblyInputs};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Indexed declarations and completed Build-context bodies before admission.
/// With authored Build, Product imports remain unopened under captured custody;
/// otherwise the default-Build route retains pending Product implementations.
/// This is a reusable compilation checkpoint, not a portable program form.
#[derive(Clone)]
pub struct TypedCompilationFrontier {
    pub source: psi_compiler::SourceContinuation,
    /// Product imports and declarations are prepared only after Build's join.
    /// No checked identity or initial name-resolution context is rewritten.
    pub product_sources: Option<package_compilation::source_assembly::DeferredProductSources>,
    pub build_source_id: Option<source::SourceId>,
    pub source_file_count: usize,
    pub generated_source_custody: Vec<(source::SourceId, build_output::PackageGeneratedSource)>,
    pub base_sources: Arc<source::SourceMap>,
}

/// Immutable parsing shared by exact checked children. Forks share storage;
/// consuming the last checkpoint allows assembly to move the original arenas.
/// Every child supplies its own policy and independently admits its custody.
pub struct PreparedCheckedSource {
    source_checkpoint: ImmutableSourceParseCheckpoint,
    shared_timings: CompileTimings,
    frontiers: Arc<Mutex<Vec<(SourceAssemblyInputs, TypedCompilationFrontier)>>>,
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
    inputs: SourceAssemblyInputs,
    frontiers: Arc<Mutex<Vec<(SourceAssemblyInputs, TypedCompilationFrontier)>>>,
}

impl PreparedCheckedSource {
    /// Fork only at a real sibling boundary, freezing shared source pages once.
    pub fn fork(&mut self) -> Self {
        Self {
            source_checkpoint: self.source_checkpoint.fork(),
            shared_timings: self.shared_timings.clone(),
            frontiers: self.frontiers.clone(),
        }
    }

    pub fn from_parsed(
        source_checkpoint: ImmutableSourceParseCheckpoint,
        shared_timings: CompileTimings,
    ) -> Self {
        Self {
            source_checkpoint,
            shared_timings,
            frontiers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn admit_root(&self, root_path: &Path) -> Result<(), Vec<Diagnostic>> {
        if root_path != self.source_checkpoint.root_path() {
            return Err(vec![Diagnostic::error(
                "checked child compilation root does not match its prepared source checkpoint",
            )]);
        }
        Ok(())
    }

    /// Select by the package owner's complete assembly inputs. Even an exact
    /// hit revalidates live package custody before any Build effects can run.
    /// The final consumer retains no new frontend copy for nonexistent siblings.
    pub fn into_frontier(
        self,
        package_inputs: Option<&PackageCompilationInputs>,
        build_execution_profile: Option<target::TargetProfile>,
    ) -> Result<PreparedSourceFrontier, Vec<Diagnostic>> {
        let inputs = SourceAssemblyInputs::new(package_inputs, build_execution_profile);
        let retain = Arc::strong_count(&self.frontiers) > 1;
        let retained = self
            .frontiers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .iter()
            .find(|(stored, _)| *stored == inputs)
            .map(|(_, frontier)| frontier.clone());
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
